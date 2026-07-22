//! NotificationCenter widget.

use crate::core::{HorizontalAlignment, Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Notification severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationLevel {
    Info,
    Warning,
    Error,
}

/// One notification entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationItem {
    /// Stable notification id.
    pub id: String,
    /// Notification title.
    pub title: String,
    /// Notification body.
    pub message: String,
    /// Severity level.
    pub level: NotificationLevel,
    /// Whether this item has been read.
    pub read: bool,
}

impl NotificationItem {
    /// Creates a notification item.
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        message: impl Into<String>,
        level: NotificationLevel,
    ) -> Self {
        Self { id: id.into(), title: title.into(), message: message.into(), level, read: false }
    }
}

/// Notification list center with unread management.
pub struct NotificationCenter {
    base: BaseWidget,
    items: Vec<NotificationItem>,
    selected_index: Option<usize>,
    row_height: u32,
    /// Emitted when selected notification changes. Payload is id.
    pub notification_selected: Signal1<String>,
    /// Emitted when notification is activated. Payload is id.
    pub notification_activated: Signal1<String>,
    /// Emitted when unread count changes.
    pub unread_count_changed: Signal1<usize>,
}

impl NotificationCenter {
    /// Creates empty notification center.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ListView, geometry, "NotificationCenter"),
            items: Vec::new(),
            selected_index: None,
            row_height: 36,
            notification_selected: Signal1::new(),
            notification_activated: Signal1::new(),
            unread_count_changed: Signal1::new(),
        }
    }

    /// Returns all notifications.
    pub fn items(&self) -> &[NotificationItem] {
        &self.items
    }

    /// Adds a notification.
    pub fn push(&mut self, item: NotificationItem) {
        self.items.push(item);
        if self.selected_index.is_none() {
            self.selected_index = Some(0);
        }
        self.unread_count_changed.emit(self.unread_count());
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Clears all notifications.
    pub fn clear(&mut self) {
        self.items.clear();
        self.selected_index = None;
        self.unread_count_changed.emit(0);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns unread count.
    pub fn unread_count(&self) -> usize {
        self.items.iter().filter(|item| !item.read).count()
    }

    /// Marks a notification read/unread by id.
    pub fn set_read(&mut self, id: &str, read: bool) -> bool {
        let before = self.unread_count();
        for item in &mut self.items {
            if item.id == id {
                item.read = read;
                let after = self.unread_count();
                if before != after {
                    self.unread_count_changed.emit(after);
                }
                self.base.request_redraw();
                return true;
            }
        }
        false
    }

    /// Marks all notifications as read.
    pub fn mark_all_read(&mut self) {
        let mut changed = false;
        for item in &mut self.items {
            if !item.read {
                item.read = true;
                changed = true;
            }
        }
        if changed {
            self.unread_count_changed.emit(0);
            self.base.request_redraw();
        }
    }

    /// Returns selected notification id.
    pub fn selected_id(&self) -> Option<&str> {
        let index = self.selected_index?;
        self.items.get(index).map(|item| item.id.as_str())
    }

    /// Selects notification index.
    pub fn select_index(&mut self, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }
        if self.selected_index == Some(index) {
            return true;
        }
        self.selected_index = Some(index);
        if let Some(item) = self.items.get(index) {
            self.notification_selected.emit(item.id.clone());
        }
        self.base.request_redraw();
        true
    }

    /// Activates current selected notification.
    pub fn activate_selected(&mut self) -> bool {
        let Some(index) = self.selected_index else {
            return false;
        };
        if index >= self.items.len() {
            return false;
        }

        let id = self.items[index].id.clone();
        let changed = !self.items[index].read;
        self.items[index].read = true;
        self.notification_activated.emit(id);

        if changed {
            self.unread_count_changed.emit(self.unread_count());
        }
        self.base.request_redraw();
        true
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
        (index < self.items.len()).then_some(index)
    }
}

impl Widget for NotificationCenter {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(300, 400)
    }
}

impl EventHandler for NotificationCenter {
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
            Event::MouseDoubleClick { pos, button: 1 } => {
                if let Some(index) = self.row_at(*pos) {
                    let _ = self.select_index(index);
                    let _ = self.activate_selected();
                }
            }
            Event::KeyPress { key, modifiers: _ } => match *key {
                38 => {
                    if let Some(index) = self.selected_index {
                        if index > 0 {
                            let _ = self.select_index(index - 1);
                        }
                    } else if !self.items.is_empty() {
                        let _ = self.select_index(0);
                    }
                }
                40 => {
                    if let Some(index) = self.selected_index {
                        if index + 1 < self.items.len() {
                            let _ = self.select_index(index + 1);
                        }
                    } else if !self.items.is_empty() {
                        let _ = self.select_index(0);
                    }
                }
                13 => {
                    let _ = self.activate_selected();
                }
                // Unknown key; ignore
                _ => {}
            },
            // Other events are not relevant for this widget
            _ => {}
        }
    }
}

impl Draw for NotificationCenter {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        context.fill_rect(rect, Color::rgb(250, 251, 253));
        context.draw_rect(rect, Color::rgb(194, 201, 214));

        for (index, item) in self.items.iter().enumerate() {
            let y = rect.y + index as i32 * self.row_height as i32;
            if y >= rect.y + rect.height as i32 {
                break;
            }
            let row_rect = Rect::new(rect.x, y, rect.width, self.row_height);

            let bg = if self.selected_index == Some(index) {
                Color::rgb(220, 232, 249)
            } else if !item.read {
                Color::rgb(240, 246, 255)
            } else {
                Color::rgb(250, 251, 253)
            };
            context.fill_rect(row_rect, bg);

            let badge_color = match item.level {
                NotificationLevel::Info => Color::rgb(74, 122, 199),
                NotificationLevel::Warning => Color::rgb(222, 153, 42),
                NotificationLevel::Error => Color::rgb(208, 82, 72),
            };

            context.fill_rect(Rect::new(rect.x + 8, y + 14, 8, 8), badge_color);
            context.draw_text(
                Point::new(rect.x + 22, y + 14),
                &item.title,
                &Font::default(),
                Color::rgb(40, 51, 68),
                HorizontalAlignment::Left,
            );
            context.draw_text(
                Point::new(rect.x + 22, y + 28),
                &item.message,
                &Font::default(),
                Color::rgb(91, 103, 121),
                HorizontalAlignment::Left,
            );

            context.draw_line(
                Point::new(rect.x, y + self.row_height as i32),
                Point::new(rect.x + rect.width as i32, y + self.row_height as i32),
                Color::rgb(229, 233, 240),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn sample_center() -> NotificationCenter {
        let mut center = NotificationCenter::new(Rect::new(0, 0, 420, 160));
        center.push(NotificationItem::new(
            "n1",
            "Build finished",
            "All checks passed",
            NotificationLevel::Info,
        ));
        center.push(NotificationItem::new(
            "n2",
            "Deploy warning",
            "One region has latency spike",
            NotificationLevel::Warning,
        ));
        center.push(NotificationItem::new(
            "n3",
            "Runtime error",
            "Worker crashed",
            NotificationLevel::Error,
        ));
        center
    }

    #[test]
    fn unread_count_and_mark_read_work() {
        let mut center = sample_center();
        assert_eq!(center.unread_count(), 3);

        assert!(center.set_read("n2", true));
        assert_eq!(center.unread_count(), 2);

        center.mark_all_read();
        assert_eq!(center.unread_count(), 0);
    }

    #[test]
    fn activate_selected_marks_read_and_emits() {
        let mut center = sample_center();
        let activated = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = activated.clone();
        center.notification_activated.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });

        assert!(center.select_index(1));
        assert!(center.activate_selected());
        assert_eq!(center.selected_id(), Some("n2"));
        assert_eq!(center.unread_count(), 2);

        let got = activated.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec!["n2".to_string()]);
    }

    #[test]
    fn keyboard_navigation_changes_selection() {
        let mut center = sample_center();

        center.handle_event(&Event::key_press(40, 0));
        assert_eq!(center.selected_id(), Some("n2"));

        center.handle_event(&Event::key_press(40, 0));
        assert_eq!(center.selected_id(), Some("n3"));

        center.handle_event(&Event::key_press(38, 0));
        assert_eq!(center.selected_id(), Some("n2"));
    }

    #[test]
    fn new_creates_default_state() {
        let center = NotificationCenter::new(Rect::new(0, 0, 800, 600));
        assert!(center.items().is_empty());
        assert_eq!(center.selected_id(), None);
        assert_eq!(center.unread_count(), 0);
    }

    #[test]
    fn items_returns_items() {
        let center = sample_center();
        assert_eq!(center.items().len(), 3);
        assert_eq!(center.items()[0].id, "n1");
        assert_eq!(center.items()[1].id, "n2");
        assert_eq!(center.items()[2].id, "n3");
    }

    #[test]
    fn push_sets_selection_to_first() {
        let mut center = NotificationCenter::new(Rect::new(0, 0, 800, 600));
        center.push(NotificationItem::new("n1", "Title", "Body", NotificationLevel::Info));
        assert_eq!(center.selected_id(), Some("n1"));
    }

    #[test]
    fn clear_removes_all_and_resets_state() {
        let mut center = sample_center();
        center.clear();
        assert!(center.items().is_empty());
        assert_eq!(center.selected_id(), None);
        assert_eq!(center.unread_count(), 0);
    }

    #[test]
    fn select_index_invalid_returns_false() {
        let mut center = NotificationCenter::new(Rect::new(0, 0, 800, 600));
        assert!(!center.select_index(0));

        center.push(NotificationItem::new("n1", "Title", "Body", NotificationLevel::Info));
        assert!(!center.select_index(5));
        assert_eq!(center.selected_id(), Some("n1"));
    }

    #[test]
    fn unread_count_changed_signal_on_push() {
        let mut center = NotificationCenter::new(Rect::new(0, 0, 800, 600));
        let counts = Arc::new(Mutex::new(Vec::<usize>::new()));
        let sink = counts.clone();
        center.unread_count_changed.connect(move |count| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*count);
            }
        });

        center.push(NotificationItem::new("n1", "Title", "Body", NotificationLevel::Info));
        let got = counts.lock().ok().map(|g| g.clone()).unwrap_or_default();
        assert_eq!(got, vec![1]);
    }

    #[test]
    fn keyboard_navigation_on_empty_does_nothing() {
        let mut center = NotificationCenter::new(Rect::new(0, 0, 800, 600));

        // Should not panic
        center.handle_event(&Event::key_press(40, 0));
        assert_eq!(center.selected_id(), None);

        center.handle_event(&Event::key_press(38, 0));
        assert_eq!(center.selected_id(), None);
    }

    #[test]
    fn activate_selected_marks_read_and_updates_unread_count() {
        let mut center = NotificationCenter::new(Rect::new(0, 0, 800, 600));
        center.push(NotificationItem::new("n1", "Title", "Body", NotificationLevel::Info));
        assert_eq!(center.unread_count(), 1);

        // activate_selected on index 0 (first/only item)
        assert!(center.activate_selected());
        assert_eq!(center.unread_count(), 0);
    }

    #[test]
    fn set_read_nonexistent_returns_false() {
        let mut center = sample_center();
        assert!(!center.set_read("nonexistent", true));
        assert_eq!(center.unread_count(), 3);
    }

    #[test]
    fn select_index_duplicate_guard_returns_true() {
        let mut center = sample_center();
        // First select a different index
        assert!(center.select_index(1));

        let emitted = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = emitted.clone();
        center.notification_selected.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });

        // Select index 0 - emits "n1"
        assert!(center.select_index(0));
        // Second call with same index returns true but doesn't emit
        assert!(center.select_index(0));
        let got = emitted.lock().ok().map(|g| g.clone()).unwrap_or_default();
        assert_eq!(got.len(), 1);
    }

    #[test]
    fn enter_key_activates_selected() {
        let mut center = sample_center();
        let activated = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = activated.clone();
        center.notification_activated.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });

        // Navigate down to n2, then activate
        center.handle_event(&Event::key_press(40, 0));
        center.handle_event(&Event::key_press(13, 0));

        let got = activated.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec!["n2".to_string()]);
    }
}
