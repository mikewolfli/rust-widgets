//! `ZoomStrip` — the shared minus/label/plus/fit control for zoomable surfaces (COMP-A25).
//!
//! # Why a companion
//!
//! `ImageView`, `MapView`, the designer canvas and the Gantt timeline each painted their own
//! zoom affordance, so the step size, the clamping rule and the percentage wording differed per
//! control. This type owns the strip once: it holds the range and the step, paints the segments,
//! and reports intent through two signals. The host remains responsible for applying the request
//! (the companion never moves content by itself), which is why the signal is named
//! `zoom_requested` rather than `zoom_changed`.
//!
//! # Pairing with `ZoomPanController`
//!
//! `ZoomPanController` owns the actual view transform. A host typically connects
//! [`ZoomStrip::zoom_requested`] to `ZoomPanController::set_zoom_level` and
//! [`ZoomStrip::fit_requested`] to the appropriate fit method.
//!
//! # Non-finite input
//!
//! A NaN zoom means "no meaningful request". It is resolved to `1.0` when the configured range
//! contains `1.0`, and to `min` otherwise. Infinite values take the normal clamp path (`+inf`
//! becomes `max`, `-inf` becomes `min`). Invalid bounds or steps are refused rather than stored.

use crate::compat::{format, lock, Arc, Mutex, String, ToString};
use crate::core::{HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::impl_draw_bridge;
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{EventSignalRef, Signal1};
use crate::style::EdgeOffsets;
use crate::widget::capability::coercion::{expect_bool, expect_f32};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::companions::ZoomMode;
use crate::widget::metrics::{
    effective_font, estimate_line_height, estimate_text_width, ControlMetrics,
};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

use super::helpers::{accent_color, surface_ink};

/// The narrowest usable strip.
const STRIP_MIN_WIDTH: u32 = 220;
/// The strip's own minimum height.
const STRIP_MIN_HEIGHT: u32 = 28;
/// Vertical inset used by the metric derivation.
const STRIP_PADDING_V: u32 = 4;
/// Horizontal inset used by the metric derivation.
const STRIP_PADDING_H: u32 = 6;
/// Extra air a minus/plus/Fit segment needs beyond one text line.
const SEGMENT_AIR: u32 = 16;

/// A three- or four-segment zoom control.
///
/// Segments, left to right: minus, the current zoom label, plus, and — when `fit_actions` is
/// true — a Fit button. A click on the label segment is deliberately inert.
pub struct ZoomStrip {
    base: BaseWidget,
    zoom_level: f32,
    min: f32,
    max: f32,
    step: f32,
    show_percentage: bool,
    fit_actions: bool,
    /// Writer end of a chart-side level queue, when a chart is attached (COMP-A8).
    pending_zoom: Option<Arc<Mutex<Option<f32>>>>,
    /// Emitted with the new zoom after a zoom action changed it.
    pub zoom_requested: Signal1<f32>,
    /// Emitted with the requested fit mode when the optional Fit segment is activated.
    pub fit_requested: Signal1<ZoomMode>,
}

impl ZoomStrip {
    /// Creates a strip at 100% over the range `0.1..=8.0`, stepping by `0.25`.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Slider, geometry, "ZoomStrip"),
            zoom_level: 1.0,
            min: 0.1,
            max: 8.0,
            step: 0.25,
            show_percentage: true,
            fit_actions: true,
            pending_zoom: None,
            zoom_requested: Signal1::new(),
            fit_requested: Signal1::new(),
        }
    }

    /// Returns the current zoom level.
    pub fn zoom_level(&self) -> f32 {
        self.zoom_level
    }

    /// Sets the zoom level, clamped to the configured range.
    ///
    /// A NaN request is resolved to `1.0` when the range contains `1.0`, and to `min` otherwise.
    /// A write that leaves the level unchanged emits nothing; a real change emits
    /// [`Self::zoom_requested`] exactly once.
    pub fn set_zoom(&mut self, zoom_level: f32) {
        let zoom_level = self.resolved_zoom(zoom_level);
        if self.zoom_level != zoom_level {
            self.zoom_level = zoom_level;
            self.base.request_redraw();
            self.zoom_requested.emit(zoom_level);
        }
    }

    /// Accepts a chart-side level queue (COMP-A8).
    ///
    /// The chart keeps the write end and this strip drains it through
    /// [`Self::sync_zoom_updates`], which the paint and event paths call. A mirrored level updates
    /// the readout without emitting [`Self::zoom_requested`]: it reports where the chart already
    /// is, not a new user request, so a wheel zoom mirrored back cannot bounce.
    pub(crate) fn accept_zoom_updates(&mut self, source: Arc<Mutex<Option<f32>>>) {
        self.pending_zoom = Some(source);
        self.base.request_redraw();
    }

    /// Absorbs the level a paired chart pushed since the last call.
    ///
    /// Returns whether a level was applied. The value is resolved and clamped exactly as
    /// [`Self::set_zoom`] resolves it, but emits nothing; see [`Self::accept_zoom_updates`].
    pub fn sync_zoom_updates(&mut self) -> bool {
        let Some(source) = self.pending_zoom.as_ref() else {
            return false;
        };
        let Some(level) = lock(source).take() else {
            return false;
        };
        self.mirror_zoom(level);
        true
    }

    /// Writes a level that came from the paired chart, without announcing a request.
    fn mirror_zoom(&mut self, zoom_level: f32) {
        let zoom_level = self.resolved_zoom(zoom_level);
        if self.zoom_level != zoom_level {
            self.zoom_level = zoom_level;
            self.base.request_redraw();
        }
    }

    /// Zooms in by one step.
    pub fn zoom_in(&mut self) {
        self.set_zoom(self.zoom_level + self.step);
    }

    /// Zooms out by one step.
    pub fn zoom_out(&mut self) {
        self.set_zoom(self.zoom_level - self.step);
    }

    /// Sets the zoom bounds, swapping an inverted pair rather than producing an empty range.
    ///
    /// Non-finite and non-positive bounds are refused, and the method returns `false`. The
    /// current zoom is clamped into the new range but no [`Self::zoom_requested`] is emitted: this
    /// is configuration, not a user zoom action.
    pub fn set_zoom_range(&mut self, min: f32, max: f32) -> bool {
        if !min.is_finite() || !max.is_finite() || min <= 0.0 || max <= 0.0 {
            return false;
        }
        let (min, max) = if min <= max { (min, max) } else { (max, min) };
        self.min = min;
        self.max = max;
        let clamped = self.zoom_level.clamp(min, max);
        if clamped != self.zoom_level {
            self.zoom_level = clamped;
            self.base.request_redraw();
        }
        true
    }

    /// Returns the minimum zoom.
    pub fn min(&self) -> f32 {
        self.min
    }

    /// Returns the maximum zoom.
    pub fn max(&self) -> f32 {
        self.max
    }

    /// Returns the increment used by [`Self::zoom_in`] and [`Self::zoom_out`].
    pub fn step(&self) -> f32 {
        self.step
    }

    /// Sets the step used by [`Self::zoom_in`] and [`Self::zoom_out`].
    ///
    /// A non-finite or non-positive step is refused, and the method returns `false`. The current
    /// zoom is unchanged.
    pub fn set_step(&mut self, step: f32) -> bool {
        if !step.is_finite() || step <= 0.0 {
            return false;
        }
        self.step = step;
        self.base.request_redraw();
        true
    }

    /// Whether the label is painted as a percentage or as a factor.
    pub fn show_percentage(&self) -> bool {
        self.show_percentage
    }

    /// Chooses between percentage (`125%`) and factor (`1.25x`) labels.
    pub fn set_show_percentage(&mut self, show_percentage: bool) {
        if self.show_percentage != show_percentage {
            self.show_percentage = show_percentage;
            self.base.request_redraw();
        }
    }

    /// Whether the optional Fit segment is painted and clickable.
    pub fn fit_actions(&self) -> bool {
        self.fit_actions
    }

    /// Shows or hides the Fit segment.
    pub fn set_fit_actions(&mut self, fit_actions: bool) {
        if self.fit_actions != fit_actions {
            self.fit_actions = fit_actions;
            self.base.request_redraw();
        }
    }

    /// The text painted in the middle segment: `125%` or `1.25x`.
    ///
    /// Whole numbers lose their decimal tail, so 100% is `100%` rather than `100.00%`.
    pub fn label(&self) -> String {
        if self.show_percentage {
            format!("{}%", trim_number(self.zoom_level * 100.0))
        } else {
            format!("{}x", trim_number(self.zoom_level))
        }
    }

    /// The number of painted segments.
    fn segment_count(&self) -> usize {
        if self.fit_actions {
            4
        } else {
            3
        }
    }

    /// The rectangle of one segment, sharing the split with the paint pass.
    fn segment_rect(&self, index: usize) -> Rect {
        let rect = self.geometry();
        let count = self.segment_count();
        let width = (rect.width / count as u32).max(1);
        let offset = (index as u32).saturating_mul(width);
        let extent = if index + 1 == count { rect.width.saturating_sub(offset) } else { width };
        Rect::new(
            rect.x.saturating_add(offset.min(i32::MAX as u32) as i32),
            rect.y,
            extent,
            rect.height,
        )
    }

    /// The segment under `pos`, or `None` outside the strip.
    fn segment_at(&self, pos: Point) -> Option<usize> {
        if !self.geometry().contains_point(pos) {
            return None;
        }
        let rect = self.geometry();
        let count = self.segment_count();
        let width = (rect.width / count as u32).max(1);
        let offset = (pos.x - rect.x).max(0) as u32;
        Some(((offset / width) as usize).min(count - 1))
    }

    /// Resolves a requested zoom according to the documented NaN and clamp rules.
    fn resolved_zoom(&self, zoom_level: f32) -> f32 {
        if zoom_level.is_nan() {
            if self.min <= 1.0 && 1.0 <= self.max {
                1.0
            } else {
                self.min
            }
        } else {
            let low = self.min.min(self.max);
            let high = self.min.max(self.max);
            zoom_level.clamp(low, high)
        }
    }
}

impl EventHandler for ZoomStrip {
    fn handle_event(&mut self, event: &Event) {
        // A chart-side mirror is drained before routing, so a click already sees the new label.
        self.sync_zoom_updates();
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        let Event::MouseRelease { pos, button: 1 } = event else {
            return;
        };
        let Some(index) = self.segment_at(*pos) else {
            return;
        };
        match index {
            0 => self.zoom_out(),
            2 => self.zoom_in(),
            3 if self.fit_actions => self.fit_requested.emit(ZoomMode::FitWidth),
            _ => {}
        }
    }
}

impl Draw for ZoomStrip {
    fn draw(&mut self, context: &mut RenderContext) {
        // Drain a level the paired chart pushed, so this frame paints it.
        self.sync_zoom_updates();
        let rect = self.geometry();
        let style = self.style();
        let (surface, ink) = surface_ink(style);
        let accent = accent_color();
        let font = effective_font(style);
        context.fill_rect(rect, surface);
        context.draw_rect(rect, surface.blend(&ink, 0.18));

        let count = self.segment_count();
        for index in 0..count {
            let cell = self.segment_rect(index);
            let fill =
                if index == 1 { surface.blend(&ink, 0.02) } else { surface.blend(&ink, 0.06) };
            context.fill_rect(cell, fill);
            context.draw_rect(cell, surface.blend(&ink, 0.16));
        }

        context.draw_text_fitted_cow(
            self.segment_rect(0),
            "-",
            font,
            ink,
            HorizontalAlignment::Center,
        );
        let label = self.label();
        context.draw_text_fitted_cow(
            self.segment_rect(1),
            &label,
            font,
            accent,
            HorizontalAlignment::Center,
        );
        context.draw_text_fitted_cow(
            self.segment_rect(2),
            "+",
            font,
            ink,
            HorizontalAlignment::Center,
        );
        if self.fit_actions {
            context.draw_text_fitted_cow(
                self.segment_rect(3),
                "Fit",
                font,
                surface.blend(&ink, 0.75),
                HorizontalAlignment::Center,
            );
        }
    }
}

impl Widget for ZoomStrip {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn size_hint(&self) -> Size {
        let font = effective_font(self.style());
        let line_height = estimate_line_height(font, 1.0);
        let label_width = estimate_text_width(&self.label(), font, 1.0);
        let segment_width = line_height.saturating_add(SEGMENT_AIR);
        let segment_count = if self.fit_actions { 3 } else { 2 };
        let content = Size::new(
            label_width.saturating_add(segment_width.saturating_mul(segment_count)),
            line_height,
        );
        ControlMetrics::implicit_size(
            content,
            EdgeOffsets::symmetric(STRIP_PADDING_V, STRIP_PADDING_H),
            Size::new(STRIP_MIN_WIDTH, STRIP_MIN_HEIGHT),
        )
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    fn event_signal_dyn(&self, name: &str) -> Option<EventSignalRef> {
        match name {
            "zoom_requested" => {
                Some(EventSignalRef::mapped("zoom_requested", &self.zoom_requested, |value| {
                    CapabilityValue::Float(f64::from(*value))
                }))
            }
            "fit_requested" => {
                Some(EventSignalRef::mapped("fit_requested", &self.fit_requested, |mode| {
                    CapabilityValue::String(mode.as_str().to_string())
                }))
            }
            _ => None,
        }
    }
}

/// Builds a pointable refusal for a numeric property that failed its range rule.
fn constraint_violation(property: &str, reason: &str) -> CapabilityAccessError {
    CapabilityAccessError::ConstraintViolation {
        property: String::from(property),
        reason: String::from(reason),
    }
}

/// Formats a float without a pointless decimal tail, keeping at most two decimals.
fn trim_number(value: f32) -> String {
    if !value.is_finite() {
        return "-".to_string();
    }
    let rounded = (value * 100.0).round() / 100.0;
    if rounded.abs() < 1.0e15 && (rounded - rounded.round()).abs() < f32::EPSILON {
        return format!("{}", rounded.round() as i64);
    }
    let text = format!("{rounded:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

impl WidgetProperties for ZoomStrip {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "zoom_level" => Ok(CapabilityValue::Float(f64::from(self.zoom_level))),
            "min" => Ok(CapabilityValue::Float(f64::from(self.min))),
            "max" => Ok(CapabilityValue::Float(f64::from(self.max))),
            "step" => Ok(CapabilityValue::Float(f64::from(self.step))),
            "show_percentage" => Ok(CapabilityValue::Bool(self.show_percentage)),
            "fit_actions" => Ok(CapabilityValue::Bool(self.fit_actions)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "zoom_level" => {
                self.set_zoom(expect_f32(value)?);
                Ok(())
            }
            "min" => {
                let min = expect_f32(value)?;
                if self.set_zoom_range(min, self.max) {
                    Ok(())
                } else {
                    Err(constraint_violation("min", "zoom bounds must be finite and positive"))
                }
            }
            "max" => {
                let max = expect_f32(value)?;
                if self.set_zoom_range(self.min, max) {
                    Ok(())
                } else {
                    Err(constraint_violation("max", "zoom bounds must be finite and positive"))
                }
            }
            "step" => {
                let step = expect_f32(value)?;
                if self.set_step(step) {
                    Ok(())
                } else {
                    Err(constraint_violation("step", "the zoom step must be finite and positive"))
                }
            }
            "show_percentage" => {
                self.set_show_percentage(expect_bool(value)?);
                Ok(())
            }
            "fit_actions" => {
                self.set_fit_actions(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "zoom_level",
            "min",
            "max",
            "step",
            "show_percentage",
            "fit_actions",
            BASE_PROPERTY_NAMES
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::{lock, Arc, Mutex, Vec};

    /// Records a payload signal into a shared vector.
    fn record<T: Clone + Send + 'static>(signal: &Signal1<T>) -> Arc<Mutex<Vec<T>>> {
        let sink = Arc::new(Mutex::new(Vec::new()));
        let handle = Arc::clone(&sink);
        signal.connect(move |value: Arc<T>| {
            lock(&handle).push((*value).clone());
        });
        sink
    }

    /// Reads the recorded values.
    fn values<T: Clone>(sink: &Arc<Mutex<Vec<T>>>) -> Vec<T> {
        lock(sink).clone()
    }

    #[test]
    fn zoom_is_clamped_to_the_configured_range() {
        let mut strip = ZoomStrip::new(Rect::new(0, 0, 240, 28));
        strip.set_zoom_range(0.5, 4.0);
        strip.set_zoom(100.0);
        assert_eq!(strip.zoom_level(), 4.0);
        strip.set_zoom(0.01);
        assert_eq!(strip.zoom_level(), 0.5);

        // An inverted range is swapped instead of becoming empty.
        assert!(strip.set_zoom_range(8.0, 2.0));
        assert_eq!(strip.min(), 2.0);
        assert_eq!(strip.max(), 8.0);
        assert_eq!(strip.zoom_level(), 2.0);

        // A non-positive bound is refused rather than producing a zero-width range.
        assert!(!strip.set_zoom_range(0.0, 4.0));
        assert_eq!(strip.min(), 2.0);
    }

    #[test]
    fn zoom_steps_move_by_the_configured_step() {
        let mut strip = ZoomStrip::new(Rect::new(0, 0, 240, 28));
        strip.set_zoom_range(0.5, 4.0);
        strip.set_zoom(1.0);
        assert!(strip.set_step(0.5));
        strip.zoom_in();
        assert_eq!(strip.zoom_level(), 1.5);
        strip.zoom_out();
        assert_eq!(strip.zoom_level(), 1.0);

        // A zero step would make the buttons inert, so it is refused.
        assert!(!strip.set_step(0.0));
        assert_eq!(strip.step(), 0.5);
    }

    #[test]
    fn the_label_switches_between_percentage_and_factor() {
        let mut strip = ZoomStrip::new(Rect::new(0, 0, 240, 28));
        strip.set_zoom_range(0.1, 8.0);
        strip.set_zoom(1.25);
        assert_eq!(strip.label(), "125%");

        strip.set_show_percentage(false);
        assert_eq!(strip.label(), "1.25x");

        strip.set_zoom(2.0);
        assert_eq!(strip.label(), "2x");

        strip.set_show_percentage(true);
        assert_eq!(strip.label(), "200%");
    }

    #[test]
    fn each_zoom_action_emits_exactly_one_request() {
        let mut strip = ZoomStrip::new(Rect::new(0, 0, 240, 28));
        strip.set_zoom_range(0.5, 2.0);
        let requested = record(&strip.zoom_requested);

        strip.zoom_in();
        assert_eq!(values(&requested), vec![1.25]);
        strip.zoom_out();
        assert_eq!(values(&requested), vec![1.25, 1.0]);

        // Setting the same level again is not an action.
        strip.set_zoom(1.0);
        assert_eq!(values(&requested).len(), 2);

        // At the ceiling the step cannot change the level, so it emits nothing.
        strip.set_zoom(2.0);
        assert_eq!(values(&requested).len(), 3);
        strip.zoom_in();
        assert_eq!(values(&requested).len(), 3);
    }

    #[test]
    fn a_nan_zoom_resets_to_the_documented_value() {
        let mut strip = ZoomStrip::new(Rect::new(0, 0, 240, 28));
        strip.set_zoom_range(0.5, 4.0);
        strip.set_zoom(f32::NAN);
        assert_eq!(strip.zoom_level(), 1.0, "1.0 is inside the range");

        strip.set_zoom_range(2.0, 4.0);
        strip.set_zoom(f32::NAN);
        assert_eq!(strip.zoom_level(), 2.0, "1.0 is outside, so the minimum is used");
    }

    #[test]
    fn the_fit_segment_emits_one_fit_request() {
        let mut strip = ZoomStrip::new(Rect::new(0, 0, 240, 28));
        let fits = record(&strip.fit_requested);

        // Four equal 60-pixel segments; the fourth is Fit.
        strip.handle_event(&Event::MouseRelease { pos: Point::new(200, 10), button: 1 });
        assert_eq!(values(&fits), vec![ZoomMode::FitWidth]);

        // A release outside the strip is ignored.
        strip.handle_event(&Event::MouseRelease { pos: Point::new(400, 10), button: 1 });
        assert_eq!(values(&fits).len(), 1);
    }

    #[test]
    fn the_property_contract_round_trips() {
        let mut strip = ZoomStrip::new(Rect::new(0, 0, 240, 28));
        strip.set("zoom_level", CapabilityValue::Float(1.5)).expect("writable");
        strip.set("step", CapabilityValue::Float(0.5)).expect("writable");
        strip.set("show_percentage", CapabilityValue::Bool(false)).expect("writable");
        strip.set("fit_actions", CapabilityValue::Bool(false)).expect("writable");

        assert_eq!(strip.get("zoom_level"), Ok(CapabilityValue::Float(1.5)));
        assert_eq!(strip.get("step"), Ok(CapabilityValue::Float(0.5)));
        assert_eq!(strip.get("show_percentage"), Ok(CapabilityValue::Bool(false)));
        assert_eq!(strip.get("fit_actions"), Ok(CapabilityValue::Bool(false)));
        assert_eq!(strip.label(), "1.5x");

        // An invalid step is refused as a pointable constraint violation.
        assert!(matches!(
            strip.set("step", CapabilityValue::Float(0.0)),
            Err(CapabilityAccessError::ConstraintViolation { .. })
        ));
        assert_eq!(strip.step(), 0.5);
    }
}
