// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

// Hand-maintained const arrays for widget properties.
//
// This header used to claim the file was auto-generated from `properties.rs` with a
// "DO NOT EDIT DIRECTLY" instruction, but no generator exists in `tools/` or
// `scripts/` that writes `.in.rs` files. The only script that touches them
// (`generate_control_route_matrix.py`) *reads* the `create_widgets_*.in.rs` set to
// emit a report, which is a different family. Edit these directly.

macro_rules! impl_properties_other {
    () => {
        pub(crate) const WINDOW_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("title", PropertyValueKind::String, true, true),
            PropertySchema::new("title_bar_height", PropertyValueKind::UInt, true, true),
            PropertySchema::new("close_button_size", PropertyValueKind::UInt, true, true),
            PropertySchema::new("button_spacing", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const LCD_NUMBER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("value", PropertyValueKind::Float, true, true),
            PropertySchema::new("min_value", PropertyValueKind::Float, true, true),
            PropertySchema::new("max_value", PropertyValueKind::Float, true, true),
            PropertySchema::new("num_digits", PropertyValueKind::Int, true, true),
            PropertySchema::new("small_decimal_point", PropertyValueKind::Bool, true, true),
            // `lcd_mode_to_str` returns `hex`/`dec`/`oct`/`bin`; the writer also accepts the
            // long forms, so the published list must be the reader's four.
            PropertySchema::enumerated("mode", true, true, &["hex", "dec", "oct", "bin"]),
            PropertySchema::enumerated("segment_style", true, true, &["outline", "filled", "flat"]),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const CODE_EDITOR_PROPERTIES: &[PropertySchema] = &[
            // The diagnostic markers as records (BLUE26 B26.09): `set_markers` takes a list the
            // property API could not carry, so the list itself is the property.
            PropertySchema::records(
                "markers",
                true,
                true,
                &crate::widget::special_widgets::code_editor::DIAGNOSTIC_MARKER_SCHEMA,
            ),
            PropertySchema::new("text", PropertyValueKind::String, true, true),
            PropertySchema::new("line_count", PropertyValueKind::UInt, true, false),
            // The caret is writable state: a document may place it, and each component's
            // setter supplies the other live one to the control's own `set_cursor`.
            PropertySchema::new("cursor_line", PropertyValueKind::UInt, true, true),
            PropertySchema::new("cursor_column", PropertyValueKind::UInt, true, true),
            PropertySchema::new("marker_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("undo_stack_limit", PropertyValueKind::UInt, true, true),
            PropertySchema::new("undo_depth", PropertyValueKind::UInt, true, false),
            PropertySchema::new("goto_bar_visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("breakpoint_gutter", PropertyValueKind::Bool, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const GANTT_WIDGET_PROPERTIES: &[PropertySchema] = &[
            // The whole schedule as records (B26.09). Dependencies travel as `<id>:<fts|sts|ftf>`
            // tokens because a record field cannot hold the nested edge structure; `set_tasks` is
            // the payload-free spelling of the write (B26.03).
            PropertySchema::records(
                "tasks",
                true,
                true,
                &crate::widget::chart_widgets::gantt_widget::GANTT_TASK_SCHEMA,
            ),
            PropertySchema::new("task_count", PropertyValueKind::UInt, true, false),
            // The selection is reported only. The control's selection entry point is the Rust-side
            // `select_index`; publishing `select_task` would need a writable id property as its
            // payload channel, which would be a second spelling of the same action (B26.03).
            PropertySchema::new("selected_id", PropertyValueKind::String, true, false),
            // `set_viewport` is never published: no property is named `viewport`, and the two
            // bounds travel through these writable scalars — the same reason `mini_chart.set_range`
            // stays unpublished.
            PropertySchema::new("viewport_start", PropertyValueKind::Int, true, true),
            PropertySchema::new("viewport_end", PropertyValueKind::Int, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("row_height", PropertyValueKind::UInt, true, true),
            PropertySchema::new("show_critical_path", PropertyValueKind::Bool, true, true),
            PropertySchema::new("show_today", PropertyValueKind::Bool, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const TERMINAL_VIEW_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("output_line_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("input_line", PropertyValueKind::String, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("scrollback_lines", PropertyValueKind::UInt, true, true),
            PropertySchema::new("wrap_output", PropertyValueKind::Bool, true, true),
            PropertySchema::new("show_scrollbar", PropertyValueKind::Bool, true, true),
            PropertySchema::enumerated("caret_style", true, true, &["line", "block", "underline"]),
            PropertySchema::new("caret_blink_enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("prompt", PropertyValueKind::String, true, true),
            PropertySchema::enumerated("selection_mode", true, true, &["lines", "characters"]),
            PropertySchema::new("ansi_palette", PropertyValueKind::NumberList, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const SNACKBAR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("message", PropertyValueKind::String, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("action_label", PropertyValueKind::String, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("queued_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("max_queued", PropertyValueKind::UInt, true, true),
            PropertySchema::enumerated("anchor", true, true, &["top", "center", "bottom"]),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const MAP_VIEW_PROPERTIES: &[PropertySchema] = &[
            // The marker list as records (BLUE26 B26.09): `set_markers` takes a list the property
            // API could not carry, so the list itself is the property.
            PropertySchema::records(
                "markers",
                true,
                true,
                &crate::widget::special_widgets::map_view::MAP_MARKER_SCHEMA,
            ),
            PropertySchema::new("center_x", PropertyValueKind::Float, true, true),
            PropertySchema::new("center_y", PropertyValueKind::Float, true, true),
            PropertySchema::new("zoom", PropertyValueKind::Float, true, true),
            PropertySchema::new("marker_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("selected_marker_id", PropertyValueKind::String, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::enumerated("projection", true, true, &["linear", "mercator"]),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const MEDIA_PLAYER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("source", PropertyValueKind::String, true, true),
            PropertySchema::new("playing", PropertyValueKind::Bool, true, true),
            PropertySchema::new("duration_ms", PropertyValueKind::UInt, true, false),
            PropertySchema::new("position_ms", PropertyValueKind::UInt, true, true),
            PropertySchema::new("volume", PropertyValueKind::UInt, true, true),
            PropertySchema::new("muted", PropertyValueKind::Bool, true, true),
            PropertySchema::new("fullscreen", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("playlist_index", PropertyValueKind::UInt, true, true),
            PropertySchema::new("track_count", PropertyValueKind::UInt, true, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const BREADCRUMB_PROPERTIES: &[PropertySchema] = &[
            // The full path is a *property*, not only the `set_segments` command, so a document can
            // spell the trail and a designer can edit it. Each row is a record because a segment has
            // an identity (the id `segment_activated` reports) as well as a label.
            PropertySchema::records(
                "segments",
                true,
                true,
                &crate::widget::special_widgets::breadcrumb::BREADCRUMB_SEGMENT_SCHEMA,
            ),
            PropertySchema::new("segment_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("selected_index", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("collapsed_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("separator", PropertyValueKind::String, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const SIGNATURE_PAD_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("stroke_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("stroke_width", PropertyValueKind::UInt, true, true),
            PropertySchema::new("stroke_color", PropertyValueKind::Color, true, true),
            PropertySchema::new("min_point_distance", PropertyValueKind::Float, true, true),
            // The time half of the sampling rule: a point is kept when it is far enough from
            // the last one **or** this long after it, and a wider gap is filled by
            // interpolation. Without it a fast stroke was recorded once per input event and
            // drawn as a polygon.
            PropertySchema::new("min_point_interval_ms", PropertyValueKind::UInt, true, true),
            // D09-POINTER-01: whether tip pressure modulates the drawn stroke width, plus the
            // retained pen datum (pressure and tilt) of the most recent pointer event. The datum
            // is read-only — writing it would fabricate a fact about the input device.
            PropertySchema::new("pressure_affects_width", PropertyValueKind::Bool, true, true),
            PropertySchema::new("last_pressure", PropertyValueKind::Float, true, false),
            PropertySchema::new("last_tilt_x", PropertyValueKind::Float, true, false),
            PropertySchema::new("last_tilt_y", PropertyValueKind::Float, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const DROP_ZONE_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("accepted_type", PropertyValueKind::String, true, true),
            // The five feedback states. Writable so a drag session can be driven without a live
            // pointer (a document, a test, an automated walkthrough).
            PropertySchema::enumerated(
                "state",
                true,
                true,
                &["idle", "hovering", "accepted", "rejected", "dropped"],
            ),
            // Derived from `state`: `true` exactly when the zone would take a drop.
            PropertySchema::new("hovered", PropertyValueKind::Bool, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("accepted_types", PropertyValueKind::StringList, true, true),
            PropertySchema::new("multi_file", PropertyValueKind::Bool, true, true),
            PropertySchema::new("hover_style", PropertyValueKind::String, true, true),
            PropertySchema::new("show_count_badge", PropertyValueKind::Bool, true, true),
            PropertySchema::new("dropped_count", PropertyValueKind::UInt, true, false),
        ];

        // `ChartWidget`'s property contract. This is the single definition of the
        // constant; it was previously written out twice in this file, which stayed
        // invisible only while both copies were byte-identical.
        #[cfg(not(alloc_frugal))]
        pub(crate) const CHART_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::enumerated(
                "chart_type",
                true,
                true,
                &[
                    "bar",
                    "line",
                    "area",
                    "pie",
                    "scatter",
                    "waterfall",
                    "funnel",
                    "candlestick",
                    "box_plot",
                ],
            ),
            PropertySchema::new("point_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("label_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("series_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const SPLIT_BUTTON_PROPERTIES: &[PropertySchema] = &[
            // The action list as records (BLUE26 B26.09): `add_action` takes an action the
            // property API cannot carry, so the list itself is the property.
            PropertySchema::records(
                "actions",
                true,
                true,
                &crate::widget::special_widgets::split_button::SPLIT_ACTION_SCHEMA,
            ),
            PropertySchema::new("text", PropertyValueKind::String, true, true),
            PropertySchema::new("action_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("menu_open", PropertyValueKind::Bool, true, true),
            PropertySchema::new("row_height", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::enumerated("popup_direction", true, true, &["down", "up", "auto"]),
            PropertySchema::new("max_menu_height", PropertyValueKind::UInt, true, true),
            PropertySchema::new("menu_scroll", PropertyValueKind::UInt, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const SEGMENTED_CONTROL_PROPERTIES: &[PropertySchema] = &[
            // The item list is a *property*, not only a command (BLUE26 B26.09): a document's
            // `"items": [...]` had nowhere to go while `set_items` could only be called with a Rust
            // argument the property API could not carry.
            PropertySchema::records(
                "items",
                true,
                true,
                &crate::widget::special_widgets::segmented_control::SEGMENT_ITEM_SCHEMA,
            ),
            PropertySchema::new("item_count", PropertyValueKind::UInt, true, false),
            // Read-only: a fresh instance has no selection, so the write-back default a
            // writable row must have cannot exist here (see the widget's `set` doc).
            PropertySchema::new("selected_index", PropertyValueKind::UInt, true, false),
            PropertySchema::new("selected_id", PropertyValueKind::String, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("size_to_fit", PropertyValueKind::Bool, true, true),
            PropertySchema::new("min_segment_width", PropertyValueKind::UInt, true, true),
            PropertySchema::new("multi_select", PropertyValueKind::Bool, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const CHIP_PROPERTIES: &[PropertySchema] = &[
            // The item list is a *property*, not only a command (BLUE26 B26.09). It was reachable
            // only as `set_items`, whose signature takes the items as an argument the property API
            // could not carry — so a JSON document's `"items": [...]` had nowhere to go and was
            // dropped, and a designer had no way to see or edit the chips at all.
            PropertySchema::records(
                "items",
                true,
                true,
                &crate::widget::special_widgets::chip::CHIP_ITEM_SCHEMA,
            ),
            PropertySchema::new("item_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("multi_select", PropertyValueKind::Bool, true, true),
            PropertySchema::new("focused_index", PropertyValueKind::UInt, true, true),
            PropertySchema::new("selected_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        // `TimelineWidget` publishes the shared four plus its own viewport and row
        // metrics. `item_count` / `selected_index` / `viewport_start` / `viewport_end` /
        // `row_height` are all answered by its `WidgetProperties` impl; `selected_index`
        // is deliberately read-only because selecting is a command-shaped operation
        // (`select_index(i)` returns whether the index was in range) rather than an
        // assignment a caller can undo by writing a different number.
        #[cfg(not(alloc_frugal))]
        pub(crate) const TIMELINE_WIDGET_PROPERTIES: &[PropertySchema] = &[
            // The item list as records (BLUE26 B26.09): `set_items` takes a list the property API
            // cannot carry, so the list itself is the property.
            PropertySchema::records(
                "items",
                true,
                true,
                &crate::widget::special_widgets::timeline_widget::TIMELINE_ITEM_SCHEMA,
            ),
            PropertySchema::new("item_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("selected_index", PropertyValueKind::UInt, true, false),
            PropertySchema::new("viewport_start", PropertyValueKind::Int, true, true),
            PropertySchema::new("viewport_end", PropertyValueKind::Int, true, true),
            PropertySchema::new("row_height", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("show_time_labels", PropertyValueKind::Bool, true, true),
            PropertySchema::enumerated("lane_mode", true, true, &["row", "lane_packed"]),
        ];

        // `CommandPalette` is a queryable command list: the filtered result set,
        // not the raw entry list, is what a consumer can act on, so it publishes
        // the filter state **and the entry list itself** as records (BLUE26 B26.09) —
        // `set_entries` takes a list the property API could not carry.
        #[cfg(not(alloc_frugal))]
        pub(crate) const COMMAND_PALETTE_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::records(
                "entries",
                true,
                true,
                &crate::widget::special_widgets::command_palette::COMMAND_ENTRY_SCHEMA,
            ),
            PropertySchema::new("query", PropertyValueKind::String, true, true),
            PropertySchema::new("entry_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("filtered_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("highlighted_index", PropertyValueKind::UInt, true, false),
            PropertySchema::new("row_height", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("group_by_category", PropertyValueKind::Bool, true, true),
        ];

        // `NotificationCenter` publishes its unread state and selection, and the item list as
        // records (BLUE26 B26.09) — `push` / `set_read` take arguments the bare command channel
        // cannot carry, and a whole-list write covers both.
        #[cfg(not(alloc_frugal))]
        pub(crate) const NOTIFICATION_CENTER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::records(
                "items",
                true,
                true,
                &crate::widget::special_widgets::notification_center::NOTIFICATION_ITEM_SCHEMA,
            ),
            PropertySchema::new("item_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("unread_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("selected_index", PropertyValueKind::UInt, true, false),
            PropertySchema::new("row_height", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::enumerated(
                "level_filter",
                true,
                true,
                &["all", "info", "warning", "error"],
            ),
            PropertySchema::new("max_items", PropertyValueKind::UInt, true, true),
        ];

        // `DiffViewer` compares two snapshots. Both texts are readable and
        // writable (writing either recomputes the diff), and `change_count` /
        // `line_count` report the result. `selected_index` stays read-only for the
        // same reason as the timeline's.
        #[cfg(not(alloc_frugal))]
        pub(crate) const DIFF_VIEWER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("left_text", PropertyValueKind::String, true, true),
            PropertySchema::new("right_text", PropertyValueKind::String, true, true),
            PropertySchema::new("line_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("change_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("selected_index", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::enumerated("view_mode", true, true, &["side_by_side", "unified"]),
            PropertySchema::new("fold_unchanged", PropertyValueKind::Bool, true, true),
            PropertySchema::new("fold_threshold", PropertyValueKind::UInt, true, true),
            PropertySchema::new("fold_context_lines", PropertyValueKind::UInt, true, true),
            PropertySchema::new("unified_changed_pair", PropertyValueKind::Bool, true, true),
            PropertySchema::enumerated(
                "whitespace_mode",
                true,
                true,
                &["show", "ignore_eol", "ignore_all", "mark_blank_changes"],
            ),
            PropertySchema::new("word_granularity", PropertyValueKind::Bool, true, true),
        ];

        // `ToastStack` publishes the stack depth and the selected toast id. The
        // id is a string because that is what `selected_id()` returns and what the
        // `toast_activated` / `toast_dismissed` signals carry.
        #[cfg(not(alloc_frugal))]
        pub(crate) const TOAST_STACK_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("toast_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("selected_id", PropertyValueKind::String, true, false),
            PropertySchema::new("row_height", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("max_visible", PropertyValueKind::UInt, true, true),
            PropertySchema::enumerated(
                "anchor",
                true,
                true,
                &["top_left", "top_right", "bottom_left", "bottom_right"],
            ),
            PropertySchema::enumerated(
                "stacking_direction",
                true,
                true,
                &["newest_at_bottom", "newest_at_top"],
            ),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const GRID_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("rows", PropertyValueKind::UInt, true, true),
            PropertySchema::new("columns", PropertyValueKind::UInt, true, true),
            PropertySchema::new("spacing", PropertyValueKind::UInt, true, true),
            PropertySchema::new("line_color", PropertyValueKind::String, true, true),
            PropertySchema::new("cell_width", PropertyValueKind::UInt, true, false),
            PropertySchema::new("cell_height", PropertyValueKind::UInt, true, false),
            // The grid tracks a selection, so both directions are served: the reader reports the
            // selected cell as a flat index (or null), and the writer accepts the same spelling.
            PropertySchema::new("selected_cell", PropertyValueKind::UInt, true, true),
            PropertySchema::new("selected_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("page", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("page_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("headers", PropertyValueKind::StringList, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const FREEFORM_SHAPE_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::enumerated(
                "path_kind",
                true,
                false,
                &["bubble", "rounded_rect", "polygon", "star", "heart", "custom"],
            ),
            PropertySchema::new("fill_rgba", PropertyValueKind::String, true, true),
            PropertySchema::new("stroke_rgba", PropertyValueKind::String, true, true),
            PropertySchema::new("stroke_width", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("movable", PropertyValueKind::Bool, true, true),
        ];


        pub(crate) const ARC_PROPERTIES: &[PropertySchema] = &[
            // D-01: the centre-reading template.
            PropertySchema::new("value_format", PropertyValueKind::String, true, true),
            PropertySchema::new("value", PropertyValueKind::UInt, true, true),
            PropertySchema::new("minimum", PropertyValueKind::UInt, true, true),
            PropertySchema::new("maximum", PropertyValueKind::UInt, true, true),
            PropertySchema::new("thickness", PropertyValueKind::UInt, true, true),
            PropertySchema::new("sweep_angle", PropertyValueKind::UInt, true, true),
            // The 12-o'clock origin the arc sweeps from. It existed as a field with no
            // accessor, so a document could not place a gauge's start at anywhere but the top.
            PropertySchema::new("start_angle", PropertyValueKind::Float, true, true),
            PropertySchema::new("indeterminate", PropertyValueKind::Bool, true, true),
            // Both are painted (`rounded` changes the cap geometry, `show_value` the label band)
            // and had public setters, but were unreachable through the contract.
            PropertySchema::new("rounded", PropertyValueKind::Bool, true, true),
            PropertySchema::new("show_value", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const MENTION_PROPERTIES: &[PropertySchema] = &[
            // The candidate list as records (BLUE26 B26.09): `set_candidates` takes a list the
            // property API cannot carry, so the list itself is the property.
            PropertySchema::records(
                "candidates",
                true,
                true,
                &crate::widget::input_widgets::mention::MENTION_CANDIDATE_SCHEMA,
            ),
            // The text and the trigger are written directly; the counts are what the property layer
            // reports. Same convention as the other list-valued controls.
            PropertySchema::new("text", PropertyValueKind::String, true, true),
            PropertySchema::new("trigger", PropertyValueKind::String, true, true),
            PropertySchema::new("candidate_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("visible_candidate_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("mention_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("popup_open", PropertyValueKind::Bool, true, false),
            PropertySchema::new("caret", PropertyValueKind::UInt, true, false),
            // The popup's size budget: the row cap it shows before scrolling and the height of
            // one candidate row. Both are assigned through the control's own setters.
            PropertySchema::new("max_visible", PropertyValueKind::UInt, true, true),
            PropertySchema::new("row_height", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const EMOJI_PICKER_PROPERTIES: &[PropertySchema] = &[
            // D-05: how many recently used glyphs are kept.
            PropertySchema::new("recent_capacity", PropertyValueKind::UInt, true, true),
            // The glyph table and the category tabs are properties, not only commands (BLUE26
            // B26.09): `set_glyphs`/`set_categories` take arguments the property API cannot carry.
            PropertySchema::records(
                "glyphs",
                true,
                true,
                &crate::widget::display_widgets::emoji_picker::EMOJI_GLYPH_SCHEMA,
            ),
            // Category names are the caller's own taxonomy and may repeat no more than a
            // positional string list can, so the tab order travels as a `StringList`.
            PropertySchema::new("categories", PropertyValueKind::StringList, true, true),
            // The glyph table is written through `set_glyphs`; the counts and the
            // two editable scalars are what the property layer reports. Same
            // convention as the other list-valued controls.
            PropertySchema::new("glyph_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("visible_glyph_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("tab_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("active_tab", PropertyValueKind::UInt, true, true),
            PropertySchema::new("search", PropertyValueKind::String, true, true),
            PropertySchema::new("recent_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const QUERY_BUILDER_PROPERTIES: &[PropertySchema] = &[
            // The fields and the rows as records (BLUE26 B26.09); the counts are what the property
            // layer reports. `set_fields` / `add_row` / `remove_row` take arguments the property
            // API cannot carry, so each list itself is the property.
            PropertySchema::records(
                "fields",
                true,
                true,
                &crate::widget::view_widgets::query_builder::FILTER_FIELD_SCHEMA,
            ),
            PropertySchema::records(
                "rows",
                true,
                true,
                &crate::widget::view_widgets::query_builder::QUERY_ROW_SCHEMA,
            ),
            PropertySchema::new("row_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("incomplete_row_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("field_count", PropertyValueKind::UInt, true, false),
            PropertySchema::enumerated("conjunction", true, true, &["and", "or"]),
            PropertySchema::new("active_row", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("live_update", PropertyValueKind::Bool, true, true),
            PropertySchema::new("commit_delay_ms", PropertyValueKind::UInt, true, true),
            PropertySchema::new("has_pending_commit", PropertyValueKind::Bool, true, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const CASCADER_PROPERTIES: &[PropertySchema] = &[
            // The committed path as an index sequence (BLUE26 B26.09): `set_selected_path` takes a
            // list the property API cannot carry, so the sequence itself is the property.
            PropertySchema::new("selected_path", PropertyValueKind::NumberList, true, true),
            // The selected path is a sequence, written through `set_selected_path`; what the
            // property layer reports is the joined text, the derived depth, and the three scalar
            // display switches.
            PropertySchema::new("value", PropertyValueKind::String, true, false),
            // I-11: the empty-state text the control paints when no path is chosen.
            PropertySchema::new("placeholder", PropertyValueKind::String, true, true),
            PropertySchema::new("depth", PropertyValueKind::UInt, true, false),
            PropertySchema::new("expanded", PropertyValueKind::Bool, true, true),
            PropertySchema::new("separator", PropertyValueKind::String, true, true),
            PropertySchema::new("filterable", PropertyValueKind::Bool, true, true),
            PropertySchema::new("filter", PropertyValueKind::String, true, false),
            // A nullable row cap applied to every column of the browsed path.
            PropertySchema::new("max_visible", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const KANBAN_BOARD_PROPERTIES: &[PropertySchema] = &[
            // The two levels of the model travel as two flat record lists (B26.09): `columns`
            // carries column metadata, `cards` carries every card with its `column` index and
            // optional `lane`. Their `set_*` commands are the payload-free spelling of the write
            // (B26.03); `add_column` / `add_card` / `move_card` stay Rust-side methods whose
            // payloads the records now transport, so they are not published.
            PropertySchema::records(
                "columns",
                true,
                true,
                &crate::widget::chart_widgets::kanban_board::KANBAN_COLUMN_SCHEMA,
            ),
            PropertySchema::records(
                "cards",
                true,
                true,
                &crate::widget::chart_widgets::kanban_board::KANBAN_CARD_SCHEMA,
            ),
            PropertySchema::new("column_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("card_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("dragging_card_id", PropertyValueKind::String, true, false),
            PropertySchema::new("hovered_column", PropertyValueKind::UInt, true, false),
            PropertySchema::new("column_width", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("horizontal_scroll_offset", PropertyValueKind::UInt, true, true),
            // Swimlanes: `lane_mode` is the explicit switch, `swimlanes` the boolean alias the
            // control accepts for it, and the count is read-only because a lane is an identity.
            PropertySchema::enumerated("lane_mode", true, true, &["none", "lanes"]),
            PropertySchema::new("swimlanes", PropertyValueKind::Bool, true, true),
            PropertySchema::new("lane_count", PropertyValueKind::UInt, true, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const RADAR_CHART_PROPERTIES: &[PropertySchema] = &[
            // The axes and series are lists, written through `set_axes` /
            // `set_series`; the counts are what the property layer reports.
            PropertySchema::new("axis_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("series_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("show_grid", PropertyValueKind::Bool, true, true),
            PropertySchema::new("show_axis_labels", PropertyValueKind::Bool, true, true),
            PropertySchema::new("show_legend", PropertyValueKind::Bool, true, true),
            PropertySchema::new("hovered_axis", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("series_names", PropertyValueKind::StringList, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const HEATMAP_PROPERTIES: &[PropertySchema] = &[
            // The labels and the matrix are written through `set_data`; what the property
            // layer reports is the shape of what was set, plus the scale the colours
            // actually use. Reporting the *resolved* scale is what makes a derived range
            // inspectable — a caller otherwise cannot tell which values the published
            // `scale_minimum` / `scale_maximum` pair is standing in for.
            PropertySchema::new("row_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("column_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("scale_minimum", PropertyValueKind::Float, true, true),
            PropertySchema::new("scale_maximum", PropertyValueKind::Float, true, true),
            PropertySchema::new("resolved_minimum", PropertyValueKind::Float, true, false),
            PropertySchema::new("resolved_maximum", PropertyValueKind::Float, true, false),
            PropertySchema::new("show_labels", PropertyValueKind::Bool, true, true),
            PropertySchema::new("show_legend", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("viewport_rows", PropertyValueKind::UInt, true, true),
            PropertySchema::new("viewport_columns", PropertyValueKind::UInt, true, true),
        ];

        pub(crate) const METER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("value", PropertyValueKind::UInt, true, true),
            PropertySchema::new("minimum", PropertyValueKind::UInt, true, true),
            PropertySchema::new("maximum", PropertyValueKind::UInt, true, true),
            PropertySchema::new("tick_count", PropertyValueKind::UInt, true, true),
            PropertySchema::new("show_tick_labels", PropertyValueKind::Bool, true, true),
            PropertySchema::new("unit", PropertyValueKind::String, true, true),
            // Derived from the value and the unit.
            PropertySchema::new("value_text", PropertyValueKind::String, true, false),
            // The threshold list is written through `add_threshold_range` /
            // `clear_thresholds`; the count is what the property layer reports.
            PropertySchema::new("threshold_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        pub(crate) const MINI_CHART_PROPERTIES: &[PropertySchema] = &[
            // The series as records (BLUE26 B26.09) and the two axis bounds as scalars: `set_data`
            // and `set_range` take arguments the property API cannot carry, so the values
            // themselves are the properties.
            PropertySchema::records(
                "data",
                true,
                true,
                &crate::widget::display_widgets::mini_chart::MINI_CHART_DATA_SCHEMA,
            ),
            PropertySchema::new("min_value", PropertyValueKind::UInt, true, true),
            PropertySchema::new("max_value", PropertyValueKind::UInt, true, true),
            // `MiniChart` supports a strict subset of `Chart`'s types: its own `set`
            // arm accepts only `line` and `bar`, so publishing `pie`/`scatter` here
            // would advertise values the control refuses.
            PropertySchema::enumerated("chart_type", true, true, &["line", "bar"]),
            // The hover switch, published like the other Bool switches: the getter
            // reports the switch and the setter mirrors `set_hover_enabled`, which
            // also clears any marker the previous state left behind.
            PropertySchema::new("hover_enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        pub(crate) const MINI_CANVAS_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        // `Canvas` is a command-recording drawing surface: the caller pushes
        // `RenderCommand`s and the control replays them. Its one piece of state a
        // consumer can inspect is how many commands are queued, so that is what it
        // publishes. It previously carried the *map view* schema (`center_x`,
        // `zoom`, ...) — a copy of `MAP_VIEW_PROPERTIES` — because both report
        // `WidgetKind::Canvas`; the schema described a control this type is not.
        #[cfg(not(alloc_frugal))]
        pub(crate) const CANVAS_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("command_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("layer_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("anti_alias", PropertyValueKind::Bool, true, true),
            PropertySchema::new("show_grid", PropertyValueKind::Bool, true, true),
            PropertySchema::new("grid_step", PropertyValueKind::UInt, true, true),
            PropertySchema::new("snap_to_grid", PropertyValueKind::Bool, true, true),
            PropertySchema::new("zoom_range", PropertyValueKind::NumberList, true, true),
            PropertySchema::new("zoom_step", PropertyValueKind::Float, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        // `ChartWidget` holds a numeric series plus its labels. It previously carried
        // only `selected_marker_id` — a leftover from a marker concept this type
        // never had — so a caller could discover the control but not read its data.
        //
        // (This constant used to be defined twice in this file. The duplicate was
        // invisible while both copies were identical; updating one of them turned
        // it into a compile error, which is how it was found. The single
        // definition above is the one in use.)
        #[cfg(not(alloc_frugal))]
        pub(crate) const BADGE_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("text", PropertyValueKind::String, true, true),
            PropertySchema::new("count", PropertyValueKind::Int, true, true),
            // A dot is the badge's "presence" mode: it draws a small disc instead of the count.
            // The setter existed and the paint path read it, but neither the schema nor the
            // property list mentioned it.
            PropertySchema::new("dot_mode", PropertyValueKind::Bool, true, true),
            // The severity, which is what decides the badge's colour.
            PropertySchema::enumerated(
                "level",
                true,
                true,
                &["info", "success", "warning", "error"],
            ),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const SKELETON_LOADER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("active", PropertyValueKind::Bool, true, true),
            // D-15: the row count of the text-line placeholder.
            PropertySchema::new("text_line_rows", PropertyValueKind::UInt, true, true),
            PropertySchema::enumerated("shape", true, true, &["rect", "circle", "text_line"]),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const FAB_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("icon", PropertyValueKind::String, true, true),
            PropertySchema::new("icon_source", PropertyValueKind::IconSpec, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const BOTTOM_NAVIGATION_BAR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("item_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("selected_index", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const NAVIGATION_DRAWER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("open", PropertyValueKind::Bool, true, true),
            PropertySchema::new("width", PropertyValueKind::Float, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("section_separator_visible", PropertyValueKind::Bool, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const APP_BAR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("title", PropertyValueKind::String, true, true),
            // Which side the back arrow and the action are placed on. The bar's leading affordance
            // belongs where the reader starts, so this is not a cosmetic setting.
            PropertySchema::enumerated("direction", true, true, &["ltr", "rtl"]),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const MOBILE_DATE_PICKER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("selected_date", PropertyValueKind::String, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("year_span", PropertyValueKind::UInt, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const DIVIDER_PROPERTIES: &[PropertySchema] = &[
            // D-04: the shared shaded-line vocabulary.
            PropertySchema::enumerated("line_style", true, true, &["plain", "sunken", "raised"]),
            PropertySchema::new("orientation", PropertyValueKind::String, true, true),
            PropertySchema::new("thickness", PropertyValueKind::Float, true, true),
            // The line colour. `set_color` was public and painted but absent from the contract,
            // so the capability layer and the JSON loader could not address it.
            PropertySchema::new("color", PropertyValueKind::Color, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const RATING_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("value", PropertyValueKind::Float, true, true),
            // D-10: half-star commits.
            PropertySchema::new("allow_half_step", PropertyValueKind::Bool, true, true),
            PropertySchema::new("max", PropertyValueKind::UInt, true, true),
            // The control's own geometry of the row. `star_size` had a setter that nothing could reach
            // through the contract, and the two derived readings are what a consumer would otherwise
            // have to recompute from `value`/`max`.
            PropertySchema::new("star_size", PropertyValueKind::UInt, true, true),
            PropertySchema::new("fill", PropertyValueKind::Float, true, false),
            PropertySchema::new("display_text", PropertyValueKind::String, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const AVATAR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("initials", PropertyValueKind::String, true, true),
            // MISC-02: the shape and the requested diameter.
            PropertySchema::new("square", PropertyValueKind::Bool, true, true),
            PropertySchema::new("size", PropertyValueKind::UInt, true, true),
            PropertySchema::new("image_source", PropertyValueKind::String, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("badge", PropertyValueKind::String, true, true),
            PropertySchema::new("ring_color", PropertyValueKind::Color, true, true),
            PropertySchema::new("ring_width", PropertyValueKind::UInt, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const EMPTY_STATE_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("message", PropertyValueKind::String, true, true),
            // D-06: the optional second action.
            PropertySchema::new("secondary_action_text", PropertyValueKind::String, true, true),
            PropertySchema::new("description", PropertyValueKind::String, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const COLOR_HISTORY_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("color_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const COLOR_WELL_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("color", PropertyValueKind::Color, true, true),
            // The well's frame is optional, and `draw` reads this switch.
            PropertySchema::new("show_border", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const QR_CODE_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("data", PropertyValueKind::String, true, true),
            PropertySchema::new("size", PropertyValueKind::UInt, true, true),
            // The margin around the symbol, in modules. It has to be published for the same
            // reason `qrencode`'s `margin` and every other QR library expose it: a decoder
            // needs a light border to find the finder patterns, so the value is a caller's
            // correctness knob rather than a cosmetic one. The control already stored and drew
            // with it; without a schema row there was no way to read or set it.
            PropertySchema::new("quiet_zone", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::enumerated("ec_level", true, true, &["L", "M", "Q", "H"]),
            PropertySchema::new("min_version", PropertyValueKind::UInt, true, true),
            PropertySchema::new("max_version", PropertyValueKind::UInt, true, true),
            PropertySchema::new("foreground_color", PropertyValueKind::Color, true, true),
            PropertySchema::new("background_color", PropertyValueKind::Color, true, true),
            // MISC-03: the logo cut-out and its module size. `logo` is a structured icon spec,
            // the same representation `Icon` uses, so a document can round-trip it.
            PropertySchema::new("logo", PropertyValueKind::IconSpec, true, true),
            PropertySchema::new("logo_size_modules", PropertyValueKind::UInt, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const MATERIAL_SNACKBAR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("message", PropertyValueKind::String, true, true),
            PropertySchema::new("action_text", PropertyValueKind::String, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("duration_ms", PropertyValueKind::UInt, true, true),
            PropertySchema::new("slide_progress", PropertyValueKind::Float, true, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const ADAPTIVE_SCAFFOLD_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("title", PropertyValueKind::String, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::enumerated("badge_style", true, true, &["text", "dot"]),
            PropertySchema::new("drawer_animated", PropertyValueKind::Bool, true, true),
            PropertySchema::new("scrim_opacity", PropertyValueKind::Float, true, true),
            PropertySchema::new("page_transition", PropertyValueKind::Bool, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const SEGMENTED_BUTTON_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("selected_index", PropertyValueKind::UInt, true, true),
            // The whole selection, comma-joined, which is the spelling `list_box`'s
            // `selected_indices` and `tag_input`'s `tags` already use. Published because
            // `selected_index` is the *primary* selection and cannot express the multi-select set
            // that `allows_multiple` creates — the same gap `list_box` documents.
            PropertySchema::new("selected_indices", PropertyValueKind::String, true, true),
            PropertySchema::new("allows_multiple", PropertyValueKind::Bool, true, true),
            PropertySchema::new("segment_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("group_id", PropertyValueKind::String, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const NAVIGATION_STACK_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("page_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("current_page", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("back_visible", PropertyValueKind::Bool, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const PROGRESS_CIRCLE_PROPERTIES: &[PropertySchema] = &[
            // The control bounds the value to `[0.0, 1.0]` (it coerces), so the bound is a fact
            // about the control rather than a designer's preference: a property editor shows a 0–1
            // slider, and every write path refuses a value outside it with the property named,
            // rather than storing one the control would move (BLUE26 B26.02).
            PropertySchema::new("value", PropertyValueKind::Float, true, true).constrained(&[
                Constraint::Range(RangeSpec { minimum: 0.0, maximum: 1.0, step: None }),
            ]),
            PropertySchema::new("thickness", PropertyValueKind::Float, true, true),
            // The two colours and the ring's own size are painted/laid out by the control but were
            // unreachable through the contract, so a designer saw no route to them.
            PropertySchema::new("progress_color", PropertyValueKind::Color, true, true),
            PropertySchema::new("diameter", PropertyValueKind::UInt, true, true),
            PropertySchema::new("indeterminate", PropertyValueKind::Bool, true, true),
            // The centre reading: an opt-in switch and the template it renders through.
            PropertySchema::new("show_center_text", PropertyValueKind::Bool, true, true),
            PropertySchema::new("center_text_format", PropertyValueKind::String, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const ICON_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("icon_name", PropertyValueKind::String, true, true),
            // The typed source, published for the same reason the button-like controls publish it:
            // a document can name a source the legacy string cannot express, and a host can read
            // one back. A borrowed static geometry is refused on read rather than serialised as
            // something it is not — the rule the five other `icon_source` owners follow.
            PropertySchema::new("icon_source", PropertyValueKind::IconSpec, true, true),
            PropertySchema::new("size", PropertyValueKind::Float, true, true),
            PropertySchema::new("color", PropertyValueKind::String, true, true),
            // The transform pair the getter/setter arms serve: the rotation in
            // degrees, and the flip axis as the reader's own token (the writer
            // rejects any spelling outside this list).
            PropertySchema::new("icon_rotation", PropertyValueKind::Float, true, true),
            PropertySchema::enumerated(
                "icon_flip",
                true,
                true,
                &["none", "horizontal", "vertical", "both"],
            ),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const FONT_PREVIEW_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("font_family", PropertyValueKind::String, true, true),
            PropertySchema::new("font_size", PropertyValueKind::Float, true, true),
            PropertySchema::new("preview_text", PropertyValueKind::String, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const CUPERTINO_NAVIGATION_BAR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("title", PropertyValueKind::String, true, true),
            PropertySchema::new("large_title", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("prompt", PropertyValueKind::String, true, true),
            PropertySchema::new("collapse_offset", PropertyValueKind::Float, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const REFRESH_CONTROL_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("refreshing", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const DATE_RANGE_PICKER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("start_date", PropertyValueKind::String, true, true),
            PropertySchema::new("end_date", PropertyValueKind::String, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("presets", PropertyValueKind::StringList, true, true),
            PropertySchema::new("double_month", PropertyValueKind::Bool, true, true),
            PropertySchema::new("min_nights", PropertyValueKind::UInt, true, true),
            PropertySchema::new("max_nights", PropertyValueKind::UInt, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const MATERIAL_NAVIGATION_RAIL_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("selected_index", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const TAB_VIEW_PROPERTIES: &[PropertySchema] = &[
            // The widget has answered `tab_count()` since it existed; without the schema row a
            // host could call it in Rust but could not read it through the property contract.
            PropertySchema::new("tab_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("selected_index", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("keyboard_navigation_enabled", PropertyValueKind::Bool, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const SWIPE_TO_DISMISS_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("is_dismissed", PropertyValueKind::Bool, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const LINE_CHART_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("stroke_width", PropertyValueKind::Float, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const SPARKLINE_PROPERTIES: &[PropertySchema] = &[
            // The sample series is a property as well as a command, so a document can spell it and
            // a designer can read it back. Order is the value, so it is a `NumberList` rather
            // than `Records`.
            PropertySchema::new("data", PropertyValueKind::NumberList, true, true),
            PropertySchema::new("stroke_width", PropertyValueKind::Float, true, true),
            PropertySchema::new("show_end_value", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("fill", PropertyValueKind::Bool, true, true),
            PropertySchema::new("show_points", PropertyValueKind::Bool, true, true),
            PropertySchema::new("show_last_marker", PropertyValueKind::Bool, true, true),
            PropertySchema::new("capacity", PropertyValueKind::UInt, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const BAR_CHART_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("bar_spacing", PropertyValueKind::Float, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const PIE_CHART_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("donut", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const BEZIER_CURVE_EDITOR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("snap_to_grid", PropertyValueKind::Bool, true, true),
            // MISC-06: the segment count and the curve token.
            PropertySchema::new("segments", PropertyValueKind::UInt, true, false),
            PropertySchema::new("curve_points", PropertyValueKind::String, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const BARCODE_SCANNER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("is_scanning", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];
        /// Property schema for `candlestick_chart`.
        pub(crate) const CANDLESTICK_CHART_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("series", PropertyValueKind::String, true, true),
            // `readable: true`, because the control **answers** it: `CandlestickChart::get`
            // returns `self.overlays.len()`. It was declared `false`, and that flag is not
            // advisory — `capability.rs` turns a non-readable schema into
            // `UnsupportedOnWidget`, so a caller could `set` the overlay count and then could
            // not read it back. "Wrote it, cannot read it" is the one-directional contract
            // the schema exists to prevent.
            PropertySchema::new("overlay_count", PropertyValueKind::UInt, true, true),
            PropertySchema::new("show_price_levels", PropertyValueKind::Bool, true, true),
            PropertySchema::new("show_overlay_legend", PropertyValueKind::Bool, true, true),
        ];
        /// Property schema for `volume_chart`.
        pub(crate) const VOLUME_CHART_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("series", PropertyValueKind::String, true, true),
            PropertySchema::enumerated("color_mode", true, true, &["direction", "uniform"]),
            PropertySchema::new("headroom", PropertyValueKind::Float, true, true),
        ];
        /// Property schema for `depth_chart`.
        pub(crate) const DEPTH_CHART_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("depth", PropertyValueKind::UInt, true, true),
            // Declared `Color`, matching what `DepthChart::set` accepts: it destructures
            // `CapabilityValue::Color` and answers `TypeMismatch` for anything else, so
            // a `String` declaration made the property unwritable through the ABI —
            // `write_property` validated a string against the schema and the control
            // then refused it.
            PropertySchema::new("bid_color", PropertyValueKind::Color, true, true),
            PropertySchema::new("ask_color", PropertyValueKind::Color, true, true),
        ];
        /// Property schema for `order_book`.
        pub(crate) const ORDER_BOOK_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("depth", PropertyValueKind::UInt, true, true),
            PropertySchema::new("decimals", PropertyValueKind::UInt, true, true),
            // Read-only: `OrderBookWidget::set` destructures the value and then
            // refuses the write. The spread band is a display decision the control
            // owns, so there is nothing for a caller to assign.
            PropertySchema::new("show_spread", PropertyValueKind::Bool, true, false),
        ];
        /// Property schema for `quote_board`.
        pub(crate) const QUOTE_BOARD_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::enumerated(
                "sort",
                true,
                true,
                &[
                    "none",
                    "symbol_ascending",
                    "symbol_descending",
                    "last_ascending",
                    "last_descending",
                    "change_magnitude_ascending",
                    "change_magnitude_descending",
                ],
            ),
            PropertySchema::new("selected_index", PropertyValueKind::UInt, true, true),
            // Read-only: `QuoteBoard::set` answers `OutOfRange` because the board's
            // row height is a layout decision the control owns, and `selected_index`
            // is derived from the rows rather than assignable.
            PropertySchema::new("row_height", PropertyValueKind::UInt, true, false),
            PropertySchema::new("scroll_offset", PropertyValueKind::UInt, true, true),
        ];
        /// Property schema for `indicator_chart`.
        pub(crate) const INDICATOR_CHART_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("series", PropertyValueKind::String, true, true),
            PropertySchema::enumerated(
                "mode",
                true,
                true,
                &["macd", "rsi", "stochastic", "money_flow_index", "atr", "on_balance_volume"],
            ),
            PropertySchema::new("period", PropertyValueKind::UInt, true, true),
            PropertySchema::new("show_reference_levels", PropertyValueKind::Bool, true, true),
        ];

        // ── Small display controls ────────────────────────────────────────

        #[cfg(not(alloc_frugal))]
        pub(crate) const TAG_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("text", PropertyValueKind::String, true, true),
            // D-19: the leading icon token; an empty string clears it.
            PropertySchema::new("icon", PropertyValueKind::String, true, true),
            PropertySchema::enumerated(
                "tone",
                true,
                true,
                &["neutral", "primary", "success", "warning", "error"],
            ),
            PropertySchema::new("selected", PropertyValueKind::Bool, true, true),
            PropertySchema::new("closable", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const CLOCK_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("hour", PropertyValueKind::UInt, true, true),
            PropertySchema::new("minute", PropertyValueKind::UInt, true, true),
            PropertySchema::new("second", PropertyValueKind::UInt, true, true),
            PropertySchema::new("show_seconds", PropertyValueKind::Bool, true, true),
            // Where the face's time comes from: the host (`manual`) or the machine clock
            // (`system`). Published as a token so the loader can select it.
            PropertySchema::enumerated("time_source", true, true, &["manual", "system"]),
            // The face form, published as the reader's own tokens; the writer parses
            // exactly this list and refuses anything else.
            PropertySchema::enumerated("display", true, true, &["dial", "digital"]),
            // The zone offset in minutes. An absent offset reads as `null` and writes
            // back as `null`, so the kind is the carried value and `null` stays legal
            // the same way the nullable `shortcut` is.
            PropertySchema::new("utc_offset", PropertyValueKind::Int, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const GAUGE_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("value", PropertyValueKind::Float, true, true),
            PropertySchema::new("minimum", PropertyValueKind::Float, true, true),
            PropertySchema::new("maximum", PropertyValueKind::Float, true, true),
            PropertySchema::new("label", PropertyValueKind::String, true, true),
            PropertySchema::new("unit", PropertyValueKind::String, true, true),
            PropertySchema::new("zone_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("animated", PropertyValueKind::Bool, true, true),
            PropertySchema::new("thickness", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("bands", PropertyValueKind::StringList, true, true),
            PropertySchema::new("tick_count", PropertyValueKind::UInt, true, true),
            PropertySchema::new("start_angle", PropertyValueKind::Float, true, true),
            PropertySchema::new("end_angle", PropertyValueKind::Float, true, true),
            PropertySchema::enumerated("needle_style", true, true, &["none", "line", "triangle"]),
            PropertySchema::new("unit_label", PropertyValueKind::String, true, true),
        ];

        // ── macOS (desktop) component set ───────────────────────────────
        //
        // Five controls that model the desktop platform's own shapes, beside the
        // touch-styled `cupertino` set. Each shares a `WidgetKind` with the control it
        // refines, so its schema is its own table rather than a row on the sibling's.

        #[cfg(not(alloc_frugal))]
        pub(crate) const MAC_SEGMENTED_CONTROL_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("selected_index", PropertyValueKind::UInt, true, true),
            PropertySchema::new("segment_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const MAC_STEPPER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("value", PropertyValueKind::Int, true, true),
            PropertySchema::new("minimum", PropertyValueKind::Int, true, true),
            PropertySchema::new("maximum", PropertyValueKind::Int, true, true),
            PropertySchema::new("step", PropertyValueKind::Int, true, true),
            PropertySchema::new("wraps", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("repeat_delay_ms", PropertyValueKind::UInt, true, true),
            PropertySchema::new("repeat_interval_ms", PropertyValueKind::UInt, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const MAC_TOOLBAR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("item_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("icon_size", PropertyValueKind::Float, true, true),
            // Laid out horizontally by construction, so the name is readable and not writable:
            // publishing a token the control would refuse to honour would be a lie.
            PropertySchema::new("orientation", PropertyValueKind::String, true, false),
            PropertySchema::new("selectable", PropertyValueKind::Bool, true, true),
            PropertySchema::new("selected_index", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("overflow_count", PropertyValueKind::UInt, true, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const MAC_SOURCE_LIST_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("section_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("row_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("row_height", PropertyValueKind::UInt, true, false),
            PropertySchema::new("selected_row", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const MAC_WINDOW_CHROME_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("title", PropertyValueKind::String, true, true),
            // The bar's height is the control's own metric; a caller wanting a different bar
            // draws a different component rather than stretching this one.
            PropertySchema::new("chrome_height", PropertyValueKind::UInt, true, false),
            PropertySchema::new("buttons_visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("draggable", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("fullscreen", PropertyValueKind::Bool, true, true),
            PropertySchema::new("document_edited", PropertyValueKind::Bool, true, true),
            PropertySchema::new("focused", PropertyValueKind::Bool, true, true),
        ];

        // ── Companion controls (COMP-01/05/07/08/09/10/12/13/14/15/16/17/18/19/20) ──
        //
        // The companion controls are first-class capabilities. Their property tables are
        // declared here beside the controls they describe; each name is the one the
        // control's own `WidgetProperties::property_names` publishes.

        /// Palette grid shared by the colour controls (`COMP-01`).
        ///
        /// `colors` is a list of typed colours carried as a `Tuple` of
        /// `CapabilityValue::Color`; the schema names the element kind because there is no
        /// dedicated colour-list kind in `PropertyValueKind`.
        #[cfg(not(alloc_frugal))]
        pub(crate) const SWATCH_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("colors", PropertyValueKind::Color, true, true),
            PropertySchema::new("selected_index", PropertyValueKind::UInt, true, true),
            PropertySchema::new("cell_size", PropertyValueKind::UInt, true, true),
            PropertySchema::new("columns", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// One filter editor per column (`COMP-05`).
        #[cfg(not(alloc_frugal))]
        pub(crate) const FILTER_ROW_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("columns", PropertyValueKind::StringList, true, true),
            // Each entry is the `[column, operator, operand]` triple the reader produces.
            PropertySchema::new("conditions", PropertyValueKind::StringList, true, true),
            PropertySchema::enumerated(
                "operator",
                true,
                true,
                &[
                    "contains",
                    "equals",
                    "starts_with",
                    "ends_with",
                    "not_contains",
                    "equals_number",
                    "greater_than",
                    "greater_or_equal",
                    "less_than",
                    "less_or_equal",
                ],
            ),
            PropertySchema::new("live", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Series key for charts (`COMP-07`).
        #[cfg(not(alloc_frugal))]
        pub(crate) const LEGEND_PROPERTIES: &[PropertySchema] = &[
            // Each entry is the `[label, colour, visible]` triple the reader produces.
            PropertySchema::new("entries", PropertyValueKind::StringList, true, true),
            PropertySchema::enumerated("orientation", true, true, &["horizontal", "vertical"]),
            PropertySchema::new("checkable", PropertyValueKind::Bool, true, true),
            PropertySchema::enumerated(
                "position",
                true,
                true,
                &["top", "right", "bottom", "left", "outside"],
            ),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Follow-pointer chart bubble (`COMP-08`).
        #[cfg(not(alloc_frugal))]
        pub(crate) const CHART_TOOLTIP_PROPERTIES: &[PropertySchema] = &[
            // Points travel as a two-number tuple, so `NumberList` is the closest carrier.
            PropertySchema::new("anchor", PropertyValueKind::NumberList, true, true),
            PropertySchema::new("offset", PropertyValueKind::NumberList, true, true),
            PropertySchema::new("follow_pointer", PropertyValueKind::Bool, true, true),
            PropertySchema::new("content", PropertyValueKind::StringList, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Host-wide hover-to-tooltip pipeline (`COMP-09`).
        #[cfg(not(alloc_frugal))]
        pub(crate) const TOOLTIP_MANAGER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("delay_ms", PropertyValueKind::UInt, true, true),
            PropertySchema::new("duration_ms", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("follow_pointer", PropertyValueKind::Bool, true, true),
            PropertySchema::new("max_width", PropertyValueKind::UInt, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Zoom/pan/ROI state for an image view (`COMP-10`).
        #[cfg(not(alloc_frugal))]
        pub(crate) const ZOOM_PAN_CONTROLLER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("zoom_level", PropertyValueKind::Float, true, true),
            PropertySchema::new("pan_offset", PropertyValueKind::NumberList, true, true),
            PropertySchema::enumerated(
                "zoom_mode",
                true,
                true,
                &["free", "fit_width", "fit_height", "original"],
            ),
            PropertySchema::new("min_zoom", PropertyValueKind::Float, true, true),
            PropertySchema::new("max_zoom", PropertyValueKind::Float, true, true),
            PropertySchema::new("roi", PropertyValueKind::Rect, true, true),
            // The content extent is a `Size`, which the vocabulary does not declare; the two
            // scalars are the pair convention this crate uses for such a value.
            PropertySchema::new("content_width", PropertyValueKind::UInt, true, true),
            PropertySchema::new("content_height", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Bounded, expiring toast queue (`COMP-12`).
        #[cfg(not(alloc_frugal))]
        pub(crate) const TOAST_HOST_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("max_visible", PropertyValueKind::UInt, true, true),
            PropertySchema::new("max_queued", PropertyValueKind::UInt, true, true),
            PropertySchema::new("default_duration_ms", PropertyValueKind::UInt, true, true),
            PropertySchema::enumerated(
                "anchor",
                true,
                true,
                &[
                    "top_left",
                    "top_center",
                    "top_right",
                    "bottom_left",
                    "bottom_center",
                    "bottom_right",
                ],
            ),
            PropertySchema::enumerated(
                "stacking_direction",
                true,
                true,
                &["newest_at_top", "newest_at_bottom"],
            ),
            PropertySchema::enumerated(
                "overflow_policy",
                true,
                true,
                &["drop_oldest", "drop_newest", "queue"],
            ),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Tile addressing and cache for a map (`COMP-13`).
        #[cfg(not(alloc_frugal))]
        pub(crate) const TILE_LOADER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("url_template", PropertyValueKind::String, true, true),
            PropertySchema::new("min_zoom", PropertyValueKind::UInt, true, true),
            PropertySchema::new("max_zoom", PropertyValueKind::UInt, true, true),
            PropertySchema::new("tile_size", PropertyValueKind::UInt, true, true),
            PropertySchema::new("cache_limit", PropertyValueKind::UInt, true, true),
            PropertySchema::new("cache_size", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Three-button zoom strip for a map (`COMP-13`).
        #[cfg(not(alloc_frugal))]
        pub(crate) const ZOOM_CONTROL_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("zoom_level", PropertyValueKind::Float, true, true),
            PropertySchema::new("min", PropertyValueKind::Float, true, true),
            PropertySchema::new("max", PropertyValueKind::Float, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Distance legend for a map (`COMP-13`).
        #[cfg(not(alloc_frugal))]
        pub(crate) const SCALE_BAR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("meters_per_pixel", PropertyValueKind::Float, true, true),
            PropertySchema::enumerated("units", true, true, &["metric", "imperial"]),
            PropertySchema::enumerated("style", true, true, &["plain", "boxed"]),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Bearing dial for a map (`COMP-13`).
        #[cfg(not(alloc_frugal))]
        pub(crate) const COMPASS_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("bearing_deg", PropertyValueKind::Float, true, true),
            PropertySchema::new("clickable", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Date/value axis for a Gantt chart (`COMP-14`).
        #[cfg(not(alloc_frugal))]
        pub(crate) const TIMELINE_AXIS_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("tick_density", PropertyValueKind::UInt, true, true),
            PropertySchema::enumerated("date_format", true, true, &["numeric", "iso"]),
            PropertySchema::new("zoom_level", PropertyValueKind::Float, true, true),
            // The range pair is read as `[start, end]`; the two scalar names below write it.
            PropertySchema::new("range", PropertyValueKind::NumberList, true, false),
            PropertySchema::new("range_start", PropertyValueKind::Int, true, true),
            PropertySchema::new("range_end", PropertyValueKind::Int, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Editable dependency edges for a Gantt chart (`COMP-14`).
        #[cfg(not(alloc_frugal))]
        pub(crate) const DEPENDENCY_EDITOR_PROPERTIES: &[PropertySchema] = &[
            // The reader reports the list's length; the list itself is replaced through the
            // control's `set_dependencies` method, so the property is read-only.
            PropertySchema::new("dependencies", PropertyValueKind::UInt, true, false),
            // The edge list itself, round-tripped through the `Records` contract (SP-123).
            PropertySchema::records(
                "dependency_edges",
                true,
                true,
                &crate::widget::chart_widgets::gantt_widget::companions::DEPENDENCY_SCHEMA,
            ),
            PropertySchema::new("editable", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Multi-file selector beside a diff viewer (`COMP-15`).
        #[cfg(not(alloc_frugal))]
        pub(crate) const DIFF_FILE_LIST_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("files", PropertyValueKind::StringList, true, true),
            PropertySchema::new("file_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("selected_path", PropertyValueKind::String, true, false),
            PropertySchema::enumerated(
                "selection_mode",
                true,
                true,
                &["single", "multi", "extended", "none"],
            ),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Heading outline beside a markdown editor (`COMP-16`).
        ///
        /// Each `entries` row is the `level|title|line` token the control reads.
        #[cfg(not(alloc_frugal))]
        pub(crate) const MARKDOWN_OUTLINE_PANEL_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("entries", PropertyValueKind::StringList, true, true),
            PropertySchema::new("active_index", PropertyValueKind::UInt, true, true),
            PropertySchema::new("width", PropertyValueKind::UInt, true, true),
            PropertySchema::new("entry_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Markdown formatting strip beside a markdown editor (SP-022).
        ///
        /// Mirrors [`MarkdownToolbar`](crate::widget::special_widgets::markdown_editor::MarkdownToolbar)'s
        /// property contract: the enabled action tokens, which side the icon is drawn on and whether
        /// labels are shown. The action list is writable because the control validates every token in
        /// `set_actions` and refuses an unknown one rather than painting a dead cell.
        #[cfg(not(alloc_frugal))]
        pub(crate) const MARKDOWN_TOOLBAR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("actions", PropertyValueKind::StringList, true, true),
            PropertySchema::enumerated("icon_side", true, true, &["none", "left", "right"]),
            PropertySchema::new("show_labels", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Unified dropdown popup host (COMP-A21).
        #[cfg(not(alloc_frugal))]
        pub(crate) const DROP_DOWN_POPUP_HOST_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::records(
                "items",
                true,
                true,
                &crate::widget::companions::POPUP_ITEM_SCHEMA,
            ),
            PropertySchema::new("open", PropertyValueKind::Bool, true, true),
            PropertySchema::new("highlighted_index", PropertyValueKind::UInt, true, true),
            PropertySchema::new("scroll_offset", PropertyValueKind::Float, true, true),
            PropertySchema::new("max_visible_rows", PropertyValueKind::UInt, true, true),
            PropertySchema::new("anchor_rect", PropertyValueKind::Rect, true, true),
            PropertySchema::enumerated("placement", true, true, &["below", "above", "automatic"]),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Command palette host (COMP-A30).
        #[cfg(not(alloc_frugal))]
        pub(crate) const COMMAND_PALETTE_HOST_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::records(
                "entries",
                true,
                true,
                &crate::widget::companions::PALETTE_ENTRY_SCHEMA,
            ),
            PropertySchema::new("filter", PropertyValueKind::String, true, true),
            PropertySchema::new("selected_index", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Header menu (COMP-A32).
        #[cfg(not(alloc_frugal))]
        pub(crate) const HEADER_MENU_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("sortable", PropertyValueKind::Bool, true, true),
            PropertySchema::new("filterable", PropertyValueKind::Bool, true, true),
            PropertySchema::enumerated("sort_state", true, true, &["none", "asc", "desc"]),
            PropertySchema::new("filter_text", PropertyValueKind::String, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Group avatar (COMP-MISC-010). It publishes only the base contract; membership is set
        /// through `push`/`clear`, not a scalar property.
        #[cfg(not(alloc_frugal))]
        pub(crate) const GROUP_AVATAR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Grid status strip (COMP-A23). Counts are read-only summaries of the grid's model; the
        /// filter flag and explicit text are writable, and `summary` is the painted string.
        #[cfg(not(alloc_frugal))]
        pub(crate) const GRID_STATUS_BAR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("row_count", PropertyValueKind::UInt, true, true),
            PropertySchema::new("selected_count", PropertyValueKind::UInt, true, true),
            PropertySchema::new("filter_active", PropertyValueKind::Bool, true, true),
            PropertySchema::new("text", PropertyValueKind::String, true, true),
            PropertySchema::new("summary", PropertyValueKind::String, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Chart reference line (COMP-A1). `line_count` is a read-only summary of the
        /// annotations the host attached; the rest are the line's own paint switches.
        #[cfg(not(alloc_frugal))]
        pub(crate) const CHART_REFERENCE_LINE_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("line_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("label_visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("hit_target", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Terminal scrollback find bar (COMP-A2). The match count is produced by a scan of the
        /// attached terminal, so it is read-only; `visible` is the bar's own open/close flag.
        #[cfg(not(alloc_frugal))]
        pub(crate) const TERMINAL_FIND_BAR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("query", PropertyValueKind::String, true, true),
            PropertySchema::new("case_sensitive", PropertyValueKind::Bool, true, true),
            PropertySchema::new("use_regex", PropertyValueKind::Bool, true, true),
            PropertySchema::new("wrap_search", PropertyValueKind::Bool, true, true),
            PropertySchema::new("match_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Map attribution overlay (COMP-A3). The entries are the text the tile source requires,
        /// so only their count is published; `corner` selects the placement.
        #[cfg(not(alloc_frugal))]
        pub(crate) const MAP_ATTRIBUTION_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("entry_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("corner", PropertyValueKind::String, true, true),
            PropertySchema::new("background_opacity", PropertyValueKind::UInt, true, true),
            PropertySchema::new("font_scale", PropertyValueKind::Float, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Gantt task column (COMP-A4). The derived counts are read-only; the row metrics and the
        /// header switch are the column's own settings.
        #[cfg(not(alloc_frugal))]
        pub(crate) const GANTT_TASK_GRID_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("row_height", PropertyValueKind::UInt, true, true),
            PropertySchema::new("indent_step", PropertyValueKind::UInt, true, true),
            PropertySchema::new("header_visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("columns", PropertyValueKind::UInt, true, false),
            PropertySchema::new("tasks", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Map overview (COMP-A7). The overview mirrors an attached map, so `map_view_attached`
        /// reports that connection rather than being a setting.
        #[cfg(not(alloc_frugal))]
        pub(crate) const MAP_OVERVIEW_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("viewport_ratio", PropertyValueKind::Float, true, true),
            PropertySchema::new("collapsed", PropertyValueKind::Bool, true, true),
            PropertySchema::new("map_view_attached", PropertyValueKind::Bool, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Find/replace strip beside an editor (`COMP-16`).
        #[cfg(not(alloc_frugal))]
        pub(crate) const FIND_BAR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("query", PropertyValueKind::String, true, true),
            PropertySchema::new("case_sensitive", PropertyValueKind::Bool, true, true),
            PropertySchema::new("whole_word", PropertyValueKind::Bool, true, true),
            PropertySchema::new("replace_text", PropertyValueKind::String, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("replace_visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("match_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("current_range", PropertyValueKind::String, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Scene container for free-form shapes (`COMP-17`).
        #[cfg(not(alloc_frugal))]
        pub(crate) const SHAPE_CANVAS_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("shapes", PropertyValueKind::NumberList, true, false),
            PropertySchema::new("shape_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("selection", PropertyValueKind::NumberList, true, true),
            PropertySchema::new("editable", PropertyValueKind::Bool, true, true),
            PropertySchema::new("snap_to_grid", PropertyValueKind::Bool, true, true),
            PropertySchema::new("grid_step", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Real card control for a kanban column (`COMP-18`).
        #[cfg(not(alloc_frugal))]
        pub(crate) const KANBAN_CARD_VIEW_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("title", PropertyValueKind::String, true, true),
            PropertySchema::new("description", PropertyValueKind::String, true, true),
            PropertySchema::new("done", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tags", PropertyValueKind::StringList, true, true),
            PropertySchema::new("assignee", PropertyValueKind::String, true, true),
            PropertySchema::new("draggable", PropertyValueKind::Bool, true, true),
            PropertySchema::new("editing", PropertyValueKind::Bool, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Real column header for a kanban board (`COMP-18`).
        #[cfg(not(alloc_frugal))]
        pub(crate) const KANBAN_COLUMN_HEADER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("title", PropertyValueKind::String, true, true),
            PropertySchema::new("count", PropertyValueKind::UInt, true, true),
            PropertySchema::new("wip_limit", PropertyValueKind::UInt, true, true),
            PropertySchema::new("collapsed", PropertyValueKind::Bool, true, true),
            PropertySchema::new("editable", PropertyValueKind::Bool, true, true),
            PropertySchema::new("at_wip_limit", PropertyValueKind::Bool, true, false),
            PropertySchema::new("editing", PropertyValueKind::Bool, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Quick date-range presets beside a range picker (`COMP-19`).
        #[cfg(not(alloc_frugal))]
        pub(crate) const RANGE_PRESET_BAR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("presets", PropertyValueKind::StringList, true, true),
            PropertySchema::enumerated("orientation", true, true, &["horizontal", "vertical"]),
            PropertySchema::new("compact", PropertyValueKind::Bool, true, true),
            PropertySchema::new("selected_label", PropertyValueKind::String, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Shared table header with sorting, resizing and filtering (`COMP-20`).
        #[cfg(not(alloc_frugal))]
        pub(crate) const GRID_VIEW_HEADER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("columns", PropertyValueKind::StringList, true, true),
            PropertySchema::new("column_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("sort_indicator_visible", PropertyValueKind::Bool, true, true),
            PropertySchema::enumerated(
                "resize_mode",
                true,
                true,
                &["fixed", "stretch", "stretch_last", "interactive"],
            ),
            PropertySchema::new("filter_icon_visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("sort_columns", PropertyValueKind::String, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Drop-down menu panel a host anchors itself (`COMP-A01`).
        #[cfg(not(alloc_frugal))]
        pub(crate) const MENU_PANEL_PROPERTIES: &[PropertySchema] = &[
            // The row list travels as one flat record list (B26.09); `selected_index` and
            // `highlighted_index` are indices into it.
            PropertySchema::records(
                "items",
                true,
                true,
                &crate::widget::companions::MENU_PANEL_ITEM_SCHEMA,
            ),
            PropertySchema::new("selected_index", PropertyValueKind::UInt, true, true),
            PropertySchema::new("highlighted_index", PropertyValueKind::UInt, true, true),
            PropertySchema::new("max_visible_rows", PropertyValueKind::UInt, true, true),
            PropertySchema::new("open", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Floating trigger that fans out its actions (`COMP-A02`).
        #[cfg(not(alloc_frugal))]
        pub(crate) const SPEED_DIAL_PROPERTIES: &[PropertySchema] = &[
            // The action list travels as one flat record list (B26.09); `action_activated`
            // reports the row's index into it.
            PropertySchema::records(
                "actions",
                true,
                true,
                &crate::widget::companions::SPEED_DIAL_ACTION_SCHEMA,
            ),
            PropertySchema::new("expanded", PropertyValueKind::Bool, true, true),
            PropertySchema::enumerated("direction", true, true, &["up", "left"]),
            PropertySchema::new("spacing", PropertyValueKind::UInt, true, true),
            PropertySchema::new("collapse_on_activate", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Multi-action background for a swiped row (`COMP-A03`).
        #[cfg(not(alloc_frugal))]
        pub(crate) const SWIPE_ACTIONS_PROPERTIES: &[PropertySchema] = &[
            // Each side travels as its own flat record list (B26.09); `action_triggered`
            // reports the side token and the row's index on that side.
            PropertySchema::records(
                "leading_actions",
                true,
                true,
                &crate::widget::companions::SWIPE_ACTION_SCHEMA,
            ),
            PropertySchema::records(
                "trailing_actions",
                true,
                true,
                &crate::widget::companions::SWIPE_ACTION_SCHEMA,
            ),
            PropertySchema::new("max_reveal_width", PropertyValueKind::UInt, true, true),
            PropertySchema::new("icon_side", PropertyValueKind::UInt, true, true),
            PropertySchema::new("offset", PropertyValueKind::Float, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Small heading row that groups a drawer's items (`COMP-A04`).
        #[cfg(not(alloc_frugal))]
        pub(crate) const DRAWER_SECTION_HEADER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("label", PropertyValueKind::String, true, true),
            PropertySchema::new("inset", PropertyValueKind::UInt, true, true),
            PropertySchema::new("uppercase", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Companion list for a canvas's layers (`COMP-I01`).
        #[cfg(not(alloc_frugal))]
        pub(crate) const LAYER_PANEL_PROPERTIES: &[PropertySchema] = &[
            // The layer list travels as one flat record list (B26.09); identity is the `id`
            // field, never the row index. `entry_count` is its read-only count and
            // `selected_layer` the id the host reports through `layer_selected`.
            PropertySchema::records(
                "entries",
                true,
                true,
                &crate::widget::companions::LAYER_ENTRY_SCHEMA,
            ),
            PropertySchema::new("entry_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("selected_layer", PropertyValueKind::UInt, true, false),
            PropertySchema::new("show_visibility_checkboxes", PropertyValueKind::Bool, true, true),
            PropertySchema::new("show_reorder_handles", PropertyValueKind::Bool, true, true),
            PropertySchema::new("row_height", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Companion list for a media player's queue (`COMP-I03`).
        #[cfg(not(alloc_frugal))]
        pub(crate) const PLAYLIST_PANEL_PROPERTIES: &[PropertySchema] = &[
            // The queue travels as one flat record list (B26.09); identity is position in the
            // list, and every signal reports that original position. `track_count` is its
            // read-only count.
            PropertySchema::records(
                "tracks",
                true,
                true,
                &crate::widget::companions::PLAYLIST_TRACK_SCHEMA,
            ),
            PropertySchema::new("track_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("current_index", PropertyValueKind::UInt, true, true),
            PropertySchema::new("show_track_numbers", PropertyValueKind::Bool, true, true),
            PropertySchema::new("show_now_playing_badge", PropertyValueKind::Bool, true, true),
            PropertySchema::new("row_height", PropertyValueKind::UInt, true, true),
            PropertySchema::new("title_from_source", PropertyValueKind::Bool, true, true),
            PropertySchema::new("filter", PropertyValueKind::String, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        // ── Editor/chart/data companion overlays (COMP-21..29) ──
        //
        // These controls are first-class capabilities even though each reports the kind of the
        // control it partners (`Panel`, `Table`, `Slider`, `SearchBar`, `EmptyState`). The kind
        // is what the theme and the accessibility role classify by; the capability name is what
        // the factory, the manifest and the property contract address. Every name below is the
        // one the control's own `WidgetProperties::property_names` publishes.

        /// Line-number rail for an editor or a log view (`COMP-21`).
        ///
        /// `marker_lines` is the breakpoint row list: each entry is a bare document line number
        /// with no identity of its own, so the list kind (not `Records`) is the honest shape.
        #[cfg(not(alloc_frugal))]
        pub(crate) const LINE_NUMBER_GUTTER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("first_line", PropertyValueKind::UInt, true, true),
            PropertySchema::new("line_count", PropertyValueKind::UInt, true, true),
            PropertySchema::new("current_line", PropertyValueKind::UInt, true, true),
            PropertySchema::enumerated("align", true, true, &["left", "right"]),
            PropertySchema::new("marker_lines", PropertyValueKind::NumberList, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Find/replace strip beside an editor (`COMP-22`).
        #[cfg(not(alloc_frugal))]
        pub(crate) const FIND_REPLACE_BAR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("query", PropertyValueKind::String, true, true),
            PropertySchema::new("replacement", PropertyValueKind::String, true, true),
            PropertySchema::new("match_case", PropertyValueKind::Bool, true, true),
            PropertySchema::new("use_regex", PropertyValueKind::Bool, true, true),
            PropertySchema::new("match_count", PropertyValueKind::UInt, true, true),
            PropertySchema::new("current_match", PropertyValueKind::UInt, true, true),
            PropertySchema::new("replace_visible", PropertyValueKind::Bool, true, true),
            // `open` is the strip's visibility state, and writing it is the same edge-triggered
            // open/close the control's own `open()` / `close()` methods perform.
            PropertySchema::new("open", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Loading / empty / error surface for a list (`COMP-23`).
        ///
        /// `state` is the four-token state machine the reader reports, and `empty_icon` is the
        /// optional built-in icon the surface paints (a static `IconSpec` with no built-in source
        /// is refused by the writer, so the schema declares `IconSpec` rather than `String`).
        #[cfg(not(alloc_frugal))]
        pub(crate) const LIST_STATE_HOST_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::enumerated("state", true, true, &["idle", "loading", "empty", "error"]),
            PropertySchema::new("empty_icon", PropertyValueKind::IconSpec, true, true),
            PropertySchema::new("empty_title", PropertyValueKind::String, true, true),
            PropertySchema::new("empty_hint", PropertyValueKind::String, true, true),
            PropertySchema::new("error_text", PropertyValueKind::String, true, true),
            PropertySchema::new("loading_rows", PropertyValueKind::UInt, true, true),
            PropertySchema::new("pull_progress", PropertyValueKind::Float, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Aggregate row of a table (`COMP-24`).
        ///
        /// Each `aggregates` entry is the `[column_token, kind, label]` triple the reader
        /// produces, so the list kind is the carrier rather than a nested record schema.
        #[cfg(not(alloc_frugal))]
        pub(crate) const SUMMARY_ROW_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("aggregates", PropertyValueKind::StringList, true, true),
            PropertySchema::new("sticky", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Zoom strip beside a chart or a canvas (`COMP-25`).
        #[cfg(not(alloc_frugal))]
        pub(crate) const ZOOM_STRIP_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("zoom_level", PropertyValueKind::Float, true, true),
            PropertySchema::new("min", PropertyValueKind::Float, true, true),
            PropertySchema::new("max", PropertyValueKind::Float, true, true),
            PropertySchema::new("step", PropertyValueKind::Float, true, true),
            PropertySchema::new("show_percentage", PropertyValueKind::Bool, true, true),
            PropertySchema::new("fit_actions", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Per-series axis assignment for a chart (`COMP-26`).
        ///
        /// `assignments` is one edge token per series in `series_names` order, and each `edges`
        /// entry is the `[edge, label, auto_range]` triple the reader produces.
        #[cfg(not(alloc_frugal))]
        pub(crate) const AXIS_PICKER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("series_names", PropertyValueKind::StringList, true, true),
            PropertySchema::new("assignments", PropertyValueKind::StringList, true, true),
            PropertySchema::new("edges", PropertyValueKind::StringList, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Tabular view of a chart's series data (`COMP-27`).
        ///
        /// `rows` is a list of rows, each row a list of cell strings; the vocabulary has no
        /// two-dimensional list kind, so `StringList` is the carrier and the row boundary is the
        /// inner list the reader returns. `rows` and `row_count` are derived from the series
        /// snapshot, so both are read-only (rewriting rows alone would dangle every series index).
        #[cfg(not(alloc_frugal))]
        pub(crate) const CHART_DATA_TABLE_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("headers", PropertyValueKind::StringList, true, true),
            PropertySchema::new("rows", PropertyValueKind::StringList, true, false),
            PropertySchema::new("visible_series", PropertyValueKind::NumberList, true, true),
            PropertySchema::enumerated("mode", true, true, &["preview", "export"]),
            PropertySchema::new("max_rows", PropertyValueKind::UInt, true, true),
            PropertySchema::new("row_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Per-date mark overlay for a calendar (`COMP-28`).
        ///
        /// `mark_count` is the derived total across every date the layer holds, so it is reported
        /// only; the marks themselves are written through the control's Rust-side `set_mark`.
        #[cfg(not(alloc_frugal))]
        pub(crate) const CALENDAR_MARK_LAYER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("year", PropertyValueKind::Int, true, true),
            PropertySchema::new("month", PropertyValueKind::UInt, true, true),
            PropertySchema::new("max_per_cell", PropertyValueKind::UInt, true, true),
            PropertySchema::new("mark_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        /// Translucent proxy card that follows the pointer during a drag (`COMP-29`).
        ///
        /// The gesture geometry (`active`, `ghost_rect`, `origin_rect`) is owned by
        /// `begin`/`follow`/`finish`, so those three are reported only. `drop_bounds` is the
        /// optional acceptance region; the property name is the control's own `drop_bounds`.
        #[cfg(not(alloc_frugal))]
        pub(crate) const DRAG_GHOST_LAYER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("title", PropertyValueKind::String, true, true),
            PropertySchema::new("active", PropertyValueKind::Bool, true, false),
            PropertySchema::new("snap_back", PropertyValueKind::Bool, true, true),
            PropertySchema::new("opacity", PropertyValueKind::Float, true, true),
            PropertySchema::new("drop_bounds", PropertyValueKind::Rect, true, true),
            PropertySchema::new("ghost_rect", PropertyValueKind::Rect, true, false),
            PropertySchema::new("origin_rect", PropertyValueKind::Rect, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];
    };
}
