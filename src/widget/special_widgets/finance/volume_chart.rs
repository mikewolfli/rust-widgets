// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The volume pane — the histogram that sits under a K-line chart.
//!
//! # Why it takes the price series rather than a list of volumes
//!
//! A volume bar's colour is the direction of its own price bar: green when the bar
//! closed up, red when it closed down. Given only the volumes, a caller would have to
//! pass a parallel colour vector and keep it aligned — the off-by-one this crate's
//! `PriceSeries` exists to prevent. Taking the series means the direction is read from the
//! same bar the volume came from, so it cannot disagree.
//!
//! # Why it shares the index axis rather than owning one
//!
//! A volume pane is only readable against the price pane above it: bar 17's volume must
//! sit under bar 17's candle. Both derive their `IndexAxis` from the same
//! [`PlotArea::index_axis`] with the same count, so the alignment is arithmetic rather
//! than a convention two widgets must keep.

use crate::core::{Color, Rect};
use crate::event::EventHandler;
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::special_widgets::finance::layout::PlotArea;
use crate::widget::special_widgets::finance::types::Bar;
use crate::widget::special_widgets::finance::types::PriceSeries;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Which colour a volume bar is drawn in.
///
/// An enum with two variants rather than a `bool`, because it selects a *policy* — how
/// the colour is decided — and a caller reading `set_color_mode(VolumeColorMode::Close)`
/// learns what it does, where `set_use_close(true)` does not say so.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VolumeColorMode {
    /// Green for an up bar, red for a down bar — the usual convention.
    #[default]
    Direction,
    /// A single flat colour, for a pane where the candle colours are already doing the
    /// directional work and a second colour scale would be noise.
    Uniform,
}

impl VolumeColorMode {
    /// The factory spelling of this mode.
    pub fn as_str(self) -> &'static str {
        match self {
            VolumeColorMode::Direction => "direction",
            VolumeColorMode::Uniform => "uniform",
        }
    }

    /// Parses a factory spelling into a mode.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "direction" => VolumeColorMode::Direction,
            "uniform" => VolumeColorMode::Uniform,
            _ => return None,
        })
    }
}

/// The volume histogram control.
pub struct VolumeChart {
    base: BaseWidget,
    series: PriceSeries,
    color_mode: VolumeColorMode,
    /// The volume the pane scales against, as a fraction of the maximum.
    ///
    /// Below `1.0` so the tallest bar does not touch the top of the pane, which reads as
    /// clipped. A caller can raise it to `1.0` for a pane with no headroom wanted.
    headroom: f64,
    hovered_index: Option<usize>,
    /// Emitted with the bar index when a volume bar is clicked.
    pub bar_clicked: Signal1<usize>,
    /// Emitted with the bar index as the pointer moves across bars.
    pub bar_hovered: Signal1<usize>,
}

impl VolumeChart {
    /// Creates an empty volume pane.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::VolumeChart, geometry, "VolumeChart"),
            series: PriceSeries::new(),
            color_mode: VolumeColorMode::default(),
            headroom: 0.92,
            hovered_index: None,
            bar_clicked: Signal1::new(),
            bar_hovered: Signal1::new(),
        }
    }

    /// The series being drawn.
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

    /// The colour policy.
    pub fn color_mode(&self) -> VolumeColorMode {
        self.color_mode
    }

    /// Sets the colour policy.
    pub fn set_color_mode(&mut self, mode: VolumeColorMode) {
        self.color_mode = mode;
        self.base.request_redraw();
    }

    /// The fraction of the pane height the tallest bar occupies.
    pub fn headroom(&self) -> f64 {
        self.headroom
    }

    /// Sets the headroom fraction, clamped to `0.1..=1.0`.
    ///
    /// Clamped rather than validated with a `Result`: a headroom outside that range
    /// produces a pane that is unusably small or overflows its frame, and neither is a
    /// caller mistake worth halting over — the useful range is what gets applied.
    pub fn set_headroom(&mut self, headroom: f64) {
        if headroom.is_finite() {
            self.headroom = headroom.clamp(0.1, 1.0);
            self.base.request_redraw();
        }
    }

    /// The bar index under the pointer.
    pub fn hovered_index(&self) -> Option<usize> {
        self.hovered_index
    }

    /// The plot area for this control's geometry.
    fn plot_area(&self) -> PlotArea {
        // No bottom margin: a volume pane carries no index labels of its own, because
        // the price pane above already has them and a second copy would be misleading
        // about which rows they belong to.
        PlotArea::with_margins(self.base.geometry(), 48, 8, 6, 6)
    }

    /// A volume bar's colour under the current policy.
    fn bar_color(&self, index: usize) -> Color {
        match self.color_mode {
            VolumeColorMode::Uniform => Color::rgb(96, 125, 139),
            VolumeColorMode::Direction => {
                let rising = self.series.bars().get(index).is_some_and(|bar| bar.is_rising());
                if rising {
                    Color::rgb(38, 166, 91)
                } else {
                    Color::rgb(220, 68, 70)
                }
            }
        }
    }
}

impl Widget for VolumeChart {
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

impl EventHandler for VolumeChart {
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
            Event::MouseLeave { .. } => {
                if self.hovered_index.take().is_some() {
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

impl Draw for VolumeChart {
    fn draw(&mut self, context: &mut RenderContext) {
        let area = self.plot_area();
        if area.rect.width == 0 || area.rect.height == 0 {
            return;
        }
        context.fill_rect(area.rect, Color::rgb(18, 22, 28));

        let bars = self.series.bars();
        if bars.is_empty() {
            return;
        }
        let max_volume = self.series.max_volume();
        if max_volume <= 0.0 {
            // Every volume is zero or unusable: there is no scale to draw against, and a
            // zero-height bar for each would be invisible anyway.
            return;
        }
        let usable_height = (area.rect.height as f64 * self.headroom).max(1.0);
        let index_axis = area.index_axis(bars.len());
        let body_width = index_axis.body_width();
        let baseline = area.bottom();

        for (index, bar) in bars.iter().enumerate() {
            if !bar.volume.is_finite() || bar.volume <= 0.0 {
                continue;
            }
            let height = (bar.volume / max_volume * usable_height).round().max(1.0) as u32;
            context.fill_rect(
                Rect::new(
                    index_axis.x_for(index),
                    baseline - height as i32,
                    body_width as u32,
                    height,
                ),
                self.bar_color(index),
            );
        }

        // A crosshair marking the hovered column, matching the price pane's.
        if let Some(index) = self.hovered_index.filter(|index| *index < bars.len()) {
            let x = index_axis.center_for(index);
            context.draw_line_stroke(
                crate::core::Point { x, y: area.rect.y },
                crate::core::Point { x, y: area.bottom() },
                Color::rgb(120, 120, 120),
                1,
            );
        }
    }
}

/// Test module for the volume pane.
impl WidgetProperties for VolumeChart {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "series" => {
                let mut text = alloc::string::String::new();
                for (index, bar) in self.series.bars().iter().enumerate() {
                    if index > 0 {
                        text.push(',');
                    }
                    text.push_str(&alloc::format!("{}", bar.volume));
                }
                Ok(CapabilityValue::String(text))
            }
            "color_mode" => {
                Ok(CapabilityValue::String(alloc::string::String::from(self.color_mode.as_str())))
            }
            "headroom" => Ok(CapabilityValue::Float(self.headroom)),
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
                    let volume: f64 =
                        token.trim().parse().map_err(|_| CapabilityAccessError::OutOfRange)?;
                    series.push(Bar::flat(0.0, volume));
                }
                self.set_series(series);
                Ok(())
            }
            "color_mode" => {
                let CapabilityValue::String(text) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                let mode =
                    VolumeColorMode::from_name(&text).ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_color_mode(mode);
                Ok(())
            }
            "headroom" => {
                // Accepts both a float and a uint, because `0.75` and `1` are both
                // natural ways to write a fraction and refusing one would be a
                // type-system detail leaking into the API.
                let headroom = match value {
                    CapabilityValue::Float(value) => value,
                    CapabilityValue::UInt(value) => value as f64,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_headroom(headroom);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["series", "color_mode", "headroom", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `volume_chart` publishes.
    ///
    /// `add_overlay` is published by every pane in this module, but the method only
    /// exists on `CandlestickChart` — overlays are price-space studies, and this pane
    /// plots volume, which has no price scale to compute a moving average against. It
    /// is refused as [`CapabilityAccessError::UnsupportedOnWidget`]: the name is real
    /// and this control cannot perform it. (The dispatcher maps the trait default's
    /// `UnknownCommand` to that same error before the caller sees it, so a bare
    /// fallback would report value as well; naming the case here is about intent, and
    /// about keeping the refusal of a name this control genuinely has separate from
    /// the fallback for names it has never heard of.)
    ///
    /// The refusal is an honest answer for a caller, not a fix: the capability's
    /// `commands` list still advertises an action this control cannot take, and
    /// reconciling that list is a registry decision rather than something a dispatch
    /// arm can resolve without contradicting the task's "do not change `commands`"
    /// constraint. The `set_series` / `set_color_mode` / `set_headroom` write names
    /// carry their values and are refused as [`CapabilityAccessError::OutOfRange`].
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "add_overlay" => Err(CapabilityAccessError::UnsupportedOnWidget),
            "set_series" | "set_color_mode" | "set_headroom" => {
                Err(CapabilityAccessError::OutOfRange)
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::EventHandler;
    use crate::widget::special_widgets::finance::types::fixtures;
    use crate::widget::special_widgets::finance::types::{Bar, PriceSeries};
    use alloc::sync::Arc;
    use core::sync::atomic::{AtomicUsize, Ordering};

    /// The pane carries the series it was given.
    #[test]
    fn a_volume_pane_carries_its_series() {
        let mut chart = VolumeChart::new(Rect::new(0, 0, 480, 160));
        assert!(chart.series().is_empty());
        chart.set_series(fixtures::sample_series(30));
        assert_eq!(chart.series().len(), 30);
    }

    /// The colour mode defaults to direction and can be changed.
    ///
    /// `Direction` is the default because it is the convention on every platform, and a
    /// pane that came up uniform would silently lose the up/down reading.
    #[test]
    fn the_colour_mode_defaults_to_direction() {
        let mut chart = VolumeChart::new(Rect::new(0, 0, 480, 160));
        assert_eq!(chart.color_mode(), VolumeColorMode::Direction);
        chart.set_color_mode(VolumeColorMode::Uniform);
        assert_eq!(chart.color_mode(), VolumeColorMode::Uniform);
    }

    /// Both colour-mode spellings round-trip.
    #[test]
    fn colour_mode_names_round_trip() {
        for mode in [VolumeColorMode::Direction, VolumeColorMode::Uniform] {
            let name = mode.as_str();
            assert_eq!(VolumeColorMode::from_name(name), Some(mode));
        }
        assert_eq!(VolumeColorMode::from_name("nonsense"), None);
    }

    /// Headroom is clamped to a usable range.
    ///
    /// The clamp is the contract: a headroom of zero would make every bar zero pixels
    /// tall, and one above 1.0 would draw outside the pane. Neither is a caller mistake
    /// worth refusing outright, so the useful range is what gets applied.
    #[test]
    fn headroom_is_clamped() {
        let mut chart = VolumeChart::new(Rect::new(0, 0, 480, 160));
        chart.set_headroom(0.0);
        assert!(chart.headroom() >= 0.1, "zero headroom would hide every bar");
        chart.set_headroom(5.0);
        assert!(chart.headroom() <= 1.0, "headroom above 1.0 would overflow the pane");
        chart.set_headroom(0.75);
        assert!((chart.headroom() - 0.75).abs() < 1e-9);
    }

    /// A non-finite headroom is ignored rather than applied.
    #[test]
    fn a_non_finite_headroom_is_ignored() {
        let mut chart = VolumeChart::new(Rect::new(0, 0, 480, 160));
        let before = chart.headroom();
        chart.set_headroom(f64::NAN);
        assert_eq!(chart.headroom(), before, "a NaN headroom must not replace a good one");
    }

    /// An empty series draws without panicking.
    #[test]
    fn an_empty_series_draws_without_panicking() {
        let mut chart = VolumeChart::new(Rect::new(0, 0, 320, 160));
        let mut backend =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(320, 160), 1.0);
        let mut context = RenderContext::new(&mut backend);
        chart.draw(&mut context);
    }

    /// A series whose volumes are all zero draws without dividing by zero.
    ///
    /// This is what a halted instrument's pane looks like, and the maximum volume is the
    /// divisor for every bar height.
    #[test]
    fn an_all_zero_volume_series_draws_without_panicking() {
        let mut series = PriceSeries::new();
        for _ in 0..10 {
            series.push(Bar::new(10.0, 11.0, 9.0, 10.5, 0.0));
        }
        let mut chart = VolumeChart::new(Rect::new(0, 0, 320, 160));
        chart.set_series(series);
        let mut backend =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(320, 160), 1.0);
        let mut context = RenderContext::new(&mut backend);
        chart.draw(&mut context);
    }

    /// A malformed series draws without panicking.
    #[test]
    fn a_malformed_series_draws_without_panicking() {
        let mut series = PriceSeries::new();
        series.push(Bar::new(10.0, 11.0, 9.0, 10.5, f64::NAN));
        series.push(Bar::new(10.0, 11.0, 9.0, 9.5, f64::INFINITY));
        series.push(Bar::new(10.0, 11.0, 9.0, 10.0, -5.0));
        let mut chart = VolumeChart::new(Rect::new(0, 0, 320, 160));
        chart.set_series(series);
        let mut backend =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(320, 160), 1.0);
        let mut context = RenderContext::new(&mut backend);
        chart.draw(&mut context);
    }

    /// Every colour mode draws.
    #[test]
    fn every_colour_mode_draws() {
        for mode in [VolumeColorMode::Direction, VolumeColorMode::Uniform] {
            let mut chart = VolumeChart::new(Rect::new(0, 0, 320, 160));
            chart.set_series(fixtures::sample_series(40));
            chart.set_color_mode(mode);
            let mut backend =
                crate::render::SoftwarePaintBackend::new(crate::core::Size::new(320, 160), 1.0);
            let mut context = RenderContext::new(&mut backend);
            chart.draw(&mut context);
        }
    }

    /// Clicking a bar emits its index; clicking outside emits nothing.
    #[test]
    fn clicks_report_only_inside_the_plot() {
        let mut chart = VolumeChart::new(Rect::new(0, 0, 480, 160));
        chart.set_series(fixtures::sample_series(20));
        let count = Arc::new(AtomicUsize::new(0));
        let sink = Arc::clone(&count);
        chart.bar_clicked.connect(move |_| {
            sink.fetch_add(1, Ordering::SeqCst);
        });
        chart.handle_event(&crate::event::Event::MousePress {
            pos: crate::core::Point { x: 300, y: 80 },
            button: 0,
        });
        assert_eq!(count.load(Ordering::SeqCst), 1, "a press inside the plot emits once");
        chart.handle_event(&crate::event::Event::MousePress {
            pos: crate::core::Point { x: 2, y: 80 },
            button: 0,
        });
        assert_eq!(count.load(Ordering::SeqCst), 1, "a press on the axis emits nothing");
    }

    /// Replacing the series drops a hover that no longer points at a bar.
    #[test]
    fn replacing_the_series_drops_a_stale_hover() {
        let mut chart = VolumeChart::new(Rect::new(0, 0, 480, 160));
        chart.set_series(fixtures::sample_series(40));
        chart.handle_event(&crate::event::Event::MouseMove {
            pos: crate::core::Point { x: 470, y: 80 },
        });
        chart.set_series(fixtures::sample_series(2));
        assert!(
            chart.hovered_index().is_none_or(|index| index < 2),
            "a hover must not survive past its series"
        );
    }
}
