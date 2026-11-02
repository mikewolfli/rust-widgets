// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Chart-series data binding (RW-68).
//!
//! # Why the chart family needed a shared snapshot
//!
//! The chart setters take owned `Vec`s (`LineChart::set_data`, `BarChart::set_series`), so a
//! host that keeps its model in a `Binding` had no way to push a change: the model and the
//! control each held a different value, and only a full re-mount re-synchronised them. This
//! module provides the smallest shared shape — a [`ChartSeries`] snapshot with a monotonic
//! [`ChartSeries::revision`] — and the `bind_chart_series` bridge that writes it into the
//! control's own setter.
//!
//! # Scope
//!
//! The bridge is model → control, one way. A chart gesture (legend toggle, zoom) mutates the
//! control but not the model; mirroring those back would require the chart to publish its
//! series on every mutation and is intentionally not claimed here.

use crate::compat::{format, String, Vec};
use crate::core::ObjectId;
use crate::data_binding::Binding;
use crate::widget::chart_widgets::bar_chart::{BarChart, BarSeries};
use crate::widget::chart_widgets::line_chart::LineChart;

/// One chart snapshot: ordered `(x, y)` points plus optional category labels.
///
/// A line chart reads [`Self::points`] directly. A bar chart reads the `y` components as its
/// values, and uses [`Self::labels`] when the lengths agree; otherwise it derives the labels
/// from the `x` components, which is the same rule the chart's own property carrier uses.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ChartSeries {
    /// The ordered samples.
    pub points: Vec<(f64, f64)>,
    /// Category labels, parallel to `points` for a bar chart. Empty means "derive from x".
    pub labels: Vec<String>,
    /// Monotonic revision, bumped by [`Self::set_points`] and [`Self::set`].
    revision: u64,
}

impl ChartSeries {
    /// Creates an empty snapshot.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a snapshot from points, with labels derived from the `x` components.
    pub fn from_points(points: Vec<(f64, f64)>) -> Self {
        Self { points, labels: Vec::new(), revision: 0 }
    }

    /// Creates a snapshot from per-category values (`x` is the index).
    pub fn from_values(values: &[f64]) -> Self {
        Self {
            points: values
                .iter()
                .enumerate()
                .map(|(index, value)| (index as f64, *value))
                .collect(),
            labels: Vec::new(),
            revision: 0,
        }
    }

    /// Creates a bar-chart snapshot from values and their category labels.
    pub fn from_labeled_values(values: &[f64], labels: Vec<String>) -> Self {
        let mut snapshot = Self::from_values(values);
        snapshot.labels = labels;
        snapshot
    }

    /// The samples.
    pub fn points(&self) -> &[(f64, f64)] {
        &self.points
    }

    /// The `y` components, in order.
    pub fn values(&self) -> Vec<f64> {
        self.points.iter().map(|(_, y)| *y).collect()
    }

    /// The revision, bumped by every write through this type.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Replaces the samples and bumps the revision.
    pub fn set_points(&mut self, points: Vec<(f64, f64)>) {
        self.points = points;
        self.revision = self.revision.wrapping_add(1);
    }

    /// Replaces the whole snapshot and bumps the revision.
    pub fn set(&mut self, points: Vec<(f64, f64)>, labels: Vec<String>) {
        self.points = points;
        self.labels = labels;
        self.revision = self.revision.wrapping_add(1);
    }

    /// The labels a bar chart should use: the explicit ones when they line up, otherwise the `x`
    /// components spelled out.
    fn resolved_labels(&self) -> Vec<String> {
        if self.labels.len() == self.points.len() {
            return self.labels.clone();
        }
        self.points.iter().map(|(x, _)| format!("{}", x)).collect()
    }
}

/// Writes one snapshot into a chart control.
///
/// Returns `false` when the mounted control is neither a line chart nor a bar chart, which is the
/// honest answer for an id that names something else.
pub fn write_chart_series(widget_id: ObjectId, series: &ChartSeries) -> bool {
    crate::widget::runtime::with_widget_mut(widget_id, |widget| {
        let any = widget as &mut dyn core::any::Any;
        if let Some(line) = any.downcast_mut::<LineChart>() {
            line.set_data(series.points.clone());
            true
        } else if let Some(bar) = any.downcast_mut::<BarChart>() {
            bar.set_series(vec![BarSeries::new("", series.values())], series.resolved_labels());
            true
        } else {
            false
        }
    })
    .unwrap_or(false)
}

/// Binds a [`Binding<ChartSeries>`] to a chart control (model → control).
///
/// The initial value is applied immediately, mirroring `bind_widget_property`; every later
/// `Binding::set` applies through the chart's own setter. `key` is the subscription name, so the
/// same binding can drive several charts under distinct keys.
///
/// Returns whether the initial write reached a line or bar chart. A `false` means the id is not a
/// supported chart, and no subscription is installed — a silent success would promise updates that
/// can never arrive.
pub fn bind_chart_series(binding: &Binding<ChartSeries>, key: &str, widget_id: ObjectId) -> bool {
    let initial = binding.get();
    if !write_chart_series(widget_id, &initial) {
        return false;
    }
    binding.observe(key, move |series| {
        let _ = write_chart_series(widget_id, series);
    });
    true
}
