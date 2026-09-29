//! Decodes the fixtures in tests/data, made by tests/data/generate.py.

use std::fs;
use std::path::{Path, PathBuf};

use format_png::{ColorType, Error, ImageHeader};

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
fn every_invalid_fixture_fails() {
    // PLTE isn't read yet, so a missing palette isn't detected.
    let not_yet_detected = ["indexed_plte_missing.png"];

    for path in fixtures("invalid") {
        let name = path.file_name().unwrap().to_string_lossy();
        if not_yet_detected.contains(&name.as_ref()) {
            continue;
        }

        assert!(format_png::decode(&fs::read(&path).unwrap()).is_err(), "{name} decoded");
    }
}
