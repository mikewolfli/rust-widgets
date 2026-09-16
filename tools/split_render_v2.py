#!/usr/bin/env python3
"""
Split src/render/mod.rs (5760 lines) into focused sub-modules.
Reads the entire file, then writes exact byte-level copies to new sub-files.
All items remain pub via re-exports in mod.rs.

FILE STRUCTURE:
  render/
    mod.rs          - re-export hub + is_empty_rect helper
    types.rs        - TextMetrics, TextCluster, ShapedText
    command.rs      - RenderCommand enum
    surface.rs      - BackBuffer, SoftwareSurface, SoftwareRenderConfig
    paint.rs        - PaintBackend trait, SoftwarePaintBackend
    scene.rs        - SceneLayer, RenderScene, AutoRenderBackend, compose fns
    
    pixel_ops.rs    - fill_pixels, blend_pixel, set_pixel, helpers
    pipeline/
      mod.rs        - all append_*_visual_commands + helpers
    tests.rs        - #[cfg(test)] mod tests
"""
import os, shutil

RENDER_DIR = "src/render"
FILE = f"{RENDER_DIR}/mod.rs"

with open(FILE) as f:
    lines = f.readlines()

TOTAL = len(lines)
print(f"Total lines: {TOTAL}")
assert TOTAL == 5760, f"Expected 5760 lines, got {TOTAL}"

# Backup
shutil.copy2(FILE, FILE + ".bak2")

def extract(start_1idx, end_1idx):
    """Extract lines (1-indexed, inclusive)."""
    return "".join(lines[start_1idx-1:end_1idx])

# =========================================================
# Line map (1-indexed):
#   1-22    = doc comment (//! ...)
#   23-25   = pub mod batch, quality, text_cache
#   26-48   = imports + is_empty_rect fn
#   49-88   = TextMetrics, TextCluster, ShapedText
#   89-178  = BackBuffer + SoftwareRenderConfig + config fns
#   179-329 = impl SoftwareSurface { ... }
#   330-348 = PaintBackend trait
#   349-498 = SoftwarePaintBackend struct + impl PaintBackend
#   499-586 = RenderCommand enum
#   587-776 = AutoRenderBackend + quality fns + SceneLayer + RenderScene + compose + GpuRenderError
#   777-902 = compose_scene_to_surface (GPU + CPU) + cached_wgpu_renderer
#   903-3213 = all append_*_visual_commands + helpers (push_widget_fill_and_border etc.)
#   3214-3484 = fill_pixels, blend_pixel, set_pixel, geometry helper fns
#   3485-5760 = #[cfg(test)] mod tests { ... }

# =========================================================
# 1. types.rs: TextMetrics, TextCluster, ShapedText (lines 49-88)
# =========================================================
# These types depend on crate::core::Font (already imported)
types_content = """//! Core rendering data types for text and geometry.
use crate::core::Font;

""" + extract(49, 88)

with open(f"{RENDER_DIR}/types.rs", "w") as f:
    f.write(types_content)
print(f"types.rs: {len(types_content.splitlines())} lines")

# =========================================================
# 2. command.rs: RenderCommand enum (lines 499-586)
# =========================================================
# Uses Color, Font, Point, Rect from crate::core
command_content = """//! Render commands for composing widget visuals.
use crate::core::{Color, Font, Point, Rect};

""" + extract(499, 586)

with open(f"{RENDER_DIR}/command.rs", "w") as f:
    f.write(command_content)
print(f"command.rs: {len(command_content.splitlines())} lines")

# =========================================================
# 3. surface.rs: BackBuffer, SoftwareSurface, SoftwareRenderConfig (lines 89-329)
# =========================================================
# Depends on: crate::core::{Color, Font, Point, Rect, Size}
# Depends on: crate::render::{RenderCommand, TextMetrics, TextCluster, ShapedText}
# But wait - TextCluster is used in shape_text. However in original code,
# everything is in same module. Now surface.rs needs types from types.rs and command.rs.
# In Rust, sibling modules use `super::TypeName`.
# We'll use `crate::render::TypeName` since that's more stable.

surface_content = """//! Software rendering surface: back buffer, surface, and configuration.
use crate::core::{Color, Font, Point, Rect, Size};
use crate::render::{RenderCommand, TextMetrics, TextCluster, ShapedText};
use std::sync::{Mutex, OnceLock};

fn pixel_bytes_len(size: Size) -> usize {
    size.width.saturating_mul(size.height).saturating_mul(4) as usize
}

""" + extract(89, 329)

with open(f"{RENDER_DIR}/surface.rs", "w") as f:
    f.write(surface_content)
print(f"surface.rs: {len(surface_content.splitlines())} lines")

# =========================================================
# 4. paint.rs: PaintBackend + SoftwarePaintBackend (lines 330-498)
# =========================================================
# Depends on: crate::core types, crate::render types including SoftwareSurface

paint_content = """//! Paint backend trait and software implementation.
use crate::core::{Color, Font, Point, Rect, Size};
use crate::render::{RenderCommand, TextMetrics, ShapedText, SoftwareSurface, SoftwareRenderConfig};

""" + extract(330, 498)

with open(f"{RENDER_DIR}/paint.rs", "w") as f:
    f.write(paint_content)
print(f"paint.rs: {len(paint_content.splitlines())} lines")

# =========================================================
# 5. scene.rs: AutoRenderBackend + quality + SceneLayer + RenderScene + compose (lines 587-902)
# =========================================================
# Depends on: crate::core types, crate::render types including SoftwareSurface, PaintBackend, etc.

scene_content = """//! Render scene composition and auto-backend selection.
use crate::core::{Color, Point, Rect, Size};
use crate::render::{RenderCommand, SceneLayer, SoftwareSurface, SoftwarePaintBackend, SoftwareRenderConfig, PaintBackend, AutoRenderBackend};
use crate::quality::QualityLevel;
use std::sync::{Mutex, OnceLock};
#[cfg(feature = "gpu-wgpu")]
use crate::wgpu_backend::WgpuRenderer;

""" + extract(587, 902)

with open(f"{RENDER_DIR}/scene.rs", "w") as f:
    f.write(scene_content)
print(f"scene.rs: {len(scene_content.splitlines())} lines")

# =========================================================
# 6. pixel_ops.rs: fill_pixels, blend_pixel, set_pixel, geometry helpers (lines 3214-3484)
# =========================================================
# Depends on: crate::core types, TextCluster (used in helper fns)

pixel_ops_content = """//! Low-level pixel operations and geometry helpers.
use crate::core::{Color, Point, Rect, Size};
use crate::render::{TextCluster, SoftwareSurface};

""" + extract(3214, 3484)

with open(f"{RENDER_DIR}/pixel_ops.rs", "w") as f:
    f.write(pixel_ops_content)
print(f"pixel_ops.rs: {len(pixel_ops_content.splitlines())} lines")

# =========================================================
# 7. tests.rs: #[cfg(test)] mod tests (lines 3485-5760)
# =========================================================
tests_content = extract(3485, 5760)

with open(f"{RENDER_DIR}/tests.rs", "w") as f:
    f.write(tests_content)
print(f"tests.rs: {len(tests_content.splitlines())} lines")

# =========================================================
# 8. pipeline/mod.rs: append_*_visual_commands + helpers (lines 816-3213)
# =========================================================
# This includes push_widget_fill_and_border, centered_text_origin,
# normalized_progress_u32, normalized_progress_i32, and all append_* functions

os.makedirs(f"{RENDER_DIR}/pipeline", exist_ok=True)

pipeline_content = """//! Visual command pipeline: widget rendering functions.
use crate::core::{Color, Font, Point, Rect, Size};
use crate::render::is_empty_rect;
use crate::render::{RenderCommand, SceneLayer};
use crate::widget::{ActivityIndicator, Button, ButtonState, Canvas, ChartWidget, CheckBox, CheckState, ColorDialog,
    ComboBox, ContextMenu, Dialog, DirectoryDialog, DockPanel, FileDialog, FontDialog, GridWidget,
    GroupBox, Label, LineEdit, ListBox, MdiArea, Menu, MenuBar, MessageBox, Panel, PopupWindow,
    ProgressBar, RadioButton, RichEdit, ScrollBar, Slider, Splitter, StatusBar, TabWidget,
    TableWidget, TextEdit, ToolBar, TreeView, Widget};
use font8x8::{UnicodeFonts, BASIC_FONTS};

""" + extract(816, 3213)

with open(f"{RENDER_DIR}/pipeline/mod.rs", "w") as f:
    f.write(pipeline_content)
print(f"pipeline/mod.rs: {len(pipeline_content.splitlines())} lines")

# =========================================================
# 9. mod.rs: Complete rewrite as re-export hub
# =========================================================
# Keep: doc comment, pub mod declarations for sub-modules + existing batch/quality/text_cache
# Keep: is_empty_rect helper (needed by pipeline)

mod_content = """//! Rendering primitives and software surface baseline.
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

// Split sub-modules
mod types;
mod command;
mod surface;
mod paint;
mod scene;
mod pixel_ops;
mod pipeline;
#[cfg(test)]
mod tests;

// Existing sub-modules (unchanged)
pub mod batch;
pub mod quality;
pub mod text_cache;

// Re-exports from sub-modules
pub use types::{TextMetrics, TextCluster, ShapedText};
pub use command::RenderCommand;
pub use surface::{BackBuffer, SoftwareSurface, SoftwareRenderConfig,
    set_default_software_render_config, default_software_render_config};
pub use paint::{PaintBackend, SoftwarePaintBackend};
pub use scene::{SceneLayer, RenderScene, AutoRenderBackend,
    last_auto_render_backend,
    current_quality_level, set_quality_level, current_fps, average_frame_time};
pub use pixel_ops::{fill_pixels, blend_pixel};

// Pipeline re-exports
pub use pipeline::{
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

// Internal helper used across sub-modules
fn is_empty_rect(rect: &crate::core::Rect) -> bool {
    rect.width == 0 || rect.height == 0
}
"""

with open(FILE, "w") as f:
    f.write(mod_content)
print(f"mod.rs: {len(mod_content.splitlines())} lines (rewritten)")

# =========================================================
# Summary
# =========================================================
print("\n=== SPLIT COMPLETE ===")
print("Files created/updated:")
for root, dirs, files in os.walk(RENDER_DIR):
    for fn in sorted(files):
        path = os.path.join(root, fn)
        with open(path) as f:
            lc = len(f.readlines())
        print(f"  {path} ({lc} lines)")
