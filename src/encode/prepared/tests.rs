#![cfg(test)]

use std::num::NonZeroUsize;

use crate::encode::parallel::SEGMENT_SIZE;
use crate::error::Error;
use crate::png::metadata::{RenderingIntent, Text, TextKind};
use crate::png::{
    ChunkPosition, ChunkType, ColorType, FilterType, ImageHeader, Interlace, Metadata, OwnedChunk,
};
use crate::{
    CompressionLevel, CompressionStrategy, EncodeOptions, Encoder, FilterStrategy, ImageRef,
    PaletteMode, PreparedPng, StripChunks, Threads, compress_segment,
};

fn header(width: u32, height: u32, color_type: ColorType) -> ImageHeader {
    ImageHeader {
        width,
        height,
        bit_depth: 8,
        color_type,
        interlace: Interlace::None,
    }
}

/// Deterministic pixels that compress a little, like an image.
fn pixels(header: &ImageHeader) -> Vec<u8> {
    (0..header.image_size().unwrap())
        .map(|i| {
            ((i / 7) as u8)
                .wrapping_mul(31)
                .wrapping_add((i % 13) as u8)
        })
        .collect()
}

/// 600x450 RGBA: just over 1 MiB of filtered rows, so two segments.
fn large() -> (ImageHeader, Vec<u8>) {
    let header = header(600, 450, ColorType::Rgba);
    let data = pixels(&header);
    (header, data)
}

fn compress_all(prepared: &PreparedPng) -> Vec<Vec<u8>> {
    (0..prepared.segment_count())
        .map(|i| compress_segment(prepared.segment(i), prepared.compression()))
        .collect()
}

fn via_segments(options: EncodeOptions, image: ImageRef<'_>) -> Vec<u8> {
    let prepared = Encoder::with_options(options).prepare(image).unwrap();
    let compressed = compress_all(&prepared);
    prepared.finish(&compressed).unwrap()
}

fn with_auto_threads(options: EncodeOptions, image: ImageRef<'_>) -> Vec<u8> {
    Encoder::with_options(EncodeOptions {
        threads: Threads::Auto,
        ..options
    })
    .encode(image)
    .unwrap()
}

fn extra(bytes: &[u8; 4], position: ChunkPosition) -> OwnedChunk {
    OwnedChunk::from_data(
        ChunkType::from_bytes(*bytes).unwrap(),
        b"data".to_vec(),
        position,
    )
}

#[test]
fn segments_make_the_same_file_as_auto_threads() {
    let (header, data) = large();
    let mut interlaced = header;
    interlaced.interlace = Interlace::Adam7;
    let metadata = Metadata::default()
        .with_srgb(RenderingIntent::Perceptual)
        .with_text(Text {
            keyword: "Title".to_string(),
            text: "segments".to_string(),
            language_tag: String::new(),
            translated_keyword: String::new(),
            kind: TextKind::Compressed,
        });
    let chunks = [
        extra(b"beFr", ChunkPosition::BeforePalette),
        extra(b"afTr", ChunkPosition::AfterImageData),
    ];
    let d = EncodeOptions::default;
    let cases = [
        ("default", d(), header),
        (
            "level 1",
            EncodeOptions {
                compression: CompressionLevel::new(1),
                ..d()
            },
            header,
        ),
        (
            "level 9",
            EncodeOptions {
                compression: CompressionLevel::BEST,
                ..d()
            },
            header,
        ),
        (
            "fixed blocks",
            EncodeOptions {
                compression_strategy: CompressionStrategy::Fixed,
                ..d()
            },
            header,
        ),
        (
            "stored",
            EncodeOptions {
                compression_strategy: CompressionStrategy::Stored,
                ..d()
            },
            header,
        ),
        (
            "Paeth",
            EncodeOptions {
                filter: FilterStrategy::Fixed(FilterType::Paeth),
                ..d()
            },
            header,
        ),
        (
            "palette auto",
            EncodeOptions {
                palette: PaletteMode::Auto,
                ..d()
            },
            header,
        ),
        (
            "strip all",
            EncodeOptions {
                strip: StripChunks::All,
                ..d()
            },
            header,
        ),
        ("interlaced", d(), interlaced),
    ];

    for (label, options, header) in cases {
        let image = ImageRef::new(header, &data)
            .with_metadata(&metadata)
            .with_chunks(&chunks);

        assert_eq!(
            via_segments(options.clone(), image),
            with_auto_threads(options, image),
            "{label}"
        );
    }
}

#[test]
fn segments_can_be_compressed_in_any_order_and_on_any_thread() {
    let (header, data) = large();
    let prepared = Encoder::new()
        .prepare(ImageRef::new(header, &data))
        .unwrap();

    let compressed: Vec<Vec<u8>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..prepared.segment_count())
            .rev()
            .map(|i| {
                scope.spawn({
                    let prepared = &prepared;
                    move || {
                        (
                            i,
                            compress_segment(prepared.segment(i), prepared.compression()),
                        )
                    }
                })
            })
            .collect();
        let mut results: Vec<(usize, Vec<u8>)> =
            handles.into_iter().map(|h| h.join().unwrap()).collect();
        results.sort_by_key(|(i, _)| *i);
        results.into_iter().map(|(_, segment)| segment).collect()
    });

    let png = prepared.finish(&compressed).unwrap();
    assert_eq!(
        png,
        with_auto_threads(EncodeOptions::default(), ImageRef::new(header, &data))
    );
    assert_eq!(crate::decode(&png).unwrap().data(), data);
}

#[test]
fn segments_are_one_mib_except_the_last() {
    let (header, data) = large();
    let prepared = Encoder::new()
        .prepare(ImageRef::new(header, &data))
        .unwrap();

    // One filter type byte per row.
    let filtered = header.image_size().unwrap() + header.height as usize;
    assert_eq!(prepared.segment_count(), filtered.div_ceil(SEGMENT_SIZE));
    let lengths: Vec<usize> = (0..prepared.segment_count())
        .map(|i| prepared.segment(i).len())
        .collect();
    assert!(
        lengths[..lengths.len() - 1]
            .iter()
            .all(|&length| length == SEGMENT_SIZE)
    );
    assert_eq!(lengths.iter().sum::<usize>(), filtered);
}

#[test]
fn small_images_are_encoded_by_prepare() {
    let header = header(64, 64, ColorType::Rgba);
    let data = pixels(&header);

    let prepared = Encoder::new()
        .prepare(ImageRef::new(header, &data))
        .unwrap();

    assert_eq!(prepared.segment_count(), 0);
    assert_eq!(
        prepared.finish(&[] as &[Vec<u8>]).unwrap(),
        Encoder::new().encode(ImageRef::new(header, &data)).unwrap()
    );
}

#[test]
fn small_images_with_auto_palette_keep_the_smaller_file() {
    // 1x1: the palette costs more than it saves, so the image is kept as given.
    let header = header(1, 1, ColorType::Rgba);
    let options = EncodeOptions {
        palette: PaletteMode::Auto,
        ..EncodeOptions::default()
    };

    let prepared = Encoder::with_options(options.clone())
        .prepare(ImageRef::new(header, &[1, 2, 3, 255]))
        .unwrap();

    let png = prepared.finish(&[] as &[Vec<u8>]).unwrap();
    assert_eq!(
        png,
        Encoder::with_options(options)
            .encode(ImageRef::new(header, &[1, 2, 3, 255]))
            .unwrap()
    );
}

#[test]
fn prepare_ignores_the_threads_option() {
    let (header, data) = large();
    let image = ImageRef::new(header, &data);
    let count = Threads::Count(NonZeroUsize::new(3).unwrap());

    let single = via_segments(EncodeOptions::default(), image);
    let counted = via_segments(
        EncodeOptions {
            threads: count,
            ..EncodeOptions::default()
        },
        image,
    );

    assert_eq!(single, counted);
}

#[test]
fn finish_rejects_the_wrong_number_of_segments() {
    let (header, data) = large();
    let prepared = Encoder::new()
        .prepare(ImageRef::new(header, &data))
        .unwrap();
    let mut compressed = compress_all(&prepared);
    let expected = compressed.len();
    compressed.pop();

    assert_eq!(
        prepared.finish(&compressed),
        Err(Error::InvalidSegmentCount {
            expected,
            actual: expected - 1
        })
    );
}

#[test]
fn finish_rejects_segments_for_an_image_that_has_none() {
    let header = header(4, 4, ColorType::Rgba);
    let prepared = Encoder::new()
        .prepare(ImageRef::new(header, &pixels(&header)))
        .unwrap();

    assert_eq!(
        prepared.finish(&[vec![0u8]]),
        Err(Error::InvalidSegmentCount {
            expected: 0,
            actual: 1
        })
    );
}

#[test]
fn prepare_returns_the_same_errors_as_encode() {
    let header = header(4, 4, ColorType::Rgba);

    assert_eq!(
        Encoder::new()
            .prepare(ImageRef::new(header, &[0; 3]))
            .unwrap_err(),
        Encoder::new()
            .encode(ImageRef::new(header, &[0; 3]))
            .unwrap_err()
    );
}

#[test]
#[should_panic(expected = "segment 2 of 2")]
fn segment_past_the_end_panics() {
    let (header, data) = large();
    let prepared = Encoder::new()
        .prepare(ImageRef::new(header, &data))
        .unwrap();

    let _ = prepared.segment(2);
}

#[test]
fn the_encoder_still_works_after_prepare() {
    let (header, data) = large();
    let mut encoder = Encoder::new();

    let _ = encoder.prepare(ImageRef::new(header, &data)).unwrap();
    let png = encoder.encode(ImageRef::new(header, &data)).unwrap();

    assert_eq!(crate::decode(&png).unwrap().data(), data);
}

#[test]
fn debug_shows_sizes_not_bytes() {
    let (header, data) = large();
    let prepared = Encoder::new()
        .prepare(ImageRef::new(header, &data))
        .unwrap();

    let debug = format!("{prepared:?}");

    assert!(debug.contains("segment_count: 2"), "{debug}");
    assert!(debug.len() < 300, "{debug}");
}
