#![cfg(test)]

use super::{choose_filter, filter_image, filter_row};
use crate::decode::unfilter::{unfilter, unfilter_row};
use crate::encode::options::FilterStrategy;
use crate::png::{ColorType, FilterType, ImageHeader, Interlace};

const FILTERS: [FilterType; 5] = [FilterType::None, FilterType::Sub, FilterType::Up, FilterType::Average, FilterType::Paeth];

fn filtered(filter_type: FilterType, bpp: usize, previous: Option<&[u8]>, row: &[u8]) -> Vec<u8> {
    let mut out = vec![0; row.len()];
    filter_row(filter_type, bpp, previous, row, &mut out);
    out
}

/// Bytes that exercise wrapping: a cheap, deterministic pseudo-random sequence.
fn noise(len: usize, seed: u8) -> Vec<u8> {
    (0..len).map(|i| (i as u8).wrapping_mul(97).wrapping_add(seed).rotate_left(3)).collect()
}

fn header(width: u32, height: u32, bit_depth: u8, color_type: ColorType) -> ImageHeader {
    ImageHeader { width, height, bit_depth, color_type, interlace: Interlace::None }
}

// ---------- filter_row ----------

#[test]
fn none_copies_the_row() {
    assert_eq!(filtered(FilterType::None, 1, Some(&[9, 9, 9]), &[1, 2, 3]), [1, 2, 3]);
}

#[test]
fn sub_subtracts_the_byte_one_pixel_left() {
    assert_eq!(filtered(FilterType::Sub, 1, None, &[1, 2, 3, 5]), [1, 1, 1, 2]);
    // bpp 3: each byte minus the same channel of the pixel before.
    assert_eq!(filtered(FilterType::Sub, 3, None, &[10, 20, 30, 11, 22, 33]), [10, 20, 30, 1, 2, 3]);
}

#[test]
fn sub_wraps() {
    assert_eq!(filtered(FilterType::Sub, 1, None, &[200, 100]), [200, 156]);
}

#[test]
fn up_subtracts_the_byte_above() {
    assert_eq!(filtered(FilterType::Up, 1, Some(&[1, 1, 1]), &[3, 4, 5]), [2, 3, 4]);
}

#[test]
fn first_row_has_zeros_above() {
    assert_eq!(filtered(FilterType::Up, 1, None, &[3, 4, 5]), [3, 4, 5]);
    // Average with nothing above is half the left byte.
    assert_eq!(filtered(FilterType::Average, 1, None, &[10, 20]), [10, 15]);
}

#[test]
fn average_subtracts_the_mean_of_left_and_above() {
    assert_eq!(filtered(FilterType::Average, 1, Some(&[10, 20]), &[30, 40]), [25, 15]);
}

#[test]
fn average_sums_without_wrapping() {
    // (255 + 255) / 2 = 255, so 0 - 255 = 1. Wrapping the sum would give 127 and 129.
    assert_eq!(filtered(FilterType::Average, 1, Some(&[0, 255]), &[255, 0]), [255, 1]);
}

#[test]
fn paeth_subtracts_the_predictor() {
    // First byte: a = 0, b = 5, c = 0, so p = 5 and b is closest: 10 - 5.
    // Second: a = 10, b = 20, c = 5, so p = 25 and b is closest: 22 - 20.
    assert_eq!(filtered(FilterType::Paeth, 1, Some(&[5, 20]), &[10, 22]), [5, 2]);
}

#[test]
fn paeth_predicts_from_the_raw_left_byte() {
    // Second byte: a = 50, b = 0, c = 100, so p = -50 and b is closest: 60 - 0.
    // Predicting from the filtered left byte (206) would pick c instead.
    assert_eq!(filtered(FilterType::Paeth, 1, Some(&[100, 0, 0]), &[50, 60, 70]), [206, 60, 10]);
}

#[test]
fn unfilter_row_reverses_every_filter_at_every_bpp() {
    for filter_type in FILTERS {
        for bpp in [1, 2, 3, 4, 6, 8] {
            let previous = noise(bpp * 5, 7);
            let row = noise(bpp * 5, 91);
            for previous in [None, Some(&previous[..])] {
                let mut restored = vec![0; row.len()];
                unfilter_row(filter_type, bpp, previous, &filtered(filter_type, bpp, previous, &row), &mut restored);

                assert_eq!(restored, row, "{filter_type:?}, bpp {bpp}, previous {}", previous.is_some());
            }
        }
    }
}

// ---------- choose_filter ----------

#[test]
fn choose_filter_picks_sub_for_a_flat_row() {
    // None and Up: 8 × 10. Sub: 10. Paeth with nothing above is Sub too; ties go
    // to the lower type.
    let row = [10; 8];

    assert_eq!(choose_filter(1, None, &row, &mut [0; 8]), FilterType::Sub);
}

#[test]
fn choose_filter_picks_up_for_a_repeated_row() {
    let row = [0, 50, 100, 150, 200, 250];

    assert_eq!(choose_filter(1, Some(&row), &row, &mut [0; 6]), FilterType::Up);
}

#[test]
fn choose_filter_breaks_ties_with_the_lowest_type() {
    assert_eq!(choose_filter(1, Some(&[0; 4]), &[0; 4], &mut [0; 4]), FilterType::None);
}

#[test]
fn choose_filter_compares_whole_rows_not_partial_sums() {
    // Sums: None 160, Sub 64, Up 160, Average 129, Paeth 64. None's and Average's
    // first 64 bytes already sum to 64, the best so far, but their whole rows don't
    // tie it; stopping there and calling it a tie would pick None.
    let mut row = vec![1; 64];
    row.extend([1, 2].repeat(32));

    assert_eq!(choose_filter(1, None, &row, &mut vec![0; row.len()]), FilterType::Sub);
}

#[test]
fn choose_filter_reads_bytes_as_signed() {
    // Sub gives [254, 255, 255, 255]. As signed that's -2, -1, -1, -1, summing to
    // 5, less than None's 2 + 3 + 4 + 5 = 14. As unsigned, Sub's 1019 would lose
    // to None's 1010.
    assert_eq!(choose_filter(1, None, &[254, 253, 252, 251], &mut [0; 4]), FilterType::Sub);
}

// ---------- filter_image ----------

#[test]
fn fixed_none_prefixes_each_row_with_a_zero() {
    let data = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]; // 2x2 RGB
    let mut out = Vec::new();

    filter_image(FilterStrategy::Fixed(FilterType::None), &header(2, 2, 8, ColorType::Rgb), &data, &mut out).unwrap();

    assert_eq!(out, [0, 1, 2, 3, 4, 5, 6, 0, 7, 8, 9, 10, 11, 12]);
}

#[test]
fn fixed_filter_is_used_for_every_row() {
    let data = noise(3 * 4 * 3, 1); // 4x3 RGB
    let mut out = Vec::new();

    filter_image(FilterStrategy::Fixed(FilterType::Paeth), &header(4, 3, 8, ColorType::Rgb), &data, &mut out).unwrap();

    let types: Vec<_> = out.chunks(13).map(|row| row[0]).collect();
    assert_eq!(types, [4, 4, 4]);
}

#[test]
fn adaptive_uses_none_for_indexed_and_sub_8_bit_images() {
    for header in [header(8, 3, 8, ColorType::Indexed), header(8, 3, 1, ColorType::Grayscale), header(8, 3, 4, ColorType::Grayscale)] {
        let stride = header.stride().unwrap();
        let data = noise(stride * 3, 5);
        let mut out = Vec::new();

        filter_image(FilterStrategy::Adaptive, &header, &data, &mut out).unwrap();

        assert!(out.chunks(stride + 1).all(|row| row[0] == 0), "{header:?}");
    }
}

#[test]
fn every_strategy_unfilters_back_to_the_image() {
    let strategies = FILTERS.map(FilterStrategy::Fixed).into_iter().chain([FilterStrategy::Adaptive]);
    let headers = [header(5, 4, 8, ColorType::Rgb), header(5, 4, 16, ColorType::Rgba), header(5, 4, 2, ColorType::Grayscale)];

    for strategy in strategies {
        for header in headers {
            let stride = header.stride().unwrap();
            let data = noise(stride * 4, 33);
            let mut scanlines = Vec::new();
            filter_image(strategy, &header, &data, &mut scanlines).unwrap();
            assert_eq!(scanlines.len(), (stride + 1) * 4, "{strategy:?}, {header:?}");

            let mut restored = vec![0; data.len()];
            unfilter(&scanlines, stride, header.filter_bpp(), &mut restored).unwrap();
            assert_eq!(restored, data, "{strategy:?}, {header:?}");
        }
    }
}

#[test]
fn filter_image_replaces_old_contents() {
    let mut out = vec![0xEE; 100];

    filter_image(FilterStrategy::Fixed(FilterType::None), &header(1, 1, 8, ColorType::Grayscale), &[7], &mut out).unwrap();

    assert_eq!(out, [0, 7]);
}
