# format-png

A PNG decoder in Rust, with no `unsafe` code. It has a simple API for getting
pixels on screen, and full control when you need it: reusable decoders and
buffers for decoding many images, the image in its own pixel format, and every
chunk of the file.

> **Status:** early development. Decoding works for every standard color type,
> bit depth and interlacing method, including palettes and `tRNS` transparency.
> Encoding isn't implemented yet. See [Not supported yet](#not-supported-yet).

## Features

- Every color type (grayscale, RGB, indexed, grayscale + alpha, RGBA) and every
  bit depth the specification allows (1, 2, 4, 8 and 16)
- Adam7 interlacing
- Conversion to 8-bit RGB or RGBA, in the layout a browser canvas's `ImageData` expects
- CRC checks, which can be turned off
- Image data split over any number of `IDAT` chunks
- Reusable `Decoder` that keeps its decompressor and buffers between images
- Raw access to every chunk, including private and unknown ones, while reading
  or kept with the decoded image
- Metadata: `gAMA`, `cHRM`, `sRGB`, `pHYs` and `tIME`, parsed into typed values
- Decompressed size capped at exactly what the header allows, so a small
  malicious file can't expand without limit

## Installation

The crate isn't on crates.io yet. Add it from Git:

```toml
[dependencies]
format-png = { git = "https://github.com/YohanSandun/format-png-rust" }
```

The crate is imported as `format_png`.

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
data, and so on. Malformed input returns an error; it doesn't panic. `Error` is
`#[non_exhaustive]`, as variants will be added as more of the format is supported.

## Not supported yet

- **Parsing text (`tEXt`, `zTXt`, `iTXt`), `iCCP`, `bKGD`, `sBIT`, `eXIf` and other
  ancillary chunks.** They're kept as raw chunks with `preserve_chunks`, and
  available through `ChunkReader`.
- **Color management:** gamma, chromaticities and ICC profiles are read but
  not applied.
- **Encoding.**

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

To time decoding, see [benches/README.md](benches/README.md):

```sh
cargo bench --bench decode -- path/to/image.png
```

## License

MIT. See [LICENSE](LICENSE).
