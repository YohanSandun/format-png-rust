use rust_deflate::{CompressionLevel, Strategy as CompressionStrategy};

use crate::png::FilterType;

/// Settings for an [`Encoder`](crate::Encoder).
///
/// Use struct update syntax to change only some of them:
///
/// ```
/// use format_png::{CompressionLevel, EncodeOptions};
///
/// let options = EncodeOptions { compression: CompressionLevel::BEST, ..EncodeOptions::default() };
/// ```
#[derive(Debug, Clone)]
pub struct EncodeOptions {
    /// How hard to compress the image data. [`CompressionLevel::MEDIUM`] by default.
    pub compression: CompressionLevel,

    /// Which kind of DEFLATE blocks the image data is compressed into.
    /// [`CompressionStrategy::Dynamic`] by default, which gives the smallest
    /// files. [`CompressionStrategy::Fixed`] is a little faster;
    /// [`CompressionStrategy::Stored`] doesn't compress at all, as does
    /// [`CompressionLevel::NONE`] whatever the strategy.
    pub compression_strategy: CompressionStrategy,

    /// How to filter each row before compressing. [`FilterStrategy::Adaptive`] by default.
    pub filter: FilterStrategy,

    /// Whether to write extra chunks that aren't safe to copy, such as `bKGD`
    /// and `sBIT`: those with an uppercase fourth letter, whose data depends on
    /// the image. Off by default, as the spec asks of editors that rewrite the
    /// image data. Turn it on to keep them when the pixels, header and palette
    /// are unchanged, as when re-encoding a decoded image, or to write your own.
    /// See [`ImageRef::with_chunks`](crate::ImageRef::with_chunks).
    pub keep_unsafe_chunks: bool,
}

impl Default for EncodeOptions {
    fn default() -> Self {
        Self {
            compression: CompressionLevel::MEDIUM,
            compression_strategy: CompressionStrategy::Dynamic,
            filter: FilterStrategy::Adaptive,
            keep_unsafe_chunks: false,
        }
    }
}

/// How the encoder picks the filter for each row.
///
/// Filtering doesn't change the pixels, only how well they compress. New
/// strategies may be added, so matches on it need a wildcard arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FilterStrategy {
    /// Every row uses this filter. [`FilterType::None`] is fastest; the others
    /// help some images.
    Fixed(FilterType),

    /// Each row gets the filter that minimizes the sum of the absolute values of
    /// its filtered bytes, read as signed. That's the heuristic the PNG spec
    /// suggests, and libpng's default.
    ///
    /// Indexed images and images under 8 bits per sample use
    /// [`FilterType::None`] instead, as the spec recommends: filtering rarely
    /// helps them.
    Adaptive,
}
