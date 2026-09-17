// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

// Auto-generated from properties.rs — const arrays for widget properties.
// DO NOT EDIT DIRECTLY.

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
            PropertySchema::enumerated("mode", true, true, &["hex", "decimal", "octal", "binary"]),
            PropertySchema::enumerated("segment_style", true, true, &["outline", "filled", "flat"]),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const CODE_EDITOR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("text", PropertyValueKind::String, true, true),
            PropertySchema::new("line_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("cursor_line", PropertyValueKind::UInt, true, false),
            PropertySchema::new("cursor_column", PropertyValueKind::UInt, true, false),
            PropertySchema::new("marker_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const GANTT_WIDGET_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("task_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("selected_id", PropertyValueKind::String, true, false),
            PropertySchema::new("viewport_start", PropertyValueKind::Int, true, true),
            PropertySchema::new("viewport_end", PropertyValueKind::Int, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const TERMINAL_VIEW_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("output_line_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("input_line", PropertyValueKind::String, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const SNACKBAR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("message", PropertyValueKind::String, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("action_label", PropertyValueKind::String, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const MAP_VIEW_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("center_x", PropertyValueKind::Float, true, true),
            PropertySchema::new("center_y", PropertyValueKind::Float, true, true),
            PropertySchema::new("zoom", PropertyValueKind::Float, true, true),
            PropertySchema::new("marker_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("selected_marker_id", PropertyValueKind::String, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
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
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const BREADCRUMB_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("segment_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("selected_index", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
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
            PropertySchema::new("text", PropertyValueKind::String, true, true),
            PropertySchema::new("action_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("menu_open", PropertyValueKind::Bool, true, true),
            PropertySchema::new("row_height", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const SEGMENTED_CONTROL_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("item_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("selected_index", PropertyValueKind::UInt, true, false),
            PropertySchema::new("selected_id", PropertyValueKind::String, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const CHIP_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("item_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("multi_select", PropertyValueKind::Bool, true, true),
            PropertySchema::new("focused_index", PropertyValueKind::UInt, true, false),
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
            PropertySchema::new("item_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("selected_index", PropertyValueKind::UInt, true, false),
            PropertySchema::new("viewport_start", PropertyValueKind::Int, true, true),
            PropertySchema::new("viewport_end", PropertyValueKind::Int, true, true),
            PropertySchema::new("row_height", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        // `CommandPalette` is a queryable command list: the filtered result set,
        // not the raw entry list, is what a consumer can act on, so it publishes
        // the filter state rather than the entries themselves (those are added
        // through `set_entries`).
        #[cfg(not(alloc_frugal))]
        pub(crate) const COMMAND_PALETTE_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("query", PropertyValueKind::String, true, true),
            PropertySchema::new("entry_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("filtered_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("highlighted_index", PropertyValueKind::UInt, true, false),
            PropertySchema::new("row_height", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        // `NotificationCenter` publishes its unread state and selection. The item
        // list itself is reachable through `push` / `clear` / `set_read`; these
        // names report the derived state a badge or list header needs.
        #[cfg(not(alloc_frugal))]
        pub(crate) const NOTIFICATION_CENTER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("item_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("unread_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("selected_index", PropertyValueKind::UInt, true, false),
            PropertySchema::new("row_height", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
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
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const GRID_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("rows", PropertyValueKind::UInt, true, true),
            PropertySchema::new("columns", PropertyValueKind::UInt, true, true),
            PropertySchema::new("spacing", PropertyValueKind::UInt, true, true),
            PropertySchema::new("line_color", PropertyValueKind::String, true, true),
            PropertySchema::new("cell_width", PropertyValueKind::UInt, true, false),
            PropertySchema::new("cell_height", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
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
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const WEB_VIEW_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("url", PropertyValueKind::String, false, false),
            PropertySchema::new("loading", PropertyValueKind::Bool, false, false),
            PropertySchema::new("title", PropertyValueKind::String, false, false),
            PropertySchema::new("can_go_back", PropertyValueKind::Bool, false, false),
            PropertySchema::new("can_go_forward", PropertyValueKind::Bool, false, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        pub(crate) const ARC_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("value", PropertyValueKind::UInt, true, true),
            PropertySchema::new("minimum", PropertyValueKind::UInt, true, true),
            PropertySchema::new("maximum", PropertyValueKind::UInt, true, true),
            PropertySchema::new("thickness", PropertyValueKind::UInt, true, true),
            PropertySchema::new("sweep_angle", PropertyValueKind::UInt, true, true),
            PropertySchema::new("indeterminate", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const MENTION_PROPERTIES: &[PropertySchema] = &[
            // The candidates and the text are written through `set_candidates` /
            // `set_text`; the counts and the trigger are what the property layer
            // reports. Same convention as the other list-valued controls.
            PropertySchema::new("text", PropertyValueKind::String, true, true),
            PropertySchema::new("trigger", PropertyValueKind::String, true, true),
            PropertySchema::new("candidate_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("visible_candidate_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("mention_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("popup_open", PropertyValueKind::Bool, true, false),
            PropertySchema::new("caret", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const EMOJI_PICKER_PROPERTIES: &[PropertySchema] = &[
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
            // The fields and rows are lists, written through `set_fields` /
            // `add_row` and the per-row setters; the counts are what the property
            // layer reports. Same convention as `Meter`/`RadarChart`/`KanbanBoard`.
            PropertySchema::new("row_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("incomplete_row_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("field_count", PropertyValueKind::UInt, true, false),
            PropertySchema::enumerated("conjunction", true, true, &["and", "or"]),
            PropertySchema::new("active_row", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const CASCADER_PROPERTIES: &[PropertySchema] = &[
            // The selected path is a sequence, written through `set_selected_path`;
            // what the property layer reports is the joined text, the derived depth,
            // and the three scalar display switches. Same convention as `Meter`,
            // `RadarChart` and `KanbanBoard`.
            PropertySchema::new("value", PropertyValueKind::String, true, false),
            PropertySchema::new("depth", PropertyValueKind::UInt, true, false),
            PropertySchema::new("expanded", PropertyValueKind::Bool, true, true),
            PropertySchema::new("separator", PropertyValueKind::String, true, true),
            PropertySchema::new("filterable", PropertyValueKind::Bool, true, true),
            PropertySchema::new("filter", PropertyValueKind::String, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const KANBAN_BOARD_PROPERTIES: &[PropertySchema] = &[
            // The columns and cards are two-level lists, written through
            // `add_column` / `add_card`; the counts are what the property layer
            // reports, following the same convention as `Meter` and `RadarChart`.
            PropertySchema::new("column_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("card_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("dragging_card_id", PropertyValueKind::String, true, false),
            PropertySchema::new("hovered_column", PropertyValueKind::UInt, true, false),
            PropertySchema::new("column_width", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
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
            // `MiniChart` supports a strict subset of `Chart`'s types: its own `set`
            // arm accepts only `line` and `bar`, so publishing `pie`/`scatter` here
            // would advertise values the control refuses.
            PropertySchema::enumerated("chart_type", true, true, &["line", "bar"]),
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
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const SKELETON_LOADER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("active", PropertyValueKind::Bool, true, true),
            PropertySchema::enumerated("shape", true, true, &["rect", "circle", "text_line"]),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const FAB_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("icon", PropertyValueKind::String, true, true),
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
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const APP_BAR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("title", PropertyValueKind::String, true, true),
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
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const DIVIDER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("orientation", PropertyValueKind::String, true, true),
            PropertySchema::new("thickness", PropertyValueKind::Float, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const RATING_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("value", PropertyValueKind::Float, true, true),
            PropertySchema::new("max", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const AVATAR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("initials", PropertyValueKind::String, true, true),
            PropertySchema::new("image_source", PropertyValueKind::String, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const EMPTY_STATE_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("message", PropertyValueKind::String, true, true),
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
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const QR_CODE_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("data", PropertyValueKind::String, true, true),
            PropertySchema::new("size", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const MATERIAL_SNACKBAR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("message", PropertyValueKind::String, true, true),
            PropertySchema::new("action_text", PropertyValueKind::String, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const ADAPTIVE_SCAFFOLD_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("title", PropertyValueKind::String, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const SEGMENTED_BUTTON_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("selected_index", PropertyValueKind::UInt, true, true),
            PropertySchema::new("segment_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const NAVIGATION_STACK_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("page_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("current_page", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const PROGRESS_CIRCLE_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("value", PropertyValueKind::Float, true, true),
            PropertySchema::new("thickness", PropertyValueKind::Float, true, true),
            PropertySchema::new("indeterminate", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const ICON_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("icon_name", PropertyValueKind::String, true, true),
            PropertySchema::new("size", PropertyValueKind::Float, true, true),
            PropertySchema::new("color", PropertyValueKind::String, true, true),
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
            PropertySchema::new("selected_index", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
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
            PropertySchema::new("stroke_width", PropertyValueKind::Float, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
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
    };
}
