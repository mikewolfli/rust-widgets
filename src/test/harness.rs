use crate::core::{Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::widget::Widget;
use std::collections::VecDeque;
/// Test harness for widget testing
pub struct TestHarness {
    events: VecDeque<Event>,
    screen_size: Size,
}
impl TestHarness {
    pub fn new() -> Self {
        Self {
            events: VecDeque::new(),
            screen_size: Size::new(800, 600),
        }
    }
    pub fn with_screen_size(mut self, size: Size) -> Self {
        self.screen_size = size;
        self
    }
    pub fn screen_size(&self) -> Size {
        self.screen_size
    }
    pub fn send_event(&mut self, event: Event) {
        self.events.push_back(event);
    }
    pub fn send_mouse_click(&mut self, x: i32, y: i32, button: u32) {
        let point = Point::new(x as f32, y as f32);
        self.send_event(Event::MousePress { pos: point, button });
        self.send_event(Event::MouseRelease { pos: point, button });
    }
    pub fn send_mouse_move(&mut self, x: i32, y: i32) {
        self.send_event(Event::MouseMove {
            pos: Point::new(x as f32, y as f32),
        });
    }
    pub fn send_key_press(&mut self, key: u32, modifiers: u32) {
        self.send_event(Event::KeyPress { key, modifiers });
    }
    pub fn send_key_release(&mut self, key: u32, modifiers: u32) {
        self.send_event(Event::KeyRelease { key, modifiers });
    }
    pub fn next_event(&mut self) -> Option<Event> {
        self.events.pop_front()
    }
    pub fn event_count(&self) -> usize {
        self.events.len()
    }
    pub fn clear_events(&mut self) {
        self.events.clear();
    }
    pub fn dispatch_to<W: EventHandler>(&mut self, widget: &mut W) -> usize {
        let mut handled = 0;
        while let Some(event) = self.next_event() {
            widget.handle_event(&event);
            handled += 1;
        }
        handled
    }
}
impl Default for TestHarness {
    fn default() -> Self {
        Self::new()
    }
}
/// Widget tester with assertions
pub struct WidgetTester<W: Widget> {
    widget: W,
    harness: TestHarness,
}
impl<W: Widget> WidgetTester<W> {
    pub fn new(widget: W) -> Self {
        Self {
            widget,
            harness: TestHarness::new(),
        }
    }
    pub fn with_harness(mut self, harness: TestHarness) -> Self {
        self.harness = harness;
        self
    }
    pub fn widget(&self) -> &W {
        &self.widget
    }
    pub fn widget_mut(&mut self) -> &mut W {
        &mut self.widget
    }
    pub fn click(&mut self, x: i32, y: i32) -> &mut Self {
        self.harness.send_mouse_click(x, y, 0); // 0 = Left button
        self.harness.dispatch_to(&mut self.widget);
        self
    }
    pub fn move_mouse(&mut self, x: i32, y: i32) -> &mut Self {
        self.harness.send_mouse_move(x, y);
        self.harness.dispatch_to(&mut self.widget);
        self
    }
    pub fn press_key(&mut self, key: u32) -> &mut Self {
        self.harness.send_key_press(key, 0);
        self.harness.dispatch_to(&mut self.widget);
        self
    }
    pub fn assert_visible(&self) -> &Self {
        assert!(self.widget.is_visible(), "Widget should be visible");
        self
    }
    pub fn assert_hidden(&self) -> &Self {
        assert!(!self.widget.is_visible(), "Widget should be hidden");
        self
    }
    pub fn assert_enabled(&self) -> &Self {
        assert!(self.widget.is_enabled(), "Widget should be enabled");
        self
    }
    pub fn assert_disabled(&self) -> &Self {
        assert!(!self.widget.is_enabled(), "Widget should be disabled");
        self
    }
    pub fn assert_geometry(&self, expected: Rect) -> &Self {
        assert_eq!(self.widget.geometry(), expected, "Widget geometry mismatch");
        self
    }
    pub fn assert_size(&self, expected: Size) -> &Self {
        assert_eq!(self.widget.size(), expected, "Widget size mismatch");
        self
    }
    pub fn assert_position(&self, expected: Point) -> &Self {
        assert_eq!(self.widget.position(), expected, "Widget position mismatch");
        self
    }
}
/// Layout tester
pub struct LayoutTester {
    container_rect: Rect,
}
impl LayoutTester {
    pub fn new(container_rect: Rect) -> Self {
        Self { container_rect }
    }
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
                "Position {} mismatch: got {:?}, expected {:?}",
                i, actual, expected
            );
        }
    }
    pub fn assert_fits_in_container(&self, positions: &[Rect]) {
        for (i, rect) in positions.iter().enumerate() {
            assert!(
                self.container_rect.contains_rect(rect),
                "Position {} ({:?}) does not fit in container",
                i,
                rect
            );
        }
    }
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
        let handled = harness.dispatch_to(&mut Label::new(
            "Test".to_string(),
            Rect::new(0, 0, 100, 30),
        ));
        assert_eq!(handled, 0);
    }
    #[test]
    fn test_layout_tester() {
        let tester = LayoutTester::new(Rect::new(0, 0, 400, 300));
        let positions = vec![
            Rect::new(0, 0, 100, 50),
            Rect::new(100, 0, 100, 50),
            Rect::new(200, 0, 100, 50),
        ];
        tester.assert_fits_in_container(&positions);
        tester.assert_no_overlap(&positions);
    }
}
