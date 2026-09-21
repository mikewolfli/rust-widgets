// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! AutoCompleteEdit widget — a text input with an auto-completion dropdown.
//!
//! The AutoCompleteEdit widget provides a text entry field that filters and
//! displays a dropdown list of suggestions as the user types. The user can
//! select a suggestion with the keyboard (Enter) or by clicking.

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

/// A text input field with auto-completion dropdown support.
///
/// As the user types, the widget filters its suggestions list and shows
/// matching entries in a dropdown below the text field. The user can select
/// a highlighted suggestion via Enter key or by clicking.
pub struct AutoCompleteEdit {
    base: BaseWidget,
    text: String,
    suggestions: Vec<String>,
    filtered_suggestions: Vec<String>,
    show_dropdown: bool,
    selected_suggestion: Option<usize>,
    max_visible: usize,
    /// Emitted when the text content changes.
    pub text_changed: Signal1<String>,
    /// Emitted when a suggestion is selected from the dropdown.
    pub suggestion_selected: Signal1<String>,
    undo_stack: UndoStack,
    history_target: Rc<RefCell<String>>,
    restoring_history: bool,
}

impl AutoCompleteEdit {
    /// Creates a new AutoCompleteEdit widget with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::AutoCompleteEdit, geometry, "AutoCompleteEdit"),
            text: String::new(),
            suggestions: Vec::new(),
            filtered_suggestions: Vec::new(),
            show_dropdown: false,
            selected_suggestion: None,
            max_visible: 5,
            text_changed: Signal1::new(),
            suggestion_selected: Signal1::new(),
            undo_stack: UndoStack::new(),
            history_target: Rc::new(RefCell::new(String::new())),
            restoring_history: false,
        }
    }

    /// Returns the current text content.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Sets the text content, updates the filtered suggestions list,
    /// and emits the `text_changed` signal.
    pub fn set_text(&mut self, text: String) {
        let cloned = text.clone();
        let before = self.text.clone();
        self.text = text;
        if !self.restoring_history {
            *self.history_target.borrow_mut() = self.text.clone();
            self.undo_stack.push(Box::new(TextSnapshotCommand::new(
                self.history_target.clone(),
                before,
                self.text.clone(),
                "auto_complete_text",
            )));
        }
        self.filter_suggestions();
        self.text_changed.emit(cloned);
        self.base.request_redraw();
    }

    /// Adds a single suggestion to the suggestion list.
    pub fn add_suggestion(&mut self, suggestion: String) {
        self.suggestions.push(suggestion);
        self.base.request_redraw();
    }

    /// Removes a suggestion by value. Returns true if the suggestion was found and removed.
    pub fn remove_suggestion(&mut self, suggestion: &str) -> bool {
        let idx = self.suggestions.iter().position(|s| s == suggestion);
        if let Some(pos) = idx {
            self.suggestions.remove(pos);
            self.filter_suggestions();
            self.base.request_redraw();
            true
        } else {
            false
        }
    }

    /// Replaces the entire suggestion list with the given vector.
    pub fn set_suggestions(&mut self, suggestions: Vec<String>) {
        self.suggestions = suggestions;
        self.filter_suggestions();
        self.base.request_redraw();
    }

    /// Returns a reference to the full suggestion list.
    pub fn suggestions(&self) -> &[String] {
        &self.suggestions
    }

    /// Removes all suggestions and hides the dropdown.
    pub fn clear_suggestions(&mut self) {
        self.suggestions.clear();
        self.filtered_suggestions.clear();
        self.show_dropdown = false;
        self.selected_suggestion = None;
        self.base.request_redraw();
    }

    /// Steps back one text edit and returns `true`, or `false` when there is
    /// nothing to undo.
    ///
    /// Replaying history emits `text_changed` but deliberately does not push a
    /// new undo entry. The undo stack is **not** cleared, so a redo of the
    /// undone change remains available.
    pub fn undo(&mut self) -> bool {
        if self.undo_stack.undo().is_err() {
            return false;
        }
        self.restore_history_text();
        true
    }

    /// Steps forward one undone edit and returns `true`, or `false` when there is
    /// nothing to redo. Signal behaviour matches
    /// [`AutoCompleteEdit::undo`].
    pub fn redo(&mut self) -> bool {
        if self.undo_stack.redo().is_err() {
            return false;
        }
        self.restore_history_text();
        true
    }

    /// Returns `true` if [`AutoCompleteEdit::undo`] would change the text.
    pub fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }
    /// Returns `true` if [`AutoCompleteEdit::redo`] would change the text.
    pub fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }

    fn restore_history_text(&mut self) {
        self.restoring_history = true;
        self.text = self.history_target.borrow().clone();
        self.restoring_history = false;
        self.filter_suggestions();
        self.text_changed.emit(self.text.clone());
        self.base.request_redraw();
    }

    /// Returns whether the dropdown is currently visible.
    pub fn is_showing_dropdown(&self) -> bool {
        self.show_dropdown
    }

    /// Returns the number of suggestions currently offered for the typed text.
    ///
    /// Counts the filtered list the dropdown draws, not the full suggestion set,
    /// so the number matches what the user can actually pick right now.
    pub fn suggestion_count(&self) -> usize {
        self.filtered_suggestions.len()
    }

    /// Shows the dropdown (if there are filtered suggestions).
    pub fn show_dropdown(&mut self) {
        if !self.filtered_suggestions.is_empty() {
            self.show_dropdown = true;
            self.selected_suggestion = Some(0);
            self.base.request_redraw();
        }
    }

    /// Hides the dropdown.
    pub fn hide_dropdown(&mut self) {
        self.show_dropdown = false;
        self.selected_suggestion = None;
        self.base.request_redraw();
    }

    /// Toggles the dropdown visibility.
    pub fn toggle_dropdown(&mut self) {
        if self.show_dropdown {
            self.hide_dropdown();
        } else {
            self.show_dropdown();
        }
    }

    /// Filters the suggestion list based on the current text.
    fn filter_suggestions(&mut self) {
        if self.text.is_empty() {
            self.filtered_suggestions.clear();
            self.show_dropdown = false;
            self.selected_suggestion = None;
            return;
        }
        let lower = self.text.to_lowercase();
        self.filtered_suggestions = self
            .suggestions
            .iter()
            .filter(|s| s.to_lowercase().contains(&lower))
            .cloned()
            .collect();
        if self.filtered_suggestions.is_empty() {
            self.show_dropdown = false;
            self.selected_suggestion = None;
        } else {
            self.show_dropdown = true;
            self.selected_suggestion = Some(0);
        }
    }

    /// Selects the highlighted suggestion and commits it as the current text.
    fn select_highlighted(&mut self) {
        if let Some(idx) = self.selected_suggestion {
            if let Some(suggestion) = self.filtered_suggestions.get(idx) {
                let selected = suggestion.clone();
                self.set_text(selected.clone());
                self.suggestion_selected.emit(selected);
                self.hide_dropdown();
                self.base.request_redraw();
            }
        }
    }

    /// Moves the selection highlight up (towards the first item).
    fn select_previous(&mut self) {
        if let Some(idx) = self.selected_suggestion {
            if idx > 0 {
                self.selected_suggestion = Some(idx - 1);
                self.base.request_redraw();
            }
        }
    }

    /// Moves the selection highlight down (towards the last item).
    fn select_next(&mut self) {
        if let Some(idx) = self.selected_suggestion {
            if idx + 1 < self.filtered_suggestions.len() {
                self.selected_suggestion = Some(idx + 1);
                self.base.request_redraw();
            }
        }
    }
}

impl Widget for AutoCompleteEdit {
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

/// `AutoCompleteEdit`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_input.in.rs` / `access_write_input.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `suggestion_count` is derived
/// from the filtered list, so it is refused as read-only rather than reported as a
/// name this control does not know.
impl WidgetProperties for AutoCompleteEdit {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "suggestion_count" => Ok(CapabilityValue::UInt(self.suggestion_count() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(expect_string(value)?);
                Ok(())
            }
            "suggestion_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["text", "suggestion_count", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `auto_complete_edit` publishes.
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

impl Draw for AutoCompleteEdit {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();

        // Chrome colours resolve explicit style first, then the theme's resolved style
        // for this control, and only then fall back to a literal. Without the theme step
        // a light/dark switch would change nothing on screen, because the field fill and
        // its outline were previously hardcoded.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global
        // manager's mutex is not re-entrant.
        let style = self.base.style().clone();
        let theme = crate::theme::resolved_theme_style("auto_complete_edit");
        let field_background = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::rgba(255, 255, 255, 255));
        let border_color = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or(Color::rgba(180, 180, 180, 255));
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        // The list is chrome of the same family as the field it drops from: its fill
        // and its ink are the resolved field colours, so both move with the appearance.
        let dropdown_background = if style.background_color.is_some() || theme.is_some() {
            field_background
        } else {
            Color::rgba(255, 255, 255, 255)
        };
        let dropdown_text = text_color;

        // Background
        let bg_color = if is_enabled { field_background } else { Color::rgba(240, 240, 240, 255) };
        context.fill_rounded_rect(rect, 4, bg_color);

        // Border
        context.draw_rounded_rect_stroke(rect, 4, border_color, 1);

        // Draw text
        let font = Font::simple("sans-serif", 13.0);
        let padding = 6i32;
        let text_x = rect.x + padding;
        let text_y = rect.y + padding + 13;

        let display_text = if self.text.is_empty() { "Type to search..." } else { &self.text };
        // Empty field = placeholder: the resolved ink damped toward the fill, so the
        // hint stays legible on either appearance.
        let input_text_color = if self.text.is_empty() {
            text_color.blend(&field_background, 0.4)
        } else if is_enabled {
            text_color
        } else {
            Color::rgba(160, 160, 160, 255)
        };
        context.draw_text(
            Point::new(text_x, text_y),
            display_text,
            &font,
            input_text_color,
            HorizontalAlignment::Left,
        );

        // Draw dropdown if visible
        if !self.show_dropdown || self.filtered_suggestions.is_empty() {
            return;
        }

        let drop_down_y = rect.y + rect.height as i32;
        let item_height = 24u32;
        let visible_count = self.filtered_suggestions.len().min(self.max_visible);
        let drop_down_height = item_height * visible_count as u32;
        let drop_rect = Rect::new(rect.x, drop_down_y, rect.width, drop_down_height);

        // Dropdown background
        context.fill_rounded_rect(drop_rect, 2, dropdown_background);
        context.draw_rounded_rect_stroke(
            drop_rect,
            2,
            border_color.blend(&dropdown_background, 0.3),
            1,
        );

        for i in 0..visible_count {
            let item_rect = Rect::new(
                rect.x + 1,
                drop_down_y + (i as i32) * (item_height as i32),
                rect.width.saturating_sub(2),
                item_height,
            );

            if Some(i) == self.selected_suggestion {
                context.fill_rounded_rect(
                    item_rect,
                    2,
                    dropdown_text.blend(&dropdown_background, 0.85),
                );
            }

            if let Some(suggestion) = self.filtered_suggestions.get(i) {
                let item_text_x = item_rect.x + 4;
                let item_text_y = item_rect.y + 16;
                context.draw_text(
                    Point::new(item_text_x, item_text_y),
                    suggestion,
                    &font,
                    dropdown_text,
                    HorizontalAlignment::Left,
                );
            }
        }
    }
}

impl EventHandler for AutoCompleteEdit {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } if *button == 1 => {
                let rect = self.geometry();
                // Check if click is inside the text field area
                if rect.contains_point(*pos) {
                    self.show_dropdown();
                }
                // Check if click is on a dropdown item
                if self.show_dropdown {
                    let item_height = 24i32;
                    let drop_down_y = rect.y + rect.height as i32;
                    let visible_count = self.filtered_suggestions.len().min(self.max_visible);
                    let drop_down_height = item_height * visible_count as i32;
                    let drop_rect =
                        Rect::new(rect.x, drop_down_y, rect.width, drop_down_height as u32);

                    if drop_rect.contains_point(*pos) {
                        let rel_y = pos.y - drop_down_y;
                        let idx = (rel_y / item_height) as usize;
                        if idx < self.filtered_suggestions.len() {
                            self.selected_suggestion = Some(idx);
                            self.select_highlighted();
                        }
                    }
                }
            }
            Event::KeyPress { key, modifiers } => {
                if *key == 13 {
                    // Enter
                    if self.show_dropdown {
                        self.select_highlighted();
                    }
                } else if *key == 27 {
                    // Escape
                    if self.show_dropdown {
                        self.hide_dropdown();
                    }
                } else if *key == 38 && *modifiers == 0 && self.show_dropdown {
                    // Up arrow
                    self.select_previous();
                } else if *key == 40 && *modifiers == 0 && self.show_dropdown {
                    // Down arrow
                    self.select_next();
                } else if *modifiers == 2 && *key == 90 {
                    let _ = self.undo();
                } else if *modifiers == 2 && *key == 89 {
                    let _ = self.redo();
                } else if *key >= 32 && *key <= 126 {
                    // Printable ASCII — append to text
                    let c = char::from_u32(*key).unwrap_or(' ');
                    let mut new_text = self.text.clone();
                    new_text.push(c);
                    self.set_text(new_text);
                } else if *key == 8 {
                    // Backspace
                    if !self.text.is_empty() {
                        let mut new_text = self.text.clone();
                        new_text.pop();
                        self.set_text(new_text);
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
    use std::sync::{Arc, Mutex};

    #[test]
    fn auto_complete_edit_default_creation() {
        let edit = AutoCompleteEdit::new(Rect::new(0, 0, 200, 30));
        assert_eq!(edit.kind(), WidgetKind::AutoCompleteEdit);
        assert!(edit.text().is_empty());
        assert!(edit.suggestions().is_empty());
        assert!(!edit.is_showing_dropdown());
        assert_eq!(edit.geometry(), Rect::new(0, 0, 200, 30));
    }

    #[test]
    fn auto_complete_edit_add_and_remove_suggestion() {
        let mut edit = AutoCompleteEdit::new(Rect::new(0, 0, 200, 30));
        edit.add_suggestion("Apple".to_string());
        edit.add_suggestion("Banana".to_string());
        edit.add_suggestion("Cherry".to_string());
        assert_eq!(edit.suggestions().len(), 3);

        assert!(edit.remove_suggestion("Banana"));
        assert_eq!(edit.suggestions().len(), 2);

        assert!(!edit.remove_suggestion("NonExistent"));
        assert_eq!(edit.suggestions().len(), 2);
    }

    #[test]
    fn auto_complete_edit_set_text_filters_suggestions() {
        let mut edit = AutoCompleteEdit::new(Rect::new(0, 0, 200, 30));
        edit.set_suggestions(vec![
            "Apple".to_string(),
            "Banana".to_string(),
            "Apricot".to_string(),
            "Cherry".to_string(),
        ]);
        assert_eq!(edit.suggestions().len(), 4);

        edit.set_text("Ap".to_string());
        assert_eq!(edit.text(), "Ap");
        assert!(edit.is_showing_dropdown());

        edit.set_text("XYZ".to_string());
        assert!(!edit.is_showing_dropdown());
    }

    #[test]
    fn auto_complete_edit_text_changed_signal() {
        let mut edit = AutoCompleteEdit::new(Rect::new(0, 0, 200, 30));
        let captured = Arc::new(Mutex::new(None::<String>));
        edit.text_changed.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<String>| {
                *captured.lock().unwrap() = Some(val.to_string());
            }
        });

        edit.set_text("Hello".to_string());
        assert_eq!(captured.lock().unwrap().as_deref(), Some("Hello"));
    }

    #[test]
    fn auto_complete_edit_suggestion_selected_signal() {
        let mut edit = AutoCompleteEdit::new(Rect::new(0, 0, 200, 30));
        edit.set_suggestions(vec!["Option 1".to_string(), "Option 2".to_string()]);
        edit.set_text("Opt".to_string());

        let captured = Arc::new(Mutex::new(None::<String>));
        edit.suggestion_selected.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<String>| {
                *captured.lock().unwrap() = Some(val.to_string());
            }
        });

        // Simulate Enter key to select highlighted suggestion
        edit.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
        assert_eq!(captured.lock().unwrap().as_deref(), Some("Option 1"));
    }

    #[test]
    fn auto_complete_edit_clear_suggestions() {
        let mut edit = AutoCompleteEdit::new(Rect::new(0, 0, 200, 30));
        edit.add_suggestion("Test".to_string());
        assert_eq!(edit.suggestions().len(), 1);

        edit.clear_suggestions();
        assert!(edit.suggestions().is_empty());
        assert!(!edit.is_showing_dropdown());
    }

    #[test]
    fn auto_complete_edit_toggle_dropdown() {
        let mut edit = AutoCompleteEdit::new(Rect::new(0, 0, 200, 30));
        edit.set_suggestions(vec!["Item".to_string()]);
        edit.set_text("It".to_string());

        assert!(edit.is_showing_dropdown());
        edit.hide_dropdown();
        assert!(!edit.is_showing_dropdown());
        edit.show_dropdown();
        assert!(edit.is_showing_dropdown());
        edit.toggle_dropdown();
        assert!(!edit.is_showing_dropdown());
    }

    #[test]
    fn auto_complete_edit_svg_output() {
        let mut edit = AutoCompleteEdit::new(Rect::new(0, 0, 200, 30));
        edit.add_suggestion("Suggestion".to_string());
        edit.set_text("Sug".to_string());
        let svg = render_to_svg(&mut edit);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }
}
