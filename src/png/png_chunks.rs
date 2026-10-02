use crate::png::{Chunk, ChunkType, ImageChunks, ImageHeader, Metadata, Palette, Transparency};

/// The chunk types this crate parses. Every other chunk is unknown to it.
const KNOWN: [ChunkType; 16] = [
    ChunkType::IHDR,
    ChunkType::PLTE,
    ChunkType::IDAT,
    ChunkType::IEND,
    ChunkType::TRNS,
    ChunkType::GAMA,
    ChunkType::CHRM,
    ChunkType::SRGB,
    ChunkType::PHYS,
    ChunkType::TIME,
    ChunkType::TEXT,
    ChunkType::ZTXT,
    ChunkType::ITXT,
    ChunkType::ICCP,
    ChunkType::CICP,
    ChunkType::EXIF,
];

/// Every chunk of a PNG, read and parsed, without decompressing the image data.
///
/// Returned by [`read_chunks`](crate::read_chunks) and
/// [`Decoder::read_chunks`](crate::Decoder::read_chunks). Reading chunks this way
/// is much faster than decoding, since the `IDAT` stream, usually most of the
/// file, is only checked for its CRC and position, never decompressed.
///
/// The known chunks are parsed into [`header`](Self::header),
/// [`palette`](Self::palette), [`transparency`](Self::transparency) and
/// [`metadata`](Self::metadata). Every chunk is also kept raw, in file order, in
/// [`chunks`](Self::chunks), including private, custom and unknown ones, critical
/// or not; [`unknown_chunks`](Self::unknown_chunks) gives just the ones this crate
/// doesn't parse. Raw chunks borrow from the input, so it must outlive this.
///
/// ```
/// let data = std::fs::read("tests/data/valid/ancillary_chunks.png")?;
/// let png = format_png::read_chunks(&data)?;
///
/// println!("{}x{}", png.header().width, png.header().height);
/// if let Some(gamma) = png.metadata().gamma() {
///     println!("gamma {}", gamma.value());
/// }
/// for text in png.metadata().text() {
///     println!("{}: {}", text.keyword, text.text);
/// }
/// for chunk in png.unknown_chunks() {
///     println!("{}: {} bytes", chunk.chunk_type(), chunk.data().len());
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PngChunks<'a> {
    header: ImageHeader,
    chunks: Vec<Chunk<'a>>,
    found: ImageChunks,
}

impl<'a> PngChunks<'a> {
    pub(crate) fn new(header: ImageHeader, chunks: Vec<Chunk<'a>>, found: ImageChunks) -> Self {
        Self { header, chunks, found }
    }

    /// The `IHDR` chunk: the image's size and pixel format.
    pub fn header(&self) -> &ImageHeader {
        &self.header
    }

    /// The `PLTE` chunk, if any. Indexed images always have one.
    pub fn palette(&self) -> Option<&Palette> {
        self.found.palette.as_ref()
    }

    /// The `tRNS` chunk, if any.
    pub fn transparency(&self) -> Option<&Transparency> {
        self.found.transparency.as_ref()
    }

    /// The known ancillary chunks, parsed, such as `gAMA`, `tIME` and text.
    ///
    /// Always collected here, whatever
    /// [`DecodeOptions::preserve_metadata`](crate::DecodeOptions::preserve_metadata)
    /// says. An invalid, misplaced or repeated one is left out unless
    /// [`DecodeOptions::strict_ancillary`](crate::DecodeOptions::strict_ancillary)
    /// is set, but is still in [`chunks`](Self::chunks).
    pub fn metadata(&self) -> &Metadata {
        &self.found.metadata
    }

    /// Every chunk from `IHDR` to `IEND`, in file order, raw. Anything after
    /// `IEND` isn't read.
    pub fn chunks(&self) -> &[Chunk<'a>] {
        &self.chunks
    }

    /// The chunks of `chunk_type`, in file order. Useful for chunks that can
    /// repeat, such as `IDAT` or your own.
    pub fn chunks_of_type(&self, chunk_type: ChunkType) -> impl Iterator<Item = Chunk<'a>> + '_ {
        self.chunks.iter().copied().filter(move |chunk| chunk.chunk_type() == chunk_type)
    }

    /// The chunks this crate doesn't parse, in file order: private and custom
    /// chunks, critical or not, and public ones not supported yet, such as `bKGD`.
    /// Read them with [`Chunk::data`].
    pub fn unknown_chunks(&self) -> impl Iterator<Item = Chunk<'a>> + '_ {
        self.chunks.iter().copied().filter(|chunk| !KNOWN.contains(&chunk.chunk_type()))
    }
}

#[cfg(test)]
mod tests;
