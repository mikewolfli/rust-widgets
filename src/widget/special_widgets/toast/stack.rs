// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `ToastStack` — the container that queues toasts and lays them out as rows.

use super::item::{ToastItem, ToastLevel};
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

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
