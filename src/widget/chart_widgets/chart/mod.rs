// The inner module keeps the control's own name: the folder is the family and the file is
// the control, so the historical `…::chart::chart` path stays valid. `module_inception` would
// prefer a different inner name; renaming here would move a published module path for a lint.
#![allow(clippy::module_inception)]

// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The Chart family: the general-purpose chart control and the two overlays it draws.
//!
//! This folder groups the whole family so its members are identifiable at a glance:
//!
//! * [`chart`] — the main `ChartWidget` control: the series model, the runtime-switchable
//!   [`chart::ChartType`], axes, legend, and the hover/click hit-testing.
//! * [`chart_crosshair`] — the companion `ChartCrosshair` painted at the hover position by
//!   the bar and line charts (COMP-F01).
//! * [`chart_reference_line`] — the companion `ChartReferenceLine` that draws fixed
//!   value/threshold lines (COMP-A1).
//!
//! # Layering
//!
//! These three files are all **control layer** code. The drawing engine they share stays
//! its own sibling module — [`super::charts`] (concrete renderers), [`super::layout`]
//! (cartesian plot area, axes, ticks, legend), [`super::types`] (engine data model),
//! [`super::svg`] (an SVG `ChartContext`) and [`super::adapter`] (a `ChartContext` over a
//! widget `RenderContext`). Grouping the control and moving the engine in with it would
//! make two layers share one path (principle #49), so only the family moves here.
//!
//! # Companion nature of the overlays
//!
//! `ChartCrosshair` and `ChartReferenceLine` are drawn *inside their owner* rather than as
//! standalone siblings, which is why they are listed in
//! `tools/check_drawn_types_are_covered.py` as covered by their owner. They remain separate
//! modules so each overlay's state, property surface and hit-test stay testable on its own.
//!
//! Each file stays its own module; this module only declares them. The previous flat paths
//! (`chart_widgets::chart_crosshair` and `chart_widgets::chart_reference_line`) keep
//! resolving through the re-exports in [`super`].

pub mod chart;
pub mod chart_crosshair;
pub mod chart_reference_line;

// Keep the historical `chart_widgets::chart::…` item paths alive now that `chart.rs` became a
// folder: the parent module's re-exports select from these names, and external code still
// names the control layer through the old module path.
pub use chart::{ChartCrosshair, ChartType, ChartWidget};
