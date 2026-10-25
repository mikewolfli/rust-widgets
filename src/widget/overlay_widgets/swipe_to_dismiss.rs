// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! SwipeToDismiss — swipe-to-dismiss/delete gesture widget.
//!
//! Wraps a child widget that can be swiped left/right to reveal an action
//! background (e.g., red "Delete"). When the swipe passes the threshold,
//! the widget emits the `dismissed` signal.

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::style::{MotionSlot, PropertyDriver};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::role_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Combined trait for widgets that can both be managed and drawn.
pub trait WidgetDraw: Widget + Draw {}
impl<T: Widget + Draw> WidgetDraw for T {}

/// SwipeToDismiss widget — swipe gesture to reveal actions and dismiss.
///
/// Wraps a child widget. The user can drag left/right to reveal an action
/// background behind the child. Releasing past the dismiss threshold emits
/// the `dismissed` signal.
pub struct SwipeToDismiss {
    base: BaseWidget,
    child: Option<Box<dyn WidgetDraw>>,
    /// Distance in pixels the user must swipe to trigger dismissal.
    dismiss_threshold: f32,
    /// Current horizontal swipe offset in pixels.
    swipe_offset: f32,
    /// X position where the active drag began, in the widget's coordinate space.
    /// `None` when no left-button drag is in progress.
    drag_origin_x: Option<f32>,
    /// Whether the widget has been dismissed (one-shot).
    is_dismissed: bool,
    /// How far the row has travelled **out of the viewport** after a dismiss was committed,
    /// `0.0` (still in place) to `1.0` (fully gone).
    ///
    /// # Why a dismiss needs a transition at all
    ///
    /// `dismiss` used to set `is_dismissed` and zero the offset in one statement, so a row that a
    /// user had dragged most of the way across the screen **snapped back to its seat and vanished**
    /// in the same frame. A gesture whose whole meaning is "this thing left" ended with it appearing
    /// to be restored first — the opposite of what the user just did.
    ///
    /// The transition drives the row the rest of the way out from wherever the gesture released it,
    /// which is what every platform's swipe-to-delete does, and it costs nothing while at rest:
    /// progress `0.0` is exactly the un-dismissed picture, so every existing snapshot is unchanged.
    exit: crate::style::PropertyDriver,
    /// Text displayed in the action background (e.g., "Delete").
    action_text: String,
    /// Label shown when the row is swiped toward its trailing edge (the default direction).
    leading_action_text: Option<String>,
    /// Label shown when the row is swiped toward its leading edge.
    trailing_action_text: Option<String>,
    /// The spring-back of a failed swipe, in pixels.
    ///
    /// `swipe_offset` is the gesture's live measurement; this driver carries the residual the
    /// row was released at, so a swipe below the threshold returns over frames instead of
    /// snapping to zero in one. At rest it is exactly zero, which is what keeps the settled
    /// picture unchanged.
    settle: PropertyDriver,
    /// Estimated pointer velocity of the current swipe, in pixels per millisecond.
    velocity: f32,
    /// The previous pointer x during a drag, used to estimate velocity.
    last_x: Option<f32>,
    /// Emitted when the item is dismissed.
    pub dismissed: Signal1<()>,
}

impl SwipeToDismiss {
    /// Creates a new SwipeToDismiss widget with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        let base = BaseWidget::new(WidgetKind::SwipeToDismiss, geometry, "SwipeToDismiss");
        Self {
            base,
            child: None,
            dismiss_threshold: 100.0,
            swipe_offset: 0.0,
            drag_origin_x: None,
            is_dismissed: false,
            exit: crate::style::PropertyDriver::default(),
            action_text: "Delete".to_string(),
            leading_action_text: None,
            trailing_action_text: None,
            settle: PropertyDriver::at(0.0, MotionSlot::Normal),
            velocity: 0.0,
            last_x: None,
            dismissed: Signal1::new(),
        }
    }

    /// Sets the child widget to be wrapped.
    pub fn set_child(&mut self, widget: Box<dyn WidgetDraw>) {
        self.child = Some(widget);
        // Theme and lay the child once here rather than on every draw: the wrapper owns the
        // child's presentation, so a settled frame must not rewrite it (G-A12).
        if let Some(child) = self.child.as_deref_mut().map(|child| child as &mut dyn Widget) {
            crate::theme::apply_theme_to_widget(child);
            child.set_geometry(self.base.geometry());
        }
        self.base.request_redraw();
    }

    /// Returns a shared reference to the child widget, if any.
    pub fn child(&self) -> Option<&dyn Widget> {
        self.child.as_deref().map(|c| c as &dyn Widget)
    }

    /// Returns a mutable reference to the child widget, if any.
    pub fn child_mut(&mut self) -> Option<&mut dyn Widget> {
        self.child.as_deref_mut().map(|c| c as &mut dyn Widget)
    }

    /// Sets the dismiss threshold in pixels.
    pub fn set_dismiss_threshold(&mut self, threshold: f32) {
        self.dismiss_threshold = threshold;
    }

    /// Returns the dismiss threshold.
    pub fn dismiss_threshold(&self) -> f32 {
        self.dismiss_threshold
    }

    /// Returns the current swipe offset.
    pub fn swipe_offset(&self) -> f32 {
        self.swipe_offset
    }

    /// Returns whether the widget has been dismissed.
    pub fn is_dismissed(&self) -> bool {
        self.is_dismissed
    }

    /// Sets the action text displayed behind the child.
    pub fn set_action_text(&mut self, text: &str) {
        if self.action_text == text {
            return;
        }
        self.action_text = text.to_string();
        self.base.request_redraw();
    }

    /// Returns the action text.
    pub fn action_text(&self) -> &str {
        &self.action_text
    }

    /// Sets the label revealed by a swipe toward the leading edge.
    pub fn set_leading_action_text(&mut self, text: Option<impl Into<String>>) {
        let text = text.map(Into::into);
        if self.leading_action_text == text {
            return;
        }
        self.leading_action_text = text;
        self.base.request_redraw();
    }

    /// Sets the label revealed by a swipe toward the trailing edge.
    pub fn set_trailing_action_text(&mut self, text: Option<impl Into<String>>) {
        let text = text.map(Into::into);
        if self.trailing_action_text == text {
            return;
        }
        self.trailing_action_text = text;
        self.base.request_redraw();
    }

    /// The label for the background revealed by `offset`, falling back to the shared text.
    fn action_label(&self, offset: f32) -> &str {
        let dedicated = if offset < 0.0 {
            self.leading_action_text.as_deref()
        } else {
            self.trailing_action_text.as_deref()
        };
        dedicated.unwrap_or(&self.action_text)
    }

    /// Resets the swipe offset (e.g., when dismissing fails or is undone).
    pub fn reset_swipe(&mut self) {
        self.swipe_offset = 0.0;
        self.is_dismissed = false;
        self.settle.jump_to(0.0);
        self.velocity = 0.0;
        // The same reasoning as `dismiss`: undoing the decision must also un-aim the departure, or
        // `is_animating()` would keep reporting a journey the control is no longer making.
        self.exit.set_target(self.exit_target());
        self.base.request_redraw();
    }

    /// Programmatically triggers the dismiss.
    ///
    /// The row does **not** jump: it keeps the offset the gesture left it at and slides the rest of
    /// the way out over the theme's `normal` tempo. `dismissed` is emitted at once — the *decision*
    /// is immediate, only the departure is animated — because a subscriber that removes the row
    /// from its model must not have to wait for a frame loop to hear about it.
    ///
    /// # Why this aims the driver rather than leaving it to `tick`
    ///
    /// The decision to leave is made **here**, so the fact "the row owes frames" has to be true
    /// here too — `is_animating()` is what a frame loop consults *before* it decides to tick
    /// anything, so a control that only becomes animating inside its own `tick` is never ticked at
    /// all. `PropertyDriver` stores the target, which means this is the one statement that states
    /// it: `set_target` is the same fact `tick` aims at, so the two cannot drift (BLUE24 §2.3).
    pub fn dismiss(&mut self) {
        if !self.is_dismissed {
            self.is_dismissed = true;
            self.exit.set_target(self.exit_target());
            self.dismissed.emit(());
            self.base.request_redraw();
        }
    }

    /// Advances the departure by `delta_ms`, reporting whether another frame is needed.
    ///
    /// The same contract as every other self-animating control here: the duration is the active
    /// theme's `Motion::normal` rather than a constant in this file, so a theme can state its own
    /// tempo and a test can drive it to the end deterministically.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        let settling = self.settle.tick(delta_ms);
        self.exit.set_target(self.exit_target());
        let exiting = self.exit.tick(delta_ms);
        settling || exiting
    }

    /// The progress a departure should be travelling toward: `1.0` once dismissed, else `0.0`.
    fn exit_target(&self) -> f32 {
        if self.is_dismissed {
            1.0
        } else {
            0.0
        }
    }

    /// Where the row actually sits on screen, in logical pixels.
    ///
    /// # Why this is a method rather than the raw field
    ///
    /// `swipe_offset` is the **gesture's** position and stays exactly as the drag left it; the
    /// departure is carried by [`Self::exit`] on top of it. Keeping the two apart is what lets
    /// `reset_swipe` undo a gesture without having to reason about a half-finished exit, and it is
    /// why the drawn position is a derived value rather than a second mutable field that the two
    /// writers would have to keep in step.
    /// Commits a released swipe whose measured offset is `offset`.
    ///
    /// A drag past the distance threshold **or** a fast flick commits the dismissal; anything
    /// else aims the residual offset at zero and lets [`Self::tick`] spring it back. A release
    /// that never moved more than two pixels is a tap, so the child receives it — which is what
    /// makes a button inside the row clickable again.
    fn commit_swipe_offset(&mut self, offset: f32, tap: Option<&Event>) {
        let flick_floor = (self.dismiss_threshold * 0.25).clamp(1.0, 40.0);
        let flicked = self.velocity.abs() > 0.5 && offset.abs() >= flick_floor;
        let committed = offset.abs() >= self.dismiss_threshold || flicked;
        // The drawn offset starts where the gesture left it and then moves on: either the
        // departure (when committed) or the spring-back (when not).
        self.settle.jump_to(offset);
        self.swipe_offset = 0.0;
        if committed {
            self.is_dismissed = true;
            self.exit.set_target(1.0);
            self.dismissed.emit(());
        } else {
            self.settle.set_target(0.0);
        }
        if offset.abs() < 2.0 {
            if let Some(event) = tap {
                if let Some(child) = &mut self.child {
                    child.handle_event(event);
                }
            }
        }
        self.velocity = 0.0;
        self.base.request_redraw();
    }

    /// Advances an in-flight swipe to pointer `x`, with the shared damping and velocity
    /// estimate.
    ///
    /// Mouse and touch used to have separate bodies: the mouse path damped past the threshold
    /// and recorded velocity, while the touch path assigned the raw offset and never updated
    /// `last_x`, so a touch flick could never commit and an over-drag had no resistance (OV-06).
    /// Both inputs now run this one derivation.
    fn drag_to(&mut self, x: f32) {
        let Some(origin) = self.drag_origin_x else {
            return;
        };
        // Velocity is estimated against a nominal 16 ms frame: the platform event carries no
        // timestamp, and the exact wall-clock rate does not matter for the "was this a flick"
        // decision.
        if let Some(last) = self.last_x {
            self.velocity = (x - last) / 16.0;
        }
        self.last_x = Some(x);
        let raw = x - origin;
        // Rubber-band damping past the threshold: a swipe may not drag the row arbitrarily far,
        // and the extra distance past the commit point costs more than the distance before it.
        let magnitude = raw.abs();
        self.swipe_offset = if magnitude <= self.dismiss_threshold {
            raw
        } else {
            let sign = raw.signum();
            sign * (self.dismiss_threshold + (magnitude - self.dismiss_threshold) * 0.35)
        };
        self.base.request_redraw();
    }

    fn drawn_offset(&self) -> f32 {
        let gesture = self.swipe_offset + self.settle.value();
        let exit = self.exit.value();
        if exit <= 0.0 {
            return gesture;
        }
        // Always to the **left**, which is the direction both platform conventions use for
        // "remove", and the side a leftward drag was already heading. A programmatic dismiss has no
        // gesture direction to continue, so deriving one from a zero offset would be inventing a
        // preference the caller never expressed.
        let travel = self.base.geometry().width as f32;
        gesture - travel * exit
    }
}

impl Widget for SwipeToDismiss {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
        // The child is a separately owned control, so a theme change has to reach it too. Doing
        // it here (the theme's own invalidation hook) replaces the per-draw re-application
        // (G-A12).
        if let Some(child) = self.child.as_deref_mut().map(|child| child as &mut dyn Widget) {
            crate::theme::apply_theme_to_widget(child);
        }
    }

    /// Lifts the control's own `tick` onto the trait, so the animation bus can reach the
    /// departure through `&mut dyn Widget`. One line, and without it the slide is unreachable.
    fn tick(&mut self, delta_ms: u32) -> bool {
        SwipeToDismiss::tick(self, delta_ms)
    }

    fn is_animating(&self) -> bool {
        // The driver holds the target the tick aims at, so "is `is_dismissed` reflected yet" is
        // one question asked in one place -- the field that used to be compared against a
        // separately derived target is gone (BLUE24 §2.3).
        self.exit.is_moving() || self.settle.is_moving()
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    /// Lays the child out with the wrapper, so a draw never has to rewrite its geometry.
    fn set_geometry(&mut self, geometry: Rect) {
        self.base.set_geometry(geometry);
        if let Some(child) = self.child.as_deref_mut().map(|child| child as &mut dyn Widget) {
            child.set_geometry(geometry);
        }
    }

    /// Themes the child whenever the **wrapper's own** style is written.
    ///
    /// # Why the theme layer needed a second hook
    ///
    /// `theme::apply` writes only the top-level control's style: it calls `set_style`, not
    /// `set_state_theme_hook`. The child this wrapper owns is not mounted in the runtime registry
    /// either, so a global `reapply_active_theme()` never reaches it. The per-draw
    /// `apply_theme_to_widget(child)` used to hide both facts; when G-A12 moved the theming to
    /// `set_child`, an appearance switch left the child on its constructor's ink and the wrapper
    /// rendered identically in light and dark (the rendering census caught it as P3). Writing the
    /// child's theme here closes the gap at the one method the theme layer writes through, and
    /// costs nothing per frame.
    fn set_style(&mut self, style: crate::style::WidgetStyle) {
        self.base.set_style(style);
        if let Some(child) = self.child.as_deref_mut().map(|child| child as &mut dyn Widget) {
            crate::theme::apply_theme_to_widget(child);
        }
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(
            crate::widget::metrics::dimensions::SWIPE_TO_DISMISS_DEFAULT_WIDTH,
            crate::widget::metrics::dimensions::SWIPE_TO_DISMISS_DEFAULT_HEIGHT,
        )
    }

    fn kind(&self) -> WidgetKind {
        WidgetKind::SwipeToDismiss
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why the field is named per arm
    ///
    /// The published name and the Rust field name are not always the same (`find_next` is backed by
    /// `find_next_signal`, `dismissed` by a `Signal1<()>` field). `connect_event` validates a name
    /// against the capability table and registers a hub slot; only `event_signal_dyn` joins that
    /// name to the signal the control actually **emits**. A wrong arm is worse than no arm, because
    /// it reports a wire as live and never fires it, so each field is named explicitly here rather
    /// than derived from the published name.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "dismissed" => Some(EventSignalRef::mapped("dismissed", &self.dismissed, |_| {
                CapabilityValue::Null
            })),
            _ => None,
        }
    }
}

/// `SwipeToDismiss`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch: reading
/// `is_dismissed` works, and writing it answers `UnsupportedOnWidget` because
/// dismissal is a one-shot gesture result rather than settable state.
impl WidgetProperties for SwipeToDismiss {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "is_dismissed" => Ok(CapabilityValue::Bool(self.is_dismissed())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            // Dismissal is owned by the gesture recogniser, not by an external
            // writer, so the name exists but refuses writes. Reporting
            // `UnsupportedOnWidget` here would claim the control has no such
            // property at all, which is not true — `get` answers it.
            "is_dismissed" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["is_dismissed", BASE_PROPERTY_NAMES]
    }
}

impl Draw for SwipeToDismiss {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        // A dismissed row is **not** hidden outright: it is still travelling. It disappears when
        // the departure reaches `1.0`, which is the frame at which it has left the viewport — so
        // the `return` is on the progress rather than on the flag, and a snapshot taken before any
        // `tick` (progress `0.0`) is byte-identical to the un-dismissed picture.
        if self.is_dismissed && self.exit.value() >= 1.0 {
            return;
        }

        // The child is already themed and laid out by `set_child` / `set_state_theme_hook`; a
        // settled frame must not rewrite either (G-A12).
        let offset = self.drawn_offset();

        // ── Action background revealed behind the child as it slides ──
        if offset.abs() > 2.0 {
            let bg_rect = if offset < 0.0 {
                // Swiping left: reveal action on the right side
                let reveal_w = (-offset) as u32;
                Rect::new(
                    rect.x + rect.width as i32 - reveal_w as i32,
                    rect.y,
                    reveal_w,
                    rect.height,
                )
            } else {
                // Swiping right: reveal action on the left side
                let reveal_w = offset as u32;
                Rect::new(rect.x, rect.y, reveal_w, rect.height)
            };

            // The action background is SEMANTIC colour, not chrome: the red *encodes* "this
            // swipe deletes", so it reads the theme's error token rather than `style.*`. The
            // literal it used to be stays as the `unwrap_or` fallback, so a build or theme
            // with no palette still gets the iOS red this control has always painted.
            let destructive = crate::style::semantic_color(crate::style::SemanticColor::Error)
                .unwrap_or(Color::rgba(255, 59, 48, 255));
            context.fill_rect(bg_rect, destructive);

            // Action text centered in revealed area. The origin is the glyph box's *top*
            // edge, so centring is half the line box: the old `+ ascent/2 - descent/2` pair
            // only approximated a baseline-relative centre and sat the label half a line low.
            if !self.action_text.is_empty() {
                let font = role_font(self.base.style(), 16.0);
                let label = self.action_label(offset);
                let metrics =
                    crate::widget::base_widgets::measure_cache::measure_text(context, label, &font);
                let text_x = bg_rect.x + (bg_rect.width as i32 - metrics.width as i32) / 2;
                let text_y = bg_rect.y + (bg_rect.height as i32 - metrics.height as i32) / 2;
                context.draw_text(
                    Point::new(text_x, text_y),
                    label,
                    &font,
                    // Picked for legibility on whichever red the theme resolves, rather
                    // than assuming the light-theme red and hardcoding white.
                    destructive.contrast_color(),
                    HorizontalAlignment::Left,
                );
            }
        }

        // ── Draw the child widget offset by the swipe amount ──
        if let Some(child) = &mut self.child {
            // The child's own geometry is its resting place, set once at `set_child` and by
            // `set_geometry`; the swipe is a context offset, so the frame writes no geometry
            // and needs no restore (G-A12).
            context.push_offset(offset as i32, 0);
            child.draw(context);
            context.pop_offset();
        }

        // ── Draw a subtle shadow line at the child edge when swiped ──
        if offset.abs() > 5.0 {
            let edge_x = if offset < 0.0 {
                rect.x + rect.width as i32 + offset as i32
            } else {
                rect.x + offset as i32
            };
            context.draw_line(
                Point::new(edge_x, rect.y),
                Point::new(edge_x, rect.y + rect.height as i32),
                Color::rgba(0, 0, 0, 40),
            );
        }
    }
}

impl EventHandler for SwipeToDismiss {
    fn handle_event(&mut self, event: &Event) {
        if self.is_dismissed {
            return;
        }

        // A disabled container must not be dismissed. Clearing any in-flight drag
        // first matters: disabling mid-gesture used to leave `drag_origin_x` set, so
        // a later `MouseMove` would still move the child content (the pointer was
        // never pressed, yet the offset changed).
        if !self.base.is_enabled() {
            self.drag_origin_x = None;
            self.swipe_offset = 0.0;
            self.base.handle_event(event);
            return;
        }

        match event {
            Event::MousePress { pos, button, .. } => {
                if *button == 1 {
                    // Remember where the drag started; the offset is the delta
                    // from here, so the child only moves as far as the pointer.
                    self.drag_origin_x = Some(pos.x as f32);
                    self.last_x = Some(pos.x as f32);
                    self.swipe_offset = 0.0;
                    self.velocity = 0.0;
                    self.settle.jump_to(0.0);
                    // The child sees the press straight away. A swipe that never moves is a
                    // tap, and the release below completes it; without this the wrapper
                    // swallowed every press and a button inside the row was dead.
                    if let Some(child) = &mut self.child {
                        child.handle_event(event);
                    }
                }
            }
            #[cfg(feature = "touch")]
            Event::TouchBegin { pos, .. } => {
                // Touch and mouse share the drag path: tablets/mobile report
                // touches, desktops report mouse, and the gesture is identical.
                self.drag_origin_x = Some(pos.x as f32);
                self.last_x = Some(pos.x as f32);
                self.swipe_offset = 0.0;
                self.velocity = 0.0;
                self.settle.jump_to(0.0);
            }
            Event::MouseMove { pos } => {
                self.drag_to(pos.x as f32);
            }
            #[cfg(feature = "touch")]
            Event::TouchMove { pos, .. } => {
                // The touch path shares the mouse path's damping and velocity estimate (OV-06).
                self.drag_to(pos.x as f32);
            }
            Event::MouseRelease { pos: _, button } => {
                if *button == 1 {
                    let offset = self.swipe_offset;
                    self.drag_origin_x = None;
                    self.last_x = None;
                    self.commit_swipe_offset(offset, Some(event));
                }
            }
            #[cfg(feature = "touch")]
            Event::TouchEnd { .. } => {
                let offset = self.swipe_offset;
                self.drag_origin_x = None;
                self.last_x = None;
                self.commit_swipe_offset(offset, None);
            }
            // Delegate remaining events to child
            evt => {
                if let Some(child) = &mut self.child {
                    child.handle_event(evt);
                } else {
                    self.base.handle_event(evt);
                }
            }
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
    use crate::render::RenderContext;
    use crate::widget::svg::render_to_svg;
    use std::sync::Arc;

    /// A minimal test child widget used for SwipeToDismiss tests.
    struct TestChild {
        base: BaseWidget,
        draw_called: std::sync::Arc<std::sync::atomic::AtomicBool>,
    }

    impl TestChild {
        fn new(geometry: Rect, flag: std::sync::Arc<std::sync::atomic::AtomicBool>) -> Self {
            Self {
                base: BaseWidget::new(WidgetKind::Label, geometry, "TestChild"),
                draw_called: flag,
            }
        }
    }

    impl Widget for TestChild {
        fn base(&self) -> &BaseWidget {
            &self.base
        }
        fn set_state_theme_hook(&mut self) {
            crate::style::reapply_active_theme_state(self);
        }
        fn base_mut(&mut self) -> &mut BaseWidget {
            &mut self.base
        }
    }

    impl Draw for TestChild {
        fn draw(&mut self, context: &mut RenderContext) {
            self.draw_called.store(true, std::sync::atomic::Ordering::SeqCst);
            context.fill_rect(self.geometry(), Color::WHITE);
        }
    }

    impl EventHandler for TestChild {
        fn handle_event(&mut self, event: &Event) {
            self.base.handle_event(event);
        }
    }

    #[test]
    fn swipe_to_dismiss_creation() {
        let sw = SwipeToDismiss::new(Rect::new(0, 0, 200, 50));
        assert_eq!(sw.kind(), WidgetKind::SwipeToDismiss);
        assert!(!sw.is_dismissed());
        assert_eq!(sw.dismiss_threshold(), 100.0);
        assert_eq!(sw.action_text(), "Delete");
    }

    #[test]
    fn swipe_to_dismiss_action_text() {
        let mut sw = SwipeToDismiss::new(Rect::new(0, 0, 200, 50));
        assert_eq!(sw.action_text(), "Delete");

        sw.set_action_text("Archive");
        assert_eq!(sw.action_text(), "Archive");
    }

    #[test]
    fn swipe_to_dismiss_dismiss_threshold() {
        let mut sw = SwipeToDismiss::new(Rect::new(0, 0, 200, 50));
        sw.set_dismiss_threshold(80.0);
        assert_eq!(sw.dismiss_threshold(), 80.0);
    }

    #[test]
    fn swipe_to_dismiss_set_child_and_draw() {
        let draw_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let child = TestChild::new(Rect::new(0, 0, 200, 50), draw_flag.clone());

        let mut sw = SwipeToDismiss::new(Rect::new(0, 0, 200, 50));
        sw.set_child(Box::new(child));
        assert!(sw.child().is_some());

        let svg = render_to_svg(&mut sw);
        assert!(svg.starts_with("<svg"));
        assert!(draw_flag.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn swipe_to_dismiss_dismiss_programmatic() {
        let mut sw = SwipeToDismiss::new(Rect::new(0, 0, 200, 50));

        let fired = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let f = fired.clone();
        sw.dismissed.connect(move |_: Arc<()>| {
            f.store(true, std::sync::atomic::Ordering::SeqCst);
        });

        sw.dismiss();
        assert!(sw.is_dismissed());
        assert!(fired.load(std::sync::atomic::Ordering::SeqCst));
    }

    /// BLUE23 Appendix A.2 / M3: a dismiss **slides out** instead of vanishing in one frame.
    ///
    /// # The defect this pins
    ///
    /// `dismiss` set `is_dismissed` and zeroed `swipe_offset` in the same statement, so a row the
    /// user had dragged most of the way across the screen **snapped back to its seat and
    /// disappeared** in one frame. A gesture whose whole meaning is "this thing left" ended by
    /// appearing to restore it first — the opposite of what the user just did. The `dismissed`
    /// signal still fires immediately, because the *decision* is a fact the model needs at once;
    /// only the departure is animated.
    ///
    /// The three frames are read out of the **emitted document**, not off the progress field: a
    /// mutation that stopped *using* the transition in `draw` would leave a progress-only assertion
    /// green — the trap `toggle_button`'s frame test documents.
    #[test]
    #[cfg(device_profile)]
    fn a_dismiss_slides_out_across_three_frames() {
        let _guard = crate::style::theme_test_guard();
        crate::widget::census::install_preset_appearances();
        let rect = Rect::new(0, 0, 200, 50);
        let mut sw = SwipeToDismiss::new(rect);
        // Mount something to watch: the row's travel is read off the child's own ink.
        sw.set_child(Box::new(crate::widget::base_widgets::label::Label::new(
            "Row".to_string(),
            rect,
        )));

        // At rest the control owes no frames (a settled control is free).
        assert!(!sw.is_animating(), "a resting row must not ask for frames");
        let seated = child_x(&mut sw, rect);

        sw.dismiss();
        // Animating at once, before any tick — reading the tick-time field instead of the state
        // would answer `false` here and the bus would never start.
        assert!(sw.is_animating(), "a dismissed row owes frames immediately");
        // Frame 1: the row has not moved yet, which is why adding this left every snapshot
        // byte-identical.
        assert_eq!(child_x(&mut sw, rect), seated, "the first frame is the row in place");

        // Frames 2..n: it moves, and leftward — off the edge its gesture was heading for.
        let mut travelled = seated;
        for _ in 0..64 {
            if !sw.tick(16) {
                break;
            }
            let now = child_x(&mut sw, rect);
            if now != seated {
                travelled = now;
                break;
            }
        }
        assert!(
            travelled < seated,
            "the row must travel leftward: from x={seated} to x={travelled}"
        );

        // Frame n: it settles, stops asking for frames, and has left the viewport — which is the
        // point at which `draw` stops emitting anything at all.
        let mut guard = 64;
        while guard > 0 && sw.tick(1000) {
            guard -= 1;
        }
        assert!(guard > 0, "the departure must terminate, not ask for frames forever");
        assert!(!sw.is_animating(), "a settled row must stop asking for frames");
        let svg = crate::widget::svg::render_widget_to_svg(&mut sw, rect);
        assert!(
            !svg.contains("d=\"M"),
            "a fully departed row paints nothing; the document was {svg}"
        );
    }

    /// The **leftmost** ink origin the document paints, which is how the row's travel is read.
    ///
    /// # Why the minimum of every subpath, and not the first one
    ///
    /// A glyph run is emitted as one `<path>` holding one subpath per ink region, so the row's ink
    /// is the union of all of them and its left edge is the least `M`-origin in the document. Taking
    /// the *first* origin (the first draft of this helper) read `0` for a seated row and a larger
    /// number once it moved — because the first subpath is not the leftmost once the run is offset.
    /// The minimum is stable under that and is exactly "where the row is".
    ///
    /// # Why the origin is read as a float
    ///
    /// The bitmap face writes whole-pixel origins (`M8 8h...`), but an **outline** face writes
    /// fractional ones (`M18.30 59.24L...`) because sub-pixel precision is the point of an outline.
    /// The first form parsed the origin as an `i32`, so every outline origin failed to parse, the
    /// minimum stayed empty, and the caller got the "nothing painted" sentinel for a row that was
    /// plainly on screen — the `x=-2147483648` the test reported. Reading a float and truncating
    /// keeps the integer comparison the caller wants while accepting both spellings.
    ///
    /// Reading the drawing rather than `swipe_offset` is what makes the frame assertions properties
    /// of the picture: a mutation that stopped *using* the transition in `draw` would leave a
    /// progress-only assertion green.
    #[cfg(device_profile)]
    fn child_x(sw: &mut SwipeToDismiss, rect: Rect) -> i32 {
        let svg = crate::widget::svg::render_widget_to_svg(sw, rect);
        let mut leftmost: Option<i32> = None;
        let mut rest = svg.as_str();
        while let Some(i) = rest.find('M') {
            let window = &rest[i + 1..];
            // A subpath origin is `M<x> <y>` — the following character must be a digit, `-` or `.`,
            // which excludes the `M`s inside attribute names and the `xmlns` URI. A leading `.` is
            // possible for an origin that rounds to a value below one, so it is accepted too.
            if window.starts_with(|c: char| c.is_ascii_digit() || c == '-' || c == '.') {
                let end = window.find(' ').unwrap_or(window.len());
                if let Ok(x) = window[..end].trim().parse::<f32>() {
                    let x = x.floor() as i32;
                    leftmost = Some(leftmost.map_or(x, |l: i32| l.min(x)));
                }
            }
            rest = &rest[i + 1..];
        }
        // Nothing painted: the row has left. A sentinel rather than zero, so a comparison still
        // distinguishes "gone" from "at the origin" instead of reading both as `0`.
        leftmost.unwrap_or(i32::MIN)
    }

    #[test]
    fn a_swipe_below_the_threshold_springs_back_over_frames() {
        let rect = Rect::new(0, 0, 200, 50);
        let mut sw = SwipeToDismiss::new(rect);
        sw.handle_event(&Event::MousePress { pos: Point::new(180, 25), button: 1, modifiers: 0 });
        sw.handle_event(&Event::MouseMove { pos: Point::new(160, 25) });
        assert!(sw.swipe_offset() < 0.0);
        sw.handle_event(&Event::MouseRelease { pos: Point::new(160, 25), button: 1 });
        assert!(!sw.is_dismissed());
        assert!(sw.is_animating(), "the failed swipe returns over frames");
        while sw.tick(16) {}
        assert!(!sw.is_animating());
    }

    #[test]
    fn a_fast_short_flick_still_dismisses() {
        let rect = Rect::new(0, 0, 200, 50);
        let mut sw = SwipeToDismiss::new(rect);
        sw.set_dismiss_threshold(300.0);
        sw.handle_event(&Event::MousePress { pos: Point::new(180, 25), button: 1, modifiers: 0 });
        // A large move in one frame is a flick even though the distance is under the threshold.
        sw.handle_event(&Event::MouseMove { pos: Point::new(120, 25) });
        sw.handle_event(&Event::MouseRelease { pos: Point::new(120, 25), button: 1 });
        assert!(sw.is_dismissed(), "velocity commits a flick the distance alone would not");
    }

    #[test]
    fn a_direction_can_carry_its_own_action_label() {
        let mut sw = SwipeToDismiss::new(Rect::new(0, 0, 200, 50));
        sw.set_leading_action_text(Some("Archive"));
        sw.set_trailing_action_text(Some("Delete"));
        sw.set_action_text("Fallback");
        assert_eq!(sw.action_label(-10.0), "Archive");
        assert_eq!(sw.action_label(10.0), "Delete");
        assert_eq!(sw.action_text(), "Fallback");
    }

    #[cfg(feature = "touch")]
    #[test]
    fn a_touch_swipe_shares_the_mouse_damping_and_flick_rule() {
        let mut sw = SwipeToDismiss::new(Rect::new(0, 0, 200, 50));
        sw.set_dismiss_threshold(100.0);
        sw.handle_event(&Event::TouchBegin { pos: Point::new(180, 25), touch_id: 0 });
        sw.handle_event(&Event::TouchMove { pos: Point::new(60, 25), touch_id: 0 });
        assert!(
            sw.swipe_offset() < -100.0,
            "a touch past the threshold is damped, not raw: {}",
            sw.swipe_offset()
        );
        sw.handle_event(&Event::TouchEnd { pos: Point::new(60, 25), touch_id: 0 });
        assert!(sw.is_dismissed(), "the touch path records velocity, so a flick commits");
    }
}
