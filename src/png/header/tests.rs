#![cfg(test)]

use super::{ColorType, ImageHeader, Interlace};
use crate::error::Error;

/// Builds IHDR chunk data from its seven fields.
fn ihdr(
    width: u32,
    height: u32,
    bit_depth: u8,
    color_type: u8,
    compression: u8,
    filter: u8,
    interlace: u8,
) -> Vec<u8> {
    let mut data = Vec::with_capacity(ImageHeader::LENGTH);
    data.extend_from_slice(&width.to_be_bytes());
    data.extend_from_slice(&height.to_be_bytes());
    data.extend_from_slice(&[bit_depth, color_type, compression, filter, interlace]);
    data
}

// ---------- ColorType ----------

#[test]
fn color_type_from_valid_values() {
    assert_eq!(ColorType::try_from(0), Ok(ColorType::Grayscale));
    assert_eq!(ColorType::try_from(2), Ok(ColorType::Rgb));
    assert_eq!(ColorType::try_from(3), Ok(ColorType::Indexed));
    assert_eq!(ColorType::try_from(4), Ok(ColorType::GrayscaleAlpha));
    assert_eq!(ColorType::try_from(6), Ok(ColorType::Rgba));
}

#[test]
fn color_type_from_invalid_values_fails() {
    for value in [1, 5, 7, 255] {
        assert_eq!(
            ColorType::try_from(value),
            Err(Error::InvalidColorType(value))
        );
    }
}

// ---------- Interlace ----------

#[test]
fn interlace_from_valid_values() {
    assert_eq!(Interlace::try_from(0), Ok(Interlace::None));
    assert_eq!(Interlace::try_from(1), Ok(Interlace::Adam7));
}

#[test]
fn interlace_from_invalid_value_fails() {
    assert_eq!(
        Interlace::try_from(2),
        Err(Error::InvalidInterlaceMethod(2))
    );
}

// ---------- parse: success ----------

#[test]
fn parse_valid_header() {
    let header = ImageHeader::parse(&ihdr(640, 480, 16, 2, 0, 0, 1)).unwrap();

    assert_eq!(
        header,
        ImageHeader {
            width: 640,
            height: 480,
            bit_depth: 16,
            color_type: ColorType::Rgb,
            interlace: Interlace::Adam7,
        }
    );
}

#[test]
fn parse_accepts_max_dimensions() {
    let max = (1 << 31) - 1;
    let header = ImageHeader::parse(&ihdr(max, max, 8, 6, 0, 0, 0)).unwrap();

    assert_eq!((header.width, header.height), (max, max));
}

#[test]
fn parse_accepts_every_allowed_bit_depth() {
    let allowed: [(u8, &[u8]); 5] = [
        (0, &[1, 2, 4, 8, 16]),
        (2, &[8, 16]),
        (3, &[1, 2, 4, 8]),
        (4, &[8, 16]),
        (6, &[8, 16]),
    ];

    for (color_type, depths) in allowed {
        for &depth in depths {
            assert!(
                ImageHeader::parse(&ihdr(1, 1, depth, color_type, 0, 0, 0)).is_ok(),
                "color type {color_type} with bit depth {depth} should be accepted"
            );
        }
    }
}

// ---------- parse: length ----------

#[test]
fn parse_rejects_short_data() {
    let data = ihdr(1, 1, 8, 6, 0, 0, 0);

    assert_eq!(
        ImageHeader::parse(&data[..12]),
        Err(Error::InvalidImageHeaderLength(12))
    );
}

#[test]
fn parse_rejects_long_data() {
    let mut data = ihdr(1, 1, 8, 6, 0, 0, 0);
    data.push(0);

    assert_eq!(
        ImageHeader::parse(&data),
        Err(Error::InvalidImageHeaderLength(14))
    );
}

#[test]
fn parse_rejects_empty_data() {
    assert_eq!(
        ImageHeader::parse(&[]),
        Err(Error::InvalidImageHeaderLength(0))
    );
}

// ---------- parse: dimensions ----------

#[test]
fn parse_rejects_zero_width() {
    assert_eq!(
        ImageHeader::parse(&ihdr(0, 1, 8, 6, 0, 0, 0)),
        Err(Error::InvalidDimensions {
            width: 0,
            height: 1
        })
    );
}

#[test]
fn parse_rejects_zero_height() {
    assert_eq!(
        ImageHeader::parse(&ihdr(1, 0, 8, 6, 0, 0, 0)),
        Err(Error::InvalidDimensions {
            width: 1,
            height: 0
        })
    );
}

#[test]
fn parse_rejects_dimensions_above_2_pow_31_minus_1() {
    assert_eq!(
        ImageHeader::parse(&ihdr(1 << 31, 1, 8, 6, 0, 0, 0)),
        Err(Error::InvalidDimensions {
            width: 1 << 31,
            height: 1
        })
    );
    assert_eq!(
        ImageHeader::parse(&ihdr(1, u32::MAX, 8, 6, 0, 0, 0)),
        Err(Error::InvalidDimensions {
            width: 1,
            height: u32::MAX
        })
    );
}

// ---------- parse: color type and bit depth ----------

#[test]
fn parse_rejects_invalid_color_type() {
    assert_eq!(
        ImageHeader::parse(&ihdr(1, 1, 8, 5, 0, 0, 0)),
        Err(Error::InvalidColorType(5))
    );
}

#[test]
fn parse_rejects_disallowed_bit_depths() {
    let rejected = [
        (ColorType::Grayscale, 3),
        (ColorType::Grayscale, 32),
        (ColorType::Rgb, 4),
        (ColorType::Indexed, 16),
        (ColorType::GrayscaleAlpha, 1),
        (ColorType::Rgba, 0),
    ];

    for (color_type, bit_depth) in rejected {
        assert_eq!(
            ImageHeader::parse(&ihdr(1, 1, bit_depth, color_type as u8, 0, 0, 0)),
            Err(Error::InvalidBitDepth {
                color_type,
                bit_depth
            }),
        );
    }
}

// ---------- parse: methods ----------

#[test]
fn parse_rejects_unknown_compression_method() {
    assert_eq!(
        ImageHeader::parse(&ihdr(1, 1, 8, 6, 1, 0, 0)),
        Err(Error::InvalidCompressionMethod(1))
    );
}

#[test]
fn parse_rejects_unknown_filter_method() {
    assert_eq!(
        ImageHeader::parse(&ihdr(1, 1, 8, 6, 0, 1, 0)),
        Err(Error::InvalidFilterMethod(1))
    );
}

#[test]
fn parse_rejects_unknown_interlace_method() {
    assert_eq!(
        ImageHeader::parse(&ihdr(1, 1, 8, 6, 0, 0, 2)),
        Err(Error::InvalidInterlaceMethod(2))
    );
}

// ---------- sizes ----------

fn header(
    width: u32,
    height: u32,
    bit_depth: u8,
    color_type: ColorType,
    interlace: Interlace,
) -> ImageHeader {
    ImageHeader {
        width,
        height,
        bit_depth,
        color_type,
        interlace,
    }
}

#[test]
fn channels_per_color_type() {
    assert_eq!(ColorType::Grayscale.channels(), 1);
    assert_eq!(ColorType::Rgb.channels(), 3);
    assert_eq!(ColorType::Indexed.channels(), 1);
    assert_eq!(ColorType::GrayscaleAlpha.channels(), 2);
    assert_eq!(ColorType::Rgba.channels(), 4);
}

#[test]
fn bits_per_pixel() {
    assert_eq!(
        header(1, 1, 1, ColorType::Grayscale, Interlace::None).bits_per_pixel(),
        1
    );
    assert_eq!(
        header(1, 1, 4, ColorType::Indexed, Interlace::None).bits_per_pixel(),
        4
    );
    assert_eq!(
        header(1, 1, 8, ColorType::Rgb, Interlace::None).bits_per_pixel(),
        24
    );
    assert_eq!(
        header(1, 1, 16, ColorType::GrayscaleAlpha, Interlace::None).bits_per_pixel(),
        32
    );
    assert_eq!(
        header(1, 1, 16, ColorType::Rgba, Interlace::None).bits_per_pixel(),
        64
    );
}

#[test]
fn filter_bpp_rounds_sub_byte_pixels_up_to_one() {
    assert_eq!(
        header(1, 1, 1, ColorType::Grayscale, Interlace::None).filter_bpp(),
        1
    );
    assert_eq!(
        header(1, 1, 4, ColorType::Indexed, Interlace::None).filter_bpp(),
        1
    );
    assert_eq!(
        header(1, 1, 8, ColorType::Grayscale, Interlace::None).filter_bpp(),
        1
    );
    assert_eq!(
        header(1, 1, 16, ColorType::Grayscale, Interlace::None).filter_bpp(),
        2
    );
    assert_eq!(
        header(1, 1, 8, ColorType::Rgb, Interlace::None).filter_bpp(),
        3
    );
    assert_eq!(
        header(1, 1, 16, ColorType::Rgba, Interlace::None).filter_bpp(),
        8
    );
}

#[test]
fn stride_pads_sub_byte_rows() {
    assert_eq!(
        header(13, 7, 1, ColorType::Grayscale, Interlace::None).stride(),
        Ok(2)
    );
    assert_eq!(
        header(13, 7, 2, ColorType::Grayscale, Interlace::None).stride(),
        Ok(4)
    );
    assert_eq!(
        header(16, 1, 1, ColorType::Grayscale, Interlace::None).stride(),
        Ok(2)
    );
    assert_eq!(
        header(17, 1, 4, ColorType::Indexed, Interlace::None).stride(),
        Ok(9)
    );
}

#[test]
fn stride_of_whole_byte_pixels() {
    assert_eq!(
        header(13, 7, 8, ColorType::Rgba, Interlace::None).stride(),
        Ok(52)
    );
    assert_eq!(
        header(13, 7, 16, ColorType::Rgb, Interlace::None).stride(),
        Ok(78)
    );
    assert_eq!(
        header(13, 7, 16, ColorType::Rgba, Interlace::None).stride(),
        Ok(104)
    );
}

#[test]
fn row_bytes_for_narrower_rows() {
    let header = header(13, 7, 2, ColorType::Grayscale, Interlace::Adam7);

    assert_eq!(header.row_bytes(1), Ok(1));
    assert_eq!(header.row_bytes(4), Ok(1));
    assert_eq!(header.row_bytes(5), Ok(2));
}

#[test]
fn image_size_is_stride_times_height() {
    assert_eq!(
        header(13, 7, 8, ColorType::Rgba, Interlace::None).image_size(),
        Ok(364)
    );
    assert_eq!(
        header(13, 7, 1, ColorType::Grayscale, Interlace::None).image_size(),
        Ok(14)
    );
    assert_eq!(
        header(1, 1, 8, ColorType::Rgba, Interlace::Adam7).image_size(),
        Ok(4)
    );
}

#[test]
fn scanline_size_adds_a_filter_byte_per_row() {
    assert_eq!(
        header(13, 7, 8, ColorType::Rgba, Interlace::None).scanline_size(),
        Ok(371)
    );
    assert_eq!(
        header(13, 7, 1, ColorType::Grayscale, Interlace::None).scanline_size(),
        Ok(21)
    );
    assert_eq!(
        header(13, 7, 16, ColorType::Rgb, Interlace::None).scanline_size(),
        Ok(553)
    );
}

#[test]
fn scanline_size_counts_every_non_empty_adam7_pass() {
    assert_eq!(
        header(13, 7, 8, ColorType::Rgba, Interlace::Adam7).scanline_size(),
        Ok(378)
    );
    assert_eq!(
        header(13, 7, 1, ColorType::Grayscale, Interlace::Adam7).scanline_size(),
        Ok(31)
    );
    assert_eq!(
        header(13, 7, 2, ColorType::Grayscale, Interlace::Adam7).scanline_size(),
        Ok(43)
    );
    assert_eq!(
        header(13, 7, 16, ColorType::Rgba, Interlace::Adam7).scanline_size(),
        Ok(742)
    );
}

#[test]
fn scanline_size_skips_empty_adam7_passes() {
    // only the first pass has pixels: one row of one pixel plus its filter byte
    assert_eq!(
        header(1, 1, 8, ColorType::Rgba, Interlace::Adam7).scanline_size(),
        Ok(5)
    );
}

#[test]
#[cfg(target_pointer_width = "64")]
fn sizes_of_largest_image_overflow_usize() {
    let max = (1 << 31) - 1;
    let header = header(max, max, 16, ColorType::Rgba, Interlace::None);

    assert_eq!(header.stride(), Ok(8 * max as usize));
    assert_eq!(header.image_size(), Err(Error::ImageTooLarge));
    assert_eq!(header.scanline_size(), Err(Error::ImageTooLarge));
}

#[test]
#[cfg(target_pointer_width = "64")]
fn adam7_scanline_size_of_largest_image_overflows_usize() {
    let max = (1 << 31) - 1;

    assert_eq!(
        header(max, max, 16, ColorType::Rgba, Interlace::Adam7).scanline_size(),
        Err(Error::ImageTooLarge)
    );
}
