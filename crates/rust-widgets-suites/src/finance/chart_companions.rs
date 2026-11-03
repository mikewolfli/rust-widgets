// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The `ChartDataTable` / `ZoomStrip` pairing shared by the finance panes (RW-106).
//!
//! # Why this lives here rather than in the parent crate
//!
//! The parent pairs its own charts with these companions through `attach_chart_data_table` /
//! `attach_zoom_strip`, but those methods are inherent to `ChartWidget` / `BarChart` /
//! `LineChart`. A finance pane is a different type, so its pairing is the same *protocol*
//! written against the companions' public queues and signals. Keeping it in one module means
//! the K-line, volume and indicator panes cannot drift in how they subscribe.
//!
//! # What the pairing is, exactly
//!
//! Two one-way channels, matching the parent's documented contract:
//!
//! * chart → table: the snapshot queue, drained by the table's own `sync_series_updates`;
//! * table → chart: the `series_toggled` signal, drained by the pane's poll;
//! * chart → strip: the mirrored zoom level, drained by `sync_zoom_updates`;
//! * strip → chart: `zoom_requested` / `fit_requested`, drained by the pane's poll.
//!
//! No pane owns a borrow of either companion: a widget cannot hold another widget, and the
//! shared queues are the same mechanism the parent uses.

use rust_widgets::compat::{lock, Arc, Mutex, Vec};
use rust_widgets::signal::{ConnectionScope, GenericSignal};
use rust_widgets::widget::companions::{ChartDataTable, ChartSeriesSnapshot, ZoomStrip};

/// The chart-side half of a `ChartDataTable` pairing.
pub(crate) struct ChartDataTableBinding {
    snapshots: Arc<Mutex<Option<ChartSeriesSnapshot>>>,
    toggles: Arc<Mutex<Vec<usize>>>,
    /// The snapshot last pushed, so a poll can skip an unchanged pane.
    pub(crate) last_snapshot: ChartSeriesSnapshot,
    _scope: ConnectionScope,
}

impl ChartDataTableBinding {
    /// Pairs `table` with `snapshot` and starts forwarding row toggles.
    ///
    /// The table is primed immediately, so an attached table shows the pane's current data on
    /// its next paint rather than staying empty until the series changes.
    pub(crate) fn attach(
        table: &mut ChartDataTable,
        redraw: GenericSignal,
        snapshot: ChartSeriesSnapshot,
    ) -> Self {
        let snapshots = Arc::new(Mutex::new(None));
        table.accept_series_updates(Arc::clone(&snapshots));

        let toggles = Arc::new(Mutex::new(Vec::new()));
        let scope = ConnectionScope::new();
        let sink = Arc::clone(&toggles);
        table.series_toggled.connect_scoped(&scope, move |index| {
            lock(&sink).push(*index);
            // A table edit redraws the pane, so the poll that applies it happens next frame.
            redraw.emit();
        });

        table.set_from_series(&snapshot.0);
        table.set_visible_series(snapshot.1.clone());
        Self { snapshots, toggles, last_snapshot: snapshot, _scope: scope }
    }

    /// Takes the row toggles the table emitted since the last poll.
    pub(crate) fn take_toggles(&self) -> Vec<usize> {
        core::mem::take(&mut *lock(&self.toggles))
    }

    /// Publishes `snapshot` when it differs from the last one pushed, or unconditionally when
    /// `force` is set.
    ///
    /// `force` exists for a refused toggle: the table changed its own checklist for a row the
    /// pane cannot hide, so the pane must re-push an unchanged snapshot to snap the checklist back.
    ///
    /// Returns whether a new snapshot was queued.
    pub(crate) fn push(&mut self, snapshot: ChartSeriesSnapshot, force: bool) -> bool {
        if !force && snapshot == self.last_snapshot {
            return false;
        }
        *lock(&self.snapshots) = Some(snapshot.clone());
        self.last_snapshot = snapshot;
        true
    }
}

/// The chart-side half of a `ZoomStrip` pairing.
pub(crate) struct ZoomStripBinding {
    requests: Arc<Mutex<Option<f32>>>,
    fit_requests: Arc<Mutex<bool>>,
    mirrors: Arc<Mutex<Option<f32>>>,
    /// The factor last mirrored into the strip, so an unchanged view is not re-pushed.
    pub(crate) last_pushed: f32,
    _scope: ConnectionScope,
}

impl ZoomStripBinding {
    /// Pairs `strip` with `factor` and starts forwarding its requests.
    pub(crate) fn attach(strip: &mut ZoomStrip, redraw: GenericSignal, factor: f32) -> Self {
        let requests = Arc::new(Mutex::new(None));
        let fit_requests = Arc::new(Mutex::new(false));
        let mirrors = Arc::new(Mutex::new(None));

        // Mirror the current view before subscribing: a level pushed now must not come back as a
        // fresh request.
        strip.set_zoom(factor);
        strip.accept_zoom_updates(Arc::clone(&mirrors));

        let scope = ConnectionScope::new();
        let sink = Arc::clone(&requests);
        let request_redraw = redraw.clone();
        strip.zoom_requested.connect_scoped(&scope, move |level| {
            *lock(&sink) = Some(*level);
            request_redraw.emit();
        });
        let fit_sink = Arc::clone(&fit_requests);
        strip.fit_requested.connect_scoped(&scope, move |_mode| {
            *lock(&fit_sink) = true;
            redraw.emit();
        });

        Self { requests, fit_requests, mirrors, last_pushed: factor, _scope: scope }
    }

    /// Takes the absolute zoom level the strip requested, if any.
    pub(crate) fn take_request(&self) -> Option<f32> {
        lock(&self.requests).take()
    }

    /// Takes the pending Fit request.
    pub(crate) fn take_fit(&self) -> bool {
        core::mem::replace(&mut *lock(&self.fit_requests), false)
    }

    /// Mirrors `factor` into the strip's readout when the pane's view changed.
    ///
    /// Returns whether a level was queued.
    pub(crate) fn push(&mut self, factor: f32) -> bool {
        if (self.last_pushed - factor).abs() < f32::EPSILON {
            return false;
        }
        self.last_pushed = factor;
        *lock(&self.mirrors) = Some(factor);
        true
    }
}

/// The bar-axis zoom of a pane as a factor over its full series.
pub(crate) fn zoom_factor(total: usize, view: Option<(usize, usize)>) -> f32 {
    if total == 0 {
        return 1.0;
    }
    let span = match view {
        Some((start, end)) => end.min(total).saturating_sub(start.min(total)),
        None => total,
    };
    if span == 0 {
        return 1.0;
    }
    (total as f32 / span as f32).max(1.0)
}

/// The bar window a strip's absolute `level` asks for, centred on `view`.
///
/// `None` means the whole series: a level of 1 (or a series too short to narrow) restores the
/// full view rather than producing an empty window.
pub(crate) fn zoom_range_for(
    level: f32,
    total: usize,
    view: Option<(usize, usize)>,
) -> Option<(usize, usize)> {
    if total == 0 || !level.is_finite() {
        return view;
    }
    let span = ((total as f32) / level.max(1.0)).round().max(1.0) as usize;
    let span = span.min(total);
    if span >= total {
        return None;
    }
    let (start, end) = match view {
        Some((start, end)) => (start.min(total), end.min(total)),
        None => (0, total),
    };
    let center = (start + end) / 2;
    let first = center.saturating_sub(span / 2).min(total - span);
    Some((first, first + span))
}

/// Drains a strip's requests and mirrors the pane's view back (RW-106).
///
/// The pane supplies its own range as a value plus a setter, so the three finance panes share the
/// request/resolve/mirror rules and cannot drift. Returns whether the view changed or a level
/// was mirrored.
pub(crate) fn poll_zoom_strip(
    binding: &mut Option<ZoomStripBinding>,
    total: usize,
    view: Option<(usize, usize)>,
    mut set_view: impl FnMut(Option<(usize, usize)>),
) -> bool {
    if binding.is_none() {
        return false;
    }
    let (request, fit) = match binding.as_ref() {
        Some(binding) => (binding.take_request(), binding.take_fit()),
        None => (None, false),
    };
    let mut current = view;
    let mut changed = false;
    if fit && current.is_some() {
        current = None;
        set_view(None);
        changed = true;
    }
    if let Some(level) = request {
        let range = zoom_range_for(level, total, current);
        if range != current {
            set_view(range);
            changed = true;
        }
        current = range;
    }
    if let Some(binding) = binding.as_mut() {
        binding.push(zoom_factor(total, current));
    }
    changed
}

#[cfg(test)]
mod tests {
    use crate::finance::{Bar, CandlestickChart, Overlay, PriceSeries, VolumeChart};
    use rust_widgets::core::Rect;
    use rust_widgets::widget::companions::{ChartDataTable, ZoomStrip};

    fn pane() -> CandlestickChart {
        let mut chart = CandlestickChart::new(Rect::new(0, 0, 480, 240));
        chart.set_series(PriceSeries::from_bars(
            (0..20)
                .map(|index| {
                    let base = 10.0 + f64::from(index);
                    Bar::new(base, base + 1.0, base - 1.0, base + 0.5, 100.0)
                })
                .collect(),
        ));
        chart
    }

    /// RW-106: a K-line pane pairs both companions, and a table row toggle really hides its
    /// overlay rather than leaving the table's checklist to claim it did.
    #[test]
    fn a_candlestick_pane_pairs_with_a_table_and_a_strip() {
        let mut chart = pane();
        chart.add_overlay(Overlay::moving_average(5));
        let mut table = ChartDataTable::new(Rect::new(0, 0, 240, 180));
        chart.attach_chart_data_table(&mut table);
        assert!(chart.is_chart_data_table_attached());
        assert_eq!(table.series_count(), 2, "the closes and the overlay");
        assert!(table.series_visible(1));

        assert!(table.toggle_series(1), "the overlay row exists");
        assert!(chart.poll_chart_data_table(), "the toggle reaches the pane");
        assert!(!chart.overlay_visible(0), "the overlay is hidden");
        assert!(table.sync_series_updates(), "the pane pushed the new visibility");
        assert!(!table.series_visible(1));

        // The base price row has no visibility flag: the toggle is refused and the table snaps
        // back rather than showing a hide that did not happen.
        assert!(table.toggle_series(0));
        assert!(chart.poll_chart_data_table());
        assert!(table.sync_series_updates());
        assert!(table.series_visible(0), "row 0 is re-reported as visible");

        chart.detach_chart_data_table();
        assert!(!chart.is_chart_data_table_attached());

        let mut strip = ZoomStrip::new(Rect::new(0, 0, 120, 24));
        chart.set_view_range(Some((0, 10)));
        chart.attach_zoom_strip(&mut strip);
        assert!(chart.is_zoom_strip_attached());
        assert!((strip.zoom_level() - 2.0).abs() < 0.001, "half the bars is 2x");

        strip.set_zoom(4.0);
        assert!(chart.poll_zoom_strip(), "the request reaches the pane");
        let (start, end) = chart.view_range().expect("a narrowed view");
        assert_eq!(end - start, 5, "a 4x level asks for a quarter of the bars");
        assert!(start + (end - start) <= 20);
    }

    /// RW-106: the volume pane pairs with the strip and refuses a data table by construction.
    #[test]
    fn a_volume_pane_pairs_with_a_strip() {
        let mut chart = VolumeChart::new(Rect::new(0, 0, 480, 120));
        chart.set_series(PriceSeries::from_bars(
            (0..10).map(|index| Bar::new(1.0, 1.0, 1.0, 1.0, f64::from(index + 1))).collect(),
        ));
        let mut strip = ZoomStrip::new(Rect::new(0, 0, 120, 24));
        chart.set_view_range(Some((0, 5)));
        chart.attach_zoom_strip(&mut strip);
        assert!((strip.zoom_level() - 2.0).abs() < 0.001);
        strip.set_zoom(1.0);
        assert!(chart.poll_zoom_strip());
        assert!(chart.view_range().is_none(), "a 1x level restores the full series");
        chart.detach_zoom_strip();
        assert!(!chart.is_zoom_strip_attached());
    }
}
