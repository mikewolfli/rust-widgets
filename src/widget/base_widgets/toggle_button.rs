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
/// # Why the two interaction states are derived rather than stored
///
/// This enum used to have exactly the two *persistent* states (`Checked` / `Normal`) plus
/// `Disabled`, so `draw` had no way to express "the pointer is over this" or "this is being held"
/// — the control's own `is_pressed` flag was maintained and then never read. A user toggling a
/// button therefore got no feedback at all until the latch flipped, which is the one moment the
/// feedback was least needed.
///
/// `Hover` and `Pressed` are read off [`BaseWidget`], which is where the crate keeps its single
/// copy of those facts (that is what makes M1 a zero-code win for every control). They sit
/// **below** `Checked`/`Disabled` in the precedence because those are persistent facts about the
/// control while these are momentary: a checked button that is hovered is still usefully called
/// `Checked`, and the hover is then an *overlay* the draw path adds — the same layering
/// [`Widget::widget_state`] describes. Putting it in this order is what keeps the existing
/// `state` property value stable for every caller that already reads it.
///
/// Derived from the checked and enabled flags, never stored: see
/// [`ToggleButton::state`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToggleButtonState {
    /// Enabled, not checked, and not being pointed at or held.
    Normal,
    /// Enabled, not checked, and the pointer is over it.
    Hover,
    /// Enabled, not checked, and currently held down.
    Pressed,
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
    /// The last value `pressed_signal`/`released_signal` were emitted for.
    ///
    /// The *paint* flag is [`BaseWidget::is_pressed`] (the crate's single source); this
    /// is only the edge latch that decides whether the signals have been announced, so a
    /// no-op repeat set cannot re-emit. See `Button::signaled_pressed` for the full
    /// argument.
    signaled_pressed: bool,
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
    /// The interpolated interaction progress, 0.0 at rest and 1.0 fully pressed.
    ///
    /// The same type and the same contract as `Button::interaction_progress`. It exists so a
    /// hover fades in and a press deepens it instead of both snapping — and, at progress `0.0`,
    /// the fill is exactly the resting colour, which is why adding it left every committed
    /// snapshot byte-identical.
    ///
    /// [`crate::style::PropertyDriver`] owns the value **and its target**, so the second
    /// `interaction_target` field this control used to carry is gone — see `Button`'s field
    /// for the same reasoning (BLUE24 §2.3).
    interaction_progress: crate::style::PropertyDriver,
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
            signaled_pressed: false,
            toggled: Signal1::new(),
            checked_changed: Signal1::new(),
            pressed_signal: GenericSignal::new(),
            released_signal: GenericSignal::new(),
            state_changed: Signal1::new(),
            interaction_progress: crate::style::PropertyDriver::default(),
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
        self.base.is_pressed()
    }
    /// Sets the pressed flag.
    ///
    /// No-op when unchanged, so the signals fire only on an actual edge: a
    /// rising edge emits `pressed_signal`, a falling edge emits
    /// `released_signal`. Unlike the checked flag, this does not request a
    /// redraw.
    pub fn set_pressed(&mut self, pressed: bool) {
        self.base.set_pressed(pressed);
        if self.signaled_pressed == pressed {
            return;
        }
        self.signaled_pressed = pressed;
        if pressed {
            self.pressed_signal.emit();
        } else {
            self.released_signal.emit();
        }
    }
    /// Returns the interaction state derived from the enabled, checked, hovered
    /// and pressed flags. Disabled takes precedence over checked, and checked over
    /// the two momentary states; see [`ToggleButtonState`] for why that order.
    pub fn state(&self) -> ToggleButtonState {
        if !self.base.enabled {
            ToggleButtonState::Disabled
        } else if self.checked {
            ToggleButtonState::Checked
        } else if self.base.is_pressed() {
            ToggleButtonState::Pressed
        } else if self.base.is_hovered() {
            ToggleButtonState::Hover
        } else {
            ToggleButtonState::Normal
        }
    }

    /// The progress the current interaction state calls for.
    ///
    /// Mirrors `Button::interaction_target_progress`: a press is the furthest point, a hover a
    /// partial step, and a disabled control always rests. The checked state is deliberately **not**
    /// part of this — a latch is a value, not a gesture, and animating it would make the toggle's
    /// fill depend on when the user last clicked rather than on whether it is on.
    fn interaction_target_progress(&self) -> f32 {
        if !self.base.enabled {
            0.0
        } else if self.base.is_pressed() {
            1.0
        } else if self.base.is_hovered() {
            0.5
        } else {
            0.0
        }
    }

    /// Advances the interaction transition by `delta_ms`, reporting whether another
    /// frame is needed.
    ///
    /// The same contract as `Button::tick` — and, like it, the duration is the active theme's
    /// `Motion::normal` rather than a constant in this file, so a theme can state its own tempo
    /// and a test can shorten it to reach the end state deterministically.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        self.interaction_progress.set_target(self.interaction_target_progress());
        self.interaction_progress.tick(delta_ms)
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

    /// Lifts the control's own `tick` onto the trait so a host holding `&mut dyn Widget` can
    /// advance it. One line, and it is the whole reason the trait method exists: without it
    /// the animation is written and unreachable.
    fn tick(&mut self, delta_ms: u32) -> bool {
        ToggleButton::tick(self, delta_ms)
    }

    fn is_animating(&self) -> bool {
        // Derived from the *state*, not from the tick-time target field: a hover arriving makes
        // the control animating at once, before any `tick` has run to recompute the target.
        // Reading the stale field would answer `false` for a button that was just hovered, so the
        // bus would never start its frames. `Button::is_animating` documents the same trap.
        self.interaction_progress.value() != self.interaction_target_progress()
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
                    ToggleButtonState::Hover => "hover",
                    ToggleButtonState::Pressed => "pressed",
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
        // Read the three role colours out and **release the guard** before drawing. `theme_manager()`
        // returns a `MutexGuard`, and the accessors below (`layer_color`, `semantic_color`) take the
        // same non-reentrant lock themselves — so holding it here and calling one of them deadlocks.
        // The crate's own doc says a guard must not be held across a draw; this is that rule.
        let (outline, outline_variant, disabled) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => (
                    Some(active.colors.outline),
                    Some(active.colors.outline_variant),
                    Some(active.colors.disabled),
                ),
                None => (None, None, None),
            }
        };

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
        //
        // Three literals used to decide this, and the enum had no hover or pressed arm at all, so
        // a toggle button gave **no feedback until the latch flipped** — the one moment feedback
        // was least needed. The resting colours now come from the theme like every other control's
        // (BLUE23 §5 / M4), and the interaction is a **blend** whose weight is the transition's
        // progress, so a hover fades in and a press deepens it rather than both snapping. At
        // progress `0.0` the result is exactly the resting colour, which is why every committed
        // snapshot stayed byte-identical.
        let resting = style.background_color.unwrap_or_else(|| {
            // Both resting colours are **theme roles** rather than the literals that used to be
            // here (`rgb(200,220,255)` checked, `rgb(240,240,240)` normal). The checked one was
            // the light preset's tint, so a checked toggle read wrong on every dark appearance;
            // the roles move with the appearance by construction.
            let role = if self.checked {
                crate::style::LayerColor::SurfaceContainerHigh
            } else {
                crate::style::LayerColor::SurfaceContainer
            };
            crate::style::layer_color(role).unwrap_or(if self.checked {
                Color::rgb(200, 220, 255)
            } else {
                Color::rgb(240, 240, 240)
            })
        });
        let bg_color = if state == ToggleButtonState::Disabled {
            resting
        } else {
            let progress = self.interaction_progress.value();
            if progress <= 0.0 {
                resting
            } else {
                let interactive = if state == ToggleButtonState::Pressed {
                    resting
                } else {
                    resting.blend(&resting.contrast_color(), 0.22)
                };
                resting.blend(&interactive, progress)
            }
        };
        context.fill_rect(rect, bg_color);

        // ── Border ──
        //
        // The checked border is the theme's **info** token rather than the literal
        // `rgb(80, 120, 200)`, for the same reason the fill above is: a control whose "on" colour
        // does not move with the appearance is theme-blind in exactly the way the census exists to
        // catch. The unchecked border reads the theme's separator role — the weak one when
        // disabled, so "switched off" and "inert" are two different lines rather than the same
        // grey.
        let border_color = style.border_color.unwrap_or_else(|| {
            if self.checked {
                crate::style::semantic_color(crate::style::SemanticColor::Info)
                    .unwrap_or(Color::rgb(80, 120, 200))
            } else if state == ToggleButtonState::Disabled {
                outline_variant.unwrap_or(Color::rgb(180, 180, 180))
            } else {
                outline.unwrap_or(Color::rgb(180, 180, 180))
            }
        });
        let bw = style.border_width.unwrap_or(0);
        let border_width = if bw > 0 { bw } else { 1 };
        context.draw_rect_stroke(rect, border_color, border_width);

        // ── Text ──
        if !self.text.is_empty() {
            let text_color = style.text_color.unwrap_or_else(|| {
                if state == ToggleButtonState::Disabled {
                    // The disabled ink is the theme's own disabled colour rather than the
                    // near-invisible `rgb(150, 150, 150)` literal, which measured below the
                    // 4.5:1 floor once the disabled fill went to the theme's surface.
                    disabled.unwrap_or(Color::rgb(150, 150, 150))
                } else {
                    // The label sits on the control's own fill, so it takes that fill's contrast
                    // colour instead of a literal black — the same rule `CheckBox`'s mark, the
                    // `group_box` tick and this crate's other indicators follow.
                    bg_color.contrast_color()
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
                // still over it — the same two conditions `Button` applies. The guard reads
                // `signaled_pressed`, not the base flag: the base clears `pressed` on the
                // release before this arm runs, so the paint flag can no longer say
                // whether this control had armed the gesture.
                if self.signaled_pressed && self.base.contains_point_with_touch_expansion(*pos) {
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

    /// BLUE23 附录 A.2 / M1: the two *momentary* states are now reachable.
    ///
    /// # The defect this pins
    ///
    /// `ToggleButtonState` had exactly `Normal` / `Checked` / `Disabled`. The control maintained
    /// `is_pressed` and this crate's `BaseWidget` maintains `hovered`, and **neither reached the
    /// enum**, so `draw` could not express either. A user pointing at a toggle button got no
    /// feedback at all until the latch flipped — the one moment feedback was least needed.
    #[test]
    fn a_momentary_state_is_reachable_on_a_toggle_button() {
        let inside = Point::new(20, 15);
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 100, 30));
        assert_eq!(tb.state(), ToggleButtonState::Normal);

        tb.handle_event(&Event::MouseEnter { pos: inside });
        assert_eq!(tb.state(), ToggleButtonState::Hover, "a pointer over the control is a state");

        tb.handle_event(&Event::MousePress { pos: inside, button: 1 });
        assert_eq!(tb.state(), ToggleButtonState::Pressed, "a held control is a state");

        tb.handle_event(&Event::MouseRelease { pos: inside, button: 1 });
        // The latch wins once it is set: "checked" is a persistent fact, and the hover it
        // happens to sit under is the *overlay* the draw path adds. Asserting the precedence
        // stops someone "fixing" the enum order later and silently breaking the property value.
        assert_eq!(tb.state(), ToggleButtonState::Checked);

        tb.handle_event(&Event::MouseLeave { pos: inside });
        tb.set_checked(false);
        assert_eq!(tb.state(), ToggleButtonState::Normal);

        tb.set_enabled(false);
        assert_eq!(
            tb.state(),
            ToggleButtonState::Disabled,
            "disabled outranks a momentary state, exactly as it outranks checked"
        );
    }

    /// The state the property path publishes has to name the new arms, or the two views of the
    /// enum disagree (BLUE23 M10: "declared but cannot be read back").
    #[test]
    fn the_state_property_names_every_arm() {
        let inside = Point::new(20, 15);
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 100, 30));
        let read = |w: &ToggleButton| match w.get("state") {
            Ok(CapabilityValue::String(s)) => s,
            other => panic!("state must read back as a string, got {other:?}"),
        };
        assert_eq!(read(&tb), "normal");
        tb.handle_event(&Event::MouseEnter { pos: inside });
        assert_eq!(read(&tb), "hover");
        tb.handle_event(&Event::MousePress { pos: inside, button: 1 });
        assert_eq!(read(&tb), "pressed");
    }

    /// BLUE23 附录 A.2 / M3 + §9 判据 10: three frames of the interaction are **geometrically
    /// distinct**, and the fill arrives at the resting colour when the transition is at rest.
    ///
    /// The plan asks for "三帧几何互异" on a button. A toggle button's shape does not move, so the
    /// observable is the **fill**: at `t=0` it is the resting colour, mid-transition it is between
    /// the resting and interactive colours, and at the end it has settled on the interactive one.
    #[test]
    fn the_interaction_transition_moves_the_fill_across_three_frames() {
        let _guard = crate::theme::theme_test_guard();
        // The presets carry the `toggle_button:hover` / `:pressed` overrides, so a build with no
        // theme would have nothing to move the fill *toward* — and this test would be asserting a
        // property of a palette that is not installed.
        crate::widget::census::install_preset_appearances();
        let inside = Point::new(20, 15);
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 100, 30));

        // At rest the control owes no frames, so it costs nothing per frame (judgement 8).
        assert!(!tb.is_animating(), "a resting toggle must not ask for frames");
        let resting = rendered_fill(&mut tb);

        // Hovering makes it animating *before* any tick has run — the trap `Button` documents:
        // reading the tick-time target field instead of the state answers `false` here and the
        // bus would never start.
        tb.handle_event(&Event::MouseEnter { pos: inside });
        assert!(tb.is_animating(), "a hovered toggle owes frames at once");

        // Frame 1: progress is still 0, so the fill is the resting colour exactly. This is also
        // why adding the transition left every committed snapshot byte-identical.
        assert_eq!(rendered_fill(&mut tb), resting, "the first frame is the resting colour");

        // The theme's own tempo, so a test can shorten it rather than wait. A one-millisecond step
        // would never land *exactly* on the target (`tick` answers `false` only within
        // `f32::EPSILON`), so the step is a whole frame's worth — the same 1000 ms `Button`'s own
        // transition tests use — and the loop is bounded besides.
        let mut mid = resting;
        for _ in 0..64 {
            if !tb.tick(16) {
                break;
            }
            let now = rendered_fill(&mut tb);
            if now != resting {
                mid = now;
                break;
            }
        }
        assert_ne!(mid, resting, "the transition must move the fill, not snap at the end");

        // Frame 3: run it out, and require it to settle *and* to stop asking for frames — the
        // economy `is_animating` exists for (a settled control costs nothing per frame). The bound
        // is on the loop itself, so a `tick` that never reports rest fails the test rather than
        // hanging it — the failure mode the first draft of this test actually had.
        let mut guard = 64;
        while guard > 0 && tb.tick(1000) {
            guard -= 1;
        }
        assert!(guard > 0, "the transition must terminate, not ask for frames forever");
        assert!(!tb.is_animating(), "a settled toggle must stop asking for frames");
        let settled = rendered_fill(&mut tb);
        assert_ne!(settled, resting, "a hovered toggle must not look like a resting one");
        assert_ne!(settled, mid, "the end state is reached, not abandoned mid-transition");
    }

    /// The fill the control actually paints, read out of the emitted SVG.
    ///
    /// Reading the drawing rather than the progress field is what makes the frame assertions
    /// properties of the picture: a mutation that stopped *using* `interaction_progress` in
    /// `draw` would leave every progress assertion green.
    ///
    /// # Why the *second* filled rect
    ///
    /// The exporter composites a control over the active theme's background, so the first
    /// `fill="rgba(` in the document is the **backdrop**, not the control. Reading it made this
    /// helper answer a constant white for every widget, which would have made the frame assertions
    /// compare a value against itself — the same trap the `group_box` tick test documents.
    fn rendered_fill(tb: &mut ToggleButton) -> (u8, u8, u8) {
        let svg = crate::widget::svg::render_widget_to_svg(tb, Rect::new(0, 0, 100, 30));
        let key = "fill=\"rgba(";
        let backdrop = svg.find(key).expect("a backdrop rect") + key.len();
        let face =
            svg[backdrop..].find(key).expect("the control's own face") + backdrop + key.len();
        let end = svg[face..].find(')').expect("the fill's close") + face;
        let mut parts = svg[face..end].split(',');
        let r = parts.next().and_then(|v| v.trim().parse().ok()).expect("r");
        let g = parts.next().and_then(|v| v.trim().parse().ok()).expect("g");
        let b = parts.next().and_then(|v| v.trim().parse().ok()).expect("b");
        (r, g, b)
    }
}
