// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Chart widgets and the chart drawing engine they share.
//!
//! # Two layers, one module tree
//!
//! This module deliberately contains **two layers** of chart code, because they
//! are two halves of one feature and shipping them in two disconnected trees was
//! the defect (see principle #1 / #8): the engine had no widget-aware sibling to
//! live beside, and the widgets duplicated the engine's math because reaching it
//! from another top-level module looked like an unrelated dependency.
//!
//! * **Control layer** — the data-visualisation *widgets*: [`bar_chart`],
//!   [`line_chart`], [`pie_chart`], [`sparkline`], and the classified set that moved in from
//!   `special_widgets` — [`chart`], [`heatmap`], [`radar_chart`], [`gantt_widget`],
//!   [`kanban_board`] and [`finance`]. They own `WidgetKind` semantics, events and signals.
//! * **Engine layer** — the resolution-independent *drawing* code: [`types`]
//!   (data model, [`types::Chart`] / [`types::ChartContext`] contracts), [`charts`]
//!   (concrete `LineChart`/`BarChart`/… renderers), [`layout`] (cartesian plot
//!   area, axes, ticks, legend), [`svg`] (an SVG `ChartContext`) and [`adapter`]
//!   (a `ChartContext` over a widget `RenderContext`).
//!
//! # No name collision
//!
//! The engine's `LineChart`/`BarChart`/`PieChart` and the widgets' namesakes are
//! **not** ambiguous, because the engine types are never re-exported at this
//! module's root — reach them through [`charts::LineChart`] et al. The bare
//! names (`chart_widgets::LineChart`) always mean the control layer. This is the
//! explicitly allowed form of principle #49: distinct layers, distinct paths.
//!
//! # Adapter
//!
//! [`adapter::ChartContextAdapter`] is why both layers exist here. Every
//! cartesian widget draws its backdrop through the engine, so the margins, grid
//! loops and tick math live in exactly one tested place instead of three.
//!
//! # Profile gating
//!
//! The whole module is `full_widgets`-gated in [`crate::widget`] (principle #47).
//! The engine files additionally keep their historical `mini`/`embedded` opt-out
//! so that a `full_widgets` build that also enables `mini` (possible via explicit
//! `--features`) still compiles, matching the previous `#[cfg(feature = "chart")]`
//! behaviour of the removed top-level `chart` module.

#[cfg(widgets_unstripped)]
pub mod adapter;
pub mod bar_chart;
/// The general-purpose chart panel (series model, axes, legend, crosshair hit-testing).
pub mod chart;
/// COMP-F01: vertical/horizontal chart reference lines with axis value chips.
pub mod chart_crosshair;
/// COMP-A1: fixed value/threshold reference lines for line and bar charts.
pub mod chart_reference_line;
#[cfg(widgets_unstripped)]
pub mod charts;
/// Market-data widgets: candles, depth, ladder, quote board, volume and indicators.
pub mod finance;
/// Gantt chart and the two companion controls it ships with.
pub mod gantt_widget;
/// Gauge — a radial dial showing one value against a range.
#[cfg(full_widgets)]
pub mod gauge;
/// Heatmap — a dense grid of colour-coded cells with a legend band.
pub mod heatmap;
/// Kanban board and the card/column controls it composes.
pub mod kanban_board;
#[cfg(widgets_unstripped)]
pub mod layout;
pub mod line_chart;
pub mod pie_chart;
/// Radar chart — a polar plot of series over shared axes.
pub mod radar_chart;
pub mod sparkline;
#[cfg(widgets_unstripped)]
pub mod svg;
#[cfg(widgets_unstripped)]
pub mod types;

// ── Control layer re-exports (the bare names mean the widgets) ──
pub use bar_chart::{BarChart, BarEntry};
pub use chart::ChartWidget;
pub use chart_crosshair::{ChartCrosshair, CrosshairMode};
pub use chart_reference_line::*;
pub use gantt_widget::{
    DateFormat, Dependency, DependencyEditor, GanttTask, GanttWidget, TimelineAxis,
};
pub use heatmap::{Heatmap, HeatmapCell};
pub use kanban_board::{
    CardPosition, KanbanBoard, KanbanCard, KanbanCardView, KanbanColumn, KanbanColumnHeader,
};
pub use line_chart::LineChart;
pub use pie_chart::{PieChart, PieSlice};
pub use radar_chart::RadarChart;
pub use sparkline::Sparkline;

// ── Engine layer re-exports (qualified, never shadowing the controls) ──
//
// The engine's `LineChart`/`BarChart`/`PieChart` are intentionally **not**
// re-exported here; use the `charts::` path so the control-layer names above
// stay unambiguous (principle #49).
#[cfg(widgets_unstripped)]
pub use adapter::ChartContextAdapter;
#[cfg(widgets_unstripped)]
pub use svg::{MemoryChartContext, SvgChartContext};
pub(crate) use types::{finite_samples, is_finite_sample};
#[cfg(widgets_unstripped)]
pub use types::{Axis, AxisEdge, Chart, ChartContext, ChartSeries, ChartType, DataPoint};

#[cfg(all(test, widgets_unstripped))]
mod tests;
