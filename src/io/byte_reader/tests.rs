#![cfg(test)]

use super::ByteReader;
use crate::error::Error;

// ---------- read_u8 ----------

#[test]
fn read_u8_returns_bytes_in_order() {
    let data = [0x01, 0x7F, 0xFF];
    let mut reader = ByteReader::new(&data);

    assert_eq!(reader.read_u8(), Ok(0x01));
    assert_eq!(reader.read_u8(), Ok(0x7F));
    assert_eq!(reader.read_u8(), Ok(0xFF));
}

#[test]
fn read_u8_on_empty_input_fails() {
    let mut reader = ByteReader::new(&[]);

    assert_eq!(reader.read_u8(), Err(Error::UnexpectedEndOfInput));
}

#[test]
fn read_u8_past_end_fails() {
    let data = [0xAB];
    let mut reader = ByteReader::new(&data);

    assert_eq!(reader.read_u8(), Ok(0xAB));
    assert_eq!(reader.read_u8(), Err(Error::UnexpectedEndOfInput));
}

#[test]
fn read_u8_keeps_failing_after_end() {
    let data = [0x00];
    let mut reader = ByteReader::new(&data);

    reader.read_u8().unwrap();
    assert_eq!(reader.read_u8(), Err(Error::UnexpectedEndOfInput));
    assert_eq!(reader.read_u8(), Err(Error::UnexpectedEndOfInput));
    assert_eq!(reader.pos, 1);
}

// ---------- read_u32 ----------

#[test]
fn read_u32_is_big_endian() {
    let data = [0x12, 0x34, 0x56, 0x78];
    let mut reader = ByteReader::new(&data);

    assert_eq!(reader.read_u32(), Ok(0x1234_5678));
}

#[test]
fn read_u32_min_and_max_values() {
    let data = [0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF];
    let mut reader = ByteReader::new(&data);

    assert_eq!(reader.read_u32(), Ok(0));
    assert_eq!(reader.read_u32(), Ok(u32::MAX));
}

#[test]
fn read_u32_reads_png_chunk_length_and_type() {
    // IHDR chunk header: length = 13, type = "IHDR"
    let data = [0x00, 0x00, 0x00, 0x0D, b'I', b'H', b'D', b'R'];
    let mut reader = ByteReader::new(&data);

    assert_eq!(reader.read_u32(), Ok(13));
    assert_eq!(reader.read_u32(), Ok(u32::from_be_bytes(*b"IHDR")));
}

#[test]
fn read_u32_consumes_exactly_all_remaining_bytes() {
    let data = [0xDE, 0xAD, 0xBE, 0xEF];
    let mut reader = ByteReader::new(&data);

    assert_eq!(reader.read_u32(), Ok(0xDEAD_BEEF));
    assert_eq!(reader.read_u8(), Err(Error::UnexpectedEndOfInput));
}

#[test]
fn read_u32_on_empty_input_fails() {
    let mut reader = ByteReader::new(&[]);

    assert_eq!(reader.read_u32(), Err(Error::UnexpectedEndOfInput));
}

#[test]
fn read_u32_with_too_few_bytes_fails() {
    for len in 1..4 {
        let data = vec![0xAA; len];
        let mut reader = ByteReader::new(&data);

        assert_eq!(
            reader.read_u32(),
            Err(Error::UnexpectedEndOfInput),
            "expected failure with {len} byte(s) of input"
        );
    }
}

#[test]
fn read_u32_failure_does_not_advance_position() {
    let data = [0x01, 0x02, 0x03];
    let mut reader = ByteReader::new(&data);

    assert_eq!(reader.read_u32(), Err(Error::UnexpectedEndOfInput));
    assert_eq!(reader.pos, 0);
    assert_eq!(reader.read_u8(), Ok(0x01));
}

#[test]
fn read_u32_after_read_u8_is_offset_correctly() {
    let data = [0xFF, 0x00, 0x00, 0x01, 0x00];
    let mut reader = ByteReader::new(&data);

    assert_eq!(reader.read_u8(), Ok(0xFF));
    assert_eq!(reader.read_u32(), Ok(0x0000_0100));
    assert_eq!(reader.read_u8(), Err(Error::UnexpectedEndOfInput));
}

#[test]
fn read_u32_fails_when_fewer_than_four_bytes_remain() {
    let data = [0x00, 0x00, 0x00, 0x01, 0x02, 0x03];
    let mut reader = ByteReader::new(&data);

    assert_eq!(reader.read_u32(), Ok(1));
    assert_eq!(reader.read_u32(), Err(Error::UnexpectedEndOfInput));
    assert_eq!(reader.read_u8(), Ok(0x02));
}

// ---------- read_bytes ----------

#[test]
fn read_bytes_returns_requested_slice() {
    let data = [1, 2, 3, 4, 5];
    let mut reader = ByteReader::new(&data);

    assert_eq!(reader.read_bytes(3), Ok(&[1, 2, 3][..]));
    assert_eq!(reader.read_u8(), Ok(4));
}

#[test]
fn read_bytes_consecutive_calls_advance() {
    let data = [1, 2, 3, 4, 5, 6];
    let mut reader = ByteReader::new(&data);

    assert_eq!(reader.read_bytes(2), Ok(&[1, 2][..]));
    assert_eq!(reader.read_bytes(2), Ok(&[3, 4][..]));
    assert_eq!(reader.read_bytes(1), Ok(&[5][..]));
}

#[test]
fn read_bytes_can_read_all_remaining_bytes() {
    let data = [1, 2, 3, 4];
    let mut reader = ByteReader::new(&data);

    assert_eq!(reader.read_bytes(4), Ok(&data[..]));
    assert_eq!(reader.read_u8(), Err(Error::UnexpectedEndOfInput));
}

#[test]
fn read_bytes_can_read_remaining_bytes_after_offset() {
    let data = [1, 2, 3, 4];
    let mut reader = ByteReader::new(&data);

    reader.read_u8().unwrap();
    assert_eq!(reader.read_bytes(3), Ok(&[2, 3, 4][..]));
}

#[test]
fn read_bytes_zero_length_returns_empty_slice() {
    let data = [1, 2];
    let mut reader = ByteReader::new(&data);

    assert_eq!(reader.read_bytes(0), Ok(&[][..]));
    assert_eq!(reader.read_u8(), Ok(1));
}

#[test]
fn read_bytes_zero_length_at_end_returns_empty_slice() {
    let data = [1];
    let mut reader = ByteReader::new(&data);

    reader.read_u8().unwrap();
    assert_eq!(reader.read_bytes(0), Ok(&[][..]));
}

#[test]
fn read_bytes_zero_length_on_empty_input_returns_empty_slice() {
    let mut reader = ByteReader::new(&[]);

    assert_eq!(reader.read_bytes(0), Ok(&[][..]));
}

#[test]
fn read_bytes_more_than_available_fails() {
    let data = [1, 2, 3];
    let mut reader = ByteReader::new(&data);

    assert_eq!(reader.read_bytes(4), Err(Error::UnexpectedEndOfInput));
}

#[test]
fn read_bytes_failure_does_not_advance_position() {
    let data = [1, 2, 3];
    let mut reader = ByteReader::new(&data);

    assert_eq!(reader.read_bytes(10), Err(Error::UnexpectedEndOfInput));
    assert_eq!(reader.pos, 0);
    assert_eq!(reader.read_bytes(3), Ok(&data[..]));
}

#[test]
fn read_bytes_huge_length_fails_without_overflow_panic() {
    let data = [1, 2, 3];
    let mut reader = ByteReader::new(&data);

    reader.read_u8().unwrap();
    assert_eq!(reader.read_bytes(usize::MAX), Err(Error::UnexpectedEndOfInput));
}

#[test]
fn read_bytes_slice_outlives_reader() {
    let data = [9, 8, 7];
    let slice = {
        let mut reader = ByteReader::new(&data);
        reader.read_bytes(2).unwrap()
    };

    assert_eq!(slice, &[9, 8]);
}

// ---------- mixed ----------

#[test]
fn mixed_reads_parse_png_chunk() {
    // length(4) | type(4) | data(length) | crc(4)
    let data = [
        0x00, 0x00, 0x00, 0x03, // length = 3
        b't', b'E', b'X', b't', // type
        b'a', b'b', b'c', // data
        0x11, 0x22, 0x33, 0x44, // crc
    ];
    let mut reader = ByteReader::new(&data);

    let length = reader.read_u32().unwrap() as usize;
    assert_eq!(length, 3);
    assert_eq!(reader.read_bytes(4), Ok(&b"tEXt"[..]));
    assert_eq!(reader.read_bytes(length), Ok(&b"abc"[..]));
    assert_eq!(reader.read_u32(), Ok(0x1122_3344));
    assert_eq!(reader.read_u8(), Err(Error::UnexpectedEndOfInput));
}

// ---------- is_empty ----------

#[test]
fn is_empty_on_empty_input() {
    let reader = ByteReader::new(&[]);

    assert!(reader.is_empty());
}

#[test]
fn is_empty_becomes_true_after_reading_everything() {
    let data = [1, 2];
    let mut reader = ByteReader::new(&data);

    assert!(!reader.is_empty());
    reader.read_u8().unwrap();
    assert!(!reader.is_empty());
    reader.read_u8().unwrap();
    assert!(reader.is_empty());
}

#[test]
fn is_empty_unchanged_by_failed_read() {
    let data = [1, 2, 3];
    let mut reader = ByteReader::new(&data);

    assert!(reader.read_u32().is_err());
    assert!(!reader.is_empty());
}
