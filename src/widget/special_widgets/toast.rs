// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Toast stack widget.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Toast severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastLevel {
    /// Neutral informational message.
    Info,
    /// Positive outcome confirmation.
    Success,
    /// Non-fatal problem the user may want to act on.
    Warning,
    /// Failure the user must notice; hosts typically keep these on screen
    /// longer than the other levels.
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

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(300, 48)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `ToastStack`'s property contract.
///
/// `toast_count` and `selected_id` describe the live stack. `row_height` is the
/// one writable name here: the stack lays its toasts out bottom-up from the
/// geometry it was given, so its row height must stay adjustable when a host
/// changes density.
impl WidgetProperties for ToastStack {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "toast_count" => Ok(CapabilityValue::UInt(self.toasts().len() as u64)),
            "selected_id" => match self.selected_id() {
                Some(id) => Ok(CapabilityValue::String(id.to_string())),
                None => Ok(CapabilityValue::Null),
            },
            "row_height" => Ok(CapabilityValue::UInt(self.row_height as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "row_height" => match value {
                CapabilityValue::UInt(height) => {
                    let height =
                        u32::try_from(height).map_err(|_| CapabilityAccessError::TypeMismatch)?;
                    self.row_height = height.max(1);
                    self.base.request_layout();
                    self.base.request_redraw();
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "toast_count" | "selected_id" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["toast_count", "selected_id", "row_height", BASE_PROPERTY_NAMES]
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
        context.fill_rect(rect, Color::rgb(250, 251, 253));
        context.draw_rect(rect, Color::rgb(208, 214, 223));

        let bottom = rect.y + rect.height as i32;
        for (index, item) in self.toasts.iter().enumerate() {
            let visual_order = self.toasts.len() - 1 - index;
            let y = bottom - ((visual_order + 1) as i32 * self.row_height as i32);
            if y < rect.y {
                continue;
            }

            let row = Rect::new(
                rect.x + 4,
                y + 2,
                rect.width.saturating_sub(8),
                self.row_height.saturating_sub(4),
            );
            let bg = if self.selected_index == Some(index) {
                Color::rgb(225, 235, 250)
            } else {
                Color::rgb(241, 245, 251)
            };
            context.fill_rect(row, bg);
            context.draw_rect(row, Color::rgb(184, 194, 208));

            let badge = match item.level {
                ToastLevel::Info => Color::rgb(76, 124, 201),
                ToastLevel::Success => Color::rgb(58, 161, 103),
                ToastLevel::Warning => Color::rgb(220, 158, 54),
                ToastLevel::Error => Color::rgb(209, 85, 74),
            };
            context.fill_rect(Rect::new(row.x + 6, row.y + 9, 8, 8), badge);
            context.draw_text(
                Point::new(row.x + 20, row.y + 17),
                &item.message,
                &Font::default(),
                Color::rgb(44, 55, 72),
                HorizontalAlignment::Left,
            );
        }
    }
}

/// A single transient notification message.
///
/// # Why this is not `ToastStack`
///
/// `ToastStack` is a *container*: it queues items, lays them out as rows, and
/// tracks a selection among them. The common case — show one message — does not
/// need any of that, and forcing it through the stack made callers construct an
/// item, own a stack, and push into it just to display one line.
///
/// This control owns one message and one level, and emits `dismissed` by itself,
/// so a host can mount it, connect the signal, and forget it. It deliberately does
/// not schedule its own expiry: a UI library has no clock it can trust, and a
/// control that silently disappears on a timer is untestable. The host owns the
/// timer and calls [`Self::dismiss`], which is what makes the ttl data rather than
/// behaviour.
pub struct Toast {
    base: BaseWidget,
    message: String,
    level: ToastLevel,
    ttl_ms: u32,
    dismissible: bool,
    /// Emitted when the toast is dismissed, with its message.
    pub dismissed: Signal1<String>,
}

impl Toast {
    /// Creates a toast showing `message` at [`ToastLevel::Info`].
    pub fn new(geometry: Rect, message: impl Into<String>) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Toast, geometry, "Toast"),
            message: message.into(),
            level: ToastLevel::Info,
            ttl_ms: 3000,
            dismissible: true,
            dismissed: Signal1::new(),
        }
    }

    /// Returns the message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Sets the message.
    pub fn set_message(&mut self, message: impl Into<String>) {
        self.message = message.into();
        self.base.request_redraw();
    }

    /// Returns the severity level.
    pub fn level(&self) -> ToastLevel {
        self.level
    }

    /// Sets the severity level.
    pub fn set_level(&mut self, level: ToastLevel) {
        self.level = level;
        self.base.request_redraw();
    }

    /// Returns the host-owned time-to-live hint in milliseconds.
    pub fn ttl_ms(&self) -> u32 {
        self.ttl_ms
    }

    /// Sets the time-to-live hint. Values below 100 ms are raised to it, matching
    /// [`ToastItem::new`]: a toast shorter than a frame is not a duration.
    pub fn set_ttl_ms(&mut self, ttl_ms: u32) {
        self.ttl_ms = ttl_ms.max(100);
    }

    /// Whether the close affordance is shown.
    pub fn is_dismissible(&self) -> bool {
        self.dismissible
    }

    /// Shows or hides the close affordance.
    pub fn set_dismissible(&mut self, dismissible: bool) {
        self.dismissible = dismissible;
        self.base.request_redraw();
    }

    /// Dismisses the toast, emitting `dismissed`.
    ///
    /// Emitting is unconditional rather than guarded on `dismissible`: that flag
    /// controls whether the *user* is offered a close button, not whether the
    /// program may end the toast. A non-dismissible toast still expires.
    pub fn dismiss(&mut self) {
        self.dismissed.emit(self.message.clone());
    }

    /// The rectangle of the close affordance, when it is shown.
    fn close_rect(&self) -> Option<Rect> {
        if !self.dismissible {
            return None;
        }
        let rect = self.geometry();
        let size = 14.min(rect.height);
        Some(Rect::new(
            rect.x + rect.width as i32 - size as i32 - 6,
            rect.y + (rect.height as i32 - size as i32) / 2,
            size,
            size,
        ))
    }

    /// Whether `pos` is over the close affordance.
    fn is_over_close(&self, pos: Point) -> bool {
        self.close_rect().is_some_and(|close| {
            pos.x >= close.x
                && pos.x < close.x + close.width as i32
                && pos.y >= close.y
                && pos.y < close.y + close.height as i32
        })
    }
}

impl Widget for Toast {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    /// One text row plus padding, matching a toast's shape in both the Material and
    /// the desktop conventions.
    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(320, 48)
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

impl WidgetProperties for Toast {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "message" | "text" => Ok(CapabilityValue::String(self.message.clone())),
            "level" => Ok(CapabilityValue::String(toast_level_token(self.level).to_string())),
            "ttl_ms" => Ok(CapabilityValue::UInt(self.ttl_ms as u64)),
            "dismissible" => Ok(CapabilityValue::Bool(self.dismissible)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "message" | "text" => match value {
                CapabilityValue::String(text) => {
                    self.set_message(text);
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "level" => match value {
                CapabilityValue::String(token) => {
                    let level =
                        parse_toast_level(&token).ok_or(CapabilityAccessError::TypeMismatch)?;
                    self.set_level(level);
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "ttl_ms" => match value {
                CapabilityValue::UInt(ms) => {
                    self.set_ttl_ms(u32::try_from(ms).unwrap_or(u32::MAX));
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "dismissible" => match value {
                CapabilityValue::Bool(flag) => {
                    self.set_dismissible(flag);
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["message", "level", "ttl_ms", "dismissible", BASE_PROPERTY_NAMES]
    }
}

impl EventHandler for Toast {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button: 1 } if self.is_over_close(*pos) => {
                self.dismiss();
            }
            Event::KeyPress { key: 27, modifiers: _ } => {
                self.dismiss();
            }
            // Other events are not relevant for this widget
            _ => {}
        }
    }
}

impl Draw for Toast {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let (accent, background) = match self.level {
            ToastLevel::Info => (Color::rgb(76, 124, 201), Color::rgb(240, 245, 253)),
            ToastLevel::Success => (Color::rgb(58, 161, 103), Color::rgb(240, 250, 244)),
            ToastLevel::Warning => (Color::rgb(220, 158, 54), Color::rgb(253, 249, 240)),
            ToastLevel::Error => (Color::rgb(209, 85, 74), Color::rgb(253, 242, 241)),
        };

        context.fill_rect(rect, background);
        context.draw_rect(rect, Color::rgb(208, 214, 223));
        // A severity stripe rather than a badge: at toast height there is no room
        // for a square, and a stripe reads at any width.
        context.fill_rect(Rect::new(rect.x, rect.y, 4, rect.height), accent);

        let text_x = rect.x + 12;
        context.draw_text(
            Point::new(text_x, rect.y + (rect.height as i32 + 12) / 2),
            &self.message,
            &Font::default(),
            Color::rgb(44, 55, 72),
            HorizontalAlignment::Left,
        );

        if let Some(close) = self.close_rect() {
            context.draw_line(
                Point::new(close.x + 3, close.y + 3),
                Point::new(close.x + close.width as i32 - 3, close.y + close.height as i32 - 3),
                Color::rgb(120, 131, 148),
            );
            context.draw_line(
                Point::new(close.x + close.width as i32 - 3, close.y + 3),
                Point::new(close.x + 3, close.y + close.height as i32 - 3),
                Color::rgb(120, 131, 148),
            );
        }
    }
}

/// Token spelling of a [`ToastLevel`], as the property contract publishes it.
fn toast_level_token(level: ToastLevel) -> &'static str {
    match level {
        ToastLevel::Info => "info",
        ToastLevel::Success => "success",
        ToastLevel::Warning => "warning",
        ToastLevel::Error => "error",
    }
}

/// Inverse of [`toast_level_token`]; `None` for an unrecognised token.
fn parse_toast_level(token: &str) -> Option<ToastLevel> {
    match token {
        "info" => Some(ToastLevel::Info),
        "success" => Some(ToastLevel::Success),
        "warning" => Some(ToastLevel::Warning),
        "error" => Some(ToastLevel::Error),
        _ => None,
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
