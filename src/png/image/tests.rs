#![cfg(test)]

use super::Image;
use crate::error::Error;
use crate::png::{ColorType, ImageHeader, Interlace, Palette, PaletteAlpha, PixelFormat, Transparency};

/// 2x3 RGB, 8-bit: 6 bytes per row.
fn image() -> Image {
    let header = ImageHeader {
        width: 2,
        height: 3,
        bit_depth: 8,
        color_type: ColorType::Rgb,
        interlace: Interlace::None,
    };
    Image::new(header, 6, (0..18).collect(), None, None)
}

/// 3x1 indexed, 8-bit, with `pixels` as indices into a 2-color palette.
fn indexed_image(pixels: [u8; 3]) -> Image {
    let header = ImageHeader {
        width: 3,
        height: 1,
        bit_depth: 8,
        color_type: ColorType::Indexed,
        interlace: Interlace::None,
    };
    let palette = Palette::parse(&[10, 20, 30, 40, 50, 60], &header).unwrap();
    Image::new(header, 3, pixels.to_vec(), Some(palette), None)
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
fn palette_is_none_without_plte() {
    assert_eq!(image().palette(), None);
}

#[test]
fn palette_of_indexed_image() {
    let image = indexed_image([0, 1, 0]);

    assert_eq!(image.palette().map(Palette::colors), Some(&[[10, 20, 30], [40, 50, 60]][..]));
}

#[test]
fn to_bitmap_of_indexed_image_uses_the_palette() {
    let bitmap = indexed_image([1, 0, 1]).to_bitmap(PixelFormat::Rgb8).unwrap();

    assert_eq!(bitmap.data(), &[40, 50, 60, 10, 20, 30, 40, 50, 60]);
}

#[test]
fn to_bitmap_of_indexed_image_rejects_index_past_the_palette() {
    let image = indexed_image([0, 2, 1]);

    assert_eq!(
        image.to_bitmap(PixelFormat::Rgba8),
        Err(Error::PaletteIndexOutOfRange { index: 2, entries: 2 })
    );
}

#[test]
fn transparency_is_none_without_trns() {
    assert_eq!(image().transparency(), None);
}

#[test]
fn to_bitmap_applies_transparency() {
    // Pixel (1, 0) of `image()` is 3, 4, 5.
    let mut image = image();
    image.transparency = Some(Transparency::Rgb([3, 4, 5]));

    let bitmap = image.to_bitmap(PixelFormat::Rgba8).unwrap();

    assert_eq!(&bitmap.data()[..8], &[0, 1, 2, 255, 3, 4, 5, 0]);
    assert_eq!(image.transparency(), Some(&Transparency::Rgb([3, 4, 5])));
}

#[test]
fn to_bitmap_of_indexed_image_applies_palette_alpha() {
    let mut image = indexed_image([1, 0, 1]);
    image.transparency = Some(Transparency::Palette(PaletteAlpha::new(&[255, 0])));

    let bitmap = image.to_bitmap(PixelFormat::Rgba8).unwrap();

    assert_eq!(bitmap.data(), &[40, 50, 60, 0, 10, 20, 30, 255, 40, 50, 60, 0]);
}
