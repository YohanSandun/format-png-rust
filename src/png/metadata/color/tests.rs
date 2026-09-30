#![cfg(test)]

use super::{Chromaticities, Gamma, RenderingIntent};
use crate::error::Error;
use crate::png::ChunkType;

// ---------- gAMA ----------

#[test]
fn gamma_reads_a_big_endian_value() {
    let gamma = Gamma::parse(&45455u32.to_be_bytes()).unwrap();

    assert_eq!(gamma.scaled(), 45455);
    assert!((gamma.value() - 0.45455).abs() < 1e-9);
}

#[test]
fn gamma_reads_the_largest_value() {
    assert_eq!(Gamma::parse(&[0xFF; 4]).map(|g| g.scaled()), Ok(u32::MAX));
}

#[test]
fn gamma_rejects_zero() {
    assert_eq!(Gamma::parse(&[0; 4]), Err(Error::InvalidChunkData(ChunkType::GAMA)));
}

#[test]
fn gamma_rejects_other_lengths() {
    for length in [0, 3, 5, 8] {
        assert_eq!(
            Gamma::parse(&vec![1; length]),
            Err(Error::InvalidChunkLength { chunk_type: ChunkType::GAMA, length }),
            "{length} bytes"
        );
    }
}

// ---------- cHRM ----------

/// The sRGB primaries and D65 white point, as cHRM stores them.
const SRGB: [u32; 8] = [31270, 32900, 64000, 33000, 30000, 60000, 15000, 6000];

#[test]
fn chromaticities_read_eight_values_in_order() {
    let data: Vec<u8> = SRGB.iter().flat_map(|v| v.to_be_bytes()).collect();

    assert_eq!(
        Chromaticities::parse(&data),
        Ok(Chromaticities {
            white_x: 31270,
            white_y: 32900,
            red_x: 64000,
            red_y: 33000,
            green_x: 30000,
            green_y: 60000,
            blue_x: 15000,
            blue_y: 6000,
        })
    );
}

#[test]
fn chromaticities_reject_other_lengths() {
    for length in [0, 4, 31, 33] {
        assert_eq!(
            Chromaticities::parse(&vec![0; length]),
            Err(Error::InvalidChunkLength { chunk_type: ChunkType::CHRM, length }),
            "{length} bytes"
        );
    }
}

// ---------- sRGB ----------

#[test]
fn rendering_intent_reads_every_value() {
    let intents = [
        RenderingIntent::Perceptual,
        RenderingIntent::RelativeColorimetric,
        RenderingIntent::Saturation,
        RenderingIntent::AbsoluteColorimetric,
    ];
    for (byte, intent) in intents.into_iter().enumerate() {
        assert_eq!(RenderingIntent::parse(&[byte as u8]), Ok(intent));
    }
}

#[test]
fn rendering_intent_rejects_values_above_3() {
    for byte in [4, 255] {
        assert_eq!(RenderingIntent::parse(&[byte]), Err(Error::InvalidChunkData(ChunkType::SRGB)));
    }
}

#[test]
fn rendering_intent_rejects_other_lengths() {
    for length in [0, 2] {
        assert_eq!(
            RenderingIntent::parse(&vec![0; length]),
            Err(Error::InvalidChunkLength { chunk_type: ChunkType::SRGB, length }),
            "{length} bytes"
        );
    }
}
