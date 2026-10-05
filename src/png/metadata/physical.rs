use crate::error::Error;
use crate::png::ChunkType;

/// The unit of [`PhysicalDimensions`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Unit {
    /// No unit: the values only give the pixels' aspect ratio.
    Unknown = 0,
    /// Pixels per meter.
    Meter = 1,
}

/// The `pHYs` chunk: the intended pixel size, or just the pixel aspect ratio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PhysicalDimensions {
    /// Pixels per unit, horizontally.
    pub x: u32,
    /// Pixels per unit, vertically.
    pub y: u32,
    /// What `x` and `y` count pixels per.
    pub unit: Unit,
}

impl PhysicalDimensions {
    /// Parses `pHYs` chunk data: pixels per unit on the x axis and on the y axis,
    /// 4 bytes each, big-endian, then a unit byte, 0 or 1.
    ///
    /// # Errors
    ///
    /// - [`Error::InvalidChunkLength`] if `data` isn't 9 bytes.
    /// - [`Error::InvalidChunkData`] if the unit byte is above 1.
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        if data.len() != 9 {
            return Err(Error::InvalidChunkLength { chunk_type: ChunkType::PHYS, length: data.len() });
        }

        let unit = match data[8] {
            0 => Unit::Unknown,
            1 => Unit::Meter,
            _ => return Err(Error::InvalidChunkData(ChunkType::PHYS)),
        };

        Ok(Self {
            x: u32::from_be_bytes(data[..4].try_into().unwrap()),
            y: u32::from_be_bytes(data[4..8].try_into().unwrap()),
            unit,
        })
    }

    /// Dots per inch horizontally and vertically, if the unit is meters.
    pub fn dots_per_inch(&self) -> Option<(f64, f64)> {
        const METERS_PER_INCH: f64 = 0.0254;
        match self.unit {
            Unit::Meter => Some((f64::from(self.x) * METERS_PER_INCH, f64::from(self.y) * METERS_PER_INCH)),
            Unit::Unknown => None,
        }
    }
}

#[cfg(test)]
mod tests;
