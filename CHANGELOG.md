# Changelog

## 0.2.0

Faster encoding of large images, on threads or anywhere else.

### Breaking

- `EncodeOptions` has a new field, `threads`. Code that lists every field
  without `..EncodeOptions::default()` needs to add it. Nothing else changes:
  with the default, `Threads::Single`, the encoder writes the same bytes as
  0.1.1.

### Added

- `EncodeOptions::threads` and `Threads`: with `Threads::Auto` or
  `Threads::Count`, the image data is compressed on several threads, in 1 MiB
  segments. On a 2500×3800 photo at the default level, encoding takes 1.2 s
  instead of 8 to 10 s on 20 cores, for a file 0.15% larger. The output is the
  same whatever the number of threads, and where threads aren't available, as
  in WebAssembly, everything runs on the calling thread.
- `Encoder::prepare`, `compress_segment` and `PreparedPng::finish` split
  encoding into steps, so the segments can be compressed somewhere threads
  can't reach, such as Web Workers. The result is byte for byte the file
  `Threads::Auto` writes. `Error::InvalidSegmentCount` is returned when
  `finish` gets the wrong number of segments.
- Every public type is now guaranteed `Send` and `Sync`, checked by a test,
  and the crate docs explain using it from threads and async code.

### Docs

- `EncodeOptions::compression` says what each level costs: on photos, level 4
  is about 7 times faster than the default for a file 6% larger, and levels 7
  to 9 are much slower for under 1% smaller.
- The README has a section on encoding large images faster.

## 0.1.1

- Getters and `with_` builder methods are `#[must_use]`, so ignoring their
  result is a warning. This catches calls like `image.with_palette(&palette);`
  that have no effect: `ImageRef` is `Copy`, so the result has to be used.
- Badges for crates.io, docs.rs, CI and the license in the README.
- The published crate no longer includes the `.github` folder.

## 0.1.0

The first release.

### Decoding

- Every color type and bit depth, and Adam7 interlacing.
- Conversion to 8-bit RGB or RGBA, applying `tRNS` transparency.
- The image in its own pixel format, with its palette and transparency.
- Metadata parsed into typed values: `gAMA`, `cHRM`, `sRGB`, `iCCP`, `cICP`,
  `pHYs`, `tIME`, `eXIf`, and text (`tEXt`, `zTXt`, `iTXt`).
- Raw access to every chunk, with `ChunkReader`, and `read_chunks` to read and
  parse every chunk without decompressing the image data.
- Reusable `Decoder`, CRC checks that can be turned off, lenient or strict
  handling of bad ancillary chunks, and a cap on the decompressed size.

### Encoding

- Every color type and bit depth, and Adam7 interlacing.
- Palette, `tRNS`, every metadata chunk the decoder reads, and extra chunks,
  written in the order the spec allows.
- Lossless re-encoding of a decoded image with `ImageRef::from`.
- Adaptive or fixed row filters, and a choice of compression level and strategy.
- `PaletteMode::Auto`: images with at most 256 colors written as indexed color
  at the smallest bit depth, keeping the smaller file for small images.
- `StripChunks`: leaves out ancillary chunks, keeping either only `tRNS` or
  also the chunks that change how the image looks.
- Reusable `Encoder`.
