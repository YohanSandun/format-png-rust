#![cfg(test)]

use super::palettize;
use crate::png::{ColorType, ImageHeader, Interlace, Transparency};

fn header(width: u32, height: u32, bit_depth: u8, color_type: ColorType) -> ImageHeader {
    ImageHeader { width, height, bit_depth, color_type, interlace: Interlace::None }
}

/// An RGB row of `count` distinct colors, one pixel each.
fn distinct_rgb(count: usize) -> Vec<u8> {
    (0..count).flat_map(|i| [i as u8, (i >> 8) as u8, 7]).collect()
}

#[test]
fn rgb_becomes_indices_into_a_palette_in_order_of_appearance() {
    let red = [255, 0, 0];
    let green = [0, 255, 0];
    let blue = [0, 0, 255];
    let data = [red, green, red, blue].concat();
    let mut out = Vec::new();

    let palettized = palettize(&header(4, 1, 8, ColorType::Rgb), &data, None, &mut out).unwrap();

    assert_eq!(palettized.header, header(4, 1, 2, ColorType::Indexed));
    assert_eq!(palettized.palette.colors(), [red, green, blue]);
    assert_eq!(palettized.transparency, None);
    assert_eq!(out, [0b00_01_00_10]); // 0, 1, 0, 2 at 2 bits
}

#[test]
fn bit_depth_is_the_smallest_that_fits() {
    for (colors, bit_depth) in [(1, 1), (2, 1), (3, 2), (4, 2), (5, 4), (16, 4), (17, 8), (256, 8)] {
        let mut out = Vec::new();

        let palettized = palettize(&header(colors as u32, 1, 8, ColorType::Rgb), &distinct_rgb(colors), None, &mut out).unwrap();

        assert_eq!(palettized.header.bit_depth, bit_depth, "{colors} colors");
        assert_eq!(palettized.palette.len(), colors);
    }
}

#[test]
fn more_than_256_colors_stay_as_they_are() {
    let mut out = Vec::new();

    assert_eq!(palettize(&header(257, 1, 8, ColorType::Rgb), &distinct_rgb(257), None, &mut out), None);
}

#[test]
fn only_8_bit_rgb_and_rgba_are_converted() {
    let mut out = Vec::new();

    for header in [
        header(2, 1, 8, ColorType::Grayscale),
        header(2, 1, 8, ColorType::GrayscaleAlpha),
        header(2, 1, 8, ColorType::Indexed),
        header(2, 1, 16, ColorType::Rgb),
        header(2, 1, 16, ColorType::Rgba),
    ] {
        let data = vec![0; header.image_size().unwrap()];
        assert_eq!(palettize(&header, &data, None, &mut out), None, "{header:?}");
    }
}

#[test]
fn translucent_colors_come_first_and_go_in_trns() {
    let opaque_red = [255, 0, 0, 255];
    let clear_green = [0, 255, 0, 0];
    let opaque_blue = [0, 0, 255, 255];
    let half_white = [255, 255, 255, 128];
    let data = [opaque_red, clear_green, opaque_blue, half_white].concat();
    let mut out = Vec::new();

    let palettized = palettize(&header(4, 1, 8, ColorType::Rgba), &data, None, &mut out).unwrap();

    // Translucent ones first, each group in order of appearance.
    assert_eq!(palettized.palette.colors(), [[0, 255, 0], [255, 255, 255], [255, 0, 0], [0, 0, 255]]);
    match palettized.transparency {
        Some(Transparency::Palette(alpha)) => assert_eq!(alpha.values(), [0, 128]),
        other => panic!("{other:?}"),
    }
    assert_eq!(out, [0b10_00_11_01]); // red 2, green 0, blue 3, white 1
}

#[test]
fn opaque_rgba_has_no_trns() {
    let data = [[1, 2, 3, 255], [4, 5, 6, 255]].concat();
    let mut out = Vec::new();

    let palettized = palettize(&header(2, 1, 8, ColorType::Rgba), &data, None, &mut out).unwrap();

    assert_eq!(palettized.transparency, None);
}

#[test]
fn one_color_at_two_alphas_is_two_entries() {
    let data = [[9, 9, 9, 255], [9, 9, 9, 10]].concat();
    let mut out = Vec::new();

    let palettized = palettize(&header(2, 1, 8, ColorType::Rgba), &data, None, &mut out).unwrap();

    assert_eq!(palettized.palette.colors(), [[9, 9, 9], [9, 9, 9]]);
    assert_eq!(out, [0b1000_0000]); // opaque one is entry 1, translucent one entry 0
}

#[test]
fn rgb_color_key_becomes_a_transparent_entry() {
    let data = [[1, 1, 1], [2, 2, 2], [1, 1, 1]].concat();
    let key = Transparency::Rgb([2, 2, 2]);
    let mut out = Vec::new();

    let palettized = palettize(&header(3, 1, 8, ColorType::Rgb), &data, Some(&key), &mut out).unwrap();

    assert_eq!(palettized.palette.colors(), [[2, 2, 2], [1, 1, 1]]);
    match palettized.transparency {
        Some(Transparency::Palette(alpha)) => assert_eq!(alpha.values(), [0]),
        other => panic!("{other:?}"),
    }
}

#[test]
fn rows_are_padded_with_zero_bits() {
    // 3x2 at 1 bit: each row is one byte with 5 bits of padding.
    let black = [0, 0, 0];
    let white = [255, 255, 255];
    let data = [black, white, white, white, black, black].concat();
    let mut out = Vec::new();

    let palettized = palettize(&header(3, 2, 8, ColorType::Rgb), &data, None, &mut out).unwrap();

    assert_eq!(palettized.header.bit_depth, 1);
    assert_eq!(out, [0b0110_0000, 0b1000_0000]);
}

#[test]
fn interlacing_is_kept() {
    let mut rgb = header(2, 2, 8, ColorType::Rgb);
    rgb.interlace = Interlace::Adam7;
    let mut out = Vec::new();

    let palettized = palettize(&rgb, &[0; 12], None, &mut out).unwrap();

    assert_eq!(palettized.header.interlace, Interlace::Adam7);
}

#[test]
fn palettize_replaces_old_contents() {
    let mut out = vec![0xEE; 50];

    palettize(&header(1, 1, 8, ColorType::Rgb), &[1, 2, 3], None, &mut out).unwrap();

    assert_eq!(out, [0]);
}
