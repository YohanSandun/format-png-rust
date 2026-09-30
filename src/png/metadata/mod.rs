//! Typed ancillary chunks: image metadata such as gamma, physical size and time.
//!
//! The decoder collects these into a [`Metadata`] when
//! [`DecodeOptions::preserve_metadata`](crate::DecodeOptions::preserve_metadata)
//! is set. Each type can also parse its chunk on its own, for chunks read with a
//! [`ChunkReader`](crate::ChunkReader):
//!
//! ```
//! use format_png::ChunkReader;
//! use format_png::png::ChunkType;
//! use format_png::png::metadata::Gamma;
//!
//! let data = std::fs::read("tests/data/valid/ancillary_chunks.png")?;
//! let mut chunks = ChunkReader::new(&data)?;
//! while let Some(chunk) = chunks.next_chunk()? {
//!     if chunk.chunk_type() == ChunkType::GAMA {
//!         println!("gamma {}", Gamma::parse(chunk.data())?.value());
//!     }
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod color;
mod physical;
mod time;

pub use color::{Chromaticities, Gamma, RenderingIntent};
pub use physical::{PhysicalDimensions, Unit};
pub use time::Time;

/// The known ancillary chunks of an image, parsed.
///
/// Each is `None` if the image doesn't have that chunk, if metadata wasn't
/// collected, or if the chunk was invalid or misplaced and
/// [`DecodeOptions::strict_ancillary`](crate::DecodeOptions::strict_ancillary)
/// is off. More chunks will be added, so there is no public constructor.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Metadata {
    pub(crate) gamma: Option<Gamma>,
    pub(crate) chromaticities: Option<Chromaticities>,
    pub(crate) srgb: Option<RenderingIntent>,
    pub(crate) physical_dimensions: Option<PhysicalDimensions>,
    pub(crate) time: Option<Time>,
}

impl Metadata {
    /// The `gAMA` chunk: the image's gamma.
    pub fn gamma(&self) -> Option<Gamma> {
        self.gamma
    }

    /// The `cHRM` chunk: the chromaticities of the image's primaries and white point.
    pub fn chromaticities(&self) -> Option<Chromaticities> {
        self.chromaticities
    }

    /// The `sRGB` chunk: the image is in the sRGB color space, with this rendering intent.
    pub fn srgb(&self) -> Option<RenderingIntent> {
        self.srgb
    }

    /// The `pHYs` chunk: the intended pixel size or aspect ratio.
    pub fn physical_dimensions(&self) -> Option<PhysicalDimensions> {
        self.physical_dimensions
    }

    /// The `tIME` chunk: when the image was last modified.
    pub fn time(&self) -> Option<Time> {
        self.time
    }

    /// Whether no known chunk was found.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}
