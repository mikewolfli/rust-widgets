// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Material Design controls that share the Cupertino module's registration.
//!
//! These two controls are Material widgets by design language, not Cupertino ones. They used
//! to live in `core.rs`, which is the iOS collection's file — a reader looking for the iOS
//! switch found a snackbar and a navigation rail beside it. Moving the definitions here makes
//! each module's name describe its contents; the public paths are re-exported by
//! `cupertino::mod`, so no caller's import changes.

use crate::compat::VecDeque;
use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::style::{MotionSlot, PropertyDriver};
use crate::widget::capability::coercion::{expect_string, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::role_font;
use crate::widget::{BaseWidget, Draw, IconItemError, IconSource, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

// ── MaterialSnackbar ─────────────────────────────────────────────────────────

/// The most messages a snackbar holds behind the visible one by default.
const SNACKBAR_MAX_QUEUED: usize = 4;

/// One snackbar's content, captured so a host that sets the next message before calling
/// `show` cannot overwrite the message that is still on screen.
#[derive(Clone)]
struct SnackbarContent {
    message: String,
    action_text: String,
    duration_ms: u64,
    elapsed_ms: u64,
}

/// Material Design snackbar notification.
///
/// A snackbar displays a brief message at the bottom of the screen with an
/// optional action button. It appears temporarily and can be dismissed by
/// the user or programmatically.
pub struct MaterialSnackbar {
    base: BaseWidget,
    message: String,
    action_text: String,
    /// Emitted when the action button is pressed.
    pub action_pressed: GenericSignal,
    /// Emitted when the snackbar is dismissed.
    pub dismissed: GenericSignal,
    /// How long the snackbar stays on screen, in milliseconds; `0` means it stays until dismissed.
    duration_ms: u64,
    /// Milliseconds the current presentation has been visible.
    elapsed_ms: u64,
    /// Entrance/exit slide fraction, `0.0` off screen and `1.0` fully presented.
    slide: PropertyDriver,
    /// Whether an animated dismissal is in progress.
    closing: bool,
    /// The action button's rectangle from the last paint, so the hit test uses the same
    /// text measurement the ink does (MW-15).
    action_rect: Option<Rect>,
    /// Whether the press landed on the action button and has not been released yet.
    pressed_action: bool,
    /// Whether a `show` issued while a message is presented waits its turn.
    queueing: bool,
    /// The number of messages that may wait behind the visible one.
    max_queued: usize,
    /// Messages waiting behind the visible one, oldest first.
    queue: VecDeque<SnackbarContent>,
    /// The content the visible bar was last presented with, so `show` can restore it when the
    /// host has already set the next message.
    presented: Option<SnackbarContent>,
}

impl MaterialSnackbar {
    /// Creates a new MaterialSnackbar with the given geometry.
    ///
    /// The snackbar starts hidden. Use [`show()`](Self::show) to display it.
    pub fn new(geometry: Rect) -> Self {
        let mut base = BaseWidget::new(WidgetKind::MaterialSnackbar, geometry, "MaterialSnackbar");
        base.hide();
        Self {
            base,
            message: String::new(),
            action_text: String::new(),
            action_pressed: GenericSignal::new(),
            dismissed: GenericSignal::new(),
            duration_ms: 4_000,
            elapsed_ms: 0,
            slide: PropertyDriver::at(0.0, MotionSlot::Normal),
            closing: false,
            action_rect: None,
            pressed_action: false,
            queueing: false,
            max_queued: SNACKBAR_MAX_QUEUED,
            queue: VecDeque::new(),
            presented: None,
        }
    }

    /// Sets the message text displayed in the snackbar.
    pub fn set_message(&mut self, message: &str) {
        if self.message == message {
            return;
        }
        self.message = message.to_string();
        self.base.request_redraw();
    }

    /// Returns the current message text.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Sets the action button text. Pass an empty string to hide the action button.
    pub fn set_action_text(&mut self, text: &str) {
        if self.action_text == text {
            return;
        }
        self.action_text = text.to_string();
        self.action_rect = None;
        self.pressed_action = false;
        self.base.request_redraw();
    }

    /// Returns the current action button text.
    pub fn action_text(&self) -> &str {
        &self.action_text
    }

    /// Shows the snackbar. Does nothing if already visible.
    ///
    /// With [`Self::set_queueing`] on, a `show` issued while a message is presented queues its
    /// content behind the visible one instead of racing it: the bar then displays the messages
    /// one at a time, oldest first. Queueing is off by default, which keeps the historical
    /// behaviour — the visible bar simply keeps showing its current text.
    pub fn show(&mut self) {
        if self.base.is_visible() {
            if self.queueing {
                self.enqueue_current();
            } else {
                // The bar is already up, so whatever the setters just wrote is what it shows.
                self.presented = Some(self.content());
            }
            return;
        }
        self.present(self.content());
    }

    /// The current message and action as one value.
    fn content(&self) -> SnackbarContent {
        SnackbarContent {
            message: self.message.clone(),
            action_text: self.action_text.clone(),
            duration_ms: self.duration_ms,
            elapsed_ms: self.elapsed_ms,
        }
    }

    /// Puts `content` on screen, sliding in from below when the bar was hidden.
    fn present(&mut self, content: SnackbarContent) {
        self.message = content.message.clone();
        self.action_text = content.action_text.clone();
        self.duration_ms = content.duration_ms;
        self.elapsed_ms = 0;
        self.action_rect = None;
        self.pressed_action = false;
        self.presented = Some(content);
        self.base.show();
        self.closing = false;
        // Slide in from below: start off screen and animate to the presented position.
        self.slide.jump_to(0.0);
        self.slide.set_target(1.0);
        self.base.request_redraw();
    }

    /// Queues the text a host has just set, restoring the message still on screen first.
    ///
    /// A host spells a later message as `set_message(...)` followed by `show`, so by the time
    /// the queue sees the call the visible fields already hold the new text. The last presented
    /// content is kept for exactly this case: without restoring it, the bar would show the
    /// queued message early and then show it again when its turn came.
    fn enqueue_current(&mut self) {
        if self.max_queued == 0 {
            return;
        }
        let incoming = SnackbarContent {
            message: self.message.clone(),
            action_text: self.action_text.clone(),
            duration_ms: self.duration_ms,
            elapsed_ms: 0,
        };
        if let Some(presented) = self.presented.clone() {
            self.message = presented.message;
            self.action_text = presented.action_text;
            self.duration_ms = presented.duration_ms;
            self.elapsed_ms = presented.elapsed_ms;
            self.action_rect = None;
            self.pressed_action = false;
            self.base.request_redraw();
        }
        // Bounded: the oldest waiting message is dropped, so a producer that outpaces the
        // display cannot grow the queue without limit. A message that never appeared has not
        // been dismissed, so no `dismissed` is emitted for it.
        while self.queue.len() >= self.max_queued {
            self.queue.pop_front();
        }
        self.queue.push_back(incoming);
    }

    /// Presents the next queued message, if there is one. Returns whether one was presented.
    fn present_next_queued(&mut self) -> bool {
        let Some(next) = self.queue.pop_front() else {
            return false;
        };
        self.present(next);
        true
    }

    /// Dismisses the snackbar immediately. Emits the `dismissed` signal.
    ///
    /// `dismissed` is deliberately not gated by `enabled`: it reports the snackbar leaving
    /// the screen (a timeout or a host decision), not a user action, and a host that
    /// disabled the snackbar's interactions still needs to know it went away. Hiding the
    /// snackbar does block the signal, which is the condition `dismiss` actually depends on.
    pub fn dismiss(&mut self) {
        if self.base.is_visible() {
            self.base.hide();
            self.closing = false;
            self.slide.jump_to(0.0);
            self.dismissed.emit();
            // A queued message takes the screen as soon as the visible one leaves, which is what
            // makes the queue serial rather than an overwrite.
            self.present_next_queued();
            self.base.request_redraw();
        }
    }

    /// Whether `show` queues a message behind the visible one instead of racing it.
    pub fn is_queueing(&self) -> bool {
        self.queueing
    }

    /// Enables or disables queueing of messages shown while another is visible.
    ///
    /// Off by default, which is the historical behaviour: a second `show` while the bar is up
    /// changes nothing but the visible text. Turning it off clears anything already waiting, so
    /// a picture taken after the setter cannot show a message that will never be presented.
    pub fn set_queueing(&mut self, queueing: bool) {
        if self.queueing == queueing {
            return;
        }
        self.queueing = queueing;
        if !queueing {
            self.queue.clear();
        }
    }

    /// The number of messages waiting behind the visible one.
    pub fn queued_count(&self) -> usize {
        self.queue.len()
    }

    /// The cap on queued messages.
    pub fn max_queued(&self) -> usize {
        self.max_queued
    }

    /// Caps the queue; when it is full the oldest waiting message is dropped.
    pub fn set_max_queued(&mut self, max_queued: usize) {
        let max_queued = max_queued.min(64);
        if self.max_queued == max_queued {
            return;
        }
        self.max_queued = max_queued;
        while self.queue.len() > self.max_queued {
            self.queue.pop_front();
        }
    }

    /// Starts an animated dismissal: the bar slides out, then hides and emits `dismissed`.
    ///
    /// [`Self::dismiss`] stays immediate because a host that tears the control down must not be
    /// left with a visible bar waiting for a frame. This variant is for the user-visible exit.
    pub fn dismiss_animated(&mut self) {
        if self.base.is_visible() && !self.closing {
            self.closing = true;
            self.slide.set_target(0.0);
            self.base.request_redraw();
        }
    }

    /// How long the snackbar stays on screen in milliseconds; `0` means it never auto-dismisses.
    pub fn duration_ms(&self) -> u64 {
        self.duration_ms
    }

    /// Sets the auto-dismiss delay. Zero keeps the bar until the user or host dismisses it.
    pub fn set_duration_ms(&mut self, duration_ms: u64) {
        if self.duration_ms == duration_ms {
            return;
        }
        self.duration_ms = duration_ms;
        self.elapsed_ms = 0;
    }

    /// The entrance/exit slide fraction, `0.0` off screen and `1.0` presented.
    pub fn slide_progress(&self) -> f32 {
        self.slide.value()
    }

    /// Returns whether the snackbar is currently visible.
    pub fn is_visible(&self) -> bool {
        self.base.is_visible()
    }
}

impl Widget for MaterialSnackbar {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(
            crate::widget::metrics::dimensions::CUPERTINO_BAR_DEFAULT_WIDTH,
            crate::widget::metrics::dimensions::CUPERTINO_BAR_DEFAULT_HEIGHT,
        )
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();

    /// Drives the auto-dismiss countdown and the slide animation.
    fn tick(&mut self, delta_ms: u32) -> bool {
        let mut changed = false;
        if self.base.is_visible() && !self.closing && self.duration_ms > 0 {
            self.elapsed_ms = self.elapsed_ms.saturating_add(u64::from(delta_ms));
            if self.elapsed_ms >= self.duration_ms {
                self.dismiss();
                return true;
            }
            changed = true;
        }
        let moving = self.slide.tick(delta_ms);
        changed |= moving;
        if self.closing && !self.slide.is_moving() && self.slide.value() <= 0.001 {
            self.base.hide();
            self.closing = false;
            self.dismissed.emit();
            // The exit is what frees the screen, so the next queued message slides in here.
            self.present_next_queued();
            changed = true;
        }
        if changed {
            self.base.request_redraw();
        }
        changed
    }

    /// A running countdown owes a timeout frame, and a slide in flight owes an animation frame.
    fn is_animating(&self) -> bool {
        self.slide.is_moving() || (self.base.is_visible() && !self.closing && self.duration_ms > 0)
    }

    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually **emits**. Without an
    /// arm a published name is valid and inert, which is the silent failure
    /// `tools/check_event_signal_dyn.sh` exists to make impossible: the arm set is compared against
    /// the capability's published names, so the two cannot drift.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "action_pressed" => Some(EventSignalRef::unit("action_pressed", &self.action_pressed)),
            "dismissed" => Some(EventSignalRef::unit("dismissed", &self.dismissed)),
            _ => None,
        }
    }
}

/// `MaterialSnackbar`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. Both properties now report
/// the snackbar's real text instead of the placeholder defaults that dispatch
/// returned.
impl WidgetProperties for MaterialSnackbar {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "message" => Ok(CapabilityValue::String(self.message().to_string())),
            "action_text" => Ok(CapabilityValue::String(self.action_text().to_string())),
            "duration_ms" => Ok(CapabilityValue::UInt(self.duration_ms())),
            "slide_progress" => Ok(CapabilityValue::Float(self.slide_progress() as f64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "message" => {
                self.set_message(&expect_string(value)?);
                Ok(())
            }
            "action_text" => {
                self.set_action_text(&expect_string(value)?);
                Ok(())
            }
            "duration_ms" => {
                let duration = match value {
                    CapabilityValue::UInt(ms) => ms,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_duration_ms(duration);
                Ok(())
            }
            "slide_progress" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "message",
            "action_text",
            "duration_ms",
            "slide_progress",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `material_snackbar` publishes.
    ///
    /// Both `show` and `dismiss` map onto the widget's real methods and take no
    /// payload. `set_message` and `set_action_text` carry the text the caller
    /// wants displayed, so a bare invocation is reported as needing one rather
    /// than being called unknown.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "show" => {
                self.show();
                Ok(())
            }
            "dismiss" => {
                self.dismiss();
                Ok(())
            }
            "set_message" | "set_action_text" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for MaterialSnackbar {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        // Chrome colours resolve the explicit style first, then the theme's resolved style for
        // this control, and only then a literal. The pill used to be a fixed near-black and the
        // action a fixed Material blue, so a light/dark switch left the snackbar unchanged — the
        // rendering census reported the control as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's mutex
        // is not re-entrant.
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("material_snackbar");
        // Read from the shared palette snapshot, which is revision-cached and applies the
        // crossfade/high-contrast resolution the draw should use, so this does not take the
        // manager lock.
        let (window_fill, foreground, secondary, primary) = {
            match crate::style::palette_shared() {
                Some(colors) => {
                    (colors.background, colors.foreground, colors.secondary, colors.primary)
                }
                None => (
                    Color::rgb(240, 240, 240),
                    Color::BLACK,
                    Color::rgb(158, 158, 158),
                    Color::rgb(33, 150, 243),
                ),
            }
        };
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(foreground);
        // `material_snackbar` is absent from `WidgetRole::for_kind_name`'s table, so it classifies
        // as `Surface` and the active theme writes the window fill into `style.background_color`.
        // The pill is eased away from the window fill in both directions — darker on the light
        // theme, lighter on the dark one — so it never coincides with the frame behind it, while a
        // colour the caller set still wins.
        let pill = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.72),
        };
        // The label is drawn in whatever contrasts with the pill, rather than a fixed white that
        // only read against the old near-black bar.
        let label = pill.contrast_color();
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != pill)
            .unwrap_or_else(|| pill.blend(&secondary, 0.45));

        let bar_height = 48u32;
        // The entrance/exit slide moves the bar off its resting slot; at rest the fraction is 1.0 and
        // the bar is exactly where it always was. A hidden snackbar paints its resting slot (at
        // reduced presence, below), not the off-screen end of the slide: the census draws every
        // control inside its own box, and an off-screen pill would otherwise escape it.
        let slide = if self.base.is_visible() { self.slide.value().clamp(0.0, 1.0) } else { 1.0 };
        let bar_y = rect.y + rect.height as i32 - bar_height as i32 - 16
            + ((1.0 - slide) * (bar_height + 16) as f32) as i32;

        // ── Background pill ──
        //
        // Painted before the visibility check. It used to `return` while hidden, so a freshly
        // constructed snackbar — which starts hidden by contract — painted nothing and the census
        // reported `ink = 0`. The bar slot is shown at reduced presence while hidden and becomes the
        // opaque toast when `show` is called, which is the same shape the bottom sheet uses for its
        // own closed state.
        let bar_rect = Rect::new(rect.x + 12, bar_y, rect.width.saturating_sub(24), bar_height);
        let fill = if self.base.is_visible() { pill } else { pill.with_alpha(96) };
        context.fill_rounded_rect(bar_rect, bar_height / 2, fill);
        context.draw_rounded_rect_stroke(bar_rect, bar_height / 2, border, 1);

        if !self.base.is_visible() {
            return;
        }

        // ── Message text (left side) ──
        if !self.message.is_empty() {
            // The message takes the **effective font** at its role size, so the theme family and the
            // user's text scale reach it (D09-STYLE-01).
            let font = role_font(style, 14.0);
            let metrics = context.measure_text(&self.message, &font);

            let text_x = bar_rect.x + 16;
            // Centre on the bar. The origin is the glyph box's top edge, so the offset is half
            // the *line box*; the old ascent/descent pair claimed to centre but sat half a
            // line low.
            let text_y = bar_rect.y + (bar_height as i32 - metrics.height as i32) / 2;

            context.draw_text(
                crate::core::Point::new(text_x, text_y),
                &self.message,
                &font,
                label,
                HorizontalAlignment::Left,
            );
        }

        // ── Action text (right side, if non-empty) ──
        if !self.action_text.is_empty() {
            let action_font = role_font(style, 14.0).with_bold(true);
            let metrics = context.measure_text(&self.action_text, &action_font);

            let action_x = bar_rect.x + bar_rect.width as i32 - metrics.width as i32 - 16;
            // The hit region is the measured ink plus padding, cached for the event path.
            // The old estimate (0.6 x point size x byte count) disagreed with this by tens
            // of pixels on mixed-case text, so clicks missed the label they aimed at.
            self.action_rect =
                Some(Rect::new(action_x - 8, bar_rect.y, metrics.width + 16, bar_height));
            // Same top-edge origin as the message: centre on the line box, not on the
            // ascent/descent midpoint, which sat the action half a line low.
            let action_y = bar_rect.y + (bar_height as i32 - metrics.height as i32) / 2;

            context.draw_text(
                crate::core::Point::new(action_x, action_y),
                &self.action_text,
                &action_font,
                primary,
                HorizontalAlignment::Left,
            );
        } else {
            self.action_rect = None;
        }
    }
}

impl EventHandler for MaterialSnackbar {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_visible() {
            return;
        }
        // A disabled control stays out of play even while visible: without this the action
        // button still fired for a snackbar the host had disabled.
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button, .. } => {
                if *button != 1 {
                    return;
                }
                let rect = self.geometry();
                let bar_height = 48u32;
                let bar_y = rect.y + rect.height as i32 - bar_height as i32 - 16;
                let bar_rect =
                    Rect::new(rect.x + 12, bar_y, rect.width.saturating_sub(24), bar_height);
                if !bar_rect.contains_point(*pos) {
                    return;
                }
                // The action button is the rectangle the paint path cached; before the first
                // paint it is derived from the same estimated advance the layout uses (MW-15).
                // A press only latches the state; the signal fires on release exactly once
                // (L-P10).
                let action = self.action_rect.unwrap_or_else(|| {
                    let font = role_font(self.style(), 14.0).with_bold(true);
                    let width =
                        crate::widget::metrics::estimate_text_width(&self.action_text, &font, 1.0);
                    Rect::new(
                        bar_rect.right() - width as i32 - 24,
                        bar_rect.y,
                        width + 16,
                        bar_height,
                    )
                });
                self.pressed_action = action.contains_point(*pos);
                // Remember the derived rectangle so the release hit-tests the same area the
                // press latched.
                self.action_rect = Some(action);
                if !self.pressed_action {
                    // Click anywhere else on the snackbar dismisses it.
                    self.dismiss();
                }
            }
            Event::MouseRelease { pos, button } => {
                if *button != 1 {
                    return;
                }
                // A press that has dragged off the action cancels it, which is the standard
                // desktop button contract.
                let within = self.action_rect.is_some_and(|action| action.contains_point(*pos));
                if self.pressed_action && within {
                    self.action_pressed.emit();
                    self.base.request_redraw();
                }
                self.pressed_action = false;
            }
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

#[cfg(test)]
mod snackbar_tests {
    use super::*;

    #[test]
    fn a_snackbar_dismisses_after_its_duration() {
        let mut snackbar = MaterialSnackbar::new(Rect::new(0, 0, 375, 160));
        snackbar.set_message("Saved");
        assert_eq!(snackbar.duration_ms(), 4_000);
        snackbar.show();
        assert!(snackbar.is_visible());
        assert!(snackbar.is_animating(), "a visible snackbar owes its timeout frame");
        assert!((snackbar.slide_progress() - 0.0).abs() < f32::EPSILON);

        assert!(snackbar.tick(16), "the slide moved");
        assert!(snackbar.slide_progress() > 0.0);

        let dismissed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let d = dismissed.clone();
        snackbar.dismissed.connect(move || {
            d.store(true, std::sync::atomic::Ordering::SeqCst);
        });
        assert!(snackbar.tick(5_000));
        assert!(!snackbar.is_visible(), "the timeout hides the bar");
        assert!(dismissed.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn zero_duration_is_permanent_and_the_exit_slides_out() {
        let mut snackbar = MaterialSnackbar::new(Rect::new(0, 0, 375, 160));
        snackbar.set_duration_ms(0);
        snackbar.show();
        for _ in 0..100 {
            snackbar.tick(100);
        }
        assert!(snackbar.is_visible(), "duration 0 must not auto-dismiss");
        assert!((snackbar.slide_progress() - 1.0).abs() < 1e-3);

        snackbar.dismiss_animated();
        assert!(snackbar.is_visible(), "the exit starts while still on screen");
        for _ in 0..100 {
            snackbar.tick(16);
        }
        assert!(!snackbar.is_visible());
        assert_eq!(snackbar.slide_progress(), 0.0);
    }
}

// ── MaterialNavigationRail ─────────────────────────────────────────────────────

/// A single item in a MaterialNavigationRail.
#[derive(Clone)]
pub struct RailItem {
    /// Icon glyph or icon-name text. A text stand-in, not image data.
    pub icon: String,
    /// Text label shown beneath (or beside) the icon.
    pub label: String,
    /// An unread/notification count drawn as a trailing capsule.
    pub badge: Option<u32>,
}

impl RailItem {
    /// Creates a new rail item with the given icon and label.
    pub fn new(icon: &str, label: &str) -> Self {
        Self { icon: icon.to_string(), label: label.to_string(), badge: None }
    }

    /// The same item with a badge count.
    pub fn with_badge(mut self, badge: u32) -> Self {
        self.badge = Some(badge);
        self
    }
}

/// How a navigation-rail destination paints its caption.
///
/// Material's rail is defined with the icon alone; the label variant is the shape this control
/// has always drawn, so it stays the default and no existing picture changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NavigationRailLabelMode {
    /// The icon with its caption beneath it, for every destination.
    #[default]
    IconAndLabel,
    /// The icon alone, centred in the destination band. The item's label is kept, so a host and
    /// an accessibility reader still have the destination's name.
    IconOnly,
}

/// The widest the rail's own strip is drawn.
///
/// A navigation rail is a fixed-width edge strip (Material's spec is 80 dp), so the
/// control's rectangle is the *room offered*, not the rail's width. Painting the whole
/// rectangle made the rail a full-width panel and left its items against the far-left edge.
const RAIL_MAX_WIDTH: u32 = 80;
/// The vertical pitch of one rail destination.
///
/// Named once because the paint and the hit test both step by it; two literals would let
/// the click target drift from the destination it describes.
const RAIL_ITEM_HEIGHT: u32 = 72;

/// The height reserved for an optional header widget above the rail's items.
const RAIL_HEADER_HEIGHT: u32 = 56;

/// Material Design navigation rail for tablets (BLUE11 R10.22).
///
/// A vertically stacked set of destinations with one selected at a time. The
/// selected index always names an item: it defaults to `0` even while the rail
/// is empty, so it is out of range until the first item is added.
pub struct MaterialNavigationRail {
    base: BaseWidget,
    items: Vec<RailItem>,
    item_icon_sources: Vec<Option<IconSource>>,
    selected_index: usize,
    /// Emitted with the newly selected index whenever the selection changes.
    /// One-based ordering is not implied; indices are positions in
    /// `MaterialNavigationRail::items()`.
    pub current_changed: Signal1<usize>,
    /// An optional header widget (an avatar, a fab, a search box) drawn above the items.
    header_widget: Option<crate::compat::Box<dyn Widget>>,
    /// Whether each destination paints its caption.
    label_mode: NavigationRailLabelMode,
}

impl MaterialNavigationRail {
    /// Creates a new MaterialNavigationRail with the given geometry.
    pub fn new(geometry: crate::core::Rect) -> Self {
        let base =
            BaseWidget::new(WidgetKind::MaterialNavigationRail, geometry, "MaterialNavigationRail");
        Self {
            base,
            items: Vec::new(),
            item_icon_sources: Vec::new(),
            selected_index: 0,
            current_changed: Signal1::new(),
            header_widget: None,
            label_mode: NavigationRailLabelMode::IconAndLabel,
        }
    }

    /// Compatibility spelling of [`Self::current_changed`] (W-20): the same emitted signal.
    pub fn selected_changed(&self) -> &Signal1<usize> {
        &self.current_changed
    }

    /// Adds an item with the given icon and label.
    pub fn add_item(&mut self, icon: &str, label: &str) {
        self.items.push(RailItem::new(icon, label));
        self.item_icon_sources.push(None);
        self.base.request_redraw();
    }

    /// Assigns a typed icon source to an existing rail item index.
    pub fn set_item_icon_source(
        &mut self,
        index: usize,
        source: Option<IconSource>,
    ) -> Result<(), IconItemError> {
        let len = self.items.len();
        let Some(slot) = self.item_icon_sources.get_mut(index) else {
            return Err(IconItemError::IndexOutOfBounds { index, len });
        };
        *slot = source;
        self.base.request_redraw();
        Ok(())
    }

    /// Sets the selected index. Clamped to valid range.
    ///
    /// `current_changed` is deliberately not gated by `enabled`: selection here is driven
    /// by the host (navigation state), and the disabled control already ignores presses, so
    /// silencing the programmatic path would only desynchronise the host's state.
    pub fn set_selected(&mut self, index: usize) {
        if self.items.is_empty() {
            self.selected_index = 0;
            return;
        }
        let clamped = index.min(self.items.len() - 1);
        if clamped != self.selected_index {
            self.selected_index = clamped;
            self.current_changed.emit(clamped);
            self.base.request_redraw();
        }
    }

    /// Returns the currently selected index.
    pub fn selected(&self) -> usize {
        self.selected_index
    }

    /// Returns the number of items.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Removes all items.
    pub fn clear_items(&mut self) {
        self.items.clear();
        self.item_icon_sources.clear();
        self.selected_index = 0;
        self.base.request_redraw();
    }

    /// Removes the item at `index`, returning whether it existed.
    ///
    /// The selection follows the list: removing a row before it moves the selection up one,
    /// removing the selected row keeps the numeric position (which now names the next item), and
    /// an emptied list resets to zero.
    pub fn remove_item(&mut self, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }
        self.items.remove(index);
        self.item_icon_sources.remove(index);
        if self.items.is_empty() {
            self.selected_index = 0;
        } else if index < self.selected_index {
            self.selected_index -= 1;
        } else {
            self.selected_index = self.selected_index.min(self.items.len() - 1);
        }
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    /// Replaces one item's caption, returning whether the index exists.
    pub fn set_item_label(&mut self, index: usize, label: &str) -> bool {
        let Some(item) = self.items.get_mut(index) else {
            return false;
        };
        if item.label == label {
            return true;
        }
        item.label = label.to_string();
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    /// Replaces one item's icon text/name, returning whether the index exists.
    pub fn set_item_icon(&mut self, index: usize, icon: &str) -> bool {
        let Some(item) = self.items.get_mut(index) else {
            return false;
        };
        if item.icon == icon {
            return true;
        }
        item.icon = icon.to_string();
        self.base.request_redraw();
        true
    }

    /// Sets one item's badge, returning whether the index exists.
    pub fn set_item_badge(&mut self, index: usize, badge: Option<u32>) -> bool {
        let Some(item) = self.items.get_mut(index) else {
            return false;
        };
        if item.badge == badge {
            return true;
        }
        item.badge = badge;
        self.base.request_redraw();
        true
    }

    /// Installs an optional header widget drawn above the first item.
    ///
    /// The widget is painted in a band of its own height at the top of the rail; the item
    /// positions and hit tests shift below it. Passing `None` removes the slot.
    pub fn set_header_widget(&mut self, widget: Option<crate::compat::Box<dyn Widget>>) {
        let rect = self.geometry();
        let width = rect.width.min(RAIL_MAX_WIDTH);
        self.header_widget = widget;
        if let Some(widget) = self.header_widget.as_mut() {
            widget.set_geometry(crate::core::Rect::new(rect.x, rect.y, width, RAIL_HEADER_HEIGHT));
        }
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The header slot, when one is installed.
    pub fn header_widget_mut(&mut self) -> Option<&mut (dyn Widget + '_)> {
        self.header_widget.as_deref_mut()
    }

    /// How each destination paints its caption.
    pub fn label_mode(&self) -> NavigationRailLabelMode {
        self.label_mode
    }

    /// Sets whether destinations paint their captions.
    ///
    /// [`NavigationRailLabelMode::IconAndLabel`] is the default and the historical shape, so a
    /// rail that never calls this setter draws exactly as it did. The hit target is one whole
    /// destination band in either mode; only the paint changes.
    pub fn set_label_mode(&mut self, mode: NavigationRailLabelMode) {
        if self.label_mode == mode {
            return;
        }
        self.label_mode = mode;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The y offset the first item starts at (below the header, when present).
    fn items_top(&self) -> i32 {
        self.geometry().y + if self.header_widget.is_some() { RAIL_HEADER_HEIGHT as i32 } else { 0 }
    }
}

impl Widget for MaterialNavigationRail {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(
            crate::widget::metrics::dimensions::CUPERTINO_TAB_BAR_DEFAULT_WIDTH,
            crate::widget::metrics::dimensions::CUPERTINO_TAB_BAR_DEFAULT_HEIGHT,
        )
    }

    fn kind(&self) -> WidgetKind {
        WidgetKind::MaterialNavigationRail
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually **emits**. Without an
    /// arm a published name is valid and inert, which is the silent failure
    /// `tools/check_event_signal_dyn.sh` exists to make impossible: the arm set is compared against
    /// the capability's published names, so the two cannot drift.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "current_changed" => {
                Some(EventSignalRef::mapped("current_changed", &self.current_changed, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            _ => None,
        }
    }
}

/// `MaterialNavigationRail`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch: `selected_index`
/// is the published name for the field the `selected()` / `set_selected()`
/// accessors back.
impl WidgetProperties for MaterialNavigationRail {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "selected_index" => Ok(CapabilityValue::UInt(self.selected() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "selected_index" => {
                self.set_selected(expect_usize(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["selected_index", BASE_PROPERTY_NAMES]
    }
}

impl Draw for MaterialNavigationRail {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        // Chrome colours resolve the explicit style first, then the theme's resolved style for
        // this control, and only then a literal. The rail was a fixed white, its indicator and
        // labels a fixed Material blue and grey, so a light/dark switch left it unchanged — the
        // rendering census reported the control as theme-blind. It also `return`ed while empty, so
        // a freshly constructed rail reported `ink = 0`.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's mutex
        // is not re-entrant.
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("material_navigation_rail");
        // Read from the shared palette snapshot, which is revision-cached and applies the
        // crossfade/high-contrast resolution the draw should use, so this does not take the
        // manager lock.
        let (window_fill, foreground, secondary, primary) = {
            match crate::style::palette_shared() {
                Some(colors) => {
                    (colors.background, colors.foreground, colors.secondary, colors.primary)
                }
                None => (
                    Color::rgb(240, 240, 240),
                    Color::BLACK,
                    Color::rgb(158, 158, 158),
                    Color::rgb(33, 150, 243),
                ),
            }
        };
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(foreground);
        // `material_navigation_rail` is absent from `WidgetRole::for_kind_name`'s table, so it
        // classifies as `Surface` and the active theme writes the window fill into
        // `style.background_color`. A rail painted in that colour would be byte-identical to the
        // frame behind it, so a resolved surface equal to the window fill is re-derived a visible
        // step away from it, while a colour the caller set still wins.
        let rail = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.08),
        };
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != rail)
            .unwrap_or_else(|| rail.blend(&secondary, 0.45));

        let rail_width = rect.width.min(RAIL_MAX_WIDTH);

        // ── Rail background ──
        //
        // Painted before the empty check: a rail with nothing in it is still a rail, and hiding it
        // made the control invisible rather than empty. The strip is at most `RAIL_MAX_WIDTH` wide:
        // a navigation rail is a fixed-width edge strip, and painting the caller's whole rectangle
        // made it a full-width panel with the items stranded at its left edge.
        context.fill_rect(Rect::new(rect.x, rect.y, rail_width, rect.height), rail);
        context.draw_line(
            Point::new(rect.x + rail_width as i32 - 1, rect.y),
            Point::new(rect.x + rail_width as i32 - 1, rect.y + rect.height as i32),
            border,
        );

        if self.items.is_empty() && self.header_widget.is_none() {
            return;
        }

        // ── Optional header slot ──
        if self.header_widget.is_some() {
            let header = Rect::new(rect.x, rect.y, rail_width, RAIL_HEADER_HEIGHT.min(rect.height));
            if let Some(widget) = self.header_widget.as_mut() {
                widget.set_geometry(header);
                if let Some(drawable) = crate::widget::draw_bridge::draw_of(widget.as_mut()) {
                    drawable.draw(context);
                }
            }
            context.draw_line(
                Point::new(header.x, header.bottom() - 1),
                Point::new(header.right() - 1, header.bottom() - 1),
                border,
            );
        }

        let item_height = RAIL_ITEM_HEIGHT;
        let icon_font = role_font(style, 14.0);
        let label_font = role_font(style, 12.0);
        let items_top = self.items_top();

        for (i, item) in self.items.iter().enumerate() {
            let item_y = items_top + (i as u32 * item_height) as i32;
            let is_selected = i == self.selected_index;

            // ── Selected indicator bar ──
            if is_selected {
                context.fill_rect(
                    crate::core::Rect::new(rect.x, item_y + 8, 4, item_height.saturating_sub(16)),
                    primary,
                );
            }

            // ── Selected background tint ──
            if is_selected {
                context.fill_rounded_rect(
                    crate::core::Rect::new(
                        rect.x + 8,
                        item_y + 8,
                        rail_width.saturating_sub(16),
                        item_height.saturating_sub(16),
                    ),
                    8,
                    primary.with_alpha(25),
                );
            }

            // ── Icon ──
            //
            // A named icon (`home`, `search`, …) is resolved to the bundled outline. The text
            // path was the control's *only* icon rendering, so a rail built with icon names
            // painted the word `home` above the label `Home` — a duplicated label, not an icon.
            // The glyph fallback is kept for a caller that passes a character.
            let icon_color = if is_selected { primary } else { secondary };
            let typed_source = self.item_icon_sources.get(i).copied().flatten();
            let icon_side = 24u32;
            // An icon-only rail centres the glyph in the destination band; with a caption the
            // glyph sits above it, which is the offset the control has always used.
            let icon_top = if self.label_mode == NavigationRailLabelMode::IconOnly {
                item_y + ((item_height - icon_side) / 2) as i32
            } else {
                item_y + 16
            };
            let icon_rect = Rect::new(
                rect.x + (rail_width as i32 - icon_side as i32) / 2,
                icon_top,
                icon_side,
                icon_side,
            );
            if let Some(source) = typed_source {
                if let Err(error) =
                    crate::widget::paint_icon(context, source.into(), icon_rect, icon_color)
                {
                    log::warn!("navigation rail item {i} icon failed: {error:?}");
                }
            } else if !crate::widget::draw_icon_named(context, icon_rect, icon_color, &item.icon) {
                // A character rather than a registered name: draw it as a mark in the same box.
                context.draw_text_fitted_cow(
                    icon_rect,
                    &item.icon,
                    &icon_font,
                    icon_color,
                    HorizontalAlignment::Center,
                );
            }

            // ── Badge ──
            // A count capsule at the icon's trailing top corner, on the same role colours the
            // icon uses so it reads as part of the same destination.
            if let Some(badge) = item.badge {
                let mut text = String::new();
                {
                    use core::fmt::Write as _;
                    let _ = write!(text, "{badge}");
                }
                let metrics = context.measure_text(&text, &label_font);
                let width = (metrics.width + 8).max(14);
                let height = metrics.height.max(12);
                let badge_rect = Rect::new(
                    rect.x + (rail_width as i32 + icon_side as i32) / 2 - width as i32 + 2,
                    icon_rect.y - 6,
                    width,
                    height,
                );
                context.fill_rounded_rect(badge_rect, height / 2, primary);
                context.draw_text_fitted_cow(
                    badge_rect,
                    &text,
                    &label_font,
                    primary.contrast_color(),
                    HorizontalAlignment::Center,
                );
            }

            // ── Label ──
            // Centred in a band whose top is the label's top edge: the origin is the glyph box,
            // so `item_y + 44 + height / 2` put every label half a line low.
            if self.label_mode == NavigationRailLabelMode::IconAndLabel {
                let label_color = if is_selected { primary } else { secondary };
                let label_metrics = context.measure_text(&item.label, &label_font);
                let label_band = Rect::new(
                    rect.x + 4,
                    item_y + 44,
                    rail_width.saturating_sub(8),
                    label_metrics.height.max(1),
                );
                context.draw_text_fitted_cow(
                    label_band,
                    &item.label,
                    &label_font,
                    label_color,
                    HorizontalAlignment::Center,
                );
            }
        }
    }
}

impl EventHandler for MaterialNavigationRail {
    /// Interactions are ignored while the control is disabled.
    ///
    /// The disabled state was not consulted, so a host that disabled the control — the
    /// usual way to take a control out of play — still received clicks and value changes
    /// from it.
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button, .. } | Event::MouseRelease { pos, button } => {
                if *button != 1 {
                    return;
                }

                let rect = self.geometry();
                if self.items.is_empty() {
                    return;
                }

                let item_height = RAIL_ITEM_HEIGHT;
                let items_top = self.items_top();
                for i in 0..self.items.len() {
                    let item_y = items_top + (i as u32 * item_height) as i32;
                    let item_rect = crate::core::Rect::new(
                        rect.x,
                        item_y,
                        rect.width.min(RAIL_MAX_WIDTH),
                        item_height,
                    );
                    if item_rect.contains_point(*pos) {
                        if let Event::MouseRelease { .. } = event {
                            self.set_selected(i);
                        }
                        return;
                    }
                }
            }
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::svg::{render_to_svg, text_ink_boxes};

    fn make_snackbar() -> MaterialSnackbar {
        MaterialSnackbar::new(Rect::new(0, 0, 320, 48))
    }

    /// Queueing is off by default, which is the historical behaviour: a second `show` while the
    /// bar is up changes nothing but the text the setters already wrote.
    #[test]
    fn a_second_show_does_not_queue_by_default() {
        let mut snack = make_snackbar();
        assert!(!snack.is_queueing(), "queueing is opt-in");
        snack.set_message("first");
        snack.show();
        snack.set_message("second");
        snack.show();
        assert_eq!(snack.message(), "second");
        assert_eq!(snack.queued_count(), 0, "nothing waits behind the visible message");
        assert_eq!(snack.max_queued(), SNACKBAR_MAX_QUEUED);
    }

    /// CB-002: with queueing on, a later message waits instead of overwriting the one on screen,
    /// and the two are presented one after the other.
    #[test]
    fn queued_messages_are_presented_one_at_a_time() {
        let mut snack = make_snackbar();
        snack.set_queueing(true);
        snack.set_message("first");
        snack.show();
        snack.set_message("second");
        snack.show();

        assert_eq!(snack.message(), "first", "the presented message is not overwritten");
        assert_eq!(snack.queued_count(), 1);

        snack.dismiss();
        assert!(snack.is_visible(), "the queued message takes the screen");
        assert_eq!(snack.message(), "second");
        assert_eq!(snack.queued_count(), 0, "the queue drained in order");
    }

    /// The queue is bounded; a producer that outpaces the display drops the oldest waiting
    /// message rather than growing without limit.
    #[test]
    fn the_queue_is_bounded_and_drops_the_oldest_waiting_message() {
        let mut snack = make_snackbar();
        snack.set_queueing(true);
        snack.set_max_queued(1);
        snack.set_message("first");
        snack.show();
        snack.set_message("second");
        snack.show();
        snack.set_message("third");
        snack.show();
        assert_eq!(snack.queued_count(), 1);
        snack.dismiss();
        assert_eq!(snack.message(), "third", "the newest waiting message is kept");
    }

    /// Turning queueing off clears what was waiting, so nothing is promised that will never be
    /// presented.
    #[test]
    fn turning_queueing_off_clears_the_waiting_messages() {
        let mut snack = make_snackbar();
        snack.set_queueing(true);
        snack.set_message("first");
        snack.show();
        snack.set_message("second");
        snack.show();
        assert_eq!(snack.queued_count(), 1);
        snack.set_queueing(false);
        assert_eq!(snack.queued_count(), 0);
    }

    /// The animated exit is a dismissal too, so the next queued message slides in once the
    /// outgoing one has left.
    #[test]
    fn an_animated_exit_hands_the_screen_to_the_next_queued_message() {
        let mut snack = make_snackbar();
        snack.set_queueing(true);
        snack.set_message("first");
        snack.show();
        snack.set_message("second");
        snack.show();
        snack.dismiss_animated();
        for _ in 0..200 {
            if snack.message() == "second" {
                break;
            }
            snack.tick(16);
        }
        assert_eq!(snack.message(), "second");
        assert!(snack.is_visible(), "the queued message is presented after the exit");
    }

    /// CB-003: the label mode selects the painted shape without changing the destinations, and
    /// the default keeps the historical icon-plus-caption picture.
    ///
    /// Text leaves the SVG renderer as glyph geometry, so the caption is counted rather than
    /// searched for: the labelled rail must paint exactly one more text run per destination.
    #[test]
    fn an_icon_only_rail_paints_no_captions_and_still_selects() {
        let mut rail = MaterialNavigationRail::new(Rect::new(0, 0, 80, 400));
        rail.add_item("home", "Home");
        rail.add_item("search", "Find");
        assert_eq!(rail.label_mode(), NavigationRailLabelMode::IconAndLabel);

        let labelled = text_ink_boxes(&render_to_svg(&mut rail)).len();

        rail.set_label_mode(NavigationRailLabelMode::IconOnly);
        assert_eq!(rail.label_mode(), NavigationRailLabelMode::IconOnly);
        let icon_only = text_ink_boxes(&render_to_svg(&mut rail)).len();
        assert_eq!(
            labelled,
            icon_only + 2,
            "the only text an icon-only rail drops is the two captions"
        );
        assert_eq!(rail.item_count(), 2, "the labels are kept on the items");

        // The hit target is unchanged: a release inside a destination band still selects it.
        rail.handle_event(&Event::mouse_release(40, 100, 1));
        assert_eq!(rail.selected(), 1);
    }
}
