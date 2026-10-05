#![cfg(test)]

use super::strip_metadata;
use crate::encode::options::StripChunks;
use crate::png::Metadata;
use crate::png::metadata::{
    Chromaticities, CodingIndependentCodePoints, Exif, Gamma, IccProfile, PhysicalDimensions, RenderingIntent, Text,
    TextKind, Time, Unit,
};

fn full_metadata() -> Metadata {
    Metadata::default()
        .with_gamma(Gamma::new(45455).unwrap())
        .with_chromaticities(Chromaticities { white_x: 1, white_y: 2, red_x: 3, red_y: 4, green_x: 5, green_y: 6, blue_x: 7, blue_y: 8 })
        .with_srgb(RenderingIntent::Perceptual)
        .with_physical_dimensions(PhysicalDimensions { x: 2835, y: 2835, unit: Unit::Meter })
        .with_time(Time { year: 2026, month: 10, day: 5, hour: 12, minute: 0, second: 0 })
        .with_text(Text {
            keyword: "Title".to_string(),
            text: "x".to_string(),
            language_tag: String::new(),
            translated_keyword: String::new(),
            kind: TextKind::Plain,
        })
        .with_icc_profile(IccProfile { name: "ICC Profile".to_string(), profile: vec![1, 2, 3] })
        .with_cicp(CodingIndependentCodePoints { color_primaries: 9, transfer_function: 16, matrix_coefficients: 0, full_range: true })
        .with_exif(Exif::parse(&[b'M', b'M', 0, 42, 0, 0, 0, 8]).unwrap())
}

#[test]
fn keep_keeps_everything() {
    let metadata = full_metadata();

    assert_eq!(strip_metadata(&metadata, StripChunks::Keep), metadata);
}

#[test]
fn safe_keeps_the_chunks_that_change_how_the_image_looks() {
    let metadata = full_metadata();

    let stripped = strip_metadata(&metadata, StripChunks::Safe);

    assert_eq!(stripped.cicp(), metadata.cicp());
    assert_eq!(stripped.icc_profile(), metadata.icc_profile());
    assert_eq!(stripped.srgb(), metadata.srgb());
    assert_eq!(stripped.gamma(), metadata.gamma());
    assert_eq!(stripped.chromaticities(), metadata.chromaticities());
    assert_eq!(stripped.physical_dimensions(), metadata.physical_dimensions());
}

#[test]
fn safe_drops_text_time_and_exif() {
    let stripped = strip_metadata(&full_metadata(), StripChunks::Safe);

    assert!(stripped.text().is_empty());
    assert_eq!(stripped.time(), None);
    assert_eq!(stripped.exif(), None);
}

#[test]
fn all_drops_everything() {
    assert!(strip_metadata(&full_metadata(), StripChunks::All).is_empty());
}
