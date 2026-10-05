//! Writes [`Metadata`] as ancillary chunks.
//!
//! Each chunk type has a function giving its data, the inverse of its type's
//! `parse`. `write_metadata_before_palette` and `write_metadata_after_palette`
//! write them in an order the spec allows:
//!
//! - Before `PLTE`: `cICP`, `iCCP`, `sRGB`, `gAMA`, `cHRM`. The spec requires
//!   the color chunks before `PLTE` and the image data.
//! - After `PLTE` and `tRNS`, before the image data: `pHYs`, `eXIf`, `tIME`,
//!   then the text chunks in [`Metadata::text`] order. `pHYs` and `eXIf` must
//!   come before the image data; `tIME` and text may go anywhere, and writing
//!   them early lets readers find them without reading the image data.

use rust_deflate::{CompressionOptions, Compressor};

use super::chunk_writer::write_chunk;
use crate::error::Error;
use crate::png::metadata::{
    Chromaticities, CodingIndependentCodePoints, Gamma, IccProfile, PhysicalDimensions, RenderingIntent, Text, TextKind,
    Time,
};
use crate::png::{ChunkType, Metadata};

const MAX_KEYWORD_LENGTH: usize = 79;
const NULL_TERMINATOR: u8 = 0;
const COMPRESSION_METHOD: u8 = 0;

/// Checks that every chunk in `metadata` can be written as valid PNG.
///
/// Errors, all `Error::InvalidChunkData` for the chunk type at fault:
/// - A text keyword or `iCCP` profile name that isn't 1 to 79 characters of
///   printable Latin-1 (U+0020 to U+007E and U+00A1 to U+00FF), or has
///   leading, trailing or consecutive spaces: the rules the decoder's
///   `read_keyword` enforces.
/// - `tEXt` or `zTXt` text with a character outside Latin-1 (above U+00FF),
///   which those chunks can't store; use [`TextKind::International`] instead.
/// - Text, an `iTXt` translated keyword, or an `iTXt` language tag containing
///   a null character, which separates the fields of these chunks.
/// - An `iTXt` language tag with characters other than ASCII letters, digits
///   and hyphens, as the spec's BCP 47 tags use. An empty tag is allowed.
/// - A `tIME` field out of the ranges `Time::parse` checks.
///
/// Every other chunk type can only hold valid values, so isn't checked.
pub(crate) fn validate_metadata(metadata: &Metadata) -> Result<(), Error> {
    if let Some(icc_profile) = metadata.icc_profile() {
        validate_keyword(&icc_profile.name, ChunkType::ICCP)?;
    }

    if let Some(time) = metadata.time() {
        // The same ranges the decoder accepts, so whatever is written reads back.
        Time::parse(&time_data(&time))?;
    }

    for text in metadata.text() {
        validate_text(text)?;
    }

    Ok(())
}

/// Checks one text chunk, as `validate_metadata` describes. Errors are
/// `Error::InvalidChunkData` for the chunk type its kind writes.
fn validate_text(text: &Text) -> Result<(), Error> {
    let chunk_type = text.chunk_type();
    let invalid = Err(Error::InvalidChunkData(chunk_type));

    validate_keyword(&text.keyword, chunk_type)?;
    if text.text.contains('\0') {
        return invalid;
    }

    match text.kind {
        TextKind::Plain | TextKind::Compressed => {
            if !text.text.chars().all(is_latin1) {
                return invalid;
            }
        }
        TextKind::International { .. } => {
            // A null character fails this too.
            if !text.language_tag.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                return invalid;
            }
            if text.translated_keyword.contains('\0') {
                return invalid;
            }
        }
    }

    Ok(())
}

/// Checks a text keyword or `iCCP` profile name, as `validate_metadata`
/// describes. Returns `Error::InvalidChunkData` for `chunk_type` if it's invalid.
fn validate_keyword(keyword: &str, chunk_type: ChunkType) -> Result<(), Error> {
    // Counted in characters: each is one Latin-1 byte in the file, though
    // those above U+007F take two bytes in a `str`.
    let length = keyword.chars().count();
    if length == 0 || length > MAX_KEYWORD_LENGTH {
        return Err(Error::InvalidChunkData(chunk_type));
    }
    if !is_printable_latin1(keyword) {
        return Err(Error::InvalidChunkData(chunk_type));
    }
    if keyword.starts_with(' ') || keyword.ends_with(' ') || keyword.contains("  ") {
        return Err(Error::InvalidChunkData(chunk_type));
    }
    Ok(())
}

/// Whether every character of `s` is printable Latin-1: U+0020 to U+007E and
/// U+00A1 to U+00FF. That leaves out control characters and the non-breaking space.
fn is_printable_latin1(s: &str) -> bool {
    s.chars().all(|c| matches!(c as u32, 32..=126 | 161..=255))
}

/// Whether `c` is in Latin-1 (U+0000 to U+00FF), so it fits in one byte.
fn is_latin1(c: char) -> bool {
    u32::from(c) <= 0xFF
}

/// Encodes `text` as Latin-1, one byte per character. Returns
/// `Error::InvalidChunkData` for `chunk_type` if a character is above U+00FF.
fn latin1_bytes(text: &str, chunk_type: ChunkType) -> Result<Vec<u8>, Error> {
    text.chars()
        .map(|c| u8::try_from(c).map_err(|_| Error::InvalidChunkData(chunk_type)))
        .collect()
}

/// Appends the chunks that must come before `PLTE`, for those `metadata` has:
/// `cICP`, `iCCP`, `sRGB`, `gAMA` and `cHRM`, in that order. `iCCP` is
/// compressed with `compressor` and `options`.
///
/// `metadata` must have passed `validate_metadata`.
pub(crate) fn write_metadata_before_palette(
    out: &mut Vec<u8>,
    metadata: &Metadata,
    compressor: &mut Compressor,
    options: CompressionOptions,
) {
    if let Some(cicp) = metadata.cicp() {
        write_chunk(out, ChunkType::CICP, &cicp_data(&cicp));
    }

    if let Some(iccp) = metadata.icc_profile() {
        write_chunk(out, ChunkType::ICCP, &icc_profile_data(iccp, compressor, options));
    }

    if let Some(srgb) = metadata.srgb() {
        write_chunk(out, ChunkType::SRGB, &rendering_intent_data(srgb));
    }

    if let Some(gamma) = metadata.gamma() {
        write_chunk(out, ChunkType::GAMA, &gamma_data(&gamma));
    }

    if let Some(chrm) = metadata.chromaticities() {
        write_chunk(out, ChunkType::CHRM, &chromaticities_data(&chrm));
    }
}

/// Appends the chunks that go after `PLTE` and `tRNS` and before the image
/// data, for those `metadata` has: `pHYs`, `eXIf`, `tIME`, then every text
/// chunk in order. `zTXt` and compressed `iTXt` are compressed with
/// `compressor` and `options`.
///
/// `metadata` must have passed `validate_metadata`.
pub(crate) fn write_metadata_after_palette(
    out: &mut Vec<u8>,
    metadata: &Metadata,
    compressor: &mut Compressor,
    options: CompressionOptions,
) {
    if let Some(phys) = metadata.physical_dimensions() {
        write_chunk(out, ChunkType::PHYS, &physical_dimensions_data(&phys));
    }

    if let Some(exif) = metadata.exif() {
        write_chunk(out, ChunkType::EXIF, exif.data());
    }

    if let Some(time) = metadata.time() {
        write_chunk(out, ChunkType::TIME, &time_data(&time));
    }

    for text in metadata.text() {
        let (chunk_type, text_data) = &text_data(text, compressor, options);
        write_chunk(out, *chunk_type, text_data);
    }
}

/// `gAMA` data: the gamma times 100000, 4 bytes big-endian.
pub(crate) fn gamma_data(gamma: &Gamma) -> [u8; 4] {
    gamma.scaled().to_be_bytes()
}

/// `cHRM` data: white x and y, then red, green and blue x and y, each 4 bytes
/// big-endian, in the order of `Chromaticities`' fields.
pub(crate) fn chromaticities_data(chromaticities: &Chromaticities) -> [u8; 32] {
    let mut data = [0u8; 32];
    data[..4].copy_from_slice(&chromaticities.white_x.to_be_bytes());
    data[4..8].copy_from_slice(chromaticities.white_y.to_be_bytes().as_ref());
    data[8..12].copy_from_slice(chromaticities.red_x.to_be_bytes().as_ref());
    data[12..16].copy_from_slice(chromaticities.red_y.to_be_bytes().as_ref());
    data[16..20].copy_from_slice(chromaticities.green_x.to_be_bytes().as_ref());
    data[20..24].copy_from_slice(chromaticities.green_y.to_be_bytes().as_ref());
    data[24..28].copy_from_slice(chromaticities.blue_x.to_be_bytes().as_ref());
    data[28..32].copy_from_slice(chromaticities.blue_y.to_be_bytes().as_ref());
    data
}

/// `sRGB` data: the rendering intent as one byte.
pub(crate) fn rendering_intent_data(intent: RenderingIntent) -> [u8; 1] {
    [intent as u8]
}

/// `pHYs` data: x and y pixels per unit, 4 bytes each big-endian, then the
/// unit as one byte.
pub(crate) fn physical_dimensions_data(dimensions: &PhysicalDimensions) -> [u8; 9] {
    let mut data = [0u8; 9];
    data[..4].copy_from_slice(&dimensions.x.to_be_bytes());
    data[4..8].copy_from_slice(dimensions.y.to_be_bytes().as_ref());
    data[8] = dimensions.unit as u8;
    data
}

/// `tIME` data: the year, 2 bytes big-endian, then month, day, hour, minute and
/// second, one byte each.
pub(crate) fn time_data(time: &Time) -> [u8; 7] {
    let mut data = [0u8; 7];
    data[..2].copy_from_slice(&time.year.to_be_bytes());
    data[2] = time.month;
    data[3] = time.day;
    data[4] = time.hour;
    data[5] = time.minute;
    data[6] = time.second;
    data
}

/// `cICP` data: color primaries, transfer function, matrix coefficients and the
/// full-range flag (1 or 0), one byte each.
pub(crate) fn cicp_data(cicp: &CodingIndependentCodePoints) -> [u8; 4] {
    [cicp.color_primaries, cicp.transfer_function, cicp.matrix_coefficients, cicp.full_range as u8]
}

/// `iCCP` data: the profile name in Latin-1, a null byte, the compression
/// method (0), then the profile zlib-compressed with `compressor` and `options`.
pub(crate) fn icc_profile_data(icc_profile: &IccProfile, compressor: &mut Compressor, options: CompressionOptions) -> Vec<u8> {
    let mut data = latin1_bytes(&icc_profile.name, ChunkType::ICCP).expect("checked by validate_metadata");
    data.push(NULL_TERMINATOR);
    data.push(COMPRESSION_METHOD);
    data.extend(compressor.compress_zlib_with(&icc_profile.profile, options));
    data
}

/// The chunk type and data for `text`, from its [`TextKind`]:
///
/// - `Plain`: `tEXt`. The keyword, a null byte, then the text, both Latin-1.
/// - `Compressed`: `zTXt`. The keyword, a null byte, the compression method
///   (0), then the Latin-1 text zlib-compressed.
/// - `International`: `iTXt`. The keyword in Latin-1, a null byte, the
///   compression flag (1 if `compressed`, else 0), the compression method (0),
///   the language tag, a null byte, the translated keyword in UTF-8, a null
///   byte, then the text in UTF-8, zlib-compressed if the flag is 1.
///
/// Compression uses `compressor` and `options`. The language tag and
/// translated keyword of `tEXt` and `zTXt` aren't written: those chunks have
/// no place for them.
pub(crate) fn text_data(text: &Text, compressor: &mut Compressor, options: CompressionOptions) -> (ChunkType, Vec<u8>) {
    const VALIDATED: &str = "checked by validate_metadata";
    let chunk_type = text.chunk_type();

    let mut data = latin1_bytes(&text.keyword, chunk_type).expect(VALIDATED);
    data.push(NULL_TERMINATOR);

    match text.kind {
        TextKind::Plain => {
            data.extend(latin1_bytes(&text.text, chunk_type).expect(VALIDATED));
        }
        TextKind::Compressed => {
            data.push(COMPRESSION_METHOD);
            data.extend(compressor.compress_zlib_with(&latin1_bytes(&text.text, chunk_type).expect(VALIDATED), options));
        }
        TextKind::International { compressed } => {
            data.push(compressed as u8);
            data.push(COMPRESSION_METHOD);
            // ASCII, as validate_metadata checks, so its UTF-8 bytes are the same.
            data.extend_from_slice(text.language_tag.as_bytes());
            data.push(NULL_TERMINATOR);
            data.extend_from_slice(text.translated_keyword.as_bytes());
            data.push(NULL_TERMINATOR);
            if compressed {
                data.extend(compressor.compress_zlib_with(text.text.as_bytes(), options));
            } else {
                data.extend_from_slice(text.text.as_bytes());
            }
        }
    }

    (chunk_type, data)
}

#[cfg(test)]
mod tests;
