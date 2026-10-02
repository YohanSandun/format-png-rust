//! Decodes the fixtures in tests/data, made by tests/data/generate.py.

use std::fs;
use std::path::{Path, PathBuf};

use format_png::png::ChunkType;
use format_png::png::metadata::{RenderingIntent, Time, Unit};
use format_png::{ChunkPosition, ColorType, DecodeOptions, Decoder, Error, ImageHeader};

fn data_dir(dir: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data").join(dir)
}

fn fixtures(dir: &str) -> Vec<PathBuf> {
    let mut paths: Vec<_> = fs::read_dir(data_dir(dir))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "png"))
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no fixtures in {dir}; run tests/data/generate.py");
    paths
}

fn decode(dir: &str, name: &str) -> Result<format_png::Image, Error> {
    format_png::decode(&fs::read(data_dir(dir).join(name)).unwrap())
}

/// `sample()` from generate.py.
fn sample(x: u32, y: u32, channel: u32, header: &ImageHeader) -> u64 {
    let range = 1u64 << header.bit_depth;
    if header.color_type == ColorType::Indexed {
        u64::from(x + 3 * y) % range
    } else {
        u64::from(x * 37 + y * 101 + channel * 53) % range
    }
}

/// The pixels generate.py wrote, packed the way `Image::data` stores them.
fn expected_pixels(header: &ImageHeader) -> Vec<u8> {
    let channels = match header.color_type {
        ColorType::Grayscale | ColorType::Indexed => 1,
        ColorType::GrayscaleAlpha => 2,
        ColorType::Rgb => 3,
        ColorType::Rgba => 4,
    };
    let depth = header.bit_depth as usize;
    let stride = (header.width as usize * channels * depth).div_ceil(8);
    let mut data = vec![0u8; stride * header.height as usize];

    for y in 0..header.height {
        for x in 0..header.width {
            for c in 0..channels {
                let value = sample(x, y, c as u32, header);
                let bit = y as usize * stride * 8 + (x as usize * channels + c) * depth;
                for i in 0..depth {
                    if (value >> (depth - 1 - i)) & 1 == 1 {
                        data[(bit + i) / 8] |= 0x80 >> ((bit + i) % 8);
                    }
                }
            }
        }
    }
    data
}

#[test]
fn every_valid_fixture_decodes_to_the_generated_pixels() {
    for path in fixtures("valid") {
        let name = path.file_name().unwrap().to_string_lossy();
        let image = format_png::decode(&fs::read(&path).unwrap())
            .unwrap_or_else(|error| panic!("{name}: {error}"));

        assert_eq!(image.data(), expected_pixels(image.header()), "{name}");
        assert_eq!(image.data().len(), image.stride() * image.height() as usize, "{name}");
    }
}

#[test]
fn interlaced_and_plain_fixtures_decode_the_same() {
    for path in fixtures("valid") {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let Some(plain) = name.strip_suffix("_adam7.png") else { continue };

        let interlaced = decode("valid", &name).unwrap();
        let plain = decode("valid", &format!("{plain}.png")).unwrap();
        assert_eq!(interlaced.data(), plain.data(), "{name}");
    }
}

#[test]
fn decoder_reused_across_all_fixtures_matches_fresh_decodes() {
    let mut decoder = format_png::Decoder::new();
    let mut pixels = Vec::new();

    for path in fixtures("valid") {
        let data = fs::read(&path).unwrap();
        decoder.decode_into(&data, &mut pixels).unwrap();

        assert_eq!(pixels, format_png::decode(&data).unwrap().into_data(), "{}", path.display());
    }
}

#[test]
fn invalid_image_data_fixtures_fail_with_the_right_error() {
    let cases: [(&str, fn(&Error) -> bool); 7] = [
        ("filter_type_5.png", |e| *e == Error::InvalidFilterType(5)),
        ("zlib_corrupt.png", |e| matches!(e, Error::Decompression(_))),
        ("idat_missing.png", |e| *e == Error::MissingImageData),
        ("iend_missing.png", |e| *e == Error::MissingImageEnd),
        ("idat_truncated.png", |e| *e == Error::UnexpectedEndOfInput),
        ("idat_crc_wrong.png", |e| matches!(e, Error::CrcMismatch { .. })),
        ("image_data_too_short.png", |e| *e == Error::ImageDataTooShort { expected: 371, actual: 351 }),
    ];

    for (name, is_expected) in cases {
        match decode("invalid", name) {
            Err(error) => assert!(is_expected(&error), "{name}: got {error:?}"),
            Ok(_) => panic!("{name}: decoded"),
        }
    }
}

#[test]
fn invalid_palette_fixtures_fail_with_the_right_error() {
    let cases = [
        ("indexed_plte_missing.png", Error::MissingPalette),
        ("plte_after_idat.png", Error::PaletteAfterImageData),
        ("plte_duplicate.png", Error::DuplicatePalette),
        ("plte_empty.png", Error::InvalidPaletteLength(0)),
        ("plte_length_4.png", Error::InvalidPaletteLength(4)),
        ("plte_257_entries.png", Error::InvalidPaletteLength(771)),
        ("plte_too_many_entries.png", Error::TooManyPaletteEntries { entries: 3, bit_depth: 1 }),
        ("plte_in_grayscale.png", Error::UnexpectedPalette(ColorType::Grayscale)),
        ("plte_after_trns.png", Error::TransparencyBeforePalette),
    ];

    for (name, expected) in cases {
        assert_eq!(decode("invalid", name), Err(expected), "{name}");
    }
}

#[test]
fn invalid_transparency_fixtures_fail_with_the_right_error() {
    let cases = [
        ("trns_after_idat.png", Error::TransparencyAfterImageData),
        ("trns_duplicate.png", Error::DuplicateTransparency),
        ("trns_before_plte.png", Error::TransparencyBeforePalette),
        ("trns_in_rgba.png", Error::UnexpectedTransparency(ColorType::Rgba)),
        ("trns_in_gray_alpha.png", Error::UnexpectedTransparency(ColorType::GrayscaleAlpha)),
        ("trns_gray_length_3.png", Error::InvalidTransparencyLength { color_type: ColorType::Grayscale, length: 3 }),
        ("trns_rgb_length_2.png", Error::InvalidTransparencyLength { color_type: ColorType::Rgb, length: 2 }),
        ("trns_indexed_empty.png", Error::InvalidTransparencyLength { color_type: ColorType::Indexed, length: 0 }),
        ("trns_too_many_entries.png", Error::TooManyTransparencyEntries { entries: 5, palette_entries: 4 }),
    ];

    for (name, expected) in cases {
        assert_eq!(decode("invalid", name), Err(expected), "{name}");
    }
}

#[test]
fn indexed_fixtures_have_the_generated_palette() {
    for path in fixtures("valid") {
        let image = format_png::decode(&fs::read(&path).unwrap()).unwrap();
        if image.header().color_type != ColorType::Indexed {
            continue;
        }

        // palette() from generate.py: one color per possible index.
        let expected: Vec<[u8; 3]> = (0..1u32 << image.header().bit_depth)
            .map(|i| [(i * 67 % 256) as u8, (i * 131 % 256) as u8, (i * 199 % 256) as u8])
            .collect();
        assert_eq!(image.palette().unwrap().colors(), expected, "{}", path.display());
    }
}

#[test]
fn every_invalid_fixture_fails() {
    // Decode by default: the first fails only when converted to colors
    // (tests/bitmap.rs), the second only in strict mode (below).
    let fails_only_when_converted = ["indexed_index_out_of_range.png", "gama_after_plte.png"];

    for path in fixtures("invalid") {
        let name = path.file_name().unwrap().to_string_lossy();
        if fails_only_when_converted.contains(&name.as_ref()) {
            continue;
        }

        assert!(format_png::decode(&fs::read(&path).unwrap()).is_err(), "{name} decoded");
    }
}

// ---------- metadata ----------

fn read(dir: &str, name: &str) -> Vec<u8> {
    fs::read(data_dir(dir).join(name)).unwrap()
}

fn keep_everything() -> DecodeOptions {
    DecodeOptions { preserve_chunks: true, preserve_metadata: true, ..DecodeOptions::default() }
}

#[test]
fn metadata_fixture_has_every_known_chunk() {
    let image = Decoder::with_options(keep_everything()).decode(&read("valid", "metadata.png")).unwrap();
    let metadata = image.metadata();

    assert_eq!(metadata.srgb(), Some(RenderingIntent::Perceptual));
    assert_eq!(metadata.gamma().map(|g| g.scaled()), Some(45455));
    assert_eq!(metadata.chromaticities().map(|c| (c.white_x, c.blue_y)), Some((31270, 6000)));
    let physical = metadata.physical_dimensions().unwrap();
    assert_eq!((physical.x, physical.y, physical.unit), (3780, 3780, Unit::Meter));
    assert_eq!(metadata.time(), Some(Time { year: 2026, month: 9, day: 30, hour: 12, minute: 34, second: 56 }));
}

#[test]
fn ancillary_fixture_keeps_every_chunk_in_order() {
    let image = Decoder::with_options(keep_everything()).decode(&read("valid", "ancillary_chunks.png")).unwrap();

    let types: Vec<_> = image.ancillary_chunks().iter().map(|c| c.chunk_type().to_string()).collect();
    assert_eq!(types, ["gAMA", "pHYs", "tEXt", "zTXt", "ruSt"]);
    assert!(image.ancillary_chunks().iter().all(|c| c.position() == ChunkPosition::BeforePalette));
    // Only the chunks Metadata knows are parsed.
    assert!(image.metadata().gamma().is_some());
    assert!(image.metadata().physical_dimensions().is_some());
}

#[test]
fn chunk_after_the_image_data_is_kept() {
    let image = Decoder::with_options(keep_everything()).decode(&read("valid", "ancillary_after_idat.png")).unwrap();

    let text = image.ancillary_chunks().last().unwrap();
    assert_eq!(text.chunk_type().to_string(), "tEXt");
    assert_eq!(text.data(), b"Author\0format-png");
    assert_eq!(text.position(), ChunkPosition::AfterImageData);
}

#[test]
fn misplaced_ancillary_fixture_decodes_unless_strict() {
    let data = read("invalid", "gama_after_plte.png");

    let image = Decoder::with_options(keep_everything()).decode(&data).unwrap();
    assert_eq!(image.metadata().gamma(), None);

    let strict = DecodeOptions { strict_ancillary: true, ..keep_everything() };
    assert_eq!(Decoder::with_options(strict).decode(&data), Err(Error::MisplacedChunk(ChunkType::GAMA)));
}

#[test]
fn every_valid_fixture_decodes_with_everything_kept() {
    let mut decoder = Decoder::with_options(DecodeOptions { strict_ancillary: true, ..keep_everything() });

    for path in fixtures("valid") {
        let image = decoder.decode(&fs::read(&path).unwrap()).unwrap_or_else(|e| panic!("{}: {e}", path.display()));

        assert_eq!(image.data(), expected_pixels(image.header()), "{}", path.display());
    }
}

#[test]
fn every_valid_fixture_reads_chunks_like_it_decodes() {
    let options = DecodeOptions { strict_ancillary: true, ..keep_everything() };

    for path in fixtures("valid") {
        let data = fs::read(&path).unwrap();
        let png = Decoder::with_options(options.clone()).read_chunks(&data).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let image = Decoder::with_options(options.clone()).decode(&data).unwrap();

        assert_eq!(png.header(), image.header(), "{}", path.display());
        assert_eq!(png.palette(), image.palette(), "{}", path.display());
        assert_eq!(png.transparency(), image.transparency(), "{}", path.display());
        assert_eq!(png.metadata(), image.metadata(), "{}", path.display());
    }
}

#[test]
fn ancillary_fixture_reads_text_and_unknown_chunks() {
    let data = read("valid", "ancillary_chunks.png");
    let png = format_png::read_chunks(&data).unwrap();

    let text: Vec<_> = png.metadata().text().iter().map(|t| (t.keyword.as_str(), t.text.as_str())).collect();
    assert_eq!(text, [("Title", "format-png test image"), ("Comment", "compressed text")]);
    let unknown: Vec<_> = png.unknown_chunks().map(|c| c.chunk_type().to_string()).collect();
    assert_eq!(unknown, ["ruSt"]);
}
