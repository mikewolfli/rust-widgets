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
//! `ToastHost` → `PopupWindow`. The kind is bookkeeping only; behaviour is implemented here.
//!
//! # Safety
//!
//! These modules contain no `unsafe` code and own all of their state. There are therefore no
//! unsafe preconditions to uphold: geometry and property inputs are validated and clamped in
//! safe Rust, and every collection is indexed through checked accessors.

mod chart_tooltip;
mod filter_row;
mod helpers;
mod legend;
mod swatch;
mod toast_host;
mod tooltip_manager;
mod zoom_pan;

#[cfg(test)]
mod tests;

pub use chart_tooltip::{ChartTooltip, TooltipLine};
pub use filter_row::FilterRow;
pub use legend::{Legend, LegendEntry, LegendOrientation, LegendPosition};
pub use swatch::Swatch;
pub use toast_host::{
    HostedToast, ToastAnchor, ToastHost, ToastOverflowPolicy, ToastStackingDirection,
};
pub use tooltip_manager::TooltipManager;
pub use zoom_pan::{ZoomMode, ZoomPanController};
