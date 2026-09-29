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

/// Converts one row. `row` is one row of `source.data` (`source.stride`
/// bytes), and `out` is one output row, `width × format.bytes_per_pixel()` bytes.
///
/// Returns `Error::MissingPalette` for indexed images.
pub(crate) fn convert_row(source: &Source<'_>, row: &[u8], format: PixelFormat, out: &mut [u8]) -> Result<(), Error> {
    let header = source.header;
    if header.color_type == ColorType::Indexed {
        return Err(Error::MissingPalette);
    }

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
            ColorType::Indexed => unreachable!("rejected above"),
        };
        // RGB is the first 3 bytes of RGBA, so this drops alpha for `Rgb8`.
        pixel.copy_from_slice(&rgba[..bytes_per_pixel]);
    }

    Ok(())
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

    out.clear();
    out.resize(bitmap_size, 0);

    let rows = source.data.chunks_exact(source.stride);
    for (row, out_row) in rows.zip(out.chunks_exact_mut(bitmap_stride)) {
        convert_row(source, row, format, out_row)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests;
