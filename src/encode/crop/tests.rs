#![cfg(test)]

use super::{Area, Cropped, changed_area, crop, crop_frames};
use crate::encode::animation_ref::FrameRef;
use crate::png::{BlendOp, ColorType, DisposeOp, ImageHeader, Interlace};

fn header(width: u32, height: u32, color_type: ColorType, bit_depth: u8) -> ImageHeader {
    ImageHeader {
        width,
        height,
        bit_depth,
        color_type,
        interlace: Interlace::None,
    }
}

fn area(x: u32, y: u32, width: u32, height: u32) -> Area {
    Area {
        x,
        y,
        width,
        height,
    }
}

// ---------- changed_area ----------

#[test]
fn changed_area_of_identical_images_is_none() {
    let gray = header(3, 2, ColorType::Grayscale, 8);
    let data = [1, 2, 3, 4, 5, 6];

    assert_eq!(changed_area(&gray, &data, &data), None);
}

#[test]
fn changed_area_bounds_every_changed_pixel() {
    let gray = header(4, 3, ColorType::Grayscale, 8);
    let before = [0; 12];
    #[rustfmt::skip]
    let after = [
        0, 0, 0, 0,
        0, 9, 0, 0,
        0, 0, 0, 9,
    ];

    assert_eq!(changed_area(&gray, &before, &after), Some(area(1, 1, 3, 2)));
}

#[test]
fn changed_area_compares_whole_pixels() {
    // Only the second byte of the second RGB pixel differs.
    let rgb = header(3, 1, ColorType::Rgb, 8);
    let before = [0; 9];
    let after = [0, 0, 0, 0, 1, 0, 0, 0, 0];

    assert_eq!(changed_area(&rgb, &before, &after), Some(area(1, 0, 1, 1)));
}

#[test]
fn changed_area_reads_packed_pixels() {
    // 2-bit gray, 3 pixels a row: pixel 2 is bits 4-5.
    let gray = header(3, 1, ColorType::Grayscale, 2);

    assert_eq!(
        changed_area(&gray, &[0b0000_0000], &[0b0000_1100]),
        Some(area(2, 0, 1, 1))
    );
}

#[test]
fn changed_area_ignores_row_padding() {
    let gray = header(3, 1, ColorType::Grayscale, 2);

    assert_eq!(changed_area(&gray, &[0b0000_0000], &[0b0000_0011]), None);
}

// ---------- crop ----------

#[test]
fn crop_copies_whole_byte_pixels() {
    let rgb16 = header(3, 2, ColorType::Rgb, 16);
    let data: Vec<u8> = (0..36).collect();
    let mut out = Vec::new();

    crop(&rgb16, &data, area(1, 1, 2, 1), &mut out);

    assert_eq!(out, (24..36).collect::<Vec<u8>>());
}

#[test]
fn crop_repacks_packed_pixels() {
    // 1-bit, 5 pixels a row: 10110, 01101.
    let gray = header(5, 2, ColorType::Grayscale, 1);
    let data = [0b1011_0000, 0b0110_1000];
    let mut out = Vec::new();

    crop(&gray, &data, area(1, 0, 3, 2), &mut out);

    assert_eq!(out, [0b0110_0000, 0b1100_0000]);
}

#[test]
fn crop_then_changed_area_round_trip() {
    let gray = header(9, 5, ColorType::Grayscale, 4);
    let data: Vec<u8> = (0..gray.image_size().unwrap() as u8).collect();
    let mut out = Vec::new();

    crop(&gray, &data, area(0, 0, 9, 5), &mut out);

    // Cropping to the whole image only clears the padding bits.
    assert_eq!(changed_area(&gray, &data, &out), None);
}

// ---------- crop_frames ----------

const CANVAS: ImageHeader = ImageHeader {
    width: 3,
    height: 2,
    bit_depth: 8,
    color_type: ColorType::Grayscale,
    interlace: Interlace::None,
};
const FIRST: [u8; 6] = [0; 6];
const SECOND: [u8; 6] = [0, 0, 0, 0, 7, 0];

#[test]
fn crop_frames_crops_to_what_changed() {
    let frames = [FrameRef::new(3, 2, &FIRST), FrameRef::new(3, 2, &SECOND)];

    let cropped = crop_frames(&CANVAS, &frames);

    assert_eq!(
        cropped,
        [
            None,
            Some(Cropped {
                area: area(1, 1, 1, 1),
                data: vec![7]
            })
        ]
    );
}

#[test]
fn crop_frames_makes_a_repeated_frame_one_pixel() {
    let frames = [FrameRef::new(3, 2, &FIRST), FrameRef::new(3, 2, &FIRST)];

    let cropped = crop_frames(&CANVAS, &frames);

    assert_eq!(
        cropped[1],
        Some(Cropped {
            area: area(0, 0, 1, 1),
            data: vec![0]
        })
    );
}

#[test]
fn crop_frames_leaves_frames_whose_display_would_change() {
    let first = FrameRef::new(3, 2, &FIRST);
    let second = FrameRef::new(3, 2, &SECOND);
    let small = [7];
    let cases = [
        // The canvas isn't the first frame's pixels after it.
        [first.with_dispose_op(DisposeOp::Background), second],
        [first.with_dispose_op(DisposeOp::Previous), second],
        [first.with_blend_op(BlendOp::Over), second],
        // Blending or clearing would differ outside the cropped area.
        [first, second.with_blend_op(BlendOp::Over)],
        [first, second.with_dispose_op(DisposeOp::Background)],
        // Not whole-canvas frames.
        [first, FrameRef::new(1, 1, &small).at(1, 1)],
    ];

    for frames in cases {
        assert_eq!(crop_frames(&CANVAS, &frames), [None, None], "{frames:?}");
    }
}

#[test]
fn crop_frames_crops_a_frame_restored_afterwards() {
    let frames = [
        FrameRef::new(3, 2, &FIRST),
        FrameRef::new(3, 2, &SECOND).with_dispose_op(DisposeOp::Previous),
    ];

    assert!(crop_frames(&CANVAS, &frames)[1].is_some());
}

#[test]
fn crop_frames_leaves_a_frame_that_changed_everywhere() {
    let all = [1; 6];
    let frames = [FrameRef::new(3, 2, &FIRST), FrameRef::new(3, 2, &all)];

    assert_eq!(crop_frames(&CANVAS, &frames), [None, None]);
}
