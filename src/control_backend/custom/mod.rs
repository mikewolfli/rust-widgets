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
// Each table is re-exported under the gate its **consumer** compiles in, not under
// `full_widgets`. The two consumers differ:
//
//  * `LABEL_PROPERTY_NAMES` is read by the label accessor, gated `widgets_unstripped`;
//  * `MENU_KIND` is read by the menu creation arm, which is `all(not(alloc_frugal),
//    not(embedded_surface))` — it covers the no-device-profile desktop build
//    (`windows controls-custom …`) where `full_widgets` is false but the popup still has to be
//    mounted, and it is absent from `embedded` because the menu fragment is too.
//
// Gating both on `full_widgets` broke exactly that cell: the menu arm compiled and the table
// it names was cfg'd out (check_profiles step 6b).
#[cfg(widgets_unstripped)]
pub(crate) use property_names::LABEL_PROPERTY_NAMES;
#[cfg(all(not(alloc_frugal), not(embedded_surface)))]
pub(crate) use property_names::MENU_KIND;

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
