// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The indicator pane — MACD, RSI, KDJ and the volume-derived oscillators.
//!
//! # Why one control with a mode rather than four controls
//!
//! These panes differ in *which* series they compute and how they are scaled. They do not
//! differ in anything structural: all of them take a price series, produce one to three
//! derived series, draw them against a y axis, and are read against zero or against a
//! band. Four separate controls would repeat the axis, the scaling, the pointer handling
//! and the signal plumbing four times — which is exactly the duplication principle #23
//! exists to prevent.
//!
//! What is *not* shared is the arithmetic, and that is not duplicated either: every mode
//! calls into `indicators`.
//!
//! # Why the y axis is mode-dependent
//!
//! MACD is unbounded and centred on zero, so its axis scales to the data. RSI and MFI are
//! bounded `0..=100`, so their axis is fixed and the 30/70 bands mean the same thing in
//! every window. KDJ is bounded `0..=100` like RSI but its signal lines are the %K/%D
//! pair. A single auto-scaling axis would make RSI's overbought line move from window to
//! window, which defeats the point of a level that is supposed to be comparable.

use alloc::vec::Vec;

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::render::RenderContext;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::special_widgets::finance::indicators;
use crate::widget::special_widgets::finance::layout::{IndexAxis, PlotArea, PriceAxis};
use crate::widget::special_widgets::finance::types::Bar;
use crate::widget::special_widgets::finance::types::PriceSeries;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Which oscillator the pane shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IndicatorMode {
    /// Moving Average Convergence/Divergence: two lines and a histogram, centred on zero.
    #[default]
    Macd,
    /// Relative Strength Index, bounded `0..=100`.
    Rsi,
    /// Stochastic oscillator, bounded `0..=100`.
    Stochastic,
    /// Money Flow Index, bounded `0..=100` and volume-weighted.
    MoneyFlowIndex,
    /// Average True Range — one line, unbounded, scaling to the data.
    Atr,
    /// On-Balance Volume — one cumulative line, scaling to the data.
    OnBalanceVolume,
}

impl IndicatorMode {
    /// The factory spelling of this mode.
    pub fn as_str(self) -> &'static str {
        match self {
            IndicatorMode::Macd => "macd",
            IndicatorMode::Rsi => "rsi",
            IndicatorMode::Stochastic => "stochastic",
            IndicatorMode::MoneyFlowIndex => "money_flow_index",
            IndicatorMode::Atr => "atr",
            IndicatorMode::OnBalanceVolume => "on_balance_volume",
        }
    }

    /// Parses a factory spelling into a mode.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "macd" => IndicatorMode::Macd,
            "rsi" => IndicatorMode::Rsi,
            "stochastic" | "kdj" => IndicatorMode::Stochastic,
            "money_flow_index" | "mfi" => IndicatorMode::MoneyFlowIndex,
            "atr" => IndicatorMode::Atr,
            "on_balance_volume" | "obv" => IndicatorMode::OnBalanceVolume,
            _ => return None,
        })
    }

    /// Whether this mode's arithmetic needs each bar's traded volume.
    ///
    /// Money Flow Index weights the typical price by volume, and On-Balance Volume *is* a
    /// running total of signed volume — both are meaningless without it. Named so a caller
    /// (or the `series` property) can tell that a price-only series cannot feed them,
    /// rather than discovering it from a blank pane.
    pub fn requires_volume(self) -> bool {
        matches!(self, IndicatorMode::MoneyFlowIndex | IndicatorMode::OnBalanceVolume)
    }

    /// The fixed `0..=100` range for the bounded modes, or `None` when the axis scales
    /// to the data.
    ///
    /// Public because it is the difference between "the 70 line means overbought" and
    /// "the 70 line is where 70% of this window's maximum happens to be"; a caller doing
    /// its own annotation needs to know which it has.
    pub fn fixed_range(self) -> Option<(f64, f64)> {
        match self {
            IndicatorMode::Rsi | IndicatorMode::Stochastic | IndicatorMode::MoneyFlowIndex => {
                Some((0.0, 100.0))
            }
            IndicatorMode::Macd | IndicatorMode::Atr | IndicatorMode::OnBalanceVolume => None,
        }
    }

    /// The overbought and oversold levels to draw, for the bounded modes.
    ///
    /// 70/30 for RSI and stochastic, 80/20 for MFI — the conventional settings. Drawn as
    /// dashed horizontals so a reader can see at a glance whether the reading is in a
    /// band rather than having to remember where the bands are.
    pub fn reference_levels(self) -> &'static [f64] {
        match self {
            IndicatorMode::Rsi | IndicatorMode::Stochastic => &[30.0, 70.0],
            IndicatorMode::MoneyFlowIndex => &[20.0, 80.0],
            IndicatorMode::Macd => &[0.0],
            IndicatorMode::Atr | IndicatorMode::OnBalanceVolume => &[],
        }
    }
}

/// One drawn series in the pane.
///
/// Kept as `(values, colour, width)` triples rather than three parallel vectors, so a
/// series cannot lose its colour when the set is rebuilt — the failure mode of parallel
/// arrays that this crate's `PriceSeries` avoids for the same reason.
struct PaneSeries {
    values: Vec<f64>,
    color: Color,
    width: u32,
}

/// The indicator pane control.
pub struct IndicatorChart {
    base: BaseWidget,
    series: PriceSeries,
    mode: IndicatorMode,
    /// The MACD fast/slow/signal periods, the standard 12/26/9.
    macd_periods: (usize, usize, usize),
    /// The lookback for RSI, MFI and ATR.
    period: usize,
    /// The stochastic `%K` smoothing and `%D` period, the standard 3/3.
    stochastic_periods: (usize, usize),
    show_reference_levels: bool,
    hovered_index: Option<usize>,
    /// The values last computed, kept so a caller can read the numbers behind the picture
    /// without recomputing them — and so a tooltip and the drawn line cannot disagree.
    last_values: Vec<Vec<f64>>,
    /// Emitted with the bar index when the pane is clicked.
    pub bar_clicked: crate::signal::Signal1<usize>,
    /// Emitted with the bar index as the pointer moves across bars.
    pub bar_hovered: crate::signal::Signal1<usize>,
}

impl IndicatorChart {
    /// Creates an empty MACD pane.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::IndicatorChart, geometry, "IndicatorChart"),
            series: PriceSeries::new(),
            mode: IndicatorMode::default(),
            macd_periods: (12, 26, 9),
            period: 14,
            stochastic_periods: (3, 3),
            show_reference_levels: true,
            hovered_index: None,
            last_values: Vec::new(),
            bar_clicked: crate::signal::Signal1::new(),
            bar_hovered: crate::signal::Signal1::new(),
        }
    }

    /// The series being analysed.
    pub fn series(&self) -> &PriceSeries {
        &self.series
    }

    /// Replaces the series and requests a repaint.
    pub fn set_series(&mut self, series: PriceSeries) {
        self.series = series;
        if self.hovered_index.is_some_and(|index| index >= self.series.len()) {
            self.hovered_index = None;
        }
        self.base.request_redraw();
    }

    /// The active mode.
    pub fn mode(&self) -> IndicatorMode {
        self.mode
    }

    /// Sets the mode and requests a repaint.
    pub fn set_mode(&mut self, mode: IndicatorMode) {
        self.mode = mode;
        self.base.request_redraw();
    }

    /// The MACD fast, slow and signal periods.
    pub fn macd_periods(&self) -> (usize, usize, usize) {
        self.macd_periods
    }

    /// Sets the MACD periods. A zero period falls back to the default for that position,
    /// because a zero-period EMA is the identity and would draw the price itself.
    pub fn set_macd_periods(&mut self, fast: usize, slow: usize, signal: usize) {
        self.macd_periods = (
            if fast == 0 { 12 } else { fast },
            if slow == 0 { 26 } else { slow },
            if signal == 0 { 9 } else { signal },
        );
        self.base.request_redraw();
    }

    /// The lookback period used by RSI, MFI and ATR.
    pub fn period(&self) -> usize {
        self.period
    }

    /// Sets the lookback period, floored at 1.
    ///
    /// A period of zero would make every indicator return all gaps, which is a blank pane
    /// with no explanation; one is the smallest period that produces any output at all.
    pub fn set_period(&mut self, period: usize) {
        self.period = period.max(1);
        self.base.request_redraw();
    }

    /// The stochastic `%K` smoothing and `%D` period.
    pub fn stochastic_periods(&self) -> (usize, usize) {
        self.stochastic_periods
    }

    /// Sets the stochastic smoothing periods.
    pub fn set_stochastic_periods(&mut self, k_smoothing: usize, d_period: usize) {
        self.stochastic_periods = (k_smoothing.max(1), d_period.max(1));
        self.base.request_redraw();
    }

    /// Whether the reference levels are drawn.
    pub fn shows_reference_levels(&self) -> bool {
        self.show_reference_levels
    }

    /// Shows or hides the overbought/oversold reference levels.
    pub fn set_show_reference_levels(&mut self, show: bool) {
        self.show_reference_levels = show;
        self.base.request_redraw();
    }

    /// The bar index under the pointer.
    pub fn hovered_index(&self) -> Option<usize> {
        self.hovered_index
    }

    /// The series last drawn, in draw order.
    ///
    /// Empty until the pane has drawn at least once, because the values depend on the
    /// series and the mode. A caller wanting them without waiting for a frame can call
    /// [`Self::compute`].
    pub fn last_values(&self) -> &[Vec<f64>] {
        &self.last_values
    }

    /// Computes the active mode's series without drawing.
    ///
    /// Split out from `draw` so a caller can read the numbers — for a table, an alert or a
    /// test — without needing a render target. `draw` calls this, so the two cannot
    /// disagree about what is on screen.
    pub fn compute(&self) -> Vec<Vec<f64>> {
        let closes = self.series.closes();
        let highs = self.series.highs();
        let lows = self.series.lows();
        let volumes = self.series.volumes();
        match self.mode {
            IndicatorMode::Macd => {
                let (fast, slow, signal) = self.macd_periods;
                let (line, signal_line, histogram) = indicators::macd(&closes, fast, slow, signal);
                alloc::vec![line, signal_line, histogram]
            }
            IndicatorMode::Rsi => alloc::vec![indicators::rsi(&closes, self.period)],
            IndicatorMode::Stochastic => {
                let (k_smoothing, d_period) = self.stochastic_periods;
                let (k, d) = indicators::stochastic(
                    &highs,
                    &lows,
                    &closes,
                    self.period,
                    k_smoothing,
                    d_period,
                );
                alloc::vec![k, d]
            }
            IndicatorMode::MoneyFlowIndex => {
                alloc::vec![indicators::money_flow_index(
                    &highs,
                    &lows,
                    &closes,
                    &volumes,
                    self.period
                )]
            }
            IndicatorMode::Atr => alloc::vec![indicators::atr(&highs, &lows, &closes, self.period)],
            IndicatorMode::OnBalanceVolume => {
                alloc::vec![indicators::on_balance_volume(&closes, &volumes)]
            }
        }
    }

    /// The coloured series to draw, in order, with the histogram last so it sits behind.
    fn pane_series(&self) -> Vec<PaneSeries> {
        let values = self.compute();
        let palette = match self.mode {
            IndicatorMode::Macd => alloc::vec![
                Color::rgb(33, 150, 243),
                Color::rgb(255, 193, 7),
                Color::rgb(120, 144, 156),
            ],
            IndicatorMode::Rsi | IndicatorMode::MoneyFlowIndex | IndicatorMode::Atr => {
                alloc::vec![Color::rgb(33, 150, 243)]
            }
            IndicatorMode::Stochastic => {
                alloc::vec![Color::rgb(33, 150, 243), Color::rgb(255, 193, 7)]
            }
            IndicatorMode::OnBalanceVolume => alloc::vec![Color::rgb(156, 39, 176)],
        };
        values
            .into_iter()
            .enumerate()
            .map(|(index, series)| PaneSeries {
                values: series,
                color: palette[index % palette.len()],
                width: 1,
            })
            .collect()
    }

    /// The axis range for the active mode.
    fn axis_range(&self, series: &[PaneSeries]) -> (f64, f64) {
        if let Some(fixed) = self.mode.fixed_range() {
            return fixed;
        }
        let mut low = f64::INFINITY;
        let mut high = f64::NEG_INFINITY;
        for pane in series {
            let (series_low, series_high) = indicators::series_extent(&pane.values);
            if series_low.is_finite() {
                low = low.min(series_low);
            }
            if series_high.is_finite() {
                high = high.max(series_high);
            }
        }
        if !low.is_finite() || !high.is_finite() {
            return (0.0, 1.0);
        }
        // A zero-centred mode must keep zero visible or the sign of the reading — which is
        // the entire message of a MACD histogram — is off the pane.
        if self.mode == IndicatorMode::Macd {
            let magnitude = low.abs().max(high.abs());
            if magnitude > 0.0 {
                return (-magnitude, magnitude);
            }
        }
        let span = high - low;
        let pad = if span > 0.0 { span * 0.05 } else { high.abs().max(1.0) * 0.01 };
        (low - pad, high + pad)
    }

    /// Draws a dashed horizontal reference level.
    fn draw_reference_line(
        context: &mut RenderContext,
        area: &PlotArea,
        price_axis: &PriceAxis,
        level: f64,
    ) {
        let y = price_axis.y_for(level);
        let mut x = area.rect.x;
        while x < area.right() {
            let dash_end = (x + 4).min(area.right());
            context.draw_line_stroke(
                Point { x, y },
                Point { x: dash_end, y },
                Color::rgb(88, 96, 108),
                1,
            );
            x += 8;
        }
    }

    /// Draws one derived series, breaking at gaps.
    fn draw_series(
        context: &mut RenderContext,
        pane: &PaneSeries,
        index_axis: &IndexAxis,
        price_axis: &PriceAxis,
    ) {
        let mut previous: Option<(i32, i32)> = None;
        for (index, value) in pane.values.iter().enumerate() {
            if !value.is_finite() || index >= index_axis.count {
                previous = None;
                continue;
            }
            let point = (index_axis.center_for(index), price_axis.y_for(*value));
            if let Some(from) = previous {
                context.draw_line_stroke(
                    Point { x: from.0, y: from.1 },
                    Point { x: point.0, y: point.1 },
                    pane.color,
                    pane.width,
                );
            }
            previous = Some(point);
        }
    }

    /// Draws the MACD histogram as signed columns from the zero line.
    ///
    /// Only for `Macd`, because the histogram is the third series and its meaning is its
    /// sign. Drawing it as a third line would lose that: a line crossing zero twice per
    /// cycle does not read as "momentum building" the way a growing column does.
    fn draw_histogram(
        &self,
        context: &mut RenderContext,
        series: &[PaneSeries],
        index_axis: &IndexAxis,
        price_axis: &PriceAxis,
    ) {
        if self.mode != IndicatorMode::Macd {
            return;
        }
        let Some(histogram) = series.get(2) else {
            return;
        };
        let zero_y = price_axis.y_for(0.0);
        let body_width = index_axis.body_width();
        for (index, value) in histogram.values.iter().enumerate() {
            if !value.is_finite() || index >= index_axis.count {
                continue;
            }
            let y = price_axis.y_for(*value);
            let top = y.min(zero_y);
            let height = (y - zero_y).unsigned_abs().max(1);
            context.fill_rect(
                Rect::new(index_axis.x_for(index), top, body_width as u32, height),
                if *value >= 0.0 { Color::rgb(38, 166, 91) } else { Color::rgb(220, 68, 70) },
            );
        }
    }

    /// Draws the pane's left-hand value labels.
    fn draw_labels(&self, context: &mut RenderContext, area: &PlotArea, price_axis: &PriceAxis) {
        let (low, high) = (price_axis.low, price_axis.high);
        for step in 0..=3 {
            let fraction = step as f64 / 3.0;
            let value = low + fraction * (high - low);
            let y = price_axis.y_for(value);
            // Two decimals throughout: an indicator's own scale is not a price, so
            // inferring precision from the data would give an unbounded axis like MACD a
            // changing number of decimals as the window scrolls.
            let text = alloc::format!("{value:.2}");
            let width = text.chars().count() as i32 * 7;
            context.draw_text(
                Point { x: area.rect.x - 6 - width, y: y - 6 },
                &text,
                &Font::simple("Sans", 10.0),
                Color::rgb(150, 160, 172),
                HorizontalAlignment::Left,
            );
        }
    }

    /// The plot area, leaving room for the value labels.
    fn plot_area(&self) -> PlotArea {
        PlotArea::with_margins(self.base.geometry(), 52, 8, 8, 8)
    }
}

impl Widget for IndicatorChart {
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

impl crate::event::EventHandler for IndicatorChart {
    fn handle_event(&mut self, event: &crate::event::Event) {
        use crate::event::Event;
        match event {
            Event::MouseMove { pos } | Event::PointerMove { pos, .. } => {
                let area = self.plot_area();
                let index_axis = area.index_axis(self.series.len());
                let next = index_axis.index_at(pos.x).filter(|index| *index < self.series.len());
                if next != self.hovered_index {
                    if let Some(index) = next {
                        self.bar_hovered.emit(index);
                    }
                    self.hovered_index = next;
                    self.base.request_redraw();
                }
            }
            Event::MouseLeave { .. } if self.hovered_index.take().is_some() => {
                self.base.request_redraw();
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

impl Draw for IndicatorChart {
    fn draw(&mut self, context: &mut RenderContext) {
        let area = self.plot_area();
        if area.rect.width == 0 || area.rect.height == 0 {
            return;
        }
        context.fill_rect(area.rect, Color::rgb(18, 22, 28));
        if self.series.is_empty() {
            return;
        }

        let series = self.pane_series();
        let (low, high) = self.axis_range(&series);
        let price_axis = area.price_axis(low, high);
        let index_axis = area.index_axis(self.series.len());

        // The histogram first, so the lines are drawn over it.
        self.draw_histogram(context, &series, &index_axis, &price_axis);

        if self.show_reference_levels {
            for level in self.mode.reference_levels() {
                Self::draw_reference_line(context, &area, &price_axis, *level);
            }
        }

        // The MACD histogram is also the third series; drawing it as a line too would
        // trace a column chart with a polyline and make both harder to read.
        let line_count = if self.mode == IndicatorMode::Macd { 2 } else { series.len() };
        for pane in series.iter().take(line_count) {
            Self::draw_series(context, pane, &index_axis, &price_axis);
        }

        self.draw_labels(context, &area, &price_axis);

        if let Some(index) = self.hovered_index.filter(|index| *index < self.series.len()) {
            let x = index_axis.center_for(index);
            context.draw_line_stroke(
                Point { x, y: area.rect.y },
                Point { x, y: area.bottom() },
                Color::rgb(120, 120, 120),
                1,
            );
            // The readings at that bar, one per drawn series.
            let mut text = alloc::string::String::new();
            for (position, pane) in series.iter().enumerate() {
                if let Some(value) = pane.values.get(index).filter(|value| value.is_finite()) {
                    if !text.is_empty() {
                        text.push_str("  ");
                    }
                    text.push_str(&alloc::format!("{:.2}", value));
                    let _ = position;
                }
            }
            if !text.is_empty() {
                let width = text.chars().count() as i32 * 7;
                let label_x = if x + 8 + width > area.right() { x - 8 - width } else { x + 8 };
                context.draw_text(
                    Point { x: label_x.max(area.rect.x), y: area.rect.y + 2 },
                    &text,
                    &Font::simple("Sans", 11.0),
                    Color::rgb(230, 230, 230),
                    HorizontalAlignment::Left,
                );
            }
        }

        // The values are kept so a caller can read them without recomputing.
        self.last_values = series.into_iter().map(|pane| pane.values).collect();
    }
}

/// Test module for the indicator pane.
impl WidgetProperties for IndicatorChart {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "mode" => Ok(CapabilityValue::String(alloc::string::String::from(self.mode.as_str()))),
            "period" => Ok(CapabilityValue::UInt(self.period as u64)),
            "show_reference_levels" => Ok(CapabilityValue::Bool(self.show_reference_levels)),
            "series" => {
                let mut text = alloc::string::String::new();
                for (index, bar) in self.series.bars().iter().enumerate() {
                    if index > 0 {
                        text.push(',');
                    }
                    text.push_str(&alloc::format!("{}", bar.close));
                }
                Ok(CapabilityValue::String(text))
            }
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "mode" => {
                let CapabilityValue::String(text) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                let mode =
                    IndicatorMode::from_name(&text).ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_mode(mode);
                Ok(())
            }
            "period" => {
                let CapabilityValue::UInt(period) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                self.set_period(period as usize);
                Ok(())
            }
            "show_reference_levels" => {
                let CapabilityValue::Bool(show) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                self.set_show_reference_levels(show);
                Ok(())
            }
            "series" => {
                let CapabilityValue::String(text) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                // The property carries closes only, so the bars it builds have no volume.
                // A volume-weighted mode read off such a series is undefined (MFI produces
                // `NAN` throughout, OBV a flat zero line), and silently accepting the write
                // would leave the caller with a blank pane and no error. Refusing names the
                // problem instead: the property cannot express what the mode needs.
                if self.mode.requires_volume() {
                    return Err(CapabilityAccessError::UnsupportedOnWidget);
                }
                let mut series = PriceSeries::new();
                for token in text.split([',', ' ', ';']).filter(|token| !token.is_empty()) {
                    let price: f64 =
                        token.trim().parse().map_err(|_| CapabilityAccessError::OutOfRange)?;
                    series.push(Bar::flat(price, 0.0));
                }
                self.set_series(series);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["mode", "period", "show_reference_levels", "series", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `indicator_chart` publishes.
    ///
    /// The pane's own studies are selected through `mode` and `period`, not stacked
    /// as overlays, so `add_overlay` — whose method exists only on
    /// `CandlestickChart` — is answered as
    /// [`CapabilityAccessError::UnsupportedOnWidget`]. That refusal is an honest
    /// answer, not a fix: the `commands` list still advertises an action this control
    /// cannot take, and reconciling it is a registry decision. The `set_*` write names
    /// carry their values.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "add_overlay" => Err(CapabilityAccessError::UnsupportedOnWidget),
            _ if name.starts_with("set_") => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::special_widgets::finance::types::fixtures;

    /// The pane carries the series it was given.
    #[test]
    fn an_indicator_pane_carries_its_series() {
        let mut chart = IndicatorChart::new(Rect::new(0, 0, 480, 160));
        assert!(chart.series().is_empty());
        chart.set_series(fixtures::sample_series(60));
        assert_eq!(chart.series().len(), 60);
    }

    /// MACD is the default mode, as it is the most common secondary pane.
    #[test]
    fn macd_is_the_default_mode() {
        let chart = IndicatorChart::new(Rect::new(0, 0, 480, 160));
        assert_eq!(chart.mode(), IndicatorMode::Macd);
        assert_eq!(chart.macd_periods(), (12, 26, 9), "the standard periods");
        assert_eq!(chart.period(), 14);
        assert_eq!(chart.stochastic_periods(), (3, 3));
    }

    /// Every mode spelling round-trips, including the common aliases.
    #[test]
    fn mode_names_round_trip() {
        for mode in [
            IndicatorMode::Macd,
            IndicatorMode::Rsi,
            IndicatorMode::Stochastic,
            IndicatorMode::MoneyFlowIndex,
            IndicatorMode::Atr,
            IndicatorMode::OnBalanceVolume,
        ] {
            let name = mode.as_str();
            assert_eq!(IndicatorMode::from_name(name), Some(mode), "{name} must round-trip");
        }
        assert_eq!(IndicatorMode::from_name("kdj"), Some(IndicatorMode::Stochastic));
        assert_eq!(IndicatorMode::from_name("mfi"), Some(IndicatorMode::MoneyFlowIndex));
        assert_eq!(IndicatorMode::from_name("obv"), Some(IndicatorMode::OnBalanceVolume));
        assert_eq!(IndicatorMode::from_name("nonsense"), None);
    }

    /// The bounded modes declare a fixed range and the unbounded ones do not.
    ///
    /// This is the difference between "the 70 line means overbought" and "the 70 line is
    /// wherever 70% of this window happens to fall", so it is a contract rather than an
    /// implementation detail.
    #[test]
    fn bounded_modes_have_a_fixed_range() {
        for mode in [IndicatorMode::Rsi, IndicatorMode::Stochastic, IndicatorMode::MoneyFlowIndex] {
            assert_eq!(mode.fixed_range(), Some((0.0, 100.0)), "{mode:?} is bounded");
        }
        for mode in [IndicatorMode::Macd, IndicatorMode::Atr, IndicatorMode::OnBalanceVolume] {
            assert_eq!(mode.fixed_range(), None, "{mode:?} scales to its data");
        }
    }

    /// The reference levels are the conventional settings per mode.
    #[test]
    fn reference_levels_follow_the_mode() {
        assert_eq!(IndicatorMode::Rsi.reference_levels(), &[30.0, 70.0]);
        assert_eq!(IndicatorMode::Stochastic.reference_levels(), &[30.0, 70.0]);
        assert_eq!(IndicatorMode::MoneyFlowIndex.reference_levels(), &[20.0, 80.0]);
        assert_eq!(IndicatorMode::Macd.reference_levels(), &[0.0]);
        assert!(IndicatorMode::Atr.reference_levels().is_empty());
    }

    /// MACD computes three series, one of which is the histogram.
    #[test]
    fn macd_computes_three_series() {
        let mut chart = IndicatorChart::new(Rect::new(0, 0, 480, 160));
        chart.set_series(fixtures::sample_series(120));
        let values = chart.compute();
        assert_eq!(values.len(), 3, "the line, the signal and the histogram");
        for series in &values {
            assert_eq!(series.len(), 120, "every series must align with the bars");
        }
    }

    /// Every mode computes at least one series of the right length.
    #[test]
    fn every_mode_computes_aligned_series() {
        for mode in [
            IndicatorMode::Macd,
            IndicatorMode::Rsi,
            IndicatorMode::Stochastic,
            IndicatorMode::MoneyFlowIndex,
            IndicatorMode::Atr,
            IndicatorMode::OnBalanceVolume,
        ] {
            let mut chart = IndicatorChart::new(Rect::new(0, 0, 480, 160));
            chart.set_series(fixtures::sample_series(90));
            chart.set_mode(mode);
            let values = chart.compute();
            assert!(!values.is_empty(), "{mode:?} must compute something");
            for series in &values {
                assert_eq!(series.len(), 90, "{mode:?} must align with the bars");
            }
        }
    }

    /// RSI's output stays inside the mode's declared range.
    ///
    /// The declared range and the arithmetic must agree, or the pane would clamp a value
    /// that is already correct and draw a flat line at the edge.
    #[test]
    fn rsi_output_fits_its_declared_range() {
        let mut chart = IndicatorChart::new(Rect::new(0, 0, 480, 160));
        chart.set_series(fixtures::sample_series(120));
        chart.set_mode(IndicatorMode::Rsi);
        let values = chart.compute();
        for value in values[0].iter().filter(|value| value.is_finite()) {
            assert!((0.0..=100.0).contains(value), "RSI out of its declared range: {value}");
        }
    }

    /// A zero period is floored at one rather than producing a blank pane.
    #[test]
    fn a_zero_period_is_floored() {
        let mut chart = IndicatorChart::new(Rect::new(0, 0, 480, 160));
        chart.set_period(0);
        assert_eq!(chart.period(), 1, "a zero period would make every indicator blank");
        chart.set_stochastic_periods(0, 0);
        assert_eq!(chart.stochastic_periods().1, 1, "a zero %D period is not a period");
    }

    /// A zero MACD period falls back to the standard for that position.
    #[test]
    fn a_zero_macd_period_falls_back() {
        let mut chart = IndicatorChart::new(Rect::new(0, 0, 480, 160));
        chart.set_macd_periods(0, 0, 0);
        assert_eq!(chart.macd_periods(), (12, 26, 9), "a zero-period EMA is the identity");
    }

    /// The reference levels can be hidden.
    #[test]
    fn reference_levels_can_be_hidden() {
        let mut chart = IndicatorChart::new(Rect::new(0, 0, 480, 160));
        assert!(chart.shows_reference_levels(), "shown by default");
        chart.set_show_reference_levels(false);
        assert!(!chart.shows_reference_levels());
    }

    /// An empty series draws without panicking.
    #[test]
    fn an_empty_series_draws_without_panicking() {
        let mut chart = IndicatorChart::new(Rect::new(0, 0, 320, 160));
        let mut backend =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(320, 160), 1.0);
        let mut context = RenderContext::new(&mut backend);
        chart.draw(&mut context);
    }

    /// Every mode draws with a real series.
    #[test]
    fn every_mode_draws() {
        for mode in [
            IndicatorMode::Macd,
            IndicatorMode::Rsi,
            IndicatorMode::Stochastic,
            IndicatorMode::MoneyFlowIndex,
            IndicatorMode::Atr,
            IndicatorMode::OnBalanceVolume,
        ] {
            let mut chart = IndicatorChart::new(Rect::new(0, 0, 400, 160));
            chart.set_series(fixtures::sample_series(80));
            chart.set_mode(mode);
            let mut backend =
                crate::render::SoftwarePaintBackend::new(crate::core::Size::new(400, 160), 1.0);
            let mut context = RenderContext::new(&mut backend);
            chart.draw(&mut context);
        }
    }

    /// A malformed series draws without panicking in every mode.
    #[test]
    fn a_malformed_series_draws_in_every_mode() {
        let mut series = crate::widget::special_widgets::finance::types::PriceSeries::new();
        for index in 0..30 {
            if index % 7 == 0 {
                series.push(crate::widget::special_widgets::finance::types::Bar::new(
                    f64::NAN,
                    f64::NAN,
                    f64::NAN,
                    f64::NAN,
                    f64::NAN,
                ));
            } else {
                series.push(crate::widget::special_widgets::finance::types::Bar::flat(
                    100.0 + index as f64,
                    0.0,
                ));
            }
        }
        for mode in [
            IndicatorMode::Macd,
            IndicatorMode::Rsi,
            IndicatorMode::Stochastic,
            IndicatorMode::MoneyFlowIndex,
            IndicatorMode::Atr,
            IndicatorMode::OnBalanceVolume,
        ] {
            let mut chart = IndicatorChart::new(Rect::new(0, 0, 400, 160));
            chart.set_series(series.clone());
            chart.set_mode(mode);
            let mut backend =
                crate::render::SoftwarePaintBackend::new(crate::core::Size::new(400, 160), 1.0);
            let mut context = RenderContext::new(&mut backend);
            chart.draw(&mut context);
        }
    }

    /// The values a caller reads match what was computed.
    ///
    /// `last_values` exists so a tooltip and the drawn line cannot disagree; if it held
    /// something other than `compute`'s output, that guarantee would be false.
    ///
    /// Compared element-wise rather than with `assert_eq!`, because every indicator is
    /// `NAN` through its warm-up and `NAN != NAN` — a whole-slice comparison would fail
    /// on the first gap of a series that is in fact identical.
    #[test]
    fn the_reported_values_match_the_computed_ones() {
        let mut chart = IndicatorChart::new(Rect::new(0, 0, 400, 160));
        chart.set_series(fixtures::sample_series(60));
        chart.set_mode(IndicatorMode::Rsi);
        let expected = chart.compute();
        let mut backend =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(400, 160), 1.0);
        let mut context = RenderContext::new(&mut backend);
        chart.draw(&mut context);

        let reported = chart.last_values();
        assert_eq!(reported.len(), expected.len(), "the series count must match");
        for (reported_series, expected_series) in reported.iter().zip(expected.iter()) {
            assert_eq!(reported_series.len(), expected_series.len());
            for (index, (left, right)) in
                reported_series.iter().zip(expected_series.iter()).enumerate()
            {
                if left.is_nan() && right.is_nan() {
                    continue;
                }
                assert!(
                    (left - right).abs() < 1e-9,
                    "index {index}: reported {left} but computed {right}"
                );
            }
        }
        assert!(
            reported[0].iter().any(|value| value.is_finite()),
            "the fixture must produce at least one real reading, or this proves nothing"
        );
    }
}
