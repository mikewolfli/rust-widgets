// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//!
//! The `TimelineAxis` control and its date-label vocabulary.

use rust_widgets::core::{Color, Font, HorizontalAlignment, Point, Rect};
use rust_widgets::event::{Event, EventHandler};
use rust_widgets::render::RenderContext;
use rust_widgets::signal::Signal1;
use rust_widgets::widget::capability::coercion::{
    expect_f32, expect_i64, expect_string, expect_u32,
};
use rust_widgets::widget::capability::properties_trait::{base_property_get, base_property_set};
use rust_widgets::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use rust_widgets::widget::capability::WidgetProperties;
use rust_widgets::widget::metrics::effective_font;
use rust_widgets::widget::{BaseWidget, Draw, Widget, WidgetKind};
use rust_widgets::{impl_widget_property_hooks, property_names_of};

use super::common::chart_chrome_palette;
use rust_widgets::impl_draw_bridge;

/// Smallest magnification a [`TimelineAxis`] accepts.
pub const AXIS_MIN_ZOOM: f32 = 0.25;
/// Largest magnification a [`TimelineAxis`] accepts.
pub const AXIS_MAX_ZOOM: f32 = 64.0;

/// Horizontal padding between the control's edge and the axis track.
const AXIS_PADDING: i32 = 8;
/// Height of a tick mark.
const AXIS_TICK: i32 = 4;

/// How a [`TimelineAxis`] renders the label of a tick.
///
/// The axis unit is the caller's (see `GanttTask`), so only two interpretations
/// are offered: print the value, or read it as days since the Unix epoch. A format
/// string language would silently assume a unit the chart does not own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DateFormat {
    /// The axis value as a plain integer (sprints, quarters, …).
    #[default]
    Numeric,
    /// Days since 1970-01-01 rendered as `YYYY-MM-DD`.
    IsoDate,
}

impl DateFormat {
    /// The stable property token.
    pub fn token(self) -> &'static str {
        match self {
            Self::Numeric => "numeric",
            Self::IsoDate => "iso",
        }
    }

    /// Parses a property token.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "numeric" => Some(Self::Numeric),
            "iso" => Some(Self::IsoDate),
            _ => None,
        }
    }
}

/// A date axis that converts between pixels and axis values.
///
/// The chart's own axis labels are bare integers (`GanttWidget` formats the tick
/// value with `to_string`). This control owns the other half of that contract: a
/// caller can ask which date a pixel stands for, and where a date falls, without
/// re-deriving the chart's margins. `range` is authoritative — `zoom_level` is the
/// magnification the host requested, kept for display and for the next zoom step.
pub struct TimelineAxis {
    base: BaseWidget,
    tick_density: u32,
    date_format: DateFormat,
    /// Visible axis values, `start ..= end`.
    range: (i64, i64),
    /// Magnification relative to `base_span`.
    zoom_level: f32,
    /// The span `zoom_level == 1.0` refers to.
    base_span: f64,
    /// Tick positions, labels and label widths, with the key they were built for.
    ///
    /// Every tick used to be formatted and measured on every frame, which made a scrub or a zoom
    /// pay for the whole axis per animation step.
    tick_entries: Vec<(i32, String, i32)>,
    tick_key: Option<(i64, i64, u32, &'static str, Font, u32)>,
    /// Emitted with the new visible range after a range or zoom change.
    pub scale_changed: Signal1<(i64, i64)>,
}

impl TimelineAxis {
    /// Creates an axis over `0 ..= 100` with six ticks.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::TimelineAxis, geometry, "TimelineAxis"),
            tick_density: 6,
            date_format: DateFormat::Numeric,
            range: (0, 100),
            zoom_level: 1.0,
            base_span: 100.0,
            tick_entries: Vec::new(),
            tick_key: None,
            scale_changed: Signal1::new(),
        }
    }

    /// Returns the visible range.
    pub fn range(&self) -> (i64, i64) {
        self.range
    }

    /// Sets the visible range and returns the magnification to 1:1.
    ///
    /// A reversed range is widened to a single unit, mirroring `GanttWidget`'s
    /// viewport rule; an unchanged range at 1:1 emits nothing.
    pub fn set_range(&mut self, start: i64, end: i64) {
        let end = end.max(start.saturating_add(1));
        let unchanged = self.range == (start, end) && (self.zoom_level - 1.0).abs() < f32::EPSILON;
        if unchanged {
            return;
        }
        self.range = (start, end);
        self.base_span = (end - start).max(1) as f64;
        self.zoom_level = 1.0;
        self.scale_changed.emit(self.range);
        self.base.request_redraw();
    }

    /// Returns the magnification.
    pub fn zoom_level(&self) -> f32 {
        self.zoom_level
    }

    /// Magnifies the axis around its centre, clamped to
    /// [`AXIS_MIN_ZOOM`]` ..= `[`AXIS_MAX_ZOOM`].
    ///
    /// Returns `false` when the value is non-finite or unchanged. The span is
    /// `base_span / zoom_level`, so zooming is symmetric: 2.0 shows half the range
    /// that 1.0 showed, regardless of how many steps it took to get there.
    pub fn set_zoom_level(&mut self, zoom_level: f32) -> bool {
        if !zoom_level.is_finite() {
            return false;
        }
        let level = zoom_level.clamp(AXIS_MIN_ZOOM, AXIS_MAX_ZOOM);
        if (self.zoom_level - level).abs() < f32::EPSILON {
            return false;
        }
        let center = (self.range.0 as f64 + self.range.1 as f64) / 2.0;
        let span = (self.base_span / f64::from(level)).max(1.0);
        let start = (center - span / 2.0).round() as i64;
        let end = (center + span / 2.0).round() as i64;
        self.zoom_level = level;
        self.range = (start, end.max(start.saturating_add(1)));
        self.scale_changed.emit(self.range);
        self.base.request_redraw();
        true
    }

    /// Returns the requested tick count.
    pub fn tick_density(&self) -> u32 {
        self.tick_density
    }

    /// Sets the tick count, clamped to `1 ..= 32`.
    pub fn set_tick_density(&mut self, tick_density: u32) {
        let density = tick_density.clamp(1, 32);
        if self.tick_density == density {
            return;
        }
        self.tick_density = density;
        self.base.request_redraw();
    }

    /// Returns how tick labels are formatted.
    pub fn date_format(&self) -> DateFormat {
        self.date_format
    }

    /// Sets how tick labels are formatted.
    pub fn set_date_format(&mut self, date_format: DateFormat) {
        if self.date_format == date_format {
            return;
        }
        self.date_format = date_format;
        self.base.request_redraw();
    }

    /// Formats one axis value for a tick label.
    pub fn format_tick(&self, value: i64) -> String {
        match self.date_format {
            DateFormat::Numeric => value.to_string(),
            DateFormat::IsoDate => iso_date_from_days(value),
        }
    }

    /// The ticks the axis paints, as `(value, x)` pairs.
    pub fn ticks(&self) -> Vec<(i64, i32)> {
        let density = i64::from(self.tick_density.max(1));
        let (start, end) = self.range;
        let span = (end - start).max(1);
        let mut ticks: Vec<(i64, i32)> = Vec::new();
        for index in 0..=density {
            let value = if index == density { end } else { start + span * index / density };
            if ticks.last().is_some_and(|(previous, _)| *previous == value) {
                continue;
            }
            ticks.push((value, self.date_to_pixel(value)));
        }
        ticks
    }

    /// Converts an x coordinate to the axis value it stands for.
    ///
    /// `x` outside the track is clamped to the range's ends; the mapping is the
    /// inverse of [`Self::date_to_pixel`] up to rounding.
    pub fn pixel_to_date(&self, x: i32) -> i64 {
        let (left, width) = self.track_geometry();
        let span = (self.range.1 - self.range.0).max(1) as f64;
        let ratio = (f64::from(x - left) / f64::from(width)).clamp(0.0, 1.0);
        self.range.0 + (ratio * span).round() as i64
    }

    /// Converts an axis value to an x coordinate.
    ///
    /// Values outside the range project outside the track rather than clamping, so
    /// a caller can tell "off the axis" from "at the edge".
    pub fn date_to_pixel(&self, date: i64) -> i32 {
        let (left, width) = self.track_geometry();
        let span = (self.range.1 - self.range.0).max(1) as f64;
        let ratio = (date - self.range.0) as f64 / span;
        left + (ratio * f64::from(width)).round() as i32
    }

    /// The track's left edge and width.
    fn track_geometry(&self) -> (i32, u32) {
        let rect = self.geometry();
        (rect.x + AXIS_PADDING, rect.width.saturating_sub(2 * AXIS_PADDING as u32).max(1))
    }
}

/// Renders `days` since 1970-01-01 as `YYYY-MM-DD`.
///
/// Uses Howard Hinnant's `civil_from_days` decomposition, which is exact for the
/// whole `i64` range and needs no calendar table.
fn iso_date_from_days(days: i64) -> String {
    let shifted = days.saturating_add(719_468);
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted - era * 146_097; // 0 ..= 146096
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153; // 0 ..= 11
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 { month_prime + 3 } else { month_prime - 9 };
    if month <= 2 {
        year += 1;
    }
    format!("{:04}-{:02}-{:02}", year, month, day)
}

impl Widget for TimelineAxis {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn capability_name(&self) -> Option<&'static str> {
        Some("timeline_axis")
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn set_state_theme_hook(&mut self) {
        rust_widgets::style::reapply_active_theme_state(self);
    }

    fn size_hint(&self) -> rust_widgets::core::Size {
        rust_widgets::core::Size::new(320, 24)
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();

    fn event_signal_dyn(&self, name: &str) -> Option<rust_widgets::signal::EventSignalRef> {
        use rust_widgets::signal::EventSignalRef;
        match name {
            "scale_changed" => {
                Some(EventSignalRef::mapped("scale_changed", &self.scale_changed, |range| {
                    CapabilityValue::Tuple(alloc::vec![
                        CapabilityValue::Int(range.0),
                        CapabilityValue::Int(range.1),
                    ])
                }))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for TimelineAxis {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "tick_density" => Ok(CapabilityValue::UInt(u64::from(self.tick_density))),
            "date_format" => Ok(CapabilityValue::String(self.date_format.token().to_string())),
            "zoom_level" => Ok(CapabilityValue::Float(f64::from(self.zoom_level))),
            "range" => Ok(CapabilityValue::Tuple(alloc::vec![
                CapabilityValue::Int(self.range.0),
                CapabilityValue::Int(self.range.1),
            ])),
            "range_start" => Ok(CapabilityValue::Int(self.range.0)),
            "range_end" => Ok(CapabilityValue::Int(self.range.1)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "tick_density" => {
                self.set_tick_density(expect_u32(value)?);
                Ok(())
            }
            "date_format" => {
                let token = expect_string(value)?;
                let format =
                    DateFormat::from_token(&token).ok_or(CapabilityAccessError::TypeMismatch)?;
                self.set_date_format(format);
                Ok(())
            }
            "zoom_level" => {
                self.set_zoom_level(expect_f32(value)?);
                Ok(())
            }
            "range_start" => {
                self.set_range(expect_i64(value)?, self.range.1);
                Ok(())
            }
            "range_end" => {
                self.set_range(self.range.0, expect_i64(value)?);
                Ok(())
            }
            "range" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "tick_density",
            "date_format",
            "zoom_level",
            "range",
            "range_start",
            "range_end",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl EventHandler for TimelineAxis {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        if let Event::Wheel { delta, .. } = event {
            if delta.y < 0 {
                self.set_zoom_level(self.zoom_level * 1.25);
            } else if delta.y > 0 {
                self.set_zoom_level(self.zoom_level / 1.25);
            }
        }
    }
}

impl Draw for TimelineAxis {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let style = self.base.style();
        let (surface, border, ink) = chart_chrome_palette(style, "timeline_axis");
        context.face_with_gradient(rect, surface, None, Default::default(), 0, Color::BLACK);
        let (left, width) = self.track_geometry();
        let baseline = rect.y + rect.height as i32 - 1;
        context.draw_line(
            Point::new(left, baseline),
            Point::new(left + width as i32, baseline),
            border,
        );
        let font = effective_font(style);
        // The labels and their advances are a pure function of the range, the density, the format,
        // the face and the track width; those form the cache key and nothing else is measured
        // while they hold.
        let key = (
            self.range.0,
            self.range.1,
            self.tick_density,
            self.date_format.token(),
            font.clone(),
            width,
        );
        if self.tick_key.as_ref() != Some(&key) {
            let entries = self
                .ticks()
                .into_iter()
                .map(|(value, x)| {
                    let label = self.format_tick(value);
                    let label_width = context.measure_text(&label, font).width as i32;
                    (x, label, label_width)
                })
                .collect();
            self.tick_entries = entries;
            self.tick_key = Some(key);
        }
        for (x, label, label_width) in &self.tick_entries {
            let x = *x;
            context.draw_line(Point::new(x, baseline - AXIS_TICK), Point::new(x, baseline), border);
            if !label.is_empty() {
                let text_x =
                    (x + 2).clamp(rect.x, (rect.x + rect.width as i32 - *label_width).max(rect.x));
                context.draw_text(
                    Point::new(text_x, rect.y + 2),
                    label,
                    font,
                    ink,
                    HorizontalAlignment::Left,
                );
            }
        }
    }
}
