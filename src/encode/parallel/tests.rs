#![cfg(test)]

use rust_deflate::{
    CompressionLevel, CompressionOptions, Strategy as CompressionStrategy, decompress_zlib,
};

use super::{
    SEGMENT_SIZE, SegmentCompression, adler32, compress_segment, compress_zlib_segments, join_zlib,
    zlib_header,
};

/// Bytes that compress somewhat, like filtered image data: small values with
/// some repetition, from a fixed pseudo-random sequence.
fn sample(len: usize) -> Vec<u8> {
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    (0..len)
        .map(|i| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            if i % 4 == 3 { 0 } else { (state % 9) as u8 }
        })
        .collect()
}

fn compression(level: u8, strategy: CompressionStrategy) -> SegmentCompression {
    SegmentCompression {
        level: CompressionLevel::new(level),
        strategy,
    }
}

fn compress(data: &[u8], level: u8, threads: usize) -> Vec<u8> {
    let mut out = Vec::new();
    compress_zlib_segments(
        data,
        compression(level, CompressionStrategy::Dynamic),
        threads,
        &mut out,
    );
    out
}

#[test]
fn adler32_matches_known_values() {
    assert_eq!(adler32(b""), 1);
    assert_eq!(adler32(b"a"), 0x0062_0062);
    assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
}

#[test]
fn adler32_matches_the_checksum_rust_deflate_writes() {
    // Long enough to take the modulus many times, with bytes up to 255.
    let data: Vec<u8> = (0..SEGMENT_SIZE + 12_345)
        .map(|i| (i * 7 + i / 1000) as u8)
        .collect();

    let zlib = rust_deflate::compress_zlib(&data);

    assert_eq!(adler32(&data).to_be_bytes(), zlib[zlib.len() - 4..]);
}

#[test]
fn zlib_header_matches_rust_deflate_for_every_level_and_strategy() {
    let strategies = [
        CompressionStrategy::Dynamic,
        CompressionStrategy::Fixed,
        CompressionStrategy::Stored,
    ];

    for level in 0..=9 {
        for strategy in strategies {
            let options = CompressionOptions::new()
                .level(CompressionLevel::new(level))
                .strategy(strategy);
            let expected = rust_deflate::compress_zlib_with(b"x", options);

            assert_eq!(
                zlib_header(CompressionLevel::new(level), strategy),
                expected[..2],
                "level {level}, {strategy:?}"
            );
        }
    }
}

#[test]
fn segments_decompress_to_the_data() {
    // Several whole segments and a partial one.
    let data = sample(2 * SEGMENT_SIZE + 54_321);

    for level in [1, 6] {
        assert_eq!(
            decompress_zlib(&compress(&data, level, 4)).unwrap(),
            data,
            "level {level}"
        );
    }
}

#[test]
fn output_is_the_same_whatever_the_thread_count() {
    let data = sample(3 * SEGMENT_SIZE + 1);
    let one = compress(&data, 1, 1);

    for threads in [2, 3, 8, 64] {
        assert_eq!(compress(&data, 1, threads), one, "{threads} threads");
    }
}

#[test]
fn data_of_whole_segments_ends_with_a_final_block() {
    let data = sample(2 * SEGMENT_SIZE);

    assert_eq!(decompress_zlib(&compress(&data, 6, 2)).unwrap(), data);
}

#[test]
fn every_strategy_round_trips() {
    let data = sample(2 * SEGMENT_SIZE + 10);

    for strategy in [
        CompressionStrategy::Dynamic,
        CompressionStrategy::Fixed,
        CompressionStrategy::Stored,
    ] {
        let mut out = Vec::new();
        compress_zlib_segments(&data, compression(6, strategy), 3, &mut out);

        assert_eq!(decompress_zlib(&out).unwrap(), data, "{strategy:?}");
    }
}

#[test]
fn segments_cost_little_compression() {
    let data = sample(2 * SEGMENT_SIZE);

    let whole = rust_deflate::compress_zlib(&data).len();
    let segmented = compress(&data, 6, 4).len();

    assert!(
        segmented as f64 <= whole as f64 * 1.01,
        "{segmented} bytes vs {whole} in one stream"
    );
}

#[test]
fn compress_zlib_segments_replaces_old_contents() {
    let data = sample(SEGMENT_SIZE + 1);
    let mut out = vec![0xEE; 100];

    compress_zlib_segments(
        &data,
        compression(6, CompressionStrategy::Dynamic),
        2,
        &mut out,
    );

    assert_eq!(
        &out[..2],
        zlib_header(CompressionLevel::MEDIUM, CompressionStrategy::Dynamic)
    );
    assert_eq!(decompress_zlib(&out).unwrap(), data);
}

#[test]
fn no_segments_join_to_an_empty_stream() {
    let mut out = Vec::new();

    join_zlib(
        &[],
        compression(6, CompressionStrategy::Dynamic),
        &[] as &[Vec<u8>],
        &mut out,
    );

    assert_eq!(decompress_zlib(&out).unwrap(), b"");
}

#[test]
fn compress_segment_gives_the_threads_their_segments() {
    let data = sample(2 * SEGMENT_SIZE + 99);
    let settings = compression(6, CompressionStrategy::Dynamic);
    let segments: Vec<Vec<u8>> = data
        .chunks(SEGMENT_SIZE)
        .map(|segment| compress_segment(segment, settings))
        .collect();
    let mut joined = Vec::new();

    join_zlib(&data, settings, &segments, &mut joined);

    assert_eq!(joined, compress(&data, 6, 3));
}

#[test]
fn compress_segment_depends_only_on_its_input() {
    let segment = sample(SEGMENT_SIZE);
    let settings = compression(4, CompressionStrategy::Dynamic);

    // A different segment in between must leave no state behind.
    let first = compress_segment(&segment, settings);
    let _ = compress_segment(&sample(1000), settings);

    assert_eq!(compress_segment(&segment, settings), first);
}
