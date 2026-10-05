use crate::png::ImageHeader;
use crate::png::adam7::Pass;

/// Gathers the pixels of one Adam7 pass from the full image into `out`, as the
/// pass's rows back to back, each `header.row_bytes(pass_width)` bytes. The
/// inverse of `decode::deinterlace::deinterlace_pass`.
///
/// `data` is the full image, `header.stride()` bytes per row. `out`'s old
/// contents are replaced and its allocation reused. Padding bits at the end of
/// each row of pixels under 8 bits are zero. An empty pass leaves `out` empty.
///
/// Works for every bit depth: pixels under 8 bits are moved bit by bit. The
/// caller has already checked the sizes, so `row_bytes` and `stride` can't fail.
pub(crate) fn interlace_pass(header: &ImageHeader, pass: &Pass, data: &[u8], out: &mut Vec<u8>) {
    out.clear();
    let (width, height) = pass.size(header.width, header.height);
    if width == 0 || height == 0 {
        return;
    }

    let bits = usize::from(header.bits_per_pixel());
    let pass_row_bytes = header.row_bytes(width).expect("checked by scanline_size");
    let stride = header.stride().expect("checked by image_size");
    out.resize(pass_row_bytes * height as usize, 0);

    let x_start = pass.x_start as usize;
    let x_step = pass.x_step as usize;

    for (py, pass_row) in out.chunks_exact_mut(pass_row_bytes).enumerate() {
        let y = pass.y_start as usize + py * pass.y_step as usize;
        let image_row = &data[y * stride..][..stride];

        if bits >= 8 {
            gather_whole_byte_pixels(image_row, pass_row, bits / 8, x_start, x_step);
        } else {
            gather_packed_pixels(image_row, pass_row, bits, width as usize, x_start, x_step);
        }
    }
}

/// Pixels of 1 to 8 bytes: each is copied as a block.
fn gather_whole_byte_pixels(image_row: &[u8], pass_row: &mut [u8], bytes: usize, x_start: usize, x_step: usize) {
    for (px, pixel) in pass_row.chunks_exact_mut(bytes).enumerate() {
        let x = x_start + px * x_step;
        pixel.copy_from_slice(&image_row[x * bytes..][..bytes]);
    }
}

/// Pixels of 1, 2 or 4 bits, packed most significant bits first: each is masked
/// out of the image row and into its place in the pass row, which starts zeroed.
fn gather_packed_pixels(image_row: &[u8], pass_row: &mut [u8], bits: usize, width: usize, x_start: usize, x_step: usize) {
    let mask = (1u8 << bits) - 1;

    for px in 0..width {
        let src_bit = (x_start + px * x_step) * bits;
        let value = (image_row[src_bit / 8] >> (8 - bits - src_bit % 8)) & mask;

        let dst_bit = px * bits;
        pass_row[dst_bit / 8] |= value << (8 - bits - dst_bit % 8);
    }
}

#[cfg(test)]
mod tests;
