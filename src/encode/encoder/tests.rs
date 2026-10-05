#![cfg(test)]

use rust_deflate::{CompressionLevel, Strategy as CompressionStrategy};

use super::{AUTO_PALETTE_COMPARE_LIMIT, Encoder};
use crate::encode::image_ref::ImageRef;
use crate::encode::options::{EncodeOptions, FilterStrategy, PaletteMode, StripChunks};
use crate::error::Error;
use crate::png::metadata::{
    Gamma, PhysicalDimensions, RenderingIntent, Text, TextKind, Time, Unit,
};
use crate::png::{
    ChunkPosition, ChunkType, ColorType, FilterType, Image, ImageHeader, Interlace, Metadata,
    OwnedChunk, Palette, PaletteAlpha, Transparency,
};
use crate::{ChunkReader, DecodeOptions, Decoder};

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
    ImageHeader {
        width,
        height,
        bit_depth,
        color_type,
        interlace: Interlace::None,
    }
}

/// Deterministic pixel data of the right size for `header`.
fn pixels(header: &ImageHeader) -> Vec<u8> {
    (0..header.image_size().unwrap())
        .map(|i| (i as u8).wrapping_mul(97).wrapping_add(13).rotate_left(3))
        .collect()
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
    let colors: Vec<[u8; 3]> = (0..1u16 << bit_depth)
        .map(|i| [i as u8, (i * 3) as u8, (255 - i) as u8])
        .collect();
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
    let encoder = Encoder::with_options(EncodeOptions {
        compression: CompressionLevel::BEST,
        ..EncodeOptions::default()
    });

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

    let decoded = decode(
        &Encoder::new()
            .encode(ImageRef::new(header, &[1, 2, 3, 4]))
            .unwrap(),
    );

    assert_eq!(decoded.data(), [1, 2, 3, 4]);
}

#[test]
fn every_option_round_trips() {
    let header = header(17, 9, 8, ColorType::Rgb);
    let data = pixels(&header);
    let filters = [
        FilterType::None,
        FilterType::Sub,
        FilterType::Up,
        FilterType::Average,
        FilterType::Paeth,
    ]
    .map(FilterStrategy::Fixed)
    .into_iter()
    .chain([FilterStrategy::Adaptive]);

    for filter in filters {
        for compression in [
            CompressionLevel::NONE,
            CompressionLevel::FAST,
            CompressionLevel::MEDIUM,
            CompressionLevel::BEST,
        ] {
            for compression_strategy in [
                CompressionStrategy::Stored,
                CompressionStrategy::Fixed,
                CompressionStrategy::Dynamic,
            ] {
                let mut encoder = Encoder::with_options(EncodeOptions {
                    compression,
                    compression_strategy,
                    filter,
                    ..EncodeOptions::default()
                });

                let decoded = decode(&encoder.encode(ImageRef::new(header, &data)).unwrap());

                assert_eq!(
                    decoded.data(),
                    data,
                    "{filter:?}, {compression:?}, {compression_strategy:?}"
                );
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
        let options = EncodeOptions {
            compression_strategy,
            compression: CompressionLevel::BEST,
            ..EncodeOptions::default()
        };
        Encoder::with_options(options)
            .encode(ImageRef::new(header, &data))
            .unwrap()
    };

    let stored = encode(CompressionStrategy::Stored);
    let fixed = encode(CompressionStrategy::Fixed);
    let dynamic = encode(CompressionStrategy::Dynamic);

    assert!(
        stored.len() > scanlines,
        "stored: {} bytes for {scanlines} of scanlines",
        stored.len()
    );
    assert!(fixed.len() < scanlines / 10, "fixed: {} bytes", fixed.len());
    assert!(
        dynamic.len() < scanlines / 10,
        "dynamic: {} bytes",
        dynamic.len()
    );
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

            assert_eq!(
                *decoded.header(),
                header,
                "{color_type:?} {bit_depth}, {width}x{height}"
            );
            assert_eq!(
                decoded.data(),
                data,
                "{color_type:?} {bit_depth}, {width}x{height}"
            );
        }
    }
}

#[test]
fn interlaced_and_plain_encodes_decode_to_the_same_pixels() {
    let plain = header(13, 7, 8, ColorType::Rgb);
    let interlaced = ImageHeader {
        interlace: Interlace::Adam7,
        ..plain
    };
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

    let png = Encoder::new()
        .encode(ImageRef::new(header, &data).with_palette(&palette))
        .unwrap();

    assert_eq!(chunk_types(&png), ["IHDR", "PLTE", "IDAT", "IEND"]);
}

#[test]
fn rgb_images_may_have_a_suggested_palette() {
    let header = header(4, 4, 8, ColorType::Rgb);
    let data = pixels(&header);
    let palette = full_palette(2);

    let png = Encoder::new()
        .encode(ImageRef::new(header, &data).with_palette(&palette))
        .unwrap();

    assert_eq!(decode(&png).palette(), Some(&palette));
}

#[test]
fn encode_into_replaces_old_contents() {
    let header = header(2, 2, 8, ColorType::Grayscale);
    let mut out = vec![0xEE; 1000];

    Encoder::new()
        .encode_into(ImageRef::new(header, &[1, 2, 3, 4]), &mut out)
        .unwrap();

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

        encoder
            .encode_into(ImageRef::new(header, &data), &mut out)
            .unwrap();

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
            Err(Error::InvalidImageDataLength {
                expected: 12,
                actual: length
            }),
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
    for (color_type, bit_depth) in [
        (ColorType::Grayscale, 3),
        (ColorType::Rgb, 4),
        (ColorType::Indexed, 16),
        (ColorType::Rgba, 1),
    ] {
        let header = header(1, 1, bit_depth, color_type);

        assert_eq!(
            Encoder::new().encode(ImageRef::new(header, &[0; 8])),
            Err(Error::InvalidBitDepth {
                color_type,
                bit_depth
            }),
            "{color_type:?} {bit_depth}"
        );
    }
}

#[test]
fn indexed_images_need_a_palette() {
    let header = header(2, 2, 8, ColorType::Indexed);

    assert_eq!(
        Encoder::new().encode(ImageRef::new(header, &[0; 4])),
        Err(Error::MissingPalette)
    );
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
        Err(Error::TooManyPaletteEntries {
            entries: 4,
            bit_depth: 1
        })
    );
}

// ---------- tRNS ----------

/// Encodes `header` with deterministic pixels, the palette `indexed` images
/// need, and `transparency`.
fn encode_with_transparency(
    header: ImageHeader,
    transparency: &Transparency,
) -> Result<Vec<u8>, Error> {
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
        (
            header(13, 7, 1, ColorType::Grayscale),
            Transparency::Gray(1),
        ),
        (
            header(13, 7, 2, ColorType::Grayscale),
            Transparency::Gray(3),
        ),
        (
            header(13, 7, 4, ColorType::Grayscale),
            Transparency::Gray(9),
        ),
        (
            header(13, 7, 8, ColorType::Grayscale),
            Transparency::Gray(200),
        ),
        (
            header(13, 7, 16, ColorType::Grayscale),
            Transparency::Gray(0xFFFF),
        ),
        (
            header(13, 7, 8, ColorType::Rgb),
            Transparency::Rgb([1, 2, 3]),
        ),
        (
            header(13, 7, 16, ColorType::Rgb),
            Transparency::Rgb([0x1234, 0, 0xFFFF]),
        ),
        (
            header(13, 7, 2, ColorType::Indexed),
            Transparency::Palette(PaletteAlpha::from_values(&[0, 128]).unwrap()),
        ),
        (
            header(13, 7, 8, ColorType::Indexed),
            Transparency::Palette(PaletteAlpha::from_values(&[7; 256]).unwrap()),
        ),
    ];

    for (header, transparency) in cases {
        let decoded = decode(&encode_with_transparency(header, &transparency).unwrap());

        assert_eq!(decoded.transparency(), Some(&transparency), "{header:?}");
        assert_eq!(decoded.data(), pixels(&header), "{header:?}");
    }
}

#[test]
fn trns_comes_after_plte_and_before_idat() {
    let gray = encode_with_transparency(
        header(4, 4, 8, ColorType::Grayscale),
        &Transparency::Gray(0),
    )
    .unwrap();
    let indexed = encode_with_transparency(
        header(4, 4, 2, ColorType::Indexed),
        &Transparency::Palette(PaletteAlpha::from_values(&[0]).unwrap()),
    )
    .unwrap();

    assert_eq!(chunk_types(&gray), ["IHDR", "tRNS", "IDAT", "IEND"]);
    assert_eq!(
        chunk_types(&indexed),
        ["IHDR", "PLTE", "tRNS", "IDAT", "IEND"]
    );
}

#[test]
fn rgb_with_a_suggested_palette_writes_trns_after_it() {
    let header = header(4, 4, 8, ColorType::Rgb);
    let data = pixels(&header);
    let palette = full_palette(2);
    let transparency = Transparency::Rgb([1, 2, 3]);

    let png = Encoder::new()
        .encode(
            ImageRef::new(header, &data)
                .with_palette(&palette)
                .with_transparency(&transparency),
        )
        .unwrap();

    assert_eq!(chunk_types(&png), ["IHDR", "PLTE", "tRNS", "IDAT", "IEND"]);
}

#[test]
fn transparent_color_decodes_with_alpha_0() {
    let header = header(2, 1, 8, ColorType::Grayscale);
    let transparency = Transparency::Gray(5);

    let png = Encoder::new()
        .encode(ImageRef::new(header, &[5, 6]).with_transparency(&transparency))
        .unwrap();

    assert_eq!(
        crate::decode_rgba8(&png).unwrap().data(),
        [5, 5, 5, 0, 6, 6, 6, 255]
    );
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
        (
            header(2, 2, 4, ColorType::Grayscale),
            Transparency::Gray(16),
        ),
        (
            header(2, 2, 8, ColorType::Grayscale),
            Transparency::Gray(256),
        ),
        (
            header(2, 2, 8, ColorType::Rgb),
            Transparency::Rgb([0, 256, 0]),
        ),
    ];
    for (header, transparency) in too_big {
        assert_eq!(
            encode_with_transparency(header, &transparency),
            Err(Error::InvalidChunkData(ChunkType::TRNS)),
            "{header:?} with {transparency:?}"
        );
    }

    // The largest values that fit.
    assert!(
        encode_with_transparency(
            header(2, 2, 4, ColorType::Grayscale),
            &Transparency::Gray(15)
        )
        .is_ok()
    );
    assert!(
        encode_with_transparency(
            header(2, 2, 16, ColorType::Rgb),
            &Transparency::Rgb([0xFFFF; 3])
        )
        .is_ok()
    );
}

#[test]
fn palette_alpha_must_not_outnumber_the_palette() {
    let header = header(2, 2, 8, ColorType::Indexed);
    let palette = Palette::from_colors(&[[0; 3], [255; 3]]).unwrap();
    let transparency = Transparency::Palette(PaletteAlpha::from_values(&[0, 0, 0]).unwrap());

    assert_eq!(
        Encoder::new().encode(
            ImageRef::new(header, &[0, 1, 1, 0])
                .with_palette(&palette)
                .with_transparency(&transparency)
        ),
        Err(Error::TooManyTransparencyEntries {
            entries: 3,
            palette_entries: 2
        })
    );
}

// ---------- metadata ----------

fn title(text: &str, kind: TextKind) -> Text {
    Text {
        keyword: "Title".to_string(),
        text: text.to_string(),
        language_tag: String::new(),
        translated_keyword: String::new(),
        kind,
    }
}

/// A chunk of every kind that goes before `PLTE`, and of every kind that goes after.
fn some_metadata() -> Metadata {
    Metadata::default()
        .with_srgb(RenderingIntent::Perceptual)
        .with_gamma(Gamma::parse(&45455u32.to_be_bytes()).unwrap())
        .with_physical_dimensions(PhysicalDimensions {
            x: 3780,
            y: 3780,
            unit: Unit::Meter,
        })
        .with_time(Time {
            year: 2026,
            month: 10,
            day: 3,
            hour: 9,
            minute: 30,
            second: 0,
        })
        .with_text(title("format-png", TextKind::Plain))
        .with_text(title("compressed", TextKind::Compressed))
        .with_text(title(
            "blåbær",
            TextKind::International { compressed: true },
        ))
}

fn decode_with_metadata(png: &[u8]) -> Image {
    let options = DecodeOptions {
        preserve_metadata: true,
        strict_ancillary: true,
        ..DecodeOptions::default()
    };
    Decoder::with_options(options).decode(png).unwrap()
}

#[test]
fn metadata_round_trips_through_the_decoder() {
    let header = header(13, 7, 8, ColorType::Rgb);
    let data = pixels(&header);
    let metadata = some_metadata();

    let decoded = decode_with_metadata(
        &Encoder::new()
            .encode(ImageRef::new(header, &data).with_metadata(&metadata))
            .unwrap(),
    );

    assert_eq!(decoded.metadata(), &metadata);
    assert_eq!(decoded.data(), data);
}

#[test]
fn metadata_chunks_go_around_plte_and_trns() {
    let header = header(4, 4, 2, ColorType::Indexed);
    let data = pixels(&header);
    let palette = full_palette(2);
    let transparency = Transparency::Palette(PaletteAlpha::from_values(&[0]).unwrap());
    let metadata = some_metadata();

    let png = Encoder::new()
        .encode(
            ImageRef::new(header, &data)
                .with_palette(&palette)
                .with_transparency(&transparency)
                .with_metadata(&metadata),
        )
        .unwrap();

    assert_eq!(
        chunk_types(&png),
        [
            "IHDR", "sRGB", "gAMA", "PLTE", "tRNS", "pHYs", "tIME", "tEXt", "zTXt", "iTXt", "IDAT",
            "IEND"
        ]
    );
}

#[test]
fn empty_metadata_writes_no_chunks() {
    let header = header(4, 4, 8, ColorType::Rgb);
    let data = pixels(&header);
    let metadata = Metadata::default();

    let png = Encoder::new()
        .encode(ImageRef::new(header, &data).with_metadata(&metadata))
        .unwrap();

    assert_eq!(chunk_types(&png), ["IHDR", "IDAT", "IEND"]);
}

#[test]
fn invalid_metadata_is_rejected() {
    let header = header(4, 4, 8, ColorType::Rgb);
    let data = pixels(&header);
    let metadata = Metadata::default().with_text(Text {
        keyword: " Title".to_string(),
        ..title("x", TextKind::Plain)
    });

    assert_eq!(
        Encoder::new().encode(ImageRef::new(header, &data).with_metadata(&metadata)),
        Err(Error::InvalidChunkData(ChunkType::TEXT))
    );
}

#[test]
fn stored_compression_applies_to_compressed_text_too() {
    let header = header(4, 4, 8, ColorType::Rgb);
    let data = pixels(&header);
    let long = "the same words again and again ".repeat(100);
    let metadata = Metadata::default().with_text(title(&long, TextKind::Compressed));
    let options = EncodeOptions {
        compression_strategy: CompressionStrategy::Stored,
        ..EncodeOptions::default()
    };

    let png = Encoder::with_options(options)
        .encode(ImageRef::new(header, &data).with_metadata(&metadata))
        .unwrap();

    assert!(png.len() > long.len(), "{} bytes", png.len());
    assert_eq!(decode_with_metadata(&png).metadata(), &metadata);
}

// ---------- extra chunks ----------

fn extra(bytes: &[u8; 4], data: &[u8], position: ChunkPosition) -> OwnedChunk {
    OwnedChunk::from_data(
        ChunkType::from_bytes(*bytes).unwrap(),
        data.to_vec(),
        position,
    )
}

#[test]
fn extra_chunks_go_at_their_positions() {
    let header = header(4, 4, 2, ColorType::Indexed);
    let data = pixels(&header);
    let palette = full_palette(2);
    let transparency = Transparency::Palette(PaletteAlpha::from_values(&[0]).unwrap());
    let metadata = Metadata::default()
        .with_srgb(RenderingIntent::Perceptual)
        .with_text(title("x", TextKind::Plain));
    let chunks = [
        extra(b"afTr", b"", ChunkPosition::AfterImageData),
        extra(b"miDl", b"", ChunkPosition::BeforeImageData),
        extra(b"beFr", b"", ChunkPosition::BeforePalette),
    ];

    let png = Encoder::new()
        .encode(
            ImageRef::new(header, &data)
                .with_palette(&palette)
                .with_transparency(&transparency)
                .with_metadata(&metadata)
                .with_chunks(&chunks),
        )
        .unwrap();

    assert_eq!(
        chunk_types(&png),
        [
            "IHDR", "sRGB", "beFr", "PLTE", "tRNS", "tEXt", "miDl", "IDAT", "afTr", "IEND"
        ]
    );
}

#[test]
fn extra_chunks_keep_their_order_and_data() {
    let header = header(2, 2, 8, ColorType::Grayscale);
    let chunks = [
        extra(b"onEa", b"first", ChunkPosition::AfterImageData),
        extra(b"twOa", b"second", ChunkPosition::AfterImageData),
    ];

    let png = Encoder::new()
        .encode(ImageRef::new(header, &[0; 4]).with_chunks(&chunks))
        .unwrap();

    let read = crate::read_chunks(&png).unwrap();
    let unknown: Vec<_> = read
        .unknown_chunks()
        .map(|c| (c.chunk_type().to_string(), c.data().to_vec()))
        .collect();
    assert_eq!(
        unknown,
        [
            ("onEa".to_string(), b"first".to_vec()),
            ("twOa".to_string(), b"second".to_vec())
        ]
    );
}

#[test]
fn unsafe_extra_chunks_need_keep_unsafe_chunks() {
    let header = header(2, 2, 8, ColorType::Grayscale);
    let chunks = [
        extra(b"bKGD", &[0, 7], ChunkPosition::BeforeImageData),
        extra(b"ruSt", b"", ChunkPosition::BeforeImageData),
    ];
    let image = ImageRef::new(header, &[0; 4]).with_chunks(&chunks);

    let default = Encoder::new().encode(image).unwrap();
    let kept = Encoder::with_options(EncodeOptions {
        keep_unsafe_chunks: true,
        ..EncodeOptions::default()
    })
    .encode(image)
    .unwrap();

    assert_eq!(chunk_types(&default), ["IHDR", "ruSt", "IDAT", "IEND"]);
    assert_eq!(chunk_types(&kept), ["IHDR", "bKGD", "ruSt", "IDAT", "IEND"]);
}

#[test]
fn raw_chunks_of_types_the_encoder_writes_are_skipped() {
    let header = header(2, 2, 8, ColorType::Grayscale);
    let metadata = Metadata::default().with_text(title("typed", TextKind::Plain));
    let chunks = [extra(
        b"tEXt",
        b"Title\0raw",
        ChunkPosition::BeforeImageData,
    )];

    let png = Encoder::new()
        .encode(
            ImageRef::new(header, &[0; 4])
                .with_metadata(&metadata)
                .with_chunks(&chunks),
        )
        .unwrap();

    assert_eq!(chunk_types(&png), ["IHDR", "tEXt", "IDAT", "IEND"]);
    assert_eq!(
        decode_with_metadata(&png).metadata().text()[0].text,
        "typed"
    );
}

#[test]
fn critical_extra_chunks_are_rejected() {
    let header = header(2, 2, 8, ColorType::Grayscale);
    let chunks = [extra(b"CuSt", b"", ChunkPosition::BeforeImageData)];

    assert_eq!(
        Encoder::new().encode(ImageRef::new(header, &[0; 4]).with_chunks(&chunks)),
        Err(Error::UnexpectedCriticalChunk(
            ChunkType::from_bytes(*b"CuSt").unwrap()
        ))
    );
}

// ---------- PaletteMode ----------

/// 80x60 RGBA: 19200 bytes, over `AUTO_PALETTE_COMPARE_LIMIT`, so `Auto`
/// converts without comparing and the tests below see the conversion itself.
const OVER_LIMIT: (u32, u32) = (80, 60);

/// An 8-bit RGB image over `AUTO_PALETTE_COMPARE_LIMIT`, alternating `colors`.
fn rgb_over_limit(colors: &[[u8; 3]]) -> (ImageHeader, Vec<u8>) {
    let header = header(100, 60, 8, ColorType::Rgb);
    let data = (0..100 * 60)
        .flat_map(|i| colors[i % colors.len()])
        .collect();
    (header, data)
}

fn auto_palette() -> EncodeOptions {
    EncodeOptions {
        palette: PaletteMode::Auto,
        ..EncodeOptions::default()
    }
}

/// An 8-bit RGBA image using `colors` distinct colors in a pattern, some of
/// them translucent.
fn few_colors(width: u32, height: u32, colors: usize) -> (ImageHeader, Vec<u8>) {
    let header = header(width, height, 8, ColorType::Rgba);
    let data = (0..(width * height) as usize)
        .flat_map(|i| {
            let c = (i * 7 + i / width as usize) % colors;
            // Distinct for every c below 65536.
            [
                c as u8,
                (c >> 8) as u8,
                (c * 3) as u8,
                if c.is_multiple_of(3) { 128 } else { 255 },
            ]
        })
        .collect();
    (header, data)
}

#[test]
fn default_keeps_the_color_type() {
    let (header, data) = few_colors(8, 8, 4);

    let png = Encoder::new().encode(ImageRef::new(header, &data)).unwrap();

    assert_eq!(decode(&png).header().color_type, ColorType::Rgba);
}

#[test]
fn auto_palette_writes_few_colors_as_indexed_with_the_same_pixels() {
    let (header, data) = few_colors(OVER_LIMIT.0, OVER_LIMIT.1, 10);

    let indexed = Encoder::with_options(auto_palette())
        .encode(ImageRef::new(header, &data))
        .unwrap();
    let rgba = Encoder::new().encode(ImageRef::new(header, &data)).unwrap();

    let decoded = decode(&indexed);
    assert_eq!(decoded.header().color_type, ColorType::Indexed);
    assert_eq!(decoded.header().bit_depth, 4);
    assert_eq!(
        crate::decode_rgba8(&indexed).unwrap(),
        crate::decode_rgba8(&rgba).unwrap()
    );
    assert_eq!(
        chunk_types(&indexed),
        ["IHDR", "PLTE", "tRNS", "IDAT", "IEND"]
    );
}

#[test]
fn auto_palette_makes_few_color_images_smaller() {
    let (header, data) = few_colors(64, 64, 4);

    let indexed = Encoder::with_options(auto_palette())
        .encode(ImageRef::new(header, &data))
        .unwrap();
    let rgba = Encoder::new().encode(ImageRef::new(header, &data)).unwrap();

    assert!(
        indexed.len() < rgba.len(),
        "indexed {} bytes, RGBA {} bytes",
        indexed.len(),
        rgba.len()
    );
}

#[test]
fn auto_palette_leaves_images_with_more_colors() {
    let (header, data) = few_colors(20, 20, 257);

    let png = Encoder::with_options(auto_palette())
        .encode(ImageRef::new(header, &data))
        .unwrap();

    assert_eq!(decode(&png).header().color_type, ColorType::Rgba);
    assert_eq!(decode(&png).data(), data);
}

#[test]
fn auto_palette_leaves_other_color_types() {
    for (color_type, bit_depth) in [
        (ColorType::Grayscale, 8),
        (ColorType::GrayscaleAlpha, 8),
        (ColorType::Rgb, 16),
    ] {
        let header = header(4, 4, bit_depth, color_type);
        let data = pixels(&header);

        let png = Encoder::with_options(auto_palette())
            .encode(ImageRef::new(header, &data))
            .unwrap();

        assert_eq!(*decode(&png).header(), header);
    }
}

#[test]
fn auto_palette_keeps_interlacing() {
    let (mut header, data) = few_colors(OVER_LIMIT.0, OVER_LIMIT.1, 5);
    header.interlace = Interlace::Adam7;

    let png = Encoder::with_options(auto_palette())
        .encode(ImageRef::new(header, &data))
        .unwrap();

    let decoded = decode(&png);
    assert_eq!(
        (decoded.header().color_type, decoded.header().interlace),
        (ColorType::Indexed, Interlace::Adam7)
    );
    assert_eq!(crate::decode_rgba8(&png).unwrap().data(), data);
}

#[test]
fn auto_palette_turns_an_rgb_color_key_into_alpha() {
    let (header, data) = rgb_over_limit(&[[5, 5, 5], [6, 6, 6]]);
    let key = Transparency::Rgb([5, 5, 5]);

    let png = Encoder::with_options(auto_palette())
        .encode(ImageRef::new(header, &data).with_transparency(&key))
        .unwrap();

    assert_eq!(decode(&png).header().color_type, ColorType::Indexed);
    assert_eq!(
        crate::decode_rgba8(&png).unwrap().data()[..8],
        [5, 5, 5, 0, 6, 6, 6, 255]
    );
}

#[test]
fn auto_palette_replaces_a_suggested_palette() {
    let (header, data) = rgb_over_limit(&[[9, 9, 9], [8, 8, 8]]);
    let suggested = full_palette(2);

    let png = Encoder::with_options(auto_palette())
        .encode(ImageRef::new(header, &data).with_palette(&suggested))
        .unwrap();

    assert_eq!(chunk_types(&png), ["IHDR", "PLTE", "IDAT", "IEND"]);
    assert_eq!(
        decode(&png).palette().unwrap().colors(),
        [[9, 9, 9], [8, 8, 8]]
    );
}

#[test]
fn auto_palette_drops_unsafe_chunks_only_when_it_converts() {
    let options = EncodeOptions {
        keep_unsafe_chunks: true,
        ..auto_palette()
    };
    let chunks = [
        extra(b"bKGD", &[0, 0, 0, 0, 0, 0], ChunkPosition::BeforeImageData),
        extra(b"ruSt", b"", ChunkPosition::BeforeImageData),
    ];
    let (few, few_data) = few_colors(OVER_LIMIT.0, OVER_LIMIT.1, 3);
    let (many, many_data) = few_colors(20, 20, 300);

    let converted = Encoder::with_options(options.clone())
        .encode(ImageRef::new(few, &few_data).with_chunks(&chunks))
        .unwrap();
    let kept = Encoder::with_options(options)
        .encode(ImageRef::new(many, &many_data).with_chunks(&chunks))
        .unwrap();

    assert!(!chunk_types(&converted).contains(&"bKGD".to_string()));
    assert!(chunk_types(&converted).contains(&"ruSt".to_string()));
    assert!(chunk_types(&kept).contains(&"bKGD".to_string()));
}

#[test]
fn auto_palette_keeps_metadata() {
    let (header, data) = few_colors(4, 4, 3);
    let metadata = some_metadata();

    let png = Encoder::with_options(auto_palette())
        .encode(ImageRef::new(header, &data).with_metadata(&metadata))
        .unwrap();

    assert_eq!(decode_with_metadata(&png).metadata(), &metadata);
}

#[test]
fn auto_palette_encoder_can_be_reused() {
    let mut encoder = Encoder::with_options(auto_palette());
    let mut out = Vec::new();

    for (width, height, colors) in [(13, 7, 10), (20, 20, 300), (3, 3, 2), (64, 2, 256)] {
        let (header, data) = few_colors(width, height, colors);

        encoder
            .encode_into(ImageRef::new(header, &data), &mut out)
            .unwrap();

        assert_eq!(
            crate::decode_rgba8(&out).unwrap().data(),
            data,
            "{width}x{height}, {colors} colors"
        );
    }
}

// ---------- StripChunks ----------

fn strip(strip: StripChunks) -> EncodeOptions {
    EncodeOptions {
        strip,
        ..EncodeOptions::default()
    }
}

/// An RGB image with a `tRNS` color, metadata of every kind `some_metadata`
/// has, and a private extra chunk, encoded with `options`.
fn encode_with_everything(options: EncodeOptions) -> Vec<u8> {
    let header = header(13, 7, 8, ColorType::Rgb);
    let data = pixels(&header);
    let transparency = Transparency::Rgb([0, 0, 0]);
    let metadata = some_metadata();
    let chunks = [extra(b"ruSt", b"private", ChunkPosition::BeforeImageData)];

    Encoder::with_options(options)
        .encode(
            ImageRef::new(header, &data)
                .with_transparency(&transparency)
                .with_metadata(&metadata)
                .with_chunks(&chunks),
        )
        .unwrap()
}

#[test]
fn keep_writes_every_chunk() {
    assert_eq!(
        chunk_types(&encode_with_everything(EncodeOptions::default())),
        [
            "IHDR", "sRGB", "gAMA", "tRNS", "pHYs", "tIME", "tEXt", "zTXt", "iTXt", "ruSt", "IDAT",
            "IEND"
        ]
    );
}

#[test]
fn safe_keeps_color_trns_and_physical_size() {
    assert_eq!(
        chunk_types(&encode_with_everything(strip(StripChunks::Safe))),
        ["IHDR", "sRGB", "gAMA", "tRNS", "pHYs", "IDAT", "IEND"]
    );
}

#[test]
fn all_keeps_only_trns() {
    assert_eq!(
        chunk_types(&encode_with_everything(strip(StripChunks::All))),
        ["IHDR", "tRNS", "IDAT", "IEND"]
    );
}

#[test]
fn stripping_keeps_the_pixels_and_shrinks_the_file() {
    let kept = encode_with_everything(EncodeOptions::default());
    let safe = encode_with_everything(strip(StripChunks::Safe));
    let all = encode_with_everything(strip(StripChunks::All));

    for png in [&safe, &all] {
        assert_eq!(
            crate::decode_rgba8(png).unwrap(),
            crate::decode_rgba8(&kept).unwrap()
        );
    }
    assert!(
        all.len() < safe.len() && safe.len() < kept.len(),
        "{} < {} < {}",
        all.len(),
        safe.len(),
        kept.len()
    );
}

#[test]
fn stripped_chunks_are_not_validated() {
    let header = header(2, 2, 8, ColorType::Rgb);
    let data = pixels(&header);
    let bad_text = Metadata::default().with_text(Text {
        keyword: " bad".to_string(),
        ..title("x", TextKind::Plain)
    });
    let critical = [extra(b"CuSt", b"", ChunkPosition::BeforeImageData)];
    let image = ImageRef::new(header, &data)
        .with_metadata(&bad_text)
        .with_chunks(&critical);

    assert_eq!(
        Encoder::new().encode(image),
        Err(Error::InvalidChunkData(ChunkType::TEXT))
    );
    for mode in [StripChunks::Safe, StripChunks::All] {
        assert!(
            Encoder::with_options(strip(mode)).encode(image).is_ok(),
            "{mode:?}"
        );
    }
}

#[test]
fn kept_chunks_are_still_validated() {
    let header = header(2, 2, 8, ColorType::Rgb);
    let data = pixels(&header);
    let bad_icc = Metadata::default().with_icc_profile(crate::png::metadata::IccProfile {
        name: String::new(),
        profile: vec![1],
    });

    assert_eq!(
        Encoder::with_options(strip(StripChunks::Safe))
            .encode(ImageRef::new(header, &data).with_metadata(&bad_icc)),
        Err(Error::InvalidChunkData(ChunkType::ICCP))
    );
}

#[test]
fn stripping_works_with_auto_palette() {
    let (header, data) = few_colors(OVER_LIMIT.0, OVER_LIMIT.1, 5);
    let metadata = some_metadata();
    let options = EncodeOptions {
        strip: StripChunks::All,
        ..auto_palette()
    };

    let png = Encoder::with_options(options)
        .encode(ImageRef::new(header, &data).with_metadata(&metadata))
        .unwrap();

    // tRNS here is the palette's alpha, which All keeps like any other tRNS.
    assert_eq!(chunk_types(&png), ["IHDR", "PLTE", "tRNS", "IDAT", "IEND"]);
    assert_eq!(crate::decode_rgba8(&png).unwrap().data(), data);
}

#[test]
fn stripping_drops_a_suggested_palette() {
    let header = header(4, 4, 8, ColorType::Rgb);
    let data = pixels(&header);
    let suggested = full_palette(2);
    let image = ImageRef::new(header, &data).with_palette(&suggested);

    assert_eq!(
        chunk_types(&Encoder::new().encode(image).unwrap()),
        ["IHDR", "PLTE", "IDAT", "IEND"]
    );
    for mode in [StripChunks::Safe, StripChunks::All] {
        let png = Encoder::with_options(strip(mode)).encode(image).unwrap();

        assert_eq!(chunk_types(&png), ["IHDR", "IDAT", "IEND"], "{mode:?}");
        assert_eq!(decode(&png).data(), data, "{mode:?}");
    }
}

#[test]
fn stripping_keeps_an_indexed_images_palette() {
    let header = header(4, 4, 2, ColorType::Indexed);
    let data = pixels(&header);
    let palette = full_palette(2);

    for mode in [StripChunks::Safe, StripChunks::All] {
        let png = Encoder::with_options(strip(mode))
            .encode(ImageRef::new(header, &data).with_palette(&palette))
            .unwrap();

        assert_eq!(
            chunk_types(&png),
            ["IHDR", "PLTE", "IDAT", "IEND"],
            "{mode:?}"
        );
        assert_eq!(decode(&png).palette(), Some(&palette), "{mode:?}");
    }
}

#[test]
fn stripping_drops_a_suggested_palette_even_when_auto_palette_cannot_convert() {
    let (header, data) = few_colors(20, 20, 300);
    let suggested = full_palette(2);
    let options = EncodeOptions {
        strip: StripChunks::All,
        ..auto_palette()
    };

    let png = Encoder::with_options(options)
        .encode(ImageRef::new(header, &data).with_palette(&suggested))
        .unwrap();

    assert_eq!(chunk_types(&png), ["IHDR", "IDAT", "IEND"]);
}

// ---------- PaletteMode::Auto on small images ----------

#[test]
fn the_size_helpers_are_on_the_right_side_of_the_limit() {
    let (over, _) = few_colors(OVER_LIMIT.0, OVER_LIMIT.1, 1);
    let (rgb_over, _) = rgb_over_limit(&[[0; 3]]);

    assert!(over.image_size().unwrap() > AUTO_PALETTE_COMPARE_LIMIT);
    assert!(rgb_over.image_size().unwrap() > AUTO_PALETTE_COMPARE_LIMIT);
    assert_eq!(
        header(64, 64, 8, ColorType::Rgba).image_size().unwrap(),
        AUTO_PALETTE_COMPARE_LIMIT
    );
}

#[test]
fn auto_palette_keeps_a_tiny_image_as_given_when_the_palette_costs_more() {
    // One opaque pixel: the RGBA data compresses to a few bytes, less than the
    // 15-byte PLTE chunk that indexed color would add.
    let header = header(1, 1, 8, ColorType::Rgba);
    let data = [10, 20, 30, 255];

    let auto = Encoder::with_options(auto_palette())
        .encode(ImageRef::new(header, &data))
        .unwrap();
    let as_given = Encoder::new().encode(ImageRef::new(header, &data)).unwrap();

    assert_eq!(decode(&auto).header().color_type, ColorType::Rgba);
    assert_eq!(auto, as_given);
}

#[test]
fn auto_palette_still_converts_small_images_when_it_helps() {
    // 64x64 RGBA in 4 colors, right at the limit: indexed is far smaller.
    let (header, data) = few_colors(64, 64, 4);

    let png = Encoder::with_options(auto_palette())
        .encode(ImageRef::new(header, &data))
        .unwrap();

    assert_eq!(decode(&png).header().color_type, ColorType::Indexed);
}

#[test]
fn auto_palette_is_never_bigger_than_the_image_as_given_for_small_images() {
    let sizes = [
        (1, 1),
        (2, 1),
        (3, 3),
        (4, 4),
        (8, 8),
        (13, 7),
        (16, 16),
        (32, 32),
        (64, 64),
    ];

    for (width, height) in sizes {
        for colors in [1, 2, 3, 5, 17, 256] {
            let (header, data) = few_colors(width, height, colors);
            let image = ImageRef::new(header, &data);

            let auto = Encoder::with_options(auto_palette()).encode(image).unwrap();
            let as_given = Encoder::new().encode(image).unwrap();

            assert!(
                auto.len() <= as_given.len(),
                "{width}x{height}, {colors} colors: {} > {}",
                auto.len(),
                as_given.len()
            );
            assert_eq!(
                crate::decode_rgba8(&auto).unwrap().data(),
                data,
                "{width}x{height}, {colors} colors"
            );
        }
    }
}

#[test]
fn the_kept_file_is_complete_after_reusing_the_encoder() {
    // Alternates images where each side wins, so a buffer swap gone wrong
    // would show up as a file from the previous image.
    let mut encoder = Encoder::with_options(auto_palette());
    let mut out = Vec::new();

    for (width, height, colors) in [(1, 1, 1), (64, 64, 4), (1, 1, 1), (80, 60, 3), (2, 2, 2)] {
        let (header, data) = few_colors(width, height, colors);

        encoder
            .encode_into(ImageRef::new(header, &data), &mut out)
            .unwrap();

        assert_eq!(
            crate::decode_rgba8(&out).unwrap().data(),
            data,
            "{width}x{height}"
        );
    }
}
