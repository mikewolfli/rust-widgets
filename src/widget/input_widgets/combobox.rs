// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Combo box widget.
use crate::compat::{String, ToString, Vec};
use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;

use crate::widget::capability::coercion::{expect_bool, expect_string, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
/// Combo box widget.
pub struct ComboBox {
    base: BaseWidget,
    items: Vec<String>,
    current_index: Option<usize>,
    editable: bool,
    max_visible_items: usize,
    /// Emitted with the new index after `current_index` changes, including when
    /// it is cleared to `None`. An out-of-range index is ignored (and emits
    /// nothing); re-applying the same value emits nothing.
    pub current_index_changed: Signal1<Option<usize>>,
    /// Emitted with the text of the new item after `current_index` changes; the
    /// empty string is emitted when the index is cleared.
    pub current_text_changed: Signal1<String>,
    /// Emitted after `current_index_changed` when the user activates an item
    /// (click, or keyboard confirm) with the activated item's index. Not emitted
    /// by programmatic `set_current_index`.
    pub activated: Signal1<usize>,
}
impl ComboBox {
    /// Creates an empty combo box with geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ComboBox, geometry, "ComboBox"),
            items: Vec::new(),
            current_index: None,
            editable: false,
            max_visible_items: 10,
            current_index_changed: Signal1::new(),
            current_text_changed: Signal1::new(),
            activated: Signal1::new(),
        }
    }
    /// Returns number of items.
    pub fn count(&self) -> usize {
        self.items.len()
    }
    /// Returns whether the combo box is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
    /// Returns item at specified index.
    pub fn item(&self, index: usize) -> Option<&str> {
        self.items.get(index).map(|s| s.as_str())
    }
    /// Adds an item.
    pub fn add_item(&mut self, text: String) {
        self.items.push(text);
    }
    /// Adds multiple items.
    pub fn add_items(&mut self, items: Vec<String>) {
        self.items.extend(items);
    }
    /// Replaces all items with the given items. Clears the current selection.
    pub fn set_items(&mut self, items: Vec<String>) {
        self.items = items;
        self.current_index = None;
        self.current_index_changed.emit(None);
        self.current_text_changed.emit(String::new());
    }
    /// Inserts an item at specified position.
    pub fn insert_item(&mut self, index: usize, text: String) {
        if index <= self.items.len() {
            self.items.insert(index, text);
            // Adjust current index if needed
            if let Some(current) = &mut self.current_index {
                if index <= *current {
                    *current += 1;
                }
            }
        }
    }
    /// Removes item at specified index.
    pub fn remove_item(&mut self, index: usize) {
        if index < self.items.len() {
            self.items.remove(index);
            // Adjust current index if needed
            if let Some(current) = &mut self.current_index {
                if index == *current {
                    self.current_index = None;
                    self.current_text_changed.emit(String::new());
                    self.current_index_changed.emit(None);
                } else if index < *current {
                    *current -= 1;
                }
            }
        }
    }
    /// Clears all items.
    pub fn clear(&mut self) {
        self.items.clear();
        self.current_index = None;
        self.current_text_changed.emit(String::new());
        self.current_index_changed.emit(None);
    }
    /// Returns current index.
    pub fn current_index(&self) -> Option<usize> {
        self.current_index
    }
    /// Sets current index.
    pub fn set_current_index(&mut self, index: Option<usize>) {
        if index == self.current_index {
            return;
        }
        if let Some(idx) = index {
            if idx < self.items.len() {
                self.current_index = Some(idx);
                self.current_text_changed.emit(self.items[idx].clone());
                self.current_index_changed.emit(Some(idx));
            }
        } else {
            self.current_index = None;
            self.current_text_changed.emit(String::new());
            self.current_index_changed.emit(None);
        }
        self.base.request_redraw();
    }
    /// Returns current text.
    pub fn current_text(&self) -> String {
        self.current_index.and_then(|idx| self.items.get(idx)).cloned().unwrap_or_default()
    }
    /// Sets current text (for editable combo boxes).
    ///
    /// When the text matches an existing item, that item becomes current. When
    /// it does not match and the box is editable, the text is added as a new
    /// item, which then becomes current — the usual editable-combobox contract of
    /// "type a custom value and it is kept". An empty string selects nothing.
    /// Non-editable boxes ignore the call.
    pub fn set_current_text(&mut self, text: String) {
        if !self.editable {
            return;
        }
        let index = self.items.iter().position(|item| item == &text);
        let index = match index {
            Some(idx) => Some(idx),
            None if !text.is_empty() => {
                self.items.push(text);
                Some(self.items.len() - 1)
            }
            None => None,
        };
        self.set_current_index(index);
    }
    /// Returns whether the combo box is editable.
    pub fn is_editable(&self) -> bool {
        self.editable
    }
    /// Sets editable state.
    pub fn set_editable(&mut self, editable: bool) {
        self.editable = editable;
        self.base.request_redraw();
    }
    /// Returns maximum number of visible items in dropdown.
    pub fn max_visible_items(&self) -> usize {
        self.max_visible_items
    }
    /// Sets maximum number of visible items in dropdown.
    pub fn set_max_visible_items(&mut self, max: usize) {
        self.max_visible_items = max.max(1);
    }
    /// Finds index of item with specified text.
    pub fn find_text(&self, text: &str) -> Option<usize> {
        self.items.iter().position(|item| item == text)
    }
    /// Returns all items.
    pub fn items(&self) -> &[String] {
        &self.items
    }
    /// Shared activation logic for mouse/touch/gesture input.
    fn activate_combo(&mut self) {
        self.base.clicked.emit();
        if !self.items.is_empty() {
            let new_index = if let Some(current) = self.current_index {
                (current + 1) % self.items.len()
            } else {
                0
            };
            self.set_current_index(Some(new_index));
            self.activated.emit(new_index);
        }
    }
}
// Implement Widget trait
impl Widget for ComboBox {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        // Find widest item
        let max_w = self.items().iter().map(|s| s.len() as u32).max().unwrap_or(8) * 8 + 30; // + dropdown arrow
        Size::new(max_w.max(80), 24)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `ComboBox`'s property contract.
impl WidgetProperties for ComboBox {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "item_count" => Ok(CapabilityValue::UInt(self.count() as u64)),
            "current_index" => match self.current_index() {
                Some(idx) => Ok(CapabilityValue::UInt(idx as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "current_text" => Ok(CapabilityValue::String(self.current_text().to_string())),
            "editable" => Ok(CapabilityValue::Bool(self.is_editable())),
            "max_visible_items" => Ok(CapabilityValue::UInt(self.max_visible_items() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "current_index" => {
                match value {
                    CapabilityValue::Null => self.set_current_index(None),
                    other => self.set_current_index(Some(expect_usize(other)?)),
                }
                Ok(())
            }
            "current_text" => {
                self.set_current_text(expect_string(value)?);
                Ok(())
            }
            "editable" => {
                self.set_editable(expect_bool(value)?);
                Ok(())
            }
            "max_visible_items" => {
                self.set_max_visible_items(expect_usize(value)?);
                Ok(())
            }
            // Derived from the item list.
            "item_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `COMBO_BOX_PROPERTIES`.
        property_names_of![
            "item_count",
            "current_index",
            "current_text",
            "editable",
            "max_visible_items",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `combo_box` publishes.
    ///
    /// `clear` empties the item list and drops the selection — the only zero-argument
    /// action in the set. `set_items` and `set_current_index` assign state and need a
    /// payload, so they are answered through the property route: `OutOfRange` tells the
    /// caller the name is right and the property form is the one to use.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "clear" => {
                self.clear();
                Ok(())
            }
            "set_items" | "set_current_index" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for ComboBox {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { button, .. } if *button == 1 => {
                self.activate_combo();
            }
            #[cfg(feature = "touch")]
            Event::TouchBegin { .. } | Event::Tap { .. } => {
                self.activate_combo();
            }
            Event::KeyPress { key, modifiers: _ } => {
                match *key {
                    38 => {
                        // Up arrow - previous item
                        if let Some(current) = self.current_index {
                            if current > 0 {
                                self.set_current_index(Some(current - 1));
                                self.activated.emit(current - 1);
                            }
                        } else if !self.items.is_empty() {
                            self.set_current_index(Some(self.items.len() - 1));
                            self.activated.emit(self.items.len() - 1);
                        }
                    }
                    40 => {
                        // Down arrow - next item
                        if let Some(current) = self.current_index {
                            if current < self.items.len() - 1 {
                                self.set_current_index(Some(current + 1));
                                self.activated.emit(current + 1);
                            }
                        } else if !self.items.is_empty() {
                            self.set_current_index(Some(0));
                            self.activated.emit(0);
                        }
                    }
                    13 => {
                        // Enter - activate current item
                        if let Some(current) = self.current_index {
                            self.activated.emit(current);
                        }
                    }
                    // Unknown key; ignore
                    _ => {}
                }
            }
            // Other events are not relevant for this widget
            _ => {}
        }
    }
}
impl Draw for ComboBox {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw base widget
        let rect = self.geometry();
        let style = self.style();
        let padding = 4;
        let text_x = rect.x + padding;
        let text_y = rect.y as f32 + rect.height as f32 / 2.0;
        // Draw background
        let bg = style.background_color.unwrap_or(Color::rgb(255, 255, 255));
        context.fill_rect(Rect::new(rect.x, rect.y, rect.width, rect.height), bg);
        // Draw border
        if let Some(border_color) = style.border_color {
            context.draw_rect(Rect::new(rect.x, rect.y, rect.width, rect.height), border_color);
        }
        // Draw dropdown arrow
        let arrow_color = style.text_color.unwrap_or(Color::rgb(100, 100, 100));
        let arrow_size = 8;
        let arrow_x_f = rect.x as f32 + rect.width as f32 - padding as f32 - arrow_size as f32;
        let arrow_y_f = rect.y as f32 + rect.height as f32 / 2.0;
        let arrow_size_f = arrow_size as f32;
        // Draw arrow (triangle)
        context.draw_line(
            Point::from_f32(arrow_x_f, arrow_y_f - arrow_size_f / 2.0),
            Point::from_f32(arrow_x_f + arrow_size_f, arrow_y_f - arrow_size_f / 2.0),
            arrow_color,
        );
        context.draw_line(
            Point::from_f32(arrow_x_f + arrow_size_f, arrow_y_f - arrow_size_f / 2.0),
            Point::from_f32(arrow_x_f + arrow_size_f / 2.0, arrow_y_f + arrow_size_f / 2.0),
            arrow_color,
        );
        context.draw_line(
            Point::from_f32(arrow_x_f + arrow_size_f / 2.0, arrow_y_f + arrow_size_f / 2.0),
            Point::from_f32(arrow_x_f, arrow_y_f - arrow_size_f / 2.0),
            arrow_color,
        );
        // Draw current text
        let text_color = style.text_color.unwrap_or(Color::rgb(0, 0, 0));
        let default_font = crate::core::Font::default();
        let font = style.font.as_ref().unwrap_or(&default_font);
        let current_text = self.current_text();
        if !current_text.is_empty() {
            context.draw_text(
                Point::new(text_x, text_y as i32),
                &current_text,
                font,
                text_color,
                HorizontalAlignment::Left,
            );
        } else if self.items.is_empty() {
            // Draw placeholder
            context.draw_text(
                Point::new(text_x, text_y as i32),
                "(Empty)",
                font,
                text_color,
                HorizontalAlignment::Left,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;

    #[test]
    fn combobox_creation_defaults() {
        let cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        assert!(cb.items().is_empty());
        assert!(cb.is_empty());
        assert_eq!(cb.count(), 0);
        assert_eq!(cb.current_index(), None);
        assert!(!cb.is_editable());
        assert_eq!(cb.max_visible_items(), 10);
    }

    #[test]
    fn combobox_add_items() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.add_item("Item 1".to_string());
        cb.add_item("Item 2".to_string());
        assert_eq!(cb.items().len(), 2);
        assert_eq!(cb.items()[0], "Item 1");
        assert_eq!(cb.items()[1], "Item 2");
    }

    #[test]
    fn combobox_add_items_vec() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.add_items(vec!["A".to_string(), "B".to_string(), "C".to_string()]);
        assert_eq!(cb.count(), 3);
    }

    #[test]
    fn combobox_set_current_index() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.add_items(vec!["A".to_string(), "B".to_string(), "C".to_string()]);
        cb.set_current_index(Some(1));
        assert_eq!(cb.current_index(), Some(1));
        assert_eq!(cb.current_text(), "B".to_string());
    }

    #[test]
    fn combobox_set_current_index_none() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.add_items(vec!["A".to_string(), "B".to_string()]);
        cb.set_current_index(Some(1));
        cb.set_current_index(None);
        assert_eq!(cb.current_index(), None);
        assert!(cb.current_text().is_empty());
    }

    #[test]
    fn combobox_editable() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        assert!(!cb.is_editable());
        cb.set_editable(true);
        assert!(cb.is_editable());
        cb.set_editable(false);
        assert!(!cb.is_editable());
    }

    #[test]
    fn combobox_set_current_text_selects_existing_item() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.set_editable(true);
        cb.add_items(vec!["Apple".to_string(), "Banana".to_string()]);
        cb.set_current_text("Banana".to_string());
        assert_eq!(cb.current_index(), Some(1));
        assert_eq!(cb.current_text(), "Banana");
        assert_eq!(cb.count(), 2);
    }

    #[test]
    fn combobox_set_current_text_adds_unknown_custom_value() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.set_editable(true);
        cb.add_items(vec!["Apple".to_string()]);
        cb.set_current_text("Custom entry".to_string());
        assert_eq!(cb.current_text(), "Custom entry");
        assert_eq!(cb.count(), 2);
        assert_eq!(cb.current_index(), Some(1));
    }

    #[test]
    fn combobox_set_current_text_ignored_when_not_editable() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.add_items(vec!["Apple".to_string()]);
        cb.set_current_text("Apple".to_string());
        assert_eq!(cb.current_index(), None);
        assert_eq!(cb.count(), 1);
    }

    #[test]
    fn combobox_set_current_text_empty_selects_nothing() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.set_editable(true);
        cb.set_current_text(String::new());
        assert_eq!(cb.current_index(), None);
        assert_eq!(cb.count(), 0);
    }

    #[test]
    fn combobox_max_visible_items() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        assert_eq!(cb.max_visible_items(), 10);
        cb.set_max_visible_items(5);
        assert_eq!(cb.max_visible_items(), 5);
        cb.set_max_visible_items(0); // floors at 1
        assert_eq!(cb.max_visible_items(), 1);
    }

    #[test]
    fn combobox_insert_item() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.add_items(vec!["A".to_string(), "C".to_string()]);
        cb.insert_item(1, "B".to_string());
        assert_eq!(cb.count(), 3);
        assert_eq!(cb.item(1), Some("B"));
    }

    #[test]
    fn combobox_remove_item() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.add_items(vec!["A".to_string(), "B".to_string(), "C".to_string()]);
        cb.remove_item(1);
        assert_eq!(cb.count(), 2);
        assert_eq!(cb.item(1), Some("C"));
    }

    #[test]
    fn combobox_clear() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.add_items(vec!["A".to_string(), "B".to_string()]);
        cb.set_current_index(Some(0));
        cb.clear();
        assert!(cb.is_empty());
        assert_eq!(cb.current_index(), None);
    }

    #[test]
    fn combobox_find_text() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.add_items(vec!["Apple".to_string(), "Banana".to_string(), "Cherry".to_string()]);
        assert_eq!(cb.find_text("Banana"), Some(1));
        assert_eq!(cb.find_text("Missing"), None);
    }

    #[test]
    fn combobox_set_items() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.set_items(vec!["X".to_string(), "Y".to_string(), "Z".to_string()]);
        assert_eq!(cb.count(), 3);
        assert_eq!(cb.current_index(), None);
    }

    #[test]
    fn combobox_geometry_delegation() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.set_geometry(Rect::new(10, 10, 150, 30));
        assert_eq!(cb.geometry(), Rect::new(10, 10, 150, 30));
    }

    #[test]
    fn combobox_visibility() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        assert!(cb.is_visible());
        cb.hide();
        assert!(!cb.is_visible());
        cb.show();
        assert!(cb.is_visible());
    }

    #[test]
    fn combobox_enabled() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        assert!(cb.is_enabled());
        cb.set_enabled(false);
        assert!(!cb.is_enabled());
        cb.set_enabled(true);
        assert!(cb.is_enabled());
    }

    #[test]
    fn combobox_id_kind() {
        let cb_a = ComboBox::new(Rect::new(0, 0, 100, 24));
        let cb_b = ComboBox::new(Rect::new(0, 0, 100, 24));
        assert_ne!(cb_a.id(), cb_b.id());
        assert_eq!(cb_a.kind(), WidgetKind::ComboBox);
        assert_eq!(cb_b.kind(), WidgetKind::ComboBox);
    }

    #[test]
    fn combobox_signal_accessors() {
        let cb = ComboBox::new(Rect::new(0, 0, 100, 24));
        let _ = &cb.current_index_changed;
        let _ = &cb.current_text_changed;
        let _ = &cb.activated;
    }
}
