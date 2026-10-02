#![cfg(test)]

use rust_deflate::Decompressor;

use super::Decoder;
use crate::decode::options::DecodeOptions;
use crate::error::Error;
use crate::png::{ChunkType, ColorType, ImageHeader, Interlace, PixelFormat, SIGNATURE, Transparency};

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

// ---------- decode: helpers ----------

fn chunk(chunk_type: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = (data.len() as u32).to_be_bytes().to_vec();
    out.extend_from_slice(chunk_type);
    out.extend_from_slice(data);
    out.extend_from_slice(&crate::png::crc::crc32(chunk_type, data).to_be_bytes());
    out
}

fn ihdr(width: u32, height: u32, bit_depth: u8, color_type: ColorType, interlace: Interlace) -> Vec<u8> {
    let mut data = width.to_be_bytes().to_vec();
    data.extend_from_slice(&height.to_be_bytes());
    data.extend_from_slice(&[bit_depth, color_type as u8, 0, 0, interlace as u8]);
    chunk(b"IHDR", &data)
}

/// Signature, then `chunks`.
fn png_of(chunks: &[Vec<u8>]) -> Vec<u8> {
    let mut data = SIGNATURE.to_vec();
    for chunk in chunks {
        data.extend_from_slice(chunk);
    }
    data
}

/// A complete PNG whose single IDAT holds `scanlines`, zlib-compressed.
fn png(header: Vec<u8>, scanlines: &[u8]) -> Vec<u8> {
    png_of(&[header, chunk(b"IDAT", &rust_deflate::compress_zlib(scanlines)), chunk(b"IEND", b"")])
}

fn rgba_1x1() -> Vec<u8> {
    png(ihdr(1, 1, 8, ColorType::Rgba, Interlace::None), &[0, 1, 2, 3, 4])
}

// ---------- decode: success ----------

#[test]
fn decode_1x1_rgba() {
    let image = Decoder::new().decode(&rgba_1x1()).unwrap();

    assert_eq!(*image.header(), rgba_1x1_header());
    assert_eq!(image.stride(), 4);
    assert_eq!(image.data(), &[1, 2, 3, 4]);
}

#[test]
fn decode_reverses_filters() {
    // 2x2 gray: row 0 uses Sub, row 1 uses Up
    let data = png(ihdr(2, 2, 8, ColorType::Grayscale, Interlace::None), &[1, 10, 5, 2, 1, 1]);

    assert_eq!(Decoder::new().decode(&data).unwrap().data(), &[10, 15, 11, 16]);
}

#[test]
fn decode_keeps_sub_byte_pixels_packed() {
    // 10x1 1-bit gray: 2 bytes per row, the last 6 bits padding
    let data = png(ihdr(10, 1, 1, ColorType::Grayscale, Interlace::None), &[0, 0b1010_1010, 0b1100_0000]);
    let image = Decoder::new().decode(&data).unwrap();

    assert_eq!(image.stride(), 2);
    assert_eq!(image.data(), &[0b1010_1010, 0b1100_0000]);
}

#[test]
fn decode_keeps_16_bit_samples_big_endian() {
    let data = png(ihdr(2, 1, 16, ColorType::Grayscale, Interlace::None), &[0, 0x12, 0x34, 0xAB, 0xCD]);

    assert_eq!(Decoder::new().decode(&data).unwrap().data(), &[0x12, 0x34, 0xAB, 0xCD]);
}

#[test]
fn decode_puts_adam7_passes_back_together() {
    // 3x2 gray: pass 1 is (0, 0), pass 4 is (2, 0), pass 6 is (1, 0), pass 7 is row 1.
    // Pass 7 uses Sub to check each pass is unfiltered on its own.
    let scanlines = [0, 1, 0, 3, 0, 2, 1, 4, 1, 1];
    let data = png(ihdr(3, 2, 8, ColorType::Grayscale, Interlace::Adam7), &scanlines);
    let image = Decoder::new().decode(&data).unwrap();

    assert_eq!(image.header().interlace, Interlace::Adam7);
    assert_eq!(image.data(), &[1, 2, 3, 4, 5, 6]);
}

#[test]
fn decode_joins_split_idat_chunks() {
    let compressed = rust_deflate::compress_zlib(&[0, 1, 2, 3, 4]);
    let (first, rest) = compressed.split_at(3);
    let data = png_of(&[
        ihdr(1, 1, 8, ColorType::Rgba, Interlace::None),
        chunk(b"IDAT", first),
        chunk(b"IDAT", b""),
        chunk(b"IDAT", rest),
        chunk(b"IEND", b""),
    ]);

    assert_eq!(Decoder::new().decode(&data).unwrap().data(), &[1, 2, 3, 4]);
}

#[test]
fn decode_skips_ancillary_chunks() {
    let data = png_of(&[
        ihdr(1, 1, 8, ColorType::Rgba, Interlace::None),
        chunk(b"tEXt", b"Title\0x"),
        chunk(b"IDAT", &rust_deflate::compress_zlib(&[0, 1, 2, 3, 4])),
        chunk(b"ruSt", b"private"),
        chunk(b"IEND", b""),
    ]);

    assert_eq!(Decoder::new().decode(&data).unwrap().data(), &[1, 2, 3, 4]);
}

#[test]
fn decode_into_replaces_old_contents() {
    let mut out = vec![0xEE; 100];

    let header = Decoder::new().decode_into(&rgba_1x1(), &mut out).unwrap();

    assert_eq!(header, rgba_1x1_header());
    assert_eq!(out, [1, 2, 3, 4]);
}

#[test]
fn decoder_and_buffer_can_be_reused_for_different_images() {
    let mut decoder = Decoder::new();
    let mut out = Vec::new();
    let gray = png(ihdr(2, 2, 8, ColorType::Grayscale, Interlace::None), &[1, 10, 5, 2, 1, 1]);
    let interlaced = png(ihdr(3, 2, 8, ColorType::Grayscale, Interlace::Adam7), &[0, 1, 0, 3, 0, 2, 1, 4, 1, 1]);

    decoder.decode_into(&gray, &mut out).unwrap();
    assert_eq!(out, [10, 15, 11, 16]);

    decoder.decode_into(&interlaced, &mut out).unwrap();
    assert_eq!(out, [1, 2, 3, 4, 5, 6]);

    decoder.decode_into(&rgba_1x1(), &mut out).unwrap();
    assert_eq!(out, [1, 2, 3, 4]);
}

#[test]
fn decode_still_works_after_a_failure() {
    let mut decoder = Decoder::new();
    let broken = png(ihdr(1, 1, 8, ColorType::Rgba, Interlace::None), &[9, 1, 2, 3, 4]);

    assert!(decoder.decode(&broken).is_err());
    assert_eq!(decoder.decode(&rgba_1x1()).unwrap().data(), &[1, 2, 3, 4]);
}

// ---------- decode: errors ----------

#[test]
fn decode_passes_on_header_errors() {
    let mut decoder = Decoder::new();

    assert_eq!(decoder.decode(b"not a png"), Err(Error::InvalidSignature));
    assert_eq!(decoder.decode(&SIGNATURE), Err(Error::MissingImageHeader));
}

#[test]
fn decode_rejects_missing_idat() {
    let data = png_of(&[ihdr(1, 1, 8, ColorType::Rgba, Interlace::None), chunk(b"IEND", b"")]);

    assert_eq!(Decoder::new().decode(&data), Err(Error::MissingImageData));
}

#[test]
fn decode_rejects_missing_iend() {
    let data = png_of(&[
        ihdr(1, 1, 8, ColorType::Rgba, Interlace::None),
        chunk(b"IDAT", &rust_deflate::compress_zlib(&[0, 1, 2, 3, 4])),
    ]);

    assert_eq!(Decoder::new().decode(&data), Err(Error::MissingImageEnd));
}

#[test]
fn decode_rejects_non_consecutive_idat() {
    let compressed = rust_deflate::compress_zlib(&[0, 1, 2, 3, 4]);
    let (first, rest) = compressed.split_at(3);
    let data = png_of(&[
        ihdr(1, 1, 8, ColorType::Rgba, Interlace::None),
        chunk(b"IDAT", first),
        chunk(b"tEXt", b"Title\0x"),
        chunk(b"IDAT", rest),
        chunk(b"IEND", b""),
    ]);

    assert_eq!(Decoder::new().decode(&data), Err(Error::NonConsecutiveImageData));
}

#[test]
fn decode_rejects_short_image_data() {
    let data = png(ihdr(1, 1, 8, ColorType::Rgba, Interlace::None), &[0, 1, 2]);

    assert_eq!(
        Decoder::new().decode(&data),
        Err(Error::ImageDataTooShort { expected: 5, actual: 3 })
    );
}

#[test]
fn decode_rejects_long_image_data() {
    let data = png(ihdr(1, 1, 8, ColorType::Rgba, Interlace::None), &[0, 1, 2, 3, 4, 5]);

    assert_eq!(Decoder::new().decode(&data), Err(Error::ImageDataTooLong { expected: 5 }));
}

#[test]
fn decode_rejects_invalid_filter_type() {
    let data = png(ihdr(1, 1, 8, ColorType::Rgba, Interlace::None), &[7, 1, 2, 3, 4]);

    assert_eq!(Decoder::new().decode(&data), Err(Error::InvalidFilterType(7)));
}

#[test]
fn decode_rejects_invalid_filter_type_in_adam7_pass() {
    let scanlines = [0, 1, 0, 3, 0, 2, 9, 4, 1, 1];
    let data = png(ihdr(3, 2, 8, ColorType::Grayscale, Interlace::Adam7), &scanlines);

    assert_eq!(Decoder::new().decode(&data), Err(Error::InvalidFilterType(9)));
}

#[test]
fn decode_rejects_corrupt_zlib_stream() {
    let data = png_of(&[
        ihdr(1, 1, 8, ColorType::Rgba, Interlace::None),
        chunk(b"IDAT", &[0x78, 0x9C, 0xFF, 0xFF]),
        chunk(b"IEND", b""),
    ]);

    assert!(matches!(Decoder::new().decode(&data), Err(Error::Decompression(_))));
}

#[test]
fn decode_rejects_wrong_adler32() {
    let mut compressed = rust_deflate::compress_zlib(&[0, 1, 2, 3, 4]);
    *compressed.last_mut().unwrap() ^= 0xFF;
    let data = png_of(&[
        ihdr(1, 1, 8, ColorType::Rgba, Interlace::None),
        chunk(b"IDAT", &compressed),
        chunk(b"IEND", b""),
    ]);

    assert_eq!(
        Decoder::new().decode(&data),
        Err(Error::Decompression(rust_deflate::Error::ChecksumMismatch))
    );
}

#[test]
fn decode_checks_idat_crc_unless_turned_off() {
    let mut data = rgba_1x1();
    let iend = data.len() - 12;
    data[iend - 1] ^= 0xFF; // last byte of the IDAT CRC

    assert!(matches!(Decoder::new().decode(&data), Err(Error::CrcMismatch { .. })));
    assert_eq!(
        Decoder::with_options(lenient_options()).decode(&data).unwrap().data(),
        &[1, 2, 3, 4]
    );
}

// ---------- decode_bitmap ----------

#[test]
fn decode_bitmap_rgba_1x1() {
    let bitmap = Decoder::new().decode_bitmap(&rgba_1x1(), PixelFormat::Rgba8).unwrap();

    assert_eq!((bitmap.width(), bitmap.height()), (1, 1));
    assert_eq!(bitmap.format(), PixelFormat::Rgba8);
    assert_eq!(bitmap.data(), &[1, 2, 3, 4]);
}

#[test]
fn decode_bitmap_rgb_drops_alpha() {
    let bitmap = Decoder::new().decode_bitmap(&rgba_1x1(), PixelFormat::Rgb8).unwrap();

    assert_eq!(bitmap.data(), &[1, 2, 3]);
}

#[test]
fn decode_bitmap_expands_gray() {
    let data = png(ihdr(2, 2, 8, ColorType::Grayscale, Interlace::None), &[1, 10, 5, 2, 1, 1]);
    let bitmap = Decoder::new().decode_bitmap(&data, PixelFormat::Rgb8).unwrap();

    assert_eq!(bitmap.data(), &[10, 10, 10, 15, 15, 15, 11, 11, 11, 16, 16, 16]);
}

#[test]
fn decode_bitmap_of_interlaced_image() {
    let data = png(ihdr(3, 2, 8, ColorType::Grayscale, Interlace::Adam7), &[0, 1, 0, 3, 0, 2, 1, 4, 1, 1]);
    let bitmap = Decoder::new().decode_bitmap(&data, PixelFormat::Rgba8).unwrap();

    let expected: Vec<u8> = (1..=6).flat_map(|v| [v, v, v, 255]).collect();
    assert_eq!(bitmap.data(), expected);
}

#[test]
fn decode_bitmap_into_reuses_decoder_and_buffer() {
    let mut decoder = Decoder::new();
    let mut out = vec![0xEE; 64];
    let gray = png(ihdr(2, 2, 8, ColorType::Grayscale, Interlace::None), &[1, 10, 5, 2, 1, 1]);

    let header = decoder.decode_bitmap_into(&rgba_1x1(), PixelFormat::Rgba8, &mut out).unwrap();
    assert_eq!(header, rgba_1x1_header());
    assert_eq!(out, [1, 2, 3, 4]);

    decoder.decode_bitmap_into(&gray, PixelFormat::Rgba8, &mut out).unwrap();
    assert_eq!(out, [10, 10, 10, 255, 15, 15, 15, 255, 11, 11, 11, 255, 16, 16, 16, 255]);
}

#[test]
fn decode_bitmap_still_works_after_a_failure() {
    let mut decoder = Decoder::new();
    let broken = png(ihdr(1, 1, 8, ColorType::Rgba, Interlace::None), &[9, 1, 2, 3, 4]);

    assert_eq!(decoder.decode_bitmap(&broken, PixelFormat::Rgba8), Err(Error::InvalidFilterType(9)));
    assert_eq!(decoder.decode_bitmap(&rgba_1x1(), PixelFormat::Rgba8).unwrap().data(), &[1, 2, 3, 4]);
}

#[test]
fn decode_bitmap_passes_on_decode_errors() {
    assert_eq!(
        Decoder::new().decode_bitmap(b"not a png", PixelFormat::Rgba8),
        Err(Error::InvalidSignature)
    );
}

// ---------- palettes ----------

/// A 3x1, 8-bit indexed PNG with pixels 0, 1, 0 and `palette` as its PLTE data.
fn indexed_3x1(palette: &[u8]) -> Vec<u8> {
    png_of(&[
        ihdr(3, 1, 8, ColorType::Indexed, Interlace::None),
        chunk(b"PLTE", palette),
        chunk(b"IDAT", &rust_deflate::compress_zlib(&[0, 0, 1, 0])),
        chunk(b"IEND", b""),
    ])
}

const TWO_COLORS: [u8; 6] = [10, 20, 30, 40, 50, 60];

#[test]
fn decode_indexed_image_keeps_indices_and_palette() {
    let image = Decoder::new().decode(&indexed_3x1(&TWO_COLORS)).unwrap();

    assert_eq!(image.data(), &[0, 1, 0]);
    assert_eq!(image.palette().unwrap().colors(), &[[10, 20, 30], [40, 50, 60]]);
}

#[test]
fn decode_bitmap_of_indexed_image_uses_the_palette() {
    let bitmap = Decoder::new().decode_bitmap(&indexed_3x1(&TWO_COLORS), PixelFormat::Rgba8).unwrap();

    assert_eq!(bitmap.data(), &[10, 20, 30, 255, 40, 50, 60, 255, 10, 20, 30, 255]);
}

#[test]
fn decoder_palette_is_the_last_images() {
    let mut decoder = Decoder::new();
    let mut pixels = Vec::new();
    assert_eq!(decoder.palette(), None);

    decoder.decode_into(&indexed_3x1(&TWO_COLORS), &mut pixels).unwrap();
    assert_eq!(decoder.palette().map(|p| p.len()), Some(2));

    decoder.decode_into(&rgba_1x1(), &mut pixels).unwrap();
    assert_eq!(decoder.palette(), None);
}

#[test]
fn decoder_palette_is_cleared_by_a_failed_decode() {
    let mut decoder = Decoder::new();
    let mut pixels = Vec::new();
    decoder.decode_into(&indexed_3x1(&TWO_COLORS), &mut pixels).unwrap();

    // One failure after the header, and one in it.
    for broken in [&rgba_1x1()[..40], &b"not a png"[..]] {
        decoder.decode_into(&indexed_3x1(&TWO_COLORS), &mut pixels).unwrap();

        assert!(decoder.decode_into(broken, &mut pixels).is_err());
        assert_eq!(decoder.palette(), None);
    }
}

#[test]
fn decode_rgb_image_keeps_its_suggested_palette() {
    let data = png_of(&[
        ihdr(1, 1, 8, ColorType::Rgb, Interlace::None),
        chunk(b"PLTE", &[1, 2, 3]),
        chunk(b"IDAT", &rust_deflate::compress_zlib(&[0, 7, 8, 9])),
        chunk(b"IEND", b""),
    ]);

    let image = Decoder::new().decode(&data).unwrap();
    assert_eq!(image.palette().map(|p| p.len()), Some(1));
    // ...but it isn't used for converting
    assert_eq!(image.to_bitmap(PixelFormat::Rgb8).unwrap().data(), &[7, 8, 9]);
}

#[test]
fn decode_indexed_image_without_plte_fails() {
    let data = png(ihdr(2, 1, 8, ColorType::Indexed, Interlace::None), &[0, 0, 1]);

    assert_eq!(Decoder::new().decode(&data), Err(Error::MissingPalette));
    assert_eq!(Decoder::new().decode_bitmap(&data, PixelFormat::Rgba8), Err(Error::MissingPalette));
}

#[test]
fn decode_rejects_invalid_plte() {
    assert_eq!(Decoder::new().decode(&indexed_3x1(&[1, 2, 3, 4])), Err(Error::InvalidPaletteLength(4)));
}

#[test]
fn decode_bitmap_rejects_index_past_the_palette() {
    // Pixel 1 has index 1, but the palette has one color.
    let data = indexed_3x1(&[1, 2, 3]);

    assert!(Decoder::new().decode(&data).is_ok());
    assert_eq!(
        Decoder::new().decode_bitmap(&data, PixelFormat::Rgb8),
        Err(Error::PaletteIndexOutOfRange { index: 1, entries: 1 })
    );
}

// ---------- transparency ----------

/// A 2x1, 8-bit grayscale PNG with pixels 5 and 6, and 5 transparent.
fn gray_2x1_with_key() -> Vec<u8> {
    png_of(&[
        ihdr(2, 1, 8, ColorType::Grayscale, Interlace::None),
        chunk(b"tRNS", &[0, 5]),
        chunk(b"IDAT", &rust_deflate::compress_zlib(&[0, 5, 6])),
        chunk(b"IEND", b""),
    ])
}

#[test]
fn decode_keeps_transparency() {
    let image = Decoder::new().decode(&gray_2x1_with_key()).unwrap();

    assert_eq!(image.data(), &[5, 6]);
    assert_eq!(image.transparency(), Some(&Transparency::Gray(5)));
}

#[test]
fn decode_bitmap_applies_transparency() {
    let bitmap = Decoder::new().decode_bitmap(&gray_2x1_with_key(), PixelFormat::Rgba8).unwrap();

    assert_eq!(bitmap.data(), &[5, 5, 5, 0, 6, 6, 6, 255]);
}

#[test]
fn decode_bitmap_of_indexed_image_applies_palette_alpha() {
    let data = png_of(&[
        ihdr(3, 1, 8, ColorType::Indexed, Interlace::None),
        chunk(b"PLTE", &TWO_COLORS),
        chunk(b"tRNS", &[64]),
        chunk(b"IDAT", &rust_deflate::compress_zlib(&[0, 0, 1, 0])),
        chunk(b"IEND", b""),
    ]);

    let bitmap = Decoder::new().decode_bitmap(&data, PixelFormat::Rgba8).unwrap();

    assert_eq!(bitmap.data(), &[10, 20, 30, 64, 40, 50, 60, 255, 10, 20, 30, 64]);
}

#[test]
fn decoder_transparency_is_the_last_images() {
    let mut decoder = Decoder::new();
    let mut pixels = Vec::new();
    assert_eq!(decoder.transparency(), None);

    decoder.decode_into(&gray_2x1_with_key(), &mut pixels).unwrap();
    assert_eq!(decoder.transparency(), Some(&Transparency::Gray(5)));

    decoder.decode_into(&rgba_1x1(), &mut pixels).unwrap();
    assert_eq!(decoder.transparency(), None);
}

#[test]
fn decoder_transparency_is_cleared_by_a_failed_decode() {
    let mut decoder = Decoder::new();
    let mut pixels = Vec::new();

    for broken in [&rgba_1x1()[..40], &b"not a png"[..]] {
        decoder.decode_into(&gray_2x1_with_key(), &mut pixels).unwrap();

        assert!(decoder.decode_into(broken, &mut pixels).is_err());
        assert_eq!(decoder.transparency(), None);
    }
}

#[test]
fn decode_rejects_invalid_trns() {
    let data = png_of(&[
        ihdr(1, 1, 8, ColorType::Rgba, Interlace::None),
        chunk(b"tRNS", &[0, 0]),
        chunk(b"IDAT", &rust_deflate::compress_zlib(&[0, 1, 2, 3, 4])),
        chunk(b"IEND", b""),
    ]);

    assert_eq!(Decoder::new().decode(&data), Err(Error::UnexpectedTransparency(ColorType::Rgba)));
}

// ---------- metadata and preserved chunks ----------

/// A 1x1 RGBA PNG with a gAMA before the image data and a private chunk after it.
fn rgba_1x1_with_ancillary() -> Vec<u8> {
    png_of(&[
        ihdr(1, 1, 8, ColorType::Rgba, Interlace::None),
        chunk(b"gAMA", &45455u32.to_be_bytes()),
        chunk(b"IDAT", &rust_deflate::compress_zlib(&[0, 1, 2, 3, 4])),
        chunk(b"ruSt", b"private"),
        chunk(b"IEND", b""),
    ])
}

fn keep_everything() -> DecodeOptions {
    DecodeOptions { preserve_chunks: true, preserve_metadata: true, ..DecodeOptions::default() }
}

#[test]
fn metadata_and_chunks_are_empty_by_default() {
    let image = Decoder::new().decode(&rgba_1x1_with_ancillary()).unwrap();

    assert!(image.metadata().is_empty());
    assert!(image.ancillary_chunks().is_empty());
}

#[test]
fn decode_keeps_metadata_and_chunks_when_asked() {
    let image = Decoder::with_options(keep_everything()).decode(&rgba_1x1_with_ancillary()).unwrap();

    assert_eq!(image.metadata().gamma().map(|g| g.scaled()), Some(45455));
    let types: Vec<_> = image.ancillary_chunks().iter().map(|c| *c.chunk_type().as_bytes()).collect();
    assert_eq!(types, [*b"gAMA", *b"ruSt"]);
    assert_eq!(image.ancillary_chunks()[1].data(), b"private");
}

#[test]
fn decoder_metadata_is_the_last_images() {
    let mut decoder = Decoder::with_options(keep_everything());
    let mut pixels = Vec::new();

    decoder.decode_into(&rgba_1x1_with_ancillary(), &mut pixels).unwrap();
    assert!(decoder.metadata().gamma().is_some());
    assert_eq!(decoder.ancillary_chunks().len(), 2);

    decoder.decode_into(&rgba_1x1(), &mut pixels).unwrap();
    assert!(decoder.metadata().is_empty());
    assert!(decoder.ancillary_chunks().is_empty());
}

#[test]
fn decoder_metadata_is_cleared_by_a_failed_decode() {
    let mut decoder = Decoder::with_options(keep_everything());
    let mut pixels = Vec::new();

    for broken in [&rgba_1x1()[..40], &b"not a png"[..]] {
        decoder.decode_into(&rgba_1x1_with_ancillary(), &mut pixels).unwrap();

        assert!(decoder.decode_into(broken, &mut pixels).is_err());
        assert!(decoder.metadata().is_empty());
        assert!(decoder.ancillary_chunks().is_empty());
    }
}

#[test]
fn decode_bitmap_keeps_metadata_too() {
    let mut decoder = Decoder::with_options(keep_everything());

    decoder.decode_bitmap(&rgba_1x1_with_ancillary(), PixelFormat::Rgb8).unwrap();

    assert!(decoder.metadata().gamma().is_some());
}

#[test]
fn strict_ancillary_fails_the_decode() {
    let data = png_of(&[
        ihdr(1, 1, 8, ColorType::Rgba, Interlace::None),
        chunk(b"gAMA", &[0, 0, 0, 0]),
        chunk(b"IDAT", &rust_deflate::compress_zlib(&[0, 1, 2, 3, 4])),
        chunk(b"IEND", b""),
    ]);
    let strict = DecodeOptions { preserve_metadata: true, strict_ancillary: true, ..DecodeOptions::default() };

    assert!(Decoder::with_options(keep_everything()).decode(&data).is_ok());
    assert_eq!(Decoder::with_options(strict).decode(&data), Err(Error::InvalidChunkData(ChunkType::GAMA)));
}

// ---------- read_chunks ----------

/// IHDR, known ancillary chunks, a custom critical chunk, the image data, a
/// private chunk after it, IEND, and a chunk after IEND.
fn png_with_every_kind_of_chunk(image_data: &[u8]) -> Vec<u8> {
    png_of(&[
        ihdr(1, 1, 8, ColorType::Rgba, Interlace::None),
        chunk(b"gAMA", &45455u32.to_be_bytes()),
        chunk(b"tEXt", b"Title\0format-png"),
        chunk(b"CuSt", b"custom critical"),
        chunk(b"IDAT", image_data),
        chunk(b"ruSt", b"private"),
        chunk(b"IEND", b""),
        chunk(b"tEXt", b"After\0IEND"),
    ])
}

#[test]
fn read_chunks_keeps_every_chunk_in_file_order() {
    let data = png_with_every_kind_of_chunk(&rust_deflate::compress_zlib(&[0, 1, 2, 3, 4]));

    let png = Decoder::new().read_chunks(&data).unwrap();

    let types: Vec<_> = png.chunks().iter().map(|c| c.chunk_type().to_string()).collect();
    assert_eq!(types, ["IHDR", "gAMA", "tEXt", "CuSt", "IDAT", "ruSt", "IEND"]);
    assert_eq!(png.chunks()[3].data(), b"custom critical");
}

#[test]
fn read_chunks_parses_known_chunks() {
    let data = png_with_every_kind_of_chunk(&rust_deflate::compress_zlib(&[0, 1, 2, 3, 4]));

    let png = Decoder::new().read_chunks(&data).unwrap();

    assert_eq!(*png.header(), rgba_1x1_header());
    assert_eq!(png.metadata().gamma().map(|g| g.scaled()), Some(45455));
    let text = &png.metadata().text()[0];
    assert_eq!((text.keyword.as_str(), text.text.as_str()), ("Title", "format-png"));
}

#[test]
fn read_chunks_lists_unknown_chunks_critical_or_not() {
    let data = png_with_every_kind_of_chunk(&rust_deflate::compress_zlib(&[0, 1, 2, 3, 4]));

    let png = Decoder::new().read_chunks(&data).unwrap();

    let unknown: Vec<_> = png.unknown_chunks().map(|c| (c.chunk_type().to_string(), c.data())).collect();
    assert_eq!(unknown, [("CuSt".to_string(), &b"custom critical"[..]), ("ruSt".to_string(), &b"private"[..])]);
}

#[test]
fn read_chunks_does_not_decompress_image_data() {
    let data = png_with_every_kind_of_chunk(b"not zlib");

    assert!(Decoder::new().read_chunks(&data).is_ok());
    assert!(matches!(Decoder::new().decode(&data), Err(Error::Decompression(_))));
}

#[test]
fn read_chunks_checks_chunk_order() {
    let idat = chunk(b"IDAT", b"x");
    let header = ihdr(1, 1, 8, ColorType::Rgba, Interlace::None);

    let no_idat = png_of(&[header.clone(), chunk(b"IEND", b"")]);
    assert_eq!(Decoder::new().read_chunks(&no_idat), Err(Error::MissingImageData));

    let split = png_of(&[header.clone(), idat.clone(), chunk(b"ruSt", b""), idat.clone(), chunk(b"IEND", b"")]);
    assert_eq!(Decoder::new().read_chunks(&split), Err(Error::NonConsecutiveImageData));

    let no_iend = png_of(&[header, idat]);
    assert_eq!(Decoder::new().read_chunks(&no_iend), Err(Error::MissingImageEnd));
}

#[test]
fn read_chunks_parses_metadata_whatever_preserve_metadata_says() {
    let data = png_with_every_kind_of_chunk(b"x");
    let options = DecodeOptions { preserve_metadata: false, ..DecodeOptions::default() };

    let png = Decoder::with_options(options).read_chunks(&data).unwrap();

    assert!(png.metadata().gamma().is_some());
}

#[test]
fn read_chunks_skips_invalid_metadata_unless_strict() {
    let data = png_of(&[
        ihdr(1, 1, 8, ColorType::Rgba, Interlace::None),
        chunk(b"gAMA", &[0; 3]),
        chunk(b"IDAT", b"x"),
        chunk(b"IEND", b""),
    ]);

    let png = Decoder::new().read_chunks(&data).unwrap();
    assert_eq!(png.metadata().gamma(), None);
    assert_eq!(png.chunks_of_type(ChunkType::GAMA).count(), 1);

    let strict = DecodeOptions { strict_ancillary: true, ..DecodeOptions::default() };
    assert_eq!(
        Decoder::with_options(strict).read_chunks(&data),
        Err(Error::InvalidChunkLength { chunk_type: ChunkType::GAMA, length: 3 })
    );
}

#[test]
fn read_chunks_respects_validate_crc() {
    // Corrupts IEND's CRC: the last chunk read.
    let data = with_corrupted_crc(&png(ihdr(1, 1, 8, ColorType::Rgba, Interlace::None), &[0, 1, 2, 3, 4]));

    assert!(matches!(Decoder::new().read_chunks(&data), Err(Error::CrcMismatch { .. })));
    assert!(Decoder::with_options(lenient_options()).read_chunks(&data).is_ok());
}
