use std::fmt;

use crate::error::Error;

const ASCII_UPPERCASE_A: u8 = 0x41;
const ASCII_UPPERCASE_Z: u8 = 0x5A;

/// A 4-byte PNG chunk type such as `IHDR` or `tEXt`.
///
/// The case of each letter carries a property bit (bit 5): see [`is_critical`],
/// [`is_public`], [`is_reserved_bit_valid`] and [`is_safe_to_copy`].
///
/// [`is_critical`]: ChunkType::is_critical
/// [`is_public`]: ChunkType::is_public
/// [`is_reserved_bit_valid`]: ChunkType::is_reserved_bit_valid
/// [`is_safe_to_copy`]: ChunkType::is_safe_to_copy
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkType([u8; 4]);

impl ChunkType {
    /// Image header, always the first chunk.
    pub const IHDR: ChunkType = ChunkType(*b"IHDR");
    /// Palette.
    pub const PLTE: ChunkType = ChunkType(*b"PLTE");
    /// Image data. There can be several, which must be consecutive.
    pub const IDAT: ChunkType = ChunkType(*b"IDAT");
    /// Image trailer, always the last chunk.
    pub const IEND: ChunkType = ChunkType(*b"IEND");
    /// Transparency for images without an alpha channel.
    pub const TRNS: ChunkType = ChunkType(*b"tRNS");
    /// Image gamma.
    pub const GAMA: ChunkType = ChunkType(*b"gAMA");
    /// Primary chromaticities and white point.
    pub const CHRM: ChunkType = ChunkType(*b"cHRM");
    /// The image is in the sRGB color space.
    pub const SRGB: ChunkType = ChunkType(*b"sRGB");
    /// Physical pixel dimensions.
    pub const PHYS: ChunkType = ChunkType(*b"pHYs");
    /// Last modification time.
    pub const TIME: ChunkType = ChunkType(*b"tIME");

    /// Creates a chunk type, checking every byte is an ASCII letter.
    ///
    /// ```
    /// use format_png::png::ChunkType;
    ///
    /// let text = ChunkType::from_bytes(*b"tEXt")?;
    /// assert!(!text.is_critical());
    /// assert!(ChunkType::from_bytes(*b"tEX1").is_err());
    /// # Ok::<(), format_png::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidChunkType`] if any byte is outside `A-Z` / `a-z`.
    pub fn from_bytes(bytes: [u8; 4]) -> Result<Self, Error> {
        if !bytes.iter().all(u8::is_ascii_alphabetic) {
            return Err(Error::InvalidChunkType(bytes));
        }
        Ok(ChunkType(bytes))
    }

    /// The four type bytes, as they appear in the file.
    pub fn as_bytes(&self) -> &[u8; 4] {
        &self.0
    }

    /// Critical chunks (uppercase first letter) must be understood to display the
    /// image. Ancillary chunks (lowercase) can be ignored by decoders that don't know them.
    pub fn is_critical(&self) -> bool {
        self.0[0] >= ASCII_UPPERCASE_A && self.0[0] <= ASCII_UPPERCASE_Z
    }

    /// Public chunks (uppercase second letter) are defined by the PNG specification or
    /// registered with it. Private chunks (lowercase) are application-specific.
    pub fn is_public(&self) -> bool {
        self.0[1] >= ASCII_UPPERCASE_A && self.0[1] <= ASCII_UPPERCASE_Z
    }

    /// The third letter must be uppercase in the current version of PNG. A chunk with a
    /// lowercase third letter isn't invalid, but should be treated as unknown.
    pub fn is_reserved_bit_valid(&self) -> bool {
        self.0[2] >= ASCII_UPPERCASE_A && self.0[2] <= ASCII_UPPERCASE_Z
    }

    /// Safe-to-copy chunks (lowercase fourth letter) may be copied by editors that don't
    /// understand them, even after changing critical chunks such as the image data.
    pub fn is_safe_to_copy(&self) -> bool {
        !(self.0[3] >= ASCII_UPPERCASE_A && self.0[3] <= ASCII_UPPERCASE_Z)
    }
}

impl fmt::Debug for ChunkType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ChunkType({})", self)
    }
}

impl fmt::Display for ChunkType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&String::from_utf8_lossy(&self.0))
    }
}

/// One chunk as it appears in the file. Its data is borrowed from the input, not copied.
///
/// This is the raw view for users who handle chunks themselves, including
/// ancillary, private and unknown ones. Read chunks with a [`ChunkReader`].
///
/// [`ChunkReader`]: crate::ChunkReader
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chunk<'a> {
    chunk_type: ChunkType,
    data: &'a [u8],
    crc: u32,
}

impl<'a> Chunk<'a> {
    pub(crate) fn new(chunk_type: ChunkType, data: &'a [u8], crc: u32) -> Self {
        Self { chunk_type, data, crc }
    }

    /// The chunk's type.
    pub fn chunk_type(&self) -> ChunkType {
        self.chunk_type
    }

    /// The chunk's data, without the length, type and CRC fields.
    pub fn data(&self) -> &'a [u8] {
        self.data
    }

    /// The CRC stored in the file. It has already been checked unless CRC validation was
    /// turned off.
    pub fn crc(&self) -> u32 {
        self.crc
    }
}

#[cfg(test)]
mod tests;
