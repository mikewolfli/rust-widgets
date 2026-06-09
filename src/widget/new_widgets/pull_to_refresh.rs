//! PullToRefresh widget — pull-to-refresh gesture control for scrollable views.
//!
//! Shows a loading indicator when the user pulls down past a threshold,
//! then emits a refresh signal. Supports idle/pulling/refreshing states.

use crate::core::{Color, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::GenericSignal;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Pull state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullState {
    Idle,
    Pulling,
    Refreshing,
}

/// PullToRefresh widget for pull-to-refresh gesture handling.
pub struct PullToRefresh {
    base: BaseWidget,
    /// Current pull state.
    state: PullState,
    /// How far the user has pulled (pixels).
    pull_offset: f32,
    /// Threshold after which release triggers refresh.
    threshold: f32,
    /// Emitted when refresh is triggered.
    pub refreshed: GenericSignal,
}

impl PullToRefresh {
    /// Creates a new PullToRefresh widget with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::PullToRefresh, geometry, "PullToRefresh"),
            state: PullState::Idle,
            pull_offset: 0.0,
            threshold: 60.0,
            refreshed: GenericSignal::new(),
        }
    }

    /// Returns the current pull state.
    pub fn state(&self) -> PullState {
        self.state
    }

    /// Returns the current pull offset.
    pub fn pull_offset(&self) -> f32 {
        self.pull_offset
    }

    /// Sets the pull threshold (pixels) that triggers refresh on release.
    pub fn set_threshold(&mut self, threshold: f32) {
        self.threshold = threshold;
    }

    /// Returns the pull threshold.
    pub fn threshold(&self) -> f32 {
        self.threshold
    }

    /// Called when the user pulls down.
    pub fn start_pull(&mut self) {
        if self.state == PullState::Idle {
            self.state = PullState::Pulling;
            self.base.request_redraw();
        }
    }

    /// Called to update the pull offset during a drag.
    pub fn update_pull(&mut self, delta: f32) {
        if self.state == PullState::Pulling || self.state == PullState::Idle {
            self.state = PullState::Pulling;
            self.pull_offset = (self.pull_offset + delta).min(self.threshold * 2.0).max(0.0);
            self.base.request_redraw();
        }
    }

    /// Called when the user releases the pull.
    /// Triggers refresh if above threshold, otherwise snaps back.
    pub fn end_pull(&mut self) {
        if self.state == PullState::Pulling {
            if self.pull_offset >= self.threshold {
                self.state = PullState::Refreshing;
                self.refreshed.emit();
            } else {
                self.state = PullState::Idle;
            }
            self.pull_offset = 0.0;
            self.base.request_redraw();
        }
    }

    /// Called when refresh is complete (externally).
    pub fn finish_refresh(&mut self) {
        if self.state == PullState::Refreshing {
            self.state = PullState::Idle;
            self.pull_offset = 0.0;
            self.base.request_redraw();
        }
    }
}

impl Widget for PullToRefresh {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl Draw for PullToRefresh {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_refreshing = self.state == PullState::Refreshing;
        let show_indicator = is_refreshing || self.pull_offset > 5.0;

        // Draw background
        context.fill_rect(rect, Color::rgba(240, 240, 240, 255));

        if show_indicator {
            let center_y =
                if is_refreshing { rect.y + 20 } else { rect.y + (self.pull_offset * 0.5) as i32 };
            let indicator_size = 16;
            let indicator_x = rect.x + (rect.width as i32 - indicator_size) / 2;
            let indicator_y = center_y - indicator_size / 2;

            let progress =
                if is_refreshing { 1.0 } else { (self.pull_offset / self.threshold).min(1.0) };

            // Draw arrow/circle
            let color = if progress >= 1.0 {
                Color::rgba(52, 120, 246, 200)
            } else {
                Color::rgba(120, 120, 120, 180)
            };

            context.draw_circle_stroke(
                Point::new(indicator_x + indicator_size / 2, indicator_y + indicator_size / 2),
                (indicator_size as f32 / 2.0) as u32,
                color,
                2,
            );

            // Draw arrow line
            let center =
                Point::new(indicator_x + indicator_size / 2, indicator_y + indicator_size / 2);
            context.draw_line(
                Point::new(center.x - 4, center.y + 3),
                Point::new(center.x, center.y - 3),
                color,
            );
            context.draw_line(
                Point::new(center.x, center.y - 3),
                Point::new(center.x + 4, center.y + 3),
                color,
            );
        }
    }
}

impl EventHandler for PullToRefresh {
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
            Event::MouseMove { pos: _, .. } => {
                // In a real integration, delta_y would come from the scroll position
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

    #[test]
    fn pull_to_refresh_default_state() {
        let ptr = PullToRefresh::new(Rect::new(0, 0, 200, 400));
        assert_eq!(ptr.state(), PullState::Idle);
        assert_eq!(ptr.pull_offset(), 0.0);
        assert_eq!(ptr.threshold(), 60.0);
        assert_eq!(ptr.kind(), WidgetKind::PullToRefresh);
    }

    #[test]
    fn pull_to_refresh_start_pull_changes_state() {
        let mut ptr = PullToRefresh::new(Rect::new(0, 0, 200, 400));
        ptr.start_pull();
        assert_eq!(ptr.state(), PullState::Pulling);
    }

    #[test]
    fn pull_to_refresh_update_pull_increases_offset() {
        let mut ptr = PullToRefresh::new(Rect::new(0, 0, 200, 400));
        ptr.start_pull();
        ptr.update_pull(30.0);
        assert_eq!(ptr.pull_offset(), 30.0);
        ptr.update_pull(20.0);
        assert_eq!(ptr.pull_offset(), 50.0);
    }

    #[test]
    fn pull_to_refresh_release_below_threshold_returns_to_idle() {
        let mut ptr = PullToRefresh::new(Rect::new(0, 0, 200, 400));
        ptr.start_pull();
        ptr.update_pull(30.0);
        ptr.end_pull();
        assert_eq!(ptr.state(), PullState::Idle);
        assert_eq!(ptr.pull_offset(), 0.0);
    }

    #[test]
    fn pull_to_refresh_release_above_threshold_triggers_refresh() {
        let mut ptr = PullToRefresh::new(Rect::new(0, 0, 200, 400));
        let captured = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        ptr.refreshed.connect({
            let captured = std::sync::Arc::clone(&captured);
            move || {
                captured.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        });
        ptr.start_pull();
        ptr.update_pull(80.0);
        ptr.end_pull();
        assert_eq!(ptr.state(), PullState::Refreshing);
        assert!(captured.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn pull_to_refresh_finish_refresh_returns_to_idle() {
        let mut ptr = PullToRefresh::new(Rect::new(0, 0, 200, 400));
        ptr.start_pull();
        ptr.update_pull(80.0);
        ptr.end_pull();
        assert_eq!(ptr.state(), PullState::Refreshing);
        ptr.finish_refresh();
        assert_eq!(ptr.state(), PullState::Idle);
    }

    #[test]
    fn pull_to_refresh_threshold_setter() {
        let mut ptr = PullToRefresh::new(Rect::new(0, 0, 200, 400));
        ptr.set_threshold(100.0);
        assert_eq!(ptr.threshold(), 100.0);
    }

    #[test]
    fn pull_to_refresh_svg_output() {
        let mut ptr = PullToRefresh::new(Rect::new(0, 0, 200, 400));
        let svg = crate::widget::svg::render_to_svg(&mut ptr);
        assert!(svg.starts_with("<svg"));
    }

    #[test]
    fn pull_to_refresh_disabled_blocks_events() {
        let mut ptr = PullToRefresh::new(Rect::new(0, 0, 200, 400));
        ptr.set_enabled(false);
        ptr.handle_event(&Event::MousePress { pos: Point::new(100, 100), button: 1 });
        assert_eq!(ptr.state(), PullState::Idle);
    }
}
