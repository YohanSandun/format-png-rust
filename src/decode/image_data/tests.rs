#![cfg(test)]

use super::collect_image_data;
use crate::decode::chunk_reader::ChunkReader;
use crate::error::Error;
use crate::png::SIGNATURE;

/// A chunk with a zero CRC; the readers below don't validate CRCs.
fn chunk(chunk_type: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = (data.len() as u32).to_be_bytes().to_vec();
    out.extend_from_slice(chunk_type);
    out.extend_from_slice(data);
    out.extend_from_slice(&[0; 4]);
    out
}

/// The signature followed by `chunks`, standing in for everything after IHDR.
fn png(chunks: &[Vec<u8>]) -> Vec<u8> {
    let mut data = SIGNATURE.to_vec();
    for chunk in chunks {
        data.extend_from_slice(chunk);
    }
    data
}

fn collect(data: &[u8]) -> Result<Vec<u8>, Error> {
    let mut chunks = ChunkReader::new(data).unwrap().validate_crc(false);
    let mut out = Vec::new();
    collect_image_data(&mut chunks, &mut out)?;
    Ok(out)
}

// ---------- success ----------

#[test]
fn single_idat() {
    let data = png(&[chunk(b"IDAT", b"abc"), chunk(b"IEND", b"")]);

    assert_eq!(collect(&data), Ok(b"abc".to_vec()));
}

#[test]
fn consecutive_idats_are_joined() {
    let data = png(&[
        chunk(b"IDAT", b"ab"),
        chunk(b"IDAT", b""),
        chunk(b"IDAT", b"cde"),
        chunk(b"IEND", b""),
    ]);

    assert_eq!(collect(&data), Ok(b"abcde".to_vec()));
}

#[test]
fn chunks_before_idat_are_skipped() {
    let data = png(&[
        chunk(b"PLTE", &[0, 0, 0]),
        chunk(b"tEXt", b"Title\0x"),
        chunk(b"ruSt", b"private"),
        chunk(b"IDAT", b"abc"),
        chunk(b"IEND", b""),
    ]);

    assert_eq!(collect(&data), Ok(b"abc".to_vec()));
}

#[test]
fn chunks_after_idat_are_skipped() {
    let data = png(&[chunk(b"IDAT", b"abc"), chunk(b"tEXt", b"Author\0x"), chunk(b"IEND", b"")]);

    assert_eq!(collect(&data), Ok(b"abc".to_vec()));
}

#[test]
fn stops_after_iend() {
    let data = png(&[chunk(b"IDAT", b"abc"), chunk(b"IEND", b""), chunk(b"tEXt", b"late\0x")]);
    let mut chunks = ChunkReader::new(&data).unwrap().validate_crc(false);
    let mut out = Vec::new();

    collect_image_data(&mut chunks, &mut out).unwrap();

    let next = chunks.next_chunk().unwrap().unwrap();
    assert_eq!(next.chunk_type().as_bytes(), b"tEXt");
}

#[test]
fn appends_to_existing_contents() {
    let data = png(&[chunk(b"IDAT", b"abc"), chunk(b"IEND", b"")]);
    let mut chunks = ChunkReader::new(&data).unwrap().validate_crc(false);
    let mut out = b"xy".to_vec();

    collect_image_data(&mut chunks, &mut out).unwrap();

    assert_eq!(out, b"xyabc");
}

#[test]
fn empty_idat_is_still_image_data() {
    let data = png(&[chunk(b"IDAT", b""), chunk(b"IEND", b"")]);

    assert_eq!(collect(&data), Ok(Vec::new()));
}

// ---------- errors ----------

#[test]
fn iend_before_idat_fails() {
    let data = png(&[chunk(b"tEXt", b"Title\0x"), chunk(b"IEND", b"")]);

    assert_eq!(collect(&data), Err(Error::MissingImageData));
}

#[test]
fn only_iend_fails() {
    assert_eq!(collect(&png(&[chunk(b"IEND", b"")])), Err(Error::MissingImageData));
}

#[test]
fn idat_after_another_chunk_after_idat_fails() {
    let data = png(&[
        chunk(b"IDAT", b"ab"),
        chunk(b"tEXt", b"Title\0x"),
        chunk(b"IDAT", b"cd"),
        chunk(b"IEND", b""),
    ]);

    assert_eq!(collect(&data), Err(Error::NonConsecutiveImageData));
}

#[test]
fn missing_iend_fails() {
    assert_eq!(collect(&png(&[chunk(b"IDAT", b"abc")])), Err(Error::MissingImageEnd));
}

#[test]
fn no_chunks_at_all_fails() {
    assert_eq!(collect(&SIGNATURE), Err(Error::MissingImageEnd));
}

#[test]
fn truncated_chunk_fails() {
    let mut data = png(&[chunk(b"IDAT", b"abcdef")]);
    data.truncate(data.len() - 6);

    assert_eq!(collect(&data), Err(Error::UnexpectedEndOfInput));
}

#[test]
fn crc_errors_are_passed_on() {
    let data = png(&[chunk(b"IDAT", b"abc"), chunk(b"IEND", b"")]);
    let mut chunks = ChunkReader::new(&data).unwrap(); // CRC validation on
    let mut out = Vec::new();

    assert!(matches!(
        collect_image_data(&mut chunks, &mut out),
        Err(Error::CrcMismatch { .. })
    ));
}
