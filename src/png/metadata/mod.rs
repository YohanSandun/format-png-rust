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

mod cicp;
mod color;
mod exif;
mod icc;
mod physical;
mod text;
mod time;

pub use cicp::CodingIndependentCodePoints;
pub use color::{Chromaticities, Gamma, RenderingIntent};
pub use exif::{Exif, ExifByteOrder};
pub use icc::IccProfile;
pub use physical::{PhysicalDimensions, Unit};
pub use text::{Text, TextKind};
pub use time::Time;

/// The known ancillary chunks of an image, parsed.
///
/// Each is `None` if the image doesn't have that chunk, if metadata wasn't
/// collected, or if the chunk was invalid or misplaced and
/// [`DecodeOptions::strict_ancillary`](crate::DecodeOptions::strict_ancillary)
/// is off.
///
/// To encode an image with metadata, start from [`Metadata::default`] and add
/// chunks with the `with_` methods, then pass it to
/// [`ImageRef::with_metadata`](crate::ImageRef::with_metadata). More chunks
/// will be added, so the fields are private.
///
/// ```
/// use format_png::Metadata;
/// use format_png::png::metadata::{PhysicalDimensions, Text, TextKind, Unit};
///
/// let metadata = Metadata::default()
///     .with_physical_dimensions(PhysicalDimensions { x: 2835, y: 2835, unit: Unit::Meter })
///     .with_text(Text {
///         keyword: "Title".to_string(),
///         text: "A sunset".to_string(),
///         language_tag: String::new(),
///         translated_keyword: String::new(),
///         kind: TextKind::Plain,
///     });
/// assert_eq!(metadata.text().len(), 1);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Metadata {
    pub(crate) gamma: Option<Gamma>,
    pub(crate) chromaticities: Option<Chromaticities>,
    pub(crate) srgb: Option<RenderingIntent>,
    pub(crate) physical_dimensions: Option<PhysicalDimensions>,
    pub(crate) time: Option<Time>,
    pub(crate) text: Vec<Text>,
    pub(crate) icc_profile: Option<IccProfile>,
    pub(crate) cicp: Option<CodingIndependentCodePoints>,
    pub(crate) exif: Option<Exif>,
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

    /// The `tEXt`, `zTXt` and `iTXt` chunks, in file order. A keyword may appear
    /// more than once.
    pub fn text(&self) -> &[Text] {
        &self.text
    }

    /// The `iCCP` chunk: the image's embedded ICC color profile.
    pub fn icc_profile(&self) -> Option<&IccProfile> {
        self.icc_profile.as_ref()
    }

    /// The `cICP` chunk: the image's color space as coding-independent code
    /// points. Takes precedence over every other color chunk.
    pub fn cicp(&self) -> Option<CodingIndependentCodePoints> {
        self.cicp
    }

    /// The `eXIf` chunk: the image's Exif metadata, raw.
    pub fn exif(&self) -> Option<&Exif> {
        self.exif.as_ref()
    }

    /// Sets the `gAMA` chunk.
    pub fn with_gamma(mut self, gamma: Gamma) -> Self {
        self.gamma = Some(gamma);
        self
    }

    /// Sets the `cHRM` chunk.
    pub fn with_chromaticities(mut self, chromaticities: Chromaticities) -> Self {
        self.chromaticities = Some(chromaticities);
        self
    }

    /// Sets the `sRGB` chunk.
    pub fn with_srgb(mut self, intent: RenderingIntent) -> Self {
        self.srgb = Some(intent);
        self
    }

    /// Sets the `pHYs` chunk.
    pub fn with_physical_dimensions(mut self, dimensions: PhysicalDimensions) -> Self {
        self.physical_dimensions = Some(dimensions);
        self
    }

    /// Sets the `tIME` chunk.
    pub fn with_time(mut self, time: Time) -> Self {
        self.time = Some(time);
        self
    }

    /// Adds a `tEXt`, `zTXt` or `iTXt` chunk, after any added before; which one
    /// is written depends on its [`TextKind`]. A keyword may be used more than once.
    pub fn with_text(mut self, text: Text) -> Self {
        self.text.push(text);
        self
    }

    /// Sets the `iCCP` chunk.
    pub fn with_icc_profile(mut self, icc_profile: IccProfile) -> Self {
        self.icc_profile = Some(icc_profile);
        self
    }

    /// Sets the `cICP` chunk.
    pub fn with_cicp(mut self, cicp: CodingIndependentCodePoints) -> Self {
        self.cicp = Some(cicp);
        self
    }

    /// Sets the `eXIf` chunk.
    pub fn with_exif(mut self, exif: Exif) -> Self {
        self.exif = Some(exif);
        self
    }

    /// Whether no known chunk was found.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

#[cfg(test)]
mod tests;
