#!/usr/bin/env python3
"""
Split src/render/mod.rs (5760 lines) into focused sub-modules.

SECTION MAP (line numbers, 1-indexed):
  1-25    Module doc + pub mod declarations
  26-48   imports + is_empty_rect
  49-88   TextMetrics, TextCluster, ShapedText → types.rs
  89-173  BackBuffer + SoftwareRenderConfig + global_config fns → surface.rs
  174-329 SoftwareSurface impl
  330-348 PaintBackend trait → paint.rs
  349-498 SoftwarePaintBackend → paint.rs
  499-586 RenderCommand enum → command.rs
  587-776 AutoRenderBackend + quality fns + SceneLayer + RenderScene + GPU error + compose fns → scene.rs
  777-902   [GPU compose fn body, cached_wgpu_renderer] → scene.rs
  903-3213   append_*_visual_commands (widgets, containers, dialogs...) → pipeline/
  3214-3484  fill_pixels, blend_pixel + SoftwareSurface helper impls → pixel_ops.rs
  3485-5760  #[cfg(test)] mod tests → tests.rs
"""

import os
import shutil

RENDER_DIR = "src/render"
BACKUP = f"{RENDER_DIR}/mod.rs.bak"

# Read entire file
with open(f"{RENDER_DIR}/mod.rs", "r") as f:
    lines = f.readlines()

total = len(lines)
print(f"Total lines: {total}")

# Create backup
shutil.copy2(f"{RENDER_DIR}/mod.rs", BACKUP)
print(f"Backup saved to {BACKUP}")

# Ensure subdirectories exist
os.makedirs(f"{RENDER_DIR}", exist_ok=True)

def extract_lines(start, end):
    """Extract lines 1-indexed start to end (inclusive), return as string."""
    # Convert to 0-indexed
    s = max(0, start - 1)
    e = min(total, end)
    return "".join(lines[s:e])

# ============================================================
# 1. types.rs - TextMetrics, TextCluster, ShapedText
# ============================================================
# Lines 49-88 (TextMetrics, TextCluster, ShapedText)
# Needs: use crate::core::{Font, Point, Rect, Size};
types_content = """//! Render data types: text metrics, clusters, and shaped text.
use crate::core::Font;

""" + extract_lines(49, 88)

with open(f"{RENDER_DIR}/types.rs", "w") as f:
    f.write(types_content)
print(f"Created types.rs ({len(types_content.splitlines())} lines)")

# ============================================================
# 2. surface.rs - BackBuffer, SoftwareSurface, SoftwareRenderConfig
# ============================================================
# Lines 89-329 (BackBuffer + SoftwareSurface + SoftwareRenderConfig + config fns)
# + SoftwareSurface impl blocks (need to find exact boundaries)
# From BackBuffer to end of SoftwareSurface impl
# Need: use crate::core::{Color, Font, Point, Rect, Size};
# Need: use std::sync::{Mutex, OnceLock};
surface_content = """//! Software rendering surface types: back buffer, surface, and render config.
use crate::core::{Color, Font, Point, Rect, Size};
use crate::render::{RenderCommand, TextMetrics, TextCluster, ShapedText};
use std::sync::{Mutex, OnceLock};

fn pixel_bytes_len(size: Size) -> usize {
    (size.width as usize).saturating_mul(size.height as usize).saturating_mul(4)
}

""" + extract_lines(89, 329)

with open(f"{RENDER_DIR}/surface.rs", "w") as f:
    f.write(surface_content)
print(f"Created surface.rs ({len(surface_content.splitlines())} lines)")

# ============================================================
# 3. paint.rs - PaintBackend trait + SoftwarePaintBackend
# ============================================================
# Lines 330-498 (PaintBackend + SoftwarePaintBackend + impl PaintBackend for SoftwarePaintBackend)
paint_content = """//! Paint backend trait and software implementation.
use crate::core::{Color, Font, Point, Rect, Size};
use crate::render::{RenderCommand, TextMetrics, ShapedText, SoftwareSurface, SoftwareRenderConfig};

""" + extract_lines(330, 498)

with open(f"{RENDER_DIR}/paint.rs", "w") as f:
    f.write(paint_content)
print(f"Created paint.rs ({len(paint_content.splitlines())} lines)")

# ============================================================
# 4. command.rs - RenderCommand enum
# ============================================================
command_content = """//! Render command enum for scene composition.
use crate::core::{Color, Font, Point, Rect};

""" + extract_lines(499, 586)

with open(f"{RENDER_DIR}/command.rs", "w") as f:
    f.write(command_content)
print(f"Created command.rs ({len(command_content.splitlines())} lines)")

# ============================================================
# 5. scene.rs - SceneLayer, RenderScene, compose functions
# ============================================================
# Lines 587-902 (AutoRenderBackend + quality fns + SceneLayer + RenderScene + compose + GPU)
scene_content = """//! Render scene and layer composition.
use crate::core::{Color, Point, Rect, Size};
use crate::render::{RenderCommand, SceneLayer, SoftwareSurface, SoftwarePaintBackend, SoftwareRenderConfig, PaintBackend, AutoRenderBackend};
#[cfg(feature = "gpu-wgpu")]
use crate::wgpu_backend::WgpuRenderer;
#[cfg(feature = "quality-management")]
use crate::quality::QualityManager;
use std::sync::{Mutex, OnceLock};

""" + extract_lines(587, 902)

with open(f"{RENDER_DIR}/scene.rs", "w") as f:
    f.write(scene_content)
print(f"Created scene.rs ({len(scene_content.splitlines())} lines)")

# ============================================================
# 6. pixel_ops.rs - fill_pixels, blend_pixel
# ============================================================
# Lines 3214-3484 (pixel operations + SoftwareSurface helper impls)
pixel_ops_content = """//! Low-level pixel operations for software rendering.
use crate::core::{Color, Point, Rect, Size};
use crate::render::SoftwareSurface;

""" + extract_lines(3214, 3484)

with open(f"{RENDER_DIR}/pixel_ops.rs", "w") as f:
    f.write(pixel_ops_content)
print(f"Created pixel_ops.rs ({len(pixel_ops_content.splitlines())} lines)")

# ============================================================
# 7. tests.rs - #[cfg(test)] mod tests
# ============================================================
# Lines 3485-5760
tests_content = extract_lines(3485, 5760)

with open(f"{RENDER_DIR}/tests.rs", "w") as f:
    f.write(tests_content)
print(f"Created tests.rs ({len(tests_content.splitlines())} lines)")

# ============================================================
# 8. pipeline/ - append_*_visual_commands functions
# ============================================================
# Lines 903-3213
os.makedirs(f"{RENDER_DIR}/pipeline", exist_ok=True)

# Read the pipeline section
pipeline_text = extract_lines(903, 3213)

# Split into:
# - pipeline/controls.rs: basic widgets (button, checkbox, label, radiobutton, line_edit, combo_box, list_box, progress_bar, slider, scroll_bar, spin_box, text_edit, rich_edit, tree_view, table_widget, list_view, canvas, chart, toggle_button, dial, activity_indicator)
# - pipeline/containers.rs: container widgets (window, panel, menu_bar, menu, context_menu, tool_bar, status_bar, tab_widget, dock_panel, group_box, splitter, mdi_area, scroll_area)
# - pipeline/dialogs.rs: dialog widgets (dialog, message_box, file_dialog, color_dialog, font_dialog, popup_window, directory_dialog)
# - pipeline/misc.rs: the rest (wizard, check_list_box, double_spin_box)

pipeline_common_imports = """//! Widget visual command pipeline functions.
use crate::core::{Color, Font, Point, Rect, Size};
use crate::render::{RenderCommand, SceneLayer, is_empty_rect};
use crate::widget::Widget;

fn push_widget_fill_and_border<W: Widget>(
    layer: &mut SceneLayer,
    widget: &W,
    fallback_background: Option<Color>,
    fallback_border: Option<(Color, u32)>,
) {
    let rect = widget.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    if let Some(background) = widget.background_color().or(fallback_background) {
        if widget.border_radius() > 0 {
            layer.push(RenderCommand::FillRoundedRect {
                rect,
                radius: widget.border_radius(),
                color: background,
            });
        } else {
            layer.push(RenderCommand::FillRect {
                rect,
                color: background,
            });
        }
    }
    let border_color = widget
        .border_color()
        .or_else(|| fallback_border.map(|value| value.0));
    let border_width = if widget.border_width() > 0 {
        widget.border_width()
    } else {
        fallback_border.map(|value| value.1).unwrap_or(0)
    };
    if let Some(color) = border_color {
        if border_width > 0 {
            if widget.border_radius() > 0 {
                layer.push(RenderCommand::DrawRoundedRectStroke {
                    rect,
                    radius: widget.border_radius(),
                    color,
                    width: border_width,
                });
            } else {
                layer.push(RenderCommand::DrawRectStroke {
                    rect,
                    color,
                    width: border_width,
                });
            }
        }
    }
}

fn centered_text_origin(rect: Rect) -> Point {
    Point {
        x: rect.x + 6,
        y: rect.y + (rect.height as i32 / 2) - 4,
    }
}

#[allow(dead_code)]
fn normalized_progress_u32(value: u32, min: u32, max: u32) -> f32 {
    if max <= min {
        return 0.0;
    }
    ((value.saturating_sub(min)) as f32 / (max - min) as f32).clamp(0.0, 1.0)
}

fn normalized_progress_i32(value: i32, min: i32, max: i32) -> f32 {
    if max <= min {
        return 0.0;
    }
    ((value - min) as f32 / (max - min) as f32).clamp(0.0, 1.0)
}

"""

# Find line boundaries for each section
# The original file has these functions in order from line 903-3213:
# I need to identify exact line ranges for each group

# Let me instead keep the pipeline in one file since the functions share helper code
# and splitting them would require duplicating the helpers or making a shared module

with open(f"{RENDER_DIR}/pipeline/mod.rs", "w") as f:
    # Write the complete pipeline section with all append functions + helpers
    content = """//! Widget visual command pipeline functions.
use crate::core::{Color, Font, Point, Rect, Size};
use crate::widget::{Window, Panel, Label, Button, ButtonState, CheckBox, CheckState, RadioButton, LineEdit, ComboBox, ListBox, ProgressBar, Slider, ScrollBar, MenuBar, Menu, ContextMenu, ToolBar, StatusBar, TabWidget, TextEdit, RichEdit, TreeView, TableWidget, GridWidget, ChartWidget, DockPanel, GroupBox, Splitter, MdiArea, Canvas, ScrollArea, ListView, Dialog, MessageBox, FileDialog, ColorDialog, FontDialog, PopupWindow, DirectoryDialog, ActivityIndicator, ToggleButton, CheckListBox, DoubleSpinBox};
use crate::widget::Widget;
use crate::render::{RenderCommand, SceneLayer};

""" +     extract_lines(816, 825) + "\n" +  # push_widget_fill_and_border
    extract_lines(835, 901) + "\n" +  # centered_text_origin + normalized_progress fns + append_window
    extract_lines(903, 3213)  # All append_* functions
    f.write(content)

print(f"Created pipeline/mod.rs ({len(content.splitlines())} lines)")

# ============================================================
# 9. Rewrite mod.rs as re-export hub
# ============================================================
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

// Core types
mod types;
mod command;
mod surface;
mod paint;
mod scene;
mod pixel_ops;
mod pipeline;
#[cfg(test)]
mod tests;

// Sub-modules (existing, unchanged)
pub mod batch;
pub mod quality;
pub mod text_cache;

// Re-exports
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

// Internal helpers used across sub-modules
fn is_empty_rect(rect: &crate::core::Rect) -> bool {
    rect.width == 0 || rect.height == 0
}
"""

with open(f"{RENDER_DIR}/mod.rs", "w") as f:
    f.write(mod_content)
print(f"Rewrote mod.rs ({len(mod_content.splitlines())} lines)")

print("\n=== SPLIT COMPLETE ===")
print("Files created:")
for root, dirs, files in os.walk(RENDER_DIR):
    for fn in files:
        path = os.path.join(root, fn)
        with open(path) as f:
            lc = len(f.readlines())
        print(f"  {path} ({lc} lines)")
