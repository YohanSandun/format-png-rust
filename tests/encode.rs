//! Re-encodes the fixtures in tests/data and decodes them again.

use std::fs;
use std::path::Path;

use format_png::{DecodeOptions, Decoder, EncodeOptions, Encoder, Error, ImageRef};

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
    let mut decoder = Decoder::with_options(DecodeOptions {
        strict_ancillary: true,
        ..DecodeOptions::default()
    });
    let mut encoder = format_png::Encoder::new();

    for path in valid_fixtures() {
        let original = decoder.decode(&fs::read(&path).unwrap()).unwrap();

        let png = encoder
            .encode(ImageRef::from(&original))
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let again = decoder
            .decode(&png)
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));

        assert_eq!(again.header(), original.header(), "{}", path.display());
        assert_eq!(again.data(), original.data(), "{}", path.display());
        assert_eq!(again.palette(), original.palette(), "{}", path.display());
    }
}

#[test]
fn encode_rgba8_reverses_decode_rgba8() {
    let data = fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/valid/rgba_8.png"))
        .unwrap();
    let bitmap = format_png::decode_rgba8(&data).unwrap();

    let png = format_png::encode_rgba8(bitmap.width(), bitmap.height(), bitmap.data()).unwrap();

    assert_eq!(format_png::decode_rgba8(&png).unwrap(), bitmap);
}

#[test]
fn encode_rgba8_checks_the_data_length() {
    assert_eq!(
        format_png::encode_rgba8(2, 2, &[0; 15]),
        Err(Error::InvalidImageDataLength {
            expected: 16,
            actual: 15
        })
    );
}

#[test]
fn trns_fixtures_keep_their_transparency() {
    let mut decoder = Decoder::new();
    let mut encoder = format_png::Encoder::new();

    for path in valid_fixtures()
        .into_iter()
        .filter(|path| path.to_string_lossy().contains("trns"))
    {
        let original = decoder.decode(&fs::read(&path).unwrap()).unwrap();
        assert!(original.transparency().is_some(), "{}", path.display());

        let again = decoder
            .decode(&encoder.encode(ImageRef::from(&original)).unwrap())
            .unwrap();

        assert_eq!(
            again.transparency(),
            original.transparency(),
            "{}",
            path.display()
        );
        assert_eq!(again.data(), original.data(), "{}", path.display());
    }
}

#[test]
fn metadata_fixtures_keep_their_metadata() {
    let options = DecodeOptions {
        preserve_metadata: true,
        strict_ancillary: true,
        ..DecodeOptions::default()
    };
    let mut decoder = Decoder::with_options(options);
    let mut encoder = format_png::Encoder::new();

    for name in [
        "metadata.png",
        "ancillary_chunks.png",
        "ancillary_after_idat.png",
    ] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data/valid")
            .join(name);
        let original = decoder.decode(&fs::read(&path).unwrap()).unwrap();
        assert!(!original.metadata().is_empty(), "{name}");

        let again = decoder
            .decode(&encoder.encode(ImageRef::from(&original)).unwrap())
            .unwrap();

        assert_eq!(again.metadata(), original.metadata(), "{name}");
        assert_eq!(again.data(), original.data(), "{name}");
    }
}

fn chunk_types(png: &[u8]) -> Vec<String> {
    format_png::read_chunks(png)
        .unwrap()
        .chunks()
        .iter()
        .map(|c| c.chunk_type().to_string())
        .collect()
}

fn reencode(name: &str, decode: DecodeOptions, encode: EncodeOptions) -> Vec<u8> {
    let data = fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data/valid")
            .join(name),
    )
    .unwrap();
    let image = Decoder::with_options(decode).decode(&data).unwrap();
    Encoder::with_options(encode)
        .encode(ImageRef::from(&image))
        .unwrap()
}

#[test]
fn preserved_chunks_survive_a_round_trip() {
    let chunks_only = DecodeOptions {
        preserve_chunks: true,
        ..DecodeOptions::default()
    };
    let keep_unsafe = EncodeOptions {
        keep_unsafe_chunks: true,
        ..EncodeOptions::default()
    };

    // gAMA isn't safe to copy, so it needs keep_unsafe_chunks.
    assert_eq!(
        chunk_types(&reencode(
            "ancillary_chunks.png",
            chunks_only.clone(),
            EncodeOptions::default()
        )),
        ["IHDR", "pHYs", "tEXt", "zTXt", "ruSt", "IDAT", "IEND"]
    );
    assert_eq!(
        chunk_types(&reencode("ancillary_chunks.png", chunks_only, keep_unsafe)),
        [
            "IHDR", "gAMA", "pHYs", "tEXt", "zTXt", "ruSt", "IDAT", "IEND"
        ]
    );
}

#[test]
fn preserved_chunks_after_the_image_data_stay_there() {
    let chunks_only = DecodeOptions {
        preserve_chunks: true,
        ..DecodeOptions::default()
    };

    let png = reencode(
        "ancillary_after_idat.png",
        chunks_only,
        EncodeOptions::default(),
    );

    assert_eq!(chunk_types(&png), ["IHDR", "IDAT", "tEXt", "IEND"]);
}

#[test]
fn metadata_and_preserved_chunks_together_write_nothing_twice() {
    let both = DecodeOptions {
        preserve_chunks: true,
        preserve_metadata: true,
        ..DecodeOptions::default()
    };

    let png = reencode("ancillary_chunks.png", both, EncodeOptions::default());

    // Metadata writes gAMA, pHYs and the text; the raw copies of those are
    // skipped, and only ruSt comes from the preserved chunks.
    assert_eq!(
        chunk_types(&png),
        [
            "IHDR", "gAMA", "ruSt", "pHYs", "tEXt", "zTXt", "IDAT", "IEND"
        ]
    );
}

#[test]
fn every_valid_fixture_looks_the_same_with_auto_palette() {
    let mut encoder = Encoder::with_options(EncodeOptions {
        palette: format_png::PaletteMode::Auto,
        ..EncodeOptions::default()
    });
    let mut converted = 0;

    for path in valid_fixtures() {
        let data = fs::read(&path).unwrap();
        let original = format_png::decode(&data).unwrap();

        let png = encoder.encode(ImageRef::from(&original)).unwrap();

        assert_eq!(
            format_png::decode_rgba8(&png).unwrap(),
            format_png::decode_rgba8(&data).unwrap(),
            "{}",
            path.display()
        );
        let header = format_png::read_header(&png).unwrap();
        if header.color_type == format_png::ColorType::Indexed
            && original.header().color_type != format_png::ColorType::Indexed
        {
            converted += 1;
        }
    }

    assert!(converted > 0, "no fixture was converted");
}

#[test]
fn stripping_a_fixture_leaves_only_what_it_needs() {
    let everything = DecodeOptions {
        preserve_chunks: true,
        preserve_metadata: true,
        ..DecodeOptions::default()
    };

    let all = reencode(
        "metadata.png",
        everything.clone(),
        EncodeOptions {
            strip: format_png::StripChunks::All,
            ..EncodeOptions::default()
        },
    );
    let safe = reencode(
        "metadata.png",
        everything,
        EncodeOptions {
            strip: format_png::StripChunks::Safe,
            ..EncodeOptions::default()
        },
    );

    assert_eq!(chunk_types(&all), ["IHDR", "IDAT", "IEND"]);
    assert!(!chunk_types(&safe).contains(&"tIME".to_string()));
    assert!(chunk_types(&safe).contains(&"sRGB".to_string()));
}
