// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Reusable companion controls: swatch grid, filter row, legend, chart tooltip, tooltip
//! pipeline, zoom/pan controller and toast queue.
//!
//! # Why these live together
//!
//! Each of these controls is the *partner* of a primary control rather than a standalone
//! feature: a colour dialog, a table, a chart, an image view. They share the same shape —
//! small owned state, one signal per user intent, and no dependency on the primary control —
//! so they are declared under one module. Each control has its own file so the 2000-line
//! production limit (principle #9) is not crossed as the set grows.
//!
//! The capability property/event tables in `src/widget/capability/` remain the single
//! publication surface; this module adds no entries there.
//!
//! # Reused models instead of local copies
//!
//! * [`FilterRow`] stores [`FilterCondition`](crate::widget::view_widgets::filter_expr::FilterCondition)
//!   and [`FilterOperator`](crate::widget::view_widgets::filter_expr::FilterOperator), so a
//!   filter row, `DataGrid` and a query builder evaluate one filter model.
//! * [`ToastHost`] accepts `Toast` and `ToastItem` from the existing toast module and reuses
//!   its `ToastLevel`.
//!
//! # `WidgetKind` mapping
//!
//! The shared `WidgetKind` enum is intentionally not extended here (it is owned by the
//! capability/schema surface). Each companion declares the closest existing kind for base
//! bookkeeping: `Swatch` → `ColorHistory`, `FilterRow` → `QueryBuilder`, `Legend` → `Chart`,
//! `ChartTooltip`/`TooltipManager` → `Tooltip`, `ZoomPanController` → `ImageView`,
//! `ToastHost` → `PopupWindow`, `MenuPanel` → `Menu`, `SpeedDial` → `FAB`,
//! `SwipeActions` → `SwipeToDismiss` and `DrawerSectionHeader` → `Label`. The kind is
//! bookkeeping only; behaviour is implemented here.
//!
//! # What the borrowed kinds cost in audits (G-306)
//!
//! The mapping is not invisible to kind-level accounting: any `WidgetKind` statistic, and
//! any registry audit that asks "which module implements this kind?", counts a companion
//! under the kind it borrows. A `Swatch` is counted as `ColorHistory`, a `FilterRow` as
//! `QueryBuilder`, a `Legend` as `Chart`, a `ChartTooltip`/`TooltipManager` as `Tooltip`, a
//! `ZoomPanController` as `ImageView` and a `ToastHost` as `PopupWindow`. The direction is
//! one-way — the borrowed kind never claims the companion as its own type — but a report
//! saying "N controls of kind X" includes the companion even when X's module does not
//! implement it. Because the capability tables publish no row for these controls (see
//! "Reused models instead of local copies" above), a reader that needs an exact answer
//! must consult this table rather than the reported kind alone; a registry test that pins
//! the borrowed-kind → borrower pairs would make the deviation testable instead of only
//! documented.
//!
//! # Safety
//!
//! These modules contain no `unsafe` code and own all of their state. There are therefore no
//! unsafe preconditions to uphold: geometry and property inputs are validated and clamped in
//! safe Rust, and every collection is indexed through checked accessors.

mod chart_tooltip;
mod drawer_section_header;
mod filter_row;
mod helpers;
mod layer_panel;
mod legend;
mod menu_panel;
mod playlist_panel;
mod speed_dial;
mod swatch;
mod swipe_actions;
mod toast_host;
mod tooltip_manager;
mod zoom_pan;

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
pub use chart_tooltip::{ChartTooltip, TooltipLine};
pub use drawer_section_header::DrawerSectionHeader;
pub use filter_row::FilterRow;
pub use layer_panel::{LayerEntry, LayerPanel};
pub use legend::{Legend, LegendEntry, LegendOrientation, LegendPosition};
pub use menu_panel::{MenuPanel, MenuPanelItem};
pub use playlist_panel::PlaylistPanel;
pub use speed_dial::{SpeedDial, SpeedDialAction, SpeedDialDirection};
pub use swatch::Swatch;
pub use swipe_actions::{SwipeAction, SwipeActions, SwipeSide};
pub use toast_host::{
    HostedToast, ToastAnchor, ToastHost, ToastOverflowPolicy, ToastStackingDirection,
};
pub use tooltip_manager::TooltipManager;
pub use zoom_pan::{ZoomMode, ZoomPanController};
