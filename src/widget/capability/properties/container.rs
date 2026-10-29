// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

// Capability constructors for the `container` widgets.
//
// The property arrays these constructors reference are declared in
// `properties_container.in.rs` and expanded by the parent module. `use super::*` brings
// those arrays and the shared imports into scope; `events_of!` is visible through the
// parent module's textual macro scope.

use super::*;

pub(crate) fn group_box_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::GroupBox,
        canonical_name: "group_box",
        aliases: &[],
        properties: GROUP_BOX_PROPERTIES,
        events: events_of!("group_box"),
        commands: &["set_title", "set_checkable", "set_checked", "toggle"],
    }
}

/// The capability for the `panel` entry — the plain container spelling.
///
/// # Why the schema and the kind both point at `GroupBox`
///
/// `Panel` is a `pub type` for `GroupBox` (`src/widget/mod.rs`), so `create_panel(..)`
/// builds a `GroupBox` and the control reports `WidgetKind::GroupBox`. Naming the kind
/// here as `Panel` was the earlier attempt, and it produced a second, undecidable
/// question: a mounted `GroupBox` then claimed *two* kinds depending on which entry it
/// came from, so the property contract had to publish a schema that matched neither
/// entry — which the schema/contract gate correctly rejected.
///
/// What was actually wrong was smaller: `WidgetKind::Panel` had **no capability named
/// after it**, so `create_panel(..)` resolved to whichever kind-`Panel` entry was
/// registered first (`breadcrumb`). Registering the `panel` name against the kind the
/// control really reports removes the ordering from that lookup and keeps the mounted
/// widget's kind, its schema and its contract agreeing with each other.
#[cfg(full_widgets)]
pub(crate) fn panel_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::GroupBox,
        canonical_name: "panel",
        aliases: &[],
        properties: GROUP_BOX_PROPERTIES,
        events: events_of!("panel"),
        commands: &["set_title", "set_checkable", "set_checked", "toggle"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn frame_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Frame,
        canonical_name: "frame",
        aliases: &["frame_widget"],
        properties: FRAME_PROPERTIES,
        events: &[],
        commands: &["set_frame_shape", "set_frame_shadow", "set_line_width"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn splitter_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Splitter,
        canonical_name: "splitter",
        aliases: &["pane_splitter"],
        properties: SPLITTER_PROPERTIES,
        events: events_of!("splitter"),
        commands: &["set_orientation"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn tool_box_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Toolbox,
        canonical_name: "tool_box",
        aliases: &[],
        properties: TOOL_BOX_PROPERTIES,
        events: events_of!("tool_box"),
        // Audit 20261026: `add_item`, `remove_item` stay unpublished — each item carries the
        // `ObjectId` of the page widget it shows, and no property value can carry a widget
        // reference; `item_count`/`current_index` report the list's shape instead (principle
        // #18).
        commands: &["set_current_index", "set_orientation"],
    }
}

// NOTE: there is deliberately no second `toolbox` entry here.
//
// One existed, registered against the same kind, so `tool_box` and `toolbox` were two
// capabilities for one control. That made `capability("toolbox")` answer by
// registration order and gave `WidgetKind::Toolbox` two entries to disambiguate between
// with a tie-break row — all to buy a spelling `normalize_key` cannot distinguish from
// `tool_box` (`"toolbox"` and `"tool_box"` are the same key). The single `tool_box`
// entry accepts both spellings, so the second entry was duplication (rule #54) with no
// behaviour behind it.

// ── Container widget capabilities ─────────────────────────────
/// Scroll area capability.
///
/// Not registered in mini mode, but kept compiled so the shared
/// `SCROLL_AREA_PROPERTIES` table (defined in the ungated
/// `properties_container.in.rs`) stays reachable.
#[cfg_attr(alloc_frugal, allow(dead_code))]
pub(crate) fn scroll_area_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ScrollArea,
        canonical_name: "scroll_area",
        aliases: &[],
        properties: SCROLL_AREA_PROPERTIES,
        events: events_of!("scroll_area"),
        commands: &[
            "set_widget_resizable",
            "set_horizontal_scroll_bar_policy",
            "set_vertical_scroll_bar_policy",
        ],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn tab_widget_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TabWidget,
        canonical_name: "tab_widget",
        aliases: &[],
        properties: TAB_WIDGET_PROPERTIES,
        events: events_of!("tab_widget"),
        // Audit 20261026: `add_tab`, `remove_tab` stay unpublished — a tab carries the page widget
        // it shows, and no property value can carry a widget reference; `tab_count` and the
        // text/current-index properties report the strip instead (principle #18).
        commands: &[
            "set_current_index",
            "set_closable",
            "set_movable",
            "set_tab_position",
            "set_tab_shape",
        ],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn stacked_widget_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::StackedWidget,
        canonical_name: "stacked_widget",
        aliases: &["stacked"],
        properties: STACKED_WIDGET_PROPERTIES,
        events: events_of!("stacked_widget"),
        // Audit 20261026: `add_widget`, `remove_widget` stay unpublished — each names a mounted
        // widget (or the `ObjectId` of one), and no property value can carry a widget reference;
        // `current_index` is the writable position property (principle #18).
        commands: &["set_current_index"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn collapsible_pane_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::CollapsiblePane,
        canonical_name: "collapsible_pane",
        aliases: &["collapsible"],
        properties: COLLAPSIBLE_PANE_PROPERTIES,
        events: events_of!("collapsible_pane"),
        commands: &["set_title", "set_collapsed", "toggle"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn dock_widget_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::DockWidget,
        canonical_name: "dock_widget",
        aliases: &["dock"],
        properties: DOCK_WIDGET_PROPERTIES,
        events: events_of!("dock_widget"),
        // Audit 20261026: `set_features`, `set_allowed_areas` stay unpublished — each assigns a
        // bit-flag set (`DockFeatures` / `DockAreas`) and the control publishes no property that
        // carries one; the control's own setters are the write path (principle #18).
        commands: &["set_title", "set_floating"],
    }
}

/// The dock host's published events (C-001).
///
/// [`DockArea`](crate::widget::container_widgets::DockArea) is a layout owner rather than a
/// `Widget`: it holds child ids and arranges the docks registered with it, so it has no
/// `WidgetKind` of its own and cannot carry a capability row under its own kind. Its three
/// host-facing signals therefore travel on this row, which shares the dock family's kind and
/// property schema because the constructible member of that family is the dock widget.
///
/// The events are resolved by `DockArea::event_signal_dyn`, not by `DockWidget`: the area owns
/// `active_dock_changed`, `dock_removed` and `layout_changed`, and a host that owns the area
/// connects those names through [`WidgetFactory::connect_event`](crate::widget::WidgetFactory).
#[cfg(not(alloc_frugal))]
pub(crate) fn dock_area_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::DockWidget,
        canonical_name: "dock_area",
        aliases: &["dock_host"],
        properties: DOCK_WIDGET_PROPERTIES,
        events: events_of!("dock_area"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn mdi_area_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MdiArea,
        canonical_name: "mdi_area",
        aliases: &["mdi"],
        properties: MDI_AREA_PROPERTIES,
        events: events_of!("mdi_area"),
        // Audit 20261026: `add_subwindow`, `remove_subwindow`, `activate_subwindow`
        // stay unpublished — a sub-window is a mounted widget (or its index) and the control
        // publishes `active_subwindow` as read-only; `set_view_mode` stays published — the
        // property route runs the control's real `set_view_mode` (principle #18).
        // writes the control's real view mode.
        commands: &["set_view_mode"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn stepper_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Stepper,
        canonical_name: "stepper",
        aliases: &["stepper_widget", "step_control"],
        properties: STEPPER_PROPERTIES,
        events: events_of!("stepper"),
        commands: &["set_value", "set_minimum", "set_maximum", "set_step"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn masonry_layout_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MasonryLayout,
        canonical_name: "masonry_layout",
        aliases: &["waterfall_layout"],
        properties: MASONRY_LAYOUT_PROPERTIES,
        events: events_of!("masonry_layout"),
        commands: &["set_column_count"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn safe_area_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::SafeArea,
        canonical_name: "safe_area",
        aliases: &["safe_area_insets"],
        properties: SAFE_AREA_PROPERTIES,
        events: &[],
        commands: &["set_top_inset", "set_bottom_inset", "set_left_inset", "set_right_inset"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn carousel_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Carousel,
        canonical_name: "carousel",
        aliases: &["carousel_widget", "swipe_view"],
        properties: CAROUSEL_PROPERTIES,
        events: events_of!("carousel"),
        commands: &["next", "previous"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn spacer_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Spacer,
        canonical_name: "spacer",
        aliases: &["flexible_space"],
        properties: SPACER_PROPERTIES,
        events: &[],
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn center_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Center,
        canonical_name: "center",
        aliases: &["centre", "center_widget"],
        properties: CENTER_PROPERTIES,
        events: &[],
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn align_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Align,
        canonical_name: "align",
        aliases: &["alignment_widget"],
        properties: ALIGN_PROPERTIES,
        events: &[],
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn sized_box_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::SizedBox,
        canonical_name: "sized_box",
        aliases: &["size_box"],
        properties: SIZED_BOX_PROPERTIES,
        events: &[],
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn padding_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Padding,
        canonical_name: "padding",
        aliases: &["padding_widget"],
        properties: PADDING_PROPERTIES,
        events: &[],
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn expanded_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Expanded,
        canonical_name: "expanded",
        aliases: &["expand"],
        properties: EXPANDED_PROPERTIES,
        events: &[],
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn flexible_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Flexible,
        canonical_name: "flexible",
        aliases: &["flex_child"],
        properties: FLEXIBLE_PROPERTIES,
        events: &[],
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn intrinsic_width_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::IntrinsicWidth,
        canonical_name: "intrinsic_width",
        aliases: &["shrink_wrap"],
        properties: INTRINSIC_WIDTH_PROPERTIES,
        events: &[],
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn aspect_ratio_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::AspectRatio,
        canonical_name: "aspect_ratio",
        aliases: &["ratio_box"],
        properties: ASPECT_RATIO_PROPERTIES,
        events: &[],
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn flex_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Flex,
        canonical_name: "flex",
        aliases: &["flex_box"],
        properties: FLEX_PROPERTIES,
        events: events_of!("flex"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn flow_layout_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::FlowLayout,
        canonical_name: "flow_layout",
        aliases: &["flow"],
        properties: FLOW_LAYOUT_PROPERTIES,
        events: events_of!("flow_layout"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn wrap_layout_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::WrapLayout,
        canonical_name: "wrap_layout",
        aliases: &["wrap"],
        properties: WRAP_LAYOUT_PROPERTIES,
        events: events_of!("wrap_layout"),
        commands: &[],
    }
}

/// `split_layout` — the two-pane spelling of [`splitter`](Self::splitter).
///
/// # Why a second name over the same control
///
/// A split *layout* is two panes and a draggable divider; a splitter is that same widget with
/// an arbitrary pane count. The concrete type, the properties and the events are therefore
/// identical, and the established pattern for "one control, two names" (`table_widget` /
/// `table`, `tool_box` / `toolbox`) applies. A separate type would duplicate the drag
/// machinery for no behavioural gain.
#[cfg(not(alloc_frugal))]
pub(crate) fn split_layout_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Splitter,
        canonical_name: "split_layout",
        aliases: &["split_view"],
        properties: SPLITTER_PROPERTIES,
        events: events_of!("splitter"),
        commands: &["set_orientation"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn accordion_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Accordion,
        canonical_name: "accordion",
        aliases: &["collapsible_list"],
        properties: ACCORDION_PROPERTIES,
        events: events_of!("accordion"),
        commands: &[],
    }
}

// ── Small display controls ──────────────────────────────────────────────────
