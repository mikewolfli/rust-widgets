//! Breadcrumb navigation widget.

use crate::core::{Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Single breadcrumb segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreadcrumbSegment {
    /// Stable segment identifier.
    pub id: String,
    /// Visible segment label.
    pub label: String,
}

impl BreadcrumbSegment {
    /// Creates a breadcrumb segment.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self { id: id.into(), label: label.into() }
    }
}

/// Breadcrumb navigation control with keyboard and mouse interaction.
pub struct Breadcrumb {
    base: BaseWidget,
    segments: Vec<BreadcrumbSegment>,
    selected_index: Option<usize>,
    segment_padding: i32,
    separator_width: i32,
    /// Emitted when a segment is activated. Payload is segment id.
    pub segment_activated: Signal1<String>,
}

impl Breadcrumb {
    /// Creates an empty breadcrumb.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Panel, geometry, "Breadcrumb"),
            segments: Vec::new(),
            selected_index: None,
            segment_padding: 8,
            separator_width: 14,
            segment_activated: Signal1::new(),
        }
    }

    /// Replaces full segment path.
    pub fn set_segments(&mut self, segments: Vec<BreadcrumbSegment>) {
        self.segments = segments;
        self.selected_index =
            if self.segments.is_empty() { None } else { Some(self.segments.len() - 1) };
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns immutable segment list.
    pub fn segments(&self) -> &[BreadcrumbSegment] {
        &self.segments
    }

    /// Appends one segment.
    pub fn push_segment(&mut self, segment: BreadcrumbSegment) {
        self.segments.push(segment);
        self.selected_index = Some(self.segments.len() - 1);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Clears segment path.
    pub fn clear_segments(&mut self) {
        self.segments.clear();
        self.selected_index = None;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns selected segment index.
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index.filter(|index| *index < self.segments.len())
    }

    /// Sets selected segment.
    pub fn set_selected_index(&mut self, index: usize) -> bool {
        if index >= self.segments.len() {
            return false;
        }
        if self.selected_index == Some(index) {
            return true;
        }
        self.selected_index = Some(index);
        self.base.request_redraw();
        true
    }

    /// Activates currently selected segment.
    pub fn activate_selected(&mut self) -> bool {
        let Some(index) = self.selected_index() else {
            return false;
        };
        let Some(segment) = self.segments.get(index) else {
            return false;
        };
        self.segment_activated.emit(segment.id.clone());
        true
    }

    /// Moves selection by signed delta.
    pub fn move_selection(&mut self, delta: isize) {
        if self.segments.is_empty() {
            self.selected_index = None;
            return;
        }
        let current = self.selected_index.unwrap_or(0) as isize;
        let max = self.segments.len().saturating_sub(1) as isize;
        let next = (current + delta).clamp(0, max) as usize;
        self.selected_index = Some(next);
        self.base.request_redraw();
    }

    fn segment_width(segment: &BreadcrumbSegment, padding: i32) -> i32 {
        (segment.label.chars().count() as i32) * 8 + padding * 2
    }

    fn segment_rect_at(&self, index: usize) -> Option<Rect> {
        let rect = self.geometry();
        let mut x = rect.x;

        for (i, segment) in self.segments.iter().enumerate() {
            let width = Self::segment_width(segment, self.segment_padding).max(1);
            if i == index {
                return Some(Rect::new(x, rect.y, width as u32, rect.height));
            }
            x += width + self.separator_width;
        }

        None
    }

    fn hit_index(&self, pos: Point) -> Option<usize> {
        let rect = self.geometry();
        if pos.y < rect.y || pos.y >= rect.y + rect.height as i32 {
            return None;
        }

        for index in 0..self.segments.len() {
            let Some(seg_rect) = self.segment_rect_at(index) else {
                continue;
            };
            if pos.x >= seg_rect.x && pos.x < seg_rect.x + seg_rect.width as i32 {
                return Some(index);
            }
        }

        None
    }
}

impl Widget for Breadcrumb {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl EventHandler for Breadcrumb {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button: 1 } => {
                if let Some(index) = self.hit_index(*pos) {
                    let _ = self.set_selected_index(index);
                }
            }
            Event::MouseDoubleClick { pos, button: 1 } => {
                if let Some(index) = self.hit_index(*pos) {
                    let _ = self.set_selected_index(index);
                    let _ = self.activate_selected();
                }
            }
            Event::KeyPress { key, modifiers: _ } => match *key {
                37 => self.move_selection(-1),
                39 => self.move_selection(1),
                13 => {
                    let _ = self.activate_selected();
                }
                _ => {}
            },
            _ => {}
        }
    }
}

impl Draw for Breadcrumb {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        context.fill_rect(rect, Color::from_rgb(249, 250, 252));
        context.draw_rect(rect, Color::from_rgb(206, 211, 220));

        let mut x = rect.x;
        for (index, segment) in self.segments.iter().enumerate() {
            let width = Self::segment_width(segment, self.segment_padding).max(1);
            let segment_rect = Rect::new(x, rect.y, width as u32, rect.height);

            if self.selected_index == Some(index) {
                context.fill_rect(segment_rect, Color::from_rgb(220, 231, 247));
            }

            context.draw_text(
                Point::new(x + self.segment_padding, rect.y + rect.height as i32 / 2),
                &segment.label,
                &Font::default(),
                Color::from_rgb(34, 45, 64),
            );

            x += width;
            if index + 1 < self.segments.len() {
                context.draw_text(
                    Point::new(x + 3, rect.y + rect.height as i32 / 2),
                    ">",
                    &Font::default(),
                    Color::from_rgb(120, 128, 142),
                );
                x += self.separator_width;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn sample_segments() -> Vec<BreadcrumbSegment> {
        vec![
            BreadcrumbSegment::new("root", "Root"),
            BreadcrumbSegment::new("project", "Project"),
            BreadcrumbSegment::new("src", "src"),
        ]
    }

    #[test]
    fn set_segments_selects_last_by_default() {
        let mut breadcrumb = Breadcrumb::new(Rect::new(0, 0, 320, 28));
        breadcrumb.set_segments(sample_segments());

        assert_eq!(breadcrumb.selected_index(), Some(2));
        assert_eq!(breadcrumb.segments().len(), 3);
    }

    #[test]
    fn keyboard_navigation_and_activation_emit_segment_id() {
        let mut breadcrumb = Breadcrumb::new(Rect::new(0, 0, 320, 28));
        breadcrumb.set_segments(sample_segments());

        let activated = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = activated.clone();
        breadcrumb.segment_activated.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });

        breadcrumb.handle_event(&Event::key_press(37, 0));
        assert_eq!(breadcrumb.selected_index(), Some(1));
        breadcrumb.handle_event(&Event::key_press(13, 0));

        let got = activated.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec!["project".to_string()]);
    }

    #[test]
    fn mouse_selection_hits_expected_segment() {
        let mut breadcrumb = Breadcrumb::new(Rect::new(0, 0, 400, 28));
        breadcrumb.set_segments(sample_segments());

        // Root label area
        breadcrumb.handle_event(&Event::mouse_press(12, 10, 1));
        assert_eq!(breadcrumb.selected_index(), Some(0));

        // Project label area after first separator
        breadcrumb.handle_event(&Event::mouse_press(90, 10, 1));
        assert_eq!(breadcrumb.selected_index(), Some(1));
    }

    #[test]
    fn default_state() {
        let breadcrumb = Breadcrumb::new(Rect::new(0, 0, 800, 600));
        assert!(breadcrumb.segments().is_empty());
        assert_eq!(breadcrumb.selected_index(), None);
    }

    #[test]
    fn push_segment_increases_count() {
        let mut breadcrumb = Breadcrumb::new(Rect::new(0, 0, 800, 600));
        breadcrumb.push_segment(BreadcrumbSegment::new("a", "A"));
        assert_eq!(breadcrumb.segments().len(), 1);
        assert_eq!(breadcrumb.selected_index(), Some(0));

        breadcrumb.push_segment(BreadcrumbSegment::new("b", "B"));
        assert_eq!(breadcrumb.segments().len(), 2);
        assert_eq!(breadcrumb.selected_index(), Some(1));

        breadcrumb.push_segment(BreadcrumbSegment::new("c", "C"));
        assert_eq!(breadcrumb.segments().len(), 3);
        assert_eq!(breadcrumb.selected_index(), Some(2));
    }

    #[test]
    fn clear_segments_resets() {
        let mut breadcrumb = Breadcrumb::new(Rect::new(0, 0, 800, 600));
        breadcrumb
            .set_segments(vec![BreadcrumbSegment::new("a", "A"), BreadcrumbSegment::new("b", "B")]);
        assert_eq!(breadcrumb.segments().len(), 2);

        breadcrumb.clear_segments();
        assert!(breadcrumb.segments().is_empty());
        assert_eq!(breadcrumb.selected_index(), None);
    }

    #[test]
    fn empty_breadcrumb_state() {
        let mut breadcrumb = Breadcrumb::new(Rect::new(0, 0, 800, 600));
        // activate on empty = false
        assert!(!breadcrumb.activate_selected());

        // move selection on empty should not panic
        breadcrumb.move_selection(1);
        assert_eq!(breadcrumb.selected_index(), None);

        breadcrumb.move_selection(-1);
        assert_eq!(breadcrumb.selected_index(), None);

        // set_selected_index on empty = false
        assert!(!breadcrumb.set_selected_index(0));
    }

    #[test]
    fn invalid_segment_activation_index() {
        let mut breadcrumb = Breadcrumb::new(Rect::new(0, 0, 800, 600));
        breadcrumb.set_segments(vec![BreadcrumbSegment::new("a", "A")]);

        // Setting index beyond bounds returns false
        assert!(!breadcrumb.set_selected_index(5));

        // Setting valid index returns true
        assert!(breadcrumb.set_selected_index(0));

        // Re-setting same index returns true
        assert!(breadcrumb.set_selected_index(0));
    }
}
