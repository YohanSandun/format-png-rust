#![cfg(test)]

use super::PngChunks;
use crate::png::{Chunk, ChunkType, ColorType, ImageChunks, ImageHeader, Interlace};

fn chunk(chunk_type: &[u8; 4], data: &'static [u8]) -> Chunk<'static> {
    Chunk::new(ChunkType::from_bytes(*chunk_type).unwrap(), data, 0)
}

fn png_chunks(chunks: Vec<Chunk<'static>>) -> PngChunks<'static> {
    let header = ImageHeader { width: 1, height: 1, bit_depth: 8, color_type: ColorType::Rgb, interlace: Interlace::None };
    PngChunks::new(header, chunks, ImageChunks::default())
}

fn types<'a>(chunks: impl Iterator<Item = Chunk<'a>>) -> Vec<String> {
    chunks.map(|c| c.chunk_type().to_string()).collect()
}

#[test]
fn unknown_chunks_leaves_out_every_chunk_this_crate_parses() {
    let png = png_chunks(vec![
        chunk(b"IHDR", b""),
        chunk(b"cHRM", b""),
        chunk(b"gAMA", b""),
        chunk(b"iCCP", b""),
        chunk(b"cICP", b""),
        chunk(b"bKGD", b""),
        chunk(b"sRGB", b""),
        chunk(b"PLTE", b""),
        chunk(b"tRNS", b""),
        chunk(b"pHYs", b""),
        chunk(b"CuSt", b""),
        chunk(b"IDAT", b""),
        chunk(b"tEXt", b""),
        chunk(b"zTXt", b""),
        chunk(b"iTXt", b""),
        chunk(b"tIME", b""),
        chunk(b"eXIf", b""),
        chunk(b"ruSt", b""),
        chunk(b"IEND", b""),
    ]);

    assert_eq!(types(png.unknown_chunks()), ["bKGD", "CuSt", "ruSt"]);
}

#[test]
fn chunks_of_type_keeps_file_order() {
    let png = png_chunks(vec![
        chunk(b"IHDR", b""),
        chunk(b"ruSt", b"1"),
        chunk(b"IDAT", b""),
        chunk(b"ruSt", b"2"),
        chunk(b"IEND", b""),
    ]);

    let data: Vec<_> = png.chunks_of_type(ChunkType::from_bytes(*b"ruSt").unwrap()).map(|c| c.data()).collect();
    assert_eq!(data, [b"1", b"2"]);
    assert_eq!(png.chunks_of_type(ChunkType::PLTE).count(), 0);
}
