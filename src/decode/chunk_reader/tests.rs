#![cfg(test)]

use super::ChunkReader;
use crate::error::Error;
use crate::png::{ChunkType, SIGNATURE};

/// IHDR for a 1x1, 8-bit RGBA image, with its correct CRC.
const IHDR_CHUNK: [u8; 25] = [
    0x00, 0x00, 0x00, 0x0D, // length = 13
    b'I', b'H', b'D', b'R', // type
    0x00, 0x00, 0x00, 0x01, // width = 1
    0x00, 0x00, 0x00, 0x01, // height = 1
    0x08, 0x06, 0x00, 0x00, 0x00, // bit depth, color type, compression, filter, interlace
    0x1F, 0x15, 0xC4, 0x89, // crc
];

/// Empty IEND chunk with its correct CRC.
const IEND_CHUNK: [u8; 12] = [
    0x00, 0x00, 0x00, 0x00, // length = 0
    b'I', b'E', b'N', b'D', // type
    0xAE, 0x42, 0x60, 0x82, // crc
];

fn png(chunks: &[&[u8]]) -> Vec<u8> {
    let mut data = SIGNATURE.to_vec();
    for chunk in chunks {
        data.extend_from_slice(chunk);
    }
    data
}

// ---------- new ----------

#[test]
fn new_accepts_signature_only() {
    assert!(ChunkReader::new(&SIGNATURE).is_ok());
}

#[test]
fn new_rejects_empty_input() {
    assert!(matches!(
        ChunkReader::new(&[]),
        Err(Error::UnexpectedEndOfInput)
    ));
}

#[test]
fn new_rejects_truncated_signature() {
    assert!(matches!(
        ChunkReader::new(&SIGNATURE[..7]),
        Err(Error::UnexpectedEndOfInput)
    ));
}

#[test]
fn new_rejects_wrong_signature() {
    let mut data = SIGNATURE;
    data[1] = b'Q';

    assert!(matches!(
        ChunkReader::new(&data),
        Err(Error::InvalidSignature)
    ));
}

#[test]
fn new_rejects_short_non_png() {
    assert!(matches!(
        ChunkReader::new(b"GIF89a"),
        Err(Error::InvalidSignature)
    ));
}

// ---------- next_chunk ----------

#[test]
fn next_chunk_returns_none_after_signature() {
    let mut reader = ChunkReader::new(&SIGNATURE).unwrap();

    assert_eq!(reader.next_chunk(), Ok(None));
}

#[test]
fn next_chunk_reads_ihdr() {
    let data = png(&[&IHDR_CHUNK]);
    let mut reader = ChunkReader::new(&data).unwrap();

    let chunk = reader.next_chunk().unwrap().unwrap();
    assert_eq!(chunk.chunk_type(), ChunkType::IHDR);
    assert_eq!(chunk.data(), &IHDR_CHUNK[8..21]);
    assert_eq!(chunk.crc(), 0x1F15_C489);
}

#[test]
fn next_chunk_reads_chunks_in_order_then_none() {
    let data = png(&[&IHDR_CHUNK, &IEND_CHUNK]);
    let mut reader = ChunkReader::new(&data).unwrap();

    assert_eq!(
        reader.next_chunk().unwrap().unwrap().chunk_type(),
        ChunkType::IHDR
    );

    let iend = reader.next_chunk().unwrap().unwrap();
    assert_eq!(iend.chunk_type(), ChunkType::IEND);
    assert!(iend.data().is_empty());

    assert_eq!(reader.next_chunk(), Ok(None));
}

#[test]
fn next_chunk_returns_unknown_private_chunks() {
    let chunk = [
        0x00, 0x00, 0x00, 0x03, b'r', b'u', b'S', b't', b'a', b'b', b'c', 0, 0, 0, 0,
    ];
    let data = png(&[&chunk]);
    let mut reader = ChunkReader::new(&data).unwrap().validate_crc(false);

    let chunk = reader.next_chunk().unwrap().unwrap();
    assert_eq!(chunk.chunk_type().as_bytes(), b"ruSt");
    assert_eq!(chunk.data(), b"abc");
}

#[test]
fn next_chunk_rejects_length_above_2_pow_31_minus_1() {
    let chunk = [0x80, 0x00, 0x00, 0x00, b'I', b'D', b'A', b'T'];
    let data = png(&[&chunk]);
    let mut reader = ChunkReader::new(&data).unwrap();

    assert_eq!(reader.next_chunk(), Err(Error::ChunkTooLong(0x8000_0000)));
}

#[test]
fn next_chunk_rejects_invalid_chunk_type() {
    let chunk = [0x00, 0x00, 0x00, 0x00, b'I', b'1', b'2', b'3', 0, 0, 0, 0];
    let data = png(&[&chunk]);
    let mut reader = ChunkReader::new(&data).unwrap().validate_crc(false);

    assert_eq!(reader.next_chunk(), Err(Error::InvalidChunkType(*b"I123")));
}

#[test]
fn next_chunk_rejects_truncated_length() {
    let data = png(&[&IHDR_CHUNK[..3]]);
    let mut reader = ChunkReader::new(&data).unwrap();

    assert_eq!(reader.next_chunk(), Err(Error::UnexpectedEndOfInput));
}

#[test]
fn next_chunk_rejects_truncated_data() {
    let data = png(&[&IHDR_CHUNK[..15]]);
    let mut reader = ChunkReader::new(&data).unwrap();

    assert_eq!(reader.next_chunk(), Err(Error::UnexpectedEndOfInput));
}

#[test]
fn next_chunk_rejects_missing_crc() {
    let data = png(&[&IHDR_CHUNK[..21]]);
    let mut reader = ChunkReader::new(&data).unwrap();

    assert_eq!(reader.next_chunk(), Err(Error::UnexpectedEndOfInput));
}

// ---------- CRC validation ----------

#[test]
fn next_chunk_rejects_wrong_crc_by_default() {
    let mut chunk = IHDR_CHUNK;
    chunk[24] ^= 0xFF;
    let data = png(&[&chunk]);
    let mut reader = ChunkReader::new(&data).unwrap();

    assert_eq!(
        reader.next_chunk(),
        Err(Error::CrcMismatch {
            expected: 0x1F15_C476,
            actual: 0x1F15_C489
        })
    );
}

#[test]
fn next_chunk_accepts_wrong_crc_when_validation_off() {
    let mut chunk = IHDR_CHUNK;
    chunk[24] ^= 0xFF;
    let data = png(&[&chunk]);
    let mut reader = ChunkReader::new(&data).unwrap().validate_crc(false);

    let chunk = reader.next_chunk().unwrap().unwrap();
    assert_eq!(chunk.chunk_type(), ChunkType::IHDR);
    assert_eq!(chunk.crc(), 0x1F15_C476);
}

#[test]
fn next_chunk_detects_corrupted_data() {
    let mut chunk = IHDR_CHUNK;
    chunk[11] = 0x02; // width = 2, CRC still for width = 1
    let data = png(&[&chunk]);
    let mut reader = ChunkReader::new(&data).unwrap();

    assert!(matches!(
        reader.next_chunk(),
        Err(Error::CrcMismatch { .. })
    ));
}
