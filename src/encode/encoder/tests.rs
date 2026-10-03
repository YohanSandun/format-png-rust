#![cfg(test)]

use rust_deflate::{CompressionLevel, Strategy as CompressionStrategy};

use super::Encoder;
use crate::encode::image_ref::ImageRef;
use crate::encode::options::{EncodeOptions, FilterStrategy};
use crate::error::Error;
use crate::png::{ChunkType, ColorType, FilterType, Image, ImageHeader, Interlace, Palette, PaletteAlpha, Transparency};
use crate::{ChunkReader, Decoder};

/// Every color type with every bit depth it allows.
const FORMATS: [(ColorType, u8); 15] = [
    (ColorType::Grayscale, 1),
    (ColorType::Grayscale, 2),
    (ColorType::Grayscale, 4),
    (ColorType::Grayscale, 8),
    (ColorType::Grayscale, 16),
    (ColorType::Rgb, 8),
    (ColorType::Rgb, 16),
    (ColorType::Indexed, 1),
    (ColorType::Indexed, 2),
    (ColorType::Indexed, 4),
    (ColorType::Indexed, 8),
    (ColorType::GrayscaleAlpha, 8),
    (ColorType::GrayscaleAlpha, 16),
    (ColorType::Rgba, 8),
    (ColorType::Rgba, 16),
];

fn header(width: u32, height: u32, bit_depth: u8, color_type: ColorType) -> ImageHeader {
    ImageHeader { width, height, bit_depth, color_type, interlace: Interlace::None }
}

/// Deterministic pixel data of the right size for `header`.
fn pixels(header: &ImageHeader) -> Vec<u8> {
    (0..header.image_size().unwrap()).map(|i| (i as u8).wrapping_mul(97).wrapping_add(13).rotate_left(3)).collect()
}

/// [`pixels`] with the padding bits at the end of each row zeroed. Interlaced
/// images don't store them, so they decode as zeros.
fn pixels_with_zero_padding(header: &ImageHeader) -> Vec<u8> {
    let stride = header.stride().unwrap();
    let mut data = pixels(header);
    let used_bits = header.width as usize * usize::from(header.bits_per_pixel()) % 8;
    if used_bits != 0 {
        for row in data.chunks_exact_mut(stride) {
            *row.last_mut().unwrap() &= 0xFF << (8 - used_bits);
        }
    }
    data
}

/// A palette with as many colors as `bit_depth` can index, so any pixel data is valid.
fn full_palette(bit_depth: u8) -> Palette {
    let colors: Vec<[u8; 3]> = (0..1u16 << bit_depth).map(|i| [i as u8, (i * 3) as u8, (255 - i) as u8]).collect();
    Palette::from_colors(&colors).unwrap()
}

fn decode(png: &[u8]) -> Image {
    Decoder::new().decode(png).unwrap()
}

fn chunk_types(png: &[u8]) -> Vec<String> {
    let mut chunks = ChunkReader::new(png).unwrap();
    let mut types = Vec::new();
    while let Some(chunk) = chunks.next_chunk().unwrap() {
        types.push(chunk.chunk_type().to_string());
    }
    types
}

// ---------- construction ----------

#[test]
fn new_uses_default_options() {
    let options = Encoder::new().options().clone();

    assert_eq!(options.compression, CompressionLevel::MEDIUM);
    assert_eq!(options.compression_strategy, CompressionStrategy::Dynamic);
    assert_eq!(options.filter, FilterStrategy::Adaptive);
}

#[test]
fn with_options_keeps_options() {
    let encoder = Encoder::with_options(EncodeOptions { compression: CompressionLevel::BEST, ..EncodeOptions::default() });

    assert_eq!(encoder.options().compression, CompressionLevel::BEST);
}

// ---------- round trips ----------

#[test]
fn every_format_round_trips_through_the_decoder() {
    for (color_type, bit_depth) in FORMATS {
        // A width that leaves padding bits in rows under 8 bits.
        let header = header(13, 7, bit_depth, color_type);
        let data = pixels(&header);
        let palette = full_palette(bit_depth.min(8));
        let image = match color_type {
            ColorType::Indexed => ImageRef::new(header, &data).with_palette(&palette),
            _ => ImageRef::new(header, &data),
        };

        let decoded = decode(&Encoder::new().encode(image).unwrap());

        assert_eq!(*decoded.header(), header, "{color_type:?} {bit_depth}");
        assert_eq!(decoded.data(), data, "{color_type:?} {bit_depth}");
        if color_type == ColorType::Indexed {
            assert_eq!(decoded.palette(), Some(&palette), "{bit_depth}");
        }
    }
}

#[test]
fn a_1x1_image_round_trips() {
    let header = header(1, 1, 8, ColorType::Rgba);

    let decoded = decode(&Encoder::new().encode(ImageRef::new(header, &[1, 2, 3, 4])).unwrap());

    assert_eq!(decoded.data(), [1, 2, 3, 4]);
}

#[test]
fn every_option_round_trips() {
    let header = header(17, 9, 8, ColorType::Rgb);
    let data = pixels(&header);
    let filters = [FilterType::None, FilterType::Sub, FilterType::Up, FilterType::Average, FilterType::Paeth]
        .map(FilterStrategy::Fixed)
        .into_iter()
        .chain([FilterStrategy::Adaptive]);

    for filter in filters {
        for compression in [CompressionLevel::NONE, CompressionLevel::FAST, CompressionLevel::MEDIUM, CompressionLevel::BEST] {
            for compression_strategy in [CompressionStrategy::Stored, CompressionStrategy::Fixed, CompressionStrategy::Dynamic] {
                let mut encoder = Encoder::with_options(EncodeOptions { compression, compression_strategy, filter });

                let decoded = decode(&encoder.encode(ImageRef::new(header, &data)).unwrap());

                assert_eq!(decoded.data(), data, "{filter:?}, {compression:?}, {compression_strategy:?}");
            }
        }
    }
}

#[test]
fn stored_strategy_does_not_compress() {
    // A flat image, which any real compression shrinks to almost nothing.
    let header = header(64, 64, 8, ColorType::Rgb);
    let data = vec![0; header.image_size().unwrap()];
    let scanlines = header.scanline_size().unwrap();
    let encode = |compression_strategy| {
        let options = EncodeOptions { compression_strategy, compression: CompressionLevel::BEST, ..EncodeOptions::default() };
        Encoder::with_options(options).encode(ImageRef::new(header, &data)).unwrap()
    };

    let stored = encode(CompressionStrategy::Stored);
    let fixed = encode(CompressionStrategy::Fixed);
    let dynamic = encode(CompressionStrategy::Dynamic);

    assert!(stored.len() > scanlines, "stored: {} bytes for {scanlines} of scanlines", stored.len());
    assert!(fixed.len() < scanlines / 10, "fixed: {} bytes", fixed.len());
    assert!(dynamic.len() < scanlines / 10, "dynamic: {} bytes", dynamic.len());
    for png in [stored, fixed, dynamic] {
        assert_eq!(decode(&png).data(), data);
    }
}

#[test]
fn every_format_round_trips_interlaced() {
    // Sizes where some passes are empty (1x1, 2x2, 3x5), every pass is full
    // (8x8), and rows under 8 bits end mid-byte (13x7, 9x9).
    for (color_type, bit_depth) in FORMATS {
        for (width, height) in [(1, 1), (2, 2), (3, 5), (8, 8), (9, 9), (13, 7)] {
            let mut header = header(width, height, bit_depth, color_type);
            header.interlace = Interlace::Adam7;
            let data = pixels_with_zero_padding(&header);
            let palette = full_palette(bit_depth.min(8));
            let image = match color_type {
                ColorType::Indexed => ImageRef::new(header, &data).with_palette(&palette),
                _ => ImageRef::new(header, &data),
            };

            let png = Encoder::new().encode(image).unwrap();
            let decoded = decode(&png);

            assert_eq!(*decoded.header(), header, "{color_type:?} {bit_depth}, {width}x{height}");
            assert_eq!(decoded.data(), data, "{color_type:?} {bit_depth}, {width}x{height}");
        }
    }
}

#[test]
fn interlaced_and_plain_encodes_decode_to_the_same_pixels() {
    let plain = header(13, 7, 8, ColorType::Rgb);
    let interlaced = ImageHeader { interlace: Interlace::Adam7, ..plain };
    let data = pixels(&plain);
    let mut encoder = Encoder::new();

    let plain_png = encoder.encode(ImageRef::new(plain, &data)).unwrap();
    let interlaced_png = encoder.encode(ImageRef::new(interlaced, &data)).unwrap();

    assert_ne!(plain_png, interlaced_png);
    assert_eq!(decode(&plain_png).data(), decode(&interlaced_png).data());
}

// ---------- the file written ----------

#[test]
fn writes_ihdr_idat_iend() {
    let header = header(4, 4, 8, ColorType::Rgb);
    let data = pixels(&header);

    let png = Encoder::new().encode(ImageRef::new(header, &data)).unwrap();

    assert_eq!(chunk_types(&png), ["IHDR", "IDAT", "IEND"]);
}

#[test]
fn writes_plte_between_ihdr_and_idat() {
    let header = header(4, 4, 2, ColorType::Indexed);
    let data = pixels(&header);
    let palette = full_palette(2);

    let png = Encoder::new().encode(ImageRef::new(header, &data).with_palette(&palette)).unwrap();

    assert_eq!(chunk_types(&png), ["IHDR", "PLTE", "IDAT", "IEND"]);
}

#[test]
fn rgb_images_may_have_a_suggested_palette() {
    let header = header(4, 4, 8, ColorType::Rgb);
    let data = pixels(&header);
    let palette = full_palette(2);

    let png = Encoder::new().encode(ImageRef::new(header, &data).with_palette(&palette)).unwrap();

    assert_eq!(decode(&png).palette(), Some(&palette));
}

#[test]
fn encode_into_replaces_old_contents() {
    let header = header(2, 2, 8, ColorType::Grayscale);
    let mut out = vec![0xEE; 1000];

    Encoder::new().encode_into(ImageRef::new(header, &[1, 2, 3, 4]), &mut out).unwrap();

    assert!(out.starts_with(&crate::png::SIGNATURE));
    assert_eq!(decode(&out).data(), [1, 2, 3, 4]);
}

#[test]
fn encoder_can_be_reused_for_many_images() {
    let mut encoder = Encoder::new();
    let mut out = Vec::new();

    for (width, height) in [(13, 7), (1, 1), (64, 3), (2, 2)] {
        let header = header(width, height, 8, ColorType::Rgba);
        let data = pixels(&header);

        encoder.encode_into(ImageRef::new(header, &data), &mut out).unwrap();

        assert_eq!(decode(&out).data(), data, "{width}x{height}");
    }
}

#[test]
fn same_image_and_options_give_the_same_bytes() {
    let header = header(13, 7, 8, ColorType::Rgb);
    let data = pixels(&header);
    let mut encoder = Encoder::new();

    let first = encoder.encode(ImageRef::new(header, &data)).unwrap();
    let second = encoder.encode(ImageRef::new(header, &data)).unwrap();

    assert_eq!(first, second);
}

// ---------- errors ----------

#[test]
fn rejects_pixel_data_of_the_wrong_length() {
    let header = header(2, 2, 8, ColorType::Rgb);

    for length in [0, 11, 13] {
        assert_eq!(
            Encoder::new().encode(ImageRef::new(header, &vec![0; length])),
            Err(Error::InvalidImageDataLength { expected: 12, actual: length }),
            "{length} bytes"
        );
    }
}

#[test]
fn rejects_invalid_dimensions() {
    for (width, height) in [(0, 1), (1, 0), (2_147_483_648, 1)] {
        let header = header(width, height, 8, ColorType::Grayscale);

        assert_eq!(
            Encoder::new().encode(ImageRef::new(header, &[])),
            Err(Error::InvalidDimensions { width, height }),
            "{width}x{height}"
        );
    }
}

#[test]
fn rejects_bit_depths_the_color_type_does_not_allow() {
    for (color_type, bit_depth) in [(ColorType::Grayscale, 3), (ColorType::Rgb, 4), (ColorType::Indexed, 16), (ColorType::Rgba, 1)] {
        let header = header(1, 1, bit_depth, color_type);

        assert_eq!(
            Encoder::new().encode(ImageRef::new(header, &[0; 8])),
            Err(Error::InvalidBitDepth { color_type, bit_depth }),
            "{color_type:?} {bit_depth}"
        );
    }
}

#[test]
fn indexed_images_need_a_palette() {
    let header = header(2, 2, 8, ColorType::Indexed);

    assert_eq!(Encoder::new().encode(ImageRef::new(header, &[0; 4])), Err(Error::MissingPalette));
}

#[test]
fn grayscale_images_must_not_have_a_palette() {
    let palette = full_palette(1);

    for color_type in [ColorType::Grayscale, ColorType::GrayscaleAlpha] {
        let header = header(1, 1, 8, color_type);
        let data = pixels(&header);

        assert_eq!(
            Encoder::new().encode(ImageRef::new(header, &data).with_palette(&palette)),
            Err(Error::UnexpectedPalette(color_type))
        );
    }
}

#[test]
fn palette_must_fit_the_bit_depth() {
    let header = header(8, 1, 1, ColorType::Indexed);
    let palette = full_palette(2); // 4 colors; 1 bit indexes 2

    assert_eq!(
        Encoder::new().encode(ImageRef::new(header, &[0]).with_palette(&palette)),
        Err(Error::TooManyPaletteEntries { entries: 4, bit_depth: 1 })
    );
}

// ---------- tRNS ----------

/// Encodes `header` with deterministic pixels, the palette `indexed` images
/// need, and `transparency`.
fn encode_with_transparency(header: ImageHeader, transparency: &Transparency) -> Result<Vec<u8>, Error> {
    let data = pixels(&header);
    let palette = full_palette(header.bit_depth.min(8));
    let mut image = ImageRef::new(header, &data).with_transparency(transparency);
    if header.color_type == ColorType::Indexed {
        image = image.with_palette(&palette);
    }
    Encoder::new().encode(image)
}

#[test]
fn transparency_round_trips_for_every_color_type_that_allows_it() {
    let cases = [
        (header(13, 7, 1, ColorType::Grayscale), Transparency::Gray(1)),
        (header(13, 7, 2, ColorType::Grayscale), Transparency::Gray(3)),
        (header(13, 7, 4, ColorType::Grayscale), Transparency::Gray(9)),
        (header(13, 7, 8, ColorType::Grayscale), Transparency::Gray(200)),
        (header(13, 7, 16, ColorType::Grayscale), Transparency::Gray(0xFFFF)),
        (header(13, 7, 8, ColorType::Rgb), Transparency::Rgb([1, 2, 3])),
        (header(13, 7, 16, ColorType::Rgb), Transparency::Rgb([0x1234, 0, 0xFFFF])),
        (header(13, 7, 2, ColorType::Indexed), Transparency::Palette(PaletteAlpha::from_values(&[0, 128]).unwrap())),
        (header(13, 7, 8, ColorType::Indexed), Transparency::Palette(PaletteAlpha::from_values(&[7; 256]).unwrap())),
    ];

    for (header, transparency) in cases {
        let decoded = decode(&encode_with_transparency(header, &transparency).unwrap());

        assert_eq!(decoded.transparency(), Some(&transparency), "{header:?}");
        assert_eq!(decoded.data(), pixels(&header), "{header:?}");
    }
}

#[test]
fn trns_comes_after_plte_and_before_idat() {
    let gray = encode_with_transparency(header(4, 4, 8, ColorType::Grayscale), &Transparency::Gray(0)).unwrap();
    let indexed = encode_with_transparency(header(4, 4, 2, ColorType::Indexed), &Transparency::Palette(PaletteAlpha::from_values(&[0]).unwrap())).unwrap();

    assert_eq!(chunk_types(&gray), ["IHDR", "tRNS", "IDAT", "IEND"]);
    assert_eq!(chunk_types(&indexed), ["IHDR", "PLTE", "tRNS", "IDAT", "IEND"]);
}

#[test]
fn rgb_with_a_suggested_palette_writes_trns_after_it() {
    let header = header(4, 4, 8, ColorType::Rgb);
    let data = pixels(&header);
    let palette = full_palette(2);
    let transparency = Transparency::Rgb([1, 2, 3]);

    let png = Encoder::new().encode(ImageRef::new(header, &data).with_palette(&palette).with_transparency(&transparency)).unwrap();

    assert_eq!(chunk_types(&png), ["IHDR", "PLTE", "tRNS", "IDAT", "IEND"]);
}

#[test]
fn transparent_color_decodes_with_alpha_0() {
    let header = header(2, 1, 8, ColorType::Grayscale);
    let transparency = Transparency::Gray(5);

    let png = Encoder::new().encode(ImageRef::new(header, &[5, 6]).with_transparency(&transparency)).unwrap();

    assert_eq!(crate::decode_rgba8(&png).unwrap().data(), [5, 5, 5, 0, 6, 6, 6, 255]);
}

#[test]
fn interlaced_images_keep_their_transparency() {
    let mut header = header(13, 7, 8, ColorType::Rgb);
    header.interlace = Interlace::Adam7;
    let transparency = Transparency::Rgb([10, 20, 30]);

    let decoded = decode(&encode_with_transparency(header, &transparency).unwrap());

    assert_eq!(decoded.transparency(), Some(&transparency));
}

#[test]
fn images_with_alpha_must_not_have_transparency() {
    for color_type in [ColorType::GrayscaleAlpha, ColorType::Rgba] {
        assert_eq!(
            encode_with_transparency(header(2, 2, 8, color_type), &Transparency::Gray(0)),
            Err(Error::UnexpectedTransparency(color_type))
        );
    }
}

#[test]
fn transparency_kind_must_match_the_color_type() {
    let two_values = Transparency::Palette(PaletteAlpha::from_values(&[0, 0]).unwrap());
    let cases = [
        (ColorType::Rgb, Transparency::Gray(0), 2),
        (ColorType::Indexed, Transparency::Gray(0), 2),
        (ColorType::Grayscale, Transparency::Rgb([0; 3]), 6),
        (ColorType::Indexed, Transparency::Rgb([0; 3]), 6),
        (ColorType::Grayscale, two_values.clone(), 2),
        (ColorType::Rgb, two_values, 2),
    ];

    for (color_type, transparency, length) in cases {
        assert_eq!(
            encode_with_transparency(header(2, 2, 8, color_type), &transparency),
            Err(Error::InvalidTransparencyLength { color_type, length }),
            "{color_type:?} with {transparency:?}"
        );
    }
}

#[test]
fn transparent_color_must_fit_the_bit_depth() {
    let too_big = [
        (header(2, 2, 1, ColorType::Grayscale), Transparency::Gray(2)),
        (header(2, 2, 4, ColorType::Grayscale), Transparency::Gray(16)),
        (header(2, 2, 8, ColorType::Grayscale), Transparency::Gray(256)),
        (header(2, 2, 8, ColorType::Rgb), Transparency::Rgb([0, 256, 0])),
    ];
    for (header, transparency) in too_big {
        assert_eq!(
            encode_with_transparency(header, &transparency),
            Err(Error::InvalidChunkData(ChunkType::TRNS)),
            "{header:?} with {transparency:?}"
        );
    }

    // The largest values that fit.
    assert!(encode_with_transparency(header(2, 2, 4, ColorType::Grayscale), &Transparency::Gray(15)).is_ok());
    assert!(encode_with_transparency(header(2, 2, 16, ColorType::Rgb), &Transparency::Rgb([0xFFFF; 3])).is_ok());
}

#[test]
fn palette_alpha_must_not_outnumber_the_palette() {
    let header = header(2, 2, 8, ColorType::Indexed);
    let palette = Palette::from_colors(&[[0; 3], [255; 3]]).unwrap();
    let transparency = Transparency::Palette(PaletteAlpha::from_values(&[0, 0, 0]).unwrap());

    assert_eq!(
        Encoder::new().encode(ImageRef::new(header, &[0, 1, 1, 0]).with_palette(&palette).with_transparency(&transparency)),
        Err(Error::TooManyTransparencyEntries { entries: 3, palette_entries: 2 })
    );
}
