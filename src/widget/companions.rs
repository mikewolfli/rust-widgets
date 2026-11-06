// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Reusable companion controls: swatch grid, filter row, legend, chart tooltip, tooltip
//! pipeline, zoom/pan controller, toast queue, menu panel, speed dial, swipe actions, drawer
//! section header, layer panel and playlist panel.
//!
//! # Why these live together
//!
//! Each of these controls is the *partner* of a primary control rather than a standalone
//! feature: a colour dialog, a table, a chart, an image view. They share the same shape —
//! small owned state, one signal per user intent, and no dependency on the primary control —
//! so they are declared under one module. Each control has its own file so the 2000-line
//! production limit (principle #9) is not crossed as the set grows.
//!
//! # Capability publication
//!
//! The capability property/event tables in `src/widget/capability/` are the single
//! publication surface, and every companion publishes a row there. The seven foundational
//! companions — `Swatch`, `FilterRow`, `Legend`, `ChartTooltip`, `TooltipManager`,
//! `ZoomPanController` and `ToastHost` — own a kind of their own and are registered by name
//! (`capability/registration.rs`), with their property/event rows in
//! `capability/properties/other_companion.rs`. The remaining companions (`MenuPanel`,
//! `SpeedDial`, `SwipeActions`, `DrawerSectionHeader`, `LayerPanel`, `PlaylistPanel` and the
//! COMP-21..29 overlays) are registered the same way, so a document can create any companion
//! by name instead of reaching it through its Rust type only.
//!
//! # Reused models instead of local copies
//!
//! * [`FilterRow`] stores [`FilterCondition`](crate::widget::view_widgets::filter_expr::FilterCondition)
//!   and [`FilterOperator`](crate::widget::view_widgets::filter_expr::FilterOperator), so a
//!   filter row, `DataGrid` and a query builder evaluate one filter model. Its published
//!   `filter_changed` payload is the same structured condition list (`CAP-1`).
//! * [`ToastHost`] accepts `Toast` and `ToastItem` from the existing toast module and reuses its
//!   `ToastLevel`; `push`/`push_at` admit through `push_item`, so both payload spellings share
//!   one admission path.
//!
//! # `WidgetKind` ownership
//!
//! The shared `WidgetKind` enum is owned by the capability/schema surface. Thirteen companions
//! own a kind they report under their own name: `Swatch`, `FilterRow`, `Legend`, `ChartTooltip`,
//! `TooltipManager`, `ZoomPanController`, `ToastHost`, `MenuPanel`, `SpeedDial`, `SwipeActions`,
//! `DrawerSectionHeader`, `LayerPanel` and `PlaylistPanel`. The other thirteen report the kind of
//! the primary control they partner (see the deviation table below); behaviour lives here in
//! both cases.
//!
//! # What the borrowed kinds cost in audits (G-306)
//!
//! The borrowed-kind mapping is not invisible to kind-level accounting: any `WidgetKind`
//! statistic, and any registry audit that asks "which module implements this kind?", counts a
//! borrowing companion under the kind it borrows. The current deviation set is:
//!
//! * `ListStateHost` → `EmptyState`
//! * `SummaryRow`, `ChartDataTable` → `Table`
//! * `ZoomStrip` → `Slider`
//! * `AxisPicker`, `CalendarMarkLayer`, `DragGhostLayer`, `LineNumberGutter`,
//!   `DropDownPopupHost` → `Panel`
//! * `FindReplaceBar` → `SearchBar`
//! * `GridStatusBar` → `StatusBar`
//! * `HeaderMenu` → `Menu`
//! * `CommandPaletteHost` → `ListView`
//!
//! The direction is one-way — the borrowed kind never claims the companion as its own type —
//! but a report saying "N controls of kind X" includes the companion even when X's module does
//! not implement it. Each borrower still has its own capability row and a concrete-type
//! tie-break in `WidgetFactory::widget_matches_capability` (`capability.rs`), so the name route
//! stays unambiguous; a reader that needs "N controls *implemented by* kind X" must subtract
//! this set. This is the same rule `registration.rs` states for the COMP-21..29 registrations:
//! those overlays "report the kind of the control they partner".
//!
//! # Safety
//!
//! These modules contain no `unsafe` code and own all of their state. There are therefore no
//! unsafe preconditions to uphold: geometry and property inputs are validated and clamped in
//! safe Rust, and every collection is indexed through checked accessors.

mod axis_picker;
mod calendar_mark_layer;
mod chart_data_table;
mod chart_tooltip;
mod command_palette_host;
mod drag_ghost_layer;
mod drawer_section_header;
mod drop_down_popup_host;
mod filter_row;
mod find_replace_bar;
mod grid_status_bar;
mod header_menu;
mod helpers;
mod layer_panel;
mod legend;
mod line_number_gutter;
mod list_state_host;
mod menu_panel;
mod playlist_panel;
mod speed_dial;
mod summary_row;
mod swatch;
mod swipe_actions;
mod toast_host;
mod tooltip_manager;
mod zoom_pan;
mod zoom_strip;

#[cfg(test)]
mod tests;

pub use crate::widget::chart_widgets::chart_crosshair::{ChartCrosshair, CrosshairMode};
pub use crate::widget::container_widgets::sticky_header::StickyHeader;
pub use crate::widget::display_widgets::tag_group::{TagGroup, TagSelectionMode};
pub use crate::widget::input_widgets::input_companions::{
    Chip, DropSide, FormatAction, FormatActionSet, FormatToolbar, ScrollOrientation, Scrollbar,
    StepButtons, StepDirection, StepOrientation,
};
pub use crate::widget::view_widgets::column_chooser::ColumnChooser;
pub use crate::widget::view_widgets::grid_find_bar::GridFindBar;
pub use crate::widget::view_widgets::thumbnail_cache::ThumbnailCache;
pub use axis_picker::{AxisEdge, AxisPicker};
// `AxisSpec` is the module's own property-routing type: it has no consumer outside
// `axis_picker.rs`, so it is not re-exported at the module root (CP2-6) and remains a `pub`
// spelling inside `axis_picker` (`axis_picker::AxisSpec`) rather than part of this module's
// public surface. `AxisEdge` is the shared enum the chart family consumes, which is why it is
// exported here.
/// The record schema that lets the overlay's marks be written through the property contract.
pub(crate) use calendar_mark_layer::CALENDAR_MARK_SCHEMA;
pub use calendar_mark_layer::{CalendarMark, CalendarMarkLayer, CalendarMarkShape};
pub use chart_data_table::{ChartDataMode, ChartDataTable, ChartSeriesSnapshot};
pub use chart_tooltip::{ChartTooltip, TooltipLine};
pub use command_palette_host::{
    CommandPaletteHost, PaletteEntryRef, PaletteSource, PALETTE_ENTRY_SCHEMA,
};
pub use drag_ghost_layer::DragGhostLayer;
pub use drawer_section_header::DrawerSectionHeader;
pub use drop_down_popup_host::{
    Direction, DropDownPopupHost, PopupItem, PopupPlacement, POPUP_ITEM_SCHEMA,
};
pub use filter_row::FilterRow;
pub use find_replace_bar::FindReplaceBar;
pub use grid_status_bar::GridStatusBar;
pub use header_menu::{HeaderMenu, HeaderMenuAction, SortState};
pub use layer_panel::{LayerEntry, LayerPanel, LAYER_ENTRY_SCHEMA};
pub use legend::{Legend, LegendEntry, LegendOrientation, LegendPosition};
pub use line_number_gutter::{LineNumberAlign, LineNumberGutter};
pub use list_state_host::{ListState, ListStateHost};
pub use menu_panel::{MenuPanel, MenuPanelItem, MENU_PANEL_ITEM_SCHEMA};
pub use playlist_panel::{PlaylistPanel, PLAYLIST_TRACK_SCHEMA};
pub use speed_dial::{SpeedDial, SpeedDialAction, SpeedDialDirection, SPEED_DIAL_ACTION_SCHEMA};
/// The record schema for a summary row's per-column values.
pub(crate) use summary_row::SUMMARY_VALUE_SCHEMA;
pub use summary_row::{ColumnAggregate, ColumnAggregateKind, SummaryRow};
pub use swatch::Swatch;
pub use swipe_actions::{SwipeAction, SwipeActions, SwipeSide, SWIPE_ACTION_SCHEMA};
pub use toast_host::{
    HostedToast, ToastAnchor, ToastHost, ToastOverflowPolicy, ToastStackingDirection,
};
pub use tooltip_manager::TooltipManager;
pub use zoom_pan::{ZoomMode, ZoomPanController};
pub use zoom_strip::ZoomStrip;
