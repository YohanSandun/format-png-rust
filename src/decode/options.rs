/// Settings for a [`Decoder`](crate::Decoder).
///
/// Use struct update syntax to change only some of them:
///
/// ```
/// use format_png::DecodeOptions;
///
/// let options = DecodeOptions { validate_crc: false, ..DecodeOptions::default() };
/// ```
#[derive(Debug, Clone)]
pub struct DecodeOptions {
    /// Whether to validate CRC values of PNG chunks. On by default.
    pub validate_crc: bool,

    /// Whether to keep a raw copy of every ancillary chunk, including private and
    /// unknown ones, as [`OwnedChunk`](crate::OwnedChunk)s. Off by default.
    pub preserve_chunks: bool,

    /// Whether to parse known ancillary chunks, such as `gAMA` and `tIME`, into
    /// [`Metadata`](crate::Metadata). Off by default.
    pub preserve_metadata: bool,

    /// Whether an invalid, misplaced or repeated ancillary chunk fails the decode.
    /// Off by default: such chunks are skipped, as the PNG spec allows, so a bad
    /// piece of metadata doesn't stop an image from decoding. Only applies to
    /// chunks read into [`Metadata`](crate::Metadata); `tRNS` is always checked.
    pub strict_ancillary: bool,
}

impl Default for DecodeOptions {
    fn default() -> Self {
        Self {
            validate_crc: true,
            preserve_chunks: false,
            preserve_metadata: false,
            strict_ancillary: false,
        }
    }
}