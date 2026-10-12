#![cfg(test)]

use super::{AnimationControl, BlendOp, DisposeOp, FrameControl};
use crate::error::Error;
use crate::png::ChunkType;

fn frame() -> FrameControl {
    FrameControl {
        sequence_number: 7,
        width: 3,
        height: 2,
        x_offset: 1,
        y_offset: 4,
        delay_num: 5,
        delay_den: 20,
        dispose_op: DisposeOp::Previous,
        blend_op: BlendOp::Over,
    }
}

// ---------- AnimationControl ----------

#[test]
fn animation_control_parses_and_writes_back() {
    let data = [0, 0, 0, 3, 0, 0, 0, 2];

    let control = AnimationControl::parse(&data).unwrap();

    assert_eq!(
        control,
        AnimationControl {
            num_frames: 3,
            num_plays: 2
        }
    );
    assert_eq!(control.to_bytes(), data);
}

#[test]
fn animation_control_rejects_wrong_length() {
    assert_eq!(
        AnimationControl::parse(&[0; 9]),
        Err(Error::InvalidChunkLength {
            chunk_type: ChunkType::ACTL,
            length: 9
        })
    );
}

#[test]
fn animation_control_rejects_zero_frames() {
    assert_eq!(
        AnimationControl::parse(&[0, 0, 0, 0, 0, 0, 0, 1]),
        Err(Error::InvalidChunkData(ChunkType::ACTL))
    );
}

// ---------- FrameControl ----------

#[test]
fn frame_control_round_trips() {
    let bytes = frame().to_bytes();

    assert_eq!(FrameControl::parse(&bytes), Ok(frame()));
}

#[test]
fn frame_control_layout_matches_the_spec() {
    let bytes = frame().to_bytes();

    assert_eq!(&bytes[..4], &[0, 0, 0, 7]); // sequence number
    assert_eq!(&bytes[4..8], &[0, 0, 0, 3]); // width
    assert_eq!(&bytes[12..16], &[0, 0, 0, 1]); // x offset
    assert_eq!(&bytes[20..24], &[0, 5, 0, 20]); // delay
    assert_eq!(&bytes[24..], &[2, 1]); // dispose, blend
}

#[test]
fn frame_control_rejects_wrong_length() {
    assert_eq!(
        FrameControl::parse(&[0; 25]),
        Err(Error::InvalidChunkLength {
            chunk_type: ChunkType::FCTL,
            length: 25
        })
    );
}

#[test]
fn frame_control_rejects_zero_size() {
    let mut bytes = frame().to_bytes();
    bytes[4..8].fill(0);

    assert_eq!(
        FrameControl::parse(&bytes),
        Err(Error::InvalidChunkData(ChunkType::FCTL))
    );
}

#[test]
fn frame_control_rejects_unknown_operations() {
    let mut bad_dispose = frame().to_bytes();
    bad_dispose[24] = 3;
    let mut bad_blend = frame().to_bytes();
    bad_blend[25] = 2;

    assert_eq!(
        FrameControl::parse(&bad_dispose),
        Err(Error::InvalidChunkData(ChunkType::FCTL))
    );
    assert_eq!(
        FrameControl::parse(&bad_blend),
        Err(Error::InvalidChunkData(ChunkType::FCTL))
    );
}

#[test]
fn fits_in_checks_both_edges() {
    // x 1..4, y 4..6
    assert!(frame().fits_in(4, 6));
    assert!(!frame().fits_in(3, 6));
    assert!(!frame().fits_in(4, 5));
}

#[test]
fn fits_in_does_not_overflow() {
    let control = FrameControl {
        x_offset: u32::MAX,
        ..frame()
    };

    assert!(!control.fits_in(u32::MAX, u32::MAX));
}

#[test]
fn covers_needs_zero_offset_and_full_size() {
    let full = FrameControl {
        x_offset: 0,
        y_offset: 0,
        ..frame()
    };

    assert!(full.covers(3, 2));
    assert!(!full.covers(4, 2));
    assert!(!frame().covers(3, 2));
}

#[test]
fn delay_seconds_treats_zero_denominator_as_100() {
    let control = FrameControl {
        delay_num: 25,
        delay_den: 0,
        ..frame()
    };

    assert_eq!(control.delay_seconds(), 0.25);
    assert_eq!(frame().delay_seconds(), 0.25);
}
