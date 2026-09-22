// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Switch/Toggle widget — a modern on/off binary state control.
//!
//! The Switch widget presents a sliding toggle that represents a boolean state,
//! similar to iOS UISwitch or Android Switch material widget. It supports
//! on/off toggling, animated transitions, and accessibility role mapping.

use crate::core::{Color, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::expect_bool;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Switch/Toggle widget for binary on/off state selection.
pub struct Switch {
    base: BaseWidget,
    checked: bool,
    /// `true` between a pointer press that hit this control and the release that ends
    /// it. The release is what commits the toggle, so a press that began elsewhere and
    /// merely *ends* over the switch must not flip it — see `handle_event`.
    pressed: bool,
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
}

impl Widget for Switch {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(50, 28)
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

        // Track dimensions
        //
        // The track is a **fixed-size** stadium centred inside `rect`, not a scaling of
        // it. Deriving the track from the geometry made the *control* the track: a
        // 240x120 census cell drew a 240x120 stadium with a 116x116 thumb, which is a
        // picture of a switch-shaped rectangle rather than a switch. The geometry is the
        // widget's *occupancy* — its hit area and its layout slot — and the drawn chrome
        // is a separate, fixed proportion of it. That is the same split Flutter's
        // `Switch` makes (a 52x32 track, a 14px thumb radius, 60px overall), and it is
        // why `size_hint` can return a comfortable 50x28 without the drawn shape
        // depending on the layout engine's answer.
        //
        // The values are still clamped *down* to the geometry, never up: a control laid
        // out narrower than the nominal track must not paint outside the rectangle it was
        // given, since nothing clips a widget at this layer. `rect` itself is untouched —
        // `handle_event` hit-tests against `self.geometry()`, so the full rectangle stays
        // the hit area and a press anywhere on it arms the switch.
        const TRACK_WIDTH: u32 = 52;
        const TRACK_HEIGHT: u32 = 32;
        const THUMB_INSET: u32 = 2;
        let track_width = rect.width.clamp(1, TRACK_WIDTH);
        // A stadium's end caps have radius `height / 2`, so the only shape constraint is
        // `height <= width` — clamping to `width / 2` (which is what this used to do) turned
        // the nominal 52x32 into a 52x26 and made the thumb a quarter smaller than the
        // 14px-radius disc the reference draws.
        let track_height = rect.height.clamp(1, TRACK_HEIGHT).min(track_width);
        // The thumb is a disc inset from the track's edge. `saturating_sub` keeps a
        // degenerate track from producing a negative size; the `knob_size == 0` guard
        // below then draws no thumb at all.
        let knob_size = track_height.saturating_sub(THUMB_INSET * 2);

        let track_x = rect.x + (rect.width as i32 - track_width as i32) / 2;
        let track_y = rect.y + (rect.height as i32 - track_height as i32) / 2;
        let track_rect = Rect::new(track_x, track_y, track_width, track_height);

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

        let track_color = if !is_enabled {
            caller_background.or(themed_track).unwrap_or(Color::rgba(200, 200, 200, 128))
        } else if self.checked {
            // The ON state is the control's *meaning*, and its meaning is "success": it
            // takes the semantic token rather than the theme's field grey. The literal
            // stays as the no-theme fallback.
            caller_background.or(themed_accent).unwrap_or(Color::rgba(52, 199, 89, 200))
        // iOS green
        } else {
            caller_background.or(themed_track).unwrap_or(Color::rgba(180, 180, 180, 200))
        };
        context.fill_rounded_rect(track_rect, track_height / 2, track_color);

        // Draw knob
        if knob_size == 0 {
            return;
        }
        let knob_x = if self.checked {
            track_x + track_width as i32 - knob_size as i32 - THUMB_INSET as i32
        } else {
            track_x + THUMB_INSET as i32
        };
        let knob_y = track_y + THUMB_INSET as i32;
        let knob_rect = Rect::new(knob_x, knob_y, knob_size, knob_size);

        let knob_color = if !is_enabled {
            Color::rgba(240, 240, 240, 200)
        } else {
            // The knob reads the theme's foreground (a light knob on a dark
            // surface, a dark-ish knob never — the knob is always the lighter of
            // the pair), falling back to white only when no theme is active.
            theme
                .as_ref()
                .and_then(|resolved| resolved.text_color)
                .map(|text| {
                    // Keep the knob light: mix most of the way to white so it stays
                    // the highlight against the coloured track.
                    text.blend(&Color::WHITE, 0.85)
                })
                .unwrap_or(Color::WHITE)
        };
        context.fill_rounded_rect(knob_rect, knob_size / 2, knob_color);

        // Draw knob shadow/border
        let knob_border_color = style.border_color.unwrap_or(Color::rgba(0, 0, 0, 30));
        context.draw_rounded_rect_stroke(knob_rect, knob_size / 2, knob_border_color, 1);
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
            // Focus loss abandons a held press, so the latch cannot survive a
            // window switch and fire on an unrelated later release.
            Event::FocusLost => {
                self.pressed = false;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
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
}
