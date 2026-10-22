// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Property and kind name tables used by the custom-paint backend.
//!
//! Kept out of `mod.rs` (principle #8).

/// The property names a control may use for its user-visible label.
///
/// Kinds disagree: a `Button` exposes `text`, a `Window`/`GroupBox`/`PopupWindow`
/// exposes `title`, a `StatusBar` exposes `message`. A backend that sets or reads
/// "the label" has to know the spellings, so they are listed once here rather than
/// repeated at each call site.
///
/// # Why `label` is first
///
/// The list is also consulted in order, and while `text` is the most *common* spelling of a
/// label it is not the most *descriptive* one for every control. `FloatingLabel` is the
/// case that makes the difference visible: it publishes both `text` (what the user typed)
/// and `label` (the caption that floats above the field), so resolving "the label" to `text`
/// put the caption into the input and left the control with nothing to float — the exact
/// defect a floating-label field cannot afford. Listing `label` first means a control that
/// has a dedicated label property is asked for that property, and a control that does not
/// (every button, checkbox and menu item, which answers `UnknownProperty` to `label`) falls
/// through to `text` unchanged.
pub(crate) const LABEL_PROPERTY_NAMES: &[&str] = &["label", "text", "title", "message"];

/// The property names a control may use for its user-visible **value**.
///
/// Separate from [`LABEL_PROPERTY_NAMES`] because a label and a value are different facts even when
/// they share a spelling for a particular control: a slider's label is its caption and its value is
/// where the handle sits, and a screen reader announces both. The order is the order of decreasing
/// specificity — `value` is the property the controls in this crate publish for exactly this
/// purpose, and the rest are the domain names a gauge-like control uses.
///
/// # Why `text` is deliberately absent
///
/// `text` is the most common spelling of a *label* in this crate, so including it would make every
/// button, menu item and tab announce its caption twice — once as its name and once as its value.
/// A field whose content genuinely is its value has a dedicated `value` property for that, and a
/// control that stores its content under `text` says so by overriding `Widget::accessible_value`.
/// The generic walk must not guess, because guessing here is indistinguishable from being wrong.
pub(crate) const VALUE_PROPERTY_NAMES: &[&str] = &["value", "progress", "rating", "level"];

/// The property a control answers "its label" with, or `None` when it has no label concept.
///
/// # Why this is a shared function and not a loop at each call site
///
/// The order of [`LABEL_PROPERTY_NAMES`] is a decision, and the three callers (the
/// constructor helper, the backend's `set_widget_text`/`get_widget_text` pair, and
/// `Widget::accessible_name`) each had their own loop. Spreading one decision over three
/// copies is how a control ends up written through one spelling and read through another;
/// the shared function makes the choice once and lets the widget it is asked about decide
/// which of the names it actually answers.
///
/// A control that answers *several* of the names is resolved by the list's order, so
/// `FloatingLabel` reports `label` while a `Button` reports `text`.
pub(crate) fn widget_label_property_name(
    properties: &dyn crate::widget::capability::WidgetProperties,
) -> Option<&'static str> {
    LABEL_PROPERTY_NAMES.iter().copied().find(|name| {
        matches!(properties.get(name), Ok(crate::widget::capability::CapabilityValue::String(_)))
    })
}

/// The property a control answers "its current value" with, or `None` when it has none.
///
/// # Why the value is read as text
///
/// Assistive technology receives a value as an announcement, not as a number to compute with, so
/// the property is rendered here rather than handed over as a `CapabilityValue`. That rendering is
/// the contract's own job: [`crate::widget::capability::coercion::capability_value_to_str`] knows
/// how each variant spells itself, so a slider reporting `0.5` and a rating reporting `3` are both
/// announced the way the control itself would report them, instead of through a second formatting
/// scheme invented here.
///
/// A property that errors is skipped rather than treated as a value: `UnknownProperty` means the
/// control does not have that concept, which is exactly the case this walk exists to fall past.
pub(crate) fn widget_value_property_name(
    properties: &dyn crate::widget::capability::WidgetProperties,
) -> Option<&'static str> {
    VALUE_PROPERTY_NAMES.iter().copied().find(|name| properties.get(name).is_ok())
}

/// The kind used for a toggle button when the stripped profiles compile that
/// variant out.
///
/// A stripped profile drops `WidgetKind::ToggleButton`, but the trait method that
/// creates one must stay callable in **every** profile (the public API surface must
/// not vary — principle #53). Substituting the closest always-present kind keeps
/// the signature stable; the creation then reports `0` because the profile has no
/// constructor registry at all, which is the honest outcome.
pub(crate) const TOGGLE_BUTTON_KIND: crate::widget::WidgetKind = {
    #[cfg(widgets_unstripped)]
    {
        crate::widget::WidgetKind::ToggleButton
    }
    #[cfg(not(widgets_unstripped))]
    {
        crate::widget::WidgetKind::Button
    }
};

/// The kind used for a menu when the stripped profiles compile that variant out.
///
/// Same reasoning as [`TOGGLE_BUTTON_KIND`]: the trait method must stay callable in
/// every profile, so the variant is substituted rather than the signature gated.
#[allow(dead_code)]
pub(crate) const MENU_KIND: crate::widget::WidgetKind = {
    #[cfg(widgets_unstripped)]
    {
        crate::widget::WidgetKind::Menu
    }
    #[cfg(not(widgets_unstripped))]
    {
        crate::widget::WidgetKind::Panel
    }
};

/// The `WidgetFactory` name for a [`crate::widget::WidgetKind`].
///
/// Split out of the creation helper so both the mount path and any future caller
/// resolve a kind to its constructor name the same way. The mapping lives here
/// rather than on `WidgetKind` because it belongs to the factory's vocabulary, not
/// to the enum. Alias variants are resolved by the callee.
///
/// Gated `full_widgets`, not `widgets_unstripped`, because the mount path below is
/// the only caller and it is gated `full_widgets`. `widgets_unstripped` is the
/// strictly wider predicate, so a build with no device profile (e.g.
/// `--features gpu`) used to compile this function and never call it.
#[cfg(full_widgets)]
pub(crate) fn kind_factory_name(kind: crate::widget::WidgetKind) -> Option<&'static str> {
    crate::widget::capability::factory_name_for_kind(kind)
}
