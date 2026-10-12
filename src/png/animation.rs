//! The chunks of an animated PNG (APNG): `acTL`, `fcTL` and `fdAT`.
//!
//! An APNG is a PNG whose `acTL` chunk, before the image data, says how many
//! frames it has and how many times to play them. Each frame is an `fcTL` chunk,
//! which gives its size, position, delay and how it's drawn, followed by its
//! image data: the `IDAT` chunks for the first frame if the `fcTL` comes before
//! them, and `fdAT` chunks for the others. Viewers that don't know APNG show
//! the `IDAT` image, the default image, and skip the rest.
//!
//! `fcTL` and `fdAT` share one sequence of numbers, starting at 0, so frames
//! can't be reordered by editors that don't understand them.

use crate::error::Error;
use crate::png::ChunkType;

/// The `acTL` chunk: how many frames an animated PNG has and how many times to
/// play them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnimationControl {
    /// The number of frames, including the default image if it's the first
    /// frame. Never zero.
    pub num_frames: u32,
    /// How many times to play the animation; 0 means forever.
    pub num_plays: u32,
}

impl AnimationControl {
    /// The length of `acTL` data.
    pub const LENGTH: usize = 8;

    /// Parses `acTL` chunk data.
    ///
    /// # Errors
    ///
    /// - [`Error::InvalidChunkLength`] if `data` isn't 8 bytes.
    /// - [`Error::InvalidChunkData`] if the number of frames is zero.
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        let data: &[u8; Self::LENGTH] = data.try_into().map_err(|_| Error::InvalidChunkLength {
            chunk_type: ChunkType::ACTL,
            length: data.len(),
        })?;

        let num_frames = read_u32(data, 0);
        if num_frames == 0 {
            return Err(Error::InvalidChunkData(ChunkType::ACTL));
        }

        Ok(Self {
            num_frames,
            num_plays: read_u32(data, 4),
        })
    }

    /// The `acTL` chunk data: the inverse of [`parse`](Self::parse).
    #[must_use]
    pub fn to_bytes(&self) -> [u8; Self::LENGTH] {
        let mut data = [0u8; Self::LENGTH];
        data[..4].copy_from_slice(&self.num_frames.to_be_bytes());
        data[4..].copy_from_slice(&self.num_plays.to_be_bytes());
        data
    }
}

/// What happens to a frame's area of the canvas after it has been shown, before
/// the next frame is drawn.
///
/// The discriminants are the values used in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DisposeOp {
    /// Leave the canvas as it is.
    #[default]
    None = 0,
    /// Clear the frame's area to fully transparent black.
    Background = 1,
    /// Put the frame's area back as it was before the frame was drawn. On the
    /// first frame this acts as [`Background`](Self::Background).
    Previous = 2,
}

impl TryFrom<u8> for DisposeOp {
    type Error = Error;

    /// Converts the `dispose_op` byte from `fcTL`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidChunkData`] for anything other than 0, 1 or 2.
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(DisposeOp::None),
            1 => Ok(DisposeOp::Background),
            2 => Ok(DisposeOp::Previous),
            _ => Err(Error::InvalidChunkData(ChunkType::FCTL)),
        }
    }
}

/// How a frame is drawn onto the canvas.
///
/// The discriminants are the values used in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BlendOp {
    /// Replace the frame's area of the canvas, alpha included.
    #[default]
    Source = 0,
    /// Draw the frame over the canvas, using its alpha.
    Over = 1,
}

impl TryFrom<u8> for BlendOp {
    type Error = Error;

    /// Converts the `blend_op` byte from `fcTL`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidChunkData`] for anything other than 0 or 1.
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(BlendOp::Source),
            1 => Ok(BlendOp::Over),
            _ => Err(Error::InvalidChunkData(ChunkType::FCTL)),
        }
    }
}

/// The `fcTL` chunk: one frame's size, position on the canvas, delay, and how
/// it's drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameControl {
    /// The frame's place in the sequence shared by `fcTL` and `fdAT` chunks.
    pub sequence_number: u32,
    /// Width of the frame in pixels. Never zero.
    pub width: u32,
    /// Height of the frame in pixels. Never zero.
    pub height: u32,
    /// Where the frame's left edge is on the canvas.
    pub x_offset: u32,
    /// Where the frame's top edge is on the canvas.
    pub y_offset: u32,
    /// The numerator of how long to show the frame, in seconds.
    pub delay_num: u16,
    /// The denominator of how long to show the frame, in seconds. 0 means 100,
    /// so the delay is in hundredths of a second.
    pub delay_den: u16,
    /// What happens to the frame's area after it's shown.
    pub dispose_op: DisposeOp,
    /// How the frame is drawn onto the canvas.
    pub blend_op: BlendOp,
}

impl FrameControl {
    /// The length of `fcTL` data.
    pub const LENGTH: usize = 26;

    /// Parses `fcTL` chunk data. The frame isn't checked against the canvas;
    /// see [`fits_in`](Self::fits_in).
    ///
    /// # Errors
    ///
    /// - [`Error::InvalidChunkLength`] if `data` isn't 26 bytes.
    /// - [`Error::InvalidChunkData`] if the width or height is zero or over
    ///   2^31 - 1, or the dispose or blend operation is unknown.
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        let data: &[u8; Self::LENGTH] = data.try_into().map_err(|_| Error::InvalidChunkLength {
            chunk_type: ChunkType::FCTL,
            length: data.len(),
        })?;

        let control = Self {
            sequence_number: read_u32(data, 0),
            width: read_u32(data, 4),
            height: read_u32(data, 8),
            x_offset: read_u32(data, 12),
            y_offset: read_u32(data, 16),
            delay_num: u16::from_be_bytes([data[20], data[21]]),
            delay_den: u16::from_be_bytes([data[22], data[23]]),
            dispose_op: DisposeOp::try_from(data[24])?,
            blend_op: BlendOp::try_from(data[25])?,
        };

        crate::png::ImageHeader::validate_dimensions(control.width, control.height)
            .map_err(|_| Error::InvalidChunkData(ChunkType::FCTL))?;

        Ok(control)
    }

    /// The `fcTL` chunk data: the inverse of [`parse`](Self::parse).
    #[must_use]
    pub fn to_bytes(&self) -> [u8; Self::LENGTH] {
        let mut data = [0u8; Self::LENGTH];
        data[..4].copy_from_slice(&self.sequence_number.to_be_bytes());
        data[4..8].copy_from_slice(&self.width.to_be_bytes());
        data[8..12].copy_from_slice(&self.height.to_be_bytes());
        data[12..16].copy_from_slice(&self.x_offset.to_be_bytes());
        data[16..20].copy_from_slice(&self.y_offset.to_be_bytes());
        data[20..22].copy_from_slice(&self.delay_num.to_be_bytes());
        data[22..24].copy_from_slice(&self.delay_den.to_be_bytes());
        data[24] = self.dispose_op as u8;
        data[25] = self.blend_op as u8;
        data
    }

    /// Whether the frame lies entirely within a canvas of `width × height`, as
    /// the spec requires.
    #[must_use]
    pub fn fits_in(&self, width: u32, height: u32) -> bool {
        self.x_offset
            .checked_add(self.width)
            .is_some_and(|right| right <= width)
            && self
                .y_offset
                .checked_add(self.height)
                .is_some_and(|bottom| bottom <= height)
    }

    /// Whether the frame covers the whole of a canvas of `width × height`, as
    /// the first frame must when it's the default image.
    #[must_use]
    pub fn covers(&self, width: u32, height: u32) -> bool {
        self.x_offset == 0 && self.y_offset == 0 && self.width == width && self.height == height
    }

    /// How long to show the frame, in seconds: `delay_num / delay_den`, with a
    /// `delay_den` of 0 meaning 100.
    #[must_use]
    pub fn delay_seconds(&self) -> f64 {
        let den = if self.delay_den == 0 {
            100
        } else {
            self.delay_den
        };
        f64::from(self.delay_num) / f64::from(den)
    }
}

/// The big-endian `u32` at `offset` in `data`. The caller has checked the length.
fn read_u32(data: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes(data[offset..offset + 4].try_into().unwrap())
}

#[cfg(test)]
mod tests;
