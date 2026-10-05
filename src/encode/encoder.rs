use rust_deflate::{CompressionOptions, Compressor};

use super::chunk_writer::{
    MAX_CHUNK_LENGTH, header_data, transparency_data, write_chunk, write_image_data,
    write_signature,
};
use super::filter::filter_image;
use super::image_ref::ImageRef;
use super::options::{EncodeOptions, PaletteMode, StripChunks};
use super::palettize::palettize;
use super::strip::strip_metadata;
use crate::error::Error;
use crate::png::{ChunkType, Palette, Transparency};
use crate::{ChunkPosition, ColorType, ImageHeader};
use crate::encode::extra_chunks::{validate_extra_chunks, write_extra_chunks};
use crate::encode::metadata::{validate_metadata, write_metadata_after_palette, write_metadata_before_palette};

/// The largest image data, in bytes, for which `PaletteMode::Auto` also
/// encodes the image as given and keeps the smaller file: 64×64 RGBA. The
/// palette's `PLTE` and `tRNS` chunks cost up to about 1 KB, so above this the
/// converted image is smaller in practice, and encoding twice wouldn't pay.
pub(crate) const AUTO_PALETTE_COMPARE_LIMIT: usize = 16 * 1024;

/// A reusable PNG encoder.
///
/// It owns a [`Compressor`] and its working buffers, and keeps them between
/// images, so encoding many PNGs with one `Encoder` avoids setting them up for
/// each one. [`encode_into`](Self::encode_into) also reuses your output buffer.
/// For a single image, [`encode`](crate::encode) is simpler.
///
/// The PNG written has the signature, `IHDR`, `PLTE` if the image has a
/// palette, `tRNS` if it has transparency, the image data as one or more `IDAT`
/// chunks, and `IEND`.
///
/// The image is interlaced if its header says
/// [`Interlace::Adam7`](crate::Interlace::Adam7).
///
/// Metadata chunks are written around `PLTE`, in the order the spec allows;
/// see [`ImageRef::with_metadata`].
///
/// Extra chunks, such as the raw ones kept by
/// [`DecodeOptions::preserve_chunks`](crate::DecodeOptions::preserve_chunks),
/// are written at their positions; see [`ImageRef::with_chunks`].
///
/// With [`PaletteMode::Auto`], images with few colors are written as indexed color.
#[derive(Debug)]
pub struct Encoder {
    options: EncodeOptions,
    compressor: Compressor,
    /// The filtered rows, each with its filter type byte: what's compressed.
    scanlines: Vec<u8>,
    /// The zlib stream, before it's split into `IDAT` chunks.
    compressed: Vec<u8>,
    /// Palette indices, when `PaletteMode::Auto` converts an image.
    indexed: Vec<u8>,
    /// The PNG written as given, when `PaletteMode::Auto` compares it with the
    /// converted one; see `AUTO_PALETTE_COMPARE_LIMIT`.
    unconverted: Vec<u8>,
}

impl Encoder {
    /// Creates an encoder with [`EncodeOptions::default`].
    pub fn new() -> Self {
        Self::with_options(EncodeOptions::default())
    }

    /// Creates an encoder with the given options.
    pub fn with_options(options: EncodeOptions) -> Self {
        Self {
            options,
            compressor: Compressor::new(),
            scanlines: Vec::new(),
            compressed: Vec::new(),
            indexed: Vec::new(),
            unconverted: Vec::new(),
        }
    }

    /// The options this encoder was created with.
    pub fn options(&self) -> &EncodeOptions {
        &self.options
    }

    /// Encodes `image` to a PNG file's bytes.
    ///
    /// # Errors
    ///
    /// Same as [`encode_into`](Self::encode_into).
    pub fn encode(&mut self, image: ImageRef<'_>) -> Result<Vec<u8>, Error> {
        let mut out = Vec::new();
        self.encode_into(image, &mut out)?;
        Ok(out)
    }

    /// Like [`encode`](Self::encode), but writes the PNG to `out`. `out`'s old
    /// contents are replaced and its allocation is reused, so passing the same
    /// buffer for many images avoids allocating one each time. After an error,
    /// `out`'s contents are unspecified.
    ///
    /// # Errors
    ///
    /// Any error from `validate`:
    /// - [`Error::InvalidDimensions`] or [`Error::InvalidBitDepth`] if the header
    ///   isn't valid, as [`ImageHeader::parse`](crate::ImageHeader::parse) checks.
    /// - [`Error::InvalidImageDataLength`] if the pixel data isn't
    ///   [`ImageHeader::image_size`](crate::ImageHeader::image_size) bytes.
    /// - [`Error::MissingPalette`] for an indexed image without a palette, and
    ///   [`Error::UnexpectedPalette`] for a grayscale image with one.
    /// - [`Error::TooManyPaletteEntries`] if an indexed image's palette has more
    ///   entries than its bit depth can index.
    /// - [`Error::ImageTooLarge`] if the image doesn't fit in memory on this platform.
    pub fn encode_into(&mut self, image: ImageRef<'_>, out: &mut Vec<u8>) -> Result<(), Error> {
        let stripped_metadata;
        let image = match self.options.strip {
            StripChunks::Keep => image,
            mode => {
                stripped_metadata = image.metadata().map(|metadata| strip_metadata(metadata, mode));
                // No extra chunks: neither mode keeps any.
                let mut stripped = ImageRef::new(*image.header(), image.data());
                // An indexed image needs its palette. Any other image's palette is
                // only a suggestion that viewers ignore, so it goes too.
                if let Some(palette) = image.palette().filter(|_| image.header().color_type == ColorType::Indexed) {
                    stripped = stripped.with_palette(palette);
                }
                if let Some(transparency) = image.transparency() {
                    stripped = stripped.with_transparency(transparency);
                }
                if let Some(metadata) = &stripped_metadata {
                    stripped = stripped.with_metadata(metadata);
                }
                stripped
            }
        };

        validate(&image)?;

        // The converted image borrows this buffer while `write_png` borrows the
        // encoder, so it's taken out for the call and put back after, even on error.
        let mut indexed = std::mem::take(&mut self.indexed);
        let result = self.encode_validated(image, &mut indexed, out);
        self.indexed = indexed;
        result
    }

    /// `encode_into` after stripping and validation: converts the image to
    /// indexed color if `PaletteMode::Auto` can, with the indices in `indexed`,
    /// and writes the smaller of the two for small images.
    fn encode_validated(&mut self, image: ImageRef<'_>, indexed: &mut Vec<u8>, out: &mut Vec<u8>) -> Result<(), Error> {
        let palettized = match self.options.palette {
            PaletteMode::Auto => palettize(image.header(), image.data(), image.transparency(), indexed),
            _ => None,
        };
        let Some(palettized) = palettized else {
            return self.write_png(&image, self.options.keep_unsafe_chunks, out);
        };

        let mut converted = ImageRef::new(palettized.header, indexed)
            .with_palette(&palettized.palette)
            .with_chunks(image.chunks());
        if let Some(transparency) = &palettized.transparency {
            converted = converted.with_transparency(transparency);
        }
        if let Some(metadata) = image.metadata() {
            converted = converted.with_metadata(metadata);
        }
        // Unsafe-to-copy chunks describe the image as given; after a change of
        // color type they'd be wrong.
        self.write_png(&converted, false, out)?;

        // The palette's chunks cost up to about 1 KB, which a small image may not
        // win back. Encoding it again as given is cheap there, so keep whichever
        // is smaller, and the image as given on a tie.
        if image.data().len() <= AUTO_PALETTE_COMPARE_LIMIT {
            let mut unconverted = std::mem::take(&mut self.unconverted);
            let result = self.write_png(&image, self.options.keep_unsafe_chunks, &mut unconverted);
            if result.is_ok() && unconverted.len() <= out.len() {
                std::mem::swap(out, &mut unconverted);
            }
            self.unconverted = unconverted;
            result?;
        }

        Ok(())
    }

    /// Filters, compresses and writes `image` as a PNG to `out`, replacing its
    /// contents. `image` has been validated. `keep_unsafe` is
    /// `EncodeOptions::keep_unsafe_chunks`, or `false` for a converted image.
    fn write_png(&mut self, image: &ImageRef<'_>, keep_unsafe: bool, out: &mut Vec<u8>) -> Result<(), Error> {
        out.clear();
        self.scanlines.clear();
        self.compressed.clear();

        let header = image.header();

        let compression = CompressionOptions::new()
            .level(self.options.compression)
            .strategy(self.options.compression_strategy);

        filter_image(
            self.options.filter,
            header,
            image.data(),
            &mut self.scanlines,
        )?;
        self.compressor.compress_zlib_into_with(&self.scanlines, &mut self.compressed, compression);

        write_signature(out);
        write_chunk(out, ChunkType::IHDR, &header_data(header));

        if let Some(metadata) = image.metadata() {
            write_metadata_before_palette(out, metadata, &mut self.compressor, compression);
        }

        write_extra_chunks(out, image, ChunkPosition::BeforePalette, keep_unsafe);

        if let Some(palette) = image.palette() {
            write_chunk(out, ChunkType::PLTE, palette.colors().as_flattened());
        }

        if let Some(transparency) = image.transparency() {
            write_chunk(out, ChunkType::TRNS, &transparency_data(transparency));
        }

        if let Some(metadata) = image.metadata() {
            write_metadata_after_palette(out, metadata, &mut self.compressor, compression);
        }

        write_extra_chunks(out, image, ChunkPosition::BeforeImageData, keep_unsafe);

        write_image_data(out, &self.compressed, MAX_CHUNK_LENGTH);

        write_extra_chunks(out, image, ChunkPosition::AfterImageData, keep_unsafe);

        write_chunk(out, ChunkType::IEND, &[]);
        Ok(())
    }
}

impl Default for Encoder {
    fn default() -> Self {
        Self::new()
    }
}

/// Checks that `image` can be written as a valid PNG; see
/// [`Encoder::encode_into`] for the errors.
fn validate(image: &ImageRef<'_>) -> Result<(), Error> {
    let header = image.header();
    ImageHeader::validate_dimensions(header.width, header.height)?;
    ImageHeader::validate_bit_depth(header.bit_depth, header.color_type)?;

    if header.color_type == ColorType::Indexed && image.palette().is_none() {
        return Err(Error::MissingPalette);
    }

    let image_size = header.image_size()?;
    if image_size != image.data().len() {
        return Err(Error::InvalidImageDataLength {
            expected: image_size,
            actual: image.data().len(),
        });
    }

    if let Some(palette) = image.palette() {
        if header.color_type == ColorType::Grayscale
            || header.color_type == ColorType::GrayscaleAlpha
        {
            return Err(Error::UnexpectedPalette(header.color_type));
        }

        if header.color_type == ColorType::Indexed && palette.len() > 1 << header.bit_depth {
            return Err(Error::TooManyPaletteEntries {
                entries: palette.len(),
                bit_depth: header.bit_depth,
            });
        }
    }

    if let Some(transparency) = image.transparency() {
        validate_transparency(header, image.palette(), transparency)?;
    }

    if let Some(metadata) = image.metadata() {
        validate_metadata(metadata)?;
    }

    validate_extra_chunks(image.chunks())?;

    Ok(())
}

/// Checks that `transparency` can be written as the `tRNS` chunk of an image
/// with `header` and `palette`. `palette` has already been checked, and is there
/// for indexed images.
///
/// Errors:
/// - `Error::UnexpectedTransparency` for grayscale with alpha and RGBA, which
///   must not have a `tRNS` chunk.
/// - `Error::InvalidTransparencyLength` if the kind doesn't match the color
///   type: `Gray` is only for grayscale, `Rgb` only for RGB and `Palette` only
///   for indexed images. The length given is the one the chunk would have: 2,
///   6, or the number of alpha values.
/// - `Error::InvalidChunkData` for `ChunkType::TRNS` if a gray or RGB value
///   doesn't fit the bit depth, such as 16 for a 4-bit image. The decoder
///   ignores those bits, so writing them would change which color is transparent.
/// - `Error::TooManyTransparencyEntries` if an indexed image has more alpha
///   values than palette entries.
fn validate_transparency(
    header: &ImageHeader,
    palette: Option<&Palette>,
    transparency: &Transparency,
) -> Result<(), Error> {
    if header.color_type == ColorType::GrayscaleAlpha || header.color_type == ColorType::Rgba {
        return Err(Error::UnexpectedTransparency(header.color_type));
    }

    let max_sample = (1u32 << header.bit_depth) - 1;
    let fits = |v: u16| u32::from(v) <= max_sample;

    match transparency {
        Transparency::Gray(gray) => {
            if header.color_type != ColorType::Grayscale {
                return Err(Error::InvalidTransparencyLength {
                    color_type: header.color_type,
                    length: 2,
                });
            }
            if !fits(*gray) {
                return Err(Error::InvalidChunkData(ChunkType::TRNS));
            }
        }
        Transparency::Rgb(rgb) => {
            if header.color_type != ColorType::Rgb {
                return Err(Error::InvalidTransparencyLength {
                    color_type: header.color_type,
                    length: rgb.len() * 2,
                });
            }
            if !rgb.iter().all(|&v| fits(v)) {
                return Err(Error::InvalidChunkData(ChunkType::TRNS));
            }
        }
        Transparency::Palette(palette_alpha) => {
            if header.color_type != ColorType::Indexed {
                return Err(Error::InvalidTransparencyLength {
                    color_type: header.color_type,
                    length: palette_alpha.len(),
                });
            }
            let palette = palette.ok_or(Error::MissingPalette)?;
            if palette_alpha.len() > palette.len() {
                return Err(Error::TooManyTransparencyEntries {
                    entries: palette_alpha.len(),
                    palette_entries: palette.len(),
                });
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests;
