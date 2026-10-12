use rust_deflate::{CompressionOptions, Compressor};

use super::animation_ref::{AnimationRef, FrameRef};
use super::chunk_writer::{
    MAX_CHUNK_LENGTH, header_data, transparency_data, write_chunk, write_frame_data,
    write_image_data, write_signature,
};
use super::crop::crop_frames;
use super::filter::filter_image;
use super::image_ref::ImageRef;
use super::options::{EncodeOptions, PaletteMode, StripChunks};
use super::palettize::{palettize, palettize_all};
use super::parallel::{SEGMENT_SIZE, SegmentCompression, compress_zlib_segments};
use super::prepared::{Deferred, PreparedPng};
use super::strip::strip_metadata;
use crate::encode::extra_chunks::{validate_extra_chunks, write_extra_chunks};
use crate::encode::metadata::{
    validate_metadata, write_metadata_after_palette, write_metadata_before_palette,
};
use crate::error::Error;
use crate::png::{AnimationControl, ChunkType, Metadata, Palette, Transparency};
use crate::{ChunkPosition, ColorType, ImageHeader};

/// The largest image data, in bytes, for which `PaletteMode::Auto` also
/// encodes the image as given and keeps the smaller file: 64×64 RGBA. The
/// palette's `PLTE` and `tRNS` chunks cost up to about 1 KB, so above this the
/// converted image is smaller in practice, and encoding twice wouldn't pay.
pub(crate) const AUTO_PALETTE_COMPARE_LIMIT: usize = 16 * 1024;

/// A reusable PNG encoder.
///
/// It owns a [`Compressor`] and its working buffers, and keeps them between
/// images, so encoding many PNGs with one `Encoder` avoids setting them up for
/// each one. [`encode_into`](Self::encode_into) also reuses your output buffer.
/// For a single image, [`encode`](crate::encode) is simpler.
///
/// The PNG written has the signature, `IHDR`, `PLTE` if the image has a
/// palette, `tRNS` if it has transparency, the image data as one or more `IDAT`
/// chunks, and `IEND`.
///
/// The image is interlaced if its header says
/// [`Interlace::Adam7`](crate::Interlace::Adam7).
///
/// Metadata chunks are written around `PLTE`, in the order the spec allows;
/// see [`ImageRef::with_metadata`].
///
/// Extra chunks, such as the raw ones kept by
/// [`DecodeOptions::preserve_chunks`](crate::DecodeOptions::preserve_chunks),
/// are written at their positions; see [`ImageRef::with_chunks`].
///
/// With [`PaletteMode::Auto`], images with few colors are written as indexed color.
#[derive(Debug)]
pub struct Encoder {
    options: EncodeOptions,
    compressor: Compressor,
    /// The filtered rows, each with its filter type byte: what's compressed.
    scanlines: Vec<u8>,
    /// The zlib stream, before it's split into `IDAT` chunks.
    compressed: Vec<u8>,
    /// Palette indices, when `PaletteMode::Auto` converts an image.
    indexed: Vec<u8>,
    /// The PNG written as given, when `PaletteMode::Auto` compares it with the
    /// converted one; see `AUTO_PALETTE_COMPARE_LIMIT`.
    unconverted: Vec<u8>,
}

impl Encoder {
    /// Creates an encoder with [`EncodeOptions::default`].
    #[must_use]
    pub fn new() -> Self {
        Self::with_options(EncodeOptions::default())
    }

    /// Creates an encoder with the given options.
    #[must_use]
    pub fn with_options(options: EncodeOptions) -> Self {
        Self {
            options,
            compressor: Compressor::new(),
            scanlines: Vec::new(),
            compressed: Vec::new(),
            indexed: Vec::new(),
            unconverted: Vec::new(),
        }
    }

    /// The options this encoder was created with.
    #[must_use]
    pub fn options(&self) -> &EncodeOptions {
        &self.options
    }

    /// Encodes `image` to a PNG file's bytes.
    ///
    /// # Errors
    ///
    /// Same as [`encode_into`](Self::encode_into).
    pub fn encode(&mut self, image: ImageRef<'_>) -> Result<Vec<u8>, Error> {
        let mut out = Vec::new();
        self.encode_into(image, &mut out)?;
        Ok(out)
    }

    /// Like [`encode`](Self::encode), but writes the PNG to `out`. `out`'s old
    /// contents are replaced and its allocation is reused, so passing the same
    /// buffer for many images avoids allocating one each time. After an error,
    /// `out`'s contents are unspecified.
    ///
    /// # Errors
    ///
    /// Any error from `validate`:
    /// - [`Error::InvalidDimensions`] or [`Error::InvalidBitDepth`] if the header
    ///   isn't valid, as [`ImageHeader::parse`](crate::ImageHeader::parse) checks.
    /// - [`Error::InvalidImageDataLength`] if the pixel data isn't
    ///   [`ImageHeader::image_size`](crate::ImageHeader::image_size) bytes.
    /// - [`Error::MissingPalette`] for an indexed image without a palette, and
    ///   [`Error::UnexpectedPalette`] for a grayscale image with one.
    /// - [`Error::TooManyPaletteEntries`] if an indexed image's palette has more
    ///   entries than its bit depth can index.
    /// - [`Error::ImageTooLarge`] if the image doesn't fit in memory on this platform.
    pub fn encode_into(&mut self, image: ImageRef<'_>, out: &mut Vec<u8>) -> Result<(), Error> {
        self.run(image, out, false).map(|_| ())
    }

    /// Does everything [`encode_into`](Self::encode_into) does except compress the
    /// image data, which is left in segments to compress anywhere: on other
    /// threads, or in other WebAssembly instances such as Web Workers. See
    /// [`PreparedPng`].
    ///
    /// The segments are the same as with [`Threads::Auto`](crate::Threads::Auto),
    /// whatever [`EncodeOptions::threads`] says, so the PNG
    /// [`PreparedPng::finish`] writes is the one `Threads::Auto` would.
    ///
    /// # Errors
    ///
    /// Same as [`encode_into`](Self::encode_into).
    pub fn prepare(&mut self, image: ImageRef<'_>) -> Result<PreparedPng, Error> {
        let mut head = Vec::new();
        let deferred = self.run(image, &mut head, true)?;
        let compression = SegmentCompression {
            level: self.options.compression,
            strategy: self.options.compression_strategy,
        };
        Ok(PreparedPng::new(head, deferred, compression))
    }

    /// Strips, validates and writes `image` to `out`. With `defer`, image data
    /// over one segment isn't compressed: `out` gets everything before it, and
    /// the rest is returned.
    fn run(
        &mut self,
        image: ImageRef<'_>,
        out: &mut Vec<u8>,
        defer: bool,
    ) -> Result<Option<Deferred>, Error> {
        let mut stripped_metadata = None;
        let image = strip(image, self.options.strip, &mut stripped_metadata);

        validate(&image)?;

        // The converted image borrows this buffer while `write_png` borrows the
        // encoder, so it's taken out for the call and put back after, even on error.
        let mut indexed = std::mem::take(&mut self.indexed);
        let result = self.encode_validated(image, &mut indexed, out, defer);
        self.indexed = indexed;
        result
    }

    /// `run` after stripping and validation: converts the image to indexed color
    /// if `PaletteMode::Auto` can, with the indices in `indexed`, and writes the
    /// smaller of the two for small images.
    fn encode_validated(
        &mut self,
        image: ImageRef<'_>,
        indexed: &mut Vec<u8>,
        out: &mut Vec<u8>,
        defer: bool,
    ) -> Result<Option<Deferred>, Error> {
        let palettized = match self.options.palette {
            PaletteMode::Auto => {
                palettize(image.header(), image.data(), image.transparency(), indexed)
            }
            _ => None,
        };
        let Some(palettized) = palettized else {
            return self.write_png(&image, self.options.keep_unsafe_chunks, out, defer);
        };

        let mut converted = ImageRef::new(palettized.header, indexed)
            .with_palette(&palettized.palette)
            .with_chunks(image.chunks());
        if let Some(transparency) = &palettized.transparency {
            converted = converted.with_transparency(transparency);
        }
        if let Some(metadata) = image.metadata() {
            converted = converted.with_metadata(metadata);
        }
        // Unsafe-to-copy chunks describe the image as given; after a change of
        // color type they'd be wrong.
        let deferred = self.write_png(&converted, false, out, defer)?;

        // The palette's chunks cost up to about 1 KB, which a small image may not
        // win back. Encoding it again as given is cheap there, so keep whichever
        // is smaller, and the image as given on a tie.
        // Images that small are a single segment, so nothing is ever deferred here.
        if image.data().len() <= AUTO_PALETTE_COMPARE_LIMIT {
            debug_assert!(deferred.is_none());
            let mut unconverted = std::mem::take(&mut self.unconverted);
            let result = self
                .write_png(
                    &image,
                    self.options.keep_unsafe_chunks,
                    &mut unconverted,
                    false,
                )
                .map(|_| ());
            if result.is_ok() && unconverted.len() <= out.len() {
                std::mem::swap(out, &mut unconverted);
            }
            self.unconverted = unconverted;
            result?;
        }

        Ok(deferred)
    }

    /// Filters, compresses and writes `image` as a PNG to `out`, replacing its
    /// contents. `image` has been validated. `keep_unsafe` is
    /// `EncodeOptions::keep_unsafe_chunks`, or `false` for a converted image.
    ///
    /// With `defer`, filtered data of more than one segment isn't compressed:
    /// `out` gets everything before the image data, and the filtered data and
    /// everything after it are returned.
    fn write_png(
        &mut self,
        image: &ImageRef<'_>,
        keep_unsafe: bool,
        out: &mut Vec<u8>,
        defer: bool,
    ) -> Result<Option<Deferred>, Error> {
        out.clear();
        self.compressed.clear();

        let header = image.header();
        let compression = self.compression_options();

        self.filter(header, image.data())?;
        let defer = defer && self.scanlines.len() > SEGMENT_SIZE;
        if !defer {
            self.compress_scanlines();
        }

        write_signature(out);
        write_chunk(out, ChunkType::IHDR, &header_data(header));

        if let Some(metadata) = image.metadata() {
            write_metadata_before_palette(out, metadata, &mut self.compressor, compression);
        }

        write_extra_chunks(out, image, ChunkPosition::BeforePalette, keep_unsafe);

        if let Some(palette) = image.palette() {
            write_chunk(out, ChunkType::PLTE, palette.colors().as_flattened());
        }

        if let Some(transparency) = image.transparency() {
            write_chunk(out, ChunkType::TRNS, &transparency_data(transparency));
        }

        if let Some(metadata) = image.metadata() {
            write_metadata_after_palette(out, metadata, &mut self.compressor, compression);
        }

        write_extra_chunks(out, image, ChunkPosition::BeforeImageData, keep_unsafe);

        if defer {
            let mut tail = Vec::new();
            write_extra_chunks(&mut tail, image, ChunkPosition::AfterImageData, keep_unsafe);
            write_chunk(&mut tail, ChunkType::IEND, &[]);
            // Moved out rather than copied; the encoder grows a new buffer next time.
            let scanlines = std::mem::take(&mut self.scanlines);
            return Ok(Some(Deferred { scanlines, tail }));
        }

        write_image_data(out, &self.compressed, MAX_CHUNK_LENGTH);

        write_extra_chunks(out, image, ChunkPosition::AfterImageData, keep_unsafe);

        write_chunk(out, ChunkType::IEND, &[]);
        Ok(None)
    }

    fn compression_options(&self) -> CompressionOptions {
        CompressionOptions::new()
            .level(self.options.compression)
            .strategy(self.options.compression_strategy)
    }

    /// Filters the rows of `data`, an image with `header`, into `self.scanlines`,
    /// replacing its contents.
    fn filter(&mut self, header: &ImageHeader, data: &[u8]) -> Result<(), Error> {
        self.scanlines.clear();
        filter_image(self.options.filter, header, data, &mut self.scanlines)
    }

    /// Compresses `self.scanlines` into `self.compressed`, replacing its
    /// contents: in segments on several threads if the options ask for it and
    /// there's more than one segment, otherwise as one stream.
    fn compress_scanlines(&mut self) {
        self.compressed.clear();
        let segmented = self.scanlines.len() > SEGMENT_SIZE;
        match self.options.threads.parallel_count() {
            // A single segment would compress the same way on one thread.
            Some(threads) if segmented => compress_zlib_segments(
                &self.scanlines,
                SegmentCompression {
                    level: self.options.compression,
                    strategy: self.options.compression_strategy,
                },
                threads,
                &mut self.compressed,
            ),
            _ => {
                let compression = self.compression_options();
                self.compressor.compress_zlib_into_with(
                    &self.scanlines,
                    &mut self.compressed,
                    compression,
                );
            }
        }
    }

    /// Encodes an animated PNG (APNG); see [`AnimationRef`].
    ///
    /// # Errors
    ///
    /// Same as [`encode_animation_into`](Self::encode_animation_into).
    pub fn encode_animation(&mut self, animation: AnimationRef<'_>) -> Result<Vec<u8>, Error> {
        let mut out = Vec::new();
        self.encode_animation_into(animation, &mut out)?;
        Ok(out)
    }

    /// Like [`encode_animation`](Self::encode_animation), but writes the PNG to
    /// `out`, as [`encode_into`](Self::encode_into) does.
    ///
    /// The PNG has the chunks [`encode_into`](Self::encode_into) writes, with an
    /// `acTL` chunk after `IHDR`, and each frame as an `fcTL` chunk followed by
    /// its image data: `IDAT` chunks for the first frame, unless there's a
    /// separate default image, and `fdAT` chunks for the others.
    ///
    /// The options apply as for a single image, and
    /// [`EncodeOptions::crop_frames`] crops the frames to what changed.
    /// [`PaletteMode::Auto`] converts the animation to indexed color if every
    /// frame and the default image together have at most 256 colors, since
    /// they share one palette, and compares the two when they total at most
    /// 16 KiB of pixel data.
    ///
    /// # Errors
    ///
    /// - [`Error::FrameCountMismatch`] if there are no frames, or more than
    ///   fit in a `u32`.
    /// - [`Error::InvalidChunkData`] for [`ChunkType::FCTL`] if there's no
    ///   default image and the first frame doesn't cover the whole canvas.
    /// - Any error from [`encode_into`](Self::encode_into) for the canvas: its
    ///   header, palette, transparency, metadata and extra chunks, and the
    ///   default image's pixels, which are the first frame's unless there's a
    ///   default image.
    /// - [`Error::InvalidDimensions`] if a frame's width or height is zero or
    ///   over 2^31 - 1, and [`Error::FrameOutOfBounds`] if it doesn't fit on
    ///   the canvas.
    /// - [`Error::InvalidImageDataLength`] if a frame's pixels are the wrong size.
    pub fn encode_animation_into(
        &mut self,
        animation: AnimationRef<'_>,
        out: &mut Vec<u8>,
    ) -> Result<(), Error> {
        let mut stripped_metadata = None;
        let image = strip(
            animation.image(),
            self.options.strip,
            &mut stripped_metadata,
        );
        let has_default_image = animation.default_image().is_some();

        validate_frames(image.header(), animation.frames(), has_default_image)?;
        validate(&image)?;

        let cropped = if self.options.crop_frames {
            crop_frames(image.header(), animation.frames())
        } else {
            Vec::new()
        };
        let frames: Vec<FrameRef<'_>> = animation
            .frames()
            .iter()
            .enumerate()
            .map(|(i, frame)| match cropped.get(i) {
                Some(Some(cropped)) => frame.with_area(cropped.area).with_data(&cropped.data),
                _ => *frame,
            })
            .collect();
        let animation = Animation {
            image,
            frames: &frames,
            has_default_image,
            num_plays: animation.num_plays(),
        };

        // As in `run`: the converted frames borrow this buffer.
        let mut indexed = std::mem::take(&mut self.indexed);
        let result = self.encode_animation_validated(&animation, &mut indexed, out);
        self.indexed = indexed;
        result
    }

    /// `encode_animation_into` after stripping, validation and cropping: like
    /// `encode_validated`, converts the animation to indexed color if
    /// `PaletteMode::Auto` can, with the indices in `indexed`, and writes the
    /// smaller of the two for small animations.
    fn encode_animation_validated(
        &mut self,
        animation: &Animation<'_>,
        indexed: &mut Vec<u8>,
        out: &mut Vec<u8>,
    ) -> Result<(), Error> {
        let header = animation.image.header();
        let mut images = Vec::with_capacity(animation.frames.len() + 1);
        if animation.has_default_image {
            images.push((*header, animation.image.data()));
        }
        images.extend(
            animation
                .frames
                .iter()
                .map(|frame| (frame.header(header), frame.data())),
        );

        let palettized = match self.options.palette {
            PaletteMode::Auto => palettize_all(&images, animation.image.transparency(), indexed),
            _ => None,
        };
        let Some((palettized, ranges)) = palettized else {
            return self.write_animation(animation, self.options.keep_unsafe_chunks, out);
        };

        // The first range is the default image's: the first frame's unless
        // there's a separate one.
        let frame_ranges = &ranges[usize::from(animation.has_default_image)..];
        let frames: Vec<FrameRef<'_>> = animation
            .frames
            .iter()
            .zip(frame_ranges)
            .map(|(frame, range)| frame.with_data(&indexed[range.clone()]))
            .collect();
        let image = &animation.image;
        let mut converted = ImageRef::new(palettized.header, &indexed[ranges[0].clone()])
            .with_palette(&palettized.palette)
            .with_chunks(image.chunks());
        if let Some(transparency) = &palettized.transparency {
            converted = converted.with_transparency(transparency);
        }
        if let Some(metadata) = image.metadata() {
            converted = converted.with_metadata(metadata);
        }
        let converted = Animation {
            image: converted,
            frames: &frames,
            ..*animation
        };
        // As in `encode_validated`, unsafe-to-copy chunks describe the old color type.
        self.write_animation(&converted, false, out)?;

        // As in `encode_validated`, keep the smaller of the two for small animations.
        let size: usize = images.iter().map(|(_, data)| data.len()).sum();
        if size <= AUTO_PALETTE_COMPARE_LIMIT {
            let mut unconverted = std::mem::take(&mut self.unconverted);
            let result =
                self.write_animation(animation, self.options.keep_unsafe_chunks, &mut unconverted);
            if result.is_ok() && unconverted.len() <= out.len() {
                std::mem::swap(out, &mut unconverted);
            }
            self.unconverted = unconverted;
            result?;
        }

        Ok(())
    }

    /// Filters, compresses and writes `animation` as an APNG to `out`,
    /// replacing its contents, as `write_png` does for an image.
    fn write_animation(
        &mut self,
        animation: &Animation<'_>,
        keep_unsafe: bool,
        out: &mut Vec<u8>,
    ) -> Result<(), Error> {
        out.clear();
        let image = &animation.image;
        let header = image.header();
        let compression = self.compression_options();

        write_signature(out);
        write_chunk(out, ChunkType::IHDR, &header_data(header));

        let control = AnimationControl {
            // `validate_frames` checked it fits.
            num_frames: animation.frames.len() as u32,
            num_plays: animation.num_plays,
        };
        write_chunk(out, ChunkType::ACTL, &control.to_bytes());

        if let Some(metadata) = image.metadata() {
            write_metadata_before_palette(out, metadata, &mut self.compressor, compression);
        }
        write_extra_chunks(out, image, ChunkPosition::BeforePalette, keep_unsafe);
        if let Some(palette) = image.palette() {
            write_chunk(out, ChunkType::PLTE, palette.colors().as_flattened());
        }
        if let Some(transparency) = image.transparency() {
            write_chunk(out, ChunkType::TRNS, &transparency_data(transparency));
        }
        if let Some(metadata) = image.metadata() {
            write_metadata_after_palette(out, metadata, &mut self.compressor, compression);
        }
        write_extra_chunks(out, image, ChunkPosition::BeforeImageData, keep_unsafe);

        if animation.has_default_image {
            self.filter(header, image.data())?;
            self.compress_scanlines();
            write_image_data(out, &self.compressed, MAX_CHUNK_LENGTH);
        }

        let mut sequence = 0u32;
        for (index, frame) in animation.frames.iter().enumerate() {
            write_chunk(out, ChunkType::FCTL, &frame.control(sequence).to_bytes());
            sequence += 1;

            self.filter(&frame.header(header), frame.data())?;
            self.compress_scanlines();
            if index == 0 && !animation.has_default_image {
                write_image_data(out, &self.compressed, MAX_CHUNK_LENGTH);
            } else {
                write_frame_data(out, &self.compressed, &mut sequence, MAX_CHUNK_LENGTH);
            }
        }

        write_extra_chunks(out, image, ChunkPosition::AfterImageData, keep_unsafe);
        write_chunk(out, ChunkType::IEND, &[]);
        Ok(())
    }
}

/// An animation ready to write: stripped, validated, and maybe cropped or
/// converted, so its frames and pixels may no longer be the caller's.
#[derive(Debug, Clone, Copy)]
struct Animation<'a> {
    /// The canvas, with the default image's pixels: the first frame's unless
    /// `has_default_image`.
    image: ImageRef<'a>,
    frames: &'a [FrameRef<'a>],
    has_default_image: bool,
    num_plays: u32,
}

impl Default for Encoder {
    fn default() -> Self {
        Self::new()
    }
}

/// Drops the chunks `mode` doesn't keep from `image`; see [`StripChunks`].
/// The stripped metadata is stored in `metadata`, which the result borrows.
fn strip<'a>(
    image: ImageRef<'a>,
    mode: StripChunks,
    metadata: &'a mut Option<Metadata>,
) -> ImageRef<'a> {
    if mode == StripChunks::Keep {
        return image;
    }

    *metadata = image
        .metadata()
        .map(|metadata| strip_metadata(metadata, mode));
    // No extra chunks: neither mode keeps any.
    let mut stripped = ImageRef::new(*image.header(), image.data());
    // An indexed image needs its palette. Any other image's palette is
    // only a suggestion that viewers ignore, so it goes too.
    if let Some(palette) = image
        .palette()
        .filter(|_| image.header().color_type == ColorType::Indexed)
    {
        stripped = stripped.with_palette(palette);
    }
    if let Some(transparency) = image.transparency() {
        stripped = stripped.with_transparency(transparency);
    }
    if let Some(metadata) = &*metadata {
        stripped = stripped.with_metadata(metadata);
    }
    stripped
}

/// Checks that `frames` can be written as an animation on a canvas with
/// `header`; see [`Encoder::encode_animation_into`] for the errors. The canvas
/// itself is checked by `validate`.
fn validate_frames(
    header: &ImageHeader,
    frames: &[FrameRef<'_>],
    has_default_image: bool,
) -> Result<(), Error> {
    let count = u32::try_from(frames.len()).map_err(|_| Error::FrameCountMismatch {
        expected: u32::MAX,
        actual: u32::MAX,
    })?;
    let Some(first) = frames.first() else {
        return Err(Error::FrameCountMismatch {
            expected: 1,
            actual: count,
        });
    };
    if !has_default_image && !first.control(0).covers(header.width, header.height) {
        return Err(Error::InvalidChunkData(ChunkType::FCTL));
    }

    for frame in frames {
        let frame_header = frame.header(header);
        ImageHeader::validate_dimensions(frame_header.width, frame_header.height)?;
        if !frame.control(0).fits_in(header.width, header.height) {
            return Err(Error::FrameOutOfBounds);
        }

        let expected = frame_header.image_size()?;
        if frame.data().len() != expected {
            return Err(Error::InvalidImageDataLength {
                expected,
                actual: frame.data().len(),
            });
        }
    }

    Ok(())
}

/// Checks that `image` can be written as a valid PNG; see
/// [`Encoder::encode_into`] for the errors.
fn validate(image: &ImageRef<'_>) -> Result<(), Error> {
    let header = image.header();
    ImageHeader::validate_dimensions(header.width, header.height)?;
    ImageHeader::validate_bit_depth(header.bit_depth, header.color_type)?;

    if header.color_type == ColorType::Indexed && image.palette().is_none() {
        return Err(Error::MissingPalette);
    }

    let image_size = header.image_size()?;
    if image_size != image.data().len() {
        return Err(Error::InvalidImageDataLength {
            expected: image_size,
            actual: image.data().len(),
        });
    }

    if let Some(palette) = image.palette() {
        if header.color_type == ColorType::Grayscale
            || header.color_type == ColorType::GrayscaleAlpha
        {
            return Err(Error::UnexpectedPalette(header.color_type));
        }

        if header.color_type == ColorType::Indexed && palette.len() > 1 << header.bit_depth {
            return Err(Error::TooManyPaletteEntries {
                entries: palette.len(),
                bit_depth: header.bit_depth,
            });
        }
    }

    if let Some(transparency) = image.transparency() {
        validate_transparency(header, image.palette(), transparency)?;
    }

    if let Some(metadata) = image.metadata() {
        validate_metadata(metadata)?;
    }

    validate_extra_chunks(image.chunks())?;

    Ok(())
}

/// Checks that `transparency` can be written as the `tRNS` chunk of an image
/// with `header` and `palette`. `palette` has already been checked, and is there
/// for indexed images.
///
/// Errors:
/// - `Error::UnexpectedTransparency` for grayscale with alpha and RGBA, which
///   must not have a `tRNS` chunk.
/// - `Error::InvalidTransparencyLength` if the kind doesn't match the color
///   type: `Gray` is only for grayscale, `Rgb` only for RGB and `Palette` only
///   for indexed images. The length given is the one the chunk would have: 2,
///   6, or the number of alpha values.
/// - `Error::InvalidChunkData` for `ChunkType::TRNS` if a gray or RGB value
///   doesn't fit the bit depth, such as 16 for a 4-bit image. The decoder
///   ignores those bits, so writing them would change which color is transparent.
/// - `Error::TooManyTransparencyEntries` if an indexed image has more alpha
///   values than palette entries.
fn validate_transparency(
    header: &ImageHeader,
    palette: Option<&Palette>,
    transparency: &Transparency,
) -> Result<(), Error> {
    if header.color_type == ColorType::GrayscaleAlpha || header.color_type == ColorType::Rgba {
        return Err(Error::UnexpectedTransparency(header.color_type));
    }

    let max_sample = (1u32 << header.bit_depth) - 1;
    let fits = |v: u16| u32::from(v) <= max_sample;

    match transparency {
        Transparency::Gray(gray) => {
            if header.color_type != ColorType::Grayscale {
                return Err(Error::InvalidTransparencyLength {
                    color_type: header.color_type,
                    length: 2,
                });
            }
            if !fits(*gray) {
                return Err(Error::InvalidChunkData(ChunkType::TRNS));
            }
        }
        Transparency::Rgb(rgb) => {
            if header.color_type != ColorType::Rgb {
                return Err(Error::InvalidTransparencyLength {
                    color_type: header.color_type,
                    length: rgb.len() * 2,
                });
            }
            if !rgb.iter().all(|&v| fits(v)) {
                return Err(Error::InvalidChunkData(ChunkType::TRNS));
            }
        }
        Transparency::Palette(palette_alpha) => {
            if header.color_type != ColorType::Indexed {
                return Err(Error::InvalidTransparencyLength {
                    color_type: header.color_type,
                    length: palette_alpha.len(),
                });
            }
            let palette = palette.ok_or(Error::MissingPalette)?;
            if palette_alpha.len() > palette.len() {
                return Err(Error::TooManyTransparencyEntries {
                    entries: palette_alpha.len(),
                    palette_entries: palette.len(),
                });
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests;
