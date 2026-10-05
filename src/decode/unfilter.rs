use crate::error::Error;
use crate::png::FilterType;

/// The Paeth predictor from the PNG spec: whichever of `a` (left), `b` (above)
/// and `c` (upper left) is closest to `a + b - c`, preferring `a`, then `b` on ties.
///
/// This is stb_image's formulation (`stbi__paeth`). It gives the same result as the
/// spec's, which the tests check for every input, with fewer operations. Paeth
/// unfiltering can't be vectorized along a row, because each pixel depends on the
/// one to its left, so the length of this chain sets its speed.
#[inline(always)]
pub(crate) fn paeth_predictor(a: u8, b: u8, c: u8) -> u8 {
    let threshold = 3 * i16::from(c) - (i16::from(a) + i16::from(b));
    let (low, high) = (a.min(b), a.max(b));
    let low_or_c = if i16::from(high) <= threshold { low } else { c };
    if threshold <= i16::from(low) {
        high
    } else {
        low_or_c
    }
}
/// Reverses one filter. `filtered` holds one scanline's bytes without the filter
/// type byte, and the raw bytes are written to `out`, which has the same length.
///
/// `previous` is the raw row above in the same pass, or `None` for the first row,
/// which is treated as all zeros. `bpp` is `ImageHeader::filter_bpp`: 1, 2, 3, 4,
/// 6 or 8. All arithmetic wraps modulo 256.
pub(crate) fn unfilter_row(
    filter_type: FilterType,
    bpp: usize,
    previous: Option<&[u8]>,
    filtered: &[u8],
    out: &mut [u8],
) {
    debug_assert_eq!(filtered.len(), out.len());
    if let Some(previous) = previous {
        debug_assert_eq!(previous.len(), out.len());
    }

    // A loop for each pixel size, so each works on whole pixels of a size known
    // at compile time.
    match bpp {
        1 => unfilter_pixels::<1>(filter_type, previous, filtered, out),
        2 => unfilter_pixels::<2>(filter_type, previous, filtered, out),
        3 => unfilter_pixels::<3>(filter_type, previous, filtered, out),
        4 => unfilter_pixels::<4>(filter_type, previous, filtered, out),
        6 => unfilter_pixels::<6>(filter_type, previous, filtered, out),
        8 => unfilter_pixels::<8>(filter_type, previous, filtered, out),
        _ => unreachable!("filter_bpp is 1, 2, 3, 4, 6 or 8, not {bpp}"),
    }
}

/// [`unfilter_row`] for pixels of `BPP` bytes. Rows are always a whole number of
/// pixels: images under 8 bits per pixel have `BPP` 1, and wider ones have no padding.
#[inline(always)]
fn unfilter_pixels<const BPP: usize>(
    filter_type: FilterType,
    previous: Option<&[u8]>,
    filtered: &[u8],
    out: &mut [u8],
) {
    let (filtered, rest) = filtered.as_chunks::<BPP>();
    debug_assert!(rest.is_empty());
    let (out, _) = out.as_chunks_mut::<BPP>();

    let Some(previous) = previous else {
        // The row above is all zeros: Up changes nothing, Paeth always predicts
        // the left pixel like Sub, and Average adds half the left pixel.
        match filter_type {
            FilterType::None | FilterType::Up => out.copy_from_slice(filtered),
            FilterType::Sub | FilterType::Paeth => sub(filtered, out),
            FilterType::Average => map_with_left(filtered, out, |x, a| x.wrapping_add(a >> 1)),
        }
        return;
    };
    let (previous, _) = previous.as_chunks::<BPP>();

    match filter_type {
        FilterType::None => out.copy_from_slice(filtered),
        FilterType::Sub => sub(filtered, out),
        FilterType::Up => {
            for ((out, x), b) in out.iter_mut().zip(filtered).zip(previous) {
                *out = std::array::from_fn(|i| x[i].wrapping_add(b[i]));
            }
        }
        FilterType::Average => {
            let mut a = [0; BPP];
            for ((out, x), b) in out.iter_mut().zip(filtered).zip(previous) {
                // The mean of two bytes without overflow: (a + b) / 2 rounded down.
                a = std::array::from_fn(|i| {
                    x[i].wrapping_add((a[i] & b[i]) + ((a[i] ^ b[i]) >> 1))
                });
                *out = a;
            }
        }
        FilterType::Paeth => {
            let (mut a, mut c) = ([0; BPP], [0; BPP]);
            for ((out, x), b) in out.iter_mut().zip(filtered).zip(previous) {
                a = std::array::from_fn(|i| x[i].wrapping_add(paeth_predictor(a[i], b[i], c[i])));
                c = *b;
                *out = a;
            }
        }
    }
}

fn sub<const BPP: usize>(filtered: &[[u8; BPP]], out: &mut [[u8; BPP]]) {
    map_with_left(filtered, out, u8::wrapping_add);
}

/// Unfilters pixels that depend only on the raw pixel to their left, which is
/// zero for the first. `f` takes a filtered byte and the byte to its left.
#[inline(always)]
fn map_with_left<const BPP: usize>(
    filtered: &[[u8; BPP]],
    out: &mut [[u8; BPP]],
    f: impl Fn(u8, u8) -> u8,
) {
    let mut left = [0; BPP];
    for (out, x) in out.iter_mut().zip(filtered) {
        left = std::array::from_fn(|i| f(x[i], left[i]));
        *out = left;
    }
}

/// Unfilters consecutive scanlines of `row_bytes` bytes each.
///
/// `scanlines` is decompressed data: each row's filter type byte followed by its
/// bytes. The raw rows are written to `out` back to back, without filter type
/// bytes, so `out.len() / row_bytes` is the number of rows. The caller makes sure
/// `scanlines.len() == rows * (row_bytes + 1)`.
///
/// Returns `Error::InvalidFilterType` for a filter type byte above 4.
pub(crate) fn unfilter(
    scanlines: &[u8],
    row_bytes: usize,
    bpp: usize,
    out: &mut [u8],
) -> Result<(), Error> {
    if row_bytes == 0 {
        return Ok(());
    }
    debug_assert_eq!(scanlines.len() % (row_bytes + 1), 0);
    debug_assert_eq!(out.len(), scanlines.len() / (row_bytes + 1) * row_bytes);

    for (i, scanline) in scanlines.chunks_exact(row_bytes + 1).enumerate() {
        let filter_type = FilterType::try_from(scanline[0])?;

        // `done` holds the rows already unfiltered; `current` starts at this row.
        let (done, current) = out.split_at_mut(i * row_bytes);
        let previous = (i > 0).then(|| &done[done.len() - row_bytes..]);
        unfilter_row(
            filter_type,
            bpp,
            previous,
            &scanline[1..],
            &mut current[..row_bytes],
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests;
