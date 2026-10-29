// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

// Capability constructors for the `base` widgets.
//
// The property arrays these constructors reference are declared in
// `properties_base.in.rs` and expanded by the parent module. `use super::*` brings
// those arrays and the shared imports into scope; `events_of!` is visible through the
// parent module's textual macro scope.

use super::*;

// ── Widget capability functions ─────────────────────────────────
// ── Dialog widgets ──────────────────────────────────────────
// ── Container widgets ───────────────────────────────────────
// ── Input / text widgets ────────────────────────────────────
// ── Web widgets ─────────────────────────────────────────────
// ── Advanced widgets ────────────────────────────────────────
pub(crate) fn button_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Button,
        canonical_name: "button",
        aliases: &["pushbutton", "btn"],
        properties: BUTTON_PROPERTIES,
        events: events_of!("button"),
        commands: &["click"],
    }
}

pub(crate) fn label_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Label,
        canonical_name: "label",
        aliases: &["text_label"],
        properties: LABEL_PROPERTIES,
        events: events_of!("label"),
        commands: &["set_text", "set_alignment"],
    }
}

pub(crate) fn check_box_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::CheckBox,
        canonical_name: "check_box",
        // Deliberately empty. BLUE20 §1.7 recorded this as a gap because
        // `Platform::create_checkbox` had no literal match here — but `WidgetFactory`
        // stores every name under `normalize_key`, which strips `_`/`-`/spaces, so
        // `"checkbox"` already resolves to `check_box` and adding the alias would be a
        // no-op that breaks `capability_alias_hygiene_test::no_alias_is_inert_under_normalisation`.
        // The gap was in the audit script's criterion, not in this table (rule #110).
        aliases: &[],
        properties: CHECK_BOX_PROPERTIES,
        events: events_of!("check_box"),
        commands: &["set_checked", "toggle", "set_state"],
    }
}

pub(crate) fn radio_button_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::RadioButton,
        canonical_name: "radio_button",
        aliases: &[],
        properties: RADIO_BUTTON_PROPERTIES,
        events: events_of!("radio_button"),
        commands: &["set_checked", "set_group_id"],
    }
}

// ── Always-available widget capability functions ──────────────────
#[cfg(not(alloc_frugal))]
pub(crate) fn toggle_button_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ToggleButton,
        canonical_name: "toggle_button",
        aliases: &["toggle"],
        properties: TOGGLE_BUTTON_PROPERTIES,
        events: events_of!("toggle_button"),
        commands: &["set_checked", "toggle", "set_text"],
    }
}

pub(crate) fn line_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Line,
        canonical_name: "line",
        aliases: &["line_widget"],
        properties: LINE_PROPERTIES,
        events: &[],
        commands: &["set_orientation"],
    }
}
