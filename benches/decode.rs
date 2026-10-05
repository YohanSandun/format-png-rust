//! Decoding benchmark. See benches/README.md.
//!
//! Run with `cargo bench --bench decode -- [FILE.png ...]`. Without files, it
//! decodes every PNG in benches/data/, or a small fixture if that's empty.

use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use format_png::png::ChunkType;
use format_png::{ChunkReader, DecodeOptions, Decoder, PixelFormat};
use rust_deflate::Decompressor;

const DEFAULT_RUNS: usize = 7;
const FALLBACK: &str = "tests/data/valid/large_rgba_8.png";

fn main() {
    let runs = std::env::var("BENCH_RUNS")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|&runs| runs > 0)
        .unwrap_or(DEFAULT_RUNS);

    for path in files() {
        bench_file(&path, runs);
        println!();
    }
}

/// Files from the command line, then benches/data/*.png, then a small fixture.
/// Flags such as the `--bench` that `cargo bench` passes are ignored.
fn files() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let from_args: Vec<PathBuf> = std::env::args()
        .skip(1)
        .filter(|arg| !arg.starts_with('-'))
        .map(PathBuf::from)
        .collect();
    if !from_args.is_empty() {
        return from_args;
    }

    let mut from_dir: Vec<PathBuf> = std::fs::read_dir(root.join("benches/data"))
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
        })
        .collect();
    from_dir.sort();
    if !from_dir.is_empty() {
        return from_dir;
    }

    println!("No files given and benches/data/ has no PNGs; using {FALLBACK}.\n");
    vec![root.join(FALLBACK)]
}

fn bench_file(path: &Path, runs: usize) {
    let data = std::fs::read(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let header = format_png::read_header(&data)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let image_size = header.image_size().expect("image fits in memory");
    let rgba_size = header.width as usize * header.height as usize * 4;

    println!(
        "{}: {}x{} {:?} {}-bit, {:?} interlace, {:.1} MB file, {:.1} MB decoded, {runs} runs",
        path.display(),
        header.width,
        header.height,
        header.color_type,
        header.bit_depth,
        header.interlace,
        data.len() as f64 / 1e6,
        image_size as f64 / 1e6,
    );

    // The joined IDAT stream, to time decompression on its own.
    let mut compressed = Vec::new();
    let mut chunks = ChunkReader::new(&data).unwrap();
    while let Some(chunk) = chunks.next_chunk().unwrap() {
        if chunk.chunk_type() == ChunkType::IDAT {
            compressed.extend_from_slice(chunk.data());
        }
    }

    let bench = |name: &str, bytes_out: usize, f: &mut dyn FnMut()| {
        report(name, bytes_out, measure(runs, f))
    };

    bench("read_header", 0, &mut || {
        black_box(format_png::read_header(black_box(&data)).unwrap());
    });

    bench("walk all chunks, checking CRCs", 0, &mut || {
        let mut chunks = ChunkReader::new(black_box(&data)).unwrap();
        while let Some(chunk) = chunks.next_chunk().unwrap() {
            black_box(chunk);
        }
    });

    // Each row's buffers are dropped at the end of its block, so peak memory is
    // about three times the decoded size rather than the sum over all rows.
    {
        let mut decompressor = Decompressor::new();
        let mut scanlines = Vec::new();
        decompressor
            .decompress_zlib_into(&compressed, &mut scanlines)
            .unwrap();
        let scanline_size = scanlines.len();
        bench("zlib decompression only", scanline_size, &mut || {
            scanlines.clear();
            decompressor
                .decompress_zlib_into(black_box(&compressed), &mut scanlines)
                .unwrap();
            black_box(&scanlines);
        });
    }

    {
        let mut decoder = Decoder::new();
        let mut pixels = Vec::new();
        bench("Decoder::decode_into (reused)", image_size, &mut || {
            decoder.decode_into(black_box(&data), &mut pixels).unwrap();
            black_box(&pixels);
        });
    }

    {
        let no_crc = DecodeOptions {
            validate_crc: false,
            ..DecodeOptions::default()
        };
        let mut decoder = Decoder::with_options(no_crc);
        let mut pixels = Vec::new();
        bench(
            "Decoder::decode_into (reused, no CRC)",
            image_size,
            &mut || {
                decoder.decode_into(black_box(&data), &mut pixels).unwrap();
                black_box(&pixels);
            },
        );
    }

    bench(
        "format_png::decode (new each time)",
        image_size,
        &mut || {
            black_box(format_png::decode(black_box(&data)).unwrap());
        },
    );

    {
        let mut decoder = Decoder::new();
        let mut rgba = Vec::new();
        bench(
            "Decoder::decode_bitmap_into Rgba8 (reused)",
            rgba_size,
            &mut || {
                decoder
                    .decode_bitmap_into(black_box(&data), PixelFormat::Rgba8, &mut rgba)
                    .unwrap();
                black_box(&rgba);
            },
        );
    }

    bench(
        "format_png::decode_rgba8 (new each time)",
        rgba_size,
        &mut || {
            black_box(format_png::decode_rgba8(black_box(&data)).unwrap());
        },
    );

    let image = format_png::decode(&data).unwrap();
    bench(
        "Image::to_bitmap Rgba8 (conversion only)",
        rgba_size,
        &mut || {
            black_box(image.to_bitmap(PixelFormat::Rgba8).unwrap());
        },
    );
}

/// One warm-up run, then `runs` timed runs, sorted.
fn measure(runs: usize, f: &mut dyn FnMut()) -> Vec<Duration> {
    f();
    let mut times: Vec<Duration> = (0..runs)
        .map(|_| {
            let start = Instant::now();
            f();
            start.elapsed()
        })
        .collect();
    times.sort();
    times
}

fn report(name: &str, bytes_out: usize, times: Vec<Duration>) {
    let ms = |time: Duration| time.as_secs_f64() * 1e3;
    let median = times[times.len() / 2];
    let throughput = if bytes_out > 0 {
        format!(
            "{:>7.0} MB/s",
            bytes_out as f64 / median.as_secs_f64() / 1e6
        )
    } else {
        String::new()
    };
    println!(
        "  {name:<44} min {:>9.2} ms   median {:>9.2} ms   {throughput}",
        ms(times[0]),
        ms(median)
    );
}
