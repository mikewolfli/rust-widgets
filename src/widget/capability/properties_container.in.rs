// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

// Hand-maintained const arrays for widget properties.
//
// This header used to claim the file was auto-generated from `properties.rs` with a
// "DO NOT EDIT DIRECTLY" instruction, but no generator exists in `tools/` or
// `scripts/` that writes `.in.rs` files. The only script that touches them
// (`generate_control_route_matrix.py`) *reads* the `create_widgets_*.in.rs` set to
// emit a report, which is a different family. Edit these directly.

macro_rules! impl_properties_container {
    () => {
        pub(crate) const GROUP_BOX_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("title", PropertyValueKind::String, true, true),
            PropertySchema::enumerated(
                "alignment",
                true,
                true,
                &["left", "centre", "right", "top", "bottom"],
            ),
            PropertySchema::new("checkable", PropertyValueKind::Bool, true, true),
            PropertySchema::new("checked", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const FRAME_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::enumerated(
                "frame_shape",
                true,
                true,
                &["no_frame", "box", "panel", "styled_panel", "hline", "vline", "win_panel"],
            ),
            PropertySchema::enumerated("frame_shadow", true, true, &["plain", "raised", "sunken"]),
            PropertySchema::new("line_width", PropertyValueKind::Float, true, true),
            PropertySchema::new("mid_line_width", PropertyValueKind::Float, true, true),
            // An explicit frame width overrides `line_width` for every painted rule; an unset one
            // falls back to it.
            PropertySchema::new("frame_width", PropertyValueKind::Float, true, true),
            PropertySchema::new("contents_margins", PropertyValueKind::String, true, true),
            PropertySchema::new("theme_bevel", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const SPLITTER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::enumerated("orientation", true, true, &["horizontal", "vertical"]),
            // Read-only: the count is derived from the registered panes, and
            // `Splitter::set` answers `ReadOnlyProperty` for it. Declaring it
            // writable made `set_pane_count` a command whose property route refuses
            // the write it names.
            PropertySchema::new("pane_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const TOOL_BOX_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("item_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("current_index", PropertyValueKind::UInt, true, true),
            PropertySchema::enumerated("orientation", true, true, &["horizontal", "vertical"]),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        
            PropertySchema::enumerated("mode", true, true, &["exclusive", "non_exclusive", "popup"]),
        ];

        pub(crate) const SCROLL_AREA_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("widget_resizable", PropertyValueKind::Bool, true, true),
            PropertySchema::enumerated(
                "horizontal_scroll_bar_policy",
                true,
                true,
                &["always_on", "always_off", "as_needed"],
            ),
            PropertySchema::enumerated(
                "vertical_scroll_bar_policy",
                true,
                true,
                &["always_on", "always_off", "as_needed"],
            ),
            PropertySchema::new("scroll_position_x", PropertyValueKind::Int, true, true),
            PropertySchema::new("scroll_position_y", PropertyValueKind::Int, true, true),
            // The viewport content extent. Without these the scroll position is clamped
            // against the control's own content size, but nothing could set that size
            // through the property/handle route.
            PropertySchema::new("content_width", PropertyValueKind::UInt, true, true),
            PropertySchema::new("content_height", PropertyValueKind::UInt, true, true),
            // The sticky list is written through `add_sticky_region` /
            // `clear_sticky_regions`; the count is what the property layer reports,
            // following the same convention as `Meter`'s threshold bands.
            PropertySchema::new("sticky_region_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const TAB_WIDGET_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("tab_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("current_index", PropertyValueKind::UInt, true, true),
            // `text`/`title` are declared because the control **answers** them. `TabWidget::set`
            // applies them to the first tab and `get` reads that tab's title back, so the
            // constructor's `text` parameter reaches a tab instead of being dropped. The schema
            // has to say so: the schema and the contract disagreeing about which properties
            // exist is itself the defect this table is checked for.
            PropertySchema::new("text", PropertyValueKind::String, true, true),
            PropertySchema::new("title", PropertyValueKind::String, true, true),
            // These three used to be declared `false, false`, which was wrong in the
            // harmful direction: the control's own contract refused them, so a host could
            // not turn dragging on (`movable`), show close buttons (`closable`) or move the
            // strip (`tab_position`) through the property API at all — and the `movable`
            // field was consequently read by nothing. They are served now, so they are
            // declared as the two-way properties they are.
            PropertySchema::new("closable", PropertyValueKind::Bool, true, true),
            PropertySchema::new("movable", PropertyValueKind::Bool, true, true),
            PropertySchema::enumerated(
                "tab_position",
                true,
                true,
                &["north", "south", "west", "east"],
            ),
            // Served by `TabWidget::get`/`set` since the shape setter existed; declaring it keeps
            // the schema and the contract from disagreeing about which properties exist.
            PropertySchema::enumerated(
                "tab_shape",
                true,
                true,
                &["rounded", "triangular", "rectangular"],
            ),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        
            PropertySchema::new("document_mode", PropertyValueKind::Bool, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const STACKED_WIDGET_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("widget_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("current_index", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const COLLAPSIBLE_PANE_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("title", PropertyValueKind::String, true, true),
            PropertySchema::new("collapsed", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const DOCK_WIDGET_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("title", PropertyValueKind::String, true, true),
            PropertySchema::new("floating", PropertyValueKind::Bool, true, true),
            // `docked` is writable: the control's `set` routes it through `set_docked`, which keeps
            // the `floating`/`docked` pair consistent and emits the change signal, so writing it is
            // an observable operation rather than a silent field assignment. Declaring it read-only
            // while the control answers a *value* verdict (rather than `ReadOnlyProperty`) is the
            // declaration/implementation disagreement `check_declaration_implementation_alignment`
            // exists to catch; the declaration is the half that was wrong.
            PropertySchema::new("docked", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const MDI_AREA_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("subwindow_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("active_subwindow", PropertyValueKind::UInt, true, false),
            // The reader's two tokens, not a copy of `list_view`'s four.
            //
            // This published `list` / `icon` / `details` / `thumbnails` while `MdiArea`'s
            // reader answers `sub_window_view` / `tabbed`. The two are different properties
            // that happened to share a name — an MDI host switches between sub-windows and tabs,
            // it does not switch between list layouts — so the schema was describing the wrong
            // control entirely. `MdiArea`'s own comment above its reader already said "the two
            // the reader produces"; the table had not been brought in line.
            PropertySchema::enumerated("view_mode", true, false, &["sub_window_view", "tabbed"]),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const STEPPER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("value", PropertyValueKind::Int, true, true),
            PropertySchema::new("minimum", PropertyValueKind::Int, true, true),
            PropertySchema::new("maximum", PropertyValueKind::Int, true, true),
            PropertySchema::new("step", PropertyValueKind::Int, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const MASONRY_LAYOUT_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("column_count", PropertyValueKind::UInt, true, true),
            PropertySchema::new("item_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        
            PropertySchema::new("item_spacing", PropertyValueKind::UInt, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const SAFE_AREA_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("top_inset", PropertyValueKind::Float, true, true),
            PropertySchema::new("bottom_inset", PropertyValueKind::Float, true, true),
            PropertySchema::new("left_inset", PropertyValueKind::Float, true, true),
            PropertySchema::new("right_inset", PropertyValueKind::Float, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const CAROUSEL_PROPERTIES: &[PropertySchema] = &[
            // The page position, the derived page count, the visible page's title,
            // wrap-around, the autoplay interval, and the two indicator enums.
            PropertySchema::new("current_index", PropertyValueKind::UInt, true, true),
            PropertySchema::new("item_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("current_page_title", PropertyValueKind::String, true, false),
            PropertySchema::new("loop", PropertyValueKind::Bool, true, true),
            // Milliseconds; `null` means autoplay is off, which is why this is not
            // split into a bool and an interval that could contradict each other.
            PropertySchema::new("autoplay_interval", PropertyValueKind::UInt, true, true),
            PropertySchema::enumerated(
                "indicator_style",
                true,
                true,
                &["dots", "bars", "numeric", "hidden"],
            ),
            PropertySchema::enumerated(
                "indicator_position",
                true,
                true,
                &["bottom", "top", "left", "right"],
            ),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        // ── Layout family ──────────────────────────────────────────────────
        //
        // The wrapper controls share three content property names; the distribution controls
        // share `item_count` and the per-axis tokens. Every enum below lists exactly the
        // tokens its parser accepts, so a value read from the schema can be written back.

        #[cfg(not(alloc_frugal))]
        pub(crate) const SPACER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("flex", PropertyValueKind::UInt, true, true),
            PropertySchema::new("base_size", PropertyValueKind::UInt, true, true),
            // The design-time dashed frame. A shipping application turns it off.
            PropertySchema::new("paint_hint", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        
            PropertySchema::new("widget", PropertyValueKind::UInt, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const CENTER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("content", PropertyValueKind::String, true, true),
            PropertySchema::new("content_width", PropertyValueKind::UInt, true, true),
            PropertySchema::new("content_height", PropertyValueKind::UInt, true, true),
            PropertySchema::new("width_factor", PropertyValueKind::Float, true, true),
            PropertySchema::new("height_factor", PropertyValueKind::Float, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        
            PropertySchema::new("widget", PropertyValueKind::UInt, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const ALIGN_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("content", PropertyValueKind::String, true, true),
            PropertySchema::new("content_width", PropertyValueKind::UInt, true, true),
            PropertySchema::new("content_height", PropertyValueKind::UInt, true, true),
            PropertySchema::enumerated(
                "alignment",
                true,
                true,
                &[
                    "top_left",
                    "top_centre",
                    "top_right",
                    "centre_left",
                    "centre",
                    "centre_right",
                    "bottom_left",
                    "bottom_centre",
                    "bottom_right",
                ],
            ),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        
            PropertySchema::new("widget", PropertyValueKind::UInt, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const SIZED_BOX_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("content", PropertyValueKind::String, true, true),
            PropertySchema::new("content_width", PropertyValueKind::UInt, true, true),
            PropertySchema::new("content_height", PropertyValueKind::UInt, true, true),
            PropertySchema::new("box_width", PropertyValueKind::UInt, true, true),
            PropertySchema::new("box_height", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        
            PropertySchema::new("widget", PropertyValueKind::UInt, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const PADDING_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("content", PropertyValueKind::String, true, true),
            PropertySchema::new("content_width", PropertyValueKind::UInt, true, true),
            PropertySchema::new("content_height", PropertyValueKind::UInt, true, true),
            PropertySchema::new("padding_top", PropertyValueKind::UInt, true, true),
            PropertySchema::new("padding_right", PropertyValueKind::UInt, true, true),
            PropertySchema::new("padding_bottom", PropertyValueKind::UInt, true, true),
            PropertySchema::new("padding_left", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        
            PropertySchema::new("widget", PropertyValueKind::UInt, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const EXPANDED_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("content", PropertyValueKind::String, true, true),
            PropertySchema::new("content_width", PropertyValueKind::UInt, true, true),
            PropertySchema::new("content_height", PropertyValueKind::UInt, true, true),
            PropertySchema::new("flex", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        
            PropertySchema::new("widget", PropertyValueKind::UInt, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const FLEXIBLE_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("content", PropertyValueKind::String, true, true),
            PropertySchema::new("content_width", PropertyValueKind::UInt, true, true),
            PropertySchema::new("content_height", PropertyValueKind::UInt, true, true),
            PropertySchema::new("flex", PropertyValueKind::UInt, true, true),
            PropertySchema::enumerated("fit", true, true, &["loose", "tight"]),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        
            PropertySchema::new("widget", PropertyValueKind::UInt, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const INTRINSIC_WIDTH_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("content", PropertyValueKind::String, true, true),
            PropertySchema::new("content_width", PropertyValueKind::UInt, true, true),
            PropertySchema::new("content_height", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        
            PropertySchema::new("widget", PropertyValueKind::UInt, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const ASPECT_RATIO_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("content", PropertyValueKind::String, true, true),
            PropertySchema::new("content_width", PropertyValueKind::UInt, true, true),
            PropertySchema::new("content_height", PropertyValueKind::UInt, true, true),
            PropertySchema::new("ratio", PropertyValueKind::Float, true, true),
            PropertySchema::new("respect_parent", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        
            PropertySchema::new("widget", PropertyValueKind::UInt, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const FLEX_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("item_count", PropertyValueKind::UInt, true, false),
            PropertySchema::enumerated(
                "direction",
                true,
                true,
                &["row", "row_reverse", "column", "column_reverse"],
            ),
            PropertySchema::enumerated("wrap", true, true, &["no_wrap", "wrap", "wrap_reverse"]),
            PropertySchema::enumerated(
                "justify_content",
                true,
                true,
                &[
                    "flex_start",
                    "flex_end",
                    "center",
                    "space_between",
                    "space_around",
                    "space_evenly",
                ],
            ),
            PropertySchema::enumerated(
                "align_items",
                true,
                true,
                &["stretch", "flex_start", "flex_end", "center", "baseline"],
            ),
            PropertySchema::new("gap", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const FLOW_LAYOUT_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("item_count", PropertyValueKind::UInt, true, false),
            PropertySchema::enumerated("direction", true, true, &["horizontal", "vertical"]),
            PropertySchema::enumerated(
                "alignment",
                true,
                true,
                &["start", "center", "end", "space_between", "space_around"],
            ),
            PropertySchema::new("spacing", PropertyValueKind::UInt, true, true),
            PropertySchema::new("padding", PropertyValueKind::UInt, true, true),
            PropertySchema::new("wrap", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const WRAP_LAYOUT_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("item_count", PropertyValueKind::UInt, true, false),
            PropertySchema::enumerated("direction", true, true, &["horizontal", "vertical"]),
            PropertySchema::enumerated(
                "alignment",
                true,
                true,
                &["start", "center", "end", "space_between", "space_around"],
            ),
            PropertySchema::new("spacing", PropertyValueKind::UInt, true, true),
            // `null` means "follow the item spacing"; it is a documented state, not an
            // impossible sentinel.
            PropertySchema::new("run_spacing", PropertyValueKind::UInt, true, true),
            PropertySchema::new("padding", PropertyValueKind::UInt, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const ACCORDION_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("section_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("single_expand", PropertyValueKind::Bool, true, true),
            PropertySchema::new("expanded_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];
    };
}
