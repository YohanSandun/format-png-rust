#![cfg(test)]

use rust_deflate::Decompressor;

use super::{check_position, preserve_chunk, read_ancillary, read_known_chunk};
use crate::error::Error;
use crate::png::metadata::{Gamma, PhysicalDimensions, RenderingIntent, Time, Unit};
use crate::png::{Chunk, ChunkPosition, ChunkType, Metadata};

use ChunkPosition::{AfterImageData, BeforeImageData, BeforePalette};

const ALL_POSITIONS: [ChunkPosition; 3] = [BeforePalette, BeforeImageData, AfterImageData];

fn chunk<'a>(chunk_type: &[u8; 4], data: &'a [u8]) -> Chunk<'a> {
    Chunk::new(ChunkType::from_bytes(*chunk_type).unwrap(), data, 0)
}

const GAMA: [u8; 4] = 45455u32.to_be_bytes();
const PHYS: [u8; 9] = [0, 0, 0x0B, 0x13, 0, 0, 0x0B, 0x13, 1]; // 2835 x 2835 per meter
const TIME: [u8; 7] = [0x07, 0xEA, 9, 30, 12, 0, 0]; // 2026-09-30 12:00:00

/// The sRGB primaries and D65 white point, as cHRM stores them.
fn chrm() -> Vec<u8> {
    [31270u32, 32900, 64000, 33000, 30000, 60000, 15000, 6000].iter().flat_map(|v| v.to_be_bytes()).collect()
}

// ---------- preserve_chunk ----------

#[test]
fn preserve_copies_ancillary_chunks_with_their_position() {
    let mut chunks = Vec::new();

    preserve_chunk(&chunk(b"tEXt", b"Title\0x"), BeforePalette, &mut chunks);
    preserve_chunk(&chunk(b"ruSt", b"private"), AfterImageData, &mut chunks);

    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0].chunk_type().as_bytes(), b"tEXt");
    assert_eq!(chunks[0].data(), b"Title\0x");
    assert_eq!(chunks[0].position(), BeforePalette);
    assert_eq!(chunks[1].chunk_type().as_bytes(), b"ruSt");
    assert_eq!(chunks[1].position(), AfterImageData);
}

#[test]
fn preserve_copies_known_ancillary_chunks_too() {
    let mut chunks = Vec::new();

    preserve_chunk(&chunk(b"gAMA", &GAMA), BeforePalette, &mut chunks);
    preserve_chunk(&chunk(b"tRNS", &[0, 1]), BeforeImageData, &mut chunks);

    assert_eq!(chunks.len(), 2);
}

#[test]
fn preserve_skips_critical_chunks() {
    let mut chunks = Vec::new();

    for chunk_type in [b"IHDR", b"PLTE", b"IDAT", b"IEND", b"UNKN"] {
        preserve_chunk(&chunk(chunk_type, b"data"), BeforePalette, &mut chunks);
    }

    assert!(chunks.is_empty());
}

#[test]
fn preserve_appends() {
    let mut chunks = Vec::new();
    preserve_chunk(&chunk(b"tEXt", b"a\0b"), BeforePalette, &mut chunks);
    preserve_chunk(&chunk(b"tEXt", b"c\0d"), BeforePalette, &mut chunks);

    assert_eq!(chunks.iter().map(|c| c.data()).collect::<Vec<_>>(), [b"a\0b", b"c\0d"]);
}

// ---------- check_position ----------

#[test]
fn color_chunks_must_come_before_the_palette() {
    for chunk_type in [ChunkType::GAMA, ChunkType::CHRM, ChunkType::SRGB] {
        assert_eq!(check_position(chunk_type, BeforePalette), Ok(()));
        assert_eq!(check_position(chunk_type, BeforeImageData), Err(Error::MisplacedChunk(chunk_type)));
        assert_eq!(check_position(chunk_type, AfterImageData), Err(Error::MisplacedChunk(chunk_type)));
    }
}

#[test]
fn phys_must_come_before_the_image_data() {
    assert_eq!(check_position(ChunkType::PHYS, BeforePalette), Ok(()));
    assert_eq!(check_position(ChunkType::PHYS, BeforeImageData), Ok(()));
    assert_eq!(check_position(ChunkType::PHYS, AfterImageData), Err(Error::MisplacedChunk(ChunkType::PHYS)));
}

#[test]
fn time_and_unknown_chunks_may_come_anywhere() {
    for position in ALL_POSITIONS {
        assert_eq!(check_position(ChunkType::TIME, position), Ok(()));
        assert_eq!(check_position(ChunkType::from_bytes(*b"tEXt").unwrap(), position), Ok(()));
    }
}

// ---------- read_known_chunk ----------

fn read(chunk_type: &[u8; 4], data: &[u8], position: ChunkPosition) -> Result<Metadata, Error> {
    let mut metadata = Metadata::default();
    read_known_chunk(&chunk(chunk_type, data), position, &mut Decompressor::new(), &mut metadata)?;
    Ok(metadata)
}

#[test]
fn reads_each_known_chunk() {
    let metadata = read(b"gAMA", &GAMA, BeforePalette).unwrap();
    assert_eq!(metadata.gamma(), Some(Gamma::parse(&GAMA).unwrap()));

    let metadata = read(b"cHRM", &chrm(), BeforePalette).unwrap();
    assert_eq!(metadata.chromaticities().map(|c| c.white_x), Some(31270));

    let metadata = read(b"sRGB", &[0], BeforePalette).unwrap();
    assert_eq!(metadata.srgb(), Some(RenderingIntent::Perceptual));

    let metadata = read(b"pHYs", &PHYS, BeforeImageData).unwrap();
    assert_eq!(metadata.physical_dimensions(), Some(PhysicalDimensions { x: 2835, y: 2835, unit: Unit::Meter }));

    let metadata = read(b"tIME", &TIME, AfterImageData).unwrap();
    assert_eq!(metadata.time(), Some(Time { year: 2026, month: 9, day: 30, hour: 12, minute: 0, second: 0 }));
}

#[test]
fn unknown_chunks_are_skipped() {
    for (chunk_type, data) in [(b"ruSt", &b"private"[..]), (b"zzZZ", b"")] {
        assert_eq!(read(chunk_type, data, BeforePalette), Ok(Metadata::default()));
    }
}

#[test]
fn text_chunks_are_read_anywhere_and_repeatedly() {
    let mut metadata = Metadata::default();
    let mut decompressor = Decompressor::new();

    read_known_chunk(&chunk(b"tEXt", b"Title\0first"), BeforePalette, &mut decompressor, &mut metadata).unwrap();
    read_known_chunk(&chunk(b"tEXt", b"Title\0second"), AfterImageData, &mut decompressor, &mut metadata).unwrap();

    let texts: Vec<_> = metadata.text().iter().map(|t| (t.keyword.as_str(), t.text.as_str())).collect();
    assert_eq!(texts, [("Title", "first"), ("Title", "second")]);
}

#[test]
fn trns_is_not_read_here() {
    // The decoder reads tRNS itself, before this is called.
    assert_eq!(read(b"tRNS", &[0, 1], BeforePalette), Ok(Metadata::default()));
}

#[test]
fn misplaced_chunk_fails_and_changes_nothing() {
    let mut metadata = Metadata::default();

    assert_eq!(
        read_known_chunk(&chunk(b"gAMA", &GAMA), AfterImageData, &mut Decompressor::new(), &mut metadata),
        Err(Error::MisplacedChunk(ChunkType::GAMA))
    );
    assert!(metadata.is_empty());
}

#[test]
fn invalid_chunk_fails_and_changes_nothing() {
    let mut metadata = Metadata::default();

    assert_eq!(
        read_known_chunk(&chunk(b"pHYs", &[0; 4]), BeforePalette, &mut Decompressor::new(), &mut metadata),
        Err(Error::InvalidChunkLength { chunk_type: ChunkType::PHYS, length: 4 })
    );
    assert!(metadata.is_empty());
}

#[test]
fn second_chunk_of_a_type_fails_and_keeps_the_first() {
    let mut metadata = Metadata::default();
    read_known_chunk(&chunk(b"tIME", &TIME), BeforePalette, &mut Decompressor::new(), &mut metadata).unwrap();

    let later = [0x07, 0xEB, 1, 1, 0, 0, 0];
    assert_eq!(
        read_known_chunk(&chunk(b"tIME", &later), AfterImageData, &mut Decompressor::new(), &mut metadata),
        Err(Error::DuplicateChunk(ChunkType::TIME))
    );
    assert_eq!(metadata.time().map(|t| t.year), Some(2026));
}

#[test]
fn duplicate_is_reported_even_if_the_second_is_invalid() {
    // Either error is reasonable; this pins the order: position, then duplicate, then parse.
    let mut metadata = Metadata::default();
    read_known_chunk(&chunk(b"gAMA", &GAMA), BeforePalette, &mut Decompressor::new(), &mut metadata).unwrap();

    assert_eq!(
        read_known_chunk(&chunk(b"gAMA", &[0; 4]), BeforePalette, &mut Decompressor::new(), &mut metadata),
        Err(Error::DuplicateChunk(ChunkType::GAMA))
    );
}

#[test]
fn different_chunks_fill_different_fields() {
    let mut metadata = Metadata::default();

    read_known_chunk(&chunk(b"gAMA", &GAMA), BeforePalette, &mut Decompressor::new(), &mut metadata).unwrap();
    read_known_chunk(&chunk(b"sRGB", &[1]), BeforePalette, &mut Decompressor::new(), &mut metadata).unwrap();
    read_known_chunk(&chunk(b"pHYs", &PHYS), BeforeImageData, &mut Decompressor::new(), &mut metadata).unwrap();

    assert!(metadata.gamma().is_some());
    assert_eq!(metadata.srgb(), Some(RenderingIntent::RelativeColorimetric));
    assert!(metadata.physical_dimensions().is_some());
    assert_eq!(metadata.chromaticities(), None);
    assert_eq!(metadata.time(), None);
}

// ---------- read_ancillary ----------

#[test]
fn lenient_skips_bad_chunks() {
    let mut metadata = Metadata::default();

    for (chunk_type, data, position) in [
        (b"gAMA", &GAMA[..], AfterImageData),  // misplaced
        (b"gAMA", &[0; 4][..], BeforePalette), // invalid value
        (b"tIME", &[1][..], BeforePalette),    // invalid length
    ] {
        assert_eq!(read_ancillary(&chunk(chunk_type, data), position, false, &mut Decompressor::new(), &mut metadata), Ok(()));
    }
    assert!(metadata.is_empty());
}

#[test]
fn lenient_still_reads_good_chunks() {
    let mut metadata = Metadata::default();

    read_ancillary(&chunk(b"gAMA", &GAMA), BeforePalette, false, &mut Decompressor::new(), &mut metadata).unwrap();

    assert!(metadata.gamma().is_some());
}

#[test]
fn strict_returns_the_error() {
    let mut metadata = Metadata::default();

    assert_eq!(
        read_ancillary(&chunk(b"gAMA", &GAMA), AfterImageData, true, &mut Decompressor::new(), &mut metadata),
        Err(Error::MisplacedChunk(ChunkType::GAMA))
    );
    assert_eq!(
        read_ancillary(&chunk(b"gAMA", &[0; 4]), BeforePalette, true, &mut Decompressor::new(), &mut metadata),
        Err(Error::InvalidChunkData(ChunkType::GAMA))
    );
}

// ---------- iCCP, cICP and eXIf ----------

const CICP: [u8; 4] = [9, 16, 0, 1]; // BT.2100 PQ, full range
const EXIF: [u8; 8] = [b'M', b'M', 0, 42, 0, 0, 0, 8];

fn iccp() -> Vec<u8> {
    let mut data = b"ICC Profile\0\0".to_vec();
    data.extend_from_slice(&rust_deflate::compress_zlib(b"profile"));
    data
}

#[test]
fn reads_iccp_cicp_and_exif() {
    assert_eq!(read(b"iCCP", &iccp(), BeforePalette).unwrap().icc_profile().map(|p| p.profile.as_slice()), Some(&b"profile"[..]));
    assert_eq!(read(b"cICP", &CICP, BeforePalette).unwrap().cicp().map(|c| c.transfer_function), Some(16));
    assert_eq!(read(b"eXIf", &EXIF, BeforeImageData).unwrap().exif().map(|e| e.data()), Some(&EXIF[..]));
}

#[test]
fn iccp_and_cicp_must_come_before_the_palette() {
    for position in [BeforeImageData, AfterImageData] {
        assert_eq!(read(b"iCCP", &iccp(), position), Err(Error::MisplacedChunk(ChunkType::ICCP)), "{position:?}");
        assert_eq!(read(b"cICP", &CICP, position), Err(Error::MisplacedChunk(ChunkType::CICP)), "{position:?}");
    }
}

#[test]
fn exif_must_come_before_the_image_data() {
    assert!(read(b"eXIf", &EXIF, BeforePalette).is_ok());
    assert!(read(b"eXIf", &EXIF, BeforeImageData).is_ok());
    assert_eq!(read(b"eXIf", &EXIF, AfterImageData), Err(Error::MisplacedChunk(ChunkType::EXIF)));
}

#[test]
fn iccp_cicp_and_exif_may_appear_only_once() {
    let iccp = iccp();
    for (chunk_type, data) in [(b"iCCP", &iccp[..]), (b"cICP", &CICP), (b"eXIf", &EXIF)] {
        let mut metadata = Metadata::default();
        let mut decompressor = Decompressor::new();
        read_known_chunk(&chunk(chunk_type, data), BeforePalette, &mut decompressor, &mut metadata).unwrap();
        let first = metadata.clone();

        assert_eq!(
            read_known_chunk(&chunk(chunk_type, data), BeforePalette, &mut decompressor, &mut metadata),
            Err(Error::DuplicateChunk(ChunkType::from_bytes(*chunk_type).unwrap()))
        );
        assert_eq!(metadata, first);
    }
}
