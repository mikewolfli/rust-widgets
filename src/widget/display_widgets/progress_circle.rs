// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! ProgressCircle widget — a circular progress indicator.
//!
//! The ProgressCircle widget displays a circular progress track with a filled arc
//! representing the current progress value (0.0 to 1.0). It supports both determinate
//! mode (showing actual progress) and indeterminate mode (animated spinning arc).
//! The track and progress colors, as well as stroke width, are customizable.

use core::fmt::Write as _;

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::style::{MotionSlot, PropertyDriver};
use crate::widget::capability::coercion::{expect_bool, expect_f64, expect_string, expect_u32};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// The gap left where the progress arc meets the track, in logical pixels.
///
/// Material's `ProgressIndicator.trackGap` (`progress_indicator.dart:1636`). It is the
/// difference between "this ring is 0%" and "this ring is broken": with a gap, a value of
/// zero still shows two distinct ends, while without one the arc and the track are the same
/// stroke at the same angle and the reader cannot tell the two apart at all.
const TRACK_GAP: f32 = 4.0;

/// How long one full revolution of the indeterminate sweep takes, in milliseconds.
///
/// # Why this is a multiple of the theme's `slow` token
///
/// The sweep is a *loop*, not a state change: `Motion::slow` is sized for a transition that
/// ends (300 ms in the preset), and a ring completing a revolution that fast reads as a
/// stutter rather than as progress. The revolution is therefore paced from `slow` scaled up
/// by [`SPIN_SLOW_MULTIPLE`], so a theme that slows everything down slows the ring with it,
/// and the no-theme fallback is this constant — the value the preset produces.
const DEFAULT_SPIN_PERIOD_MS: u32 = 1400;

/// How many `slow` tokens one revolution is worth.
const SPIN_SLOW_MULTIPLE: u32 = 5;

/// ProgressCircle widget — a Material-style circular progress indicator.
///
/// In determinate mode, draws a track circle and a progress arc.
/// In indeterminate mode, draws a spinning arc segment that cycles around the circle.
///
/// The widget uses the `stroke_width` to determine the thickness of both the
/// track and progress arcs. The progress arc is drawn using closely-spaced
/// line segments approximating a circular arc.
pub struct ProgressCircle {
    base: BaseWidget,
    /// The determinate value, driven rather than assigned directly.
    ///
    /// [`ProgressCircle::set_value`] aims the driver, [`ProgressCircle::tick`] advances it, and
    /// `draw` paints [`PropertyDriver::value`]. The published value ([`ProgressCircle::value`])
    /// is the driver's target, so a caller immediately reads back what it wrote while the ring
    /// travels toward it over the theme's motion token.
    value_animation: PropertyDriver,
    indeterminate: bool,
    track_color: Color,
    /// True when the caller assigned a track colour explicitly.
    ///
    /// The theme's groove colour wins by default (a near-white literal made the track identical
    /// in both appearances), but an explicit `set_track_color` must still take effect — it used to
    /// be stored, read back, and never painted.
    track_color_is_explicit: bool,
    progress_color: Color,
    stroke_width: f32,
    diameter: u32,
    /// The indeterminate sweep's rotation, in turns: `0.0` and `1.0` are the same angle.
    ///
    /// # Why this is a field and not `SystemTime::now()`
    ///
    /// The sweep used to read the wall clock inside `draw`. That made the control's appearance
    /// a function of something no test could set, so the only way to assert the animation moved
    /// was to sleep; it also meant the ring kept rotating in a frame nobody had asked for, and
    /// that it could not participate in `Widget::tick` at all. A turning ring is state, and state
    /// belongs to the control: the frame bus advances it through [`ProgressCircle::tick`], and a
    /// paused or idle app simply does not advance it.
    sweep: f32,
    /// Emitted with the aimed-at value whenever [`ProgressCircle::set_value`] changes it.
    pub value_changed: Signal1<f32>,
    /// Emitted with the new flag when [`ProgressCircle::set_indeterminate`] changes it.
    pub indeterminate_changed: Signal1<bool>,
    /// Whether the current value is painted as text in the ring's hole.
    show_center_text: bool,
    /// The template the centre text is rendered from.
    ///
    /// `{percent}` is replaced with the value as a rounded percentage and `{value}` with
    /// the value in `0.0..=1.0` to two decimals; every other character is copied through.
    center_text_format: String,
    /// The rendered centre reading and its measurement, reused while its inputs are unchanged.
    center_text_cache: Option<CenterTextCache>,
}

/// The centre reading and the exact inputs that produced it.
///
/// During a value tween the reading is recomputed and re-measured on every frame even though the
/// rounded percentage — the only thing the default template prints — usually did not move. The key
/// is the same one `arc` uses: the driving value, the template, the resolved face and the device
/// scale.
struct CenterTextCache {
    value_bits: u32,
    format: String,
    font: Font,
    dpi_scale_bits: u32,
    show_center_text: bool,
    indeterminate: bool,
    text: String,
    width: u32,
    height: u32,
}

impl ProgressCircle {
    /// Creates a new ProgressCircle widget with the given geometry.
    ///
    /// Defaults: value 0.0, determinate, light gray track, and the theme's value-indicator
    /// colour for the arc, stroke width 4.
    ///
    /// `progress_color` starts as the token `progress_bar` uses for its fill — the theme's
    /// `Accent` role colour — rather than the `Color::PRIMARY` literal it carried before.
    /// The two progress controls are the same reading of the same data, so a caller who
    /// left the colour alone must not get a blue ring beside an orange bar. The literal is
    /// still the no-theme fallback, so a build without a theme renders what it used to.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ProgressCircle, geometry, "ProgressCircle"),
            // Born at rest, so a freshly built ring paints no arc and does not animate away from
            // its own state on the first frame. A caller that wants a value aims the driver with
            // `set_value`.
            value_animation: PropertyDriver::at(0.0, MotionSlot::Normal),
            indeterminate: false,
            track_color: Color::rgba(220, 220, 220, 200),
            track_color_is_explicit: false,
            progress_color: crate::style::resolved_theme_style_shared("progress_bar")
                .and_then(|resolved| resolved.background_color)
                .unwrap_or(Color::PRIMARY),
            stroke_width: 4.0,
            diameter: geometry.width.min(geometry.height),
            sweep: 0.0,
            value_changed: Signal1::new(),
            indeterminate_changed: Signal1::new(),
            show_center_text: false,
            center_text_format: "{percent}%".to_string(),
            center_text_cache: None,
        }
    }

    /// Returns the current progress value (0.0 to 1.0).
    ///
    /// This is the value the control has been **aimed** at. The arc paints the driver's current
    /// value, which approaches this one while the tween is running; see
    /// [`ProgressCircle::tick`].
    pub fn value(&self) -> f32 {
        self.value_animation.target()
    }

    /// Sets the progress value. Clamped to 0.0..=1.0.
    ///
    /// The value is reached by an animated tween driven by [`ProgressCircle::tick`]. A
    /// same-value write is a no-op, so progress controls that are written every frame do not
    /// restart the movement. When the environment asks for reduced motion the driver jumps
    /// straight to the target instead.
    pub fn set_value(&mut self, value: f32) {
        let clamped = value.clamp(0.0, 1.0);
        if self.value_animation.target() == clamped {
            return;
        }
        self.value_animation.set_target(clamped);
        if crate::style::environment::environment().prefers_reduced_motion() {
            self.value_animation.jump_to(clamped);
        }
        self.request_frame_repaint(None);
        self.value_changed.emit(clamped);
    }

    /// Returns whether the indicator is in indeterminate (spinning) mode.
    pub fn is_indeterminate(&self) -> bool {
        self.indeterminate
    }

    /// Sets whether the indicator shows indeterminate (spinning) animation.
    pub fn set_indeterminate(&mut self, indeterminate: bool) {
        if self.indeterminate == indeterminate {
            return;
        }
        self.indeterminate = indeterminate;
        // Entering the spinning state starts from the top rather than wherever the last sweep
        // stopped, so two rings switched on at different times still read as one animation.
        if indeterminate {
            self.sweep = 0.0;
        }
        self.request_frame_repaint(None);
        self.indeterminate_changed.emit(indeterminate);
    }

    /// Submits one animated frame as damage bounded by the painted box plus the
    /// focus-ring and halo margin.
    ///
    /// `bounds` is supplied by [`Self::tick`], because the animation sweep calls `tick`
    /// while it holds the widget registry; passing the rectangle keeps that path from
    /// asking the runtime to read the geometry back. Event and setter paths pass `None`
    /// and let the runtime derive the box.
    ///
    /// A control that is unmounted falls back to [`BaseWidget::request_redraw`], which
    /// keeps the redraw signal contract for host-owned controls. An interaction frame in
    /// [`crate::widget::runtime::RepaintMode::Full`] still routes through
    /// [`crate::widget::runtime::request_repaint_rect`], whose documented fallback repaints
    /// the whole control; a tick frame keeps the base signal there because the animation
    /// sweep holds the registry across `tick`.
    fn request_frame_repaint(&self, bounds: Option<Rect>) {
        let _ = bounds;
        #[cfg(not(alloc_frugal))]
        {
            if let Some(id) = crate::widget::runtime::registry_id_of(self.id()) {
                let narrow = crate::widget::runtime::repaint_mode(id)
                    != crate::widget::runtime::RepaintMode::Full;
                // `bounds` is `Some` only from `tick`, which the animation sweep calls
                // while it holds the registry; the whole-control fallback would re-enter
                // that borrow, so `Full` stays on the base signal there. Interaction paths
                // may take the documented fallback.
                if narrow || bounds.is_none() {
                    let damage = bounds
                        .or_else(|| crate::widget::runtime::painted_box_with_damage_margin(id))
                        .unwrap_or_else(|| self.damage_bounds());
                    crate::widget::runtime::request_repaint_rect(id, damage);
                    // The replaced `request_redraw` emitted this signal; keep it.
                    self.base.redraw_requested.emit();
                    return;
                }
            }
        }
        self.base.request_redraw();
    }

    /// This control's own rectangle grown by
    /// [`crate::widget::metrics::DAMAGE_MARGIN`].
    ///
    /// The animation sweep holds the registry across `tick`, so the geometry lookup
    /// [`crate::widget::runtime::painted_box_with_damage_margin`] performs cannot run
    /// there. Deriving the rectangle from `Widget::geometry` yields the same answer the
    /// runtime returns for the mounted control.
    fn damage_bounds(&self) -> Rect {
        let rect = self.geometry();
        let margin = crate::widget::metrics::DAMAGE_MARGIN;
        let offset = margin as i32;
        Rect::new(
            rect.x.saturating_sub(offset),
            rect.y.saturating_sub(offset),
            rect.width.saturating_add(margin).saturating_add(margin),
            rect.height.saturating_add(margin).saturating_add(margin),
        )
    }

    /// The indeterminate sweep's current angle, in radians.
    ///
    /// Exposed so a test can assert the ring actually turns across frames without sleeping:
    /// the value is a function of the ticks the control was given, not of the wall clock.
    pub fn sweep_angle(&self) -> f32 {
        self.sweep * 2.0 * core::f32::consts::PI
    }

    /// Advances the indeterminate sweep, or the determinate value tween, by `delta_ms`.
    ///
    /// An indeterminate ring turns and always owes another frame. A determinate ring owes
    /// frames only while its value is still travelling; once it settles it is a still picture
    /// and this returns `false`, which is \u00a73.2's "no animation, no repaint". The landing frame
    /// is still repainted, because that is the frame that shows the exact target value.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        if self.indeterminate {
            let period = self.spin_period_ms();
            // Wrapped rather than clamped: the sweep is a revolution, so `1.0` is `0.0` and the
            // fraction is kept in `0.0..1.0` so a long-running app cannot drift into a large value
            // and lose float precision on the angle.
            self.sweep = (self.sweep + delta_ms as f32 / period as f32).fract();
            self.request_frame_repaint(Some(self.damage_bounds()));
            return true;
        }
        let was_moving = self.value_animation.is_moving();
        let moving = self.value_animation.tick(delta_ms);
        // The tick that lands on the target reports `false`, but it also writes the final value,
        // so it is still a frame that must be painted.
        if was_moving {
            self.request_frame_repaint(Some(self.damage_bounds()));
        }
        moving
    }

    /// The duration of one full revolution, priced by the theme.
    fn spin_period_ms(&self) -> u32 {
        let slow = crate::style::motion_tokens().2;
        if slow == 0 {
            DEFAULT_SPIN_PERIOD_MS
        } else {
            slow.saturating_mul(SPIN_SLOW_MULTIPLE).max(1)
        }
    }

    /// Returns the current track color.
    pub fn track_color(&self) -> Color {
        self.track_color
    }

    /// Sets the track (background circle) color.
    pub fn set_track_color(&mut self, color: Color) {
        if self.track_color == color {
            return;
        }
        self.track_color = color;
        self.track_color_is_explicit = true;
        self.base.request_redraw();
    }

    /// Returns the current progress (foreground arc) color.
    pub fn progress_color(&self) -> Color {
        self.progress_color
    }

    /// Sets the progress (foreground arc) color.
    pub fn set_progress_color(&mut self, color: Color) {
        if self.progress_color == color {
            return;
        }
        self.progress_color = color;
        self.base.request_redraw();
    }

    /// Returns the current stroke width in logical pixels.
    pub fn stroke_width(&self) -> f32 {
        self.stroke_width
    }

    /// Returns the arc thickness published as the `thickness` property.
    ///
    /// The widget draws both the track and the progress arc with a single stroke
    /// width, so `thickness` and [`ProgressCircle::stroke_width`] are the same
    /// quantity read through the name the schema declares.
    pub fn thickness(&self) -> f32 {
        self.stroke_width
    }

    /// Returns the diameter of the progress circle.
    pub fn diameter(&self) -> u32 {
        self.diameter
    }

    /// Sets the diameter of the progress circle.
    pub fn set_diameter(&mut self, diameter: u32) {
        let diameter = diameter.max(1);
        if self.diameter == diameter {
            return;
        }
        self.diameter = diameter;
        self.base.request_redraw();
    }

    /// Sets the stroke width for both track and progress arcs.
    pub fn set_stroke_width(&mut self, width: f32) {
        let width = width.max(0.5);
        if self.stroke_width == width {
            return;
        }
        self.stroke_width = width;
        self.base.request_redraw();
    }

    /// Whether the current value is painted as text in the ring's hole.
    pub fn show_center_text(&self) -> bool {
        self.show_center_text
    }

    /// Turns the centre value text on or off.
    pub fn set_show_center_text(&mut self, show: bool) {
        if self.show_center_text == show {
            return;
        }
        self.show_center_text = show;
        self.base.request_redraw();
    }

    /// The template the centre text is rendered from.
    pub fn center_text_format(&self) -> &str {
        &self.center_text_format
    }

    /// Replaces the centre-text template.
    ///
    /// See [`Self::center_text_format`] for the two placeholders the template understands.
    pub fn set_center_text_format(&mut self, format: impl Into<String>) {
        let format = format.into();
        if self.center_text_format == format {
            return;
        }
        self.center_text_format = format;
        self.base.request_redraw();
    }

    /// Renders the centre text into `out`.
    ///
    /// Only determinate mode has a reading to print, so an indeterminate ring writes nothing.
    fn write_center_text(&self, out: &mut String) {
        if !self.show_center_text || self.indeterminate {
            return;
        }
        let value = self.value_animation.value();
        let mut rest = self.center_text_format.as_str();
        while let Some(open) = rest.find('{') {
            out.push_str(&rest[..open]);
            let tail = &rest[open..];
            if let Some(stripped) = tail.strip_prefix("{percent}") {
                let _ = write!(out, "{}", (value * 100.0).round() as u32);
                rest = stripped;
            } else if let Some(stripped) = tail.strip_prefix("{value}") {
                let _ = write!(out, "{value:.2}");
                rest = stripped;
            } else {
                // An unknown brace is literal text rather than a reason to drop the rest.
                out.push('{');
                rest = &tail[1..];
            }
        }
        out.push_str(rest);
    }

    /// Rebuilds [`Self::center_text_cache`] only when the reading or its presentation changed.
    fn ensure_center_text(&mut self, context: &RenderContext) {
        let value = self.value_animation.value();
        let value_bits = value.to_bits();
        let dpi_scale_bits = context.dpi_scale().to_bits();
        let font = effective_font(self.base.style()).clone();
        let fresh = self.center_text_cache.as_ref().is_some_and(|cache| {
            cache.value_bits == value_bits
                && cache.format == self.center_text_format
                && cache.font == font
                && cache.dpi_scale_bits == dpi_scale_bits
                && cache.show_center_text == self.show_center_text
                && cache.indeterminate == self.indeterminate
        });
        if fresh {
            return;
        }
        let mut text = String::new();
        self.write_center_text(&mut text);
        let metrics = context.measure_text(&text, &font);
        self.center_text_cache = Some(CenterTextCache {
            value_bits,
            format: self.center_text_format.clone(),
            font,
            dpi_scale_bits,
            show_center_text: self.show_center_text,
            indeterminate: self.indeterminate,
            text,
            width: metrics.width,
            height: metrics.height,
        });
    }

    /// Computes the center point and radius of the progress circle.
    fn center_and_radius(&self) -> Option<(Point, f32)> {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return None;
        }
        let cx = rect.x + (rect.width as i32) / 2;
        let cy = rect.y + (rect.height as i32) / 2;
        let radius = (rect.width.min(rect.height) as f32 / 2.0) - self.stroke_width / 2.0 - 1.0;
        if radius <= 0.0 {
            return None;
        }
        Some((Point::new(cx, cy), radius))
    }
}

impl Widget for ProgressCircle {
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
        let diameter =
            self.diameter.max(crate::widget::metrics::dimensions::PROGRESS_CIRCLE_DEFAULT_DIAMETER);
        Size::new(diameter, diameter)
    }

    /// One frame of the indeterminate sweep or the determinate value tween.
    fn tick(&mut self, delta_ms: u32) -> bool {
        ProgressCircle::tick(self, delta_ms)
    }

    /// The ring owes frames while spinning or while its value is still travelling.
    fn is_animating(&self) -> bool {
        self.indeterminate || self.value_animation.is_moving()
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();

    /// Resolves a published event name to the signal the ring emits.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "value_changed" => {
                Some(EventSignalRef::mapped("value_changed", &self.value_changed, |value| {
                    CapabilityValue::Float(f64::from(*value))
                }))
            }
            "indeterminate_changed" => Some(EventSignalRef::mapped(
                "indeterminate_changed",
                &self.indeterminate_changed,
                |value| CapabilityValue::Bool(*value),
            )),
            _ => None,
        }
    }
}

/// `ProgressCircle`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. The schema publishes the arc
/// thickness as `thickness` although the widget draws through a single stroke
/// width, so the two names address the same field.
impl WidgetProperties for ProgressCircle {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "value" => Ok(CapabilityValue::Float(f64::from(self.value()))),
            "thickness" => Ok(CapabilityValue::Float(f64::from(self.thickness()))),
            "progress_color" => Ok(CapabilityValue::Color(self.progress_color())),
            "diameter" => Ok(CapabilityValue::UInt(self.diameter() as u64)),
            "indeterminate" => Ok(CapabilityValue::Bool(self.is_indeterminate())),
            "show_center_text" => Ok(CapabilityValue::Bool(self.show_center_text())),
            "center_text_format" => {
                Ok(CapabilityValue::String(self.center_text_format().to_string()))
            }
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "value" => {
                self.set_value(expect_f64(value)? as f32);
                Ok(())
            }
            "thickness" => {
                self.set_stroke_width(expect_f64(value)? as f32);
                Ok(())
            }
            "progress_color" => {
                // Accept the declared kind; a CSS string is still accepted because the C ABI and
                // JSON surfaces carry colours as text (the same allowance `color_well` makes).
                let color = match value {
                    CapabilityValue::Color(color) => color,
                    CapabilityValue::String(raw) => match crate::core::Color::parse_hex(&raw) {
                        Some(color) => color,
                        None => return Err(CapabilityAccessError::TypeMismatch),
                    },
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_progress_color(color);
                Ok(())
            }
            "diameter" => {
                self.set_diameter(expect_u32(value)?);
                Ok(())
            }
            "indeterminate" => {
                self.set_indeterminate(expect_bool(value)?);
                Ok(())
            }
            "show_center_text" => {
                self.set_show_center_text(expect_bool(value)?);
                Ok(())
            }
            "center_text_format" => {
                self.set_center_text_format(expect_string(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "value",
            "thickness",
            "progress_color",
            "diameter",
            "indeterminate",
            "show_center_text",
            "center_text_format",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `progress_circle` publishes.
    ///
    /// All three assign state — the progress value, the stroke width and the
    /// indeterminate flag — so each needs an argument a command carries none of. They
    /// are refused as [`CapabilityAccessError::OutOfRange`] (use the property route
    /// `set("value", ..)` / `set("thickness", ..)` / `set("indeterminate", ..)`)
    /// rather than reported as unknown.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_value" | "set_thickness" | "set_indeterminate" => {
                Err(CapabilityAccessError::OutOfRange)
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for ProgressCircle {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        let Some((center, radius)) = self.center_and_radius() else {
            return;
        };

        let is_enabled = self.base.is_enabled();
        let stroke_w = self.stroke_width as u32;

        // Draw track (full circle). The track is the ring's empty groove — chrome, not a
        // value — so it follows the theme. Chrome colours resolve the explicit style first,
        // then the theme's resolved style for this control, and only then the original
        // literal, which stays as the fallback so an inactive theme still has a defined
        // appearance. The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style();
        let themed = crate::style::resolved_theme_style_shared("progress_circle");
        let themed_bg = themed.as_ref().and_then(|r| r.background_color);
        // `progress_circle` is `WidgetRole::Accent`, so the theme writes a groove colour into
        // the style's background. The control's own `track_color` literal is deliberately
        // **not** the base here: it is a near-white grey in every palette, so using it would
        // leave the track identical in light and dark — the exact defect being fixed. It
        // stays as the enabled-state literal fallback instead.
        let fallback_track = themed_bg.unwrap_or(Color::rgba(220, 220, 220, 200));
        let surface = style.background_color.unwrap_or(fallback_track);
        // An explicit `set_track_color` wins over the theme groove (it is a caller decision, like
        // every other explicit colour in this crate); with no explicit colour the theme's groove
        // is used, because the field's near-white literal made the track identical in both
        // appearances. A disabled control derives its groove from the resolved colours instead.
        let track_color = if !is_enabled {
            surface.blend(&Color::DISABLED_FOREGROUND, 0.5)
        } else if self.track_color_is_explicit {
            self.track_color
        } else {
            surface
        };

        // Draw track as a circle stroke
        context.draw_circle_stroke(center, radius as u32, track_color, stroke_w.max(1));

        // The progress arc is deliberately NOT themed: its colour can encode a value or a
        // threshold the caller chose, which is data, so its computation is left untouched.

        if self.indeterminate {
            // The arc sweeps a fixed share of the circle and the whole thing revolves, which is
            // the shape Material's indeterminate indicator describes: the arc's own length says
            // "work is happening", the revolution says "and it has not stopped".
            //
            // The angle comes from `self.sweep`, advanced by `tick`, so nothing here reads the
            // clock: two frames of the same control with the same tick history paint the same
            // ring, which is what makes the animation assertable without sleeping.
            let arc_sweep = 2.4; // ~135 degrees in radians
            let start_angle = self.sweep_angle();
            let end_angle = start_angle + arc_sweep;

            let prog_color =
                if is_enabled { self.progress_color } else { Color::DISABLED_FOREGROUND };
            crate::render::draw_arc_segments(
                context,
                center,
                radius,
                start_angle,
                end_angle,
                prog_color,
                stroke_w.max(1),
            );
        } else {
            // The arc paints the driver's current value, not its target: that difference is the
            // tween. After the driver settles the two are equal, so a still ring is unchanged.
            //
            // The arc is skipped rather than returning from `draw` when it has nothing to paint,
            // because the centre reading below belongs to a 0% ring too.
            let value = self.value_animation.value();
            if value > 0.0 {
                // In determinate mode, draw the progress arc from 12 o'clock
                // -PI/2 (12 o'clock) to -PI/2 + 2*PI*value
                let start_angle = -std::f32::consts::FRAC_PI_2;
                // A non-zero value starts after a `TRACK_GAP`, so the arc's two ends are distinct
                // from the track's two ends even at the smallest visible value. The gap is converted
                // from pixels to radians at the drawn radius, so the visible breaking is the same
                // size on a small ring and a large one.
                let gap_radians = TRACK_GAP / radius.max(1.0);
                let start_angle = start_angle + gap_radians;
                let end_angle = -std::f32::consts::FRAC_PI_2 + 2.0 * std::f32::consts::PI * value;
                // The arc must never run backwards or past its own start when the value is small
                // enough that the gap would swallow it; a zero-or-negative sweep is nothing to draw.
                if end_angle > start_angle {
                    // A sweep too small to advance a pixel is skipped rather than emitted as a stream
                    // of zero-length lines. This guard used to be the only thing standing between a
                    // tiny `value` and 40 degenerate `<line>` elements, because the helper's vertex
                    // rounding collapsed every sample onto one pixel. `draw_arc_segments` now derives
                    // its sample count from the arc's own pixel length and drops any chord whose
                    // endpoints round together, so it is degenerate-safe on its own; the explicit
                    // check stays because "no arc at all" is a decision about *this* control's
                    // contract (a rounded-away sweep is nothing to draw), and stating it here keeps
                    // the intent readable without relying on a helper's internals.
                    let sweep_px = (radius * (end_angle - start_angle)).round() as u32;
                    if sweep_px > 0 {
                        let prog_color = if is_enabled {
                            self.progress_color
                        } else {
                            Color::DISABLED_FOREGROUND
                        };
                        crate::render::draw_arc_segments(
                            context,
                            center,
                            radius,
                            start_angle,
                            end_angle,
                            prog_color,
                            stroke_w.max(1),
                        );
                    }
                }
            }
        }

        // The centre reading, drawn last so it sits inside the ring rather than under it. It is
        // opt-in (`show_center_text` defaults to `false`) and determinate-only, because an
        // indeterminate ring has no reading to print. The text is dropped when it cannot fit in
        // the hole, for the same reason `Arc` drops its own: a reading that covers the progress it
        // reports is worse than no reading.
        self.ensure_center_text(context);
        if let Some(cache) = self.center_text_cache.as_ref() {
            if !cache.text.is_empty() {
                let hole = (radius - self.stroke_width / 2.0).max(0.0);
                if (cache.width as f32) <= hole * 2.0 && (cache.height as f32) <= hole * 2.0 {
                    let ink = self.base.style().text_color.unwrap_or(self.progress_color);
                    // The centring term is the *measured* glyph-box height, so the ring's
                    // centre is the text's centre rather than the top edge's position.
                    let line_height = cache.height as i32;
                    context.draw_text(
                        Point::new(
                            center.x - (cache.width as i32 / 2),
                            center.y - (line_height / 2),
                        ),
                        cache.text.as_str(),
                        &cache.font,
                        ink,
                        HorizontalAlignment::Left,
                    );
                }
            }
        }
    }
}

impl EventHandler for ProgressCircle {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::PaintBackend;

    #[test]
    fn progress_circle_default_creation() {
        let pc = ProgressCircle::new(Rect::new(0, 0, 48, 48));
        assert_eq!(pc.kind(), WidgetKind::ProgressCircle);
        assert!((pc.value() - 0.0).abs() < f32::EPSILON);
        assert!(!pc.is_indeterminate());
        assert!((pc.stroke_width() - 4.0).abs() < f32::EPSILON);
        assert_eq!(pc.track_color(), Color::rgba(220, 220, 220, 200));
        // The arc's default is the token `progress_bar` fills with, so the two progress
        // controls agree; the literal is only the no-theme fallback. Asserting the literal
        // here would re-assert the defect the default was changed to remove.
        let expected = crate::style::resolved_theme_style_shared("progress_bar")
            .and_then(|resolved| resolved.background_color)
            .unwrap_or(Color::PRIMARY);
        assert_eq!(pc.progress_color(), expected);
    }

    #[test]
    fn progress_circle_set_value() {
        let mut pc = ProgressCircle::new(Rect::new(0, 0, 48, 48));
        pc.set_value(0.5);
        assert!((pc.value() - 0.5).abs() < f32::EPSILON);

        pc.set_value(1.5); // Should clamp to 1.0
        assert!((pc.value() - 1.0).abs() < f32::EPSILON);

        pc.set_value(-0.5); // Should clamp to 0.0
        assert!((pc.value() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn progress_circle_indeterminate_toggle() {
        let mut pc = ProgressCircle::new(Rect::new(0, 0, 48, 48));
        assert!(!pc.is_indeterminate());

        pc.set_indeterminate(true);
        assert!(pc.is_indeterminate());

        pc.set_indeterminate(false);
        assert!(!pc.is_indeterminate());
    }

    #[test]
    fn progress_circle_colors_and_stroke() {
        let mut pc = ProgressCircle::new(Rect::new(0, 0, 48, 48));

        pc.set_track_color(Color::LIGHT_GRAY);
        assert_eq!(pc.track_color(), Color::LIGHT_GRAY);

        pc.set_progress_color(Color::SUCCESS);
        assert_eq!(pc.progress_color(), Color::SUCCESS);

        pc.set_stroke_width(6.0);
        assert!((pc.stroke_width() - 6.0).abs() < f32::EPSILON);

        pc.set_stroke_width(-1.0); // Should clamp to minimum
        assert!((pc.stroke_width() - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn progress_circle_svg_output_determinate() {
        let mut pc = ProgressCircle::new(Rect::new(0, 0, 48, 48));
        pc.set_value(0.75);
        settle(&mut pc);

        let svg = crate::widget::svg::render_to_svg(&mut pc);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn progress_circle_svg_output_indeterminate() {
        let mut pc = ProgressCircle::new(Rect::new(0, 0, 48, 48));
        pc.set_indeterminate(true);

        let svg = crate::widget::svg::render_to_svg(&mut pc);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn progress_circle_zero_geometry_no_crash() {
        let mut pc = ProgressCircle::new(Rect::new(0, 0, 0, 0));
        // Should not panic
        let svg = crate::widget::svg::render_to_svg(&mut pc);
        assert!(svg.starts_with("<svg"));
    }

    /// Advances the determinate tween until it settles, so a geometry assertion sees the target.
    fn settle(circle: &mut ProgressCircle) {
        let mut frames = 0;
        while circle.tick(16) {
            frames += 1;
            assert!(frames < 1_000, "a value tween must terminate rather than tick forever");
        }
    }

    /// Every arc segment in the stream, as `(x1, y1, x2, y2)`.
    ///
    /// The arc helper emits one `<line>` per segment at the widget's stroke width, so selecting on
    /// that pair counts the arc's own output without having to reason about the track circle.
    fn arc_segments(svg: &str) -> Vec<(i32, i32, i32, i32)> {
        svg.lines()
            .filter(|line| line.contains("<line") && line.contains("stroke-width=\"4\""))
            .map(|line| {
                let attr = |name: &str| -> i32 {
                    line.split(&std::format!("{name}=\""))
                        .nth(1)
                        .and_then(|rest| rest.split('"').next())
                        .and_then(|value| value.parse().ok())
                        .unwrap_or_else(|| panic!("no {name} on: {line}"))
                };
                (attr("x1"), attr("y1"), attr("x2"), attr("y2"))
            })
            .collect()
    }

    /// A value whose arc sweep rounds away must emit **no** segment with any length.
    ///
    /// Two guards used to leave this value uncovered. The helper drops a sweep below 0.001 rad, and
    /// the draw skipped `value == 0.0` — but value 0.001 gives a 0.0063 rad sweep, which passes the
    /// first test while 40 vertex pairs all round onto the same pixel. The result was 40
    /// zero-length `<line>` elements: a stream full of drawing commands that draw nothing. The track
    /// circle must survive the guard, because it is the ring's own chrome.
    #[test]
    fn progress_circle_ignores_a_value_too_small_to_sweep_a_pixel() {
        let mut pc = ProgressCircle::new(Rect::new(0, 0, 48, 48));
        pc.set_value(0.001);
        settle(&mut pc);

        let svg = crate::widget::svg::render_to_svg(&mut pc);
        let segments = arc_segments(&svg);
        assert!(
            segments.iter().all(|(x1, y1, x2, y2)| x1 == x2 && y1 == y2),
            "no arc segment of a swept-away value may have length: {segments:?}"
        );
        assert!(
            svg.contains("<circle") && svg.contains("fill=\"none\""),
            "the track ring is chrome and must still be drawn: {svg}"
        );
    }

    /// The complement: a value that does sweep whole pixels must still draw the arc.
    #[test]
    fn progress_circle_draws_an_arc_that_sweeps_whole_pixels() {
        let mut pc = ProgressCircle::new(Rect::new(0, 0, 48, 48));
        pc.set_value(0.25);
        settle(&mut pc);

        let svg = crate::widget::svg::render_to_svg(&mut pc);
        let segments = arc_segments(&svg);
        assert!(
            !segments.is_empty() && segments.iter().any(|(x1, y1, x2, y2)| x1 != x2 || y1 != y2),
            "a quarter turn is a visible arc, not 40 dots: {segments:?}"
        );
    }

    #[test]
    fn progress_circle_event_forwarding() {
        let mut pc = ProgressCircle::new(Rect::new(0, 0, 48, 48));
        // Should not panic
        pc.handle_event(&Event::MouseMove { pos: Point::new(10, 10) });
        pc.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1, modifiers: 0 });
    }

    /// The indeterminate ring must turn, and it must turn on the ticks it is given.
    ///
    /// This is the \u00a70.3 three-frame judgement: the three frames' geometry differ, and the
    /// angle advances monotonically. It replaces a sweep read from `SystemTime::now()` inside
    /// `draw`, which could only be tested by sleeping and which kept the ring turning in a
    /// frame nobody had requested.
    #[test]
    fn the_indeterminate_ring_turns_on_the_ticks_it_is_given() {
        // Holds the crate-wide theme guard: this test renders, and a concurrent
        // test that switches the appearance would otherwise change a later frame.
        let _theme_guard = crate::style::theme_test_guard();
        let mut pc = ProgressCircle::new(Rect::new(0, 0, 48, 48));
        pc.set_indeterminate(true);
        let first = crate::widget::svg::render_to_svg(&mut pc);
        let angle_before = pc.sweep_angle();

        assert!(pc.tick(100), "an indeterminate ring owes another frame");
        let second = crate::widget::svg::render_to_svg(&mut pc);
        assert!(pc.tick(100), "and the frame after that");
        let third = crate::widget::svg::render_to_svg(&mut pc);

        assert!(pc.sweep_angle() > angle_before, "the ring must advance, not sit still");
        assert_ne!(first, second, "frame one and frame two must not be the same picture");
        assert_ne!(second, third, "nor frame two and frame three");
    }

    /// A determinate value travels over frames and settles exactly on its target.
    #[test]
    fn a_determinate_value_tweens_before_reaching_its_target() {
        let mut pc = ProgressCircle::new(Rect::new(0, 0, 48, 48));
        pc.set_value(1.0);
        assert_eq!(pc.value(), 1.0, "the published value is the value the caller wrote");
        assert!(pc.is_animating(), "a new target puts the driver in flight");
        assert!(pc.value_animation.value() < 1.0, "the painted value starts behind the target");

        assert!(pc.tick(16), "the tween owes another frame");
        let mid = pc.value_animation.value();
        assert!(mid > 0.0 && mid < 1.0, "the arc is painted mid-travel: {mid}");

        settle(&mut pc);
        assert!((pc.value_animation.value() - 1.0).abs() < f32::EPSILON);
        assert!(!pc.is_animating(), "a settled ring owes no frames");
    }

    /// Reduced motion reaches the target without a tween.
    #[test]
    fn reduced_motion_jumps_straight_to_the_target() {
        struct Reduced;
        impl crate::style::environment::EnvironmentProvider for Reduced {
            fn motion_preference(&self) -> crate::style::MotionPreference {
                crate::style::MotionPreference::ReduceMotion
            }
        }

        let previous =
            crate::style::environment::install_environment(crate::compat::Box::new(Reduced));
        let mut pc = ProgressCircle::new(Rect::new(0, 0, 48, 48));
        pc.set_value(0.75);

        assert_eq!(
            pc.value_animation.value(),
            0.75,
            "reduced motion must reach the end state, not skip it"
        );
        assert!(!pc.is_animating(), "and there is nothing left to animate");

        match previous {
            Some(previous) => {
                crate::style::environment::install_environment(previous);
            }
            None => {
                crate::style::environment::uninstall_environment();
            }
        }
    }

    /// A determinate ring is a still picture: it owes no frames, and telling it so is a no-op.
    ///
    /// \u00a73.2 in one assertion: an animation that is not running must not keep the frame bus
    /// alive, or every idle progress ring repaints the whole window forever. The tween means the
    /// ring does owe frames *while the value is travelling*; this pins the settled state.
    #[test]
    fn a_settled_determinate_ring_owes_no_frames() {
        // Holds the crate-wide theme guard: this test renders, and a concurrent
        // test that switches the appearance would otherwise change a later frame.
        let _theme_guard = crate::style::theme_test_guard();
        let mut pc = ProgressCircle::new(Rect::new(0, 0, 48, 48));
        pc.set_value(0.5);
        settle(&mut pc);
        assert!(!pc.tick(100), "a settled determinate ring must not request another frame");
        let before = crate::widget::svg::render_to_svg(&mut pc);
        assert!(!pc.tick(100), "and still not after being asked twice");
        assert_eq!(before, crate::widget::svg::render_to_svg(&mut pc), "its picture is unchanged");
    }

    /// At value zero the arc and the track must still be two different strokes.
    ///
    /// Regression (BLUE21 A.3.6): without a track gap, a value just above zero draws the arc
    /// flush against the track's own start, so the two read as one unbroken ring and the reader
    /// cannot tell an empty indicator from a full one. The gap is what makes the arc's start
    /// visible.
    ///
    /// # Why this measures the first *chord* and not the first vertex
    ///
    /// The arc helper rounds every sample onto a pixel, so on a 48 px ring a 4 px gap moves the
    /// start by less than a pixel and the rounded corner is the same either way — an assertion on
    /// the vertex passes with and without the gap. The arc's *length* is what actually changes:
    /// the same value sweeps fewer pixels once the gap is taken out of it, so the gap is asserted
    /// by measuring the swept length against the value's own angle.
    #[test]
    fn the_smallest_visible_arc_starts_after_a_track_gap() {
        let mut pc = ProgressCircle::new(Rect::new(0, 0, 48, 48));
        pc.set_value(0.25);
        settle(&mut pc);
        let svg = crate::widget::svg::render_to_svg(&mut pc);
        let segments = arc_segments(&svg);
        assert!(!segments.is_empty(), "a quarter ring is a visible arc: {svg}");

        // The radius the control draws at is `min(w, h)/2 - stroke/2 - 1` — **not** half the
        // box: the stroke is centred on the path, so the outer half of it would otherwise fall
        // outside the rectangle. The gap comes out of the sweep's own angle, so the arc's pixel
        // length is `radius * (quarter - gap/radius)` = `radius * quarter - gap`.
        let radius = 48.0 / 2.0 - 4.0 / 2.0 - 1.0;
        let gapped_px = (radius * core::f32::consts::FRAC_PI_2 - TRACK_GAP).ceil() as usize;
        let ungapped_px = (radius * core::f32::consts::FRAC_PI_2).ceil() as usize;
        assert!(
            ungapped_px > gapped_px,
            "the fixture must distinguish the two cases, or the assertion below is vacuous"
        );
        // The helper emits one chord per pixel of the gapped length (the last sample's rounding
        // can collapse it by one), and this is strictly fewer than the ungapped sweep would be.
        assert!(
            segments.len() <= gapped_px && segments.len() < ungapped_px,
            "the arc must be the gapped sweep, not the raw one: {} segments, expected <= \
             {gapped_px} (gapped) and < {ungapped_px} (ungapped)",
            segments.len()
        );
    }

    /// An animation frame submits damage bounded by the painted box and the margin.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn a_spin_frame_submits_the_painted_box_with_the_damage_margin() {
        use crate::widget::runtime;

        let mut circle = ProgressCircle::new(Rect::new(4, 8, 48, 48));
        circle.set_indeterminate(true);
        let id = runtime::register(crate::compat::Box::new(circle)).expect("registry");
        assert!(runtime::set_repaint_mode(id, runtime::RepaintMode::Dirty));

        let moved = runtime::with_widget_mut(id, |widget| Widget::tick(widget, 16))
            .expect("mounted control");
        assert!(moved, "the spinning ring owes a frame");

        let expected = runtime::painted_box_with_damage_margin(id).expect("painted box");
        assert_eq!(
            runtime::dirty_rects(id),
            vec![expected],
            "one animated frame must damage the painted box plus the margin"
        );
        runtime::unregister(id);
    }

    /// `Full` keeps the whole-control path and records no damage rectangle.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn a_spin_frame_in_full_mode_keeps_the_whole_control_repaint() {
        use crate::widget::runtime;

        let mut circle = ProgressCircle::new(Rect::new(4, 8, 48, 48));
        circle.set_indeterminate(true);
        let id = runtime::register(crate::compat::Box::new(circle)).expect("registry");
        assert_eq!(runtime::repaint_mode(id), runtime::RepaintMode::Full);

        let moved = runtime::with_widget_mut(id, |widget| Widget::tick(widget, 16))
            .expect("mounted control");
        assert!(moved);
        assert!(
            runtime::dirty_rects(id).is_empty(),
            "Full mode records no damage, so the control repaints whole"
        );
        runtime::unregister(id);
    }

    /// The centre reading is built and measured once per `(value, template, face, scale)` key.
    #[test]
    fn the_centre_reading_is_cached_across_frames() {
        let mut pc = ProgressCircle::new(Rect::new(0, 0, 80, 80));
        pc.set_show_center_text(true);
        pc.set_value(0.5);
        let mut backend = crate::render::SoftwarePaintBackend::new(Size::new(80, 80), 1.0);
        backend.begin_frame(Color::WHITE);
        let context = RenderContext::new(&mut backend);
        // The driver starts at rest; aim it at the value so the reading is the one written.
        pc.value_animation.jump_to(0.5);
        pc.ensure_center_text(&context);
        let first = pc.center_text_cache.as_ref().expect("the reading is cached");
        assert_eq!(first.text, "50%");
        let ptr = first.text.as_ptr();
        pc.ensure_center_text(&context);
        assert_eq!(
            pc.center_text_cache.as_ref().unwrap().text.as_ptr(),
            ptr,
            "an unchanged reading reuses the buffer"
        );

        pc.set_center_text_format("{value}");
        pc.ensure_center_text(&context);
        assert_eq!(pc.center_text_cache.as_ref().unwrap().text, "0.50");
    }
}
