// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

// Auto-generated from properties.rs — const arrays for widget properties.
// DO NOT EDIT DIRECTLY.

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
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const SPLITTER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::enumerated("orientation", true, true, &["horizontal", "vertical"]),
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
        ];

        pub(crate) const SCROLL_AREA_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("widget_resizable", PropertyValueKind::Bool, false, false),
            PropertySchema::enumerated(
                "horizontal_scroll_bar_policy",
                false,
                false,
                &["always_on", "always_off", "as_needed"],
            ),
            PropertySchema::enumerated(
                "vertical_scroll_bar_policy",
                false,
                false,
                &["always_on", "always_off", "as_needed"],
            ),
            PropertySchema::new("scroll_position_x", PropertyValueKind::Int, false, false),
            PropertySchema::new("scroll_position_y", PropertyValueKind::Int, false, false),
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
            PropertySchema::new("tab_count", PropertyValueKind::UInt, false, false),
            PropertySchema::new("current_index", PropertyValueKind::UInt, false, false),
            PropertySchema::new("closable", PropertyValueKind::Bool, false, false),
            PropertySchema::new("movable", PropertyValueKind::Bool, false, false),
            PropertySchema::enumerated(
                "tab_position",
                false,
                false,
                &["north", "south", "west", "east"],
            ),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const STACKED_WIDGET_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("widget_count", PropertyValueKind::UInt, false, false),
            PropertySchema::new("current_index", PropertyValueKind::UInt, false, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const COLLAPSIBLE_PANE_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("title", PropertyValueKind::String, false, false),
            PropertySchema::new("collapsed", PropertyValueKind::Bool, false, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const DOCK_WIDGET_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("title", PropertyValueKind::String, false, false),
            PropertySchema::new("floating", PropertyValueKind::Bool, false, false),
            PropertySchema::new("docked", PropertyValueKind::Bool, false, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const MDI_AREA_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("subwindow_count", PropertyValueKind::UInt, false, false),
            PropertySchema::new("active_subwindow", PropertyValueKind::UInt, false, false),
            PropertySchema::enumerated(
                "view_mode",
                false,
                false,
                &["list", "icon", "details", "thumbnails"],
            ),
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
    };
}
