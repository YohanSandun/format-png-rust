#![cfg(test)]

use super::CodingIndependentCodePoints;
use crate::error::Error;
use crate::png::ChunkType;

#[test]
fn parse_reads_every_field() {
    // BT.2100 PQ, full range: the usual HDR PNG.
    assert_eq!(
        CodingIndependentCodePoints::parse(&[9, 16, 0, 1]),
        Ok(CodingIndependentCodePoints { color_primaries: 9, transfer_function: 16, matrix_coefficients: 0, full_range: true })
    );
}

#[test]
fn parse_reads_narrow_range() {
    assert_eq!(CodingIndependentCodePoints::parse(&[1, 13, 0, 0]).map(|c| c.full_range), Ok(false));
}

#[test]
fn parse_keeps_values_it_has_no_name_for() {
    // H.273 values are kept as numbers, so unassigned ones aren't rejected.
    assert!(CodingIndependentCodePoints::parse(&[200, 250, 0, 1]).is_ok());
}

#[test]
fn parse_rejects_matrix_coefficients_other_than_rgb() {
    for matrix in [1, 9, 255] {
        assert_eq!(
            CodingIndependentCodePoints::parse(&[9, 16, matrix, 1]),
            Err(Error::InvalidChunkData(ChunkType::CICP)),
            "{matrix}"
        );
    }
}

#[test]
fn parse_rejects_full_range_flags_other_than_0_or_1() {
    assert_eq!(CodingIndependentCodePoints::parse(&[9, 16, 0, 2]), Err(Error::InvalidChunkData(ChunkType::CICP)));
}

#[test]
fn parse_rejects_other_lengths() {
    for length in [0, 3, 5] {
        assert_eq!(
            CodingIndependentCodePoints::parse(&vec![0; length]),
            Err(Error::InvalidChunkLength { chunk_type: ChunkType::CICP, length }),
            "{length} bytes"
        );
    }
}
