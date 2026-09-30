use crate::convert::{Source, convert};
use crate::error::Error;
use crate::png::{Bitmap, ImageHeader, Palette, PixelFormat};

/// A decoded image, in the PNG's own pixel format.
///
/// Pixels aren't converted. Rows are stored top to bottom, [`stride`](Self::stride)
/// bytes each, with the samples and bit depth that [`header`](Self::header) gives:
///
/// - 16-bit samples are big-endian.
/// - Pixels under 8 bits are packed most significant bits first, and each row is
///   padded to a whole byte.
/// - Indexed images hold palette indices, not colors; see [`palette`](Self::palette).
/// - Interlaced images have already been put back together.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    header: ImageHeader,
    stride: usize,
    data: Vec<u8>,
    palette: Option<Palette>,
}

impl Image {
    pub(crate) fn new(header: ImageHeader, stride: usize, data: Vec<u8>, palette: Option<Palette>) -> Self {
        debug_assert_eq!(data.len(), stride * header.height as usize);
        Self { header, stride, data, palette }
    }

    /// The image's header.
    pub fn header(&self) -> &ImageHeader {
        &self.header
    }

    /// Width in pixels.
    pub fn width(&self) -> u32 {
        self.header.width
    }

    /// Height in pixels.
    pub fn height(&self) -> u32 {
        self.header.height
    }

    /// Bytes per row.
    pub fn stride(&self) -> usize {
        self.stride
    }

    /// All the pixel data, `stride() * height()` bytes.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// The bytes of row `y`, counting from the top.
    ///
    /// # Panics
    ///
    /// Panics if `y` is not less than [`height`](Self::height).
    pub fn row(&self, y: u32) -> &[u8] {
        assert!(y < self.height(), "row {y} is out of range for an image {} rows high", self.height());

        let start = y as usize * self.stride;
        &self.data[start..start + self.stride]
    }

    /// The `PLTE` palette. Indexed images always have one. RGB and RGBA images
    /// may have one as a suggestion for displays with few colors; it isn't used
    /// when converting them. Grayscale images never have one.
    pub fn palette(&self) -> Option<&Palette> {
        self.palette.as_ref()
    }

    /// Consumes the image and returns its pixel data.
    pub fn into_data(self) -> Vec<u8> {
        self.data
    }

    /// Converts the pixels to `format`, for display.
    ///
    /// Samples under 8 bits are scaled to the full range, 16-bit samples keep
    /// their high byte, grayscale is copied to red, green and blue, and indexed
    /// pixels become their palette color. Converting to [`PixelFormat::Rgba8`] adds
    /// alpha 255 to images without alpha, and [`PixelFormat::Rgb8`] drops alpha.
    ///
    /// # Errors
    ///
    /// - [`Error::PaletteIndexOutOfRange`] if an indexed pixel refers to a color
    ///   past the end of the palette.
    /// - [`Error::ImageTooLarge`] if the bitmap doesn't fit in memory on this platform.
    pub fn to_bitmap(&self, format: PixelFormat) -> Result<Bitmap, Error> {
        let mut data = Vec::new();
        convert(&self.source(), format, &mut data)?;
        Ok(Bitmap::new(self.width(), self.height(), format, data))
    }

    pub(crate) fn source(&self) -> Source<'_> {
        Source { header: &self.header, stride: self.stride, data: &self.data, palette: self.palette.as_ref() }
    }
}

#[cfg(test)]
mod tests;
