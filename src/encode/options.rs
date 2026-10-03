use rust_deflate::CompressionLevel;

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

    /// How to filter each row before compressing. [`FilterStrategy::Adaptive`] by default.
    pub filter: FilterStrategy,
}

impl Default for EncodeOptions {
    fn default() -> Self {
        Self {
            compression: CompressionLevel::MEDIUM,
            filter: FilterStrategy::Adaptive,
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
