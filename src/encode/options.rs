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

    /// Whether to convert images with few colors to indexed color, which is
    /// usually much smaller. [`PaletteMode::Keep`] by default.
    pub palette: PaletteMode,

    /// Which ancillary chunks to leave out, to make the file smaller.
    /// [`StripChunks::Keep`] by default.
    pub strip: StripChunks,
}

impl Default for EncodeOptions {
    fn default() -> Self {
        Self {
            compression: CompressionLevel::MEDIUM,
            compression_strategy: CompressionStrategy::Dynamic,
            filter: FilterStrategy::Adaptive,
            keep_unsafe_chunks: false,
            palette: PaletteMode::Keep,
            strip: StripChunks::Keep,
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

/// Whether the encoder may change an image to indexed color.
///
/// New modes may be added, so matches on it need a wildcard arm.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PaletteMode {
    /// Encode in the image's own color type.
    #[default]
    Keep,

    /// Convert 8-bit RGB and RGBA images with at most 256 distinct colors,
    /// counting alpha, to indexed color at the smallest bit depth that fits:
    /// 1, 2, 4 or 8 bits. The pixels don't change, so this is lossless; images
    /// with more colors, and other color types, are encoded as they are.
    ///
    /// Logos, icons, screenshots and pixel art often get several times smaller.
    /// Very small images may not: the palette costs up to about 1 KB in chunks,
    /// which can be more than it saves. So images with at most 16 KiB of pixel
    /// data, such as 64×64 RGBA, are encoded both ways and the smaller file is
    /// kept, the image as given on a tie.
    /// The file's color type changes, though: [`decode`](crate::decode) gives
    /// palette indices back, while [`decode_rgba8`](crate::decode_rgba8) gives
    /// the same pixels as before.
    ///
    /// When an image is converted:
    /// - Alpha is written as `tRNS`, and an RGB image's `tRNS` color becomes a
    ///   palette entry with alpha 0.
    /// - A suggested palette given with the image is replaced.
    /// - Extra chunks that aren't safe to copy, such as `bKGD` and `sBIT`, are
    ///   dropped even with [`EncodeOptions::keep_unsafe_chunks`]: they describe
    ///   the old color type.
    Auto,
}

/// Which ancillary chunks the encoder leaves out.
///
/// `tRNS` is always kept: it's part of the pixels, saying which are transparent.
/// The others only describe the image, but some change how viewers show it, so
/// [`Safe`](Self::Safe) keeps those. Chunks that are left out aren't validated,
/// so a bad one doesn't stop the encode.
///
/// Both [`Safe`](Self::Safe) and [`All`](Self::All) also drop the suggested
/// palette of an RGB or RGBA image: a `PLTE` chunk its pixels don't use, which
/// viewers ignore. An indexed image's palette holds its colors, so it's always kept.
///
/// New modes may be added, so matches on it need a wildcard arm.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum StripChunks {
    /// Write every chunk given.
    #[default]
    Keep,

    /// Keep the chunks that change how the image looks, and drop the rest:
    ///
    /// - Kept: `tRNS`; `cICP`, `iCCP`, `sRGB`, `gAMA` and `cHRM`, which browsers
    ///   use to show the colors right; and `pHYs`, which gives the aspect ratio.
    /// - Dropped: text, `tIME`, `eXIf`, every extra chunk from
    ///   [`ImageRef::with_chunks`](crate::ImageRef::with_chunks), and a
    ///   suggested palette.
    ///
    /// Exif can hold a photo's orientation, which some viewers apply, as well as
    /// the camera, location and a thumbnail. Dropping it saves space and keeps
    /// those private, but a rotated photo may then show sideways.
    Safe,

    /// Keep only `tRNS`, and an indexed image's palette, for the smallest file.
    /// Colors may look different in
    /// viewers that apply color management, such as browsers: an `iCCP`
    /// profile, a `cICP` color space or a `gAMA` far from 1/2.2 is lost.
    All,
}
