//! A PNG decoder with a simple API for getting pixels on screen, and full
//! control when you need it.
//!
//! Every color type, every bit depth and Adam7 interlacing are supported.
//! Encoding isn't implemented yet.
//!
//! # Getting started
//!
//! [`decode_rgba8`] decodes a PNG to 8-bit RGBA, the layout a browser canvas's
//! `ImageData` expects:
//!
//! ```
//! let data = std::fs::read("tests/data/valid/rgb_8.png")?;
//! let bitmap = format_png::decode_rgba8(&data)?;
//!
//! assert_eq!((bitmap.width(), bitmap.height()), (13, 7));
//! assert_eq!(bitmap.data().len(), 13 * 7 * 4); // no row padding
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! [`decode_rgb8`] does the same without alpha. See [`Image::to_bitmap`] for
//! the conversion rules.
//!
//! # More control
//!
//! - A [`Decoder`] can be reused for many images. It keeps its decompressor and
//!   working buffers between them, and [`Decoder::decode_bitmap_into`] and
//!   [`Decoder::decode_into`] also reuse your output buffer.
//! - [`decode`] returns an [`Image`] in the file's own pixel format, without
//!   conversion: 16-bit samples, packed pixels under 8 bits, and palette indices
//!   with their [`Palette`].
//! - [`read_header`] reads only the image header.
//! - [`read_chunks`] reads and parses every chunk without decompressing the
//!   image data, which is much faster than decoding. It returns a [`PngChunks`]
//!   with the header, palette, metadata and text, and every chunk raw, including
//!   private, custom and unknown ones.
//! - A [`ChunkReader`] returns every chunk raw, including ancillary, private and
//!   unknown ones. Chunk types are in the [`png`] module.
//! - [`DecodeOptions::preserve_metadata`] parses known ancillary chunks into
//!   [`Image::metadata`], and [`DecodeOptions::preserve_chunks`] keeps raw copies
//!   of all of them in [`Image::ancillary_chunks`].
//!
//! ```
//! let data = std::fs::read("tests/data/valid/rgba_16.png")?;
//! let image = format_png::decode(&data)?;
//!
//! assert_eq!(image.header().color_type, format_png::ColorType::Rgba);
//! assert_eq!(image.stride(), 13 * 8); // 4 channels of 2 bytes
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Not supported yet
//!
//! `iCCP` and most other ancillary chunks aren't parsed yet; see
//! [`png::metadata`] for the ones that are. The others can be kept raw with
//! [`DecodeOptions::preserve_chunks`], and [`read_chunks`] and a [`ChunkReader`]
//! return them all.

pub mod png;
mod convert;
mod io;
mod decode;
mod error;

pub use decode::{ChunkReader, DecodeOptions, Decoder};
pub use error::Error;
pub use png::{
    Bitmap, ChunkPosition, ColorType, Image, ImageHeader, Interlace, Metadata, OwnedChunk, Palette, PaletteAlpha,
    PixelFormat, PngChunks, Transparency,
};

/// Decodes a PNG to 8-bit RGBA, ready for a browser canvas's `ImageData`.
///
/// This is the simplest way to display an image. To decode many images, reuse a
/// [`Decoder`] and call [`Decoder::decode_bitmap_into`] instead.
///
/// ```
/// let data = std::fs::read("tests/data/valid/rgb_8.png")?;
/// let bitmap = format_png::decode_rgba8(&data)?;
///
/// assert_eq!((bitmap.width(), bitmap.height()), (13, 7));
/// assert_eq!(bitmap.data().len(), 13 * 7 * 4);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Same as [`Decoder::decode_bitmap_into`].
pub fn decode_rgba8(data: &[u8]) -> Result<Bitmap, Error> {
    Decoder::new().decode_bitmap(data, PixelFormat::Rgba8)
}

/// Decodes a PNG to 8-bit RGB, dropping any alpha.
///
/// # Errors
///
/// Same as [`Decoder::decode_bitmap_into`].
pub fn decode_rgb8(data: &[u8]) -> Result<Bitmap, Error> {
    Decoder::new().decode_bitmap(data, PixelFormat::Rgb8)
}

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

/// Reads and parses every chunk of a PNG with default options, without
/// decompressing the image data; see [`PngChunks`].
///
/// This is the simple API. To read many images, reuse a [`Decoder`] and call
/// [`Decoder::read_chunks`] instead.
///
/// # Errors
///
/// Same as [`Decoder::read_chunks`].
pub fn read_chunks(data: &[u8]) -> Result<PngChunks<'_>, Error> {
    Decoder::new().read_chunks(data)
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

// Compiles the README's examples as doctests, so they can't drift from the API.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
