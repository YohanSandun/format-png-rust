use rust_deflate::Decompressor;

use super::text::{decompress, read_keyword};
use crate::error::Error;
use crate::png::ChunkType;

/// The only compression method defined for `iCCP`: zlib.
const COMPRESSION_METHOD_ZLIB: u8 = 0;

/// The `iCCP` chunk: an embedded ICC color profile.
///
/// The profile is kept as the ICC data, decompressed but not interpreted. Pass
/// [`profile`](Self::profile) to a color management library, such as lcms2, to
/// apply it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IccProfile {
    /// The profile's name, 1 to 79 bytes of printable Latin-1, following the
    /// same rules as a text keyword. Only meaningful to people: `ICC Profile`
    /// and `Photoshop ICC profile` are common.
    pub name: String,
    /// The ICC profile, decompressed.
    pub profile: Vec<u8>,
}

impl IccProfile {
    /// The cap on the decompressed profile that [`parse`](Self::parse) uses, and
    /// the decoder uses when collecting [`Metadata`](crate::Metadata): 16 MB.
    /// Profiles are rarely over 1 MB, but some printer profiles are a few MB.
    pub const DEFAULT_MAX_SIZE: usize = 16_000_000;

    /// Parses `iCCP` chunk data: the profile name, a null byte, the compression
    /// method (0), then the profile as a zlib stream.
    ///
    /// The profile is capped at [`DEFAULT_MAX_SIZE`](Self::DEFAULT_MAX_SIZE)
    /// bytes once decompressed, and a decompressor is set up for this call. To
    /// choose the cap, or reuse a decompressor, use [`parse_with`](Self::parse_with).
    ///
    /// # Errors
    ///
    /// Same as [`parse_with`](Self::parse_with).
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        Self::parse_with(data, &mut Decompressor::new(), Self::DEFAULT_MAX_SIZE)
    }

    /// Like [`parse`](Self::parse), but decompresses with `decompressor` and caps
    /// the decompressed profile at `max_size` bytes.
    ///
    /// The profile isn't checked beyond decompressing: an ICC header that's
    /// truncated or invalid, or an empty profile, is still returned.
    ///
    /// # Errors
    ///
    /// - [`Error::InvalidChunkData`] if there is no null byte, the name is
    ///   invalid, the compression method is missing or isn't 0, or the zlib
    ///   stream is corrupt.
    /// - [`Error::TextTooLong`] if the profile decompresses to more than
    ///   `max_size` bytes.
    pub fn parse_with(data: &[u8], decompressor: &mut Decompressor, max_size: usize) -> Result<Self, Error> {
        let (name, rest) = read_keyword(data, ChunkType::ICCP)?;
        let [method, compressed @ ..] = rest else {
            return Err(Error::InvalidChunkData(ChunkType::ICCP));
        };
        if *method != COMPRESSION_METHOD_ZLIB {
            return Err(Error::InvalidChunkData(ChunkType::ICCP));
        }

        Ok(Self { name, profile: decompress(compressed, ChunkType::ICCP, decompressor, max_size)? })
    }
}

#[cfg(test)]
mod tests;
