// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

// Capability constructors for the `advanced` widgets.
//
// The property arrays these constructors reference are declared in
// `properties_advanced.in.rs` and expanded by the parent module. `use super::*` brings
// those arrays and the shared imports into scope; `events_of!` is visible through the
// parent module's textual macro scope.

use super::*;

#[cfg(not(alloc_frugal))]
pub(crate) fn tab_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TabBar,
        canonical_name: "tab_bar",
        aliases: &[],
        properties: TAB_BAR_PROPERTIES,
        events: events_of!("tab_bar"),
        // Audit 20261026: `add_tab`, `remove_tab` stay unpublished — each takes a payload the bare
        // command channel cannot carry. `set_tabs` is the published form of a whole-strip write:
        // its rows travel through the `tabs` records property (B26.03).
        commands: &["set_current_index", "set_closable", "set_movable", "set_tabs"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn calendar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Calendar,
        canonical_name: "calendar",
        aliases: &["date_calendar"],
        properties: CALENDAR_PROPERTIES,
        events: events_of!("calendar"),
        // Audit 20261026: `set_date_range` stays unpublished — the range is the writable
        // `minimum_date`/`maximum_date` pair, so a whole-range write is reachable through the
        // property route (principle #18).
        commands: &["set_selected_date", "set_first_day_of_week", "show_today"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn date_edit_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::DatePicker,
        canonical_name: "date_edit",
        // `date_picker` is the variant's own spelling (`WidgetKind::DatePicker`,
        // and `pub type DatePicker = DateEdit` in `src/widget/mod.rs`), so a caller
        // naming the control after the kind it is declared under must reach it. Its
        // two siblings already answer to theirs (`time_picker`, `date_time_picker`),
        // and the absence here made `factory.create("date_picker", ..)` return `None`
        // while `factory.create("time_picker", ..)` succeeded.
        aliases: &["date_picker"],
        properties: DATE_EDIT_PROPERTIES,
        events: events_of!("date_edit"),
        commands: &["set_date", "set_minimum_date", "set_maximum_date", "set_display_format"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn time_edit_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TimePicker,
        canonical_name: "time_edit",
        // `time_picker` is no longer an alias here: it is its own canonical row over the same
        // control (see `time_picker_capability`), and a name may not be both an alias and a
        // canonical spelling — the alias index would answer it twice.
        aliases: &["time_input"],
        properties: TIME_EDIT_PROPERTIES,
        events: events_of!("time_edit"),
        commands: &[
            "set_time",
            "set_minimum_time",
            "set_maximum_time",
            "set_display_format",
            "set_clock_popup",
        ],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn ribbon_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::RibbonBar,
        canonical_name: "ribbon_bar",
        aliases: &["ribbon"],
        properties: RIBBON_BAR_PROPERTIES,
        events: events_of!("ribbon_bar"),
        // Audit 20261026: `add_tab` stays unpublished — it takes a title the bare command channel
        // cannot carry; `tabs` is the published property and `set_tabs` its command form
        // (B26.03). `add_group`, `add_item`, `add_large_item` stay unpublished: they address a
        // position inside a tab/group nesting a flat property cannot express.
        commands: &["clear", "set_tabs"],
    }
}

// ── Web widget capabilities ──────────────────────────────────
/// The web view's capability.
///
/// # Why the canonical name is `web_engine_view`
///
/// `web_view` is one of the two things `WidgetKind::WebEngineView` is called, and it
/// was the canonical name until it became clear what that cost. Two capabilities
/// declare this kind — this one and `media_player` — and
/// [`WidgetFactory::capability_by_kind`] resolves a kind by matching the capability's
/// `canonical_name` against **the kind's own name**. With the canonical name spelled
/// `web_view`, that match failed for `WebEngineView`, the lookup fell through to
/// `indices[0]`, and `create_web_view(..)` — and the C ABI function behind it — built
/// a **`MediaPlayer`** while returning a valid id.
///
/// Naming the entry after the kind is the same fix as `table` / `panel` / `data_view`:
/// the kind's own name is the only key that cannot be reordered out from under the
/// lookup. `web_view` stays reachable as an alias, so `factory.create("web_view", ..)`
/// and `WebView` (the `pub type` for this same control) are unaffected.
#[cfg(not(alloc_frugal))]
// ── Advanced widget capabilities ─────────────────────────────
#[cfg(not(alloc_frugal))]
pub(crate) fn pie_menu_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::PieMenu,
        canonical_name: "pie_menu",
        aliases: &["radial_menu"],
        properties: PIE_MENU_PROPERTIES,
        events: events_of!("pie_menu"),
        // Audit 20261026: `add_item`, `remove_item` stay unpublished — each takes a payload the bare
        // command channel cannot carry, and a whole-list write covers the same ground. `set_items`
        // is the published form of that write: its value travels through the `items` records
        // property (B26.03).
        commands: &["set_radius", "set_current_index", "set_items"],
    }
}

// ── Always-available widget property arrays ────────────────────────
// These are NOT gated behind `

pub(crate) fn date_time_edit_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::DateTimePicker,
        canonical_name: "date_time_edit",
        aliases: &["datetimepicker", "date_time_picker"],
        properties: DATE_TIME_EDIT_PROPERTIES,
        events: events_of!("date_time_edit"),
        commands: &["set_datetime", "set_display_format", "set_calendar_popup"],
    }
}

/// The chord recorder (`KeySequenceEdit`), published separately from `ShortcutEditor`.
///
/// The two controls capture different things: `ShortcutEditor` edits a command id plus a list of
/// keys, while `KeySequenceEdit` records one chord. Its published names are the ones that control
/// emits, so a subscriber is not told about an event the other control would have to fake.
#[cfg(not(alloc_frugal))]
pub(crate) fn key_sequence_editor_capability() -> WidgetCapability {
    WidgetCapability {
        // `KeySequenceEdit`'s base widget reports the line-edit kind; the capability follows the
        // control it describes rather than inventing a second kind for a text-shaped editor.
        kind: WidgetKind::LineEdit,
        canonical_name: "key_sequence_editor",
        aliases: &["key_sequence_edit"],
        properties: KEY_SEQUENCE_EDITOR_PROPERTIES,
        events: events_of!("key_sequence_editor"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `time_picker` — the kind's own spelling of [`time_edit`](Self::time_edit).
///
/// # Why a second name over the same control
///
/// `WidgetKind::TimePicker` is the variant the time field reports, and `time_edit` was already
/// published with `time_picker` as an alias. A canonical row makes the kind's own spelling a
/// first-class name — the same shape `table_widget` / `table` and `splitter` / `split_layout`
/// use — without introducing a second time field, which would give one concept two
/// implementations (rules #54 and #101).
pub(crate) fn time_picker_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TimePicker,
        canonical_name: "time_picker",
        aliases: &["time_field"],
        properties: TIME_EDIT_PROPERTIES,
        events: events_of!("time_edit"),
        commands: &[
            "set_time",
            "set_minimum_time",
            "set_maximum_time",
            "set_display_format",
            "set_clock_popup",
        ],
    }
}

// ── macOS (desktop) component set ───────────────────────────────
//
// Five controls that carry the desktop platform's own shapes beside the touch-styled
// `cupertino` set. Each one declares the kind of the control it refines — a mac segmented
// picker is a segmented picker, a source list is a list view — because the kind is what the
// theme layer and the accessibility role classify by. The capability name is what the factory,
// the design manifest and the property contract address; the kind is shared on purpose and
// resolved by the concrete-type tie-break in `WidgetFactory::widget_matches_capability`.
