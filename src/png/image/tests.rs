#![cfg(test)]

use super::Image;
use crate::error::Error;
use crate::png::{ColorType, ImageHeader, Interlace, PixelFormat};

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

#[test]
fn to_bitmap_rgba_adds_opaque_alpha() {
    let bitmap = image().to_bitmap(PixelFormat::Rgba8).unwrap();

    assert_eq!((bitmap.width(), bitmap.height()), (2, 3));
    assert_eq!(bitmap.format(), PixelFormat::Rgba8);
    assert_eq!(&bitmap.data()[..8], &[0, 1, 2, 255, 3, 4, 5, 255]);
    assert_eq!(bitmap.data().len(), 24);
}

#[test]
fn to_bitmap_rgb_of_rgb_image_is_unchanged() {
    let bitmap = image().to_bitmap(PixelFormat::Rgb8).unwrap();

    assert_eq!(bitmap.data(), image().data());
}

#[test]
fn to_bitmap_of_indexed_image_needs_a_palette() {
    let header = ImageHeader {
        width: 2,
        height: 1,
        bit_depth: 8,
        color_type: ColorType::Indexed,
        interlace: Interlace::None,
    };
    let image = Image::new(header, 2, vec![0, 1]);

    assert_eq!(image.to_bitmap(PixelFormat::Rgba8), Err(Error::MissingPalette));
}
