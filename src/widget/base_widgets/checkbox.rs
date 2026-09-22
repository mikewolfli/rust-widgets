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
use crate::widget::metrics::dimensions;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Side of the indicator box, in logical pixels.
///
/// A **constant**, not a fraction of the control's rectangle: the indicator is a piece of the
/// control's own chrome whose size is defined by the checkbox, while the rectangle it is laid
/// out in belongs to whoever placed the control. Deriving one from the other meant the census
/// render of a 240x120 cell drew a 120 px box, and a checkbox in a wide row drew one wider
/// than its own label.
///
/// Read from the shared table so a checkbox and a radio — which sit side by side in a form —
/// cannot drift apart in size or in the gap to their labels.
const INDICATOR_SIZE: u32 = dimensions::CHECKBOX_BOX;

/// Gap between the control's left edge and the indicator box.
pub(crate) const INDICATOR_INSET: i32 = 2;

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
    /// The indicator box, in control coordinates.
    ///
    /// Extracted so the **hit test and the drawing cannot disagree** about where the checkbox is.
    /// They were previously independent: the draw derived the box from the measured line box, while
    /// the hit test ignored the pointer entirely. Sharing one derivation is what lets the hit test
    /// be narrowed to the control's own contents without risking a box drawn in one place and
    /// clicked in another.
    ///
    /// `line_height` is passed in because the line box has to come from a `RenderContext`, which
    /// the hit test does not have. A caller with a context measures; a caller without one (the
    /// event path) uses the same font's nominal height, which is what the context would return.
    fn indicator_rect(&self, line_height: u32, line_y: i32) -> Rect {
        let rect = self.geometry();
        let size = INDICATOR_SIZE.min(rect.height).min(rect.width);
        Rect::new(
            rect.x + INDICATOR_INSET,
            line_y + (line_height as i32 - size as i32) / 2,
            size,
            size,
        )
    }

    /// The region a press must land in to toggle this checkbox.
    ///
    /// The indicator plus the label beside it, **not** the whole rectangle the caller laid out. A
    /// checkbox in a wide row used to toggle from a press on empty space far to the right of its
    /// label, because its `MousePress` arm ignored the pointer position entirely.
    ///
    /// The region is then widened to the style's `touch_target` when one is set, which is the
    /// platform's minimum-touch-size mechanism: on a phone the same small indicator needs a larger
    /// reachable area than on a desktop with a mouse.
    /// The gap between the indicator and the label.
    ///
    /// Read from the style so a theme can tune it, falling back to the shared table. This is
    /// what `spacing` means throughout the crate: the distance from a control's *own*
    /// indicator to its *own* text — never the distance between two siblings, which is the
    /// parent layout's decision (QML draws the same line: `CheckBox.qml:61` uses `spacing`
    /// for this pair only).
    fn label_gap(&self) -> i32 {
        self.style().spacing.unwrap_or(dimensions::INDICATOR_TEXT_SPACING) as i32
    }

    fn hit_area(&self) -> Rect {
        let rect = self.geometry();
        // The renderer's line height for the default font is one em, which is what
        // `measure_text` returns; using the font size directly keeps this derivation identical to
        // the draw path's without needing a context here.
        let line_height = Font::default().size().max(1.0) as u32;
        let line_y = rect.y + ((rect.height as i32 - line_height as i32) / 2).max(0);
        let indicator = self.indicator_rect(line_height, line_y);
        let contents = if self.text.is_empty() {
            indicator
        } else {
            // Indicator through the end of the label, on the indicator's own row.
            let label_width = self.text.chars().count() as u32 * (line_height * 3 / 5).max(1);
            Rect::new(
                indicator.x,
                indicator.y,
                indicator.width + self.label_gap() as u32 + label_width,
                indicator.height,
            )
        };
        match self.style().touch_target {
            Some(min_size) => contents.expand_to_touch_target(min_size),
            None => contents,
        }
    }

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
            // **Hit test, not "anywhere in the rectangle".**
            //
            // This arm used to ignore `pos` entirely: a press *anywhere* inside the control's
            // rectangle toggled it. That is a different statement from "the control's touch
            // target is at least N points", and only the second one is what the platform's
            // touch-target machinery means. The two were conflated because the rectangle a
            // checkbox is given by a layout is usually close to its own indicator plus label —
            // until it is not, and a press on empty space beside a checkbox in a wide row toggles
            // it.
            //
            // The control now tests the **indicator and its label** through the shared expansion,
            // which is the reading every toolkit uses: `QCheckBox` reacts to its own contents,
            // and the minimum touch size widens that region rather than making the whole row
            // live. A caller that wants the row to toggle should size the control to the row.
            Event::MousePress { pos, button } if *button == 1 && self.base.is_enabled() => {
                if self.hit_area().contains_point(*pos) {
                    self.toggle();
                }
            }
            #[cfg(feature = "touch")]
            Event::TouchBegin { pos, .. } if self.base.is_enabled() => {
                if self.hit_area().contains_point(*pos) {
                    self.toggle();
                }
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
        let enabled = self.base.is_enabled();

        // The indicator is a **fixed-size** box centred on the control's own line box, not a
        // fraction of whatever rectangle the caller laid out. Deriving it from `rect` is what
        // made a 240x120 census render a 120 px checkbox: the control drew its indicator at
        // the scale of its container, so the same control looked like a different one from one
        // layout to the next. The line box is the honest reference — it is what the label
        // beside the box is aligned to, and it is measured rather than assumed, so a larger
        // theme font grows the indicator with the text it accompanies.
        let font = Font::default();
        let line = context.text_line(rect, &font);
        // The same derivation the hit test uses, so the box that is drawn is the box that responds.
        let checkbox_rect = self.indicator_rect(line.height, line.y);

        // The box must be visible against the surface it sits on. `style.background_color`
        // reaches here already resolved by the theme's `Input` role, so the literal fallback
        // only applies to a control whose style was never themed; the important part is that
        // the fallback is derived from the resolved value rather than from an assumption about
        // the appearance.
        let surface = style.background_color.unwrap_or(Color::WHITE);
        let field = if enabled { surface } else { surface.with_alpha(180) };
        context.fill_rect(checkbox_rect, field);

        let border_color = style.border_color.unwrap_or_else(|| field.contrast_color());
        context.draw_rect(checkbox_rect, border_color);
        if self.state != CheckState::Unchecked {
            // The mark lands on the box's own fill, so its colour is that fill's contrast
            // colour. A literal accent blue here would be unreadable whenever the field is
            // dark, which is every dark appearance.
            let check_color = style.text_color.unwrap_or_else(|| field.contrast_color());
            match self.state {
                CheckState::Checked => {
                    // A compact tick drawn as two strokes, so it scales with the box instead
                    // of being a glyph in an unrelated font's advance model.
                    let x = checkbox_rect.x;
                    let y = checkbox_rect.y;
                    let size = checkbox_rect.width as i32;
                    let thickness = (size / 8).max(1) as u32;
                    let short = size * 3 / 8;
                    let long = size * 5 / 8;
                    let mid_drop = size * 5 / 8;
                    context.draw_line_stroke(
                        Point { x: x + size / 5, y: y + mid_drop },
                        Point { x: x + short, y: y + size - size / 5 },
                        check_color,
                        thickness,
                    );
                    context.draw_line_stroke(
                        Point { x: x + short, y: y + size - size / 5 },
                        Point { x: x + long, y: y + size * 3 / 8 },
                        check_color,
                        thickness,
                    );
                }
                CheckState::PartiallyChecked => {
                    // The minus sign is centred on the box rather than offset from its middle
                    // by a hand-tuned pixel, so it stays centred at every indicator size.
                    let bar_h = (checkbox_rect.height / 8).max(1);
                    let partial_rect = Rect::new(
                        checkbox_rect.x + (checkbox_rect.width as i32 / 4),
                        checkbox_rect.y + (checkbox_rect.height as i32 - bar_h as i32) / 2,
                        checkbox_rect.width.saturating_sub(checkbox_rect.width / 2),
                        bar_h,
                    );
                    context.fill_rect(partial_rect, check_color);
                }
                CheckState::Unchecked => {}
            }
        }
        // The label shares the indicator's line box, so the two cannot drift apart when the
        // font or the control's height changes.
        if !self.text.is_empty() {
            // `spacing` is the indicator-to-label gap, and that is the *only* thing the field
            // means — the gap between two sibling controls belongs to whichever layout placed
            // them (QML keeps the two separate for the same reason: `CheckBox.qml:61` uses
            // `spacing` for this pair, never for siblings). A themed spacing therefore tunes
            // this one relation and cannot accidentally re-space a whole row.
            let gap = self.label_gap();
            let text_color = style.text_color.unwrap_or_else(|| {
                if enabled {
                    surface.contrast_color()
                } else {
                    surface.contrast_color().with_alpha(150)
                }
            });
            context.draw_text_fitted(
                Rect::new(
                    checkbox_rect.x + checkbox_rect.width as i32 + gap,
                    line.y,
                    rect.width.saturating_sub(
                        (checkbox_rect.x - rect.x) as u32 + checkbox_rect.width + gap as u32,
                    ),
                    line.height,
                ),
                &self.text,
                &font,
                text_color,
                HorizontalAlignment::Left,
            );
        }
    }
}

// These tests drive the **theme**, which only exists in a build with a device profile
// (see `crate::lib`: `pub mod theme` is gated on `device_profile`). Without this gate the
// `mini` and `embedded` profiles fail to compile their test targets, because the test code
// names a module that those builds compile out — the production code is profile-clean and
// only the fixture was not.
#[cfg(all(test, full_widgets))]
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
    fn a_press_beside_the_label_does_not_toggle_the_checkbox() {
        // The `MousePress` arm used to ignore `pos` entirely, so a press *anywhere* in the control's
        // rectangle toggled it. A checkbox given a wide row by its layout therefore flipped when the
        // user clicked empty space far to the right of its own label — a different statement from
        // "the touch target is at least N points", and the wrong one.
        let mut cb = CheckBox::new(Rect::new(0, 0, 240, 30));
        cb.set_text("Label".to_string());
        cb.handle_event(&Event::MousePress { pos: Point::new(230, 15), button: 1 });
        assert_eq!(
            cb.state(),
            CheckState::Unchecked,
            "a press on the empty part of the row must not toggle the checkbox"
        );
    }

    #[test]
    fn a_press_outside_the_control_still_hits_through_the_touch_target() {
        // The other half of the same judgement, and the mechanism BLUE21 AR1 found unconnected: a
        // control smaller than the platform's minimum touch size must still respond just outside
        // its own rectangle. The checkbox is 18 px tall, so on a phone profile (48 px) a point a few
        // pixels above it is inside the reachable target.
        let mut cb = CheckBox::new(Rect::new(20, 20, 24, 18));
        // Theming is what installs the touch target, so the test installs and applies one the way
        // the runtime does before registering a control. Reading it off an un-themed control would
        // assert the wrong precondition — the point of the mechanism is that the *platform* supplies
        // the size.
        {
            let mut manager = crate::theme::global_theme_manager();
            manager.register_theme(crate::theme::Theme::default());
            manager.set_appearance(crate::theme::AppearanceMode::Light);
        }
        crate::theme::apply_active_theme(&mut cb);
        let target = cb.style().touch_target.expect(
            "a themed control must carry a touch target — that is what `role_base_style` writes",
        );
        assert!(
            target.height > 18,
            "the target must exceed the control's own height for this test to mean anything"
        );
        // One pixel above the control's top edge, inside the expansion.
        cb.handle_event(&Event::MousePress { pos: Point::new(24, 19), button: 1 });
        assert_eq!(
            cb.state(),
            CheckState::Checked,
            "a press within the minimum touch target must reach the control"
        );
    }

    #[test]
    fn a_press_well_outside_the_touch_target_is_ignored() {
        // The reverse direction, so the previous test cannot pass by accepting everything.
        let mut cb = CheckBox::new(Rect::new(100, 100, 24, 18));
        cb.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
        assert_eq!(cb.state(), CheckState::Unchecked);
    }

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
