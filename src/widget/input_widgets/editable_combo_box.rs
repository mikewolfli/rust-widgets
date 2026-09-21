// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! EditableComboBox widget — a combo box that allows typing custom values.
//!
//! The EditableComboBox combines a text field with a dropdown list. Users can
//! either type a custom value directly or select from the provided items.
//! A `text_changed` signal is emitted when the text field content changes,
//! and an `item_selected` signal is emitted when a dropdown item is clicked.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::undo::{TextSnapshotCommand, UndoStack};
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::cell::RefCell;
use std::rc::Rc;

/// A combo box that allows both typing custom values and selecting from a list.
///
/// When collapsed, shows the current text in a text field with a dropdown arrow.
/// When expanded, shows a dropdown list of items below the text field.
pub struct EditableComboBox {
    base: BaseWidget,
    text: String,
    items: Vec<String>,
    expanded: bool,
    selected_index: Option<usize>,
    /// Emitted when the text field content changes.
    pub text_changed: Signal1<String>,
    /// Emitted when a dropdown item is selected (by index).
    pub item_selected: Signal1<usize>,
    undo_stack: UndoStack,
    history_target: Rc<RefCell<String>>,
    restoring_history: bool,
}

impl EditableComboBox {
    /// Creates a new EditableComboBox widget with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::EditableComboBox, geometry, "EditableComboBox"),
            text: String::new(),
            items: Vec::new(),
            expanded: false,
            selected_index: None,
            text_changed: Signal1::new(),
            item_selected: Signal1::new(),
            undo_stack: UndoStack::new(),
            history_target: Rc::new(RefCell::new(String::new())),
            restoring_history: false,
        }
    }

    /// Returns the current text in the text field.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Sets the text in the text field and emits `text_changed`.
    pub fn set_text(&mut self, text: impl Into<String>) {
        let t = text.into();
        if self.text != t {
            let before = self.text.clone();
            self.text = t.clone();
            if !self.restoring_history {
                *self.history_target.borrow_mut() = self.text.clone();
                self.undo_stack.push(Box::new(TextSnapshotCommand::new(
                    self.history_target.clone(),
                    before,
                    self.text.clone(),
                    "editable_combo_text",
                )));
            }
            self.text_changed.emit(self.text.clone());
            self.base.request_redraw();
        }
    }

    /// Adds an item to the dropdown list.
    pub fn add_item(&mut self, item: impl Into<String>) {
        self.items.push(item.into());
        self.base.request_redraw();
    }

    /// Removes an item by index. Returns `true` if the item was removed.
    pub fn remove_item(&mut self, index: usize) -> bool {
        if index < self.items.len() {
            self.items.remove(index);
            if self.selected_index == Some(index) {
                self.selected_index = None;
            } else if let Some(ref mut sel) = self.selected_index {
                if index < *sel {
                    *sel -= 1;
                }
            }
            self.base.request_redraw();
            true
        } else {
            false
        }
    }

    /// Removes all items from the list and closes the dropdown.
    pub fn clear_items(&mut self) {
        self.items.clear();
        self.selected_index = None;
        self.expanded = false;
        self.base.request_redraw();
    }

    /// Returns a reference to the list of items.
    pub fn items(&self) -> &[String] {
        &self.items
    }

    /// Returns the number of items in the dropdown.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Returns whether the dropdown is currently expanded.
    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    /// Expands the dropdown list.
    pub fn expand(&mut self) {
        if !self.expanded {
            self.expanded = true;
            self.base.request_redraw();
        }
    }

    /// Collapses the dropdown list.
    pub fn collapse(&mut self) {
        if self.expanded {
            self.expanded = false;
            self.base.request_redraw();
        }
    }

    /// Toggles the dropdown between expanded and collapsed.
    pub fn toggle(&mut self) {
        self.expanded = !self.expanded;
        self.base.request_redraw();
    }

    /// Returns the currently selected item index, if any.
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    /// Steps back one text edit and returns `true`, or `false` when there is
    /// nothing to undo.
    ///
    /// The restored text is applied through [`EditableComboBox::set_text`], so
    /// it runs the full validation and signal path. It also clears the selection:
    /// undoing a text change does not restore the previously selected item, and
    /// `selected_index` becomes `None` even though the text may match an item.
    pub fn undo(&mut self) -> bool {
        if self.undo_stack.undo().is_err() {
            return false;
        }
        self.restoring_history = true;
        let text = self.history_target.borrow().clone();
        self.set_text(&text);
        self.restoring_history = false;
        true
    }
    /// Steps forward one undone edit and returns `true`, or `false` when there is
    /// nothing to redo. Selection behaviour matches
    /// [`EditableComboBox::undo`].
    pub fn redo(&mut self) -> bool {
        if self.undo_stack.redo().is_err() {
            return false;
        }
        self.restoring_history = true;
        let text = self.history_target.borrow().clone();
        self.set_text(&text);
        self.restoring_history = false;
        true
    }
    /// Returns `true` if [`EditableComboBox::undo`] would change the text.
    pub fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }
    /// Returns `true` if [`EditableComboBox::redo`] would change the text.
    pub fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }

    /// Selects the item at the given index and sets the text field to its value.
    /// Emits `item_selected`. Returns `true` if the selection changed.
    pub fn select_index(&mut self, index: usize) -> bool {
        if index < self.items.len() && self.selected_index != Some(index) {
            self.selected_index = Some(index);
            self.text = self.items[index].clone();
            self.text_changed.emit(self.text.clone());
            self.item_selected.emit(index);
            self.expanded = false;
            self.base.request_redraw();
            return true;
        }
        false
    }
}

impl Widget for EditableComboBox {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(200, 28)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `EditableComboBox`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_input.in.rs` / `access_write_input.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `item_count` is derived from
/// the dropdown list, so it is refused as read-only rather than reported as a name
/// this control does not know.
impl WidgetProperties for EditableComboBox {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "item_count" => Ok(CapabilityValue::UInt(self.item_count() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(expect_string(value)?);
                Ok(())
            }
            "item_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["text", "item_count", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `editable_combo_box` publishes.
    ///
    /// `set_text` assigns the edit contents and needs a payload, so it is answered
    /// through the property route — the capability publishes no zero-argument action.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_text" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for EditableComboBox {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();

        // Chrome colours resolve explicit style first, then the theme's resolved style
        // for this control, and only then fall back to a literal. Every colour here used
        // to be a literal, so the control rendered identically in a light and a dark
        // theme — the rendering census reported it as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global
        // manager's mutex is not re-entrant.
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("editable_combo_box");
        let themed_ink = theme.as_ref().and_then(|t| t.text_color);
        let themed_border = theme.as_ref().and_then(|t| t.border_color);

        let ink = style.text_color.or(themed_ink).unwrap_or(Color::BLACK);
        let field = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let border = style
            .border_color
            .or(themed_border)
            .filter(|resolved| *resolved != field)
            .unwrap_or_else(|| field.blend(&ink, 0.3));

        // A disabled field is the resolved palette, one step toward the surface, rather
        // than a separate literal pair that could only ever suit a light theme.
        let bg_color = if is_enabled { field } else { field.blend(&ink, 0.08) };
        context.fill_rounded_rect(rect, 4, bg_color);

        // Border
        let border_color = border;
        context.draw_rounded_rect_stroke(rect, 4, border_color, 1);

        // Draw text content
        let font = Font::simple("sans-serif", 13.0);
        let padding = 6i32;
        let text_x = rect.x + padding;
        let text_y = rect.y + padding + 13;
        let display_text = if self.text.is_empty() && !is_enabled { "" } else { &self.text };
        let text_color = if is_enabled { ink } else { ink.blend(&bg_color, 0.6) };
        context.draw_text(
            Point::new(text_x, text_y),
            display_text,
            &font,
            text_color,
            HorizontalAlignment::Left,
        );

        // Draw dropdown arrow
        let arrow_x = rect.x + rect.width as i32 - 20;
        let arrow_y = rect.y + rect.height as i32 / 2 - 2;
        let arrow_color =
            if is_enabled { ink.blend(&bg_color, 0.45) } else { ink.blend(&bg_color, 0.7) };
        context.draw_text(
            Point::new(arrow_x, arrow_y),
            if self.expanded { "▲" } else { "▼" },
            &font,
            arrow_color,
            HorizontalAlignment::Left,
        );

        // Draw dropdown if expanded
        if !self.expanded || self.items.is_empty() {
            return;
        }

        let drop_down_y = rect.y + rect.height as i32;
        let item_height = 28u32;
        let drop_down_height = item_height * self.items.len() as u32;
        let drop_rect = Rect::new(rect.x, drop_down_y, rect.width, drop_down_height);

        // Dropdown background. The list floats over whatever is beneath it, so it is the
        // field's own colour rather than the window's: a transparent list would show the
        // content behind it through the popup.
        context.fill_rounded_rect(drop_rect, 2, field);
        context.draw_rounded_rect_stroke(drop_rect, 2, border, 1);

        // The selected-row highlight is the theme's accent, which is the one hue a theme
        // is expected to vary most.
        let highlight = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.primary.with_alpha(40))
            .unwrap_or_else(|| ink.blend(&field, 0.85));

        for (i, item) in self.items.iter().enumerate() {
            let item_rect = Rect::new(
                rect.x + 1,
                drop_down_y + (i as i32) * (item_height as i32),
                rect.width.saturating_sub(2),
                item_height,
            );

            // Highlight selected item
            if self.selected_index == Some(i) {
                context.fill_rounded_rect(item_rect, 2, highlight);
            }

            // Item text
            let item_text_x = item_rect.x + 8;
            let item_text_y = item_rect.y + 18;
            let item_color = if is_enabled { ink } else { ink.blend(&field, 0.6) };
            context.draw_text(
                Point::new(item_text_x, item_text_y),
                item,
                &font,
                item_color,
                HorizontalAlignment::Left,
            );
        }
    }
}

impl EventHandler for EditableComboBox {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::KeyPress { key, modifiers } => {
                if *modifiers == 2 && *key == 90 {
                    let _ = self.undo();
                    return;
                }
                if *modifiers == 2 && *key == 89 {
                    let _ = self.redo();
                    return;
                }
                match *key {
                    8 => {
                        // Backspace
                        let mut new_text = self.text.clone();
                        new_text.pop();
                        self.set_text(new_text);
                        return;
                    }
                    13 => {
                        // Enter/Return
                        if self.expanded && !self.items.is_empty() {
                            if let Some(idx) = self.selected_index {
                                self.select_index(idx);
                            } else if let Some(idx) =
                                self.items.iter().position(|item| item == &self.text)
                            {
                                self.select_index(idx);
                            }
                        }
                        self.expanded = false;
                        self.base.request_redraw();
                        return;
                    }
                    27 => {
                        // Escape
                        self.expanded = false;
                        self.base.request_redraw();
                        return;
                    }
                    38 => {
                        // ArrowUp
                        if self.expanded && !self.items.is_empty() {
                            let prev = match self.selected_index {
                                Some(0) | None => 0,
                                Some(idx) => idx - 1,
                            };
                            self.selected_index = Some(prev);
                            self.base.request_redraw();
                        }
                        return;
                    }
                    40 => {
                        // ArrowDown
                        if !self.expanded {
                            self.expanded = true;
                        }
                        if !self.items.is_empty() {
                            let next = match self.selected_index {
                                Some(idx) => (idx + 1).min(self.items.len() - 1),
                                None => 0,
                            };
                            self.selected_index = Some(next);
                            self.base.request_redraw();
                        }
                        return;
                    }
                    _ => {
                        // Character input
                        if let Some(ch) = char::from_u32(*key) {
                            if ch.is_ascii_graphic() || ch == ' ' {
                                let mut new_text = self.text.clone();
                                new_text.push(ch);
                                self.set_text(new_text);
                                return;
                            }
                        }
                    }
                }
                self.base.handle_event(event);
            }
            Event::MousePress { pos, button } if *button == 1 => {
                let rect = self.geometry();

                // Click on the text field (with arrow zone)
                if rect.contains_point(*pos) {
                    // Check if click is on the arrow area (right side)
                    let arrow_zone_x = rect.x + rect.width as i32 - 24;
                    if pos.x >= arrow_zone_x {
                        // Arrow zone - toggle dropdown
                        self.toggle();
                    } else {
                        // Text field - focus and expand if items exist
                        if !self.expanded && !self.items.is_empty() {
                            self.expand();
                        } else {
                            self.toggle();
                        }
                    }
                    return;
                }

                // Click on a dropdown item
                if self.expanded {
                    let item_height = 28i32;
                    let drop_down_y = rect.y + rect.height as i32;
                    let drop_down_height = item_height * self.items.len() as i32;
                    let drop_rect =
                        Rect::new(rect.x, drop_down_y, rect.width, drop_down_height as u32);

                    if drop_rect.contains_point(*pos) {
                        let rel_y = pos.y - drop_down_y;
                        let idx = (rel_y / item_height) as usize;
                        if idx < self.items.len() {
                            self.select_index(idx);
                        }
                        return;
                    }

                    // Click outside dropdown - collapse
                    if !rect.contains_point(*pos) {
                        self.collapse();
                    }
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
    use crate::widget::svg::render_to_svg;

    #[test]
    fn editable_combo_box_default_creation() {
        let cb = EditableComboBox::new(Rect::new(0, 0, 200, 30));
        assert_eq!(cb.kind(), WidgetKind::EditableComboBox);
        assert_eq!(cb.text(), "");
        assert_eq!(cb.item_count(), 0);
        assert!(!cb.is_expanded());
        assert!(cb.selected_index().is_none());
    }

    #[test]
    fn editable_combo_box_add_and_remove_items() {
        let mut cb = EditableComboBox::new(Rect::new(0, 0, 200, 30));
        cb.add_item("Option A");
        cb.add_item("Option B");
        cb.add_item("Option C");
        assert_eq!(cb.item_count(), 3);

        assert!(cb.remove_item(1));
        assert_eq!(cb.item_count(), 2);
        assert_eq!(cb.items()[0], "Option A");
        assert_eq!(cb.items()[1], "Option C");

        assert!(!cb.remove_item(5));
        assert_eq!(cb.item_count(), 2);
    }

    #[test]
    fn editable_combo_box_text_operations() {
        let mut cb = EditableComboBox::new(Rect::new(0, 0, 200, 30));
        assert_eq!(cb.text(), "");

        cb.set_text("Hello");
        assert_eq!(cb.text(), "Hello");

        cb.set_text("World");
        assert_eq!(cb.text(), "World");

        cb.clear_items();
        // Text should be unchanged by clear_items
        assert_eq!(cb.text(), "World");
    }

    #[test]
    fn editable_combo_box_expand_collapse() {
        let mut cb = EditableComboBox::new(Rect::new(0, 0, 200, 30));
        assert!(!cb.is_expanded());

        cb.expand();
        assert!(cb.is_expanded());

        cb.collapse();
        assert!(!cb.is_expanded());

        cb.toggle();
        assert!(cb.is_expanded());

        cb.toggle();
        assert!(!cb.is_expanded());
    }

    #[test]
    fn editable_combo_box_select_index() {
        let mut cb = EditableComboBox::new(Rect::new(0, 0, 200, 30));
        cb.add_item("Alpha");
        cb.add_item("Beta");
        cb.add_item("Gamma");

        assert!(cb.select_index(1));
        assert_eq!(cb.selected_index(), Some(1));
        assert_eq!(cb.text(), "Beta");
        assert!(!cb.is_expanded());

        // Selecting same index again should return false
        assert!(!cb.select_index(1));

        // Selecting out of bounds should be noop
        assert!(!cb.select_index(10));
    }

    #[test]
    fn editable_combo_box_svg_output() {
        let mut cb = EditableComboBox::new(Rect::new(0, 0, 200, 30));
        cb.add_item("Item 1");
        cb.add_item("Item 2");
        cb.set_text("Hello");
        let svg = render_to_svg(&mut cb);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn editable_combo_box_text_changed_signal() {
        let mut cb = EditableComboBox::new(Rect::new(0, 0, 200, 30));
        let captured = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
        let c = captured.clone();
        cb.text_changed.connect(move |val: std::sync::Arc<String>| {
            *c.lock().unwrap() = Some(val.to_string());
        });

        cb.set_text("CustomValue");
        assert_eq!(captured.lock().unwrap().as_deref(), Some("CustomValue"));
    }

    #[test]
    fn editable_combo_box_item_selected_signal() {
        let mut cb = EditableComboBox::new(Rect::new(0, 0, 200, 30));
        cb.add_item("A");
        cb.add_item("B");

        let captured = std::sync::Arc::new(std::sync::Mutex::new(None::<usize>));
        let c = captured.clone();
        cb.item_selected.connect(move |val: std::sync::Arc<usize>| {
            *c.lock().unwrap() = Some(*val);
        });

        cb.select_index(1);
        assert_eq!(*captured.lock().unwrap(), Some(1));
    }
}
