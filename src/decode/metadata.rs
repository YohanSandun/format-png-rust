//! Reads ancillary chunks while decoding: typed ones into [`Metadata`], and raw
//! copies of all of them into [`OwnedChunk`]s.
//!
//! To support a new chunk: add its type under `png::metadata`, a field and
//! accessor on `Metadata`, and an arm in `read_known_chunk` with its placement rule.

use rust_deflate::Decompressor;

use crate::error::Error;
use crate::png::metadata::{Chromaticities, CodingIndependentCodePoints, Exif, Gamma, IccProfile, PhysicalDimensions, RenderingIntent, Text, Time};
use crate::png::{Chunk, ChunkPosition, ChunkType, Metadata, OwnedChunk};

/// Appends a copy of `chunk`, found at `position`, to `chunks` if it's ancillary.
/// Critical chunks (`IHDR`, `PLTE`, `IDAT`, `IEND`, and unknown critical ones)
/// are never copied: the decoder has already used them.
pub(crate) fn preserve_chunk(chunk: &Chunk<'_>, position: ChunkPosition, chunks: &mut Vec<OwnedChunk>) {
    if !chunk.chunk_type().is_critical() {
        chunks.push(OwnedChunk::new(chunk, position));
    }
}

/// Reads `chunk`, found at `position`, into `metadata` if it's a known ancillary
/// chunk. Unknown chunks are skipped.
///
/// With `strict` off, a chunk that fails `read_known_chunk` is skipped and the
/// decode goes on, as the PNG spec allows for ancillary chunks. With `strict` on,
/// the error is returned.
pub(crate) fn read_ancillary(
    chunk: &Chunk<'_>,
    position: ChunkPosition,
    strict: bool,
    decompressor: &mut Decompressor,
    metadata: &mut Metadata,
) -> Result<(), Error> {
    match read_known_chunk(chunk, position, decompressor, metadata) {
        Err(error) if strict => Err(error),
        _ => Ok(()),
    }
}

/// Parses `chunk` into its field of `metadata`, if it's one of the chunks
/// `Metadata` holds, and does nothing for other chunks.
///
/// On an error `metadata` must be left unchanged, so a skipped chunk leaves no
/// trace. Check everything first, then store.
///
/// Errors:
/// - `Error::MisplacedChunk` if `check_position` rejects the chunk's position.
/// - `Error::DuplicateChunk` if the field is already set: each of these chunks
///   may appear only once, and the first one is kept.
/// - Any error from the chunk type's `parse`.
fn read_known_chunk(
    chunk: &Chunk<'_>,
    position: ChunkPosition,
    decompressor: &mut Decompressor,
    metadata: &mut Metadata,
) -> Result<(), Error> {
    let chunk_type = chunk.chunk_type();
    check_position(chunk_type, position)?;

    let data = chunk.data();
    match chunk_type {
        ChunkType::GAMA => store(&mut metadata.gamma, chunk_type, || Gamma::parse(data)),
        ChunkType::CHRM => store(&mut metadata.chromaticities, chunk_type, || Chromaticities::parse(data)),
        ChunkType::SRGB => store(&mut metadata.srgb, chunk_type, || RenderingIntent::parse(data)),
        ChunkType::PHYS => store(&mut metadata.physical_dimensions, chunk_type, || PhysicalDimensions::parse(data)),
        ChunkType::TIME => store(&mut metadata.time, chunk_type, || Time::parse(data)),
        ChunkType::TEXT => push(&mut metadata.text, Text::parse_text(data)),
        ChunkType::ZTXT => push(&mut metadata.text, Text::parse_compressed_with(data, decompressor, Text::DEFAULT_MAX_SIZE)),
        ChunkType::ITXT => push(&mut metadata.text, Text::parse_international_with(data, decompressor, Text::DEFAULT_MAX_SIZE)),
        ChunkType::ICCP => store(&mut metadata.icc_profile, chunk_type, || IccProfile::parse_with(data, decompressor, IccProfile::DEFAULT_MAX_SIZE)),
        ChunkType::CICP => store(&mut metadata.cicp, chunk_type, || CodingIndependentCodePoints::parse(data)),
        ChunkType::EXIF => store(&mut metadata.exif, chunk_type, || Exif::parse(data)),
        _ => Ok(()),
    }
}

/// Fills `slot` with the result of `parse`, for a chunk that may appear only once.
/// Returns `Error::DuplicateChunk` if `slot` is already filled, or `parse`'s error;
/// either way `slot` is left as it was.
fn store<T>(slot: &mut Option<T>, chunk_type: ChunkType, parse: impl FnOnce() -> Result<T, Error>) -> Result<(), Error> {
    if slot.is_some() {
        return Err(Error::DuplicateChunk(chunk_type));
    }
    *slot = Some(parse()?);
    Ok(())
}

/// Appends the result of a parse to `list`, for a chunk that may appear more than
/// once. On an error `list` is left as it was.
fn push<T>(list: &mut Vec<T>, parsed: Result<T, Error>) -> Result<(), Error> {
    list.push(parsed?);
    Ok(())
}

/// Checks that a chunk of `chunk_type` may appear at `position`:
/// - `gAMA`, `cHRM`, `sRGB`, `iCCP` and `cICP` must come before `PLTE` and the
///   image data, so only `ChunkPosition::BeforePalette` is allowed.
/// - `pHYs` and `eXIf` must come before the image data.
/// - `tIME` may appear anywhere.
///
/// Returns `Error::MisplacedChunk` otherwise. Other chunk types are always allowed.
fn check_position(chunk_type: ChunkType, position: ChunkPosition) -> Result<(), Error> {
    match chunk_type {
        ChunkType::GAMA | ChunkType::CHRM | ChunkType::SRGB | ChunkType::ICCP | ChunkType::CICP => {
            if position != ChunkPosition::BeforePalette {
                Err(Error::MisplacedChunk(chunk_type))
            } else {
                Ok(())
            }
        },
        ChunkType::PHYS | ChunkType::EXIF => {
            if position == ChunkPosition::AfterImageData {
                Err(Error::MisplacedChunk(chunk_type))
            } else {
                Ok(())
            }
        },
        _ => Ok(())
    }
}

#[cfg(test)]
mod tests;
