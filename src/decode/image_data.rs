use rust_deflate::Decompressor;

use super::chunk_reader::ChunkReader;
use super::metadata::{preserve_chunk, read_ancillary};
use super::options::DecodeOptions;
use crate::ColorType;
use crate::error::Error;
use crate::png::{
    Chunk, ChunkPosition, ChunkType, ImageChunks, ImageHeader, Palette, Transparency,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum IdatState {
    NotSeen,
    InRun,
    Done,
}

/// Reads the chunks after `IHDR` up to and including `IEND`, appending the data
/// of every `IDAT` chunk to `out`, so it holds the whole zlib stream, and returns
/// the other chunks the image needs. Ancillary chunks are parsed into metadata
/// and copied as raw chunks if `options` asks for it, and skipped otherwise.
/// Nothing after `IEND` is read.
///
/// Errors:
/// - `Error::MissingImageData` if `IEND` comes before any `IDAT`.
/// - `Error::NonConsecutiveImageData` if another chunk comes between two `IDAT`s.
/// - `Error::MissingImageEnd` if the input ends before `IEND`.
/// - `Error::TransparencyBeforePalette` if a `PLTE` follows a `tRNS`.
/// - Any error from `read_palette`, `require_palette`, `read_transparency` or
///   `read_ancillary`.
/// - Any error from `ChunkReader::next_chunk`.
pub(crate) fn collect_image_data(
    chunks: &mut ChunkReader<'_>,
    header: &ImageHeader,
    options: &DecodeOptions,
    decompressor: &mut Decompressor,
    out: &mut Vec<u8>,
) -> Result<ImageChunks, Error> {
    read_image_chunks(chunks, header, options, decompressor, |chunk| {
        if chunk.chunk_type() == ChunkType::IDAT {
            out.extend_from_slice(chunk.data());
        }
    })
}

/// Reads the chunks after `IHDR` up to and including `IEND` like
/// `collect_image_data`, with the same checks and errors, but passes every chunk
/// to `on_chunk`, in file order, instead of collecting the image data. A chunk
/// is passed before it's checked, so after an error `on_chunk` may have seen the
/// chunk that failed.
pub(crate) fn read_image_chunks<'a>(
    chunks: &mut ChunkReader<'a>,
    header: &ImageHeader,
    options: &DecodeOptions,
    decompressor: &mut Decompressor,
    mut on_chunk: impl FnMut(Chunk<'a>),
) -> Result<ImageChunks, Error> {
    let mut state = IdatState::NotSeen;
    let mut found = ImageChunks::default();

    while let Some(chunk) = chunks.next_chunk()? {
        on_chunk(chunk);
        let chunk_type = chunk.chunk_type();

        // Any other chunk ends a run of IDATs. Updating the state first means the
        // chunk that ends the run is still handled below.
        if state == IdatState::InRun && chunk_type != ChunkType::IDAT {
            state = IdatState::Done;
        }
        let position = chunk_position(state, found.palette.is_some());
        if options.preserve_chunks {
            preserve_chunk(&chunk, position, &mut found.ancillary);
        }

        match chunk_type {
            ChunkType::PLTE => {
                // `tRNS` must follow `PLTE`. After image data, `read_palette`
                // reports the misplaced `PLTE` instead.
                if found.transparency.is_some() && state == IdatState::NotSeen {
                    return Err(Error::TransparencyBeforePalette);
                }
                read_palette(chunk.data(), header, state, &mut found.palette)?;
            }
            ChunkType::TRNS => {
                read_transparency(
                    chunk.data(),
                    header,
                    state,
                    found.palette.as_ref(),
                    &mut found.transparency,
                )?;
            }
            ChunkType::IDAT => {
                match state {
                    IdatState::Done => return Err(Error::NonConsecutiveImageData),
                    IdatState::NotSeen => require_palette(header, found.palette.as_ref())?,
                    IdatState::InRun => {}
                }
                state = IdatState::InRun;
            }
            ChunkType::IEND if state == IdatState::NotSeen => return Err(Error::MissingImageData),
            ChunkType::IEND => return Ok(found),
            _ if options.preserve_metadata => {
                read_ancillary(
                    &chunk,
                    position,
                    options.strict_ancillary,
                    decompressor,
                    &mut found.metadata,
                )?;
            }
            _ => {}
        }
    }

    Err(Error::MissingImageEnd)
}

/// Where a chunk read now is, from whether image data has started and whether
/// a `PLTE` has been read.
fn chunk_position(state: IdatState, palette_seen: bool) -> ChunkPosition {
    match state {
        IdatState::NotSeen if palette_seen => ChunkPosition::BeforeImageData,
        IdatState::NotSeen => ChunkPosition::BeforePalette,
        IdatState::InRun | IdatState::Done => ChunkPosition::AfterImageData,
    }
}
/// Handles a `PLTE` chunk: parses `data` into `palette`, which holds the palette
/// found so far, if any. `state` says whether image data has started.
///
/// Errors:
/// - `Error::PaletteAfterImageData` if an `IDAT` came before it.
/// - `Error::DuplicatePalette` if there was already a `PLTE`.
/// - Any error from `Palette::parse`.
fn read_palette(
    data: &[u8],
    header: &ImageHeader,
    state: IdatState,
    palette: &mut Option<Palette>,
) -> Result<(), Error> {
    if state != IdatState::NotSeen {
        return Err(Error::PaletteAfterImageData);
    }

    if palette.is_some() {
        return Err(Error::DuplicatePalette);
    }

    palette.replace(Palette::parse(data, header)?);
    Ok(())
}

/// Handles a `tRNS` chunk: parses `data` into `transparency`, which holds the
/// `tRNS` found so far, if any. `palette` is the `PLTE` read before it, if any,
/// and `state` says whether image data has started.
///
/// Errors:
/// - `Error::TransparencyAfterImageData` if an `IDAT` came before it.
/// - `Error::DuplicateTransparency` if there was already a `tRNS`.
/// - Any error from `Transparency::parse`.
fn read_transparency(
    data: &[u8],
    header: &ImageHeader,
    state: IdatState,
    palette: Option<&Palette>,
    transparency: &mut Option<Transparency>,
) -> Result<(), Error> {
    if state != IdatState::NotSeen {
        return Err(Error::TransparencyAfterImageData);
    }

    if transparency.is_some() {
        return Err(Error::DuplicateTransparency);
    }

    transparency.replace(Transparency::parse(data, header, palette)?);
    Ok(())
}

/// Checks, when image data starts, that an indexed image has a palette. Other
/// color types don't need one.
///
/// Returns `Error::MissingPalette` for an indexed image without one.
fn require_palette(header: &ImageHeader, palette: Option<&Palette>) -> Result<(), Error> {
    if header.color_type == ColorType::Indexed && palette.is_none() {
        return Err(Error::MissingPalette);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
