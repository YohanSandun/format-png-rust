#![cfg(test)]

use super::{
    Source, apply_palette_alpha, convert, convert_indexed_row, convert_row, convert_row_general, is_unchanged,
    palette_table, read_sample, scale_to_8,
};
use crate::error::Error;
use crate::png::{ColorType, ImageHeader, Interlace, Palette, PaletteAlpha, PixelFormat, Transparency};

fn header(width: u32, height: u32, bit_depth: u8, color_type: ColorType) -> ImageHeader {
    ImageHeader { width, height, bit_depth, color_type, interlace: Interlace::None }
}

/// Converts a single row, with `row` as the whole image.
fn row_to(header: ImageHeader, row: &[u8], format: PixelFormat) -> Result<Vec<u8>, Error> {
    let source = Source { header: &header, stride: row.len(), data: row, palette: None, transparency: None };
    let mut out = vec![0; header.width as usize * format.bytes_per_pixel()];
    convert_row(&source, row, format, &mut out)?;
    Ok(out)
}

// ---------- scale_to_8 ----------

#[test]
fn scale_1_bit() {
    assert_eq!(scale_to_8(0, 1), 0);
    assert_eq!(scale_to_8(1, 1), 255);
}

#[test]
fn scale_2_bit() {
    assert_eq!([0, 1, 2, 3].map(|v| scale_to_8(v, 2)), [0, 85, 170, 255]);
}

#[test]
fn scale_4_bit() {
    assert_eq!(scale_to_8(0, 4), 0);
    assert_eq!(scale_to_8(7, 4), 119);
    assert_eq!(scale_to_8(15, 4), 255);
}

#[test]
fn scale_8_bit_is_unchanged() {
    for value in [0, 1, 127, 200, 255] {
        assert_eq!(scale_to_8(value, 8), value as u8);
    }
}

#[test]
fn scale_16_bit_keeps_high_byte() {
    assert_eq!(scale_to_8(0xABCD, 16), 0xAB);
    assert_eq!(scale_to_8(0x00FF, 16), 0x00);
    assert_eq!(scale_to_8(0xFFFF, 16), 0xFF);
}

// ---------- read_sample ----------

#[test]
fn read_1_bit_samples() {
    let row = [0b1010_0000, 0b0000_0001];

    assert_eq!(read_sample(&row, 0, 1), 1);
    assert_eq!(read_sample(&row, 1, 1), 0);
    assert_eq!(read_sample(&row, 2, 1), 1);
    assert_eq!(read_sample(&row, 15, 1), 1);
}

#[test]
fn read_2_bit_samples() {
    let row = [0b11_01_10_00];

    assert_eq!([0, 1, 2, 3].map(|i| read_sample(&row, i, 2)), [3, 1, 2, 0]);
}

#[test]
fn read_4_bit_samples() {
    let row = [0xA5, 0x0F];

    assert_eq!([0, 1, 2, 3].map(|i| read_sample(&row, i, 4)), [0xA, 0x5, 0x0, 0xF]);
}

#[test]
fn read_8_bit_samples() {
    assert_eq!(read_sample(&[1, 2, 3], 2, 8), 3);
}

#[test]
fn read_16_bit_samples_big_endian() {
    let row = [0x12, 0x34, 0xAB, 0xCD];

    assert_eq!(read_sample(&row, 0, 16), 0x1234);
    assert_eq!(read_sample(&row, 1, 16), 0xABCD);
}

// ---------- convert_row: to RGBA ----------

#[test]
fn gray_8_to_rgba() {
    let out = row_to(header(2, 1, 8, ColorType::Grayscale), &[10, 200], PixelFormat::Rgba8);

    assert_eq!(out, Ok(vec![10, 10, 10, 255, 200, 200, 200, 255]));
}

#[test]
fn gray_1_to_rgba_scales_and_ignores_padding() {
    // 3 pixels 1, 0, 1; the padding bits are set to check they're ignored
    let out = row_to(header(3, 1, 1, ColorType::Grayscale), &[0b1011_1111], PixelFormat::Rgba8);

    assert_eq!(out, Ok(vec![255, 255, 255, 255, 0, 0, 0, 255, 255, 255, 255, 255]));
}

#[test]
fn gray_4_to_rgba() {
    let out = row_to(header(2, 1, 4, ColorType::Grayscale), &[0x7F], PixelFormat::Rgba8);

    assert_eq!(out, Ok(vec![119, 119, 119, 255, 255, 255, 255, 255]));
}

#[test]
fn gray_16_to_rgba() {
    let out = row_to(header(1, 1, 16, ColorType::Grayscale), &[0xAB, 0xCD], PixelFormat::Rgba8);

    assert_eq!(out, Ok(vec![0xAB, 0xAB, 0xAB, 255]));
}

#[test]
fn gray_alpha_8_to_rgba() {
    let out = row_to(header(1, 1, 8, ColorType::GrayscaleAlpha), &[50, 128], PixelFormat::Rgba8);

    assert_eq!(out, Ok(vec![50, 50, 50, 128]));
}

#[test]
fn gray_alpha_16_to_rgba() {
    let row = [0x12, 0x34, 0x80, 0x00];
    let out = row_to(header(1, 1, 16, ColorType::GrayscaleAlpha), &row, PixelFormat::Rgba8);

    assert_eq!(out, Ok(vec![0x12, 0x12, 0x12, 0x80]));
}

#[test]
fn rgb_8_to_rgba_adds_opaque_alpha() {
    let out = row_to(header(2, 1, 8, ColorType::Rgb), &[1, 2, 3, 4, 5, 6], PixelFormat::Rgba8);

    assert_eq!(out, Ok(vec![1, 2, 3, 255, 4, 5, 6, 255]));
}

#[test]
fn rgb_16_to_rgba() {
    let row = [0x10, 0xFF, 0x20, 0xFF, 0x30, 0xFF];
    let out = row_to(header(1, 1, 16, ColorType::Rgb), &row, PixelFormat::Rgba8);

    assert_eq!(out, Ok(vec![0x10, 0x20, 0x30, 255]));
}

#[test]
fn rgba_8_to_rgba_is_unchanged() {
    let row = [1, 2, 3, 4, 5, 6, 7, 8];

    assert_eq!(row_to(header(2, 1, 8, ColorType::Rgba), &row, PixelFormat::Rgba8), Ok(row.to_vec()));
}

#[test]
fn rgba_16_to_rgba() {
    let row = [0x01, 0x99, 0x02, 0x99, 0x03, 0x99, 0x04, 0x99];
    let out = row_to(header(1, 1, 16, ColorType::Rgba), &row, PixelFormat::Rgba8);

    assert_eq!(out, Ok(vec![1, 2, 3, 4]));
}

// ---------- convert_row: to RGB ----------

#[test]
fn gray_to_rgb() {
    let out = row_to(header(2, 1, 8, ColorType::Grayscale), &[10, 200], PixelFormat::Rgb8);

    assert_eq!(out, Ok(vec![10, 10, 10, 200, 200, 200]));
}

#[test]
fn rgb_8_to_rgb_is_unchanged() {
    let row = [1, 2, 3, 4, 5, 6];

    assert_eq!(row_to(header(2, 1, 8, ColorType::Rgb), &row, PixelFormat::Rgb8), Ok(row.to_vec()));
}

#[test]
fn rgba_to_rgb_drops_alpha() {
    let row = [1, 2, 3, 4, 5, 6, 7, 8];

    assert_eq!(row_to(header(2, 1, 8, ColorType::Rgba), &row, PixelFormat::Rgb8), Ok(vec![1, 2, 3, 5, 6, 7]));
}

#[test]
fn gray_alpha_to_rgb_drops_alpha() {
    let out = row_to(header(1, 1, 8, ColorType::GrayscaleAlpha), &[50, 128], PixelFormat::Rgb8);

    assert_eq!(out, Ok(vec![50, 50, 50]));
}

// ---------- convert_row: indexed ----------

/// A palette of `colors`, parsed as for an 8-bit indexed image.
fn palette(colors: &[[u8; 3]]) -> Palette {
    Palette::parse(colors.as_flattened(), &header(1, 1, 8, ColorType::Indexed)).unwrap()
}

/// Converts one indexed row with `palette`.
fn indexed_row_to(width: u32, bit_depth: u8, palette: &Palette, row: &[u8], format: PixelFormat) -> Result<Vec<u8>, Error> {
    let header = header(width, 1, bit_depth, ColorType::Indexed);
    let source = Source { header: &header, stride: row.len(), data: row, palette: Some(palette), transparency: None };
    let mut out = vec![0; width as usize * format.bytes_per_pixel()];
    convert_row(&source, row, format, &mut out)?;
    Ok(out)
}

const COLORS: [[u8; 3]; 4] = [[10, 20, 30], [40, 50, 60], [70, 80, 90], [100, 110, 120]];

#[test]
fn palette_table_holds_the_colors_with_opaque_alpha() {
    let table = palette_table(&palette(&COLORS));

    assert_eq!(table[0], [10, 20, 30, 255]);
    assert_eq!(table[3], [100, 110, 120, 255]);
}

#[test]
fn palette_table_past_the_end_is_opaque_black() {
    let table = palette_table(&palette(&COLORS));

    assert!(table[4..].iter().all(|&entry| entry == [0, 0, 0, 255]));
}

#[test]
fn indexed_8_to_rgba() {
    let out = indexed_row_to(3, 8, &palette(&COLORS), &[2, 0, 3], PixelFormat::Rgba8);

    assert_eq!(out, Ok(vec![70, 80, 90, 255, 10, 20, 30, 255, 100, 110, 120, 255]));
}

#[test]
fn indexed_8_to_rgb() {
    let out = indexed_row_to(2, 8, &palette(&COLORS), &[1, 1], PixelFormat::Rgb8);

    assert_eq!(out, Ok(vec![40, 50, 60, 40, 50, 60]));
}

#[test]
fn indexed_1_bit() {
    // 3 pixels 1, 0, 1, then padding.
    let out = indexed_row_to(3, 1, &palette(&COLORS[..2]), &[0b1010_0000], PixelFormat::Rgb8);

    assert_eq!(out, Ok(vec![40, 50, 60, 10, 20, 30, 40, 50, 60]));
}

#[test]
fn indexed_padding_bits_are_not_indices() {
    // 3 pixels of index 0 with a 1-color palette. The padding bits are set, so
    // reading them as indices would fail with index 1.
    let out = indexed_row_to(3, 1, &palette(&COLORS[..1]), &[0b0001_1111], PixelFormat::Rgb8);

    assert_eq!(out, Ok(vec![10, 20, 30, 10, 20, 30, 10, 20, 30]));
}

#[test]
fn indexed_2_bit() {
    let out = indexed_row_to(4, 2, &palette(&COLORS), &[0b11_10_01_00], PixelFormat::Rgb8);

    assert_eq!(out, Ok(COLORS.iter().rev().flatten().copied().collect()));
}

#[test]
fn indexed_4_bit_across_bytes() {
    // 3 pixels 1, 3, 2, then 4 bits of padding.
    let out = indexed_row_to(3, 4, &palette(&COLORS), &[0x13, 0x2F], PixelFormat::Rgba8);

    assert_eq!(out, Ok(vec![40, 50, 60, 255, 100, 110, 120, 255, 70, 80, 90, 255]));
}

#[test]
fn indexed_index_past_the_palette_fails() {
    let out = indexed_row_to(3, 8, &palette(&COLORS[..2]), &[0, 1, 2], PixelFormat::Rgba8);

    assert_eq!(out, Err(Error::PaletteIndexOutOfRange { index: 2, entries: 2 }));
}

#[test]
fn indexed_sub_byte_index_past_the_palette_fails() {
    // A 2-bit index of 3 with a 3-color palette.
    let out = indexed_row_to(2, 2, &palette(&COLORS[..3]), &[0b00_11_00_00], PixelFormat::Rgb8);

    assert_eq!(out, Err(Error::PaletteIndexOutOfRange { index: 3, entries: 3 }));
}

#[test]
fn indexed_row_with_every_8_bit_index() {
    let colors: Vec<[u8; 3]> = (0..=255u8).map(|i| [i, !i, i / 2]).collect();
    let row: Vec<u8> = (0..=255).collect();
    let table = palette_table(&palette(&colors));
    let mut out = vec![0; 256 * 3];

    convert_indexed_row(&table, 256, 8, PixelFormat::Rgb8, &row, &mut out).unwrap();

    assert_eq!(out, colors.as_flattened());
}

#[test]
fn indexed_needs_a_palette() {
    for format in [PixelFormat::Rgb8, PixelFormat::Rgba8] {
        let out = row_to(header(2, 1, 8, ColorType::Indexed), &[0, 1], format);

        assert_eq!(out, Err(Error::MissingPalette));
    }
}

// ---------- convert ----------

#[test]
fn convert_packs_rows_without_padding() {
    // 10x2 1-bit gray: 2 bytes per source row, 40 bytes per RGBA row
    let header = header(10, 2, 1, ColorType::Grayscale);
    let data = [0b1000_0000, 0b0100_0000, 0b0000_0000, 0b0000_0000];
    let source = Source { header: &header, stride: 2, data: &data, palette: None, transparency: None };
    let mut out = Vec::new();

    convert(&source, PixelFormat::Rgba8, &mut out).unwrap();

    assert_eq!(out.len(), 10 * 2 * 4);
    assert_eq!(&out[..4], &[255, 255, 255, 255]); // (0, 0)
    assert_eq!(&out[4..8], &[0, 0, 0, 255]); // (1, 0)
    assert_eq!(&out[36..40], &[255, 255, 255, 255]); // (9, 0)
    assert!(out[40..].chunks_exact(4).all(|px| px == [0, 0, 0, 255])); // row 1
}

#[test]
fn convert_replaces_old_contents() {
    let header = header(1, 1, 8, ColorType::Rgb);
    let source = Source { header: &header, stride: 3, data: &[7, 8, 9], palette: None, transparency: None };
    let mut out = vec![0xEE; 64];

    convert(&source, PixelFormat::Rgba8, &mut out).unwrap();

    assert_eq!(out, [7, 8, 9, 255]);
}

#[test]
fn convert_reads_each_row_at_the_stride() {
    // 2x2 RGB with rows of 6 bytes
    let header = header(2, 2, 8, ColorType::Rgb);
    let data = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
    let source = Source { header: &header, stride: 6, data: &data, palette: None, transparency: None };
    let mut out = Vec::new();

    convert(&source, PixelFormat::Rgb8, &mut out).unwrap();

    assert_eq!(out, data);
}

// ---------- fast paths ----------

#[test]
fn fast_paths_match_the_general_converter() {
    let color_types = [ColorType::Grayscale, ColorType::GrayscaleAlpha, ColorType::Rgb, ColorType::Rgba];

    for bit_depth in [8, 16] {
        for color_type in color_types {
            for format in [PixelFormat::Rgb8, PixelFormat::Rgba8] {
                // 5 pixels, with every byte different so a wrong offset shows up.
                let header = header(5, 1, bit_depth, color_type);
                let stride = header.stride().unwrap();
                let row: Vec<u8> = (0..stride).map(|i| (i * 37 + 11) as u8).collect();

                let fast = row_to(header, &row, format).unwrap();
                let mut general = vec![0; 5 * format.bytes_per_pixel()];
                convert_row_general(&header, format, &row, &mut general);

                assert_eq!(fast, general, "{bit_depth}-bit {color_type:?} to {format:?}");
            }
        }
    }
}

#[test]
fn only_8_bit_rgb_and_rgba_are_unchanged() {
    assert!(is_unchanged(&header(1, 1, 8, ColorType::Rgb), PixelFormat::Rgb8));
    assert!(is_unchanged(&header(1, 1, 8, ColorType::Rgba), PixelFormat::Rgba8));

    assert!(!is_unchanged(&header(1, 1, 8, ColorType::Rgb), PixelFormat::Rgba8));
    assert!(!is_unchanged(&header(1, 1, 8, ColorType::Rgba), PixelFormat::Rgb8));
    assert!(!is_unchanged(&header(1, 1, 16, ColorType::Rgba), PixelFormat::Rgba8));
    assert!(!is_unchanged(&header(1, 1, 8, ColorType::Grayscale), PixelFormat::Rgb8));
}

#[test]
fn convert_copies_unchanged_images_whole() {
    let header = header(2, 2, 8, ColorType::Rgba);
    let data: Vec<u8> = (0..16).collect();
    let source = Source { header: &header, stride: 8, data: &data, palette: None, transparency: None };
    let mut out = vec![0xEE; 3];

    convert(&source, PixelFormat::Rgba8, &mut out).unwrap();

    assert_eq!(out, data);
}

#[test]
fn convert_16_bit_rows() {
    // 1x2 RGB 16-bit
    let header = header(1, 2, 16, ColorType::Rgb);
    let data = [0x10, 0, 0x20, 0, 0x30, 0, 0x40, 0, 0x50, 0, 0x60, 0];
    let source = Source { header: &header, stride: 6, data: &data, palette: None, transparency: None };
    let mut out = Vec::new();

    convert(&source, PixelFormat::Rgba8, &mut out).unwrap();

    assert_eq!(out, [0x10, 0x20, 0x30, 255, 0x40, 0x50, 0x60, 255]);
}

#[test]
fn convert_indexed_image() {
    // 2x2 4-bit indexed: 1 byte per row, the second pixel of each row in the low bits.
    let header = header(2, 2, 4, ColorType::Indexed);
    let palette = palette(&COLORS);
    let source = Source { header: &header, stride: 1, data: &[0x01, 0x23], palette: Some(&palette), transparency: None };
    let mut out = Vec::new();

    convert(&source, PixelFormat::Rgb8, &mut out).unwrap();

    assert_eq!(out, COLORS.as_flattened());
}

#[test]
fn convert_indexed_passes_on_index_errors() {
    let header = header(1, 2, 8, ColorType::Indexed);
    let palette = palette(&COLORS);
    let source = Source { header: &header, stride: 1, data: &[0, 9], palette: Some(&palette), transparency: None };

    assert_eq!(
        convert(&source, PixelFormat::Rgba8, &mut Vec::new()),
        Err(Error::PaletteIndexOutOfRange { index: 9, entries: 4 })
    );
}

#[test]
fn convert_ignores_a_suggested_palette() {
    let header = header(1, 1, 8, ColorType::Rgb);
    let palette = palette(&COLORS);
    let source = Source { header: &header, stride: 3, data: &[7, 8, 9], palette: Some(&palette), transparency: None };
    let mut out = Vec::new();

    convert(&source, PixelFormat::Rgba8, &mut out).unwrap();

    assert_eq!(out, [7, 8, 9, 255]);
}

#[test]
fn convert_indexed_fails() {
    let header = header(1, 1, 8, ColorType::Indexed);
    let source = Source { header: &header, stride: 1, data: &[0], palette: None, transparency: None };

    assert_eq!(convert(&source, PixelFormat::Rgba8, &mut Vec::new()), Err(Error::MissingPalette));
}

// ---------- tRNS: grayscale and RGB ----------

/// Converts one row with `key` as the image's transparency.
fn keyed_row_to(header: ImageHeader, key: Transparency, row: &[u8], format: PixelFormat) -> Result<Vec<u8>, Error> {
    let source = Source { header: &header, stride: row.len(), data: row, palette: None, transparency: Some(&key) };
    let mut out = vec![0; header.width as usize * format.bytes_per_pixel()];
    convert_row(&source, row, format, &mut out)?;
    Ok(out)
}

#[test]
fn gray_8_key_is_transparent() {
    let out = keyed_row_to(header(3, 1, 8, ColorType::Grayscale), Transparency::Gray(37), &[10, 37, 200], PixelFormat::Rgba8);

    assert_eq!(out, Ok(vec![10, 10, 10, 255, 37, 37, 37, 0, 200, 200, 200, 255]));
}

#[test]
fn gray_key_is_ignored_for_rgb8() {
    let out = keyed_row_to(header(3, 1, 8, ColorType::Grayscale), Transparency::Gray(37), &[10, 37, 200], PixelFormat::Rgb8);

    assert_eq!(out, Ok(vec![10, 10, 10, 37, 37, 37, 200, 200, 200]));
}

#[test]
fn gray_1_bit_key_compares_before_scaling() {
    // Pixels 1, 0, 1 with 0 transparent; the padding bits are ignored.
    let out = keyed_row_to(header(3, 1, 1, ColorType::Grayscale), Transparency::Gray(0), &[0b1010_1111], PixelFormat::Rgba8);

    assert_eq!(out, Ok(vec![255, 255, 255, 255, 0, 0, 0, 0, 255, 255, 255, 255]));
}

#[test]
fn gray_4_bit_key() {
    // Pixels 7 and 15 with 7 transparent; 7 scales to 119.
    let out = keyed_row_to(header(2, 1, 4, ColorType::Grayscale), Transparency::Gray(7), &[0x7F], PixelFormat::Rgba8);

    assert_eq!(out, Ok(vec![119, 119, 119, 0, 255, 255, 255, 255]));
}

#[test]
fn gray_16_key_compares_the_whole_sample() {
    // Both pixels have high byte 0x12, but only the first equals the key.
    let row = [0x12, 0x34, 0x12, 0x35];
    let out = keyed_row_to(header(2, 1, 16, ColorType::Grayscale), Transparency::Gray(0x1234), &row, PixelFormat::Rgba8);

    assert_eq!(out, Ok(vec![0x12, 0x12, 0x12, 0, 0x12, 0x12, 0x12, 255]));
}

#[test]
fn rgb_8_key_is_transparent() {
    let key = Transparency::Rgb([1, 2, 3]);
    let out = keyed_row_to(header(2, 1, 8, ColorType::Rgb), key, &[1, 2, 3, 1, 2, 4], PixelFormat::Rgba8);

    assert_eq!(out, Ok(vec![1, 2, 3, 0, 1, 2, 4, 255]));
}

#[test]
fn rgb_key_needs_every_channel_to_match() {
    let key = Transparency::Rgb([1, 2, 3]);
    let row = [9, 2, 3, 1, 9, 3, 1, 2, 9];
    let out = keyed_row_to(header(3, 1, 8, ColorType::Rgb), key, &row, PixelFormat::Rgba8);

    assert_eq!(out, Ok(vec![9, 2, 3, 255, 1, 9, 3, 255, 1, 2, 9, 255]));
}

#[test]
fn rgb_16_key_compares_the_whole_samples() {
    let key = Transparency::Rgb([0x0100, 0x0200, 0x0300]);
    let row = [1, 0, 2, 0, 3, 0, 1, 0, 2, 0, 3, 1];
    let out = keyed_row_to(header(2, 1, 16, ColorType::Rgb), key, &row, PixelFormat::Rgba8);

    assert_eq!(out, Ok(vec![1, 2, 3, 0, 1, 2, 3, 255]));
}

#[test]
fn rgb_key_is_ignored_for_rgb8() {
    let key = Transparency::Rgb([1, 2, 3]);
    let out = keyed_row_to(header(1, 1, 8, ColorType::Rgb), key, &[1, 2, 3], PixelFormat::Rgb8);

    assert_eq!(out, Ok(vec![1, 2, 3]));
}

#[test]
fn convert_applies_the_key_to_every_row() {
    // 2x2 gray, 8-bit, with 5 transparent.
    let header = header(2, 2, 8, ColorType::Grayscale);
    let key = Transparency::Gray(5);
    let source = Source { header: &header, stride: 2, data: &[5, 6, 7, 5], palette: None, transparency: Some(&key) };
    let mut out = Vec::new();

    convert(&source, PixelFormat::Rgba8, &mut out).unwrap();

    assert_eq!(out, [5, 5, 5, 0, 6, 6, 6, 255, 7, 7, 7, 255, 5, 5, 5, 0]);
}

#[test]
fn convert_rgb_8_to_rgb8_with_key_is_unchanged() {
    let header = header(2, 1, 8, ColorType::Rgb);
    let key = Transparency::Rgb([1, 2, 3]);
    let data = [1, 2, 3, 4, 5, 6];
    let source = Source { header: &header, stride: 6, data: &data, palette: None, transparency: Some(&key) };
    let mut out = Vec::new();

    convert(&source, PixelFormat::Rgb8, &mut out).unwrap();

    assert_eq!(out, data);
}

// ---------- tRNS: indexed ----------

#[test]
fn apply_palette_alpha_sets_the_first_entries() {
    let mut table = palette_table(&palette(&COLORS));

    apply_palette_alpha(&mut table, &PaletteAlpha::new(&[0, 128]));

    assert_eq!(table[0], [10, 20, 30, 0]);
    assert_eq!(table[1], [40, 50, 60, 128]);
    assert_eq!(table[2], [70, 80, 90, 255]);
    assert!(table[4..].iter().all(|&entry| entry == [0, 0, 0, 255]));
}

#[test]
fn apply_palette_alpha_for_every_entry() {
    let colors = [[1, 1, 1]; 256];
    let alpha: Vec<u8> = (0..=255).collect();
    let mut table = palette_table(&palette(&colors));

    apply_palette_alpha(&mut table, &PaletteAlpha::new(&alpha));

    assert!(table.iter().enumerate().all(|(i, entry)| *entry == [1, 1, 1, i as u8]));
}

/// Converts one indexed row with `palette` and `alpha` from tRNS.
fn indexed_alpha_row_to(palette: &Palette, alpha: &[u8], row: &[u8], format: PixelFormat) -> Result<Vec<u8>, Error> {
    let header = header(row.len() as u32, 1, 8, ColorType::Indexed);
    let transparency = Transparency::Palette(PaletteAlpha::new(alpha));
    let source = Source { header: &header, stride: row.len(), data: row, palette: Some(palette), transparency: Some(&transparency) };
    let mut out = vec![0; row.len() * format.bytes_per_pixel()];
    convert_row(&source, row, format, &mut out)?;
    Ok(out)
}

#[test]
fn indexed_trns_sets_alpha() {
    let out = indexed_alpha_row_to(&palette(&COLORS), &[0, 128], &[0, 1, 2], PixelFormat::Rgba8);

    assert_eq!(out, Ok(vec![10, 20, 30, 0, 40, 50, 60, 128, 70, 80, 90, 255]));
}

#[test]
fn indexed_trns_is_ignored_for_rgb8() {
    let out = indexed_alpha_row_to(&palette(&COLORS), &[0, 128], &[0, 1, 2], PixelFormat::Rgb8);

    assert_eq!(out, Ok(vec![10, 20, 30, 40, 50, 60, 70, 80, 90]));
}

#[test]
fn indexed_trns_still_checks_indices() {
    let out = indexed_alpha_row_to(&palette(&COLORS[..2]), &[0, 128], &[0, 2], PixelFormat::Rgba8);

    assert_eq!(out, Err(Error::PaletteIndexOutOfRange { index: 2, entries: 2 }));
}
