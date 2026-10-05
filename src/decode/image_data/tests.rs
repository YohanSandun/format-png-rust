#![cfg(test)]

use rust_deflate::Decompressor;

use super::collect_image_data;
use crate::decode::chunk_reader::ChunkReader;
use crate::decode::options::DecodeOptions;
use crate::error::Error;
use crate::png::metadata::Time;
use crate::png::{
    ChunkPosition, ColorType, ImageChunks, ImageHeader, Interlace, Palette, SIGNATURE, Transparency,
};

fn header(color_type: ColorType) -> ImageHeader {
    ImageHeader {
        width: 1,
        height: 1,
        bit_depth: 8,
        color_type,
        interlace: Interlace::None,
    }
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
    collect_with_options(header, &DecodeOptions::default(), data)
}

fn collect_with_options(
    header: &ImageHeader,
    options: &DecodeOptions,
    data: &[u8],
) -> Result<(Vec<u8>, ImageChunks), Error> {
    let mut chunks = ChunkReader::new(data).unwrap().validate_crc(false);
    let mut out = Vec::new();
    let found = collect_image_data(
        &mut chunks,
        header,
        options,
        &mut Decompressor::new(),
        &mut out,
    )?;
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
    let data = png(&[
        chunk(b"IDAT", b"abc"),
        chunk(b"tEXt", b"Author\0x"),
        chunk(b"IEND", b""),
    ]);

    assert_eq!(collect(&data), Ok(b"abc".to_vec()));
}

#[test]
fn stops_after_iend() {
    let data = png(&[
        chunk(b"IDAT", b"abc"),
        chunk(b"IEND", b""),
        chunk(b"tEXt", b"late\0x"),
    ]);
    let mut chunks = ChunkReader::new(&data).unwrap().validate_crc(false);
    let mut out = Vec::new();

    collect_image_data(
        &mut chunks,
        &rgb(),
        &DecodeOptions::default(),
        &mut Decompressor::new(),
        &mut out,
    )
    .unwrap();

    let next = chunks.next_chunk().unwrap().unwrap();
    assert_eq!(next.chunk_type().as_bytes(), b"tEXt");
}

#[test]
fn appends_to_existing_contents() {
    let data = png(&[chunk(b"IDAT", b"abc"), chunk(b"IEND", b"")]);
    let mut chunks = ChunkReader::new(&data).unwrap().validate_crc(false);
    let mut out = b"xy".to_vec();

    collect_image_data(
        &mut chunks,
        &rgb(),
        &DecodeOptions::default(),
        &mut Decompressor::new(),
        &mut out,
    )
    .unwrap();

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
    assert_eq!(
        collect(&png(&[chunk(b"IEND", b"")])),
        Err(Error::MissingImageData)
    );
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
    assert_eq!(
        collect(&png(&[chunk(b"IDAT", b"abc")])),
        Err(Error::MissingImageEnd)
    );
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
        collect_image_data(
            &mut chunks,
            &rgb(),
            &DecodeOptions::default(),
            &mut Decompressor::new(),
            &mut out
        ),
        Err(Error::CrcMismatch { .. })
    ));
}

// ---------- PLTE ----------

#[test]
fn indexed_image_returns_its_palette() {
    let data = png(&[
        chunk(b"PLTE", &[1, 2, 3, 4, 5, 6]),
        chunk(b"IDAT", b"abc"),
        chunk(b"IEND", b""),
    ]);

    let palette = palette_of(&header(ColorType::Indexed), &data)
        .unwrap()
        .unwrap();

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
    let data = png(&[
        chunk(b"PLTE", &[1, 2, 3]),
        chunk(b"IDAT", b"abc"),
        chunk(b"IEND", b""),
    ]);

    for color_type in [ColorType::Rgb, ColorType::Rgba] {
        let palette = palette_of(&header(color_type), &data).unwrap();

        assert_eq!(palette.map(|p| p.len()), Some(1), "{color_type:?}");
    }
}

#[test]
fn images_without_plte_return_no_palette() {
    let data = png(&[chunk(b"IDAT", b"abc"), chunk(b"IEND", b"")]);

    for color_type in [
        ColorType::Grayscale,
        ColorType::GrayscaleAlpha,
        ColorType::Rgb,
        ColorType::Rgba,
    ] {
        assert_eq!(
            palette_of(&header(color_type), &data),
            Ok(None),
            "{color_type:?}"
        );
    }
}

#[test]
fn indexed_image_without_plte_fails() {
    let data = png(&[chunk(b"IDAT", b"abc"), chunk(b"IEND", b"")]);

    assert_eq!(
        palette_of(&header(ColorType::Indexed), &data),
        Err(Error::MissingPalette)
    );
}

#[test]
fn indexed_image_with_plte_only_after_idat_fails() {
    // The missing palette is noticed when image data starts, before the late PLTE.
    let data = png(&[
        chunk(b"IDAT", b"abc"),
        chunk(b"PLTE", &[1, 2, 3]),
        chunk(b"IEND", b""),
    ]);

    assert_eq!(
        palette_of(&header(ColorType::Indexed), &data),
        Err(Error::MissingPalette)
    );
}

#[test]
fn plte_after_idat_fails() {
    for data in [
        png(&[
            chunk(b"IDAT", b"abc"),
            chunk(b"PLTE", &[1, 2, 3]),
            chunk(b"IEND", b""),
        ]),
        png(&[
            chunk(b"IDAT", b"a"),
            chunk(b"PLTE", &[1, 2, 3]),
            chunk(b"IDAT", b"b"),
            chunk(b"IEND", b""),
        ]),
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

    assert_eq!(
        palette_of(&header(ColorType::Indexed), &data),
        Err(Error::DuplicatePalette)
    );
}

#[test]
fn plte_for_grayscale_fails() {
    let data = png(&[
        chunk(b"PLTE", &[1, 2, 3]),
        chunk(b"IDAT", b"abc"),
        chunk(b"IEND", b""),
    ]);

    for color_type in [ColorType::Grayscale, ColorType::GrayscaleAlpha] {
        assert_eq!(
            palette_of(&header(color_type), &data),
            Err(Error::UnexpectedPalette(color_type))
        );
    }
}

#[test]
fn invalid_plte_contents_fail() {
    let data = png(&[
        chunk(b"PLTE", &[1, 2, 3, 4]),
        chunk(b"IDAT", b"abc"),
        chunk(b"IEND", b""),
    ]);

    assert_eq!(
        palette_of(&header(ColorType::Indexed), &data),
        Err(Error::InvalidPaletteLength(4))
    );
}

// ---------- tRNS ----------

/// The transparency `collect_image_data` returns for `data`.
fn transparency_of(header: &ImageHeader, data: &[u8]) -> Result<Option<Transparency>, Error> {
    collect_with(header, data).map(|(_, found)| found.transparency)
}

#[test]
fn rgb_image_returns_its_transparent_color() {
    let data = png(&[
        chunk(b"tRNS", &[0, 1, 0, 2, 0, 3]),
        chunk(b"IDAT", b"abc"),
        chunk(b"IEND", b""),
    ]);

    assert_eq!(
        transparency_of(&rgb(), &data),
        Ok(Some(Transparency::Rgb([1, 2, 3])))
    );
}

#[test]
fn gray_image_returns_its_transparent_value() {
    let data = png(&[
        chunk(b"tRNS", &[0, 37]),
        chunk(b"IDAT", b"abc"),
        chunk(b"IEND", b""),
    ]);

    assert_eq!(
        transparency_of(&header(ColorType::Grayscale), &data),
        Ok(Some(Transparency::Gray(37)))
    );
}

#[test]
fn indexed_image_returns_palette_alpha() {
    let data = png(&[
        chunk(b"PLTE", &[1, 2, 3, 4, 5, 6]),
        chunk(b"tRNS", &[0, 128]),
        chunk(b"IDAT", b"abc"),
        chunk(b"IEND", b""),
    ]);

    let (_, found) = collect_with(&header(ColorType::Indexed), &data).unwrap();

    assert_eq!(found.palette.map(|p| p.len()), Some(2));
    let Some(Transparency::Palette(alpha)) = found.transparency else {
        panic!("expected palette alpha")
    };
    assert_eq!(alpha.values(), &[0, 128]);
}

#[test]
fn transparency_is_found_among_other_chunks() {
    let data = png(&[
        chunk(b"gAMA", &[0, 0, 177, 143]),
        chunk(b"tRNS", &[0, 37]),
        chunk(b"tEXt", b"Title\0x"),
        chunk(b"IDAT", b"abc"),
        chunk(b"IEND", b""),
    ]);

    assert_eq!(
        transparency_of(&header(ColorType::Grayscale), &data),
        Ok(Some(Transparency::Gray(37)))
    );
}

#[test]
fn images_without_trns_return_no_transparency() {
    let data = png(&[chunk(b"IDAT", b"abc"), chunk(b"IEND", b"")]);

    for color_type in [
        ColorType::Grayscale,
        ColorType::Rgb,
        ColorType::GrayscaleAlpha,
        ColorType::Rgba,
    ] {
        assert_eq!(
            transparency_of(&header(color_type), &data),
            Ok(None),
            "{color_type:?}"
        );
    }
}

#[test]
fn trns_after_idat_fails() {
    for data in [
        png(&[
            chunk(b"IDAT", b"abc"),
            chunk(b"tRNS", &[0, 1, 0, 2, 0, 3]),
            chunk(b"IEND", b""),
        ]),
        png(&[
            chunk(b"IDAT", b"a"),
            chunk(b"tRNS", &[0, 1, 0, 2, 0, 3]),
            chunk(b"IDAT", b"b"),
            chunk(b"IEND", b""),
        ]),
    ] {
        assert_eq!(
            transparency_of(&rgb(), &data),
            Err(Error::TransparencyAfterImageData)
        );
    }
}

#[test]
fn second_trns_fails() {
    let data = png(&[
        chunk(b"tRNS", &[0, 1, 0, 2, 0, 3]),
        chunk(b"tRNS", &[0, 4, 0, 5, 0, 6]),
        chunk(b"IDAT", b"abc"),
        chunk(b"IEND", b""),
    ]);

    assert_eq!(
        transparency_of(&rgb(), &data),
        Err(Error::DuplicateTransparency)
    );
}

#[test]
fn indexed_trns_before_plte_fails() {
    let data = png(&[
        chunk(b"tRNS", &[0, 128]),
        chunk(b"PLTE", &[1, 2, 3, 4, 5, 6]),
        chunk(b"IDAT", b"abc"),
        chunk(b"IEND", b""),
    ]);

    assert_eq!(
        transparency_of(&header(ColorType::Indexed), &data),
        Err(Error::TransparencyBeforePalette)
    );
}

#[test]
fn rgb_plte_after_trns_fails() {
    // A suggested palette must come before tRNS too.
    let data = png(&[
        chunk(b"tRNS", &[0, 1, 0, 2, 0, 3]),
        chunk(b"PLTE", &[1, 2, 3]),
        chunk(b"IDAT", b"abc"),
        chunk(b"IEND", b""),
    ]);

    assert_eq!(
        transparency_of(&rgb(), &data),
        Err(Error::TransparencyBeforePalette)
    );
}

#[test]
fn plte_after_trns_and_idat_is_reported_as_after_image_data() {
    let data = png(&[
        chunk(b"tRNS", &[0, 1, 0, 2, 0, 3]),
        chunk(b"IDAT", b"abc"),
        chunk(b"PLTE", &[1, 2, 3]),
        chunk(b"IEND", b""),
    ]);

    assert_eq!(
        transparency_of(&rgb(), &data),
        Err(Error::PaletteAfterImageData)
    );
}

#[test]
fn trns_for_color_types_with_alpha_fails() {
    let data = png(&[
        chunk(b"tRNS", &[0, 1]),
        chunk(b"IDAT", b"abc"),
        chunk(b"IEND", b""),
    ]);

    for color_type in [ColorType::GrayscaleAlpha, ColorType::Rgba] {
        assert_eq!(
            transparency_of(&header(color_type), &data),
            Err(Error::UnexpectedTransparency(color_type))
        );
    }
}

#[test]
fn invalid_trns_contents_fail() {
    let data = png(&[
        chunk(b"tRNS", &[0, 1, 2]),
        chunk(b"IDAT", b"abc"),
        chunk(b"IEND", b""),
    ]);

    assert_eq!(
        transparency_of(&rgb(), &data),
        Err(Error::InvalidTransparencyLength {
            color_type: ColorType::Rgb,
            length: 3
        })
    );
}

// ---------- ancillary chunks ----------

fn options(
    preserve_chunks: bool,
    preserve_metadata: bool,
    strict_ancillary: bool,
) -> DecodeOptions {
    DecodeOptions {
        preserve_chunks,
        preserve_metadata,
        strict_ancillary,
        ..DecodeOptions::default()
    }
}

const TIME: [u8; 7] = [0x07, 0xEA, 9, 30, 12, 0, 0]; // 2026-09-30 12:00:00

/// An indexed image with an ancillary chunk in each position.
fn indexed_with_ancillary() -> Vec<u8> {
    png(&[
        chunk(b"gAMA", &45455u32.to_be_bytes()),
        chunk(b"PLTE", &[1, 2, 3]),
        chunk(b"tRNS", &[0]),
        chunk(b"ruSt", b"private"),
        chunk(b"IDAT", b"ab"),
        chunk(b"IDAT", b"c"),
        chunk(b"tIME", &TIME),
        chunk(b"tEXt", b"Author\0x"),
        chunk(b"IEND", b""),
    ])
}

#[test]
fn ancillary_chunks_are_not_kept_by_default() {
    let (out, found) =
        collect_with(&header(ColorType::Indexed), &indexed_with_ancillary()).unwrap();

    assert_eq!(out, b"abc");
    assert!(found.ancillary.is_empty());
    assert!(found.metadata.is_empty());
}

#[test]
fn preserve_chunks_keeps_every_ancillary_chunk_in_order() {
    let (_, found) = collect_with_options(
        &header(ColorType::Indexed),
        &options(true, false, false),
        &indexed_with_ancillary(),
    )
    .unwrap();

    let kept: Vec<_> = found
        .ancillary
        .iter()
        .map(|c| (*c.chunk_type().as_bytes(), c.position()))
        .collect();
    assert_eq!(
        kept,
        [
            (*b"gAMA", ChunkPosition::BeforePalette),
            (*b"tRNS", ChunkPosition::BeforeImageData),
            (*b"ruSt", ChunkPosition::BeforeImageData),
            (*b"tIME", ChunkPosition::AfterImageData),
            (*b"tEXt", ChunkPosition::AfterImageData),
        ]
    );
    // Preserving doesn't parse metadata...
    assert!(found.metadata.is_empty());
    // ...and doesn't stop tRNS from being read as usual.
    assert!(found.transparency.is_some());
}

#[test]
fn chunks_without_plte_are_before_the_palette() {
    let data = png(&[
        chunk(b"tEXt", b"a\0b"),
        chunk(b"IDAT", b"abc"),
        chunk(b"IEND", b""),
    ]);

    let (_, found) = collect_with_options(&rgb(), &options(true, false, false), &data).unwrap();

    assert_eq!(found.ancillary[0].position(), ChunkPosition::BeforePalette);
}

#[test]
fn preserve_metadata_parses_known_chunks() {
    let (_, found) = collect_with_options(
        &header(ColorType::Indexed),
        &options(false, true, false),
        &indexed_with_ancillary(),
    )
    .unwrap();

    assert_eq!(found.metadata.gamma().map(|g| g.scaled()), Some(45455));
    assert!(found.ancillary.is_empty());
}

#[test]
fn chunk_right_after_the_image_data_is_read() {
    // tIME is the first chunk after the IDAT run, which ends the run.
    let (_, found) = collect_with_options(
        &header(ColorType::Indexed),
        &options(false, true, false),
        &indexed_with_ancillary(),
    )
    .unwrap();

    assert_eq!(
        found.metadata.time(),
        Some(Time {
            year: 2026,
            month: 9,
            day: 30,
            hour: 12,
            minute: 0,
            second: 0
        })
    );
}

#[test]
fn idat_after_a_chunk_after_idat_still_fails_with_metadata_on() {
    let data = png(&[
        chunk(b"IDAT", b"ab"),
        chunk(b"tIME", &TIME),
        chunk(b"IDAT", b"cd"),
        chunk(b"IEND", b""),
    ]);

    assert_eq!(
        collect_with_options(&rgb(), &options(true, true, false), &data).map(|_| ()),
        Err(Error::NonConsecutiveImageData)
    );
}

/// An RGB image with a gAMA after the image data, which isn't allowed.
fn misplaced_gamma() -> Vec<u8> {
    png(&[
        chunk(b"IDAT", b"abc"),
        chunk(b"gAMA", &45455u32.to_be_bytes()),
        chunk(b"IEND", b""),
    ])
}

#[test]
fn misplaced_ancillary_chunk_is_skipped_by_default() {
    let (out, found) =
        collect_with_options(&rgb(), &options(false, true, false), &misplaced_gamma()).unwrap();

    assert_eq!(out, b"abc");
    assert_eq!(found.metadata.gamma(), None);
}

#[test]
fn misplaced_ancillary_chunk_fails_when_strict() {
    assert_eq!(
        collect_with_options(&rgb(), &options(false, true, true), &misplaced_gamma()).map(|_| ()),
        Err(Error::MisplacedChunk(crate::png::ChunkType::GAMA))
    );
}

#[test]
fn skipped_chunks_are_still_preserved() {
    // A raw copy doesn't depend on the chunk being valid.
    let (_, found) =
        collect_with_options(&rgb(), &options(true, true, false), &misplaced_gamma()).unwrap();

    assert_eq!(found.ancillary.len(), 1);
    assert_eq!(found.ancillary[0].position(), ChunkPosition::AfterImageData);
}
