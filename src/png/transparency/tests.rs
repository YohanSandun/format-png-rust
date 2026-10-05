#![cfg(test)]

use super::{PaletteAlpha, Transparency};
use crate::error::Error;
use crate::png::{ColorType, ImageHeader, Interlace, Palette};

fn header(bit_depth: u8, color_type: ColorType) -> ImageHeader {
    ImageHeader { width: 1, height: 1, bit_depth, color_type, interlace: Interlace::None }
}

/// A palette of `entries` colors for an 8-bit indexed image.
fn palette(entries: usize) -> Palette {
    Palette::parse(&vec![0; entries * 3], &header(8, ColorType::Indexed)).unwrap()
}

fn parse(data: &[u8], header: &ImageHeader) -> Result<Transparency, Error> {
    Transparency::parse(data, header, None)
}

fn parse_indexed(data: &[u8], palette_entries: usize) -> Result<Transparency, Error> {
    Transparency::parse(data, &header(8, ColorType::Indexed), Some(&palette(palette_entries)))
}

// ---------- grayscale ----------

#[test]
fn gray_reads_a_big_endian_value() {
    assert_eq!(parse(&[0x12, 0x34], &header(16, ColorType::Grayscale)), Ok(Transparency::Gray(0x1234)));
    assert_eq!(parse(&[0x00, 0x25], &header(8, ColorType::Grayscale)), Ok(Transparency::Gray(37)));
}

#[test]
fn gray_keeps_only_the_bits_of_the_bit_depth() {
    // 0x0123 has bits above the bit depth set, which must be ignored.
    for (bit_depth, expected) in [(1, 1), (2, 3), (4, 3), (8, 0x23)] {
        assert_eq!(
            parse(&[0x01, 0x23], &header(bit_depth, ColorType::Grayscale)),
            Ok(Transparency::Gray(expected)),
            "{bit_depth}-bit"
        );
    }
}

#[test]
fn gray_rejects_other_lengths() {
    for length in [0, 1, 3, 6] {
        assert_eq!(
            parse(&vec![0; length], &header(8, ColorType::Grayscale)),
            Err(Error::InvalidTransparencyLength { color_type: ColorType::Grayscale, length }),
            "{length} bytes"
        );
    }
}

// ---------- RGB ----------

#[test]
fn rgb_reads_three_big_endian_values() {
    let data = [0x00, 0x01, 0x00, 0x35, 0xAB, 0xCD];

    assert_eq!(parse(&data, &header(16, ColorType::Rgb)), Ok(Transparency::Rgb([1, 53, 0xABCD])));
}

#[test]
fn rgb_8_keeps_only_the_low_byte() {
    let data = [0xFF, 0x10, 0x00, 0x20, 0x01, 0x30];

    assert_eq!(parse(&data, &header(8, ColorType::Rgb)), Ok(Transparency::Rgb([0x10, 0x20, 0x30])));
}

#[test]
fn rgb_rejects_other_lengths() {
    for length in [0, 2, 5, 7] {
        assert_eq!(
            parse(&vec![0; length], &header(8, ColorType::Rgb)),
            Err(Error::InvalidTransparencyLength { color_type: ColorType::Rgb, length }),
            "{length} bytes"
        );
    }
}

#[test]
fn rgb_ignores_a_suggested_palette() {
    let rgb = header(8, ColorType::Rgb);

    assert_eq!(
        Transparency::parse(&[0, 1, 0, 2, 0, 3], &rgb, Some(&palette(2))),
        Ok(Transparency::Rgb([1, 2, 3]))
    );
}

// ---------- indexed ----------

#[test]
fn indexed_reads_one_alpha_per_entry() {
    let Ok(Transparency::Palette(alpha)) = parse_indexed(&[0, 128, 255], 4) else {
        panic!("expected palette alpha");
    };

    assert_eq!(alpha.values(), &[0, 128, 255]);
}

#[test]
fn indexed_may_have_as_many_entries_as_the_palette() {
    let Ok(Transparency::Palette(alpha)) = parse_indexed(&[7; 256], 256) else {
        panic!("expected palette alpha");
    };

    assert_eq!(alpha.values().len(), 256);
}

#[test]
fn indexed_rejects_more_entries_than_the_palette() {
    assert_eq!(
        parse_indexed(&[0; 5], 4),
        Err(Error::TooManyTransparencyEntries { entries: 5, palette_entries: 4 })
    );
}

#[test]
fn indexed_rejects_empty_data() {
    assert_eq!(
        parse_indexed(&[], 4),
        Err(Error::InvalidTransparencyLength { color_type: ColorType::Indexed, length: 0 })
    );
}

#[test]
fn indexed_needs_the_palette_first() {
    assert_eq!(parse(&[0, 128], &header(8, ColorType::Indexed)), Err(Error::TransparencyBeforePalette));
}

// ---------- color types with alpha ----------

#[test]
fn color_types_with_alpha_reject_transparency() {
    for color_type in [ColorType::GrayscaleAlpha, ColorType::Rgba] {
        assert_eq!(
            parse(&[0, 0], &header(8, color_type)),
            Err(Error::UnexpectedTransparency(color_type)),
            "{color_type:?}"
        );
    }
}

// ---------- PaletteAlpha ----------

#[test]
fn palette_alpha_past_the_end_is_opaque() {
    let alpha = PaletteAlpha::new(&[0, 50]);

    assert_eq!(alpha.get(0), 0);
    assert_eq!(alpha.get(1), 50);
    assert_eq!(alpha.get(2), 255);
    assert_eq!(alpha.get(255), 255);
}

#[test]
fn palette_alphas_with_the_same_values_are_equal() {
    assert_eq!(PaletteAlpha::new(&[1, 2]), PaletteAlpha::new(&[1, 2]));
    // Same alpha for every index, but a different chunk.
    assert_ne!(PaletteAlpha::new(&[1, 2]), PaletteAlpha::new(&[1, 2, 255]));
}

// ---------- PaletteAlpha::from_values ----------

#[test]
fn from_values_keeps_the_values_in_order() {
    let alpha = PaletteAlpha::from_values(&[0, 128]).unwrap();

    assert_eq!(alpha.values(), [0, 128]);
    assert_eq!((alpha.get(0), alpha.get(1), alpha.get(2)), (0, 128, 255));
}

#[test]
fn from_values_accepts_1_to_256_values() {
    assert!(PaletteAlpha::from_values(&[0]).is_ok());
    assert_eq!(PaletteAlpha::from_values(&[7; 256]).map(|a| a.values().len()), Ok(256));
}

#[test]
fn from_values_rejects_no_values_or_more_than_256() {
    for length in [0, 257] {
        assert_eq!(
            PaletteAlpha::from_values(&vec![0; length]),
            Err(Error::InvalidTransparencyLength { color_type: ColorType::Indexed, length }),
            "{length} values"
        );
    }
}

#[test]
fn from_values_equals_the_same_alpha_parsed() {
    let parsed = parse_indexed(&[0, 128], 4).unwrap();

    assert_eq!(Transparency::Palette(PaletteAlpha::from_values(&[0, 128]).unwrap()), parsed);
}
