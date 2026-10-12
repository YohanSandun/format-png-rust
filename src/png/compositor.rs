use crate::convert::convert;
use crate::error::Error;
use crate::png::PixelFormat;
use crate::png::animated_image::Frame;
use crate::png::animation::{BlendOp, DisposeOp, FrameControl};

const RGBA: usize = 4;

/// Draws the frames of an animated PNG onto a canvas, one at a time, as a viewer
/// shows them.
///
/// The canvas is 8-bit RGBA, without padding, like a [`Bitmap`](crate::Bitmap)
/// in [`PixelFormat::Rgba8`]. It starts fully transparent. Each call to
/// [`render`](Self::render) first disposes of the previous frame as its
/// [`DisposeOp`] says, then draws the new one as its [`BlendOp`] says, and
/// returns the canvas to show.
///
/// [`AnimatedImage::to_bitmaps`](crate::AnimatedImage::to_bitmaps) renders every
/// frame at once; a `Compositor` keeps only one canvas, for long animations.
///
/// ```
/// # fn main() -> Result<(), format_png::Error> {
/// # use format_png::{AnimationRef, ColorType, FrameRef, ImageHeader, Interlace};
/// # let header = ImageHeader { width: 1, height: 1, bit_depth: 8, color_type: ColorType::Rgba, interlace: Interlace::None };
/// # let frames = [FrameRef::new(1, 1, &[255, 0, 0, 255]), FrameRef::new(1, 1, &[0, 0, 255, 255])];
/// # let data = format_png::encode_animation(AnimationRef::new(header, &frames))?;
/// use format_png::Compositor;
///
/// let animation = format_png::decode_animation(&data)?;
/// let mut compositor = Compositor::new(animation.width(), animation.height())?;
///
/// for frame in animation.frames() {
///     let canvas = compositor.render(frame)?;
///     // Show `canvas` for `frame.control().delay_seconds()`.
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct Compositor {
    width: u32,
    height: u32,
    canvas: Vec<u8>,
    /// The canvas under the last frame's area before it was drawn, for
    /// `DisposeOp::Previous`.
    saved: Vec<u8>,
    /// The frame being drawn, converted to RGBA.
    pixels: Vec<u8>,
    /// The last frame's area and how to dispose of it, applied at the start of
    /// the next `render`.
    pending: Option<(FrameControl, DisposeOp)>,
    /// Whether a frame has been drawn since the start or the last `reset`.
    started: bool,
}

impl Compositor {
    /// Creates a compositor with a fully transparent canvas of `width × height`.
    ///
    /// # Errors
    ///
    /// [`Error::ImageTooLarge`] if the canvas doesn't fit in memory on this platform.
    pub fn new(width: u32, height: u32) -> Result<Self, Error> {
        let size = (width as usize)
            .checked_mul(height as usize)
            .and_then(|pixels| pixels.checked_mul(RGBA))
            .ok_or(Error::ImageTooLarge)?;

        Ok(Self {
            width,
            height,
            canvas: vec![0; size],
            saved: Vec::new(),
            pixels: Vec::new(),
            pending: None,
            started: false,
        })
    }

    /// The canvas as the last [`render`](Self::render) left it, before that
    /// frame is disposed of.
    #[must_use]
    pub fn canvas(&self) -> &[u8] {
        &self.canvas
    }

    /// Clears the canvas to start the animation again, for its next play.
    pub fn reset(&mut self) {
        self.canvas.fill(0);
        self.pending = None;
        self.started = false;
    }

    /// Disposes of the previous frame, draws `frame`, and returns the canvas to
    /// show.
    ///
    /// # Errors
    ///
    /// - [`Error::FrameOutOfBounds`] if the frame doesn't fit on the canvas.
    /// - Any error from [`Image::to_bitmap`](crate::Image::to_bitmap).
    pub fn render(&mut self, frame: &Frame) -> Result<&[u8], Error> {
        let control = *frame.control();
        if !control.fits_in(self.width, self.height) {
            return Err(Error::FrameOutOfBounds);
        }

        if let Some((previous, dispose)) = self.pending.take() {
            self.dispose(&previous, dispose);
        }

        // On the first frame there's nothing to go back to.
        let dispose = match control.dispose_op {
            DisposeOp::Previous if !self.started => DisposeOp::Background,
            op => op,
        };
        if dispose == DisposeOp::Previous {
            self.save(&control);
        }

        convert(
            &frame.image().source(),
            PixelFormat::Rgba8,
            &mut self.pixels,
        )?;
        self.draw(&control);

        self.pending = Some((control, dispose));
        self.started = true;
        Ok(&self.canvas)
    }

    /// Applies `dispose` to `control`'s area: the previous frame's disposal.
    fn dispose(&mut self, control: &FrameControl, dispose: DisposeOp) {
        match dispose {
            DisposeOp::None => {}
            DisposeOp::Background => {
                for range in area_rows(self.width, control) {
                    self.canvas[range].fill(0);
                }
            }
            DisposeOp::Previous => {
                let row_bytes = control.width as usize * RGBA;
                let rows = area_rows(self.width, control).zip(self.saved.chunks_exact(row_bytes));
                for (range, saved) in rows {
                    self.canvas[range].copy_from_slice(saved);
                }
            }
        }
    }

    /// Copies `control`'s area of the canvas into `self.saved`.
    fn save(&mut self, control: &FrameControl) {
        self.saved.clear();
        for range in area_rows(self.width, control) {
            self.saved.extend_from_slice(&self.canvas[range]);
        }
    }

    /// Draws `self.pixels`, the frame in RGBA, onto its area of the canvas.
    fn draw(&mut self, control: &FrameControl) {
        let row_bytes = control.width as usize * RGBA;
        let rows = area_rows(self.width, control).zip(self.pixels.chunks_exact(row_bytes));

        for (range, src) in rows {
            let dst = &mut self.canvas[range];
            match control.blend_op {
                BlendOp::Source => dst.copy_from_slice(src),
                BlendOp::Over => {
                    let pixels = dst.as_chunks_mut::<RGBA>().0.iter_mut();
                    for (d, s) in pixels.zip(src.as_chunks::<RGBA>().0) {
                        blend_over(d, s);
                    }
                }
            }
        }
    }
}

/// The canvas bytes of each row of `control`'s area, top to bottom, on an RGBA
/// canvas `canvas_width` pixels wide. The frame fits on the canvas.
fn area_rows(
    canvas_width: u32,
    control: &FrameControl,
) -> impl Iterator<Item = std::ops::Range<usize>> + use<> {
    let canvas_stride = canvas_width as usize * RGBA;
    let x = control.x_offset as usize * RGBA;
    let row_bytes = control.width as usize * RGBA;
    let top = control.y_offset as usize;

    (top..top + control.height as usize).map(move |y| {
        let start = y * canvas_stride + x;
        start..start + row_bytes
    })
}

/// Draws the RGBA pixel `src` over `dst`, with straight (not premultiplied)
/// alpha, as the APNG spec gives it.
pub(crate) fn blend_over(dst: &mut [u8; RGBA], src: &[u8; RGBA]) {
    let src_alpha = u32::from(src[3]);
    match src_alpha {
        0 => return,
        255 => {
            *dst = *src;
            return;
        }
        _ => {}
    }

    // Weights scaled by 255, so the output alpha is `total / 255`.
    let src_weight = src_alpha * 255;
    let dst_weight = u32::from(dst[3]) * (255 - src_alpha);
    let total = src_weight + dst_weight;

    for c in 0..3 {
        let value = u32::from(src[c]) * src_weight + u32::from(dst[c]) * dst_weight;
        dst[c] = ((value + total / 2) / total) as u8;
    }
    dst[3] = ((total + 127) / 255) as u8;
}

#[cfg(test)]
mod tests;
