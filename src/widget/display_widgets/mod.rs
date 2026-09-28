// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Display widgets: progress bars, sliders, scroll bars, etc.
/// Circular arc / gauge primitive used by the progress and meter widgets.
pub mod arc;
#[cfg(widgets_unstripped)]
pub mod badge;
#[cfg(widgets_unstripped)]
pub mod color_history;
#[cfg(widgets_unstripped)]
pub mod color_well;
#[cfg(widgets_unstripped)]
pub mod divider;
/// Shell for choosing a glyph; the glyph table is supplied by the caller.
#[cfg(full_widgets)]
pub mod emoji_picker;
#[cfg(widgets_unstripped)]
pub mod empty_state;
#[cfg(widgets_unstripped)]
pub mod floating_label;
#[cfg(widgets_unstripped)]
pub mod font_preview;
/// The icon set: the built-in tokens (generated), their bundled geometry, the fallback geometry,
/// and the storage for host-registered icons.
#[cfg(widgets_unstripped)]
pub mod icon;
/// The storage behind the runtime icon registry. Its own module so the lock and the table can be
/// reasoned about without the public API (`crate::widget::icon_registry`) in view.
#[cfg(widgets_unstripped)]
pub(crate) mod icon_data_set;
#[cfg(feature = "image")]
pub mod image_view;
#[cfg(widgets_unstripped)]
/// Seven-segment style numeric readout.
pub mod lcd_number;
pub mod line;
pub mod meter;
pub mod mini_canvas;
pub mod mini_chart;
#[cfg(widgets_unstripped)]
pub mod progress_circle;
pub mod progressbar;
#[cfg(widgets_unstripped)]
pub mod rating;
pub mod roller;
pub mod scrollbar;
#[cfg(widgets_unstripped)]
pub mod skeleton_loader;
pub mod slider;
pub mod spinner;
pub mod switch;

// Re-export shared helpers.
mod draw_helpers;
pub(crate) use draw_helpers::draw_line;
