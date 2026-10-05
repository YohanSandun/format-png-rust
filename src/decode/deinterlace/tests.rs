#![cfg(test)]

use super::deinterlace_pass;
use crate::png::adam7::{PASSES, Pass};
use crate::png::{ColorType, ImageHeader, Interlace};

fn header(width: u32, height: u32, bit_depth: u8, color_type: ColorType) -> ImageHeader {
    ImageHeader {
        width,
        height,
        bit_depth,
        color_type,
        interlace: Interlace::Adam7,
    }
}

/// Bits per pixel and bytes per row, computed here so these tests don't depend
/// on the `ImageHeader` helpers.
fn layout(header: &ImageHeader, width: u32) -> (usize, usize) {
    let channels = match header.color_type {
        ColorType::Grayscale | ColorType::Indexed => 1,
        ColorType::GrayscaleAlpha => 2,
        ColorType::Rgb => 3,
        ColorType::Rgba => 4,
    };
    let bits = channels * header.bit_depth as usize;
    (bits, (width as usize * bits).div_ceil(8))
}

fn get_pixel(data: &[u8], row_bytes: usize, bits: usize, x: usize, y: usize) -> u64 {
    let bit = y * row_bytes * 8 + x * bits;
    (0..bits).fold(0, |value, i| {
        let b = bit + i;
        (value << 1) | u64::from((data[b / 8] >> (7 - b % 8)) & 1)
    })
}

fn set_pixel(data: &mut [u8], row_bytes: usize, bits: usize, x: usize, y: usize, value: u64) {
    let bit = y * row_bytes * 8 + x * bits;
    for i in 0..bits {
        let b = bit + i;
        let mask = 1 << (7 - b % 8);
        if (value >> (bits - 1 - i)) & 1 == 1 {
            data[b / 8] |= mask;
        } else {
            data[b / 8] &= !mask;
        }
    }
}

/// A full image where every pixel differs from its neighbours.
fn full_image(header: &ImageHeader) -> Vec<u8> {
    let (bits, stride) = layout(header, header.width);
    let mut data = vec![0; stride * header.height as usize];
    let mask = if bits == 64 {
        u64::MAX
    } else {
        (1 << bits) - 1
    };
    for y in 0..header.height as usize {
        for x in 0..header.width as usize {
            let value = (x as u64 * 0x9E37_79B9 + y as u64 * 0x85EB_CA6B + 1) & mask;
            set_pixel(&mut data, stride, bits, x, y, value);
        }
    }
    data
}

/// The unfiltered rows of one pass, taken from a full image.
fn extract_pass(header: &ImageHeader, pass: &Pass, full: &[u8]) -> Vec<u8> {
    let (bits, stride) = layout(header, header.width);
    let (width, height) = pass_size(header, pass);
    let (_, row_bytes) = layout(header, width as u32);
    let mut data = vec![0; row_bytes * height];
    for py in 0..height {
        for px in 0..width {
            let x = pass.x_start as usize + px * pass.x_step as usize;
            let y = pass.y_start as usize + py * pass.y_step as usize;
            set_pixel(
                &mut data,
                row_bytes,
                bits,
                px,
                py,
                get_pixel(full, stride, bits, x, y),
            );
        }
    }
    data
}

fn pass_size(header: &ImageHeader, pass: &Pass) -> (usize, usize) {
    let count = |size: u32, start: u32, step: u32| {
        if size > start {
            ((size - start).div_ceil(step)) as usize
        } else {
            0
        }
    };
    (
        count(header.width, pass.x_start, pass.x_step),
        count(header.height, pass.y_start, pass.y_step),
    )
}

fn assert_round_trip(header: ImageHeader) {
    let full = full_image(&header);
    let mut out = vec![0; full.len()];

    for pass in &PASSES {
        let (width, height) = pass_size(&header, pass);
        if width > 0 && height > 0 {
            deinterlace_pass(&header, pass, &extract_pass(&header, pass, &full), &mut out);
        }
    }

    assert_eq!(
        out, full,
        "{}x{} {:?} at {} bits",
        header.width, header.height, header.color_type, header.bit_depth
    );
}

// ---------- single pass ----------

#[test]
fn first_pass_writes_only_its_pixels() {
    let header = header(9, 9, 8, ColorType::Grayscale);
    let mut out = vec![0xAA; 81];

    // pass 1 of a 9x9 image is the pixels (0, 0), (8, 0), (0, 8) and (8, 8)
    deinterlace_pass(&header, &PASSES[0], &[1, 2, 3, 4], &mut out);

    let mut expected = vec![0xAA; 81];
    expected[0] = 1;
    expected[8] = 2;
    expected[72] = 3;
    expected[80] = 4;
    assert_eq!(out, expected);
}

#[test]
fn last_pass_fills_odd_rows() {
    let header = header(3, 2, 8, ColorType::Grayscale);
    let mut out = vec![0; 6];

    deinterlace_pass(&header, &PASSES[6], &[4, 5, 6], &mut out);

    assert_eq!(out, [0, 0, 0, 4, 5, 6]);
}

#[test]
fn sub_byte_pass_keeps_neighbouring_bits() {
    // 1-bit, 8x1: pass 2 is the single pixel at x = 4
    let header = header(8, 1, 1, ColorType::Grayscale);
    let mut out = vec![0b1110_0111];

    deinterlace_pass(&header, &PASSES[1], &[0b1000_0000], &mut out);

    assert_eq!(out, [0b1110_1111]);
}

#[test]
fn multi_byte_pixels_move_whole() {
    // 16-bit RGB, 2x1: pass 6 is the pixel at x = 1
    let header = header(2, 1, 16, ColorType::Rgb);
    let mut out = vec![0; 12];

    deinterlace_pass(&header, &PASSES[5], &[1, 2, 3, 4, 5, 6], &mut out);

    assert_eq!(out, [0, 0, 0, 0, 0, 0, 1, 2, 3, 4, 5, 6]);
}

// ---------- all passes ----------

#[test]
fn all_passes_rebuild_every_bit_depth() {
    let formats = [
        (1, ColorType::Grayscale),
        (2, ColorType::Grayscale),
        (4, ColorType::Grayscale),
        (8, ColorType::Grayscale),
        (16, ColorType::Grayscale),
        (4, ColorType::Indexed),
        (8, ColorType::Rgb),
        (16, ColorType::GrayscaleAlpha),
        (8, ColorType::Rgba),
        (16, ColorType::Rgba),
    ];
    for (bit_depth, color_type) in formats {
        assert_round_trip(header(13, 7, bit_depth, color_type));
    }
}

#[test]
fn all_passes_rebuild_small_and_odd_sizes() {
    for (width, height) in [(1, 1), (2, 1), (1, 2), (3, 2), (8, 8), (9, 9), (17, 3)] {
        assert_round_trip(header(width, height, 1, ColorType::Grayscale));
        assert_round_trip(header(width, height, 8, ColorType::Rgb));
    }
}
