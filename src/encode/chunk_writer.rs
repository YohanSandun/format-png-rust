use crate::png::crc::crc32;
use crate::png::{ChunkType, ImageHeader, SIGNATURE, Transparency};

/// The longest chunk data the spec allows: 2^31 - 1 bytes.
pub(crate) const MAX_CHUNK_LENGTH: usize = 2_147_483_647;

/// Appends the 8-byte PNG signature to `out`.
pub(crate) fn write_signature(out: &mut Vec<u8>) {
    out.extend_from_slice(&SIGNATURE);
}

/// Appends one chunk to `out`: the data's length as 4 bytes big-endian, the
/// type, the data, then the CRC of the type and data, 4 bytes big-endian.
///
/// `data` must be at most [`MAX_CHUNK_LENGTH`] bytes; callers split longer data,
/// as [`write_image_data`] does.
pub(crate) fn write_chunk(out: &mut Vec<u8>, chunk_type: ChunkType, data: &[u8]) {
    let length: [u8; 4] = (data.len() as u32).to_be_bytes();
    out.extend_from_slice(&length);

    let chunk_type_bytes = chunk_type.as_bytes();
    out.extend_from_slice(chunk_type_bytes);
    out.extend_from_slice(data);

    let crc: [u8; 4] = crc32(chunk_type_bytes, data).to_be_bytes();
    out.extend_from_slice(&crc);
}

/// The 13 bytes of `IHDR` data for `header`: width and height, 4 bytes each
/// big-endian, then bit depth, color type, compression method (0), filter
/// method (0) and interlace method, one byte each. The inverse of
/// `ImageHeader::parse`.
pub(crate) fn header_data(header: &ImageHeader) -> [u8; ImageHeader::LENGTH] {
    let mut ihdr = [0u8; ImageHeader::LENGTH];
    ihdr[..4].copy_from_slice(&header.width.to_be_bytes());
    ihdr[4..8].copy_from_slice(&header.height.to_be_bytes());
    ihdr[8] = header.bit_depth;
    ihdr[9] = header.color_type as u8;
    ihdr[10] = 0u8;
    ihdr[11] = 0u8;
    ihdr[12] = header.interlace as u8;
    ihdr
}

/// The `tRNS` data for `transparency`: the inverse of `Transparency::parse`.
///
/// - `Gray`: the value as 2 bytes big-endian.
/// - `Rgb`: red, green and blue, 2 bytes each big-endian.
/// - `Palette`: the alpha values, one byte each.
///
/// Values aren't checked against the bit depth; `encoder::validate_transparency`
/// does that.
pub(crate) fn transparency_data(transparency: &Transparency) -> Vec<u8> {
    match transparency {
        Transparency::Gray(gray) => gray.to_be_bytes().to_vec(),
        Transparency::Rgb(rgb) => {
            let [r, g, b] = rgb.map(u16::to_be_bytes);
            vec![r[0], r[1], g[0], g[1], b[0], b[1]]
        }
        Transparency::Palette(palette_alpha) => palette_alpha.values().to_vec(),
    }
}

/// Appends the zlib stream `compressed` as `IDAT` chunks of at most `max_length`
/// bytes each, in order, the last one holding what's left. The decoder joins
/// them back into one stream. `max_length` is [`MAX_CHUNK_LENGTH`] outside tests.
///
/// An empty `compressed` still gets one empty `IDAT`, since the spec requires
/// at least one; a zlib stream is never empty anyway.
pub(crate) fn write_image_data(out: &mut Vec<u8>, compressed: &[u8], max_length: usize) {
    if compressed.is_empty() {
        write_chunk(out, ChunkType::IDAT, compressed);
    } else {
        for chunk in compressed.chunks(max_length) {
            write_chunk(out, ChunkType::IDAT, chunk);
        }
    }
}

#[cfg(test)]
mod tests;
