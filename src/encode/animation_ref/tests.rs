#![cfg(test)]

use super::{AnimationRef, FrameRef};
use crate::png::{BlendOp, ColorType, DisposeOp, ImageHeader, Interlace, Palette};

const HEADER: ImageHeader = ImageHeader {
    width: 2,
    height: 2,
    bit_depth: 8,
    color_type: ColorType::Grayscale,
    interlace: Interlace::None,
};

#[test]
fn frame_defaults() {
    let control = FrameRef::new(2, 1, &[1, 2]).control(5);

    assert_eq!(control.sequence_number, 5);
    assert_eq!((control.width, control.height), (2, 1));
    assert_eq!((control.x_offset, control.y_offset), (0, 0));
    assert_eq!((control.delay_num, control.delay_den), (1, 10));
    assert_eq!(control.dispose_op, DisposeOp::None);
    assert_eq!(control.blend_op, BlendOp::Source);
}

#[test]
fn frame_builders_set_control() {
    let control = FrameRef::new(1, 1, &[0])
        .at(1, 1)
        .with_delay(3, 4)
        .with_dispose_op(DisposeOp::Background)
        .with_blend_op(BlendOp::Over)
        .control(0);

    assert_eq!((control.x_offset, control.y_offset), (1, 1));
    assert_eq!((control.delay_num, control.delay_den), (3, 4));
    assert_eq!(control.dispose_op, DisposeOp::Background);
    assert_eq!(control.blend_op, BlendOp::Over);
}

#[test]
fn frame_header_takes_format_from_canvas() {
    let header = FrameRef::new(1, 2, &[0, 0]).header(&HEADER);

    assert_eq!((header.width, header.height), (1, 2));
    assert_eq!(header.color_type, ColorType::Grayscale);
    assert_eq!(header.bit_depth, 8);
}

#[test]
fn default_image_is_first_frame_unless_given() {
    let pixels = [1, 2, 3, 4];
    let default = [9, 9, 9, 9];
    let frames = [FrameRef::new(2, 2, &pixels)];

    let animation = AnimationRef::new(HEADER, &frames);
    assert_eq!(animation.default_image(), None);
    assert_eq!(animation.image().data(), pixels);

    let animation = animation.with_default_image(&default);
    assert_eq!(animation.default_image(), Some(&default[..]));
    assert_eq!(animation.image().data(), default);
}

#[test]
fn builders_set_shared_chunks() {
    let palette = Palette::from_colors(&[[0, 0, 0]]).unwrap();
    let frames = [FrameRef::new(2, 2, &[0; 4])];

    let animation = AnimationRef::new(HEADER, &frames)
        .with_num_plays(2)
        .with_palette(&palette);

    assert_eq!(animation.num_plays(), 2);
    assert_eq!(animation.image().palette(), Some(&palette));
    assert_eq!(animation.frames().len(), 1);
}
