use crate::png::{Chunk, ChunkType};

/// Where a chunk was in the file, relative to the chunks whose position the PNG
/// spec fixes. An encoder writing the chunk back needs this to put it in a valid place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChunkPosition {
    /// Before `PLTE` and the image data. For images without a `PLTE`, this is
    /// every chunk before the image data.
    BeforePalette,
    /// After `PLTE`, before the image data.
    BeforeImageData,
    /// After the image data, before `IEND`.
    AfterImageData,
}

/// A copy of one chunk, owned so it can outlive the input it was read from.
///
/// The decoder keeps these for ancillary chunks when
/// [`DecodeOptions::preserve_chunks`](crate::DecodeOptions::preserve_chunks) is
/// set, including private and unknown ones, so nothing in the file is lost.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OwnedChunk {
    chunk_type: ChunkType,
    data: Vec<u8>,
    position: ChunkPosition,
}

impl OwnedChunk {
    /// Copies `chunk`, found at `position`.
    pub(crate) fn new(chunk: &Chunk<'_>, position: ChunkPosition) -> Self {
        Self {
            chunk_type: chunk.chunk_type(),
            data: chunk.data().to_vec(),
            position,
        }
    }

    /// A chunk of your own, to write with
    /// [`ImageRef::with_chunks`](crate::ImageRef::with_chunks) at `position`.
    ///
    /// Use an ancillary, private type: a lowercase first and second letter, such
    /// as `myAp`. Make the fourth letter lowercase too if the data doesn't depend
    /// on the image's pixels, so editors may copy it; see
    /// [`ChunkType::is_safe_to_copy`].
    ///
    /// ```
    /// use format_png::{ChunkPosition, OwnedChunk};
    /// use format_png::png::ChunkType;
    ///
    /// let chunk = OwnedChunk::from_data(ChunkType::from_bytes(*b"myAp")?, b"settings".to_vec(), ChunkPosition::AfterImageData);
    /// assert_eq!(chunk.data(), b"settings");
    /// # Ok::<(), format_png::Error>(())
    /// ```
    #[must_use]
    pub fn from_data(chunk_type: ChunkType, data: Vec<u8>, position: ChunkPosition) -> Self {
        Self {
            chunk_type,
            data,
            position,
        }
    }

    /// The chunk's type.
    #[must_use]
    pub fn chunk_type(&self) -> ChunkType {
        self.chunk_type
    }

    /// The chunk's data, without the length, type and CRC fields.
    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Where the chunk was in the file.
    #[must_use]
    pub fn position(&self) -> ChunkPosition {
        self.position
    }
}
