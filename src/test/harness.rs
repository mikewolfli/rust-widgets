// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use crate::core::{Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::widget::Widget;
use alloc::collections::VecDeque;
/// Test harness for widget testing
///
/// Collects synthetic [`Event`]s in a queue so unit tests can script input
/// without a windowing system, then delivers them to a widget on demand. The
/// queue is FIFO, so events are dispatched in the order they were sent.
///
/// The harness itself never touches a display: it only builds and stores event
/// values, which keeps tests deterministic and platform-independent.
pub struct TestHarness {
    events: VecDeque<Event>,
    screen_size: Size,
}
impl TestHarness {
    /// Creates an empty harness for an `800 x 600` screen.
    ///
    /// The screen size defaults to `800 x 600` (in logical pixels) and is only
    /// reported by [`TestHarness::screen_size`]; it does not clip or scale the
    /// coordinates passed to the event helpers.
    pub fn new() -> Self {
        Self { events: VecDeque::new(), screen_size: Size::new(800, 600) }
    }
    /// Builder-style setter for the simulated screen size.
    ///
    /// The value is informational: harness helpers pass coordinates through
    /// unmodified, so no clamping to this size occurs.
    pub fn with_screen_size(mut self, size: Size) -> Self {
        self.screen_size = size;
        self
    }
    /// Returns the simulated screen size in logical pixels.
    pub fn screen_size(&self) -> Size {
        self.screen_size
    }
    /// Queues an already-built event for later dispatch.
    pub fn send_event(&mut self, event: Event) {
        self.events.push_back(event);
    }
    /// Queues a full click at `(x, y)`: a press followed by a release.
    ///
    /// `button` is the platform button code (`0` is the left button, as used by
    /// [`WidgetTester::click`]). Because this produces two events, it advances
    /// the queue length by two.
    pub fn send_mouse_click(&mut self, x: i32, y: i32, button: u32) {
        let point = Point::from_f32(x as f32, y as f32);
        self.send_event(Event::MousePress { pos: point, button });
        self.send_event(Event::MouseRelease { pos: point, button });
    }
    /// Queues a pointer move to `(x, y)`, with no buttons held.
    pub fn send_mouse_move(&mut self, x: i32, y: i32) {
        self.send_event(Event::MouseMove { pos: Point::from_f32(x as f32, y as f32) });
    }
    /// Queues a key press.
    ///
    /// `key` is the platform key code and `modifiers` a modifier bitmask; both
    /// are stored as given and interpreted by the widget.
    pub fn send_key_press(&mut self, key: u32, modifiers: u32) {
        self.send_event(Event::KeyPress { key, modifiers });
    }
    /// Queues a key release.
    ///
    /// `key` and `modifiers` are stored as given.
    pub fn send_key_release(&mut self, key: u32, modifiers: u32) {
        self.send_event(Event::KeyRelease { key, modifiers });
    }
    /// Removes and returns the oldest queued event, or `None` when empty.
    pub fn next_event(&mut self) -> Option<Event> {
        self.events.pop_front()
    }
    /// Returns the number of events still queued.
    pub fn event_count(&self) -> usize {
        self.events.len()
    }
    /// Discards every queued event without dispatching it.
    pub fn clear_events(&mut self) {
        self.events.clear();
    }
    /// Delivers every queued event to `widget` in order, draining the queue.
    ///
    /// Returns the number of events delivered, **not** the number the widget
    /// considered handled: [`EventHandler::handle_event`] reports nothing, so
    /// the return value always equals the queue length at the time of the call.
    /// The queue is left empty even if the widget responds to none of them.
    pub fn dispatch_to<W: EventHandler>(&mut self, widget: &mut W) -> usize {
        let mut handled = 0;
        while let Some(event) = self.next_event() {
            widget.handle_event(&event);
            handled += 1;
        }
        handled
    }
}
crate::impl_default_via_new!(TestHarness);
/// Widget tester with assertions
///
/// Pairs a widget with a [`TestHarness`] and re-dispatches queued events
/// automatically after each interaction, so a test can read as a sequence of
/// user actions followed by assertions. The interaction methods return
/// `&mut Self`, allowing the calls to be chained.
///
/// The assertion methods panic on failure (they are `assert!` wrappers meant to
/// be called directly from `#[test]` functions), and return `&Self` so they can
/// also be chained. This type is a testing aid and is not intended for
/// production code paths.
pub struct WidgetTester<W: Widget> {
    widget: W,
    harness: TestHarness,
}
impl<W: Widget> WidgetTester<W> {
    /// Takes ownership of `widget` and creates the tester with a default
    /// harness (an `800 x 600` screen and an empty event queue).
    pub fn new(widget: W) -> Self {
        Self { widget, harness: TestHarness::new() }
    }
    /// Replaces the harness, for example to supply a custom screen size.
    pub fn with_harness(mut self, harness: TestHarness) -> Self {
        self.harness = harness;
        self
    }
    /// Borrows the widget under test, for assertions the tester does not wrap.
    pub fn widget(&self) -> &W {
        &self.widget
    }
    /// Mutably borrows the widget under test, for direct manipulation.
    pub fn widget_mut(&mut self) -> &mut W {
        &mut self.widget
    }
    /// Simulates a left-button click at `(x, y)` and dispatches it.
    ///
    /// Queues both the press and the release, so the widget sees a complete
    /// click before the call returns.
    pub fn click(&mut self, x: i32, y: i32) -> &mut Self {
        self.harness.send_mouse_click(x, y, 0); // 0 = Left button
        self.harness.dispatch_to(&mut self.widget);
        self
    }
    /// Simulates a pointer move to `(x, y)` and dispatches it.
    pub fn move_mouse(&mut self, x: i32, y: i32) -> &mut Self {
        self.harness.send_mouse_move(x, y);
        self.harness.dispatch_to(&mut self.widget);
        self
    }
    /// Simulates a key press of `key` with no modifier keys, and dispatches it.
    ///
    /// `key` is the platform key code; use [`TestHarness::send_key_press`]
    /// directly when a modifier bitmask is needed.
    pub fn press_key(&mut self, key: u32) -> &mut Self {
        self.harness.send_key_press(key, 0);
        self.harness.dispatch_to(&mut self.widget);
        self
    }
    /// Asserts that the widget is visible; panics otherwise.
    pub fn assert_visible(&self) -> &Self {
        assert!(self.widget.is_visible(), "Widget should be visible");
        self
    }
    /// Asserts that the widget is hidden; panics otherwise.
    pub fn assert_hidden(&self) -> &Self {
        assert!(!self.widget.is_visible(), "Widget should be hidden");
        self
    }
    /// Asserts that the widget is enabled; panics otherwise.
    pub fn assert_enabled(&self) -> &Self {
        assert!(self.widget.is_enabled(), "Widget should be enabled");
        self
    }
    /// Asserts that the widget is disabled; panics otherwise.
    pub fn assert_disabled(&self) -> &Self {
        assert!(!self.widget.is_enabled(), "Widget should be disabled");
        self
    }
    /// Asserts the widget's exact geometry (position and size); panics otherwise.
    pub fn assert_geometry(&self, expected: Rect) -> &Self {
        assert_eq!(self.widget.geometry(), expected, "Widget geometry mismatch");
        self
    }
    /// Asserts the widget's exact size; panics otherwise.
    pub fn assert_size(&self, expected: Size) -> &Self {
        assert_eq!(self.widget.size(), expected, "Widget size mismatch");
        self
    }
    /// Asserts the widget's exact position; panics otherwise.
    pub fn assert_position(&self, expected: Point) -> &Self {
        assert_eq!(self.widget.position(), expected, "Widget position mismatch");
        self
    }
}
/// Layout tester
///
/// Runs a layout function against a fixed container rectangle and checks the
/// rectangles it produces. The layout function is a plain closure, so the
/// tester works with any layout algorithm without that algorithm depending on
/// this module.
///
/// All checks are exact integer comparisons and panic with a message naming the
/// offending index; there is no tolerance for off-by-one geometry.
pub struct LayoutTester {
    container_rect: Rect,
}
impl LayoutTester {
    /// Creates a tester whose assertions are relative to `container_rect`.
    pub fn new(container_rect: Rect) -> Self {
        Self { container_rect }
    }
    /// Runs `layout_fn` on the container and asserts the results equal
    /// `expected_positions`, in order.
    ///
    /// Panics if the count differs or if any rectangle differs. The closure is
    /// given the container rectangle and returns the laid-out child rectangles.
    pub fn test_layout<F>(&self, layout_fn: F, expected_positions: &[Rect])
    where
        F: FnOnce(&Rect) -> Vec<Rect>,
    {
        let positions = layout_fn(&self.container_rect);
        assert_eq!(
            positions.len(),
            expected_positions.len(),
            "Layout produced wrong number of positions"
        );
        for (i, (actual, expected)) in positions.iter().zip(expected_positions.iter()).enumerate() {
            assert_eq!(
                actual, expected,
                "Position {i} mismatch: got {actual:?}, expected {expected:?}"
            );
        }
    }
    /// Asserts that every rectangle lies fully inside the container.
    ///
    /// Panics naming the first rectangle that does not. Rectangles are expected
    /// to be in the same coordinate space as the container (i.e. already
    /// absolute, not parent-relative).
    pub fn assert_fits_in_container(&self, positions: &[Rect]) {
        for (i, rect) in positions.iter().enumerate() {
            assert!(
                self.container_rect.contains_rect(rect),
                "Position {i} ({rect:?}) does not fit in container"
            );
        }
    }
    /// Asserts that no two rectangles overlap.
    ///
    /// Compares every pair, so the check is quadratic in the number of
    /// rectangles. Rectangles that merely touch along an edge are not considered
    /// overlapping.
    pub fn assert_no_overlap(&self, positions: &[Rect]) {
        for i in 0..positions.len() {
            for j in (i + 1)..positions.len() {
                assert!(
                    !positions[i].intersects(&positions[j]),
                    "Positions {} and {} overlap: {:?} and {:?}",
                    i,
                    j,
                    positions[i],
                    positions[j]
                );
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::Label;
    #[test]
    fn test_harness() {
        let mut harness = TestHarness::new();
        harness.send_mouse_click(100, 100, 0); // 0 = Left button
        harness.send_key_press(65, 0);
        assert_eq!(harness.event_count(), 3);
        let handled =
            harness.dispatch_to(&mut Label::new("Test".to_string(), Rect::new(0, 0, 100, 30)));
        // dispatch_to returns the number of dispatched events (3)
        // Label handles events without returning a "handled" count,
        // so dispatch_to always returns the number of events processed
        assert_eq!(handled, 3);
    }
    #[test]
    fn test_layout_tester() {
        let tester = LayoutTester::new(Rect::new(0, 0, 400, 300));
        let positions =
            vec![Rect::new(0, 0, 100, 50), Rect::new(100, 0, 100, 50), Rect::new(200, 0, 100, 50)];
        tester.assert_fits_in_container(&positions);
        tester.assert_no_overlap(&positions);
    }
}
