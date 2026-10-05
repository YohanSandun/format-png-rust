//! Writes extra chunks: raw [`OwnedChunk`]s given with
//! [`ImageRef::with_chunks`](crate::ImageRef::with_chunks), such as the ones
//! [`DecodeOptions::preserve_chunks`](crate::DecodeOptions::preserve_chunks)
//! keeps, or an application's own.
//!
//! Which chunks are written follows the spec's rules for editors copying
//! chunks they don't understand:
//!
//! - Critical chunks are an error: the encoder writes `IHDR`, `PLTE`, `IDAT`
//!   and `IEND` itself, and an unknown critical chunk can't be written safely.
//! - Unsafe-to-copy chunks (an uppercase fourth letter, such as `bKGD` or `sBIT`)
//!   depend on the image data, which re-encoding rewrites, so they're skipped
//!   unless [`EncodeOptions::keep_unsafe_chunks`](crate::EncodeOptions::keep_unsafe_chunks)
//!   is set.
//! - A chunk of a type the encoder writes itself for this image, from
//!   [`ImageRef::with_transparency`](crate::ImageRef::with_transparency) or
//!   [`ImageRef::with_metadata`](crate::ImageRef::with_metadata), is skipped,
//!   so nothing is written twice. Raw chunks of those types are written when the
//!   image has no typed value for them, so re-encoding an image decoded with
//!   `preserve_chunks` alone keeps them.
//!
//! Each chunk is written at its [`ChunkPosition`], in the order given.

use super::chunk_writer::{MAX_CHUNK_LENGTH, write_chunk};
use super::image_ref::ImageRef;
use crate::error::Error;
use crate::png::{ChunkPosition, ChunkType, OwnedChunk};

/// Checks that every chunk in `chunks` can be written.
///
/// Errors:
/// - `Error::UnexpectedCriticalChunk` for a critical chunk, known or not.
/// - `Error::InvalidChunkLength` for data longer than `MAX_CHUNK_LENGTH`.
pub(crate) fn validate_extra_chunks(chunks: &[OwnedChunk]) -> Result<(), Error> {
    for chunk in chunks {
        if chunk.chunk_type().is_critical() {
            return Err(Error::UnexpectedCriticalChunk(chunk.chunk_type()));
        }

        if chunk.data().len() > MAX_CHUNK_LENGTH {
            return Err(Error::InvalidChunkLength {
                chunk_type: chunk.chunk_type(),
                length: chunk.data().len(),
            });
        }
    }
    Ok(())
}

/// Whether the encoder writes chunks of `chunk_type` itself for `image`:
///
/// - `tRNS` if the image has transparency.
/// - Each metadata chunk type the image's [`Metadata`](crate::Metadata) has a
///   value for: `gAMA` if it has a gamma, and so on. `tEXt`, `zTXt` and `iTXt`
///   all count as written if it has any text, so raw text chunks never mix with
///   the typed ones.
pub(crate) fn encoder_writes(chunk_type: ChunkType, image: &ImageRef<'_>) -> bool {
    if chunk_type == ChunkType::TRNS {
        return image.transparency().is_some();
    }
    let Some(metadata) = image.metadata() else {
        return false;
    };

    match chunk_type {
        ChunkType::GAMA => metadata.gamma().is_some(),
        ChunkType::CHRM => metadata.chromaticities().is_some(),
        ChunkType::SRGB => metadata.srgb().is_some(),
        ChunkType::PHYS => metadata.physical_dimensions().is_some(),
        ChunkType::TIME => metadata.time().is_some(),
        ChunkType::ICCP => metadata.icc_profile().is_some(),
        ChunkType::CICP => metadata.cicp().is_some(),
        ChunkType::EXIF => metadata.exif().is_some(),
        ChunkType::TEXT | ChunkType::ZTXT | ChunkType::ITXT => !metadata.text().is_empty(),
        _ => false,
    }
}

/// Whether `chunk` should be written for `image`, by the rules in the module
/// docs. `chunk` has passed `validate_extra_chunks`, so isn't critical.
pub(crate) fn should_write(chunk: &OwnedChunk, image: &ImageRef<'_>, keep_unsafe: bool) -> bool {
    (chunk.chunk_type().is_safe_to_copy() || keep_unsafe)
        && !encoder_writes(chunk.chunk_type(), image)
}

/// Appends the chunks of `image` at `position` that `should_write` keeps, in order.
pub(crate) fn write_extra_chunks(
    out: &mut Vec<u8>,
    image: &ImageRef<'_>,
    position: ChunkPosition,
    keep_unsafe: bool,
) {
    for chunk in image.chunks() {
        if chunk.position() == position && should_write(chunk, image, keep_unsafe) {
            write_chunk(out, chunk.chunk_type(), chunk.data());
        }
    }
}

#[cfg(test)]
mod tests;
