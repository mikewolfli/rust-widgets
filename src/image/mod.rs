//! Image processing module — format detection, decoding, encoding, transform, color conversion.
//!
//! # Format support matrix (truthful)
//!
//! Decoding (`decoder::decode`) is implemented for: **PNG** (real scanline
//! filter reconstruction; interlaced PNG is rejected), **JPEG** (baseline),
//! **BMP** (24/32-bit), **QOI**, **Farbfeld**, and binary **PNM** (P5/P6).
//!
//! Decoding **GIF**, **WebP**, **TIFF**, **AVIF**, **ICO**, **SVG** and
//! **SVGZ** returns `Err("... not implemented ...")` — these formats are
//! detected by magic bytes but have no codec here. They never yield
//! fabricated/placeholder pixels.
//!
//! Encoding is implemented for PNG, JPEG, BMP, GIF, TIFF, QOI, Farbfeld,
//! PNM and SVG (SVG embeds a base64 PNG). WebP/AVIF/ICO encoding returns
//! `Err`.

mod color;
pub mod decoder;
mod encoder;
pub mod exif;
pub mod format;
pub mod image_impl;
pub mod transform;

pub use color::*;
pub use decoder::{decode, decode_to_rgba8, detect_format};
pub use encoder::*;
pub use exif::*;
pub use format::{ColorSpace, DecodedImage, ExifData, ImageData, ImageFormat};
pub use transform::*;

pub use image_impl::Image;
