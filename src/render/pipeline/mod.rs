// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

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
//!
//! # Scanline fills: three implementations, three semantics
//!
//! Three fills deliberately coexist and must not be merged; each answers a different question
//! (G-204):
//! - `primitives::fill_polygon` — the hard-edged baseline: binary in/out coverage, no winding.
//! - `primitives::fill_path_non_zero_borrowed` — non-zero winding rule for `FillPathNonZero`.
//! - `containers::path_coverage` — a 4× vertically sub-sampled coverage mask for path clipping.
//!
//! New fill needs (a different fill rule, or a different sampling density) must be derived from
//! `path_coverage` by parameterising its sub-sampling, rather than adding a fourth intersection
//! loop.
mod containers;
// The depth-buffered `3d` mesh rasteriser, compiled only with the `3d` feature.
#[cfg(feature = "3d")]
mod mesh;
mod pixel_ops;
mod primitives;

pub use pixel_ops::{blend_pixel, blend_pixel_with_mode, fill_pixels};

// Re-export internal helper used by surface.rs
pub(crate) use pixel_ops::pixel_bytes_len;

// Invalidates cached text measurements when the faces a family resolves to change.
#[cfg(feature = "runtime-fonts")]
pub(crate) use containers::invalidate_text_measurements;

// The one conic rasteriser and its ramp builder, shared by `fill_rect_gradient` and the
// `DrawConicGradient` command arm (G-203).
pub(crate) use containers::{fill_conic_ramp, ramp_lut_for_stops, ConicWrite};

// Bounded coordinate arithmetic shared by the software and SVG backends.
pub(crate) use pixel_ops::{sat_add_i32, sat_mul_i32, sat_neg_i32, u32_to_i32_saturating};

// The glyph geometry both renderers read.
pub(crate) use pixel_ops::glyph_rects;
pub(crate) use pixel_ops::{rounded_rect_effective_radii, uniform_radii};
