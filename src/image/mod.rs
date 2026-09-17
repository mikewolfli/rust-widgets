// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Image processing module — format detection, decoding, encoding, transform, color conversion.
//!
//! # Format support matrix (truthful)
//!
//! Decoding (`decoder::decode`) is implemented for PNG, JPEG, BMP, QOI,
//! Farbfeld, PNM (P1-P6), GIF, WebP, TIFF, AVIF, ICO, SVG and SVGZ.
//! GIF and animated WebP use the existing single-frame API and return the
//! first frame; use a future animation API when frame timing is required.
//!
//! Encoding is implemented for PNG, JPEG, BMP, GIF, TIFF, QOI, Farbfeld,
//! PNM and SVG (SVG embeds a base64 PNG). WebP/AVIF/ICO encoding returns
//! `Err`.
//!
//! # Reachability
//!
//! **State:** Production callers: `src/widget/display_widgets/image_view.rs:7` (`use crate::image::{Image, ImageFormat}`).

mod color;
pub mod decoder;
mod encoder;
pub mod exif;
pub mod format;
pub mod image_impl;
pub mod transform;

pub use color::*;
pub use decoder::{decode, decode_animation, decode_to_rgba8, detect_format};
pub use encoder::*;
pub use exif::*;
pub use format::{ColorSpace, DecodedAnimation, DecodedImage, ExifData, ImageData, ImageFormat};
pub use transform::*;

pub use image_impl::Image;
