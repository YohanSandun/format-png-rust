#![cfg(test)]

use super::Image;
use crate::png::{ColorType, ImageHeader, Interlace};

/// 2x3 RGB, 8-bit: 6 bytes per row.
fn image() -> Image {
    let header = ImageHeader {
        width: 2,
        height: 3,
        bit_depth: 8,
        color_type: ColorType::Rgb,
        interlace: Interlace::None,
    };
    Image::new(header, 6, (0..18).collect())
}

#[test]
fn accessors_return_parts() {
    let image = image();

    assert_eq!(image.width(), 2);
    assert_eq!(image.height(), 3);
    assert_eq!(image.stride(), 6);
    assert_eq!(image.header().color_type, ColorType::Rgb);
    assert_eq!(image.data().len(), 18);
}

#[test]
fn row_returns_one_row() {
    let image = image();

    assert_eq!(image.row(0), &[0, 1, 2, 3, 4, 5]);
    assert_eq!(image.row(1), &[6, 7, 8, 9, 10, 11]);
    assert_eq!(image.row(2), &[12, 13, 14, 15, 16, 17]);
}

#[test]
#[should_panic]
fn row_past_the_end_panics() {
    image().row(3);
}

#[test]
fn into_data_returns_pixels() {
    assert_eq!(image().into_data(), (0..18).collect::<Vec<u8>>());
}
