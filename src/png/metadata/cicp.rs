use crate::error::Error;
use crate::png::ChunkType;

/// The `cICP` chunk: coding-independent code points, as used for video and HDR.
///
/// The four values are from ITU-T H.273, the same ones video formats use. The
/// most common combinations are:
///
/// | Color space | `color_primaries` | `transfer_function` |
/// |---|---|---|
/// | sRGB | 1 (BT.709) | 13 (sRGB) |
/// | Display P3 | 12 | 13 (sRGB) |
/// | BT.2100 PQ (HDR) | 9 (BT.2020) | 16 (PQ) |
/// | BT.2100 HLG (HDR) | 9 (BT.2020) | 18 (HLG) |
///
/// When it's present, `cICP` takes precedence over `iCCP`, `sRGB`, `gAMA` and
/// `cHRM`. The values are kept as numbers, since H.273 defines more than a PNG
/// decoder needs to name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CodingIndependentCodePoints {
    /// The color primaries and white point, as an H.273 `ColourPrimaries` value.
    pub color_primaries: u8,
    /// The transfer function, as an H.273 `TransferCharacteristics` value.
    pub transfer_function: u8,
    /// The matrix coefficients, as an H.273 `MatrixCoefficients` value. Always 0
    /// (RGB) in PNG, which stores RGB rather than YCbCr.
    pub matrix_coefficients: u8,
    /// Whether samples use the full range (`true`), as almost all PNGs do, or
    /// the narrow range video uses, such as 16 to 235 for 8 bits (`false`).
    pub full_range: bool,
}

impl CodingIndependentCodePoints {
    /// Parses `cICP` chunk data: one byte each for the color primaries, transfer
    /// function, matrix coefficients and full-range flag.
    ///
    /// # Errors
    ///
    /// - [`Error::InvalidChunkLength`] if `data` isn't 4 bytes.
    /// - [`Error::InvalidChunkData`] if the matrix coefficients aren't 0, or the
    ///   full-range flag isn't 0 or 1.
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        if data.len() != 4 {
            return Err(Error::InvalidChunkLength {
                chunk_type: ChunkType::CICP,
                length: data.len(),
            });
        }
        if data[2] != 0 || data[3] > 1 {
            return Err(Error::InvalidChunkData(ChunkType::CICP));
        }

        Ok(Self {
            color_primaries: data[0],
            transfer_function: data[1],
            matrix_coefficients: data[2],
            full_range: data[3] == 1,
        })
    }
}

#[cfg(test)]
mod tests;
