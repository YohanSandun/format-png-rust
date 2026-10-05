//! Drops ancillary chunks for [`StripChunks`], to make files smaller.

use super::options::StripChunks;
use crate::png::Metadata;

/// The part of `metadata` that `mode` keeps:
///
/// - [`StripChunks::Keep`]: all of it.
/// - [`StripChunks::Safe`]: the chunks that change how the image looks: `cICP`,
///   `iCCP`, `sRGB`, `gAMA` and `cHRM` for its colors, and `pHYs` for its aspect
///   ratio. Text, `tIME` and `eXIf` go.
/// - [`StripChunks::All`]: none of it.
///
/// `tRNS` and extra chunks aren't metadata; `Encoder::encode_into` handles them.
pub(crate) fn strip_metadata(metadata: &Metadata, mode: StripChunks) -> Metadata {
    match mode {
        StripChunks::Keep => metadata.clone(),
        StripChunks::Safe => Metadata {
            gamma: metadata.gamma,
            chromaticities: metadata.chromaticities,
            srgb: metadata.srgb,
            physical_dimensions: metadata.physical_dimensions,
            icc_profile: metadata.icc_profile.clone(),
            cicp: metadata.cicp,
            ..Metadata::default()
        },
        StripChunks::All => Metadata::default(),
    }
}

#[cfg(test)]
mod tests;
