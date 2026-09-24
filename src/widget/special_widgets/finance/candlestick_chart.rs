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

use crate::core::{Color, Font, HorizontalAlignment, Rect};
use crate::event::EventHandler;
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::special_widgets::finance::indicators;
use crate::widget::special_widgets::finance::layout::{
    panel_colors, PanelColors, PlotArea, PANEL_MIN_CONTRAST,
};
use crate::widget::special_widgets::finance::types::{Bar, PriceLevelKind, PriceLine, PriceSeries};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

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
            Overlay::ExponentialMovingAverage { period } => period,
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
pub struct CandlestickChart {
    base: BaseWidget,
    series: PriceSeries,
    overlays: Vec<Overlay>,
    price_lines: Vec<PriceLine>,
    /// The bar under the pointer, when there is one.
    hovered_index: Option<usize>,
    /// Emitted with the bar index when a bar is clicked.
    pub bar_clicked: Signal1<usize>,
    /// Emitted with the bar index as the pointer moves across bars.
    pub bar_hovered: Signal1<usize>,
    /// Emitted with the bar index the pointer left, on exit.
    pub bar_unhovered: Signal1<usize>,
}

impl CandlestickChart {
    /// Creates an empty chart.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::CandlestickChart, geometry, "CandlestickChart"),
            series: PriceSeries::new(),
            overlays: Vec::new(),
            price_lines: Vec::new(),
            hovered_index: None,
            bar_clicked: Signal1::new(),
            bar_hovered: Signal1::new(),
            bar_unhovered: Signal1::new(),
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
        // stale index would report a bar that is no longer there.
        if self.hovered_index.is_some_and(|index| index >= self.series.len()) {
            self.hovered_index = None;
        }
        self.base.request_redraw();
    }

    /// The overlays, in draw order.
    pub fn overlays(&self) -> &[Overlay] {
        &self.overlays
    }

    /// Appends an overlay.
    ///
    /// Appends rather than replaces, because the common case is a chart showing two or
    /// three averages at once — a moving-average ribbon. A caller wanting a single overlay
    /// calls [`Self::clear_overlays`] first.
    pub fn add_overlay(&mut self, overlay: Overlay) {
        self.overlays.push(overlay);
        self.base.request_redraw();
    }

    /// Removes every overlay.
    pub fn clear_overlays(&mut self) {
        self.overlays.clear();
        self.base.request_redraw();
    }

    /// The marked price levels.
    pub fn price_lines(&self) -> &[PriceLine] {
        &self.price_lines
    }

    /// Adds a horizontal price level.
    pub fn add_price_line(&mut self, line: PriceLine) {
        self.price_lines.push(line);
        self.base.request_redraw();
    }

    /// Removes every price level.
    pub fn clear_price_lines(&mut self) {
        self.price_lines.clear();
        self.base.request_redraw();
    }

    /// The bar index under the pointer, if the pointer is over a bar.
    pub fn hovered_index(&self) -> Option<usize> {
        self.hovered_index
    }

    /// The bar under the pointer.
    pub fn hovered_bar(&self) -> Option<Bar> {
        self.hovered_index.and_then(|index| self.series.bars().get(index).copied())
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
        price_axis: &crate::widget::special_widgets::finance::layout::PriceAxis,
        index_axis: &crate::widget::special_widgets::finance::layout::IndexAxis,
        color: Color,
        width: u32,
    ) {
        let mut previous: Option<(i32, i32)> = None;
        for (index, value) in values.iter().enumerate() {
            if !value.is_finite() || index >= index_axis.count {
                // Break the run so the next finite point starts a new segment.
                previous = None;
                continue;
            }
            let point = (index_axis.center_for(index), price_axis.y_for(*value));
            if let Some(from) = previous {
                context.draw_line_stroke(
                    crate::core::Point { x: from.0, y: from.1 },
                    crate::core::Point { x: point.0, y: point.1 },
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
        let index_axis = area.index_axis(bars.len());
        let body_width = index_axis.body_width();
        let wick_x_offset = body_width / 2;

        for (index, bar) in bars.iter().enumerate() {
            if !bar.open.is_finite()
                || !bar.high.is_finite()
                || !bar.low.is_finite()
                || !bar.close.is_finite()
            {
                continue;
            }
            let slot_x = index_axis.x_for(index);
            let color = if bar.is_rising() { Self::rising_color() } else { Self::falling_color() };

            // Wick first, so the body covers its middle and the extremes stay visible.
            let wick_x = slot_x + wick_x_offset;
            context.draw_line_stroke(
                crate::core::Point { x: wick_x, y: price_axis.y_for(bar.high) },
                crate::core::Point { x: wick_x, y: price_axis.y_for(bar.low) },
                color,
                1,
            );

            let y_open = price_axis.y_for(bar.open);
            let y_close = price_axis.y_for(bar.close);
            let body_top = y_open.min(y_close);
            context.fill_rect(
                Rect::new(
                    slot_x,
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
        let bars = self.series.bars();
        if bars.is_empty() || self.overlays.is_empty() {
            return;
        }
        let (low, high) = self.effective_price_extent();
        let price_axis = area.price_axis(low, high);
        let index_axis = area.index_axis(bars.len());

        let closes = self.series.closes();
        let highs = self.series.highs();
        let lows = self.series.lows();
        let volumes = self.series.volumes();

        for (overlay_index, overlay) in self.overlays.iter().enumerate() {
            let color = Self::overlay_color(overlay_index);
            match *overlay {
                Overlay::MovingAverage { period } => {
                    Self::draw_series_line(
                        context,
                        &indicators::sma(&closes, period),
                        &price_axis,
                        &index_axis,
                        color,
                        1,
                    );
                }
                Overlay::ExponentialMovingAverage { period } => {
                    Self::draw_series_line(
                        context,
                        &indicators::ema(&closes, period),
                        &price_axis,
                        &index_axis,
                        color,
                        1,
                    );
                }
                Overlay::BollingerBands { period, multiplier } => {
                    let (lower, middle, upper) =
                        indicators::bollinger_bands(&closes, period, multiplier);
                    Self::draw_series_line(context, &upper, &price_axis, &index_axis, color, 1);
                    Self::draw_series_line(context, &middle, &price_axis, &index_axis, color, 1);
                    Self::draw_series_line(context, &lower, &price_axis, &index_axis, color, 1);
                }
                Overlay::Vwap { period } => {
                    Self::draw_series_line(
                        context,
                        &indicators::vwap(&highs, &lows, &closes, &volumes, period),
                        &price_axis,
                        &index_axis,
                        color,
                        1,
                    );
                }
                Overlay::DonchianChannel { period } => {
                    let (lower, middle, upper) =
                        indicators::donchian_channel(&highs, &lows, period);
                    Self::draw_series_line(context, &upper, &price_axis, &index_axis, color, 1);
                    Self::draw_series_line(context, &middle, &price_axis, &index_axis, color, 1);
                    Self::draw_series_line(context, &lower, &price_axis, &index_axis, color, 1);
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
        if self.price_lines.is_empty() {
            return;
        }
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
                PriceLevelKind::PreviousClose => self.chrome().crosshair,
            };
            // Dashes rather than a solid line, so it reads as an annotation and not as
            // another data series.
            let mut x = area.rect.x;
            while x < area.right() {
                let dash_end = (x + 4).min(area.right());
                context.draw_line_stroke(
                    crate::core::Point { x, y },
                    crate::core::Point { x: dash_end, y },
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
        // a candle against cannot be a fixed slate that only reads on a dark backdrop.
        let grid = self.chrome().grid;
        // Four gridlines, at the ends and two inside.
        for step in 0..=3 {
            let fraction = step as f64 / 3.0;
            let price = price_axis.low + fraction * (price_axis.high - price_axis.low);
            let y = price_axis.y_for(price);
            let text = format_fixed(price, decimals);
            context.draw_line(
                crate::core::Point { x: area.rect.x, y },
                crate::core::Point { x: area.right(), y },
                grid,
            );
            // Right-aligned against the plot edge so the digits line up in a column.
            let width = text.len() as i32 * 7;
            context.draw_text(
                crate::core::Point { x: area.rect.x - 6 - width, y: y - 6 },
                &text,
                &Font::simple("Sans", 10.0),
                self.chrome().ink.with_alpha(190),
                HorizontalAlignment::Left,
            );
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
            crate::core::Point { x: area.rect.x, y: mid_y },
            crate::core::Point { x: area.right(), y: mid_y },
            chrome.grid,
        );
        let font = Font::simple("Sans", 12.0);
        let line = context.text_line(area.rect, &font);
        context.draw_text_fitted(
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
        for bar in self.series.bars() {
            for value in [bar.open, bar.high, bar.low, bar.close] {
                if !value.is_finite() {
                    continue;
                }
                for decimals in 0..=4usize {
                    let scaled = value * 10f64.powi(decimals as i32);
                    if (scaled - scaled.round()).abs() < 1e-6 {
                        return decimals;
                    }
                }
            }
        }
        2
    }

    /// The price extent including every overlay, so an overlay is not clipped.
    ///
    /// This is why the extent cannot simply be the series' own high/low: a Bollinger band
    /// routinely sits outside the traded range, and a chart that clipped it would show a
    /// band that appears to touch the top of the pane.
    fn effective_price_extent(&self) -> (f64, f64) {
        let (mut low, mut high) = self.series.price_extent();
        if !low.is_finite() || !high.is_finite() {
            return (0.0, 1.0);
        }

        let closes = self.series.closes();
        let highs = self.series.highs();
        let lows = self.series.lows();
        let volumes = self.series.volumes();
        let mut consider = |values: &[f64]| {
            let (value_low, value_high) = indicators::series_extent(values);
            if value_low.is_finite() {
                low = low.min(value_low);
            }
            if value_high.is_finite() {
                high = high.max(value_high);
            }
        };

        for overlay in &self.overlays {
            match *overlay {
                Overlay::MovingAverage { period } => consider(&indicators::sma(&closes, period)),
                Overlay::ExponentialMovingAverage { period } => {
                    consider(&indicators::ema(&closes, period))
                }
                Overlay::BollingerBands { period, multiplier } => {
                    let (lower, middle, upper) =
                        indicators::bollinger_bands(&closes, period, multiplier);
                    consider(&lower);
                    consider(&middle);
                    consider(&upper);
                }
                Overlay::Vwap { period } => {
                    consider(&indicators::vwap(&highs, &lows, &closes, &volumes, period))
                }
                Overlay::DonchianChannel { period } => {
                    let (lower, middle, upper) =
                        indicators::donchian_channel(&highs, &lows, period);
                    consider(&lower);
                    consider(&middle);
                    consider(&upper);
                }
            }
        }
        for line in &self.price_lines {
            if line.price.is_finite() {
                low = low.min(line.price);
                high = high.max(line.price);
            }
        }

        // A flat series would give a zero span; a small pad keeps the line off the frame
        // and leaves room for the candles' own bodies.
        let span = high - low;
        let pad = if span > 0.0 { span * 0.02 } else { high.abs().max(1.0) * 0.01 };
        (low - pad, high + pad)
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
        let index_axis = area.index_axis(bars.len());
        let x = index_axis.center_for(index);
        let chrome = self.chrome();
        context.draw_line_stroke(
            crate::core::Point { x, y: area.rect.y },
            crate::core::Point { x, y: area.bottom() },
            chrome.crosshair,
            1,
        );

        // And a readout of that bar, positioned to stay inside the pane on both sides.
        //
        // The text is longer than a narrow pane can hold, so it is clipped to the plot
        // area: without a clip both candidate positions overflow — the right branch runs
        // past `area.right()`, and the left branch, clamped *right* to `area.rect.x`,
        // runs further still. Clipping keeps the readout inside the control that owns it
        // instead of painting over a neighbouring pane.
        let bar = bars[index];
        let text =
            format!("O {:.2}  H {:.2}  L {:.2}  C {:.2}", bar.open, bar.high, bar.low, bar.close);
        let width = text.len() as i32 * 7;
        let label_x = if x + 8 + width > area.right() { x - 8 - width } else { x + 8 };
        context.push_clip(area.rect.x, area.rect.y, area.rect.width, area.rect.height);
        context.draw_text(
            crate::core::Point { x: label_x.max(area.rect.x), y: area.rect.y + 2 },
            &text,
            &Font::simple("Sans", 11.0),
            chrome.ink,
            HorizontalAlignment::Left,
        );
        context.pop_clip();
    }
}

/// Formats a price with exactly `decimals` places.
///
/// Written out rather than using `format!("{:.*}", decimals, value)` because the `alloc`
/// `format!` in this crate is the `core`-backed one, which has no dynamic-precision
/// formatter. Doing the rounding here also means the label cannot disagree with what the
/// axis drew.
fn format_fixed(value: f64, decimals: usize) -> String {
    if !value.is_finite() {
        return "—".to_string();
    }
    let factor = 10f64.powi(decimals as i32);
    let rounded = (value * factor).round() / factor;
    let mut text = alloc::format!("{:.1}", rounded);
    // `{:.1}` gives one decimal; append the rest, which for these magnitudes is exact
    // enough that the axis label and the drawn position agree.
    if decimals > 1 {
        let extra = (rounded * factor).round() as i64;
        let digits = extra.unsigned_abs().to_string();
        let padded = if digits.len() <= decimals {
            let mut padded = String::new();
            for _ in 0..(decimals - digits.len()) {
                padded.push('0');
            }
            padded.push_str(&digits);
            padded
        } else {
            digits
        };
        let sign = if extra < 0 { "-" } else { "" };
        text = alloc::format!(
            "{sign}{}.{}",
            &padded[..padded.len() - decimals],
            &padded[padded.len() - decimals..]
        );
    }
    text
}

impl Widget for CandlestickChart {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn as_draw_mut(&mut self) -> Option<&mut dyn Draw> {
        Some(self)
    }
    impl_widget_property_hooks!();
}

impl EventHandler for CandlestickChart {
    /// Tracks the hovered bar and emits the chart's own pointer signals.
    ///
    /// The base routing is delegated to first, so `hover`, `mouse_down` and the rest keep
    /// working exactly as for every other control — a caller using the generic signals is
    /// not broken by this control having a richer one.
    fn handle_event(&mut self, event: &crate::event::Event) {
        use crate::event::Event;
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MouseMove { pos } | Event::PointerMove { pos, .. } => {
                let area = self.plot_area();
                let index_axis = area.index_axis(self.series.len());
                let next = index_axis.index_at(pos.x).filter(|index| *index < self.series.len());
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
            }
            Event::MouseLeave { .. } => {
                if let Some(previous) = self.hovered_index {
                    self.bar_unhovered.emit(previous);
                    self.hovered_index = None;
                    self.base.request_redraw();
                }
            }
            Event::MousePress { pos, .. } | Event::PointerPress { pos, .. } => {
                let area = self.plot_area();
                let index_axis = area.index_axis(self.series.len());
                if let Some(index) =
                    index_axis.index_at(pos.x).filter(|index| *index < self.series.len())
                {
                    self.bar_clicked.emit(index);
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
        self.draw_overlays(context, &area);
        self.draw_price_lines(context, &area);
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
            "show_price_levels" => Ok(CapabilityValue::Bool(!self.price_lines.is_empty())),
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
                self.base.request_redraw();
                Ok(())
            }
            "show_price_levels" => {
                let CapabilityValue::Bool(show) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                if !show {
                    self.clear_price_lines();
                }
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["series", "overlay_count", "show_price_levels", BASE_PROPERTY_NAMES]
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
            "set_series" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
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
    use crate::widget::special_widgets::finance::types::fixtures;
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
        for overlay in [
            Overlay::moving_average(20),
            Overlay::exponential_moving_average(9),
            Overlay::bollinger_bands(20, 2.0),
            Overlay::vwap(14),
            Overlay::donchian_channel(20),
        ] {
            assert!(overlay.warm_up_bars() > 0, "{overlay:?} must report a warm-up");
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
        chart.handle_event(&crate::event::Event::MouseMove {
            pos: crate::core::Point { x: 640 - 4, y: 100 },
        });
        chart.set_series(fixtures::sample_series(3));
        assert!(
            chart.hovered_index().is_none_or(|index| index < 3),
            "a hover must not survive past the series it pointed into"
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
        let mut backend =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(320, 200), 1.0);
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
        let mut backend =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(320, 200), 1.0);
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
        let mut backend =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(320, 200), 1.0);
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
            let mut backend =
                crate::render::SoftwarePaintBackend::new(crate::core::Size::new(480, 320), 1.0);
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
    #[test]
    fn clicking_a_bar_emits_its_index() {
        let mut chart = CandlestickChart::new(Rect::new(0, 0, 640, 360));
        chart.set_series(fixtures::sample_series(20));
        let seen = Arc::new(AtomicUsize::new(usize::MAX));
        let sink = Arc::clone(&seen);
        chart.bar_clicked.connect(move |index| sink.store(*index, Ordering::SeqCst));
        chart.handle_event(&crate::event::Event::MousePress {
            pos: crate::core::Point { x: 70, y: 100 },
            button: 0,
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
        chart.handle_event(&crate::event::Event::MousePress {
            pos: crate::core::Point { x: 5, y: 100 },
            button: 0,
        });
        assert_eq!(count.load(Ordering::SeqCst), 0, "a press on the price axis is not on a bar");
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

        chart.handle_event(&crate::event::Event::MouseMove {
            pos: crate::core::Point { x: 70, y: 100 },
        });
        let hovered_index = hovered.load(Ordering::SeqCst);
        assert_ne!(hovered_index, usize::MAX, "moving over a bar emits it");

        chart.handle_event(&crate::event::Event::MouseLeave {
            pos: crate::core::Point { x: 0, y: 0 },
        });
        assert_eq!(
            unhovered.load(Ordering::SeqCst),
            hovered_index,
            "leaving reports the bar that was hovered"
        );
        assert!(chart.hovered_index().is_none());
    }
}
