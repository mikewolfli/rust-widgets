//! AutoCompleteEdit widget — a text input with an auto-completion dropdown.
//!
//! The AutoCompleteEdit widget provides a text entry field that filters and
//! displays a dropdown list of suggestions as the user types. The user can
//! select a suggestion with the keyboard (Enter) or by clicking.

use crate::core::{Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

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
        self.text = text;
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

    /// Returns whether the dropdown is currently visible.
    pub fn is_showing_dropdown(&self) -> bool {
        self.show_dropdown
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
                self.text = selected.clone();
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
}

impl Draw for AutoCompleteEdit {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();

        // Background
        let bg_color = if is_enabled {
            Color::rgba(255, 255, 255, 255)
        } else {
            Color::rgba(240, 240, 240, 255)
        };
        context.fill_rounded_rect(rect, 4, bg_color);

        // Border
        let border_color = Color::rgba(180, 180, 180, 255);
        context.draw_rounded_rect_stroke(rect, 4, border_color, 1);

        // Draw text
        let font = Font::simple("sans-serif", 13.0);
        let padding = 6i32;
        let text_x = rect.x + padding;
        let text_y = rect.y + padding + 13;

        let display_text = if self.text.is_empty() { "Type to search..." } else { &self.text };
        let text_color = if self.text.is_empty() {
            Color::rgba(160, 160, 160, 255)
        } else {
            Color::rgba(0, 0, 0, 255)
        };
        context.draw_text(Point::new(text_x, text_y), display_text, &font, text_color);

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
        context.fill_rounded_rect(drop_rect, 2, Color::rgba(255, 255, 255, 255));
        context.draw_rounded_rect_stroke(drop_rect, 2, Color::rgba(200, 200, 200, 255), 1);

        for i in 0..visible_count {
            let item_rect = Rect::new(
                rect.x + 1,
                drop_down_y + (i as i32) * (item_height as i32),
                rect.width - 2,
                item_height,
            );

            if Some(i) == self.selected_suggestion {
                context.fill_rounded_rect(item_rect, 2, Color::rgba(52, 120, 246, 40));
            }

            if let Some(suggestion) = self.filtered_suggestions.get(i) {
                let item_text_x = item_rect.x + 4;
                let item_text_y = item_rect.y + 16;
                context.draw_text(
                    Point::new(item_text_x, item_text_y),
                    suggestion,
                    &font,
                    Color::rgba(0, 0, 0, 255),
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
            Event::KeyPress { key, modifiers: _ } => {
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
                } else if *key == 38 && self.show_dropdown {
                    // Up arrow
                    self.select_previous();
                } else if *key == 40 && self.show_dropdown {
                    // Down arrow
                    self.select_next();
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
