//! Finds the frames of an animated PNG among its chunks; see `png::animation`
//! for the format.
//!
//! `image_data::read_image_chunks` has already checked the chunks a static PNG
//! needs, so this only checks the animation's own rules:
//!
//! - `acTL` comes once, before the image data.
//! - `fcTL` and `fdAT` sequence numbers count up from 0 in file order.
//! - Only the first `fcTL` may come before the image data, and then it must
//!   cover the whole canvas: the default image is the first frame.
//! - `fdAT` comes after the image data, following an `fcTL` of its own.
//! - Every frame has data, fits on the canvas, and there are as many as `acTL` says.

use crate::error::Error;
use crate::png::{AnimationControl, Chunk, ChunkType, FrameControl, ImageHeader};

/// Where the frames of an animated PNG are.
#[derive(Debug)]
pub(crate) struct AnimationLayout<'a> {
    pub(crate) control: AnimationControl,
    /// Whether the `IDAT` image is the first frame.
    pub(crate) default_image_is_frame: bool,
    pub(crate) frames: Vec<FrameLayout<'a>>,
}

/// One frame: its control and its zlib stream, in pieces, from `IDAT` data or
/// `fdAT` data without the sequence number.
#[derive(Debug)]
pub(crate) struct FrameLayout<'a> {
    pub(crate) control: FrameControl,
    pub(crate) data: Vec<&'a [u8]>,
    /// Whether this frame is the default image, so its data is in `IDAT`s.
    is_default_image: bool,
}

/// Reads the animation from `chunks`: every chunk after `IHDR` up to and
/// including `IEND`, in file order, already checked as a static PNG.
///
/// Errors:
/// - `Error::NotAnimated` if there's no `acTL`.
/// - `Error::MisplacedChunk` for an `acTL` after the image data, an `fcTL`
///   before it that isn't the first, or an `fdAT` before it or without an
///   `fcTL` of its own.
/// - `Error::DuplicateChunk` for a second `acTL`.
/// - `Error::FrameSequenceMismatch` if a sequence number is out of order.
/// - `Error::InvalidChunkData` for an `fcTL` before the image data that doesn't
///   cover the whole canvas.
/// - `Error::FrameOutOfBounds` for a frame that doesn't fit on the canvas.
/// - `Error::InvalidChunkLength` for an `fdAT` too short for its sequence number.
/// - `Error::MissingFrameData` for an `fcTL` with no data after it.
/// - `Error::FrameCountMismatch` if the number of frames isn't what `acTL` says.
/// - Any error from `AnimationControl::parse` or `FrameControl::parse`.
pub(crate) fn read_animation<'a>(
    header: &ImageHeader,
    chunks: &[Chunk<'a>],
) -> Result<AnimationLayout<'a>, Error> {
    let mut control = None;
    let mut image_data_seen = false;
    let mut next_sequence = 0u32;
    let mut frames: Vec<FrameLayout<'a>> = Vec::new();

    let mut check_sequence = |actual: u32| {
        if actual != next_sequence {
            return Err(Error::FrameSequenceMismatch {
                expected: next_sequence,
                actual,
            });
        }
        next_sequence = next_sequence.wrapping_add(1);
        Ok(())
    };

    for chunk in chunks {
        match chunk.chunk_type() {
            ChunkType::ACTL => {
                if image_data_seen {
                    return Err(Error::MisplacedChunk(ChunkType::ACTL));
                }
                if control.is_some() {
                    return Err(Error::DuplicateChunk(ChunkType::ACTL));
                }
                control = Some(AnimationControl::parse(chunk.data())?);
            }
            ChunkType::FCTL => {
                let frame = FrameControl::parse(chunk.data())?;
                check_sequence(frame.sequence_number)?;

                if !image_data_seen {
                    if !frames.is_empty() {
                        return Err(Error::MisplacedChunk(ChunkType::FCTL));
                    }
                    if !frame.covers(header.width, header.height) {
                        return Err(Error::InvalidChunkData(ChunkType::FCTL));
                    }
                }
                if !frame.fits_in(header.width, header.height) {
                    return Err(Error::FrameOutOfBounds);
                }
                finish_frame(frames.last())?;

                frames.push(FrameLayout {
                    control: frame,
                    data: Vec::new(),
                    is_default_image: !image_data_seen,
                });
            }
            ChunkType::IDAT => {
                image_data_seen = true;
                if let Some(frame) = frames.last_mut().filter(|f| f.is_default_image) {
                    frame.data.push(chunk.data());
                }
            }
            ChunkType::FDAT => {
                let Some(frame) = frames.last_mut().filter(|f| !f.is_default_image) else {
                    return Err(Error::MisplacedChunk(ChunkType::FDAT));
                };
                let Some((sequence, data)) = chunk.data().split_first_chunk::<4>() else {
                    return Err(Error::InvalidChunkLength {
                        chunk_type: ChunkType::FDAT,
                        length: chunk.data().len(),
                    });
                };
                check_sequence(u32::from_be_bytes(*sequence))?;
                frame.data.push(data);
            }
            _ => {}
        }
    }

    let control = control.ok_or(Error::NotAnimated)?;
    finish_frame(frames.last())?;

    let actual = u32::try_from(frames.len()).unwrap_or(u32::MAX);
    if actual != control.num_frames {
        return Err(Error::FrameCountMismatch {
            expected: control.num_frames,
            actual,
        });
    }

    Ok(AnimationLayout {
        control,
        default_image_is_frame: frames.first().is_some_and(|f| f.is_default_image),
        frames,
    })
}

/// Checks that `frame`, the last one started, if any, had data before the
/// next `fcTL` or the end. Returns `Error::MissingFrameData` if not.
fn finish_frame(frame: Option<&FrameLayout<'_>>) -> Result<(), Error> {
    match frame {
        Some(frame) if frame.data.is_empty() => Err(Error::MissingFrameData),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests;
