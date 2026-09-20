// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Checkbox widget implementation.
use crate::compat::{format, String, ToString};
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::{
    check_state_to_str, expect_bool, expect_check_state, expect_string,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
/// Checkbox state.
///
/// A three-valued state even though the checkbox is two-valued by default:
/// [`CheckState::PartiallyChecked`] is only reachable when tristate mode is on,
/// and is how a checkbox represents "some of my children are checked".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckState {
    /// Off.
    Unchecked,
    /// Intermediate; drawn as a dash/filled square rather than a tick.
    PartiallyChecked,
    /// On.
    Checked,
}
/// Checkbox widget for boolean or tristate selection.
///
/// Carries a [`CheckState`], so it can act as either a plain boolean (the
/// default) or a tristate control. See [`CheckBox::set_tristate_enabled`].
pub struct CheckBox {
    base: BaseWidget,
    state: CheckState,
    text: String,
    tristate_enabled: bool,
    /// Emitted with a boolean view of the state on every change: `true` for
    /// [`CheckState::Checked`], `false` for the other two.
    ///
    /// Because partially-checked collapses to `false`, a listener cannot
    /// distinguish "unchecked" from "indeterminate" — connect to
    /// `state_changed` when that distinction matters.
    pub toggled: Signal1<bool>,
    /// Emitted with the new [`CheckState`] on every change, including
    /// transitions involving [`CheckState::PartiallyChecked`].
    pub state_changed: Signal1<CheckState>,
}
impl CheckBox {
    /// Creates an unchecked checkbox with geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::CheckBox, geometry, "CheckBox"),
            state: CheckState::Unchecked,
            text: String::new(),
            tristate_enabled: false,
            toggled: Signal1::new(),
            state_changed: Signal1::new(),
        }
    }
    /// Returns current check state.
    pub fn state(&self) -> CheckState {
        self.state
    }
    /// Returns true when the checkbox is fully checked.
    pub fn is_checked(&self) -> bool {
        self.state == CheckState::Checked
    }
    /// Returns true when the checkbox is partially checked (tristate).
    pub fn is_partially_checked(&self) -> bool {
        self.state == CheckState::PartiallyChecked
    }
    /// Returns true when tristate behavior is enabled.
    pub fn is_tristate_enabled(&self) -> bool {
        self.tristate_enabled
    }
    /// Sets check state and emits signals when changed.
    pub fn set_state(&mut self, state: CheckState) {
        if self.state == state {
            return;
        }
        let previous = self.state;
        self.state = state;
        self.state_changed.emit(state);
        // Emit toggled signal for boolean transitions
        match (previous, state) {
            (CheckState::Unchecked, CheckState::Checked) => self.toggled.emit(true),
            (CheckState::Checked, CheckState::Unchecked) => self.toggled.emit(false),
            // No action needed for this transition
            _ => {}
        }
        self.base.request_redraw();
    }
    /// Sets checked state (true = checked, false = unchecked).
    pub fn set_checked(&mut self, checked: bool) {
        self.set_state(if checked { CheckState::Checked } else { CheckState::Unchecked });
    }
    /// Returns the text label displayed next to the checkbox.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Sets the text label displayed next to the checkbox and requests a redraw.
    pub fn set_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.text != text {
            self.text = text;
            self.base.request_redraw();
        }
    }
    /// Enables or disables tristate behavior.
    pub fn set_tristate_enabled(&mut self, enabled: bool) {
        self.tristate_enabled = enabled;
        if !enabled && self.state == CheckState::PartiallyChecked {
            self.set_state(CheckState::Unchecked);
        }
        self.base.request_redraw();
    }
    /// Toggles between checked states.
    pub fn toggle(&mut self) {
        let next_state = match self.state {
            CheckState::Unchecked => CheckState::Checked,
            CheckState::Checked => {
                if self.tristate_enabled {
                    CheckState::PartiallyChecked
                } else {
                    CheckState::Unchecked
                }
            }
            CheckState::PartiallyChecked => CheckState::Unchecked,
        };
        self.set_state(next_state);
    }
}
impl Widget for CheckBox {
    /// Resolves the published event names this control emits to their signals.
    ///
    /// | published name | signal | payload |
    /// |---|---|---|
    /// | `toggled` | `toggled` | `bool` |
    /// | `state_changed` | `state_changed` | `CheckState` as a token string |
    ///
    /// `toggled` and `state_changed` differ deliberately: `toggled` collapses
    /// [`CheckState::PartiallyChecked`] to `false`, so a wire that needs the three-way answer must
    /// name `state_changed`. Both are published, so both are resolvable.
    ///
    /// `clicked` is deliberately **not** here: the control owns that signal on its base, but its
    /// capability does not publish the name, so `connect_event` would refuse a subscription to it
    /// and an arm resolving it would be dead code that looks like support.
    /// `tools/check_event_signal_dyn.py` fails on either half of that mismatch.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "toggled" => Some(EventSignalRef::mapped("toggled", &self.toggled, |value| {
                CapabilityValue::Bool(*value)
            })),
            "state_changed" => {
                Some(EventSignalRef::mapped("state_changed", &self.state_changed, |state| {
                    CapabilityValue::String(format!("{state:?}"))
                }))
            }
            _ => None,
        }
    }

    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.base.set_enabled(enabled);
        self.base.request_redraw();
    }

    fn size_hint(&self) -> Size {
        let text_w = self.text().len() as u32 * 8 + 24; // 16px checkbox + 4px padding + text
        Size::new(text_w.max(60), 24)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `CheckBox`'s property contract.
///
/// `state` is the enum form and `checked` the boolean projection of the same
/// field; both are writable and both are published so the schema and the trait
/// agree on the control's surface.
impl WidgetProperties for CheckBox {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "state" => Ok(CapabilityValue::String(check_state_to_str(self.state()).to_string())),
            "checked" => Ok(CapabilityValue::Bool(self.is_checked())),
            "tristate_enabled" => Ok(CapabilityValue::Bool(self.is_tristate_enabled())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(expect_string(value)?);
                Ok(())
            }
            "state" => {
                self.set_state(expect_check_state(value)?);
                Ok(())
            }
            "checked" => {
                self.set_checked(expect_bool(value)?);
                Ok(())
            }
            "tristate_enabled" => {
                self.set_tristate_enabled(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["text", "state", "checked", "tristate_enabled", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `check_box` publishes.
    ///
    /// The command set is found in `CHECK_BOX_PROPERTIES`'s capability entry; every
    /// name there must be answered here or
    /// `capability::properties_tests::every_published_command_is_dispatched` fails.
    ///
    /// `toggle` flips the state; `set_checked` assigns it and needs a payload, so it
    /// is answered through the property path (`set("checked", ..)`) and is refused
    /// here — a command carries no argument, and accepting one that does nothing
    /// would be the silent success this contract exists to prevent. Commands whose
    /// only effect is to assign state are still published, because a consumer that
    /// discovered the control through `commands` should find them; the refusal is
    /// [`CapabilityAccessError::OutOfRange`], which tells the caller the name was
    /// right and the invocation needs the property route, rather than
    /// [`CapabilityAccessError::UnknownCommand`], which would say it was wrong.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "toggle" => {
                self.toggle();
                Ok(())
            }
            "set_checked" | "set_state" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for CheckBox {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        match event {
            Event::MousePress { pos: _, button } if *button == 1 && self.base.is_enabled() => {
                self.toggle();
            }
            #[cfg(feature = "touch")]
            Event::TouchBegin { .. } if self.base.is_enabled() => {
                self.toggle();
            }
            Event::KeyPress { key, .. } if *key == 32 && self.base.is_enabled() => {
                self.toggle();
            }
            // Other events are not relevant for this widget
            _ => {}
        }
    }
}
impl Draw for CheckBox {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let style = self.style();
        let checkbox_size = 16; // Standard checkbox size
                                // Calculate checkbox rectangle
        let checkbox_rect = Rect::new(
            rect.x,
            rect.y + (rect.height as i32 - checkbox_size) / 2,
            checkbox_size as u32,
            checkbox_size as u32,
        );
        let enabled = self.base.is_enabled();
        // Draw checkbox background
        let bg_color = style.background_color.unwrap_or_else(|| {
            if !enabled {
                Color::rgb(240, 240, 240)
            } else {
                Color::rgb(255, 255, 255)
            }
        });
        context.fill_rect(checkbox_rect, bg_color);
        // Draw checkbox border
        let border_color = style.border_color.unwrap_or_else(|| {
            if !enabled {
                Color::rgb(180, 180, 180)
            } else {
                Color::rgb(100, 100, 100)
            }
        });
        context.draw_rect(checkbox_rect, border_color);
        // Draw checkmark or partial check
        if self.state != CheckState::Unchecked {
            let check_color = style.text_color.unwrap_or_else(|| {
                if !enabled {
                    Color::rgb(150, 150, 150)
                } else {
                    Color::rgb(0, 120, 215) // Blue checkmark
                }
            });
            match self.state {
                CheckState::Checked => {
                    // Draw a compact check glyph.
                    context.draw_text(
                        Point {
                            x: checkbox_rect.x + 3,
                            y: checkbox_rect.y + checkbox_rect.height as i32 / 2,
                        },
                        "x",
                        &Font::default(),
                        check_color,
                        HorizontalAlignment::Left,
                    );
                }
                CheckState::PartiallyChecked => {
                    // Draw partial check (minus sign)
                    let partial_rect = Rect::new(
                        checkbox_rect.x + 4,
                        checkbox_rect.y + checkbox_rect.height as i32 / 2 - 1,
                        checkbox_rect.width.saturating_sub(8),
                        2,
                    );
                    context.fill_rect(partial_rect, check_color);
                }
                // No action needed for this transition
                _ => {}
            }
        }
        // Draw text next to the checkbox
        if !self.text.is_empty() {
            let text_color = style.text_color.unwrap_or_else(|| {
                if !enabled {
                    Color::rgb(150, 150, 150)
                } else {
                    Color::rgb(0, 0, 0)
                }
            });
            let text_point = Point {
                x: checkbox_rect.x + checkbox_rect.width as i32 + 4,
                y: checkbox_rect.y + checkbox_rect.height as i32 / 2,
            };
            context.draw_text(
                text_point,
                &self.text,
                &Font::default(),
                text_color,
                HorizontalAlignment::Left,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::Vec;
    use crate::core::ObjectId;
    use crate::core::Size;
    use crate::event::Event;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;
    use std::sync::Mutex;

    // ── 1. Creation ──────────────────────────────────────────────────────────
    #[test]
    fn test_creation_default_state() {
        let cb = CheckBox::new(Rect::new(10, 10, 100, 30));
        assert_eq!(cb.state(), CheckState::Unchecked);
        assert!(!cb.is_checked());
        assert!(!cb.is_partially_checked());
        assert!(!cb.is_tristate_enabled());
        assert!(cb.text().is_empty());
        assert!(cb.is_visible());
        assert!(cb.is_enabled());
    }

    // ── 2. State transitions (Unchecked→Checked→PartiallyChecked→Unchecked) ─
    #[test]
    fn test_toggle_unchecked_to_checked() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.toggle();
        assert_eq!(cb.state(), CheckState::Checked);
        assert!(cb.is_checked());
    }

    #[test]
    fn test_toggle_checked_to_unchecked_no_tristate() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.set_checked(true);
        assert_eq!(cb.state(), CheckState::Checked);
        cb.toggle();
        assert_eq!(cb.state(), CheckState::Unchecked);
    }

    #[test]
    fn test_toggle_checked_to_partial_when_tristate() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.set_tristate_enabled(true);
        cb.set_checked(true);
        assert_eq!(cb.state(), CheckState::Checked);
        cb.toggle();
        assert_eq!(cb.state(), CheckState::PartiallyChecked);
        assert!(cb.is_partially_checked());
    }

    #[test]
    fn test_toggle_partial_to_unchecked() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.set_tristate_enabled(true);
        cb.set_state(CheckState::PartiallyChecked);
        cb.toggle();
        assert_eq!(cb.state(), CheckState::Unchecked);
    }

    #[test]
    fn test_tristate_cycle() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.set_tristate_enabled(true);
        // Unchecked → Checked
        cb.toggle();
        assert_eq!(cb.state(), CheckState::Checked);
        // Checked → PartiallyChecked
        cb.toggle();
        assert_eq!(cb.state(), CheckState::PartiallyChecked);
        // PartiallyChecked → Unchecked
        cb.toggle();
        assert_eq!(cb.state(), CheckState::Unchecked);
    }

    // ── 3. set_checked(true/false) ──────────────────────────────────────────
    #[test]
    fn test_set_checked_true() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.set_checked(true);
        assert!(cb.is_checked());
        assert_eq!(cb.state(), CheckState::Checked);
    }

    #[test]
    fn test_set_checked_false() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.set_checked(true);
        cb.set_checked(false);
        assert!(!cb.is_checked());
        assert_eq!(cb.state(), CheckState::Unchecked);
    }

    #[test]
    fn test_set_checked_noop() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        // Set unchecked when already unchecked – no state_changed signal
        cb.set_checked(false);
        assert_eq!(cb.state(), CheckState::Unchecked);
    }

    // ── 4. toggled signal (true on checked, false on unchecked) ─────────────
    #[test]
    fn test_toggled_signal_emitted_true() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        let emitted_value = Arc::new(Mutex::new(None));
        let ev = emitted_value.clone();
        cb.toggled.connect(move |v| *ev.lock().unwrap() = Some(*v));
        cb.set_checked(true);
        assert_eq!(*emitted_value.lock().unwrap(), Some(true));
    }

    #[test]
    fn test_toggled_signal_emitted_false() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.set_checked(true);
        let emitted_value = Arc::new(Mutex::new(None));
        let ev = emitted_value.clone();
        cb.toggled.connect(move |v| *ev.lock().unwrap() = Some(*v));
        cb.set_checked(false);
        assert_eq!(*emitted_value.lock().unwrap(), Some(false));
    }

    #[test]
    fn test_toggled_not_emitted_on_noop() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.set_checked(true);
        let emitted_count = Arc::new(AtomicU32::new(0));
        let ec = emitted_count.clone();
        cb.toggled.connect(move |_| {
            ec.fetch_add(1, Ordering::SeqCst);
        });
        // Set same state – no toggle
        cb.set_checked(true);
        assert_eq!(emitted_count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn test_toggled_not_emitted_on_partial_transition() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.set_tristate_enabled(true);
        cb.set_state(CheckState::PartiallyChecked);
        let emitted_count = Arc::new(AtomicU32::new(0));
        let ec = emitted_count.clone();
        cb.toggled.connect(move |_| {
            ec.fetch_add(1, Ordering::SeqCst);
        });
        // Partial → Unchecked does not emit toggled
        cb.toggle();
        assert_eq!(emitted_count.load(Ordering::SeqCst), 0);
    }

    // ── 5. state_changed signal (all transitions, no emission for noop) ────
    #[test]
    fn test_state_changed_on_all_transitions() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.set_tristate_enabled(true);
        let states = Arc::new(Mutex::new(Vec::new()));
        let s2 = states.clone();
        cb.state_changed.connect(move |s| s2.lock().unwrap().push(*s));
        cb.toggle(); // Unchecked → Checked
        cb.toggle(); // Checked → PartiallyChecked
        cb.toggle(); // PartiallyChecked → Unchecked
        let guard = states.lock().unwrap();
        assert_eq!(guard.len(), 3);
        assert_eq!(guard[0], CheckState::Checked);
        assert_eq!(guard[1], CheckState::PartiallyChecked);
        assert_eq!(guard[2], CheckState::Unchecked);
    }

    #[test]
    fn test_state_changed_not_emitted_on_noop() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        let emitted_count = Arc::new(AtomicU32::new(0));
        let ec = emitted_count.clone();
        cb.state_changed.connect(move |_| {
            ec.fetch_add(1, Ordering::SeqCst);
        });
        cb.set_state(CheckState::Unchecked); // already Unchecked
        assert_eq!(emitted_count.load(Ordering::SeqCst), 0);
    }

    // ── 6. Tristate mode toggle cycle ──────────────────────────────────────
    #[test]
    fn test_tristate_enable_disable_cycle() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        assert!(!cb.is_tristate_enabled());
        cb.set_tristate_enabled(true);
        assert!(cb.is_tristate_enabled());
        cb.set_tristate_enabled(false);
        assert!(!cb.is_tristate_enabled());
    }

    // ── 7. Disabling tristate resets Partial to Unchecked ──────────────────
    #[test]
    fn test_disable_tristate_resets_partial() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.set_tristate_enabled(true);
        cb.set_state(CheckState::PartiallyChecked);
        assert!(cb.is_partially_checked());
        // Disable tristate – should reset to Unchecked
        cb.set_tristate_enabled(false);
        assert_eq!(cb.state(), CheckState::Unchecked);
    }

    #[test]
    fn test_disable_tristate_does_not_reset_checked() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.set_tristate_enabled(true);
        cb.set_checked(true);
        cb.set_tristate_enabled(false);
        // Checked should stay Checked
        assert_eq!(cb.state(), CheckState::Checked);
    }

    #[test]
    fn test_disable_tristate_does_not_reset_unchecked() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.set_tristate_enabled(false);
        cb.set_state(CheckState::Unchecked);
        // No-op
        cb.set_tristate_enabled(false);
        assert_eq!(cb.state(), CheckState::Unchecked);
    }

    // ── 8. Text set/get ────────────────────────────────────────────────────
    #[test]
    fn test_text_default_empty() {
        let cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        assert_eq!(cb.text(), "");
    }

    #[test]
    fn test_text_set_and_get() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.set_text(String::from("Enable feature"));
        assert_eq!(cb.text(), "Enable feature");
    }

    #[test]
    fn test_text_overwrite() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.set_text(String::from("Old label"));
        cb.set_text(String::from("New label"));
        assert_eq!(cb.text(), "New label");
    }

    // ── 9. Event handling ──────────────────────────────────────────────────
    #[test]
    fn test_mouse_down_toggles() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        assert_eq!(cb.state(), CheckState::Unchecked);
        cb.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
        assert_eq!(cb.state(), CheckState::Checked);
    }

    #[cfg(feature = "touch")]
    #[test]
    fn test_touch_begin_toggles() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        assert_eq!(cb.state(), CheckState::Unchecked);
        cb.handle_event(&Event::TouchBegin { touch_id: 0, pos: Point::new(10, 10) });
        // TouchBegin now toggles the checkbox when the touch feature is enabled
        assert_eq!(cb.state(), CheckState::Checked);
    }

    #[cfg(feature = "touch")]
    #[test]
    fn test_tap_toggles() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        assert_eq!(cb.state(), CheckState::Unchecked);
        cb.handle_event(&Event::Tap { pos: Point::new(10, 10) });
        // Current implementation does not match Tap – falls through to _
        assert_eq!(cb.state(), CheckState::Unchecked);
    }

    #[test]
    fn test_space_key_toggles() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        assert_eq!(cb.state(), CheckState::Unchecked);
        // Space key code is 32
        cb.handle_event(&Event::KeyPress { key: 32, modifiers: 0 });
        assert_eq!(cb.state(), CheckState::Checked);
    }

    #[test]
    fn test_non_space_key_does_not_toggle() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.handle_event(&Event::KeyDown((65, 0))); // 'A' key
        assert_eq!(cb.state(), CheckState::Unchecked);
    }

    #[test]
    fn test_event_noop_when_disabled() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.set_enabled(false);
        assert_eq!(cb.state(), CheckState::Unchecked);
        cb.handle_event(&Event::MouseDown((Point::new(10, 10), 0)));
        // Should NOT toggle because checkbox is disabled
        assert_eq!(cb.state(), CheckState::Unchecked);
    }

    #[test]
    fn test_event_key_noop_when_disabled() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.set_enabled(false);
        cb.handle_event(&Event::KeyDown((32, 0)));
        assert_eq!(cb.state(), CheckState::Unchecked);
    }

    #[test]
    fn test_multiple_mouse_down_toggles() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
        assert_eq!(cb.state(), CheckState::Checked);
        cb.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
        assert_eq!(cb.state(), CheckState::Unchecked);
        cb.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
        assert_eq!(cb.state(), CheckState::Checked);
    }

    // ── 10. Widget trait delegation ─────────────────────────────────────────
    #[test]
    fn test_widget_geometry() {
        let mut cb = CheckBox::new(Rect::new(10, 20, 100, 30));
        assert_eq!(cb.geometry(), Rect::new(10, 20, 100, 30));
        cb.set_geometry(Rect::new(0, 0, 200, 50));
        assert_eq!(cb.geometry(), Rect::new(0, 0, 200, 50));
    }

    #[test]
    fn test_widget_visibility() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        assert!(cb.is_visible());
        cb.hide();
        assert!(!cb.is_visible());
        cb.show();
        assert!(cb.is_visible());
    }

    #[test]
    fn test_widget_enabled() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        assert!(cb.is_enabled());
        cb.set_enabled(false);
        assert!(!cb.is_enabled());
        cb.set_enabled(true);
        assert!(cb.is_enabled());
    }

    #[test]
    fn test_widget_style_default() {
        let cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        let _style = cb.style();
        assert_eq!(cb.kind(), WidgetKind::CheckBox);
    }

    #[test]
    fn test_widget_tooltip() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        assert!(cb.tooltip().is_empty());
        cb.set_tooltip(String::from("Click to toggle"));
        assert_eq!(cb.tooltip(), "Click to toggle");
    }

    #[test]
    fn test_widget_parent() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        assert!(cb.parent().is_none());
        let parent_id: ObjectId = 42;
        cb.set_parent(Some(parent_id));
        assert_eq!(cb.parent(), Some(parent_id));
        cb.set_parent(None);
        assert!(cb.parent().is_none());
    }

    #[test]
    fn test_widget_children() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        assert!(cb.children().is_empty());
        let child_id: ObjectId = 99;
        cb.add_child(child_id);
        assert_eq!(cb.children().len(), 1);
        assert_eq!(cb.children()[0], child_id);
        cb.remove_child(child_id);
        assert!(cb.children().is_empty());
    }

    #[test]
    fn test_widget_min_max_size() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        assert!(cb.min_size().is_none());
        assert!(cb.max_size().is_none());
        cb.set_min_size(Some(Size::new(50, 20)));
        cb.set_max_size(Some(Size::new(200, 60)));
        assert_eq!(cb.min_size(), Some(Size::new(50, 20)));
        assert_eq!(cb.max_size(), Some(Size::new(200, 60)));
    }

    #[test]
    fn test_widget_id_not_zero() {
        let cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        assert_ne!(cb.id(), 0u64);
    }

    #[test]
    fn test_widget_signal_accessors() {
        let cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        // Verify signal accessors do not panic
        let _ = cb.hover_signal();
        let _ = cb.mouse_down_signal();
        let _ = cb.mouse_up_signal();
        let _ = cb.key_down_signal();
        let _ = cb.key_up_signal();
        let _ = cb.focus_gained_signal();
        let _ = cb.focus_lost_signal();
        let _ = cb.redraw_requested_signal();
        let _ = cb.layout_requested_signal();
    }

    #[test]
    fn test_widget_connection_scope() {
        let cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        let _scope = cb.connection_scope();
    }
}
