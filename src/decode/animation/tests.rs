#![cfg(test)]

use super::read_animation;
use crate::error::Error;
use crate::png::{
    AnimationControl, BlendOp, Chunk, ChunkType, ColorType, DisposeOp, FrameControl, ImageHeader,
    Interlace,
};

const HEADER: ImageHeader = ImageHeader {
    width: 4,
    height: 3,
    bit_depth: 8,
    color_type: ColorType::Rgba,
    interlace: Interlace::None,
};

/// Builds chunks from (type, data) pairs. The CRC isn't used here.
fn chunks<'a>(list: &[(ChunkType, &'a [u8])]) -> Vec<Chunk<'a>> {
    list.iter()
        .map(|&(chunk_type, data)| Chunk::new(chunk_type, data, 0))
        .collect()
}

fn actl(num_frames: u32) -> [u8; 8] {
    AnimationControl {
        num_frames,
        num_plays: 0,
    }
    .to_bytes()
}

fn fctl(sequence_number: u32, x: u32, y: u32, width: u32, height: u32) -> [u8; 26] {
    FrameControl {
        sequence_number,
        width,
        height,
        x_offset: x,
        y_offset: y,
        delay_num: 1,
        delay_den: 10,
        dispose_op: DisposeOp::None,
        blend_op: BlendOp::Source,
    }
    .to_bytes()
}

fn full(sequence_number: u32) -> [u8; 26] {
    fctl(sequence_number, 0, 0, 4, 3)
}

fn fdat(sequence_number: u32, data: &[u8]) -> Vec<u8> {
    let mut chunk = sequence_number.to_be_bytes().to_vec();
    chunk.extend_from_slice(data);
    chunk
}

#[test]
fn default_image_as_first_frame() {
    let (a, f0, f1, d2) = (actl(2), full(0), fctl(1, 1, 1, 2, 2), fdat(2, b"xyz"));
    let list = chunks(&[
        (ChunkType::ACTL, &a),
        (ChunkType::FCTL, &f0),
        (ChunkType::IDAT, b"ab"),
        (ChunkType::IDAT, b"cd"),
        (ChunkType::FCTL, &f1),
        (ChunkType::FDAT, &d2),
        (ChunkType::IEND, b""),
    ]);

    let layout = read_animation(&HEADER, &list).unwrap();

    assert!(layout.default_image_is_frame);
    assert_eq!(layout.control.num_frames, 2);
    assert_eq!(layout.frames.len(), 2);
    assert_eq!(layout.frames[0].data, [b"ab", b"cd"]);
    assert_eq!(layout.frames[1].data, [b"xyz"]);
    assert_eq!(layout.frames[1].control.x_offset, 1);
}

#[test]
fn default_image_outside_the_animation() {
    let (a, f0, d1) = (actl(1), fctl(0, 0, 0, 1, 1), fdat(1, b"frame"));
    let list = chunks(&[
        (ChunkType::ACTL, &a),
        (ChunkType::IDAT, b"default"),
        (ChunkType::FCTL, &f0),
        (ChunkType::FDAT, &d1),
        (ChunkType::IEND, b""),
    ]);

    let layout = read_animation(&HEADER, &list).unwrap();

    assert!(!layout.default_image_is_frame);
    assert_eq!(layout.frames.len(), 1);
    assert_eq!(layout.frames[0].data, [b"frame"]);
}

#[test]
fn not_animated_without_actl() {
    let list = chunks(&[(ChunkType::IDAT, b"x"), (ChunkType::IEND, b"")]);

    assert_eq!(
        read_animation(&HEADER, &list).map(|_| ()),
        Err(Error::NotAnimated)
    );
}

#[test]
fn rejects_actl_after_image_data() {
    let a = actl(1);
    let list = chunks(&[(ChunkType::IDAT, b"x"), (ChunkType::ACTL, &a)]);

    assert_eq!(
        read_animation(&HEADER, &list).map(|_| ()),
        Err(Error::MisplacedChunk(ChunkType::ACTL))
    );
}

#[test]
fn rejects_duplicate_actl() {
    let a = actl(1);
    let list = chunks(&[(ChunkType::ACTL, &a), (ChunkType::ACTL, &a)]);

    assert_eq!(
        read_animation(&HEADER, &list).map(|_| ()),
        Err(Error::DuplicateChunk(ChunkType::ACTL))
    );
}

#[test]
fn rejects_sequence_out_of_order() {
    let (a, f0, f1, d3) = (actl(2), full(0), fctl(1, 0, 0, 1, 1), fdat(3, b"x"));
    let list = chunks(&[
        (ChunkType::ACTL, &a),
        (ChunkType::FCTL, &f0),
        (ChunkType::IDAT, b"x"),
        (ChunkType::FCTL, &f1),
        (ChunkType::FDAT, &d3),
    ]);

    assert_eq!(
        read_animation(&HEADER, &list).map(|_| ()),
        Err(Error::FrameSequenceMismatch {
            expected: 2,
            actual: 3
        })
    );
}

#[test]
fn rejects_first_frame_smaller_than_canvas() {
    let (a, f0) = (actl(1), fctl(0, 0, 0, 1, 1));
    let list = chunks(&[(ChunkType::ACTL, &a), (ChunkType::FCTL, &f0)]);

    assert_eq!(
        read_animation(&HEADER, &list).map(|_| ()),
        Err(Error::InvalidChunkData(ChunkType::FCTL))
    );
}

#[test]
fn rejects_two_fctl_before_image_data() {
    let (a, f0, f1) = (actl(2), full(0), full(1));
    let list = chunks(&[
        (ChunkType::ACTL, &a),
        (ChunkType::FCTL, &f0),
        (ChunkType::FCTL, &f1),
    ]);

    assert_eq!(
        read_animation(&HEADER, &list).map(|_| ()),
        Err(Error::MisplacedChunk(ChunkType::FCTL))
    );
}

#[test]
fn rejects_frame_off_the_canvas() {
    let (a, f0, f1) = (actl(2), full(0), fctl(1, 3, 0, 2, 1));
    let list = chunks(&[
        (ChunkType::ACTL, &a),
        (ChunkType::FCTL, &f0),
        (ChunkType::IDAT, b"x"),
        (ChunkType::FCTL, &f1),
    ]);

    assert_eq!(
        read_animation(&HEADER, &list).map(|_| ()),
        Err(Error::FrameOutOfBounds)
    );
}

#[test]
fn rejects_fdat_without_its_own_fctl() {
    let (a, f0, d1) = (actl(1), full(0), fdat(1, b"x"));
    let list = chunks(&[
        (ChunkType::ACTL, &a),
        (ChunkType::FCTL, &f0),
        (ChunkType::IDAT, b"x"),
        (ChunkType::FDAT, &d1),
    ]);

    assert_eq!(
        read_animation(&HEADER, &list).map(|_| ()),
        Err(Error::MisplacedChunk(ChunkType::FDAT))
    );
}

#[test]
fn rejects_short_fdat() {
    let (a, f0, f1) = (actl(2), full(0), fctl(1, 0, 0, 1, 1));
    let list = chunks(&[
        (ChunkType::ACTL, &a),
        (ChunkType::FCTL, &f0),
        (ChunkType::IDAT, b"x"),
        (ChunkType::FCTL, &f1),
        (ChunkType::FDAT, b"abc"),
    ]);

    assert_eq!(
        read_animation(&HEADER, &list).map(|_| ()),
        Err(Error::InvalidChunkLength {
            chunk_type: ChunkType::FDAT,
            length: 3
        })
    );
}

#[test]
fn rejects_frame_without_data() {
    let (a, f0, f1) = (actl(2), full(0), fctl(1, 0, 0, 1, 1));
    let list = chunks(&[
        (ChunkType::ACTL, &a),
        (ChunkType::FCTL, &f0),
        (ChunkType::IDAT, b"x"),
        (ChunkType::FCTL, &f1),
        (ChunkType::IEND, b""),
    ]);

    assert_eq!(
        read_animation(&HEADER, &list).map(|_| ()),
        Err(Error::MissingFrameData)
    );
}

#[test]
fn rejects_wrong_frame_count() {
    let (a, f0) = (actl(3), full(0));
    let list = chunks(&[
        (ChunkType::ACTL, &a),
        (ChunkType::FCTL, &f0),
        (ChunkType::IDAT, b"x"),
        (ChunkType::IEND, b""),
    ]);

    assert_eq!(
        read_animation(&HEADER, &list).map(|_| ()),
        Err(Error::FrameCountMismatch {
            expected: 3,
            actual: 1
        })
    );
}
