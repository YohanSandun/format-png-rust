//! A PNG decoder and encoder in pure Rust, with no `unsafe` code. It has a
//! simple API for getting pixels on screen or into a file, and full control
//! when you need it.
//!
//! Every color type, every bit depth and Adam7 interlacing are supported, in
//! both directions.
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
//! # Encoding
//!
//! [`encode_rgba8`] is the reverse of [`decode_rgba8`]:
//!
//! ```
//! let pixels = [255, 0, 0, 255, 0, 0, 255, 128]; // red, then half-transparent blue
//! let png = format_png::encode_rgba8(2, 1, &pixels)?;
//!
//! assert_eq!(format_png::decode_rgba8(&png)?.data(), pixels);
//! # Ok::<(), format_png::Error>(())
//! ```
//!
//! - [`encode`] writes any [`ImageRef`]: pixels in any color type and bit depth,
//!   in the layout [`Image::data`] uses, with a palette, transparency,
//!   [`Metadata`] and extra chunks. `ImageRef::from(&image)` re-encodes a
//!   decoded [`Image`].
//! - An [`Encoder`] can be reused for many images, like a [`Decoder`].
//! - [`EncodeOptions`] choose the compression level and strategy, and the row
//!   filters. [`PaletteMode::Auto`] writes images with few colors as indexed
//!   color, and [`StripChunks`] leaves out ancillary chunks, both to make files
//!   smaller.
//!
//! # Not supported yet
//!
//! - `bKGD`, `sBIT`, `hIST`, `sPLT` and some other ancillary chunks aren't
//!   parsed; see [`png::metadata`] for the ones that are. The others can be kept
//!   raw with [`DecodeOptions::preserve_chunks`], written back with
//!   [`ImageRef::with_chunks`], and read with [`read_chunks`] or a [`ChunkReader`].
//! - Color management: gamma, chromaticities, ICC profiles and `cICP` are read
//!   and written but not applied to the pixels.
//! - Animated PNG: only the default image is decoded and encoded.
//! - Lossy encoding: [`PaletteMode::Auto`] only converts images that fit a
//!   palette exactly.

#![warn(missing_docs)]

mod convert;
mod decode;
mod encode;
mod error;
mod io;
pub mod png;

pub use decode::{ChunkReader, DecodeOptions, Decoder};
pub use encode::{EncodeOptions, Encoder, FilterStrategy, ImageRef, PaletteMode, StripChunks};
pub use error::Error;
pub use png::{
    Bitmap, ChunkPosition, ColorType, Image, ImageHeader, Interlace, Metadata, OwnedChunk, Palette,
    PaletteAlpha, PixelFormat, PngChunks, Transparency,
};
pub use rust_deflate::CompressionLevel;
/// Which kind of DEFLATE blocks the encoder writes; see
/// [`EncodeOptions::compression_strategy`].
pub use rust_deflate::Strategy as CompressionStrategy;

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

/// Encodes an image to PNG with default options; see [`ImageRef`].
///
/// This is the simple API. To encode many images, reuse an [`Encoder`] and call
/// [`Encoder::encode_into`] instead.
///
/// # Errors
///
/// Same as [`Encoder::encode_into`].
pub fn encode(image: ImageRef<'_>) -> Result<Vec<u8>, Error> {
    Encoder::new().encode(image)
}

/// Encodes 8-bit RGBA pixels to PNG with default options: the reverse of
/// [`decode_rgba8`]. `data` is `width × height × 4` bytes, rows top to bottom
/// with no padding, as a browser canvas's `ImageData` holds them.
///
/// # Errors
///
/// Same as [`Encoder::encode_into`]; [`Error::InvalidImageDataLength`] if `data`
/// is the wrong size.
pub fn encode_rgba8(width: u32, height: u32, data: &[u8]) -> Result<Vec<u8>, Error> {
    let header = ImageHeader {
        width,
        height,
        bit_depth: 8,
        color_type: ColorType::Rgba,
        interlace: Interlace::None,
    };
    encode(ImageRef::new(header, data))
}

#[cfg(test)]
mod tests;

// Compiles the README's examples as doctests, so they can't drift from the API.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
