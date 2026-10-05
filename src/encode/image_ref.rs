use crate::png::{Image, ImageHeader, Metadata, Palette, Transparency};

/// An image to encode: its header, its pixels in the PNG's own format, and the
/// chunks that go with them. Everything is borrowed, so nothing is copied.
///
/// The pixel layout is the one [`Image::data`] uses: rows top to bottom,
/// [`ImageHeader::stride`] bytes each, 16-bit samples big-endian, pixels under 8
/// bits packed most significant bits first with each row padded to a whole
/// byte, and palette indices for indexed images. Interlaced images are given as
/// the whole image, not as passes.
///
/// More chunks will be added, each with a `with_` method, so the fields are private.
///
/// ```
/// use format_png::{ColorType, ImageHeader, ImageRef, Interlace};
///
/// let header = ImageHeader { width: 2, height: 1, bit_depth: 8, color_type: ColorType::Rgb, interlace: Interlace::None };
/// let pixels = [255, 0, 0, 0, 0, 255]; // red, blue
/// let image = ImageRef::new(header, &pixels);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageRef<'a> {
    header: ImageHeader,
    data: &'a [u8],
    palette: Option<&'a Palette>,
    transparency: Option<&'a Transparency>,
    metadata: Option<&'a Metadata>,
}

impl<'a> ImageRef<'a> {
    /// An image with `header` and the pixels in `data`, and no other chunks.
    pub fn new(header: ImageHeader, data: &'a [u8]) -> Self {
        Self { header, data, palette: None, transparency: None, metadata: None }
    }

    /// Adds a `PLTE` chunk. Indexed images need one. RGB and RGBA images may
    /// have one, as a suggested palette; grayscale images must not.
    pub fn with_palette(mut self, palette: &'a Palette) -> Self {
        self.palette = Some(palette);
        self
    }

    /// Adds a `tRNS` chunk: one fully transparent color for grayscale and RGB
    /// images, or an alpha value per palette entry for indexed images. Images
    /// with an alpha channel must not have one.
    pub fn with_transparency(mut self, transparency: &'a Transparency) -> Self {
        self.transparency = Some(transparency);
        self
    }

    /// Adds the metadata chunks: color space, physical size, time, text, ICC
    /// profile and Exif; see [`Metadata`].
    pub fn with_metadata(mut self, metadata: &'a Metadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// The image's header.
    pub fn header(&self) -> &ImageHeader {
        &self.header
    }

    /// The pixels, in the layout described above.
    pub fn data(&self) -> &'a [u8] {
        self.data
    }

    /// The palette, if one was added.
    pub fn palette(&self) -> Option<&'a Palette> {
        self.palette
    }

    /// The transparency, if it was added.
    pub fn transparency(&self) -> Option<&'a Transparency> {
        self.transparency
    }

    /// The metadata, if it was added.
    pub fn metadata(&self) -> Option<&'a Metadata> {
        self.metadata
    }
}

/// Re-encodes a decoded image, with its palette, transparency and metadata. The
/// metadata is only there if the image was decoded with
/// [`DecodeOptions::preserve_metadata`](crate::DecodeOptions::preserve_metadata).
///
/// TODO: carry over the preserved ancillary chunks once the encoder writes them.
impl<'a> From<&'a Image> for ImageRef<'a> {
    fn from(image: &'a Image) -> Self {
        let mut image_ref = ImageRef::new(*image.header(), image.data());
        if let Some(palette) = image.palette() {
            image_ref = image_ref.with_palette(palette);
        }
        if let Some(transparency) = image.transparency() {
            image_ref = image_ref.with_transparency(transparency);
        }
        if !image.metadata().is_empty() {
            image_ref = image_ref.with_metadata(image.metadata());
        }
        image_ref
    }
}

#[cfg(test)]
mod tests;
