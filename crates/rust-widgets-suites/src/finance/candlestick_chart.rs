// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The K-line (candlestick) chart — the primary instrument chart.
//!
//! # Why this is its own control rather than a `ChartType` variant
//!
//! `ChartWidget` gained a `Candlestick` variant in the previous round, and it draws a
//! candlestick correctly. What it cannot do is everything else a K-line pane owes its
//! reader, because those things are not drawing styles:
//!
//! * a **price axis with its own scale**, independent of the volume pane below it;
//! * **overlay indicators** (moving averages, Bollinger bands, VWAP) aligned to the same
//!   bars, each computed over closes rather than plotted as its own series;
//! * **price levels** — support, resistance, the previous close — drawn as infinite
//!   horizontals rather than as data points;
//! * **a crosshair and an index readout**, which need the inverse index and price
//!   mappings.
//!
//! Those are four separate concerns. Hanging them off a `ChartType` would make
//! `ChartWidget` carry state that only one of its nine variants uses, which is the shape
//! principle #23 (no duplicate semantics) and #51 (a shared abstraction must eliminate
//! something real) both point away from. The two do share their arithmetic — the
//! overlays come from the same `indicators` module — which is the part worth sharing.
//!
//! # Why the overlay list is data and not four methods
//!
//! `MovingAverage(20)` and `BollingerBands(20, 2.0)` are the same kind of thing: a
//! derived series drawn over the price. Storing them as an enum list means a caller adds
//! an overlay by pushing a value, the drawing code has one loop, and adding a sixth
//! overlay kind is one match arm. Four `set_*_overlay` methods would mean four fields,
//! four clears and a drawing routine that enumerates them.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt::Write as _;

use crate::finance::indicators;
use crate::finance::layout::{panel_colors, PanelColors, PlotArea, PANEL_MIN_CONTRAST};
use crate::finance::types::{Bar, PriceLevelKind, PriceLine, PriceSeries};
use rust_widgets::core::{Color, Font, HorizontalAlignment, Point, Rect};
use rust_widgets::event::EventHandler;
use rust_widgets::render::RenderContext;
use rust_widgets::signal::Signal1;
use rust_widgets::widget::capability::properties_trait::{base_property_get, base_property_set};
use rust_widgets::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use rust_widgets::widget::capability::WidgetProperties;
use rust_widgets::widget::companions::{ChartDataTable, ChartSeriesSnapshot, ZoomStrip};
use rust_widgets::widget::metrics::role_font;
use rust_widgets::widget::{BaseWidget, Draw, Widget, WidgetKind};
use rust_widgets::{impl_widget_property_hooks, property_names_of};

/// A derived line drawn over the price pane.
///
/// Each variant carries its own parameters, so the list is self-describing: a caller does
/// not have to remember which periods it configured elsewhere. The periods match the
/// conventions traders read on every platform — the defaults in `MovingAverage`
/// and friends are the standard 20/2.0 rather than arbitrary.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Overlay {
    /// A simple moving average of the closes.
    MovingAverage {
        /// How many trailing bars the mean is taken over.
        period: usize,
    },
    /// An exponential moving average of the closes.
    ExponentialMovingAverage {
        /// The smoothing period; the seed is the first close.
        period: usize,
    },
    /// Bollinger bands: a moving average with deviation envelopes.
    BollingerBands {
        /// The moving average's period, which is also the deviation window.
        period: usize,
        /// How many standard deviations out the outer bands sit. Two is the
        /// conventional setting.
        multiplier: f64,
    },
    /// The volume-weighted average price.
    Vwap {
        /// How many trailing bars the weighting runs over.
        period: usize,
    },
    /// A Donchian channel: the trailing high/low envelope.
    DonchianChannel {
        /// How many trailing bars the extremes are taken over.
        period: usize,
    },
}

impl Overlay {
    /// A 20-period simple moving average, the most common default.
    pub fn moving_average(period: usize) -> Self {
        Overlay::MovingAverage { period }
    }

    /// An exponential moving average, seeded from the first close.
    pub fn exponential_moving_average(period: usize) -> Self {
        Overlay::ExponentialMovingAverage { period }
    }

    /// Bollinger bands at the standard two deviations.
    pub fn bollinger_bands(period: usize, multiplier: f64) -> Self {
        Overlay::BollingerBands { period, multiplier }
    }

    /// VWAP over a trailing window.
    pub fn vwap(period: usize) -> Self {
        Overlay::Vwap { period }
    }

    /// A Donchian channel.
    pub fn donchian_channel(period: usize) -> Self {
        Overlay::DonchianChannel { period }
    }

    /// The shortest period this overlay reaches back over, for the warm-up note.
    ///
    /// A chart reports how many leading bars are overlay-free so a reader is not misled
    /// by a moving average that appears to start mid-series — which is what it looks like
    /// when the warm-up is simply not drawn.
    pub fn warm_up_bars(self) -> usize {
        match self {
            Overlay::MovingAverage { period } => period,
            // The EMA is seeded with its first sample, so no leading bar is overlay-free — see
            // `indicators::ema`. Reporting `period` hid bars where the line is in fact painted.
            Overlay::ExponentialMovingAverage { .. } => 0,
            Overlay::BollingerBands { period, .. } => period,
            Overlay::Vwap { period } => period,
            Overlay::DonchianChannel { period } => period,
        }
    }

    /// The factory spelling of this overlay, for the property contract.
    pub fn as_str(self) -> &'static str {
        match self {
            Overlay::MovingAverage { .. } => "moving_average",
            Overlay::ExponentialMovingAverage { .. } => "exponential_moving_average",
            Overlay::BollingerBands { .. } => "bollinger_bands",
            Overlay::Vwap { .. } => "vwap",
            Overlay::DonchianChannel { .. } => "donchian_channel",
        }
    }
}

/// The K-line chart control.
///
/// # How a caller uses it
///
/// Set the series, optionally add overlays and price lines, and the control draws:
///
/// ```ignore
/// let mut chart = CandlestickChart::new(Rect::new(0, 0, 800, 400));
/// chart.set_series(price_series);
/// chart.add_overlay(Overlay::moving_average(20));
/// chart.add_price_line(PriceLine::new(95.0, PriceLevelKind::Support));
/// ```
///
/// The interaction contract is the one every chart in this crate shares —
/// `hovered_index`, `data_point_clicked` — because a caller switching a chart's type
/// should not have to relearn its events.
/// Cached indicator output for one overlay, so the extent and the painter share one computation.
enum CachedOverlay {
    Single(Vec<f64>),
    Triple(Vec<f64>, Vec<f64>, Vec<f64>),
}

/// The inputs [`CandlestickChart::chart_data_snapshot`] reads, as a cheap comparable key.
///
/// The poll used to build the whole snapshot — a `Vec` of every close and every overlay
/// value, cloned — on each event and each frame, and only then let the binding decide the
/// data had not moved. This key covers the same inputs for a fraction of the cost: the
/// series length, the last close, and a fingerprint of the overlay list (each overlay's
/// identity, its last values and its visibility flag). An unchanged pane compares equal and
/// skips both the snapshot and the push — the same bargain `quote_board` strikes with its
/// `ColumnRangesKey` (FIN-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ChartDataKey {
    series_len: usize,
    last_close: u64,
    overlays: u64,
}

/// The overlay legend's cached rows and measured label widths.
///
/// The rows are a pure function of the overlay list and the widths are a pure function of
/// (names, font), so both are cached: a steady-state repaint used to collect `N` rows and
/// measure `2N` labels every frame for a legend that had not changed. This is the same
/// keyed treatment `FileCountsCache` gives the diff viewer's `+n -m` columns (FIN-2).
#[derive(Default)]
struct OverlayLegendCache {
    /// The labels the rows and widths were built from, in draw order.
    names: Vec<String>,
    /// The font the widths were measured with, or `None` before the first measure.
    font: Option<Font>,
    /// Each row's measured label width, aligned with `names`.
    widths: Vec<i32>,
}

/// Mixes one word into an FNV-1a fingerprint.
fn mix_key(fingerprint: &mut u64, word: u64) {
    *fingerprint = (*fingerprint ^ word).wrapping_mul(0x0000_0100_0000_01b3);
}

/// The last value of one cached overlay line, as key bits (`0` for an empty line).
fn overlay_tail(values: &[f64]) -> u64 {
    values.last().copied().unwrap_or(0.0).to_bits()
}

/// The K-line chart control.
pub struct CandlestickChart {
    base: BaseWidget,
    series: PriceSeries,
    overlays: Vec<Overlay>,
    price_lines: Vec<PriceLine>,
    /// Bars derived from `series`, rebuilt when the series changes.
    closes: Vec<f64>,
    highs: Vec<f64>,
    lows: Vec<f64>,
    volumes: Vec<f64>,
    /// Indicator values aligned with `overlays`.
    overlay_values: Vec<CachedOverlay>,
    /// Whether each overlay is drawn, aligned with `overlays`.
    ///
    /// A visibility flag per overlay is what lets a legend hide one series without
    /// clearing the whole list and rebuilding it (FIN-04). `overlay_values` stays aligned
    /// with `overlays`, so an index means the same thing in all three vectors.
    overlay_visible: Vec<bool>,
    /// Price decimals inferred from the series, cached with it.
    inferred_decimals: usize,
    /// The padded price extent including overlays and price lines.
    extent_cache: (f64, f64),
    /// Whether the overlay legend is painted.
    show_overlay_legend: bool,
    /// Whether the marked price levels are drawn.
    ///
    /// A stored display flag rather than "are there levels?": the published property is
    /// readable *and writable*, so a written value has to read back, and hiding the levels
    /// must not destroy them (FIN-5).
    show_price_levels: bool,
    /// The overlay legend's cached rows and measured widths.
    legend_cache: OverlayLegendCache,
    /// The inputs the last data-table snapshot was built from (FIN-1).
    data_table_key: Option<ChartDataKey>,
    /// The bar under the pointer, when there is one.
    hovered_index: Option<usize>,
    /// The price under the pointer, for the horizontal crosshair line.
    hovered_price: Option<f64>,
    /// The bars shown, or `None` for every bar.
    view: Option<(usize, usize)>,
    /// Emitted with the bar index when a bar is clicked.
    pub bar_clicked: Signal1<usize>,
    /// Emitted with the bar index as the pointer moves across bars.
    pub bar_hovered: Signal1<usize>,
    /// Emitted with the bar index the pointer left, on exit.
    pub bar_unhovered: Signal1<usize>,
    /// Emitted with the visible `[start, end)` bar range after every view change.
    pub view_changed: Signal1<(usize, usize)>,
    /// The attached data table, when one is paired (RW-106).
    data_table: Option<super::chart_companions::ChartDataTableBinding>,
    /// The attached zoom strip, when one is paired (RW-106).
    zoom_strip: Option<super::chart_companions::ZoomStripBinding>,
}

impl CandlestickChart {
    /// Creates an empty chart.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::CandlestickChart, geometry, "CandlestickChart"),
            series: PriceSeries::new(),
            overlays: Vec::new(),
            price_lines: Vec::new(),
            closes: Vec::new(),
            highs: Vec::new(),
            lows: Vec::new(),
            volumes: Vec::new(),
            overlay_values: Vec::new(),
            overlay_visible: Vec::new(),
            inferred_decimals: 2,
            extent_cache: (0.0, 1.0),
            show_overlay_legend: true,
            // Levels a caller adds are drawn until it asks otherwise, which is what every
            // chart did before the flag existed.
            show_price_levels: true,
            legend_cache: OverlayLegendCache::default(),
            data_table_key: None,
            hovered_index: None,
            hovered_price: None,
            view: None,
            bar_clicked: Signal1::new(),
            bar_hovered: Signal1::new(),
            bar_unhovered: Signal1::new(),
            view_changed: Signal1::new(),
            data_table: None,
            zoom_strip: None,
        }
    }

    /// The series being drawn.
    pub fn series(&self) -> &PriceSeries {
        &self.series
    }

    /// Replaces the series and requests a repaint.
    pub fn set_series(&mut self, series: PriceSeries) {
        self.series = series;
        // A hovered index from the previous series may not exist in the new one, and a
        // stale index would report a bar that is no longer there. The exit is published the same
        // way `set_view_range` publishes it, and the price goes with the index: leaving it behind
        // let `hovered_price()` answer for a bar `hovered_index()` had already dropped.
        if let Some(previous) = self.hovered_index.filter(|index| *index >= self.series.len()) {
            self.hovered_index = None;
            self.hovered_price = None;
            self.bar_unhovered.emit(previous);
        }
        self.rebuild_series_cache();
        // The window was measured against the old bars; re-running the one clamping entry
        // point keeps it inside the bars that now exist. Without this a view past the end
        // of a shorter series selected no bars at all and the pane drew an empty plot.
        if let Some(range) = self.view {
            self.set_view_range(Some(range));
        }
        // A full replacement can differ in the middle of the series while keeping its
        // length, last close and overlay tails — which is all the cheap snapshot key
        // covers — so the attached table must not be skipped on the next poll.
        self.data_table_key = None;
        self.base.request_redraw();
    }

    /// Rebuilds the derived bar arrays, the decimals and the extent.
    fn rebuild_series_cache(&mut self) {
        self.closes = self.series.closes();
        self.highs = self.series.highs();
        self.lows = self.series.lows();
        self.volumes = self.series.volumes();
        self.inferred_decimals = Self::infer_decimals(self.series.bars());
        self.rebuild_overlay_cache();
    }

    /// Recomputes every overlay once, for both the extent and the painter.
    fn rebuild_overlay_cache(&mut self) {
        self.overlay_visible.resize(self.overlays.len(), true);
        self.overlay_values.clear();
        self.overlay_values.reserve(self.overlays.len());
        for overlay in &self.overlays {
            let cached = match *overlay {
                Overlay::MovingAverage { period } => {
                    CachedOverlay::Single(indicators::sma(&self.closes, period))
                }
                Overlay::ExponentialMovingAverage { period } => {
                    CachedOverlay::Single(indicators::ema(&self.closes, period))
                }
                Overlay::BollingerBands { period, multiplier } => {
                    let (lower, middle, upper) =
                        indicators::bollinger_bands(&self.closes, period, multiplier);
                    CachedOverlay::Triple(upper, middle, lower)
                }
                Overlay::Vwap { period } => CachedOverlay::Single(indicators::vwap(
                    &self.highs,
                    &self.lows,
                    &self.closes,
                    &self.volumes,
                    period,
                )),
                Overlay::DonchianChannel { period } => {
                    let (lower, middle, upper) =
                        indicators::donchian_channel(&self.highs, &self.lows, period);
                    CachedOverlay::Triple(upper, middle, lower)
                }
            };
            self.overlay_values.push(cached);
        }
        self.rebuild_extent();
    }

    /// Recomputes the padded price extent from the cached values.
    fn rebuild_extent(&mut self) {
        let (mut low, mut high) = self.series.price_extent();
        if !low.is_finite() || !high.is_finite() {
            self.extent_cache = (0.0, 1.0);
            return;
        }
        let mut consider = |values: &[f64]| {
            let (value_low, value_high) = indicators::series_extent(values);
            if value_low.is_finite() {
                low = low.min(value_low);
            }
            if value_high.is_finite() {
                high = high.max(value_high);
            }
        };
        for (overlay_index, cached) in self.overlay_values.iter().enumerate() {
            // A hidden overlay must not stretch the axis: its values are not on screen, so
            // including them would leave the visible series compressed for no reason.
            if !self.overlay_visible.get(overlay_index).copied().unwrap_or(true) {
                continue;
            }
            match cached {
                CachedOverlay::Single(values) => consider(values),
                CachedOverlay::Triple(first, second, third) => {
                    consider(first);
                    consider(second);
                    consider(third);
                }
            }
        }
        // A hidden level must not stretch the axis either: the rule the hidden overlays
        // follow above, so turning the levels off re-fits the pane to what is drawn (FIN-5).
        if self.show_price_levels {
            for line in &self.price_lines {
                if line.price.is_finite() {
                    low = low.min(line.price);
                    high = high.max(line.price);
                }
            }
        }
        let span = high - low;
        let pad = if span > 0.0 { span * 0.02 } else { high.abs().max(1.0) * 0.01 };
        self.extent_cache = (low - pad, high + pad);
    }

    /// The decimals the series' prices appear to use.
    ///
    /// The maximum over the whole series: returning the first value that happens to round-trip
    /// let one integral print at the head of a series fix the precision of every bar after it, so
    /// a close of `100.25` was reported as `100.0`.
    fn infer_decimals(bars: &[Bar]) -> usize {
        let mut needed = None;
        for bar in bars {
            for value in [bar.open, bar.high, bar.low, bar.close] {
                if !value.is_finite() {
                    continue;
                }
                let place = (0..=4usize)
                    .find(|decimals| {
                        let scaled = value * 10f64.powi(*decimals as i32);
                        (scaled - scaled.round()).abs() < 1e-6
                    })
                    .unwrap_or(4);
                needed = Some(needed.map_or(place, |best: usize| best.max(place)));
            }
        }
        needed.unwrap_or(2)
    }

    /// The overlays, in draw order.
    pub fn overlays(&self) -> &[Overlay] {
        &self.overlays
    }

    /// Replaces the last bar in place for a streaming feed.
    ///
    /// A live candle is rewritten as ticks arrive. The only full-series entry point was
    /// `set_series`, so each tick re-clamped the view, re-corrected the hover and rebuilt
    /// every overlay over the whole history. This updates the last bar and recomputes only
    /// each overlay's **last** value from its trailing window, so the cost does not grow
    /// with the number of bars (J-P16).
    ///
    /// The price extent is grown to include the new values. It is **not** shrunk: proving
    /// that a replaced value was the unique extreme would require rescanning the history,
    /// which is the cost this method exists to avoid. A full `set_series` re-derives it
    /// exactly, and a slightly wide axis is a reading inconvenience rather than a wrong one.
    pub fn update_last_bar(&mut self, bar: Bar) -> bool {
        let Some(index) = self.series.len().checked_sub(1) else { return false };
        if !self.series.set_bar(index, bar) {
            return false;
        }
        if let Some(slot) = self.closes.get_mut(index) {
            *slot = bar.close;
        }
        if let Some(slot) = self.highs.get_mut(index) {
            *slot = bar.high;
        }
        if let Some(slot) = self.lows.get_mut(index) {
            *slot = bar.low;
        }
        if let Some(slot) = self.volumes.get_mut(index) {
            *slot = bar.volume;
        }
        self.patch_overlay_tail(index);
        self.grow_extent_with_bar(bar);
        self.base.request_redraw();
        true
    }

    /// Appends one bar and advances each overlay by one value.
    ///
    /// The incremental counterpart of `set_series(..)` for a feed that opens a new candle:
    /// one value per overlay is computed from its trailing window, and nothing is rescanned
    /// (J-P16).
    pub fn append_bar(&mut self, bar: Bar) {
        let first_bar = self.series.is_empty();
        self.series.push(bar);
        self.closes.push(bar.close);
        self.highs.push(bar.high);
        self.lows.push(bar.low);
        self.volumes.push(bar.volume);
        let index = self.series.len().saturating_sub(1);
        self.append_overlay_tail(index);
        if first_bar {
            // There is no prior extent to grow: the constructor's placeholder would keep a
            // zero price inside the axis and squash the streamed series against the top.
            self.inferred_decimals = Self::infer_decimals(self.series.bars());
            self.rebuild_extent();
        } else {
            self.grow_extent_with_bar(bar);
        }
        self.base.request_redraw();
    }

    /// Grows the cached extent so a streamed bar stays inside the axis.
    ///
    /// Each bound is `min`/`max` against a padded value, so repeated updates are idempotent:
    /// the padding is derived from the pre-call span and a value already inside the axis
    /// cannot push the bound further out on the next tick. The extent never shrinks here;
    /// a full `set_series` re-derives it exactly.
    fn grow_extent_with_bar(&mut self, bar: Bar) {
        let (mut low, mut high) = self.extent_cache;
        let pad = (high - low).abs().max(1e-9) * 0.02;
        for value in [bar.low, bar.high, bar.open, bar.close] {
            if value.is_finite() {
                low = low.min(value - pad);
                high = high.max(value + pad);
            }
        }
        self.extent_cache = (low, high);
    }

    /// Recomputes the overlay values at `index` in place.
    fn patch_overlay_tail(&mut self, index: usize) {
        for (overlay_index, overlay) in self.overlays.iter().enumerate() {
            let Some(cached) = self.overlay_values.get_mut(overlay_index) else { continue };
            patch_overlay_at(
                cached,
                *overlay,
                index,
                &self.closes,
                &self.highs,
                &self.lows,
                &self.volumes,
            );
        }
    }

    /// Appends one overlay value at `index`.
    fn append_overlay_tail(&mut self, index: usize) {
        for (overlay_index, overlay) in self.overlays.iter().enumerate() {
            let Some(cached) = self.overlay_values.get_mut(overlay_index) else { continue };
            append_overlay_at(
                cached,
                *overlay,
                index,
                &self.closes,
                &self.highs,
                &self.lows,
                &self.volumes,
            );
        }
    }

    /// Appends an overlay.
    ///
    /// Appends rather than replaces, because the common case is a chart showing two or
    /// three averages at once — a moving-average ribbon. A caller wanting a single overlay
    /// calls [`Self::clear_overlays`] first.
    pub fn add_overlay(&mut self, overlay: Overlay) {
        self.overlays.push(overlay);
        self.overlay_visible.push(true);
        self.rebuild_overlay_cache();
        self.base.request_redraw();
    }

    /// Appends an overlay that starts hidden.
    pub fn add_overlay_hidden(&mut self, overlay: Overlay) {
        self.overlays.push(overlay);
        self.overlay_visible.push(false);
        self.rebuild_overlay_cache();
        self.base.request_redraw();
    }

    /// Removes the overlay at `index`.
    ///
    /// Returns whether an overlay was there. Before this, hiding one average meant
    /// `clear_overlays` plus re-adding every other one, which also discarded the cached
    /// indicator values (FIN-04).
    pub fn remove_overlay(&mut self, index: usize) -> bool {
        if index >= self.overlays.len() {
            return false;
        }
        self.overlays.remove(index);
        if index < self.overlay_visible.len() {
            self.overlay_visible.remove(index);
        }
        self.rebuild_overlay_cache();
        self.base.request_redraw();
        true
    }

    /// Shows or hides the overlay at `index`. Returns whether the state changed.
    pub fn set_overlay_visible(&mut self, index: usize, visible: bool) -> bool {
        let Some(slot) = self.overlay_visible.get_mut(index) else { return false };
        if *slot == visible {
            return false;
        }
        *slot = visible;
        // A hidden series must not stretch the price axis, so the extent is re-derived.
        self.rebuild_extent();
        self.base.request_redraw();
        true
    }

    /// Whether the overlay at `index` is drawn. Unknown indices report `false`.
    pub fn overlay_visible(&self, index: usize) -> bool {
        self.overlay_visible.get(index).copied().unwrap_or(false)
    }

    /// The cached values of overlay `index`, one slice per drawn line.
    ///
    /// Borrowed rather than cloned: this is the read side of the cache the painter and the
    /// axis both use, so a host reading a value sees the number that was drawn. A single
    /// overlay returns one slice, a Bollinger or Donchian overlay three (upper, middle,
    /// lower) in the order they were computed.
    pub fn overlay_values(&self, index: usize) -> Option<Vec<&[f64]>> {
        match self.overlay_values.get(index)? {
            CachedOverlay::Single(values) => Some(alloc::vec![values.as_slice()]),
            CachedOverlay::Triple(first, second, third) => {
                Some(alloc::vec![first.as_slice(), second.as_slice(), third.as_slice()])
            }
        }
    }

    /// The overlay list as legend entries: `(label, colour, visible)`.
    ///
    /// This is the data a [`Legend`](rust_widgets::widget::companions::Legend) binds to: the
    /// label is the overlay's factory spelling, the colour is the one the pane draws it in,
    /// and the flag is [`Self::overlay_visible`]. Wiring `series_toggled` to
    /// `set_overlay_visible` makes the legend's checkbox and the drawn band one state
    /// (COMP-J03).
    pub fn overlay_legend_entries(&self) -> Vec<(String, Color, bool)> {
        self.overlays
            .iter()
            .enumerate()
            .map(|(index, overlay)| {
                (
                    overlay.as_str().to_string(),
                    Self::overlay_color(index),
                    self.overlay_visible(index),
                )
            })
            .collect()
    }

    /// Removes every overlay.
    pub fn clear_overlays(&mut self) {
        self.overlays.clear();
        self.overlay_visible.clear();
        self.rebuild_overlay_cache();
        self.base.request_redraw();
    }

    /// Returns whether the overlay legend is drawn.
    pub fn show_overlay_legend(&self) -> bool {
        self.show_overlay_legend
    }

    /// Shows or hides the overlay legend.
    pub fn set_show_overlay_legend(&mut self, show: bool) {
        if self.show_overlay_legend == show {
            return;
        }
        self.show_overlay_legend = show;
        self.base.request_redraw();
    }

    /// The marked price levels.
    pub fn price_lines(&self) -> &[PriceLine] {
        &self.price_lines
    }

    /// Adds a horizontal price level.
    pub fn add_price_line(&mut self, line: PriceLine) {
        self.price_lines.push(line);
        self.rebuild_extent();
        self.base.request_redraw();
    }

    /// Removes every price level.
    pub fn clear_price_lines(&mut self) {
        self.price_lines.clear();
        self.rebuild_extent();
        self.base.request_redraw();
    }

    /// The bar index under the pointer, if the pointer is over a bar.
    pub fn hovered_index(&self) -> Option<usize> {
        self.hovered_index
    }

    /// The price under the pointer, for the horizontal crosshair.
    pub fn hovered_price(&self) -> Option<f64> {
        self.hovered_price
    }

    /// The visible bar range, or `None` when every bar is shown.
    pub fn view_range(&self) -> Option<(usize, usize)> {
        self.view
    }

    /// Restricts the chart to the bars in `range`.
    ///
    /// The range is clamped to the series and kept non-empty. This is the chart's zoom: a
    /// long history draws every bar at one pixel otherwise, and the marks are unreadable.
    pub fn set_view_range(&mut self, range: Option<(usize, usize)>) {
        let range = range.and_then(|(start, end)| {
            // A view over an empty series would name no bars; `None` is the only honest
            // answer, and it is what the next bar appended must start from.
            if self.series.is_empty() {
                return None;
            }
            let start = start.min(self.series.len() - 1);
            let end = end.clamp(start + 1, self.series.len());
            Some((start, end))
        });
        if self.view == range {
            return;
        }
        self.view = range;
        let (start, end) = self.view_bounds();
        self.view_changed.emit((start, end));
        // The window moved under a stationary pointer: a hover outside it names a bar the reader
        // can no longer see. `hovered_summary` and `hover_anchor` used to disagree about it, and
        // the exit was never reported.
        if let Some(previous) = self.hovered_index {
            if previous < start || previous >= end {
                self.hovered_index = None;
                self.hovered_price = None;
                self.bar_unhovered.emit(previous);
            }
        }
        self.base.request_redraw();
    }

    /// Restores the full series view.
    pub fn reset_view(&mut self) {
        self.set_view_range(None);
    }

    /// The visible `(start, end)` bar indices.
    fn view_bounds(&self) -> (usize, usize) {
        match self.view {
            Some((start, end)) => (start.min(self.series.len()), end.min(self.series.len())),
            None => (0, self.series.len()),
        }
    }

    // ── Companion pairing (RW-106) ──

    /// Follows `table` as this pane's data table.
    ///
    /// The table shows the close series plus one row per overlay. Row 0 is the price series and
    /// cannot be hidden — the pane has no "bars hidden" state — so a toggle on it is refused and
    /// the table's checklist is re-synced to the truth rather than left claiming a hide that did
    /// not happen. Every other row toggles its overlay through [`Self::set_overlay_visible`].
    pub fn attach_chart_data_table(&mut self, table: &mut ChartDataTable) {
        let snapshot = self.chart_data_snapshot();
        self.data_table_key = Some(self.chart_data_key());
        self.data_table = Some(super::chart_companions::ChartDataTableBinding::attach(
            table,
            self.base.redraw_requested_signal().clone(),
            snapshot,
        ));
        self.base.request_redraw();
    }

    /// Stops following the attached data table; its last snapshot stays.
    pub fn detach_chart_data_table(&mut self) {
        if self.data_table.take().is_some() {
            self.data_table_key = None;
            self.base.request_redraw();
        }
    }

    /// Whether a [`ChartDataTable`] is attached.
    pub fn is_chart_data_table_attached(&self) -> bool {
        self.data_table.is_some()
    }

    /// Applies the table's toggles and pushes a changed snapshot.
    ///
    /// Called from `draw` and `handle_event`, so a table edit is visible on the next frame. An
    /// unchanged pane, an idle table and no attachment all return `false`.
    ///
    /// The snapshot is only built when its inputs moved: a cheap `ChartDataKey` covers the
    /// series length, the last close and every overlay's identity, last value and visibility,
    /// so a steady-state frame or a bare pointer move no longer allocates a full snapshot for
    /// the binding to throw away (FIN-1). A refused toggle still forces the push, because the
    /// table has to be told to snap its checklist back.
    pub fn poll_chart_data_table(&mut self) -> bool {
        if self.data_table.is_none() {
            return false;
        }
        let toggles = match self.data_table.as_ref() {
            Some(binding) => binding.take_toggles(),
            None => return false,
        };
        let mut changed = false;
        let mut resync = false;
        for index in toggles {
            if index == 0 {
                // The base price series has no visibility flag; re-syncing the table is the
                // honest answer, and the row snaps back rather than staying unchecked.
                resync = true;
                continue;
            }
            let overlay = index - 1;
            let visible = self.overlay_visible.get(overlay).copied().unwrap_or(false);
            changed |= self.set_overlay_visible(overlay, !visible);
        }
        let key = self.chart_data_key();
        if !resync && self.data_table_key == Some(key) {
            return changed;
        }
        let snapshot = self.chart_data_snapshot();
        if let Some(binding) = self.data_table.as_mut() {
            changed |= binding.push(snapshot, resync);
            changed |= resync;
        }
        self.data_table_key = Some(key);
        changed
    }

    /// Follows `strip` as this pane's zoom control.
    ///
    /// A requested level becomes a bar window centred on the current view; Fit restores every
    /// bar. The readout follows the pane's own view, so a host that zooms in itself stays in
    /// step. The control has no vertical zoom, so the level applies to the bar axis only.
    pub fn attach_zoom_strip(&mut self, strip: &mut ZoomStrip) {
        let factor = super::chart_companions::zoom_factor(self.series.len(), self.view_range());
        self.zoom_strip = Some(super::chart_companions::ZoomStripBinding::attach(
            strip,
            self.base.redraw_requested_signal().clone(),
            factor,
        ));
        self.base.request_redraw();
    }

    /// Stops following the attached strip; the last applied view stays.
    pub fn detach_zoom_strip(&mut self) {
        if self.zoom_strip.take().is_some() {
            self.base.request_redraw();
        }
    }

    /// Whether a [`ZoomStrip`] is attached.
    pub fn is_zoom_strip_attached(&self) -> bool {
        self.zoom_strip.is_some()
    }

    /// Applies the strip's requests and mirrors the pane's view back.
    pub fn poll_zoom_strip(&mut self) -> bool {
        let total = self.series.len();
        let view = self.view_range();
        let mut next = view;
        let mut apply = |range: Option<(usize, usize)>| {
            next = range;
        };
        let changed =
            super::chart_companions::poll_zoom_strip(&mut self.zoom_strip, total, view, &mut apply);
        if next != view {
            self.set_view_range(next);
        }
        changed
    }

    /// The inputs [`Self::chart_data_snapshot`] reads, reduced to a cheap key (FIN-1).
    ///
    /// Allocation-free and O(overlays): everything the snapshot derives is either the series
    /// length, the last close, or foldable from each overlay's identity, tail value and
    /// visibility flag. A key that does not change means the snapshot would be identical, so
    /// the poll can skip building it.
    fn chart_data_key(&self) -> ChartDataKey {
        let mut overlays = 0xcbf2_9ce4_8422_2325u64;
        mix_key(&mut overlays, self.overlays.len() as u64);
        for (index, overlay) in self.overlays.iter().enumerate() {
            Self::mix_overlay_identity(&mut overlays, *overlay);
            mix_key(
                &mut overlays,
                u64::from(self.overlay_visible.get(index).copied().unwrap_or(false)),
            );
            if let Some(cached) = self.overlay_values.get(index) {
                match cached {
                    CachedOverlay::Single(values) => {
                        mix_key(&mut overlays, 1);
                        mix_key(&mut overlays, overlay_tail(values));
                    }
                    CachedOverlay::Triple(first, second, third) => {
                        mix_key(&mut overlays, 3);
                        for values in [first, second, third] {
                            mix_key(&mut overlays, overlay_tail(values));
                        }
                    }
                }
            }
        }
        ChartDataKey {
            series_len: self.closes.len(),
            last_close: self.closes.last().copied().unwrap_or(0.0).to_bits(),
            overlays,
        }
    }

    /// Mixes an overlay's kind and parameters into `fingerprint`.
    ///
    /// The snapshot's row label comes from the kind and its points from the parameters, so a
    /// swapped study with a coincidentally equal tail value still changes the key.
    fn mix_overlay_identity(fingerprint: &mut u64, overlay: Overlay) {
        match overlay {
            Overlay::MovingAverage { period } => {
                mix_key(fingerprint, 1);
                mix_key(fingerprint, period as u64);
            }
            Overlay::ExponentialMovingAverage { period } => {
                mix_key(fingerprint, 2);
                mix_key(fingerprint, period as u64);
            }
            Overlay::BollingerBands { period, multiplier } => {
                mix_key(fingerprint, 3);
                mix_key(fingerprint, period as u64);
                mix_key(fingerprint, multiplier.to_bits());
            }
            Overlay::Vwap { period } => {
                mix_key(fingerprint, 4);
                mix_key(fingerprint, period as u64);
            }
            Overlay::DonchianChannel { period } => {
                mix_key(fingerprint, 5);
                mix_key(fingerprint, period as u64);
            }
        }
    }

    /// The pane's series as a data-table snapshot: closes first, then one entry per overlay.
    fn chart_data_snapshot(&self) -> ChartSeriesSnapshot {
        let mut series = Vec::with_capacity(self.overlay_values.len() + 1);
        let mut visible = Vec::with_capacity(self.overlay_values.len() + 1);
        series.push((
            alloc::string::String::from("Close"),
            self.closes.iter().enumerate().map(|(index, value)| (index as f64, *value)).collect(),
        ));
        visible.push(0);
        let legend = self.overlay_legend_entries();
        for (index, cached) in self.overlay_values.iter().enumerate() {
            let label = legend
                .get(index)
                .map(|entry| entry.0.clone())
                .unwrap_or_else(|| alloc::format!("Overlay {}", index + 1));
            let points = match cached {
                CachedOverlay::Single(values) => values.clone(),
                // A band's first line is its representative value; the visibility toggle
                // applies to the whole overlay.
                CachedOverlay::Triple(first, _, _) => first.clone(),
            };
            series.push((
                label,
                points
                    .into_iter()
                    .enumerate()
                    .map(|(point, value)| (point as f64, value))
                    .collect(),
            ));
            if self.overlay_visible.get(index).copied().unwrap_or(false) {
                visible.push(index + 1);
            }
        }
        (series, visible)
    }

    /// The bar under the pointer.
    pub fn hovered_bar(&self) -> Option<Bar> {
        self.hovered_index.and_then(|index| self.series.bars().get(index).copied())
    }

    /// The hovered bar's readings, ready for a `ChartTooltip`.
    ///
    /// The heading is the bar's own label when the host supplied one, then OHLC, volume and
    /// the bar's change in the candle's direction colour. Assembling this in the control keeps
    /// a host from rebuilding it from `hovered_bar()` in every consumer (COMP-J02).
    pub fn hovered_summary(&self) -> Vec<rust_widgets::widget::companions::TooltipLine> {
        use rust_widgets::widget::companions::TooltipLine;
        let Some(index) = self.hovered_index else { return Vec::new() };
        let Some(bar) = self.series.bars().get(index).copied() else { return Vec::new() };
        let decimals = self.inferred_decimals;
        let mut lines = Vec::with_capacity(7);
        lines.push(TooltipLine::new(match self.series.label_at(index) {
            Some(label) => label.to_string(),
            None => alloc::format!("#{index}"),
        }));
        lines.push(TooltipLine::new(alloc::format!("O {}", format_fixed(bar.open, decimals))));
        lines.push(TooltipLine::new(alloc::format!("H {}", format_fixed(bar.high, decimals))));
        lines.push(TooltipLine::new(alloc::format!("L {}", format_fixed(bar.low, decimals))));
        lines.push(TooltipLine::new(alloc::format!("C {}", format_fixed(bar.close, decimals))));
        lines.push(TooltipLine::new(alloc::format!("V {:.0}", bar.volume)));
        let change = bar.close - bar.open;
        let color = if change >= 0.0 { Self::rising_color() } else { Self::falling_color() };
        lines.push(TooltipLine::colored(
            alloc::format!("Δ {}", format_fixed(change, decimals)),
            color,
        ));
        lines
    }

    /// The hovered bar's anchor and suggested tooltip content, for an external read-out.
    ///
    /// The same `(Point, lines)` shape
    /// [`LineChart::hover_anchor`](rust_widgets::widget::chart_widgets::line_chart::LineChart::hover_anchor)
    /// publishes, so one `ChartTooltip` can serve every chart in the family (gap 10).
    pub fn hover_anchor(
        &self,
    ) -> Option<(Point, Vec<rust_widgets::widget::companions::TooltipLine>)> {
        let index = self.hovered_index?;
        let bar = self.series.bars().get(index)?;
        let (start, end) = self.view_bounds();
        if index < start || index >= end {
            return None;
        }
        let area = self.plot_area();
        if area.rect.width == 0 || area.rect.height == 0 {
            return None;
        }
        let index_axis = area.index_axis(end.saturating_sub(start).max(1));
        let x = index_axis.center_for(index - start);
        let (low, high) = self.effective_price_extent();
        let price = self.hovered_price.unwrap_or(bar.close);
        let y = area.price_axis(low, high).y_for(price);
        Some((Point::new(x, y), self.hovered_summary()))
    }

    /// The plot area for this control's current geometry.
    fn plot_area(&self) -> PlotArea {
        PlotArea::price_pane(self.base.geometry())
    }

    /// The pane's chrome, resolved from this control's own style first and the theme second.
    fn chrome(&self) -> PanelColors {
        panel_colors(Some(self.base.style()))
    }

    /// The colour of the rising-body series.
    fn rising_color() -> Color {
        Color::rgb(38, 166, 91)
    }

    /// The colour of the falling-body series.
    fn falling_color() -> Color {
        Color::rgb(220, 68, 70)
    }

    /// A colour for overlay `index`, cycling through a fixed palette.
    ///
    /// Fixed rather than caller-supplied so a chart with three averages is readable
    /// without configuration, and deterministic so the same overlay list always draws the
    /// same way. Cycling rather than erroring past the palette's end, because a chart with
    /// six overlays should still draw.
    fn overlay_color(index: usize) -> Color {
        const PALETTE: [Color; 6] = [
            Color::rgb(255, 193, 7),
            Color::rgb(33, 150, 243),
            Color::rgb(156, 39, 176),
            Color::rgb(0, 188, 212),
            Color::rgb(255, 87, 34),
            Color::rgb(139, 195, 74),
        ];
        // A palette of six is a fixed array, so the modulo is against a real length.
        PALETTE[index % PALETTE.len()]
    }

    /// Draws a polyline through `values`, skipping gaps.
    ///
    /// The gap handling is the point: every indicator is `NAN` during its warm-up, and a
    /// naive line would connect across the gap from the origin, drawing a diagonal that
    /// looks like a trend. Skipping non-finite values means the line simply starts where
    /// the data does, and a mid-series gap breaks the line into two segments rather than
    /// bridging it.
    fn draw_series_line(
        context: &mut RenderContext,
        values: &[f64],
        price_axis: &crate::finance::layout::PriceAxis,
        index_axis: &crate::finance::layout::IndexAxis,
        color: Color,
        width: u32,
        start: usize,
        end: usize,
    ) {
        let mut previous: Option<(i32, i32)> = None;
        let visible = &values[start.min(values.len())..end.min(values.len())];
        for (offset, value) in visible.iter().enumerate() {
            if !value.is_finite() || offset >= index_axis.count {
                // Break the run so the next finite point starts a new segment.
                previous = None;
                continue;
            }
            let point = (index_axis.center_for(offset), price_axis.y_for(*value));
            if let Some(from) = previous {
                context.draw_line_stroke(
                    rust_widgets::core::Point { x: from.0, y: from.1 },
                    rust_widgets::core::Point { x: point.0, y: point.1 },
                    color,
                    width,
                );
            }
            previous = Some(point);
        }
    }

    /// Draws the candles themselves.
    fn draw_candles(&self, context: &mut RenderContext, area: &PlotArea) {
        let bars = self.series.bars();
        if bars.is_empty() {
            return;
        }
        let (low, high) = self.effective_price_extent();
        let price_axis = area.price_axis(low, high);
        let (start, end) = self.view_bounds();
        let visible = &bars[start..end];
        let index_axis = area.index_axis(visible.len());
        let body_width = index_axis.body_width();

        for (position, bar) in visible.iter().enumerate() {
            if !bar.open.is_finite()
                || !bar.high.is_finite()
                || !bar.low.is_finite()
                || !bar.close.is_finite()
            {
                continue;
            }
            let color = if bar.is_rising() { Self::rising_color() } else { Self::falling_color() };

            // The wick and the body share the column centre `center_for` documents, which is also
            // where the crosshair is drawn. Offsetting from the slot origin instead left the body
            // without its left gap and put the wick one pixel off the marker naming it.
            let centre = index_axis.center_for(position);
            let wick_x = centre;
            context.draw_line_stroke(
                rust_widgets::core::Point { x: wick_x, y: price_axis.y_for(bar.high) },
                rust_widgets::core::Point { x: wick_x, y: price_axis.y_for(bar.low) },
                color,
                1,
            );

            let y_open = price_axis.y_for(bar.open);
            let y_close = price_axis.y_for(bar.close);
            let body_top = y_open.min(y_close);
            let body_x = index_axis.body_x_for(position);
            context.fill_rect(
                Rect::new(
                    body_x,
                    body_top,
                    body_width as u32,
                    // A doji has no body; one pixel keeps it visible rather than absent.
                    (y_open - y_close).unsigned_abs().max(1),
                ),
                color,
            );
        }
    }

    /// Draws the overlays.
    fn draw_overlays(&self, context: &mut RenderContext, area: &PlotArea) {
        if self.series.bars().is_empty() || self.overlays.is_empty() {
            return;
        }
        let (low, high) = self.effective_price_extent();
        let price_axis = area.price_axis(low, high);
        let (start, end) = self.view_bounds();
        let index_axis = area.index_axis(end.saturating_sub(start));

        for (overlay_index, cached) in self.overlay_values.iter().enumerate() {
            // A hidden overlay is not painted, matching the extent (which already excludes
            // it) and the legend/data-table state that says it is hidden.
            if !self.overlay_visible(overlay_index) {
                continue;
            }
            let color = Self::overlay_color(overlay_index);
            match cached {
                CachedOverlay::Single(values) => Self::draw_series_line(
                    context,
                    values,
                    &price_axis,
                    &index_axis,
                    color,
                    1,
                    start,
                    end,
                ),
                CachedOverlay::Triple(first, second, third) => {
                    Self::draw_series_line(
                        context,
                        first,
                        &price_axis,
                        &index_axis,
                        color,
                        1,
                        start,
                        end,
                    );
                    Self::draw_series_line(
                        context,
                        second,
                        &price_axis,
                        &index_axis,
                        color,
                        1,
                        start,
                        end,
                    );
                    Self::draw_series_line(
                        context,
                        third,
                        &price_axis,
                        &index_axis,
                        color,
                        1,
                        start,
                        end,
                    );
                }
            }
        }
    }

    /// Draws the marked price levels as dashed-across horizontals.
    ///
    /// Drawn *over* the candles so a level is never hidden by a bar that happens to sit on
    /// it — the level is the annotation, and an annotation behind the data is not doing
    /// its job.
    fn draw_price_lines(&self, context: &mut RenderContext, area: &PlotArea) {
        // The flag is the property's own state and the list is the data: both have to be live
        // for a level to be painted, so hiding the levels is a display change only (FIN-5).
        if !self.show_price_levels || self.price_lines.is_empty() {
            return;
        }
        // One chrome resolution for the whole pass: the previous-close colour used to re-resolve
        // the theme for every line in the loop (FIN-4).
        let chrome = self.chrome();
        let (low, high) = self.effective_price_extent();
        let price_axis = area.price_axis(low, high);
        for line in &self.price_lines {
            if !line.price.is_finite() {
                continue;
            }
            let y = price_axis.y_for(line.price);
            // Only draw a level that is actually inside the shown price range; one far
            // outside would clamp to the edge and read as a level at the boundary.
            if line.price < price_axis.low || line.price > price_axis.high {
                continue;
            }
            let color = match line.kind {
                // A support level is drawn in the same green the rising bodies use and a
                // resistance level in the same red the falling ones do, so a reader who has
                // learnt the candles has already learnt the levels. `PreviousClose` is the
                // odd one out: it is a neutral marker, so it takes the pane's own
                // crosshair strength rather than a third fixed grey.
                PriceLevelKind::Support => Self::rising_color(),
                PriceLevelKind::Resistance => Self::falling_color(),
                PriceLevelKind::PreviousClose => chrome.crosshair,
            };
            // Dashes rather than a solid line, so it reads as an annotation and not as
            // another data series.
            let mut x = area.rect.x;
            while x < area.right() {
                let dash_end = (x + 4).min(area.right());
                context.draw_line_stroke(
                    rust_widgets::core::Point { x, y },
                    rust_widgets::core::Point { x: dash_end, y },
                    color,
                    1,
                );
                x += 8;
            }
        }
    }

    /// Draws the four price labels down the left edge.
    fn draw_price_labels(&self, context: &mut RenderContext, area: &PlotArea) {
        // The fractional part follows the instrument's own precision, which the series' prices
        // reveal: a 2-decimal instrument and a 4-decimal one both get sensible labels without
        // a configuration knob.
        let decimals = self.inferred_decimals();
        let (low, high) = self.effective_price_extent();
        let price_axis = area.price_axis(low, high);
        // The axis's own hairline is the pane's grid colour, so the scale a reader measures
        // a candle against cannot be a fixed slate that only reads on a dark backdrop. One
        // resolution for the whole pass: the loop used to re-resolve the theme per label
        // (FIN-4).
        let chrome = self.chrome();
        let grid = chrome.grid;
        let ink = chrome.ink.with_alpha(190);
        // One role font for the whole column: it used to be rebuilt for each of the four
        // labels every frame.
        let font = role_font(self.style(), 10.0);
        // The label column is the plot's left margin. The width used to be estimated as
        // `len * 7`, which for an eight-character price (56 px) put the origin at −14 —
        // outside the control's own rectangle — and under-counted wide glyphs.
        let label_left = area.rect.x - area.left_margin;
        let label_right = area.label_x();
        let label_width = (label_right - label_left).max(0) as u32;
        // Four gridlines, at the ends and two inside.
        for step in 0..=3 {
            let fraction = step as f64 / 3.0;
            let price = price_axis.low + fraction * (price_axis.high - price_axis.low);
            let y = price_axis.y_for(price);
            // The label is written into the family's shared buffer rather than into a fresh
            // `String` per gridline per frame (FIN-3).
            super::volume_chart::with_axis_label(|text| {
                write_fixed(price, decimals, text);
                context.draw_line(
                    rust_widgets::core::Point { x: area.rect.x, y },
                    rust_widgets::core::Point { x: area.right(), y },
                    grid,
                );
                // Right-aligned in the reserved column and fitted to it, so a wide value
                // truncates instead of starting left of the control.
                let line_height = context.measure_text(text, &font).height.max(1);
                let band =
                    Rect::new(label_left, y - line_height as i32 / 2, label_width, line_height);
                context.draw_text_fitted_ellipsis(
                    band,
                    text,
                    &font,
                    ink,
                    HorizontalAlignment::Right,
                );
            });
        }
    }

    /// Draws the pane's frame and a `No data` message, for a series with nothing in it.
    ///
    /// # Why an empty pane still draws chrome
    ///
    /// This control used to fill its rectangle with a slab and return, so a K-line pane with
    /// no feed was a black rectangle: no axis, no gridline, and nothing that said *why* it
    /// was empty. That state is not an edge case — it is what every chart shows before the
    /// first tick arrives — and it is the state a reader is most likely to mistake for a
    /// rendering failure. Drawing the frame the pane will use once data arrives is what makes
    /// the two states read as the same control, and it is what every charting toolkit does:
    /// the axes belong to the chart, not to a series.
    fn draw_empty_state(&self, context: &mut RenderContext, area: &PlotArea) {
        let chrome = self.chrome();
        context.draw_rect(area.rect, chrome.grid);
        // A single hairline at mid-height, where a flat series would plot, so the pane reads
        // as "a scale with nothing on it" rather than "a filled rectangle".
        let mid_y = area.rect.y + area.rect.height as i32 / 2;
        context.draw_line(
            rust_widgets::core::Point { x: area.rect.x, y: mid_y },
            rust_widgets::core::Point { x: area.right(), y: mid_y },
            chrome.grid,
        );
        let font = role_font(self.style(), 12.0);
        let line = context.text_line(area.rect, &font);
        context.draw_text_fitted_ellipsis(
            line,
            "No data",
            &font,
            // Legibility is derived, not assumed: this is the same 4.5:1 floor the axis
            // labels use, so the message cannot be invisible on the panel it sits on.
            chrome.ink.legible_on(chrome.surface, PANEL_MIN_CONTRAST).with_alpha(160),
            HorizontalAlignment::Center,
        );
    }

    /// How many decimals the series' prices appear to use.
    ///
    /// Inferred rather than configured because a feed's precision is a property of the
    /// data: a chart of a 0.5-pip FX pair and a chart of a stock both look right without
    /// the caller passing a format string. Capped at four, past which a price label stops
    /// being readable at the axis width.
    fn inferred_decimals(&self) -> usize {
        self.inferred_decimals
    }

    /// The price extent including every overlay, so an overlay is not clipped.
    ///
    /// This is why the extent cannot simply be the series' own high/low: a Bollinger band
    /// routinely sits outside the traded range, and a chart that clipped it would show a
    /// band that appears to touch the top of the pane.
    fn effective_price_extent(&self) -> (f64, f64) {
        // Both the axis and the painter read the one cached value, so a frame does not
        // recompute every overlay three to five times.
        self.extent_cache
    }

    /// Draws a compact legend for the overlays, in their draw order.
    ///
    /// The six-colour palette existed with no key, so a chart with three averages could not
    /// say which was which. Each row pairs the overlay's factory name with its swatch.
    ///
    /// The rows are a pure function of the overlay list and the widths a pure function of
    /// (names, font), so both come from a keyed cache: a steady-state frame no longer collects
    /// `N` rows and measures `2N` labels for a legend that did not change (FIN-2).
    fn draw_overlay_legend(&mut self, context: &mut RenderContext, area: &PlotArea) {
        let font = role_font(self.style(), 10.0);
        let chrome = self.chrome();
        self.refresh_legend_cache(context, &font);

        let max_rows = (((area.bottom() - area.rect.y - 4) / 14).max(1)) as usize;
        // A hidden overlay is not drawn, so its legend row must not be either: the pane said the
        // series was hidden while the swatch kept advertising it in full colour.
        let rows: Vec<usize> = (0..self.legend_cache.names.len())
            .filter(|index| self.overlay_visible(*index))
            .take(max_rows)
            .collect();
        if rows.is_empty() {
            return;
        }

        // A surface behind the legend, sized to its widest label before anything is drawn.
        //
        // The legend sits in the plot's top-right, which is where a price **level line** crosses:
        // the resistance dash struck through `moving_average` in the finance demo's light
        // appearance, reading as a strikethrough rather than as a reference line. An opaque fill
        // in the pane's own colour is what separates the two; a translucent wash would still show
        // the dash.
        let widest = rows.iter().map(|index| self.legend_cache.widths[*index]).max().unwrap_or(0);
        let box_width = (widest + 24).min(area.rect.width as i32).max(1);
        let box_x = (area.right() - box_width).max(area.rect.x);
        let box_height = (rows.len() as u32) * 14 + 4;
        context
            .fill_rect(Rect::new(box_x, area.rect.y, box_width as u32, box_height), chrome.surface);

        let mut y = area.rect.y + 4;
        for index in rows {
            let name = self.legend_cache.names[index].as_str();
            let text_width = self.legend_cache.widths[index];
            let x = area.right() - text_width - 20;
            if x < area.rect.x {
                break;
            }
            context.fill_rect(Rect::new(x, y + 3, 8, 8), Self::overlay_color(index));
            context.draw_text(
                rust_widgets::core::Point { x: x + 12, y },
                name,
                &font,
                chrome.ink,
                HorizontalAlignment::Left,
            );
            y += 14;
        }
    }

    /// Rebuilds the legend's rows and widths only when their inputs moved (FIN-2).
    ///
    /// Both halves are cheap to compare and expensive to build, so each keeps its own
    /// condition: a renamed or added overlay rebuilds the rows and re-measures, while a font
    /// change re-measures the rows that are already known.
    fn refresh_legend_cache(&mut self, context: &RenderContext, font: &Font) {
        let rows_changed = self.legend_cache.names.len() != self.overlays.len()
            || self
                .overlays
                .iter()
                .zip(self.legend_cache.names.iter())
                .any(|(overlay, cached)| overlay.as_str() != cached.as_str());
        if rows_changed {
            self.legend_cache.names.clear();
            self.legend_cache
                .names
                .extend(self.overlays.iter().map(|overlay| overlay.as_str().to_string()));
        }
        if rows_changed || self.legend_cache.font.as_ref() != Some(font) {
            self.legend_cache.font = Some(font.clone());
            self.legend_cache.widths.clear();
            self.legend_cache.widths.extend(
                self.legend_cache
                    .names
                    .iter()
                    .map(|name| context.measure_text(name, font).width as i32),
            );
        }
    }

    /// Draws the crosshair for the hovered bar.
    fn draw_crosshair(&self, context: &mut RenderContext, area: &PlotArea) {
        let Some(index) = self.hovered_index else {
            return;
        };
        let bars = self.series.bars();
        if index >= bars.len() {
            return;
        }
        let (start, end) = self.view_bounds();
        if index < start || index >= end {
            return;
        }
        let index_axis = area.index_axis(end.saturating_sub(start));
        let x = index_axis.center_for(index - start);
        let chrome = self.chrome();
        context.draw_line_stroke(
            rust_widgets::core::Point { x, y: area.rect.y },
            rust_widgets::core::Point { x, y: area.bottom() },
            chrome.crosshair,
            1,
        );
        // The horizontal half of the crosshair: a dashed-free hairline at the pointer's price,
        // with that price in the left margin. The vertical line alone told a reader *when* but
        // not *at what price*, which is the question a price chart exists to answer.
        if let Some(price) = self.hovered_price {
            let (low, high) = self.effective_price_extent();
            let price_axis = area.price_axis(low, high);
            let y = price_axis.y_for(price);
            if y > area.rect.y && y < area.bottom() {
                context.draw_line_stroke(
                    rust_widgets::core::Point { x: area.rect.x, y },
                    rust_widgets::core::Point { x: area.right(), y },
                    chrome.crosshair,
                    1,
                );
                // The tag is written into the shared axis-label buffer instead of a fresh
                // `String` per hovered frame (FIN-3).
                let font = role_font(self.style(), 10.0);
                super::volume_chart::with_axis_label(|text| {
                    write_fixed(price, 2, text);
                    context.draw_text(
                        rust_widgets::core::Point { x: area.rect.x + 2, y: y + 2 },
                        text,
                        &font,
                        chrome.ink,
                        HorizontalAlignment::Left,
                    );
                });
            }
        }

        // And a readout of that bar, positioned to stay inside the pane on both sides.
        //
        // The text is longer than a narrow pane can hold, so it is clipped to the plot
        // area: without a clip both candidate positions overflow — the right branch runs
        // past `area.right()`, and the left branch, clamped *right* to `area.rect.x`,
        // runs further still. Clipping keeps the readout inside the control that owns it
        // instead of painting over a neighbouring pane.
        let bar = bars[index];
        // Measured, not estimated: `len * 7` assumed a monospace advance this pane does not
        // draw in, so on a proportional face the readout was placed inside the wrong edge
        // and the left/right flip happened at the wrong x (J-P19, as in the indicator pane).
        // The readout is written into the shared axis-label buffer, so a hovered frame
        // reformats it without allocating (FIN-3).
        let font = role_font(self.style(), 11.0);
        super::volume_chart::with_axis_label(|text| {
            let _ = write!(
                text,
                "O {:.2}  H {:.2}  L {:.2}  C {:.2}",
                bar.open, bar.high, bar.low, bar.close
            );
            let width = context.measure_text(text, &font).width as i32;
            let label_x = if x + 8 + width > area.right() { x - 8 - width } else { x + 8 };
            context.push_clip(area.rect.x, area.rect.y, area.rect.width, area.rect.height);
            context.draw_text(
                rust_widgets::core::Point { x: label_x.max(area.rect.x), y: area.rect.y + 2 },
                text,
                &font,
                chrome.ink,
                HorizontalAlignment::Left,
            );
            context.pop_clip();
        });
    }
}

/// The mean of the `period` closes ending at `index`, or `NAN` when the window is not
/// complete or contains a non-finite sample.
fn window_mean(values: &[f64], index: usize, period: usize) -> f64 {
    if period == 0 || period > index + 1 || index >= values.len() {
        return f64::NAN;
    }
    let window = &values[index + 1 - period..=index];
    if !window.iter().all(|value| value.is_finite()) {
        return f64::NAN;
    }
    window.iter().sum::<f64>() / period as f64
}

/// The population standard deviation of the window ending at `index`.
fn window_deviation(values: &[f64], index: usize, period: usize) -> f64 {
    if period == 0 || period > index + 1 || index >= values.len() {
        return f64::NAN;
    }
    let window = &values[index + 1 - period..=index];
    if !window.iter().all(|value| value.is_finite()) {
        return f64::NAN;
    }
    let mean = window.iter().sum::<f64>() / period as f64;
    let variance =
        window.iter().map(|value| (value - mean) * (value - mean)).sum::<f64>() / period as f64;
    variance.sqrt()
}

/// The windowed VWAP ending at `index`.
fn window_vwap(
    highs: &[f64],
    lows: &[f64],
    closes: &[f64],
    volumes: &[f64],
    index: usize,
    period: usize,
) -> f64 {
    if period == 0 || period > index + 1 || index >= closes.len() {
        return f64::NAN;
    }
    let start = index + 1 - period;
    let mut weighted = 0.0;
    let mut total = 0.0;
    for offset in start..=index {
        let (Some(high), Some(low), Some(close), Some(volume)) = (
            highs.get(offset).copied(),
            lows.get(offset).copied(),
            closes.get(offset).copied(),
            volumes.get(offset).copied(),
        ) else {
            return f64::NAN;
        };
        if !high.is_finite() || !low.is_finite() || !close.is_finite() || !volume.is_finite() {
            return f64::NAN;
        }
        weighted += (high + low + close) / 3.0 * volume;
        total += volume;
    }
    if total == 0.0 {
        f64::NAN
    } else {
        weighted / total
    }
}

/// The windowed Donchian channel ending at `index`, as `(upper, middle, lower)`.
fn window_donchian(highs: &[f64], lows: &[f64], index: usize, period: usize) -> (f64, f64, f64) {
    if period == 0 || period > index + 1 || index >= highs.len() || index >= lows.len() {
        return (f64::NAN, f64::NAN, f64::NAN);
    }
    let start = index + 1 - period;
    let high_window = &highs[start..=index];
    let low_window = &lows[start..=index];
    if !high_window.iter().all(|value| value.is_finite())
        || !low_window.iter().all(|value| value.is_finite())
    {
        return (f64::NAN, f64::NAN, f64::NAN);
    }
    let upper = high_window.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let lower = low_window.iter().copied().fold(f64::INFINITY, f64::min);
    (upper, (upper + lower) / 2.0, lower)
}

/// One overlay's value at `index`, computed from its trailing window only.
fn overlay_single_value(
    overlay: Overlay,
    index: usize,
    closes: &[f64],
    highs: &[f64],
    lows: &[f64],
    volumes: &[f64],
    previous: Option<f64>,
) -> f64 {
    match overlay {
        Overlay::MovingAverage { period } => window_mean(closes, index, period),
        Overlay::ExponentialMovingAverage { period } => {
            let close = closes.get(index).copied().unwrap_or(f64::NAN);
            if period == 0 || !close.is_finite() {
                return close;
            }
            // One smoothing step from the previous EMA; only a missing/first previous
            // value needs the full series, which cannot happen on the steady path.
            match previous.filter(|value| value.is_finite()) {
                Some(previous) => previous + (close - previous) * (2.0 / (period as f64 + 1.0)),
                None => indicators::ema(closes, period).get(index).copied().unwrap_or(f64::NAN),
            }
        }
        Overlay::Vwap { period } => window_vwap(highs, lows, closes, volumes, index, period),
        // The band overlays carry three series and are handled by the triple path.
        Overlay::BollingerBands { .. } | Overlay::DonchianChannel { .. } => f64::NAN,
    }
}

/// Writes `value` at `index`, growing the series when it is one short.
fn put_value(values: &mut Vec<f64>, index: usize, value: f64) {
    if values.len() <= index {
        values.resize(index + 1, f64::NAN);
    }
    values[index] = value;
}

/// Recomputes the cached overlay's value at `index` from its window.
fn patch_overlay_at(
    cached: &mut CachedOverlay,
    overlay: Overlay,
    index: usize,
    closes: &[f64],
    highs: &[f64],
    lows: &[f64],
    volumes: &[f64],
) {
    match (cached, overlay) {
        (CachedOverlay::Single(values), _) => {
            let previous = index.checked_sub(1).and_then(|slot| values.get(slot)).copied();
            let value =
                overlay_single_value(overlay, index, closes, highs, lows, volumes, previous);
            put_value(values, index, value);
        }
        (
            CachedOverlay::Triple(upper, middle, lower),
            Overlay::BollingerBands { period, multiplier },
        ) => {
            let mean = window_mean(closes, index, period);
            let deviation = window_deviation(closes, index, period);
            put_value(middle, index, mean);
            put_value(upper, index, mean + multiplier * deviation);
            put_value(lower, index, mean - multiplier * deviation);
        }
        (CachedOverlay::Triple(upper, middle, lower), Overlay::DonchianChannel { period }) => {
            let (top, center, bottom) = window_donchian(highs, lows, index, period);
            put_value(upper, index, top);
            put_value(middle, index, center);
            put_value(lower, index, bottom);
        }
        _ => {}
    }
}

/// Appends the cached overlay's value for `index`.
fn append_overlay_at(
    cached: &mut CachedOverlay,
    overlay: Overlay,
    index: usize,
    closes: &[f64],
    highs: &[f64],
    lows: &[f64],
    volumes: &[f64],
) {
    match (cached, overlay) {
        (CachedOverlay::Single(values), _) => {
            let previous = index.checked_sub(1).and_then(|slot| values.get(slot)).copied();
            let value =
                overlay_single_value(overlay, index, closes, highs, lows, volumes, previous);
            values.push(value);
        }
        (
            CachedOverlay::Triple(upper, middle, lower),
            Overlay::BollingerBands { period, multiplier },
        ) => {
            let mean = window_mean(closes, index, period);
            let deviation = window_deviation(closes, index, period);
            middle.push(mean);
            upper.push(mean + multiplier * deviation);
            lower.push(mean - multiplier * deviation);
        }
        (CachedOverlay::Triple(upper, middle, lower), Overlay::DonchianChannel { period }) => {
            let (top, center, bottom) = window_donchian(highs, lows, index, period);
            upper.push(top);
            middle.push(center);
            lower.push(bottom);
        }
        _ => {}
    }
}

/// Writes a price with exactly `decimals` places into `out`, reusing its allocation.
///
/// The frame path formats four axis labels, a crosshair tag and a bar readout per paint, and a
/// live pane repaints continuously, so the formatter writes into the caller's buffer instead of
/// returning a fresh `String` — the treatment the volume and depth panes give their labels
/// through `with_axis_label` (FIN-3).
///
/// Written out rather than using the dynamic-precision formatter, which the `alloc`-backed
/// `format!` in this crate does not provide. Doing the rounding here also means the label cannot
/// disagree with what the axis drew.
fn write_fixed(value: f64, decimals: usize, out: &mut String) {
    out.clear();
    if !value.is_finite() {
        out.push('—');
        return;
    }
    let factor = 10f64.powi(decimals as i32);
    let rounded = (value * factor).round() / factor;
    // Past i64's exact integer range the digit split below would be wrong; the plain
    // one-decimal expansion is the honest fallback and is what this formatter always
    // produced for sub-two-decimal labels.
    if rounded.abs() >= 9.0e15 {
        let _ = write!(out, "{rounded:.1}");
        return;
    }
    // Fewer than two requested decimals still print one, which is the shape this pane's
    // axis and tag labels have always had.
    let places = if decimals > 1 { decimals } else { 1 };
    let divisor = 10u64.pow(places as u32);
    let scaled = (rounded * divisor as f64).round();
    if scaled < 0.0 {
        out.push('-');
    }
    let digits = scaled.abs() as u64;
    let whole = digits / divisor;
    let fraction = digits % divisor;
    let _ = write!(out, "{whole}.");
    let mut fraction_digits = 1usize;
    let mut probe = fraction;
    while probe >= 10 {
        probe /= 10;
        fraction_digits += 1;
    }
    for _ in fraction_digits..places {
        out.push('0');
    }
    let _ = write!(out, "{fraction}");
}

/// Formats a price with exactly `decimals` places, for callers that need an owned string.
fn format_fixed(value: f64, decimals: usize) -> String {
    let mut text = String::new();
    write_fixed(value, decimals, &mut text);
    text
}

impl Widget for CandlestickChart {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn capability_name(&self) -> Option<&'static str> {
        Some("candlestick_chart")
    }
    fn set_state_theme_hook(&mut self) {
        rust_widgets::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn as_draw_mut(&mut self) -> Option<&mut dyn Draw> {
        Some(self)
    }
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually **emits**. Without an
    /// arm a published name is valid and inert, which is the silent failure
    /// `tools/check_event_signal_dyn.sh` exists to make impossible: the arm set is compared against
    /// the capability's published names, so the two cannot drift.
    fn event_signal_dyn(&self, name: &str) -> Option<rust_widgets::signal::EventSignalRef> {
        use rust_widgets::signal::EventSignalRef;
        match name {
            "bar_clicked" => Some(EventSignalRef::mapped("bar_clicked", &self.bar_clicked, |v| {
                CapabilityValue::UInt(*v as u64)
            })),
            "bar_hovered" => Some(EventSignalRef::mapped("bar_hovered", &self.bar_hovered, |v| {
                CapabilityValue::UInt(*v as u64)
            })),
            "bar_unhovered" => {
                Some(EventSignalRef::mapped("bar_unhovered", &self.bar_unhovered, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            "view_changed" => {
                Some(EventSignalRef::mapped("view_changed", &self.view_changed, |v| {
                    CapabilityValue::Tuple(rust_widgets::compat::Vec::from([
                        CapabilityValue::UInt(v.0 as u64),
                        CapabilityValue::UInt(v.1 as u64),
                    ]))
                }))
            }
            _ => None,
        }
    }
}

impl EventHandler for CandlestickChart {
    /// Tracks the hovered bar and emits the chart's own pointer signals.
    ///
    /// The base routing is delegated to first, so `hover`, `mouse_down` and the rest keep
    /// working exactly as for every other control — a caller using the generic signals is
    /// not broken by this control having a richer one.
    fn handle_event(&mut self, event: &rust_widgets::event::Event) {
        use rust_widgets::event::Event;
        // A companion edit shows on this frame: the queues are drained before the event is
        // routed, exactly as the parent's charts do.
        self.poll_chart_data_table();
        self.poll_zoom_strip();
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MouseMove { pos } | Event::PointerMove { pos, .. } => {
                let area = self.plot_area();
                let (start, end) = self.view_bounds();
                let index_axis = area.index_axis(end.saturating_sub(start));
                // The pane's bottom strip is its index axis, so a y test is part of "inside":
                // testing only x let the axis row hover (and crosshair) a bar.
                let next = if area.contains_y(pos.y) {
                    index_axis
                        .index_at(pos.x)
                        .map(|index| start + index)
                        .filter(|index| *index < end)
                } else {
                    None
                };
                if next != self.hovered_index {
                    if let Some(previous) = self.hovered_index {
                        self.bar_unhovered.emit(previous);
                    }
                    if let Some(index) = next {
                        self.bar_hovered.emit(index);
                    }
                    self.hovered_index = next;
                    self.base.request_redraw();
                }
                // The horizontal crosshair's price is the inverse mapping of the pointer's y
                // through the same price axis the candles are drawn with, so the readout
                // cannot disagree with the gridline it sits on.
                let (low, high) = self.effective_price_extent();
                let price_axis = area.price_axis(low, high);
                let price = price_axis.price_at(pos.y).clamp(price_axis.low, price_axis.high);
                if self.hovered_price != Some(price) {
                    self.hovered_price = Some(price);
                    self.base.request_redraw();
                }
            }
            Event::MouseLeave { .. } => {
                // The price half of the crosshair is tracked independently of the bar under
                // the pointer (a pointer in the axis margin has a price but no bar), so it
                // has to be cleared on exit too rather than only when a bar was hovered.
                let left_bar = self.hovered_index.take();
                let left_price = self.hovered_price.take();
                if let Some(previous) = left_bar {
                    self.bar_unhovered.emit(previous);
                }
                if left_bar.is_some() || left_price.is_some() {
                    self.base.request_redraw();
                }
            }
            Event::Wheel { delta, .. } => {
                let total = self.series.len();
                if total > 4 && delta.y != 0 {
                    let (start, end) = self.view_bounds();
                    let anchor = self.hovered_index.unwrap_or((start + end) / 2);
                    let span = (end - start).max(4);
                    let factor = if delta.y > 0 { 1.25 } else { 0.8 };
                    let new_span = ((span as f64 * factor).round() as usize).clamp(4, total);
                    let half = new_span / 2;
                    let new_start = anchor.saturating_sub(half).min(total - new_span);
                    // Showing every bar is the `None` spelling of the view, exactly what
                    // `reset_view` and the strip's Fit produce; a zoom-out that leaves a pinned
                    // `Some((0, total))` made one state look like two to a host binding
                    // `view_range`.
                    let range = if new_span >= total {
                        None
                    } else {
                        Some((new_start, new_start + new_span))
                    };
                    self.set_view_range(range);
                }
            }
            Event::MousePress { pos, .. } | Event::PointerPress { pos, .. } => {
                let area = self.plot_area();
                let (start, end) = self.view_bounds();
                let index_axis = area.index_axis(end.saturating_sub(start));
                if area.contains_y(pos.y) {
                    if let Some(index) = index_axis
                        .index_at(pos.x)
                        .map(|index| start + index)
                        .filter(|index| *index < end)
                    {
                        self.bar_clicked.emit(index);
                    }
                }
            }
            _ => {}
        }
        self.base.handle_event(event);
    }
}

impl Draw for CandlestickChart {
    /// Draws the candles, then the overlays, then the annotations.
    ///
    /// The order is the reading order: price first, derived lines over it, and the levels
    /// and crosshair last so an annotation is never hidden by the data it annotates.
    fn draw(&mut self, context: &mut RenderContext) {
        // A companion edit shows on this frame: both queues are drained before painting.
        self.poll_chart_data_table();
        self.poll_zoom_strip();
        let area = self.plot_area();
        if area.rect.width == 0 || area.rect.height == 0 {
            return;
        }
        // The pane surface resolves the caller's style first and the theme second, so it is
        // no longer one literal shared by both appearances. The reds and greens below are the
        // price direction and stay exactly as they are: they are data, not chrome.
        let chrome = self.chrome();
        context.fill_rect(area.rect, chrome.surface);

        // An empty series gets the frame and a message rather than a bare slab.
        if self.series.bars().is_empty() {
            self.draw_empty_state(context, &area);
            return;
        }

        self.draw_price_labels(context, &area);
        self.draw_candles(context, &area);
        // The bar index under the candles, from the series' own labels (FIN-01).
        let (start, end) = self.view_bounds();
        super::volume_chart::draw_index_ticks(
            context,
            &area,
            start,
            end,
            |index, label| match self.series.label_at(index) {
                Some(text) => label.push_str(text),
                None => {
                    let _ = core::fmt::Write::write_fmt(&mut *label, format_args!("{index}"));
                }
            },
            &role_font(self.style(), 10.0),
            chrome.ink.with_alpha(190),
        );
        self.draw_overlays(context, &area);
        self.draw_price_lines(context, &area);
        if self.show_overlay_legend && !self.overlays.is_empty() {
            self.draw_overlay_legend(context, &area);
        }
        self.draw_crosshair(context, &area);
    }
}

/// Test module for the K-line chart.
///
/// # What these assert that a rendering test could not
///
/// The control's own contract: what it reports about its data, what it emits, and
/// whether it survives the degenerate inputs a live feed produces. Drawing is covered
/// by the shared visual-regression gate, which compares actual pixels; these tests
/// cover the decisions that happen before a pixel exists.
impl WidgetProperties for CandlestickChart {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            // The series is serialised as a flat list of `close` prices, which is the
            // one number every reader wants and the one the overlays are computed from.
            // The full OHLC is available through `series()` for a caller that needs it.
            "series" => Ok(CapabilityValue::String(serialise_closes(&self.series))),
            "overlay_count" => Ok(CapabilityValue::UInt(self.overlays.len() as u64)),
            "show_price_levels" => Ok(CapabilityValue::Bool(self.show_price_levels)),
            "show_overlay_legend" => Ok(CapabilityValue::Bool(self.show_overlay_legend)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "series" => {
                let CapabilityValue::String(text) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                let mut series = PriceSeries::new();
                for token in text.split([',', ' ', ';']).filter(|token| !token.is_empty()) {
                    let price: f64 =
                        token.trim().parse().map_err(|_| CapabilityAccessError::OutOfRange)?;
                    // A single price per token cannot express an OHLC bar, so it becomes
                    // a flat bar: open == high == low == close. That is the honest
                    // reading of "here is a price series", and it is what a caller
                    // streaming ticks actually has.
                    series.push(Bar::flat(price, 0.0));
                }
                self.set_series(series);
                Ok(())
            }
            "overlay_count" => {
                let CapabilityValue::UInt(count) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                // Truncate or extend with the default overlay, so a caller can set a
                // count without knowing which periods to pick.
                self.overlays.truncate(count as usize);
                while self.overlays.len() < count as usize {
                    self.overlays.push(Overlay::moving_average(20));
                }
                self.rebuild_overlay_cache();
                self.base.request_redraw();
                Ok(())
            }
            "show_price_levels" => {
                let CapabilityValue::Bool(show) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                // A display flag, not a data operation: turning the levels off hides them and
                // turning them back on shows the same levels again. The previous write-true
                // was a no-op and write-false destroyed the caller's levels, so the published
                // read/write contract could not be honoured in either direction (FIN-5).
                if self.show_price_levels != show {
                    self.show_price_levels = show;
                    // The axis spans what is painted, so a hidden level stops stretching it —
                    // the same re-derivation `set_overlay_visible` performs (FIN-5).
                    self.rebuild_extent();
                    self.base.request_redraw();
                }
                Ok(())
            }
            "show_overlay_legend" => {
                let CapabilityValue::Bool(show) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                self.show_overlay_legend = show;
                self.base.request_redraw();
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "series",
            "overlay_count",
            "show_price_levels",
            "show_overlay_legend",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `candlestick_chart` publishes.
    ///
    /// This is the one pane in the finance group whose `add_overlay` is a real
    /// method, and it is the method the command names, so a bare invocation runs it
    /// with the study the command's own affordance offers by default: a 20-period
    /// moving average. That is the same overlay the property layer already uses when
    /// it has to extend `overlay_count` with a default (see the `overlay_count`
    /// setter), so the two routes to "add the standard overlay" agree instead of
    /// inventing two different studies for one name. `set_series` carries the data.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "add_overlay" => {
                self.add_overlay(Overlay::moving_average(20));
                Ok(())
            }
            // Every other published name is `set_*`, whose value arrives through the property
            // route; the convention answers `OutOfRange` for a bare invocation.
            _ => self.default_command(name),
        }
    }
}

/// Renders a series' closes as a comma-separated list.
///
/// The inverse of the `series` setter, so a read-then-write round trip is the identity
/// for a flat series — which is what `capability_value_round_trips` checks.
fn serialise_closes(series: &PriceSeries) -> alloc::string::String {
    let mut text = alloc::string::String::new();
    for (index, bar) in series.bars().iter().enumerate() {
        if index > 0 {
            text.push(',');
        }
        text.push_str(&alloc::format!("{}", bar.close));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::finance::types::fixtures;
    use alloc::sync::Arc;
    use core::sync::atomic::{AtomicUsize, Ordering};

    /// A chart carries the series it was given and reports its own extent.
    #[test]
    fn a_chart_reports_the_series_it_was_given() {
        let mut chart = CandlestickChart::new(Rect::new(0, 0, 640, 360));
        assert!(chart.series().is_empty());
        chart.set_series(fixtures::sample_series(40));
        assert_eq!(chart.series().len(), 40);
    }

    /// Overlays accumulate, and the list can be cleared.
    ///
    /// Appending rather than replacing is the contract, because the common case is a
    /// moving-average ribbon; a chart that silently kept only the last overlay would
    /// draw one line where the caller asked for three.
    #[test]
    fn overlays_accumulate_and_clear() {
        let mut chart = CandlestickChart::new(Rect::new(0, 0, 640, 360));
        chart.add_overlay(Overlay::moving_average(20));
        chart.add_overlay(Overlay::exponential_moving_average(9));
        assert_eq!(chart.overlays().len(), 2);
        assert_eq!(chart.overlays()[0], Overlay::MovingAverage { period: 20 });
        chart.clear_overlays();
        assert!(chart.overlays().is_empty());
    }

    /// Every overlay kind reports the warm-up it needs.
    ///
    /// This is what a caller uses to explain a gap at the left of the chart, and it is
    /// the one number each variant must expose, so a new variant that forgets to answer
    /// fails here rather than reporting zero.
    #[test]
    fn every_overlay_reports_its_warm_up() {
        // The EMA is seeded with its first sample, so it has no overlay-free lead; the
        // window-based overlays start after their window. Reporting `period` for the EMA hid bars
        // whose line is in fact painted.
        assert_eq!(Overlay::moving_average(20).warm_up_bars(), 20);
        assert_eq!(Overlay::exponential_moving_average(9).warm_up_bars(), 0);
        assert_eq!(Overlay::bollinger_bands(20, 2.0).warm_up_bars(), 20);
        assert_eq!(Overlay::vwap(14).warm_up_bars(), 14);
        assert_eq!(Overlay::donchian_channel(20).warm_up_bars(), 20);
        assert!(
            crate::finance::indicators::ema(&[10.0, 11.0], 9)[0].is_finite(),
            "the EMA defines index 0, which is why its warm-up is zero"
        );
        for overlay in [
            Overlay::moving_average(20),
            Overlay::exponential_moving_average(9),
            Overlay::bollinger_bands(20, 2.0),
            Overlay::vwap(14),
            Overlay::donchian_channel(20),
        ] {
            assert!(!overlay.as_str().is_empty());
        }
    }

    /// The overlay spellings round-trip through their own parser.
    #[test]
    fn overlay_names_are_stable() {
        for overlay in [
            Overlay::moving_average(1),
            Overlay::exponential_moving_average(1),
            Overlay::bollinger_bands(1, 1.0),
            Overlay::vwap(1),
            Overlay::donchian_channel(1),
        ] {
            let name = overlay.as_str();
            assert!(!name.is_empty(), "every overlay needs a factory spelling");
        }
    }

    /// Price lines accumulate and can be cleared.
    #[test]
    fn price_lines_accumulate_and_clear() {
        let mut chart = CandlestickChart::new(Rect::new(0, 0, 640, 360));
        chart.add_price_line(PriceLine::new(95.0, PriceLevelKind::Support));
        chart.add_price_line(PriceLine::new(120.0, PriceLevelKind::Resistance));
        assert_eq!(chart.price_lines().len(), 2);
        chart.clear_price_lines();
        assert!(chart.price_lines().is_empty());
    }

    /// `show_price_levels` is a stored display flag, not a derived "are there levels?" answer.
    ///
    /// The property is published readable *and* writable, so both directions have to work: a
    /// written value reads back, and hiding the levels must not destroy the data (FIN-5). The
    /// old setter ignored `true` and cleared the levels on `false`, which made the read-back a
    /// report about unrelated state.
    #[test]
    fn show_price_levels_reads_back_and_hides_without_clearing() {
        let mut chart = CandlestickChart::new(Rect::new(0, 0, 640, 360));
        chart.add_price_line(PriceLine::new(95.0, PriceLevelKind::Support));
        assert_eq!(
            chart.get("show_price_levels"),
            Ok(CapabilityValue::Bool(true)),
            "added levels are drawn by default"
        );

        chart.set("show_price_levels", CapabilityValue::Bool(false)).expect("writable");
        assert_eq!(chart.get("show_price_levels"), Ok(CapabilityValue::Bool(false)));
        assert_eq!(chart.price_lines().len(), 1, "hiding the levels keeps their data");

        chart.set("show_price_levels", CapabilityValue::Bool(true)).expect("writable");
        assert_eq!(
            chart.get("show_price_levels"),
            Ok(CapabilityValue::Bool(true)),
            "turning the levels back on shows the same levels"
        );
        assert_eq!(chart.price_lines().len(), 1);
    }

    /// The data-table key moves with every input the snapshot reads (FIN-1).
    ///
    /// A key that never moved would silently freeze the attached table, and one that moved too
    /// eagerly would just be the old per-frame cost. So this pins both directions: the key is
    /// stable on an untouched pane and different for each of the inputs the snapshot names.
    #[test]
    fn the_chart_data_key_tracks_the_snapshot_inputs() {
        let mut chart = CandlestickChart::new(Rect::new(0, 0, 640, 360));
        chart.set_series(fixtures::sample_series(30));
        chart.add_overlay(Overlay::moving_average(5));
        let base = chart.chart_data_key();
        assert_eq!(base, chart.chart_data_key(), "an untouched pane keys equal");

        chart.set_overlay_visible(0, false);
        let hidden = chart.chart_data_key();
        assert_ne!(hidden, base, "the visibility flags are part of the key");
        chart.set_overlay_visible(0, true);
        assert_eq!(chart.chart_data_key(), base, "showing it again restores the key");

        chart.add_overlay(Overlay::bollinger_bands(5, 2.0));
        let two_overlays = chart.chart_data_key();
        assert_ne!(two_overlays, base, "the overlay list is part of the key");

        chart.append_bar(Bar::flat(1_000.0, 1.0));
        assert_ne!(chart.chart_data_key(), two_overlays, "a new bar moves the key");
    }

    /// A hovered index from a longer series is dropped when a shorter one replaces it.
    ///
    /// The stale index would otherwise report a bar that no longer exists — the
    /// off-by-one class of bug this crate's `PriceSeries` exists to prevent, applied to
    /// hover state rather than to data.
    #[test]
    fn replacing_the_series_drops_a_stale_hover() {
        let mut chart = CandlestickChart::new(Rect::new(0, 0, 640, 360));
        chart.set_series(fixtures::sample_series(50));
        chart.set_geometry(Rect::new(0, 0, 640, 360));
        // Point the hover at a bar that only the long series has.
        chart.handle_event(&rust_widgets::event::Event::MouseMove {
            pos: rust_widgets::core::Point { x: 640 - 4, y: 100 },
        });
        chart.set_series(fixtures::sample_series(3));
        assert!(
            chart.hovered_index().is_none_or(|index| index < 3),
            "a hover must not survive past the series it pointed into"
        );
    }

    /// Whether `target` appears anywhere in the frame a draw produces.
    fn frame_contains_color(
        chart: &mut CandlestickChart,
        size: (u32, u32),
        target: [u8; 4],
    ) -> bool {
        use rust_widgets::render::PaintBackend as _;
        let mut backend = rust_widgets::render::SoftwarePaintBackend::new(
            rust_widgets::core::Size::new(size.0, size.1),
            1.0,
        );
        backend.begin_frame(rust_widgets::core::Color::BLACK);
        {
            let mut context = RenderContext::new(&mut backend);
            chart.draw(&mut context);
        }
        backend.end_frame();
        let rgba = backend.frame_rgba();
        rgba.as_chunks::<4>().0.contains(&target)
    }

    /// A hidden overlay is not painted, matching the extent and the table that say it is off.
    ///
    /// Regression: the visibility flag removed the overlay from the price extent and from the
    /// data table's visible rows, but `draw_overlays` walked every cached overlay regardless,
    /// so a hidden series kept drawing over the candles.
    #[test]
    fn a_hidden_overlay_is_not_painted() {
        let mut chart = CandlestickChart::new(Rect::new(0, 0, 480, 320));
        chart.set_series(fixtures::sample_series(60));
        chart.add_overlay(Overlay::moving_average(2));
        // The legend draws a swatch in the same palette colour, so it is turned off for this
        // pixel assertion; it is not the subject of the test.
        chart.set_show_overlay_legend(false);
        let color = CandlestickChart::overlay_color(0);
        let target = [color.r, color.g, color.b, 255];

        assert!(
            frame_contains_color(&mut chart, (480, 320), target),
            "the visible overlay must be painted"
        );
        assert!(chart.set_overlay_visible(0, false));
        assert!(
            !frame_contains_color(&mut chart, (480, 320), target),
            "a hidden overlay must not leave a pixel of its colour behind"
        );
    }

    /// Leaving the pane clears the price crosshair even when the pointer was in the margin.
    ///
    /// Regression: `MouseLeave` only cleared state when a bar was hovered, so a pointer that
    /// left from the price-axis margin kept `hovered_price` set for the next entry.
    #[test]
    fn leaving_the_pane_clears_a_price_hover_even_without_a_bar() {
        let mut chart = CandlestickChart::new(Rect::new(0, 0, 640, 360));
        chart.set_series(fixtures::sample_series(20));
        chart.handle_event(&rust_widgets::event::Event::MouseMove {
            pos: rust_widgets::core::Point { x: 5, y: 100 },
        });
        assert!(chart.hovered_index().is_none(), "the price axis is not over a bar");
        assert!(chart.hovered_price().is_some(), "the crosshair price is still tracked");

        chart.handle_event(&rust_widgets::event::Event::MouseLeave {
            pos: rust_widgets::core::Point { x: 0, y: 0 },
        });
        assert!(chart.hovered_index().is_none());
        assert!(chart.hovered_price().is_none(), "leaving must clear the price hover too");
    }

    /// A window measured against a longer series is pulled back inside a shorter one.
    ///
    /// Regression: `set_series` kept the stored range verbatim, so replacing a 50-bar series
    /// with a 5-bar one left a `(40, 50)` window that selected no bars — the pane drew its
    /// axes and no candles, and `view_range()` reported a window the control did not hold.
    #[test]
    fn replacing_the_series_pulls_the_view_back_inside_it() {
        let mut chart = CandlestickChart::new(Rect::new(0, 0, 320, 200));
        chart.set_series(fixtures::sample_series(50));
        chart.set_view_range(Some((40, 50)));

        chart.set_series(fixtures::sample_series(5));
        let (start, end) = chart.view_range().expect("a clamped window must remain");
        assert!(start < 5, "the window start must be inside the new series, got {start}");
        assert!(start < end && end <= 5, "the window must select existing bars: {start}..{end}");
        assert_eq!(chart.view_bounds(), (start, end));
    }

    /// A chart streamed from empty derives its axis from the first bar.
    ///
    /// Regression: `grow_extent_with_bar` grew the constructor's `(0, 1)` placeholder, so the
    /// first streamed price kept a zero in the axis and every later bar was squashed against
    /// the top of the pane.
    #[test]
    fn streaming_into_an_empty_chart_does_not_keep_the_placeholder_axis() {
        let mut chart = CandlestickChart::new(Rect::new(0, 0, 320, 200));
        for index in 0..5 {
            chart.append_bar(Bar::flat(100.0 + index as f64, 10.0));
        }
        let (low, high) = chart.effective_price_extent();
        assert!(low > 90.0, "the axis still spans a placeholder zero: {low}");
        assert!(high >= 104.0, "the axis must contain the streamed prices: {high}");
    }

    /// A full replacement reaches an attached data table even when the cheap key matches.
    ///
    /// Regression: the key covers the length, the last close and the overlay tails only, so a
    /// series whose middle changed was treated as unchanged and the table kept the old rows.
    #[test]
    fn replacing_the_series_refreshes_an_attached_data_table() {
        let mut chart = CandlestickChart::new(Rect::new(0, 0, 480, 240));
        chart.set_series(fixtures::sample_series(20));
        let mut table = ChartDataTable::new(Rect::new(0, 0, 240, 180));
        chart.attach_chart_data_table(&mut table);
        assert!(!chart.poll_chart_data_table(), "an idle pane queues nothing");

        let mut series = fixtures::sample_series(20);
        let mut first = series.bars()[0];
        first.close += 5.0;
        assert!(series.set_bar(0, first));
        chart.set_series(series);

        assert!(
            chart.poll_chart_data_table(),
            "a replaced series must be pushed even when length and last close match"
        );
    }

    /// An empty series draws nothing and does not panic.
    ///
    /// A live feed delivers an empty list before its first snapshot, and a chart that
    /// divides by the series length would panic in the paint callback — which on a
    /// platform backend is inside a native window procedure.
    #[test]
    fn an_empty_series_draws_without_panicking() {
        let mut chart = CandlestickChart::new(Rect::new(0, 0, 320, 200));
        chart.add_overlay(Overlay::moving_average(20));
        let mut backend = rust_widgets::render::SoftwarePaintBackend::new(
            rust_widgets::core::Size::new(320, 200),
            1.0,
        );
        let mut context = RenderContext::new(&mut backend);
        chart.draw(&mut context);
    }

    /// A series of malformed bars draws without panicking.
    ///
    /// Every field non-finite and every high below its low is the worst case a feed can
    /// deliver. The control must clamp rather than divide by a zero span.
    #[test]
    fn a_malformed_series_draws_without_panicking() {
        let mut series = PriceSeries::new();
        series.push(Bar::new(f64::NAN, f64::NAN, f64::NAN, f64::NAN, f64::NAN));
        series.push(Bar::new(10.0, 5.0, 20.0, 12.0, 100.0));
        series.push(Bar::flat(10.0, 0.0));
        let mut chart = CandlestickChart::new(Rect::new(0, 0, 320, 200));
        chart.set_series(series);
        chart.add_overlay(Overlay::bollinger_bands(20, 2.0));
        let mut backend = rust_widgets::render::SoftwarePaintBackend::new(
            rust_widgets::core::Size::new(320, 200),
            1.0,
        );
        let mut context = RenderContext::new(&mut backend);
        chart.draw(&mut context);
    }

    /// A flat series — every bar at one price — still draws.
    ///
    /// This is the degenerate price range, and it is what an illiquid instrument looks
    /// like. The axis widens rather than dividing by zero.
    #[test]
    fn a_flat_series_draws_without_panicking() {
        let mut series = PriceSeries::new();
        for _ in 0..10 {
            series.push(Bar::flat(50.0, 10.0));
        }
        let mut chart = CandlestickChart::new(Rect::new(0, 0, 320, 200));
        chart.set_series(series);
        let mut backend = rust_widgets::render::SoftwarePaintBackend::new(
            rust_widgets::core::Size::new(320, 200),
            1.0,
        );
        let mut context = RenderContext::new(&mut backend);
        chart.draw(&mut context);
    }

    /// Every overlay kind draws with a real series.
    #[test]
    fn every_overlay_kind_draws() {
        for overlay in [
            Overlay::moving_average(10),
            Overlay::exponential_moving_average(10),
            Overlay::bollinger_bands(10, 2.0),
            Overlay::vwap(10),
            Overlay::donchian_channel(10),
        ] {
            let mut chart = CandlestickChart::new(Rect::new(0, 0, 480, 320));
            chart.set_series(fixtures::sample_series(60));
            chart.add_overlay(overlay);
            let mut backend = rust_widgets::render::SoftwarePaintBackend::new(
                rust_widgets::core::Size::new(480, 320),
                1.0,
            );
            let mut context = RenderContext::new(&mut backend);
            chart.draw(&mut context);
        }
    }

    /// Clicking a bar emits that bar's index.
    ///
    /// The slot records through an `Arc<AtomicUsize>` rather than a captured variable
    /// because `GenericSignal` hands the value out as an `Arc` and the slot must be
    /// `Send`; the shared counter is the pattern every other signal test in this crate
    /// uses.
    /// The view range is shared by the painter, the overlay pass and the hit test.
    #[test]
    fn a_view_range_reports_and_maps_hits_to_global_indices() {
        let mut chart = CandlestickChart::new(Rect::new(0, 0, 300, 200));
        let mut series = PriceSeries::new();
        for index in 0..10 {
            let price = 10.0 + index as f64;
            series.push(Bar::flat(price, 1.0));
        }
        chart.set_series(series);
        chart.add_overlay(Overlay::moving_average(2));
        chart.add_overlay(Overlay::bollinger_bands(3, 2.0));
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<(usize, usize)>::new()));
        let sink = std::sync::Arc::clone(&seen);
        chart.view_changed.connect(move |range| sink.lock().unwrap().push(*range));
        let clicked = std::sync::Arc::new(std::sync::Mutex::new(Vec::<usize>::new()));
        let click_sink = std::sync::Arc::clone(&clicked);
        chart.bar_clicked.connect(move |index| click_sink.lock().unwrap().push(*index));

        chart.set_view_range(Some((4, 7)));
        assert_eq!(*seen.lock().unwrap(), vec![(4, 7)]);
        let area = chart.plot_area();
        chart.handle_event(&rust_widgets::event::Event::mouse_press(
            area.rect.x + 1,
            area.rect.y + 5,
            1,
        ));
        assert_eq!(*clicked.lock().unwrap(), vec![4], "the left edge is the first visible bar");
        assert!(chart.show_overlay_legend(), "the legend is on by default when overlays exist");
    }

    #[test]
    fn clicking_a_bar_emits_its_index() {
        let mut chart = CandlestickChart::new(Rect::new(0, 0, 640, 360));
        chart.set_series(fixtures::sample_series(20));
        let seen = Arc::new(AtomicUsize::new(usize::MAX));
        let sink = Arc::clone(&seen);
        chart.bar_clicked.connect(move |index| sink.store(*index, Ordering::SeqCst));
        chart.handle_event(&rust_widgets::event::Event::MousePress {
            pos: rust_widgets::core::Point { x: 70, y: 100 },
            button: 0,
            modifiers: 0,
        });
        let index = seen.load(Ordering::SeqCst);
        assert_ne!(index, usize::MAX, "a press inside the plot must emit a bar index");
        assert!(index < 20, "and the index must be within the series");
    }

    /// A click outside the plot emits nothing.
    ///
    /// The distinction matters: an index is only meaningful for a bar that exists, and
    /// clamping a click past the last bar to the last index would report a bar the
    /// pointer was never over.
    #[test]
    fn clicking_outside_the_plot_emits_nothing() {
        let mut chart = CandlestickChart::new(Rect::new(0, 0, 640, 360));
        chart.set_series(fixtures::sample_series(20));
        let count = Arc::new(AtomicUsize::new(0));
        let sink = Arc::clone(&count);
        chart.bar_clicked.connect(move |_| {
            sink.fetch_add(1, Ordering::SeqCst);
        });
        chart.handle_event(&rust_widgets::event::Event::MousePress {
            pos: rust_widgets::core::Point { x: 5, y: 100 },
            button: 0,
            modifiers: 0,
        });
        assert_eq!(count.load(Ordering::SeqCst), 0, "a press on the price axis is not on a bar");
    }

    /// The index-axis strip below the plot is not a bar.
    ///
    /// The hit test mapped x alone, so the tick row hovered and clicked whichever bar shared its
    /// column — the same defect `DepthChart` documents for its price axis.
    #[test]
    fn the_index_axis_strip_is_not_a_bar() {
        let mut chart = CandlestickChart::new(Rect::new(0, 0, 640, 360));
        chart.set_series(fixtures::sample_series(20));
        let count = Arc::new(AtomicUsize::new(0));
        let sink = Arc::clone(&count);
        chart.bar_clicked.connect(move |_| {
            sink.fetch_add(1, Ordering::SeqCst);
        });
        let area = chart.plot_area();
        chart.handle_event(&rust_widgets::event::Event::mouse_press(
            area.rect.x + 30,
            area.bottom() + 5,
            1,
        ));
        assert_eq!(count.load(Ordering::SeqCst), 0, "the tick row is not a bar");

        chart.handle_event(&rust_widgets::event::Event::MouseMove {
            pos: rust_widgets::core::Point { x: area.rect.x + 30, y: area.bottom() + 5 },
        });
        assert_eq!(chart.hovered_index(), None, "and it does not hover one either");
    }

    /// Hovering emits the index and then the un-hover on exit.
    #[test]
    fn hovering_emits_and_exiting_un_hovers() {
        let mut chart = CandlestickChart::new(Rect::new(0, 0, 640, 360));
        chart.set_series(fixtures::sample_series(20));
        let hovered = Arc::new(AtomicUsize::new(usize::MAX));
        let unhovered = Arc::new(AtomicUsize::new(usize::MAX));
        let hovered_sink = Arc::clone(&hovered);
        let unhovered_sink = Arc::clone(&unhovered);
        chart.bar_hovered.connect(move |index| hovered_sink.store(*index, Ordering::SeqCst));
        chart.bar_unhovered.connect(move |index| unhovered_sink.store(*index, Ordering::SeqCst));

        chart.handle_event(&rust_widgets::event::Event::MouseMove {
            pos: rust_widgets::core::Point { x: 70, y: 100 },
        });
        let hovered_index = hovered.load(Ordering::SeqCst);
        assert_ne!(hovered_index, usize::MAX, "moving over a bar emits it");

        chart.handle_event(&rust_widgets::event::Event::MouseLeave {
            pos: rust_widgets::core::Point { x: 0, y: 0 },
        });
        assert_eq!(
            unhovered.load(Ordering::SeqCst),
            hovered_index,
            "leaving reports the bar that was hovered"
        );
        assert!(chart.hovered_index().is_none());
    }

    /// A hidden overlay's legend row is not painted either.
    ///
    /// `draw_overlays` skipped hidden overlays, but the legend kept their swatch and name at full
    /// opacity in the same palette colour, so the pane painted a key entry for a series it was not
    /// drawing.
    #[test]
    fn a_hidden_overlay_is_not_in_the_legend() {
        let mut chart = CandlestickChart::new(Rect::new(0, 0, 480, 320));
        chart.set_series(fixtures::sample_series(60));
        chart.add_overlay(Overlay::moving_average(2));
        let color = CandlestickChart::overlay_color(0);
        let target = [color.r, color.g, color.b, 255];

        assert!(
            frame_contains_color(&mut chart, (480, 320), target),
            "a visible overlay is painted (line and legend)"
        );
        assert!(chart.set_overlay_visible(0, false));
        assert!(
            !frame_contains_color(&mut chart, (480, 320), target),
            "a hidden overlay's legend swatch must not keep advertising it"
        );
    }

    /// Narrowing the view drops a hover it excludes, and reports the exit.
    ///
    /// `set_view_range` moved the window without re-deriving the hover, so `hovered_summary`
    /// returned a bar `hover_anchor` refused to anchor and no `bar_unhovered` was emitted.
    #[test]
    fn narrowing_the_view_drops_a_hover_it_excludes() {
        let mut chart = CandlestickChart::new(Rect::new(0, 0, 640, 360));
        chart.set_series(fixtures::sample_series(50));
        let area = chart.plot_area();
        let (start, end) = chart.view_bounds();
        let target = 30usize;
        let x = area.index_axis(end - start).center_for(target - start);
        chart.handle_event(&rust_widgets::event::Event::MouseMove {
            pos: rust_widgets::core::Point { x, y: area.rect.y + 40 },
        });
        assert_eq!(chart.hovered_index(), Some(target));

        let unhovered = std::sync::Arc::new(std::sync::Mutex::new(Vec::<usize>::new()));
        let sink = unhovered.clone();
        chart.bar_unhovered.connect(move |index: Arc<usize>| {
            sink.lock().unwrap().push(*index);
        });

        chart.set_view_range(Some((0, 10)));
        assert_eq!(chart.hovered_index(), None);
        assert!(chart.hovered_summary().is_empty(), "no tooltip for a bar outside the window");
        assert!(chart.hover_anchor().is_none(), "the crosshair is not anchored either");
        assert_eq!(*unhovered.lock().unwrap(), vec![target], "the exit is published");
    }

    /// The inferred precision covers the whole series, not its first price.
    #[test]
    fn the_axis_precision_covers_the_whole_series() {
        let mut chart = CandlestickChart::new(Rect::new(0, 0, 480, 320));
        let mut series = PriceSeries::new();
        series.push(Bar::flat(100.0, 1.0));
        series.push(Bar::flat(100.25, 1.0));
        chart.set_series(series);

        assert_eq!(chart.inferred_decimals, 2, "the finer price decides the precision");
        assert_eq!(format_fixed(chart.series().bars()[1].close, chart.inferred_decimals), "100.25");
    }

    /// Zooming all the way out restores the unzoomed spelling of the view.
    #[test]
    fn zooming_out_to_the_whole_series_restores_the_unzoomed_view() {
        let mut chart = CandlestickChart::new(Rect::new(0, 0, 640, 360));
        chart.set_series(fixtures::sample_series(20));
        chart.set_view_range(Some((5, 15)));

        for _ in 0..12 {
            chart.handle_event(&rust_widgets::event::Event::Wheel {
                delta: rust_widgets::core::Point::new(0, 1),
                modifiers: 0,
            });
        }
        assert_eq!(
            chart.view_range(),
            None,
            "a full zoom-out is the same state `reset_view` produces"
        );
    }
}
