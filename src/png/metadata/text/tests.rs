#![cfg(test)]

use rust_deflate::Decompressor;

use super::{Text, TextKind};
use crate::error::Error;
use crate::png::ChunkType;

fn text(kind: TextKind) -> Text {
    Text {
        keyword: "Title".to_string(),
        text: "format-png".to_string(),
        language_tag: String::new(),
        translated_keyword: String::new(),
        kind,
    }
}

#[test]
fn chunk_type_follows_the_kind() {
    assert_eq!(text(TextKind::Plain).chunk_type(), ChunkType::TEXT);
    assert_eq!(text(TextKind::Compressed).chunk_type(), ChunkType::ZTXT);
    assert_eq!(text(TextKind::International { compressed: false }).chunk_type(), ChunkType::ITXT);
    assert_eq!(text(TextKind::International { compressed: true }).chunk_type(), ChunkType::ITXT);
}

#[test]
fn keyword_accepts_printable_latin1_with_single_inner_spaces() {
    for keyword in [&b"Title"[..], b"Creation Time", b"A b c", b"~", b"\xA1\xFF", b"Caf\xE9"] {
        let mut data = keyword.to_vec();
        data.extend_from_slice(b"\0text");
        assert!(Text::parse_text(&data).is_ok(), "{keyword:?}");
    }
}

#[test]
fn keyword_rejects_unprintable_bytes() {
    for keyword in [&b"Tab\there"[..], b"\x1F", b"Del\x7F", b"\x80", b"\x9F", b"No\xA0break"] {
        let mut data = keyword.to_vec();
        data.extend_from_slice(b"\0text");
        assert_eq!(Text::parse_text(&data), Err(Error::InvalidChunkData(ChunkType::TEXT)), "{keyword:?}");
    }
}

#[test]
fn keyword_rejects_leading_trailing_and_consecutive_spaces() {
    for keyword in [&b" Title"[..], b"Title ", b"Two  spaces", b" "] {
        let mut data = keyword.to_vec();
        data.extend_from_slice(b"\0text");
        assert_eq!(Text::parse_text(&data), Err(Error::InvalidChunkData(ChunkType::TEXT)), "{keyword:?}");
    }
}

#[test]
fn keyword_length_counts_latin1_bytes() {
    // 79 bytes of 'é' (0xE9) is the longest keyword; as UTF-8 it's 158 bytes.
    let mut data = vec![0xE9; 79];
    data.extend_from_slice(b"\0text");
    assert_eq!(Text::parse_text(&data).unwrap().keyword, "é".repeat(79));

    let mut data = vec![b'a'; 80];
    data.extend_from_slice(b"\0text");
    assert_eq!(Text::parse_text(&data), Err(Error::InvalidChunkData(ChunkType::TEXT)));
}

#[test]
fn corrupt_streams_report_their_own_chunk_type() {
    let mut decompressor = Decompressor::new();

    assert_eq!(
        Text::parse_compressed(b"Comment\0\0garbage", &mut decompressor, 1000),
        Err(Error::InvalidChunkData(ChunkType::ZTXT))
    );
    assert_eq!(
        Text::parse_international(b"Comment\0\x01\0\0\0garbage", &mut decompressor, 1000),
        Err(Error::InvalidChunkData(ChunkType::ITXT))
    );
}

#[test]
fn text_over_the_limit_is_too_long() {
    let mut decompressor = Decompressor::new();
    let compressed = rust_deflate::compress_zlib(&[b'x'; 100]);

    let mut ztxt = b"Comment\0\0".to_vec();
    ztxt.extend_from_slice(&compressed);
    assert_eq!(
        Text::parse_compressed(&ztxt, &mut decompressor, 99),
        Err(Error::TextTooLong { chunk_type: ChunkType::ZTXT, max_size: 99 })
    );
    assert_eq!(Text::parse_compressed(&ztxt, &mut decompressor, 100).unwrap().text, "x".repeat(100));

    let mut itxt = b"Comment\0\x01\0\0\0".to_vec();
    itxt.extend_from_slice(&compressed);
    assert_eq!(
        Text::parse_international(&itxt, &mut decompressor, 99),
        Err(Error::TextTooLong { chunk_type: ChunkType::ITXT, max_size: 99 })
    );
}
