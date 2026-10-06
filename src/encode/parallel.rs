//! Compresses the image data in segments: on several threads, for
//! [`Threads::Auto`] and [`Threads::Count`], or anywhere the caller likes, with
//! [`Encoder::prepare`] and [`compress_segment`].
//!
//! The filtered rows are split into segments of [`SEGMENT_SIZE`] bytes, and each
//! is compressed on its own as raw DEFLATE ending in a sync flush, which ends it
//! on a byte boundary without ending the stream. Joined in order, with one zlib
//! header in front, an empty final block after them, and the Adler-32 of all the
//! data at the end, they make one zlib stream.
//!
//! Each segment starts with no history, so matches can't reach back into the
//! segment before it. That makes the output slightly larger, about 0.1%, in
//! exchange for compressing segments in any order on any thread. The segments
//! are a fixed size, so the output doesn't depend on how many threads ran.
//!
//! [`Threads::Auto`]: crate::Threads::Auto
//! [`Threads::Count`]: crate::Threads::Count
//! [`Encoder::prepare`]: crate::Encoder::prepare

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use rust_deflate::{
    CompressionLevel, CompressionOptions, Strategy as CompressionStrategy, StreamCompressor,
};

/// How much data each segment holds, except the last, which may hold less.
/// Smaller segments spread better across threads, and larger ones lose less
/// compression at their boundaries: at 1 MiB, about 0.1%.
pub(crate) const SEGMENT_SIZE: usize = 1 << 20;

/// An empty final block with fixed Huffman codes: the bits 1 (final), 01
/// (fixed codes) and the 7-bit end-of-block code, 0. It ends the stream after
/// segments that each end in a sync flush, as zlib does when finishing after a
/// flush.
const FINAL_BLOCK: [u8; 2] = [0x03, 0x00];

/// The largest number to add up before taking the Adler-32 sums modulo
/// `ADLER_MODULUS` without overflowing a `u32`.
const ADLER_BLOCK: usize = 5552;
const ADLER_MODULUS: u32 = 65521;

/// How the segments of a [`PreparedPng`](crate::PreparedPng) are compressed:
/// the encoder's [`EncodeOptions::compression`](crate::EncodeOptions::compression)
/// and [`EncodeOptions::compression_strategy`](crate::EncodeOptions::compression_strategy).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegmentCompression {
    /// How hard to compress.
    pub level: CompressionLevel,
    /// Which kind of DEFLATE blocks to write.
    pub strategy: CompressionStrategy,
}

/// Compresses one segment of a [`PreparedPng`](crate::PreparedPng), for
/// [`PreparedPng::finish`](crate::PreparedPng::finish) to join with the others.
///
/// It's a pure function, safe to call on any thread, in any order: the result
/// depends only on `segment` and `compression`. Pass it what
/// [`PreparedPng::segment`](crate::PreparedPng::segment) and
/// [`PreparedPng::compression`](crate::PreparedPng::compression) return.
#[must_use]
pub fn compress_segment(segment: &[u8], compression: SegmentCompression) -> Vec<u8> {
    let mut stream = StreamCompressor::deflate_with(options(compression));
    let mut out = Vec::new();
    compress_segment_with(&mut stream, segment, &mut out);
    out
}

/// Compresses `segment` into `out`, reusing `stream`, which was made with the
/// segment's options.
fn compress_segment_with(stream: &mut StreamCompressor, segment: &[u8], out: &mut Vec<u8>) {
    stream.reset();
    stream.push(segment, out);
    stream.flush(out);
}

fn options(compression: SegmentCompression) -> CompressionOptions {
    CompressionOptions::new()
        .level(compression.level)
        .strategy(compression.strategy)
}

/// Compresses `data` as a zlib stream into `out`, replacing its contents, with
/// segments of `SEGMENT_SIZE` compressed on up to `threads` threads.
///
/// The calling thread compresses segments too, so `threads` counts it. Other
/// threads are started only if the platform has them: where starting one fails,
/// as on `wasm32-unknown-unknown`, the calling thread does all the work, and the
/// output is the same.
pub(crate) fn compress_zlib_segments(
    data: &[u8],
    compression: SegmentCompression,
    threads: usize,
    out: &mut Vec<u8>,
) {
    let segments: Vec<&[u8]> = data.chunks(SEGMENT_SIZE).collect();
    let compressed: Vec<Mutex<Vec<u8>>> = segments.iter().map(|_| Mutex::new(Vec::new())).collect();
    let next = AtomicUsize::new(0);

    // Takes the next segment until none are left, so faster threads take more.
    let work = || {
        let mut stream = StreamCompressor::deflate_with(options(compression));
        loop {
            let i = next.fetch_add(1, Ordering::Relaxed);
            let Some(segment) = segments.get(i) else {
                break;
            };

            let mut segment_out = Vec::new();
            compress_segment_with(&mut stream, segment, &mut segment_out);
            *compressed[i]
                .lock()
                .expect("no thread panics while holding it") = segment_out;
        }
    };

    std::thread::scope(|scope| {
        for _ in 1..threads.min(segments.len()) {
            if std::thread::Builder::new()
                .spawn_scoped(scope, work)
                .is_err()
            {
                break;
            }
        }
        work();
    });

    let compressed: Vec<Vec<u8>> = compressed
        .into_iter()
        .map(|segment| {
            segment
                .into_inner()
                .expect("no thread panics while holding it")
        })
        .collect();
    join_zlib(data, compression, &compressed, out);
}

/// Writes the zlib stream for `data` from its compressed segments, in order:
/// the header, the segments, an empty final block and the Adler-32 of `data`.
/// `out`'s old contents are replaced.
pub(crate) fn join_zlib(
    data: &[u8],
    compression: SegmentCompression,
    compressed: &[impl AsRef<[u8]>],
    out: &mut Vec<u8>,
) {
    out.clear();
    out.reserve(
        compressed
            .iter()
            .map(|segment| segment.as_ref().len())
            .sum::<usize>()
            + 8,
    );
    out.extend_from_slice(&zlib_header(compression.level, compression.strategy));
    for segment in compressed {
        out.extend_from_slice(segment.as_ref());
    }
    out.extend_from_slice(&FINAL_BLOCK);
    out.extend_from_slice(&adler32(data).to_be_bytes());
}

/// The two bytes that start a zlib stream: DEFLATE with a 32 KB window, no
/// preset dictionary, and the compression level as a hint, as `rust-deflate`
/// writes them.
pub(crate) fn zlib_header(level: CompressionLevel, strategy: CompressionStrategy) -> [u8; 2] {
    const CMF: u8 = 0x78;
    let flevel: u8 = match (strategy, level.get()) {
        (CompressionStrategy::Stored, _) | (_, 0..=1) => 0,
        (_, 2..=5) => 1,
        (_, 6) => 2,
        _ => 3,
    };
    let flg = flevel << 6;
    let fcheck = (31 - (u16::from(CMF) << 8 | u16::from(flg)) % 31) % 31;
    [CMF, flg | fcheck as u8]
}

/// The Adler-32 checksum of `data`, which ends a zlib stream.
pub(crate) fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for block in data.chunks(ADLER_BLOCK) {
        for &byte in block {
            a += u32::from(byte);
            b += a;
        }
        a %= ADLER_MODULUS;
        b %= ADLER_MODULUS;
    }
    (b << 16) | a
}

#[cfg(test)]
mod tests;
