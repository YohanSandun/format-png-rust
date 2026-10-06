use std::num::NonZeroUsize;

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
    ///
    /// Higher levels are slower, especially on photos. On a 2500×3800 photo,
    /// level 4 is about 7 times faster than level 6 for a file 6% larger, while
    /// levels 7 to 9 are 1.4 to 2 times slower for one under 1% smaller. To make
    /// large images faster without giving up compression, see [`threads`](Self::threads).
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

    /// How many threads compress the image data. [`Threads::Single`] by default.
    /// Large images compress several times faster on several threads.
    pub threads: Threads,
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
            threads: Threads::Single,
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

/// How many threads the encoder compresses the image data on.
///
/// Compression is most of the time spent encoding, and on photos at the higher
/// levels it's slow: several seconds for a 10-megapixel image on one thread.
/// [`Auto`](Self::Auto) and [`Count`](Self::Count) split the image data into
/// segments of 1 MiB and compress them in parallel, which on a 2500×3800 photo
/// at the default level takes about 1 second instead of 7 on 8 or more cores.
///
/// - Each segment is compressed without the one before it, so files come out
///   slightly larger: about 0.1%.
/// - The segments are a fixed size, so the output depends only on the image and
///   the options, never on the number of threads or the machine: `Auto`
///   and every `Count` give the same bytes.
/// - Image data of 1 MiB or less is a single segment, compressed as with
///   [`Single`](Self::Single).
/// - Where threads aren't available, as on `wasm32-unknown-unknown`, everything
///   runs on the calling thread, with the same output. To spread the segments
///   over Web Workers or other processes instead, use
///   [`Encoder::prepare`](crate::Encoder::prepare).
///
/// New modes may be added, so matches on it need a wildcard arm.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Threads {
    /// Compress the image data as one stream on the calling thread: the smallest
    /// output, and no threads started.
    #[default]
    Single,

    /// Compress in parallel on as many threads as the machine can run at once,
    /// from [`std::thread::available_parallelism`].
    Auto,

    /// Compress in parallel on this many threads, counting the calling thread.
    /// `Count(1)` compresses the segments on the calling thread alone, giving the
    /// same output as `Auto`.
    Count(NonZeroUsize),
}

impl Threads {
    /// The number of threads to use, or `None` for [`Threads::Single`].
    pub(crate) fn parallel_count(self) -> Option<usize> {
        match self {
            Threads::Single => None,
            Threads::Auto => {
                Some(std::thread::available_parallelism().map_or(1, NonZeroUsize::get))
            }
            Threads::Count(count) => Some(count.get()),
        }
    }
}
