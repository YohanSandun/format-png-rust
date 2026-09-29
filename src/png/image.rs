use crate::png::ImageHeader;

/// A decoded image, in the PNG's own pixel format.
///
/// Pixels aren't converted. Rows are stored top to bottom, [`stride`](Self::stride)
/// bytes each, with the samples and bit depth that [`header`](Self::header) gives:
///
/// - 16-bit samples are big-endian.
/// - Pixels under 8 bits are packed most significant bits first, and each row is
///   padded to a whole byte.
/// - Indexed images hold palette indices, not colors.
/// - Interlaced images have already been put back together.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    header: ImageHeader,
    stride: usize,
    data: Vec<u8>,
}

impl Image {
    pub(crate) fn new(header: ImageHeader, stride: usize, data: Vec<u8>) -> Self {
        debug_assert_eq!(data.len(), stride * header.height as usize);
        Self { header, stride, data }
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
        todo!()
    }

    /// Consumes the image and returns its pixel data.
    pub fn into_data(self) -> Vec<u8> {
        self.data
    }
}

#[cfg(test)]
mod tests;
