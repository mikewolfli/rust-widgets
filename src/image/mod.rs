//! Image processing module — format detection, decoding, encoding, transform, color conversion.
//!
//! Supports all mainstream formats: PNG, JPEG, BMP, GIF, WebP, TIFF, AVIF, ICO, PNM, QOI,
//! Farbfeld, SVG, and compressed SVG (SVGZ).

mod color;
pub mod decoder;
mod encoder;
pub mod exif;
pub mod format;
pub mod image_impl;
pub mod svg_utils;
pub mod transform;

pub use color::*;
pub use decoder::{decode, decode_to_rgba8, detect_format};
pub use encoder::*;
pub use exif::*;
pub use format::{ColorSpace, DecodedImage, ExifData, ImageData, ImageFormat};
pub use svg_utils::*;
pub use transform::*;

pub use image_impl::Image;
