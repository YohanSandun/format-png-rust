//! Re-encodes the fixtures in tests/data and decodes them again.

use std::fs;
use std::path::Path;

use format_png::{DecodeOptions, Decoder, Error, ImageRef};

fn valid_fixtures() -> Vec<std::path::PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/valid");
    let mut paths: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "png"))
        .collect();
    paths.sort();
    paths
}

#[test]
fn every_valid_fixture_survives_a_round_trip() {
    let mut decoder = Decoder::with_options(DecodeOptions { strict_ancillary: true, ..DecodeOptions::default() });
    let mut encoder = format_png::Encoder::new();

    for path in valid_fixtures() {
        let original = decoder.decode(&fs::read(&path).unwrap()).unwrap();

        let png = encoder.encode(ImageRef::from(&original)).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let again = decoder.decode(&png).unwrap_or_else(|e| panic!("{}: {e}", path.display()));

        assert_eq!(again.header(), original.header(), "{}", path.display());
        assert_eq!(again.data(), original.data(), "{}", path.display());
        assert_eq!(again.palette(), original.palette(), "{}", path.display());
    }
}

#[test]
fn encode_rgba8_reverses_decode_rgba8() {
    let data = fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/valid/rgba_8.png")).unwrap();
    let bitmap = format_png::decode_rgba8(&data).unwrap();

    let png = format_png::encode_rgba8(bitmap.width(), bitmap.height(), bitmap.data()).unwrap();

    assert_eq!(format_png::decode_rgba8(&png).unwrap(), bitmap);
}

#[test]
fn encode_rgba8_checks_the_data_length() {
    assert_eq!(
        format_png::encode_rgba8(2, 2, &[0; 15]),
        Err(Error::InvalidImageDataLength { expected: 16, actual: 15 })
    );
}

#[test]
fn trns_fixtures_keep_their_transparency() {
    let mut decoder = Decoder::new();
    let mut encoder = format_png::Encoder::new();

    for path in valid_fixtures().into_iter().filter(|path| path.to_string_lossy().contains("trns")) {
        let original = decoder.decode(&fs::read(&path).unwrap()).unwrap();
        assert!(original.transparency().is_some(), "{}", path.display());

        let again = decoder.decode(&encoder.encode(ImageRef::from(&original)).unwrap()).unwrap();

        assert_eq!(again.transparency(), original.transparency(), "{}", path.display());
        assert_eq!(again.data(), original.data(), "{}", path.display());
    }
}
