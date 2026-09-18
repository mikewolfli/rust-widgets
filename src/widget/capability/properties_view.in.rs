// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

// Hand-maintained const arrays for widget properties.
//
// This header used to claim the file was auto-generated from `properties.rs` with a
// "DO NOT EDIT DIRECTLY" instruction, but no generator exists in `tools/` or
// `scripts/` that writes `.in.rs` files. The only script that touches them
// (`generate_control_route_matrix.py`) *reads* the `create_widgets_*.in.rs` set to
// emit a report, which is a different family. Edit these directly.

macro_rules! impl_properties_view {
    () => {
        #[cfg(not(alloc_frugal))]
        pub(crate) const LIST_VIEW_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("has_model", PropertyValueKind::Bool, true, false),
            PropertySchema::new("row_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("focused_row", PropertyValueKind::UInt, true, true),
            PropertySchema::enumerated(
                "selection_mode",
                true,
                true,
                &["single", "multi", "extended"],
            ),
            PropertySchema::enumerated(
                "view_mode",
                true,
                true,
                &["list", "icon", "details", "thumbnails"],
            ),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const TREE_VIEW_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("has_model", PropertyValueKind::Bool, true, false),
            PropertySchema::new("node_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("focused_node", PropertyValueKind::UInt, true, true),
            PropertySchema::new("selected_node", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const TABLE_WIDGET_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("has_model", PropertyValueKind::Bool, true, false),
            PropertySchema::new("has_delegate", PropertyValueKind::Bool, true, false),
            PropertySchema::new("row_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("column_count", PropertyValueKind::UInt, true, false),
            PropertySchema::enumerated(
                "selection_mode",
                true,
                true,
                &["single", "multi", "extended"],
            ),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        // `GridTableWidget` is the feature-rich virtualised table (grid lines,
        // headers, per-column widths, sorting). It was implemented and exported but
        // had no capability at all, so it could not be created by name. It shares
        // `WidgetKind::GridTable` with nothing else, hence its own row in the
        // type-based tie-break table is only needed once the kind has a second entry.
        // `NumberPicker` is a scrolling digit wheel: `value` is snapped to a grid
        // anchored at `minimum`, so both bounds stay selectable.
        // `OtpInput` is a fixed-length code entry: `length` and `masked` describe
        // the display, `value` is the real code, and the two derived names let a
        // caller drive a "verify" button without re-deriving them.
        #[cfg(not(alloc_frugal))]
        pub(crate) const OTP_INPUT_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("value", PropertyValueKind::String, true, true),
            PropertySchema::new("length", PropertyValueKind::UInt, true, true),
            PropertySchema::new("masked", PropertyValueKind::Bool, true, true),
            PropertySchema::new("separator", PropertyValueKind::String, true, true),
            PropertySchema::new("focused_index", PropertyValueKind::UInt, true, false),
            PropertySchema::new("is_complete", PropertyValueKind::Bool, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        // `Banner` is a persistent notice: unlike a snackbar it has no timer, so
        // `dismissed` is read-only state the user drives by clicking.
        #[cfg(not(alloc_frugal))]
        pub(crate) const BANNER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("text", PropertyValueKind::String, true, true),
            PropertySchema::enumerated(
                "severity",
                true,
                true,
                &["info", "success", "warning", "error"],
            ),
            PropertySchema::new("dismissible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("dismissed", PropertyValueKind::Bool, true, false),
            PropertySchema::new("action_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        // `Pagination` holds no content, only the index into somebody else's data:
        // every number that defines that index is writable, and the two derived from
        // it are read-only.
        #[cfg(not(alloc_frugal))]
        pub(crate) const PAGINATION_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("total", PropertyValueKind::UInt, true, true),
            PropertySchema::new("page_size", PropertyValueKind::UInt, true, true),
            PropertySchema::new("page", PropertyValueKind::UInt, true, true),
            PropertySchema::new("page_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("last_page", PropertyValueKind::UInt, true, false),
            PropertySchema::new("sibling_count", PropertyValueKind::UInt, true, true),
            PropertySchema::new("show_nav_buttons", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const NUMBER_PICKER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("value", PropertyValueKind::Int, true, true),
            PropertySchema::new("minimum", PropertyValueKind::Int, true, true),
            PropertySchema::new("maximum", PropertyValueKind::Int, true, true),
            PropertySchema::new("step", PropertyValueKind::Int, true, true),
            PropertySchema::new("wrap", PropertyValueKind::Bool, true, true),
            PropertySchema::new("suffix", PropertyValueKind::String, true, true),
            PropertySchema::new("row_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("selected_row", PropertyValueKind::UInt, true, false),
            PropertySchema::new("row_height", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const GRID_TABLE_WIDGET_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("has_data_source", PropertyValueKind::Bool, true, false),
            PropertySchema::new("row_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("column_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("scroll_row", PropertyValueKind::UInt, true, true),
            PropertySchema::new("scroll_column", PropertyValueKind::UInt, true, true),
            PropertySchema::new("row_height", PropertyValueKind::UInt, true, true),
            // `GridTable` selects *cells*, not rows, so its tokens are a different set
            // from the list/table `selection_mode` despite sharing the property name.
            PropertySchema::enumerated(
                "selection_mode",
                true,
                true,
                &["none", "cell", "row", "column"],
            ),
            PropertySchema::new("sort_spec_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("selected_cell", PropertyValueKind::String, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const DATA_GRID_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("has_data_source", PropertyValueKind::Bool, true, false),
            PropertySchema::new("row_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("column_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("scroll_row", PropertyValueKind::UInt, true, true),
            PropertySchema::new("scroll_column", PropertyValueKind::UInt, true, true),
            PropertySchema::new("row_height", PropertyValueKind::UInt, true, true),
            PropertySchema::new("column_width", PropertyValueKind::UInt, true, true),
            PropertySchema::new("frozen_columns", PropertyValueKind::UInt, true, true),
            PropertySchema::new("sort_spec_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("filter_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("sort_specs", PropertyValueKind::String, true, true),
            PropertySchema::new("filters", PropertyValueKind::String, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const TREE_TABLE_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("has_model", PropertyValueKind::Bool, true, false),
            PropertySchema::new("row_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("column_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("selected_row", PropertyValueKind::UInt, true, true),
            PropertySchema::new("row_height", PropertyValueKind::UInt, true, true),
            PropertySchema::new("column_width", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const VIRTUAL_TABLE_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("has_data_source", PropertyValueKind::Bool, true, false),
            PropertySchema::new("row_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("column_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("scroll_row", PropertyValueKind::UInt, true, true),
            PropertySchema::new("scroll_column", PropertyValueKind::UInt, true, true),
            PropertySchema::new("row_height", PropertyValueKind::UInt, true, true),
            PropertySchema::new("column_width", PropertyValueKind::UInt, true, true),
            PropertySchema::new("overscan_rows", PropertyValueKind::UInt, true, true),
            PropertySchema::new("overscan_columns", PropertyValueKind::UInt, true, true),
            PropertySchema::new("visible_window", PropertyValueKind::String, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const VIRTUAL_LIST_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("has_data_source", PropertyValueKind::Bool, true, false),
            PropertySchema::new("row_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("scroll_row", PropertyValueKind::UInt, true, true),
            PropertySchema::new("row_height", PropertyValueKind::UInt, true, true),
            PropertySchema::new("overscan", PropertyValueKind::UInt, true, true),
            PropertySchema::new("selected_row", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        pub(crate) const IMAGE_VIEW_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("scaled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const PROPERTIES_PANEL_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("property_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const IMAGE_GALLERY_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("current_index", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const PROPERTY_GRID_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("property_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("selected_index", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        // `Toast` is one transient message. `ToastStack` is the container that
        // queues them, so the properties here describe a single notification rather
        // than a collection: there is no count and no selection.
        #[cfg(not(alloc_frugal))]
        pub(crate) const TOAST_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("message", PropertyValueKind::String, true, true),
            PropertySchema::enumerated(
                "level",
                true,
                true,
                &["info", "success", "warning", "error"],
            ),
            // A ttl here is a *hint for the host*: this control does not schedule
            // its own expiry, so the value is data the host reads, not behaviour it
            // triggers. That is why it is writable but has no `expired` sibling.
            PropertySchema::new("ttl_ms", PropertyValueKind::UInt, true, true),
            PropertySchema::new("dismissible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        // `SplashScreen` is dismissed by the program, not the user, so it publishes
        // no `dismissed` state: `progress` is the only live value, and `Null` is the
        // documented way to return it to indeterminate.
        #[cfg(not(alloc_frugal))]
        pub(crate) const SPLASH_SCREEN_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("title", PropertyValueKind::String, true, true),
            PropertySchema::new("subtitle", PropertyValueKind::String, true, true),
            PropertySchema::new("progress", PropertyValueKind::Float, true, true),
            PropertySchema::new("skippable", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        // `ColorPicker` split out of `ColorDialog` in BLUE16 phase E-6. The property
        // set is identical to the dialog's because it describes the same picker; what
        // differs is the host, so the schemas stay in step deliberately.
        #[cfg(not(alloc_frugal))]
        pub(crate) const COLOR_DIALOG_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("current_color", PropertyValueKind::String, true, false),
            PropertySchema::new("modal", PropertyValueKind::Bool, true, true),
            PropertySchema::new("options_alpha", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];
    };
}
