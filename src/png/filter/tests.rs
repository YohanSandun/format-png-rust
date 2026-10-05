#![cfg(test)]

use super::FilterType;
use crate::error::Error;

#[test]
fn filter_type_from_valid_values() {
    assert_eq!(FilterType::try_from(0), Ok(FilterType::None));
    assert_eq!(FilterType::try_from(1), Ok(FilterType::Sub));
    assert_eq!(FilterType::try_from(2), Ok(FilterType::Up));
    assert_eq!(FilterType::try_from(3), Ok(FilterType::Average));
    assert_eq!(FilterType::try_from(4), Ok(FilterType::Paeth));
}

#[test]
fn filter_type_from_invalid_values_fails() {
    for value in [5, 6, 128, 255] {
        assert_eq!(
            FilterType::try_from(value),
            Err(Error::InvalidFilterType(value))
        );
    }
}

#[test]
fn filter_type_round_trips_through_u8() {
    for value in 0..=4 {
        assert_eq!(FilterType::try_from(value).unwrap() as u8, value);
    }
}
