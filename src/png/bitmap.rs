/// A pixel layout that images can be converted to.
///
/// New formats may be added later, so matches on it need a wildcard arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PixelFormat {
    /// 8-bit red, green and blue: 3 bytes per pixel.
    Rgb8,
    /// 8-bit red, green, blue and alpha: 4 bytes per pixel. Alpha isn't
    /// premultiplied. This is the layout of a browser canvas's `ImageData`.
    Rgba8,
}

impl PixelFormat {
    /// Bytes per pixel: 3 for [`Rgb8`](Self::Rgb8), 4 for [`Rgba8`](Self::Rgba8).
    pub fn bytes_per_pixel(self) -> usize {
        match self {
            PixelFormat::Rgb8 => 3,
            PixelFormat::Rgba8 => 4
        }
    }
}

/// An image converted to a [`PixelFormat`], ready to display.
///
/// Rows are stored top to bottom with no padding, so [`data`](Self::data) is
/// exactly `width × height × bytes_per_pixel` bytes. An [`Rgba8`](PixelFormat::Rgba8)
/// bitmap can go straight into a browser canvas's `ImageData`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bitmap {
    width: u32,
    height: u32,
    format: PixelFormat,
    data: Vec<u8>,
}

impl Bitmap {
    pub(crate) fn new(width: u32, height: u32, format: PixelFormat, data: Vec<u8>) -> Self {
        debug_assert_eq!(data.len(), width as usize * height as usize * format.bytes_per_pixel());
        Self { width, height, format, data }
    }

    /// Width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// The pixel format.
    pub fn format(&self) -> PixelFormat {
        self.format
    }

    /// Bytes per row: width × bytes per pixel, as rows have no padding.
    pub fn stride(&self) -> usize {
        self.width as usize * self.format.bytes_per_pixel()
    }

    /// All the pixel data.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Consumes the bitmap and returns its pixel data.
    pub fn into_data(self) -> Vec<u8> {
        self.data
    }
}

#[cfg(test)]
mod tests;
