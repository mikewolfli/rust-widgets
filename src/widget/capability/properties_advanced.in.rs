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
            PropertySchema::new("tab_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("current_index", PropertyValueKind::UInt, true, true),
            PropertySchema::new("closable", PropertyValueKind::Bool, true, true),
            PropertySchema::new("movable", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tab_min_width", PropertyValueKind::UInt, true, true),
            PropertySchema::new("tab_max_width", PropertyValueKind::UInt, true, true),
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
            PropertySchema::enumerated("first_day_of_week", true, true, &["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"]),
            PropertySchema::new("grid_visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("navigation_bar_visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("horizontal_header_visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("vertical_header_visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("date_format", PropertyValueKind::String, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const DATE_EDIT_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("date", PropertyValueKind::String, true, true),
            PropertySchema::new("minimum_date", PropertyValueKind::String, true, true),
            PropertySchema::new("maximum_date", PropertyValueKind::String, true, true),
            PropertySchema::new("display_format", PropertyValueKind::String, true, true),
            PropertySchema::new("calendar_popup", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const TIME_EDIT_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("time", PropertyValueKind::String, true, true),
            PropertySchema::new("minimum_time", PropertyValueKind::String, true, true),
            PropertySchema::new("maximum_time", PropertyValueKind::String, true, true),
            PropertySchema::new("display_format", PropertyValueKind::String, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const RIBBON_BAR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("tab_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("current_tab", PropertyValueKind::UInt, true, true),
            PropertySchema::new("expanded", PropertyValueKind::Bool, true, true),
            PropertySchema::new("minimized", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const PIE_MENU_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("item_count", PropertyValueKind::UInt, false, false),
            PropertySchema::new("radius", PropertyValueKind::Float, false, false),
            PropertySchema::new("inner_radius", PropertyValueKind::Float, false, false),
            PropertySchema::new("current_index", PropertyValueKind::UInt, false, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const DATE_TIME_EDIT_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("datetime", PropertyValueKind::String, false, false),
            PropertySchema::new("display_format", PropertyValueKind::String, false, false),
            PropertySchema::new("calendar_popup", PropertyValueKind::Bool, false, false),
            PropertySchema::new("minimum", PropertyValueKind::String, false, false),
            PropertySchema::new("maximum", PropertyValueKind::String, false, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];
    };
}
