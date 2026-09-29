#![cfg(test)]

use super::{Bitmap, PixelFormat};

#[test]
fn bytes_per_pixel() {
    assert_eq!(PixelFormat::Rgb8.bytes_per_pixel(), 3);
    assert_eq!(PixelFormat::Rgba8.bytes_per_pixel(), 4);
}

#[test]
fn accessors_return_parts() {
    let bitmap = Bitmap::new(2, 3, PixelFormat::Rgba8, (0..24).collect());

    assert_eq!(bitmap.width(), 2);
    assert_eq!(bitmap.height(), 3);
    assert_eq!(bitmap.format(), PixelFormat::Rgba8);
    assert_eq!(bitmap.stride(), 8);
    assert_eq!(bitmap.data().len(), 24);
    assert_eq!(bitmap.into_data(), (0..24).collect::<Vec<u8>>());
}

#[test]
fn rgb_stride_has_no_padding() {
    let bitmap = Bitmap::new(5, 1, PixelFormat::Rgb8, vec![0; 15]);

    assert_eq!(bitmap.stride(), 15);
}
