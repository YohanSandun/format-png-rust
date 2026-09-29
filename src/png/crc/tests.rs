#![cfg(test)]

use super::crc32;

#[test]
fn crc32_of_empty_iend_chunk() {
    assert_eq!(crc32(b"IEND", &[]), 0xAE42_6082);
}

#[test]
fn crc32_of_ihdr_chunk() {
    // 1x1, 8-bit RGBA, no interlace
    let data = [0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0];

    assert_eq!(crc32(b"IHDR", &data), 0x1F15_C489);
}

#[test]
fn crc32_of_text_chunk() {
    assert_eq!(crc32(b"tEXt", b"abc"), 0xFC85_9B88);
}

#[test]
fn crc32_depends_on_chunk_type() {
    assert_ne!(crc32(b"IEND", &[]), crc32(b"IDAT", &[]));
}
