// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Toggle button widget.
use crate::core::{HorizontalAlignment, Rect, Size};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::style::EdgeOffsets;
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, ControlMetrics};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
/// Toggle button state enumeration.
///
/// Derived from the checked and enabled flags, never stored: see
/// [`ToggleButton::state`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToggleButtonState {
    /// Enabled and not checked.
    Normal,
    /// Enabled and checked. Disabled always wins over checked, so an unchecked
    /// *and* disabled button also reports [`ToggleButtonState::Disabled`].
    Checked,
    /// Not enabled; reported regardless of the checked flag.
    Disabled,
}
/// Toggle button: a two-state push button that latches on click.
///
/// Also carries press tracking (`is_pressed`) so it can render a pressed
/// appearance between mouse-down and mouse-up.
pub struct ToggleButton {
    base: BaseWidget,
    text: String,
    checked: bool,
    auto_exclusive: bool,
    group_id: Option<String>,
    pressed: bool,
    /// Emitted with the new checked flag whenever it changes. Semantically a
    /// synonym for `checked_changed`, kept for callers using the checked-state
    /// terminology.
    pub toggled: Signal1<bool>,
    /// Emitted with the new checked flag whenever it changes.
    pub checked_changed: Signal1<bool>,
    /// Emitted on the rising edge of the pressed flag (mouse down).
    pub pressed_signal: GenericSignal,
    /// Emitted on the falling edge of the pressed flag (mouse up).
    pub released_signal: GenericSignal,
    /// Emitted with the recomputed [`ToggleButtonState`] whenever the checked
    /// flag changes. Not emitted when only the enabled flag changes, so a
    /// disabled button can still report a stale `Normal`. Reading
    /// [`ToggleButton::state`] after `set_enabled` gives the current value.
    pub state_changed: Signal1<ToggleButtonState>,
}
impl ToggleButton {
    /// Creates an unchecked, enabled toggle button with the given caption.
    ///
    /// `geometry` is in parent-relative logical pixels.
    pub fn new(text: String, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ToggleButton, geometry, "ToggleButton"),
            text,
            checked: false,
            auto_exclusive: false,
            group_id: None,
            pressed: false,
            toggled: Signal1::new(),
            checked_changed: Signal1::new(),
            pressed_signal: GenericSignal::new(),
            released_signal: GenericSignal::new(),
            state_changed: Signal1::new(),
        }
    }
    /// Returns the button caption, drawn centered. Empty by default only if
    /// constructed that way (`new` takes the text up front).
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Replaces the caption. No-op (and no redraw) when the text is unchanged.
    pub fn set_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.text != text {
            self.text = text;
            self.base.request_redraw();
        }
    }
    /// Returns the latched checked flag.
    pub fn is_checked(&self) -> bool {
        self.checked
    }
    /// Sets the checked flag.
    ///
    /// A no-op when the value is unchanged, which means **no signals fire on a
    /// redundant set**. On an actual change this emits `checked_changed` and
    /// `toggled` (both with the new flag) followed by `state_changed`, and
    /// requests a redraw.
    pub fn set_checked(&mut self, checked: bool) {
        if self.checked == checked {
            return;
        }
        self.checked = checked;
        self.base.request_redraw();
        self.checked_changed.emit(checked);
        self.toggled.emit(checked);
        self.state_changed.emit(self.state());
    }
    /// Flips the checked flag through [`ToggleButton::set_checked`], so the
    /// usual signals fire.
    pub fn toggle(&mut self) {
        self.set_checked(!self.checked);
    }
    /// Returns the auto-exclusive intent flag. Defaults to `false`.
    pub fn is_auto_exclusive(&self) -> bool {
        self.auto_exclusive
    }
    /// Sets the auto-exclusive intent flag.
    ///
    /// This records intent only: the button does **not** enforce exclusivity
    /// itself. A group manager is expected to read this flag plus
    /// [`ToggleButton::group_id`] and uncheck the group's other members when
    /// one is checked. Setting it requests a redraw even though the flag is not
    /// drawn.
    pub fn set_auto_exclusive(&mut self, exclusive: bool) {
        self.auto_exclusive = exclusive;
        self.base.request_redraw();
    }
    /// Returns the group this button belongs to, or `None` when ungrouped.
    ///
    /// The id is an opaque caller-chosen string; the widget only stores and
    /// reports it. See [`ToggleButton::set_auto_exclusive`].
    pub fn group_id(&self) -> Option<&str> {
        self.group_id.as_deref()
    }
    /// Sets (or clears, with `None`) the group id. See
    /// [`ToggleButton::group_id`].
    pub fn set_group_id(&mut self, group_id: Option<String>) {
        self.group_id = group_id;
        self.base.request_redraw();
    }
    /// Returns whether the button is currently held down.
    pub fn is_pressed(&self) -> bool {
        self.pressed
    }
    /// Sets the pressed flag.
    ///
    /// No-op when unchanged, so the signals fire only on an actual edge: a
    /// rising edge emits `pressed_signal`, a falling edge emits
    /// `released_signal`. Unlike the checked flag, this does not request a
    /// redraw.
    pub fn set_pressed(&mut self, pressed: bool) {
        if self.pressed == pressed {
            return;
        }
        self.pressed = pressed;
        if pressed {
            self.pressed_signal.emit();
        } else {
            self.released_signal.emit();
        }
    }
    /// Returns the interaction state derived from the enabled and checked
    /// flags. Disabled takes precedence over checked.
    pub fn state(&self) -> ToggleButtonState {
        if !self.base.enabled {
            ToggleButtonState::Disabled
        } else if self.checked {
            ToggleButtonState::Checked
        } else {
            ToggleButtonState::Normal
        }
    }
}
impl Widget for ToggleButton {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        // The same `max(floor, content + padding)` formula `Button` uses, through the shared
        // primitive rather than the hand-written `max(75)` this used to spell out. The old
        // literal pair (`75`/`28`) disagreed with `Button`'s `BUTTON_MIN` (64x40) about how
        // big a push button is, so two buttons that look identical reported different sizes
        // and — once `draw` started deriving its band from this hint — drew at two heights.
        let label_width = self.text().len() as u32 * 8;
        ControlMetrics::implicit_size(
            Size::new(label_width, dimensions::FONT_SIZE_BASE + 4),
            EdgeOffsets::symmetric(dimensions::BUTTON_PADDING_V, dimensions::BUTTON_PADDING_H),
            dimensions::BUTTON_MIN,
        )
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `ToggleButton`'s property contract.
///
/// `state` is the derived interaction state ("normal"/"checked"/"disabled") and
/// is read-only, which is why it has a read arm but no write arm here — exactly
/// as the previous centralised dispatch behaved.
impl WidgetProperties for ToggleButton {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "checked" => Ok(CapabilityValue::Bool(self.is_checked())),
            "state" => {
                let state = match self.state() {
                    ToggleButtonState::Normal => "normal",
                    ToggleButtonState::Checked => "checked",
                    ToggleButtonState::Disabled => "disabled",
                };
                Ok(CapabilityValue::String(state.to_string()))
            }
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(expect_string(value)?);
                Ok(())
            }
            "checked" => {
                self.set_checked(expect_bool(value)?);
                Ok(())
            }
            "state" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["text", "checked", "state", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `toggle_button` publishes.
    ///
    /// `toggle` flips the latch. `set_checked` and `set_text` assign state and need
    /// an argument, so they are answered through the property path and refused here
    /// as [`CapabilityAccessError::OutOfRange`] — "the name is right, the invocation
    /// needs a payload" — rather than `UnknownCommand`.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "toggle" => {
                self.toggle();
                Ok(())
            }
            "set_checked" | "set_text" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for ToggleButton {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        let state = self.state();
        let style = self.style();
        use crate::core::Color;

        // ── The box actually painted ──
        //
        // The toggle fills the width it was *given*, exactly as `Button` does, but its height
        // is its own. Painting `rect` directly drew a 240x120 census cell as a 240x120 slab
        // with the caption floating in the middle of it — a rectangle shaped like a button
        // rather than a button, and visibly taller than the `Button` beside it. The height
        // comes from `size_hint`, the same derivation a layout asks for, so the drawn shape
        // and the reported size cannot disagree.
        let rect = ControlMetrics::full_width_band(rect, self.size_hint().height);

        // ── Background ──
        let bg_color = style.background_color.unwrap_or_else(|| match state {
            ToggleButtonState::Disabled => Color::rgb(220, 220, 220),
            ToggleButtonState::Checked => Color::rgb(200, 220, 255),
            ToggleButtonState::Normal => Color::rgb(240, 240, 240),
        });
        context.fill_rect(rect, bg_color);

        // ── Border ──
        let border_color = style.border_color.unwrap_or_else(|| {
            if self.checked {
                Color::rgb(80, 120, 200)
            } else {
                Color::rgb(180, 180, 180)
            }
        });
        let bw = style.border_width.unwrap_or(0);
        let border_width = if bw > 0 { bw } else { 1 };
        context.draw_rect_stroke(rect, border_color, border_width);

        // ── Text ──
        if !self.text.is_empty() {
            let text_color = style.text_color.unwrap_or_else(|| {
                if state == ToggleButtonState::Disabled {
                    Color::rgb(150, 150, 150)
                } else {
                    Color::rgb(0, 0, 0)
                }
            });
            let default_font = crate::core::Font::default();
            let font = style.font.as_ref().unwrap_or(&default_font);
            // The label is centred in the button's own rectangle through the shared primitive:
            // a glyph origin is the box's top-left, so the old `rect.y + rect.height / 2` put
            // that edge on the middle line and drew the text half a line low.
            let line = context.text_line(rect, font);
            let text_band = crate::core::Rect::new(rect.x, line.y, rect.width, line.height);
            context.draw_text_fitted(
                text_band,
                &self.text,
                font,
                text_color,
                HorizontalAlignment::Center,
            );
        }
    }
}
impl crate::event::EventHandler for ToggleButton {
    /// Toggles on a completed activation, mirroring `Button`.
    ///
    /// # Why the hit test and the leave arm are load-bearing
    ///
    /// The press arm discarded the position (`pos: _`) and there was no `MouseLeave` arm, so a
    /// press **anywhere** set `pressed = true` and nothing ever cleared it if the pointer left
    /// without a release reaching the control. The latch then committed on an unrelated later
    /// release, and `draw` kept painting the pressed state meanwhile. `Button` guards the release
    /// on `self.pressed`; this control now does the same and arms only for a press that lands on it.
    fn handle_event(&mut self, event: &crate::event::Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            crate::event::Event::MousePress { pos, button } if *button == 1 => {
                // Arm only for a press on the control; a press outside must not leave the latch
                // set for a later release.
                self.set_pressed(self.base.contains_point_with_touch_expansion(*pos));
            }
            crate::event::Event::MouseRelease { pos, button } if *button == 1 => {
                // Commit only a press that this control armed, and only while the pointer is
                // still over it — the same two conditions `Button` applies.
                if self.pressed && self.base.contains_point_with_touch_expansion(*pos) {
                    self.toggle();
                }
                self.set_pressed(false);
            }
            crate::event::Event::MouseRelease { button: 1, .. } => {
                self.set_pressed(false);
            }
            crate::event::Event::MouseLeave { .. } => {
                // Abandon a held press; without arms this control had no leave handling at all.
                self.set_pressed(false);
            }
            crate::event::Event::FocusLost => {
                self.set_pressed(false);
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Color, ObjectId, Point, Rect};
    use crate::event::{Event, EventHandler};
    use crate::style::WidgetStyle;

    #[test]
    fn toggle_creation_defaults() {
        let tb = ToggleButton::new("Toggle".to_string(), Rect::new(0, 0, 100, 30));
        assert!(!tb.is_checked());
        assert_eq!(tb.text(), "Toggle");
        assert!(!tb.is_auto_exclusive());
        assert!(tb.group_id().is_none());
        assert!(!tb.is_pressed());
        assert_eq!(tb.state(), ToggleButtonState::Normal);
    }

    #[test]
    fn toggle_set_checked() {
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 50, 30));
        tb.set_checked(true);
        assert!(tb.is_checked());
        assert_eq!(tb.state(), ToggleButtonState::Checked);
        tb.set_checked(false);
        assert!(!tb.is_checked());
        assert_eq!(tb.state(), ToggleButtonState::Normal);
    }

    #[test]
    fn toggle_toggle_method() {
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 50, 30));
        assert!(!tb.is_checked());
        tb.toggle();
        assert!(tb.is_checked());
        tb.toggle();
        assert!(!tb.is_checked());
    }

    #[test]
    fn toggle_set_text() {
        let mut tb = ToggleButton::new("Old".to_string(), Rect::new(0, 0, 100, 30));
        assert_eq!(tb.text(), "Old");
        tb.set_text("New".to_string());
        assert_eq!(tb.text(), "New");
    }

    #[test]
    fn toggle_auto_exclusive() {
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 50, 30));
        assert!(!tb.is_auto_exclusive());
        tb.set_auto_exclusive(true);
        assert!(tb.is_auto_exclusive());
        tb.set_auto_exclusive(false);
        assert!(!tb.is_auto_exclusive());
    }

    #[test]
    fn toggle_group_id() {
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 50, 30));
        assert!(tb.group_id().is_none());
        tb.set_group_id(Some("group1".to_string()));
        assert_eq!(tb.group_id(), Some("group1"));
        tb.set_group_id(None);
        assert!(tb.group_id().is_none());
    }

    #[test]
    fn toggle_pressed_state() {
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 50, 30));
        assert!(!tb.is_pressed());
        tb.set_pressed(true);
        assert!(tb.is_pressed());
        tb.set_pressed(false);
        assert!(!tb.is_pressed());
    }

    #[test]
    fn toggle_geometry_delegation() {
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 100, 30));
        tb.set_geometry(Rect::new(10, 10, 200, 50));
        assert_eq!(tb.geometry(), Rect::new(10, 10, 200, 50));
    }

    #[test]
    fn toggle_visibility_delegation() {
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 100, 30));
        assert!(tb.is_visible());
        tb.hide();
        assert!(!tb.is_visible());
        tb.show();
        assert!(tb.is_visible());
    }

    #[test]
    fn toggle_enabled_delegation() {
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 100, 30));
        assert!(tb.is_enabled());
        tb.set_enabled(false);
        assert!(!tb.is_enabled());
        assert_eq!(tb.state(), ToggleButtonState::Disabled);
        tb.set_enabled(true);
        assert!(tb.is_enabled());
    }

    #[test]
    fn toggle_parent_children() {
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 100, 30));
        assert!(tb.parent().is_none());
        let pid: ObjectId = 42;
        tb.set_parent(Some(pid));
        assert_eq!(tb.parent(), Some(pid));
        tb.set_parent(None);
        assert!(tb.parent().is_none());

        let cid: ObjectId = 100;
        tb.add_child(cid);
        assert_eq!(tb.children().len(), 1);
        assert_eq!(tb.children()[0], cid);
        tb.remove_child(cid);
        assert!(tb.children().is_empty());
    }

    #[test]
    fn toggle_tooltip_roundtrip() {
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 100, 30));
        assert!(tb.tooltip().is_empty());
        tb.set_tooltip("Helpful tip".to_string());
        assert_eq!(tb.tooltip(), "Helpful tip");
        tb.set_tooltip(String::new());
        assert!(tb.tooltip().is_empty());
    }

    #[test]
    fn toggle_style_roundtrip() {
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 100, 30));
        assert_eq!(*tb.style(), WidgetStyle::default());
        let custom = WidgetStyle::default().with_background(Color::rgb(200, 200, 200));
        tb.set_style(custom.clone());
        assert_eq!(*tb.style(), custom);
    }

    #[test]
    fn toggle_id_kind() {
        let tb_a = ToggleButton::new("A".to_string(), Rect::new(0, 0, 50, 30));
        let tb_b = ToggleButton::new("B".to_string(), Rect::new(0, 0, 50, 30));
        assert_ne!(tb_a.id(), tb_b.id());
        assert_eq!(tb_a.kind(), WidgetKind::ToggleButton);
        assert_eq!(tb_b.kind(), WidgetKind::ToggleButton);
    }

    #[test]
    fn toggle_signal_accessors() {
        let tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 50, 30));
        // Signal1<bool>
        let _toggled = &tb.toggled;
        let _checked = &tb.checked_changed;
        let _state = &tb.state_changed;
        // GenericSignal
        let _pressed = &tb.pressed_signal;
        let _released = &tb.released_signal;
    }
    /// A press that leaves the control is abandoned, and never latches.
    ///
    /// The press arm discarded the position and there was no `MouseLeave` arm, so a press
    /// anywhere set `pressed = true` and only a release cleared it. A pointer that left while
    /// held left the latch set, and the widget then toggled on an unrelated later release.
    #[test]
    fn toggle_button_mouse_leave_abandons_a_held_press() {
        let inside = Point::new(20, 15);
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 100, 30));

        tb.handle_event(&Event::MousePress { pos: inside, button: 1 });
        assert!(tb.is_pressed(), "a press on the control arms the latch");

        tb.handle_event(&Event::MouseLeave { pos: Point::new(-1, -1) });
        assert!(!tb.is_pressed(), "leaving must clear the latch");

        // The stray release must not commit, and the latch must already be clear.
        tb.handle_event(&Event::MouseRelease { pos: inside, button: 1 });
        assert!(!tb.is_checked(), "a release after leaving must not toggle");
    }

    /// A press that does not land on the control must not arm the latch.
    #[test]
    fn toggle_button_press_outside_does_not_arm() {
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 100, 30));
        tb.handle_event(&Event::MousePress { pos: Point::new(9000, 9000), button: 1 });
        assert!(!tb.is_pressed());
        tb.handle_event(&Event::MouseRelease { pos: Point::new(20, 15), button: 1 });
        assert!(!tb.is_checked(), "a drag that began outside must not commit");
    }

    /// A completed activation still toggles.
    #[test]
    fn toggle_button_completed_activation_toggles() {
        let inside = Point::new(20, 15);
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 100, 30));
        tb.handle_event(&Event::MousePress { pos: inside, button: 1 });
        tb.handle_event(&Event::MouseRelease { pos: inside, button: 1 });
        assert!(tb.is_checked());
    }

    /// Losing focus abandons a held press.
    #[test]
    fn toggle_button_focus_loss_abandons_a_held_press() {
        let inside = Point::new(20, 15);
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 100, 30));
        tb.handle_event(&Event::MousePress { pos: inside, button: 1 });
        tb.handle_event(&Event::FocusLost);
        assert!(!tb.is_pressed());
        tb.handle_event(&Event::MouseRelease { pos: inside, button: 1 });
        assert!(!tb.is_checked());
    }
}
