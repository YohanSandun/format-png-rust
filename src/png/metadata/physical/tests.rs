#![cfg(test)]

use super::{PhysicalDimensions, Unit};
use crate::error::Error;
use crate::png::ChunkType;

fn phys(x: u32, y: u32, unit: u8) -> Vec<u8> {
    let mut data = x.to_be_bytes().to_vec();
    data.extend_from_slice(&y.to_be_bytes());
    data.push(unit);
    data
}

#[test]
fn parse_reads_pixels_per_meter() {
    assert_eq!(
        PhysicalDimensions::parse(&phys(2835, 3780, 1)),
        Ok(PhysicalDimensions { x: 2835, y: 3780, unit: Unit::Meter })
    );
}

#[test]
fn parse_reads_an_aspect_ratio() {
    assert_eq!(
        PhysicalDimensions::parse(&phys(2, 1, 0)),
        Ok(PhysicalDimensions { x: 2, y: 1, unit: Unit::Unknown })
    );
}

#[test]
fn parse_rejects_an_unknown_unit() {
    for unit in [2, 255] {
        assert_eq!(PhysicalDimensions::parse(&phys(1, 1, unit)), Err(Error::InvalidChunkData(ChunkType::PHYS)));
    }
}

#[test]
fn parse_rejects_other_lengths() {
    for length in [0, 8, 10] {
        assert_eq!(
            PhysicalDimensions::parse(&vec![0; length]),
            Err(Error::InvalidChunkLength { chunk_type: ChunkType::PHYS, length }),
            "{length} bytes"
        );
    }
}

#[test]
fn dots_per_inch_converts_meters() {
    // 2835 pixels per meter is 72 dpi, give or take rounding.
    let (x, y) = PhysicalDimensions { x: 2835, y: 5669, unit: Unit::Meter }.dots_per_inch().unwrap();

    assert!((x - 72.0).abs() < 0.01, "{x}");
    assert!((y - 144.0).abs() < 0.01, "{y}");
}

#[test]
fn dots_per_inch_needs_a_unit() {
    assert_eq!(PhysicalDimensions { x: 1, y: 1, unit: Unit::Unknown }.dots_per_inch(), None);
}
