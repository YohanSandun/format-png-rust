use super::chunk_reader::ChunkReader;
use crate::error::Error;
use crate::png::ChunkType;

#[derive(Clone, Copy)]
enum IdatState {
    NotSeen,
    InRun,
    Done,
}

/// Reads the chunks after `IHDR` up to and including `IEND`, appending the data
/// of every `IDAT` chunk to `out`, so it holds the whole zlib stream. Other chunks
/// are skipped for now. Nothing after `IEND` is read.
///
/// Errors:
/// - `Error::MissingImageData` if `IEND` comes before any `IDAT`.
/// - `Error::NonConsecutiveImageData` if another chunk comes between two `IDAT`s.
/// - `Error::MissingImageEnd` if the input ends before `IEND`.
/// - Any error from `ChunkReader::next_chunk`.
pub(crate) fn collect_image_data(chunks: &mut ChunkReader<'_>, out: &mut Vec<u8>) -> Result<(), Error> {
    let mut state = IdatState::NotSeen;

    while let Some(chunk) = chunks.next_chunk()? {
        match (chunk.chunk_type(), state) {
            (ChunkType::IDAT, IdatState::Done) => return Err(Error::NonConsecutiveImageData),
            (ChunkType::IDAT, _) => {
                out.extend_from_slice(chunk.data());
                state = IdatState::InRun;
            }
            (ChunkType::IEND, IdatState::NotSeen) => return Err(Error::MissingImageData),
            (ChunkType::IEND, _) => return Ok(()),
            (_, IdatState::InRun) => state = IdatState::Done,
            _ => {}
        }
    }

    Err(Error::MissingImageEnd)
}

#[cfg(test)]
mod tests;
