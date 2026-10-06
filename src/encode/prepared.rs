use std::fmt;

use super::chunk_writer::{MAX_CHUNK_LENGTH, write_image_data};
use super::parallel::{SEGMENT_SIZE, SegmentCompression, join_zlib};
use crate::error::Error;

/// An image that's ready to encode except for compressing its image data, from
/// [`Encoder::prepare`](crate::Encoder::prepare).
///
/// It splits the slow part of encoding, compression, into segments that can be
/// compressed anywhere: on other threads, or in other WebAssembly instances such
/// as Web Workers, which share no memory. Compress each
/// [`segment`](Self::segment) with [`compress_segment`](crate::compress_segment),
/// in any order, then pass the results to [`finish`](Self::finish) in segment
/// order.
///
/// ```
/// use format_png::{ColorType, Encoder, ImageHeader, ImageRef, Interlace};
///
/// let header = ImageHeader { width: 1024, height: 512, bit_depth: 8, color_type: ColorType::Rgba, interlace: Interlace::None };
/// let pixels: Vec<u8> = (0..header.image_size()?).map(|i| (i % 251) as u8).collect();
///
/// let prepared = Encoder::new().prepare(ImageRef::new(header, &pixels))?;
/// let compressed: Vec<Vec<u8>> = (0..prepared.segment_count())
///     .map(|i| format_png::compress_segment(prepared.segment(i), prepared.compression()))
///     .collect();
/// let png = prepared.finish(&compressed)?;
///
/// assert_eq!(format_png::decode(&png)?.data(), pixels);
/// # Ok::<(), format_png::Error>(())
/// ```
///
/// The result is the same file as encoding with
/// [`Threads::Auto`](crate::Threads::Auto), byte for byte. Images whose filtered
/// data is 1 MiB or less have no segments: [`Encoder::prepare`](crate::Encoder::prepare)
/// encodes them whole, and `finish` returns them.
pub struct PreparedPng {
    /// Everything before the image data, or the whole PNG if `deferred` is `None`.
    head: Vec<u8>,
    deferred: Option<Deferred>,
    compression: SegmentCompression,
}

/// The parts of a PNG left to write once its image data is compressed.
pub(crate) struct Deferred {
    /// The filtered rows, each with its filter type byte: what's compressed.
    pub(crate) scanlines: Vec<u8>,
    /// Everything after the image data: extra chunks and `IEND`.
    pub(crate) tail: Vec<u8>,
}

impl PreparedPng {
    pub(crate) fn new(
        head: Vec<u8>,
        deferred: Option<Deferred>,
        compression: SegmentCompression,
    ) -> Self {
        Self {
            head,
            deferred,
            compression,
        }
    }

    /// How many segments there are to compress: the filtered data in segments of
    /// 1 MiB, the last one possibly smaller. Zero for images with 1 MiB of
    /// filtered data or less, which are already encoded.
    #[must_use]
    pub fn segment_count(&self) -> usize {
        self.deferred.as_ref().map_or(0, |deferred| {
            deferred.scanlines.len().div_ceil(SEGMENT_SIZE)
        })
    }

    /// Segment number `index`, to pass to [`compress_segment`](crate::compress_segment).
    ///
    /// # Panics
    ///
    /// If `index` isn't less than [`segment_count`](Self::segment_count).
    #[must_use]
    pub fn segment(&self, index: usize) -> &[u8] {
        let count = self.segment_count();
        assert!(index < count, "segment {index} of {count}");
        let scanlines = &self
            .deferred
            .as_ref()
            .expect("there are segments")
            .scanlines;
        let start = index * SEGMENT_SIZE;
        &scanlines[start..scanlines.len().min(start + SEGMENT_SIZE)]
    }

    /// How to compress the segments: pass it to [`compress_segment`](crate::compress_segment)
    /// with each one.
    #[must_use]
    pub fn compression(&self) -> SegmentCompression {
        self.compression
    }

    /// Writes the PNG, with `compressed` as its image data: the result of
    /// [`compress_segment`](crate::compress_segment) for each segment, in
    /// segment order.
    ///
    /// The segments aren't checked: anything but each segment compressed with
    /// [`compression`](Self::compression), in order, makes a PNG that doesn't
    /// decode.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidSegmentCount`] if `compressed` doesn't hold exactly
    /// [`segment_count`](Self::segment_count) segments.
    pub fn finish(self, compressed: &[impl AsRef<[u8]>]) -> Result<Vec<u8>, Error> {
        let expected = self.segment_count();
        if compressed.len() != expected {
            return Err(Error::InvalidSegmentCount {
                expected,
                actual: compressed.len(),
            });
        }
        let Some(deferred) = self.deferred else {
            return Ok(self.head);
        };

        let mut zlib = Vec::new();
        join_zlib(&deferred.scanlines, self.compression, compressed, &mut zlib);

        let mut out = self.head;
        out.reserve(zlib.len() + deferred.tail.len() + 12 * (zlib.len() / MAX_CHUNK_LENGTH + 1));
        write_image_data(&mut out, &zlib, MAX_CHUNK_LENGTH);
        out.extend_from_slice(&deferred.tail);
        Ok(out)
    }
}

/// Shows sizes, not the bytes, which can be many megabytes.
impl fmt::Debug for PreparedPng {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedPng")
            .field("segment_count", &self.segment_count())
            .field("compression", &self.compression)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests;
