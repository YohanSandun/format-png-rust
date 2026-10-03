use rust_deflate::{CompressionOptions, Compressor};

use super::chunk_writer::{MAX_CHUNK_LENGTH, header_data, write_chunk, write_image_data, write_signature};
use super::filter::filter_image;
use super::image_ref::ImageRef;
use super::options::EncodeOptions;
use crate::error::Error;
use crate::{ColorType, ImageHeader};
use crate::png::ChunkType;

/// A reusable PNG encoder.
///
/// It owns a [`Compressor`] and its working buffers, and keeps them between
/// images, so encoding many PNGs with one `Encoder` avoids setting them up for
/// each one. [`encode_into`](Self::encode_into) also reuses your output buffer.
/// For a single image, [`encode`](crate::encode) is simpler.
///
/// The PNG written has the signature, `IHDR`, `PLTE` if the image has a
/// palette, the image data as one or more `IDAT` chunks, and `IEND`.
///
/// The image is interlaced if its header says
/// [`Interlace::Adam7`](crate::Interlace::Adam7).
///
/// Not supported yet: `tRNS`, metadata, and other ancillary chunks.
#[derive(Debug)]
pub struct Encoder {
    options: EncodeOptions,
    compressor: Compressor,
    /// The filtered rows, each with its filter type byte: what's compressed.
    scanlines: Vec<u8>,
    /// The zlib stream, before it's split into `IDAT` chunks.
    compressed: Vec<u8>,
}

impl Encoder {
    /// Creates an encoder with [`EncodeOptions::default`].
    pub fn new() -> Self {
        Self::with_options(EncodeOptions::default())
    }

    /// Creates an encoder with the given options.
    pub fn with_options(options: EncodeOptions) -> Self {
        Self { options, compressor: Compressor::new(), scanlines: Vec::new(), compressed: Vec::new() }
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
        validate(&image)?;

        out.clear();
        self.scanlines.clear();
        self.compressed.clear();

        let header = image.header();

        filter_image(self.options.filter, header, image.data(), &mut self.scanlines)?;
        self.compressor.compress_zlib_into_with(&self.scanlines, &mut self.compressed, CompressionOptions::default().level(self.options.compression));

        write_signature(out);
        write_chunk(out, ChunkType::IHDR, &header_data(header));

        if let Some(palette) = image.palette() {
            write_chunk(out, ChunkType::PLTE, palette.colors().as_flattened());
        }

        write_image_data(out, &self.compressed, MAX_CHUNK_LENGTH);
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
        return Err(Error::InvalidImageDataLength { expected: image_size, actual: image.data().len() });
    }

    if let Some(palette) = image.palette() {
        if header.color_type == ColorType::Grayscale || header.color_type == ColorType::GrayscaleAlpha {
            return Err(Error::UnexpectedPalette(header.color_type));
        }

        if header.color_type == ColorType::Indexed && palette.len() > 1 << header.bit_depth {
            return Err(Error::TooManyPaletteEntries { entries: palette.len(), bit_depth: header.bit_depth });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests;
