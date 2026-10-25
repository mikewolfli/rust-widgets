// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `MacStepper` — a desktop increment/decrement control.
//!
//! The desktop stepper is a narrow two-zone control: the top half increments, the bottom half
//! decrements, and the two zones share one rounded outline. It is narrower and shorter than a
//! push button, which is why it derives its size from the macOS profile's button height rather
//! than from the Material row height the crate's `Stepper` uses.

use crate::core::{Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::{expect_bool, expect_i64};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{profile_metrics, ControlMetricsProfile};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

use super::{centred_size, draw_focus_ring, mac_stepper_height, palette, MAC_STEPPER_WIDTH};

/// Half-width of the arrow drawn in each zone: the chevron spans `2 * ARROW_HALF` points.
const ARROW_HALF: i32 = 3;

/// Which half of the control a pointer is over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StepperZone {
    /// The upper zone, which increments.
    Up,
    /// The lower zone, which decrements.
    Down,
}

/// A desktop stepper: an increment zone above a decrement zone.
///
/// The value is a signed integer clamped to `[minimum, maximum]`; when [`Self::wraps`] is set,
/// passing either end continues from the other. Every real change emits
/// [`Self::value_changed`].
pub struct MacStepper {
    base: BaseWidget,
    value: i64,
    minimum: i64,
    maximum: i64,
    step: i64,
    wraps: bool,
    hovered: Option<StepperZone>,
    pressed: Option<StepperZone>,
    /// Whether a press is currently auto-repeating.
    holding: bool,
    /// Milliseconds the current hold has lasted.
    held_ms: u64,
    /// How many auto-repeats have already fired for this hold.
    repeats_done: u64,
    /// Delay before the first auto-repeat, in milliseconds; `0` disables repeating.
    repeat_delay_ms: u64,
    /// Interval between auto-repeats after the delay.
    repeat_interval_ms: u64,
    /// Emitted when the value changes, with the new value.
    pub value_changed: Signal1<i64>,
}

impl MacStepper {
    /// Creates a stepper at `0` within `[0, 100]` in steps of one, not wrapping.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Stepper, geometry, "MacStepper"),
            value: 0,
            minimum: 0,
            maximum: 100,
            step: 1,
            wraps: false,
            hovered: None,
            pressed: None,
            holding: false,
            held_ms: 0,
            repeats_done: 0,
            repeat_delay_ms: 400,
            repeat_interval_ms: 80,
            value_changed: Signal1::new(),
        }
    }

    /// The current value.
    pub fn value(&self) -> i64 {
        self.value
    }

    /// Sets the value, clamped to the current range.
    ///
    /// Returns whether the value changed; emits [`Self::value_changed`] only on a real change,
    /// so writing back the value just read is quiet.
    pub fn set_value(&mut self, value: i64) -> bool {
        let clamped = value.clamp(self.minimum, self.maximum);
        if clamped == self.value {
            return false;
        }
        self.value = clamped;
        self.base.request_redraw();
        self.value_changed.emit(clamped);
        true
    }

    /// The lower bound, inclusive.
    pub fn minimum(&self) -> i64 {
        self.minimum
    }

    /// Sets the lower bound; a bound above the current maximum moves the maximum with it.
    pub fn set_minimum(&mut self, minimum: i64) {
        if minimum == self.minimum {
            return;
        }
        self.minimum = minimum;
        self.maximum = self.maximum.max(self.minimum);
        self.clamp_value();
    }

    /// The upper bound, inclusive.
    pub fn maximum(&self) -> i64 {
        self.maximum
    }

    /// Sets the upper bound; a bound below the current minimum moves the minimum with it.
    pub fn set_maximum(&mut self, maximum: i64) {
        if maximum == self.maximum {
            return;
        }
        self.maximum = maximum;
        self.minimum = self.minimum.min(self.maximum);
        self.clamp_value();
    }

    /// The increment applied by one click or key press.
    pub fn step(&self) -> i64 {
        self.step
    }

    /// Sets the increment. A value below one is raised to one, because a zero step would make
    /// the control respond to a press with nothing at all.
    pub fn set_step(&mut self, step: i64) {
        let step = step.max(1);
        if step == self.step {
            return;
        }
        self.step = step;
    }

    /// Whether passing an end of the range continues from the other end.
    pub fn wraps(&self) -> bool {
        self.wraps
    }

    /// The delay before a held zone starts repeating.
    pub fn repeat_delay_ms(&self) -> u64 {
        self.repeat_delay_ms
    }

    /// Sets the auto-repeat delay. Zero disables repeating (a press steps once).
    pub fn set_repeat_delay_ms(&mut self, delay_ms: u64) {
        if self.repeat_delay_ms == delay_ms {
            return;
        }
        self.repeat_delay_ms = delay_ms;
    }

    /// The interval between auto-repeats.
    pub fn repeat_interval_ms(&self) -> u64 {
        self.repeat_interval_ms
    }

    /// Sets the auto-repeat interval. Zero disables repeating.
    pub fn set_repeat_interval_ms(&mut self, interval_ms: u64) {
        if self.repeat_interval_ms == interval_ms {
            return;
        }
        self.repeat_interval_ms = interval_ms;
    }

    /// Sets whether the value wraps at the ends of its range.
    pub fn set_wraps(&mut self, wraps: bool) {
        if wraps == self.wraps {
            return;
        }
        self.wraps = wraps;
        self.base.request_redraw();
    }

    /// Adds one step, honouring the range and the wrap flag. Returns whether the value changed.
    pub fn increment(&mut self) -> bool {
        let next = self.value.saturating_add(self.step);
        if self.wraps && next > self.maximum {
            return self.jump_to(self.minimum);
        }
        self.set_value(next)
    }

    /// Subtracts one step, honouring the range and the wrap flag. Returns whether the value
    /// changed.
    pub fn decrement(&mut self) -> bool {
        let next = self.value.saturating_sub(self.step);
        if self.wraps && next < self.minimum {
            return self.jump_to(self.maximum);
        }
        self.set_value(next)
    }

    /// Wraps to `value`, emitting through the same path a normal change takes.
    fn jump_to(&mut self, value: i64) -> bool {
        let clamped = value.clamp(self.minimum, self.maximum);
        if clamped == self.value {
            return false;
        }
        self.value = clamped;
        self.base.request_redraw();
        self.value_changed.emit(clamped);
        true
    }

    /// Re-clamps the value after a range change, without emitting: a range change is the host
    /// describing the control, not the user picking a value.
    fn clamp_value(&mut self) {
        let clamped = self.value.clamp(self.minimum, self.maximum);
        if clamped != self.value {
            self.value = clamped;
            self.base.request_redraw();
        }
    }

    /// The rectangle the control paints: its natural size, centred in the allocation.
    fn stepper_rect(&self) -> Rect {
        centred_size(self.geometry(), MAC_STEPPER_WIDTH, mac_stepper_height())
    }

    /// The corner radius of the whole control, taken from the desktop profile.
    fn corner_radius(&self) -> u32 {
        profile_metrics(ControlMetricsProfile::Macos).button_radius
    }

    /// The rectangle of one zone: the top half increments, the bottom half decrements.
    fn zone_rect(&self, zone: StepperZone) -> Rect {
        let rect = self.stepper_rect();
        let top_height = rect.height.div_ceil(2);
        match zone {
            StepperZone::Up => Rect::new(rect.x, rect.y, rect.width, top_height),
            StepperZone::Down => Rect::new(
                rect.x,
                rect.y + top_height as i32,
                rect.width,
                rect.height.saturating_sub(top_height),
            ),
        }
    }

    /// The zone under `pos`, if any.
    fn zone_at(&self, pos: Point) -> Option<StepperZone> {
        if self.zone_rect(StepperZone::Up).contains_point(pos) {
            Some(StepperZone::Up)
        } else if self.zone_rect(StepperZone::Down).contains_point(pos) {
            Some(StepperZone::Down)
        } else {
            None
        }
    }

    /// Paints one zone's arrow, as a two-segment chevron centred in `zone`.
    fn draw_arrow(
        &self,
        context: &mut RenderContext,
        zone: Rect,
        up: bool,
        ink: crate::core::Color,
    ) {
        let center = zone.center();
        // The chevron points away from the other zone: the upper arrow points up, the lower one
        // points down, so the pair reads as one control rather than two identical glyphs.
        let lift = if up { -ARROW_HALF } else { ARROW_HALF };
        let apex = Point::new(center.x, center.y + lift);
        let left = Point::new(center.x - ARROW_HALF, center.y - lift);
        let right = Point::new(center.x + ARROW_HALF, center.y - lift);
        context.draw_line(left, apex, ink);
        context.draw_line(apex, right, ink);
    }
}

impl Widget for MacStepper {
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
        Size::new(MAC_STEPPER_WIDTH, mac_stepper_height())
    }

    fn kind(&self) -> WidgetKind {
        WidgetKind::Stepper
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();

    /// Advances the held-zone auto-repeat clock.
    ///
    /// A press steps once immediately (in the event handler); this clock fires only after
    /// [`Self::repeat_delay_ms`] has elapsed, then once per [`Self::repeat_interval_ms`]. A zero
    /// delay or interval disables repeating, which preserves the old "one step per press"
    /// behaviour for a host that wants it.
    fn tick(&mut self, delta_ms: u32) -> bool {
        if !self.holding || self.repeat_delay_ms == 0 || self.repeat_interval_ms == 0 {
            return false;
        }
        self.held_ms = self.held_ms.saturating_add(u64::from(delta_ms));
        let after_delay = self.held_ms.saturating_sub(self.repeat_delay_ms);
        let target = after_delay / self.repeat_interval_ms;
        let mut fired = 0u64;
        while self.repeats_done < target && fired < 100 {
            self.repeats_done += 1;
            fired += 1;
            match self.pressed {
                Some(StepperZone::Up) => {
                    self.increment();
                }
                Some(StepperZone::Down) => {
                    self.decrement();
                }
                None => break,
            }
        }
        if fired > 0 {
            self.base.request_redraw();
        }
        fired > 0
    }

    /// A held zone owes repeat frames until it is released.
    fn is_animating(&self) -> bool {
        self.holding && self.repeat_delay_ms > 0 && self.repeat_interval_ms > 0
    }

    /// Resolves a published event name to the control's own signal.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "value_changed" => {
                Some(EventSignalRef::mapped("value_changed", &self.value_changed, |value| {
                    CapabilityValue::Int(*value)
                }))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for MacStepper {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "value" => Ok(CapabilityValue::Int(self.value())),
            "minimum" => Ok(CapabilityValue::Int(self.minimum())),
            "maximum" => Ok(CapabilityValue::Int(self.maximum())),
            "step" => Ok(CapabilityValue::Int(self.step())),
            "wraps" => Ok(CapabilityValue::Bool(self.wraps())),
            "repeat_delay_ms" => Ok(CapabilityValue::UInt(self.repeat_delay_ms())),
            "repeat_interval_ms" => Ok(CapabilityValue::UInt(self.repeat_interval_ms())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "value" => {
                self.set_value(expect_i64(value)?);
                Ok(())
            }
            "minimum" => {
                self.set_minimum(expect_i64(value)?);
                Ok(())
            }
            "maximum" => {
                self.set_maximum(expect_i64(value)?);
                Ok(())
            }
            "step" => {
                self.set_step(expect_i64(value)?);
                Ok(())
            }
            "wraps" => {
                self.set_wraps(expect_bool(value)?);
                Ok(())
            }
            "repeat_delay_ms" => {
                let delay = match value {
                    CapabilityValue::UInt(ms) => ms,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_repeat_delay_ms(delay);
                Ok(())
            }
            "repeat_interval_ms" => {
                let interval = match value {
                    CapabilityValue::UInt(ms) => ms,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_repeat_interval_ms(interval);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "value",
            "minimum",
            "maximum",
            "step",
            "wraps",
            "repeat_delay_ms",
            "repeat_interval_ms",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for MacStepper {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.stepper_rect();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        let colors = palette();
        let radius = self.corner_radius();
        let up = self.zone_rect(StepperZone::Up);
        let down = self.zone_rect(StepperZone::Down);

        // The two halves are drawn as one outline: the background first, then the pressed or
        // hovered zone, then the divider between them and the outline around both.
        context.fill_rounded_rect(rect, radius, colors.raised);
        for (zone, key) in [(up, StepperZone::Up), (down, StepperZone::Down)] {
            let active =
                self.pressed == Some(key) || (self.pressed.is_none() && self.hovered == Some(key));
            if active {
                let fill =
                    if self.pressed == Some(key) { colors.separator } else { colors.surface };
                context.fill_rect(zone, fill);
            }
        }
        // The divider and the outline are painted after the fills so a zone highlight cannot
        // erase them.
        let divider_y = up.bottom();
        context.draw_line(
            Point::new(rect.x + 2, divider_y),
            Point::new(rect.right() - 2, divider_y),
            colors.separator,
        );
        context.draw_rounded_rect_stroke(rect, radius, colors.separator, 1);

        let ink = if self.base.is_enabled() { colors.ink } else { colors.muted };
        self.draw_arrow(context, up, true, ink);
        self.draw_arrow(context, down, false, ink);

        draw_focus_ring(context, rect, radius, &self.base, colors.accent);
    }
}

impl EventHandler for MacStepper {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MouseMove { pos } => {
                let hovered = self.zone_at(*pos);
                if hovered != self.hovered {
                    self.hovered = hovered;
                    self.base.request_redraw();
                }
            }
            Event::MouseLeave { .. } => {
                if self.hovered.take().is_some() {
                    self.base.request_redraw();
                }
                self.holding = false;
            }
            Event::MousePress { pos, button: 1, .. } => {
                if let Some(zone) = self.zone_at(*pos) {
                    self.pressed = Some(zone);
                    // The first step is immediate; the auto-repeat clock starts from here and
                    // only fires after `repeat_delay_ms`.
                    self.holding = true;
                    self.held_ms = 0;
                    self.repeats_done = 0;
                    match zone {
                        StepperZone::Up => {
                            self.increment();
                        }
                        StepperZone::Down => {
                            self.decrement();
                        }
                    }
                    self.base.request_redraw();
                }
            }
            Event::MouseRelease { .. } => {
                self.holding = false;
                if self.pressed.take().is_some() {
                    self.base.request_redraw();
                }
            }
            Event::KeyPress { key, .. } => match crate::shortcut::Key::from_key_code(*key) {
                Some(crate::shortcut::Key::Up) => {
                    self.increment();
                }
                Some(crate::shortcut::Key::Down) => {
                    self.decrement();
                }
                Some(crate::shortcut::Key::PageUp) => {
                    self.set_value(self.value.saturating_add(self.step.saturating_mul(10)));
                }
                Some(crate::shortcut::Key::PageDown) => {
                    self.set_value(self.value.saturating_sub(self.step.saturating_mul(10)));
                }
                Some(crate::shortcut::Key::Home) => {
                    self.set_value(self.minimum);
                }
                Some(crate::shortcut::Key::End) => {
                    self.set_value(self.maximum);
                }
                _ => {}
            },
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn sample() -> MacStepper {
        MacStepper::new(Rect::new(0, 0, 19, 28))
    }

    #[test]
    fn a_fresh_stepper_holds_zero_in_its_default_range() {
        let stepper = sample();
        assert_eq!(stepper.value(), 0);
        assert_eq!(stepper.minimum(), 0);
        assert_eq!(stepper.maximum(), 100);
        assert_eq!(stepper.step(), 1);
        assert!(!stepper.wraps());
        // The desktop size, not the Material row height.
        assert_eq!(stepper.size_hint(), Size::new(19, 28));
    }

    #[test]
    fn stepping_up_and_down_clamps_at_the_range_ends() {
        let mut stepper = sample();
        stepper.set_value(99);
        assert!(stepper.increment());
        assert_eq!(stepper.value(), 100);
        // Already at the top: nothing changes, so nothing is emitted.
        assert!(!stepper.increment());

        stepper.set_value(1);
        assert!(stepper.decrement());
        assert_eq!(stepper.value(), 0);
        assert!(!stepper.decrement());
    }

    #[test]
    fn a_wrapping_stepper_continues_at_the_other_end() {
        let mut stepper = sample();
        stepper.set_wraps(true);
        stepper.set_value(100);
        assert!(stepper.increment());
        assert_eq!(stepper.value(), 0, "the top wraps to the bottom");

        assert!(stepper.decrement());
        assert_eq!(stepper.value(), 100, "the bottom wraps to the top");

        // Without the flag the ends are hard stops, not a wrap.
        stepper.set_wraps(false);
        stepper.set_value(100);
        assert!(!stepper.increment());
        assert_eq!(stepper.value(), 100);
    }

    #[test]
    fn a_range_change_re_clamps_the_value_without_emitting() {
        let mut stepper = sample();
        let seen = Arc::new(Mutex::new(0usize));
        stepper.value_changed.connect({
            let seen = Arc::clone(&seen);
            move |_value: Arc<i64>| {
                *seen.lock().unwrap() += 1;
            }
        });

        stepper.set_value(50);
        assert_eq!(*seen.lock().unwrap(), 1);
        stepper.set_maximum(10);
        assert_eq!(stepper.value(), 10);
        assert_eq!(
            *seen.lock().unwrap(),
            1,
            "describing the range is not the user picking a value"
        );
    }

    #[test]
    fn a_step_below_one_is_raised_to_one() {
        let mut stepper = sample();
        stepper.set_step(0);
        assert_eq!(stepper.step(), 1);
        stepper.set_step(-4);
        assert_eq!(stepper.step(), 1);
    }

    #[test]
    fn clicking_the_upper_zone_increments_and_the_lower_zone_decrements() {
        let mut stepper = sample();
        stepper.handle_event(&Event::MousePress { pos: Point::new(9, 5), button: 1, modifiers: 0 });
        assert_eq!(stepper.value(), 1);
        stepper.handle_event(&Event::MouseRelease { pos: Point::new(9, 5), button: 1 });
        stepper.handle_event(&Event::MousePress {
            pos: Point::new(9, 22),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(stepper.value(), 0);
    }

    #[test]
    fn the_property_route_reads_and_writes_every_published_name() {
        let mut stepper = sample();
        assert_eq!(stepper.get("value"), Ok(CapabilityValue::Int(0)));
        stepper.set("value", CapabilityValue::Int(7)).unwrap();
        assert_eq!(stepper.get("value"), Ok(CapabilityValue::Int(7)));
        stepper.set("step", CapabilityValue::Int(5)).unwrap();
        assert_eq!(stepper.get("step"), Ok(CapabilityValue::Int(5)));
        stepper.set("wraps", CapabilityValue::Bool(true)).unwrap();
        assert_eq!(stepper.get("wraps"), Ok(CapabilityValue::Bool(true)));
        assert_eq!(
            stepper.set("value", CapabilityValue::String("nope".into())),
            Err(CapabilityAccessError::TypeMismatch)
        );
    }

    #[test]
    fn both_zones_paint_their_arrow_inside_the_control() {
        let mut stepper = sample();
        let mut recorder = crate::widget::macos::test_support::Recorder::new(19, 28);
        recorder.begin_frame(crate::core::Color::WHITE);
        {
            let mut context = RenderContext::new(&mut recorder);
            stepper.draw(&mut context);
        }
        recorder.end_frame();

        let lines: Vec<_> = recorder
            .commands
            .iter()
            .filter(|command| matches!(command, crate::render::RenderCommand::DrawLine { .. }))
            .collect();
        // Two segments per arrow (four) plus the divider.
        assert!(lines.len() >= 5, "both arrows and the divider are painted");
        for command in &recorder.commands {
            if let crate::render::RenderCommand::DrawLine { from, to, .. } = command {
                assert!(from.x >= 0 && from.x <= 19 && from.y >= 0 && from.y <= 28);
                assert!(to.x >= 0 && to.x <= 19 && to.y >= 0 && to.y <= 28);
            }
        }
    }

    #[test]
    fn holding_a_zone_repeats_after_the_delay_and_stops_on_release() {
        let mut stepper = sample();
        stepper.set_value(0);
        stepper.set_repeat_delay_ms(100);
        stepper.set_repeat_interval_ms(50);
        assert_eq!(stepper.repeat_delay_ms(), 100);
        let up = stepper.zone_rect(StepperZone::Up).center();
        stepper.handle_event(&Event::MousePress { pos: up, button: 1, modifiers: 0 });
        assert_eq!(stepper.value(), 1, "the first step is immediate");
        assert!(stepper.is_animating());

        // Before the delay there is no repeat.
        assert!(!stepper.tick(50));
        assert_eq!(stepper.value(), 1);
        // The delay expiring is not itself a repeat; the first repeat waits one interval.
        assert!(!stepper.tick(50));
        assert_eq!(stepper.value(), 1);
        // After the delay, one repeat per interval.
        assert!(stepper.tick(50));
        assert_eq!(stepper.value(), 2);
        assert!(stepper.tick(100));
        assert_eq!(stepper.value(), 4);

        stepper.handle_event(&Event::MouseRelease { pos: up, button: 1 });
        assert!(!stepper.is_animating());
        assert!(!stepper.tick(500), "a released stepper does not keep stepping");
        assert_eq!(stepper.value(), 4);
    }

    #[test]
    fn page_home_and_end_keys_move_by_larger_steps() {
        let mut stepper = sample();
        stepper.set_value(50);
        // Page Up (33), Page Down (34), Home (36), End (35).
        stepper.handle_event(&Event::KeyPress { key: 33, modifiers: 0 });
        assert_eq!(stepper.value(), 60);
        stepper.handle_event(&Event::KeyPress { key: 34, modifiers: 0 });
        assert_eq!(stepper.value(), 50);
        stepper.handle_event(&Event::KeyPress { key: 36, modifiers: 0 });
        assert_eq!(stepper.value(), 0);
        stepper.handle_event(&Event::KeyPress { key: 35, modifiers: 0 });
        assert_eq!(stepper.value(), 100);
    }
}
