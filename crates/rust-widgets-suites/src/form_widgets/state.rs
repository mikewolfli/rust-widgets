// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! State bookkeeping shared by the form suite's two container controls.
//!
//! `Form` and `FormSet` both override `Widget::set_enabled` so that either spelling of the call
//! reaches the controls they host (FM-06). Rust has no `super` for a trait default, so an
//! override cannot call the implementation it replaces; the default's own bookkeeping lives here
//! once and both overrides delegate to it.

use rust_widgets::widget::Widget;

/// Applies `Widget::set_enabled`'s default body to `widget`.
///
/// The steps are the parent's, in order: the unchanged-flag early return, the base write, the
/// focus release (B2-01: a focused control that is disabled must not keep receiving input), the
/// theme re-resolution (the disabled state selects a different style key), the repaint request
/// and the accessibility announcement.
pub(crate) fn apply_enabled_state<W: Widget>(widget: &mut W, enabled: bool) {
    if widget.base().is_enabled() == enabled {
        return;
    }
    widget.base_mut().set_enabled(enabled);
    if !enabled
        && rust_widgets::widget::runtime::focused_widget()
            == rust_widgets::widget::runtime::registry_id_of(widget.base().id())
    {
        rust_widgets::widget::runtime::clear_focus();
    }
    widget.set_state_theme_hook();
    widget.base().request_redraw();
    if let Some(id) = rust_widgets::widget::runtime::registry_id_of(widget.base().id()) {
        rust_widgets::widget::a11y_submit::submit_state_changed(id);
    }
}
