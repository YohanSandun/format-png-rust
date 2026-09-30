use super::chunk_reader::ChunkReader;
use super::options::DecodeOptions;
use crate::decode::deinterlace::deinterlace_pass;
use crate::decode::image_data::collect_image_data;
use crate::decode::unfilter::unfilter;
use crate::error::Error;
use crate::png::adam7::PASSES;
use crate::convert::{Source, convert, is_unchanged};
use crate::png::{Bitmap, ChunkType, Image, ImageHeader, Interlace, Palette, PixelFormat, Transparency};
use rust_deflate::{Decompressor, OutputOptions};

/// A reusable PNG decoder.
///
/// It owns a [`Decompressor`] and its working buffers, and keeps them between
/// images, so decoding many PNGs with one `Decoder` avoids setting them up for
/// each one. [`decode_into`](Self::decode_into) also reuses your output buffer.
/// For a single image, [`decode`](crate::decode) is simpler.
///
/// ```
/// use format_png::{Decoder, DecodeOptions};
///
/// let mut decoder = Decoder::with_options(DecodeOptions {
///     validate_crc: false,
///     ..DecodeOptions::default()
/// });
/// let mut pixels = Vec::new();
///
/// for path in ["tests/data/valid/rgb_8.png", "tests/data/valid/gray_16_adam7.png"] {
///     let header = decoder.decode_into(&std::fs::read(path)?, &mut pixels)?;
///     println!("{path}: {}x{}, {} bytes", header.width, header.height, pixels.len());
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct Decoder {
    options: DecodeOptions,
    decompressor: Decompressor,
    /// The zlib stream: all `IDAT` data joined.
    compressed: Vec<u8>,
    /// The decompressed stream: every scanline with its filter type byte.
    scanlines: Vec<u8>,
    /// One unfiltered Adam7 pass, before it's spread into the image.
    pass: Vec<u8>,
    /// The decoded image in its own format, before it's converted to a bitmap.
    pixels: Vec<u8>,
    /// The `PLTE` palette of the last image decoded, if it had one.
    palette: Option<Palette>,
    /// The `tRNS` transparency of the last image decoded, if it had one.
    transparency: Option<Transparency>,
}

impl Decoder {
    /// Creates a decoder with [`DecodeOptions::default`].
    pub fn new() -> Self {
        Self::with_options(DecodeOptions::default())
    }

    /// Creates a decoder with the given options.
    pub fn with_options(options: DecodeOptions) -> Self {
        Self::with_decompressor(options, Decompressor::new())
    }

    /// Creates a decoder that uses an existing [`Decompressor`], for example one taken
    /// from another decoder with [`into_decompressor`](Self::into_decompressor).
    pub fn with_decompressor(options: DecodeOptions, decompressor: Decompressor) -> Self {
        Self {
            options,
            decompressor,
            compressed: Vec::new(),
            scanlines: Vec::new(),
            pass: Vec::new(),
            pixels: Vec::new(),
            palette: None,
            transparency: None,
        }
    }

    /// The options this decoder was created with.
    pub fn options(&self) -> &DecodeOptions {
        &self.options
    }

    /// The `PLTE` palette of the last image decoded with [`decode_into`](Self::decode_into)
    /// or another decode method, or `None` if it had none or decoding failed.
    /// Indexed images always have one; see [`Image::palette`].
    pub fn palette(&self) -> Option<&Palette> {
        self.palette.as_ref()
    }

    /// The `tRNS` transparency of the last image decoded, like [`palette`](Self::palette);
    /// see [`Image::transparency`].
    pub fn transparency(&self) -> Option<&Transparency> {
        self.transparency.as_ref()
    }

    /// Consumes the decoder and returns its decompressor, to reuse elsewhere.
    pub fn into_decompressor(self) -> Decompressor {
        self.decompressor
    }

    /// Returns a [`ChunkReader`] over `data`, with CRC validation set from this
    /// decoder's options.
    ///
    /// # Errors
    ///
    /// Same as [`ChunkReader::new`].
    pub fn chunks<'a>(&self, data: &'a [u8]) -> Result<ChunkReader<'a>, Error> {
        Ok(ChunkReader::new(data)?.validate_crc(self.options.validate_crc))
    }

    /// Reads and validates the `IHDR` chunk, which must be the first chunk. Chunks
    /// after it aren't read.
    ///
    /// # Errors
    ///
    /// - [`Error::InvalidSignature`] or [`Error::UnexpectedEndOfInput`] if `data`
    ///   doesn't start with a complete PNG signature.
    /// - [`Error::MissingImageHeader`] if there are no chunks or the first isn't `IHDR`.
    /// - Any error from [`ChunkReader::next_chunk`] or [`ImageHeader::parse`].
    pub fn read_header(&mut self, data: &[u8]) -> Result<ImageHeader, Error> {
        let mut chunks = self.chunks(data)?;
        Self::read_image_header(&mut chunks)
    }

    fn read_image_header(chunks: &mut ChunkReader<'_>) -> Result<ImageHeader, Error> {
        match chunks.next_chunk()? {
            Some(chunk) if chunk.chunk_type() == ChunkType::IHDR => {
                ImageHeader::parse(chunk.data())
            }
            _ => Err(Error::MissingImageHeader),
        }
    }

    /// Decodes the whole image into the PNG's own pixel format; see [`Image`].
    ///
    /// # Errors
    ///
    /// Same as [`decode_into`](Self::decode_into).
    pub fn decode(&mut self, data: &[u8]) -> Result<Image, Error> {
        let mut pixels = Vec::new();
        let header = self.decode_into(data, &mut pixels)?;
        Ok(Image::new(header, header.stride()?, pixels, self.palette.clone(), self.transparency.clone()))
    }

    /// Like [`decode`](Self::decode), but writes the pixels to `out` and returns
    /// only the header. `out`'s old contents are replaced and its allocation is
    /// reused, so passing the same buffer for many images avoids allocating one each
    /// time. The layout is the same as [`Image::data`]. After an error, `out`'s
    /// contents are unspecified.
    ///
    /// Indexed images come back as palette indices; the palette is then available
    /// from [`palette`](Self::palette). Ancillary chunks are skipped for now.
    ///
    /// # Errors
    ///
    /// - Any error from [`read_header`](Self::read_header).
    /// - [`Error::MissingImageData`], [`Error::NonConsecutiveImageData`] or
    ///   [`Error::MissingImageEnd`] if the `IDAT` and `IEND` chunks aren't laid
    ///   out correctly.
    /// - [`Error::MissingPalette`] if an indexed image has no `PLTE` before its
    ///   image data, and [`Error::PaletteAfterImageData`] or
    ///   [`Error::DuplicatePalette`] if `PLTE` is misplaced or repeated.
    /// - [`Error::InvalidPaletteLength`], [`Error::TooManyPaletteEntries`] or
    ///   [`Error::UnexpectedPalette`] if the `PLTE` chunk is invalid.
    /// - [`Error::TransparencyAfterImageData`], [`Error::TransparencyBeforePalette`]
    ///   or [`Error::DuplicateTransparency`] if `tRNS` is misplaced or repeated.
    /// - [`Error::InvalidTransparencyLength`], [`Error::TooManyTransparencyEntries`]
    ///   or [`Error::UnexpectedTransparency`] if the `tRNS` chunk is invalid.
    /// - [`Error::Decompression`] if the zlib stream is corrupt.
    /// - [`Error::ImageDataTooShort`] or [`Error::ImageDataTooLong`] if it doesn't
    ///   decompress to the size the header requires.
    /// - [`Error::InvalidFilterType`] if a scanline has an unknown filter type.
    /// - [`Error::ImageTooLarge`] if the image doesn't fit in memory on this platform.
    pub fn decode_into(&mut self, data: &[u8], out: &mut Vec<u8>) -> Result<ImageHeader, Error> {
        // Cleared first, so a failed decode doesn't leave the last image's chunks.
        self.palette = None;
        self.transparency = None;

        let mut chunks = self.chunks(data)?;
        let header = Self::read_image_header(&mut chunks)?;
        let scanline_size = header.scanline_size()?;

        self.compressed.clear();
        let found = collect_image_data(&mut chunks, &header, &mut self.compressed)?;

        // The decompressor appends, so clear what the previous image left here.
        self.scanlines.clear();
        let output_size = self
            .decompressor
            .decompress_zlib_into_with(
                &self.compressed,
                &mut self.scanlines,
                OutputOptions::exact(scanline_size),
            )
            .map_err(|e| match e {
                rust_deflate::Error::OutputLimitExceeded => Error::ImageDataTooLong { expected: scanline_size },
                _ => Error::Decompression(e)
            })?;

        if output_size < scanline_size {
            return Err(Error::ImageDataTooShort { expected: scanline_size, actual: output_size });
        }

        out.clear();
        out.resize(header.image_size()?, 0);

        match header.interlace {
            Interlace::None => unfilter(&self.scanlines, header.stride()?, header.filter_bpp(), out)?,
            Interlace::Adam7 => self.unfilter_adam7(&header, out)?,
        }

        self.palette = found.palette;
        self.transparency = found.transparency;
        Ok(header)
    }

    /// Decodes the image and converts it to `format`, for display.
    ///
    /// ```
    /// use format_png::{Decoder, PixelFormat};
    ///
    /// let mut decoder = Decoder::new();
    /// let bitmap = decoder.decode_bitmap(&std::fs::read("tests/data/valid/gray_2.png")?, PixelFormat::Rgba8)?;
    ///
    /// assert_eq!(bitmap.data().len(), 13 * 7 * 4);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Same as [`decode_bitmap_into`](Self::decode_bitmap_into).
    pub fn decode_bitmap(&mut self, data: &[u8], format: PixelFormat) -> Result<Bitmap, Error> {
        let mut out = Vec::new();
        let header = self.decode_bitmap_into(data, format, &mut out)?;
        Ok(Bitmap::new(header.width, header.height, format, out))
    }

    /// Like [`decode_bitmap`](Self::decode_bitmap), but writes the pixels to `out`
    /// and returns only the header. `out`'s old contents are replaced and its
    /// allocation is reused; the decoder also keeps its own buffer for the
    /// unconverted pixels. The layout is the same as [`Bitmap::data`].
    ///
    /// # Errors
    ///
    /// - Any error from [`decode_into`](Self::decode_into).
    /// - Any error from [`Image::to_bitmap`], such as [`Error::PaletteIndexOutOfRange`]
    ///   for indexed images.
    pub fn decode_bitmap_into(&mut self, data: &[u8], format: PixelFormat, out: &mut Vec<u8>) -> Result<ImageHeader, Error> {
        // 8-bit RGB or RGBA already has the bitmap's layout, so decode straight into `out`.
        if is_unchanged(&self.read_header(data)?, format) {
            return self.decode_into(data, out);
        }

        // `decode_into` borrows all of `self`, so the pixel buffer is taken out
        // for the call and put back afterwards, even on error.
        let mut pixels = std::mem::take(&mut self.pixels);

        let result = self.decode_into(data, &mut pixels).and_then(|header| {
            let source = Source {
                header: &header,
                stride: header.stride()?,
                data: &pixels,
                palette: self.palette.as_ref(),
                transparency: self.transparency.as_ref(),
            };
            convert(&source, format, out)?;
            Ok(header)
        });

        self.pixels = pixels;
        result
    }

    /// Unfilters each non-empty pass of `self.scanlines` into `self.pass` and
    /// spreads it into `out`, the full image.
    fn unfilter_adam7(&mut self, header: &ImageHeader, out: &mut [u8]) -> Result<(), Error> {
        let bpp = header.filter_bpp();
        let mut offset = 0;

        for pass in &PASSES {
            let (width, height) = pass.size(header.width, header.height);
            let size = header.pass_scanline_size(width, height)?;
            if size == 0 {
                continue;
            }

            let row_bytes = header.row_bytes(width)?;
            self.pass.clear();
            self.pass.resize(row_bytes * height as usize, 0);

            unfilter(&self.scanlines[offset..offset + size], row_bytes, bpp, &mut self.pass)?;
            deinterlace_pass(header, pass, &self.pass, out);
            offset += size;
        }

        Ok(())
    }
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
