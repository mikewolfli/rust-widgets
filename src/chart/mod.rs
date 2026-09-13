// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Chart widgets and drawing contracts.
//!
//! # Two layers, deliberately
//!
//! This module is the **resolution-independent chart engine**: it owns the
//! geometry for cartesian layout, axes, tick marks and legends, and renders
//! through the [`types::ChartContext`] drawing trait. It has no widget semantics
//! (no `WidgetKind`, no events, no signals) and is not itself a widget.
//!
//! The widget-facing types live in [`crate::widget::chart_widgets`]. The two were
//! once disconnected — the engine had no callers and the widgets hand-rolled
//! their own margins, grid loops and tick math, duplicating it three times. The
//! `adapter` module now bridges them: a widget hands its `RenderContext` to
//! [`ChartContextAdapter`], and every chart widget draws its backdrop through
//! this engine. One implementation, tested once (including SVG snapshots),
//! used everywhere.
//!
//! Enabled by the `chart` feature. Builds without it (tablet/mobile) keep a
//! compact local fallback in the widgets.

/// Adapter presenting a widget `RenderContext` as a chart `ChartContext`.
///
/// This is what lets the chart engine in this module serve the
/// `widget::chart_widgets` types instead of their duplicating its math.
#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub mod adapter;
pub mod charts;
pub mod layout;
pub mod svg;
pub mod types;

#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub use crate::chart::adapter::ChartContextAdapter;
pub use crate::chart::charts::*;
pub use crate::chart::svg::*;
pub use crate::chart::types::*;

#[cfg(test)]
mod tests;
