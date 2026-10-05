//! Converts images with few colors to indexed color, for
//! [`PaletteMode::Auto`](crate::PaletteMode::Auto).
//!
//! The conversion is lossless: it only happens when the image has at most 256
//! distinct colors, counting alpha, so every pixel gets an exact palette entry.

use std::collections::{HashMap, HashSet};

use crate::png::palette::MAX_ENTRIES;
use crate::png::{ColorType, ImageHeader, Palette, PaletteAlpha, Transparency};

/// An image converted by [`palettize`]. Its pixels are in the buffer passed to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Palettized {
    /// The original header, with indexed color and the smallest bit depth that
    /// fits the palette.
    pub(crate) header: ImageHeader,
    pub(crate) palette: Palette,
    /// The alpha of the palette's translucent entries, which come first, or
    /// `None` if every color is opaque.
    pub(crate) transparency: Option<Transparency>,
}

/// Converts an 8-bit RGB or RGBA image with at most 256 distinct colors to
/// indexed color, writing the palette indices to `out` in the layout of
/// [`ImageRef::data`](crate::ImageRef::data). `out`'s old contents are replaced.
///
/// Returns `None`, leaving `out` unspecified, for other color types and bit
/// depths, and for images with more colors.
///
/// - Colors are told apart by alpha too, so an RGBA color at two alphas takes
///   two entries. For RGB, pixels matching `transparency`'s color key are
///   transparent, so its `tRNS` carries over as an entry with alpha 0.
/// - Entries are in order of first appearance, with every translucent one
///   moved before the opaque ones, so `tRNS` only needs values for those.
/// - The bit depth is the smallest that indexes every entry: 1 for up to 2
///   colors, 2 for up to 4, 4 for up to 16, else 8.
///
/// `data` must be the right size for `header`, which `encoder::validate` checks.
pub(crate) fn palettize(
    header: &ImageHeader,
    data: &[u8],
    transparency: Option<&Transparency>,
    out: &mut Vec<u8>,
) -> Option<Palettized> {
    let channels = match (header.color_type, header.bit_depth) {
        (ColorType::Rgb, 8) => 3,
        (ColorType::Rgba, 8) => 4,
        _ => return None,
    };
    let key = match transparency {
        Some(Transparency::Rgb(rgb)) => Some(*rgb),
        _ => None,
    };
    let rgba = |pixel: &[u8]| -> [u8; 4] {
        if channels == 4 {
            return [pixel[0], pixel[1], pixel[2], pixel[3]];
        }
        let keyed = key == Some([pixel[0], pixel[1], pixel[2]].map(u16::from));
        [pixel[0], pixel[1], pixel[2], if keyed { 0 } else { 255 }]
    };
    // 8-bit rows have no padding, so the pixels are back to back.
    let pixels = || data.chunks_exact(channels).map(rgba);

    // 1. The distinct colors, in order of first appearance. Runs of one color
    //    are common, so the last color is checked before the map.
    let mut colors: Vec<[u8; 4]> = Vec::new();
    let mut seen: HashSet<[u8; 4]> = HashSet::new();
    let mut last = None;
    for color in pixels() {
        if last == Some(color) {
            continue;
        }
        last = Some(color);
        if seen.insert(color) {
            if colors.len() == MAX_ENTRIES {
                return None;
            }
            colors.push(color);
        }
    }

    // 2. Translucent entries first. The sort is stable, so each group keeps
    //    the order of first appearance, and the output is deterministic.
    colors.sort_by_key(|color| color[3] == 255);
    let index: HashMap<[u8; 4], u8> = colors.iter().enumerate().map(|(i, &color)| (color, i as u8)).collect();

    let rgb: Vec<[u8; 3]> = colors.iter().map(|&[r, g, b, _]| [r, g, b]).collect();
    let palette = Palette::from_colors(&rgb).expect("1 to 256 colors");
    let alpha: Vec<u8> = colors.iter().map(|color| color[3]).take_while(|&a| a != 255).collect();
    let transparency = (!alpha.is_empty())
        .then(|| Transparency::Palette(PaletteAlpha::from_values(&alpha).expect("1 to 256 values")));

    // 3. Pack the indices at the smallest bit depth, most significant bits
    //    first, with each row padded to a whole byte.
    let bit_depth = match colors.len() {
        0..=2 => 1,
        3..=4 => 2,
        5..=16 => 4,
        _ => 8,
    };
    let header = ImageHeader { bit_depth, color_type: ColorType::Indexed, ..*header };
    // Can't fail: indexed rows are never longer than the RGB rows they replace.
    let stride = header.stride().ok()?;
    out.clear();
    out.resize(stride * header.height as usize, 0);

    let width = header.width as usize;
    let bits = usize::from(bit_depth);
    let per_byte = 8 / bits;
    let mut indices = pixels().map(|color| index[&color]);
    for out_row in out.chunks_exact_mut(stride) {
        for x in 0..width {
            let i = indices.next().expect("one pixel per index");
            out_row[x / per_byte] |= i << (8 - bits * (x % per_byte + 1));
        }
    }

    Some(Palettized { header, palette, transparency })
}

#[cfg(test)]
mod tests;
