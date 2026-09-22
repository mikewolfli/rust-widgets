// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Switch/Toggle widget — a modern on/off binary state control.
//!
//! The Switch widget presents a sliding toggle that represents a boolean state,
//! similar to iOS UISwitch or Android Switch material widget. It supports
//! on/off toggling, animated transitions, and accessibility role mapping.

use crate::core::{Color, Rect};
use crate::event::{Event, EventHandler, FocusReason};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::style::{Transition, TransitionTempo};
use crate::widget::capability::coercion::expect_bool;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{
    dimensions, focus_ring_color, ControlMetrics, FocusRing, FOCUS_RING_WIDTH,
};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Switch/Toggle widget for binary on/off state selection.
pub struct Switch {
    base: BaseWidget,
    /// The **logical** state: the answer `is_checked` gives, changed by the user's action
    /// the instant it happens. It is not what the thumb is drawn from — see `travel`.
    checked: bool,
    /// `true` between a pointer press that hit this control and the release that ends
    /// it. The release is what commits the toggle, so a press that began elsewhere and
    /// merely *ends* over the switch must not flip it — see `handle_event`.
    pressed: bool,
    /// Whether this control currently owns keyboard focus.
    focused: bool,
    /// Why it got focus, which decides whether a focus ring is painted.
    ///
    /// Kept alongside `focused` rather than folded into it because the two answer
    /// different questions: `focused` is "am I the keyboard target", this is "is the
    /// user on the keyboard". Qt Quick spells the pair `activeFocus` / `visualFocus`.
    focus_reason: FocusReason,
    hovered: bool,
    /// The **drawn** state: how far the thumb has travelled, `0.0` at the off end and
    /// `1.0` at the on end.
    ///
    /// # Why this is not `checked`
    ///
    /// `checked` is the logical state, and a toggle must be answered immediately: a
    /// caller that reads `is_checked` after a click has to see the new value the moment
    /// the click happened. Drawing the thumb from `checked` would therefore *also* move
    /// the thumb the instant the click arrived, with no transition — and a control whose
    /// motion has to be observable cannot put its presentation into the value the caller
    /// reads. Qt Quick draws the same distinction as `position` (logical) versus
    /// `visualPosition` (`qquickslider.cpp:395`); this field is the crate's
    /// `visualPosition`. `travel = 0` renders exactly the old off-end appearance, so a
    /// snapshot taken without a `tick` is unchanged.
    travel: Transition,
    /// Emitted when the checked state changes.
    pub toggled: Signal1<bool>,
}

impl Switch {
    /// Creates a new Switch widget with the given geometry.
    /// Initial state is unchecked (off).
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Switch, geometry, "Switch"),
            checked: false,
            pressed: false,
            focused: false,
            // No focus yet, so the reason is never read. `Programmatic` is the
            // variant that claims the least: it does not assert a gesture happened.
            focus_reason: FocusReason::Programmatic,
            hovered: false,
            // A toggle travelling across its track is a *larger* movement than a direct
            // reaction to the pointer, which is what the theme's `slow` token describes.
            // Starting at rest (0.0) keeps a freshly built switch at the off end instead
            // of fading *out* on its first frame.
            travel: Transition::with_tempo(TransitionTempo::Slow),
            toggled: Signal1::new(),
        }
    }

    /// Returns whether the switch is in the checked (on) state.
    pub fn is_checked(&self) -> bool {
        self.checked
    }

    /// Sets the checked state. Emits `toggled` signal if the state actually changes.
    pub fn set_checked(&mut self, checked: bool) {
        if self.checked != checked {
            self.checked = checked;
            self.toggled.emit(checked);
            self.base.request_redraw();
        }
    }

    /// Toggles the checked state.
    pub fn toggle(&mut self) {
        self.set_checked(!self.checked);
    }

    /// `true` while a pointer press that hit this control is still held.
    pub fn is_pressed(&self) -> bool {
        self.pressed
    }

    /// Returns whether this switch is the keyboard's current target, regardless of
    /// whether a ring is drawn for it.
    pub fn is_focused(&self) -> bool {
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

    /// Returns whether the pointer is currently over this switch.
    ///
    /// Hover is tracked from [`crate::event::Event::MouseEnter`] /
    /// [`crate::event::Event::MouseLeave`], which the widget runtime synthesises as
    /// the pointer moves between controls (no platform backend produces them).
    pub fn is_hovered(&self) -> bool {
        self.hovered
    }

    /// Sets the hovered flag.
    ///
    /// # Why this is public
    ///
    /// The `MouseEnter`/`MouseLeave` arms set the flag from real pointer events, but a
    /// host driving a control from its own input layer — a touch backend that has no
    /// hover concept, a test, or a designer previewing a state — needs a way to say
    /// "show me this control hovered". Without it the hovered appearance was reachable
    /// only by synthesising an event.
    ///
    /// Setting it also requests a redraw, because the flag changes what is painted.
    pub fn set_hovered(&mut self, hovered: bool) {
        if self.hovered == hovered {
            return;
        }
        self.hovered = hovered;
        self.base.request_redraw();
    }

    /// How far the thumb has travelled, `0.0` at the off end and `1.0` at the on end.
    ///
    /// This is the *drawn* position, not the logical state: it is what the draw site
    /// reads to place the thumb and to blend the track colour, so the two cannot slide
    /// out of step with each other.
    pub fn travel_progress(&self) -> f32 {
        self.travel.progress()
    }

    /// Advances the thumb's travel by `delta_ms` and reports whether another frame is
    /// needed.
    ///
    /// # The contract this implements
    ///
    /// Return `true` while there is still movement and `false` once the thumb has
    /// settled, so a host can stop scheduling frames for a control that is not moving. A
    /// `tick` that always returned `true` would keep the whole application repainting
    /// forever, which is why the boolean is part of the signature rather than something
    /// the caller infers.
    ///
    /// # Why the target is recomputed here
    ///
    /// The target comes from the control's own logical state on every tick, so a toggle
    /// that arrived without a `tick` in between is picked up rather than missed — and a
    /// reversal mid-flight re-aims the travel from where it currently is instead of
    /// restarting from the far end.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        let target = if self.checked { 1.0 } else { 0.0 };
        self.travel.tick(target, delta_ms)
    }
}

impl Widget for Switch {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        // The hint describes the control's own floor rather than the drawn track: the track
        // is `SWITCH_TRACK`, but a switch that can show a focus ring needs room for the
        // ring's inset on both sides. `SWITCH_TRACK.width` alone made the hint *narrower*
        // than the width at which the ring is drawable, so `size_hint` described a control
        // whose focus state could not be rendered. The height stays the track's own.
        crate::core::Size::new(
            dimensions::SWITCH_TRACK.width + FOCUS_RING_WIDTH * 2,
            dimensions::SWITCH_TRACK.height,
        )
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `Switch`'s property contract.
impl WidgetProperties for Switch {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "checked" => Ok(CapabilityValue::Bool(self.is_checked())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "checked" => {
                self.set_checked(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `SWITCH_PROPERTIES`.
        property_names_of!["checked", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `switch` publishes.
    ///
    /// `toggle` is the control's zero-argument action and flips the latch, emitting
    /// `toggled` on the transition exactly as a pointer activation does.
    /// `set_checked` assigns the same state but needs the boolean, so it is refused as
    /// [`CapabilityAccessError::OutOfRange`] — use `set("checked", ..)` — rather than
    /// reported unknown.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "toggle" => {
                self.toggle();
                Ok(())
            }
            "set_checked" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for Switch {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();
        let style = self.style();

        // Track geometry
        //
        // The track is a **fixed-size** stadium centred inside `rect`, not a scaling of
        // it. Deriving the track from the geometry made the *control* the track: a
        // 240x120 census cell drew a 240x120 stadium with a 116x116 thumb, which is a
        // picture of a switch-shaped rectangle rather than a switch. The geometry is the
        // widget's *occupancy* — its hit area and its layout slot — and the drawn chrome
        // is a separate, fixed proportion of it. That is the same split Flutter's
        // `Switch` makes (a 52x32 track, a 14px thumb radius), and it is why `size_hint`
        // describes the control without the drawn shape depending on the layout engine's
        // answer.
        //
        // The values come from the shared `dimensions` table rather than being three
        // independent literals. As literals they were `52`/`32`/`2` here and `50`/`28` in
        // `size_hint` — four numbers describing one control, with nothing tying them
        // together, so the hint and the ink could (and did) describe different switches.
        // `ControlMetrics::center_in` is the shared derivation for "fixed-size chrome
        // centred in the area I was given", and it clamps *down* rather than up: a control
        // laid out smaller than the nominal track must not paint outside the rectangle it
        // was given, since nothing clips a widget at this layer.
        let track_rect = ControlMetrics::center_in(rect, dimensions::SWITCH_TRACK);
        let track_width = track_rect.width;
        let track_height = track_rect.height;
        // The thumb is a disc the same size in every switch, inset from the track's edge.
        // Deriving it from `track_height - inset * 2` made the thumb a fraction of the
        // control's height, which is the same defect one level down: the disc's size is a
        // property of the switch, not of the rectangle it was handed.
        let thumb_size = dimensions::SWITCH_THUMB_RADIUS * 2;
        let thumb_inset = dimensions::SWITCH_THUMB_INSET;
        // A track too small to hold the disc draws no thumb at all rather than a squeezed
        // one; the guard below is the same "degenerate element is not a small element"
        // rule the progress bar's fill follows.
        let thumb_size = thumb_size.min(track_height.saturating_sub(thumb_inset * 2));

        // Draw track
        //
        // The track colour resolves **caller first, then the semantic token, then the
        // literal**. The rung that used to sit here — the theme's *resolved* background —
        // was dead code: `switch` classifies as `WidgetRole::Choice`, and `role_colors` in
        // `src/theme/manager.rs` writes `Some(input_background())` into that role's
        // background unconditionally. So `themed_background` was always `Some`, and the
        // accent rung below it (the `Success` token, which is what makes an ON switch
        // *green*) could never be reached: every switch drew the theme's field grey, and
        // `switch.svg` showed a grey track for both the ON and the OFF state.
        //
        // `theme_derived` is what separates the two things that `background_color` was
        // conflating. When the theme authored the style the value belongs to the theme,
        // not the caller, and the control is free to substitute a more meaningful token;
        // when the caller set a colour it must win. Same rule, and the same mechanism, as
        // `banner.rs`.
        let theme = crate::style::resolved_theme_style("switch");
        let caller_background = if style.theme_derived { None } else { style.background_color };
        // A stripped device build has no theme module, so the semantic token cannot be read
        // and the literal below is the only rung. `#[cfg]` on the binding rather than on the
        // call keeps the binding's type identical in both profiles.
        #[cfg(device_profile)]
        let themed_accent = crate::style::semantic_color(crate::style::SemanticColor::Success);
        #[cfg(not(device_profile))]
        let themed_accent: Option<Color> = None;
        // The OFF track is *chrome*, so it descends from the theme's own resolved
        // background — the field grey `Choice` resolves to — stepping one shade toward the
        // ink when that colour would be the window's own fill. A track painted in the
        // window colour is invisible against the surface the switch sits on.
        #[cfg(device_profile)]
        let themed_track = caller_background
            .or_else(|| theme.as_ref().and_then(|resolved| resolved.background_color))
            .map(|background| {
                let window = crate::style::theme_manager()
                    .current_theme()
                    .map(|active| active.colors.background);
                if window == Some(background) {
                    let ink = crate::style::theme_manager()
                        .current_theme()
                        .map(|active| active.colors.foreground)
                        .unwrap_or(Color::BLACK);
                    background.blend(&ink, 0.14)
                } else {
                    background
                }
            });
        #[cfg(not(device_profile))]
        let themed_track = caller_background;

        let off_track =
            caller_background.or(themed_track).unwrap_or(Color::rgba(180, 180, 180, 200));
        let on_track = caller_background.or(themed_accent).unwrap_or(Color::rgba(52, 199, 89, 200)); // iOS green
                                                                                                     // The track's colour is a function of the *travel*, not of two discrete states.
                                                                                                     // Blending the two endpoint colours by `travel.progress()` is what makes the track
                                                                                                     // change colour on the same frame as the thumb moves: a track that switched colour
                                                                                                     // on `checked` while the thumb was still crossing would read as two separate
                                                                                                     // actions, which is exactly the bug this control's `travel` exists to remove.
        let travel = if is_enabled {
            self.travel.progress()
        } else {
            // A disabled switch shows its *logical* state at rest; there is no motion to
            // follow, and leaving it at 0 would draw a grey track for a switch that is on.
            if self.checked {
                1.0
            } else {
                0.0
            }
        };
        let track_color = if !is_enabled {
            off_track.blend(&Color::rgba(200, 200, 200, 160), 1.0)
        } else {
            off_track.blend(&on_track, travel)
        };
        context.fill_rounded_rect(track_rect, track_height / 2, track_color);

        // Draw thumb
        //
        // The thumb's x is derived from `travel.progress()` and **not** from `checked`.
        // Reading `checked` here is precisely the defect the transition exists to fix: the
        // thumb would jump to the far end on the same frame the logical state changed, so
        // the animation could never be seen and a "travelling" toggle was really a
        // two-state image. `travel` is the crate's `visualPosition`.
        if thumb_size == 0 {
            return;
        }
        let travel_span = track_width.saturating_sub(thumb_size + thumb_inset * 2);
        let thumb_x = track_rect.x + thumb_inset as i32 + (travel_span as f32 * travel) as i32;
        let thumb_y = track_rect.y + thumb_inset as i32;
        let thumb_rect = Rect::new(thumb_x, thumb_y, thumb_size, thumb_size);

        let thumb_color = if !is_enabled {
            Color::rgba(240, 240, 240, 200)
        } else {
            // The thumb reads the theme's foreground (a light thumb on a dark
            // surface, a dark-ish thumb never — the thumb is always the lighter of
            // the pair), falling back to white only when no theme is active.
            theme
                .as_ref()
                .and_then(|resolved| resolved.text_color)
                .map(|text| {
                    // Keep the thumb light: mix most of the way to white so it stays
                    // the highlight against the coloured track.
                    text.blend(&Color::WHITE, 0.85)
                })
                .unwrap_or(Color::WHITE)
        };
        // A hovered switch answers the pointer before it is pressed, so the thumb steps
        // toward the ink: without it a switch that is under the cursor looks exactly like
        // one that is not, which is the affordance the control was missing.
        let thumb_color = if is_enabled && self.hovered {
            thumb_color.blend(&track_color.contrast_color(), 0.10)
        } else {
            thumb_color
        };
        context.fill_rounded_rect(thumb_rect, thumb_size / 2, thumb_color);

        // Draw thumb shadow/border
        let thumb_border_color = style.border_color.unwrap_or(Color::rgba(0, 0, 0, 30));
        context.draw_rounded_rect_stroke(thumb_rect, thumb_size / 2, thumb_border_color, 1);

        // ── Focus ring ──
        //
        // Drawn strictly inside the control's rectangle (see `ControlMetrics::focus_ring_rect`)
        // and only when the *reason* focus arrived warrants it: a pointer press focuses without
        // drawing a ring, Tab and Shortcut draw one. This is the Qt Quick rule
        // (`qquickcontrol.cpp:1433`), and it is why `Switch` carries a `FocusReason` rather than
        // a bare `focused` bool.
        if self.visual_focus() {
            let ring = FocusRing::for_control(rect, dimensions::SWITCH_THUMB_RADIUS);
            if ring.is_drawable() {
                // The ring's colour is the contrast of the track it sits beside, so it reads
                // on both the on and the off state rather than on one of them.
                context.draw_rounded_rect_stroke(
                    ring.rect,
                    ring.radius,
                    focus_ring_color(track_color.contrast_color()),
                    FOCUS_RING_WIDTH,
                );
            }
        }
    }
}

impl EventHandler for Switch {
    /// Toggles on a completed activation, not on a bare release.
    ///
    /// # Why the press arm is load-bearing
    ///
    /// This handler used to be `Event::MouseRelease => self.toggle()` with the position
    /// discarded and no press tracking. Any release the host routed here flipped the
    /// switch — including a drag that began outside it and merely ended on top, and
    /// (since the control never takes pointer capture) any release a host delivered
    /// without a preceding press inside the control.
    ///
    /// [`Button`](crate::widget::base_widgets::button) and
    /// [`CheckBox`](crate::widget::base_widgets::checkbox) both arm on
    /// `MousePress`, which is the pattern the rest of the crate's binary controls
    /// follow; a switch is the same interaction and now behaves identically. Touch,
    /// `Tap` and the space bar are accepted for the same reason they are there:
    /// the control is otherwise unreachable from a touch host or a keyboard.
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        let enabled = self.base.is_enabled();
        match event {
            Event::MousePress { pos, button: 1 } if enabled => {
                // Arm only for a press that actually lands on the control; a press
                // outside must not leave the latch armed for a later release.
                self.pressed = self.base.contains_point_with_touch_expansion(*pos);
            }
            Event::MouseRelease { pos, button: 1 } if self.pressed => {
                self.pressed = false;
                // A release off the control cancels, matching the platform convention
                // that dragging away from a toggle abandons the interaction.
                if self.base.contains_point_with_touch_expansion(*pos) {
                    self.toggle();
                }
            }
            Event::MouseRelease { button: 1, .. } => {
                self.pressed = false;
            }
            #[cfg(feature = "touch")]
            Event::TouchBegin { pos, .. } if enabled => {
                self.pressed = self.base.contains_point_with_touch_expansion(*pos);
            }
            #[cfg(feature = "touch")]
            Event::TouchEnd { pos, .. } if self.pressed => {
                self.pressed = false;
                if self.base.contains_point_with_touch_expansion(*pos) {
                    self.toggle();
                }
            }
            #[cfg(feature = "touch")]
            Event::Tap { .. } if enabled => {
                self.toggle();
            }
            Event::KeyPress { key, .. } if *key == 32 && enabled => {
                self.toggle();
            }
            // Focus entry carries the *reason*, and the reason is what decides whether a
            // focus ring is painted; storing only a bool would make a click and a Tab
            // indistinguishable and the ring would appear on click, which reads as a stuck
            // highlight under the cursor.
            Event::FocusGained { reason } => {
                self.focused = true;
                self.focus_reason = *reason;
                self.base.request_redraw();
            }
            // Focus loss abandons a held press, so the latch cannot survive a
            // window switch and fire on an unrelated later release — and it also drops
            // the ring, because the control is no longer the keyboard's target.
            Event::FocusLost => {
                self.focused = false;
                self.pressed = false;
                self.base.request_redraw();
            }
            Event::MouseEnter { .. } => {
                self.set_hovered(true);
            }
            Event::MouseLeave { .. } => {
                self.set_hovered(false);
            }
            _ => {}
        }
    }
}

#[cfg(all(test, full_widgets))]
mod tests {
    use super::*;
    use crate::core::Point;
    use std::sync::{Arc, Mutex};

    #[test]
    fn switch_default_is_unchecked() {
        let sw = Switch::new(Rect::new(0, 0, 60, 30));
        assert!(!sw.is_checked());
        assert_eq!(sw.kind(), WidgetKind::Switch);
    }

    #[test]
    fn switch_set_checked_emits_signal() {
        let mut sw = Switch::new(Rect::new(0, 0, 60, 30));
        let captured = Arc::new(Mutex::new(None));
        sw.toggled.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<bool>| {
                *captured.lock().unwrap() = Some(*val);
            }
        });

        sw.set_checked(true);
        assert!(sw.is_checked());
        assert_eq!(*captured.lock().unwrap(), Some(true));
    }

    #[test]
    fn switch_toggle_flips_state() {
        let mut sw = Switch::new(Rect::new(0, 0, 60, 30));
        assert!(!sw.is_checked());
        sw.toggle();
        assert!(sw.is_checked());
        sw.toggle();
        assert!(!sw.is_checked());
    }

    /// A completed pointer activation toggles the switch.
    ///
    /// The test used to dispatch a bare `MouseRelease`, which meant it asserted the
    /// *defect*: the release position was discarded and no press was required, so any
    /// release the host routed here flipped the control. The intent — "a pointer
    /// activation toggles" — is unchanged; only the interaction it performs is now
    /// the real one. The negative cases live in
    /// `switch_release_without_press_does_not_toggle` and
    /// `switch_press_outside_then_release_inside_does_not_toggle`.
    #[test]
    fn switch_mouse_press_toggles() {
        let mut sw = Switch::new(Rect::new(0, 0, 60, 30));
        let p = Point::new(10, 10);
        sw.handle_event(&Event::MousePress { pos: p, button: 1 });
        sw.handle_event(&Event::MouseRelease { pos: p, button: 1 });
        assert!(sw.is_checked());
    }

    /// A release with no qualifying press must not toggle.
    ///
    /// Reproduces the reported defect: the old handler toggled on *any* release, so a
    /// bare `MouseRelease` — including one at a position far outside the geometry —
    /// flipped the switch.
    #[test]
    fn switch_release_without_press_does_not_toggle() {
        let mut sw = Switch::new(Rect::new(0, 0, 60, 30));
        sw.handle_event(&Event::MouseRelease { pos: Point::new(10, 10), button: 1 });
        assert!(!sw.is_checked(), "a release with no press must not toggle");
        // The original reproduction: a release nowhere near the control.
        sw.handle_event(&Event::MouseRelease { pos: Point::new(5000, 5000), button: 1 });
        assert!(!sw.is_checked(), "a release far outside the geometry must not toggle");
    }

    /// A press outside the control must not arm the latch for a later release.
    #[test]
    fn switch_press_outside_then_release_inside_does_not_toggle() {
        let mut sw = Switch::new(Rect::new(0, 0, 60, 30));
        sw.handle_event(&Event::MousePress { pos: Point::new(5000, 5000), button: 1 });
        sw.handle_event(&Event::MouseRelease { pos: Point::new(10, 10), button: 1 });
        assert!(!sw.is_checked(), "a drag that began outside must not commit");
    }

    /// Pressing inside and releasing outside cancels, as it does for `Button`.
    #[test]
    fn switch_press_inside_then_release_outside_cancels() {
        let mut sw = Switch::new(Rect::new(0, 0, 60, 30));
        sw.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
        sw.handle_event(&Event::MouseRelease { pos: Point::new(500, 500), button: 1 });
        assert!(!sw.is_checked());
        // The latch must also be clear, so the *next* stray release does not fire.
        sw.handle_event(&Event::MouseRelease { pos: Point::new(10, 10), button: 1 });
        assert!(!sw.is_checked(), "the cancelled press must not stay armed");
    }

    /// Losing focus abandons a held press.
    #[test]
    fn switch_focus_loss_cancels_a_held_press() {
        let mut sw = Switch::new(Rect::new(0, 0, 60, 30));
        sw.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
        assert!(sw.is_pressed());
        sw.handle_event(&Event::FocusLost);
        assert!(!sw.is_pressed());
        sw.handle_event(&Event::MouseRelease { pos: Point::new(10, 10), button: 1 });
        assert!(!sw.is_checked());
    }

    /// The space bar toggles, matching `CheckBox`.
    #[test]
    fn switch_space_bar_toggles() {
        let mut sw = Switch::new(Rect::new(0, 0, 60, 30));
        sw.handle_event(&Event::KeyPress { key: 32, modifiers: 0 });
        assert!(sw.is_checked());
    }

    #[test]
    fn switch_disabled_blocks_events() {
        let mut sw = Switch::new(Rect::new(0, 0, 60, 30));
        sw.set_enabled(false);
        sw.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
        assert!(!sw.is_checked());
    }

    #[cfg(not(alloc_frugal))]
    #[test]
    fn switch_svg_output() {
        let mut sw = Switch::new(Rect::new(0, 0, 60, 30));
        let svg = crate::widget::svg::render_to_svg(&mut sw);
        assert!(svg.starts_with("<svg"));
    }

    /// The logical state and the drawn state are **two different facts**.
    ///
    /// `checked` is what a caller reads after a click and must change the instant the
    /// click lands; the thumb's position is a presentation that lags behind it. Drawing
    /// the thumb from `checked` is the defect this pins against: the thumb jumped to the
    /// far end on the same frame as the state change, so the travel could never be seen
    /// and the "animation" was really a two-state image.
    #[test]
    fn the_logical_state_changes_before_the_thumb_arrives() {
        let mut sw = Switch::new(crate::widget::census::CENSUS_RECT);
        assert_eq!(sw.travel_progress(), 0.0, "a fresh switch is at the off end");

        sw.toggle();

        // The logical answer is immediate…
        assert!(sw.is_checked(), "the caller must see the new state at once");
        // …while the thumb has not moved yet, because no frame has been delivered.
        assert_eq!(
            sw.travel_progress(),
            0.0,
            "the thumb must not teleport to the far end on the frame the state changed"
        );
    }

    /// Ticking moves the thumb toward the logical state and then **settles**.
    ///
    /// The boolean return is the contract a host schedules frames from: `true` while
    /// there is movement, `false` once there is none. A `tick` that always reported work
    /// would keep the whole application repainting forever, so the settle case is half of
    /// what this pins. A single long frame must arrive exactly, not approach forever.
    #[test]
    fn ticking_moves_the_thumb_toward_the_logical_state_and_settles() {
        let mut sw = Switch::new(crate::widget::census::CENSUS_RECT);
        sw.toggle();

        // A short frame leaves the travel in flight and reports more work owed.
        assert!(sw.tick(30), "a toggle must start a transition");
        let partway = sw.travel_progress();
        assert!(partway > 0.0 && partway < 1.0, "the thumb is mid-travel: {partway}");

        // Ticking well past the theme's slow tempo lands it exactly…
        while sw.tick(1000) {}
        assert_eq!(sw.travel_progress(), 1.0, "the travel reaches its target exactly");
        // …and then reports that there is nothing left to do.
        assert!(!sw.tick(1000), "a settled switch owes no more frames");

        // The reverse journey settles the same way.
        sw.toggle();
        while sw.tick(1000) {}
        assert_eq!(sw.travel_progress(), 0.0);
        assert!(!sw.tick(1000));
    }

    /// A freshly built switch must be at the **off end** of its travel, not the on end.
    ///
    /// A transition that started at the interactive end of its range would make every new
    /// switch fade *out* on its first frame — and, since the OFF track is drawn from the
    /// travel, it would flash the ON colour before settling grey.
    #[test]
    fn a_freshly_built_switch_is_at_the_off_end_of_its_travel() {
        let sw = Switch::new(crate::widget::census::CENSUS_RECT);
        assert!(!sw.is_checked());
        assert_eq!(sw.travel_progress(), 0.0);
    }

    /// A reversal mid-flight **re-aims from where the thumb is**, it does not restart.
    ///
    /// The target is recomputed from the control's own state on every tick, so toggling
    /// back while the thumb is still crossing turns it around from its current position.
    /// Re-aiming is what makes a quick on-off read as one movement; restarting from the
    /// far end would make the same gesture read as two fades.
    #[test]
    fn a_transition_in_flight_re_aims_rather_than_restarting() {
        let mut sw = Switch::new(crate::widget::census::CENSUS_RECT);
        sw.toggle();
        sw.tick(50);
        let forward = sw.travel_progress();
        assert!(forward > 0.0 && forward < 1.0, "the toggle is in flight: {forward}");

        // Reverse while the thumb is still travelling toward the on end.
        sw.toggle();
        assert!(!sw.is_checked());
        // The reverse journey must start from where the thumb is, so the first frame of it
        // is still *above* zero rather than having snapped back to the off end.
        sw.tick(1);
        let reversing = sw.travel_progress();
        assert!(
            reversing < forward,
            "the thumb turns around instead of continuing: {reversing} < {forward}"
        );
        assert!(reversing > 0.0, "the reversal starts from the current position: {reversing}");

        // And it settles back at the off end rather than oscillating.
        while sw.tick(1000) {}
        assert_eq!(sw.travel_progress(), 0.0);
    }

    /// Visual focus depends on the **reason** focus arrived, not merely on having it.
    ///
    /// A pointer press focuses the control — it is the keyboard's target from then on —
    /// but drawing a ring under the cursor reads as a stuck highlight, so only the
    /// keyboard reasons draw one. Qt Quick's rule, and the reason the control stores a
    /// `FocusReason` rather than a bare bool.
    #[test]
    fn switch_visual_focus_depends_on_the_reason() {
        let mut sw = Switch::new(crate::widget::census::CENSUS_RECT);

        sw.handle_event(&Event::FocusGained { reason: FocusReason::Pointer });
        assert!(sw.is_focused(), "the control is the keyboard target");
        assert!(!sw.visual_focus(), "but the user is on the pointer, so no ring is drawn");

        for reason in [FocusReason::Tab, FocusReason::BackTab, FocusReason::Shortcut] {
            let mut sw = Switch::new(crate::widget::census::CENSUS_RECT);
            sw.handle_event(&Event::FocusGained { reason });
            assert!(sw.is_focused());
            assert!(sw.visual_focus(), "{reason:?} must draw the ring");
        }

        sw.handle_event(&Event::FocusLost);
        assert!(!sw.is_focused());
        assert!(!sw.visual_focus());
    }

    /// Hover is tracked from the pointer entering and leaving the control.
    ///
    /// The control had no hover state at all, so a switch under the cursor looked
    /// identical to one that was not — the affordance was simply missing. Both directions
    /// are pinned: entering sets the flag and leaving clears it.
    #[test]
    fn switch_tracks_hover() {
        let mut sw = Switch::new(crate::widget::census::CENSUS_RECT);
        assert!(!sw.is_hovered(), "a fresh switch is not hovered");

        sw.handle_event(&Event::MouseEnter { pos: Point::new(10, 10) });
        assert!(sw.is_hovered());

        sw.handle_event(&Event::MouseLeave { pos: Point::new(10, 10) });
        assert!(!sw.is_hovered());

        // The host-facing setter is the same fact, for a backend with no hover event.
        sw.set_hovered(true);
        assert!(sw.is_hovered());
    }
}
