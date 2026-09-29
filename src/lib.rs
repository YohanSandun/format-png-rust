//! A PNG decoder with a simple API for common use and full access to the file's
//! chunks when you need it.
//!
//! Only the image header (`IHDR`) is decoded so far.
//!
//! - [`read_header`] reads the header of one image.
//! - A [`Decoder`] can be reused for many images, and keeps its decompressor
//!   between them.
//! - A [`ChunkReader`] returns every chunk raw, including ancillary, private and
//!   unknown ones. Chunk types are in the [`png`] module.
//!
//! ```
//! let data = std::fs::read("tests/data/valid/rgba_8.png")?;
//! let header = format_png::read_header(&data)?;
//!
//! assert_eq!((header.width, header.height), (13, 7));
//! assert_eq!(header.color_type, format_png::ColorType::Rgba);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod png;
mod io;
mod decode;
mod error;

pub use decode::{ChunkReader, DecodeOptions, Decoder};
pub use error::Error;
pub use png::{ColorType, ImageHeader, Interlace};

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
