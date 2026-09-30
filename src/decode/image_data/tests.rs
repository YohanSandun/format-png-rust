#![cfg(test)]

use super::{ImageChunks, collect_image_data};
use crate::decode::chunk_reader::ChunkReader;
use crate::error::Error;
use crate::png::{ColorType, ImageHeader, Interlace, Palette, SIGNATURE};

fn header(color_type: ColorType) -> ImageHeader {
    ImageHeader { width: 1, height: 1, bit_depth: 8, color_type, interlace: Interlace::None }
}

/// An 8-bit RGB header, for tests that don't involve a palette.
fn rgb() -> ImageHeader {
    header(ColorType::Rgb)
}

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

fn collect_with(header: &ImageHeader, data: &[u8]) -> Result<(Vec<u8>, ImageChunks), Error> {
    let mut chunks = ChunkReader::new(data).unwrap().validate_crc(false);
    let mut out = Vec::new();
    let found = collect_image_data(&mut chunks, header, &mut out)?;
    Ok((out, found))
}

fn collect(data: &[u8]) -> Result<Vec<u8>, Error> {
    collect_with(&rgb(), data).map(|(out, _)| out)
}

/// The palette `collect_image_data` returns for `data`.
fn palette_of(header: &ImageHeader, data: &[u8]) -> Result<Option<Palette>, Error> {
    collect_with(header, data).map(|(_, found)| found.palette)
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

    collect_image_data(&mut chunks, &rgb(), &mut out).unwrap();

    let next = chunks.next_chunk().unwrap().unwrap();
    assert_eq!(next.chunk_type().as_bytes(), b"tEXt");
}

#[test]
fn appends_to_existing_contents() {
    let data = png(&[chunk(b"IDAT", b"abc"), chunk(b"IEND", b"")]);
    let mut chunks = ChunkReader::new(&data).unwrap().validate_crc(false);
    let mut out = b"xy".to_vec();

    collect_image_data(&mut chunks, &rgb(), &mut out).unwrap();

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
        collect_image_data(&mut chunks, &rgb(), &mut out),
        Err(Error::CrcMismatch { .. })
    ));
}

// ---------- PLTE ----------

#[test]
fn indexed_image_returns_its_palette() {
    let data = png(&[chunk(b"PLTE", &[1, 2, 3, 4, 5, 6]), chunk(b"IDAT", b"abc"), chunk(b"IEND", b"")]);

    let palette = palette_of(&header(ColorType::Indexed), &data).unwrap().unwrap();

    assert_eq!(palette.colors(), &[[1, 2, 3], [4, 5, 6]]);
}

#[test]
fn palette_is_found_among_other_chunks() {
    let data = png(&[
        chunk(b"gAMA", &[0, 0, 177, 143]),
        chunk(b"PLTE", &[1, 2, 3]),
        chunk(b"tEXt", b"Title\0x"),
        chunk(b"IDAT", b"abc"),
        chunk(b"IEND", b""),
    ]);

    let (out, found) = collect_with(&header(ColorType::Indexed), &data).unwrap();

    assert_eq!(out, b"abc");
    assert_eq!(found.palette.unwrap().colors(), &[[1, 2, 3]]);
}

#[test]
fn rgb_and_rgba_return_a_suggested_palette() {
    let data = png(&[chunk(b"PLTE", &[1, 2, 3]), chunk(b"IDAT", b"abc"), chunk(b"IEND", b"")]);

    for color_type in [ColorType::Rgb, ColorType::Rgba] {
        let palette = palette_of(&header(color_type), &data).unwrap();

        assert_eq!(palette.map(|p| p.len()), Some(1), "{color_type:?}");
    }
}

#[test]
fn images_without_plte_return_no_palette() {
    let data = png(&[chunk(b"IDAT", b"abc"), chunk(b"IEND", b"")]);

    for color_type in [ColorType::Grayscale, ColorType::GrayscaleAlpha, ColorType::Rgb, ColorType::Rgba] {
        assert_eq!(palette_of(&header(color_type), &data), Ok(None), "{color_type:?}");
    }
}

#[test]
fn indexed_image_without_plte_fails() {
    let data = png(&[chunk(b"IDAT", b"abc"), chunk(b"IEND", b"")]);

    assert_eq!(palette_of(&header(ColorType::Indexed), &data), Err(Error::MissingPalette));
}

#[test]
fn indexed_image_with_plte_only_after_idat_fails() {
    // The missing palette is noticed when image data starts, before the late PLTE.
    let data = png(&[chunk(b"IDAT", b"abc"), chunk(b"PLTE", &[1, 2, 3]), chunk(b"IEND", b"")]);

    assert_eq!(palette_of(&header(ColorType::Indexed), &data), Err(Error::MissingPalette));
}

#[test]
fn plte_after_idat_fails() {
    for data in [
        png(&[chunk(b"IDAT", b"abc"), chunk(b"PLTE", &[1, 2, 3]), chunk(b"IEND", b"")]),
        png(&[chunk(b"IDAT", b"a"), chunk(b"PLTE", &[1, 2, 3]), chunk(b"IDAT", b"b"), chunk(b"IEND", b"")]),
    ] {
        assert_eq!(palette_of(&rgb(), &data), Err(Error::PaletteAfterImageData));
    }
}

#[test]
fn second_plte_fails() {
    let data = png(&[
        chunk(b"PLTE", &[1, 2, 3]),
        chunk(b"PLTE", &[4, 5, 6]),
        chunk(b"IDAT", b"abc"),
        chunk(b"IEND", b""),
    ]);

    assert_eq!(palette_of(&header(ColorType::Indexed), &data), Err(Error::DuplicatePalette));
}

#[test]
fn plte_for_grayscale_fails() {
    let data = png(&[chunk(b"PLTE", &[1, 2, 3]), chunk(b"IDAT", b"abc"), chunk(b"IEND", b"")]);

    for color_type in [ColorType::Grayscale, ColorType::GrayscaleAlpha] {
        assert_eq!(palette_of(&header(color_type), &data), Err(Error::UnexpectedPalette(color_type)));
    }
}

#[test]
fn invalid_plte_contents_fail() {
    let data = png(&[chunk(b"PLTE", &[1, 2, 3, 4]), chunk(b"IDAT", b"abc"), chunk(b"IEND", b"")]);

    assert_eq!(palette_of(&header(ColorType::Indexed), &data), Err(Error::InvalidPaletteLength(4)));
}