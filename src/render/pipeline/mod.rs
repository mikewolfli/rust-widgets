//! Visual command pipeline: grouped sub-modules for widget rendering.
//!
//! Sub-modules:
//! - `controls`: Basic widgets (window, panel, label, button, checkbox, radiobutton,
//!   line_edit, combo_box, list_box, progress_bar, slider, scroll_bar) + internal helpers
//! - `menu_toolbar`: Menu and toolbar widgets (menu_bar, menu, context_menu, tool_bar, status_bar)
//! - `containers`: Container and complex widgets (tab_widget, text_edit, rich_edit, tree_view,
//!   table_widget, grid_widget, chart_widget, dock_panel, group_box, splitter, mdi_area,
//!   canvas, spin_box, list_view, scroll_area) + `impl SoftwareSurface` methods
//! - `pixel_ops`: Pixel-level operations (fill_pixels, blend_pixel, set_pixel) and
//!   coverage/geometry helpers for anti-aliased rendering
//! - `dialogs`: Dialog widgets (dialog, message_box, file_dialog, color_dialog, font_dialog,
//!   popup_window, directory_dialog)
//! - `misc`: Miscellaneous widgets (activity_indicator, toggle_button, check_list_box,
//!   double_spin_box, dial, wizard)
//! - `mod.rs` (this file): Routing functions and re-exports

use crate::render::{PaintBackend, RenderContext};
use crate::widget::Widget;

mod controls;
mod menu_toolbar;
mod containers;
mod pixel_ops;
mod dialogs;
mod misc;

// Re-export all append_* functions from sub-modules for crate-level access.
pub use controls::{
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
};
pub use menu_toolbar::{
    append_menu_bar_visual_commands,
    append_menu_visual_commands,
    append_context_menu_visual_commands,
    append_tool_bar_visual_commands,
    append_status_bar_visual_commands,
};
pub use containers::{
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
};
pub use dialogs::{
    append_dialog_visual_commands,
    append_message_box_visual_commands,
    append_file_dialog_visual_commands,
    append_color_dialog_visual_commands,
    append_font_dialog_visual_commands,
    append_popup_window_visual_commands,
    append_directory_dialog_visual_commands,
};
pub use misc::{
    append_activity_indicator_visual_commands,
    append_toggle_button_visual_commands,
    append_check_list_box_visual_commands,
    append_double_spin_box_visual_commands,
    append_dial_visual_commands,
    append_wizard_visual_commands,
};
pub use pixel_ops::{fill_pixels, blend_pixel};

// Re-export internal helper used by surface.rs
pub(crate) use pixel_ops::pixel_bytes_len;

// ─── Routing functions ───────────────────────────────────────────────────────

/// Routing logic for native vs custom widget drawing.
/// This function provides a framework for routing between native and custom drawing paths.
/// Widgets that implement the Draw trait will use custom drawing, others use native.
pub fn route_widget_drawing<W>(
    widget: &mut W,
    context: &mut RenderContext,
    custom_renderer: impl FnOnce(&mut W, &mut RenderContext),
    _native_renderer: impl FnOnce(&mut W, &mut RenderContext),
) where
    W: Widget + ?Sized,
{
    custom_renderer(widget, context);
}

/// Check if a widget uses custom drawing.
pub fn widget_uses_custom_drawing<W>(_widget: &W) -> bool
where
    W: Widget + ?Sized,
{
    false
}

/// Render a widget with automatic routing between native and custom drawing.
pub fn render_widget<W>(
    widget: &mut W,
    backend: &mut dyn PaintBackend,
    custom_renderer: impl FnOnce(&mut W, &mut RenderContext),
) where
    W: Widget + ?Sized,
{
    let mut context = RenderContext::new(backend);
    custom_renderer(widget, &mut context);
}

/// Helper function to render widgets that implement Draw trait.
pub fn render_custom_widget<W>(widget: &mut W, context: &mut RenderContext)
where
    W: crate::widget::Draw,
{
    widget.draw(context);
}

/// Helper function to render widgets using native platform rendering.
pub fn render_native_widget<W>(widget: &W, context: &mut RenderContext)
where
    W: Widget,
{
    let rect = widget.geometry();
    let style = widget.style();
    if let Some(bg_color) = style.background_color {
        context.fill_rect(rect, bg_color);
    }
    if style.border_width > 0 {
        if let Some(border_color) = style.border_color {
            context.draw_rect_stroke(rect, border_color, style.border_width);
        }
    }
}
