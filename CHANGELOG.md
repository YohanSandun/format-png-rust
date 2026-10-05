# Changelog

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
