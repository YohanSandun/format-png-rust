//! Converts decoded pixels to a [`PixelFormat`].
//!
//! Conversion rules:
//! - Samples under 8 bits are scaled to the full range: 1-bit 1 becomes 255,
//!   2-bit values are multiplied by 85 and 4-bit values by 17.
//! - 16-bit samples keep their high byte.
//! - Grayscale is copied to red, green and blue.
//! - Images without alpha get alpha 255 in [`PixelFormat::Rgba8`];
//!   [`PixelFormat::Rgb8`] drops alpha.
//! - Indexed images fail with [`Error::MissingPalette`] until `PLTE` is read.

use crate::ColorType;
use crate::error::Error;
use crate::png::{ImageHeader, PixelFormat};

/// What a conversion reads from. Borrowed so decoded pixels aren't copied.
///
/// When `PLTE` and `tRNS` are supported, their parsed contents go here as
/// `palette` and `transparency` fields, without changing any signature below.
pub(crate) struct Source<'a> {
    pub(crate) header: &'a ImageHeader,
    /// Bytes per row of `data`, as `ImageHeader::stride` gives.
    pub(crate) stride: usize,
    /// Decoded pixels in the PNG's own format, as `Image::data` holds them.
    pub(crate) data: &'a [u8],
}

/// Scales a sample of `bit_depth` bits (1, 2, 4, 8 or 16) to 8 bits.
pub(crate) fn scale_to_8(value: u16, bit_depth: u8) -> u8 {
    match bit_depth {
        16 => (value >> 8) as u8,
        8 => value as u8,
        // 1, 2 and 4 bits: the maximum 2^depth - 1 always divides 255 exactly,
        // so this is 255, 85 or 17 times the value, with no rounding.
        _ => (u32::from(value) * 255 / ((1 << bit_depth) - 1)) as u8,
    }
}

/// Reads sample number `index` from a row of packed samples of `bit_depth` bits.
/// Samples are counted across channels, so for 8-bit RGB, index 4 is the green
/// sample of the second pixel. 16-bit samples are big-endian.
pub(crate) fn read_sample(row: &[u8], index: usize, bit_depth: u8) -> u16 {
    match bit_depth {
        16 => u16::from_be_bytes([row[index * 2], row[index * 2 + 1]]),
        8 => u16::from(row[index]),
        _ => {
            let bits = usize::from(bit_depth);
            let bit = index * bits;
            let shift = 8 - bits - bit % 8;
            let mask = (1u8 << bits) - 1;
            u16::from((row[bit / 8] >> shift) & mask)
        }
    }
}

/// Converts one row of pixels to a format: the input row, and the output row to fill.
type RowConverter = fn(&ImageHeader, PixelFormat, &[u8], &mut [u8]);

/// Whether the pixels are already in `format`, so converting is a plain copy.
/// 8-bit rows have no padding, so the data and the bitmap have the same layout.
pub(crate) fn is_unchanged(header: &ImageHeader, format: PixelFormat) -> bool {
    header.bit_depth == 8
        && matches!(
            (header.color_type, format),
            (ColorType::Rgb, PixelFormat::Rgb8) | (ColorType::Rgba, PixelFormat::Rgba8)
        )
}

/// Maps each pixel of `IN` bytes in `row` to a pixel of `OUT` bytes in `out`.
/// With the sizes known at compile time, the compiler removes the bounds checks
/// and can vectorize the loop.
#[inline(always)]
fn map_pixels<const IN: usize, const OUT: usize>(row: &[u8], out: &mut [u8], f: impl Fn(&[u8; IN]) -> [u8; OUT]) {
    let (pixels, _) = row.as_chunks::<IN>();
    let (out, _) = out.as_chunks_mut::<OUT>();
    for (out, pixel) in out.iter_mut().zip(pixels) {
        *out = f(pixel);
    }
}

/// A [`RowConverter`] mapping pixels of `$in` bytes to pixels of `$out` bytes.
macro_rules! map_row {
    ($in:literal => $out:literal, |$pixel:ident| $body:expr) => {
        |_, _, row, out| map_pixels::<$in, $out>(row, out, |$pixel| $body)
    };
}

fn copy_row(_: &ImageHeader, _: PixelFormat, row: &[u8], out: &mut [u8]) {
    out.copy_from_slice(&row[..out.len()]);
}

/// Picks the converter for a header and format once, so there is no per-pixel
/// `match`. 8 and 16-bit images get a specialised loop each; 16-bit samples are
/// big-endian, so their high byte is the first of each pair. Samples under 8 bits
/// use the general [`convert_row_general`].
fn row_converter(header: &ImageHeader, format: PixelFormat) -> RowConverter {
    use ColorType::{Grayscale, GrayscaleAlpha, Rgb, Rgba};
    use PixelFormat::{Rgb8, Rgba8};

    match (header.bit_depth, header.color_type, format) {
        (8, Grayscale, Rgba8) => map_row!(1 => 4, |p| [p[0], p[0], p[0], 255]),
        (8, Grayscale, Rgb8) => map_row!(1 => 3, |p| [p[0]; 3]),
        (8, GrayscaleAlpha, Rgba8) => map_row!(2 => 4, |p| [p[0], p[0], p[0], p[1]]),
        (8, GrayscaleAlpha, Rgb8) => map_row!(2 => 3, |p| [p[0]; 3]),
        (8, Rgb, Rgba8) => map_row!(3 => 4, |p| [p[0], p[1], p[2], 255]),
        (8, Rgb, Rgb8) | (8, Rgba, Rgba8) => copy_row,
        (8, Rgba, Rgb8) => map_row!(4 => 3, |p| [p[0], p[1], p[2]]),

        (16, Grayscale, Rgba8) => map_row!(2 => 4, |p| [p[0], p[0], p[0], 255]),
        (16, Grayscale, Rgb8) => map_row!(2 => 3, |p| [p[0]; 3]),
        (16, GrayscaleAlpha, Rgba8) => map_row!(4 => 4, |p| [p[0], p[0], p[0], p[2]]),
        (16, GrayscaleAlpha, Rgb8) => map_row!(4 => 3, |p| [p[0]; 3]),
        (16, Rgb, Rgba8) => map_row!(6 => 4, |p| [p[0], p[2], p[4], 255]),
        (16, Rgb, Rgb8) => map_row!(6 => 3, |p| [p[0], p[2], p[4]]),
        (16, Rgba, Rgba8) => map_row!(8 => 4, |p| [p[0], p[2], p[4], p[6]]),
        (16, Rgba, Rgb8) => map_row!(8 => 3, |p| [p[0], p[2], p[4]]),

        _ => convert_row_general,
    }
}

/// Converts one row. `row` is one row of `source.data` (`source.stride`
/// bytes), and `out` is one output row, `width × format.bytes_per_pixel()` bytes.
///
/// Returns `Error::MissingPalette` for indexed images.
#[cfg(test)]
pub(crate) fn convert_row(source: &Source<'_>, row: &[u8], format: PixelFormat, out: &mut [u8]) -> Result<(), Error> {
    if source.header.color_type == ColorType::Indexed {
        return Err(Error::MissingPalette);
    }
    row_converter(source.header, format)(source.header, format, row, out);
    Ok(())
}

/// Converts one row of any bit depth, reading each sample separately. Slow, but
/// it handles packed samples under 8 bits. Indexed images must be rejected first.
fn convert_row_general(header: &ImageHeader, format: PixelFormat, row: &[u8], out: &mut [u8]) {
    let bit_depth = header.bit_depth;
    let channels = usize::from(header.color_type.channels());
    let bytes_per_pixel = format.bytes_per_pixel();
    let sample = |x: usize, channel: usize| {
        scale_to_8(read_sample(row, x * channels + channel, bit_depth), bit_depth)
    };

    for (x, pixel) in out.chunks_exact_mut(bytes_per_pixel).enumerate() {
        let rgba = match header.color_type {
            ColorType::Grayscale => {
                let gray = sample(x, 0);
                [gray, gray, gray, 255]
            }
            ColorType::GrayscaleAlpha => {
                let gray = sample(x, 0);
                [gray, gray, gray, sample(x, 1)]
            }
            ColorType::Rgb => [sample(x, 0), sample(x, 1), sample(x, 2), 255],
            ColorType::Rgba => [sample(x, 0), sample(x, 1), sample(x, 2), sample(x, 3)],
            ColorType::Indexed => unreachable!("rejected by the caller"),
        };
        // RGB is the first 3 bytes of RGBA, so this drops alpha for `Rgb8`.
        pixel.copy_from_slice(&rgba[..bytes_per_pixel]);
    }
}

/// Converts the whole image into `out`, replacing its contents and reusing its
/// allocation. Rows in `out` have no padding.
///
/// Returns `Error::ImageTooLarge` if the output size overflows `usize`, and
/// `Error::MissingPalette` for indexed images.
pub(crate) fn convert(source: &Source<'_>, format: PixelFormat, out: &mut Vec<u8>) -> Result<(), Error> {
    let width = usize::try_from(source.header.width).map_err(|_| Error::ImageTooLarge)?;
    let height = usize::try_from(source.header.height).map_err(|_| Error::ImageTooLarge)?;

    let bitmap_stride = width
        .checked_mul(format.bytes_per_pixel())
        .ok_or(Error::ImageTooLarge)?;
    let bitmap_size = bitmap_stride.checked_mul(height).ok_or(Error::ImageTooLarge)?;

    let header = source.header;
    if header.color_type == ColorType::Indexed {
        return Err(Error::MissingPalette);
    }

    out.clear();
    if is_unchanged(header, format) {
        // One copy of the whole image, without zeroing `out` first.
        out.extend_from_slice(&source.data[..bitmap_size]);
        return Ok(());
    }
    out.resize(bitmap_size, 0);

    let convert_row = row_converter(header, format);
    let rows = source.data.chunks_exact(source.stride);
    for (row, out_row) in rows.zip(out.chunks_exact_mut(bitmap_stride)) {
        convert_row(header, format, row, out_row);
    }

    Ok(())
}

#[cfg(test)]
mod tests;
