// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

// Hand-maintained const arrays for widget properties.
//
// This header used to claim the file was auto-generated from `properties.rs` with a
// "DO NOT EDIT DIRECTLY" instruction, but no generator exists in `tools/` or
// `scripts/` that writes `.in.rs` files. The only script that touches them
// (`generate_control_route_matrix.py`) *reads* the `create_widgets_*.in.rs` set to
// emit a report, which is a different family. Edit these directly.

macro_rules! impl_properties_advanced {
    () => {
        #[cfg(not(alloc_frugal))]
        pub(crate) const TAB_BAR_PROPERTIES: &[PropertySchema] = &[
            // The whole strip as records (BLUE26 B26.09): `add_tab`/`remove_tab` take arguments the
            // property API cannot carry, so the list itself is the property and `set_tabs` is its
            // published command form.
            PropertySchema::records(
                "tabs",
                true,
                true,
                &crate::widget::advanced_widgets::tab_bar::TAB_BAR_TAB_SCHEMA,
            ),
            PropertySchema::new("tab_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("current_index", PropertyValueKind::UInt, true, true),
            PropertySchema::new("closable", PropertyValueKind::Bool, true, true),
            PropertySchema::new("movable", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tab_min_width", PropertyValueKind::UInt, true, true),
            PropertySchema::new("tab_max_width", PropertyValueKind::UInt, true, true),
            // Only the horizontal strip reads this: a `West`/`East` strip is not mirrored, because
            // the side a strip is attached to is a layout decision rather than a reading one.
            PropertySchema::enumerated("direction", true, true, &["ltr", "rtl"]),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const CALENDAR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("selected_date", PropertyValueKind::String, true, true),
            PropertySchema::new("minimum_date", PropertyValueKind::String, true, true),
            PropertySchema::new("maximum_date", PropertyValueKind::String, true, true),
            // `weekday_to_str` reports the three-letter forms, so these are the tokens that
            // describe what `get` returns. The long spellings are still *accepted* by
            // `expect_weekday` (a writer may take aliases), which is why the mismatch was only
            // visible from the reader's side.
            PropertySchema::enumerated(
                "first_day_of_week",
                true,
                true,
                &["mon", "tue", "wed", "thu", "fri", "sat", "sun"],
            ),
            PropertySchema::new("grid_visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("navigation_bar_visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("horizontal_header_visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("vertical_header_visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("date_format", PropertyValueKind::String, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("show_week_numbers", PropertyValueKind::Bool, true, true),
            PropertySchema::enumerated("selection_mode", true, true, &["single", "multi", "range"]),
            PropertySchema::new("selected_dates", PropertyValueKind::StringList, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const DATE_EDIT_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("date", PropertyValueKind::String, true, true),
            PropertySchema::new("minimum_date", PropertyValueKind::String, true, true),
            PropertySchema::new("maximum_date", PropertyValueKind::String, true, true),
            PropertySchema::new("display_format", PropertyValueKind::String, true, true),
            PropertySchema::new("calendar_popup", PropertyValueKind::Bool, true, true),
            PropertySchema::enumerated("alignment", true, true, &["left", "centre", "right"]),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::enumerated(
                "calendar_first_day",
                true,
                true,
                &["mon", "tue", "wed", "thu", "fri", "sat", "sun"],
            ),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const TIME_EDIT_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("time", PropertyValueKind::String, true, true),
            PropertySchema::new("minimum_time", PropertyValueKind::String, true, true),
            PropertySchema::new("maximum_time", PropertyValueKind::String, true, true),
            PropertySchema::new("display_format", PropertyValueKind::String, true, true),
            PropertySchema::new("clock_popup", PropertyValueKind::Bool, true, true),
            PropertySchema::enumerated("alignment", true, true, &["left", "centre", "right"]),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::enumerated("time_format", true, true, &["12h", "24h"]),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const RIBBON_BAR_PROPERTIES: &[PropertySchema] = &[
            // The tab titles as a `StringList` (BLUE26 B26.09): `add_tab` takes a title the
            // property API cannot carry, so the list itself is the property. A title list cannot
            // express groups or items, which stay on the control's own API.
            PropertySchema::new("tabs", PropertyValueKind::StringList, true, true),
            PropertySchema::new("tab_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("current_tab", PropertyValueKind::UInt, true, true),
            PropertySchema::new("expanded", PropertyValueKind::Bool, true, true),
            PropertySchema::new("minimized", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("quick_access_items", PropertyValueKind::StringList, true, true),
            PropertySchema::new("quick_access_overflow_menu", PropertyValueKind::Bool, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const PIE_MENU_PROPERTIES: &[PropertySchema] = &[
            // The rows are published so a document can read each item's identity and typed icon, and
            // **written** through the same arm: a write rebuilds the flat top-level ring through
            // the control's own `add_existing_item`, which recomputes the slice angles and advances
            // the id counter past a pinned id. A row cannot carry a child ring (a record field has
            // no nested-list kind), so submenu rings are outside this channel.
            PropertySchema::records(
                "items",
                true,
                true,
                &crate::widget::advanced_widgets::pie_menu::PIE_MENU_ITEM_SCHEMA,
            ),
            PropertySchema::new("item_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("radius", PropertyValueKind::Float, true, true),
            PropertySchema::new("inner_radius", PropertyValueKind::Float, true, true),
            PropertySchema::new("current_index", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const DATE_TIME_EDIT_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("datetime", PropertyValueKind::String, true, true),
            PropertySchema::new("display_format", PropertyValueKind::String, true, true),
            PropertySchema::new("calendar_popup", PropertyValueKind::Bool, true, true),
            PropertySchema::enumerated("alignment", true, true, &["left", "centre", "right"]),
            PropertySchema::new("minimum", PropertyValueKind::String, false, false),
            PropertySchema::new("maximum", PropertyValueKind::String, false, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::enumerated(
                "calendar_first_day",
                true,
                true,
                &["mon", "tue", "wed", "thu", "fri", "sat", "sun"],
            ),
        ];
        /// The base contract `KeySequenceEdit` publishes.
        ///
        /// The control's value is edited through its own API, so the capability surface is exactly the
        /// four names every widget inherits.
        #[cfg(not(alloc_frugal))]
        pub(crate) const KEY_SEQUENCE_EDITOR_PROPERTIES: &[PropertySchema] = &[
            // ADV-07: the recorded chord, the recording switch and the history flags.
            PropertySchema::new("key_sequence", PropertyValueKind::String, true, true),
            PropertySchema::new("recording", PropertyValueKind::Bool, true, true),
            PropertySchema::new("can_undo", PropertyValueKind::Bool, true, false),
            PropertySchema::new("can_redo", PropertyValueKind::Bool, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];
    };
}
