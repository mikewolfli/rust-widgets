// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Display widgets: progress bars, sliders, scroll bars, etc.
pub mod arc;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub mod badge;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub mod color_history;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub mod color_well;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub mod divider;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub mod empty_state;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub mod floating_label;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub mod font_preview;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub mod icon;
#[cfg(feature = "image")]
pub mod image_view;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub mod lcd_number;
pub mod line;
pub mod meter;
pub mod mini_canvas;
pub mod mini_chart;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub mod progress_circle;
pub mod progressbar;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub mod rating;
pub mod roller;
pub mod scrollbar;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub mod skeleton_loader;
pub mod slider;
pub mod spinner;
pub mod switch;

// Re-export shared helpers.
mod draw_helpers;
pub(crate) use draw_helpers::draw_line;
