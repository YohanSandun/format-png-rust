use crate::error::Error;
use crate::png::ChunkType;

/// The length of the TIFF header an `eXIf` chunk starts with: the byte order,
/// the number 42, and the offset of the first directory.
const TIFF_HEADER_LENGTH: usize = 8;

/// The byte order of [`Exif`] data, from its first two bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExifByteOrder {
    /// `MM`: big-endian, "Motorola" order.
    BigEndian,
    /// `II`: little-endian, "Intel" order.
    LittleEndian,
}

/// The `eXIf` chunk: Exif metadata, such as the camera, exposure and orientation.
///
/// The data is kept raw, as it's stored in a JPEG's APP1 segment after the
/// `Exif\0\0` prefix: a TIFF header, then the directories. Only the header is
/// checked; pass [`data`](Self::data) to an Exif library to read the tags.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Exif {
    data: Vec<u8>,
    byte_order: ExifByteOrder,
}

impl Exif {
    /// Parses `eXIf` chunk data, checking that it starts with a TIFF header:
    /// `MM` then 42 big-endian (`4D 4D 00 2A`), or `II` then 42 little-endian
    /// (`49 49 2A 00`), then a 4-byte directory offset.
    ///
    /// # Errors
    ///
    /// - [`Error::InvalidChunkLength`] if `data` is shorter than the 8-byte TIFF header.
    /// - [`Error::InvalidChunkData`] if it doesn't start with either byte order
    ///   and 42 in that order.
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        if data.len() < TIFF_HEADER_LENGTH {
            return Err(Error::InvalidChunkLength { chunk_type: ChunkType::EXIF, length: data.len() });
        }

        if data[0] == 0x49 && data[1] == 0x49 && data[2] == 0x2A && data[3] == 0 {
            Ok(Self {
                byte_order: ExifByteOrder::LittleEndian,
                data: data.to_vec(),
            })
        } else if data[0] == 0x4D && data[1] == 0x4D && data[2] == 0 && data[3] == 0x2A {
            Ok(Self {
                byte_order: ExifByteOrder::BigEndian,
                data: data.to_vec(),
            })
        } else {
            Err(Error::InvalidChunkData(ChunkType::EXIF))
        }
    }

    /// The Exif data, starting with the TIFF header.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// The byte order every value in [`data`](Self::data) uses.
    pub fn byte_order(&self) -> ExifByteOrder {
        self.byte_order
    }
}

#[cfg(test)]
mod tests;
