#![cfg(test)]

use rust_deflate::Decompressor;

use super::Decoder;
use crate::decode::options::DecodeOptions;
use crate::error::Error;
use crate::png::{ChunkType, ColorType, ImageHeader, Interlace, SIGNATURE};

/// Signature + IHDR for a 1x1, 8-bit RGBA image, with its correct CRC.
const PNG_1X1_RGBA: [u8; 33] = [
    0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, // signature
    0x00, 0x00, 0x00, 0x0D, b'I', b'H', b'D', b'R', // length = 13, type
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // width = 1, height = 1
    0x08, 0x06, 0x00, 0x00, 0x00, // bit depth, color type, compression, filter, interlace
    0x1F, 0x15, 0xC4, 0x89, // crc
];

/// Signature + IHDR for a 640x480, 16-bit RGB, Adam7 image, with its correct CRC.
const PNG_640X480_RGB16_ADAM7: [u8; 33] = [
    0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, // signature
    0x00, 0x00, 0x00, 0x0D, b'I', b'H', b'D', b'R', // length = 13, type
    0x00, 0x00, 0x02, 0x80, 0x00, 0x00, 0x01, 0xE0, // width = 640, height = 480
    0x10, 0x02, 0x00, 0x00, 0x01, // bit depth, color type, compression, filter, interlace
    0x9D, 0x24, 0xA7, 0x66, // crc
];

/// IEND chunk with its correct CRC.
const IEND_CHUNK: [u8; 12] = [
    0x00, 0x00, 0x00, 0x00, b'I', b'E', b'N', b'D', 0xAE, 0x42, 0x60, 0x82,
];

fn rgba_1x1_header() -> ImageHeader {
    ImageHeader {
        width: 1,
        height: 1,
        bit_depth: 8,
        color_type: ColorType::Rgba,
        interlace: Interlace::None,
    }
}

fn lenient_options() -> DecodeOptions {
    DecodeOptions { validate_crc: false, ..DecodeOptions::default() }
}

fn with_corrupted_crc(png: &[u8]) -> Vec<u8> {
    let mut data = png.to_vec();
    *data.last_mut().unwrap() ^= 0xFF;
    data
}

// ---------- construction ----------

#[test]
fn new_uses_default_options() {
    let decoder = Decoder::new();

    assert!(decoder.options().validate_crc);
}

#[test]
fn with_options_keeps_options() {
    let decoder = Decoder::with_options(lenient_options());

    assert!(!decoder.options().validate_crc);
}

#[test]
fn decompressor_can_move_between_decoders() {
    let first = Decoder::with_decompressor(DecodeOptions::default(), Decompressor::new());
    let second = Decoder::with_decompressor(lenient_options(), first.into_decompressor());

    assert!(!second.options().validate_crc);
}

// ---------- chunks ----------

#[test]
fn chunks_exposes_first_chunk() {
    let decoder = Decoder::new();
    let mut chunks = decoder.chunks(&PNG_1X1_RGBA).unwrap();

    let chunk = chunks.next_chunk().unwrap().unwrap();
    assert_eq!(chunk.chunk_type(), ChunkType::IHDR);
    assert_eq!(chunk.data().len(), ImageHeader::LENGTH);
}

#[test]
fn chunks_rejects_non_png() {
    let decoder = Decoder::new();

    assert!(matches!(decoder.chunks(b"GIF89a"), Err(Error::InvalidSignature)));
}

#[test]
fn chunks_respects_validate_crc_option() {
    let data = with_corrupted_crc(&PNG_1X1_RGBA);

    let strict = Decoder::new();
    assert!(matches!(
        strict.chunks(&data).unwrap().next_chunk(),
        Err(Error::CrcMismatch { .. })
    ));

    let lenient = Decoder::with_options(lenient_options());
    assert!(lenient.chunks(&data).unwrap().next_chunk().unwrap().is_some());
}

// ---------- read_header ----------

#[test]
fn read_header_parses_ihdr() {
    let mut decoder = Decoder::new();

    assert_eq!(decoder.read_header(&PNG_1X1_RGBA), Ok(rgba_1x1_header()));
}

#[test]
fn read_header_ignores_following_chunks() {
    let mut data = PNG_1X1_RGBA.to_vec();
    data.extend_from_slice(&IEND_CHUNK);
    let mut decoder = Decoder::new();

    assert_eq!(decoder.read_header(&data), Ok(rgba_1x1_header()));
}

#[test]
fn read_header_can_be_reused_for_many_images() {
    let mut decoder = Decoder::new();

    assert_eq!(decoder.read_header(&PNG_1X1_RGBA), Ok(rgba_1x1_header()));
    assert_eq!(
        decoder.read_header(&PNG_640X480_RGB16_ADAM7),
        Ok(ImageHeader {
            width: 640,
            height: 480,
            bit_depth: 16,
            color_type: ColorType::Rgb,
            interlace: Interlace::Adam7,
        })
    );
    assert_eq!(decoder.read_header(&PNG_1X1_RGBA), Ok(rgba_1x1_header()));
}

#[test]
fn read_header_still_works_after_a_failure() {
    let mut decoder = Decoder::new();

    assert!(decoder.read_header(b"not a png").is_err());
    assert_eq!(decoder.read_header(&PNG_1X1_RGBA), Ok(rgba_1x1_header()));
}

#[test]
fn read_header_rejects_invalid_signature() {
    let mut decoder = Decoder::new();

    assert_eq!(decoder.read_header(&PNG_1X1_RGBA[1..]), Err(Error::InvalidSignature));
}

#[test]
fn read_header_rejects_signature_without_chunks() {
    let mut decoder = Decoder::new();

    assert_eq!(decoder.read_header(&SIGNATURE), Err(Error::MissingImageHeader));
}

#[test]
fn read_header_rejects_other_first_chunk() {
    let mut data = SIGNATURE.to_vec();
    data.extend_from_slice(&IEND_CHUNK);
    let mut decoder = Decoder::new();

    assert_eq!(decoder.read_header(&data), Err(Error::MissingImageHeader));
}

#[test]
fn read_header_rejects_truncated_ihdr() {
    let mut decoder = Decoder::new();

    assert_eq!(decoder.read_header(&PNG_1X1_RGBA[..20]), Err(Error::UnexpectedEndOfInput));
}

#[test]
fn read_header_rejects_wrong_crc_by_default() {
    let mut decoder = Decoder::new();

    assert!(matches!(
        decoder.read_header(&with_corrupted_crc(&PNG_1X1_RGBA)),
        Err(Error::CrcMismatch { .. })
    ));
}

#[test]
fn read_header_accepts_wrong_crc_when_validation_off() {
    let mut decoder = Decoder::with_options(lenient_options());

    assert_eq!(decoder.read_header(&with_corrupted_crc(&PNG_1X1_RGBA)), Ok(rgba_1x1_header()));
}

#[test]
fn read_header_rejects_invalid_ihdr_contents() {
    // color type 5; CRC validation is off so only the field itself is wrong
    let mut data = PNG_1X1_RGBA.to_vec();
    data[25] = 5;
    let mut decoder = Decoder::with_options(lenient_options());

    assert_eq!(decoder.read_header(&data), Err(Error::InvalidColorType(5)));
}
