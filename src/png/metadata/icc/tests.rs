#![cfg(test)]

use rust_deflate::Decompressor;

use super::IccProfile;
use crate::error::Error;
use crate::png::ChunkType;

/// `iCCP` data: `name`, a null byte, `method`, then `compressed`.
fn iccp(name: &[u8], method: u8, compressed: &[u8]) -> Vec<u8> {
    let mut data = name.to_vec();
    data.push(0);
    data.push(method);
    data.extend_from_slice(compressed);
    data
}

/// Stands in for an ICC profile: only its size and bytes matter here.
const PROFILE: &[u8] = b"\0\0\x02\x0Cappl\x02\x10\0\0mntrRGB XYZ ";

#[test]
fn parse_reads_name_and_decompresses_the_profile() {
    let data = iccp(b"ICC Profile", 0, &rust_deflate::compress_zlib(PROFILE));

    assert_eq!(
        IccProfile::parse(&data),
        Ok(IccProfile { name: "ICC Profile".to_string(), profile: PROFILE.to_vec() })
    );
}

#[test]
fn parse_with_reuses_a_decompressor() {
    let data = iccp(b"sRGB", 0, &rust_deflate::compress_zlib(PROFILE));
    let mut decompressor = Decompressor::new();

    for _ in 0..2 {
        assert_eq!(IccProfile::parse_with(&data, &mut decompressor, 1000).unwrap().profile, PROFILE);
    }
}

#[test]
fn parse_rejects_invalid_names() {
    for name in [&b""[..], b" Leading", b"Trailing ", b"Two  spaces", b"Tab\there", &[b'a'; 80]] {
        let data = iccp(name, 0, &rust_deflate::compress_zlib(PROFILE));

        assert_eq!(IccProfile::parse(&data), Err(Error::InvalidChunkData(ChunkType::ICCP)), "{name:?}");
    }
}

#[test]
fn parse_rejects_a_missing_null_byte() {
    assert_eq!(IccProfile::parse(b"no null here"), Err(Error::InvalidChunkData(ChunkType::ICCP)));
}

#[test]
fn parse_rejects_unknown_compression_methods() {
    let data = iccp(b"ICC Profile", 1, &rust_deflate::compress_zlib(PROFILE));

    assert_eq!(IccProfile::parse(&data), Err(Error::InvalidChunkData(ChunkType::ICCP)));
}

#[test]
fn parse_rejects_a_missing_compression_method() {
    assert_eq!(IccProfile::parse(b"ICC Profile\0"), Err(Error::InvalidChunkData(ChunkType::ICCP)));
}

#[test]
fn parse_rejects_corrupt_streams() {
    let data = iccp(b"ICC Profile", 0, b"not zlib");

    assert_eq!(IccProfile::parse(&data), Err(Error::InvalidChunkData(ChunkType::ICCP)));
}

#[test]
fn parse_with_caps_the_profile_size() {
    let data = iccp(b"ICC Profile", 0, &rust_deflate::compress_zlib(PROFILE));
    let mut decompressor = Decompressor::new();

    assert!(IccProfile::parse_with(&data, &mut decompressor, PROFILE.len()).is_ok());
    assert_eq!(
        IccProfile::parse_with(&data, &mut decompressor, PROFILE.len() - 1),
        Err(Error::TextTooLong { chunk_type: ChunkType::ICCP, max_size: PROFILE.len() - 1 })
    );
}

#[test]
fn parse_accepts_an_empty_profile() {
    let data = iccp(b"ICC Profile", 0, &rust_deflate::compress_zlib(b""));

    assert_eq!(IccProfile::parse(&data).map(|icc| icc.profile), Ok(Vec::new()));
}
