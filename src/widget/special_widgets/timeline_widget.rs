//! TimelineWidget for basic time-range visualization.

use crate::core::{HorizontalAlignment, Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// One timeline entry with inclusive start/end time units.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineItem {
    /// Stable item id.
    pub id: String,
    /// Display label.
    pub label: String,
    /// Start point.
    pub start: i64,
    /// End point.
    pub end: i64,
}

impl TimelineItem {
    /// Creates a timeline item.
    pub fn new(id: impl Into<String>, label: impl Into<String>, start: i64, end: i64) -> Self {
        let normalized_end = end.max(start);
        Self { id: id.into(), label: label.into(), start, end: normalized_end }
    }
}

/// Basic vertical timeline widget with selectable rows and zoomable viewport.
pub struct TimelineWidget {
    base: BaseWidget,
    items: Vec<TimelineItem>,
    selected_index: Option<usize>,
    hovered_index: Option<usize>,
    viewport_start: i64,
    viewport_end: i64,
    row_height: u32,
    /// Emitted when selected item changes. Payload is selected item id.
    pub item_selected: Signal1<String>,
}

impl TimelineWidget {
    /// Creates empty timeline widget.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Chart, geometry, "TimelineWidget"),
            items: Vec::new(),
            selected_index: None,
            hovered_index: None,
            viewport_start: 0,
            viewport_end: 100,
            row_height: 24,
            item_selected: Signal1::new(),
        }
    }

    /// Replaces timeline items.
    pub fn set_items(&mut self, items: Vec<TimelineItem>) {
        self.items = items;
        self.selected_index = None;
        self.recompute_viewport();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns timeline items.
    pub fn items(&self) -> &[TimelineItem] {
        &self.items
    }

    /// Returns selected item index.
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index.filter(|index| *index < self.items.len())
    }

    /// Returns selected item id.
    pub fn selected_id(&self) -> Option<&str> {
        let index = self.selected_index()?;
        self.items.get(index).map(|item| item.id.as_str())
    }

    /// Returns viewport tuple `(start, end)`.
    pub fn viewport(&self) -> (i64, i64) {
        (self.viewport_start, self.viewport_end)
    }

    /// Sets viewport range.
    pub fn set_viewport(&mut self, start: i64, end: i64) {
        self.viewport_start = start;
        self.viewport_end = end.max(start + 1);
        self.base.request_redraw();
    }

    /// Zooms around current viewport center.
    pub fn zoom(&mut self, factor: f32) {
        if factor <= 0.0 {
            return;
        }
        let span = (self.viewport_end - self.viewport_start).max(1) as f32;
        let center = (self.viewport_start + self.viewport_end) as f32 / 2.0;
        let next_half = (span / factor / 2.0).max(1.0);
        let start = (center - next_half).floor() as i64;
        let end = (center + next_half).ceil() as i64;
        self.set_viewport(start, end);
    }

    /// Sets item row height.
    pub fn set_row_height(&mut self, row_height: u32) {
        self.row_height = row_height.max(1);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns item row height.
    pub fn row_height(&self) -> u32 {
        self.row_height
    }

    /// Selects item by index.
    pub fn select_index(&mut self, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }
        if self.selected_index == Some(index) {
            return true;
        }
        self.selected_index = Some(index);
        if let Some(item) = self.items.get(index) {
            self.item_selected.emit(item.id.clone());
        }
        self.base.request_redraw();
        true
    }

    fn recompute_viewport(&mut self) {
        if self.items.is_empty() {
            self.viewport_start = 0;
            self.viewport_end = 100;
            return;
        }

        let mut min_start = i64::MAX;
        let mut max_end = i64::MIN;
        for item in &self.items {
            min_start = min_start.min(item.start);
            max_end = max_end.max(item.end);
        }
        if min_start >= max_end {
            max_end = min_start + 1;
        }
        self.viewport_start = min_start;
        self.viewport_end = max_end;
    }

    fn visible_count(&self) -> usize {
        let height = self.base.geometry().height as usize;
        let row_h = self.row_height as usize;
        if row_h == 0 {
            return 0;
        }
        (height / row_h).max(1)
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
        let index = ((pos.y - rect.y) / self.row_height as i32) as usize;
        (index < self.items.len().min(self.visible_count())).then_some(index)
    }

    fn project_x(&self, value: i64, track_x: i32, track_w: u32) -> i32 {
        let start = self.viewport_start;
        let end = self.viewport_end.max(start + 1);
        let span = (end - start) as f32;
        let ratio = ((value - start) as f32 / span).clamp(0.0, 1.0);
        track_x + (ratio * track_w as f32) as i32
    }
}

impl Widget for TimelineWidget {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl EventHandler for TimelineWidget {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MouseMove { pos } => {
                self.hovered_index = self.row_at(*pos);
            }
            Event::MouseLeave { .. } => {
                self.hovered_index = None;
            }
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

impl Draw for TimelineWidget {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        context.fill_rect(rect, Color::from_rgb(251, 252, 254));
        context.draw_rect(rect, Color::from_rgb(189, 196, 209));

        if self.items.is_empty() {
            return;
        }

        let track_x = rect.x + 120;
        let track_w = rect.width.saturating_sub(130);
        let max_rows = self.visible_count().min(self.items.len());

        for index in 0..max_rows {
            let y = rect.y + index as i32 * self.row_height as i32;
            let row_rect = Rect::new(rect.x, y, rect.width, self.row_height);

            if self.selected_index == Some(index) {
                context.fill_rect(row_rect, Color::from_rgb(220, 233, 251));
            } else if self.hovered_index == Some(index) {
                context.fill_rect(row_rect, Color::from_rgb(237, 244, 253));
            }

            if let Some(item) = self.items.get(index) {
                context.draw_text(
                    Point::new(rect.x + 8, y + self.row_height as i32 / 2),
                    &item.label,
                    &Font::default(),
                    Color::from_rgb(34, 47, 67),
                    HorizontalAlignment::Left,
                );

                let x0 = self.project_x(item.start, track_x, track_w);
                let x1 = self.project_x(item.end, track_x, track_w).max(x0 + 2);
                let bar_w = (x1 - x0) as u32;
                context.fill_rect(
                    Rect::new(x0, y + 6, bar_w, self.row_height.saturating_sub(12)),
                    Color::from_rgb(116, 167, 230),
                );
            }

            context.draw_line(
                Point::new(rect.x, y + self.row_height as i32),
                Point::new(rect.x + rect.width as i32, y + self.row_height as i32),
                Color::from_rgb(228, 232, 239),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn sample_items() -> Vec<TimelineItem> {
        vec![
            TimelineItem::new("a", "Design", 0, 10),
            TimelineItem::new("b", "Build", 8, 22),
            TimelineItem::new("c", "Verify", 20, 30),
        ]
    }

    #[test]
    fn set_items_recomputes_viewport() {
        let mut timeline = TimelineWidget::new(Rect::new(0, 0, 500, 120));
        timeline.set_items(sample_items());

        assert_eq!(timeline.viewport(), (0, 30));
        assert_eq!(timeline.items().len(), 3);
    }

    #[test]
    fn selection_emits_selected_item_id() {
        let mut timeline = TimelineWidget::new(Rect::new(0, 0, 500, 120));
        timeline.set_items(sample_items());

        let emitted = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = emitted.clone();
        timeline.item_selected.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });

        assert!(timeline.select_index(1));
        assert_eq!(timeline.selected_id(), Some("b"));

        let got = emitted.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec!["b".to_string()]);
    }

    #[test]
    fn wheel_zoom_updates_viewport_span() {
        let mut timeline = TimelineWidget::new(Rect::new(0, 0, 500, 120));
        timeline.set_items(sample_items());

        let before = timeline.viewport();
        timeline.handle_event(&Event::wheel(0, -120, 0));
        let after_in = timeline.viewport();
        assert!((after_in.1 - after_in.0) < (before.1 - before.0));

        timeline.handle_event(&Event::wheel(0, 120, 0));
        let after_out = timeline.viewport();
        assert!((after_out.1 - after_out.0) >= (after_in.1 - after_in.0));
    }

    #[test]
    fn new_creates_default_state() {
        let timeline = TimelineWidget::new(Rect::new(0, 0, 800, 600));
        assert!(timeline.items().is_empty());
        assert_eq!(timeline.selected_index(), None);
        assert_eq!(timeline.selected_id(), None);
        assert_eq!(timeline.viewport(), (0, 100));
        assert_eq!(timeline.row_height(), 24);
    }

    #[test]
    fn items_returns_items() {
        let mut timeline = TimelineWidget::new(Rect::new(0, 0, 800, 600));
        let items = sample_items();
        timeline.set_items(items.clone());
        assert_eq!(timeline.items(), items.as_slice());
    }

    #[test]
    fn select_index_out_of_bounds_returns_false() {
        let mut timeline = TimelineWidget::new(Rect::new(0, 0, 800, 600));
        timeline.set_items(sample_items());
        assert!(!timeline.select_index(10));
        assert_eq!(timeline.selected_index(), None);
    }

    #[test]
    fn select_index_duplicate_guard_returns_true() {
        let mut timeline = TimelineWidget::new(Rect::new(0, 0, 800, 600));
        timeline.set_items(sample_items());
        assert!(timeline.select_index(0));
        assert_eq!(timeline.selected_index(), Some(0));
        // Selecting same index again returns true but doesn't emit again
        let emitted = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = emitted.clone();
        timeline.item_selected.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });
        assert!(timeline.select_index(0));
        assert_eq!(emitted.lock().ok().map(|g| g.len()).unwrap_or_default(), 0);
    }

    #[test]
    fn set_viewport_updates_range() {
        let mut timeline = TimelineWidget::new(Rect::new(0, 0, 800, 600));
        timeline.set_viewport(50, 150);
        assert_eq!(timeline.viewport(), (50, 150));
    }

    #[test]
    fn set_viewport_maintains_min_span() {
        let mut timeline = TimelineWidget::new(Rect::new(0, 0, 800, 600));
        timeline.set_viewport(10, 10);
        assert_eq!(timeline.viewport(), (10, 11));
    }

    #[test]
    fn empty_items_returns_default_viewport() {
        let mut timeline = TimelineWidget::new(Rect::new(0, 0, 800, 600));
        timeline.set_items(vec![]);
        assert!(timeline.items().is_empty());
        assert_eq!(timeline.selected_index(), None);
        assert_eq!(timeline.viewport(), (0, 100));
    }

    #[test]
    fn overlapping_items_viewport_spans_union() {
        let items = vec![
            TimelineItem::new("a", "Design", 0, 10),
            TimelineItem::new("b", "Build", 8, 22),
            TimelineItem::new("c", "Verify", 20, 30),
        ];
        let mut timeline = TimelineWidget::new(Rect::new(0, 0, 800, 600));
        timeline.set_items(items);
        assert_eq!(timeline.viewport(), (0, 30));
    }

    #[test]
    fn zoom_with_zero_factor_does_nothing() {
        let mut timeline = TimelineWidget::new(Rect::new(0, 0, 800, 600));
        timeline.set_items(sample_items());
        let before = timeline.viewport();
        timeline.zoom(0.0);
        assert_eq!(timeline.viewport(), before);
    }

    #[test]
    fn row_height_setter_validates() {
        let mut timeline = TimelineWidget::new(Rect::new(0, 0, 800, 600));
        timeline.set_row_height(0);
        assert_eq!(timeline.row_height(), 1);
    }
}
