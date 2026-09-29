#![cfg(test)]

//! Filtered bytes below were produced by the filter in tests/data/generate.py,
//! whose output was checked against Pillow.

use super::{paeth_predictor, unfilter, unfilter_row};
use crate::error::Error;
use crate::png::FilterType;

/// Raw rows of two 8-bit RGB pixels (bpp 3).
const PREVIOUS: [u8; 6] = [10, 200, 30, 250, 5, 128];
const ROW: [u8; 6] = [20, 100, 255, 3, 250, 64];

fn unfiltered(filter_type: FilterType, bpp: usize, previous: Option<&[u8]>, filtered: &[u8]) -> Vec<u8> {
    let mut row = filtered.to_vec();
    unfilter_row(filter_type, bpp, previous, &mut row);
    row
}

// ---------- paeth_predictor ----------

#[test]
fn paeth_picks_closest_neighbour() {
    assert_eq!(paeth_predictor(50, 60, 200), 50); // a
    assert_eq!(paeth_predictor(10, 20, 5), 20); // b
    assert_eq!(paeth_predictor(100, 10, 50), 50); // c
}

#[test]
fn paeth_prefers_a_then_b_on_ties() {
    // p = 1: pa = pb = pc = 0
    assert_eq!(paeth_predictor(1, 1, 1), 1);
    // p = 2: pa = 1, pb = 2, pc = 1, so a wins over c
    assert_eq!(paeth_predictor(3, 0, 1), 3);
    // p = 2: pa = 2, pb = 1, pc = 1, so b wins over c
    assert_eq!(paeth_predictor(0, 3, 1), 3);
    // p = 10: pa = 0, pb = 0, pc = 10
    assert_eq!(paeth_predictor(5, 5, 0), 5);
}

#[test]
fn paeth_does_not_overflow_at_extremes() {
    assert_eq!(paeth_predictor(255, 255, 0), 255);
    assert_eq!(paeth_predictor(0, 255, 255), 0);
    assert_eq!(paeth_predictor(255, 255, 255), 255);
}

// ---------- unfilter_row, with a previous row ----------

#[test]
fn unfilter_none() {
    assert_eq!(unfiltered(FilterType::None, 3, Some(&PREVIOUS), &ROW), ROW);
}

#[test]
fn unfilter_sub() {
    let filtered = [20, 100, 255, 239, 150, 65];

    assert_eq!(unfiltered(FilterType::Sub, 3, Some(&PREVIOUS), &filtered), ROW);
}

#[test]
fn unfilter_up() {
    let filtered = [10, 156, 225, 9, 245, 192];

    assert_eq!(unfiltered(FilterType::Up, 3, Some(&PREVIOUS), &filtered), ROW);
}

#[test]
fn unfilter_average() {
    let filtered = [15, 0, 240, 124, 198, 129];

    assert_eq!(unfiltered(FilterType::Average, 3, Some(&PREVIOUS), &filtered), ROW);
}

#[test]
fn unfilter_paeth() {
    let filtered = [10, 156, 225, 9, 245, 65];

    assert_eq!(unfiltered(FilterType::Paeth, 3, Some(&PREVIOUS), &filtered), ROW);
}

// ---------- unfilter_row, first row ----------

#[test]
fn first_row_up_is_unchanged() {
    assert_eq!(unfiltered(FilterType::Up, 3, None, &PREVIOUS), PREVIOUS);
}

#[test]
fn first_row_sub() {
    assert_eq!(unfiltered(FilterType::Sub, 3, None, &[10, 200, 30, 240, 61, 98]), PREVIOUS);
}

#[test]
fn first_row_average_uses_half_the_left_byte() {
    assert_eq!(unfiltered(FilterType::Average, 3, None, &[10, 200, 30, 245, 161, 113]), PREVIOUS);
}

#[test]
fn first_row_paeth_is_like_sub() {
    assert_eq!(unfiltered(FilterType::Paeth, 3, None, &[10, 200, 30, 240, 61, 98]), PREVIOUS);
}

// ---------- unfilter_row, bpp 1 ----------

#[test]
fn unfilter_with_one_byte_per_pixel() {
    let previous = [0x80, 0x0F, 0xFF];
    let row = [0x01, 0x10, 0xF0];

    assert_eq!(unfiltered(FilterType::None, 1, Some(&previous), &[1, 16, 240]), row);
    assert_eq!(unfiltered(FilterType::Sub, 1, Some(&previous), &[1, 15, 224]), row);
    assert_eq!(unfiltered(FilterType::Up, 1, Some(&previous), &[129, 1, 241]), row);
    assert_eq!(unfiltered(FilterType::Average, 1, Some(&previous), &[193, 8, 105]), row);
    assert_eq!(unfiltered(FilterType::Paeth, 1, Some(&previous), &[129, 15, 241]), row);
}

#[test]
fn unfilter_row_shorter_than_bpp() {
    // A 1-pixel row of 16-bit RGBA with bpp 8 has no left neighbours at all.
    let previous = [1, 2, 3, 4, 5, 6, 7, 8];

    assert_eq!(unfiltered(FilterType::Sub, 8, Some(&previous), &[9; 8]), [9; 8]);
    assert_eq!(unfiltered(FilterType::Paeth, 8, Some(&previous), &[0; 8]), previous);
}

// ---------- unfilter ----------

#[test]
fn unfilter_scanlines_strips_filter_bytes() {
    // 3 rows of 6 bytes with filters sub, up, paeth
    let scanlines = [
        1, 1, 2, 3, 3, 3, 3, //
        2, 9, 18, 27, 36, 45, 54, //
        4, 245, 236, 98, 8, 245, 139,
    ];
    let mut out = [0; 18];

    unfilter(&scanlines, 6, 3, &mut out).unwrap();

    assert_eq!(
        out,
        [1, 2, 3, 4, 5, 6, 10, 20, 30, 40, 50, 60, 255, 0, 128, 7, 9, 11]
    );
}

#[test]
fn unfilter_single_row() {
    let mut out = [0; 3];

    unfilter(&[0, 7, 8, 9], 3, 1, &mut out).unwrap();

    assert_eq!(out, [7, 8, 9]);
}

#[test]
fn unfilter_rejects_invalid_filter_type() {
    let scanlines = [0, 1, 2, 5, 3, 4];
    let mut out = [0; 4];

    assert_eq!(unfilter(&scanlines, 2, 1, &mut out), Err(Error::InvalidFilterType(5)));
}
