// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! SwipeToDismiss — swipe-to-dismiss/delete gesture widget.
//!
//! Wraps a child widget that can be swiped left/right to reveal an action
//! background (e.g., red "Delete"). When the swipe passes the threshold,
//! the widget emits the `dismissed` signal.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
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
            dismissed: Signal1::new(),
        }
    }

    /// Sets the child widget to be wrapped.
    pub fn set_child(&mut self, widget: Box<dyn WidgetDraw>) {
        self.child = Some(widget);
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
        self.action_text = text.to_string();
        self.base.request_redraw();
    }

    /// Returns the action text.
    pub fn action_text(&self) -> &str {
        &self.action_text
    }

    /// Resets the swipe offset (e.g., when dismissing fails or is undone).
    pub fn reset_swipe(&mut self) {
        self.swipe_offset = 0.0;
        self.is_dismissed = false;
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
        self.exit.set_target(self.exit_target());
        self.exit.tick(delta_ms)
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
    fn drawn_offset(&self) -> f32 {
        let gesture = self.swipe_offset;
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

    /// Lifts the control's own `tick` onto the trait, so the animation bus can reach the
    /// departure through `&mut dyn Widget`. One line, and without it the slide is unreachable.
    fn tick(&mut self, delta_ms: u32) -> bool {
        SwipeToDismiss::tick(self, delta_ms)
    }

    fn is_animating(&self) -> bool {
        // The driver holds the target the tick aims at, so "is `is_dismissed` reflected yet" is
        // one question asked in one place -- the field that used to be compared against a
        // separately derived target is gone (BLUE24 §2.3).
        self.exit.is_moving()
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(400, 60)
    }

    fn kind(&self) -> WidgetKind {
        WidgetKind::SwipeToDismiss
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
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

        // The child is mounted by this control, so this control is the one that has to
        // theme it. The census themes the top-level control it creates and nothing
        // below it (`render_one` draws the tree exactly as `Widget::draw` walks it), and
        // the factory that builds this wrapper does not call `apply_active_theme`
        // either — so a bare child label kept its own hardcoded ink and the wrapper's
        // dominant colour was that literal in both appearances. Ordering it on every
        // draw rather than only at `set_child` is the stronger guarantee: a child
        // swapped in by a later `set_child` is themed too, and the stylesheet layer
        // still wins because `merge_theme` only replaces what the theme itself wrote.
        if let Some(child) = self.child.as_deref_mut().map(|c| c as &mut dyn Widget) {
            crate::theme::apply_theme_to_widget(child);
        }

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
                let font = Font::new("sans-serif", 16.0, false, false);
                let metrics = context.measure_text(&self.action_text, &font);
                let text_x = bg_rect.x + (bg_rect.width as i32 - metrics.width as i32) / 2;
                let text_y = bg_rect.y + (bg_rect.height as i32 - metrics.height as i32) / 2;
                context.draw_text(
                    Point::new(text_x, text_y),
                    &self.action_text,
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
            // Save the original child geometry, offset it, draw, then restore
            let original_geom = child.geometry();
            let offset_x = offset as i32;
            let translated_rect = Rect::new(rect.x + offset_x, rect.y, rect.width, rect.height);
            child.set_geometry(translated_rect);
            child.draw(context);
            child.set_geometry(original_geom);
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
            Event::MousePress { pos, button } => {
                if *button == 1 {
                    // Remember where the drag started; the offset is the delta
                    // from here, so the child only moves as far as the pointer.
                    self.drag_origin_x = Some(pos.x as f32);
                    self.swipe_offset = 0.0;
                }
            }
            #[cfg(feature = "touch")]
            Event::TouchBegin { pos, .. } => {
                // Touch and mouse share the drag path: tablets/mobile report
                // touches, desktops report mouse, and the gesture is identical.
                self.drag_origin_x = Some(pos.x as f32);
                self.swipe_offset = 0.0;
            }
            Event::MouseMove { pos } => {
                if let Some(origin) = self.drag_origin_x {
                    self.swipe_offset = pos.x as f32 - origin;
                    self.base.request_redraw();
                }
            }
            #[cfg(feature = "touch")]
            Event::TouchMove { pos, .. } => {
                if let Some(origin) = self.drag_origin_x {
                    self.swipe_offset = pos.x as f32 - origin;
                    self.base.request_redraw();
                }
            }
            Event::MouseRelease { pos: _, button } => {
                if *button == 1 {
                    self.drag_origin_x = None;
                    if self.swipe_offset.abs() >= self.dismiss_threshold {
                        self.is_dismissed = true;
                        self.dismissed.emit(());
                    }
                    self.swipe_offset = 0.0;
                    self.base.request_redraw();
                }
            }
            #[cfg(feature = "touch")]
            Event::TouchEnd { .. } => {
                self.drag_origin_x = None;
                if self.swipe_offset.abs() >= self.dismiss_threshold {
                    self.is_dismissed = true;
                    self.dismissed.emit(());
                }
                self.swipe_offset = 0.0;
                self.base.request_redraw();
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

    /// BLUE23 附录 A.2 / M3: a dismiss **slides out** instead of vanishing in one frame.
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
        let _guard = crate::theme::theme_test_guard();
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
    /// A glyph run is emitted as one `<path>` holding an axis-aligned subpath per ink bit, so the
    /// row's ink is the union of all of them and its left edge is the least `M`-origin in the
    /// document. Taking the *first* origin (the first draft of this helper) read `0` for a seated
    /// row and a larger number once it moved — because the first subpath is not the leftmost once
    /// the run is offset. The minimum is stable under that and is exactly "where the row is".
    ///
    /// Reading the drawing rather than `swipe_offset` is what makes the frame assertions properties
    /// of the picture: a mutation that stopped *using* the transition in `draw` would leave a
    /// progress-only assertion green.
    #[cfg(device_profile)]
    fn child_x(sw: &mut SwipeToDismiss, rect: Rect) -> i32 {
        let svg = crate::widget::svg::render_widget_to_svg(sw, rect);
        let mut leftmost: Option<i32> = None;
        let mut rest = svg.as_str();
        while let Some(i) = rest.find("M") {
            let window = &rest[i + 1..];
            // A subpath origin is `M<int> ` — the following character must be a digit or `-`, which
            // excludes the `M`s inside attribute names and the `xmlns` URI.
            if window.starts_with(|c: char| c.is_ascii_digit() || c == '-') {
                let end = window.find(' ').unwrap_or(window.len());
                if let Ok(x) = window[..end].trim().parse::<i32>() {
                    leftmost = Some(leftmost.map_or(x, |l: i32| l.min(x)));
                }
            }
            rest = &rest[i + 1..];
        }
        // Nothing painted: the row has left. A sentinel rather than zero, so a comparison still
        // distinguishes "gone" from "at the origin" instead of reading both as `0`.
        leftmost.unwrap_or(i32::MIN)
    }
}
