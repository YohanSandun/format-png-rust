use crate::error::Error;
use crate::png::ChunkType;

/// The `gAMA` chunk: the gamma the image was encoded with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Gamma(u32);

impl Gamma {
    /// Parses `gAMA` chunk data: one 4-byte big-endian value, the gamma times 100000.
    ///
    /// # Errors
    ///
    /// - [`Error::InvalidChunkLength`] if `data` isn't 4 bytes.
    /// - [`Error::InvalidChunkData`] if the value is 0, which isn't a valid gamma.
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        if data.len() != 4 {
            return Err(Error::InvalidChunkLength { chunk_type: ChunkType::GAMA, length: data.len() });
        }

        let value = u32::from_be_bytes(data[..4].try_into().unwrap());
        if value == 0 {
            return Err(Error::InvalidChunkData(ChunkType::GAMA));
        }
        Ok(Gamma(value))
    }

    /// The value as stored: the gamma times 100000. 45455 means 1/2.2.
    pub fn scaled(&self) -> u32 {
        self.0
    }

    /// The gamma, for example 0.45455.
    pub fn value(&self) -> f64 {
        f64::from(self.0) / 100_000.0
    }
}

/// The `cHRM` chunk: the CIE 1931 x and y chromaticities of the white point and
/// the red, green and blue primaries. Each value is stored times 100000, so
/// 31270 means 0.3127.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Chromaticities {
    pub white_x: u32,
    pub white_y: u32,
    pub red_x: u32,
    pub red_y: u32,
    pub green_x: u32,
    pub green_y: u32,
    pub blue_x: u32,
    pub blue_y: u32,
}

impl Chromaticities {
    /// Parses `cHRM` chunk data: eight 4-byte big-endian values in the order of
    /// the fields above.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidChunkLength`] if `data` isn't 32 bytes.
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        if data.len() != 32 {
            return Err(Error::InvalidChunkLength { chunk_type: ChunkType::CHRM, length: data.len() });
        }

        Ok(Self {
            white_x: u32::from_be_bytes(data[..4].try_into().unwrap()),
            white_y: u32::from_be_bytes(data[4..8].try_into().unwrap()),
            red_x: u32::from_be_bytes(data[8..12].try_into().unwrap()),
            red_y: u32::from_be_bytes(data[12..16].try_into().unwrap()),
            green_x: u32::from_be_bytes(data[16..20].try_into().unwrap()),
            green_y: u32::from_be_bytes(data[20..24].try_into().unwrap()),
            blue_x: u32::from_be_bytes(data[24..28].try_into().unwrap()),
            blue_y: u32::from_be_bytes(data[28..].try_into().unwrap()),
        })
    }
}

/// The `sRGB` chunk: the image is in the sRGB color space, and how colors outside
/// a display's range should be handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderingIntent {
    Perceptual = 0,
    RelativeColorimetric = 1,
    Saturation = 2,
    AbsoluteColorimetric = 3,
}

impl RenderingIntent {
    /// Parses `sRGB` chunk data: one byte, 0 to 3.
    ///
    /// # Errors
    ///
    /// - [`Error::InvalidChunkLength`] if `data` isn't 1 byte.
    /// - [`Error::InvalidChunkData`] if the byte is above 3.
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        if data.len() != 1 {
            return Err(Error::InvalidChunkLength { chunk_type: ChunkType::SRGB, length: data.len() });
        }

        Ok(match data[0] {
            0 => Self::Perceptual,
            1 => Self::RelativeColorimetric,
            2 => Self::Saturation,
            3 => Self::AbsoluteColorimetric,
            _ => return Err(Error::InvalidChunkData(ChunkType::SRGB)),
        })
    }
}

#[cfg(test)]
mod tests;
