// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

pub(crate) mod create_widgets;
pub mod custom_paint;

mod backend_impl;
mod property_names;

pub use custom_paint::CustomPaintControlBackend;
pub(crate) use property_names::{
    widget_label_property_name, widget_value_property_name, TOGGLE_BUTTON_KIND,
};
// Only the full-widget-set creation path reads these two tables, so a stripped profile
// that re-exported them would carry an unused import (principle #10).
#[cfg(full_widgets)]
pub(crate) use property_names::{LABEL_PROPERTY_NAMES, MENU_KIND};

/// Tests for the custom backend's creation and state accessors.
///
/// Gated on the full widget set because every one of them exercises the real
/// creation path: they assert that a `create_*` call returns an id addressing a
/// mounted widget, and that reads and writes reach that widget. The alloc-frugal
/// profile compiles neither the factory nor the widget registry, so those
/// assertions describe machinery that is deliberately absent there — an earlier
/// revision kept these tests alive by asserting against a backend-side shadow map,
/// which is the duplication this refactor removed.
#[cfg(all(test, not(embedded_surface), widgets_unstripped))]
mod tests;
