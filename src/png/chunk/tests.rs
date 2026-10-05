#![cfg(test)]

use super::{Chunk, ChunkType};
use crate::error::Error;

// ---------- from_bytes ----------

#[test]
fn from_bytes_accepts_ascii_letters() {
    let chunk_type = ChunkType::from_bytes(*b"tEXt").unwrap();

    assert_eq!(chunk_type.as_bytes(), b"tEXt");
}

#[test]
fn from_bytes_equals_constant() {
    assert_eq!(ChunkType::from_bytes(*b"IHDR"), Ok(ChunkType::IHDR));
}

#[test]
fn from_bytes_rejects_digits() {
    assert_eq!(
        ChunkType::from_bytes(*b"IHD1"),
        Err(Error::InvalidChunkType(*b"IHD1"))
    );
}

#[test]
fn from_bytes_rejects_bytes_between_upper_and_lower_case() {
    // '[' (0x5B) and '`' (0x60) sit between 'Z' and 'a'
    assert!(ChunkType::from_bytes(*b"[HDR").is_err());
    assert!(ChunkType::from_bytes(*b"IHD`").is_err());
}

#[test]
fn from_bytes_rejects_non_ascii() {
    let bytes = [0xC9, b'H', b'D', b'R'];

    assert_eq!(
        ChunkType::from_bytes(bytes),
        Err(Error::InvalidChunkType(bytes))
    );
}

// ---------- property bits ----------

#[test]
fn critical_chunks() {
    assert!(ChunkType::IHDR.is_critical());
    assert!(ChunkType::IDAT.is_critical());
    assert!(!ChunkType::from_bytes(*b"tEXt").unwrap().is_critical());
}

#[test]
fn public_chunks() {
    assert!(ChunkType::IHDR.is_public());
    assert!(!ChunkType::from_bytes(*b"prIv").unwrap().is_public());
}

#[test]
fn reserved_bit() {
    assert!(ChunkType::IHDR.is_reserved_bit_valid());
    assert!(
        !ChunkType::from_bytes(*b"IHdR")
            .unwrap()
            .is_reserved_bit_valid()
    );
}

#[test]
fn safe_to_copy_chunks() {
    assert!(ChunkType::from_bytes(*b"tEXt").unwrap().is_safe_to_copy());
    assert!(!ChunkType::IHDR.is_safe_to_copy());
}

#[test]
fn private_ancillary_safe_to_copy_chunk() {
    let chunk_type = ChunkType::from_bytes(*b"ruSt").unwrap();

    assert!(!chunk_type.is_critical());
    assert!(!chunk_type.is_public());
    assert!(chunk_type.is_reserved_bit_valid());
    assert!(chunk_type.is_safe_to_copy());
}

// ---------- formatting ----------

#[test]
fn display_prints_type_name() {
    assert_eq!(ChunkType::IEND.to_string(), "IEND");
}

// ---------- Chunk ----------

#[test]
fn chunk_accessors_return_parts() {
    let data = [1, 2, 3];
    let chunk = Chunk::new(ChunkType::IDAT, &data, 0xDEAD_BEEF);

    assert_eq!(chunk.chunk_type(), ChunkType::IDAT);
    assert_eq!(chunk.data(), &data);
    assert_eq!(chunk.crc(), 0xDEAD_BEEF);
}
