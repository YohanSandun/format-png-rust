//! Encodes animated PNGs and decodes them back.

use format_png::png::{BlendOp, ChunkType, DisposeOp};
use format_png::{
    AnimationRef, ColorType, DecodeOptions, Decoder, EncodeOptions, Encoder, Error, FrameRef,
    ImageHeader, Interlace, Palette, PaletteAlpha, StripChunks, Threads, Transparency,
};

fn header(width: u32, height: u32, color_type: ColorType, bit_depth: u8) -> ImageHeader {
    ImageHeader {
        width,
        height,
        bit_depth,
        color_type,
        interlace: Interlace::None,
    }
}

/// Pixels for an image with `header`, different for each `seed`. Bits that pad
/// a row to a whole byte are zero, as the decoder returns them.
fn pixels(header: &ImageHeader, seed: u8) -> Vec<u8> {
    let stride = header.stride().unwrap();
    let used_bits = header.width as usize * header.bits_per_pixel() as usize % 8;
    let mut data: Vec<u8> = (0..header.image_size().unwrap())
        .map(|i| (i as u8).wrapping_mul(31).wrapping_add(seed))
        .collect();
    if used_bits != 0 {
        for row in data.chunks_exact_mut(stride) {
            row[stride - 1] &= 0xFF << (8 - used_bits);
        }
    }
    data
}

fn chunk_types(png: &[u8]) -> Vec<String> {
    let mut chunks = format_png::ChunkReader::new(png).unwrap();
    let mut types = Vec::new();
    while let Some(chunk) = chunks.next_chunk().unwrap() {
        types.push(chunk.chunk_type().to_string());
    }
    types
}

#[test]
fn frames_round_trip_in_every_format() {
    let formats = [
        (ColorType::Grayscale, 1),
        (ColorType::Grayscale, 16),
        (ColorType::Rgb, 8),
        (ColorType::Rgb, 16),
        (ColorType::GrayscaleAlpha, 8),
        (ColorType::Rgba, 8),
        (ColorType::Rgba, 16),
    ];

    for (color_type, bit_depth) in formats {
        for interlace in [Interlace::None, Interlace::Adam7] {
            let canvas = ImageHeader {
                interlace,
                ..header(9, 7, color_type, bit_depth)
            };
            let small = ImageHeader {
                width: 4,
                height: 3,
                ..canvas
            };
            let (first, second) = (pixels(&canvas, 1), pixels(&small, 2));
            let frames = [
                FrameRef::new(9, 7, &first),
                FrameRef::new(4, 3, &second).at(5, 4).with_delay(3, 0),
            ];

            let png = format_png::encode_animation(AnimationRef::new(canvas, &frames)).unwrap();
            let animation = format_png::decode_animation(&png).unwrap();

            let context = format!("{color_type:?} {bit_depth} {interlace:?}");
            assert_eq!(animation.header(), &canvas, "{context}");
            assert!(animation.default_image_is_frame(), "{context}");
            assert_eq!(animation.frames().len(), 2, "{context}");
            assert_eq!(animation.frames()[0].image().data(), first, "{context}");
            assert_eq!(animation.frames()[1].image().data(), second, "{context}");
            assert_eq!(animation.frames()[1].image().header(), &small, "{context}");

            let control = animation.frames()[1].control();
            assert_eq!((control.x_offset, control.y_offset), (5, 4), "{context}");
            assert_eq!(control.delay_seconds(), 0.03, "{context}");
        }
    }
}

#[test]
fn chunks_are_laid_out_as_the_spec_says() {
    let canvas = header(2, 2, ColorType::Grayscale, 8);
    let data = [0; 4];
    let frames = [
        FrameRef::new(2, 2, &data),
        FrameRef::new(2, 2, &data),
        FrameRef::new(2, 2, &data),
    ];

    let png = format_png::encode_animation(AnimationRef::new(canvas, &frames)).unwrap();

    assert_eq!(
        chunk_types(&png),
        [
            "IHDR", "acTL", "fcTL", "IDAT", "fcTL", "fdAT", "fcTL", "fdAT", "IEND"
        ]
    );
}

#[test]
fn plain_decoders_see_the_first_frame() {
    let canvas = header(3, 1, ColorType::Rgb, 8);
    let (first, second) = (pixels(&canvas, 10), pixels(&canvas, 20));
    let frames = [FrameRef::new(3, 1, &first), FrameRef::new(3, 1, &second)];

    let png = format_png::encode_animation(AnimationRef::new(canvas, &frames)).unwrap();

    assert_eq!(format_png::decode(&png).unwrap().data(), first);
}

#[test]
fn separate_default_image_is_not_a_frame() {
    let canvas = header(2, 1, ColorType::Grayscale, 8);
    let (default, frame) = ([7, 7], [1]);
    let frames = [FrameRef::new(1, 1, &frame).at(1, 0)];

    let png = format_png::encode_animation(
        AnimationRef::new(canvas, &frames).with_default_image(&default),
    )
    .unwrap();

    assert_eq!(
        chunk_types(&png),
        ["IHDR", "acTL", "IDAT", "fcTL", "fdAT", "IEND"]
    );
    assert_eq!(format_png::decode(&png).unwrap().data(), default);

    let animation = format_png::decode_animation(&png).unwrap();
    assert!(!animation.default_image_is_frame());
    assert_eq!(animation.frames().len(), 1);
    assert_eq!(animation.frames()[0].image().data(), frame);
}

#[test]
fn num_plays_round_trips() {
    let canvas = header(1, 1, ColorType::Grayscale, 8);
    let frames = [FrameRef::new(1, 1, &[0])];

    let png =
        format_png::encode_animation(AnimationRef::new(canvas, &frames).with_num_plays(4)).unwrap();
    let animation = format_png::decode_animation(&png).unwrap();

    assert_eq!(animation.num_plays(), 4);
    assert_eq!(animation.control().num_frames, 1);
}

#[test]
fn indexed_frames_share_palette_and_transparency() {
    let canvas = header(2, 1, ColorType::Indexed, 8);
    let palette = Palette::from_colors(&[[255, 0, 0], [0, 0, 255]]).unwrap();
    let transparency = Transparency::Palette(PaletteAlpha::from_values(&[255, 0]).unwrap());
    let (first, second) = ([0, 1], [0]);
    let frames = [
        FrameRef::new(2, 1, &first),
        FrameRef::new(1, 1, &second)
            .at(1, 0)
            .with_blend_op(BlendOp::Over),
    ];

    let png = format_png::encode_animation(
        AnimationRef::new(canvas, &frames)
            .with_palette(&palette)
            .with_transparency(&transparency),
    )
    .unwrap();
    let animation = format_png::decode_animation(&png).unwrap();

    assert_eq!(animation.palette(), Some(&palette));
    assert_eq!(animation.frames()[1].image().palette(), Some(&palette));

    let bitmaps = animation.to_bitmaps().unwrap();
    assert_eq!(bitmaps[0].data(), [255, 0, 0, 255, 0, 0, 255, 0]);
    // The second frame draws opaque red over the transparent pixel.
    assert_eq!(bitmaps[1].data(), [255, 0, 0, 255, 255, 0, 0, 255]);
}

#[test]
fn to_bitmaps_applies_disposal() {
    let canvas = header(2, 1, ColorType::Rgba, 8);
    let background = [10, 10, 10, 255, 20, 20, 20, 255];
    let blue = [0, 0, 255, 255];
    let red = [255, 0, 0, 255];
    let frames = [
        FrameRef::new(2, 1, &background),
        FrameRef::new(1, 1, &blue).with_dispose_op(DisposeOp::Previous),
        FrameRef::new(1, 1, &red)
            .at(1, 0)
            .with_dispose_op(DisposeOp::Background),
        FrameRef::new(1, 1, &blue),
    ];

    let png = format_png::encode_animation(AnimationRef::new(canvas, &frames)).unwrap();
    let bitmaps = format_png::decode_animation(&png)
        .unwrap()
        .to_bitmaps()
        .unwrap();

    let data: Vec<_> = bitmaps.iter().map(|b| b.data().to_vec()).collect();
    assert_eq!(data[0], background);
    assert_eq!(data[1], [0, 0, 255, 255, 20, 20, 20, 255]);
    // The blue frame is undone before red is drawn.
    assert_eq!(data[2], [10, 10, 10, 255, 255, 0, 0, 255]);
    // The red frame is cleared before blue is drawn.
    assert_eq!(data[3], [0, 0, 255, 255, 0, 0, 0, 0]);
}

#[test]
fn large_frames_compress_on_threads() {
    let canvas = header(700, 500, ColorType::Rgba, 8);
    let (first, second) = (pixels(&canvas, 3), pixels(&canvas, 4));
    let frames = [
        FrameRef::new(700, 500, &first),
        FrameRef::new(700, 500, &second),
    ];
    let mut encoder = Encoder::with_options(EncodeOptions {
        threads: Threads::Count(std::num::NonZeroUsize::new(4).unwrap()),
        ..EncodeOptions::default()
    });

    let png = encoder
        .encode_animation(AnimationRef::new(canvas, &frames))
        .unwrap();
    let animation = format_png::decode_animation(&png).unwrap();

    assert_eq!(animation.frames()[0].image().data(), first);
    assert_eq!(animation.frames()[1].image().data(), second);
}

#[test]
fn decoder_keeps_metadata_with_the_animation() {
    let canvas = header(1, 1, ColorType::Grayscale, 8);
    let frames = [FrameRef::new(1, 1, &[0])];
    let metadata = format_png::Metadata::default()
        .with_gamma(format_png::png::metadata::Gamma::new(45455).unwrap());

    let png =
        format_png::encode_animation(AnimationRef::new(canvas, &frames).with_metadata(&metadata))
            .unwrap();
    let mut decoder = Decoder::with_options(DecodeOptions {
        preserve_metadata: true,
        ..DecodeOptions::default()
    });
    let animation = decoder.decode_animation(&png).unwrap();

    assert_eq!(animation.metadata().gamma(), metadata.gamma());
    assert!(animation.frames()[0].image().metadata().is_empty());
}

#[test]
fn strip_all_keeps_the_animation() {
    let canvas = header(1, 1, ColorType::Grayscale, 8);
    let frames = [FrameRef::new(1, 1, &[0]), FrameRef::new(1, 1, &[1])];
    let metadata = format_png::Metadata::default()
        .with_gamma(format_png::png::metadata::Gamma::new(45455).unwrap());
    let mut encoder = Encoder::with_options(EncodeOptions {
        strip: StripChunks::All,
        ..EncodeOptions::default()
    });

    let png = encoder
        .encode_animation(AnimationRef::new(canvas, &frames).with_metadata(&metadata))
        .unwrap();

    assert_eq!(
        chunk_types(&png),
        ["IHDR", "acTL", "fcTL", "IDAT", "fcTL", "fdAT", "IEND"]
    );
}

#[test]
fn reencoding_a_single_image_drops_animation_chunks() {
    let canvas = header(1, 1, ColorType::Grayscale, 8);
    let frames = [FrameRef::new(1, 1, &[0]), FrameRef::new(1, 1, &[1])];
    let png = format_png::encode_animation(AnimationRef::new(canvas, &frames)).unwrap();

    let image = Decoder::with_options(DecodeOptions {
        preserve_chunks: true,
        ..DecodeOptions::default()
    })
    .decode(&png)
    .unwrap();
    let mut encoder = Encoder::with_options(EncodeOptions {
        keep_unsafe_chunks: true,
        ..EncodeOptions::default()
    });
    let reencoded = encoder.encode((&image).into()).unwrap();

    assert_eq!(chunk_types(&reencoded), ["IHDR", "IDAT", "IEND"]);
}

#[test]
fn static_png_is_not_animated() {
    let png = format_png::encode_rgba8(1, 1, &[0, 0, 0, 255]).unwrap();

    assert_eq!(
        format_png::decode_animation(&png).map(|_| ()),
        Err(Error::NotAnimated)
    );
}

#[test]
fn encoder_rejects_invalid_frames() {
    let canvas = header(2, 2, ColorType::Grayscale, 8);
    let encode = |frames: &[FrameRef<'_>]| {
        format_png::encode_animation(AnimationRef::new(canvas, frames)).map(|_| ())
    };
    let (full, one) = ([0; 4], [0; 1]);

    assert_eq!(
        encode(&[]),
        Err(Error::FrameCountMismatch {
            expected: 1,
            actual: 0
        })
    );
    assert_eq!(
        encode(&[FrameRef::new(1, 1, &one)]),
        Err(Error::InvalidChunkData(ChunkType::FCTL))
    );
    assert_eq!(
        encode(&[
            FrameRef::new(2, 2, &full),
            FrameRef::new(1, 1, &one).at(2, 0)
        ]),
        Err(Error::FrameOutOfBounds)
    );
    assert_eq!(
        encode(&[FrameRef::new(2, 2, &full), FrameRef::new(1, 1, &full)]),
        Err(Error::InvalidImageDataLength {
            expected: 1,
            actual: 4
        })
    );
    assert_eq!(
        encode(&[FrameRef::new(2, 2, &full), FrameRef::new(0, 1, &[])]),
        Err(Error::InvalidDimensions {
            width: 0,
            height: 1
        })
    );
}
