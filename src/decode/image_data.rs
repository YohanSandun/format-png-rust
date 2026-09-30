use crate::ColorType;
use super::chunk_reader::ChunkReader;
use crate::error::Error;
use crate::png::{ChunkType, ImageHeader, Palette, Transparency};

#[derive(Clone, Copy, PartialEq, Eq)]
enum IdatState {
    NotSeen,
    InRun,
    Done,
}

/// The chunks besides `IDAT` that decoding or converting the image needs, read
/// on the way to `IEND`.
#[derive(Debug, Default)]
pub(crate) struct ImageChunks {
    pub(crate) palette: Option<Palette>,
    pub(crate) transparency: Option<Transparency>,
}

/// Reads the chunks after `IHDR` up to and including `IEND`, appending the data
/// of every `IDAT` chunk to `out`, so it holds the whole zlib stream, and returns
/// the other chunks the image needs. Remaining chunks are skipped. Nothing after
/// `IEND` is read.
///
/// Errors:
/// - `Error::MissingImageData` if `IEND` comes before any `IDAT`.
/// - `Error::NonConsecutiveImageData` if another chunk comes between two `IDAT`s.
/// - `Error::MissingImageEnd` if the input ends before `IEND`.
/// - `Error::TransparencyBeforePalette` if a `PLTE` follows a `tRNS`.
/// - Any error from `read_palette`, `require_palette` or `read_transparency`.
/// - Any error from `ChunkReader::next_chunk`.
pub(crate) fn collect_image_data(
    chunks: &mut ChunkReader<'_>,
    header: &ImageHeader,
    out: &mut Vec<u8>,
) -> Result<ImageChunks, Error> {
    let mut state = IdatState::NotSeen;
    let mut found = ImageChunks::default();

    while let Some(chunk) = chunks.next_chunk()? {
        match (chunk.chunk_type(), state) {
            (ChunkType::PLTE, _) => {
                if found.transparency.is_some() && state == IdatState::NotSeen {
                    return Err(Error::TransparencyBeforePalette);
                }
                read_palette(chunk.data(), header, state, &mut found.palette)?;
            }
            (ChunkType::TRNS, _) => {
                read_transparency(chunk.data(), header, state, found.palette.as_ref(), &mut found.transparency)?;
            }
            (ChunkType::IDAT, IdatState::Done) => return Err(Error::NonConsecutiveImageData),
            (ChunkType::IDAT, _) => {
                if let IdatState::NotSeen = state {
                    require_palette(header, found.palette.as_ref())?;
                }
                out.extend_from_slice(chunk.data());
                state = IdatState::InRun;
            }
            (ChunkType::IEND, IdatState::NotSeen) => return Err(Error::MissingImageData),
            (ChunkType::IEND, _) => return Ok(found),
            (_, IdatState::InRun) => state = IdatState::Done,
            _ => {}
        }
    }

    Err(Error::MissingImageEnd)
}

/// Handles a `PLTE` chunk: parses `data` into `palette`, which holds the palette
/// found so far, if any. `state` says whether image data has started.
///
/// Errors:
/// - `Error::PaletteAfterImageData` if an `IDAT` came before it.
/// - `Error::DuplicatePalette` if there was already a `PLTE`.
/// - Any error from `Palette::parse`.
fn read_palette(data: &[u8], header: &ImageHeader, state: IdatState, palette: &mut Option<Palette>) -> Result<(), Error> {
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

    transparency.replace(Transparency::parse(data,header,palette)?);
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
