#![cfg(test)]

use super::{header_data, write_chunk, write_image_data, write_signature};
use crate::ChunkReader;
use crate::png::{ChunkType, ColorType, ImageHeader, Interlace, SIGNATURE};

/// `out` after a signature, read back as (type, data) pairs with CRCs checked.
fn read_back(out: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut png = SIGNATURE.to_vec();
    png.extend_from_slice(out);
    let mut chunks = ChunkReader::new(&png).unwrap();
    let mut read = Vec::new();
    while let Some(chunk) = chunks.next_chunk().unwrap() {
        read.push((chunk.chunk_type().to_string(), chunk.data().to_vec()));
    }
    read
}

#[test]
fn signature_is_the_png_signature() {
    let mut out = Vec::new();

    write_signature(&mut out);

    assert_eq!(out, SIGNATURE);
}

#[test]
fn iend_has_the_well_known_bytes() {
    let mut out = Vec::new();

    write_chunk(&mut out, ChunkType::IEND, b"");

    assert_eq!(out, [0, 0, 0, 0, b'I', b'E', b'N', b'D', 0xAE, 0x42, 0x60, 0x82]);
}

#[test]
fn chunks_read_back_with_valid_crcs() {
    let mut out = Vec::new();

    write_chunk(&mut out, ChunkType::TEXT, b"Title\0format-png");
    write_chunk(&mut out, ChunkType::from_bytes(*b"ruSt").unwrap(), &[0, 1, 2, 255]);

    assert_eq!(
        read_back(&out),
        [("tEXt".to_string(), b"Title\0format-png".to_vec()), ("ruSt".to_string(), vec![0, 1, 2, 255])]
    );
}

#[test]
fn write_chunk_appends() {
    let mut out = b"existing".to_vec();

    write_chunk(&mut out, ChunkType::IEND, b"");

    assert!(out.starts_with(b"existing"));
    assert_eq!(out.len(), 8 + 12);
}

#[test]
fn header_data_is_what_parse_reads() {
    let headers = [
        ImageHeader { width: 1, height: 1, bit_depth: 8, color_type: ColorType::Rgba, interlace: Interlace::None },
        ImageHeader { width: 640, height: 480, bit_depth: 16, color_type: ColorType::Rgb, interlace: Interlace::Adam7 },
        ImageHeader { width: 2_147_483_647, height: 3, bit_depth: 1, color_type: ColorType::Grayscale, interlace: Interlace::None },
        ImageHeader { width: 300, height: 200, bit_depth: 4, color_type: ColorType::Indexed, interlace: Interlace::None },
    ];

    for header in headers {
        assert_eq!(ImageHeader::parse(&header_data(&header)), Ok(header), "{header:?}");
    }
}

#[test]
fn header_data_has_the_spec_layout() {
    let header = ImageHeader { width: 640, height: 480, bit_depth: 16, color_type: ColorType::Rgb, interlace: Interlace::Adam7 };

    assert_eq!(header_data(&header), [0, 0, 0x02, 0x80, 0, 0, 0x01, 0xE0, 16, 2, 0, 0, 1]);
}

#[test]
fn image_data_is_split_at_the_limit() {
    let compressed: Vec<u8> = (0..10).collect();
    let mut out = Vec::new();

    write_image_data(&mut out, &compressed, 4);

    let chunks = read_back(&out);
    assert!(chunks.iter().all(|(chunk_type, _)| chunk_type == "IDAT"));
    let lengths: Vec<_> = chunks.iter().map(|(_, data)| data.len()).collect();
    assert_eq!(lengths, [4, 4, 2]);
    let joined: Vec<u8> = chunks.iter().flat_map(|(_, data)| data.iter().copied()).collect();
    assert_eq!(joined, compressed);
}

#[test]
fn image_data_of_an_exact_multiple_has_no_empty_chunk() {
    let mut out = Vec::new();

    write_image_data(&mut out, &[1; 8], 4);

    let lengths: Vec<_> = read_back(&out).iter().map(|(_, data)| data.len()).collect();
    assert_eq!(lengths, [4, 4]);
}

#[test]
fn empty_image_data_is_one_empty_idat() {
    let mut out = Vec::new();

    write_image_data(&mut out, &[], 4);

    assert_eq!(read_back(&out), [("IDAT".to_string(), Vec::new())]);
}
