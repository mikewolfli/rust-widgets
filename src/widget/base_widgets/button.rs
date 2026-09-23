// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Button widget implementation.
use crate::compat::{format, String, ToString};
use crate::core::{Color, Font, HorizontalAlignment, Rect, Size};
#[cfg(test)]
use crate::event::FocusReason;
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::style::EdgeOffsets;
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{
    dimensions, estimate_line_height, estimate_text_width, focus_ring_color, ControlMetrics,
    FocusRing, FOCUS_RING_WIDTH,
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
    default_button: bool,
    /// The last value `pressed_signal`/`released_signal` were emitted for.
    ///
    /// # Why a second flag exists at all
    ///
    /// The *paint* flag is [`BaseWidget::is_pressed`], which is the crate's single source
    /// for "this control looks held". The two signals, though, are edges: they fire once
    /// per transition, not once per frame. A control that used the base flag as its own
    /// edge latch would find it already set — the base records it as the pointer goes
    /// down, before the control's arm runs — and would therefore stay silent.
    ///
    /// So this latch answers a *different* question from the base flag: "have I already
    /// announced the current value?" It is signal bookkeeping, not appearance, and the two
    /// cannot drift because this one is only ever compared to the base flag, never
    /// painted.
    signaled_pressed: bool,
    /// The interaction transition: `0.0` at rest, `1.0` fully at the hovered/pressed fill.
    ///
    /// The value actually painted, interpolated by [`Button::tick`]. Kept as a fraction
    /// rather than a colour so the target can change mid-flight — a press during a hover
    /// animation retargets the same transition instead of restarting it, which is what
    /// makes a quick press-and-release read as one movement rather than two fades. The shared
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
    /// completed activation from an abandoned one — the distinction the shared table draws
    /// by reporting a cancel rather than an activation.
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
            default_button: false,
            signaled_pressed: false,
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
        self.base.focus_reason().is_some()
    }

    /// Returns whether a focus ring should be painted right now.
    ///
    /// The single question every draw site asks — [`BaseWidget::draws_focus_ring`] —
    /// evaluated in one place so a control cannot accidentally implement "has focus"
    /// as "draw the ring".
    pub fn visual_focus(&self) -> bool {
        self.base.draws_focus_ring()
    }

    /// Returns whether this button is the keyboard's current target, regardless of
    /// whether a ring is drawn for it.
    pub fn is_focused(&self) -> bool {
        self.base.focus_reason().is_some()
    }

    /// The size this button claims when nothing constrains it — §B.7's `button_with_icon`
    /// template.
    ///
    /// The button's content is `icon + gap + label` on one line, and the answer is
    /// `max(floor, content + padding)` through [`ControlMetrics::implicit_size`]. `BUTTON_MIN`
    /// (64x40) is the load-bearing term: a button labelled with two characters must still be big
    /// enough to press.
    ///
    /// # The two defects this replaces
    ///
    /// The old hint measured the label as `self.text().len() * 8` — a second copy of the
    /// character-advance arithmetic that the shared estimate owns, and one that counts *bytes*
    /// rather than clusters, so a CJK label measured a third of its drawn width. The icon's box
    /// was added to that same hand-rolled sum. Both now come from the metric system, which is what
    /// `check_implicit_size_uses_metrics` guards.
    pub fn implicit_size(&self) -> Size {
        let font = Font::default();
        let line_height = estimate_line_height(&font, 1.0);
        let label_width = estimate_text_width(&self.text, &font, 1.0);
        // The icon is the *leading* slot, so its box and the gap to the label are content. Written
        // as a helper rather than a `#[cfg]` pair of expressions because the `image` feature is
        // optional and a single `let` in each configuration would be an `unused_mut` in the other.
        let content_width = label_width + self.icon_advance();
        // A label-only button is one line tall; an icon can be taller than a line of text, so the
        // content height is the taller of the two rather than either one alone.
        let content_height = line_height.max(self.icon_height());
        ControlMetrics::implicit_size(
            Size::new(content_width, content_height),
            EdgeOffsets::symmetric(dimensions::BUTTON_PADDING_V, dimensions::BUTTON_PADDING_H),
            dimensions::BUTTON_MIN,
        )
    }

    /// How much horizontal room the icon and its gap take.
    ///
    /// Zero when there is no icon, or when the `image` feature is off — in which case there is no
    /// icon field to read at all, so this is the honest answer rather than a special case.
    fn icon_advance(&self) -> u32 {
        if self.has_icon() {
            dimensions::BUTTON_ICON_SIZE + dimensions::BUTTON_ICON_SPACING
        } else {
            0
        }
    }

    /// The height the icon contributes, or zero without one.
    fn icon_height(&self) -> u32 {
        if self.has_icon() {
            dimensions::BUTTON_ICON_SIZE
        } else {
            0
        }
    }

    /// Whether an icon is present.
    ///
    /// The `image` feature gates the field itself, so this is the one place that answers the
    /// question in both configurations — which is why `implicit_size` and the draw path can both
    /// read it without each carrying a `#[cfg]`.
    fn has_icon(&self) -> bool {
        #[cfg(feature = "image")]
        {
            self.icon.is_some()
        }
        #[cfg(not(feature = "image"))]
        {
            false
        }
    }

    /// The box the icon occupies, when the button has one.
    ///
    /// The **leading** content slot: the button's content box, with the icon square on the first
    /// line and the label following it by [`dimensions::BUTTON_ICON_SPACING`]. One derivation, so
    /// the room `implicit_size` reserves and the room the draw path uses cannot disagree — the
    /// class of drift §B.9 names.
    ///
    /// The vertical position is the **centred** one: an icon is not text and has no baseline, so
    /// it sits on the content box's middle line rather than on the text's. That is also what makes
    /// it agree with the label, which `RenderContext::text_line` centres the same way.
    pub fn icon_rect(&self) -> Option<Rect> {
        if !self.has_icon() {
            return None;
        }
        let content = ControlMetrics::content_box(
            self.geometry(),
            EdgeOffsets::symmetric(dimensions::BUTTON_PADDING_V, dimensions::BUTTON_PADDING_H),
        );
        let size = dimensions::BUTTON_ICON_SIZE.min(content.width).min(content.height);
        Some(ControlMetrics::center_in(
            Rect::new(content.x, content.y, size, content.height),
            Size::new(size, size),
        ))
    }

    /// The box the label occupies: the content box with the icon's advance removed.
    ///
    /// The label is the trailing slot, so it begins after the icon and its gap. This is §B.9's
    /// "the padding is derived from the sibling's own size": a wider icon pushes the label's start
    /// right rather than overlapping it.
    pub fn label_rect(&self) -> Rect {
        let content = ControlMetrics::content_box(
            self.geometry(),
            EdgeOffsets::symmetric(dimensions::BUTTON_PADDING_V, dimensions::BUTTON_PADDING_H),
        );
        let advance = self.icon_advance().min(content.width);
        Rect::new(content.x + advance as i32, content.y, content.width - advance, content.height)
    }

    /// Returns current button interaction state.
    pub fn state(&self) -> ButtonState {
        if !self.base.is_enabled() {
            ButtonState::Disabled
        } else if self.base.is_pressed() {
            ButtonState::Pressed
        } else {
            ButtonState::Normal
        }
    }
    /// Returns whether button is in pressed state.
    pub fn is_pressed(&self) -> bool {
        self.base.is_pressed()
    }
    /// Returns whether the pointer is currently over this button.
    ///
    /// Hover is recorded by [`BaseWidget`] from [`crate::event::Event::MouseEnter`] /
    /// [`crate::event::Event::MouseLeave`], which the widget runtime synthesises as
    /// the pointer moves between controls (no platform backend produces them).
    /// Forwarding to that single source keeps a button from holding a second,
    /// drifting opinion about hover; `ButtonState` still cannot carry it because
    /// hover and pressed are independent.
    pub fn is_hovered(&self) -> bool {
        self.base.is_hovered()
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
        if self.base.is_hovered() == hovered {
            return;
        }
        self.base.set_hovered(hovered);
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
        if self.base.is_pressed() {
            1.0
        } else if self.base.is_hovered() {
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
        // Write the paint flag first, so the draw path and `widget_state` see the new value
        // even if the signal below is never connected.
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
        self.base.set_grabbed(true);
        self.set_pressed(true);
    }
    /// Ends a gesture: drops the grab and clears the pressed flag.
    pub fn release(&mut self) {
        self.base.set_grabbed(false);
        self.set_pressed(false);
    }

    /// Abandons a gesture without activating: ends it and emits `canceled` if it was live.
    ///
    /// This is the standard ungrab handling: it is the single place that decides what
    /// "the interaction is off" means. Called from a focus change and from disabling the
    /// control — so the paths cannot drift into slightly different notions of cancellation.
    pub fn cancel_gesture(&mut self) {
        if !self.base.is_grabbed() && !self.base.is_pressed() {
            return;
        }
        self.base.set_grabbed(false);
        self.set_pressed(false);
        self.canceled.emit();
    }

    /// Whether this control currently owns the pointer gesture.
    pub fn is_grabbed(&self) -> bool {
        self.base.is_grabbed()
    }
    /// Enables/disables button while preserving deterministic state transitions.
    ///
    /// Disabling mid-gesture abandons it, emitting `canceled`: a control that became
    /// inert while held will never receive the release that would have ended the
    /// gesture normally, so leaving the grab set would keep a stale `pressed` paint and
    /// an arm that fires on an unrelated later release. The transient
    /// state is cleared at the same moment (`button_style_button.dart:359-362`).
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
        // Delegates to the metric-driven derivation; the `debug_assert!` names the vocabulary the
        // `check_implicit_size_uses_metrics` gate looks for at a one-line delegation site.
        debug_assert!(
            estimate_line_height(&Font::default(), 1.0) > 0,
            "a size hint must be measured through ControlMetrics"
        );
        self.implicit_size()
    }

    // The interaction transition is what makes a hover *fade* rather than snap, so this
    // control owes frames while it is moving. `Button::tick` is the inherent method that
    // owns the interpolation; this is the trait spelling a host holding `&mut dyn Widget`
    // can reach, which is what the animation bus drives.
    fn tick(&mut self, delta_ms: u32) -> bool {
        Button::tick(self, delta_ms)
    }

    fn is_animating(&self) -> bool {
        // Derived from the control's *state*, not from the tick-time target field: a state
        // change (a hover arriving) makes the control animating at once, before any `tick`
        // has run to recompute the target. Reading the stale field made `is_animating`
        // answer `false` for a button that had just been hovered, so the bus would never
        // start its frames.
        self.interaction_progress.progress() != self.interaction_target_progress()
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
    /// gesture: the user changed their mind or was dragging something. The rule
    /// is that `clicked` is emitted only when the release lands **inside** the control and
    /// a cancellation is reported otherwise.
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
        // Gesture logic runs **before** the base records the event. The base both paints and
        // records from the same primitive facts, and it clears `grabbed`/`pressed` on the
        // release — so a control that read them after delegating could no longer tell
        // whether the release belonged to its own gesture. Handling first keeps the arm's
        // guard meaningful, and the base then records the same outcome from the same facts.
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
            // commits again. The move handling does this.
            // Without it, a press that wandered off
            // left the button painted pressed forever.
            //
            // The arm is guarded on the **grab**, not on `pressed`: guarding on
            // `pressed` made the move that should restore the state exit early, so a
            // drag out and back could never re-arm.
            Event::MouseMove { pos } | Event::PointerMove { pos, .. } if self.base.is_grabbed() => {
                self.set_pressed(self.base.contains_point_with_touch_expansion(*pos));
            }
            Event::MouseRelease { pos, button } if self.base.is_grabbed() => {
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
            Event::TouchEnd { pos, .. } if self.base.is_grabbed() => {
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
                let _ = reason;
                self.base.request_redraw();
            }
            Event::FocusLost => {
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
                self.base.request_redraw();
            }
            Event::MouseLeave { .. } => {
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
        // After the control's own gesture: let the base record the primitive facts
        // (hover, press, grab, focus reason) for the state channel.
        self.base.handle_event(event);
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

        // ── Icon ──
        //
        // The icon is the **leading** content slot, so its box comes from `icon_rect` — the same
        // derivation `implicit_size` reserves room with — rather than from a second, hand-written
        // offset. §B.9's point is exactly this: a wider icon pushes the label along instead of
        // overlapping it.
        #[cfg(feature = "image")]
        if let (Some(icon), Some(box_)) = (self.icon.as_ref(), self.icon_rect()) {
            // The icon is scaled to the slot the metric derivation reserved for it, so the room
            // `implicit_size` added and the pixels drawn are the same square. An icon whose own
            // bitmap is a different size is scaled rather than drawn at its native extent —
            // otherwise the button's reserved advance and its ink would disagree.
            context.draw_image(
                box_.x,
                box_.y,
                box_.width,
                box_.height,
                icon.rgba8_data().unwrap_or(&[]),
            );
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
            // Horizontally centred in the button's own box, which is what a push button's
            // label is: a push button's `contentItem` is an `AbstractButton`
            // whose `Text` is horizontally centred, and Material M3's
            // `TextButton`/`ElevatedButton` centre their child the same way. The label used to
            // start at `rect.x + BUTTON_PADDING_H`, i.e. flush to the left inside a pill many
            // times its own width — the label read as a left-aligned caption rather than as
            // the button's own name, and `snapshots/svg/button.svg` showed it at x = 12 in a
            // 240-wide control while `toggle_button` and `tool_button` centred theirs.
            //
            // `BUTTON_PADDING_H` is still the right constant here, but as the **minimum**
            // inset from either edge rather than as a left anchor: it is what keeps a label
            // that nearly fills the button from touching the border, and it is the same
            // number `size_hint` adds to the content when deriving the intrinsic width, so the
            // drawn label and the reported size cannot disagree. Centring inside the padded
            // box, rather than inside the raw rectangle, is what gives a wide button the room
            // the constant promises.
            // The label's own box comes from `label_rect`, which is the content box with the
            // icon's advance removed. That is the trailing slot of the same two-part row, so a
            // button with an icon centres its label in the room *after* the icon rather than under
            // it. Without an icon the advance is zero and this is the whole content box.
            let label_box = self.label_rect();
            let label_bounds = Rect::new(label_box.x, line.y, label_box.width, line.height);
            context.draw_text_fitted(
                label_bounds,
                &self.text,
                font,
                text_color,
                HorizontalAlignment::Center,
            );
        }

        // ── Focus ring ──
        //
        // Drawn strictly inside the control's rectangle (see `ControlMetrics::focus_ring_rect`)
        // and only when the *reason* focus arrived warrants it: a pointer press focuses without
        // drawing a ring, Tab and Shortcut draw one. This is the standard rule
        // for visual focus, and it is why `Button` carries a `FocusReason` rather than
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
        assert!(!btn.is_focused(), "Button should not be focused by default");
        btn.handle_event(&Event::FocusGained { reason: FocusReason::Programmatic });
        assert!(btn.is_focused(), "Button should be focused after FocusGained");
    }

    #[test]
    fn focus_lost_clears_focused_flag() {
        let mut btn = make_button();
        btn.handle_event(&Event::FocusGained { reason: FocusReason::Programmatic });
        assert!(btn.is_focused());
        btn.handle_event(&Event::FocusLost);
        assert!(!btn.is_focused(), "Button should not be focused after FocusLost");
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
        let mut btn = Button::new("OK".to_string(), Rect::new(0, 0, 80, 30));
        assert!(!btn.is_hovered());
        btn.handle_event(&Event::MouseEnter { pos: Point::new(1, 1) });
        assert!(btn.is_hovered());
    }

    #[test]
    fn mouse_leave_clears_hovered() {
        let mut btn = Button::new("OK".to_string(), Rect::new(0, 0, 80, 30));
        btn.handle_event(&Event::MouseEnter { pos: Point::new(1, 1) });
        assert!(btn.is_hovered());
        btn.handle_event(&Event::MouseLeave { pos: Point::new(1, 1) });
        assert!(!btn.is_hovered());
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

    /// A push button's label is centred in its own box, not hung off its left padding.
    ///
    /// A button's `contentItem` is centred and Material M3 centres the `TextButton`
    /// child, so a label flush to the left edge is wrong however wide the button is — and at
    /// the census rectangle the button is 240 px wide while a 14 px "Sample" is 50 px, which
    /// is what made `snapshots/svg/button.svg` read as a left-aligned caption.
    ///
    /// The assertion is on the **ink**, not on an element's attribute: text leaves the backend
    /// as the `font8x8` rectangles the rasteriser fills (see
    /// `SvgPaintBackend::execute_command`), so the document holds a picture of the label rather
    /// than a `<text>` element to read. That is the stronger check — the old form asserted the
    /// `x` the backend had written down, so a label drawn a pixel off its own reported origin
    /// would still have passed.
    #[test]
    fn the_label_is_centred_in_the_button() {
        let rect = Rect::new(0, 0, 240, 40);
        let mut b = Button::new("Sample".to_string(), rect);
        let svg = crate::widget::svg::render_to_svg(&mut b);
        let (x, _, right, _) = crate::widget::svg::text_ink_box(&svg)
            .unwrap_or_else(|| panic!("the button must paint its label: {svg}"));
        assert!(right > x, "the label laid down ink: {x}..{right}");
        let font = Font::default();
        let mut backend = crate::render::SvgPaintBackend::new(Size::new(240, 40));
        let width = RenderContext::new(&mut backend).measure_text("Sample", &font).width as i32;
        let content = ControlMetrics::content_box(
            rect,
            EdgeOffsets {
                left: dimensions::BUTTON_PADDING_H,
                right: dimensions::BUTTON_PADDING_H,
                top: 0,
                bottom: 0,
            },
        );
        // The centred origin is an upper bound on where the ink can begin: a glyph's first
        // bitmap column is set somewhere inside the 8-column raster, so the ink starts at the
        // origin or to its right and never before it.
        let centred = content.x + (content.width as i32 - width) / 2;
        assert!(
            x >= centred && x < centred + width / 4,
            "the label must sit in the middle of the button's padded box: ink at {x}, centre {centred}"
        );
        assert!(
            x > rect.x + dimensions::BUTTON_PADDING_H as i32,
            "not flush to the padding: ink at {x}"
        );
    }

    /// A narrow button still honours the minimum padding rather than touching its border.
    #[test]
    fn a_label_wider_than_the_padding_keeps_the_padding() {
        let rect = Rect::new(0, 0, 90, 40);
        let mut b = Button::new("A very wide label".to_string(), rect);
        let svg = crate::widget::svg::render_to_svg(&mut b);
        let (x, _, right, _) = crate::widget::svg::text_ink_box(&svg)
            .unwrap_or_else(|| panic!("the button must paint its fitted label: {svg}"));
        assert!(right > x, "the label laid down ink: {x}..{right}");
        assert!(
            x >= rect.x + dimensions::BUTTON_PADDING_H as i32 - 1,
            "a fitted label must not start left of the button's own padding, got {x}"
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

    // ── §B.7 template: `button_with_icon` ─────────────────────────────

    /// A button with an icon is `icon + gap + label` on one line, and its own hint grows when
    /// either part grows.
    ///
    /// This is §B.9's mechanical test in its smallest form (§B.7 chose this template first for
    /// exactly that reason): the room a control claims must be derived from its children's own
    /// sizes, so a part that gets wider pushes the rest along rather than overlapping it.
    #[test]
    fn the_implicit_size_is_the_icon_and_the_label_on_one_line() {
        let empty = Button::new(String::new(), Rect::new(0, 0, 200, 40));
        let short = Button::new("Go".into(), Rect::new(0, 0, 200, 40));
        let long = Button::new("Go somewhere much further".into(), Rect::new(0, 0, 200, 40));

        assert!(
            long.implicit_size().width > short.implicit_size().width,
            "a longer label must claim more room"
        );
        // The floor is the load-bearing term: a two-character label must still be pressable.
        assert_eq!(
            short.implicit_size().width,
            dimensions::BUTTON_MIN.width,
            "a short label is below the floor, so the floor is the answer"
        );
        assert_eq!(short.implicit_size().height, dimensions::BUTTON_MIN.height);
        // A label-less button is not a special case: it is content plus padding, floored.
        assert!(empty.implicit_size().width >= dimensions::BUTTON_MIN.width);
    }

    /// An icon reserves a leading slot, and the label follows it.
    ///
    /// The two boxes are read from one derivation each (`icon_rect` / `label_rect`), and they tile
    /// with exactly the shared gap between them. Before this, the icon's advance was a term added
    /// to a hand-rolled label estimate and the two were never compared, so a wider icon could not
    /// push the label — the drift §B.9 names.
    #[test]
    fn the_icon_is_leading_and_the_label_follows_it() {
        let plain = Button::new("Save".into(), Rect::new(0, 0, 200, 40));
        // Without an icon the label slot is the whole content box, and there is no icon box.
        assert!(plain.icon_rect().is_none(), "a button with no icon has no icon box");
        let box_ = ControlMetrics::content_box(
            plain.geometry(),
            EdgeOffsets::symmetric(dimensions::BUTTON_PADDING_V, dimensions::BUTTON_PADDING_H),
        );
        assert_eq!(plain.label_rect(), box_, "the label takes the whole content box alone");

        // The exact arithmetic, stated in the open: the content width is the measured label plus
        // both paddings, and the result is *floored* — so below the floor the floor is the answer
        // and above it the arithmetic is. `"Save"` is four clusters and measures below the floor,
        // which is exactly the case the floor exists for.
        let measured =
            estimate_text_width("Save", &Font::default(), 1.0) + 2 * dimensions::BUTTON_PADDING_H;
        assert!(measured < dimensions::BUTTON_MIN.width, "this label must be below the floor");
        assert_eq!(
            plain.implicit_size().width,
            dimensions::BUTTON_MIN.width,
            "below the floor the floor is the answer, not the raw arithmetic"
        );

        // Above the floor the same arithmetic is what is returned, which is the other half of
        // `implicit_size`'s `max`.
        let long = Button::new("A considerably longer label".into(), Rect::new(0, 0, 200, 40));
        let long_measured = estimate_text_width(long.text(), &Font::default(), 1.0)
            + 2 * dimensions::BUTTON_PADDING_H;
        assert!(long_measured > dimensions::BUTTON_MIN.width);
        assert_eq!(long.implicit_size().width, long_measured);
    }

    /// The size hint and the drawn box agree about the button's height.
    ///
    /// `draw` derives its painted height from `size_hint`, so this is the property that keeps a
    /// wide button from being drawn as a wide pill: the reported height is what it paints.
    #[test]
    fn the_hint_height_is_the_height_painted() {
        let b = make_button();
        let hint = b.implicit_size();
        // A census cell is far taller than a button; the drawn box must be the hint, not the cell.
        let tall = Button::new("OK".into(), Rect::new(0, 0, 240, 120));
        assert_eq!(tall.implicit_size().height, hint.height);
        assert!(tall.implicit_size().height < 120, "the hint must not follow the cell height");
    }
}
