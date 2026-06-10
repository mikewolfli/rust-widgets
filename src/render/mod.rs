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
//! | **backend** | `backend/` | Rendering backends: software surface (`BackBuffer`, `SoftwareSurface`, `RenderContext`), paint trait (`PaintBackend`, `SoftwarePaintBackend`), batch (`BatchId`), scene (`SceneLayer`, `RenderScene`) |
//! | **pipeline** | `pipeline/` | Visual command pipeline for all widget types (controls, containers, dialogs, special, etc.) |
//! | **web** | `web/` | Web engine and web view rendering |
//! | **quality** | `quality/` | Adaptive rendering quality management |
//! | flat | `text_cache.rs` | Text caching utilities |

// ─── Sub-module declarations ─────────────────────────────────────────────────

// Core data types and commands
mod core;
// Rendering backends
mod backend;
// (controls/ directory was migrated to pipeline/special.rs — see pipeline module)
// Visual command pipeline for all widget types
mod pipeline;
// Web rendering
// Types (WebEngine, WebView) are re-exported below for use by the render pipeline.
// The web module no longer has dead_code gating; all types are properly wired.
#[cfg(feature = "desktop")]
pub mod web;
// Projection/presentation-mode rendering (BLUE8 P4-5b, gated behind `projection`)
#[cfg(feature = "projection")]
pub mod projection;
// GPU-accelerated rendering backend
#[cfg(feature = "gpu-wgpu")]
pub mod gpu;
// SVG rendering backend
pub mod svg;

// Adaptive quality
pub mod quality;
// Text caching
#[cfg(test)]
mod tests;
pub mod text_cache;

// Text shaping (pre-layout measurement)
pub mod text_shaper;

// Rich text rendering (multi-span styled text)
pub mod rich_text;

// Text overflow handling (ellipsis, clip, multi-line clamp)
pub mod text_overflow;

// Unicode grapheme cluster support (emoji, combining marks, ZWJ)
pub mod grapheme;

// ─── Re-exports ──────────────────────────────────────────────────────────────

// Core
pub use core::{BlendMode, RenderCommand, ShapedText, TextCluster, TextMetrics};

// SVG
pub use svg::SvgPaintBackend;

// Backend
#[cfg(feature = "quality-management")]
pub use backend::{average_frame_time, current_fps, current_quality_level, set_quality_level};
pub use backend::{
    default_software_render_config, last_auto_render_backend, set_default_software_render_config,
    AutoRenderBackend, BackBuffer, BatchCommand, BatchId, BatchRenderer, PaintBackend,
    RenderContext, RenderScene, SceneLayer, SoftwarePaintBackend, SoftwareRenderConfig,
    SoftwareSurface,
};

#[cfg(all(test, feature = "desktop"))]
pub(crate) use backend::software_render_config_test_lock;

// Pixel ops
pub use pipeline::{blend_pixel, fill_pixels};

// Text shaping
pub use text_shaper::{ShapedGlyphRun, SimpleTextShaper, TextShaper};

// Rich text
pub use rich_text::{RichText, TextSpan, TextStyle};

// Text overflow
pub use text_overflow::{apply_text_clamp, apply_text_overflow, TextClamp, TextOverflow};

// Grapheme support
pub use grapheme::{GraphemeCluster, GraphemeProcessor};

// GPU — re-export only when feature is active
#[cfg(feature = "gpu-wgpu")]
pub use gpu::{GpuCapability, GpuRenderer};

// Projection types
#[cfg(feature = "projection")]
pub use projection::{PresentationController, ProjectionLayoutHelper, ProjectionRenderConfig};

/// Web rendering types — available on desktop targets
#[cfg(feature = "desktop")]
pub use web::engine::WebEngine;
#[cfg(feature = "desktop")]
pub use web::view::WebView;

/// Shared helper accessible to surface.rs and backend
pub(crate) use pipeline::pixel_bytes_len;
