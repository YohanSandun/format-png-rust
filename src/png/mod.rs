//! PNG format types shared by the decoder (and, later, the encoder).

pub(crate) mod adam7;
mod chunk;
pub(crate) mod crc;
mod filter;
mod header;
mod image;

pub use chunk::{Chunk, ChunkType};
pub use filter::FilterType;
pub use header::{ColorType, ImageHeader, Interlace};
pub use image::Image;

/// The 8-byte signature every PNG file starts with.
pub const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
