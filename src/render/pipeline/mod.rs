//! Software rendering pipeline: container widgets, rendering primitives,
//! and pixel operations.
//!
//! Sub-modules:
//! - `containers`: `impl SoftwareSurface` block with lifecycle, config, text shaping,
//!   gradient fill, and clip stack methods
//! - `primitives`: `impl SoftwareSurface` block with all rendering primitives
//!   (rect, circle, line, text, image, etc.)
//! - `pixel_ops`: Pixel-level operations (fill_pixels, blend_pixel, set_pixel) and
//!   coverage/geometry helpers for anti-aliased rendering
mod containers;
mod pixel_ops;
mod primitives;

pub use pixel_ops::{blend_pixel, fill_pixels};

// Re-export internal helper used by surface.rs
pub(crate) use pixel_ops::pixel_bytes_len;
