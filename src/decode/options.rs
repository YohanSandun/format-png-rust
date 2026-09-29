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

    /// Whether to preserve ancillary chunks in the decoded result.
    pub preserve_chunks: bool,

    /// Whether to preserve PNG metadata in the decoded result.
    pub preserve_metadata: bool,
}

impl Default for DecodeOptions {
    fn default() -> Self {
        Self {
            validate_crc: true,
            preserve_chunks: false,
            preserve_metadata: false,
        }
    }
}