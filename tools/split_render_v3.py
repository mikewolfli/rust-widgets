#!/usr/bin/env python3
"""
Split src/render/mod.rs (5760 lines) into focused sub-modules.
CORRECTED BOUNDARIES based on actual file content analysis.

FILE STRUCTURE (final):
  render/
    mod.rs       - re-export hub (uses pub use from all sub-modules)
    types.rs     - TextMetrics, TextCluster, ShapedText (L49-86)
    command.rs   - RenderCommand enum (L497-584)
    surface.rs   - BackBuffer, RenderContext, SoftwareSurface, SoftwareRenderConfig (L87-328)
    paint.rs     - PaintBackend trait, SoftwarePaintBackend (L329-496)
    scene.rs     - SceneLayer, RenderScene, AutoRenderBackend, compose fns (L585-831)
    pipeline/
      mod.rs     - push_widget_fill_and_border + all append_* fns + pixel ops + routing fns (L832-3484 + 5082-5760)
    tests.rs     - #[cfg(test)] mod tests (L3485-5077)
"""
import os, shutil

RENDER_DIR = "src/render"
FILE = f"{RENDER_DIR}/mod.rs"

with open(FILE + ".bak2") as f:
    lines = f.readlines()

TOTAL = len(lines)
print(f"Total lines: {TOTAL}")

def extract(start_1idx, end_1idx):
    """Extract lines (1-indexed, inclusive)."""
    return "".join(lines[start_1idx-1:end_1idx])

# =========================================================
# 1. types.rs: TextMetrics, TextCluster, ShapedText (lines 49-86)
# =========================================================
types_content = """//! Core rendering data types for text and geometry.
use crate::core::Font;

""" + extract(49, 86)

with open(f"{RENDER_DIR}/types.rs", "w") as f:
    f.write(types_content)
print(f"types.rs: {len(types_content.splitlines())} lines")

# Make cross-module fields pub(crate)
with open(f"{RENDER_DIR}/types.rs") as f:
    content = f.read()
content = content.replace(
    "    clusters: Vec<TextCluster>,",
    "    pub(crate) clusters: Vec<TextCluster>,"
)
content = content.replace(
    "    advance: f32,",
    "    pub(crate) advance: f32,"
)
with open(f"{RENDER_DIR}/types.rs", "w") as f:
    f.write(content)

# =========================================================
# 2. command.rs: RenderCommand enum (lines 497-584)
# =========================================================
command_content = """//! Render commands for composing widget visuals.
use crate::core::{Color, Font, Point, Rect};

""" + extract(497, 584)

with open(f"{RENDER_DIR}/command.rs", "w") as f:
    f.write(command_content)
print(f"command.rs: {len(command_content.splitlines())} lines")

# =========================================================
# 3. surface.rs: BackBuffer, SoftwareSurface, SoftwareRenderConfig (lines 87-328)
# =========================================================
surface_content = """//! Software rendering surface: back buffer, surface, and configuration.
use crate::core::{Color, Font, Point, Rect, Size};
use crate::render::{PaintBackend, RenderCommand, TextMetrics, TextCluster, ShapedText};
use crate::render::pixel_bytes_len;
use std::sync::{Mutex, OnceLock};

""" + extract(87, 328)

# Make cross-module fields pub(crate)
surface_content = surface_content.replace(
    "    buffer: BackBuffer,",
    "    pub(crate) buffer: BackBuffer,"
)
surface_content = surface_content.replace(
    "    aa_samples_per_axis: u8,",
    "    pub(crate) aa_samples_per_axis: u8,"
)
surface_content = surface_content.replace(
    "    back: Vec<u8>,",
    "    pub(crate) back: Vec<u8>,"
)

with open(f"{RENDER_DIR}/surface.rs", "w") as f:
    f.write(surface_content)
print(f"surface.rs: {len(surface_content.splitlines())} lines")

# =========================================================
# 4. paint.rs: PaintBackend + SoftwarePaintBackend (lines 329-496)
# =========================================================
paint_content = """//! Paint backend trait and software implementation.
use crate::core::{Color, Font, Point, Rect, Size};
use crate::render::{RenderCommand, TextMetrics, ShapedText, SoftwareSurface, SoftwareRenderConfig};

""" + extract(329, 496)

# Make cross-module fields pub(crate)
paint_content = paint_content.replace(
    "    surface: SoftwareSurface,",
    "    pub(crate) surface: SoftwareSurface,"
)

with open(f"{RENDER_DIR}/paint.rs", "w") as f:
    f.write(paint_content)
print(f"paint.rs: {len(paint_content.splitlines())} lines")

# =========================================================
# 5. scene.rs: AutoRenderBackend + quality + SceneLayer + RenderScene + compose (lines 585-831)
# =========================================================
scene_content = """//! Render scene composition and auto-backend selection.
use crate::core::{Color, Point, Rect, Size};
use crate::render::{PaintBackend, RenderCommand, SoftwarePaintBackend, SoftwareSurface, SoftwareRenderConfig};
#[cfg(feature = "quality-management")]
use crate::quality::QualityManager;
use std::sync::{Mutex, OnceLock};
#[cfg(feature = "gpu-wgpu")]
use crate::wgpu_backend::WgpuRenderer;

""" + extract(585, 831)

with open(f"{RENDER_DIR}/scene.rs", "w") as f:
    f.write(scene_content)
print(f"scene.rs: {len(scene_content.splitlines())} lines")

# =========================================================
# 6. pipeline/mod.rs: append_*_visual_commands + pixel ops + routing (L832-3484 + 5082-5760)
# =========================================================
os.makedirs(f"{RENDER_DIR}/pipeline", exist_ok=True)

pipeline_content = """//! Visual command pipeline: widget rendering functions.
use crate::core::{Color, Font, Point, Rect, Size};
use crate::render::is_empty_rect;
use crate::render::{BackBuffer, PaintBackend, RenderCommand, RenderContext, SceneLayer, SoftwareSurface, SoftwareRenderConfig, TextCluster, TextMetrics, ShapedText,
    default_software_render_config};
use crate::window::Window;
use font8x8::BASIC_FONTS;
use font8x8::UnicodeFonts;
use crate::widget::{ActivityIndicator, Button, ButtonState, Canvas, ChartWidget, CheckBox, CheckState, ColorDialog,
    ComboBox, ContextMenu, Dialog, DirectoryDialog, DockPanel, FileDialog, FontDialog, GridWidget,
    GroupBox, Label, LineEdit, ListBox, MdiArea, Menu, MenuBar, MessageBox, Panel, PopupWindow,
    ProgressBar, RadioButton, RichEdit, ScrollBar, Slider, Splitter, StatusBar, TabWidget,
    TableWidget, TextEdit, ToolBar, TreeView, Widget};

""" + extract(832, 3484) + "\n" + extract(5082, 5760)

# Make pixel_bytes_len pub(crate) so surface.rs can access it
pipeline_content = pipeline_content.replace(
    "fn pixel_bytes_len(size: Size) -> usize {",
    "pub(crate) fn pixel_bytes_len(size: Size) -> usize {"
)

with open(f"{RENDER_DIR}/pipeline/mod.rs", "w") as f:
    f.write(pipeline_content)
print(f"pipeline/mod.rs: {len(pipeline_content.splitlines())} lines")

# =========================================================
# 7. tests.rs: #[cfg(test)] mod tests (lines 3485-5077)
# =========================================================
tests_content = extract(3485, 5077)

with open(f"{RENDER_DIR}/tests.rs", "w") as f:
    f.write(tests_content)
print(f"tests.rs: {len(tests_content.splitlines())} lines")

with open(f"{RENDER_DIR}/pipeline/mod.rs", "w") as f:
    f.write(pipeline_content)
print(f"pipeline/mod.rs: {len(pipeline_content.splitlines())} lines")

# =========================================================
# 9. mod.rs: Complete rewrite as re-export hub
# =========================================================
mod_content = """//! Rendering primitives and software surface baseline.
//!
//! # Coordinate System
//!
//! This module uses the framework's standard **screen coordinate system** with origin at **top-left**:
//!
//! - **X axis**: Increases from left to right (0 \u2192 width)
//! - **Y axis**: Increases from top to bottom (0 \u2192 height)
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
pub use surface::{BackBuffer, RenderContext, SoftwareSurface, SoftwareRenderConfig,
    set_default_software_render_config, default_software_render_config};
pub use paint::{PaintBackend, SoftwarePaintBackend};
pub use scene::{SceneLayer, RenderScene, AutoRenderBackend,
    last_auto_render_backend,
    current_quality_level, set_quality_level, current_fps, average_frame_time};

// Pipeline re-exports
pub use pipeline::{
    fill_pixels,
    blend_pixel,
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
// Shared helper accessible to surface.rs
pub(crate) use pipeline::pixel_bytes_len;
"""

with open(FILE, "w") as f:
    f.write(mod_content)
print(f"mod.rs: {len(mod_content.splitlines())} lines (rewritten)")

print("\n=== SPLIT COMPLETE ===")
print("Files:")
for root, dirs, files in os.walk(RENDER_DIR):
    for fname in sorted(files):
        path = os.path.join(root, fname)
        with open(path) as f:
            lc = len(f.readlines())
        print(f"  {path} ({lc} lines)")
