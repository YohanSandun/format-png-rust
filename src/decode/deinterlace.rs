use crate::png::ImageHeader;
use crate::png::adam7::Pass;

/// Copies the pixels of one Adam7 pass to their places in the full image.
///
/// `pass_data` holds the pass's unfiltered rows back to back, each
/// `header.row_bytes(pass_width)` bytes. `out` is the full image,
/// `header.stride()` bytes per row. Only this pass's pixels in `out` are
/// written; the rest are left as they are.
///
/// Works for every bit depth: pixels under 8 bits are moved bit by bit. The
/// caller has already checked the sizes, so `row_bytes` and `stride` can't fail.
pub(crate) fn deinterlace_pass(header: &ImageHeader, pass: &Pass, pass_data: &[u8], out: &mut [u8]) {
    let (width, height) = pass.size(header.width, header.height);
    if width == 0 || height == 0 {
        return;
    }

    let bits = usize::from(header.bits_per_pixel());
    let pass_row_bytes = header.row_bytes(width).expect("checked by scanline_size");
    let stride = header.stride().expect("checked by image_size");
    debug_assert_eq!(pass_data.len(), pass_row_bytes * height as usize);

    let x_start = pass.x_start as usize;
    let x_step = pass.x_step as usize;

    for (py, pass_row) in pass_data.chunks_exact(pass_row_bytes).enumerate() {
        let y = pass.y_start as usize + py * pass.y_step as usize;
        let out_row = &mut out[y * stride..][..stride];

        if bits >= 8 {
            copy_whole_byte_pixels(pass_row, out_row, bits / 8, x_start, x_step);
        } else {
            copy_packed_pixels(pass_row, out_row, bits, width as usize, x_start, x_step);
        }
    }
}

/// Pixels of 1 to 8 bytes: each is copied as a block.
fn copy_whole_byte_pixels(pass_row: &[u8], out_row: &mut [u8], bytes: usize, x_start: usize, x_step: usize) {
    for (px, pixel) in pass_row.chunks_exact(bytes).enumerate() {
        let x = x_start + px * x_step;
        out_row[x * bytes..][..bytes].copy_from_slice(pixel);
    }
}

/// Pixels of 1, 2 or 4 bits, packed most significant bits first: each is masked
/// out of the pass row and into its byte in the image row.
fn copy_packed_pixels(pass_row: &[u8], out_row: &mut [u8], bits: usize, width: usize, x_start: usize, x_step: usize) {
    let mask = (1u8 << bits) - 1;

    for px in 0..width {
        let src_bit = px * bits;
        let value = (pass_row[src_bit / 8] >> (8 - bits - src_bit % 8)) & mask;

        let dst_bit = (x_start + px * x_step) * bits;
        let shift = 8 - bits - dst_bit % 8;
        let byte = &mut out_row[dst_bit / 8];
        *byte = (*byte & !(mask << shift)) | (value << shift);
    }
}

#[cfg(test)]
mod tests;
