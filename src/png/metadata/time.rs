use crate::error::Error;
use crate::png::ChunkType;

/// The `tIME` chunk: when the image was last modified, in UTC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Time {
    /// The full year, for example 2026.
    pub year: u16,
    /// 1 to 12.
    pub month: u8,
    /// 1 to 31.
    pub day: u8,
    /// 0 to 23.
    pub hour: u8,
    /// 0 to 59.
    pub minute: u8,
    /// 0 to 60; 60 allows for a leap second.
    pub second: u8,
}

impl Time {
    /// Parses `tIME` chunk data: a 2-byte big-endian year, then one byte each for
    /// month, day, hour, minute and second.
    ///
    /// # Errors
    ///
    /// - [`Error::InvalidChunkLength`] if `data` isn't 7 bytes.
    /// - [`Error::InvalidChunkData`] if a field is outside the range listed on it.
    ///   The day isn't checked against the month, so February 31 is accepted.
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        if data.len() != 7 {
            return Err(Error::InvalidChunkLength {
                chunk_type: ChunkType::TIME,
                length: data.len(),
            });
        }

        let year = u16::from_be_bytes([data[0], data[1]]);
        let month = data[2];
        let day = data[3];
        let hour = data[4];
        let minute = data[5];
        let second = data[6];

        if !(1..=12).contains(&month)
            || !(1..=31).contains(&day)
            || hour > 23
            || minute > 59
            || second > 60
        {
            return Err(Error::InvalidChunkData(ChunkType::TIME));
        }

        Ok(Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
        })
    }
}

#[cfg(test)]
mod tests;
