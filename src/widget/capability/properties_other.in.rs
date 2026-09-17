// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

// Auto-generated from properties.rs — const arrays for widget properties.
// DO NOT EDIT DIRECTLY.

macro_rules! impl_properties_other {
    () => {
        pub(crate) const WINDOW_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "title",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "title_bar_height",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "close_button_size",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "button_spacing",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const LCD_NUMBER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "value",
                value_kind: PropertyValueKind::Float,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "min_value",
                value_kind: PropertyValueKind::Float,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "max_value",
                value_kind: PropertyValueKind::Float,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "num_digits",
                value_kind: PropertyValueKind::Int,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "small_decimal_point",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "mode",
                value_kind: PropertyValueKind::Enum,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "segment_style",
                value_kind: PropertyValueKind::Enum,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const CODE_EDITOR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "text",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "line_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "cursor_line",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "cursor_column",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "marker_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const GANTT_WIDGET_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "task_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "selected_id",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "viewport_start",
                value_kind: PropertyValueKind::Int,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "viewport_end",
                value_kind: PropertyValueKind::Int,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const TERMINAL_VIEW_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "output_line_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "input_line",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const SNACKBAR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "message",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "action_label",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const MAP_VIEW_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "center_x",
                value_kind: PropertyValueKind::Float,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "center_y",
                value_kind: PropertyValueKind::Float,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "zoom",
                value_kind: PropertyValueKind::Float,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "marker_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "selected_marker_id",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const MEDIA_PLAYER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "source",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "playing",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "duration_ms",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "position_ms",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "volume",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "muted",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "fullscreen",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const BREADCRUMB_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "segment_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "selected_index",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const SPLIT_BUTTON_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "text",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "action_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "menu_open",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "row_height",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const SEGMENTED_CONTROL_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "item_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "selected_index",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "selected_id",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const CHIP_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "item_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "multi_select",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "focused_index",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "selected_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        // `TimelineWidget` publishes the shared four plus its own viewport and row
        // metrics. `item_count` / `selected_index` / `viewport_start` / `viewport_end` /
        // `row_height` are all answered by its `WidgetProperties` impl; `selected_index`
        // is deliberately read-only because selecting is a command-shaped operation
        // (`select_index(i)` returns whether the index was in range) rather than an
        // assignment a caller can undo by writing a different number.
        #[cfg(not(alloc_frugal))]
        pub(crate) const TIMELINE_WIDGET_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "item_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "selected_index",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "viewport_start",
                value_kind: PropertyValueKind::Int,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "viewport_end",
                value_kind: PropertyValueKind::Int,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "row_height",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        // `CommandPalette` is a queryable command list: the filtered result set,
        // not the raw entry list, is what a consumer can act on, so it publishes
        // the filter state rather than the entries themselves (those are added
        // through `set_entries`).
        #[cfg(not(alloc_frugal))]
        pub(crate) const COMMAND_PALETTE_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "query",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "entry_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "filtered_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "highlighted_index",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "row_height",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        // `NotificationCenter` publishes its unread state and selection. The item
        // list itself is reachable through `push` / `clear` / `set_read`; these
        // names report the derived state a badge or list header needs.
        #[cfg(not(alloc_frugal))]
        pub(crate) const NOTIFICATION_CENTER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "item_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "unread_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "selected_index",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "row_height",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        // `DiffViewer` compares two snapshots. Both texts are readable and
        // writable (writing either recomputes the diff), and `change_count` /
        // `line_count` report the result. `selected_index` stays read-only for the
        // same reason as the timeline's.
        #[cfg(not(alloc_frugal))]
        pub(crate) const DIFF_VIEWER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "left_text",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "right_text",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "line_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "change_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "selected_index",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        // `ToastStack` publishes the stack depth and the selected toast id. The
        // id is a string because that is what `selected_id()` returns and what the
        // `toast_activated` / `toast_dismissed` signals carry.
        #[cfg(not(alloc_frugal))]
        pub(crate) const TOAST_STACK_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "toast_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "selected_id",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "row_height",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const GRID_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "rows",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "columns",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "spacing",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "line_color",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "cell_width",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "cell_height",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const FREEFORM_SHAPE_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "path_kind",
                value_kind: PropertyValueKind::Enum,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "fill_rgba",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "stroke_rgba",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "stroke_width",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const WEB_VIEW_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "url",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
            PropertySchema {
                name: "loading",
                value_kind: PropertyValueKind::Bool,
                readable: false,
                writable: false,
            },
            PropertySchema {
                name: "title",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
            PropertySchema {
                name: "can_go_back",
                value_kind: PropertyValueKind::Bool,
                readable: false,
                writable: false,
            },
            PropertySchema {
                name: "can_go_forward",
                value_kind: PropertyValueKind::Bool,
                readable: false,
                writable: false,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        pub(crate) const ARC_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "value",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "minimum",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "maximum",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "thickness",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "sweep_angle",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "indeterminate",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        pub(crate) const METER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "value",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "minimum",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "maximum",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        pub(crate) const MINI_CHART_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "chart_type",
                value_kind: PropertyValueKind::Enum,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        pub(crate) const MINI_CANVAS_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
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
            PropertySchema {
                name: "command_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        // `ChartWidget` holds a numeric series plus its labels. It previously carried
        // only `selected_marker_id` — a leftover from a marker concept this type
        // never had — so a caller could discover the control but not read its data.
        pub(crate) const CHART_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "chart_type",
                value_kind: PropertyValueKind::Enum,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "point_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "label_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const BADGE_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "text",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "count",
                value_kind: PropertyValueKind::Int,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const SKELETON_LOADER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "active",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "shape",
                value_kind: PropertyValueKind::Enum,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const FAB_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "icon",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const BOTTOM_NAVIGATION_BAR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "item_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "selected_index",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const NAVIGATION_DRAWER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "open",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "width",
                value_kind: PropertyValueKind::Float,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const APP_BAR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "title",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const MOBILE_DATE_PICKER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "selected_date",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const DIVIDER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "orientation",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "thickness",
                value_kind: PropertyValueKind::Float,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const RATING_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "value",
                value_kind: PropertyValueKind::Float,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "max",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const AVATAR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "initials",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "image_source",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const EMPTY_STATE_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "message",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "description",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const COLOR_HISTORY_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "color_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const COLOR_WELL_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "color",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const QR_CODE_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "data",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "size",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const MATERIAL_SNACKBAR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "message",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "action_text",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const ADAPTIVE_SCAFFOLD_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "title",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const SEGMENTED_BUTTON_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "selected_index",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "segment_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const NAVIGATION_STACK_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "page_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "current_page",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const PROGRESS_CIRCLE_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "value",
                value_kind: PropertyValueKind::Float,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "thickness",
                value_kind: PropertyValueKind::Float,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "indeterminate",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const ICON_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "icon_name",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "size",
                value_kind: PropertyValueKind::Float,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "color",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const FONT_PREVIEW_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "font_family",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "font_size",
                value_kind: PropertyValueKind::Float,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "preview_text",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const CUPERTINO_NAVIGATION_BAR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "title",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "large_title",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const REFRESH_CONTROL_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "refreshing",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const DATE_RANGE_PICKER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "start_date",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "end_date",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const MATERIAL_NAVIGATION_RAIL_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "selected_index",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const TAB_VIEW_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "selected_index",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const SWIPE_TO_DISMISS_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "is_dismissed",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const LINE_CHART_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "stroke_width",
                value_kind: PropertyValueKind::Float,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const SPARKLINE_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "stroke_width",
                value_kind: PropertyValueKind::Float,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const BAR_CHART_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "bar_spacing",
                value_kind: PropertyValueKind::Float,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const PIE_CHART_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "donut",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const BEZIER_CURVE_EDITOR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "snap_to_grid",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const BARCODE_SCANNER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "is_scanning",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];
    };
}
