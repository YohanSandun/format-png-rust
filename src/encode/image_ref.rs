use crate::png::{Image, ImageHeader, Palette};

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
}

impl<'a> ImageRef<'a> {
    /// An image with `header` and the pixels in `data`, and no other chunks.
    pub fn new(header: ImageHeader, data: &'a [u8]) -> Self {
        Self { header, data, palette: None }
    }

    /// Adds a `PLTE` chunk. Indexed images need one. RGB and RGBA images may
    /// have one, as a suggested palette; grayscale images must not.
    pub fn with_palette(mut self, palette: &'a Palette) -> Self {
        self.palette = Some(palette);
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
}

/// Re-encodes a decoded image, with its palette.
///
/// TODO: carry over `tRNS`, metadata and the preserved ancillary chunks once
/// the encoder writes them.
impl<'a> From<&'a Image> for ImageRef<'a> {
    fn from(image: &'a Image) -> Self {
        let image_ref = ImageRef::new(*image.header(), image.data());
        match image.palette() {
            Some(palette) => image_ref.with_palette(palette),
            None => image_ref,
        }
    }
}

#[cfg(test)]
mod tests;
