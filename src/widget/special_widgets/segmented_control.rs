//! SegmentedControl widget.

use crate::core::{Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Single segment entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentItem {
    /// Stable item id.
    pub id: String,
    /// Display label.
    pub label: String,
}

impl SegmentItem {
    /// Creates one segment item.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

/// Single-selection segmented control.
pub struct SegmentedControl {
    base: BaseWidget,
    items: Vec<SegmentItem>,
    selected_index: Option<usize>,
    hovered_index: Option<usize>,
    /// Emitted when selected segment changes. Payload is selected id.
    pub selection_changed: Signal1<String>,
}

impl SegmentedControl {
    /// Creates an empty segmented control.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ToggleButton, geometry, "SegmentedControl"),
            items: Vec::new(),
            selected_index: None,
            hovered_index: None,
            selection_changed: Signal1::new(),
        }
    }

    /// Replaces all segment items.
    pub fn set_items(&mut self, items: Vec<SegmentItem>) {
        self.items = items;
        self.selected_index = if self.items.is_empty() { None } else { Some(0) };
        self.hovered_index = self.selected_index;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns all segment items.
    pub fn items(&self) -> &[SegmentItem] {
        &self.items
    }

    /// Returns selected segment index.
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
            .filter(|index| *index < self.items.len())
    }

    /// Returns selected segment id.
    pub fn selected_id(&self) -> Option<&str> {
        let index = self.selected_index()?;
        self.items.get(index).map(|item| item.id.as_str())
    }

    /// Sets selected segment index.
    pub fn set_selected_index(&mut self, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }
        if self.selected_index == Some(index) {
            return true;
        }
        self.selected_index = Some(index);
        if let Some(item) = self.items.get(index) {
            self.selection_changed.emit(item.id.clone());
        }
        self.base.request_redraw();
        true
    }

    /// Moves selection by signed delta.
    pub fn move_selection(&mut self, delta: isize) {
        if self.items.is_empty() {
            self.selected_index = None;
            return;
        }
        let current = self.selected_index.unwrap_or(0) as isize;
        let max = self.items.len().saturating_sub(1) as isize;
        let next = (current + delta).clamp(0, max) as usize;
        let _ = self.set_selected_index(next);
    }

    fn segment_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.items.len() {
            return None;
        }
        let rect = self.geometry();
        if self.items.is_empty() {
            return None;
        }
        let width = (rect.width as usize / self.items.len()).max(1) as u32;
        let x = rect.x + index as i32 * width as i32;
        let mut actual_width = width;
        if index + 1 == self.items.len() {
            let consumed = width.saturating_mul(index as u32);
            actual_width = rect.width.saturating_sub(consumed);
        }
        Some(Rect::new(x, rect.y, actual_width, rect.height))
    }

    fn hit_index(&self, pos: Point) -> Option<usize> {
        let rect = self.geometry();
        if pos.x < rect.x
            || pos.x >= rect.x + rect.width as i32
            || pos.y < rect.y
            || pos.y >= rect.y + rect.height as i32
        {
            return None;
        }

        for index in 0..self.items.len() {
            let Some(seg) = self.segment_rect(index) else {
                continue;
            };
            if pos.x >= seg.x && pos.x < seg.x + seg.width as i32 {
                return Some(index);
            }
        }
        None
    }
}

impl Widget for SegmentedControl {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl EventHandler for SegmentedControl {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MouseMove { pos } => {
                self.hovered_index = self.hit_index(*pos);
            }
            Event::MouseLeave { .. } => {
                self.hovered_index = None;
            }
            Event::MousePress { pos, button: 1 } => {
                if let Some(index) = self.hit_index(*pos) {
                    let _ = self.set_selected_index(index);
                }
            }
            Event::KeyPress { key, modifiers: _ } => match *key {
                37 => self.move_selection(-1),
                39 => self.move_selection(1),
                _ => {}
            },
            _ => {}
        }
    }
}

impl Draw for SegmentedControl {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        context.fill_rect(rect, Color::from_rgb(244, 246, 250));
        context.draw_rect(rect, Color::from_rgb(186, 193, 206));

        for index in 0..self.items.len() {
            let Some(seg) = self.segment_rect(index) else {
                continue;
            };

            let bg = if self.selected_index == Some(index) {
                Color::from_rgb(203, 223, 250)
            } else if self.hovered_index == Some(index) {
                Color::from_rgb(225, 236, 251)
            } else {
                Color::from_rgb(244, 246, 250)
            };
            context.fill_rect(seg, bg);

            if index > 0 {
                context.draw_line(
                    Point::new(seg.x, seg.y),
                    Point::new(seg.x, seg.y + seg.height as i32),
                    Color::from_rgb(186, 193, 206),
                );
            }

            if let Some(item) = self.items.get(index) {
                context.draw_text(
                    Point::new(seg.x + 8, seg.y + seg.height as i32 / 2),
                    &item.label,
                    &Font::default(),
                    Color::from_rgb(36, 48, 66),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn sample_items() -> Vec<SegmentItem> {
        vec![
            SegmentItem::new("overview", "Overview"),
            SegmentItem::new("details", "Details"),
            SegmentItem::new("history", "History"),
        ]
    }

    #[test]
    fn set_items_selects_first_item() {
        let mut control = SegmentedControl::new(Rect::new(0, 0, 300, 30));
        control.set_items(sample_items());

        assert_eq!(control.selected_index(), Some(0));
        assert_eq!(control.selected_id(), Some("overview"));
    }

    #[test]
    fn keyboard_navigation_updates_selection() {
        let mut control = SegmentedControl::new(Rect::new(0, 0, 300, 30));
        control.set_items(sample_items());

        control.handle_event(&Event::key_press(39, 0));
        assert_eq!(control.selected_id(), Some("details"));

        control.handle_event(&Event::key_press(39, 0));
        assert_eq!(control.selected_id(), Some("history"));

        control.handle_event(&Event::key_press(37, 0));
        assert_eq!(control.selected_id(), Some("details"));
    }

    #[test]
    fn selection_changed_emits_selected_id() {
        let mut control = SegmentedControl::new(Rect::new(0, 0, 300, 30));
        control.set_items(sample_items());

        let emitted = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = emitted.clone();
        control.selection_changed.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });

        let _ = control.set_selected_index(2);
        let got = emitted
            .lock()
            .ok()
            .map(|guard| guard.clone())
            .unwrap_or_default();
        assert_eq!(got, vec!["history".to_string()]);
    }

    #[test]
    fn default_state() {
        let control = SegmentedControl::new(Rect::new(0, 0, 800, 600));
        assert!(control.items().is_empty());
        assert_eq!(control.selected_index(), None);
        assert_eq!(control.selected_id(), None);
    }

    #[test]
    fn set_selected_index_get_set() {
        let mut control = SegmentedControl::new(Rect::new(0, 0, 800, 600));
        control.set_items(vec![
            SegmentItem::new("tab1", "Tab 1"),
            SegmentItem::new("tab2", "Tab 2"),
            SegmentItem::new("tab3", "Tab 3"),
        ]);

        assert_eq!(control.selected_index(), Some(0));
        assert_eq!(control.selected_id(), Some("tab1"));

        assert!(control.set_selected_index(2));
        assert_eq!(control.selected_index(), Some(2));
        assert_eq!(control.selected_id(), Some("tab3"));

        assert!(control.set_selected_index(0));
        assert_eq!(control.selected_index(), Some(0));
        assert_eq!(control.selected_id(), Some("tab1"));
    }

    #[test]
    fn invalid_index_handling() {
        let mut control = SegmentedControl::new(Rect::new(0, 0, 800, 600));
        control.set_items(vec![SegmentItem::new("a", "A")]);

        // Out of bounds returns false
        assert!(!control.set_selected_index(10));
        assert_eq!(control.selected_index(), Some(0));

        // Valid index returns true
        assert!(control.set_selected_index(0));

        // Move out of bounds clamped
        control.move_selection(10);
        assert_eq!(control.selected_index(), Some(0));

        control.move_selection(-10);
        assert_eq!(control.selected_index(), Some(0));
    }

    #[test]
    fn empty_segments() {
        let mut control = SegmentedControl::new(Rect::new(0, 0, 800, 600));
        // Navigation on empty should not panic
        control.move_selection(1);
        assert_eq!(control.selected_index(), None);

        // set_selected_index on empty returns false
        assert!(!control.set_selected_index(0));

        // handle event on empty should not panic
        control.handle_event(&Event::key_press(39, 0));
        assert_eq!(control.selected_index(), None);

        control.handle_event(&Event::key_press(37, 0));
        assert_eq!(control.selected_index(), None);
    }

    #[test]
    fn move_selection_previous_next() {
        let mut control = SegmentedControl::new(Rect::new(0, 0, 800, 600));
        control.set_items(vec![
            SegmentItem::new("x", "X"),
            SegmentItem::new("y", "Y"),
            SegmentItem::new("z", "Z"),
        ]);

        // Start at 0, move forward
        control.move_selection(1);
        assert_eq!(control.selected_id(), Some("y"));

        control.move_selection(1);
        assert_eq!(control.selected_id(), Some("z"));

        // Move backward
        control.move_selection(-1);
        assert_eq!(control.selected_id(), Some("y"));

        control.move_selection(-1);
        assert_eq!(control.selected_id(), Some("x"));
    }
}
