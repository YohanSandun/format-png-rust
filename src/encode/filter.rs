use super::interlace::interlace_pass;
use super::options::FilterStrategy;
use crate::ColorType;
use crate::decode::unfilter::{paeth_predictor};
use crate::error::Error;
use crate::png::adam7::PASSES;
use crate::png::{FilterType, ImageHeader, Interlace};

/// Applies one filter: the inverse of `decode::unfilter::unfilter_row`, with the
/// same arguments. `row` holds one row's raw bytes, and the filtered bytes,
/// without the filter type byte, are written to `out`, which has the same length.
///
/// `previous` is the raw row above, or `None` for the first row, which is treated
/// as all zeros. `bpp` is `ImageHeader::filter_bpp`: 1, 2, 3, 4, 6 or 8. All
/// arithmetic wraps modulo 256. For each byte `x`, with `a` the byte `bpp` to the
/// left, `b` the byte above and `c` the byte above `a` (0 where there is none):
///
/// - `None`: `x`
/// - `Sub`: `x - a`
/// - `Up`: `x - b`
/// - `Average`: `x - floor((a + b) / 2)`, the sum taken without wrapping
/// - `Paeth`: `x - paeth_predictor(a, b, c)`
pub(crate) fn filter_row(filter_type: FilterType, bpp: usize, previous: Option<&[u8]>, row: &[u8], out: &mut [u8]) {
    debug_assert_eq!(row.len(), out.len());
    if let Some(previous) = previous {
        debug_assert_eq!(previous.len(), out.len());
    }

    match bpp {
        1 => filter_pixels::<1>(filter_type, previous, row, out),
        2 => filter_pixels::<2>(filter_type, previous, row, out),
        3 => filter_pixels::<3>(filter_type, previous, row, out),
        4 => filter_pixels::<4>(filter_type, previous, row, out),
        6 => filter_pixels::<6>(filter_type, previous, row, out),
        8 => filter_pixels::<8>(filter_type, previous, row, out),
        _ => unreachable!("filter_bpp is 1, 2, 3, 4, 6 or 8, not {bpp}"),
    }
}

fn sub<const BPP: usize>(row: &[[u8; BPP]], out: &mut [[u8; BPP]]) {
    map_with_left(row, out, u8::wrapping_sub);
}

/// Sets each pixel of `out` to `f(x, a)` per byte, where `x` is the raw pixel
/// and `a` the raw pixel to its left, or zeros for the first one.
#[inline(always)]
fn map_with_left<const BPP: usize>(row: &[[u8; BPP]], out: &mut [[u8; BPP]], f: impl Fn(u8, u8) -> u8) {
    let mut left = [0; BPP];
    for (out, x) in out.iter_mut().zip(row) {
        *out = std::array::from_fn(|i| f(x[i], left[i]));
        // Unlike unfiltering, the left neighbour is the raw pixel, not the output.
        left = *x;
    }
}

#[inline(always)]
fn filter_pixels<const BPP: usize>(filter_type: FilterType, previous: Option<&[u8]>, row: &[u8], out: &mut [u8]) {
    let (unfiltered, rest) = row.as_chunks::<BPP>();
    debug_assert!(rest.is_empty());
    let (out, _) = out.as_chunks_mut::<BPP>();

    let Some(previous) = previous else {
        match filter_type {
            FilterType::None | FilterType::Up => out.copy_from_slice(unfiltered),
            FilterType::Sub | FilterType::Paeth => sub(unfiltered, out),
            FilterType::Average => map_with_left(unfiltered, out, |x, a| x.wrapping_sub(a >> 1)),
        }
        return;
    };
    let (previous, _) = previous.as_chunks::<BPP>();

    match filter_type {
        FilterType::None => out.copy_from_slice(unfiltered),
        FilterType::Sub => sub(unfiltered, out),
        FilterType::Up => {
            for ((out, x), b) in out.iter_mut().zip(unfiltered).zip(previous) {
                *out = std::array::from_fn(|i| x[i].wrapping_sub(b[i]));
            }
        }
        FilterType::Average => {
            let mut a = [0u8; BPP];
            for ((out, x), b) in out.iter_mut().zip(unfiltered).zip(previous) {
                *out = std::array::from_fn(|i| {
                    x[i].wrapping_sub((a[i] & b[i]) + ((a[i] ^ b[i]) >> 1))
                });
                a = *x;
            }
        }
        FilterType::Paeth => {
            let (mut a, mut c) = ([0; BPP], [0; BPP]);
            for ((out, x), b) in out.iter_mut().zip(unfiltered).zip(previous) {
                *out = std::array::from_fn(|i| x[i].wrapping_sub(paeth_predictor(a[i], b[i], c[i])));
                // The predictor uses raw neighbours: this pixel and the one above it.
                a = *x;
                c = *b;
            }
        }
    }
}

/// The sum of `row`'s bytes read as signed, with their absolute values. Stops
/// early once the sum is over `limit`, returning a partial sum that is still
/// over it, so a candidate that can't beat or tie the best is skipped quickly.
///
/// `u64`, since a row can be up to about 16 GB, each byte adding up to 128.
#[inline]
fn row_sum_bounded(row: &[u8], limit: u64) -> u64 {
    let mut sum = 0u64;
    for chunk in row.chunks(64) {
        sum += chunk.iter().map(|&b| u64::from((b as i8).unsigned_abs())).sum::<u64>();
        // Strictly over: a partial sum equal to `limit` could still end above
        // it, and `choose_filter` would wrongly take it as a tie.
        if sum > limit {
            break;
        }
    }
    sum
}

/// Picks the filter for `row` that minimizes the sum of its filtered bytes read
/// as signed (`byte as i8`), with their absolute values: the "minimum sum of
/// absolute differences" heuristic. Ties go to the lowest filter type.
///
/// `scratch` is a buffer the length of `row` to filter into. The same arguments
/// as [`filter_row`] otherwise.
pub(crate) fn choose_filter(bpp: usize, previous: Option<&[u8]>, row: &[u8], scratch: &mut [u8]) -> FilterType {
    let mut best = u64::MAX;
    let mut best_filter = FilterType::None;
    // Highest type first, so with `<=` a tie goes to the lower type tried later.
    let candidates = [FilterType::Paeth, FilterType::Average, FilterType::Up, FilterType::Sub, FilterType::None];
    for filter in candidates {
        filter_row(filter, bpp, previous, row, scratch);
        let s = row_sum_bounded(scratch, best);
        if s <= best {
            best = s;
            best_filter = filter;
        }
    }
    best_filter
}

/// Filters every row of `data`, an image with `header` in the layout
/// [`ImageRef::data`](crate::ImageRef::data) describes, into `out`: each row as a
/// filter type byte then the filtered bytes, which is what goes into the zlib
/// stream. `out`'s old contents are replaced and its allocation reused.
///
/// `strategy` picks the filters; see [`FilterStrategy`]. `data` is assumed to be
/// the right length, which `encoder::validate` checks.
///
/// Interlaced images are filtered pass by pass, in the order of `PASSES`: each
/// pass's pixels are gathered into rows of their own with `interlace_pass`, and
/// the first row of each pass has no row above. Empty passes, which small images
/// have, write nothing, not even filter type bytes.
///
/// Returns `Error::ImageTooLarge` if the filtered size overflows `usize`.
pub(crate) fn filter_image(strategy: FilterStrategy, header: &ImageHeader, data: &[u8], out: &mut Vec<u8>) -> Result<(), Error> {
    out.clear();
    out.reserve(header.scanline_size()?);

    match header.interlace {
        Interlace::None => {
            debug_assert_eq!(data.len() % header.stride()?, 0);
            filter_rows(strategy, header, data, header.stride()?, out);
        }
        Interlace::Adam7 => {
            let mut pass_data = Vec::new();
            for pass in &PASSES {
                let (width, height) = pass.size(header.width, header.height);
                if width == 0 || height == 0 {
                    continue;
                }
                interlace_pass(header, pass, data, &mut pass_data);
                filter_rows(strategy, header, &pass_data, header.row_bytes(width)?, out);
            }
        }
    }

    Ok(())
}

/// Filters `data`, rows of `row_bytes` bytes each, and appends them to `out`,
/// each as a filter type byte then the filtered bytes. The first row has no row
/// above. `strategy` picks the filters, as for `filter_image`.
fn filter_rows(strategy: FilterStrategy, header: &ImageHeader, data: &[u8], row_bytes: usize, out: &mut Vec<u8>) {
    let bpp = header.filter_bpp();
    let adaptive_allowed = header.color_type != ColorType::Indexed && header.bit_depth >= 8;

    let mut scratch = vec![0u8; row_bytes];
    let mut previous: Option<&[u8]> = None;

    for scanline in data.chunks_exact(row_bytes) {
        let filter_type = match strategy {
            FilterStrategy::Fixed(ft) => ft,
            FilterStrategy::Adaptive if adaptive_allowed => choose_filter(bpp, previous, scanline, &mut scratch),
            FilterStrategy::Adaptive => FilterType::None,
        };

        out.push(filter_type as u8);
        let start = out.len();
        out.resize(start + row_bytes, 0);
        filter_row(filter_type, bpp, previous, scanline, &mut out[start..]);

        previous = Some(scanline);
    }
}

#[cfg(test)]
mod tests;
