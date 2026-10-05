#![cfg(test)]

use super::ImageRef;
use crate::png::metadata::{Text, TextKind};
use crate::png::{ChunkPosition, ChunkType, ColorType, ImageHeader, Interlace, Metadata, OwnedChunk, Palette, Transparency};

fn header(color_type: ColorType) -> ImageHeader {
    ImageHeader { width: 2, height: 1, bit_depth: 8, color_type, interlace: Interlace::None }
}

#[test]
fn new_has_no_palette() {
    let image = ImageRef::new(header(ColorType::Rgb), &[1, 2, 3, 4, 5, 6]);

    assert_eq!(*image.header(), header(ColorType::Rgb));
    assert_eq!(image.data(), [1, 2, 3, 4, 5, 6]);
    assert_eq!(image.palette(), None);
}

#[test]
fn with_palette_adds_it() {
    let header = header(ColorType::Indexed);
    let palette = Palette::parse(&[0, 0, 0, 255, 255, 255], &header).unwrap();

    let image = ImageRef::new(header, &[0, 1]).with_palette(&palette);

    assert_eq!(image.palette(), Some(&palette));
}

#[test]
fn from_image_keeps_header_pixels_and_palette() {
    let data = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/valid/indexed_4.png")).unwrap();
    let image = crate::decode(&data).unwrap();

    let image_ref = ImageRef::from(&image);

    assert_eq!(image_ref.header(), image.header());
    assert_eq!(image_ref.data(), image.data());
    assert_eq!(image_ref.palette(), image.palette());
}

#[test]
fn new_has_no_transparency() {
    assert_eq!(ImageRef::new(header(ColorType::Rgb), &[0; 6]).transparency(), None);
}

#[test]
fn with_transparency_adds_it() {
    let transparency = Transparency::Rgb([1, 2, 3]);

    let image = ImageRef::new(header(ColorType::Rgb), &[0; 6]).with_transparency(&transparency);

    assert_eq!(image.transparency(), Some(&transparency));
}

#[test]
fn from_image_keeps_transparency() {
    for name in ["gray_8_trns.png", "rgb_16_trns.png", "indexed_2_trns.png"] {
        let data = std::fs::read(format!("{}/tests/data/valid/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let image = crate::decode(&data).unwrap();

        let image_ref = ImageRef::from(&image);

        assert!(image.transparency().is_some(), "{name}");
        assert_eq!(image_ref.transparency(), image.transparency(), "{name}");
    }
}

#[test]
fn new_has_no_metadata() {
    assert_eq!(ImageRef::new(header(ColorType::Rgb), &[0; 6]).metadata(), None);
}

#[test]
fn with_metadata_adds_it() {
    let title = Text {
        keyword: "Title".to_string(),
        text: "x".to_string(),
        language_tag: String::new(),
        translated_keyword: String::new(),
        kind: TextKind::Plain,
    };
    let metadata = Metadata::default().with_text(title);

    let image = ImageRef::new(header(ColorType::Rgb), &[0; 6]).with_metadata(&metadata);

    assert_eq!(image.metadata(), Some(&metadata));
}

#[test]
fn from_image_keeps_metadata_when_it_was_decoded() {
    let data = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/valid/metadata.png")).unwrap();
    let options = crate::DecodeOptions { preserve_metadata: true, ..crate::DecodeOptions::default() };

    let with = crate::Decoder::with_options(options).decode(&data).unwrap();
    let without = crate::decode(&data).unwrap();

    assert!(!with.metadata().is_empty());
    assert_eq!(ImageRef::from(&with).metadata(), Some(with.metadata()));
    assert_eq!(ImageRef::from(&without).metadata(), None);
}

#[test]
fn new_has_no_extra_chunks() {
    assert!(ImageRef::new(header(ColorType::Rgb), &[0; 6]).chunks().is_empty());
}

#[test]
fn with_chunks_adds_them() {
    let chunks = [OwnedChunk::from_data(ChunkType::from_bytes(*b"myAp").unwrap(), b"settings".to_vec(), ChunkPosition::AfterImageData)];

    let image = ImageRef::new(header(ColorType::Rgb), &[0; 6]).with_chunks(&chunks);

    assert_eq!(image.chunks(), chunks);
}

#[test]
fn from_image_keeps_preserved_chunks() {
    let data = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/valid/ancillary_chunks.png")).unwrap();
    let options = crate::DecodeOptions { preserve_chunks: true, ..crate::DecodeOptions::default() };

    let with = crate::Decoder::with_options(options).decode(&data).unwrap();
    let without = crate::decode(&data).unwrap();

    assert!(!with.ancillary_chunks().is_empty());
    assert_eq!(ImageRef::from(&with).chunks(), with.ancillary_chunks());
    assert!(ImageRef::from(&without).chunks().is_empty());
}
