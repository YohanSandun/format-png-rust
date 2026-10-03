#![cfg(test)]

use super::interlace_pass;
use crate::decode::deinterlace::deinterlace_pass;
use crate::png::adam7::PASSES;
use crate::png::{ColorType, ImageHeader, Interlace};

fn header(width: u32, height: u32, bit_depth: u8, color_type: ColorType) -> ImageHeader {
    ImageHeader { width, height, bit_depth, color_type, interlace: Interlace::Adam7 }
}

fn pass(header: &ImageHeader, index: usize, data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    interlace_pass(header, &PASSES[index], data, &mut out);
    out
}

/// Deterministic pixels with the padding bits at the end of each row zeroed:
/// no pass holds them, so they can't come back from the passes.
fn pixels(header: &ImageHeader) -> Vec<u8> {
    let stride = header.stride().unwrap();
    let mut data: Vec<u8> = (0..header.image_size().unwrap()).map(|i| (i as u8).wrapping_mul(97).wrapping_add(13).rotate_left(3)).collect();
    let used_bits = header.width as usize * usize::from(header.bits_per_pixel()) % 8;
    if used_bits != 0 {
        for row in data.chunks_exact_mut(stride) {
            *row.last_mut().unwrap() &= 0xFF << (8 - used_bits);
        }
    }
    data
}

#[test]
fn passes_of_an_8x8_image_hold_the_spec_pixels() {
    let header = header(8, 8, 8, ColorType::Grayscale);
    let data: Vec<u8> = (0..64).collect(); // each pixel is y * 8 + x

    assert_eq!(pass(&header, 0, &data), [0]);
    assert_eq!(pass(&header, 1, &data), [4]);
    assert_eq!(pass(&header, 2, &data), [32, 36]);
    assert_eq!(pass(&header, 3, &data), [2, 6, 34, 38]);
    assert_eq!(pass(&header, 4, &data), [16, 18, 20, 22, 48, 50, 52, 54]);
    assert_eq!(pass(&header, 5, &data), [1, 3, 5, 7, 17, 19, 21, 23, 33, 35, 37, 39, 49, 51, 53, 55]);
    let odd_rows: Vec<u8> = (8..16).chain(24..32).chain(40..48).chain(56..64).collect();
    assert_eq!(pass(&header, 6, &data), odd_rows);
}

#[test]
fn multi_byte_pixels_move_whole() {
    // 2x1 RGB 16-bit: pass 0 holds pixel 0 and pass 5 pixel 1, 6 bytes each.
    let header = header(2, 1, 16, ColorType::Rgb);
    let data: Vec<u8> = (0..12).collect();

    assert_eq!(pass(&header, 0, &data), [0, 1, 2, 3, 4, 5]);
    assert_eq!(pass(&header, 5, &data), [6, 7, 8, 9, 10, 11]);
}

#[test]
fn packed_pixels_are_repacked_with_zero_padding() {
    // 8x1, 1 bit: pixels 1 0 1 0 1 0 1 0.
    let header = header(8, 1, 1, ColorType::Grayscale);
    let data = [0b1010_1010];

    assert_eq!(pass(&header, 0, &data), [0b1000_0000]); // x = 0
    assert_eq!(pass(&header, 1, &data), [0b1000_0000]); // x = 4
    assert_eq!(pass(&header, 3, &data), [0b1100_0000]); // x = 2, 6
    assert_eq!(pass(&header, 5, &data), [0b0000_0000]); // x = 1, 3, 5, 7
}

#[test]
fn packed_pixels_use_their_bit_depth() {
    // 4x1, 4 bits: pixels 1, 2, 3, 4. Pass 5 holds x = 1 and 3.
    let header = header(4, 1, 4, ColorType::Indexed);

    assert_eq!(pass(&header, 5, &[0x12, 0x34]), [0x24]);
}

#[test]
fn empty_passes_are_empty() {
    // 1x1: only pass 0 has a pixel.
    let header = header(1, 1, 8, ColorType::Rgba);

    for index in 1..7 {
        assert!(pass(&header, index, &[1, 2, 3, 4]).is_empty(), "pass {index}");
    }
}

#[test]
fn interlace_pass_replaces_old_contents() {
    let header = header(1, 1, 8, ColorType::Grayscale);
    let mut out = vec![0xEE; 50];

    interlace_pass(&header, &PASSES[0], &[7], &mut out);

    assert_eq!(out, [7]);
}

#[test]
fn deinterlace_pass_puts_every_pass_back() {
    let formats = [
        (ColorType::Grayscale, 1),
        (ColorType::Grayscale, 2),
        (ColorType::Grayscale, 4),
        (ColorType::Indexed, 8),
        (ColorType::GrayscaleAlpha, 16),
        (ColorType::Rgb, 8),
        (ColorType::Rgba, 16),
    ];
    let sizes = [(1, 1), (2, 2), (3, 5), (8, 8), (9, 9), (13, 7), (33, 2)];

    for (color_type, bit_depth) in formats {
        for (width, height) in sizes {
            let header = header(width, height, bit_depth, color_type);
            let data = pixels(&header);
            let mut restored = vec![0; data.len()];

            for pass in &PASSES {
                let mut pass_data = Vec::new();
                interlace_pass(&header, pass, &data, &mut pass_data);
                deinterlace_pass(&header, pass, &pass_data, &mut restored);
            }

            assert_eq!(restored, data, "{color_type:?} {bit_depth}, {width}x{height}");
        }
    }
}
