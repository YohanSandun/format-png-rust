use crate::png::{Image, ImageHeader, Metadata, OwnedChunk, Palette, Transparency};

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
    chunks: &'a [OwnedChunk],
}

impl<'a> ImageRef<'a> {
    /// An image with `header` and the pixels in `data`, and no other chunks.
    #[must_use]
    pub fn new(header: ImageHeader, data: &'a [u8]) -> Self {
        Self {
            header,
            data,
            palette: None,
            transparency: None,
            metadata: None,
            chunks: &[],
        }
    }

    /// Adds a `PLTE` chunk. Indexed images need one. RGB and RGBA images may
    /// have one, as a suggested palette; grayscale images must not.
    #[must_use]
    pub fn with_palette(mut self, palette: &'a Palette) -> Self {
        self.palette = Some(palette);
        self
    }

    /// Adds a `tRNS` chunk: one fully transparent color for grayscale and RGB
    /// images, or an alpha value per palette entry for indexed images. Images
    /// with an alpha channel must not have one.
    #[must_use]
    pub fn with_transparency(mut self, transparency: &'a Transparency) -> Self {
        self.transparency = Some(transparency);
        self
    }

    /// Adds the metadata chunks: color space, physical size, time, text, ICC
    /// profile and Exif; see [`Metadata`].
    #[must_use]
    pub fn with_metadata(mut self, metadata: &'a Metadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Adds extra chunks, written raw at their [`ChunkPosition`](crate::ChunkPosition)
    /// in the order given: chunks kept by
    /// [`DecodeOptions::preserve_chunks`](crate::DecodeOptions::preserve_chunks),
    /// or your own from [`OwnedChunk::from_data`].
    ///
    /// Critical chunks are an error. Chunks that aren't safe to copy are skipped
    /// unless [`EncodeOptions::keep_unsafe_chunks`](crate::EncodeOptions::keep_unsafe_chunks)
    /// is set, and so are chunks of a type the encoder writes itself from the
    /// transparency or metadata.
    #[must_use]
    pub fn with_chunks(mut self, chunks: &'a [OwnedChunk]) -> Self {
        self.chunks = chunks;
        self
    }

    /// The image's header.
    #[must_use]
    pub fn header(&self) -> &ImageHeader {
        &self.header
    }

    /// The pixels, in the layout described above.
    #[must_use]
    pub fn data(&self) -> &'a [u8] {
        self.data
    }

    /// The palette, if one was added.
    #[must_use]
    pub fn palette(&self) -> Option<&'a Palette> {
        self.palette
    }

    /// The transparency, if it was added.
    #[must_use]
    pub fn transparency(&self) -> Option<&'a Transparency> {
        self.transparency
    }

    /// The metadata, if it was added.
    #[must_use]
    pub fn metadata(&self) -> Option<&'a Metadata> {
        self.metadata
    }

    /// The extra chunks, empty unless some were added.
    #[must_use]
    pub fn chunks(&self) -> &'a [OwnedChunk] {
        self.chunks
    }
}

/// Re-encodes a decoded image, with its palette, transparency, metadata and
/// preserved chunks. The metadata and chunks are only there if the image was
/// decoded with [`DecodeOptions::preserve_metadata`](crate::DecodeOptions::preserve_metadata)
/// and [`DecodeOptions::preserve_chunks`](crate::DecodeOptions::preserve_chunks).
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
        image_ref.with_chunks(image.ancillary_chunks())
    }
}

#[cfg(test)]
mod tests;
