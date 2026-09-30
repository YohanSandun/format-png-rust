# Benchmarks

`decode.rs` times each stage of decoding a PNG, from reading the header to
converting it to RGBA. It's a plain program rather than a benchmark framework, so
it needs no extra dependencies.

## Running

```sh
# Every PNG in benches/data/
cargo bench --bench decode

# Specific files
cargo bench --bench decode -- path/to/image.png other.png

# More runs per measurement (default 7)
BENCH_RUNS=20 cargo bench --bench decode          # bash
$env:BENCH_RUNS=20; cargo bench --bench decode    # PowerShell
```

If you give no files and `benches/data/` has no PNGs, it uses a small test
fixture so that `cargo bench` always runs.

`cargo bench` builds with the `bench` profile, which is the same as `release`.
Each measurement has one warm-up run, then the timed runs. The minimum and median
are reported. The median is the number to compare, because the minimum is
sensitive to luck.

## Benchmark images

Put large images in `benches/data/`. Test fixtures in `tests/data/` are too small
to time well, and `tests/data/valid/` must only hold images made by
`tests/data/generate.py`: the tests check its pixel values, and the script
deletes everything else in that folder.

PNGs in `benches/data/` are ignored by git. To make a large one, run
`generate.py` (needs numpy):

```sh
# 32768×32768 RGBA 8-bit: 1 gigapixel, 4 GiB decoded, ~340 MB file, ~80 s
python benches/generate.py

# Any size, and optionally another output path
python benches/generate.py 10000 9000 benches/data/medium.png
```

It writes the image a few rows at a time, so it needs little memory. Rows cycle
through all five filter types, and the pixels are gradients with noise, so the
file compresses about 12:1 rather than to almost nothing.

PNG allows up to 2³¹−1 pixels on each side, but the real limit is memory. The
benchmark needs about three times the decoded size (for example the decoder's
scanline buffer, its pixels and the RGBA output), so the default 4 GiB image
needs about 12 GiB free. With less free memory, rows get slow and noisy as
Windows starts paging: compare the minimum and the median.

## What each row measures

| Row | What's timed |
|---|---|
| `read_header` | Signature and `IHDR` only |
| `walk all chunks, checking CRCs` | `ChunkReader` over the whole file, computing every CRC |
| `zlib decompression only` | `rust-deflate` on the joined `IDAT` data, with a reused buffer |
| `Decoder::decode_into (reused)` | The full decode to native pixels, reusing the decoder and output buffer |
| `… (reused, no CRC)` | The same with `validate_crc: false` |
| `format_png::decode (new each time)` | The simple API: a new decoder and fresh allocations each call |
| `Decoder::decode_bitmap_into Rgba8 (reused)` | Decode and convert to RGBA, reusing all buffers |
| `format_png::decode_rgba8 (new each time)` | The simple RGBA API |
| `Image::to_bitmap Rgba8 (conversion only)` | Only the conversion of already-decoded pixels |

`MB/s` is decoded output per second, measured at the median. The three
`decode_rgba8` rows are skipped for indexed images, which can't be converted
until `PLTE` is supported.

Differences between rows show where the time goes:

- `decode_into` minus `zlib decompression only` is roughly the cost of unfiltering
  and deinterlacing.
- `new each time` minus `reused` is the cost of allocating and zeroing fresh
  buffers.
- `to_bitmap` alone is the conversion cost.

## Comparing with C decoders

`reference.py` times the same files with C zlib (through Python) and Pillow. It
also prints which filter types the file uses, which affects unfiltering speed. It
needs Pillow (`pip install pillow`).

```sh
python benches/reference.py benches/data/huge.png
```

Python's `zlib.decompress` grows its output buffer as it goes, while the Rust
benchmark decompresses into a reused buffer. Treat the comparison as a rough
guide, not an exact match.

## Results

A 10000×9000 RGBA 8-bit image (10.6 MB file, 360 MB decoded, every row using the
Sub filter) on an i7-1370P, measured on 2026-09-29:

| Row | Median |
|---|---:|
| `zlib decompression only` | 251 ms |
| `Decoder::decode_into (reused)` | 505 ms |
| `format_png::decode (new each time)` | 721 ms |
| `format_png::decode_rgba8 (new each time)` | 1,217 ms |
| `Image::to_bitmap Rgba8 (conversion only)` | 529 ms |
| C zlib, decompression only | 380 ms |
| Pillow, full decode | 304 ms |

### Conversion fast paths, 2026-09-30

Medians before and after adding a specialised loop for each 8 and 16-bit
conversion, and skipping conversion when the pixels are already 8-bit RGB or
RGBA. The RGB and 16-bit grayscale images are `huge.png` converted with Pillow.

| Image | Row | Before | After |
|---|---|---:|---:|
| `huge.png` (RGBA 8-bit) | `Image::to_bitmap Rgba8` | 531 ms | 72 ms |
| | `Decoder::decode_bitmap_into Rgba8` | 932 ms | 452 ms |
| | `format_png::decode_rgba8` | 1,172 ms | 585 ms |
| RGB 8-bit | `Image::to_bitmap Rgba8` | 657 ms | 121 ms |
| | `format_png::decode_rgba8` | 1,143 ms | 761 ms |
| Grayscale 16-bit | `Image::to_bitmap Rgba8` | 657 ms | 109 ms |
| | `format_png::decode_rgba8` | 919 ms | 307 ms |

### Unfiltering, 2026-09-30

Unfiltering rewritten to work on whole pixels with a loop for each pixel size,
to read straight from the decompressed data instead of copying it first, and to
use stb_image's Paeth predictor. `Decoder::decode_into (reused)` minimums, with
the old and new code run alternately. The machine was noisy that day, so treat
these as rough. `mixed.png` is `python benches/generate.py 10000 9000`, which
uses all five filters.

| Image | Before | After |
|---|---:|---:|
| `huge.png` (RGBA, all Sub) | 449 ms | 389 ms |
| `mixed.png` (RGBA, all filters) | 628 ms | 605 ms |
| RGB 8-bit | 363 ms | 321 ms |

Timed on their own, per 90 MB of RGBA: Sub 23 ms, Up 22 ms, Average 41 ms and
Paeth 87 ms. The spec's form of the Paeth predictor took 113 ms in the same loop.

The default `generate.py` image (32768×32768, 338 MB file, 4 GiB decoded, all
five filters), same machine and date, 3 runs:

| Row | Min | Median |
|---|---:|---:|
| `zlib decompression only` | 4,147 ms | 4,306 ms |
| `Decoder::decode_into (reused)` | 7,520 ms | 8,066 ms |
| `format_png::decode_rgba8 (new each time)` | 17,390 ms | 18,366 ms |
| `Image::to_bitmap Rgba8 (conversion only)` | 6,249 ms | 14,664 ms |

With 30 GB free, some RGBA rows had medians well above their minimums, most likely
because of paging.

Add new results here, with the date, when you change something that affects speed.
