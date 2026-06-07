//! Software rendering pipeline: container widgets and pixel operations.
//!
//! Sub-modules:
//! - `containers`: `impl SoftwareSurface` block with all software rendering methods
//! - `pixel_ops`: Pixel-level operations (fill_pixels, blend_pixel, set_pixel) and
//!   coverage/geometry helpers for anti-aliased rendering
mod containers;
mod pixel_ops;

pub use pixel_ops::{blend_pixel, fill_pixels};

// Re-export internal helper used by surface.rs
pub(crate) use pixel_ops::pixel_bytes_len;
