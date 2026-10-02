use rust_deflate::{Decompressor, OutputOptions};

use crate::error::Error;
use crate::png::ChunkType;

/// The longest keyword the spec allows, in bytes.
const MAX_KEYWORD_LENGTH: usize = 79;

/// The only compression method defined for `zTXt` and `iTXt`: zlib.
const COMPRESSION_METHOD_ZLIB: u8 = 0;

/// Which chunk a [`Text`] was read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextKind {
    /// `tEXt`: uncompressed Latin-1 text.
    Plain,
    /// `zTXt`: zlib-compressed Latin-1 text.
    Compressed,
    /// `iTXt`: UTF-8 text with a language tag, compressed or not.
    International {
        /// Whether the text was zlib-compressed in the file.
        compressed: bool,
    },
}

/// A `tEXt`, `zTXt` or `iTXt` chunk: a keyword and its text, such as
/// `Title` or `Author`.
///
/// The text is decoded to a `String` whatever the chunk: Latin-1 for `tEXt` and
/// `zTXt`, UTF-8 for `iTXt`, decompressed when the chunk was compressed.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Text {
    /// 1 to 79 bytes of printable Latin-1, such as `Title`, `Author` or `Comment`.
    pub keyword: String,
    /// The text itself.
    pub text: String,
    /// The `iTXt` language tag, such as `en` or `nb-NO`. Empty for `tEXt` and
    /// `zTXt`, and for `iTXt` chunks that don't give one.
    pub language_tag: String,
    /// The `iTXt` keyword translated into the language. Empty for `tEXt` and
    /// `zTXt`, and for `iTXt` chunks that don't give one.
    pub translated_keyword: String,
    /// Which chunk this was read from.
    pub kind: TextKind,
}

impl Text {
    /// The cap on decompressed text that [`parse_compressed`](Self::parse_compressed)
    /// and [`parse_international`](Self::parse_international) use, and the decoder
    /// uses when collecting [`Metadata`](crate::Metadata): 8 MB.
    pub const DEFAULT_MAX_SIZE: usize = 8_000_000;

    /// Parses `tEXt` chunk data: the keyword, a null byte, then the text in
    /// Latin-1, without a terminating null.
    ///
    /// # Errors
    ///
    /// - [`Error::InvalidChunkData`] if there is no null byte or the keyword is invalid.
    pub fn parse_text(data: &[u8]) -> Result<Self, Error> {
        let (keyword, text) = read_keyword(data, ChunkType::TEXT)?;

        Ok(Self {
            keyword,
            text: latin1_to_string(text),
            language_tag: String::new(),
            translated_keyword: String::new(),
            kind: TextKind::Plain,
        })
    }

    /// Parses `zTXt` chunk data: the keyword, a null byte, the compression method
    /// (0), then the Latin-1 text as a zlib stream.
    ///
    /// The text is capped at [`DEFAULT_MAX_SIZE`](Self::DEFAULT_MAX_SIZE) bytes
    /// once decompressed, and a decompressor is set up for this call. To choose
    /// the cap, or reuse a decompressor across many chunks, use
    /// [`parse_compressed_with`](Self::parse_compressed_with).
    ///
    /// ```
    /// use format_png::png::metadata::Text;
    ///
    /// let mut data = b"Comment\0\0".to_vec();
    /// data.extend_from_slice(&rust_deflate::compress_zlib(b"made with format-png"));
    ///
    /// let text = Text::parse_compressed(&data)?;
    /// assert_eq!((text.keyword.as_str(), text.text.as_str()), ("Comment", "made with format-png"));
    /// # Ok::<(), format_png::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Same as [`parse_compressed_with`](Self::parse_compressed_with).
    pub fn parse_compressed(data: &[u8]) -> Result<Self, Error> {
        Self::parse_compressed_with(data, &mut Decompressor::new(), Self::DEFAULT_MAX_SIZE)
    }

    /// Like [`parse_compressed`](Self::parse_compressed), but decompresses with
    /// `decompressor` and caps the decompressed text at `max_size` bytes, so a
    /// small chunk can't expand without limit.
    ///
    /// # Errors
    ///
    /// - [`Error::InvalidChunkData`] if there is no null byte, the keyword is
    ///   invalid, the compression method isn't 0, or the zlib stream is corrupt.
    /// - [`Error::TextTooLong`] if the text decompresses to more than `max_size` bytes.
    pub fn parse_compressed_with(
        data: &[u8],
        decompressor: &mut Decompressor,
        max_size: usize,
    ) -> Result<Self, Error> {
        let (keyword, rest) = read_keyword(data, ChunkType::ZTXT)?;
        let [method, compressed @ ..] = rest else {
            return Err(Error::InvalidChunkData(ChunkType::ZTXT));
        };
        if *method != COMPRESSION_METHOD_ZLIB {
            return Err(Error::InvalidChunkData(ChunkType::ZTXT));
        }
        let text = decompress(compressed, ChunkType::ZTXT, decompressor, max_size)?;

        Ok(Self {
            keyword,
            text: latin1_to_string(&text),
            language_tag: String::new(),
            translated_keyword: String::new(),
            kind: TextKind::Compressed,
        })
    }

    /// Parses `iTXt` chunk data: the keyword, a null byte, the compression flag
    /// (0 or 1), the compression method (0), the language tag, a null byte, the
    /// translated keyword in UTF-8, a null byte, then the UTF-8 text, as a zlib
    /// stream if the flag is 1.
    ///
    /// Compressed text is capped at [`DEFAULT_MAX_SIZE`](Self::DEFAULT_MAX_SIZE)
    /// bytes once decompressed, and a decompressor is set up only if the text is
    /// compressed. To choose the cap, or reuse a decompressor across many chunks,
    /// use [`parse_international_with`](Self::parse_international_with).
    ///
    /// # Errors
    ///
    /// Same as [`parse_international_with`](Self::parse_international_with).
    pub fn parse_international(data: &[u8]) -> Result<Self, Error> {
        Self::parse_international_impl(data, |compressed| {
            decompress(compressed, ChunkType::ITXT, &mut Decompressor::new(), Self::DEFAULT_MAX_SIZE)
        })
    }

    /// Like [`parse_international`](Self::parse_international), but decompresses
    /// with `decompressor` and caps decompressed text at `max_size` bytes, as for
    /// [`parse_compressed_with`](Self::parse_compressed_with).
    ///
    /// # Errors
    ///
    /// - [`Error::InvalidChunkData`] if a null byte is missing, the keyword is
    ///   invalid, the flag or method has an unknown value, or the translated
    ///   keyword or text isn't valid UTF-8, or the text is compressed and the zlib
    ///   stream is corrupt.
    /// - [`Error::TextTooLong`] if the text is compressed and decompresses to more
    ///   than `max_size` bytes.
    pub fn parse_international_with(
        data: &[u8],
        decompressor: &mut Decompressor,
        max_size: usize,
    ) -> Result<Self, Error> {
        Self::parse_international_impl(data, |compressed| {
            decompress(compressed, ChunkType::ITXT, decompressor, max_size)
        })
    }

    /// Parses `iTXt` chunk data, calling `decompress` on the text only if it's
    /// compressed, so callers that set up a decompressor on demand can skip it.
    fn parse_international_impl(
        data: &[u8],
        decompress: impl FnOnce(&[u8]) -> Result<Vec<u8>, Error>,
    ) -> Result<Self, Error> {
        let (keyword, rest) = read_keyword(data, ChunkType::ITXT)?;

        let [flag, method, rest @ ..] = rest else {
            return Err(Error::InvalidChunkData(ChunkType::ITXT));
        };
        let compressed = match *flag {
            0 => false,
            1 => true,
            _ => return Err(Error::InvalidChunkData(ChunkType::ITXT)),
        };
        if *method != 0 {
            return Err(Error::InvalidChunkData(ChunkType::ITXT));
        }

        let (language_tag_data, rest) = split_at_null(rest, ChunkType::ITXT)?;
        let language_tag = latin1_to_string(language_tag_data);

        let (translated_keyword_data, text_data) = split_at_null(rest, ChunkType::ITXT)?;
        let translated_keyword = utf8_to_string(translated_keyword_data, ChunkType::ITXT)?;

        let text = if compressed {
            let decompressed = decompress(text_data)?;
            utf8_to_string(&decompressed, ChunkType::ITXT)?
        } else {
            utf8_to_string(text_data, ChunkType::ITXT)?
        };

        Ok(Self {
            keyword,
            text,
            translated_keyword,
            language_tag,
            kind: TextKind::International { compressed },
        })
    }

    /// The type of the chunk this was read from.
    pub fn chunk_type(&self) -> ChunkType {
        match self.kind {
            TextKind::Plain => ChunkType::TEXT,
            TextKind::Compressed => ChunkType::ZTXT,
            TextKind::International { .. } => ChunkType::ITXT,
        }
    }
}

/// Splits `data` at its first null byte, returning the bytes before and after it.
/// Returns `Error::InvalidChunkData` for `chunk_type` if there is no null byte.
fn split_at_null(data: &[u8], chunk_type: ChunkType) -> Result<(&[u8], &[u8]), Error> {
    for i in 0..data.len() {
        if data[i] == b'\0' {
            return Ok((&data[..i], &data[i + 1..]));
        }
    }
    Err(Error::InvalidChunkData(chunk_type))
}

/// Reads the null-terminated keyword at the start of `data` and returns it with
/// the bytes after the null.
///
/// A valid keyword is 1 to `MAX_KEYWORD_LENGTH` bytes of printable Latin-1
/// (32-126 and 161-255), with no leading, trailing or consecutive spaces.
/// Returns `Error::InvalidChunkData` for `chunk_type` otherwise.
fn read_keyword(data: &[u8], chunk_type: ChunkType) -> Result<(String, &[u8]), Error> {
    let (keyword, rest) = split_at_null(data, chunk_type)?;
    // Checked on the bytes: Latin-1 above 127 takes 2 bytes once in a `String`.
    if keyword.is_empty() || keyword.len() > MAX_KEYWORD_LENGTH {
        return Err(Error::InvalidChunkData(chunk_type));
    }
    if !keyword.iter().all(|&b| is_printable_latin1(b)) {
        return Err(Error::InvalidChunkData(chunk_type));
    }
    if keyword.first() == Some(&b' ') || keyword.last() == Some(&b' ') || keyword.windows(2).any(|w| w == b"  ") {
        return Err(Error::InvalidChunkData(chunk_type));
    }
    Ok((latin1_to_string(keyword), rest))
}

/// Whether `byte` is a printable Latin-1 character: 32-126 or 161-255. This
/// leaves out the control characters and 160, the non-breaking space.
fn is_printable_latin1(byte: u8) -> bool {
    matches!(byte, 32..=126 | 161..=255)
}

/// Decodes Latin-1 (ISO 8859-1) bytes, where each byte is the code point of the
/// same value.
fn latin1_to_string(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

/// Decodes UTF-8 bytes, returning `Error::InvalidChunkData` for `chunk_type` if
/// they aren't valid.
fn utf8_to_string(bytes: &[u8], chunk_type: ChunkType) -> Result<String, Error> {
    std::str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|_| Error::InvalidChunkData(chunk_type))
}

/// Decompresses the zlib stream of a `chunk_type` chunk, of at most `max_size`
/// bytes, reusing `decompressor`.
///
/// Returns `Error::TextTooLong` if it decompresses to more than `max_size` bytes,
/// and `Error::InvalidChunkData` for `chunk_type` if the stream is corrupt.
fn decompress(
    data: &[u8],
    chunk_type: ChunkType,
    decompressor: &mut Decompressor,
    max_size: usize,
) -> Result<Vec<u8>, Error> {
    decompressor
        .decompress_zlib_with(data, OutputOptions::new().max_output(max_size))
        .map_err(|e| match e {
            rust_deflate::Error::OutputLimitExceeded => Error::TextTooLong { chunk_type, max_size },
            _ => Error::InvalidChunkData(chunk_type),
        })
}

#[cfg(test)]
mod tests;
