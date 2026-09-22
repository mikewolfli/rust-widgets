// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Button widget implementation.
use crate::compat::{format, String, ToString};
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler, FocusReason};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::style::EdgeOffsets;
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{
    dimensions, focus_ring_color, ControlMetrics, FocusRing, FOCUS_RING_WIDTH,
};
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
    /// Whether this control owns the *gesture*: set by a press that landed on it and
    /// cleared by the release that ends it, regardless of where the pointer is.
    ///
    /// # Why `pressed` alone is not enough
    ///
    /// `pressed` answers "should I paint pressed", and it follows the pointer in and
    /// out of the control. Using it to decide whether an incoming release *belongs* to
    /// this control conflates two facts: a release that arrives while `pressed` is
    /// false — because the pointer wandered off and was not dragged back — would be
    /// discarded without ending the gesture, leaving the control armed forever. And a
    /// move could never restore `pressed`, because the arm was guarded on it.
    ///
    /// Qt Quick separates the pair as `pressed` and the grab (`explicitDown`,
    /// `qquickabstractbutton_p.h:31-32`; grab taken in `handlePress`, released in
    /// `handleRelease`/`handleUngrab`). This is that split.
    grabbed: bool,
    default_button: bool,
    /// Whether this control currently owns keyboard focus.
    focused: bool,
    /// Why it got focus, which decides whether a focus ring is painted.
    ///
    /// Kept alongside `focused` rather than folded into it because the two answer
    /// different questions: `focused` is "am I the keyboard target", this is "is the
    /// user on the keyboard". Qt Quick spells the pair `activeFocus` / `visualFocus`.
    focus_reason: FocusReason,
    hovered: bool,
    /// The interaction transition: `0.0` at rest, `1.0` fully at the hovered/pressed fill.
    ///
    /// Kept as a fraction rather than a colour so the target can change mid-flight — a press
    /// during a hover transition re-aims the same progress instead of restarting from zero, which is
    /// what makes a quick press-and-release read as one movement rather than two fades. The shared
    /// [`crate::style::Transition`] owns the interpolation and the theme-derived duration.
    interaction_progress: crate::style::Transition,
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
    /// Emitted when a press is abandoned rather than completed.
    ///
    /// # Why this is not folded into `released`
    ///
    /// `released` answers "the pointer came up", which is true for a drag that started
    /// on the button and ended off it. `canceled` answers "this gesture will not
    /// activate", which is the fact a caller routing a destructive action needs. A
    /// button that reported both as `released` would leave the caller unable to tell a
    /// completed activation from an abandoned one — the distinction Qt Quick draws by
    /// emitting `canceled` instead of `clicked` (`qquickabstractbutton.cpp:198-204`).
    pub canceled: GenericSignal,
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
            grabbed: false,
            default_button: false,
            focused: false,
            // No focus yet, so the reason is never read. `Programmatic` is the
            // variant that claims the least: it does not assert a gesture happened.
            focus_reason: FocusReason::Programmatic,
            hovered: false,
            // A freshly constructed button is at rest, so its progress is at the rest end of the
            // interpolation. Starting at the interactive end would make every button fade *out* on
            // its first frame. `Normal` is the tempo a state change takes.
            interaction_progress: crate::style::Transition::new(),
            interaction_target: 0.0,
            pressed_signal: GenericSignal::new(),
            released_signal: GenericSignal::new(),
            state_changed: Signal1::new(),
            canceled: GenericSignal::new(),
        }
    }
    /// Returns button text.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Returns whether this button currently owns keyboard focus.
    pub fn has_focus(&self) -> bool {
        self.focused
    }

    /// Returns whether a focus ring should be painted right now.
    ///
    /// The single question every draw site asks: `focused && reason.draws_focus_ring()`
    /// — the Qt Quick rule — evaluated in one place so a control cannot accidentally
    /// implement "has focus" as "draw the ring".
    pub fn visual_focus(&self) -> bool {
        self.focused && self.focus_reason.draws_focus_ring()
    }

    /// Returns whether this button is the keyboard's current target, regardless of
    /// whether a ring is drawn for it.
    pub fn is_focused(&self) -> bool {
        self.focused
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
    /// # Why the work is delegated to `Transition`
    ///
    /// The interpolation, the iteration counting and the engine's callback ownership are the
    /// same for every control that animates between two appearances. Keeping a private copy here
    /// meant the duration was read from `crate::theme` directly, which only exists in a build with
    /// a device profile — so this file failed to compile under `mini`/`embedded` until the read was
    /// routed through the `style` facade. `Transition` owns that read once.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        // The target is recomputed every tick from the control's own state, so a state change
        // that arrived without a `tick` in between is picked up rather than missed.
        let target = self.interaction_target_progress();
        let moving = self.interaction_progress.tick(target, delta_ms);
        self.interaction_target = target;
        moving
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
    /// Begins a gesture: takes the grab and marks the control pressed.
    ///
    /// The grab is taken **before** the pressed flag so that `set_pressed(false)`
    /// during an ungrab cannot re-enter and drop a grab that was never taken.
    pub fn press(&mut self) {
        if !self.base.is_enabled() {
            return;
        }
        self.grabbed = true;
        self.set_pressed(true);
    }
    /// Ends a gesture: drops the grab and clears the pressed flag.
    pub fn release(&mut self) {
        self.grabbed = false;
        self.set_pressed(false);
    }

    /// Abandons a gesture without activating: ends it and emits `canceled` if it was live.
    ///
    /// This is Qt Quick's `handleUngrab`, and it is the single place that decides what
    /// "the interaction is off" means. Called from a focus change and from disabling the
    /// control — so the paths cannot drift into slightly different notions of cancellation.
    pub fn cancel_gesture(&mut self) {
        if !self.grabbed && !self.pressed {
            return;
        }
        self.grabbed = false;
        self.set_pressed(false);
        self.canceled.emit();
    }

    /// Whether this control currently owns the pointer gesture.
    pub fn is_grabbed(&self) -> bool {
        self.grabbed
    }
    /// Enables/disables button while preserving deterministic state transitions.
    ///
    /// Disabling mid-gesture abandons it, emitting `canceled`: a control that became
    /// inert while held will never receive the release that would have ended the
    /// gesture normally, so leaving the grab set would keep a stale `pressed` paint and
    /// an arm that fires on an unrelated later release. Qt Quick clears the transient
    /// state at the same moment (`button_style_button.dart:359-362`).
    pub fn set_enabled_state(&mut self, enabled: bool) {
        let previous = self.state();
        self.base.set_enabled(enabled);
        if !enabled {
            self.cancel_gesture();
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
        // The button's intrinsic size is the QML formula — `max(floor, content + padding)` —
        // expressed through the shared primitive rather than as a hand-written `max`.
        //
        // `BUTTON_MIN` (64x40) is the *floor*, and it is the load-bearing term: a button
        // labelled with two characters must still be big enough to press. The label estimate is
        // the crate's usual `len * 8`, used everywhere text is measured for sizing; the icon's
        // own box is added when one is present.
        let label_width = self.text().len() as u32 * 8;
        // The icon's own box, when there is one, is part of the content — so it is added to
        // the label rather than to the padding, which is what keeps the floor meaningful.
        // Written as a conditional expression rather than a `mut` binding followed by an
        // assignment because the `image` feature is optional: a `mut` that is only needed in
        // one feature configuration is an `unused_mut` warning in the others.
        #[cfg(feature = "image")]
        let content = Size::new(
            label_width
                + if self.icon.is_some() {
                    dimensions::BUTTON_ICON_SIZE + dimensions::BUTTON_ICON_SPACING
                } else {
                    0
                },
            dimensions::FONT_SIZE_BASE + 4,
        );
        #[cfg(not(feature = "image"))]
        let content = Size::new(label_width, dimensions::FONT_SIZE_BASE + 4);
        ControlMetrics::implicit_size(
            content,
            EdgeOffsets::symmetric(dimensions::BUTTON_PADDING_V, dimensions::BUTTON_PADDING_H),
            dimensions::BUTTON_MIN,
        )
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
    /// The four-segment activation contract.
    ///
    /// # Why `released` and `clicked` are not the same event
    ///
    /// A pointer that goes down on a button and comes up away from it is a *canceled*
    /// gesture: the user changed their mind or was dragging something. Qt Quick encodes
    /// this in `QQuickAbstractButtonPrivate::handleRelease` (`qquickabstractbutton.cpp:198-204`),
    /// which emits `clicked` only when the release lands **inside** the control and
    /// `canceled` otherwise.
    ///
    /// This handler used to treat every release as an activation, so dragging off a
    /// button and letting go still fired it — including for a drag that started
    /// elsewhere and merely ended here.
    ///
    /// | event | result |
    /// |---|---|
    /// | press | `pressed` armed, `pressed` signal emitted |
    /// | move | `pressed` follows whether the pointer is still inside |
    /// | release inside | release, `clicked` |
    /// | release outside | release, `canceled`, **no** `clicked` |
    /// | ungrab (leave / focus loss / disable) | release, `canceled` |
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        match event {
            Event::MousePress { pos, button } if self.base.is_enabled() => {
                // Only a press that resolves to this control arms it. The runtime
                // hit-tests before delivery, but a direct dispatch (a test, a host with
                // its own routing, a designer preview) does not, and an unguarded press
                // would leave the latch armed for a release that belongs elsewhere.
                if button_activates(*button) && self.base.contains_point_with_touch_expansion(*pos)
                {
                    self.press();
                }
            }
            #[cfg(feature = "touch")]
            Event::TouchBegin { .. } if self.base.is_enabled() => {
                self.press();
            }
            // `pressed` is a *continuous* quantity, not an edge: dragging off the
            // control clears it and dragging back restores it, so the button visually
            // commits again. Qt Quick does this in `handleMove`
            // (`qquickabstractbutton.cpp:179`). Without it, a press that wandered off
            // left the button painted pressed forever.
            //
            // The arm is guarded on the **grab**, not on `pressed`: guarding on
            // `pressed` made the move that should restore the state exit early, so a
            // drag out and back could never re-arm.
            Event::MouseMove { pos } | Event::PointerMove { pos, .. } if self.grabbed => {
                self.set_pressed(self.base.contains_point_with_touch_expansion(*pos));
            }
            Event::MouseRelease { pos, button } if self.grabbed => {
                if !button_activates(*button) {
                    return;
                }
                // Inside ⇒ activate; outside ⇒ cancel. This is the whole contract.
                let inside = self.base.contains_point_with_touch_expansion(*pos);
                self.release();
                if inside {
                    self.base.clicked.emit();
                } else {
                    self.canceled.emit();
                }
            }
            #[cfg(feature = "touch")]
            Event::TouchEnd { pos, .. } if self.grabbed => {
                let inside = self.base.contains_point_with_touch_expansion(*pos);
                self.release();
                if inside {
                    self.base.clicked.emit();
                } else {
                    self.canceled.emit();
                }
            }
            #[cfg(feature = "touch")]
            Event::Tap { .. } if self.base.is_enabled() => {
                // A tap carries no position: the platform already resolved it to this
                // control, which is the same basis the hit test narrows.
                self.base.clicked.emit();
                self.state_changed.emit(self.state());
            }
            Event::FocusGained { reason } => {
                self.focused = true;
                self.focus_reason = *reason;
                self.base.request_redraw();
            }
            Event::FocusLost => {
                self.focused = false;
                // Focus lost mid-press abandons the gesture: the release belongs to
                // whatever the window switch moved to.
                self.cancel_gesture();
                self.base.request_redraw();
            }
            Event::KeyPress { key, .. }
                if is_keyboard_activation(*key) && self.base.is_enabled() =>
            {
                // A keyboard activation is `Shortcut`: the user is on the keyboard, so
                // the ring stays lit. Only the activation itself runs here; the ring
                // follows from the focus reason already stored.
                self.click();
            }
            Event::MouseEnter { .. } => {
                self.hovered = true;
                self.base.request_redraw();
            }
            Event::MouseLeave { .. } => {
                self.hovered = false;
                // A pointer that leaves while held abandons the painted pressed state,
                // but the *grab* is kept: `MouseLeave` fires as soon as the pointer
                // crosses the edge, and a drag that comes back should still be able to
                // complete. Only the release, wherever it lands, ends the gesture and
                // decides the outcome.
                self.set_pressed(false);
                self.base.request_redraw();
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}

/// Whether `button` is one that activates a control.
///
/// Only the primary button activates. The handler used to accept any button, so a
/// secondary click fired a button's action — and a secondary click is how a host opens
/// a context menu (`runtime.rs` routes `mouse_button::SECONDARY` for exactly that).
/// `CheckBox` and `RadioButton` already guarded on the primary button; this brings
/// `Button` into line rather than being the outlier.
fn button_activates(button: u32) -> bool {
    button == crate::event::mouse_button::PRIMARY
}

/// Whether `key` activates a focused button.
///
/// Space and Enter are the two the platform convention assigns, and they are named
/// here rather than left as literals at the match arm: the raw numbers appeared in
/// every binary control with slightly different spellings (`13` here, `13 | 10`
/// elsewhere), so "which keys activate a button" had no single answer to read.
fn is_keyboard_activation(key: u32) -> bool {
    matches!(key, 13 | 10 | 32)
}
impl Draw for Button {
    fn draw(&mut self, context: &mut RenderContext) {
        // Button rendering with style integration (BLUE13 R1.3)
        // Reads style fields (background_color, text_color, border_color, etc.)
        // falling back to state-based defaults when style values are not set.
        let rect = self.geometry();
        let state = self.state();
        let style = self.style();

        // ── The box actually painted ──
        //
        // A button fills the width it was *given* — that is what a button is, and a wider
        // button is still a button. But its **height** is its own: a 240x120 census cell drew a
        // 240x120 pill, which is a rectangle shaped like a button rather than a button. The
        // height comes from `size_hint`, the same derivation a layout asks for, so the drawn
        // shape and the reported size cannot disagree.
        let intrinsic = self.size_hint();
        let painted_height = intrinsic.height.min(rect.height).max(1);
        let rect = Rect::new(
            rect.x,
            rect.y + (rect.height.saturating_sub(painted_height) / 2) as i32,
            rect.width,
            painted_height,
        );

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
        let progress = self.interaction_progress.progress();
        let bg = if progress <= 0.0 {
            bg
        } else {
            let interactive = match state {
                ButtonState::Pressed => bg,
                _ => bg.blend(&bg.contrast_color(), 0.22),
            };
            bg.blend(&interactive, progress)
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
            // The label is placed at the control's own horizontal padding rather than at a
            // literal, so a themed padding and a wider font move the text together — the
            // same derivation the intrinsic size uses (`ControlMetrics::implicit_size`).
            let label_x = rect.x + dimensions::BUTTON_PADDING_H as i32;
            context.draw_text(
                Point { x: label_x, y: line.y },
                &self.text,
                font,
                text_color,
                HorizontalAlignment::Left,
            );
        }

        // ── Focus ring ──
        //
        // Drawn strictly inside the control's rectangle (see `ControlMetrics::focus_ring_rect`)
        // and only when the *reason* focus arrived warrants it: a pointer press focuses without
        // drawing a ring, Tab and Shortcut draw one. This is the Qt Quick rule
        // (`qquickcontrol.cpp:1433`), and it is why `Button` carries a `FocusReason` rather than
        // a bare `focused` bool.
        if self.visual_focus() {
            let ring = FocusRing::for_control(rect, br);
            if ring.is_drawable() {
                context.draw_rounded_rect_stroke(
                    ring.rect,
                    ring.radius,
                    focus_ring_color(bg.contrast_color()),
                    FOCUS_RING_WIDTH,
                );
            }
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
        btn.handle_event(&Event::FocusGained { reason: FocusReason::Programmatic });
        assert!(btn.focused, "Button should be focused after FocusGained");
    }

    #[test]
    fn focus_lost_clears_focused_flag() {
        let mut btn = make_button();
        btn.handle_event(&Event::FocusGained { reason: FocusReason::Programmatic });
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
        // `mouse_button::PRIMARY`, not a bare `1`: the activation guard reads the named
        // code, and a test that spelled its own number would not notice the guard moving.
        let event = Event::MousePress {
            pos: crate::core::Point::new(15, 25),
            button: crate::event::mouse_button::PRIMARY,
        };
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

        let event = Event::MouseRelease {
            pos: crate::core::Point::new(15, 25),
            button: crate::event::mouse_button::PRIMARY,
        };
        b.handle_event(&event);
        assert!(!b.is_pressed());
        assert_eq!(b.state(), ButtonState::Normal);
        assert!(clicked.load(Ordering::SeqCst));
    }

    // ── The activation contract (P0-6 / P0-7) ────────────────────────────

    #[test]
    fn a_release_outside_the_button_cancels_instead_of_clicking() {
        // The defect this guards: every release used to activate, so dragging off a
        // button and letting go still fired it.
        let mut b = make_button();
        let clicked = Arc::new(AtomicBool::new(false));
        let canceled = Arc::new(AtomicBool::new(false));
        b.base.clicked.connect({
            let flag = Arc::clone(&clicked);
            move || flag.store(true, Ordering::SeqCst)
        });
        b.canceled.connect({
            let flag = Arc::clone(&canceled);
            move || flag.store(true, Ordering::SeqCst)
        });

        b.handle_event(&Event::MousePress {
            pos: crate::core::Point::new(20, 30),
            button: crate::event::mouse_button::PRIMARY,
        });
        assert!(b.is_pressed());

        b.handle_event(&Event::MouseRelease {
            pos: crate::core::Point::new(900, 900),
            button: crate::event::mouse_button::PRIMARY,
        });
        assert!(!b.is_pressed(), "the press ends either way");
        assert!(canceled.load(Ordering::SeqCst), "a release outside must cancel");
        assert!(!clicked.load(Ordering::SeqCst), "a release outside must not click");
    }

    #[test]
    fn a_release_inside_the_button_clicks_and_does_not_cancel() {
        let mut b = make_button();
        let clicked = Arc::new(AtomicBool::new(false));
        let canceled = Arc::new(AtomicBool::new(false));
        b.base.clicked.connect({
            let flag = Arc::clone(&clicked);
            move || flag.store(true, Ordering::SeqCst)
        });
        b.canceled.connect({
            let flag = Arc::clone(&canceled);
            move || flag.store(true, Ordering::SeqCst)
        });

        b.handle_event(&Event::MousePress {
            pos: crate::core::Point::new(20, 30),
            button: crate::event::mouse_button::PRIMARY,
        });
        b.handle_event(&Event::MouseRelease {
            pos: crate::core::Point::new(30, 40),
            button: crate::event::mouse_button::PRIMARY,
        });
        assert!(clicked.load(Ordering::SeqCst));
        assert!(!canceled.load(Ordering::SeqCst), "a release inside must not cancel");
    }

    #[test]
    fn pressed_is_continuous_as_the_pointer_moves_out_and_back() {
        // `pressed` is not an edge: dragging off clears it, dragging back restores it.
        let mut b = make_button();
        b.handle_event(&Event::MousePress {
            pos: crate::core::Point::new(20, 30),
            button: crate::event::mouse_button::PRIMARY,
        });
        assert!(b.is_pressed());

        b.handle_event(&Event::MouseMove { pos: crate::core::Point::new(900, 900) });
        assert!(!b.is_pressed(), "dragging off must clear pressed");

        b.handle_event(&Event::MouseMove { pos: crate::core::Point::new(25, 35) });
        assert!(b.is_pressed(), "dragging back must restore pressed");
    }

    #[test]
    fn a_secondary_press_does_not_activate() {
        // The secondary button opens a context menu; it must not also fire the action.
        let mut b = make_button();
        b.handle_event(&Event::MousePress {
            pos: crate::core::Point::new(20, 30),
            button: crate::event::mouse_button::SECONDARY,
        });
        assert!(!b.is_pressed());
    }

    #[test]
    fn losing_focus_mid_press_abandons_the_gesture() {
        let mut b = make_button();
        let canceled = Arc::new(AtomicBool::new(false));
        b.canceled.connect({
            let flag = Arc::clone(&canceled);
            move || flag.store(true, Ordering::SeqCst)
        });

        b.handle_event(&Event::MousePress {
            pos: crate::core::Point::new(20, 30),
            button: crate::event::mouse_button::PRIMARY,
        });
        b.handle_event(&Event::FocusLost);
        assert!(!b.is_pressed());
        assert!(canceled.load(Ordering::SeqCst));
    }

    // ── Visual focus (P0-5) ────────────────────────────────────────────

    #[test]
    fn a_pointer_click_focuses_without_drawing_a_focus_ring() {
        let mut b = make_button();
        b.handle_event(&Event::FocusGained { reason: FocusReason::Pointer });
        assert!(b.is_focused(), "the control is the keyboard target");
        assert!(!b.visual_focus(), "but the user is on the pointer, so no ring is drawn");
    }

    #[test]
    fn tab_and_shortcut_focus_draw_the_focus_ring() {
        for reason in [FocusReason::Tab, FocusReason::BackTab, FocusReason::Shortcut] {
            let mut b = make_button();
            b.handle_event(&Event::FocusGained { reason });
            assert!(b.is_focused());
            assert!(b.visual_focus(), "{reason:?} must draw the ring");
        }
    }

    #[test]
    fn a_keyboard_activation_keeps_the_ring() {
        let mut b = make_button();
        let clicks = Arc::new(core::sync::atomic::AtomicUsize::new(0));
        b.base.clicked.connect({
            let counter = Arc::clone(&clicks);
            move || {
                counter.fetch_add(1, Ordering::SeqCst);
            }
        });
        b.handle_event(&Event::FocusGained { reason: FocusReason::Shortcut });
        b.handle_event(&Event::KeyPress { key: 32, modifiers: 0 });
        assert_eq!(clicks.load(Ordering::SeqCst), 1);
        assert!(b.visual_focus());
    }

    #[test]
    fn a_disabled_button_never_arms_and_never_clicks() {
        let mut b = make_button();
        let clicked = Arc::new(AtomicBool::new(false));
        b.base.clicked.connect({
            let flag = Arc::clone(&clicked);
            move || flag.store(true, Ordering::SeqCst)
        });
        b.set_enabled_state(false);
        b.handle_event(&Event::MousePress {
            pos: crate::core::Point::new(20, 30),
            button: crate::event::mouse_button::PRIMARY,
        });
        assert!(!b.is_pressed());
        b.handle_event(&Event::MouseRelease {
            pos: crate::core::Point::new(20, 30),
            button: crate::event::mouse_button::PRIMARY,
        });
        assert!(!clicked.load(Ordering::SeqCst));
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
        assert_eq!(b.interaction_progress.progress(), 0.5, "a hover settles exactly on its target");

        // Settled means settled: **each** further tick reports no work. Asserted in a loop rather
        // than once, because a single `!tick()` would also pass for an implementation that
        // alternated between reporting work and not — the defect this pins is "keeps asking for
        // frames after the value has stopped changing", and only repetition can see it.
        for frame in 0..5 {
            assert!(
                !b.tick(16),
                "a settled transition must report that no frame is needed (tick {frame})"
            );
            assert_eq!(b.interaction_progress.progress(), 0.5, "and must not drift while settled");
        }
    }

    /// A state change that arrives mid-flight re-aims the same progress rather than restarting.
    #[test]
    fn a_press_during_a_hover_re_aims_rather_than_restarting() {
        let mut b = make_button();
        b.set_hovered(true);
        b.tick(50);
        let partway = b.interaction_progress.progress();
        assert!(partway > 0.0 && partway < 0.5, "the hover is in flight: {partway}");

        b.set_pressed(true);
        assert!(b.tick(1), "the press must continue the movement, not settle on the first frame");
        assert!(
            b.interaction_progress.progress() > partway,
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
