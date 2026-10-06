#![cfg(test)]

use crate::{ColorType, Error, ImageHeader, Interlace, read_header};

#[test]
fn read_header_parses_a_png() {
    let data = [
        0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, // signature
        0x00, 0x00, 0x00, 0x0D, b'I', b'H', b'D', b'R', // length = 13, type
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // width = 1, height = 1
        0x08, 0x06, 0x00, 0x00, 0x00, // bit depth, color type, compression, filter, interlace
        0x1F, 0x15, 0xC4, 0x89, // crc
    ];

    assert_eq!(
        read_header(&data),
        Ok(ImageHeader {
            width: 1,
            height: 1,
            bit_depth: 8,
            color_type: ColorType::Rgba,
            interlace: Interlace::None,
        })
    );
}

#[test]
fn read_header_rejects_non_png() {
    assert_eq!(read_header(b"GIF89a"), Err(Error::InvalidSignature));
}

#[test]
fn decode_reads_pixels() {
    let data = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/valid/rgba_8_1x1.png"
    ))
    .unwrap();
    let image = crate::decode(&data).unwrap();

    assert_eq!((image.width(), image.height()), (1, 1));
    // generate.py: sample(0, 0, c) = c * 53
    assert_eq!(image.data(), &[0, 53, 106, 159]);
}

#[test]
fn decode_rejects_non_png() {
    assert_eq!(crate::decode(b"GIF89a"), Err(Error::InvalidSignature));
}

#[test]
fn decode_rgba8_and_rgb8_convert_pixels() {
    let data = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/valid/rgba_8_1x1.png"
    ))
    .unwrap();

    // generate.py: sample(0, 0, c) = c * 53
    let rgba = crate::decode_rgba8(&data).unwrap();
    assert_eq!(rgba.format(), crate::PixelFormat::Rgba8);
    assert_eq!(rgba.data(), &[0, 53, 106, 159]);

    let rgb = crate::decode_rgb8(&data).unwrap();
    assert_eq!(rgb.format(), crate::PixelFormat::Rgb8);
    assert_eq!(rgb.data(), &[0, 53, 106]);
}

#[test]
fn decode_rgba8_rejects_non_png() {
    assert_eq!(crate::decode_rgba8(b"GIF89a"), Err(Error::InvalidSignature));
}

/// Fails to compile, rather than to run, if a public type stops being `Send` or
/// `Sync`, for example by gaining an `Rc` or a `Cell`. Callers rely on moving
/// decoders, encoders and images to other threads, as with
/// `tokio::task::spawn_blocking`, and losing that would break them.
#[test]
fn public_types_can_be_used_across_threads() {
    fn assert_send_sync<T: Send + Sync>() {}

    assert_send_sync::<crate::Decoder>();
    assert_send_sync::<crate::DecodeOptions>();
    assert_send_sync::<crate::ChunkReader<'static>>();
    assert_send_sync::<crate::PngChunks<'static>>();
    assert_send_sync::<crate::Encoder>();
    assert_send_sync::<crate::EncodeOptions>();
    assert_send_sync::<crate::ImageRef<'static>>();
    assert_send_sync::<crate::Image>();
    assert_send_sync::<crate::Bitmap>();
    assert_send_sync::<crate::ImageHeader>();
    assert_send_sync::<crate::Palette>();
    assert_send_sync::<crate::Transparency>();
    assert_send_sync::<crate::Metadata>();
    assert_send_sync::<crate::OwnedChunk>();
    assert_send_sync::<crate::png::Chunk<'static>>();
    assert_send_sync::<crate::png::metadata::Text>();
    assert_send_sync::<crate::png::metadata::IccProfile>();
    assert_send_sync::<crate::png::metadata::Exif>();
    assert_send_sync::<Error>();
}
