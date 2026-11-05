// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Rendering primitives and software surface baseline.
//!
//! # Coordinate System
//!
//! This module uses the framework's standard **screen coordinate system** with origin at **top-left**:
//!
//! - **X axis**: Increases from left to right (0 → width)
//! - **Y axis**: Increases from top to bottom (0 → height)
//!
//! All rendering operations (drawing text, shapes, images) expect coordinates in this system.
//! The rendering context automatically handles any necessary transformations when working with
//! widgets or other components that may use different coordinate systems internally.
//!
//! ## Drawing Operations
//!
//! - `draw_text()`: Draws text at the specified (x, y) position
//! - `draw_line()`: Draws a line from (x1, y1) to (x2, y2)
//! - `draw_rect()`: Draws a rectangle outline
//! - `fill_rect()`: Fills a rectangle with a solid color
//! - `draw_image()`: Draws an image at the specified position
//!
//! All coordinates are in logical pixels and use the screen coordinate system.
//!
//! # Module Structure (feature-layered)
//!
//! | Group | Path | Contents |
//! |-------|------|---------|
//! | **gpu** | `gpu/` | GPU-accelerated rendering traits and capability enums (gated behind `gpu-wgpu`) |
//! | **core** | `core/` | Data types (`TextMetrics`, `TextCluster`, `ShapedText`) and commands (`RenderCommand`) |
//! | **backend** | `backend/` | Rendering backends: software surface (`BackBuffer`, `SoftwareSurface`, `RenderContext`), paint trait (`PaintBackend`, `SoftwarePaintBackend`), batch image registration (`BatchImage`), scene (`SceneLayer`, `RenderScene`) |
//! | **pipeline** | `pipeline/` | Visual command pipeline for all widget types (controls, containers, dialogs, special, etc.) |
//! | **web** | `web/` | Web engine and web view rendering |
//! | **quality** | `quality/` | Adaptive rendering quality management |
//!
//! # Reachability
//!
//! **State:** Production callers: `src/widget/widget_trait.rs:1`; 181 files reference it (every control's `Draw` implementation).

// ─── Sub-module declarations ─────────────────────────────────────────────────

// Core data types and commands
mod core;
// Reusable scratch byte/word buffers shared by the software and WGPU raster paths (G-004).
pub(crate) mod scratch;
// Rendering backends
mod backend;
// (controls/ directory was migrated to pipeline/special.rs — see pipeline module)
// Visual command pipeline for all widget types
mod pipeline;
// Projection/presentation-mode rendering (BLUE8 P4-5b, gated behind `projection`)
#[cfg(feature = "projection")]
pub mod projection;

// Delivering a finished frame to a window (BLUE29 §7): intent, damage, status and the presenter.
pub mod present;
// GPU-accelerated rendering backend
#[cfg(feature = "gpu-wgpu")]
pub mod gpu;
// SVG rendering backend
pub mod svg;

// Matrix maths and the batch transform stack. Compiled only with the `3d` feature so the
// default 2D build carries none of it.
#[cfg(feature = "3d")]
pub mod math3d;

// Triangle meshes, cameras and the shared projection used by every 3D backend.
#[cfg(feature = "3d")]
pub mod mesh3d;

// Text caching
#[cfg(test)]
mod tests;

// Text layer: glyph sources and the fallback stack (BLUE23 §0A.4 G-2b/G-4b)
pub mod text;

// Text shaping (pre-layout measurement)
pub mod text_shaper;

// Rich text rendering (multi-span styled text)
pub mod rich_text;

// Path geometry (SVG path-data parsing) shared by the vector backends and the icon data.
pub mod path;

// Unicode cluster predicates (combining marks, variation selectors) shared with text shaping.
pub mod grapheme;

// ─── Re-exports ──────────────────────────────────────────────────────────────

// Core
pub use core::{
    BlendMode, CompoundPath, CompoundPathError, LayerEffect, MatteMode, RenderCommand, ShapedText,
    TextCluster, TextDecoration, TextMetrics,
};

// SVG
pub use svg::SvgPaintBackend;

// Backend
//
// `fitted_origin` is the one placement rule a value and its caret must share, so it is re-exported
// at the crate-internal level rather than restated by `line_edit` (restating it is how the two
// drifted apart). It is not part of the public surface.
pub(crate) use backend::fitted_origin;
#[cfg(feature = "quality-management")]
pub use backend::{average_frame_time, current_fps, current_quality_level, set_quality_level};
pub use backend::{
    default_software_render_config, last_auto_render_backend, set_default_software_render_config,
    text_line, AutoRenderBackend, BackBuffer, BatchImage, PaintBackend, RenderContext, RenderScene,
    SceneLayer, SoftwarePaintBackend, SoftwareRenderConfig, SoftwareSurface, VerticalAlignment,
    TEXT_FIT_MARGIN,
};

#[cfg(all(test, feature = "desktop", widgets_unstripped))]
pub(crate) use backend::software_render_config_test_lock;

// Pixel ops
pub use pipeline::{blend_pixel, blend_pixel_with_mode, fill_pixels};

// Text shaping
pub use text_shaper::{ShapedGlyphRun, SimpleTextShaper, TextShaper};

// The face-backed shaper, when a face is enabled (BLUE23 §0A.4, G-1).
#[cfg(feature = "text-shaping")]
pub use text::RustybuzzShaper;

// Rich text
pub use rich_text::{RichText, TextSpan, TextStyle};

// GPU — re-export only when feature is active
#[cfg(feature = "gpu-wgpu")]
pub use gpu::{GpuCapability, GpuRenderer};

// Projection types
#[cfg(feature = "projection")]
pub use projection::{PresentationController, ProjectionLayoutHelper, ProjectionRenderConfig};

// 3D mesh and camera types
#[cfg(feature = "3d")]
pub use mesh3d::{Camera, Mesh3D, Mesh3DError, ProjectedTriangle, ProjectedVertex, Vec3, Vertex3D};

/// The effective (CSS-resolved) and uniform corner-radius helpers, shared with the WGPU fill
// bridge so a per-corner fill uses the same radius arithmetic the reference does.
#[cfg(feature = "gpu-wgpu")]
pub(crate) use pipeline::rounded_rect_effective_radii;

/// Whether a layer is plain — no effect, full opacity, no replay cache — which makes it exactly a
/// clip push/pop pair. Shared with the WGPU bridge so both sides agree which layers are clips.
#[cfg(feature = "gpu-wgpu")]
pub(crate) use core::command::layer_paints_like_a_clip;

/// The reference's own 256-entry ramp builder, shared with the WGPU bridge so a bridged gradient
/// uploads the table the software rasteriser would have sampled.
#[cfg(feature = "gpu-wgpu")]
pub(crate) use pipeline::gradient_ramp_lut;
#[cfg(feature = "gpu-wgpu")]
pub(crate) use pipeline::ramp_lut_for_stops;

/// The dash kernel a dashed polyline expands through, shared with the WGPU bridge so a bridged
/// dashed stroke is the reference's own segment list rather than a second walk of the polyline.
#[cfg(feature = "gpu-wgpu")]
pub(crate) use backend::surface::for_each_dash_segment;

/// Where a box shadow sits, shared with the WGPU bridge so a bridged unblurred shadow covers the
/// same rectangle the reference's command arm fills.
#[cfg(feature = "gpu-wgpu")]
pub(crate) use core::command::box_shadow_spread_rect;

/// The anti-aliased stroke coverage the reference rasteriser samples, shared with the WGPU bridge
/// so a bridged AA line reproduces the reference's own coverage instead of re-deriving it.
#[cfg(feature = "gpu-wgpu")]
pub(crate) use pipeline::line_stroke_coverage_grid;

/// The reference's own bilinear sampler, shared with the GPU command rasteriser so a scaled image's
/// CPU parity path resamples exactly as `draw_image_quad` does.
#[cfg(feature = "gpu-wgpu")]
pub(crate) use pipeline::sample_bilinear;

#[cfg(feature = "gpu-wgpu")]
pub(crate) use backend::surface::BoxBlurScratch;
/// The reference's box-blur kernel and its three widths, shared with the GPU command pipeline so a
/// bridged region blur runs the reference's own passes instead of a lookalike, and with the command
/// rasteriser's CPU parity path.
#[cfg(feature = "gpu-wgpu")]
pub(crate) use backend::{box_blur_region, box_blur_widths};

/// Shared helper accessible to surface.rs and backend
pub(crate) use pipeline::pixel_bytes_len;

/// Advancing/text-shaping helpers shared with the SVG backend, so the vector
/// output and the software rasteriser agree on text metrics (principle #51: one
/// heuristic, not two drifting copies).
/// The glyph geometry both renderers read, and the cluster predicates they classify with.
pub(crate) use pipeline::glyph_rects;

/// Arc and circle drawing helpers.
pub mod arc_helpers;
pub use arc_helpers::{draw_arc_segments, point_on_circle};

// The bevel primitive: a face's light/shade pair and the edges it strokes. Added as its own module
// so a face's "which way is it turned" is one relationship rather than three copies — see the
// module docs for the two expressions it replaces and why the direction has to be explicit here.
pub mod bevel;
pub use bevel::{
    Bevel, BevelDirection, BEVEL_INNER_SHADE_WEIGHT, BEVEL_INNER_WEIGHT, BEVEL_WEIGHT,
};

// The surface declaration channel: what kind of face a control's rectangle is. Added as its own
// module — compiled in every profile, including `mini`/`embedded` — so a theme can say "this is a
// flat theme" or "this is a dimensional one" as data rather than every control deciding its own
// material. See the module docs for the three measurements that motivated it and the role table
// it replaces the fixed per-control shadow literal with.
pub mod surface;
pub use surface::{
    bevel_fits, default_shadow, role_surface_style, BevelSpec, Elevation, ElevationShadow,
    Hairline, Material, SurfaceShadow, SurfaceStyle,
};
