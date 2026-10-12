#![cfg(test)]

use super::{Compositor, blend_over};
use crate::error::Error;
use crate::png::{
    BlendOp, ColorType, DisposeOp, Frame, FrameControl, Image, ImageChunks, ImageHeader, Interlace,
};

const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const CLEAR: [u8; 4] = [0, 0, 0, 0];

/// An RGBA frame of `width × height` at `(x, y)`, every pixel `color`.
fn frame(x: u32, y: u32, width: u32, height: u32, color: [u8; 4]) -> Frame {
    let header = ImageHeader {
        width,
        height,
        bit_depth: 8,
        color_type: ColorType::Rgba,
        interlace: Interlace::None,
    };
    let data = color.repeat((width * height) as usize);
    let control = FrameControl {
        sequence_number: 0,
        width,
        height,
        x_offset: x,
        y_offset: y,
        delay_num: 1,
        delay_den: 10,
        dispose_op: DisposeOp::None,
        blend_op: BlendOp::Source,
    };
    Frame::new(
        control,
        Image::new(header, width as usize * 4, data, ImageChunks::default()),
    )
}

fn with(frame: Frame, dispose_op: DisposeOp, blend_op: BlendOp) -> Frame {
    let control = FrameControl {
        dispose_op,
        blend_op,
        ..*frame.control()
    };
    Frame::new(control, frame.into_image())
}

fn pixels(canvas: &[u8]) -> Vec<[u8; 4]> {
    canvas.as_chunks::<4>().0.to_vec()
}

// ---------- render ----------

#[test]
fn canvas_starts_transparent() {
    let compositor = Compositor::new(2, 1).unwrap();

    assert_eq!(compositor.canvas(), &[0; 8]);
}

#[test]
fn render_draws_frame_at_its_offset() {
    let mut compositor = Compositor::new(3, 1).unwrap();

    let canvas = compositor.render(&frame(1, 0, 1, 1, RED)).unwrap();

    assert_eq!(pixels(canvas), [CLEAR, RED, CLEAR]);
}

#[test]
fn dispose_none_keeps_the_frame() {
    let mut compositor = Compositor::new(2, 1).unwrap();
    compositor.render(&frame(0, 0, 1, 1, RED)).unwrap();

    let canvas = compositor.render(&frame(1, 0, 1, 1, BLUE)).unwrap();

    assert_eq!(pixels(canvas), [RED, BLUE]);
}

#[test]
fn dispose_background_clears_the_frame_before_the_next() {
    let mut compositor = Compositor::new(2, 1).unwrap();
    let first = with(
        frame(0, 0, 1, 1, RED),
        DisposeOp::Background,
        BlendOp::Source,
    );
    compositor.render(&first).unwrap();

    let canvas = compositor.render(&frame(1, 0, 1, 1, BLUE)).unwrap();

    assert_eq!(pixels(canvas), [CLEAR, BLUE]);
}

#[test]
fn dispose_previous_restores_what_was_under_the_frame() {
    let mut compositor = Compositor::new(2, 1).unwrap();
    compositor.render(&frame(0, 0, 2, 1, RED)).unwrap();
    let blue = with(
        frame(0, 0, 1, 1, BLUE),
        DisposeOp::Previous,
        BlendOp::Source,
    );
    assert_eq!(pixels(compositor.render(&blue).unwrap()), [BLUE, RED]);

    let canvas = compositor.render(&frame(1, 0, 1, 1, RED)).unwrap();

    assert_eq!(pixels(canvas), [RED, RED]);
}

#[test]
fn dispose_previous_on_first_frame_acts_as_background() {
    let mut compositor = Compositor::new(1, 1).unwrap();
    let first = with(frame(0, 0, 1, 1, RED), DisposeOp::Previous, BlendOp::Source);
    compositor.render(&first).unwrap();

    let half_blue = [0, 0, 255, 128];
    let canvas = compositor
        .render(&with(
            frame(0, 0, 1, 1, half_blue),
            DisposeOp::None,
            BlendOp::Over,
        ))
        .unwrap();

    assert_eq!(pixels(canvas), [half_blue]);
}

#[test]
fn blend_source_replaces_alpha_too() {
    let mut compositor = Compositor::new(1, 1).unwrap();
    compositor.render(&frame(0, 0, 1, 1, RED)).unwrap();

    let canvas = compositor.render(&frame(0, 0, 1, 1, CLEAR)).unwrap();

    assert_eq!(pixels(canvas), [CLEAR]);
}

#[test]
fn blend_over_keeps_what_shows_through() {
    let mut compositor = Compositor::new(1, 1).unwrap();
    compositor.render(&frame(0, 0, 1, 1, RED)).unwrap();

    let over = with(frame(0, 0, 1, 1, CLEAR), DisposeOp::None, BlendOp::Over);
    let canvas = compositor.render(&over).unwrap();

    assert_eq!(pixels(canvas), [RED]);
}

#[test]
fn reset_clears_the_canvas_and_first_frame_state() {
    let mut compositor = Compositor::new(1, 1).unwrap();
    compositor.render(&frame(0, 0, 1, 1, RED)).unwrap();

    compositor.reset();

    assert_eq!(compositor.canvas(), &CLEAR);
}

#[test]
fn render_rejects_frame_off_the_canvas() {
    let mut compositor = Compositor::new(2, 2).unwrap();

    assert_eq!(
        compositor.render(&frame(1, 1, 2, 1, RED)).map(|_| ()),
        Err(Error::FrameOutOfBounds)
    );
}

// ---------- blend_over ----------

#[test]
fn blend_over_opaque_source_replaces() {
    let mut dst = RED;
    blend_over(&mut dst, &BLUE);
    assert_eq!(dst, BLUE);
}

#[test]
fn blend_over_transparent_source_keeps() {
    let mut dst = RED;
    blend_over(&mut dst, &[0, 255, 0, 0]);
    assert_eq!(dst, RED);
}

#[test]
fn blend_over_half_on_opaque_mixes_evenly() {
    let mut dst = [0, 0, 0, 255];
    blend_over(&mut dst, &[255, 255, 255, 128]);
    // 255 × 128/255 = 128
    assert_eq!(dst, [128, 128, 128, 255]);
}

#[test]
fn blend_over_on_transparent_keeps_source_color() {
    let mut dst = CLEAR;
    blend_over(&mut dst, &[10, 20, 30, 77]);
    assert_eq!(dst, [10, 20, 30, 77]);
}

#[test]
fn blend_over_half_on_half_combines_alpha() {
    let mut dst = [0, 0, 255, 128];
    blend_over(&mut dst, &[255, 0, 0, 128]);
    // alpha: 128 + 128 × 127/255 ≈ 192; red has the larger weight
    assert_eq!(dst[3], 192);
    assert!(dst[0] > dst[2]);
}
