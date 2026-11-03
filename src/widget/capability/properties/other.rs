// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

// Capability constructors for the `other` widgets.
//
// The property arrays these constructors reference are declared in
// `properties_other.in.rs` and expanded by the parent module. `use super::*` brings
// those arrays and the shared imports into scope; `events_of!` is visible through the
// parent module's textual macro scope.

use super::*;

pub(crate) fn window_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Window,
        canonical_name: "window",
        aliases: &["main_window"],
        properties: WINDOW_PROPERTIES,
        events: events_of!("window"),
        commands: &[
            "set_title",
            "close",
            "set_button_spacing",
            "set_close_button_size",
            "set_title_bar_height",
        ],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn lcd_number_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::LCDNumber,
        canonical_name: "lcd_number",
        aliases: &[],
        properties: LCD_NUMBER_PROPERTIES,
        events: events_of!("lcd_number"),
        commands: &[
            "set_value",
            "set_mode",
            "set_segment_style",
            "set_max_value",
            "set_min_value",
            "set_num_digits",
            "set_small_decimal_point",
        ],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn code_editor_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::RichEdit,
        canonical_name: "code_editor",
        aliases: &[],
        properties: CODE_EDITOR_PROPERTIES,
        events: events_of!("code_editor"),
        // set_markers now carries its list through the `markers` records property, and the caret is
        // writable through `cursor_line` / `cursor_column` (B26.03). `append_line` has no property
        // route (the document is the writable `text`), and `set_cursor` names no `cursor` property
        // — its two components are the honest route.
        commands: &[
            "set_text",
            "set_markers",
            "set_cursor_line",
            "set_cursor_column",
            "undo",
            "redo",
            "copy",
            "cut",
            "paste",
            "select_all",
            "find",
            "replace",
            "find_next",
            "find_previous",
            "toggle_comment",
            "toggle_block_comment",
            "duplicate_line",
            "delete_line",
            "indent",
            "outdent",
            "sort_lines",
            "fold_all",
            "unfold_all",
            "complete",
            "close_find",
            "set_breakpoint_gutter",
            "set_goto_bar_visible",
        ],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn snackbar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::StatusBar,
        canonical_name: "snackbar",
        aliases: &["toast_bar"],
        properties: SNACKBAR_PROPERTIES,
        events: events_of!("snackbar"),
        // Audit 20261026: `show` stays unpublished — the snackbar publishes the writable `message`
        // and `visible` properties, and that pair is exactly what `show` assigns (principle #18).
        // `show_with_action` additionally takes an action label, and `action_label` is read-only,
        // so it has no complete property route and also stays unpublished.
        commands: &["dismiss", "set_anchor", "set_max_queued"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn breadcrumb_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Breadcrumb,
        canonical_name: "breadcrumb",
        aliases: &["nav_breadcrumb"],
        properties: BREADCRUMB_PROPERTIES,
        events: events_of!("breadcrumb"),
        // Audit 20261026: `push_segment` stays unpublished — the whole trail is the writable
        // `segments` records property and `set_segments` is its published command form, so an
        // appended segment is a whole-trail write (principle #18).
        commands: &["set_segments", "clear_segments", "set_separator"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn signature_pad_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::SignaturePad,
        canonical_name: "signature_pad",
        aliases: &["signature"],
        properties: SIGNATURE_PAD_PROPERTIES,
        events: events_of!("signature_pad"),
        commands: &[
            "undo",
            "set_min_point_distance",
            "set_min_point_interval_ms",
            "set_pressure_affects_width",
            "set_stroke_color",
            "set_stroke_width",
        ],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn drop_zone_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::DropZone,
        canonical_name: "drop_zone",
        aliases: &["drop_target"],
        properties: DROP_ZONE_PROPERTIES,
        events: events_of!("drop_zone"),
        commands: &[
            "set_accepted_type",
            "set_hover_style",
            "set_multi_file",
            "set_show_count_badge",
            "set_state",
        ],
    }
}

// The six capabilities below describe controls that were implemented but never
// registered, so nothing could construct them by name (no JSON node, no CSS
// selector target, no factory lookup). Registration is the whole fix; the
// `commands` lists name only methods that actually exist on each type, because a
// command name a consumer cannot invoke is the same defect shape as an
// unreachable control.
#[cfg(not(alloc_frugal))]
pub(crate) fn timeline_widget_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Chart,
        canonical_name: "timeline_widget",
        aliases: &["timeline", "timeline_view"],
        properties: TIMELINE_WIDGET_PROPERTIES,
        events: events_of!("timeline_widget"),
        // Audit 20261026: `set_items` is published now that the `items` records property is its
        // payload channel (B26.03). `set_viewport` stays unpublished: the name does not resolve to
        // a `viewport` property — the two bounds are the writable `viewport_start` /
        // `viewport_end` properties, which is the honest route. `select_index` names a position
        // with no property route.
        commands: &[
            "zoom",
            "set_items",
            "set_lane_mode",
            "set_row_height",
            "set_show_time_labels",
            "set_scroll_row",
        ],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn command_palette_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ListView,
        canonical_name: "command_palette",
        aliases: &["command_box"],
        properties: COMMAND_PALETTE_PROPERTIES,
        events: events_of!("command_palette"),
        // set_entries now carries its list through the `entries` records property (B26.03);
        // move_highlight takes a signed delta with no property route, so it stays unpublished.
        commands: &[
            "set_query",
            "clear_query",
            "activate_highlighted",
            "set_entries",
            "set_group_by_category",
            "set_row_height",
            "set_scroll_row",
        ],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn notification_center_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ListView,
        canonical_name: "notification_center",
        aliases: &["notifications"],
        properties: NOTIFICATION_CENTER_PROPERTIES,
        events: events_of!("notification_center"),
        // Audit 20261026: `push` and `set_read` stay unpublished — their payloads are an item and
        // an id+flag, neither of which the bare command channel can carry; the `items` records
        // property is that payload channel (B26.03), and `set_items` is its published command
        // form. `select_index` names a position with no property route.
        commands: &[
            "clear",
            "mark_all_read",
            "activate_selected",
            "set_items",
            "set_level_filter",
            "set_row_height",
            "set_max_items",
            "set_scroll_row",
        ],
    }
}

/// The markdown editor's formatting strip (SP-022).
///
/// A separate control rather than a mode of `tool_bar`: its action set is the markdown token
/// vocabulary, its cells execute through [`MarkdownToolbar::execute`], and its single published
/// event carries the activated token. It shares `WidgetKind::ToolBar` with `tool_bar` and
/// `mac_toolbar` because the theme role and the accessibility role are the same, so the concrete
/// type is what distinguishes the three.
///
/// [`MarkdownToolbar::execute`]: crate::widget::special_widgets::markdown_editor::MarkdownToolbar::execute
#[cfg(not(alloc_frugal))]
pub(crate) fn markdown_toolbar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ToolBar,
        canonical_name: "markdown_toolbar",
        aliases: &["md_toolbar"],
        properties: MARKDOWN_TOOLBAR_PROPERTIES,
        events: events_of!("markdown_toolbar"),
        // The action list is the writable `actions` StringList, so `set_actions` is a property
        // write. `execute` takes the token itself and has no property carrier, so it stays a
        // Rust-side entry point (principle #18).
        commands: &["set_icon_side", "set_show_labels"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn toast_stack_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::PopupWindow,
        canonical_name: "toast_stack",
        aliases: &["toasts"],
        properties: TOAST_STACK_PROPERTIES,
        events: events_of!("toast_stack"),
        // Audit 20261026: `push` and `select_index` stay unpublished — a toast is a whole item the
        // stack owns and the stack publishes neither an item list nor a writable selection
        // (`toast_count` / `selected_id` are read-only); the control's own API is the write path
        // (principle #18).
        commands: &[
            "clear",
            "activate_selected",
            "dismiss_selected",
            "set_anchor",
            "set_stacking_direction",
            "set_max_visible",
            "set_scroll_row",
        ],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn split_button_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ToolButton,
        canonical_name: "split_button",
        aliases: &[],
        properties: SPLIT_BUTTON_PROPERTIES,
        events: events_of!("split_button"),
        // Audit 20261026: `add_action` stays unpublished — it takes an action the bare command
        // channel cannot carry. `set_actions` is the published form of a whole-list write: its
        // rows travel through the `actions` records property (B26.03). `trigger_primary` stays
        // published — its arm runs the control's real primary trigger.
        commands: &[
            "open_menu",
            "close_menu",
            "trigger_primary",
            "set_actions",
            "set_max_menu_height",
            "set_menu_scroll",
            "set_row_height",
            "set_popup_direction",
            "set_text",
        ],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn segmented_control_capability() -> WidgetCapability {
    WidgetCapability {
        // Its own kind. This row used to declare `WidgetKind::ToggleButton`, which left
        // `capability_by_kind(ToggleButton)` ambiguous between two controls and made the
        // accessibility role of a segmented bar answer "button".
        kind: WidgetKind::SegmentedControl,
        canonical_name: "segmented_control",
        aliases: &[],
        properties: SEGMENTED_CONTROL_PROPERTIES,
        events: events_of!("segmented_control"),
        // set_items carries its payload through the `items` records property (B26.03): the bare
        // command reports that a value is what is missing, and write_property("items", ...) is
        // where the list actually goes. move_selection takes a signed delta and has no property
        // route, so it stays unpublished.
        commands: &["set_items", "set_min_segment_width", "set_size_to_fit", "set_multi_select"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn chip_capability() -> WidgetCapability {
    WidgetCapability {
        // `WidgetKind::Chip` names this control. It previously declared
        // `CheckListBox` — which is a *type alias for `ListBox`* — so the lookup for
        // `Chip` found nothing and the lookup for `CheckListBox` was ambiguous
        // between this entry and the real list box. See the alias table in
        // `factory_name_for_kind` for the other half of that fix.
        kind: WidgetKind::Chip,
        canonical_name: "chip",
        aliases: &["chips"],
        properties: CHIP_PROPERTIES,
        events: events_of!("chip"),
        // Audit 20261026: `toggle_index` and `move_focus` stay unpublished — each takes a payload
        // (an index, a signed delta) the bare command channel cannot carry; a selection toggle is
        // expressed by writing the whole `items` list with `selected` flipped, and the keyboard
        // focus by writing `focused_index`, whose `set_focused_index` command form is published.
        commands: &["set_items", "set_focused_index", "set_multi_select"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn grid_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Grid,
        canonical_name: "grid",
        aliases: &["grid_widget", "gridwidget"],
        properties: GRID_PROPERTIES,
        events: events_of!("grid"),
        // `select_cell` and `clear_selection` were advertised but no such method
        // exists on `Grid`, so they named actions nobody could obtain. Removed
        // rather than left as a promise with no implementation (principle #18);
        // the cell-click path is covered by the `cell_clicked` event.
        commands: &["set_rows", "set_columns", "set_spacing", "set_line_color", "set_page"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn freeform_shape_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::FreeformShape,
        canonical_name: "freeform_shape",
        commands: &["set_fill_rgba", "set_stroke_rgba", "set_stroke_width", "set_movable"],
        aliases: &[],
        properties: FREEFORM_SHAPE_PROPERTIES,
        events: events_of!("freeform_shape"),
    }
}

pub(crate) fn arc_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Arc,
        canonical_name: "arc",
        aliases: &["arc_widget"],
        properties: ARC_PROPERTIES,
        events: events_of!("arc"),
        // Audit 20261026: `set_range` stays unpublished — the range is the writable
        // `minimum`/`maximum` pair, so a whole-range write is reachable through the property route
        // (principle #18).
        commands: &[
            "set_value",
            "set_indeterminate",
            "set_maximum",
            "set_minimum",
            "set_rounded",
            "set_show_value",
            "set_start_angle",
            "set_sweep_angle",
            "set_thickness",
        ],
    }
}

pub(crate) fn meter_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Meter,
        canonical_name: "meter",
        // `gauge` is a separate canonical control (`gauge_capability`), so it cannot also
        // be a `meter` alias: normalized names must have exactly one owner.
        aliases: &["meter_widget"],
        properties: METER_PROPERTIES,
        events: events_of!("meter"),
        // Audit 20261026: `set_range` stays unpublished — the range is the writable
        // `minimum`/`maximum` pair, so a whole-range write is reachable through the property route
        // (principle #18).
        // command arm stays as a defensive refusal.
        commands: &[
            "set_value",
            "set_maximum",
            "set_minimum",
            "set_show_tick_labels",
            "set_tick_count",
            "set_unit",
        ],
    }
}

/// `RadarChart` — series plotted over shared dimension axes.
///
/// Not registered as a `chart_type` token: the data model differs (see the
/// module docs), so it is its own control rather than a style of `ChartWidget`.
#[cfg(not(alloc_frugal))]
pub(crate) fn radar_chart_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::RadarChart,
        canonical_name: "radar_chart",
        aliases: &["radar", "spider_chart"],
        properties: RADAR_CHART_PROPERTIES,
        events: events_of!("radar_chart"),
        // Audit 20261026: `set_axes`, `set_series`, `add_series` stay unpublished — a series carries
        // a numeric sample list and an axis carries a label list, and the chart publishes only the
        // writable `series_names` StringList plus derived counts; the sample matrix has no property
        // carrier (principle #18).
        commands: &["set_show_axis_labels", "set_show_grid", "set_show_legend"],
    }
}

/// `Heatmap` — a matrix of values rendered as a grid of coloured cells.
///
/// Not registered as a `chart_type` token: the data model differs (two **categorical**
/// axes and one value per coordinate pair), so it is its own control rather than a style
/// of `ChartWidget`. See the module docs of `chart_widgets::heatmap`.
#[cfg(not(alloc_frugal))]
pub(crate) fn heatmap_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Heatmap,
        canonical_name: "heatmap",
        // `heat_map` is deliberately *not* listed: `normalize_key` strips `_`, so both
        // spellings already reach this row, and an alias that normalises to its own
        // canonical name is a no-op (`capability_alias_hygiene_test`).
        aliases: &["heatmap_chart"],
        properties: HEATMAP_PROPERTIES,
        events: events_of!("heatmap"),
        // Audit 20261026: `set_data`, `set_cell` stay unpublished — the matrix is a nested numeric
        // grid with no property carrier (a `NumberList` is flat), and `set_cell` names a position
        // while the published cell properties are the scale and viewport, not a cell value
        // (principle #18).
        commands: &[
            "clear",
            "set_scale_maximum",
            "set_scale_minimum",
            "set_show_labels",
            "set_viewport_columns",
            "set_viewport_rows",
            "set_show_legend",
        ],
    }
}

/// `Mention` — completes `@`-mentions from a candidate list.
///
/// Registered separately from `auto_complete_edit`: the completion model differs
/// (token before the caret, many mentions per field), which is the plan's §3 A2
/// judgment.
#[cfg(not(alloc_frugal))]
pub(crate) fn mention_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Mention,
        canonical_name: "mention",
        aliases: &["mention_edit", "at_mention"],
        properties: MENTION_PROPERTIES,
        events: events_of!("mention"),
        // Audit 20261026: `set_candidates` is published now that the `candidates` records property
        // is its payload channel (B26.03). `set_trigger` was already published and the `trigger`
        // property is writable, so the earlier note that claimed it was withdrawn was wrong;
        // `complete` stays published — its arm completes the highlighted candidate and refuses
        // when nothing is highlighted.
        commands: &[
            "set_trigger",
            "open_popup",
            "complete",
            "set_candidates",
            "set_row_height",
            "set_max_visible",
            "set_text",
            "set_scroll_row",
        ],
    }
}

/// `EmojiPicker` — a shell for choosing a caller-supplied glyph.
///
/// Registered as a control with no glyph data: the table arrives through
/// `set_glyphs`, which is the whole point of the shell/data split.
#[cfg(not(alloc_frugal))]
pub(crate) fn emoji_picker_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::EmojiPicker,
        canonical_name: "emoji_picker",
        aliases: &["emoji"],
        properties: EMOJI_PICKER_PROPERTIES,
        events: events_of!("emoji_picker"),
        // Audit 20261026: `set_glyphs` and `set_categories` are published again now that the
        // `glyphs` records and `categories` StringList properties are their payload channels
        // (B26.03). `choose` takes a glyph id and has no property route, so it stays unpublished.
        commands: &[
            "set_search",
            "set_glyphs",
            "set_categories",
            "set_active_tab",
            "set_recent_capacity",
        ],
    }
}

/// `QueryBuilder` — edits a recursive `FilterExpr` as condition rows.
///
/// Registered separately from `data_grid`: the model is shared (both produce a
/// `FilterExpr`) but the rendering is not, which is exactly the case the plan's
/// §4 B5 describes.
#[cfg(not(alloc_frugal))]
pub(crate) fn query_builder_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::QueryBuilder,
        canonical_name: "query_builder",
        aliases: &["filter_builder"],
        properties: QUERY_BUILDER_PROPERTIES,
        events: events_of!("query_builder"),
        // Audit 20261026: `set_fields` and `set_rows` are published now that the `fields` / `rows`
        // records properties are their payload channels (B26.03). `add_row` and `remove_row` stay
        // unpublished — their payloads are a field index and a row index, and a whole-list write
        // covers both. `toggle_conjunction` stays published — its arm runs the control's real
        // toggle.
        commands: &[
            "toggle_conjunction",
            "set_fields",
            "set_rows",
            "set_active_row",
            "set_commit_delay_ms",
            "set_conjunction",
            "set_live_update",
            "set_scroll_row",
        ],
    }
}

/// `Cascader` — walks a path down a tree of options.
///
/// Not an alias of `dropdown`: the selection is a path of varying depth, not an
/// index (see the module docs).
#[cfg(not(alloc_frugal))]
pub(crate) fn cascader_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Cascader,
        canonical_name: "cascader",
        aliases: &["cascade", "multi_level_select"],
        properties: CASCADER_PROPERTIES,
        events: events_of!("cascader"),
        // Audit 20261026: `set_selected_path` is published now that the `selected_path`
        // NumberList property is its payload channel (B26.03). `set_options` stays unpublished:
        // an option carries `children`, and a record field cannot carry a nested record list, so
        // the tree has no property route.
        commands: &[
            "expand",
            "collapse",
            "set_selected_path",
            "set_filterable",
            "set_placeholder",
            "set_separator",
            "set_max_visible",
        ],
    }
}

pub(crate) fn mini_chart_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MiniChart,
        canonical_name: "mini_chart",
        aliases: &["mini_chart_widget"],
        properties: MINI_CHART_PROPERTIES,
        events: events_of!("mini_chart"),
        // Audit 20261026: `set_data` is published now that the `data` records property is its
        // payload channel (B26.03). `set_range` stays unpublished: its name resolves to no
        // property — the two bounds travel through the writable `min_value` / `max_value` scalars.
        commands: &["set_chart_type", "set_data", "set_hover_enabled"],
    }
}

pub(crate) fn mini_canvas_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MiniCanvas,
        canonical_name: "mini_canvas",
        aliases: &[],
        properties: MINI_CANVAS_PROPERTIES,
        events: events_of!("mini_canvas"),
        // Audit 20261026: `fill_rect`, `draw_rect`, `draw_line`, `fill_circle` stay unpublished —
        // each appends a drawing instruction (a rectangle, a segment, a circle) and the control
        // publishes no property that carries one; the control's own drawing API is the write path
        // (principle #18).
        commands: &["clear"],
    }
}

// ── Group A property arrays (non-mini) ─────────────────────────
#[cfg(not(alloc_frugal))]
pub(crate) fn canvas_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Canvas,
        canonical_name: "canvas",
        aliases: &["canvas_widget", "drawing_surface"],
        properties: CANVAS_PROPERTIES,
        events: events_of!("canvas"),
        // Audit 20261026: `set_zoom`, `set_center` stay unpublished — this control is a
        // command-recording drawing surface that replays `RenderCommand`s, not a pan/zoom view;
        // its published state is the queued-command count, so neither name has a property carrier
        // here (principle #18).
        commands: &[
            "clear",
            "set_anti_alias",
            "set_grid_step",
            "set_snap_to_grid",
            "set_zoom_step",
            "set_show_grid",
        ],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn chart_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Chart,
        canonical_name: "chart",
        aliases: &["chart_widget", "chart_surface"],
        properties: CHART_PROPERTIES,
        events: events_of!("chart"),
        commands: &["set_chart_type"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn badge_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Badge,
        canonical_name: "badge",
        aliases: &["badge_widget", "notification_badge"],
        properties: BADGE_PROPERTIES,
        events: events_of!("badge"),
        commands: &["set_text", "set_count", "set_dot_mode", "set_level"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn skeleton_loader_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::SkeletonLoader,
        canonical_name: "skeleton_loader",
        aliases: &["skeleton", "shimmer"],
        properties: SKELETON_LOADER_PROPERTIES,
        events: &[],
        commands: &["set_active", "set_shape", "set_text_line_rows"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn fab_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::FAB,
        canonical_name: "fab",
        aliases: &["floating_action_button", "action_button"],
        properties: FAB_PROPERTIES,
        events: events_of!("fab"),
        commands: &["set_icon"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn bottom_navigation_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::BottomNavigationBar,
        canonical_name: "bottom_navigation_bar",
        aliases: &["bottom_nav", "bottomnav"],
        properties: BOTTOM_NAVIGATION_BAR_PROPERTIES,
        events: events_of!("bottom_navigation_bar"),
        commands: &["set_selected_index"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn navigation_drawer_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::NavigationDrawer,
        canonical_name: "navigation_drawer",
        aliases: &["nav_drawer", "drawer"],
        properties: NAVIGATION_DRAWER_PROPERTIES,
        events: events_of!("navigation_drawer"),
        commands: &["set_open", "set_width", "set_section_separator_visible"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn app_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::AppBar,
        canonical_name: "app_bar",
        aliases: &["top_app_bar"],
        properties: APP_BAR_PROPERTIES,
        // The widget emits both of these (`app_bar.rs`); advertising an empty
        // list made a by-name subscriber unable to discover them.
        events: events_of!("app_bar"),
        commands: &["set_title", "set_direction"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn mobile_date_picker_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MobileDatePicker,
        canonical_name: "mobile_date_picker",
        aliases: &[],
        properties: MOBILE_DATE_PICKER_PROPERTIES,
        events: events_of!("mobile_date_picker"),
        commands: &["set_selected_date", "set_year_span"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn divider_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Divider,
        canonical_name: "divider",
        aliases: &["divider_widget", "separator"],
        properties: DIVIDER_PROPERTIES,
        events: &[],
        commands: &["set_orientation", "set_thickness", "set_color", "set_line_style"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn rating_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Rating,
        canonical_name: "rating",
        aliases: &["rating_widget", "star_rating"],
        properties: RATING_PROPERTIES,
        events: events_of!("rating"),
        commands: &["set_value", "set_max", "set_allow_half_step", "set_star_size"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn avatar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Avatar,
        canonical_name: "avatar",
        aliases: &["avatar_widget", "user_avatar"],
        properties: AVATAR_PROPERTIES,
        events: events_of!("avatar"),
        commands: &[
            "set_initials",
            "set_image_source",
            "set_ring_color",
            "set_ring_width",
            "set_square",
            "set_badge",
            "set_size",
        ],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn empty_state_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::EmptyState,
        canonical_name: "empty_state",
        aliases: &["empty_placeholder"],
        properties: EMPTY_STATE_PROPERTIES,
        events: events_of!("empty_state"),
        commands: &["set_message", "set_description", "set_secondary_action_text"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn color_history_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ColorHistory,
        canonical_name: "color_history",
        aliases: &[],
        properties: COLOR_HISTORY_PROPERTIES,
        events: events_of!("color_history"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn color_well_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ColorWell,
        canonical_name: "color_well",
        aliases: &[],
        properties: COLOR_WELL_PROPERTIES,
        events: events_of!("color_well"),
        commands: &["set_color", "set_show_border"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn qr_code_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::QRCode,
        canonical_name: "qr_code",
        aliases: &["qr"],
        properties: QR_CODE_PROPERTIES,
        events: &[],
        commands: &[
            "set_data",
            "set_size",
            "set_ec_level",
            "set_foreground_color",
            "set_logo_size_modules",
            "set_quiet_zone",
            "set_background_color",
        ],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn material_snackbar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MaterialSnackbar,
        canonical_name: "material_snackbar",
        aliases: &[],
        properties: MATERIAL_SNACKBAR_PROPERTIES,
        events: events_of!("material_snackbar"),
        commands: &["show", "dismiss", "set_message", "set_action_text", "set_duration_ms"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn adaptive_scaffold_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::AdaptiveScaffold,
        canonical_name: "adaptive_scaffold",
        aliases: &["scaffold"],
        properties: ADAPTIVE_SCAFFOLD_PROPERTIES,
        events: events_of!("adaptive_scaffold"),
        commands: &[
            "set_title",
            "set_badge_style",
            "set_drawer_animated",
            "set_scrim_opacity",
            "set_page_transition",
        ],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn segmented_button_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::SegmentedButton,
        canonical_name: "segmented_button",
        aliases: &["segmented_btn"],
        properties: SEGMENTED_BUTTON_PROPERTIES,
        events: events_of!("segmented_button"),
        commands: &[
            "set_selected_index",
            "set_allows_multiple",
            "set_group_id",
            "set_selected_indices",
        ],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn navigation_stack_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::NavigationStack,
        canonical_name: "navigation_stack",
        aliases: &["nav_stack"],
        properties: NAVIGATION_STACK_PROPERTIES,
        // `navigation_changed` carries a `NavigationEvent`; `page_changed` was
        // advertised but never emitted by this widget.
        events: events_of!("navigation_stack"),
        // Audit 20261026: `push` stays unpublished — a page is a mounted widget and no property
        // value can carry a widget reference; the control's own `push` API is the write path
        // (principle #18).
        // command arm stays as a defensive refusal.
        commands: &["pop", "set_current_page", "set_back_visible"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn progress_circle_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ProgressCircle,
        canonical_name: "progress_circle",
        aliases: &["circular_progress"],
        properties: PROGRESS_CIRCLE_PROPERTIES,
        events: events_of!("progress_circle"),
        commands: &[
            "set_value",
            "set_thickness",
            "set_indeterminate",
            "set_center_text_format",
            "set_diameter",
            "set_progress_color",
            "set_show_center_text",
        ],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn icon_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Icon,
        canonical_name: "icon",
        aliases: &["icon_widget", "icon_display"],
        properties: ICON_PROPERTIES,
        events: &[],
        commands: &["set_icon_name", "set_size", "set_icon_flip", "set_icon_rotation", "set_color"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn font_preview_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::FontPreview,
        canonical_name: "font_preview",
        aliases: &["font_viewer"],
        properties: FONT_PREVIEW_PROPERTIES,
        events: &[],
        commands: &["set_font_family", "set_font_size", "set_preview_text"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn cupertino_navigation_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::CupertinoNavigationBar,
        canonical_name: "cupertino_navigation_bar",
        aliases: &["cupertinonavbar", "ios_nav_bar"],
        properties: CUPERTINO_NAVIGATION_BAR_PROPERTIES,
        events: events_of!("cupertino_navigation_bar"),
        commands: &["set_title", "set_large_title", "set_collapse_offset", "set_prompt"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn refresh_control_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::RefreshControl,
        canonical_name: "refresh_control",
        aliases: &["pull_to_refresh"],
        properties: REFRESH_CONTROL_PROPERTIES,
        events: events_of!("refresh_control"),
        commands: &["set_refreshing"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn date_range_picker_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::DateRangePicker,
        canonical_name: "date_range_picker",
        aliases: &["range_picker"],
        properties: DATE_RANGE_PICKER_PROPERTIES,
        events: events_of!("date_range_picker"),
        commands: &[
            "set_start_date",
            "set_end_date",
            "set_double_month",
            "set_max_nights",
            "set_min_nights",
        ],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn material_navigation_rail_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MaterialNavigationRail,
        canonical_name: "material_navigation_rail",
        aliases: &["nav_rail", "navigation_rail"],
        properties: MATERIAL_NAVIGATION_RAIL_PROPERTIES,
        events: events_of!("material_navigation_rail"),
        commands: &["set_selected_index"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn tab_view_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TabView,
        canonical_name: "tab_view",
        aliases: &["page_tab_view"],
        properties: TAB_VIEW_PROPERTIES,
        events: events_of!("tab_view"),
        commands: &["set_selected_index", "set_keyboard_navigation_enabled"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn swipe_to_dismiss_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::SwipeToDismiss,
        canonical_name: "swipe_to_dismiss",
        aliases: &["swipe_dismiss"],
        properties: SWIPE_TO_DISMISS_PROPERTIES,
        events: events_of!("swipe_to_dismiss"),
        // RW-104: the row's two programmatic operations were reachable only as Rust methods, so a
        // design document could insert the control but never trigger or undo a departure. The
        // gesture stays the primary path; these make the same two facts available as commands.
        commands: &["dismiss", "reset_swipe"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn line_chart_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::LineChart,
        canonical_name: "line_chart",
        aliases: &["line_graph"],
        properties: LINE_CHART_PROPERTIES,
        events: events_of!("line_chart"),
        commands: &["set_stroke_width"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn sparkline_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Sparkline,
        canonical_name: "sparkline",
        aliases: &["sparkline_chart"],
        properties: SPARKLINE_PROPERTIES,
        events: events_of!("sparkline"),
        commands: &[
            "set_stroke_width",
            "set_capacity",
            "set_fill",
            "set_show_end_value",
            "set_show_last_marker",
            "set_show_points",
        ],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn bar_chart_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::BarChart,
        canonical_name: "bar_chart",
        aliases: &["bar_graph"],
        properties: BAR_CHART_PROPERTIES,
        events: events_of!("bar_chart"),
        commands: &["set_bar_spacing"],
    }
}

pub(crate) fn pie_chart_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::PieChart,
        canonical_name: "pie_chart",
        aliases: &["pie_graph"],
        properties: PIE_CHART_PROPERTIES,
        events: events_of!("pie_chart"),
        commands: &["set_donut"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn bezier_curve_editor_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::BezierCurveEditor,
        canonical_name: "bezier_curve_editor",
        aliases: &["curve_editor"],
        properties: BEZIER_CURVE_EDITOR_PROPERTIES,
        events: events_of!("bezier_curve_editor"),
        commands: &["set_snap_to_grid"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn barcode_scanner_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::BarcodeScanner,
        canonical_name: "barcode_scanner",
        aliases: &["scanner"],
        properties: BARCODE_SCANNER_PROPERTIES,
        events: events_of!("barcode_scanner"),
        commands: &["set_is_scanning"],
    }
}

/// The grid status strip's capability (COMP-A23).
///
/// It shares `WidgetKind::StatusBar` with the message bar, so it needs its own canonical name and
/// a concrete-type tie-break; its events are the strip's own `status_clicked`.
#[cfg(not(alloc_frugal))]
pub(crate) fn grid_status_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::StatusBar,
        canonical_name: "grid_status_bar",
        aliases: &["table_status_bar"],
        properties: GRID_STATUS_BAR_PROPERTIES,
        events: events_of!("grid_status_bar"),
        commands: &["set_text", "set_filter_active"],
    }
}

/// The group avatar's capability (MISC-010).
///
/// It shares `WidgetKind::Avatar` with the single `Avatar`; membership is a method call
/// (`push`/`clear`), so the schema is the base contract and no event is published.
#[cfg(not(alloc_frugal))]
pub(crate) fn group_avatar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Avatar,
        canonical_name: "group_avatar",
        aliases: &[],
        properties: GROUP_AVATAR_PROPERTIES,
        events: &[],
        commands: &[],
    }
}

/// Unified dropdown popup host (COMP-A21).
#[cfg(not(alloc_frugal))]
pub(crate) fn drop_down_popup_host_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Panel,
        canonical_name: "drop_down_popup_host",
        aliases: &[],
        properties: DROP_DOWN_POPUP_HOST_PROPERTIES,
        events: events_of!("drop_down_popup_host"),
        commands: &[
            "set_highlighted_index",
            "set_max_visible_rows",
            "set_placement",
            "set_scroll_offset",
        ],
    }
}

/// Command palette host (COMP-A30).
#[cfg(not(alloc_frugal))]
pub(crate) fn command_palette_host_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ListView,
        canonical_name: "command_palette_host",
        aliases: &[],
        properties: COMMAND_PALETTE_HOST_PROPERTIES,
        events: events_of!("command_palette_host"),
        commands: &["set_selected_index", "set_filter"],
    }
}

/// Header menu (COMP-A32).
#[cfg(not(alloc_frugal))]
pub(crate) fn header_menu_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Menu,
        canonical_name: "header_menu",
        aliases: &[],
        properties: HEADER_MENU_PROPERTIES,
        events: events_of!("header_menu"),
        commands: &["set_filter_text", "set_filterable", "set_sort_state", "set_sortable"],
    }
}

/// Chart reference line (COMP-A1).
///
/// It shares `WidgetKind::LineChart` with the chart it annotates, so it needs its own canonical
/// name and a concrete-type tie-break; the annotations are attached by method call, and the line
/// publishes no factory event of its own.
#[cfg(not(alloc_frugal))]
pub(crate) fn chart_reference_line_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::LineChart,
        canonical_name: "chart_reference_line",
        aliases: &[],
        properties: CHART_REFERENCE_LINE_PROPERTIES,
        events: events_of!("chart_reference_line"),
        commands: &["set_label_visible"],
    }
}

/// Chart crosshair overlay (RW-77).
///
/// The crate has no `ChartCrosshair` kind. It borrows `WidgetKind::ChartTooltip` — the closest
/// existing role (a pointer-following chart overlay) — and its canonical name is what keeps the two
/// capabilities distinct. It publishes no events here: its `position_changed`/`crosshair_moved`
/// signals have no `event_signal_dyn` arm, so publishing names would be a wire that never fires.
#[cfg(not(alloc_frugal))]
pub(crate) fn chart_crosshair_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ChartTooltip,
        canonical_name: "chart_crosshair",
        aliases: &["crosshair"],
        properties: CHART_CROSSHAIR_PROPERTIES,
        events: events_of!("chart_crosshair"),
        commands: &["set_follow_pointer", "set_snap_to_data_point"],
    }
}

/// A scrolling picker column (RW-100). Registered under `MobileDatePicker`, the control it is the
/// building block of, with its own canonical name.
#[cfg(not(alloc_frugal))]
pub(crate) fn drum_column_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MobileDatePicker,
        canonical_name: "drum_column",
        aliases: &[],
        properties: DRUM_COLUMN_PROPERTIES,
        events: events_of!("drum_column"),
        commands: &["set_selected_index", "set_loop_wrap", "set_row_height"],
    }
}

/// A row of preset chips (RW-100). Registered under `Chip` with its own canonical name.
#[cfg(not(alloc_frugal))]
pub(crate) fn preset_chip_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Chip,
        canonical_name: "preset_chip_bar",
        aliases: &[],
        properties: PRESET_CHIP_BAR_PROPERTIES,
        events: events_of!("preset_chip_bar"),
        commands: &[
            "set_selected_preset",
            "set_chip_height",
            "set_chip_padding",
            "set_spacing",
            "set_orientation",
        ],
    }
}
