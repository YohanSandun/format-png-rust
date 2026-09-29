use crate::error::Error;
use crate::io::ByteReader;

const MAX_DIMENSION: u32 = 2_147_483_647;

/// How pixels are stored, from the `IHDR` color type byte.
///
/// The discriminants are the values used in the file, so `color_type as u8` gives the byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColorType {
    /// One gray sample per pixel. Bit depths 1, 2, 4, 8 or 16.
    Grayscale = 0,
    /// Red, green and blue samples. Bit depths 8 or 16.
    Rgb = 2,
    /// One index into the `PLTE` palette per pixel. Bit depths 1, 2, 4 or 8.
    Indexed = 3,
    /// Gray and alpha samples. Bit depths 8 or 16.
    GrayscaleAlpha = 4,
    /// Red, green, blue and alpha samples. Bit depths 8 or 16.
    Rgba = 6,
}

impl TryFrom<u8> for ColorType {
    type Error = Error;

    /// Converts the color type byte from `IHDR`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidColorType`] for anything other than 0, 2, 3, 4 or 6.
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(ColorType::Grayscale),
            2 => Ok(ColorType::Rgb),
            3 => Ok(ColorType::Indexed),
            4 => Ok(ColorType::GrayscaleAlpha),
            6 => Ok(ColorType::Rgba),
            _ => Err(Error::InvalidColorType(value)),
        }
    }
}

/// The interlace method from `IHDR`.
///
/// The discriminants are the values used in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Interlace {
    /// Rows are stored top to bottom.
    None = 0,
    /// Pixels are stored in seven passes, each a finer subset of the image.
    Adam7 = 1,
}

impl TryFrom<u8> for Interlace {
    type Error = Error;

    /// Converts the interlace method byte from `IHDR`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidInterlaceMethod`] for anything other than 0 or 1.
    fn try_from(value: u8) -> Result<Self, Self::Error> {
       match value {
           0 => Ok(Interlace::None),
           1 => Ok(Interlace::Adam7),
           _ => Err(Error::InvalidInterlaceMethod(value))
       }
    }
}

/// The contents of the `IHDR` chunk, the first chunk of every PNG.
///
/// Compression and filter methods aren't stored: PNG defines only method 0 for
/// each, and [`ImageHeader::parse`] rejects anything else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImageHeader {
    /// Width in pixels, from 1 to 2^31 - 1.
    pub width: u32,
    /// Height in pixels, from 1 to 2^31 - 1.
    pub height: u32,
    /// Bits per sample, or per palette index for [`ColorType::Indexed`]. Always valid
    /// for `color_type`.
    pub bit_depth: u8,
    /// How pixels are stored.
    pub color_type: ColorType,
    /// Whether the image data is interlaced.
    pub interlace: Interlace,
}

impl ImageHeader {
    /// Length of the `IHDR` chunk data in bytes.
    pub const LENGTH: usize = 13;
    
    /// Parses and validates the data of an `IHDR` chunk, without its length, type or CRC.
    ///
    /// Use this with [`Chunk::data`](crate::png::Chunk::data) when reading chunks
    /// yourself; [`Decoder::read_header`](crate::Decoder::read_header) does it for you.
    ///
    /// # Errors
    ///
    /// - [`Error::InvalidImageHeaderLength`] if `data` isn't exactly 13 bytes.
    /// - [`Error::InvalidDimensions`] if the width or height is 0 or above 2^31 - 1.
    /// - [`Error::InvalidColorType`] for an unknown color type, and
    ///   [`Error::InvalidBitDepth`] for a bit depth that color type doesn't allow.
    /// - [`Error::InvalidCompressionMethod`], [`Error::InvalidFilterMethod`] or
    ///   [`Error::InvalidInterlaceMethod`] for an unknown method.
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        if data.len() != Self::LENGTH {
            return Err(Error::InvalidImageHeaderLength(data.len()));
        }

        let mut reader = ByteReader::new(data);
        let width = reader.read_u32()?;
        let height = reader.read_u32()?;

        if width == 0 || height == 0 || width > MAX_DIMENSION || height > MAX_DIMENSION {
            return Err(Error::InvalidDimensions { width, height });
        }

        let bit_depth = reader.read_u8()?;
        let color_type = ColorType::try_from(reader.read_u8()?)?;
        let bit_depth_allowed = match color_type {
            ColorType::Grayscale => matches!(bit_depth, 1 | 2 | 4 | 8 | 16),
            ColorType::Indexed => matches!(bit_depth, 1 | 2 | 4 | 8),
            ColorType::Rgb | ColorType::GrayscaleAlpha | ColorType::Rgba => matches!(bit_depth, 8 | 16),
        };
        if !bit_depth_allowed {
            return Err(Error::InvalidBitDepth { color_type, bit_depth });
        }

        let compression_method = reader.read_u8()?;
        if compression_method != 0 {
            return Err(Error::InvalidCompressionMethod(compression_method));
        }

        let filter_method = reader.read_u8()?;
        if filter_method != 0 {
            return Err(Error::InvalidFilterMethod(filter_method));
        }

        let interlace = Interlace::try_from(reader.read_u8()?)?;

        Ok(Self {
            width,
            height,
            bit_depth,
            color_type,
            interlace,
        })
    }
}

#[cfg(test)]
mod tests;
