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
//! # Module Structure (按功能分层)
//!
//! | Group | Path | Contents |
//! |-------|------|---------|
//! | **core** | `core/` | Data types (`TextMetrics`, `TextCluster`, `ShapedText`) and commands (`RenderCommand`) |
//! | **backend** | `backend/` | Rendering backends: software surface (`BackBuffer`, `SoftwareSurface`, `RenderContext`), paint trait (`PaintBackend`, `SoftwarePaintBackend`), batch (`BatchId`), scene (`SceneLayer`, `RenderScene`) |
//! | **controls** | `controls/` | Widget-specific render controls (basic/input/special) |
//! | **pipeline** | `pipeline/` | Visual command pipeline for all widget types |
//! | **web** | `web/` | Web engine and web view rendering |
//! | **quality** | `quality/` | Adaptive rendering quality management |
//! | flat | `text_cache.rs` | Text caching utilities |

// ─── Sub-module declarations ─────────────────────────────────────────────────

// Core data types and commands
mod core;
// Rendering backends
mod backend;
// Widget controls (basic/input/special)
// (controls/ is a pre-grouped sub-directory)
// Visual command pipeline for all widget types
mod pipeline;
// Web rendering
mod web;
// Adaptive quality
pub mod quality;
// Text caching
pub mod text_cache;
#[cfg(test)]
mod tests;

// ─── Re-exports ──────────────────────────────────────────────────────────────

// Core
pub use core::{TextMetrics, TextCluster, ShapedText, RenderCommand};

// Backend
pub use backend::{BackBuffer, RenderContext, SoftwareSurface, SoftwareRenderConfig,
    set_default_software_render_config, default_software_render_config,
    PaintBackend, SoftwarePaintBackend,
    BatchId,
    SceneLayer, RenderScene, AutoRenderBackend,
    last_auto_render_backend,
    current_quality_level, set_quality_level, current_fps, average_frame_time};

// Pipeline — all append_* functions
pub use pipeline::{
    // Pixel ops
    fill_pixels,
    blend_pixel,
    // Widget drawing commands
    append_window_visual_commands,
    append_panel_visual_commands,
    append_label_visual_commands,
    append_button_visual_commands,
    append_checkbox_visual_commands,
    append_radiobutton_visual_commands,
    append_line_edit_visual_commands,
    append_combo_box_visual_commands,
    append_list_box_visual_commands,
    append_progress_bar_visual_commands,
    append_slider_visual_commands,
    append_scroll_bar_visual_commands,
    append_menu_bar_visual_commands,
    append_menu_visual_commands,
    append_context_menu_visual_commands,
    append_tool_bar_visual_commands,
    append_status_bar_visual_commands,
    append_tab_widget_visual_commands,
    append_text_edit_visual_commands,
    append_rich_edit_visual_commands,
    append_tree_view_visual_commands,
    append_table_widget_visual_commands,
    append_grid_widget_visual_commands,
    append_chart_widget_visual_commands,
    append_dock_panel_visual_commands,
    append_group_box_visual_commands,
    append_splitter_visual_commands,
    append_mdi_area_visual_commands,
    append_canvas_visual_commands,
    append_spin_box_visual_commands,
    append_list_view_visual_commands,
    append_scroll_area_visual_commands,
    append_dialog_visual_commands,
    append_message_box_visual_commands,
    append_file_dialog_visual_commands,
    append_color_dialog_visual_commands,
    append_font_dialog_visual_commands,
    append_popup_window_visual_commands,
    append_directory_dialog_visual_commands,
    append_activity_indicator_visual_commands,
    append_toggle_button_visual_commands,
    append_check_list_box_visual_commands,
    append_double_spin_box_visual_commands,
    append_dial_visual_commands,
    append_wizard_visual_commands,
};

// ─── Internal helpers ────────────────────────────────────────────────────────

/// Check if a rect has zero area.
fn is_empty_rect(rect: &crate::core::Rect) -> bool {
    rect.width == 0 || rect.height == 0
}

/// Shared helper accessible to surface.rs and backend
pub(crate) use pipeline::pixel_bytes_len;
