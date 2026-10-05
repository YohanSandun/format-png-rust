use crate::ColorType;
use crate::error::Error;
use crate::png::palette::MAX_ENTRIES;
use crate::png::{ImageHeader, Palette};

/// The contents of a `tRNS` chunk: transparency for an image without an alpha
/// channel.
///
/// Grayscale and RGB images get one color that is fully transparent, and indexed
/// images get an alpha value for each palette entry. Images with an alpha channel
/// can't have a `tRNS` chunk.
// `Palette` is much bigger than the other variants because its values are
// stored inline, so decoding never allocates for them. Boxing it would save
// about 250 bytes per image at the cost of an allocation.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Transparency {
    /// Grayscale pixels with this value are fully transparent. The value has the
    /// image's bit depth, so for a 4-bit image it's 0 to 15.
    Gray(u16),
    /// RGB pixels with exactly these red, green and blue values are fully
    /// transparent. The values have the image's bit depth.
    Rgb([u16; 3]),
    /// An alpha value for each palette entry of an indexed image.
    Palette(PaletteAlpha),
}

impl Transparency {
    /// Parses `tRNS` chunk data for an image with `header`. `palette` is the
    /// `PLTE` chunk read before it, if any.
    ///
    /// Gray and RGB values are 2 bytes each, big-endian. For bit depths under 16
    /// only the low bits are used: the spec says decoders must ignore the others.
    ///
    /// Errors:
    /// - `Error::UnexpectedTransparency` for grayscale with alpha and RGBA, which
    ///   must not have one.
    /// - `Error::TransparencyBeforePalette` for an indexed image with no palette
    ///   yet: `tRNS` must come after `PLTE`.
    /// - `Error::InvalidTransparencyLength` if the length isn't 2 for grayscale or
    ///   6 for RGB, or is 0 for an indexed image.
    /// - `Error::TooManyTransparencyEntries` if an indexed image's `tRNS` has more
    ///   entries than its palette.
    pub(crate) fn parse(
        data: &[u8],
        header: &ImageHeader,
        palette: Option<&Palette>,
    ) -> Result<Self, Error> {
        // Reads 2-byte value number `i` of `data`, keeping only the bits of the
        // bit depth. Shifting a u32 lets this work for 16-bit too.
        let value = |i: usize| -> u16 {
            let stored = u16::from_be_bytes([data[i * 2], data[i * 2 + 1]]);
            stored & ((1u32 << header.bit_depth) - 1) as u16
        };

        match header.color_type {
            ColorType::GrayscaleAlpha | ColorType::Rgba => {
                Err(Error::UnexpectedTransparency(header.color_type))
            }
            ColorType::Indexed => {
                if let Some(palette) = palette {
                    if data.is_empty() {
                        return Err(Error::InvalidTransparencyLength {
                            color_type: header.color_type,
                            length: data.len(),
                        });
                    } else if palette.len() < data.len() {
                        return Err(Error::TooManyTransparencyEntries {
                            entries: data.len(),
                            palette_entries: palette.len(),
                        });
                    }
                    Ok(Self::Palette(PaletteAlpha::new(data)))
                } else {
                    Err(Error::TransparencyBeforePalette)
                }
            }
            ColorType::Grayscale => {
                if data.len() != 2 {
                    Err(Error::InvalidTransparencyLength {
                        color_type: header.color_type,
                        length: data.len(),
                    })
                } else {
                    Ok(Self::Gray(value(0)))
                }
            }
            ColorType::Rgb => {
                if data.len() != 6 {
                    Err(Error::InvalidTransparencyLength {
                        color_type: header.color_type,
                        length: data.len(),
                    })
                } else {
                    Ok(Self::Rgb([value(0), value(1), value(2)]))
                }
            }
        }
    }
}

/// Alpha values for the entries of a palette, from a `tRNS` chunk.
///
/// The chunk can be shorter than the palette: entries past its end are opaque.
/// The values are stored inline, so this never allocates.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PaletteAlpha {
    /// Entries past `len` are always 255, so the derived traits only see the
    /// values that were read.
    alpha: [u8; MAX_ENTRIES],
    len: usize,
}

impl PaletteAlpha {
    /// Makes alpha values from `values`, which the caller has checked are 1 to 256.
    pub(crate) fn new(values: &[u8]) -> Self {
        debug_assert!((1..=MAX_ENTRIES).contains(&values.len()));
        let mut alpha = [255; MAX_ENTRIES];
        alpha[..values.len()].copy_from_slice(values);
        Self {
            alpha,
            len: values.len(),
        }
    }

    /// Creates alpha values for a palette, in index order, for encoding an
    /// image. There may be fewer than the palette has entries: the others are
    /// opaque.
    ///
    /// ```
    /// use format_png::{PaletteAlpha, Transparency};
    ///
    /// // Entry 0 fully transparent, entry 1 half transparent, the rest opaque.
    /// let transparency = Transparency::Palette(PaletteAlpha::from_values(&[0, 128])?);
    /// # Ok::<(), format_png::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// [`Error::InvalidTransparencyLength`] for an indexed image if there are no
    /// values or more than 256.
    pub fn from_values(values: &[u8]) -> Result<Self, Error> {
        if values.is_empty() || values.len() > MAX_ENTRIES {
            return Err(Error::InvalidTransparencyLength {
                color_type: ColorType::Indexed,
                length: values.len(),
            });
        }
        Ok(Self::new(values))
    }

    /// The alpha values in the chunk, one per palette entry from index 0. There
    /// may be fewer than the palette has entries.
    #[must_use]
    pub fn values(&self) -> &[u8] {
        &self.alpha[..self.len]
    }

    /// The number of alpha values in the chunk, 1 to 256. May be less than
    /// the palette has entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Always `false`: the chunk has at least one value.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The alpha of palette entry `index`: its value from the chunk, or 255 past
    /// the end of it.
    #[must_use]
    pub fn get(&self, index: u8) -> u8 {
        self.alpha[usize::from(index)]
    }
}

#[cfg(test)]
mod tests;
