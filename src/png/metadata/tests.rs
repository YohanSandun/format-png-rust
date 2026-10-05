#![cfg(test)]

use super::{
    Chromaticities, CodingIndependentCodePoints, Exif, Gamma, IccProfile, Metadata, PhysicalDimensions, RenderingIntent,
    Text, TextKind, Time, Unit,
};

fn text(keyword: &str, kind: TextKind) -> Text {
    Text {
        keyword: keyword.to_string(),
        text: "x".to_string(),
        language_tag: String::new(),
        translated_keyword: String::new(),
        kind,
    }
}

#[test]
fn default_is_empty() {
    assert!(Metadata::default().is_empty());
}

#[test]
fn with_methods_set_each_chunk() {
    let gamma = Gamma::parse(&45455u32.to_be_bytes()).unwrap();
    let chromaticities = Chromaticities {
        white_x: 31270,
        white_y: 32900,
        red_x: 64000,
        red_y: 33000,
        green_x: 30000,
        green_y: 60000,
        blue_x: 15000,
        blue_y: 6000,
    };
    let dimensions = PhysicalDimensions { x: 2835, y: 2835, unit: Unit::Meter };
    let time = Time { year: 2026, month: 10, day: 3, hour: 12, minute: 0, second: 0 };
    let icc_profile = IccProfile { name: "ICC Profile".to_string(), profile: vec![1, 2, 3] };
    let cicp = CodingIndependentCodePoints { color_primaries: 1, transfer_function: 13, matrix_coefficients: 0, full_range: true };
    let exif = Exif::parse(&[b'M', b'M', 0, 42, 0, 0, 0, 8]).unwrap();

    let metadata = Metadata::default()
        .with_gamma(gamma)
        .with_chromaticities(chromaticities)
        .with_srgb(RenderingIntent::Perceptual)
        .with_physical_dimensions(dimensions)
        .with_time(time)
        .with_icc_profile(icc_profile.clone())
        .with_cicp(cicp)
        .with_exif(exif.clone());

    assert_eq!(metadata.gamma(), Some(gamma));
    assert_eq!(metadata.chromaticities(), Some(chromaticities));
    assert_eq!(metadata.srgb(), Some(RenderingIntent::Perceptual));
    assert_eq!(metadata.physical_dimensions(), Some(dimensions));
    assert_eq!(metadata.time(), Some(time));
    assert_eq!(metadata.icc_profile(), Some(&icc_profile));
    assert_eq!(metadata.cicp(), Some(cicp));
    assert_eq!(metadata.exif(), Some(&exif));
    assert!(!metadata.is_empty());
}

#[test]
fn with_text_appends_in_order() {
    let first = text("Title", TextKind::Plain);
    let second = text("Title", TextKind::Compressed);
    let third = text("Author", TextKind::International { compressed: false });

    let metadata = Metadata::default().with_text(first.clone()).with_text(second.clone()).with_text(third.clone());

    assert_eq!(metadata.text(), [first, second, third]);
}

#[test]
fn with_methods_replace_single_chunks() {
    let metadata = Metadata::default().with_srgb(RenderingIntent::Perceptual).with_srgb(RenderingIntent::Saturation);

    assert_eq!(metadata.srgb(), Some(RenderingIntent::Saturation));
}
