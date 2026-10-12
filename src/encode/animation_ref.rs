use super::image_ref::ImageRef;
use crate::png::{
    BlendOp, DisposeOp, FrameControl, ImageHeader, Metadata, OwnedChunk, Palette, Transparency,
};

/// One frame of an animation to encode: its pixels, where it goes on the
/// canvas, how long it's shown, and how it's drawn. Everything is borrowed.
///
/// The pixels are in the layout [`ImageRef`] uses, for an image of the frame's
/// width and height with the animation's bit depth, color type and interlacing.
///
/// ```
/// use format_png::FrameRef;
/// use format_png::png::{BlendOp, DisposeOp};
///
/// let pixels = [0, 0, 255, 255]; // one blue RGBA pixel
/// let frame = FrameRef::new(1, 1, &pixels)
///     .at(3, 2)
///     .with_delay(1, 10) // 0.1 s
///     .with_dispose_op(DisposeOp::Previous)
///     .with_blend_op(BlendOp::Over);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameRef<'a> {
    data: &'a [u8],
    width: u32,
    height: u32,
    x_offset: u32,
    y_offset: u32,
    delay_num: u16,
    delay_den: u16,
    dispose_op: DisposeOp,
    blend_op: BlendOp,
}

impl<'a> FrameRef<'a> {
    /// A frame of `width × height` pixels from `data`, at the canvas's top-left
    /// corner, shown for 0.1 s, kept when the next frame is drawn
    /// ([`DisposeOp::None`]) and replacing what's under it ([`BlendOp::Source`]).
    #[must_use]
    pub fn new(width: u32, height: u32, data: &'a [u8]) -> Self {
        Self {
            data,
            width,
            height,
            x_offset: 0,
            y_offset: 0,
            delay_num: 1,
            delay_den: 10,
            dispose_op: DisposeOp::None,
            blend_op: BlendOp::Source,
        }
    }

    /// Puts the frame's top-left corner at `(x, y)` on the canvas.
    #[must_use]
    pub fn at(mut self, x: u32, y: u32) -> Self {
        self.x_offset = x;
        self.y_offset = y;
        self
    }

    /// Shows the frame for `num / den` seconds. A `den` of 0 means 100.
    #[must_use]
    pub fn with_delay(mut self, num: u16, den: u16) -> Self {
        self.delay_num = num;
        self.delay_den = den;
        self
    }

    /// Sets what happens to the frame's area after it's shown.
    #[must_use]
    pub fn with_dispose_op(mut self, dispose_op: DisposeOp) -> Self {
        self.dispose_op = dispose_op;
        self
    }

    /// Sets how the frame is drawn onto the canvas.
    #[must_use]
    pub fn with_blend_op(mut self, blend_op: BlendOp) -> Self {
        self.blend_op = blend_op;
        self
    }

    /// The pixels.
    #[must_use]
    pub fn data(&self) -> &'a [u8] {
        self.data
    }

    /// The frame's header: `canvas` with the frame's width and height.
    pub(crate) fn header(&self, canvas: &ImageHeader) -> ImageHeader {
        ImageHeader {
            width: self.width,
            height: self.height,
            ..*canvas
        }
    }

    /// The frame's `fcTL` chunk, with `sequence_number`.
    pub(crate) fn control(&self, sequence_number: u32) -> FrameControl {
        FrameControl {
            sequence_number,
            width: self.width,
            height: self.height,
            x_offset: self.x_offset,
            y_offset: self.y_offset,
            delay_num: self.delay_num,
            delay_den: self.delay_den,
            dispose_op: self.dispose_op,
            blend_op: self.blend_op,
        }
    }
}

/// An animated PNG to encode: the canvas header, the frames, and the chunks
/// they share. Everything is borrowed, so nothing is copied.
///
/// The header's width and height are the canvas's, and every frame has its bit
/// depth, color type and interlacing. The first frame is the default image,
/// the one viewers without APNG support show, so it must cover the whole canvas,
/// unless [`with_default_image`](Self::with_default_image) gives a separate one.
///
/// ```
/// use format_png::{AnimationRef, ColorType, FrameRef, ImageHeader, Interlace};
///
/// let header = ImageHeader { width: 2, height: 1, bit_depth: 8, color_type: ColorType::Rgb, interlace: Interlace::None };
/// let (both, right) = ([255, 0, 0, 255, 0, 0], [0, 0, 255]);
/// let frames = [FrameRef::new(2, 1, &both), FrameRef::new(1, 1, &right).at(1, 0)];
///
/// let png = format_png::encode_animation(AnimationRef::new(header, &frames).with_num_plays(3))?;
/// assert_eq!(format_png::decode_animation(&png)?.frames().len(), 2);
/// # Ok::<(), format_png::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimationRef<'a> {
    /// The canvas: the header, the shared chunks, and the default image's
    /// pixels, from `default_image` or else the first frame.
    image: ImageRef<'a>,
    frames: &'a [FrameRef<'a>],
    num_plays: u32,
    has_default_image: bool,
}

impl<'a> AnimationRef<'a> {
    /// An animation of `frames` on a canvas with `header`, played forever, with
    /// no other chunks.
    #[must_use]
    pub fn new(header: ImageHeader, frames: &'a [FrameRef<'a>]) -> Self {
        let data = frames.first().map_or(&[][..], FrameRef::data);
        Self {
            image: ImageRef::new(header, data),
            frames,
            num_plays: 0,
            has_default_image: false,
        }
    }

    /// Plays the animation `num_plays` times; 0, the default, means forever.
    #[must_use]
    pub fn with_num_plays(mut self, num_plays: u32) -> Self {
        self.num_plays = num_plays;
        self
    }

    /// Adds a default image that isn't part of the animation: viewers without
    /// APNG support show it, and the others skip it. `data` is a whole canvas,
    /// in the layout [`ImageRef`] uses.
    #[must_use]
    pub fn with_default_image(mut self, data: &'a [u8]) -> Self {
        self.image = self.image.with_data(data);
        self.has_default_image = true;
        self
    }

    /// Adds a `PLTE` chunk, shared by every frame; see [`ImageRef::with_palette`].
    #[must_use]
    pub fn with_palette(mut self, palette: &'a Palette) -> Self {
        self.image = self.image.with_palette(palette);
        self
    }

    /// Adds a `tRNS` chunk, shared by every frame; see
    /// [`ImageRef::with_transparency`].
    #[must_use]
    pub fn with_transparency(mut self, transparency: &'a Transparency) -> Self {
        self.image = self.image.with_transparency(transparency);
        self
    }

    /// Adds the metadata chunks; see [`ImageRef::with_metadata`].
    #[must_use]
    pub fn with_metadata(mut self, metadata: &'a Metadata) -> Self {
        self.image = self.image.with_metadata(metadata);
        self
    }

    /// Adds extra chunks; see [`ImageRef::with_chunks`]. `acTL`, `fcTL` and
    /// `fdAT` chunks among them are skipped: the encoder writes its own.
    #[must_use]
    pub fn with_chunks(mut self, chunks: &'a [OwnedChunk]) -> Self {
        self.image = self.image.with_chunks(chunks);
        self
    }

    /// The canvas header.
    #[must_use]
    pub fn header(&self) -> &ImageHeader {
        self.image.header()
    }

    /// The frames, in display order.
    #[must_use]
    pub fn frames(&self) -> &'a [FrameRef<'a>] {
        self.frames
    }

    /// How many times to play the animation; 0 means forever.
    #[must_use]
    pub fn num_plays(&self) -> u32 {
        self.num_plays
    }

    /// The default image, if one was added that isn't part of the animation.
    #[must_use]
    pub fn default_image(&self) -> Option<&'a [u8]> {
        self.has_default_image.then(|| self.image.data())
    }

    /// The canvas as an image: the header, the shared chunks, and the default
    /// image's pixels, which are the first frame's unless a default image was added.
    pub(crate) fn image(&self) -> ImageRef<'a> {
        self.image
    }
}

#[cfg(test)]
mod tests;
