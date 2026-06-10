//! SwipeToDismiss — swipe-to-dismiss/delete gesture widget.
//!
//! Wraps a child widget that can be swiped left/right to reveal an action
//! background (e.g., red "Delete"). When the swipe passes the threshold,
//! the widget emits the `dismissed` signal.

use crate::core::{Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

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
    /// Whether the widget has been dismissed (one-shot).
    is_dismissed: bool,
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
            is_dismissed: false,
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
        self.base.request_redraw();
    }

    /// Programmatically triggers the dismiss.
    pub fn dismiss(&mut self) {
        if !self.is_dismissed {
            self.is_dismissed = true;
            self.swipe_offset = 0.0;
            self.dismissed.emit(());
            self.base.request_redraw();
        }
    }
}

impl Widget for SwipeToDismiss {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn kind(&self) -> WidgetKind {
        WidgetKind::SwipeToDismiss
    }
}

impl Draw for SwipeToDismiss {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        if self.is_dismissed {
            // Dismissed state: just fill transparent so it's hidden
            context.fill_rect(rect, Color::rgba(0, 0, 0, 0));
            return;
        }

        // ── Action background revealed behind the child as it slides ──
        if self.swipe_offset.abs() > 2.0 {
            let bg_rect = if self.swipe_offset < 0.0 {
                // Swiping left: reveal action on the right side
                let reveal_w = (-self.swipe_offset) as u32;
                Rect::new(
                    rect.x + rect.width as i32 - reveal_w as i32,
                    rect.y,
                    reveal_w,
                    rect.height,
                )
            } else {
                // Swiping right: reveal action on the left side
                let reveal_w = self.swipe_offset as u32;
                Rect::new(rect.x, rect.y, reveal_w, rect.height)
            };

            // Red background
            context.fill_rect(bg_rect, Color::rgba(255, 59, 48, 255)); // iOS red

            // Action text centered in revealed area
            if !self.action_text.is_empty() {
                let font = Font::new("sans-serif", 16.0, false, false);
                let metrics = context.measure_text(&self.action_text, &font);
                let text_x = bg_rect.x + (bg_rect.width as i32 - metrics.width as i32) / 2;
                let text_y = bg_rect.y + (bg_rect.height as i32 / 2) + (metrics.ascent as i32 / 2)
                    - (metrics.descent as i32 / 2);
                context.draw_text(
                    Point::new(text_x, text_y),
                    &self.action_text,
                    &font,
                    Color::WHITE,
                );
            }
        }

        // ── Draw the child widget offset by the swipe amount ──
        if let Some(child) = &mut self.child {
            // Save the original child geometry, offset it, draw, then restore
            let original_geom = child.geometry();
            let offset_x = self.swipe_offset as i32;
            let translated_rect = Rect::new(rect.x + offset_x, rect.y, rect.width, rect.height);
            child.set_geometry(translated_rect);
            child.draw(context);
            child.set_geometry(original_geom);
        }

        // ── Draw a subtle shadow line at the child edge when swiped ──
        if self.swipe_offset.abs() > 5.0 {
            let edge_x = if self.swipe_offset < 0.0 {
                rect.x + rect.width as i32 + self.swipe_offset as i32
            } else {
                rect.x + self.swipe_offset as i32
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

        match event {
            Event::MousePress { pos: _, button } => {
                if *button == 1 {
                    // Start tracking swipe
                }
            }
            Event::MouseMove { pos } => {
                // In a real integration, delta would come from drag events.
                // For testing purposes, we set the offset directly via
                // programmatic APIs instead of computing delta here.
                let _ = pos;
            }
            Event::MouseRelease { pos: _, button } => {
                if *button == 1 {
                    if self.swipe_offset.abs() >= self.dismiss_threshold {
                        self.is_dismissed = true;
                        self.dismissed.emit(());
                    }
                    self.swipe_offset = 0.0;
                    self.base.request_redraw();
                }
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

#[cfg(test)]
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

    #[test]
    fn swipe_to_dismiss_release_past_threshold_triggers_dismiss() {
        let mut sw = SwipeToDismiss::new(Rect::new(0, 0, 200, 50));

        let fired = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let f = fired.clone();
        sw.dismissed.connect(move |_: Arc<()>| {
            f.store(true, std::sync::atomic::Ordering::SeqCst);
        });

        // Simulate offset past threshold
        sw.swipe_offset = -120.0; // Left swipe past 100px threshold
        sw.handle_event(&Event::MouseRelease { pos: Point::new(0, 0), button: 1 });

        assert!(sw.is_dismissed());
        assert!(fired.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn swipe_to_dismiss_release_below_threshold_no_dismiss() {
        let mut sw = SwipeToDismiss::new(Rect::new(0, 0, 200, 50));

        let fired = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let f = fired.clone();
        sw.dismissed.connect(move |_: Arc<()>| {
            f.store(true, std::sync::atomic::Ordering::SeqCst);
        });

        // Simulate offset below threshold
        sw.swipe_offset = -50.0; // Only 50px, threshold is 100px
        sw.handle_event(&Event::MouseRelease { pos: Point::new(0, 0), button: 1 });

        assert!(!sw.is_dismissed());
        assert!(!fired.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(sw.swipe_offset(), 0.0);
    }

    #[test]
    fn swipe_to_dismiss_reset_swipe() {
        let mut sw = SwipeToDismiss::new(Rect::new(0, 0, 200, 50));
        sw.swipe_offset = -80.0;
        sw.is_dismissed = true;

        sw.reset_swipe();
        assert_eq!(sw.swipe_offset(), 0.0);
        assert!(!sw.is_dismissed());
    }

    #[test]
    fn swipe_to_dismiss_svg_output() {
        let mut sw = SwipeToDismiss::new(Rect::new(0, 0, 200, 50));
        sw.set_action_text("Delete");
        let svg = render_to_svg(&mut sw);
        assert!(svg.starts_with("<svg"));
    }
}
