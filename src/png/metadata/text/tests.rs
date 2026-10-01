#![cfg(test)]

use super::{Text, TextKind};
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

// TODO: parse_text, parse_compressed and parse_international, once implemented.
