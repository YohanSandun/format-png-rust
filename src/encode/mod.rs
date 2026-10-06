//! Encodes images to PNG.
//!
//! The pipeline mirrors decoding in reverse:
//!
//! 1. With [`StripChunks::Safe`] or [`StripChunks::All`], `strip` drops the
//!    chunks the image doesn't keep. `encoder::validate` then checks the
//!    [`ImageRef`]: a valid header, pixel data
//!    of the right length, and a palette where one is needed. With
//!    [`PaletteMode::Auto`], `palettize::palettize` then converts it to indexed
//!    color if it has few enough colors.
//! 2. `filter::filter_image` adds a filter type byte to each row and filters it,
//!    as [`FilterStrategy`] says. Interlaced images are first split into their
//!    seven Adam7 passes by `interlace::interlace_pass`, and each is filtered on its own.
//! 3. The filtered rows are zlib-compressed into one stream, in parallel
//!    segments with [`Threads::Auto`] or [`Threads::Count`]; see `parallel`.
//! 4. `chunk_writer` writes the signature, `IHDR`, `PLTE` if there is a palette,
//!    `tRNS` if there is transparency, the stream as one or more `IDAT` chunks,
//!    and `IEND`. `metadata` writes the metadata chunks around `PLTE`, in the
//!    order its docs give, and `extra_chunks` writes raw chunks at their
//!    positions, by the rules its docs give.
//!
//! To support a new chunk: add a field and a `with_` method to [`ImageRef`], and
//! write it in `Encoder::encode_into` at the position the spec gives it.

mod chunk_writer;
mod encoder;
mod extra_chunks;
mod filter;
mod image_ref;
mod interlace;
mod metadata;
mod options;
mod palettize;
mod parallel;
mod prepared;
mod strip;

pub use encoder::Encoder;
pub use image_ref::ImageRef;
pub use options::{EncodeOptions, FilterStrategy, PaletteMode, StripChunks, Threads};
pub use parallel::{SegmentCompression, compress_segment};
pub use prepared::PreparedPng;
