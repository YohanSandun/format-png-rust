# format-png

[![crates.io](https://img.shields.io/crates/v/format-png.svg)](https://crates.io/crates/format-png)
[![docs.rs](https://img.shields.io/docsrs/format-png)](https://docs.rs/format-png)
[![CI](https://github.com/YohanSandun/format-png-rust/actions/workflows/ci.yml/badge.svg)](https://github.com/YohanSandun/format-png-rust/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A PNG decoder and encoder in pure Rust, with no `unsafe` code. It has a simple
API for getting pixels on screen or into a file, and full control when you need
it: reusable decoders and encoders for many images, the image in its own pixel
format, every chunk of the file, and options to make files smaller.

Both directions support every standard color type, bit depth and interlacing
method, with palettes, `tRNS` transparency and metadata. See
[Not supported yet](#not-supported-yet) for what's missing.

## Features

### Decoding

- Every color type (grayscale, RGB, indexed, grayscale + alpha, RGBA) and every
  bit depth the specification allows (1, 2, 4, 8 and 16)
- Adam7 interlacing
- Conversion to 8-bit RGB or RGBA, in the layout a browser canvas's `ImageData` expects
- CRC checks, which can be turned off
- Image data split over any number of `IDAT` chunks
- Reusable `Decoder` that keeps its decompressor and buffers between images
- Raw access to every chunk, including private and unknown ones, while reading
  or kept with the decoded image
- Metadata: `gAMA`, `cHRM`, `sRGB`, `iCCP`, `cICP`, `pHYs`, `tIME`, `eXIf` and
  text (`tEXt`, `zTXt`, `iTXt`), parsed into typed values
- Every chunk read and parsed without decompressing the image data, including
  private, custom and unknown ones
- Decompressed size capped at exactly what the header allows, so a small
  malicious file can't expand without limit

### Encoding

- Every color type, bit depth and Adam7 interlacing
- Adaptive or fixed row filters, and a choice of compression level and strategy
- Optional parallel compression, several times faster for large images
- Palette, `tRNS`, every metadata chunk the decoder reads, and extra chunks:
  raw ones kept from decoding, or your own
- Lossless re-encoding of a decoded image, metadata and chunks included
- Optional conversion of images with few colors to indexed color at the
  smallest bit depth, which is often several times smaller
- Optional stripping of ancillary chunks: all of them, or only those that don't
  change how the image looks
- Reusable `Encoder` that keeps its compressor and buffers between images

## Installation

```toml
[dependencies]
format-png = "0.2"
```

The crate is imported as `format_png`. It needs Rust 1.88 or later.

## Usage

### Decode to RGBA for display

```rust,no_run
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = std::fs::read("image.png")?;
    let bitmap = format_png::decode_rgba8(&data)?;

    println!("{}x{}", bitmap.width(), bitmap.height());
    let pixels: &[u8] = bitmap.data(); // width * height * 4 bytes, no row padding
    Ok(())
}
```

`decode_rgb8` does the same without alpha. An RGBA8 bitmap can go straight into
a browser canvas, for example from a WebAssembly wrapper:

```js
const imageData = new ImageData(new Uint8ClampedArray(pixels), width, height);
context.putImageData(imageData, 0, 0);
```

### Decode many images

A `Decoder` keeps its decompressor tables and working buffers between images.
`decode_bitmap_into` also reuses your output buffer, so decoding a batch
allocates almost nothing after the first image.

```rust,no_run
use format_png::{DecodeOptions, Decoder, PixelFormat};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut decoder = Decoder::with_options(DecodeOptions {
        validate_crc: false,
        ..DecodeOptions::default()
    });
    let mut pixels = Vec::new();

    for path in ["a.png", "b.png", "c.png"] {
        let header = decoder.decode_bitmap_into(&std::fs::read(path)?, PixelFormat::Rgba8, &mut pixels)?;
        println!("{path}: {}x{}", header.width, header.height);
    }
    Ok(())
}
```

### Get the image in its own format

`decode` returns an `Image` whose pixels haven't been converted. That means
16-bit samples (big-endian), packed pixels under 8 bits, and palette indices for
indexed images, with the colors in `Image::palette`. `Decoder::decode_into` does
the same into a buffer you reuse, and `Decoder::palette` then gives the palette.

```rust,no_run
use format_png::ColorType;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let image = format_png::decode(&std::fs::read("image.png")?)?;
    let header = image.header();

    if header.color_type == ColorType::Rgba && header.bit_depth == 16 {
        let first_row: &[u8] = image.row(0); // stride() bytes
        let red = u16::from_be_bytes([first_row[0], first_row[1]]);
        println!("top-left red: {red}");
    }

    // Convert later if you need to:
    let bitmap = image.to_bitmap(format_png::PixelFormat::Rgba8)?;
    Ok(())
}
```

### Read only the header

```rust,no_run
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let header = format_png::read_header(&std::fs::read("image.png")?)?;
    println!("{}x{} {:?}, {} bits", header.width, header.height, header.color_type, header.bit_depth);
    Ok(())
}
```

### Read every chunk without decoding

`read_chunks` reads and parses every chunk but never decompresses the image
data, so it's much faster than decoding. Known chunks come back parsed; every
chunk, including private, custom and unknown ones, critical or not, is also
there raw, in file order.

```rust,no_run
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = std::fs::read("image.png")?;
    let png = format_png::read_chunks(&data)?;

    println!("{}x{}", png.header().width, png.header().height);
    for text in png.metadata().text() {
        println!("{}: {}", text.keyword, text.text);
    }
    for chunk in png.unknown_chunks() {
        println!("{}: {} bytes", chunk.chunk_type(), chunk.data().len());
    }
    Ok(())
}
```

### Read the chunks yourself

A `ChunkReader` returns every chunk as it appears in the file, without copying
its data. Use it for metadata, private chunks, or chunks this crate doesn't handle.

```rust,no_run
use format_png::ChunkReader;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = std::fs::read("image.png")?;
    let mut chunks = ChunkReader::new(&data)?;

    while let Some(chunk) = chunks.next_chunk()? {
        let kind = chunk.chunk_type();
        println!(
            "{kind}: {} bytes, critical: {}, public: {}",
            chunk.data().len(),
            kind.is_critical(),
            kind.is_public(),
        );
    }
    Ok(())
}
```

`Decoder::chunks` does the same, and takes CRC checking from the decoder's options.

### Keep metadata and chunks with the image

Two options keep ancillary chunks while decoding: `preserve_metadata` parses the
known ones into `Image::metadata`, and `preserve_chunks` keeps a raw copy of
every one, including private and unknown chunks, in `Image::ancillary_chunks`.
Both are off by default.

```rust,no_run
use format_png::{DecodeOptions, Decoder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = DecodeOptions { preserve_metadata: true, preserve_chunks: true, ..DecodeOptions::default() };
    let image = Decoder::with_options(options).decode(&std::fs::read("image.png")?)?;

    if let Some(dpi) = image.metadata().physical_dimensions().and_then(|p| p.dots_per_inch()) {
        println!("{dpi:?} dpi");
    }
    for chunk in image.ancillary_chunks() {
        println!("{}: {} bytes", chunk.chunk_type(), chunk.data().len());
    }
    Ok(())
}
```

An invalid or misplaced ancillary chunk is skipped rather than failing the
decode, as the PNG spec allows. Set `strict_ancillary` to get an error instead.

### Encode RGBA pixels

`encode_rgba8` is the reverse of `decode_rgba8`: 8-bit RGBA pixels, rows top to
bottom with no padding, as a canvas's `ImageData` holds them.

```rust,no_run
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (width, height) = (2, 1);
    let pixels = [255, 0, 0, 255, 0, 0, 255, 128]; // red, then half-transparent blue

    std::fs::write("out.png", format_png::encode_rgba8(width, height, &pixels)?)?;
    Ok(())
}
```

### Encode any format, with metadata

`encode` takes an `ImageRef`: a header and pixels in the layout `Image::data`
uses, so any color type and bit depth, plus the chunks to write.

```rust,no_run
use format_png::png::metadata::{PhysicalDimensions, Text, TextKind, Unit};
use format_png::{ColorType, ImageHeader, ImageRef, Interlace, Metadata, Palette};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // A 4x1 indexed image at 2 bits per pixel, using a 4-color palette.
    let header = ImageHeader { width: 4, height: 1, bit_depth: 2, color_type: ColorType::Indexed, interlace: Interlace::None };
    let palette = Palette::from_colors(&[[0, 0, 0], [255, 0, 0], [0, 255, 0], [0, 0, 255]])?;
    let pixels = [0b00_01_10_11]; // indices 0, 1, 2, 3

    let metadata = Metadata::default()
        .with_physical_dimensions(PhysicalDimensions { x: 2835, y: 2835, unit: Unit::Meter }) // 72 dpi
        .with_text(Text {
            keyword: "Title".to_string(),
            text: "Four colors".to_string(),
            language_tag: String::new(),
            translated_keyword: String::new(),
            kind: TextKind::Plain,
        });

    let image = ImageRef::new(header, &pixels).with_palette(&palette).with_metadata(&metadata);
    std::fs::write("out.png", format_png::encode(image)?)?;
    Ok(())
}
```

`ImageRef::from(&image)` re-encodes a decoded `Image` with its palette and
transparency, and also its metadata and chunks if it was decoded with
`preserve_metadata` and `preserve_chunks`.

### Make files smaller

An `Encoder` takes options, and keeps its compressor and buffers between images:

```rust,no_run
use format_png::{EncodeOptions, Encoder, ImageRef, PaletteMode, StripChunks, Threads};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut encoder = Encoder::with_options(EncodeOptions {
        palette: PaletteMode::Auto, // indexed color when it's lossless
        strip: StripChunks::Safe,   // drop text, time, Exif and private chunks
        threads: Threads::Auto,     // compress on every core
        ..EncodeOptions::default()
    });
    let mut out = Vec::new();

    for path in ["a.png", "b.png"] {
        let image = format_png::decode(&std::fs::read(path)?)?;
        encoder.encode_into(ImageRef::from(&image), &mut out)?;
        std::fs::write(path, &out)?;
    }
    Ok(())
}
```

- `PaletteMode::Auto` converts 8-bit RGB and RGBA images with at most 256
  colors to indexed color, without changing a pixel. Small images are encoded
  both ways and the smaller file is kept.
- `StripChunks::Safe` keeps `tRNS` and the chunks that change how the image looks
  (`cICP`, `iCCP`, `sRGB`, `gAMA`, `cHRM`, `pHYs`); `StripChunks::All` keeps only
  `tRNS`.
- `FilterStrategy` picks the row filters: `Adaptive`, the default, chooses one per
  row; `Fixed` uses one filter for every row.

### Encode large images faster

Compression is most of the time spent encoding, and on photos it's slow: a
2500×3800 photo takes several seconds at the default level on one thread.
`Threads::Auto` compresses the image data on every core, in segments of 1 MiB,
for files about 0.1% larger:

| 2500×3800 photo, 20 cores | `Threads::Single` | `Threads::Auto` |
|---|---|---|
| Level 4 | 1.1 s | 0.3 s |
| Level 6 (default) | 9.7 s | 1.2 s |
| Level 9 | 16 s | 2.2 s |

The output depends only on the image and the options, never on the number of
threads, so every machine writes the same file. Where threads aren't available,
as in WebAssembly, the encoder runs on the calling thread.

To spread the work somewhere threads can't reach, such as Web Workers, split
encoding into steps: `Encoder::prepare` does everything but compress the image
data, `compress_segment` compresses one segment anywhere, and
`PreparedPng::finish` joins them into the same file `Threads::Auto` writes:

```rust,no_run
use format_png::{Encoder, ImageRef, compress_segment};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let image = format_png::decode(&std::fs::read("large.png")?)?;

    let prepared = Encoder::new().prepare(ImageRef::from(&image))?;
    // Each of these can run in a different worker, in any order.
    let compressed: Vec<Vec<u8>> = (0..prepared.segment_count())
        .map(|i| compress_segment(prepared.segment(i), prepared.compression()))
        .collect();
    std::fs::write("out.png", prepared.finish(&compressed)?)?;
    Ok(())
}
```

The compression level matters too: on photos, level 4 is about 7 times faster
than level 6 for a file 6% larger, and levels 7 to 9 are much slower for under 1%.

## Conversion rules

`to_bitmap`, `decode_rgba8`, `decode_rgb8` and the `Decoder` bitmap methods convert pixels like this:

| Source | Result |
|---|---|
| 1, 2 or 4-bit samples | Scaled to the full range (×255, ×85, ×17) |
| 16-bit samples | High byte kept |
| Grayscale | Copied to red, green and blue |
| No alpha, converted to RGBA8 | Alpha 255, or from `tRNS` if there is one |
| `tRNS` color (grayscale, RGB) | Pixels with exactly that color get alpha 0, compared at the image's own bit depth |
| `tRNS` alpha (indexed) | Each palette entry's alpha; entries past its end are opaque |
| Alpha or `tRNS`, converted to RGB8 | Alpha dropped, `tRNS` ignored |
| Indexed | The palette color; an index past the end of the palette is `Error::PaletteIndexOutOfRange` |
| Suggested palette in an RGB or RGBA image | Ignored |

Alpha isn't premultiplied, and no gamma or color correction is applied.

## Errors

Every fallible function returns `format_png::Error`, which says exactly what's
wrong: a bad signature, a CRC mismatch, an invalid `IHDR` field, corrupt image
data, pixel data of the wrong size for the encoder, and so on. Malformed input
returns an error; it doesn't panic. `Error` is `#[non_exhaustive]`, as variants
will be added as more of the format is supported.

## Not supported yet

- **Parsing `bKGD`, `sBIT`, `hIST`, `sPLT` and other ancillary chunks.** They're
  kept as raw chunks with `preserve_chunks`, written back with
  `ImageRef::with_chunks`, and available through `read_chunks` and `ChunkReader`.
- **Color management:** gamma, chromaticities, ICC profiles and `cICP` are read
  and written but not applied to the pixels.
- **Animated PNG:** only the default image is decoded and encoded.
- **Lossy encoding:** `PaletteMode::Auto` only converts images that fit a
  palette exactly; there's no quantization.

## Development

```sh
cargo test
```

The tests include fixture PNGs in `tests/data`: valid images covering every
format, and invalid ones with one defect each. They're made by a script that
uses only the Python standard library:

```sh
python tests/data/generate.py
```

The script sets pixel values with a formula, so the tests compute the expected
pixels instead of storing reference images. The valid fixtures were checked
against Pillow.

To time decoding, see [benches/README.md](benches/README.md) in the repository:

```sh
cargo bench --bench decode -- path/to/image.png
```

## License

MIT. See [LICENSE](LICENSE).
