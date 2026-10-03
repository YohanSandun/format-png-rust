#![cfg(test)]

use super::Palette;
use crate::error::Error;
use crate::png::{ColorType, ImageHeader, Interlace};

fn header(bit_depth: u8, color_type: ColorType) -> ImageHeader {
    ImageHeader { width: 1, height: 1, bit_depth, color_type, interlace: Interlace::None }
}

fn indexed(bit_depth: u8) -> ImageHeader {
    header(bit_depth, ColorType::Indexed)
}

/// `entries` distinct colors as PLTE data.
fn plte(entries: usize) -> Vec<u8> {
    (0..entries).flat_map(|i| [i as u8, (i * 3) as u8, (i * 7) as u8]).collect()
}

// ---------- parse: success ----------

#[test]
fn parse_reads_colors_in_order() {
    let palette = Palette::parse(&[10, 20, 30, 40, 50, 60], &indexed(8)).unwrap();

    assert_eq!(palette.len(), 2);
    assert_eq!(palette.colors(), &[[10, 20, 30], [40, 50, 60]]);
}

#[test]
fn parse_one_entry() {
    let palette = Palette::parse(&[1, 2, 3], &indexed(1)).unwrap();

    assert_eq!(palette.colors(), &[[1, 2, 3]]);
    assert!(!palette.is_empty());
}

#[test]
fn parse_256_entries() {
    let palette = Palette::parse(&plte(256), &indexed(8)).unwrap();

    assert_eq!(palette.len(), 256);
    assert_eq!(palette.get(255), Some([255, (255 * 3) as u8, (255 * 7) as u8]));
}

#[test]
fn parse_as_many_entries_as_the_bit_depth_can_index() {
    for (bit_depth, entries) in [(1, 2), (2, 4), (4, 16), (8, 256)] {
        let palette = Palette::parse(&plte(entries), &indexed(bit_depth));

        assert_eq!(palette.map(|p| p.len()), Ok(entries), "{bit_depth}-bit");
    }
}

#[test]
fn parse_fewer_entries_than_the_bit_depth_can_index() {
    assert_eq!(Palette::parse(&plte(3), &indexed(4)).map(|p| p.len()), Ok(3));
}

#[test]
fn parse_suggested_palette_for_rgb_and_rgba() {
    // Not limited by bit depth: a 16-bit RGB image may still suggest 256 colors.
    for header in [header(8, ColorType::Rgb), header(16, ColorType::Rgb), header(8, ColorType::Rgba)] {
        assert_eq!(Palette::parse(&plte(256), &header).map(|p| p.len()), Ok(256), "{header:?}");
    }
}

#[test]
fn parsed_palettes_with_the_same_colors_are_equal() {
    let a = Palette::parse(&plte(5), &indexed(8)).unwrap();
    let b = Palette::parse(&plte(5), &indexed(8)).unwrap();

    assert_eq!(a, b);
    assert_ne!(a, Palette::parse(&plte(6), &indexed(8)).unwrap());
}

// ---------- parse: errors ----------

#[test]
fn parse_rejects_empty_data() {
    assert_eq!(Palette::parse(&[], &indexed(8)), Err(Error::InvalidPaletteLength(0)));
}

#[test]
fn parse_rejects_length_not_a_multiple_of_3() {
    for length in [1, 2, 4, 5, 767] {
        assert_eq!(
            Palette::parse(&vec![0; length], &indexed(8)),
            Err(Error::InvalidPaletteLength(length)),
            "{length} bytes"
        );
    }
}

#[test]
fn parse_rejects_more_than_256_entries() {
    assert_eq!(Palette::parse(&plte(257), &indexed(8)), Err(Error::InvalidPaletteLength(771)));
    assert_eq!(Palette::parse(&plte(257), &header(8, ColorType::Rgb)), Err(Error::InvalidPaletteLength(771)));
}

#[test]
fn parse_rejects_more_entries_than_the_bit_depth_can_index() {
    for (bit_depth, entries) in [(1, 3), (2, 5), (4, 17)] {
        assert_eq!(
            Palette::parse(&plte(entries), &indexed(bit_depth)),
            Err(Error::TooManyPaletteEntries { entries, bit_depth }),
            "{bit_depth}-bit"
        );
    }
}

#[test]
fn parse_rejects_palette_for_grayscale() {
    for color_type in [ColorType::Grayscale, ColorType::GrayscaleAlpha] {
        assert_eq!(
            Palette::parse(&plte(2), &header(8, color_type)),
            Err(Error::UnexpectedPalette(color_type)),
            "{color_type:?}"
        );
    }
}

// ---------- accessors ----------

#[test]
fn get_returns_none_past_the_end() {
    let palette = Palette::parse(&plte(3), &indexed(8)).unwrap();

    assert_eq!(palette.get(0), Some([0, 0, 0]));
    assert_eq!(palette.get(2), Some([2, 6, 14]));
    assert_eq!(palette.get(3), None);
    assert_eq!(palette.get(255), None);
}

// ---------- from_colors ----------

#[test]
fn from_colors_keeps_the_colors_in_order() {
    let palette = Palette::from_colors(&[[1, 2, 3], [4, 5, 6]]).unwrap();

    assert_eq!(palette.colors(), [[1, 2, 3], [4, 5, 6]]);
    assert_eq!(palette.len(), 2);
}

#[test]
fn from_colors_accepts_1_to_256_colors() {
    assert!(Palette::from_colors(&[[0; 3]]).is_ok());
    assert_eq!(Palette::from_colors(&[[7; 3]; 256]).map(|p| p.len()), Ok(256));
}

#[test]
fn from_colors_rejects_no_colors_or_more_than_256() {
    assert_eq!(Palette::from_colors(&[]), Err(Error::InvalidPaletteLength(0)));
    assert_eq!(Palette::from_colors(&[[0; 3]; 257]), Err(Error::InvalidPaletteLength(771)));
}

#[test]
fn from_colors_equals_the_same_palette_parsed() {
    let header = ImageHeader { width: 1, height: 1, bit_depth: 8, color_type: ColorType::Indexed, interlace: Interlace::None };

    assert_eq!(Palette::from_colors(&[[1, 2, 3], [4, 5, 6]]), Palette::parse(&[1, 2, 3, 4, 5, 6], &header));
}
