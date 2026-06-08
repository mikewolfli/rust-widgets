//! Toast stack widget.

use crate::core::{Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Toast severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastLevel {
    Info,
    Success,
    Warning,
    Error,
}

/// One toast message item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToastItem {
    /// Stable toast id.
    pub id: String,
    /// Message text.
    pub message: String,
    /// Severity level.
    pub level: ToastLevel,
    /// Suggested ttl for host runtime.
    pub ttl_ms: u32,
}

impl ToastItem {
    /// Creates toast item.
    pub fn new(
        id: impl Into<String>,
        message: impl Into<String>,
        level: ToastLevel,
        ttl_ms: u32,
    ) -> Self {
        Self { id: id.into(), message: message.into(), level, ttl_ms: ttl_ms.max(100) }
    }
}

/// Toast stack with keyboard/mouse activation and dismiss.
pub struct ToastStack {
    base: BaseWidget,
    toasts: Vec<ToastItem>,
    selected_index: Option<usize>,
    row_height: u32,
    /// Emitted when toast is activated.
    pub toast_activated: Signal1<String>,
    /// Emitted when toast is dismissed.
    pub toast_dismissed: Signal1<String>,
}

impl ToastStack {
    /// Creates empty toast stack.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::PopupWindow, geometry, "ToastStack"),
            toasts: Vec::new(),
            selected_index: None,
            row_height: 30,
            toast_activated: Signal1::new(),
            toast_dismissed: Signal1::new(),
        }
    }

    /// Returns toast items.
    pub fn toasts(&self) -> &[ToastItem] {
        &self.toasts
    }

    /// Adds a toast to the stack.
    pub fn push(&mut self, item: ToastItem) {
        self.toasts.push(item);
        self.selected_index = Some(self.toasts.len() - 1);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Clears all toasts.
    pub fn clear(&mut self) {
        self.toasts.clear();
        self.selected_index = None;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns selected toast id.
    pub fn selected_id(&self) -> Option<&str> {
        let index = self.selected_index?;
        self.toasts.get(index).map(|item| item.id.as_str())
    }

    /// Selects toast by index.
    pub fn select_index(&mut self, index: usize) -> bool {
        if index >= self.toasts.len() {
            return false;
        }
        self.selected_index = Some(index);
        self.base.request_redraw();
        true
    }

    /// Activates selected toast.
    pub fn activate_selected(&mut self) -> bool {
        let Some(index) = self.selected_index else {
            return false;
        };
        let Some(item) = self.toasts.get(index) else {
            return false;
        };
        self.toast_activated.emit(item.id.clone());
        true
    }

    /// Dismisses selected toast.
    pub fn dismiss_selected(&mut self) -> bool {
        let Some(index) = self.selected_index else {
            return false;
        };
        if index >= self.toasts.len() {
            return false;
        }

        let id = self.toasts[index].id.clone();
        self.toasts.remove(index);
        self.toast_dismissed.emit(id);

        if self.toasts.is_empty() {
            self.selected_index = None;
        } else if index >= self.toasts.len() {
            self.selected_index = Some(self.toasts.len() - 1);
        }

        self.base.request_layout();
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

        if self.toasts.is_empty() {
            return None;
        }

        let bottom = rect.y + rect.height as i32;
        for index in 0..self.toasts.len() {
            let top = bottom - ((index + 1) as i32 * self.row_height as i32);
            if pos.y >= top && pos.y < top + self.row_height as i32 {
                return Some(self.toasts.len() - 1 - index);
            }
        }
        None
    }
}

impl Widget for ToastStack {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl EventHandler for ToastStack {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button: 1 } => {
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
                    }
                }
                40 => {
                    if let Some(index) = self.selected_index {
                        if index + 1 < self.toasts.len() {
                            let _ = self.select_index(index + 1);
                        }
                    }
                }
                13 => {
                    let _ = self.activate_selected();
                }
                46 => {
                    let _ = self.dismiss_selected();
                }
                // Unknown key; ignore
                _ => {}
            },
            // Other events are not relevant for this widget
            _ => {}
        }
    }
}

impl Draw for ToastStack {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        context.fill_rect(rect, Color::from_rgb(250, 251, 253));
        context.draw_rect(rect, Color::from_rgb(208, 214, 223));

        let bottom = rect.y + rect.height as i32;
        for (index, item) in self.toasts.iter().enumerate() {
            let visual_order = self.toasts.len() - 1 - index;
            let y = bottom - ((visual_order + 1) as i32 * self.row_height as i32);
            if y < rect.y {
                continue;
            }

            let row =
                Rect::new(rect.x + 4, y + 2, rect.width.saturating_sub(8), self.row_height - 4);
            let bg = if self.selected_index == Some(index) {
                Color::from_rgb(225, 235, 250)
            } else {
                Color::from_rgb(241, 245, 251)
            };
            context.fill_rect(row, bg);
            context.draw_rect(row, Color::from_rgb(184, 194, 208));

            let badge = match item.level {
                ToastLevel::Info => Color::from_rgb(76, 124, 201),
                ToastLevel::Success => Color::from_rgb(58, 161, 103),
                ToastLevel::Warning => Color::from_rgb(220, 158, 54),
                ToastLevel::Error => Color::from_rgb(209, 85, 74),
            };
            context.fill_rect(Rect::new(row.x + 6, row.y + 9, 8, 8), badge);
            context.draw_text(
                Point::new(row.x + 20, row.y + 17),
                &item.message,
                &Font::default(),
                Color::from_rgb(44, 55, 72),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn push_and_dismiss_update_len() {
        let mut stack = ToastStack::new(Rect::new(0, 0, 360, 180));
        stack.push(ToastItem::new("t1", "Saved", ToastLevel::Success, 2500));
        stack.push(ToastItem::new("t2", "Build failed", ToastLevel::Error, 3500));

        assert_eq!(stack.toasts().len(), 2);
        assert_eq!(stack.selected_id(), Some("t2"));

        assert!(stack.dismiss_selected());
        assert_eq!(stack.toasts().len(), 1);
        assert_eq!(stack.selected_id(), Some("t1"));
    }

    #[test]
    fn activate_emits_signal() {
        let mut stack = ToastStack::new(Rect::new(0, 0, 360, 180));
        stack.push(ToastItem::new("t1", "Saved", ToastLevel::Success, 2500));

        let activated = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = activated.clone();
        stack.toast_activated.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });

        assert!(stack.activate_selected());
        let got = activated.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec!["t1".to_string()]);
    }

    #[test]
    fn delete_key_dismisses_selected() {
        let mut stack = ToastStack::new(Rect::new(0, 0, 360, 180));
        stack.push(ToastItem::new("t1", "A", ToastLevel::Info, 1000));
        stack.push(ToastItem::new("t2", "B", ToastLevel::Warning, 1000));

        stack.handle_event(&Event::key_press(46, 0));
        assert_eq!(stack.toasts().len(), 1);
        assert_eq!(stack.selected_id(), Some("t1"));
    }

    #[test]
    fn new_creates_default_state() {
        let stack = ToastStack::new(Rect::new(0, 0, 800, 600));
        assert!(stack.toasts().is_empty());
        assert_eq!(stack.selected_id(), None);
    }

    #[test]
    fn clear_removes_all() {
        let mut stack = ToastStack::new(Rect::new(0, 0, 800, 600));
        stack.push(ToastItem::new("t1", "A", ToastLevel::Info, 1000));
        stack.push(ToastItem::new("t2", "B", ToastLevel::Success, 2000));
        assert_eq!(stack.toasts().len(), 2);

        stack.clear();
        assert!(stack.toasts().is_empty());
        assert_eq!(stack.selected_id(), None);
    }

    #[test]
    fn select_index_invalid_returns_false() {
        let mut stack = ToastStack::new(Rect::new(0, 0, 800, 600));
        assert!(!stack.select_index(0));

        stack.push(ToastItem::new("t1", "A", ToastLevel::Info, 1000));
        assert!(!stack.select_index(5));
        assert_eq!(stack.selected_id(), Some("t1")); // still defaults to last
    }

    #[test]
    fn activate_selected_on_empty_returns_false() {
        let mut stack = ToastStack::new(Rect::new(0, 0, 800, 600));
        assert!(!stack.activate_selected());
    }

    #[test]
    fn dismiss_selected_on_empty_returns_false() {
        let mut stack = ToastStack::new(Rect::new(0, 0, 800, 600));
        assert!(!stack.dismiss_selected());
    }

    #[test]
    fn push_sets_selected_to_new_item() {
        let mut stack = ToastStack::new(Rect::new(0, 0, 800, 600));
        stack.push(ToastItem::new("t1", "First", ToastLevel::Info, 1000));
        assert_eq!(stack.selected_id(), Some("t1"));

        stack.push(ToastItem::new("t2", "Second", ToastLevel::Info, 1000));
        assert_eq!(stack.selected_id(), Some("t2"));
    }

    #[test]
    fn keyboard_navigation_up_down() {
        let mut stack = ToastStack::new(Rect::new(0, 0, 360, 180));
        stack.push(ToastItem::new("t1", "First", ToastLevel::Info, 1000));
        stack.push(ToastItem::new("t2", "Second", ToastLevel::Warning, 1000));
        stack.push(ToastItem::new("t3", "Third", ToastLevel::Error, 1000));

        // Default selected is last pushed (t3)
        assert_eq!(stack.selected_id(), Some("t3"));

        // Up arrow to t2
        stack.handle_event(&Event::key_press(38, 0));
        assert_eq!(stack.selected_id(), Some("t2"));

        // Up again to t1
        stack.handle_event(&Event::key_press(38, 0));
        assert_eq!(stack.selected_id(), Some("t1"));

        // Up at top stays
        stack.handle_event(&Event::key_press(38, 0));
        assert_eq!(stack.selected_id(), Some("t1"));

        // Down arrow to t2
        stack.handle_event(&Event::key_press(40, 0));
        assert_eq!(stack.selected_id(), Some("t2"));

        // Down to t3
        stack.handle_event(&Event::key_press(40, 0));
        assert_eq!(stack.selected_id(), Some("t3"));

        // Down at bottom stays
        stack.handle_event(&Event::key_press(40, 0));
        assert_eq!(stack.selected_id(), Some("t3"));
    }

    #[test]
    fn dismiss_selected_emits_signal() {
        let mut stack = ToastStack::new(Rect::new(0, 0, 360, 180));
        stack.push(ToastItem::new("t1", "A", ToastLevel::Info, 1000));
        stack.push(ToastItem::new("t2", "B", ToastLevel::Warning, 1000));

        let dismissed = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = dismissed.clone();
        stack.toast_dismissed.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });

        assert!(stack.dismiss_selected());
        let got = dismissed.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec!["t2".to_string()]);
    }

    #[test]
    fn enter_key_activates_selected() {
        let mut stack = ToastStack::new(Rect::new(0, 0, 360, 180));
        stack.push(ToastItem::new("t1", "A", ToastLevel::Info, 1000));

        let activated = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = activated.clone();
        stack.toast_activated.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });

        stack.handle_event(&Event::key_press(13, 0));
        let got = activated.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec!["t1".to_string()]);
    }

    #[test]
    fn toast_item_ttl_min_100() {
        let item = ToastItem::new("id", "msg", ToastLevel::Info, 20);
        assert_eq!(item.ttl_ms, 100);
    }
}
