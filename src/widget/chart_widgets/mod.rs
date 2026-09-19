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
//!   [`line_chart`], [`pie_chart`] and [`sparkline`]. They own `WidgetKind`
//!   semantics, events and signals.
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
#[cfg(widgets_unstripped)]
pub mod charts;
#[cfg(widgets_unstripped)]
pub mod layout;
pub mod line_chart;
pub mod pie_chart;
pub mod sparkline;
#[cfg(widgets_unstripped)]
pub mod svg;
#[cfg(widgets_unstripped)]
pub mod types;

// ── Control layer re-exports (the bare names mean the widgets) ──
pub use bar_chart::{BarChart, BarEntry};
pub use line_chart::LineChart;
pub use pie_chart::{PieChart, PieSlice};
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
#[cfg(widgets_unstripped)]
pub use types::{Chart, ChartContext, ChartSeries, ChartType, DataPoint};

#[cfg(all(test, widgets_unstripped))]
mod tests;
