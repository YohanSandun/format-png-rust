//! PNG format types shared by the decoder and the encoder.

pub(crate) mod adam7;
mod animated_image;
pub(crate) mod animation;
mod bitmap;
mod chunk;
mod compositor;
pub(crate) mod crc;
mod filter;
mod header;
mod image;
pub mod metadata;
mod owned_chunk;
pub(crate) mod palette;
mod png_chunks;
mod transparency;

pub use animated_image::{AnimatedImage, Frame};
pub use animation::{AnimationControl, BlendOp, DisposeOp, FrameControl};
pub use bitmap::{Bitmap, PixelFormat};
pub use chunk::{Chunk, ChunkType};
pub use compositor::Compositor;
pub use filter::FilterType;
pub use header::{ColorType, ImageHeader, Interlace};
pub use image::Image;
pub(crate) use image::ImageChunks;
pub use metadata::Metadata;
pub use owned_chunk::{ChunkPosition, OwnedChunk};
pub use palette::Palette;
pub use png_chunks::PngChunks;
pub use transparency::{PaletteAlpha, Transparency};

/// The 8-byte signature every PNG file starts with.
pub const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
