// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//!
//! The `ScaleBar` distance legend and its unit/style vocabulary.

use super::common::companion_palette;
use rust_widgets::core::{Color, HorizontalAlignment, Point, Rect};
use rust_widgets::event::{Event, EventHandler};
use rust_widgets::impl_draw_bridge;
use rust_widgets::render::RenderContext;
use rust_widgets::widget::capability::coercion::{expect_f64, expect_string};
use rust_widgets::widget::capability::properties_trait::{base_property_get, base_property_set};
use rust_widgets::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use rust_widgets::widget::capability::WidgetProperties;
use rust_widgets::widget::metrics::effective_font;
use rust_widgets::widget::{BaseWidget, Draw, Widget, WidgetKind};
use rust_widgets::{impl_widget_property_hooks, property_names_of};

/// The unit system a [`ScaleBar`] labels its distance in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScaleUnits {
    /// Metres and kilometres.
    #[default]
    Metric,
    /// Feet and miles.
    Imperial,
}

impl ScaleUnits {
    /// The stable property token.
    pub fn token(self) -> &'static str {
        match self {
            Self::Metric => "metric",
            Self::Imperial => "imperial",
        }
    }

    /// Parses a property token.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "metric" => Some(Self::Metric),
            "imperial" => Some(Self::Imperial),
            _ => None,
        }
    }
}

/// How a [`ScaleBar`] frames its bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScaleBarStyle {
    /// A bare bar and label, for drawing directly on a map.
    #[default]
    Plain,
    /// A bordered box behind the bar, for use over busy tile imagery.
    Boxed,
}

impl ScaleBarStyle {
    /// The stable property token.
    pub fn token(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::Boxed => "boxed",
        }
    }

    /// Parses a property token.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "plain" => Some(Self::Plain),
            "boxed" => Some(Self::Boxed),
            _ => None,
        }
    }
}

/// Padding kept clear at both ends of a [`ScaleBar`].
const SCALE_BAR_PADDING: f64 = 8.0;

/// A distances legend for a map.
///
/// The bar length comes from `meters_per_pixel`, rounded down to a 1/2/5 × 10ⁿ
/// value so the label is a number a reader can hold in their head.
pub struct ScaleBar {
    base: BaseWidget,
    meters_per_pixel: f64,
    units: ScaleUnits,
    style: ScaleBarStyle,
    /// The cached label and the scale/unit/width key it was built for (SP-120).
    ///
    /// The label used to be formatted on every frame, so a still map paid for the same string
    /// sixty times a second.
    pub(super) label_cache: Option<(u64, ScaleUnits, u32, String)>,
}

impl ScaleBar {
    /// Creates a metric, plain-styled bar at 1 metre per pixel.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ScaleBar, geometry, "ScaleBar"),
            meters_per_pixel: 1.0,
            units: ScaleUnits::Metric,
            style: ScaleBarStyle::Plain,
            label_cache: None,
        }
    }

    /// Returns the current scale.
    pub fn meters_per_pixel(&self) -> f64 {
        self.meters_per_pixel
    }

    /// Sets the map scale. Non-finite and non-positive values are ignored.
    pub fn set_scale(&mut self, meters_per_pixel: f64) {
        if !meters_per_pixel.is_finite() || meters_per_pixel <= 0.0 {
            return;
        }
        if (self.meters_per_pixel - meters_per_pixel).abs() < f64::EPSILON {
            return;
        }
        self.meters_per_pixel = meters_per_pixel;
        self.base.request_redraw();
    }

    /// Returns the unit system.
    pub fn units(&self) -> ScaleUnits {
        self.units
    }

    /// Sets the unit system.
    pub fn set_units(&mut self, units: ScaleUnits) {
        if self.units == units {
            return;
        }
        self.units = units;
        self.base.request_redraw();
    }

    /// Returns the visual style.
    pub fn style(&self) -> ScaleBarStyle {
        self.style
    }

    /// Sets the visual style.
    pub fn set_style(&mut self, style: ScaleBarStyle) {
        if self.style == style {
            return;
        }
        self.style = style;
        self.base.request_redraw();
    }

    /// The largest 1/2/5 × 10ⁿ distance whose bar fits in `max_px`.
    ///
    /// The quantisation happens in the unit the label prints. Quantising metres and then
    /// converting for an imperial label made `2000 m` print `1.2 mi` while the bar was drawn
    /// 2000 m long — a 3.4 % error in a control whose purpose is measuring.
    ///
    /// Returns `0.0` for a degenerate or non-finite width; a caller drawing the
    /// result should skip the bar rather than paint a zero-length one.
    pub fn bar_distance(&self, max_px: f64) -> f64 {
        if !max_px.is_finite() || max_px <= 0.0 {
            return 0.0;
        }
        let meters = self.meters_per_pixel * max_px;
        match self.units {
            ScaleUnits::Metric => nice_floor(meters),
            ScaleUnits::Imperial => {
                let feet = meters * FEET_PER_METER;
                if feet >= FEET_PER_MILE {
                    nice_floor(feet / FEET_PER_MILE) * FEET_PER_MILE / FEET_PER_METER
                } else {
                    nice_floor(feet) / FEET_PER_METER
                }
            }
        }
    }

    /// The label a caller would draw, derived from the control's width.
    pub fn label(&self) -> String {
        let available = (f64::from(self.geometry().width) - 2.0 * SCALE_BAR_PADDING).max(16.0);
        format_scale_distance(self.units, self.bar_distance(available))
    }

    /// Rebuilds the draw-path label only when the scale, the units or the width moves (SP-120).
    fn refresh_label(&mut self) {
        let key = (self.meters_per_pixel.to_bits(), self.units, self.geometry().width);
        let stale = self
            .label_cache
            .as_ref()
            .is_none_or(|(bits, units, width, _)| (*bits, *units, *width) != key);
        if stale {
            let label = self.label();
            self.label_cache = Some((key.0, key.1, key.2, label));
        }
    }

    /// The width of the bar in pixels at the current scale.
    pub fn bar_pixels(&self) -> u32 {
        let available = (f64::from(self.geometry().width) - 2.0 * SCALE_BAR_PADDING).max(16.0);
        let distance = self.bar_distance(available);
        if distance <= 0.0 {
            return 0;
        }
        (distance / self.meters_per_pixel).round().clamp(0.0, available) as u32
    }
}

/// Feet in one metre, as used by the imperial label.
const FEET_PER_METER: f64 = 3.280_839_895;
/// Feet in one mile.
const FEET_PER_MILE: f64 = 5280.0;

/// Rounds `value` down to the nearest 1, 2 or 5 times a power of ten.
fn nice_floor(value: f64) -> f64 {
    if !value.is_finite() || value <= 0.0 {
        return 0.0;
    }
    let magnitude = 10f64.powf(value.log10().floor());
    let mantissa = value / magnitude;
    let step = if mantissa >= 5.0 {
        5.0
    } else if mantissa >= 2.0 {
        2.0
    } else {
        1.0
    };
    step * magnitude
}

/// Formats `meters` for `units` with the precision the magnitude deserves.
fn format_scale_distance(units: ScaleUnits, meters: f64) -> String {
    if !meters.is_finite() || meters <= 0.0 {
        return String::new();
    }
    match units {
        ScaleUnits::Metric => {
            if meters >= 1000.0 {
                format_scaled(meters / 1000.0, "km")
            } else {
                format_scaled(meters, "m")
            }
        }
        ScaleUnits::Imperial => {
            let feet = meters * FEET_PER_METER;
            if feet >= FEET_PER_MILE {
                format_scaled(feet / FEET_PER_MILE, "mi")
            } else if feet >= 1.0 {
                format_scaled(feet, "ft")
            } else {
                format_scaled(feet * 12.0, "in")
            }
        }
    }
}

/// Prints a whole number without a decimal point and a fraction with one.
fn format_scaled(value: f64, unit: &str) -> String {
    if (value - value.round()).abs() < 0.05 {
        format!("{:.0} {unit}", value)
    } else {
        format!("{:.1} {unit}", value)
    }
}

impl Widget for ScaleBar {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn capability_name(&self) -> Option<&'static str> {
        Some("scale_bar")
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn set_state_theme_hook(&mut self) {
        rust_widgets::style::reapply_active_theme_state(self);
    }

    fn size_hint(&self) -> rust_widgets::core::Size {
        rust_widgets::core::Size::new(120, 28)
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

impl WidgetProperties for ScaleBar {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "meters_per_pixel" => Ok(CapabilityValue::Float(self.meters_per_pixel)),
            "units" => Ok(CapabilityValue::String(self.units.token().to_string())),
            "style" => Ok(CapabilityValue::String(self.style.token().to_string())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "meters_per_pixel" => {
                self.set_scale(expect_f64(value)?);
                Ok(())
            }
            "units" => {
                let token = expect_string(value)?;
                let units =
                    ScaleUnits::from_token(&token).ok_or(CapabilityAccessError::TypeMismatch)?;
                self.set_units(units);
                Ok(())
            }
            "style" => {
                let token = expect_string(value)?;
                let style =
                    ScaleBarStyle::from_token(&token).ok_or(CapabilityAccessError::TypeMismatch)?;
                self.set_style(style);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["meters_per_pixel", "units", "style", BASE_PROPERTY_NAMES]
    }
}

impl EventHandler for ScaleBar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

impl Draw for ScaleBar {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        self.refresh_label();
        let style = self.base.style();
        let (surface, border, ink) = companion_palette(style, "scale_bar");
        if self.style == ScaleBarStyle::Boxed {
            context.face_with_gradient(rect, surface, None, Default::default(), 3, Color::BLACK);
            rust_widgets::widget::draw::stroke_surface_border(
                context,
                rect,
                // The fill above is the companion's 3 px arc; the outline follows it.
                [3, 3, 3, 3],
                border,
                1,
            );
        }
        let width = self.bar_pixels();
        if width == 0 {
            return;
        }
        let left = rect.x + SCALE_BAR_PADDING as i32;
        let bottom = rect.y + rect.height as i32 - 6;
        let right = left + width as i32;
        context.draw_line(Point::new(left, bottom), Point::new(right, bottom), ink);
        for x in [left, right] {
            context.draw_line(Point::new(x, bottom - 3), Point::new(x, bottom), ink);
        }
        let label = self.label_cache.as_ref().map(|(_, _, _, label)| label.as_str()).unwrap_or("");
        if !label.is_empty() {
            context.draw_text(
                Point::new(left, bottom - 16),
                label,
                effective_font(style),
                ink,
                HorizontalAlignment::Left,
            );
        }
    }
}
