//! PNG format types shared by the decoder (and, later, the encoder).

pub(crate) mod adam7;
mod bitmap;
mod chunk;
pub(crate) mod crc;
mod filter;
mod header;
mod image;
pub mod metadata;
mod owned_chunk;
mod png_chunks;
pub(crate) mod palette;
mod transparency;

pub use bitmap::{Bitmap, PixelFormat};
pub use chunk::{Chunk, ChunkType};
pub use filter::FilterType;
pub use header::{ColorType, ImageHeader, Interlace};
pub(crate) use image::ImageChunks;
pub use image::Image;
pub use metadata::Metadata;
pub use owned_chunk::{ChunkPosition, OwnedChunk};
pub use png_chunks::PngChunks;
pub use palette::Palette;
pub use transparency::{PaletteAlpha, Transparency};

/// The 8-byte signature every PNG file starts with.
pub const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
