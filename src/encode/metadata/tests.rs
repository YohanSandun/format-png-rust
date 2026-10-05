#![cfg(test)]

use rust_deflate::{CompressionOptions, Compressor};

use super::{
    chromaticities_data, cicp_data, gamma_data, icc_profile_data, physical_dimensions_data,
    rendering_intent_data, text_data, time_data, validate_metadata, write_metadata_after_palette,
    write_metadata_before_palette,
};
use crate::ChunkReader;
use crate::error::Error;
use crate::png::metadata::{
    Chromaticities, CodingIndependentCodePoints, Exif, Gamma, IccProfile, PhysicalDimensions,
    RenderingIntent, Text, TextKind, Time, Unit,
};
use crate::png::{ChunkType, Metadata, SIGNATURE};

const SRGB_CHROMATICITIES: Chromaticities = Chromaticities {
    white_x: 31270,
    white_y: 32900,
    red_x: 64000,
    red_y: 33000,
    green_x: 30000,
    green_y: 60000,
    blue_x: 15000,
    blue_y: 6000,
};
const EXIF: [u8; 8] = [b'M', b'M', 0, 42, 0, 0, 0, 8];

fn text(keyword: &str, value: &str, kind: TextKind) -> Text {
    Text {
        keyword: keyword.to_string(),
        text: value.to_string(),
        language_tag: String::new(),
        translated_keyword: String::new(),
        kind,
    }
}

fn international(
    keyword: &str,
    language_tag: &str,
    translated_keyword: &str,
    value: &str,
    compressed: bool,
) -> Text {
    Text {
        keyword: keyword.to_string(),
        text: value.to_string(),
        language_tag: language_tag.to_string(),
        translated_keyword: translated_keyword.to_string(),
        kind: TextKind::International { compressed },
    }
}

/// Metadata with every chunk type, and one text chunk of each kind.
fn full_metadata() -> Metadata {
    Metadata::default()
        .with_cicp(CodingIndependentCodePoints {
            color_primaries: 1,
            transfer_function: 13,
            matrix_coefficients: 0,
            full_range: true,
        })
        .with_icc_profile(IccProfile {
            name: "ICC Profile".to_string(),
            profile: b"not really a profile".to_vec(),
        })
        .with_srgb(RenderingIntent::Perceptual)
        .with_gamma(Gamma::parse(&45455u32.to_be_bytes()).unwrap())
        .with_chromaticities(SRGB_CHROMATICITIES)
        .with_physical_dimensions(PhysicalDimensions {
            x: 2835,
            y: 2835,
            unit: Unit::Meter,
        })
        .with_exif(Exif::parse(&EXIF).unwrap())
        .with_time(Time {
            year: 2026,
            month: 9,
            day: 30,
            hour: 12,
            minute: 34,
            second: 56,
        })
        .with_text(text("Title", "format-png test image", TextKind::Plain))
        .with_text(text("Comment", "compressed text", TextKind::Compressed))
        .with_text(international("Title", "nb-NO", "Tittel", "blåbær", true))
}

fn options() -> CompressionOptions {
    CompressionOptions::new()
}

/// The chunks in `out`, as (type, data), with CRCs checked.
fn chunks(out: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut png = SIGNATURE.to_vec();
    png.extend_from_slice(out);
    let mut reader = ChunkReader::new(&png).unwrap();
    let mut chunks = Vec::new();
    while let Some(chunk) = reader.next_chunk().unwrap() {
        chunks.push((chunk.chunk_type().to_string(), chunk.data().to_vec()));
    }
    chunks
}

fn types(chunks: &[(String, Vec<u8>)]) -> Vec<&str> {
    chunks
        .iter()
        .map(|(chunk_type, _)| chunk_type.as_str())
        .collect()
}

// ---------- fixed-size chunks ----------

#[test]
fn gamma_is_4_bytes_big_endian() {
    let gamma = Gamma::parse(&45455u32.to_be_bytes()).unwrap();

    assert_eq!(gamma_data(&gamma), [0, 0, 0xB1, 0x8F]);
}

#[test]
fn chromaticities_round_trip() {
    assert_eq!(
        Chromaticities::parse(&chromaticities_data(&SRGB_CHROMATICITIES)),
        Ok(SRGB_CHROMATICITIES)
    );
    assert_eq!(
        chromaticities_data(&SRGB_CHROMATICITIES)[..4],
        31270u32.to_be_bytes()
    ); // white x first
}

#[test]
fn every_rendering_intent_round_trips() {
    for (intent, byte) in [
        (RenderingIntent::Perceptual, 0),
        (RenderingIntent::RelativeColorimetric, 1),
        (RenderingIntent::Saturation, 2),
        (RenderingIntent::AbsoluteColorimetric, 3),
    ] {
        assert_eq!(rendering_intent_data(intent), [byte]);
        assert_eq!(
            RenderingIntent::parse(&rendering_intent_data(intent)),
            Ok(intent)
        );
    }
}

#[test]
fn physical_dimensions_have_the_spec_layout() {
    let meters = PhysicalDimensions {
        x: 2835,
        y: 2835,
        unit: Unit::Meter,
    };
    let aspect = PhysicalDimensions {
        x: 1,
        y: 2,
        unit: Unit::Unknown,
    };

    assert_eq!(
        physical_dimensions_data(&meters),
        [0, 0, 0x0B, 0x13, 0, 0, 0x0B, 0x13, 1]
    );
    assert_eq!(
        PhysicalDimensions::parse(&physical_dimensions_data(&aspect)),
        Ok(aspect)
    );
}

#[test]
fn time_has_the_spec_layout() {
    let time = Time {
        year: 2026,
        month: 9,
        day: 30,
        hour: 12,
        minute: 34,
        second: 56,
    };

    assert_eq!(time_data(&time), [0x07, 0xEA, 9, 30, 12, 34, 56]);
    assert_eq!(Time::parse(&time_data(&time)), Ok(time));
}

#[test]
fn cicp_writes_the_full_range_flag_as_a_byte() {
    let full = CodingIndependentCodePoints {
        color_primaries: 9,
        transfer_function: 16,
        matrix_coefficients: 0,
        full_range: true,
    };
    let narrow = CodingIndependentCodePoints {
        full_range: false,
        ..full
    };

    assert_eq!(cicp_data(&full), [9, 16, 0, 1]);
    assert_eq!(cicp_data(&narrow), [9, 16, 0, 0]);
    assert_eq!(
        CodingIndependentCodePoints::parse(&cicp_data(&narrow)),
        Ok(narrow)
    );
}

// ---------- compressed and text chunks ----------

#[test]
fn icc_profile_round_trips() {
    let icc_profile = IccProfile {
        name: "Caf\u{e9} profile".to_string(),
        profile: vec![7; 1000],
    };

    let data = icc_profile_data(&icc_profile, &mut Compressor::new(), options());

    assert!(data.starts_with(b"Caf\xE9 profile\0\0"));
    assert_eq!(IccProfile::parse(&data), Ok(icc_profile));
}

#[test]
fn plain_text_is_latin_1() {
    let (chunk_type, data) = text_data(
        &text("Title", "Caf\u{e9}", TextKind::Plain),
        &mut Compressor::new(),
        options(),
    );

    assert_eq!(chunk_type, ChunkType::TEXT);
    assert_eq!(data, b"Title\0Caf\xE9");
}

#[test]
fn compressed_text_round_trips() {
    let text = text(
        "Comment",
        &"compressed text, ".repeat(20),
        TextKind::Compressed,
    );

    let (chunk_type, data) = text_data(&text, &mut Compressor::new(), options());

    assert_eq!(chunk_type, ChunkType::ZTXT);
    assert!(data.starts_with(b"Comment\0\0"));
    assert!(data.len() < text.text.len());
    assert_eq!(Text::parse_compressed(&data), Ok(text));
}

#[test]
fn uncompressed_international_text_has_the_spec_layout() {
    let text = international("Title", "nb-NO", "Tittel", "blåbær", false);

    let (chunk_type, data) = text_data(&text, &mut Compressor::new(), options());

    let mut expected = b"Title\0\0\0nb-NO\0Tittel\0".to_vec();
    expected.extend_from_slice("blåbær".as_bytes());
    assert_eq!(chunk_type, ChunkType::ITXT);
    assert_eq!(data, expected);
    assert_eq!(Text::parse_international(&data), Ok(text));
}

#[test]
fn compressed_international_text_round_trips() {
    let text = international("Title", "", "", &"日本語のテキスト".repeat(10), true);

    let (_, data) = text_data(&text, &mut Compressor::new(), options());

    assert!(data.starts_with(b"Title\0\x01\0"));
    assert_eq!(Text::parse_international(&data), Ok(text));
}

// ---------- validate_metadata ----------

#[test]
fn full_metadata_is_valid() {
    assert_eq!(validate_metadata(&full_metadata()), Ok(()));
    assert_eq!(validate_metadata(&Metadata::default()), Ok(()));
}

#[test]
fn keywords_follow_the_spec_rules() {
    let long = "a".repeat(80);
    let invalid = [
        "",
        long.as_str(),
        " Leading",
        "Trailing ",
        "Two  spaces",
        "Tab\there",
        "\u{101}",
        "No\u{a0}break",
    ];

    for keyword in invalid {
        let metadata = Metadata::default().with_text(text(keyword, "x", TextKind::Plain));
        assert_eq!(
            validate_metadata(&metadata),
            Err(Error::InvalidChunkData(ChunkType::TEXT)),
            "{keyword:?}"
        );

        let metadata = Metadata::default().with_icc_profile(IccProfile {
            name: keyword.to_string(),
            profile: vec![1],
        });
        assert_eq!(
            validate_metadata(&metadata),
            Err(Error::InvalidChunkData(ChunkType::ICCP)),
            "{keyword:?}"
        );
    }

    // 79 characters, Latin-1 above 127 and single inner spaces are fine.
    for keyword in [
        "a".repeat(79),
        "\u{e9}".repeat(79),
        "Creation Time".to_string(),
    ] {
        let metadata = Metadata::default().with_text(text(&keyword, "x", TextKind::Plain));
        assert_eq!(validate_metadata(&metadata), Ok(()), "{keyword:?}");
    }
}

#[test]
fn latin_1_chunks_reject_other_characters() {
    for (kind, chunk_type) in [
        (TextKind::Plain, ChunkType::TEXT),
        (TextKind::Compressed, ChunkType::ZTXT),
    ] {
        let metadata = Metadata::default().with_text(text("Title", "blåbær \u{101}", kind));

        assert_eq!(
            validate_metadata(&metadata),
            Err(Error::InvalidChunkData(chunk_type))
        );
    }

    // iTXt is UTF-8, so anything goes.
    let metadata =
        Metadata::default().with_text(international("Title", "", "", "\u{101} 日本語", false));
    assert_eq!(validate_metadata(&metadata), Ok(()));
}

#[test]
fn null_characters_are_rejected() {
    let cases = [
        (text("Title", "a\0b", TextKind::Plain), ChunkType::TEXT),
        (text("Title", "a\0b", TextKind::Compressed), ChunkType::ZTXT),
        (
            international("Title", "", "", "a\0b", false),
            ChunkType::ITXT,
        ),
        (
            international("Title", "", "Tit\0tel", "x", false),
            ChunkType::ITXT,
        ),
    ];

    for (text, chunk_type) in cases {
        assert_eq!(
            validate_metadata(&Metadata::default().with_text(text.clone())),
            Err(Error::InvalidChunkData(chunk_type)),
            "{text:?}"
        );
    }
}

#[test]
fn language_tags_are_letters_digits_and_hyphens() {
    for tag in ["", "en", "nb-NO", "x-klingon", "zh-Hant-TW"] {
        let metadata = Metadata::default().with_text(international("Title", tag, "", "x", false));
        assert_eq!(validate_metadata(&metadata), Ok(()), "{tag:?}");
    }
    for tag in ["en us", "en_US", "nb-NØ", "a\0b"] {
        let metadata = Metadata::default().with_text(international("Title", tag, "", "x", false));
        assert_eq!(
            validate_metadata(&metadata),
            Err(Error::InvalidChunkData(ChunkType::ITXT)),
            "{tag:?}"
        );
    }
}

#[test]
fn time_fields_must_be_in_range() {
    let valid = Time {
        year: 2026,
        month: 12,
        day: 31,
        hour: 23,
        minute: 59,
        second: 60,
    };
    let invalid = [
        Time { month: 13, ..valid },
        Time { day: 0, ..valid },
        Time { hour: 24, ..valid },
        Time {
            second: 61,
            ..valid
        },
    ];

    assert_eq!(
        validate_metadata(&Metadata::default().with_time(valid)),
        Ok(())
    );
    for time in invalid {
        assert_eq!(
            validate_metadata(&Metadata::default().with_time(time)),
            Err(Error::InvalidChunkData(ChunkType::TIME)),
            "{time:?}"
        );
    }
}

// ---------- writing ----------

#[test]
fn chunks_before_the_palette_are_the_color_chunks_in_order() {
    let mut out = Vec::new();

    write_metadata_before_palette(
        &mut out,
        &full_metadata(),
        &mut Compressor::new(),
        options(),
    );

    assert_eq!(
        types(&chunks(&out)),
        ["cICP", "iCCP", "sRGB", "gAMA", "cHRM"]
    );
}

#[test]
fn chunks_after_the_palette_end_with_text_in_order() {
    let mut out = Vec::new();

    write_metadata_after_palette(
        &mut out,
        &full_metadata(),
        &mut Compressor::new(),
        options(),
    );

    let chunks = chunks(&out);
    assert_eq!(
        types(&chunks),
        ["pHYs", "eXIf", "tIME", "tEXt", "zTXt", "iTXt"]
    );
    assert_eq!(chunks[1].1, EXIF);
}

#[test]
fn missing_chunks_are_not_written() {
    let mut before = Vec::new();
    let mut after = Vec::new();
    let only_time = Metadata::default().with_time(Time {
        year: 2026,
        month: 1,
        day: 1,
        hour: 0,
        minute: 0,
        second: 0,
    });

    write_metadata_before_palette(&mut before, &only_time, &mut Compressor::new(), options());
    write_metadata_after_palette(&mut after, &only_time, &mut Compressor::new(), options());

    assert!(before.is_empty());
    assert_eq!(types(&chunks(&after)), ["tIME"]);
}
