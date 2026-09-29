use crate::error::Error;
use crate::png::FilterType;

/// The Paeth predictor from the PNG spec: whichever of `a` (left), `b` (above)
/// and `c` (upper left) is closest to `a + b - c`, preferring `a`, then `b` on ties.
pub(crate) fn paeth_predictor(a: u8, b: u8, c: u8) -> u8 {
    let (a16, b16, c16) = (i16::from(a), i16::from(b), i16::from(c));
    let p = a16 + b16 - c16;
    let pa = (p - a16).abs();
    let pb = (p - b16).abs();
    let pc = (p - c16).abs();

    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

#[inline]
fn left(row: &[u8], index: usize, bpp: usize) -> u8 {
    if index < bpp {
        return 0;
    }
    row[index - bpp]
}

/// Reverses one filter in place. `row` holds one scanline's filtered bytes
/// (without the filter type byte) and ends up holding the raw bytes.
///
/// `previous` is the raw row above in the same pass, or `None` for the first row,
/// which is treated as all zeros. `bpp` is `ImageHeader::filter_bpp`. All
/// arithmetic wraps modulo 256.
pub(crate) fn unfilter_row(filter_type: FilterType, bpp: usize, previous: Option<&[u8]>, row: &mut [u8]) {
    if let Some(prev) = previous {
        debug_assert_eq!(prev.len(), row.len());
    }

    let up = |i: usize| previous.map_or(0, |p| p[i]);
    let up_left = |i: usize| previous.map_or(0, |p| left(p, i, bpp));

    match filter_type {
        FilterType::None => {}
        FilterType::Sub => {
            for i in bpp..row.len() {
                row[i] = row[i].wrapping_add(row[i - bpp]);
            }
        }
        FilterType::Up => {
            for i in 0..row.len() {
                row[i] = row[i].wrapping_add(up(i));
            }
        }
        FilterType::Average => {
            for i in 0..row.len() {
                let avg = (u16::from(left(row, i, bpp)) + u16::from(up(i))) / 2;
                row[i] = row[i].wrapping_add(avg as u8);
            }
        }
        FilterType::Paeth => {
            for i in 0..row.len() {
                let pred = paeth_predictor(left(row, i, bpp), up(i), up_left(i));
                row[i] = row[i].wrapping_add(pred);
            }
        }
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
pub(crate) fn unfilter(scanlines: &[u8], row_bytes: usize, bpp: usize, out: &mut [u8]) -> Result<(), Error> {
    if row_bytes == 0 {
        return Ok(());
    }
    debug_assert_eq!(scanlines.len() % (row_bytes + 1), 0);
    debug_assert_eq!(out.len(), scanlines.len() / (row_bytes + 1) * row_bytes);

    for (i, scanline) in scanlines.chunks_exact(row_bytes + 1).enumerate() {
        let filter_type = FilterType::try_from(scanline[0])?;

        // `done` holds the rows already unfiltered; `current` starts at this row.
        let (done, current) = out.split_at_mut(i * row_bytes);
        let row = &mut current[..row_bytes];
        row.copy_from_slice(&scanline[1..]);

        let previous = (i > 0).then(|| &done[done.len() - row_bytes..]);
        unfilter_row(filter_type, bpp, previous, row);
    }

    Ok(())
}

#[cfg(test)]
mod tests;
