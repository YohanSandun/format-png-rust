//! A PNG decoder with a simple API for common use and full access to the file's
//! chunks when you need it.
//!
//! Pixels are returned in the file's own format, without conversion; see [`Image`].
//! Ancillary chunks such as `PLTE`, `tRNS` and text are skipped for now.
//!
//! - [`decode`] decodes one image, and [`read_header`] reads just its header.
//! - A [`Decoder`] can be reused for many images, and keeps its decompressor and
//!   buffers between them.
//! - A [`ChunkReader`] returns every chunk raw, including ancillary, private and
//!   unknown ones. Chunk types are in the [`png`] module.
//!
//! ```
//! let data = std::fs::read("tests/data/valid/rgba_8.png")?;
//! let image = format_png::decode(&data)?;
//!
//! assert_eq!((image.width(), image.height()), (13, 7));
//! assert_eq!(image.header().color_type, format_png::ColorType::Rgba);
//! assert_eq!(image.data().len(), 13 * 7 * 4);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod png;
mod io;
mod decode;
mod error;

pub use decode::{ChunkReader, DecodeOptions, Decoder};
pub use error::Error;
pub use png::{ColorType, Image, ImageHeader, Interlace};

/// Decodes a PNG with default options.
///
/// This is the simple API. To decode many images, reuse a [`Decoder`] instead.
///
/// # Errors
///
/// Same as [`Decoder::decode_into`].
pub fn decode(data: &[u8]) -> Result<Image, Error> {
    Decoder::new().decode(data)
}

/// Reads the `IHDR` chunk of a PNG with default options.
///
/// This is the simple API. To decode many images, reuse a [`Decoder`] instead; to
/// look at the chunks yourself, use [`Decoder::chunks`] or [`ChunkReader`].
///
/// # Errors
///
/// Same as [`Decoder::read_header`].
pub fn read_header(data: &[u8]) -> Result<ImageHeader, Error> {
    Decoder::new().read_header(data)
}

#[cfg(test)]
mod tests;
