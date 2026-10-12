//! Crops the frames of an animation to the area that changed, for
//! [`EncodeOptions::crop_frames`](crate::EncodeOptions::crop_frames).
//!
//! A frame is cropped only when what the viewer shows can't change: it and
//! the frame before it cover the whole canvas, the frame before is drawn with
//! [`BlendOp::Source`] and kept with [`DisposeOp::None`], so the canvas holds
//! exactly its pixels, and the frame is drawn with [`BlendOp::Source`] and not
//! cleared with [`DisposeOp::Background`], which would clear only the cropped
//! area. Pixels outside the area are then the same as those already shown.

use crate::encode::animation_ref::FrameRef;
use crate::png::{BlendOp, DisposeOp, ImageHeader};

/// A rectangle on the canvas, in pixels. Never empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Area {
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

/// A frame cropped by [`crop_frames`]: its area on the canvas and its pixels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Cropped {
    pub(crate) area: Area,
    pub(crate) data: Vec<u8>,
}

/// Crops each of `frames`, on a canvas with `header`, that can be cropped
/// without changing what's shown; see the module docs. Returns one entry per
/// frame, `None` for those left as they are. A frame identical to the one
/// before becomes its top-left pixel.
///
/// The frames have been validated.
pub(crate) fn crop_frames(header: &ImageHeader, frames: &[FrameRef<'_>]) -> Vec<Option<Cropped>> {
    let covers = |frame: &FrameRef<'_>| frame.control(0).covers(header.width, header.height);

    let mut cropped = vec![None; frames.len()];
    for (i, pair) in frames.windows(2).enumerate() {
        let [previous, frame] = pair else {
            unreachable!("windows of 2")
        };
        let previous_control = previous.control(0);
        let control = frame.control(0);
        let croppable = covers(previous)
            && covers(frame)
            && previous_control.blend_op == BlendOp::Source
            && previous_control.dispose_op == DisposeOp::None
            && control.blend_op == BlendOp::Source
            && control.dispose_op != DisposeOp::Background;
        if !croppable {
            continue;
        }

        let area = changed_area(header, previous.data(), frame.data()).unwrap_or(Area {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        });
        if area.width == header.width && area.height == header.height {
            continue;
        }
        let mut data = Vec::new();
        crop(header, frame.data(), area, &mut data);
        cropped[i + 1] = Some(Cropped { area, data });
    }
    cropped
}

/// The smallest area holding every pixel that differs between `previous` and
/// `current`, two images with `header`, or `None` if they're the same. Padding
/// bits at the end of rows are ignored.
pub(crate) fn changed_area(header: &ImageHeader, previous: &[u8], current: &[u8]) -> Option<Area> {
    let stride = header.stride().ok()?;
    let width = header.width as usize;
    let bits = usize::from(header.bits_per_pixel());

    let (mut left, mut right) = (width, 0);
    let (mut top, mut bottom) = (None, 0);
    let rows = previous
        .chunks_exact(stride)
        .zip(current.chunks_exact(stride));
    for (y, (a, b)) in rows.enumerate() {
        if a == b {
            continue;
        }
        let Some(first) = (0..width).find(|&x| !same_pixel(a, b, x, bits)) else {
            continue; // only padding differs
        };
        let last = (first..width)
            .rev()
            .find(|&x| !same_pixel(a, b, x, bits))
            .unwrap_or(first);
        left = left.min(first);
        right = right.max(last);
        top.get_or_insert(y);
        bottom = y;
    }

    let top = top?;
    // Within the canvas, so they fit in a `u32`.
    Some(Area {
        x: left as u32,
        y: top as u32,
        width: (right - left + 1) as u32,
        height: (bottom - top + 1) as u32,
    })
}

/// Copies `area` of `data`, an image with `header`, to `out`, replacing its
/// contents, in the layout of [`ImageRef::data`](crate::ImageRef::data) for an
/// image of the area's size.
pub(crate) fn crop(header: &ImageHeader, data: &[u8], area: Area, out: &mut Vec<u8>) {
    let cropped = ImageHeader {
        width: area.width,
        height: area.height,
        ..*header
    };
    // Both fit: the area is within an image whose size has been checked.
    let stride = header.stride().expect("validated");
    let out_stride = cropped.stride().expect("validated");
    let bits = usize::from(header.bits_per_pixel());
    let (x, width) = (area.x as usize, area.width as usize);

    out.clear();
    out.resize(out_stride * area.height as usize, 0);
    let rows = data
        .chunks_exact(stride)
        .skip(area.y as usize)
        .zip(out.chunks_exact_mut(out_stride));
    for (row, out_row) in rows {
        if bits >= 8 {
            let bytes = bits / 8;
            out_row.copy_from_slice(&row[x * bytes..(x + width) * bytes]);
        } else {
            for i in 0..width {
                let bit = i * bits;
                out_row[bit / 8] |= sample(row, x + i, bits) << (8 - bits - bit % 8);
            }
        }
    }
}

/// Whether pixel `x` is the same in rows `a` and `b`, of `bits` per pixel.
fn same_pixel(a: &[u8], b: &[u8], x: usize, bits: usize) -> bool {
    if bits >= 8 {
        let bytes = bits / 8;
        let range = x * bytes..(x + 1) * bytes;
        a[range.clone()] == b[range]
    } else {
        sample(a, x, bits) == sample(b, x, bits)
    }
}

/// Pixel `x` of `row`, packed at `bits` per pixel, under 8, most significant
/// bits first.
fn sample(row: &[u8], x: usize, bits: usize) -> u8 {
    let bit = x * bits;
    (row[bit / 8] >> (8 - bits - bit % 8)) & ((1 << bits) - 1)
}

#[cfg(test)]
mod tests;
