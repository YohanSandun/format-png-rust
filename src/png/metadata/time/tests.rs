#![cfg(test)]

use super::Time;
use crate::error::Error;
use crate::png::ChunkType;

fn time_data(year: u16, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> Vec<u8> {
    let mut data = year.to_be_bytes().to_vec();
    data.extend_from_slice(&[month, day, hour, minute, second]);
    data
}

#[test]
fn parse_reads_every_field() {
    assert_eq!(
        Time::parse(&time_data(2026, 9, 30, 14, 5, 59)),
        Ok(Time { year: 2026, month: 9, day: 30, hour: 14, minute: 5, second: 59 })
    );
}

#[test]
fn parse_accepts_the_range_limits() {
    assert!(Time::parse(&time_data(0, 1, 1, 0, 0, 0)).is_ok());
    assert!(Time::parse(&time_data(u16::MAX, 12, 31, 23, 59, 60)).is_ok());
}

#[test]
fn parse_does_not_check_the_day_against_the_month() {
    assert!(Time::parse(&time_data(2026, 2, 31, 0, 0, 0)).is_ok());
}

#[test]
fn parse_rejects_fields_out_of_range() {
    for data in [
        time_data(2026, 0, 1, 0, 0, 0),
        time_data(2026, 13, 1, 0, 0, 0),
        time_data(2026, 1, 0, 0, 0, 0),
        time_data(2026, 1, 32, 0, 0, 0),
        time_data(2026, 1, 1, 24, 0, 0),
        time_data(2026, 1, 1, 0, 60, 0),
        time_data(2026, 1, 1, 0, 0, 61),
    ] {
        assert_eq!(Time::parse(&data), Err(Error::InvalidChunkData(ChunkType::TIME)), "{data:?}");
    }
}

#[test]
fn parse_rejects_other_lengths() {
    for length in [0, 6, 8] {
        assert_eq!(
            Time::parse(&vec![1; length]),
            Err(Error::InvalidChunkLength { chunk_type: ChunkType::TIME, length }),
            "{length} bytes"
        );
    }
}

#[test]
fn times_order_chronologically() {
    let earlier = Time { year: 2025, month: 12, day: 31, hour: 23, minute: 59, second: 59 };
    let later = Time { year: 2026, month: 1, day: 1, hour: 0, minute: 0, second: 0 };

    assert!(earlier < later);
}
