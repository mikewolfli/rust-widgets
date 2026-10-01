// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! RangeSlider widget — a dual-handle slider for selecting a numeric range.
//!
//! The RangeSlider widget provides two draggable handles on a horizontal or
//! vertical track, allowing the user to select a lower and upper bound value
//! within a configurable range. The region between the handles is visually
//! highlighted.

use crate::core::{Color, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::{
    expect_f64, expect_text_direction, text_direction_to_str,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::numeric::ordered_clamp_f64;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Orientation of the RangeSlider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RangeSliderOrientation {
    /// Horizontal slider (left-to-right).
    #[default]
    Horizontal,
    /// Vertical slider (bottom-to-top).
    Vertical,
}

/// A dual-handle range slider for selecting a min-max value range.
///
/// The slider has two draggable handles: a lower handle and an upper handle.
/// The lower handle cannot exceed the upper handle, and the range between them
/// respects a configurable minimum range constraint.
pub struct RangeSlider {
    base: BaseWidget,
    min_value: f64,
    max_value: f64,
    lower_value: f64,
    upper_value: f64,
    step: f64,
    orientation: RangeSliderOrientation,
    min_range: f64,
    /// The writing direction the horizontal value axis runs in.
    ///
    /// A range selector's two handles sit on a line, and "the lower value is nearer the start" is a
    /// statement about the *reading* order of that line. In an Arabic or Hebrew interface the line
    /// begins at the right, so the lower handle belongs on the right and dragging toward it means
    /// dragging right — otherwise the handles move the opposite way from the numbers they show.
    ///
    /// The vertical axis is not affected: the block axis runs top-to-bottom in every direction.
    /// Defaults to left-to-right, so a selector that never asks behaves exactly as it did.
    direction: crate::core::TextDirection,
    /// Emitted when the range (lower, upper) changes.
    pub range_changed: Signal1<(f64, f64)>,
    /// Which handle is currently being dragged: None, Some(true) for lower, Some(false) for upper.
    dragging: Option<bool>,
}

/// The radius of a range slider's handle, in logical pixels.
///
/// # Why this is a named constant and not a local `8`
///
/// The value was spelled four times: in `value_to_pixel`, in `pixel_to_value`, as the drawn radius,
/// and again in the hit test's own `10`. That is the BLUE22 §4.3 defect class — the two conversions
/// and the two drawing paths each deriving one inset from their own literal — and it is what made a
/// click on a handle return a different value from the one the handle was drawn at. One name means
/// the drawn disc, its hit area and both mappings cannot disagree.
pub const RANGE_SLIDER_HANDLE_RADIUS: u32 = 8;

/// How much larger than the handle the clickable area is.
///
/// A touch target may be kinder than the mark it belongs to — that is what
/// [`dimensions::TOUCH_TARGET_MIN`] is for — but it must be expressed as a **relation to the
/// handle**, not as a second absolute number. `is_handle_hit` used to carry its own `let handle_radius
/// = 10i32`, so widening the drawn handle would silently have narrowed its hit area's margin.
pub const RANGE_SLIDER_HIT_SLOP: u32 = 2;

impl RangeSlider {
    /// Creates a new RangeSlider with the given geometry.
    ///
    /// Default range is 0.0 to 100.0, step 1.0, min_range 0.0.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::RangeSlider, geometry, "RangeSlider"),
            min_value: 0.0,
            max_value: 100.0,
            lower_value: 25.0,
            upper_value: 75.0,
            step: 1.0,
            orientation: RangeSliderOrientation::default(),
            min_range: 0.0,
            direction: crate::core::TextDirection::default(),
            range_changed: Signal1::new(),
            dragging: None,
        }
    }

    /// Returns the current lower value.
    pub fn lower_value(&self) -> f64 {
        self.lower_value
    }

    /// Sets the lower value, clamping it to be within bounds and respecting min_range.
    /// Emits `range_changed` if the value changes.
    pub fn set_lower_value(&mut self, value: f64) {
        let clamped = ordered_clamp_f64(value, self.min_value, self.upper_value - self.min_range);
        let stepped = (clamped / self.step).round() * self.step;
        let stepped = stepped.max(self.min_value);
        let new_value = stepped.min(self.upper_value - self.min_range);
        if (new_value - self.lower_value).abs() > f64::EPSILON {
            self.lower_value = new_value;
            self.emit_range_changed();
            self.base.request_redraw();
        }
    }

    /// Returns the current upper value.
    pub fn upper_value(&self) -> f64 {
        self.upper_value
    }

    /// Returns whether a handle is currently being dragged.
    ///
    /// # Why this is public
    ///
    /// It is the fact the draw path emphasises a handle with, and a host that owns its own input
    /// layer — a touch backend, a test, a designer previewing a state — needs to ask it the same way
    /// it can ask a [`Slider`](crate::widget::display_widgets::slider::Slider). Without it the
    /// dragged appearance was reachable only by synthesising a press.
    pub fn is_dragging(&self) -> bool {
        self.dragging.is_some()
    }

    /// Sets the upper value, clamping it to be within bounds and respecting min_range.
    /// Emits `range_changed` if the value changes.
    pub fn set_upper_value(&mut self, value: f64) {
        let clamped = ordered_clamp_f64(value, self.lower_value + self.min_range, self.max_value);
        let stepped = (clamped / self.step).round() * self.step;
        let stepped = stepped.min(self.max_value);
        let new_value = stepped.max(self.lower_value + self.min_range);
        if (new_value - self.upper_value).abs() > f64::EPSILON {
            self.upper_value = new_value;
            self.emit_range_changed();
            self.base.request_redraw();
        }
    }

    /// Sets both lower and upper values simultaneously, respecting all constraints.
    pub fn set_range(&mut self, lower: f64, upper: f64) {
        let lower = ordered_clamp_f64(lower, self.min_value, self.max_value - self.min_range);
        let upper = ordered_clamp_f64(upper, lower + self.min_range, self.max_value);
        let lower_stepped = (lower / self.step).round() * self.step;
        let upper_stepped = (upper / self.step).round() * self.step;
        let lower_stepped = lower_stepped.max(self.min_value);
        let upper_stepped = upper_stepped.max(lower_stepped + self.min_range).min(self.max_value);
        let lower_stepped = lower_stepped.min(upper_stepped - self.min_range);

        if (lower_stepped - self.lower_value).abs() > f64::EPSILON
            || (upper_stepped - self.upper_value).abs() > f64::EPSILON
        {
            self.lower_value = lower_stepped;
            self.upper_value = upper_stepped;
            self.emit_range_changed();
            self.base.request_redraw();
        }
    }

    /// Returns the minimum possible value.
    pub fn min_value(&self) -> f64 {
        self.min_value
    }

    /// Returns the maximum possible value.
    pub fn max_value(&self) -> f64 {
        self.max_value
    }

    /// Sets the minimum possible value, keeping the range well formed.
    ///
    /// The upper bound is pulled down first when it would otherwise sit below
    /// the new minimum, so `min_value <= max_value` always holds and the current
    /// handles stay inside the bounds.
    pub fn set_min_value(&mut self, min_value: f64) {
        if !min_value.is_finite() || min_value == self.min_value {
            return;
        }
        self.min_value = min_value;
        if self.max_value < self.min_value {
            self.max_value = self.min_value;
        }
        self.lower_value = ordered_clamp_f64(self.lower_value, self.min_value, self.max_value);
        self.upper_value = ordered_clamp_f64(self.upper_value, self.lower_value, self.max_value);
        self.emit_range_changed();
        self.base.request_redraw();
    }

    /// Sets the maximum possible value, keeping the range well formed.
    ///
    /// The lower bound is pushed up first when it would otherwise sit above the
    /// new maximum, so `min_value <= max_value` always holds and the current
    /// handles stay inside the bounds.
    pub fn set_max_value(&mut self, max_value: f64) {
        if !max_value.is_finite() || max_value == self.max_value {
            return;
        }
        self.max_value = max_value;
        if self.min_value > self.max_value {
            self.min_value = self.max_value;
        }
        self.lower_value = ordered_clamp_f64(self.lower_value, self.min_value, self.max_value);
        self.upper_value = ordered_clamp_f64(self.upper_value, self.lower_value, self.max_value);
        self.emit_range_changed();
        self.base.request_redraw();
    }

    /// Returns the current step increment.
    pub fn step(&self) -> f64 {
        self.step
    }

    /// Sets the step increment for handle movement.
    pub fn set_step(&mut self, step: f64) {
        self.step = step.max(0.001);
        self.base.request_redraw();
    }

    /// Returns the minimum allowed range (distance between lower and upper).
    pub fn min_range(&self) -> f64 {
        self.min_range
    }

    /// Returns the writing direction the horizontal value axis runs in.
    pub fn direction(&self) -> crate::core::TextDirection {
        self.direction
    }

    /// Returns the axis the selector's track runs along.
    pub fn orientation(&self) -> RangeSliderOrientation {
        self.orientation
    }

    /// Sets the axis the selector's track runs along, and repaints.
    ///
    /// # Why this exists
    ///
    /// The field was constructible only through the default, so a vertical range selector was
    /// unreachable from the public API even though `draw`, both coordinate mappings and the hit test
    /// all had a vertical arm. That is the "declared but unreachable" shape principle #22 forbids, and
    /// it is why the vertical arm had no test: nothing could put the control in that state.
    pub fn set_orientation(&mut self, orientation: RangeSliderOrientation) {
        self.orientation = orientation;
        self.base.request_redraw();
    }

    /// Sets the writing direction the horizontal value axis runs in, and repaints.
    ///
    /// A right-to-left selector places the *lower* value at the right edge, because that is where its
    /// line begins — so both the handles' positions and the mapping from a click's x-coordinate flip
    /// together. They flip through the same inset
    /// ([`RangeSlider::track_begin_and_length`] plus [`crate::core::TextDirection`]), which is the
    /// property that stops a click on a handle from returning a different value from the one drawn.
    pub fn set_direction(&mut self, direction: crate::core::TextDirection) {
        self.direction = direction;
        self.base.request_redraw();
    }

    /// Sets the minimum allowed range between handles.
    pub fn set_min_range(&mut self, min_range: f64) {
        self.min_range = min_range.max(0.0);
        // Clamp current values to respect new min_range
        if self.upper_value - self.lower_value < self.min_range {
            self.upper_value = (self.lower_value + self.min_range).min(self.max_value);
            self.emit_range_changed();
        }
        self.base.request_redraw();
    }

    /// Emits the range_changed signal.
    fn emit_range_changed(&self) {
        self.range_changed.emit((self.lower_value, self.upper_value));
    }

    /// Converts a value to pixel position on the track.
    ///
    /// # Why the two directions share one derivation
    ///
    /// This and [`Self::pixel_to_value`] are inverses, so they must measure the track from the same
    /// inset — the handle's own radius, because a handle is a disc and its centre must never travel
    /// closer than that to either end. Both used to spell the inset as a local `8i32`, and `draw` and
    /// `is_handle_hit` spell it a third and fourth time; BLUE22 §4.3 records the same defect in
    /// `slider` (half a handle one way, a full width the other, so a click on the handle returned a
    /// different value from the one drawn). The two conversions are therefore expressed **through one
    /// pair of helpers**, and the inset has one name.
    fn value_to_pixel(&self, value: f64, rect: &Rect) -> i32 {
        let (begin, length) = self.track_begin_and_length(rect);
        if (self.max_value - self.min_value).abs() < f64::EPSILON {
            return begin;
        }
        let ratio = (value - self.min_value) / (self.max_value - self.min_value);
        // The horizontal axis runs in **reading order**: a value is placed from the beginning of the
        // line, which is the right edge in an RTL locale. `TextDirection` is the one conversion
        // between "a fraction along the line" and "a fraction from the left edge".
        match self.orientation {
            RangeSliderOrientation::Horizontal => {
                let left_ratio = self.direction.begin_fraction_to_left_fraction(ratio as f32);
                begin + (length as f64 * left_ratio as f64) as i32
            }
            // The block axis is not a reading direction, so the vertical arm does not consult the
            // direction at all — the same rule `slider` and `progress_bar` apply.
            RangeSliderOrientation::Vertical => begin + length - (length as f64 * ratio) as i32,
        }
    }

    /// Converts a pixel position to a value on the track (the inverse of [`Self::value_to_pixel`]).
    fn pixel_to_value(&self, pos: i32, rect: &Rect) -> f64 {
        let (begin, length) = self.track_begin_and_length(rect);
        if length <= 0 {
            return self.min_value;
        }
        let end = begin + length;
        let clamped = pos.clamp(begin.min(end), begin.max(end));
        let begin_ratio = match self.orientation {
            RangeSliderOrientation::Horizontal => {
                let left_ratio = (clamped - begin) as f64 / length as f64;
                self.direction.left_fraction_to_begin_fraction(left_ratio as f32) as f64
            }
            RangeSliderOrientation::Vertical => (end - clamped) as f64 / length as f64,
        };
        self.min_value + begin_ratio * (self.max_value - self.min_value)
    }

    /// The track's **beginning** and its usable length, in pixels along its axis.
    ///
    /// The beginning is the coordinate the *minimum value* sits at when the direction is
    /// left-to-right and the axis is horizontal; for the vertical axis it is the **top** edge, which
    /// is where the maximum sits — the vertical arm's own value-to-position mapping accounts for that,
    /// so both conversions can share one origin and one length without either re-deriving the inset.
    ///
    /// The inset is the handle's own radius at both ends, so the disc never overhangs the track — the
    /// property §4.3 says the two conversions must agree about.
    fn track_begin_and_length(&self, rect: &Rect) -> (i32, i32) {
        let r = RANGE_SLIDER_HANDLE_RADIUS as i32;
        match self.orientation {
            RangeSliderOrientation::Horizontal => {
                let begin = rect.x + r;
                let end = rect.x + rect.width as i32 - r;
                (begin, (end - begin).max(0))
            }
            RangeSliderOrientation::Vertical => {
                let begin = rect.y + r;
                let end = rect.y + rect.height as i32 - r;
                (begin, (end - begin).max(0))
            }
        }
    }

    /// Checks if a point is within a handle's hit area.
    ///
    /// The centre comes from [`Self::value_to_pixel`] — the same function the paint path uses — so a
    /// click is answered by the handle that was *drawn*, not by a second derivation of where it ought
    /// to be. The radius is the drawn radius plus [`RANGE_SLIDER_HIT_SLOP`], which is how a touch
    /// target is widened here: a relation to the mark, so changing the mark cannot silently shrink the
    /// margin.
    fn is_handle_hit(&self, pos: Point, rect: &Rect, is_lower: bool) -> bool {
        let value = if is_lower { self.lower_value } else { self.upper_value };
        let handle_radius = (RANGE_SLIDER_HANDLE_RADIUS + RANGE_SLIDER_HIT_SLOP) as i32;
        let cx = self.value_to_pixel(value, rect);
        let cy = if self.orientation == RangeSliderOrientation::Horizontal {
            rect.y + rect.height as i32 / 2
        } else {
            rect.x + rect.width as i32 / 2
        };

        let handle_center = if self.orientation == RangeSliderOrientation::Horizontal {
            Point::new(cx, cy)
        } else {
            Point::new(cy, cx)
        };

        let dx = pos.x - handle_center.x;
        let dy = pos.y - handle_center.y;
        (dx * dx + dy * dy) <= (handle_radius * handle_radius)
    }
}

impl Widget for RangeSlider {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(
            crate::widget::metrics::dimensions::RANGE_SLIDER_DEFAULT_WIDTH,
            crate::widget::metrics::dimensions::RANGE_SLIDER_DEFAULT_HEIGHT,
        )
    }

    /// Announces **both** handles, because a range has two values.
    ///
    /// # Why this control needs its own answer
    ///
    /// The trait default reads a property from a fixed list — `value` / `progress` / `rating` /
    /// `level` — and this control publishes neither handle under any of those names, so a screen
    /// reader was told nothing about where the range sits. It cannot be fixed by renaming a
    /// property: `lower` and `upper` *are* the contract, and picking one to announce would be worse
    /// than silence, because a reader would then report half a range as if it were the value.
    ///
    /// Each half is announced through [`CapabilityValue::to_announcement_string`], which is the one
    /// place that decides how a float is spoken (an integral value is announced as its integer).
    /// Spelling that rule again here would let the two drift. The ends are joined with an en dash,
    /// matching how a range is written wherever one is printed, so the announcement reads as one
    /// quantity rather than two sentences.
    fn accessible_value(&self) -> String {
        let lower = CapabilityValue::Float(self.lower_value()).to_announcement_string();
        let upper = CapabilityValue::Float(self.upper_value()).to_announcement_string();
        let mut text = lower;
        text.push('–');
        text.push_str(&upper);
        text
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why the field is named per arm
    ///
    /// The published name and the Rust field name are not always the same (`find_next` is backed by
    /// `find_next_signal`, `dismissed` by a `Signal1<()>` field). `connect_event` validates a name
    /// against the capability table and registers a hub slot; only `event_signal_dyn` joins that
    /// name to the signal the control actually **emits**. A wrong arm is worse than no arm, because
    /// it reports a wire as live and never fires it, so each field is named explicitly here rather
    /// than derived from the published name.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "range_changed" => {
                Some(EventSignalRef::mapped("range_changed", &self.range_changed, |v| {
                    CapabilityValue::String(format!("{v:?}"))
                }))
            }
            _ => None,
        }
    }
}

/// `RangeSlider`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_input.in.rs` / `access_write_input.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `lower` and `upper` keep the
/// clamp-and-snap the widget already applied; `min_value` and `max_value` stay
/// consistent with each other and with the handles.
impl WidgetProperties for RangeSlider {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "min_value" => Ok(CapabilityValue::Float(self.min_value())),
            "max_value" => Ok(CapabilityValue::Float(self.max_value())),
            "lower" => Ok(CapabilityValue::Float(self.lower_value())),
            "upper" => Ok(CapabilityValue::Float(self.upper_value())),
            "orientation" => Ok(CapabilityValue::String(
                match self.orientation() {
                    RangeSliderOrientation::Horizontal => "horizontal",
                    RangeSliderOrientation::Vertical => "vertical",
                }
                .to_string(),
            )),
            "direction" => {
                Ok(CapabilityValue::String(text_direction_to_str(self.direction()).to_string()))
            }
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "min_value" => {
                self.set_min_value(expect_f64(value)?);
                Ok(())
            }
            "max_value" => {
                self.set_max_value(expect_f64(value)?);
                Ok(())
            }
            "lower" => {
                self.set_lower_value(expect_f64(value)?);
                Ok(())
            }
            "upper" => {
                self.set_upper_value(expect_f64(value)?);
                Ok(())
            }
            "orientation" => {
                let token = match value {
                    CapabilityValue::String(ref v) => crate::compat::String::to_string(v),
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                // Matched here rather than through `expect_orientation`, because the range
                // selector has its own orientation enum: sharing the parser would force a
                // conversion at every call site and one more place for the two to drift.
                match crate::widget::capability::coercion::normalize_key(&token).as_str() {
                    "horizontal" => self.set_orientation(RangeSliderOrientation::Horizontal),
                    "vertical" => self.set_orientation(RangeSliderOrientation::Vertical),
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                }
                Ok(())
            }
            "direction" => {
                self.set_direction(expect_text_direction(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "min_value",
            "max_value",
            "lower",
            "upper",
            "orientation",
            "direction",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `range_slider` publishes.
    ///
    /// `set_lower` and `set_upper` name a handle and a value, and `set_range` names
    /// both bounds: every one of them needs a payload, so the whole set is answered
    /// through the property route.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_lower" | "set_upper" | "set_range" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for RangeSlider {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();
        // The drawn radius is the same constant both conversions and the hit test measure their inset
        // from, so the disc, the track's ends and a click's answer are one derivation.
        let handle_radius = RANGE_SLIDER_HANDLE_RADIUS;

        // Chrome colours resolve the explicit style first, then the theme's resolved style for
        // this control, and only then fall back to a literal. Every colour below used to be a
        // literal, so a light/dark switch left the track, the selected run and both handles
        // unchanged — the rendering census reported the control as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's mutex
        // is not re-entrant.
        let style = self.base.style().clone();
        // `range_slider` is not in the role table, so it classifies as `Surface` and its resolved
        // background is the window fill itself; the empty track below therefore derives its own
        // distinct colour rather than painting the window's.
        let theme = crate::style::resolved_theme_style("range_slider");
        let (window_fill, foreground, primary, muted, disabled) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => (
                    active.colors.background,
                    active.colors.foreground,
                    active.colors.primary,
                    active.colors.secondary,
                    active.colors.disabled,
                ),
                None => (
                    Color::rgb(240, 240, 240),
                    Color::BLACK,
                    Color::rgb(33, 150, 243),
                    Color::rgb(158, 158, 158),
                    Color::rgb(200, 200, 200),
                ),
            }
        };

        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(foreground);
        // The empty run of the track: one step from the window fill toward the text colour, so it
        // is visible on either appearance rather than being the window's own colour. The filter is
        // on the **resolved** value, not only on the theme's: a control classified as `Surface`
        // already carries the window fill, so letting it through unfiltered is exactly the
        // invisible-track defect this guards against. A caller's own colour still wins.
        let track_from_theme = window_fill.blend(&ink, 0.14);
        let track_surface = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => track_from_theme,
        };
        // The empty run recedes when the control is disabled, and the filled run — the control's
        // value indicator — carries the theme's primary rather than a fixed blue.
        let track_color =
            if is_enabled { track_surface } else { track_surface.blend(&disabled, 0.50) };
        let range_color = if is_enabled { primary } else { disabled };
        // The handles sit *on* the track, so each is built from it: a light disc on a light track
        // and a dark one on a dark track, with a border one visible step out.
        let handle_color = if is_enabled {
            if track_color.is_dark() {
                track_color.blend(&Color::WHITE, 0.30)
            } else {
                track_color.blend(&Color::WHITE, 0.85)
            }
        } else {
            track_color.blend(&Color::WHITE, 0.55)
        };
        let handle_border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != handle_color)
            .unwrap_or_else(|| {
                if is_enabled {
                    range_color
                } else {
                    handle_color.blend(&muted, 0.40)
                }
            });

        // Track background
        let track_thickness = 6u32;

        if self.orientation == RangeSliderOrientation::Horizontal {
            let track_y = rect.y + rect.height as i32 / 2 - track_thickness as i32 / 2;
            let track_rect = Rect::new(
                rect.x + handle_radius as i32,
                track_y,
                rect.width - handle_radius * 2,
                track_thickness,
            );
            context.fill_rounded_rect(track_rect, track_thickness / 2, track_color);

            // Highlighted range between handles
            let lower_x = self.value_to_pixel(self.lower_value, &rect);
            let upper_x = self.value_to_pixel(self.upper_value, &rect);
            if upper_x > lower_x {
                let range_rect =
                    Rect::new(lower_x, track_y, (upper_x - lower_x) as u32, track_thickness);
                context.fill_rounded_rect(range_rect, track_thickness / 2, range_color);
            }

            // Draw handles
            let center_y = rect.y + rect.height as i32 / 2;

            for (index, &value) in [self.lower_value, self.upper_value].iter().enumerate() {
                let cx = self.value_to_pixel(value, &rect);
                let is_lower = index == 0;
                Self::draw_handle(
                    context,
                    Point::new(cx, center_y),
                    self.handle_state(is_lower),
                    handle_radius,
                    handle_color,
                    handle_border,
                );
            }
        } else {
            // Vertical
            let track_x = rect.x + rect.width as i32 / 2 - track_thickness as i32 / 2;
            let track_rect = Rect::new(
                track_x,
                rect.y + handle_radius as i32,
                track_thickness,
                rect.height - handle_radius * 2,
            );
            context.fill_rounded_rect(track_rect, track_thickness / 2, track_color);

            // Highlighted range
            let lower_y = self.value_to_pixel(self.lower_value, &rect);
            let upper_y = self.value_to_pixel(self.upper_value, &rect);
            if upper_y < lower_y {
                let range_rect =
                    Rect::new(track_x, upper_y, track_thickness, (lower_y - upper_y) as u32);
                context.fill_rounded_rect(range_rect, track_thickness / 2, range_color);
            }

            // Draw handles
            let center_x = rect.x + rect.width as i32 / 2;

            for (index, &value) in [self.lower_value, self.upper_value].iter().enumerate() {
                let cy = self.value_to_pixel(value, &rect);
                let is_lower = index == 0;
                Self::draw_handle(
                    context,
                    Point::new(center_x, cy),
                    self.handle_state(is_lower),
                    handle_radius,
                    handle_color,
                    handle_border,
                );
            }
        }
    }
}

/// How a range handle is being interacted with, which is the whole of what its emphasis encodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HandleInteraction {
    /// The pointer is not on this handle.
    Resting,
    /// The pointer is over the control, but not on this handle.
    Idle,
    /// This handle is the one being dragged.
    Dragging,
}

impl RangeSlider {
    /// Which emphasis one of the two handles is drawn with.
    ///
    /// # The defect this pins
    ///
    /// `dragging` was stored, set and cleared correctly — and **never drawn**. A user who grabbed a
    /// handle saw exactly the frame they saw before they touched it, so the only feedback about
    /// "which of these two overlapping discs am I holding" was the value moving afterwards. The
    /// same shape as the bezier handles, `audio_visualizer` and `data_grid`; the reading is derived
    /// from the same field the hit-routing uses, so the two cannot disagree about which is held.
    fn handle_state(&self, is_lower: bool) -> HandleInteraction {
        if self.dragging == Some(is_lower) {
            HandleInteraction::Dragging
        } else if self.base.is_hovered() {
            // The pointer being over the control at all is enough to say "these are grabbable":
            // which of the two it is over is already answered by the hover value, and making the
            // emphasis depend on that would make it flicker as the pointer crosses the gap between
            // two handles that are near each other.
            HandleInteraction::Idle
        } else {
            HandleInteraction::Resting
        }
    }

    /// Draws one handle, emphasising the one being dragged.
    ///
    /// The two weights are the pair the theme's own state overlays use (a hover step and a press
    /// step of the same accent), so a handle does not invent a third visual language, and the drag
    /// weight is the stronger of the two because "you are holding this" is a stronger statement
    /// than "the pointer is somewhere over this control".
    fn draw_handle(
        context: &mut RenderContext,
        center: Point,
        interaction: HandleInteraction,
        radius: u32,
        fill: Color,
        border: Color,
    ) {
        match interaction {
            HandleInteraction::Dragging => {
                // A halo *and* a larger disc: the halo says "held", the size says "this is the one".
                context.fill_circle(center, radius + 4, fill.with_alpha(56));
                context.fill_circle(center, radius + 2, fill);
                context.draw_circle_stroke(center, radius + 2, border, 2);
            }
            HandleInteraction::Idle => {
                context.fill_circle(center, radius + 1, fill.with_alpha(28));
                context.fill_circle(center, radius, fill);
                context.draw_circle_stroke(center, radius, border, 2);
            }
            HandleInteraction::Resting => {
                context.fill_circle(center, radius, fill);
                context.draw_circle_stroke(center, radius, border, 2);
            }
        }
    }
}

impl EventHandler for RangeSlider {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button, .. } if *button == 1 => {
                let rect = self.geometry();
                // Check which handle is hit (upper first to give it priority)
                if self.is_handle_hit(*pos, &rect, false) {
                    self.dragging = Some(false); // upper handle
                } else if self.is_handle_hit(*pos, &rect, true) {
                    self.dragging = Some(true); // lower handle
                }
            }
            Event::MouseRelease { pos: _, button } if *button == 1 => {
                self.dragging = None;
            }
            // A press whose release lands outside the widget never reaches the arm
            // above: the runtime's hit-test returns `None` for a point outside every
            // control, so no `MouseRelease` is delivered. Without this arm `dragging`
            // stayed set and the next hover kept moving a handle with no button held.
            Event::MouseLeave { .. } if self.dragging.is_some() => {
                self.dragging = None;
            }
            Event::MouseMove { pos } => {
                if let Some(is_lower) = self.dragging {
                    let rect = self.geometry();
                    let raw_value = self.pixel_to_value(pos.x, &rect);
                    if is_lower {
                        self.set_lower_value(raw_value);
                    } else {
                        self.set_upper_value(raw_value);
                    }
                }
            }
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::svg::render_to_svg;

    #[test]
    fn range_slider_default_creation() {
        let rs = RangeSlider::new(Rect::new(0, 0, 300, 40));
        assert_eq!(rs.kind(), WidgetKind::RangeSlider);
        assert!((rs.lower_value() - 25.0).abs() < f64::EPSILON);
        assert!((rs.upper_value() - 75.0).abs() < f64::EPSILON);
        assert!((rs.min_value() - 0.0).abs() < f64::EPSILON);
        assert!((rs.max_value() - 100.0).abs() < f64::EPSILON);
        assert!((rs.step() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn range_slider_set_lower_value_respects_bounds() {
        let mut rs = RangeSlider::new(Rect::new(0, 0, 300, 40));
        rs.set_lower_value(30.0);
        assert!((rs.lower_value() - 30.0).abs() < f64::EPSILON);

        // Cannot exceed upper_value
        rs.set_lower_value(80.0);
        assert!((rs.lower_value() - 75.0).abs() < f64::EPSILON); // clamped to upper - min_range

        // Cannot go below min_value
        rs.set_lower_value(-10.0);
        assert!((rs.lower_value() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn range_slider_set_upper_value_respects_bounds() {
        let mut rs = RangeSlider::new(Rect::new(0, 0, 300, 40));
        rs.set_upper_value(90.0);
        assert!((rs.upper_value() - 90.0).abs() < f64::EPSILON);

        // Cannot go below lower_value
        rs.set_upper_value(10.0);
        assert!((rs.upper_value() - 25.0).abs() < f64::EPSILON); // clamped to lower + min_range

        // Cannot exceed max_value
        rs.set_upper_value(200.0);
        assert!((rs.upper_value() - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn range_slider_set_range() {
        let mut rs = RangeSlider::new(Rect::new(0, 0, 300, 40));
        rs.set_range(10.0, 50.0);
        assert!((rs.lower_value() - 10.0).abs() < f64::EPSILON);
        assert!((rs.upper_value() - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn range_slider_step_respected() {
        let mut rs = RangeSlider::new(Rect::new(0, 0, 300, 40));
        rs.set_step(5.0);

        rs.set_lower_value(12.0);
        assert!((rs.lower_value() - 10.0).abs() < f64::EPSILON); // rounds to nearest step (10)

        rs.set_lower_value(13.0);
        assert!((rs.lower_value() - 15.0).abs() < f64::EPSILON); // rounds to nearest step (15)
    }

    #[test]
    fn range_slider_min_range() {
        let mut rs = RangeSlider::new(Rect::new(0, 0, 300, 40));
        rs.set_min_range(20.0);
        assert!((rs.min_range() - 20.0).abs() < f64::EPSILON);

        // Enforce min_range
        rs.set_lower_value(80.0);
        assert!((rs.lower_value() - 55.0).abs() < f64::EPSILON); // 75 - 20 = 55
    }

    #[test]
    fn range_slider_range_changed_signal() {
        use std::sync::{Arc, Mutex};
        let mut rs = RangeSlider::new(Rect::new(0, 0, 300, 40));
        let captured = Arc::new(Mutex::new(None::<(f64, f64)>));
        rs.range_changed.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<(f64, f64)>| {
                *captured.lock().unwrap() = Some(*val);
            }
        });

        rs.set_lower_value(40.0);
        let result = captured.lock().unwrap();
        let (lower, upper) = result.unwrap();
        assert!((lower - 40.0).abs() < f64::EPSILON);
        assert!((upper - 75.0).abs() < f64::EPSILON);
    }

    #[test]
    fn range_slider_svg_output() {
        let mut rs = RangeSlider::new(Rect::new(0, 0, 300, 40));
        let svg = render_to_svg(&mut rs);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    /// A right-to-left selector puts the lower value at the **right** edge.
    ///
    /// # What this pins
    ///
    /// BLUE22 · F-4. A range selector's two handles sit on a line, and "the lower value is nearer the
    /// start" is a statement about that line's reading order. In an Arabic or Hebrew locale the line
    /// begins at the right, so the lower handle belongs on the right; without this the handles moved
    /// the opposite way from the numbers they show, which is not a translation defect but a wrong
    /// reading of the same control.
    #[test]
    fn a_right_to_left_selector_mirrors_its_handles() {
        let rect = Rect::new(0, 0, 300, 40);
        let mut ltr = RangeSlider::new(rect);
        ltr.set_range(0.0, 100.0);
        ltr.set_lower_value(25.0);
        ltr.set_upper_value(75.0);
        let (ltr_lower, ltr_upper) =
            (ltr.value_to_pixel(25.0, &rect), ltr.value_to_pixel(75.0, &rect));

        let mut rtl = RangeSlider::new(rect);
        rtl.set_direction(crate::core::TextDirection::RightToLeft);
        let (rtl_lower, rtl_upper) =
            (rtl.value_to_pixel(25.0, &rect), rtl.value_to_pixel(75.0, &rect));

        assert!(ltr_lower < ltr_upper, "LTR puts the lower value on the left");
        assert!(rtl_lower > rtl_upper, "RTL puts it on the right: {rtl_lower} vs {rtl_upper}");
        // The mirror is exact: the two frames are reflections about the track's centre, so a value
        // keeps its *distance from its own beginning*.
        let (begin, length) = ltr.track_begin_and_length(&rect);
        assert_eq!(ltr_lower - begin, (begin + length) - rtl_lower);
        assert_eq!(ltr_upper - begin, (begin + length) - rtl_upper);
    }

    /// The two mappings are inverses in **both** directions, using one inset.
    ///
    /// # Why this is the important test
    ///
    /// BLUE22 §4.3 records the defect this guards: `slider`'s `value_to_pixel` inset by half a handle
    /// while its `pixel_to_value` used the full width, so the two were not inverses and a click on a
    /// handle returned a different value from the one the handle was drawn at. `range_slider` was
    /// right at the time because both of its arms happened to spell the same `8`; `value_to_pixel` and
    /// `pixel_to_value` now share [`RangeSlider::track_begin_and_length`] outright, so the relation is
    /// a property of the code rather than of two literals agreeing.
    ///
    /// The round trip is checked in both directions and at the extremes, where an off-by-one inset is
    /// most visible.
    #[test]
    fn the_two_mappings_round_trip_in_both_directions() {
        let rect = Rect::new(0, 0, 300, 40);
        for direction in
            [crate::core::TextDirection::LeftToRight, crate::core::TextDirection::RightToLeft]
        {
            let mut rs = RangeSlider::new(rect);
            rs.set_direction(direction);
            rs.set_range(0.0, 100.0);
            for value in [0.0, 1.0, 25.0, 50.0, 99.0, 100.0] {
                let pixel = rs.value_to_pixel(value, &rect);
                let back = rs.pixel_to_value(pixel, &rect);
                assert!(
                    (back - value).abs() <= 1.0,
                    "{direction:?}: value {value} -> x {pixel} -> {back}"
                );
            }
        }
    }

    /// The vertical axis is not mirrored, so the two orientations of one value agree on which end
    /// the minimum sits at.
    ///
    /// The block axis runs top-to-bottom in every direction — the same rule `slider` and
    /// `progress_bar` state — so a vertical selector must ignore the direction entirely rather than
    /// flipping its values upside down in an RTL locale.
    #[test]
    fn the_vertical_axis_ignores_direction() {
        let rect = Rect::new(0, 0, 40, 300);
        let mut rtl = RangeSlider::new(rect);
        rtl.set_orientation(RangeSliderOrientation::Vertical);
        rtl.set_direction(crate::core::TextDirection::RightToLeft);
        rtl.set_range(0.0, 100.0);
        let mut ltr = RangeSlider::new(rect);
        ltr.set_orientation(RangeSliderOrientation::Vertical);
        ltr.set_range(0.0, 100.0);
        for value in [0.0, 50.0, 100.0] {
            assert_eq!(
                rtl.value_to_pixel(value, &rect),
                ltr.value_to_pixel(value, &rect),
                "value {value} must land at the same row either way"
            );
        }
        // And the minimum is still at the **bottom**, which is the reading the horizontal arm's
        // left-to-right case gives at the left edge.
        assert!(ltr.value_to_pixel(0.0, &rect) > ltr.value_to_pixel(100.0, &rect));
    }

    /// A click on a handle returns the value the handle was drawn at.
    ///
    /// The end-to-end form of the round-trip test above, driven through the hit test the event
    /// handler uses: the point at a handle's own drawn centre must resolve to *that* handle, in both
    /// directions. This is what a user experiences as "dragging works", and it is the property §4.3
    /// says the two mappings must share one inset to have.
    #[test]
    fn clicking_a_handle_finds_that_handle_in_both_directions() {
        let rect = Rect::new(0, 0, 300, 40);
        for direction in
            [crate::core::TextDirection::LeftToRight, crate::core::TextDirection::RightToLeft]
        {
            let mut rs = RangeSlider::new(rect);
            rs.set_direction(direction);
            rs.set_range(0.0, 100.0);
            rs.set_lower_value(20.0);
            rs.set_upper_value(80.0);
            let centre_y = rect.y + rect.height as i32 / 2;
            let lower_centre = Point::new(rs.value_to_pixel(20.0, &rect), centre_y);
            let upper_centre = Point::new(rs.value_to_pixel(80.0, &rect), centre_y);
            assert!(
                rs.is_handle_hit(lower_centre, &rect, true),
                "{direction:?}: the lower handle's own centre must hit the lower handle"
            );
            assert!(
                rs.is_handle_hit(upper_centre, &rect, false),
                "{direction:?}: the upper handle's own centre must hit the upper handle"
            );
            // Each handle's centre is the *other* handle's answer only when the two coincide, which
            // they do not here — so a hit test that ignored the argument would be caught.
            assert!(!rs.is_handle_hit(lower_centre, &rect, false));
            assert!(!rs.is_handle_hit(upper_centre, &rect, true));
        }
    }

    /// A screen reader is told the range, not silence and not half of it.
    ///
    /// # The defect this pins
    ///
    /// `Widget::accessible_value`'s default reads a value property from a fixed list -- `value` /
    /// `progress` / `rating` / `level` -- and `range_slider` publishes `lower` / `upper` instead, so
    /// it announced **nothing** about where the range sat. Renaming a property is not the fix: two
    /// handles are the contract, and announcing one would report half a range as if it were the
    /// value.
    ///
    /// The assertion names both ends, so an implementation that silently announced only `lower`
    /// (the tempting shortcut) fails.
    #[test]
    fn the_range_is_announced_with_both_handles() {
        let mut rs = RangeSlider::new(Rect::new(0, 0, 200, 28));
        rs.set_range(0.0, 100.0);
        rs.set_lower_value(20.0);
        rs.set_upper_value(70.0);

        let announced = rs.accessible_value();
        assert!(announced.contains("20"), "the lower handle must be announced, got {announced:?}");
        assert!(announced.contains("70"), "and the upper handle too, got {announced:?}");
        assert!(
            announced.contains('\u{2013}'),
            "the two ends must read as one range rather than two sentences, got {announced:?}"
        );

        // An integral float is announced as its integer, which is the one formatting rule the
        // capability layer already owns -- asserted here so this control cannot grow a second one.
        assert_eq!(announced, "20\u{2013}70");
    }

    /// The handle being dragged is drawn larger than a resting one.
    ///
    /// # The defect this pins
    ///
    /// `dragging` was stored, set and cleared correctly, and **never painted**: grabbing a handle
    /// showed the frame the user had seen before touching it, so the only feedback about which of
    /// two overlapping discs was held came from the value moving afterwards. The assertion compares
    /// two *renders* driven through the real event path, so a draw that ignores the drag fails
    /// however correct the field is — the model-versus-pixels pair this crate keeps re-learning.
    #[test]
    fn the_dragged_handle_is_drawn_larger() {
        let mut rs = RangeSlider::new(Rect::new(0, 0, 200, 28));
        rs.set_range(0.0, 100.0);
        rs.set_lower_value(20.0);
        rs.set_upper_value(70.0);

        let resting = crate::widget::svg::render_to_svg(&mut rs);
        assert!(!rs.is_dragging());

        // Press on the lower handle's own centre. `value_to_pixel` is the same mapping the draw
        // uses, so the press lands exactly where that handle was painted.
        let rect = rs.geometry();
        let lower_centre = crate::core::Point::new(
            rs.value_to_pixel(rs.lower_value(), &rect),
            rect.y + rect.height as i32 / 2,
        );
        rs.handle_event(&Event::MousePress { pos: lower_centre, button: 1, modifiers: 0 });
        assert!(rs.is_dragging(), "a press on a handle must arm the drag");

        let dragged = crate::widget::svg::render_to_svg(&mut rs);
        assert_ne!(resting, dragged, "a held handle must not paint like a resting one");

        // Releasing returns the handle to its resting size, so the emphasis is not latched.
        rs.handle_event(&Event::MouseRelease { pos: lower_centre, button: 1 });
        assert!(!rs.is_dragging());
        let released = crate::widget::svg::render_to_svg(&mut rs);
        assert_eq!(released, resting, "releasing must restore the resting frame");
    }
}
