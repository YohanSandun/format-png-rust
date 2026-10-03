use crate::ColorType;
use crate::error::Error;
use crate::png::ImageHeader;

/// The most colors a `PLTE` chunk can hold.
pub(crate) const MAX_ENTRIES: usize = 256;

/// The colors of a `PLTE` chunk: 1 to 256 RGB entries, 8 bits per sample.
///
/// Indexed images need a palette; each pixel is an index into it. RGB and RGBA
/// images may also have one, as a suggested palette for displays with few
/// colors. It doesn't affect how they decode.
///
/// The entries are stored inline, so a palette never allocates.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Palette {
    /// Entries past `len` are always `[0, 0, 0]`, so the derived traits only
    /// see the colors that were read.
    colors: [[u8; 3]; MAX_ENTRIES],
    len: usize,
}

impl Palette {
    /// Parses `PLTE` chunk data for an image with `header`.
    ///
    /// Errors:
    /// - `Error::UnexpectedPalette` for grayscale images, which must not have one.
    /// - `Error::InvalidPaletteLength` if the length isn't a multiple of 3, or the
    ///   number of entries isn't 1 to 256.
    /// - `Error::TooManyPaletteEntries` if an indexed image's palette has more
    ///   entries than its bit depth can index (2 for 1-bit, 4 for 2-bit, and so on).
    pub(crate) fn parse(data: &[u8], header: &ImageHeader) -> Result<Self, Error> {
        if header.color_type == ColorType::Grayscale || header.color_type == ColorType::GrayscaleAlpha {
            return Err(Error::UnexpectedPalette(header.color_type));
        }

        let entries = data.len() / 3;

        if !data.len().is_multiple_of(3) || entries > MAX_ENTRIES || entries == 0 {
            return Err(Error::InvalidPaletteLength(data.len()));
        }

        // Only indexed images are limited by bit depth; a suggested palette for RGB
        // or RGBA may have up to 256 entries at any depth.
        if header.color_type == ColorType::Indexed && entries > 1 << header.bit_depth {
            return Err(Error::TooManyPaletteEntries { entries, bit_depth: header.bit_depth });
        }

        let colors: [[u8; 3]; MAX_ENTRIES] = std::array::from_fn(|i| {
            if i < entries {
                [
                    data[i * 3],
                    data[i * 3 + 1],
                    data[i * 3 + 2],
                ]
            } else {
                [0, 0, 0]
            }
        });

        Ok(Self {
            colors,
            len: entries,
        })
    }

    /// Creates a palette from `colors`, in index order, for encoding an image.
    ///
    /// ```
    /// use format_png::Palette;
    ///
    /// let palette = Palette::from_colors(&[[0, 0, 0], [255, 255, 255]])?;
    /// assert_eq!(palette.get(1), Some([255, 255, 255]));
    /// # Ok::<(), format_png::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// [`Error::InvalidPaletteLength`] if there are no colors or more than 256.
    /// The length given is in bytes, 3 per color, as for a `PLTE` chunk.
    pub fn from_colors(colors: &[[u8; 3]]) -> Result<Self, Error> {
        if colors.is_empty() || colors.len() > MAX_ENTRIES {
            return Err(Error::InvalidPaletteLength(colors.len()*3));
        }

        let mut pallete = [[0u8; 3]; MAX_ENTRIES];
        let n = colors.len().min(MAX_ENTRIES);
        pallete[..n].copy_from_slice(&colors[..n]);

        Ok(Self {
            colors: pallete,
            len: colors.len(),
        })
    }

    /// The colors, in index order.
    pub fn colors(&self) -> &[[u8; 3]] {
        &self.colors[..self.len]
    }

    /// The number of colors, 1 to 256.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Always `false`: a palette has at least one color.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The color at `index`, or `None` if the palette is shorter than that.
    pub fn get(&self, index: u8) -> Option<[u8; 3]> {
        self.colors().get(usize::from(index)).copied()
    }
}

#[cfg(test)]
mod tests;
