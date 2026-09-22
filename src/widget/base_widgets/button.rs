// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Button widget implementation.
use crate::compat::{format, String, ToString};
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
#[cfg(feature = "image")]
use crate::widget::Image;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
/// Button interaction state.
///
/// Derived from the enabled and pressed flags, never stored. Pressed and
/// disabled are exclusive with disabled taking precedence, and hover is not
/// represented at all — see [`Button::is_hovered`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonState {
    /// Enabled and not pressed.
    Normal,
    /// Enabled and currently held down.
    Pressed,
    /// Not enabled; reported even if the pressed flag was set.
    Disabled,
}
/// Button widget for clickable actions.
///
/// # Animated interaction
///
/// The button interpolates its fill between the theme's resting and interactive colours instead of
/// switching instantly. `tick` advances the interpolation by a millisecond delta and reports whether
/// another frame is still needed — the contract this library uses for a self-animating control (see
/// `FloatingLabel`'s `tick`, where the shape was first written): a host that receives `false` stops
/// scheduling frames, so a settled button costs nothing per frame.
///
/// The duration is not a constant in this file. It comes from the active theme's `Motion::normal`,
/// so a theme can state its own tempo and a test can shorten it to reach the end state
/// deterministically.
pub struct Button {
    base: BaseWidget,
    text: String,
    #[cfg(feature = "image")]
    icon: Option<Image>,
    pressed: bool,
    default_button: bool,
    focused: bool,
    hovered: bool,
    /// Progress of the interaction transition: `0.0` at rest, `1.0` fully at the hovered/pressed
    /// fill. Kept as a fraction rather than a colour so the target can change mid-flight — a press
    /// during a hover transition re-aims the same progress instead of restarting from zero, which is
    /// what makes a quick press-and-release read as one movement rather than two fades.
    interaction_progress: f32,
    /// The progress value the transition is travelling toward.
    interaction_target: f32,
    /// Emitted on the rising edge of the pressed flag (button down).
    ///
    /// Suppressed entirely while the button is disabled, so a disabled button
    /// emits no press or release. A repeated press without an intervening
    /// release emits nothing.
    pub pressed_signal: GenericSignal,
    /// Emitted on the falling edge of the pressed flag (button up).
    ///
    /// Like [`Button::pressed_signal`], suppressed while disabled — so disabling
    /// a button that is currently held down leaves the press unreleased rather
    /// than emitting a spurious release.
    pub released_signal: GenericSignal,
    /// Emitted with the recomputed [`ButtonState`] whenever the pressed or
    /// enabled flag changes. Not emitted when only the hover state changes.
    pub state_changed: Signal1<ButtonState>,
}
impl Button {
    /// Creates a button with initial text and geometry.
    pub fn new(text: String, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Button, geometry, "Button"),
            text,
            #[cfg(feature = "image")]
            icon: None,
            pressed: false,
            default_button: false,
            focused: false,
            hovered: false,
            // A freshly constructed button is at rest, so its progress is at the rest end of the
            // interpolation. Starting at the interactive end would make every button fade *out* on
            // its first frame.
            interaction_progress: 0.0,
            interaction_target: 0.0,
            pressed_signal: GenericSignal::new(),
            released_signal: GenericSignal::new(),
            state_changed: Signal1::new(),
        }
    }
    /// Returns button text.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Returns current button interaction state.
    pub fn state(&self) -> ButtonState {
        if !self.base.is_enabled() {
            ButtonState::Disabled
        } else if self.pressed {
            ButtonState::Pressed
        } else {
            ButtonState::Normal
        }
    }
    /// Returns whether button is in pressed state.
    pub fn is_pressed(&self) -> bool {
        self.pressed
    }
    /// Returns whether the pointer is currently over this button.
    ///
    /// Hover is tracked from [`crate::event::Event::MouseEnter`] /
    /// [`crate::event::Event::MouseLeave`], which the widget runtime synthesises as
    /// the pointer moves between controls (no platform backend produces them).
    /// Exposed so a host can style or test the hover state; `ButtonState` cannot
    /// carry it because hover and pressed are independent.
    pub fn is_hovered(&self) -> bool {
        self.hovered
    }
    /// Sets the hovered flag and starts the interaction transition toward it.
    ///
    /// # Why this is public
    ///
    /// The `MouseEnter`/`MouseLeave` arms set the flag from real pointer events, but a host driving
    /// a control from its own input layer — a touch backend that has no hover concept, a test, or a
    /// designer previewing a state — needs a way to say "show me this control hovered". Without it
    /// the hovered appearance was reachable only by synthesising an event, which is a heavier and
    /// less honest way to state the same fact.
    ///
    /// Setting it also requests a redraw, because the flag is now something that changes what is
    /// painted over an animation rather than only at the next event.
    pub fn set_hovered(&mut self, hovered: bool) {
        if self.hovered == hovered {
            return;
        }
        self.hovered = hovered;
        self.base.request_redraw();
    }
    /// Advances the interaction transition by `delta_ms` and reports whether another frame is needed.
    ///
    /// # The contract this implements
    ///
    /// The same one `FloatingLabel::tick` established: return `true` while there is still movement
    /// and `false` once the value has settled, so a host can stop scheduling frames for a control
    /// that is not moving. A `tick` that always returned `true` would keep the whole application
    /// repainting forever, which is why the boolean is part of the signature rather than something
    /// the caller infers.
    ///
    /// # Why the duration is read here rather than stored
    ///
    /// The length of a transition is a theme decision (`Theme::motion`), and the theme can change
    /// while the control exists — a light/dark switch carries a different tempo. Reading it per tick
    /// means a control already in flight finishes at the new tempo instead of the one it started
    /// with. The engine prices at zero is the degenerate case a test uses.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        // The target comes from the control's own state, recomputed every tick so a state change
        // that arrived without a `tick` in between is picked up rather than missed.
        self.interaction_target = self.interaction_target_progress();
        if (self.interaction_progress - self.interaction_target).abs() < f32::EPSILON {
            return false;
        }

        // The transition is driven by the crate's animation engine, not by arithmetic here.
        //
        // `AnimationDriver`/`Animation` carry the iteration counting, the easing curve and the
        // completion callback, and `advance_by` lets a host supply the frame delta instead of the
        // animation reading the wall clock — which is what makes it usable from the
        // `tick(delta_ms) -> bool` convention this library uses for every animated control.
        //
        // A one-shot driver is built per tick deliberately: it is a handful of map entries, the
        // animation is a pure function of accumulated time, and holding one in the widget would
        // require it to survive a theme switch that re-prices the duration mid-flight.
        let duration_ms = crate::style::theme_manager()
            .current_theme()
            .map(|theme| theme.motion.normal)
            .unwrap_or(200)
            .max(1);
        let from = self.interaction_progress;
        let target = self.interaction_target;
        let mut driver = crate::style::AnimationDriver::new();
        // The driver owns its callbacks, so the value it produces has to come back through a shared
        // cell rather than an assignment to a captured local: `move |v| observed = v` would move
        // `observed` into the closure and leave the caller reading the pre-move value, which is how
        // a transition silently never advances.
        let observed = std::rc::Rc::new(core::cell::Cell::new(from));
        let sink = std::rc::Rc::clone(&observed);
        driver.add_float(
            crate::style::AnimationConfig::new(core::time::Duration::from_millis(
                duration_ms as u64,
            )),
            from,
            target,
            move |value| sink.set(value),
        );
        driver.advance_by(core::time::Duration::from_millis(delta_ms as u64));
        let next = observed.get();
        self.interaction_progress =
            if (next - target).abs() < f32::EPSILON { target } else { next };

        // Another frame is owed exactly while the value has not reached its target — the same
        // question `advance_by`'s return value answers about the driver, asked about this control.
        (self.interaction_progress - self.interaction_target).abs() >= f32::EPSILON
    }

    /// The progress the current interaction state calls for.
    ///
    /// A press is the furthest point of the transition, a hover a partial step toward it, and a
    /// disabled control always rest. Disabled wins over pressed because a control that became
    /// disabled mid-press is inert, which is the same precedence `Button::state` uses.
    fn interaction_target_progress(&self) -> f32 {
        if !self.base.is_enabled() {
            return 0.0;
        }
        if self.pressed {
            1.0
        } else if self.hovered {
            0.5
        } else {
            0.0
        }
    }

    /// Sets pressed state and emits transition signals when changed.
    ///
    /// Ignored entirely (no state change, no signals) while the button is
    /// disabled. On a real change it emits `pressed_signal` or
    /// `released_signal` followed by `state_changed`, and requests a redraw.
    pub fn set_pressed(&mut self, pressed: bool) {
        if !self.base.is_enabled() {
            return;
        }
        if self.pressed == pressed {
            return;
        }
        self.pressed = pressed;
        if pressed {
            self.pressed_signal.emit();
        } else {
            self.released_signal.emit();
        }
        self.state_changed.emit(self.state());
        self.base.request_redraw();
    }
    /// Presses the button, via [`Button::set_pressed`] — so the signals and the
    /// disabled check apply. Does not emit a click; the caller decides when a
    /// press-and-release counts as an activation.
    pub fn press(&mut self) {
        self.set_pressed(true);
    }
    /// Releases the button, via [`Button::set_pressed`]. A release without a
    /// preceding press is a no-op because the flag is already clear.
    pub fn release(&mut self) {
        self.set_pressed(false);
    }
    /// Enables/disables button while preserving deterministic state transitions.
    pub fn set_enabled_state(&mut self, enabled: bool) {
        let previous = self.state();
        self.base.set_enabled(enabled);
        if !enabled {
            self.pressed = false;
        }
        let current = self.state();
        if previous != current {
            self.state_changed.emit(current);
            self.base.request_redraw();
        }
    }
    /// Sets button text.
    pub fn set_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.text != text {
            self.text = text;
            self.base.request_redraw();
        }
    }
    /// Sets the icon displayed on the button.
    #[cfg(feature = "image")]
    pub fn set_icon(&mut self, icon: Image) {
        self.icon = Some(icon);
        self.base.request_redraw();
    }
    /// Returns a reference to the button icon, if set.
    #[cfg(feature = "image")]
    pub fn icon(&self) -> Option<&Image> {
        self.icon.as_ref()
    }
    /// Sets whether this button is the default button.
    pub fn set_default(&mut self, default: bool) {
        self.default_button = default;
        self.base.request_redraw();
    }

    /// Returns whether this button is the default button.
    pub fn is_default(&self) -> bool {
        self.default_button
    }

    /// Programmatically clicks the button (press, release, emit clicked signal).
    pub fn click(&mut self) {
        if !self.base.is_enabled() {
            return;
        }
        self.press();
        self.release();
        self.base.clicked.emit();
    }
}
impl Widget for Button {
    /// Resolves the published event names this control emits to their signals.
    ///
    /// # The mapping
    ///
    /// | published name | signal | payload |
    /// |---|---|---|
    /// | `clicked` | `base.clicked` | none |
    /// | `pressed` | `pressed_signal` | none |
    /// | `released` | `released_signal` | none |
    /// | `state_changed` | `state_changed` | `ButtonState` as a token string |
    ///
    /// The names are the ones `button_capability` publishes, so a name `connect_event` accepts is a
    /// name that resolves here — that correspondence is what `tools/check_event_signal_dyn.sh`
    /// verifies for every converted control.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            // `clicked` lives on the base, so every control has it; the `Widget` trait exposes it
            // whether or not the concrete type does anything with it.
            "clicked" => Some(EventSignalRef::unit("clicked", self.clicked_signal())),
            "pressed" => Some(EventSignalRef::unit("pressed", &self.pressed_signal)),
            "released" => Some(EventSignalRef::unit("released", &self.released_signal)),
            // An enum payload travels as its token spelling, which is the same representation the
            // property side uses for `PropertyValueKind::Enum` — one spelling, not two.
            "state_changed" => {
                Some(EventSignalRef::mapped("state_changed", &self.state_changed, |state| {
                    crate::widget::capability::CapabilityValue::String(format!("{state:?}"))
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
        self.set_enabled_state(enabled);
    }

    fn size_hint(&self) -> Size {
        // Approximate: text length * ~8px + padding, minimum 75x28
        let text_w = self.text().len() as u32 * 8 + 20;
        Size::new(text_w.max(75), 28)
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `Button`'s property contract.
///
/// This is the reference implementation for the property layer: a control names
/// its own properties here, reads and writes them against its own fields, and
/// forwards every name it does not recognise to the shared base helpers. Adding a
/// property to a control means editing this block and nothing else — the read
/// path, write path and published names all come from it.
impl WidgetProperties for Button {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "pressed" => Ok(CapabilityValue::Bool(self.is_pressed())),
            "default" => Ok(CapabilityValue::Bool(self.is_default())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(expect_string(value)?);
                Ok(())
            }
            "pressed" => {
                self.set_pressed(expect_bool(value)?);
                Ok(())
            }
            "default" => {
                self.set_default(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `BUTTON_PROPERTIES` for the properties this control owns; the
        // shared four are appended from `BASE_PROPERTY_NAMES` so they are not
        // retyped per control.
        property_names_of!["text", "pressed", "default", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `button` publishes.
    ///
    /// `click` is the action a programmatic invocation means: press, release and
    /// emit `clicked`, the same sequence a pointer activation produces. It is
    /// deliberately *not* just `press()` — a caller asking for a click expects the
    /// resulting signal, and `press()` alone only mutates the visual state and emits
    /// nothing.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "click" => {
                self.press();
                self.release();
                self.base.clicked.emit();
                Ok(())
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for Button {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        match event {
            Event::MousePress { pos: _, button: _ } if self.base.is_enabled() => {
                self.press();
            }
            #[cfg(feature = "touch")]
            Event::TouchBegin { .. } if self.base.is_enabled() => {
                self.press();
            }
            Event::MouseRelease { pos: _, button: _ } if self.pressed => {
                self.release();
                self.base.clicked.emit();
            }
            #[cfg(feature = "touch")]
            Event::TouchEnd { .. } if self.pressed => {
                self.release();
                self.base.clicked.emit();
            }
            #[cfg(feature = "touch")]
            Event::Tap { .. } if self.base.is_enabled() => {
                self.base.clicked.emit();
                self.state_changed.emit(self.state());
            }
            Event::FocusGained => {
                self.focused = true;
                self.base.request_redraw();
            }
            Event::FocusLost => {
                self.focused = false;
                self.base.request_redraw();
            }
            Event::KeyPress { key, .. } if (*key == 13 || *key == 32) && self.base.is_enabled() => {
                self.click();
            }
            Event::MouseEnter { .. } => {
                self.hovered = true;
                self.base.request_redraw();
            }
            Event::MouseLeave { .. } => {
                self.hovered = false;
                // A pointer that leaves while held abandons the press. Only `hovered` used to be
                // cleared, so `pressed` stayed true: the widget then committed on the *next*
                // release, including one for an unrelated interaction, and `draw` kept painting
                // the pressed state for a button the user had already dragged away from.
                self.pressed = false;
                self.base.request_redraw();
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}
impl Draw for Button {
    fn draw(&mut self, context: &mut RenderContext) {
        // Button rendering with style integration (BLUE13 R1.3)
        // Reads style fields (background_color, text_color, border_color, etc.)
        // falling back to state-based defaults when style values are not set.
        let rect = self.geometry();
        let state = self.state();
        let style = self.style();

        // ── Background ──
        let bg = style.background_color.unwrap_or_else(|| match state {
            ButtonState::Normal => Color::rgb(240, 240, 240),
            ButtonState::Pressed => Color::rgb(200, 200, 200),
            ButtonState::Disabled => Color::rgb(220, 220, 220),
        });
        // The fill blends from the resting colour toward the interactive one by the transition's own
        // progress, so a hover fades in and a press deepens it rather than both snapping. The
        // resting and pressed colours are the ones this control already used; only the *path*
        // between them is new.
        //
        // At progress `0.0` — a fresh control, or one whose transition has settled at rest — the
        // result is exactly the resting colour, so a snapshot taken without ticking is unchanged.
        let bg = if self.interaction_progress <= 0.0 {
            bg
        } else {
            let interactive = match state {
                ButtonState::Pressed => bg,
                _ => bg.blend(&bg.contrast_color(), 0.22),
            };
            bg.blend(&interactive, self.interaction_progress)
        };
        let br = style.border_radius.unwrap_or(0);
        if br > 0 {
            context.fill_rounded_rect(rect, br, bg);
        } else {
            context.fill_rect(rect, bg);
        }

        // ── Border ──
        if let Some(border_color) = style.border_color {
            let bw = style.border_width.unwrap_or(0);
            if br > 0 && bw > 0 {
                context.draw_rounded_rect_stroke(rect, br, border_color, bw);
            } else if bw > 0 {
                context.draw_rect_stroke(rect, border_color, bw);
            }
        }

        // ── Text ──
        if !self.text.is_empty() {
            let default_font = Font::default();
            let font = style.font.as_ref().unwrap_or(&default_font);
            // The ink must be legible *on the fill this button actually got*, not on an
            // assumption about it. The fill comes from `style.background_color`, which the
            // theme may have set to a saturated accent: the dark appearance's `PRIMARY` is
            // `rgb(100,181,246)`, and the old rule drew pure black on it — chosen from
            // `state == Disabled` alone, with no reference to the background at all, so
            // `button.svg` carried a black label on a light-blue pill. Deriving the ink from
            // the resolved fill is what makes "one foreground per background" true for any
            // theme, which is the same rule `badge` and `role_colors` already use.
            let fill = style.background_color.unwrap_or(Color::WHITE);
            let text_color = style.text_color.unwrap_or_else(|| {
                if state == ButtonState::Disabled {
                    // A disabled button's ink is *deliberately* low-contrast: that is what
                    // makes it read as unavailable. It is still derived from the fill's own
                    // luminance so it recedes in the dark appearance too instead of
                    // disappearing into it.
                    fill.contrast_color().with_alpha(150)
                } else {
                    fill.contrast_color()
                }
            });
            // Vertically centred through the shared primitive, so the label sits in the
            // button's middle instead of having its glyph-box top edge on that middle line.
            let line = context.text_line(rect, font);
            context.draw_text(
                Point { x: rect.x + 6, y: line.y },
                &self.text,
                font,
                text_color,
                HorizontalAlignment::Left,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Color, Point, Rect, Size};
    use crate::event::Event;
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    use crate::widget::Image;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    // ── Helper ─────────────────────────────────────────────────────────
    fn make_button() -> Button {
        Button::new("Click".into(), Rect::new(10, 20, 120, 36))
    }

    #[cfg(all(feature = "image", not(alloc_frugal)))]
    fn make_image() -> Image {
        Image::from_rgba(vec![0u8; 8 * 8 * 4], 8, 8)
    }

    fn rect() -> Rect {
        Rect::new(10, 20, 120, 36)
    }

    // ── Property contract (BLUE15 C-1) ─────────────────────────────────

    /// `Button`'s own properties must round-trip through the new contract.
    ///
    /// This is the reference test for every other control's migration: it proves
    /// the read path, the write path and the published name list describe the same
    /// properties, which the centralised `match kind()` dispatch could not
    /// guarantee (a name could be writable and unreadable, or listed and absent).
    #[test]
    fn widget_properties_round_trip() {
        let mut button = make_button();

        assert_eq!(button.get("text"), Ok(CapabilityValue::String("Click".into())));
        button.set("text", CapabilityValue::String("Go".into())).expect("text is writable");
        assert_eq!(button.get("text"), Ok(CapabilityValue::String("Go".into())));

        assert_eq!(button.get("pressed"), Ok(CapabilityValue::Bool(false)));
        button.set("pressed", CapabilityValue::Bool(true)).expect("pressed is writable");
        assert_eq!(button.get("pressed"), Ok(CapabilityValue::Bool(true)));

        button.set("default", CapabilityValue::Bool(true)).expect("default is writable");
        assert_eq!(button.get("default"), Ok(CapabilityValue::Bool(true)));
    }

    /// The shared properties must work through `Button` too, not only through the
    /// base helpers directly — that forwarding is what a migration can silently
    /// break.
    #[test]
    fn shared_properties_are_forwarded_to_the_base_contract() {
        let mut button = make_button();

        assert_eq!(button.get("enabled"), Ok(CapabilityValue::Bool(true)));
        button.set("enabled", CapabilityValue::Bool(false)).expect("enabled is writable");
        assert!(!button.is_enabled(), "the base state must actually change");

        assert_eq!(button.get("geometry"), Ok(CapabilityValue::String("10,20,120,36".into())));
    }

    /// Every name `Button` publishes must be readable, and its own names must all
    /// be present — a `property_names` that omits a property makes it invisible to
    /// schema consumers even though `get` would answer.
    #[test]
    fn published_names_match_the_contract() {
        let button = make_button();
        let names = button.property_names();

        for required in ["text", "pressed", "default"] {
            assert!(names.contains(&required), "property_names must publish {required:?}");
            assert!(button.get(required).is_ok(), "published {required:?} must be readable");
        }
    }

    // ── 1. Button creation ─────────────────────────────────────────────
    #[cfg(feature = "image")]
    #[test]
    fn button_creation_text_geometry_defaults_icon() {
        let b = make_button();
        assert_eq!(b.text(), "Click");
        assert_eq!(b.geometry(), rect());
        assert_eq!(b.state(), ButtonState::Normal);
        assert!(!b.is_pressed());
        assert!(!b.is_default());
        assert!(b.icon().is_none());
    }

    // ── 2. State transitions ───────────────────────────────────────────
    #[test]
    fn state_transition_normal_pressed_released() {
        let mut b = make_button();
        assert_eq!(b.state(), ButtonState::Normal);

        b.press();
        assert!(b.is_pressed());
        assert_eq!(b.state(), ButtonState::Pressed);

        b.release();
        assert!(!b.is_pressed());
        assert_eq!(b.state(), ButtonState::Normal);
    }

    #[test]
    fn state_transition_idempotent_press_release_noop() {
        let mut b = make_button();
        // Already Normal → release is a no-op
        b.release();
        assert_eq!(b.state(), ButtonState::Normal);
        assert!(!b.is_pressed());

        // Press once
        b.press();
        // Second press is a no-op
        b.press();
        assert!(b.is_pressed());
        assert_eq!(b.state(), ButtonState::Pressed);

        // Release once
        b.release();
        // Second release is a no-op
        b.release();
        assert!(!b.is_pressed());
        assert_eq!(b.state(), ButtonState::Normal);
    }

    // ── 3. Signal emission ─────────────────────────────────────────────
    #[test]
    fn signal_press_emits_pressed_and_state_changed() {
        let mut b = make_button();
        let pressed_fired = Arc::new(AtomicBool::new(false));
        let changed_fired = Arc::new(AtomicBool::new(false));
        b.pressed_signal.connect({
            let flag = Arc::clone(&pressed_fired);
            move || {
                flag.store(true, Ordering::SeqCst);
            }
        });
        b.state_changed.connect({
            let flag = Arc::clone(&changed_fired);
            move |_| {
                flag.store(true, Ordering::SeqCst);
            }
        });

        b.press();

        assert!(pressed_fired.load(Ordering::SeqCst));
        assert!(changed_fired.load(Ordering::SeqCst));
    }

    #[test]
    fn signal_release_emits_released_and_state_changed() {
        let mut b = make_button();
        let released_fired = Arc::new(AtomicBool::new(false));
        let changed_fired = Arc::new(AtomicBool::new(false));
        b.released_signal.connect({
            let flag = Arc::clone(&released_fired);
            move || {
                flag.store(true, Ordering::SeqCst);
            }
        });
        b.state_changed.connect({
            let flag = Arc::clone(&changed_fired);
            move |_| {
                flag.store(true, Ordering::SeqCst);
            }
        });

        b.press(); // Press first
        released_fired.store(false, Ordering::SeqCst);
        changed_fired.store(false, Ordering::SeqCst);

        b.release(); // Then release

        assert!(released_fired.load(Ordering::SeqCst));
        assert!(changed_fired.load(Ordering::SeqCst));
    }

    #[test]
    fn set_enabled_state_requests_redraw() {
        let mut b = make_button();
        let fired = Arc::new(AtomicBool::new(false));
        b.base.redraw_requested.connect({
            let flag = Arc::clone(&fired);
            move || {
                flag.store(true, Ordering::SeqCst);
            }
        });

        b.set_enabled_state(false);

        assert!(fired.load(Ordering::SeqCst));
    }

    #[test]
    fn signal_no_emission_on_noop_transition() {
        let mut b = make_button();
        let fired = Arc::new(AtomicBool::new(false));
        b.pressed_signal.connect({
            let flag = Arc::clone(&fired);
            move || {
                flag.store(true, Ordering::SeqCst);
            }
        });
        b.released_signal.connect({
            let flag = Arc::clone(&fired);
            move || {
                flag.store(true, Ordering::SeqCst);
            }
        });
        b.state_changed.connect({
            let flag = Arc::clone(&fired);
            move |_| {
                flag.store(true, Ordering::SeqCst);
            }
        });

        // No-op: Normal → Normal
        b.release();
        assert!(!fired.load(Ordering::SeqCst));

        // Press → Pressed
        b.press();
        fired.store(false, Ordering::SeqCst);

        // No-op: Pressed → Pressed
        b.press();
        assert!(!fired.load(Ordering::SeqCst));
    }

    // ── 4. Disabled state ──────────────────────────────────────────────
    #[test]
    fn disabled_prevents_transitions() {
        let mut b = make_button();
        b.set_enabled_state(false);
        assert_eq!(b.state(), ButtonState::Disabled);

        // Press is ignored when disabled
        b.press();
        assert_eq!(b.state(), ButtonState::Disabled);
        assert!(!b.is_pressed());

        // Release is ignored when disabled
        b.release();
        assert_eq!(b.state(), ButtonState::Disabled);
    }

    #[test]
    fn re_enable_restores_normal_state() {
        let mut b = make_button();
        b.set_enabled_state(false);
        assert_eq!(b.state(), ButtonState::Disabled);

        b.set_enabled_state(true);
        assert_eq!(b.state(), ButtonState::Normal);
        assert!(!b.is_pressed());
    }

    // ── 5. Text update and icon ────────────────────────────────────────
    #[test]
    fn set_text_updates_text() {
        let mut b = make_button();
        assert_eq!(b.text(), "Click");

        b.set_text("OK");
        assert_eq!(b.text(), "OK");
    }

    #[test]
    fn set_empty_text() {
        let mut b = make_button();
        b.set_text("");
        assert_eq!(b.text(), "");
        assert!(b.text().is_empty());
    }

    #[cfg(all(feature = "image", not(alloc_frugal)))]
    #[test]
    fn set_icon_and_default_icon() {
        let mut b = make_button();
        assert!(b.icon().is_none());

        let img = make_image();
        b.set_icon(img);
        assert!(b.icon().is_some());
    }

    // ── 6. Default property ────────────────────────────────────────────
    #[test]
    fn default_property() {
        let mut b = make_button();
        assert!(!b.is_default());

        b.set_default(true);
        assert!(b.is_default());

        b.set_default(false);
        assert!(!b.is_default());
    }

    // ── 7. Event handling ──────────────────────────────────────────────
    #[test]
    fn focus_gained_sets_focused_flag() {
        let mut btn = make_button();
        assert!(!btn.focused, "Button should not be focused by default");
        btn.handle_event(&Event::FocusGained);
        assert!(btn.focused, "Button should be focused after FocusGained");
    }

    #[test]
    fn focus_lost_clears_focused_flag() {
        let mut btn = make_button();
        btn.handle_event(&Event::FocusGained);
        assert!(btn.focused);
        btn.handle_event(&Event::FocusLost);
        assert!(!btn.focused, "Button should not be focused after FocusLost");
    }

    #[test]
    fn key_press_enter_triggers_click() {
        let mut btn = make_button();
        let clicked = Arc::new(AtomicBool::new(false));
        btn.base.clicked.connect({
            let flag = Arc::clone(&clicked);
            move || {
                flag.store(true, Ordering::SeqCst);
            }
        });
        btn.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
        assert!(clicked.load(Ordering::SeqCst), "Enter key should trigger click");
    }

    #[test]
    fn key_press_space_triggers_click() {
        let mut btn = make_button();
        let clicked = Arc::new(AtomicBool::new(false));
        btn.base.clicked.connect({
            let flag = Arc::clone(&clicked);
            move || {
                flag.store(true, Ordering::SeqCst);
            }
        });
        btn.handle_event(&Event::KeyPress { key: 32, modifiers: 0 });
        assert!(clicked.load(Ordering::SeqCst), "Space key should trigger click");
    }

    #[test]
    fn key_press_other_key_does_not_trigger_click() {
        let mut btn = make_button();
        let clicked = Arc::new(AtomicBool::new(false));
        btn.base.clicked.connect({
            let flag = Arc::clone(&clicked);
            move || {
                flag.store(true, Ordering::SeqCst);
            }
        });
        btn.handle_event(&Event::KeyPress { key: 65, modifiers: 0 }); // 'A'
        assert!(!clicked.load(Ordering::SeqCst), "Non-Enter/Space key should NOT trigger click");
    }

    #[test]
    fn mouse_enter_sets_hovered() {
        let mut btn = make_button();
        assert!(!btn.hovered);
        btn.handle_event(&Event::MouseEnter { pos: Point::new(10, 10) });
        assert!(btn.hovered);
    }

    #[test]
    fn mouse_leave_clears_hovered() {
        let mut btn = make_button();
        btn.handle_event(&Event::MouseEnter { pos: Point::new(10, 10) });
        assert!(btn.hovered);
        btn.handle_event(&Event::MouseLeave { pos: Point::new(10, 10) });
        assert!(!btn.hovered);
    }

    #[test]
    fn event_mouse_down_presses_button() {
        let mut b = make_button();
        let event = Event::MousePress { pos: Point::new(15, 25), button: 0 };
        b.handle_event(&event);
        assert!(b.is_pressed());
        assert_eq!(b.state(), ButtonState::Pressed);
    }

    #[test]
    fn event_mouse_up_releases_and_clicks() {
        let mut b = make_button();
        let clicked = Arc::new(AtomicBool::new(false));
        b.base.clicked.connect({
            let flag = Arc::clone(&clicked);
            move || {
                flag.store(true, Ordering::SeqCst);
            }
        });

        // Must be pressed first
        b.press();
        assert!(b.is_pressed());

        let event = Event::MouseRelease { pos: Point::new(15, 25), button: 0 };
        b.handle_event(&event);
        assert!(!b.is_pressed());
        assert_eq!(b.state(), ButtonState::Normal);
        assert!(clicked.load(Ordering::SeqCst));
    }

    #[cfg(feature = "touch")]
    #[test]
    fn event_touch_begin_presses() {
        let mut b = make_button();
        let event = Event::TouchBegin { pos: Point::new(15, 25), touch_id: 0 };
        b.handle_event(&event);
        assert!(b.is_pressed());
        assert_eq!(b.state(), ButtonState::Pressed);
    }

    #[cfg(feature = "touch")]
    #[test]
    fn event_touch_end_releases_and_clicks() {
        let mut b = make_button();
        let clicked = Arc::new(AtomicBool::new(false));
        b.base.clicked.connect({
            let flag = Arc::clone(&clicked);
            move || {
                flag.store(true, Ordering::SeqCst);
            }
        });

        b.press();
        let event = Event::TouchEnd { pos: Point::new(15, 25), touch_id: 0 };
        b.handle_event(&event);
        assert!(!b.is_pressed());
        assert_eq!(b.state(), ButtonState::Normal);
        assert!(clicked.load(Ordering::SeqCst));
    }

    #[cfg(feature = "touch")]
    #[test]
    fn event_tap_triggers_click() {
        let mut b = make_button();
        let clicked = Arc::new(AtomicBool::new(false));
        b.base.clicked.connect({
            let flag = Arc::clone(&clicked);
            move || {
                flag.store(true, Ordering::SeqCst);
            }
        });

        let event = Event::Tap { pos: Point::new(15, 25) };
        b.handle_event(&event);
        assert!(clicked.load(Ordering::SeqCst));
        // Tap does not change pressed state
        assert!(!b.is_pressed());
    }

    #[test]
    fn event_disabled_ignores_mouse_down() {
        let mut b = make_button();
        b.set_enabled_state(false);
        let event = Event::MouseDown((Point::new(15, 25), 0));
        b.handle_event(&event);
        assert!(!b.is_pressed());
        assert_eq!(b.state(), ButtonState::Disabled);
    }

    #[test]
    fn event_disabled_ignores_mouse_up() {
        let mut b = make_button();
        b.set_enabled_state(false);
        // Even if somehow pressed, disabled handle_event should not process MouseUp
        let event = Event::MouseUp((Point::new(15, 25), 0));
        b.handle_event(&event);
        assert!(!b.is_pressed());
    }

    #[cfg(feature = "touch")]
    #[test]
    fn event_disabled_ignores_tap() {
        let mut b = make_button();
        b.set_enabled_state(false);
        let clicked = Arc::new(AtomicBool::new(false));
        b.base.clicked.connect({
            let flag = Arc::clone(&clicked);
            move || {
                flag.store(true, Ordering::SeqCst);
            }
        });
        let event = Event::Tap { pos: Point::new(15, 25) };
        b.handle_event(&event);
        assert!(!clicked.load(Ordering::SeqCst));
    }

    #[cfg(feature = "touch")]
    #[test]
    fn event_disabled_ignores_touch_begin() {
        let mut b = make_button();
        b.set_enabled_state(false);
        let event = Event::TouchBegin { pos: Point::new(15, 25), touch_id: 0 };
        b.handle_event(&event);
        assert!(!b.is_pressed());
    }

    #[cfg(feature = "touch")]
    #[test]
    fn event_disabled_ignores_touch_end() {
        let mut b = make_button();
        b.set_enabled_state(false);
        let event = Event::TouchEnd { pos: Point::new(15, 25), touch_id: 0 };
        b.handle_event(&event);
        assert!(!b.is_pressed());
    }

    // ── 8. Widget trait delegation ─────────────────────────────────────
    #[test]
    fn widget_trait_id_and_kind() {
        let b = make_button();
        assert!(b.id() != 0);
        assert_eq!(b.kind(), WidgetKind::Button);
    }

    #[test]
    fn widget_trait_geometry() {
        let mut b = make_button();
        assert_eq!(b.geometry(), rect());
        let new_rect = Rect::new(0, 0, 200, 50);
        b.set_geometry(new_rect);
        assert_eq!(b.geometry(), new_rect);
    }

    #[test]
    fn widget_trait_visibility() {
        let mut b = make_button();
        assert!(b.is_visible());
        b.hide();
        assert!(!b.is_visible());
        b.show();
        assert!(b.is_visible());
    }

    #[test]
    fn widget_trait_enabled() {
        let mut b = make_button();
        assert!(b.is_enabled());
        b.set_enabled(false);
        assert!(!b.is_enabled());
        assert_eq!(b.state(), ButtonState::Disabled);
        b.set_enabled(true);
        assert!(b.is_enabled());
        assert_eq!(b.state(), ButtonState::Normal);
    }

    #[test]
    fn widget_trait_parent_and_children() {
        let mut b = make_button();
        assert!(b.parent().is_none());
        assert!(b.children().is_empty());

        let child: crate::core::ObjectId = 42;
        let parent: crate::core::ObjectId = 99;

        b.set_parent(Some(parent));
        assert_eq!(b.parent(), Some(parent));

        b.add_child(child);
        assert_eq!(b.children(), &[child]);

        b.remove_child(child);
        assert!(b.children().is_empty());
    }

    #[test]
    fn widget_trait_min_max_size() {
        let mut b = make_button();
        assert!(b.min_size().is_none());
        assert!(b.max_size().is_none());

        b.set_min_size(Some(Size::new(80, 24)));
        assert_eq!(b.min_size(), Some(Size::new(80, 24)));

        b.set_max_size(Some(Size::new(400, 200)));
        assert_eq!(b.max_size(), Some(Size::new(400, 200)));
    }

    #[test]
    fn widget_trait_tooltip() {
        let mut b = make_button();
        assert_eq!(b.tooltip(), "");
        b.set_tooltip("Save".into());
        assert_eq!(b.tooltip(), "Save");
    }

    #[test]
    fn widget_trait_style() {
        let mut b = make_button();
        let default_style = b.style().clone();
        // Verify we can set a modified style via builder pattern.
        let new_style = default_style.clone().with_background(Color::rgb(255, 0, 0));
        b.set_style(new_style.clone());
        assert_eq!(b.style().background_color, new_style.background_color);
    }

    #[test]
    fn widget_trait_signals() {
        let b = make_button();
        // Verify signals are accessible via Widget trait
        let _ = b.hover_signal();
        let _ = b.mouse_down_signal();
        let _ = b.mouse_up_signal();
        let _ = b.key_down_signal();
        let _ = b.key_up_signal();
        let _ = b.focus_gained_signal();
        let _ = b.focus_lost_signal();
        let _ = b.redraw_requested_signal();
        let _ = b.layout_requested_signal();
    }

    #[test]
    fn widget_trait_connection_scope() {
        let b = make_button();
        let _scope = b.connection_scope();
    }

    // ── 11. Visibility toggling ────────────────────────────────────────
    #[test]
    fn test_button_visibility_toggle() {
        let mut b = make_button();
        assert!(b.is_visible());

        // Single hide
        b.hide();
        assert!(!b.is_visible());

        // Double hide is idempotent
        b.hide();
        assert!(!b.is_visible());

        // Show restores
        b.show();
        assert!(b.is_visible());

        // Double show is idempotent
        b.show();
        assert!(b.is_visible());

        // Multiple toggle cycles
        for _ in 0..3 {
            b.hide();
            assert!(!b.is_visible());
            b.show();
            assert!(b.is_visible());
        }
    }

    // ── 12. Animated interaction (BLUE21 P0-4) ─────────────────────────

    /// The transition advances toward the pressed state and then **stops**.
    ///
    /// The boolean return is the load-bearing part: a `tick` that kept reporting `true` after the
    /// value had settled would keep the host scheduling frames forever. The engine existed and
    /// nothing called it, so this is the first control to prove the contract end to end.
    #[test]
    fn the_interaction_transition_settles_and_reports_so() {
        let mut b = make_button();
        b.set_hovered(true);

        // A hover moves the fill, so at least one tick must report more work to do.
        assert!(
            b.tick(30),
            "a hover must start a transition, so the first tick reports another frame"
        );

        // Ticking well past the theme's 200 ms settles it exactly, and then reports completion.
        let mut frames = 1;
        while b.tick(1000) {
            frames += 1;
            assert!(frames < 100, "the transition must terminate rather than tick forever");
        }
        assert_eq!(b.interaction_progress, 0.5, "a hover settles exactly on its target");

        // Settled means settled: **each** further tick reports no work. Asserted in a loop rather
        // than once, because a single `!tick()` would also pass for an implementation that
        // alternated between reporting work and not — the defect this pins is "keeps asking for
        // frames after the value has stopped changing", and only repetition can see it.
        for frame in 0..5 {
            assert!(
                !b.tick(16),
                "a settled transition must report that no frame is needed (tick {frame})"
            );
            assert_eq!(b.interaction_progress, 0.5, "and must not drift while settled");
        }
    }

    /// A state change that arrives mid-flight re-aims the same progress rather than restarting.
    #[test]
    fn a_press_during_a_hover_re_aims_rather_than_restarting() {
        let mut b = make_button();
        b.set_hovered(true);
        b.tick(50);
        let partway = b.interaction_progress;
        assert!(partway > 0.0 && partway < 0.5, "the hover is in flight: {partway}");

        b.set_pressed(true);
        assert!(b.tick(1), "the press must continue the movement, not settle on the first frame");
        assert!(
            b.interaction_progress > partway,
            "a press must move the progress onward from where the hover left it, not back to zero"
        );
    }

    /// A disabled control rests, whatever its press flags say.
    #[test]
    fn a_disabled_button_rests_regardless_of_its_flags() {
        let mut b = make_button();
        b.set_enabled(false);
        assert_eq!(b.interaction_target_progress(), 0.0);
        assert!(
            !b.tick(1000),
            "a disabled button has nothing to animate toward, so no frame is owed"
        );
    }

    /// The transition is visible: a fully-progressed hover paints a different fill.
    #[test]
    fn the_progress_changes_what_is_painted() {
        let rect = Rect::new(0, 0, 120, 32);
        let mut resting = Button::new("Go".to_string(), rect);
        let rest_svg = crate::widget::svg::render_to_svg(&mut resting);

        let mut hovered = Button::new("Go".to_string(), rect);
        hovered.set_hovered(true);
        while hovered.tick(1000) {}
        let hover_svg = crate::widget::svg::render_to_svg(&mut hovered);

        assert_ne!(
            rest_svg, hover_svg,
            "a completed hover transition must change the rendered fill"
        );
    }

    #[test]
    fn test_button_zero_geometry() {
        // Zero width/height — should not panic
        let _b = Button::new("Zero".into(), Rect::new(0, 0, 0, 0));

        // Negative position — should not panic
        let _b = Button::new("Neg".into(), Rect::new(-10, -20, 100, 30));

        // Zero geometry with negative position — should not panic
        let _b = Button::new("All".into(), Rect::new(-5, -5, 0, 0));
    }

    #[test]
    fn test_button_signal_not_emitted_on_same_value() {
        let mut b = make_button();
        let changed_count = Arc::new(AtomicBool::new(false));
        let c = Arc::clone(&changed_count);
        b.base.changed.connect(move || {
            c.store(true, Ordering::SeqCst);
        });

        // set_text does NOT emit base.changed — it only calls request_redraw()
        b.set_text("Click");
        assert!(
            !changed_count.load(Ordering::SeqCst),
            "set_text with same value should not emit changed"
        );

        // Even setting different text does not emit base.changed
        b.set_text("Different");
        assert!(
            !changed_count.load(Ordering::SeqCst),
            "set_text with different value should not emit changed"
        );
    }
}
