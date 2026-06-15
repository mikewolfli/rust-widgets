//! GanttWidget for timeline task planning.

use crate::core::{HorizontalAlignment, Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// One gantt task bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GanttTask {
    pub id: String,
    pub label: String,
    pub start: i64,
    pub end: i64,
    pub progress: u8,
}

impl GanttTask {
    /// Creates task.
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        start: i64,
        end: i64,
        progress: u8,
    ) -> Self {
        let s = start;
        let e = end.max(s + 1);
        Self { id: id.into(), label: label.into(), start: s, end: e, progress: progress.min(100) }
    }
}

/// Basic gantt widget with selectable task rows and zoomable viewport.
pub struct GanttWidget {
    base: BaseWidget,
    tasks: Vec<GanttTask>,
    selected_index: Option<usize>,
    viewport_start: i64,
    viewport_end: i64,
    row_height: u32,
    /// Emitted when task selected. Payload is task id.
    pub task_selected: Signal1<String>,
}

impl GanttWidget {
    /// Creates empty gantt.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Chart, geometry, "GanttWidget"),
            tasks: Vec::new(),
            selected_index: None,
            viewport_start: 0,
            viewport_end: 100,
            row_height: 24,
            task_selected: Signal1::new(),
        }
    }

    /// Sets tasks.
    pub fn set_tasks(&mut self, tasks: Vec<GanttTask>) {
        self.tasks = tasks;
        self.selected_index = None;
        self.recompute_viewport();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns tasks.
    pub fn tasks(&self) -> &[GanttTask] {
        &self.tasks
    }

    /// Returns viewport range.
    pub fn viewport(&self) -> (i64, i64) {
        (self.viewport_start, self.viewport_end)
    }

    /// Sets viewport range.
    pub fn set_viewport(&mut self, start: i64, end: i64) {
        self.viewport_start = start;
        self.viewport_end = end.max(start + 1);
        self.base.request_redraw();
    }

    /// Zooms around center.
    pub fn zoom(&mut self, factor: f32) {
        if factor <= 0.0 {
            return;
        }
        let span = (self.viewport_end - self.viewport_start).max(1) as f32;
        let center = (self.viewport_start + self.viewport_end) as f32 / 2.0;
        let half = (span / factor / 2.0).max(1.0);
        self.set_viewport((center - half).floor() as i64, (center + half).ceil() as i64);
    }

    /// Select task by index.
    pub fn select_index(&mut self, index: usize) -> bool {
        if index >= self.tasks.len() {
            return false;
        }
        self.selected_index = Some(index);
        if let Some(task) = self.tasks.get(index) {
            self.task_selected.emit(task.id.clone());
        }
        self.base.request_redraw();
        true
    }

    /// Returns selected task id.
    pub fn selected_id(&self) -> Option<&str> {
        let index = self.selected_index?;
        self.tasks.get(index).map(|task| task.id.as_str())
    }

    fn recompute_viewport(&mut self) {
        if self.tasks.is_empty() {
            self.viewport_start = 0;
            self.viewport_end = 100;
            return;
        }

        let mut min_start = i64::MAX;
        let mut max_end = i64::MIN;
        for task in &self.tasks {
            min_start = min_start.min(task.start);
            max_end = max_end.max(task.end);
        }
        self.viewport_start = min_start;
        self.viewport_end = max_end.max(min_start + 1);
    }

    fn row_at(&self, pos: Point) -> Option<usize> {
        let rect = self.geometry();
        if pos.x < rect.x
            || pos.x >= rect.x + rect.width as i32
            || pos.y < rect.y
            || pos.y >= rect.y + rect.height as i32
        {
            return None;
        }
        let idx = ((pos.y - rect.y) / self.row_height as i32) as usize;
        (idx < self.tasks.len()).then_some(idx)
    }

    fn project_x(&self, value: i64, track_x: i32, track_w: u32) -> i32 {
        let start = self.viewport_start;
        let end = self.viewport_end.max(start + 1);
        let span = (end - start) as f32;
        let ratio = ((value - start) as f32 / span).clamp(0.0, 1.0);
        track_x + (ratio * track_w as f32) as i32
    }
}

impl Widget for GanttWidget {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl EventHandler for GanttWidget {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button: 1 } => {
                if let Some(index) = self.row_at(*pos) {
                    let _ = self.select_index(index);
                }
            }
            Event::Wheel { delta, .. } => {
                if delta.y < 0 {
                    self.zoom(1.2);
                } else if delta.y > 0 {
                    self.zoom(0.8);
                }
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}

impl Draw for GanttWidget {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        context.fill_rect(rect, Color::rgb(251, 252, 254));
        context.draw_rect(rect, Color::rgb(189, 197, 210));

        let track_x = rect.x + 150;
        let track_w = rect.width.saturating_sub(160);

        for (index, task) in self.tasks.iter().take(12).enumerate() {
            let y = rect.y + index as i32 * self.row_height as i32;
            if self.selected_index == Some(index) {
                context.fill_rect(
                    Rect::new(rect.x, y, rect.width, self.row_height),
                    Color::rgb(225, 236, 252),
                );
            }

            context.draw_text(
                Point::new(rect.x + 8, y + self.row_height as i32 / 2),
                &task.label,
                &Font::default(),
                Color::rgb(36, 49, 68),
                HorizontalAlignment::Left,
            );

            let x0 = self.project_x(task.start, track_x, track_w);
            let x1 = self.project_x(task.end, track_x, track_w).max(x0 + 2);
            let bar_h = self.row_height.saturating_sub(10);
            let bar_rect = Rect::new(x0, y + 5, (x1 - x0) as u32, bar_h);
            context.fill_rect(bar_rect, Color::rgb(104, 163, 232));

            let progress_w =
                ((bar_rect.width as f32) * (task.progress as f32 / 100.0)).round() as u32;
            if progress_w > 0 {
                context.fill_rect(
                    Rect::new(bar_rect.x, bar_rect.y, progress_w, bar_rect.height),
                    Color::rgb(74, 140, 215),
                );
            }

            context.draw_line(
                Point::new(rect.x, y + self.row_height as i32),
                Point::new(rect.x + rect.width as i32, y + self.row_height as i32),
                Color::rgb(229, 234, 242),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn sample_tasks() -> Vec<GanttTask> {
        vec![
            GanttTask::new("t1", "Design", 0, 10, 100),
            GanttTask::new("t2", "Build", 8, 22, 60),
            GanttTask::new("t3", "Verify", 20, 30, 20),
        ]
    }

    #[test]
    fn set_tasks_recomputes_viewport() {
        let mut gantt = GanttWidget::new(Rect::new(0, 0, 700, 180));
        gantt.set_tasks(sample_tasks());

        assert_eq!(gantt.viewport(), (0, 30));
        assert_eq!(gantt.tasks().len(), 3);
    }

    #[test]
    fn selecting_task_emits_signal() {
        let mut gantt = GanttWidget::new(Rect::new(0, 0, 700, 180));
        gantt.set_tasks(sample_tasks());

        let selected = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = selected.clone();
        gantt.task_selected.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });

        assert!(gantt.select_index(1));
        assert_eq!(gantt.selected_id(), Some("t2"));

        let got = selected.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec!["t2".to_string()]);
    }

    #[test]
    fn wheel_zoom_changes_viewport_span() {
        let mut gantt = GanttWidget::new(Rect::new(0, 0, 700, 180));
        gantt.set_tasks(sample_tasks());

        let before = gantt.viewport();
        gantt.handle_event(&Event::wheel(0, -120, 0));
        let after_in = gantt.viewport();
        assert!((after_in.1 - after_in.0) < (before.1 - before.0));

        gantt.handle_event(&Event::wheel(0, 120, 0));
        let after_out = gantt.viewport();
        assert!((after_out.1 - after_out.0) >= (after_in.1 - after_in.0));
    }

    #[test]
    fn new_creates_default_state() {
        let gantt = GanttWidget::new(Rect::new(0, 0, 800, 600));
        assert!(gantt.tasks().is_empty());
        assert_eq!(gantt.selected_id(), None);
        assert_eq!(gantt.viewport(), (0, 100));
    }

    #[test]
    fn task_creation_validates_end_gt_start() {
        let task = GanttTask::new("t1", "Task", 10, 5, 50);
        assert!(task.end > task.start);
        assert_eq!(task.start, 10);
        assert_eq!(task.end, 11); // end clamped to start + 1
    }

    #[test]
    fn progress_clamp_upper_bound() {
        let task = GanttTask::new("t1", "Task", 0, 10, 200);
        assert_eq!(task.progress, 100);
    }

    #[test]
    fn progress_clamp_lower_bound() {
        // Progress is u8, so 0 is the minimum. Just check behavior.
        let task = GanttTask::new("t1", "Task", 0, 10, 0);
        assert_eq!(task.progress, 0);
    }

    #[test]
    fn select_index_out_of_bounds_returns_false() {
        let mut gantt = GanttWidget::new(Rect::new(0, 0, 800, 600));
        gantt.set_tasks(sample_tasks());
        assert!(!gantt.select_index(10));
        assert_eq!(gantt.selected_id(), None);
    }

    #[test]
    fn select_index_duplicate_guard() {
        let mut gantt = GanttWidget::new(Rect::new(0, 0, 800, 600));
        gantt.set_tasks(sample_tasks());
        assert!(gantt.select_index(0));
        assert_eq!(gantt.selected_id(), Some("t1"));

        let emitted = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = emitted.clone();
        gantt.task_selected.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });
        gantt.select_index(0);
        let got = emitted.lock().ok().map(|g| g.clone()).unwrap_or_default();
        assert_eq!(got.len(), 1); // note: select_index always emits even if same
                                  // This is the existing behavior - select_index does not guard
    }

    #[test]
    fn set_viewport_updates_range() {
        let mut gantt = GanttWidget::new(Rect::new(0, 0, 800, 600));
        gantt.set_viewport(50, 150);
        assert_eq!(gantt.viewport(), (50, 150));
    }

    #[test]
    fn set_viewport_maintains_min_span() {
        let mut gantt = GanttWidget::new(Rect::new(0, 0, 800, 600));
        gantt.set_viewport(10, 10);
        assert_eq!(gantt.viewport(), (10, 11));
    }

    #[test]
    fn empty_tasks_returns_default_viewport() {
        let mut gantt = GanttWidget::new(Rect::new(0, 0, 800, 600));
        gantt.set_tasks(vec![]);
        assert!(gantt.tasks().is_empty());
        assert_eq!(gantt.selected_id(), None);
        assert_eq!(gantt.viewport(), (0, 100));
    }

    #[test]
    fn zoom_with_zero_factor_does_nothing() {
        let mut gantt = GanttWidget::new(Rect::new(0, 0, 800, 600));
        gantt.set_tasks(sample_tasks());
        let before = gantt.viewport();
        gantt.zoom(0.0);
        assert_eq!(gantt.viewport(), before);
    }
}
