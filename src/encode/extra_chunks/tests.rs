#![cfg(test)]

use super::{encoder_writes, should_write, validate_extra_chunks, write_extra_chunks};
use crate::ChunkReader;
use crate::encode::image_ref::ImageRef;
use crate::error::Error;
use crate::png::metadata::{
    Chromaticities, CodingIndependentCodePoints, Exif, Gamma, IccProfile, PhysicalDimensions,
    RenderingIntent, Text, TextKind, Time, Unit,
};
use crate::png::{
    ChunkPosition, ChunkType, ColorType, ImageHeader, Interlace, Metadata, OwnedChunk, SIGNATURE,
    Transparency,
};

use ChunkPosition::{AfterImageData, BeforeImageData, BeforePalette};

fn chunk_type(bytes: &[u8; 4]) -> ChunkType {
    ChunkType::from_bytes(*bytes).unwrap()
}

fn chunk(bytes: &[u8; 4], data: &[u8], position: ChunkPosition) -> OwnedChunk {
    OwnedChunk::from_data(chunk_type(bytes), data.to_vec(), position)
}

const HEADER: ImageHeader = ImageHeader {
    width: 1,
    height: 1,
    bit_depth: 8,
    color_type: ColorType::Rgb,
    interlace: Interlace::None,
};

fn image(chunks: &[OwnedChunk]) -> ImageRef<'_> {
    ImageRef::new(HEADER, &[0, 0, 0]).with_chunks(chunks)
}

fn plain_text() -> Text {
    Text {
        keyword: "Title".to_string(),
        text: "x".to_string(),
        language_tag: String::new(),
        translated_keyword: String::new(),
        kind: TextKind::Plain,
    }
}

/// The (type, data) of the chunks in `out`, with CRCs checked.
fn written(out: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut png = SIGNATURE.to_vec();
    png.extend_from_slice(out);
    let mut reader = ChunkReader::new(&png).unwrap();
    let mut chunks = Vec::new();
    while let Some(chunk) = reader.next_chunk().unwrap() {
        chunks.push((chunk.chunk_type().to_string(), chunk.data().to_vec()));
    }
    chunks
}

// ---------- validate_extra_chunks ----------

#[test]
fn ancillary_chunks_are_valid() {
    assert_eq!(validate_extra_chunks(&[]), Ok(()));
    assert_eq!(
        validate_extra_chunks(&[
            chunk(b"ruSt", b"x", BeforePalette),
            chunk(b"bKGD", &[0, 0], BeforeImageData)
        ]),
        Ok(())
    );
}

#[test]
fn critical_chunks_are_rejected() {
    for bytes in [b"IHDR", b"PLTE", b"IDAT", b"IEND", b"CuSt"] {
        let chunks = [
            chunk(b"ruSt", b"", BeforePalette),
            chunk(bytes, b"", BeforePalette),
        ];

        assert_eq!(
            validate_extra_chunks(&chunks),
            Err(Error::UnexpectedCriticalChunk(chunk_type(bytes))),
            "{bytes:?}"
        );
    }
}

// ---------- encoder_writes ----------

#[test]
fn nothing_is_written_by_the_encoder_for_a_bare_image() {
    let image = image(&[]);

    for bytes in [b"tRNS", b"gAMA", b"tEXt", b"iCCP", b"ruSt"] {
        assert!(!encoder_writes(chunk_type(bytes), &image), "{bytes:?}");
    }
}

#[test]
fn transparency_means_the_encoder_writes_trns() {
    let transparency = Transparency::Rgb([0, 0, 0]);

    assert!(encoder_writes(
        ChunkType::TRNS,
        &image(&[]).with_transparency(&transparency)
    ));
}

#[test]
fn each_metadata_value_means_the_encoder_writes_its_type() {
    let metadata = Metadata::default()
        .with_gamma(Gamma::new(45455).unwrap())
        .with_chromaticities(Chromaticities {
            white_x: 1,
            white_y: 2,
            red_x: 3,
            red_y: 4,
            green_x: 5,
            green_y: 6,
            blue_x: 7,
            blue_y: 8,
        })
        .with_srgb(RenderingIntent::Perceptual)
        .with_physical_dimensions(PhysicalDimensions {
            x: 1,
            y: 1,
            unit: Unit::Unknown,
        })
        .with_time(Time {
            year: 2026,
            month: 1,
            day: 1,
            hour: 0,
            minute: 0,
            second: 0,
        })
        .with_icc_profile(IccProfile {
            name: "ICC".to_string(),
            profile: vec![1],
        })
        .with_cicp(CodingIndependentCodePoints {
            color_primaries: 1,
            transfer_function: 13,
            matrix_coefficients: 0,
            full_range: true,
        })
        .with_exif(Exif::parse(&[b'M', b'M', 0, 42, 0, 0, 0, 8]).unwrap());
    let image = image(&[]).with_metadata(&metadata);

    for bytes in [
        b"gAMA", b"cHRM", b"sRGB", b"pHYs", b"tIME", b"iCCP", b"cICP", b"eXIf",
    ] {
        assert!(encoder_writes(chunk_type(bytes), &image), "{bytes:?}");
    }
    // No text, so raw text chunks are still written.
    assert!(!encoder_writes(ChunkType::TEXT, &image));
}

#[test]
fn any_text_means_the_encoder_writes_every_text_type() {
    let metadata = Metadata::default().with_text(plain_text());
    let image = image(&[]).with_metadata(&metadata);

    for bytes in [b"tEXt", b"zTXt", b"iTXt"] {
        assert!(encoder_writes(chunk_type(bytes), &image), "{bytes:?}");
    }
    assert!(!encoder_writes(ChunkType::GAMA, &image));
}

// ---------- should_write ----------

#[test]
fn safe_to_copy_chunks_are_written() {
    let image = image(&[]);

    assert!(should_write(
        &chunk(b"ruSt", b"", BeforePalette),
        &image,
        false
    ));
    assert!(should_write(
        &chunk(b"tEXt", b"Title\0x", BeforePalette),
        &image,
        false
    ));
}

#[test]
fn unsafe_to_copy_chunks_need_keep_unsafe() {
    let image = image(&[]);

    for bytes in [b"ruST", b"bKGD", b"gAMA"] {
        let chunk = chunk(bytes, b"", BeforePalette);
        assert!(!should_write(&chunk, &image, false), "{bytes:?}");
        assert!(should_write(&chunk, &image, true), "{bytes:?}");
    }
}

#[test]
fn chunks_the_encoder_writes_are_skipped_even_with_keep_unsafe() {
    let metadata = Metadata::default()
        .with_gamma(Gamma::new(45455).unwrap())
        .with_text(plain_text());
    let image = image(&[]).with_metadata(&metadata);

    for bytes in [b"gAMA", b"tEXt", b"zTXt"] {
        assert!(
            !should_write(&chunk(bytes, b"", BeforePalette), &image, true),
            "{bytes:?}"
        );
    }
    // Other types still go through.
    assert!(should_write(
        &chunk(b"pHYs", &[0; 9], BeforeImageData),
        &image,
        false
    ));
}

// ---------- write_extra_chunks ----------

#[test]
fn writes_only_the_chunks_at_the_position_in_order() {
    let chunks = [
        chunk(b"onEa", b"1", BeforePalette),
        chunk(b"twOa", b"2", BeforeImageData),
        chunk(b"thRa", b"3", BeforePalette),
        chunk(b"foUa", b"4", AfterImageData),
    ];
    let image = image(&chunks);

    for (position, expected) in [
        (BeforePalette, vec![("onEa", b"1"), ("thRa", b"3")]),
        (BeforeImageData, vec![("twOa", b"2")]),
        (AfterImageData, vec![("foUa", b"4")]),
    ] {
        let mut out = Vec::new();
        write_extra_chunks(&mut out, &image, position, false);

        let expected: Vec<_> = expected
            .into_iter()
            .map(|(t, d)| (t.to_string(), d.to_vec()))
            .collect();
        assert_eq!(written(&out), expected, "{position:?}");
    }
}

#[test]
fn write_extra_chunks_skips_what_should_write_rejects() {
    let chunks = [
        chunk(b"ruST", b"unsafe", BeforePalette),
        chunk(b"ruSt", b"safe", BeforePalette),
    ];
    let mut out = Vec::new();

    write_extra_chunks(&mut out, &image(&chunks), BeforePalette, false);

    assert_eq!(written(&out), [("ruSt".to_string(), b"safe".to_vec())]);
}
