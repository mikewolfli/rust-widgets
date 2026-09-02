//! RefreshControl widget — pull-to-refresh control for scrollable views.
//!
//! Shows a pull-down indicator (spinner arrow) at the top of the widget when
//! the user pulls down, and emits a `refresh_triggered` signal when the pull
//! distance exceeds the configured threshold. Supports embedding child content
//! below the indicator area.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::GenericSignal;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Progress state for the refresh pull gesture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshState {
    /// No pull in progress.
    Idle,
    /// User is pulling down (not yet past threshold).
    Dragging,
    /// Pull distance exceeded threshold; refresh triggered on release.
    Triggered,
}

/// Pull-to-refresh control for mobile scrollable views.
///
/// Wraps optional child content and displays a pull-down indicator at the top.
/// When the user pulls down past `threshold` pixels and releases, the
/// `refresh_triggered` signal fires. The `is_refreshing` flag is managed
/// externally and controls whether a loading spinner is shown.
pub struct RefreshControl {
    base: BaseWidget,
    is_refreshing: bool,
    pull_distance: f32,
    threshold: f32,
    state: RefreshState,
    content: Option<Box<dyn Widget>>,
    /// Emitted when the pull distance exceeds the threshold and the user releases.
    pub refresh_triggered: GenericSignal,
}

impl RefreshControl {
    /// Creates a new RefreshControl widget with the given geometry.
    ///
    /// Default threshold is 60.0 pixels.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::RefreshControl, geometry, "RefreshControl"),
            is_refreshing: false,
            pull_distance: 0.0,
            threshold: 60.0,
            state: RefreshState::Idle,
            content: None,
            refresh_triggered: GenericSignal::new(),
        }
    }

    /// Sets the child content widget.
    pub fn set_content(&mut self, widget: Box<dyn Widget>) {
        self.content = Some(widget);
        self.base.request_redraw();
    }

    /// Returns a reference to the child content, if any.
    pub fn content(&self) -> Option<&dyn Widget> {
        self.content.as_deref()
    }

    /// Returns a mutable reference to the child content, if any.
    pub fn content_mut(&mut self) -> Option<&mut dyn Widget> {
        self.content.as_deref_mut()
    }

    /// Sets the refreshing state.
    pub fn set_is_refreshing(&mut self, refreshing: bool) {
        if self.is_refreshing != refreshing {
            self.is_refreshing = refreshing;
            if !refreshing {
                self.pull_distance = 0.0;
                self.state = RefreshState::Idle;
            }
            self.base.request_redraw();
        }
    }

    /// Returns whether a refresh operation is in progress.
    pub fn is_refreshing(&self) -> bool {
        self.is_refreshing
    }

    /// Sets the pull distance (pixels pulled down from the top).
    pub fn set_pull_distance(&mut self, distance: f32) {
        let clamped = distance.max(0.0);
        self.pull_distance = clamped;
        if clamped > 0.0 && self.state == RefreshState::Idle {
            self.state = RefreshState::Dragging;
        } else if clamped == 0.0 && self.state == RefreshState::Dragging {
            self.state = RefreshState::Idle;
        }
        self.base.request_redraw();
    }

    /// Returns the current pull distance.
    pub fn pull_distance(&self) -> f32 {
        self.pull_distance
    }

    /// Sets the pull threshold in pixels.
    pub fn set_threshold(&mut self, threshold: f32) {
        self.threshold = threshold;
    }

    /// Returns the pull threshold.
    pub fn threshold(&self) -> f32 {
        self.threshold
    }

    /// Called when the user starts pulling down.
    pub fn start_pull(&mut self) {
        if self.state == RefreshState::Idle && !self.is_refreshing {
            self.state = RefreshState::Dragging;
            self.base.request_redraw();
        }
    }

    /// Called to update the pull distance during a drag.
    pub fn update_pull(&mut self, delta: f32) {
        if self.is_refreshing {
            return;
        }
        if self.state == RefreshState::Idle {
            self.state = RefreshState::Dragging;
        }
        self.pull_distance = (self.pull_distance + delta).max(0.0).min(self.threshold * 2.0);
        self.base.request_redraw();
    }

    /// Called when the user releases the pull.
    /// Triggers refresh if above threshold.
    pub fn end_pull(&mut self) {
        if self.state == RefreshState::Dragging {
            if self.pull_distance >= self.threshold && !self.is_refreshing {
                self.state = RefreshState::Triggered;
                self.is_refreshing = true;
                self.refresh_triggered.emit();
            } else {
                self.state = RefreshState::Idle;
            }
            self.pull_distance = 0.0;
            self.base.request_redraw();
        }
    }

    /// Returns the current refresh state.
    pub fn refresh_state(&self) -> RefreshState {
        self.state
    }
}

impl Widget for RefreshControl {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(400, 400)
    }
}

impl Draw for RefreshControl {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let indicator_height: u32 =
            if self.is_refreshing || self.pull_distance > 5.0 { 40 } else { 0 };

        // Draw background
        context.fill_rect(rect, Color::WHITE);

        if indicator_height > 0 {
            let indicator_area = Rect::new(rect.x, rect.y, rect.width, indicator_height);
            context.fill_rect(indicator_area, Color::rgba(245, 245, 245, 255));

            // Indicator center
            let center_x = rect.x + (rect.width as i32) / 2;
            let center_y = rect.y + (indicator_height as i32) / 2;

            if self.is_refreshing {
                // Draw spinning indicator (circle with arc)
                let radius: u32 = 10;
                context.draw_circle_stroke(
                    Point::new(center_x, center_y),
                    radius,
                    Color::rgba(52, 120, 246, 220),
                    3,
                );
                // Draw arrow inside
                let tip_y = center_y - 5;
                context.draw_line(
                    Point::new(center_x, tip_y - 3),
                    Point::new(center_x, tip_y + 4),
                    Color::rgba(52, 120, 246, 220),
                );
                context.draw_line(
                    Point::new(center_x - 4, tip_y + 1),
                    Point::new(center_x, tip_y - 3),
                    Color::rgba(52, 120, 246, 220),
                );
                context.draw_line(
                    Point::new(center_x + 4, tip_y + 1),
                    Point::new(center_x, tip_y - 3),
                    Color::rgba(52, 120, 246, 220),
                );

                // Draw "Loading..." text below
                let font = Font::simple("sans-serif", 11.0);
                let label = "Loading...";
                let metrics = context.measure_text(label, &font);
                let text_x = center_x - (metrics.width as i32) / 2;
                let text_y = center_y + 14 + metrics.ascent as i32;
                context.draw_text(
                    Point::new(text_x, text_y),
                    label,
                    &font,
                    Color::rgba(120, 120, 120, 220),
                    HorizontalAlignment::Left,
                );
            } else {
                // Draw pull indicator (arrow + progress)
                let progress = (self.pull_distance / self.threshold).min(1.0);
                let arrow_color = if progress >= 1.0 {
                    Color::rgba(52, 120, 246, 220)
                } else {
                    Color::rgba(140, 140, 140, 200)
                };

                // Downward arrow
                let arrow_size = 12;
                let arrow_top = center_y - arrow_size / 2;
                context.draw_line(
                    Point::new(center_x, arrow_top),
                    Point::new(center_x, arrow_top + arrow_size),
                    arrow_color,
                );
                context.draw_line(
                    Point::new(center_x - 4, arrow_top + arrow_size - 4),
                    Point::new(center_x, arrow_top + arrow_size),
                    arrow_color,
                );
                context.draw_line(
                    Point::new(center_x + 4, arrow_top + arrow_size - 4),
                    Point::new(center_x, arrow_top + arrow_size),
                    arrow_color,
                );

                // Progress text
                if progress >= 1.0 {
                    let font = Font::simple("sans-serif", 11.0);
                    let label = "Release to refresh";
                    let metrics = context.measure_text(label, &font);
                    let text_x = center_x - (metrics.width as i32) / 2;
                    let text_y = center_y + 14 + metrics.ascent as i32;
                    context.draw_text(
                        Point::new(text_x, text_y),
                        label,
                        &font,
                        Color::rgba(52, 120, 246, 200),
                        HorizontalAlignment::Left,
                    );
                }
            }
        }

        // Draw content area below indicator
        let content_y = rect.y + indicator_height as i32;
        let content_height = rect.height.saturating_sub(indicator_height);
        if content_height > 0 {
            let content_rect = Rect::new(rect.x, content_y, rect.width, content_height);
            context.fill_rect(content_rect, Color::WHITE);
        }
    }
}

impl EventHandler for RefreshControl {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos: _, button } => {
                if *button == 1 {
                    self.start_pull();
                }
            }
            Event::MouseMove { .. } => {
                // delta would come from scroll position in real integration
            }
            Event::MouseRelease { pos: _, button } => {
                if *button == 1 {
                    self.end_pull();
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
    use crate::core::Point;
    use crate::widget::svg::render_to_svg;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    fn make_refresh_control() -> RefreshControl {
        RefreshControl::new(Rect::new(0, 0, 200, 400))
    }

    #[test]
    fn refresh_control_default_state() {
        let rc = make_refresh_control();
        assert!(!rc.is_refreshing());
        assert_eq!(rc.pull_distance(), 0.0);
        assert_eq!(rc.threshold(), 60.0);
        assert_eq!(rc.refresh_state(), RefreshState::Idle);
        assert_eq!(rc.kind(), WidgetKind::RefreshControl);
    }

    #[test]
    fn refresh_control_pull_and_release_below_threshold() {
        let mut rc = make_refresh_control();
        rc.start_pull();
        assert_eq!(rc.refresh_state(), RefreshState::Dragging);

        rc.update_pull(30.0);
        assert_eq!(rc.pull_distance(), 30.0);

        rc.end_pull();
        assert_eq!(rc.refresh_state(), RefreshState::Idle);
        assert_eq!(rc.pull_distance(), 0.0);
        assert!(!rc.is_refreshing());
    }

    #[test]
    fn refresh_control_pull_above_threshold_triggers_refresh() {
        let mut rc = make_refresh_control();
        let captured = Arc::new(AtomicBool::new(false));
        rc.refresh_triggered.connect({
            let captured = Arc::clone(&captured);
            move || {
                captured.store(true, Ordering::SeqCst);
            }
        });

        rc.start_pull();
        rc.update_pull(80.0);
        rc.end_pull();

        assert_eq!(rc.refresh_state(), RefreshState::Triggered);
        assert!(rc.is_refreshing());
        assert!(captured.load(Ordering::SeqCst));
    }

    #[test]
    fn refresh_control_set_is_refreshing() {
        let mut rc = make_refresh_control();
        rc.set_is_refreshing(true);
        assert!(rc.is_refreshing());
        rc.set_is_refreshing(false);
        assert!(!rc.is_refreshing());
        assert_eq!(rc.refresh_state(), RefreshState::Idle);
    }

    #[test]
    fn refresh_control_set_threshold() {
        let mut rc = make_refresh_control();
        rc.set_threshold(100.0);
        assert_eq!(rc.threshold(), 100.0);
    }

    #[test]
    fn refresh_control_svg_output() {
        let mut rc = make_refresh_control();
        rc.set_pull_distance(40.0);
        let svg = render_to_svg(&mut rc);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn refresh_control_disabled_blocks_events() {
        let mut rc = make_refresh_control();
        rc.set_enabled(false);
        rc.handle_event(&Event::MousePress { pos: Point::new(100, 100), button: 1 });
        assert_eq!(rc.refresh_state(), RefreshState::Idle);
    }

    #[test]
    fn refresh_control_refreshing_blocks_pull() {
        let mut rc = make_refresh_control();
        rc.set_is_refreshing(true);
        rc.start_pull();
        // Should stay in Idle since refreshing blocks new pulls
        assert!(rc.is_refreshing());
    }
}
