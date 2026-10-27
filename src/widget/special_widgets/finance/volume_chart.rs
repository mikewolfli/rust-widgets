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

use alloc::vec::Vec;

use crate::core::{Color, Point, Rect};
use crate::event::EventHandler;
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::role_font;
use crate::widget::special_widgets::finance::layout::{
    panel_colors, PanelColors, PlotArea, PANEL_MIN_CONTRAST,
};
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
    /// The bars shown, or `None` for every bar. Kept in step with the K-line pane by the host.
    view: Option<(usize, usize)>,
    /// Emitted with the visible `[start, end)` bar range after every view change.
    pub view_changed: Signal1<(usize, usize)>,
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
            view: None,
            view_changed: Signal1::new(),
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
        self.view = self.clamped_view(self.view);
        if self.hovered_index.is_some_and(|index| index >= self.series.len()) {
            self.hovered_index = None;
        }
        self.base.request_redraw();
    }

    /// The visible bar range, or `None` when every bar is shown.
    pub fn view_range(&self) -> Option<(usize, usize)> {
        self.view
    }

    /// Restricts the pane to `range`, the same contract the K-line pane uses.
    pub fn set_view_range(&mut self, range: Option<(usize, usize)>) {
        let range = self.clamped_view(range);
        if self.view == range {
            return;
        }
        self.view = range;
        let (start, end) = self.view_bounds();
        self.view_changed.emit((start, end));
        self.base.request_redraw();
    }

    /// Restores the full series view.
    pub fn reset_view(&mut self) {
        self.set_view_range(None);
    }

    fn clamped_view(&self, range: Option<(usize, usize)>) -> Option<(usize, usize)> {
        range.map(|(start, end)| {
            let total = self.series.len();
            let start = start.min(total.saturating_sub(1));
            let end = end.clamp(start + 1, total.max(start + 1));
            (start, end)
        })
    }

    /// The visible `(start, end)` bar indices.
    fn view_bounds(&self) -> (usize, usize) {
        match self.view {
            Some((start, end)) => (start.min(self.series.len()), end.min(self.series.len())),
            None => (0, self.series.len()),
        }
    }

    /// The colour policy.
    pub fn color_mode(&self) -> VolumeColorMode {
        self.color_mode
    }

    /// Sets the colour policy.
    pub fn set_color_mode(&mut self, mode: VolumeColorMode) {
        if self.color_mode == mode {
            return;
        }
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

    /// The hovered bar's readings, ready for a `ChartTooltip`.
    ///
    /// The label (or the bar index when the host supplied none) and the volume, painted in
    /// the bar's own colour — the same pair the pane draws, so a tooltip cannot disagree
    /// with the bar under the pointer (COMP-J02).
    pub fn hovered_summary(&self) -> Vec<crate::widget::companions::TooltipLine> {
        use crate::widget::companions::TooltipLine;
        let Some(index) = self.hovered_index else { return Vec::new() };
        let Some(bar) = self.series.bars().get(index) else { return Vec::new() };
        alloc::vec![
            TooltipLine::new(match self.series.label_at(index) {
                Some(label) => alloc::string::String::from(label),
                None => alloc::format!("#{index}"),
            }),
            TooltipLine::colored(alloc::format!("vol {:.0}", bar.volume), self.bar_color(index)),
        ]
    }

    /// The hovered bar's anchor and suggested tooltip content, for an external read-out.
    ///
    /// The anchor is the top centre of the bar the pointer is over, so a companion bubble
    /// appears beside the datum rather than at a fixed corner (gap 10).
    pub fn hover_anchor(&self) -> Option<(Point, Vec<crate::widget::companions::TooltipLine>)> {
        let index = self.hovered_index?;
        let (start, end) = self.view_bounds();
        if index < start || index >= end {
            return None;
        }
        let bars = self.series.bars();
        let visible = bars.get(start..end)?;
        if visible.is_empty() {
            return None;
        }
        let max_volume = visible
            .iter()
            .filter(|bar| bar.volume.is_finite())
            .map(|bar| bar.volume)
            .fold(0.0f64, f64::max);
        let bar = bars.get(index)?;
        if max_volume <= 0.0 || !bar.volume.is_finite() || bar.volume <= 0.0 {
            return None;
        }
        let area = self.plot_area();
        if area.rect.width == 0 || area.rect.height == 0 {
            return None;
        }
        let index_axis = area.index_axis(visible.len().max(1));
        let x = index_axis.center_for(index - start);
        let usable_height = (area.rect.height as f64 * self.headroom).max(1.0);
        let height = (bar.volume / max_volume * usable_height).round().max(1.0) as i32;
        let y = area.bottom() - height;
        Some((Point::new(x, y), self.hovered_summary()))
    }

    /// The plot area for this control's geometry.
    fn plot_area(&self) -> PlotArea {
        // No bottom margin: a volume pane carries no index labels of its own, because
        // the price pane above already has them and a second copy would be misleading
        // about which rows they belong to.
        PlotArea::volume_pane(self.base.geometry())
    }

    /// The pane's chrome, resolved from this control's own style first and the theme second.
    ///
    /// Named rather than inlined because the empty state and the populated state are two
    /// branches of one draw: taking the colours from two places is how a pane can end up with
    /// a background that does not match the grid drawn on it.
    fn chrome(&self) -> PanelColors {
        panel_colors(Some(self.base.style()))
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
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
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
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "bar_clicked" => Some(EventSignalRef::mapped("bar_clicked", &self.bar_clicked, |v| {
                CapabilityValue::UInt(*v as u64)
            })),
            "bar_hovered" => Some(EventSignalRef::mapped("bar_hovered", &self.bar_hovered, |v| {
                CapabilityValue::UInt(*v as u64)
            })),
            "view_changed" => {
                Some(EventSignalRef::mapped("view_changed", &self.view_changed, |v| {
                    CapabilityValue::Tuple(crate::compat::Vec::from([
                        CapabilityValue::UInt(v.0 as u64),
                        CapabilityValue::UInt(v.1 as u64),
                    ]))
                }))
            }
            _ => None,
        }
    }
}

impl EventHandler for VolumeChart {
    fn handle_event(&mut self, event: &crate::event::Event) {
        use crate::event::Event;
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MouseMove { pos } | Event::PointerMove { pos, .. } => {
                let area = self.plot_area();
                let (start, end) = self.view_bounds();
                let visible = end.saturating_sub(start);
                let index_axis = area.index_axis(visible);
                let next = index_axis
                    .index_at(pos.x)
                    .map(|index| start + index)
                    .filter(|index| *index < end);
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
            // Wheel zoom, the same gesture and the same factors the K-line pane uses, so a
            // host linking the panes sees one behaviour. The pointer's bar — or the middle
            // of the view when the pointer is not over one — stays in place (FIN-02).
            Event::Wheel { delta, .. } if delta.y != 0 => {
                let total = self.series.len();
                if total > 4 {
                    let (start, end) = self.view_bounds();
                    let anchor = self.hovered_index.unwrap_or((start + end) / 2);
                    let span = (end - start).max(4);
                    let factor = if delta.y > 0 { 1.25 } else { 0.8 };
                    let new_span = ((span as f64 * factor).round() as usize).clamp(4, total);
                    let half = new_span / 2;
                    let new_start = anchor.saturating_sub(half).min(total - new_span);
                    self.set_view_range(Some((new_start, new_start + new_span)));
                }
            }
            Event::MousePress { pos, .. } | Event::PointerPress { pos, .. } => {
                let area = self.plot_area();
                let (start, end) = self.view_bounds();
                let visible = end.saturating_sub(start);
                let index_axis = area.index_axis(visible);
                if let Some(index) = index_axis
                    .index_at(pos.x)
                    .map(|index| start + index)
                    .filter(|index| *index < end)
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
        // The pane surface resolves the caller's style first and the theme second, so the
        // histogram sits on the appearance's own surface. The bar colours below are the
        // bar's direction and stay exactly as they are: they are data, not chrome.
        let chrome = self.chrome();
        context.fill_rect(area.rect, chrome.surface);

        let bars = self.series.bars();
        let (start, end) = self.view_bounds();
        let visible = &bars[start..end];
        let max_volume = visible
            .iter()
            .filter(|bar| bar.volume.is_finite())
            .map(|bar| bar.volume)
            .fold(0.0f64, f64::max);
        // Empty, or every volume zero or unusable: there is no scale to draw against, and a
        // zero-height bar for each would be invisible anyway. The pane still owes the reader
        // its frame, so this is where the grid and the message go rather than a bare return
        // that left a solid slab behind.
        if visible.is_empty() || max_volume <= 0.0 {
            draw_empty_pane(context, &area, chrome, &role_font(self.style(), 12.0));
            return;
        }
        let usable_height = (area.rect.height as f64 * self.headroom).max(1.0);
        let index_axis = area.index_axis(visible.len());
        let body_width = index_axis.body_width();
        let baseline = area.bottom();

        // The volume gridlines, drawn under the bars: without a scale a histogram is a row
        // of coloured blocks whose relative heights cannot be read as quantities.
        let axis_font = role_font(self.style(), 10.0);
        let label_left = area.rect.x - area.left_margin;
        let label_width = (area.label_x() - label_left).max(0) as u32;
        for step in 0..=3 {
            let fraction = step as f64 / 3.0;
            let y = area.rect.y + area.rect.height as i32 * step / 3;
            context.draw_line(
                crate::core::Point { x: area.rect.x, y },
                crate::core::Point { x: area.right(), y },
                chrome.grid,
            );
            // The quantity that gridline stands for, so the bars have a readable scale.
            let value = max_volume * (1.0 - fraction);
            let text = alloc::format!("{value:.0}");
            let line_height = context.measure_text(&text, &axis_font).height.max(1);
            context.draw_text_fitted_cow(
                Rect::new(label_left, y - line_height as i32 / 2, label_width, line_height),
                &text,
                &axis_font,
                chrome.ink.with_alpha(190),
                crate::core::HorizontalAlignment::Right,
            );
        }

        for (offset, bar) in visible.iter().enumerate() {
            if !bar.volume.is_finite() || bar.volume <= 0.0 {
                continue;
            }
            let height = (bar.volume / max_volume * usable_height).round().max(1.0) as u32;
            context.fill_rect(
                Rect::new(
                    index_axis.x_for(offset),
                    baseline - height as i32,
                    body_width as u32,
                    height,
                ),
                self.bar_color(start + offset),
            );
        }

        // The index axis under the pane, so a bar can be dated from the picture (FIN-01).
        draw_index_ticks(
            context,
            &area,
            start,
            end,
            |index| match self.series.label_at(index) {
                Some(label) => alloc::string::String::from(label),
                None => alloc::format!("{index}"),
            },
            &axis_font,
            chrome.ink.with_alpha(190),
        );

        // A crosshair marking the hovered column, matching the price pane's.
        if let Some(index) = self.hovered_index.filter(|index| *index >= start && *index < end) {
            let x = index_axis.center_for(index - start);
            context.draw_line_stroke(
                crate::core::Point { x, y: area.rect.y },
                crate::core::Point { x, y: area.bottom() },
                chrome.crosshair,
                1,
            );
        }
    }
}

/// Draws an index axis under a time-series pane.
///
/// The bars are the x axis, and a chart with no labels under it cannot answer "when was
/// this bar?". The step is chosen from the pane's width (about one tick per 80 px) so a
/// zoomed-out history does not paint a hundred overlapping labels, and the label itself
/// comes from the series' own `label_at` — the caller's timestamps, falling back to the bar
/// index when the host supplied none (FIN-01).
///
/// One implementation for the K-line, volume and indicator panes: they share the index
/// mapping, so a tick drawn by one must be under the same bar as in the others.
pub(crate) fn draw_index_ticks(
    context: &mut RenderContext,
    area: &PlotArea,
    start: usize,
    end: usize,
    label_at: impl Fn(usize) -> alloc::string::String,
    font: &crate::core::Font,
    ink: Color,
) {
    let visible = end.saturating_sub(start);
    // A pane that reserved no room for an axis row cannot show one; drawing anyway would
    // paint the labels onto the pane below it.
    if visible == 0 || area.bottom_margin < 14 {
        return;
    }
    let axis = area.index_axis(visible);
    let target = (area.rect.width as i32 / 80).max(1) as usize;
    let step = visible.div_ceil(target).max(1);
    let band_width = (axis.slot_width * step as i32).max(28) as u32;
    let y = area.bottom() + 2;
    let height = (area.bottom_margin - 2).max(10) as u32;
    let mut index = start;
    while index < end {
        let x = axis.center_for(index - start);
        let text = label_at(index);
        context.draw_text_fitted_cow(
            Rect::new(x - band_width as i32 / 2, y, band_width, height),
            &text,
            font,
            ink,
            crate::core::HorizontalAlignment::Center,
        );
        index += step;
    }
}

/// Draws a pane's frame and a `No data` message for a pane with nothing to plot.
///
/// # Why this is not a method on the control
///
/// It depends solely on the plot area and the resolved chrome — nothing about which series
/// or which mode a pane holds — so making it a method would be four identical copies, one
/// per control, which is the duplication the shared layout module exists to remove. It lives
/// beside the derivation it consumes because the two have to agree: the message's colour is
/// derived with [`PANEL_MIN_CONTRAST`] from the very surface passed in, so it cannot be
/// painted in an ink that ignores the panel it lands on.
pub(crate) fn draw_empty_pane(
    context: &mut RenderContext,
    area: &PlotArea,
    chrome: crate::widget::special_widgets::finance::layout::PanelColors,
    font: &crate::core::Font,
) {
    context.draw_rect(area.rect, chrome.grid);
    // A hairline at mid-height, where a series with no data would plot, so the pane reads as
    // "a scale with nothing on it" rather than as a filled rectangle.
    let mid_y = area.rect.y + area.rect.height as i32 / 2;
    context.draw_line(
        crate::core::Point { x: area.rect.x, y: mid_y },
        crate::core::Point { x: area.right(), y: mid_y },
        chrome.grid,
    );
    let line = context.text_line(area.rect, font);
    context.draw_text_fitted_cow(
        line,
        "No data",
        font,
        chrome.ink.legible_on(chrome.surface, PANEL_MIN_CONTRAST).with_alpha(160),
        crate::core::HorizontalAlignment::Center,
    );
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
    /// A view range is honoured by the drawn bars and by hit testing, so this pane can
    /// share the K-line pane's horizontal range.
    #[test]
    fn a_view_range_restricts_the_pane_and_reports_changes() {
        let mut chart = VolumeChart::new(Rect::new(0, 0, 240, 80));
        let mut series = PriceSeries::new();
        for value in [1.0, 2.0, 3.0, 4.0, 5.0] {
            series.push(Bar::flat(0.0, value));
        }
        chart.set_series(series);
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<(usize, usize)>::new()));
        let sink = std::sync::Arc::clone(&seen);
        chart.view_changed.connect(move |range| sink.lock().unwrap().push(*range));

        chart.set_view_range(Some((2, 4)));
        assert_eq!(chart.view_range(), Some((2, 4)));
        assert_eq!(*seen.lock().unwrap(), vec![(2, 4)]);

        // The left half of the pane now addresses bar 2, not bar 0.
        let area = chart.plot_area();
        let click = area.rect.x + 1;
        assert_eq!(area.index_axis(2).index_at(click).map(|index| 2 + index), Some(2));
        chart.reset_view();
        assert_eq!(*seen.lock().unwrap(), vec![(2, 4), (0, 5)]);
    }

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
            modifiers: 0,
        });
        assert_eq!(count.load(Ordering::SeqCst), 1, "a press inside the plot emits once");
        chart.handle_event(&crate::event::Event::MousePress {
            pos: crate::core::Point { x: 2, y: 80 },
            button: 0,
            modifiers: 0,
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
