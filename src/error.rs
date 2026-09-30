use std::fmt;

use crate::png::ColorType;

/// Everything that can go wrong while decoding a PNG.
///
/// New variants will be added as more of the format is decoded, so matches on it
/// need a wildcard arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Error {
    /// The input ended unexpectedly.
    UnexpectedEndOfInput,

    /// The input does not start with the 8-byte PNG signature.
    InvalidSignature,

    /// A chunk type contains a byte that is not an ASCII letter.
    InvalidChunkType([u8; 4]),

    /// A chunk declares a length greater than 2^31 - 1.
    ChunkTooLong(u32),

    /// A chunk's stored CRC does not match the CRC computed over its type and data.
    /// `expected` is the CRC stored in the file, `actual` the one computed.
    CrcMismatch { expected: u32, actual: u32 },

    /// The first chunk is not `IHDR`, or there are no chunks at all.
    MissingImageHeader,

    /// The `IHDR` chunk data is not exactly 13 bytes long.
    InvalidImageHeaderLength(usize),

    /// Width or height is zero or greater than 2^31 - 1.
    InvalidDimensions { width: u32, height: u32 },

    /// The color type is not one of 0, 2, 3, 4 or 6.
    InvalidColorType(u8),

    /// The bit depth is not allowed for the color type.
    InvalidBitDepth { color_type: ColorType, bit_depth: u8 },

    /// The compression method is not 0 (deflate).
    InvalidCompressionMethod(u8),

    /// The filter method is not 0 (adaptive filtering).
    InvalidFilterMethod(u8),

    /// The interlace method is not 0 (none) or 1 (Adam7).
    InvalidInterlaceMethod(u8),

    /// `IEND` comes before any `IDAT` chunk.
    MissingImageData,

    /// Another chunk comes between two `IDAT` chunks.
    NonConsecutiveImageData,

    /// The input ends without an `IEND` chunk.
    MissingImageEnd,

    /// The zlib stream in the `IDAT` chunks is corrupt.
    Decompression(rust_deflate::Error),

    /// The image data decompresses to fewer bytes than the header requires.
    ImageDataTooShort { expected: usize, actual: usize },

    /// The image data decompresses to more bytes than the header requires.
    ImageDataTooLong { expected: usize },

    /// A scanline's filter type is not 0 to 4.
    InvalidFilterType(u8),

    /// The decoded image is too large to address with `usize` on this platform.
    ImageTooLarge,

    /// An indexed image has no `PLTE` chunk before its image data.
    MissingPalette,

    /// The `PLTE` chunk's length isn't a multiple of 3, or it doesn't hold 1 to
    /// 256 entries. The value is the chunk's length in bytes.
    InvalidPaletteLength(usize),

    /// An indexed image's palette has more entries than its bit depth can index.
    TooManyPaletteEntries { entries: usize, bit_depth: u8 },

    /// A grayscale image has a `PLTE` chunk, which isn't allowed.
    UnexpectedPalette(ColorType),

    /// There is more than one `PLTE` chunk.
    DuplicatePalette,

    /// The `PLTE` chunk comes after the image data.
    PaletteAfterImageData,

    /// A pixel of an indexed image refers to a palette entry that doesn't exist.
    PaletteIndexOutOfRange { index: u8, entries: usize },

    /// The `tRNS` chunk's length is wrong for the color type: it must be 2 bytes
    /// for grayscale, 6 for RGB, and 1 or more for indexed.
    InvalidTransparencyLength { color_type: ColorType, length: usize },

    /// An indexed image's `tRNS` chunk has more alpha values than its palette has entries.
    TooManyTransparencyEntries { entries: usize, palette_entries: usize },

    /// An image with an alpha channel has a `tRNS` chunk, which isn't allowed.
    UnexpectedTransparency(ColorType),

    /// There is more than one `tRNS` chunk.
    DuplicateTransparency,

    /// The `tRNS` chunk comes after the image data.
    TransparencyAfterImageData,

    /// The `tRNS` chunk comes before `PLTE`: either an indexed image's `tRNS` has
    /// no palette before it, or a `PLTE` follows the `tRNS`.
    TransparencyBeforePalette,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::UnexpectedEndOfInput => f.write_str("unexpected end of input"),
            Error::InvalidSignature => f.write_str("invalid PNG signature"),
            Error::InvalidChunkType(bytes) => write!(f, "invalid chunk type {bytes:02X?}"),
            Error::ChunkTooLong(length) => write!(f, "chunk length {length} exceeds 2^31 - 1"),
            Error::CrcMismatch { expected, actual } => {
                write!(f, "CRC mismatch: expected {expected:#010X}, got {actual:#010X}")
            }
            Error::MissingImageHeader => f.write_str("first chunk is not IHDR"),
            Error::InvalidImageHeaderLength(length) => {
                write!(f, "IHDR length is {length}, expected 13")
            }
            Error::InvalidDimensions { width, height } => {
                write!(f, "invalid image dimensions {width}x{height}")
            }
            Error::InvalidColorType(value) => write!(f, "invalid color type {value}"),
            Error::InvalidBitDepth { color_type, bit_depth } => {
                write!(f, "bit depth {bit_depth} is not allowed for color type {color_type:?}")
            }
            Error::InvalidCompressionMethod(value) => {
                write!(f, "invalid compression method {value}")
            }
            Error::InvalidFilterMethod(value) => write!(f, "invalid filter method {value}"),
            Error::InvalidInterlaceMethod(value) => write!(f, "invalid interlace method {value}"),
            Error::MissingImageData => f.write_str("no IDAT chunk before IEND"),
            Error::NonConsecutiveImageData => f.write_str("IDAT chunks are not consecutive"),
            Error::MissingImageEnd => f.write_str("no IEND chunk"),
            Error::Decompression(error) => write!(f, "corrupt image data: {error}"),
            Error::ImageDataTooShort { expected, actual } => {
                write!(f, "image data is {actual} bytes, expected {expected}")
            }
            Error::ImageDataTooLong { expected } => {
                write!(f, "image data is longer than the expected {expected} bytes")
            }
            Error::InvalidFilterType(value) => write!(f, "invalid filter type {value}"),
            Error::ImageTooLarge => f.write_str("image is too large for this platform"),
            Error::MissingPalette => f.write_str("indexed image has no PLTE chunk"),
            Error::InvalidPaletteLength(length) => {
                write!(f, "PLTE length is {length}, expected a multiple of 3 from 3 to 768")
            }
            Error::TooManyPaletteEntries { entries, bit_depth } => {
                write!(f, "palette has {entries} entries, more than {bit_depth}-bit indices can reach")
            }
            Error::UnexpectedPalette(color_type) => write!(f, "PLTE chunk is not allowed for color type {color_type:?}"),
            Error::DuplicatePalette => f.write_str("more than one PLTE chunk"),
            Error::PaletteAfterImageData => f.write_str("PLTE chunk comes after IDAT"),
            Error::PaletteIndexOutOfRange { index, entries } => {
                write!(f, "palette index {index} is out of range for a palette of {entries} entries")
            }
            Error::InvalidTransparencyLength { color_type, length } => {
                write!(f, "tRNS length {length} is invalid for color type {color_type:?}")
            }
            Error::TooManyTransparencyEntries { entries, palette_entries } => {
                write!(f, "tRNS has {entries} entries, more than the palette's {palette_entries}")
            }
            Error::UnexpectedTransparency(color_type) => {
                write!(f, "tRNS chunk is not allowed for color type {color_type:?}")
            }
            Error::DuplicateTransparency => f.write_str("more than one tRNS chunk"),
            Error::TransparencyAfterImageData => f.write_str("tRNS chunk comes after IDAT"),
            Error::TransparencyBeforePalette => f.write_str("tRNS chunk comes before PLTE"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Decompression(error) => Some(error),
            _ => None,
        }
    }
}