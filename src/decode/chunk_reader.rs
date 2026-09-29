use crate::error::Error;
use crate::io::ByteReader;
use crate::png::{Chunk, ChunkType, SIGNATURE};
use crate::png::crc::crc32;

const MAX_CHUNK_SIZE: u32 = 2_147_483_647;

/// Reads the chunks of a PNG one at a time, without interpreting them.
///
/// This is the low-level view: every chunk comes back as a [`Chunk`], including
/// ancillary, private and unknown ones, so you can handle them however you like.
/// Chunks borrow from the input, so it must outlive them.
///
/// ```
/// use format_png::ChunkReader;
///
/// let data = std::fs::read("tests/data/valid/ancillary_chunks.png")?;
/// let mut chunks = ChunkReader::new(&data)?;
///
/// while let Some(chunk) = chunks.next_chunk()? {
///     if !chunk.chunk_type().is_critical() {
///         println!("{}: {} bytes", chunk.chunk_type(), chunk.data().len());
///     }
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// To take CRC validation from [`DecodeOptions`](crate::DecodeOptions), create the
/// reader with [`Decoder::chunks`](crate::Decoder::chunks) instead.
pub struct ChunkReader<'a> {
    reader: ByteReader<'a>,
    validate_crc: bool,
}

impl<'a> ChunkReader<'a> {
    /// Checks the PNG signature and positions the reader at the first chunk. CRC
    /// validation is on.
    ///
    /// # Errors
    ///
    /// - [`Error::InvalidSignature`] if `data` doesn't start with
    ///   [`SIGNATURE`](crate::png::SIGNATURE).
    /// - [`Error::UnexpectedEndOfInput`] if `data` is shorter than the signature but
    ///   matches it as far as it goes, as with a truncated PNG.
    pub fn new(data: &'a [u8]) -> Result<Self, Error> {
        let mut reader = ByteReader::new(data);

        // Too short: a truncated PNG if what's there is the start of the signature,
        // otherwise not a PNG at all.
        let signature = reader.read_bytes(SIGNATURE.len()).map_err(|_| {
            if SIGNATURE.starts_with(data) {
                Error::UnexpectedEndOfInput
            } else {
                Error::InvalidSignature
            }
        })?;

        if signature != SIGNATURE {
            return Err(Error::InvalidSignature)
        }

        Ok(Self {
            reader,
            validate_crc: true,
        })
    }

    /// Turns CRC validation on or off for the chunks read after this call.
    ///
    /// With it off, [`Chunk::crc`] still returns the stored CRC, unchecked.
    pub fn validate_crc(mut self, enabled: bool) -> Self {
        self.validate_crc = enabled;
        self
    }

    /// Reads the next chunk, or returns `Ok(None)` once the input is exhausted.
    ///
    /// It doesn't stop at `IEND` or check chunk order: anything after `IEND` is
    /// returned too.
    ///
    /// # Errors
    ///
    /// - [`Error::ChunkTooLong`] if the length field exceeds 2^31 - 1.
    /// - [`Error::InvalidChunkType`] if the type isn't four ASCII letters.
    /// - [`Error::UnexpectedEndOfInput`] if the chunk is truncated.
    /// - [`Error::CrcMismatch`] if CRC validation is on and the CRC is wrong.
    pub fn next_chunk(&mut self) -> Result<Option<Chunk<'a>>, Error> {
        if self.reader.is_empty() {
            return Ok(None)
        }

        let length = self.reader.read_u32()?;
        if length > MAX_CHUNK_SIZE {
            return Err(Error::ChunkTooLong(length));
        }

        let chunk_type_bytes: [u8; 4] = self.reader.read_bytes(4)?.try_into().unwrap();
        let chunk_type = ChunkType::from_bytes(chunk_type_bytes)?;

        let data = self.reader.read_bytes(length as usize)?;

        let crc = self.reader.read_u32()?;
        if self.validate_crc {
            let actual_crc = crc32(&chunk_type_bytes, data);
            if crc != actual_crc {
                return Err(Error::CrcMismatch { expected: crc, actual: actual_crc });
            }
        }

        Ok(Some(Chunk::new(chunk_type, data, crc)))
    }
}

#[cfg(test)]
mod tests;
