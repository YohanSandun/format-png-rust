//! Converts the fixtures in tests/data to RGB and RGBA bitmaps.
//!
//! `PLTE` and `tRNS` aren't read yet: indexed fixtures must fail with
//! `MissingPalette`, and `*_trns` fixtures are skipped so these tests don't pin
//! down transparency behavior that will change.

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

/// The conversion rules from `Image::to_bitmap`, for one sample.
fn to_8_bits(value: u32, bit_depth: u8) -> u8 {
    match bit_depth {
        16 => (value >> 8) as u8,
        8 => value as u8,
        _ => (value * 255 / ((1 << bit_depth) - 1)) as u8,
    }
}

/// The RGBA value generate.py's pixel (x, y) should convert to.
fn expected_rgba(header: &ImageHeader, x: u32, y: u32) -> [u8; 4] {
    let s = |channel| to_8_bits(sample(x, y, channel, header.bit_depth), header.bit_depth);
    match header.color_type {
        ColorType::Grayscale => [s(0), s(0), s(0), 255],
        ColorType::GrayscaleAlpha => [s(0), s(0), s(0), s(1)],
        ColorType::Rgb => [s(0), s(1), s(2), 255],
        ColorType::Rgba => [s(0), s(1), s(2), s(3)],
        ColorType::Indexed => unreachable!("indexed fixtures are checked separately"),
    }
}

fn expected(header: &ImageHeader, format: PixelFormat) -> Vec<u8> {
    let mut out = Vec::new();
    for y in 0..header.height {
        for x in 0..header.width {
            let rgba = expected_rgba(header, x, y);
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
    fixtures().into_iter().filter_map(|path| {
        let data = fs::read(&path).unwrap();
        let header = format_png::read_header(&data).unwrap();
        let skip = header.color_type == ColorType::Indexed || name(&path).contains("_trns");
        (!skip).then_some((path, data, header))
    })
}

#[test]
fn every_fixture_converts_to_rgba8() {
    for (path, data, header) in convertible_fixtures() {
        let bitmap = format_png::decode_rgba8(&data).unwrap_or_else(|e| panic!("{}: {e}", name(&path)));

        assert_eq!(bitmap.format(), PixelFormat::Rgba8);
        assert_eq!((bitmap.width(), bitmap.height()), (header.width, header.height));
        assert_eq!(bitmap.data(), expected(&header, PixelFormat::Rgba8), "{}", name(&path));
    }
}

#[test]
fn every_fixture_converts_to_rgb8() {
    for (path, data, header) in convertible_fixtures() {
        let bitmap = format_png::decode_rgb8(&data).unwrap_or_else(|e| panic!("{}: {e}", name(&path)));

        assert_eq!(bitmap.data(), expected(&header, PixelFormat::Rgb8), "{}", name(&path));
    }
}

#[test]
fn rgba8_bitmaps_fit_canvas_image_data() {
    // ImageData needs exactly width * height * 4 bytes, with no row padding.
    for (path, data, header) in convertible_fixtures() {
        let bitmap = format_png::decode_rgba8(&data).unwrap();

        assert_eq!(bitmap.stride(), header.width as usize * 4, "{}", name(&path));
        assert_eq!(bitmap.data().len(), bitmap.stride() * header.height as usize, "{}", name(&path));
    }
}

#[test]
fn reused_decoder_matches_the_simple_api() {
    let mut decoder = format_png::Decoder::new();
    let mut out = Vec::new();

    for (path, data, _) in convertible_fixtures() {
        decoder.decode_bitmap_into(&data, PixelFormat::Rgba8, &mut out).unwrap();

        assert_eq!(out, format_png::decode_rgba8(&data).unwrap().into_data(), "{}", name(&path));
    }
}

#[test]
fn indexed_fixtures_need_a_palette() {
    let mut indexed = 0;
    for path in fixtures() {
        let data = fs::read(&path).unwrap();
        if format_png::read_header(&data).unwrap().color_type != ColorType::Indexed {
            continue;
        }
        indexed += 1;

        assert_eq!(format_png::decode_rgba8(&data), Err(Error::MissingPalette), "{}", name(&path));
    }
    assert!(indexed > 0);
}
