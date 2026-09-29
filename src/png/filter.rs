use crate::error::Error;

/// The filter type byte at the start of every scanline.
///
/// Each filter predicts a byte from its neighbours and stores the difference,
/// which usually compresses better. The discriminants are the values used in
/// the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FilterType {
    /// Bytes are stored as they are.
    None = 0,
    /// Difference from the byte one pixel to the left.
    Sub = 1,
    /// Difference from the byte above.
    Up = 2,
    /// Difference from the average of the bytes to the left and above.
    Average = 3,
    /// Difference from whichever of left, above and upper-left is closest to
    /// left + above - upper-left.
    Paeth = 4,
}

impl TryFrom<u8> for FilterType {
    type Error = Error;

    /// Converts a scanline's filter type byte.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidFilterType`] for anything above 4.
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(FilterType::None),
            1 => Ok(FilterType::Sub),
            2 => Ok(FilterType::Up),
            3 => Ok(FilterType::Average),
            4 => Ok(FilterType::Paeth),
            _ => Err(Error::InvalidFilterType(value)),
        }
    }
}

#[cfg(test)]
mod tests;
