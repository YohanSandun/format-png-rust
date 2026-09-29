use rust_deflate::Decompressor;

use super::chunk_reader::ChunkReader;
use super::options::DecodeOptions;
use crate::error::Error;
use crate::png::{ChunkType, ImageHeader};

/// A reusable PNG decoder.
///
/// It owns a [`Decompressor`] and keeps it between images, so decoding many PNGs
/// with one `Decoder` avoids setting up its tables for each one. For a single
/// image, [`read_header`](crate::read_header) is simpler.
///
/// ```
/// use format_png::{Decoder, DecodeOptions};
///
/// let mut decoder = Decoder::with_options(DecodeOptions {
///     validate_crc: false,
///     ..DecodeOptions::default()
/// });
///
/// for path in ["tests/data/valid/rgb_8.png", "tests/data/valid/gray_16_adam7.png"] {
///     let header = decoder.read_header(&std::fs::read(path)?)?;
///     println!("{path}: {}x{}", header.width, header.height);
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct Decoder {
    options: DecodeOptions,
    #[allow(dead_code)]
    decompressor: Decompressor,
}

impl Decoder {
    /// Creates a decoder with [`DecodeOptions::default`].
    pub fn new() -> Self {
        Self::with_options(DecodeOptions::default())
    }

    /// Creates a decoder with the given options.
    pub fn with_options(options: DecodeOptions) -> Self {
        Self::with_decompressor(options, Decompressor::new())
    }

    /// Creates a decoder that uses an existing [`Decompressor`], for example one taken
    /// from another decoder with [`into_decompressor`](Self::into_decompressor).
    pub fn with_decompressor(options: DecodeOptions, decompressor: Decompressor) -> Self {
        Self { options, decompressor }
    }

    /// The options this decoder was created with.
    pub fn options(&self) -> &DecodeOptions {
        &self.options
    }

    /// Consumes the decoder and returns its decompressor, to reuse elsewhere.
    pub fn into_decompressor(self) -> Decompressor {
        self.decompressor
    }

    /// Returns a [`ChunkReader`] over `data`, with CRC validation set from this
    /// decoder's options.
    ///
    /// # Errors
    ///
    /// Same as [`ChunkReader::new`].
    pub fn chunks<'a>(&self, data: &'a [u8]) -> Result<ChunkReader<'a>, Error> {
        Ok(ChunkReader::new(data)?.validate_crc(self.options.validate_crc))
    }
    
    /// Reads and validates the `IHDR` chunk, which must be the first chunk. Chunks
    /// after it aren't read.
    ///
    /// # Errors
    ///
    /// - [`Error::InvalidSignature`] or [`Error::UnexpectedEndOfInput`] if `data`
    ///   doesn't start with a complete PNG signature.
    /// - [`Error::MissingImageHeader`] if there are no chunks or the first isn't `IHDR`.
    /// - Any error from [`ChunkReader::next_chunk`] or [`ImageHeader::parse`].
    pub fn read_header(&mut self, data: &[u8]) -> Result<ImageHeader, Error> {
        let mut chunk_reader = self.chunks(data)?;
        let chunk_option = chunk_reader.next_chunk()?;

        if let Some(chunk) = chunk_option {
            if chunk.chunk_type() == ChunkType::IHDR {
                return ImageHeader::parse(chunk.data());
            }
        }

        Err(Error::MissingImageHeader)
    }
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
