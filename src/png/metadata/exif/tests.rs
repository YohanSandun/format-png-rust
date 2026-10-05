#![cfg(test)]

use super::{Exif, ExifByteOrder};
use crate::error::Error;
use crate::png::ChunkType;

/// A TIFF header in big-endian order, with its first directory right after it.
const BIG_ENDIAN: [u8; 8] = [b'M', b'M', 0, 42, 0, 0, 0, 8];
/// The same header in little-endian order.
const LITTLE_ENDIAN: [u8; 8] = [b'I', b'I', 42, 0, 8, 0, 0, 0];

#[test]
fn parse_reads_big_endian_data() {
    let mut data = BIG_ENDIAN.to_vec();
    data.extend_from_slice(&[0, 0]); // an empty directory

    let exif = Exif::parse(&data).unwrap();

    assert_eq!(exif.byte_order(), ExifByteOrder::BigEndian);
    assert_eq!(exif.data(), data);
}

#[test]
fn parse_reads_little_endian_data() {
    let exif = Exif::parse(&LITTLE_ENDIAN).unwrap();

    assert_eq!(exif.byte_order(), ExifByteOrder::LittleEndian);
    assert_eq!(exif.data(), LITTLE_ENDIAN);
}

#[test]
fn parse_rejects_data_shorter_than_the_tiff_header() {
    for length in [0, 4, 7] {
        assert_eq!(
            Exif::parse(&BIG_ENDIAN[..length]),
            Err(Error::InvalidChunkLength {
                chunk_type: ChunkType::EXIF,
                length
            }),
            "{length} bytes"
        );
    }
}

#[test]
fn parse_rejects_unknown_byte_orders() {
    for header in [*b"MI\0*\0\0\0\x08", *b"Exif\0\0MM", [0; 8]] {
        assert_eq!(
            Exif::parse(&header),
            Err(Error::InvalidChunkData(ChunkType::EXIF)),
            "{header:?}"
        );
    }
}

#[test]
fn parse_rejects_42_in_the_wrong_byte_order() {
    let mixed = [b'M', b'M', 42, 0, 0, 0, 0, 8];

    assert_eq!(
        Exif::parse(&mixed),
        Err(Error::InvalidChunkData(ChunkType::EXIF))
    );
}
