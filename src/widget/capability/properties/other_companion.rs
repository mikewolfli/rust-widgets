// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

// Capability constructors for the `other_companion` widgets.
//
// The property arrays these constructors reference are declared in
// `properties_other.in.rs` and expanded by the parent module. `use super::*` brings
// those arrays and the shared imports into scope; `events_of!` is visible through the
// parent module's textual macro scope.

use super::*;

#[cfg(not(alloc_frugal))]
pub(crate) fn tag_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Tag,
        canonical_name: "tag",
        aliases: &["label_tag"],
        properties: TAG_PROPERTIES,
        events: events_of!("tag"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn clock_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Clock,
        canonical_name: "clock",
        aliases: &["analog_clock"],
        properties: CLOCK_PROPERTIES,
        events: events_of!("clock"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn gauge_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Gauge,
        canonical_name: "gauge",
        aliases: &["dial_gauge"],
        properties: GAUGE_PROPERTIES,
        events: events_of!("gauge"),
        commands: &[],
    }
}

// ── Menus and dialogs ───────────────────────────────────────────────────────

#[cfg(not(alloc_frugal))]
/// `mac_segmented_control` — the desktop segmented picker.
pub(crate) fn mac_segmented_control_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::SegmentedControl,
        canonical_name: "mac_segmented_control",
        aliases: &[],
        properties: MAC_SEGMENTED_CONTROL_PROPERTIES,
        events: events_of!("mac_segmented_control"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `mac_stepper` — the desktop increment/decrement stepper.
pub(crate) fn mac_stepper_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Stepper,
        canonical_name: "mac_stepper",
        aliases: &[],
        properties: MAC_STEPPER_PROPERTIES,
        events: events_of!("mac_stepper"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `mac_toolbar` — the desktop toolbar strip.
pub(crate) fn mac_toolbar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ToolBar,
        canonical_name: "mac_toolbar",
        aliases: &[],
        properties: MAC_TOOLBAR_PROPERTIES,
        events: events_of!("mac_toolbar"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `mac_source_list` — the desktop sectioned sidebar.
pub(crate) fn mac_source_list_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ListView,
        canonical_name: "mac_source_list",
        aliases: &[],
        properties: MAC_SOURCE_LIST_PROPERTIES,
        events: events_of!("mac_source_list"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `mac_window_chrome` — the desktop title bar with traffic-light controls.
pub(crate) fn mac_window_chrome_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Window,
        canonical_name: "mac_window_chrome",
        aliases: &[],
        properties: MAC_WINDOW_CHROME_PROPERTIES,
        events: events_of!("mac_window_chrome"),
        commands: &[],
    }
}

// ── Companion capabilities (COMP-01/05/07/08/09/10/12/13/14/15/16/17/18/19/20) ──
//
// Each companion is its own control with its own kind, so its capability is a
// first-class row rather than a comment on the primary control it partners.

#[cfg(not(alloc_frugal))]
/// `swatch` — the shared colour-cell grid (`COMP-01`).
pub(crate) fn swatch_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Swatch,
        canonical_name: "swatch",
        aliases: &["color_swatch", "colour_swatch"],
        properties: SWATCH_PROPERTIES,
        events: events_of!("swatch"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `filter_row` — one column filter editor per column (`COMP-05`).
pub(crate) fn filter_row_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::FilterRow,
        canonical_name: "filter_row",
        aliases: &["filter_bar"],
        properties: FILTER_ROW_PROPERTIES,
        events: events_of!("filter_row"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `legend` — a checkable chart series key (`COMP-07`).
pub(crate) fn legend_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Legend,
        canonical_name: "legend",
        aliases: &["chart_legend"],
        properties: LEGEND_PROPERTIES,
        events: events_of!("legend"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `chart_tooltip` — the follow-pointer chart bubble (`COMP-08`).
pub(crate) fn chart_tooltip_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ChartTooltip,
        canonical_name: "chart_tooltip",
        aliases: &["tooltip_bubble"],
        properties: CHART_TOOLTIP_PROPERTIES,
        events: events_of!("chart_tooltip"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `tooltip_manager` — the host hover-to-tooltip pipeline (`COMP-09`).
pub(crate) fn tooltip_manager_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TooltipManager,
        canonical_name: "tooltip_manager",
        aliases: &["tooltip_pipeline"],
        properties: TOOLTIP_MANAGER_PROPERTIES,
        events: events_of!("tooltip_manager"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `zoom_pan_controller` — zoom/pan/ROI state for an image view (`COMP-10`).
pub(crate) fn zoom_pan_controller_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ZoomPanController,
        canonical_name: "zoom_pan_controller",
        aliases: &["zoom_pan"],
        properties: ZOOM_PAN_CONTROLLER_PROPERTIES,
        events: events_of!("zoom_pan_controller"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `toast_host` — the bounded, expiring toast queue (`COMP-12`).
pub(crate) fn toast_host_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ToastHost,
        canonical_name: "toast_host",
        aliases: &["toast_queue"],
        properties: TOAST_HOST_PROPERTIES,
        events: events_of!("toast_host"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `tile_loader` — tile addresses and their cache (`COMP-13`).
pub(crate) fn tile_loader_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TileLoader,
        canonical_name: "tile_loader",
        aliases: &["map_tile_loader"],
        properties: TILE_LOADER_PROPERTIES,
        events: events_of!("tile_loader"),
        commands: &["clear_cache"],
    }
}

#[cfg(not(alloc_frugal))]
/// `zoom_control` — the map's three-button zoom strip (`COMP-13`).
pub(crate) fn zoom_control_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ZoomControl,
        canonical_name: "zoom_control",
        aliases: &["map_zoom_control"],
        properties: ZOOM_CONTROL_PROPERTIES,
        events: events_of!("zoom_control"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `scale_bar` — the map's distance legend (`COMP-13`).
pub(crate) fn scale_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ScaleBar,
        canonical_name: "scale_bar",
        aliases: &["map_scale_bar"],
        properties: SCALE_BAR_PROPERTIES,
        events: &[],
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `compass` — the map's north-up dial (`COMP-13`).
pub(crate) fn compass_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Compass,
        canonical_name: "compass",
        aliases: &["map_compass"],
        properties: COMPASS_PROPERTIES,
        events: events_of!("compass"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `timeline_axis` — the Gantt date/value axis (`COMP-14`).
pub(crate) fn timeline_axis_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TimelineAxis,
        canonical_name: "timeline_axis",
        aliases: &["gantt_timeline_axis"],
        properties: TIMELINE_AXIS_PROPERTIES,
        events: events_of!("timeline_axis"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `dependency_editor` — the Gantt dependency edge editor (`COMP-14`).
pub(crate) fn dependency_editor_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::DependencyEditor,
        canonical_name: "dependency_editor",
        aliases: &["gantt_dependency_editor"],
        properties: DEPENDENCY_EDITOR_PROPERTIES,
        events: events_of!("dependency_editor"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `diff_file_list` — the multi-file selector beside a diff (`COMP-15`).
pub(crate) fn diff_file_list_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::DiffFileList,
        canonical_name: "diff_file_list",
        aliases: &["diff_files"],
        properties: DIFF_FILE_LIST_PROPERTIES,
        events: events_of!("diff_file_list"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `markdown_outline_panel` — the markdown editor's heading outline (`COMP-16`).
pub(crate) fn markdown_outline_panel_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MarkdownOutlinePanel,
        canonical_name: "markdown_outline_panel",
        aliases: &["markdown_outline"],
        properties: MARKDOWN_OUTLINE_PANEL_PROPERTIES,
        events: events_of!("markdown_outline_panel"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `find_bar` — the editor's find/replace strip (`COMP-16`).
///
/// `find_replace_bar` used to be an alias of this control. It is now the canonical name of the
/// companion `FindReplaceBar` capability (`COMP-22`), so the spelling has to name the control a
/// caller can actually mount; keeping the alias would have made one name resolve to two different
/// widgets depending on which row registered last.
pub(crate) fn find_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::FindBar,
        canonical_name: "find_bar",
        aliases: &[],
        properties: FIND_BAR_PROPERTIES,
        events: events_of!("find_bar"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `menu_panel` — a drop-down menu panel a host anchors itself (`COMP-A01`).
pub(crate) fn menu_panel_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MenuPanel,
        canonical_name: "menu_panel",
        aliases: &[],
        properties: MENU_PANEL_PROPERTIES,
        events: events_of!("menu_panel"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `speed_dial` — a floating trigger that fans out its actions (`COMP-A02`).
pub(crate) fn speed_dial_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::SpeedDial,
        canonical_name: "speed_dial",
        aliases: &[],
        properties: SPEED_DIAL_PROPERTIES,
        events: events_of!("speed_dial"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `swipe_actions` — a multi-action background for a swiped row (`COMP-A03`).
pub(crate) fn swipe_actions_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::SwipeActions,
        canonical_name: "swipe_actions",
        aliases: &[],
        properties: SWIPE_ACTIONS_PROPERTIES,
        events: events_of!("swipe_actions"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `drawer_section_header` — the small heading row that groups a drawer's items (`COMP-A04`).
pub(crate) fn drawer_section_header_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::DrawerSectionHeader,
        canonical_name: "drawer_section_header",
        aliases: &[],
        properties: DRAWER_SECTION_HEADER_PROPERTIES,
        // This heading emits no signals (`drawer_section_header:` in the census), so the list is
        // the empty literal rather than an `events_of!` lookup.
        events: &[],
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `layer_panel` — the companion list for a canvas's layers (`COMP-I01`).
pub(crate) fn layer_panel_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::LayerPanel,
        canonical_name: "layer_panel",
        aliases: &[],
        properties: LAYER_PANEL_PROPERTIES,
        events: events_of!("layer_panel"),
        // `set_entries` names its payload through the `entries` records property (B26.03); the
        // selection / visibility / scroll verbs stay Rust-side because no property carries their
        // layer id.
        commands: &["set_entries"],
    }
}

#[cfg(not(alloc_frugal))]
/// `playlist_panel` — the companion list for a media player's queue (`COMP-I03`).
pub(crate) fn playlist_panel_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::PlaylistPanel,
        canonical_name: "playlist_panel",
        aliases: &[],
        properties: PLAYLIST_PANEL_PROPERTIES,
        events: events_of!("playlist_panel"),
        // `set_tracks` names its payload through the `tracks` records property (B26.03);
        // `track_selected` is an event name rather than a command, and `set_filter` stays Rust-side
        // because the writable `filter` property is its document route.
        commands: &["set_tracks"],
    }
}

#[cfg(not(alloc_frugal))]
/// `shape_canvas` — the free-form shape scene (`COMP-17`).
pub(crate) fn shape_canvas_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ShapeCanvas,
        canonical_name: "shape_canvas",
        aliases: &["shape_board"],
        properties: SHAPE_CANVAS_PROPERTIES,
        events: events_of!("shape_canvas"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `kanban_card_view` — a real card inside a kanban column (`COMP-18`).
pub(crate) fn kanban_card_view_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::KanbanCardView,
        canonical_name: "kanban_card_view",
        aliases: &["kanban_card_control"],
        properties: KANBAN_CARD_VIEW_PROPERTIES,
        events: events_of!("kanban_card_view"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `kanban_column_header` — a real column header for a kanban board (`COMP-18`).
pub(crate) fn kanban_column_header_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::KanbanColumnHeader,
        canonical_name: "kanban_column_header",
        aliases: &["kanban_column_bar"],
        properties: KANBAN_COLUMN_HEADER_PROPERTIES,
        events: events_of!("kanban_column_header"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `range_preset_bar` — quick date-range presets (`COMP-19`).
pub(crate) fn range_preset_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::RangePresetBar,
        canonical_name: "range_preset_bar",
        aliases: &["preset_bar"],
        properties: RANGE_PRESET_BAR_PROPERTIES,
        events: events_of!("range_preset_bar"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `grid_view_header` — the shared table header (`COMP-20`).
pub(crate) fn grid_view_header_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::GridViewHeader,
        canonical_name: "grid_view_header",
        aliases: &["table_header_bar"],
        properties: GRID_VIEW_HEADER_PROPERTIES,
        events: events_of!("grid_view_header"),
        commands: &[],
    }
}

// ── Editor/chart/data companion overlays (COMP-21..29) ──
//
// Nine companion controls that report the kind of the control they partner. Each keeps its own
// canonical name, property table and published events; the shared kind is resolved by the
// concrete-type tie-break in `WidgetFactory::widget_matches_capability`.

#[cfg(not(alloc_frugal))]
/// `line_number_gutter` — the editor's line-number rail (`COMP-21`).
pub(crate) fn line_number_gutter_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Panel,
        canonical_name: "line_number_gutter",
        aliases: &[],
        properties: LINE_NUMBER_GUTTER_PROPERTIES,
        events: events_of!("line_number_gutter"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `find_replace_bar` — the editor's find/replace strip (`COMP-22`).
pub(crate) fn find_replace_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::SearchBar,
        canonical_name: "find_replace_bar",
        aliases: &[],
        properties: FIND_REPLACE_BAR_PROPERTIES,
        events: events_of!("find_replace_bar"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `list_state_host` — the loading/empty/error surface for a list (`COMP-23`).
pub(crate) fn list_state_host_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::EmptyState,
        canonical_name: "list_state_host",
        aliases: &[],
        properties: LIST_STATE_HOST_PROPERTIES,
        events: events_of!("list_state_host"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `summary_row` — the aggregate row of a table (`COMP-24`).
pub(crate) fn summary_row_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Table,
        canonical_name: "summary_row",
        aliases: &[],
        properties: SUMMARY_ROW_PROPERTIES,
        events: events_of!("summary_row"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `zoom_strip` — the zoom strip beside a chart or a canvas (`COMP-25`).
pub(crate) fn zoom_strip_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Slider,
        canonical_name: "zoom_strip",
        aliases: &[],
        properties: ZOOM_STRIP_PROPERTIES,
        events: events_of!("zoom_strip"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `axis_picker` — per-series axis assignment for a chart (`COMP-26`).
pub(crate) fn axis_picker_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Panel,
        canonical_name: "axis_picker",
        aliases: &[],
        properties: AXIS_PICKER_PROPERTIES,
        events: events_of!("axis_picker"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `chart_data_table` — the tabular view of a chart's series data (`COMP-27`).
///
/// Like every other capability here the table lives in `properties_other.in.rs`; the control
/// itself is built by the `full_widgets` companion module, which is why the constructor and the
/// registration entry sit inside the same gates as the siblings.
pub(crate) fn chart_data_table_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Table,
        canonical_name: "chart_data_table",
        aliases: &[],
        properties: CHART_DATA_TABLE_PROPERTIES,
        events: events_of!("chart_data_table"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `calendar_mark_layer` — per-date marks painted over a calendar (`COMP-28`).
pub(crate) fn calendar_mark_layer_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Panel,
        canonical_name: "calendar_mark_layer",
        aliases: &[],
        properties: CALENDAR_MARK_LAYER_PROPERTIES,
        events: events_of!("calendar_mark_layer"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `drag_ghost_layer` — the translucent proxy card that follows a drag (`COMP-29`).
pub(crate) fn drag_ghost_layer_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Panel,
        canonical_name: "drag_ghost_layer",
        aliases: &[],
        properties: DRAG_GHOST_LAYER_PROPERTIES,
        events: events_of!("drag_ghost_layer"),
        commands: &[],
    }
}
