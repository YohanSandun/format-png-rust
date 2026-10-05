//! Converts the fixtures in tests/data to RGB and RGBA bitmaps.

use std::fs;
use std::path::{Path, PathBuf};

use format_png::{ColorType, Error, ImageHeader, PixelFormat};

fn fixtures() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/valid");
    let mut paths: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "png"))
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no fixtures; run tests/data/generate.py");
    paths
}

fn name(path: &Path) -> String {
    path.file_name().unwrap().to_string_lossy().into_owned()
}

/// `sample()` from generate.py, for non-indexed images.
fn sample(x: u32, y: u32, channel: u32, bit_depth: u8) -> u32 {
    (x * 37 + y * 101 + channel * 53) % (1 << bit_depth)
}

/// The color generate.py's `palette()` gives pixel (x, y) of an indexed image.
fn palette_color(x: u32, y: u32, bit_depth: u8) -> [u8; 3] {
    let i = (x + 3 * y) % (1 << bit_depth);
    [
        (i * 67 % 256) as u8,
        (i * 131 % 256) as u8,
        (i * 199 % 256) as u8,
    ]
}

/// The conversion rules from `Image::to_bitmap`, for one sample.
fn to_8_bits(value: u32, bit_depth: u8) -> u8 {
    match bit_depth {
        16 => (value >> 8) as u8,
        8 => value as u8,
        _ => (value * 255 / ((1 << bit_depth) - 1)) as u8,
    }
}

/// The `tRNS` chunk generate.py gives a fixture, written out here so the
/// expected pixels don't depend on the code that parses it.
enum Trns {
    Gray(u32),
    Rgb([u32; 3]),
    Alpha(Vec<u8>),
}

fn trns_of(path: &Path) -> Option<Trns> {
    match name(path).as_str() {
        "gray_8_trns.png" => Some(Trns::Gray(37)),
        "gray_2_trns.png" => Some(Trns::Gray(1)),
        "rgb_8_trns.png" | "rgb_16_trns.png" => Some(Trns::Rgb([0, 53, 106])),
        "indexed_8_trns.png" => Some(Trns::Alpha((0..=255).step_by(2).collect())),
        "indexed_2_trns.png" => Some(Trns::Alpha(vec![0, 85, 170])),
        other => {
            assert!(
                !other.contains("_trns"),
                "{other}: add its tRNS chunk to trns_of"
            );
            None
        }
    }
}

/// The RGBA value generate.py's pixel (x, y) should convert to.
fn expected_rgba(header: &ImageHeader, trns: Option<&Trns>, x: u32, y: u32) -> [u8; 4] {
    let depth = header.bit_depth;
    let raw = |channel| sample(x, y, channel, depth);
    let s = |channel| to_8_bits(raw(channel), depth);
    // tRNS colors are compared at the image's own bit depth, before scaling.
    let key_alpha = |matches: bool| if matches { 0 } else { 255 };

    match (header.color_type, trns) {
        (ColorType::Grayscale, None) => [s(0), s(0), s(0), 255],
        (ColorType::Grayscale, Some(Trns::Gray(key))) => {
            [s(0), s(0), s(0), key_alpha(raw(0) == *key)]
        }
        (ColorType::GrayscaleAlpha, None) => [s(0), s(0), s(0), s(1)],
        (ColorType::Rgb, None) => [s(0), s(1), s(2), 255],
        (ColorType::Rgb, Some(Trns::Rgb(key))) => [
            s(0),
            s(1),
            s(2),
            key_alpha([raw(0), raw(1), raw(2)] == *key),
        ],
        (ColorType::Rgba, None) => [s(0), s(1), s(2), s(3)],
        (ColorType::Indexed, trns) => {
            let [r, g, b] = palette_color(x, y, depth);
            let index = ((x + 3 * y) % (1 << depth)) as usize;
            let alpha = match trns {
                Some(Trns::Alpha(alpha)) => alpha.get(index).copied().unwrap_or(255),
                _ => 255,
            };
            [r, g, b, alpha]
        }
        (color_type, _) => panic!("unexpected tRNS for {color_type:?}"),
    }
}

fn expected(path: &Path, header: &ImageHeader, format: PixelFormat) -> Vec<u8> {
    let trns = trns_of(path);
    let mut out = Vec::new();
    for y in 0..header.height {
        for x in 0..header.width {
            let rgba = expected_rgba(header, trns.as_ref(), x, y);
            match format {
                PixelFormat::Rgba8 => out.extend_from_slice(&rgba),
                PixelFormat::Rgb8 => out.extend_from_slice(&rgba[..3]),
                _ => unreachable!(),
            }
        }
    }
    out
}

fn convertible_fixtures() -> impl Iterator<Item = (PathBuf, Vec<u8>, ImageHeader)> {
    fixtures().into_iter().map(|path| {
        let data = fs::read(&path).unwrap();
        let header = format_png::read_header(&data).unwrap();
        (path, data, header)
    })
}

#[test]
fn every_fixture_converts_to_rgba8() {
    for (path, data, header) in convertible_fixtures() {
        let bitmap =
            format_png::decode_rgba8(&data).unwrap_or_else(|e| panic!("{}: {e}", name(&path)));

        assert_eq!(bitmap.format(), PixelFormat::Rgba8);
        assert_eq!(
            (bitmap.width(), bitmap.height()),
            (header.width, header.height)
        );
        assert_eq!(
            bitmap.data(),
            expected(&path, &header, PixelFormat::Rgba8),
            "{}",
            name(&path)
        );
    }
}

#[test]
fn every_fixture_converts_to_rgb8() {
    for (path, data, header) in convertible_fixtures() {
        let bitmap =
            format_png::decode_rgb8(&data).unwrap_or_else(|e| panic!("{}: {e}", name(&path)));

        assert_eq!(
            bitmap.data(),
            expected(&path, &header, PixelFormat::Rgb8),
            "{}",
            name(&path)
        );
    }
}

#[test]
fn rgba8_bitmaps_fit_canvas_image_data() {
    // ImageData needs exactly width * height * 4 bytes, with no row padding.
    for (path, data, header) in convertible_fixtures() {
        let bitmap = format_png::decode_rgba8(&data).unwrap();

        assert_eq!(
            bitmap.stride(),
            header.width as usize * 4,
            "{}",
            name(&path)
        );
        assert_eq!(
            bitmap.data().len(),
            bitmap.stride() * header.height as usize,
            "{}",
            name(&path)
        );
    }
}

#[test]
fn reused_decoder_matches_the_simple_api() {
    let mut decoder = format_png::Decoder::new();
    let mut out = Vec::new();

    for (path, data, _) in convertible_fixtures() {
        decoder
            .decode_bitmap_into(&data, PixelFormat::Rgba8, &mut out)
            .unwrap();

        assert_eq!(
            out,
            format_png::decode_rgba8(&data).unwrap().into_data(),
            "{}",
            name(&path)
        );
    }
}

#[test]
fn indexed_fixtures_are_converted() {
    // Guards against the loops above quietly skipping every indexed fixture.
    let indexed = convertible_fixtures()
        .filter(|(_, _, header)| header.color_type == ColorType::Indexed)
        .count();

    assert_eq!(indexed, 10);
}

#[test]
fn trns_fixtures_have_transparent_pixels() {
    // Guards against the expected pixels quietly ignoring tRNS: each fixture
    // has at least one pixel with alpha 0.
    let mut trns = 0;
    for (path, data, _) in convertible_fixtures().filter(|(path, _, _)| trns_of(path).is_some()) {
        let bitmap = format_png::decode_rgba8(&data).unwrap();
        trns += 1;

        assert!(
            bitmap
                .data()
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| pixel[3] == 0),
            "{}",
            name(&path)
        );
    }
    assert_eq!(trns, 6);
}

#[test]
fn index_past_the_palette_fails_to_convert() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/invalid/indexed_index_out_of_range.png");
    let data = fs::read(path).unwrap();

    // The indices decode; only converting them to colors fails.
    assert!(format_png::decode(&data).is_ok());
    assert_eq!(
        format_png::decode_rgba8(&data),
        Err(Error::PaletteIndexOutOfRange {
            index: 16,
            entries: 16
        })
    );
}
