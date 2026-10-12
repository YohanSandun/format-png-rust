use crate::error::Error;
use crate::png::animation::{AnimationControl, FrameControl};
use crate::png::compositor::Compositor;
use crate::png::{
    Bitmap, Image, ImageChunks, ImageHeader, Metadata, OwnedChunk, Palette, PixelFormat,
    Transparency,
};

/// One frame of an [`AnimatedImage`]: its `fcTL` control and its pixels.
///
/// The [`image`](Self::image) is only the frame's own area, `control().width ×
/// control().height` pixels, in the PNG's own pixel format, with the
/// animation's palette and transparency. To get what a viewer shows, with each
/// frame drawn onto the canvas, use [`AnimatedImage::to_bitmaps`] or a
/// [`Compositor`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    control: FrameControl,
    image: Image,
}

impl Frame {
    pub(crate) fn new(control: FrameControl, image: Image) -> Self {
        debug_assert_eq!(
            (image.width(), image.height()),
            (control.width, control.height)
        );
        Self { control, image }
    }

    /// The frame's `fcTL` chunk: size, position, delay, disposal and blending.
    #[must_use]
    pub fn control(&self) -> &FrameControl {
        &self.control
    }

    /// The frame's pixels, as decoded, without compositing.
    #[must_use]
    pub fn image(&self) -> &Image {
        &self.image
    }

    /// Consumes the frame and returns its image.
    #[must_use]
    pub fn into_image(self) -> Image {
        self.image
    }
}

/// A decoded animated PNG: the canvas header, the `acTL` control, and every
/// frame, in order.
///
/// Returned by [`decode_animation`](crate::decode_animation) and
/// [`Decoder::decode_animation`](crate::Decoder::decode_animation).
///
/// ```
/// # fn main() -> Result<(), format_png::Error> {
/// # use format_png::{ColorType, ImageHeader, Interlace};
/// # use format_png::{AnimationRef, FrameRef};
/// # let header = ImageHeader { width: 2, height: 1, bit_depth: 8, color_type: ColorType::Rgba, interlace: Interlace::None };
/// # let (red, blue) = ([255, 0, 0, 255, 255, 0, 0, 255], [0, 0, 255, 255]);
/// # let frames = [FrameRef::new(2, 1, &red), FrameRef::new(1, 1, &blue).at(1, 0)];
/// # let data = format_png::encode_animation(AnimationRef::new(header, &frames))?;
/// let animation = format_png::decode_animation(&data)?;
///
/// for (frame, bitmap) in animation.frames().iter().zip(animation.to_bitmaps()?) {
///     println!("show {} bytes for {} s", bitmap.data().len(), frame.control().delay_seconds());
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimatedImage {
    header: ImageHeader,
    control: AnimationControl,
    frames: Vec<Frame>,
    default_image_is_frame: bool,
    chunks: ImageChunks,
}

impl AnimatedImage {
    pub(crate) fn new(
        header: ImageHeader,
        control: AnimationControl,
        frames: Vec<Frame>,
        default_image_is_frame: bool,
        chunks: ImageChunks,
    ) -> Self {
        debug_assert_eq!(frames.len(), control.num_frames as usize);
        Self {
            header,
            control,
            frames,
            default_image_is_frame,
            chunks,
        }
    }

    /// The `IHDR` header. Its width and height are the canvas's; every frame
    /// shares its bit depth, color type and interlacing.
    #[must_use]
    pub fn header(&self) -> &ImageHeader {
        &self.header
    }

    /// Canvas width in pixels.
    #[must_use]
    pub fn width(&self) -> u32 {
        self.header.width
    }

    /// Canvas height in pixels.
    #[must_use]
    pub fn height(&self) -> u32 {
        self.header.height
    }

    /// The `acTL` chunk: the number of frames and plays.
    #[must_use]
    pub fn control(&self) -> &AnimationControl {
        &self.control
    }

    /// How many times to play the animation; 0 means forever.
    #[must_use]
    pub fn num_plays(&self) -> u32 {
        self.control.num_plays
    }

    /// The frames, in display order.
    #[must_use]
    pub fn frames(&self) -> &[Frame] {
        &self.frames
    }

    /// Whether the default image, the one viewers without APNG support show, is
    /// the first frame. If not, it isn't part of the animation and isn't in
    /// [`frames`](Self::frames); [`decode`](crate::decode) returns it.
    #[must_use]
    pub fn default_image_is_frame(&self) -> bool {
        self.default_image_is_frame
    }

    /// The `PLTE` palette shared by every frame; see [`Image::palette`].
    #[must_use]
    pub fn palette(&self) -> Option<&Palette> {
        self.chunks.palette.as_ref()
    }

    /// The `tRNS` transparency shared by every frame; see [`Image::transparency`].
    #[must_use]
    pub fn transparency(&self) -> Option<&Transparency> {
        self.chunks.transparency.as_ref()
    }

    /// The known ancillary chunks; see [`Image::metadata`].
    #[must_use]
    pub fn metadata(&self) -> &Metadata {
        &self.chunks.metadata
    }

    /// Raw copies of the ancillary chunks; see [`Image::ancillary_chunks`]. The
    /// animation's own `acTL`, `fcTL` and `fdAT` chunks are among them.
    #[must_use]
    pub fn ancillary_chunks(&self) -> &[OwnedChunk] {
        &self.chunks.ancillary
    }

    /// Consumes the animation and returns its frames.
    #[must_use]
    pub fn into_frames(self) -> Vec<Frame> {
        self.frames
    }

    /// Composites every frame onto the canvas, as a viewer shows them: one
    /// 8-bit RGBA bitmap of the whole canvas per frame, in order. See
    /// [`Compositor`] to render one frame at a time instead of holding them all.
    ///
    /// # Errors
    ///
    /// Any error from [`Compositor::new`] or [`Compositor::render`].
    pub fn to_bitmaps(&self) -> Result<Vec<Bitmap>, Error> {
        let mut compositor = Compositor::new(self.width(), self.height())?;
        self.frames
            .iter()
            .map(|frame| {
                let canvas = compositor.render(frame)?;
                Ok(Bitmap::new(
                    self.width(),
                    self.height(),
                    PixelFormat::Rgba8,
                    canvas.to_vec(),
                ))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
