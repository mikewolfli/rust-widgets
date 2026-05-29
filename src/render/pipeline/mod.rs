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
mod containers;
mod controls;
mod dialogs;
mod menu_toolbar;
mod misc;
mod pixel_ops;
#[cfg(feature = "unstable-special-widgets")]
mod special;

// Re-export all append_* functions from sub-modules for crate-level access.
#[allow(deprecated)]
pub use containers::{
    append_canvas_visual_commands, append_chart_widget_visual_commands,
    append_dock_panel_visual_commands, append_grid_widget_visual_commands,
    append_group_box_visual_commands, append_list_view_visual_commands,
    append_mdi_area_visual_commands, append_rich_edit_visual_commands,
    append_scroll_area_visual_commands, append_spin_box_visual_commands,
    append_splitter_visual_commands, append_tab_widget_visual_commands,
    append_table_widget_visual_commands, append_text_edit_visual_commands,
    append_tree_view_visual_commands,
};
#[allow(deprecated)]
pub use controls::{
    append_button_visual_commands, append_checkbox_visual_commands,
    append_combo_box_visual_commands, append_label_visual_commands,
    append_line_edit_visual_commands, append_list_box_visual_commands,
    append_panel_visual_commands, append_progress_bar_visual_commands,
    append_radiobutton_visual_commands, append_scroll_bar_visual_commands,
    append_slider_visual_commands, append_window_visual_commands,
};
#[allow(deprecated)]
pub use dialogs::{
    append_color_dialog_visual_commands, append_dialog_visual_commands,
    append_directory_dialog_visual_commands, append_file_dialog_visual_commands,
    append_font_dialog_visual_commands, append_message_box_visual_commands,
    append_popup_window_visual_commands,
};
#[allow(deprecated)]
pub use menu_toolbar::{
    append_context_menu_visual_commands, append_menu_bar_visual_commands,
    append_menu_visual_commands, append_status_bar_visual_commands,
    append_tool_bar_visual_commands,
};
#[allow(deprecated)]
pub use misc::{
    append_activity_indicator_visual_commands, append_check_list_box_visual_commands,
    append_dial_visual_commands, append_double_spin_box_visual_commands,
    append_toggle_button_visual_commands, append_wizard_visual_commands,
};
pub use pixel_ops::{blend_pixel, fill_pixels};
#[cfg(feature = "unstable-special-widgets")]
#[allow(unused_imports)]
#[allow(deprecated)]
pub use special::append_lcd_number_visual_commands;

// Re-export internal helper used by surface.rs
pub(crate) use pixel_ops::pixel_bytes_len;
