//! MultiSelectComboBox widget — a combo box that allows multiple selections.
//!
//! The MultiSelectComboBox widget displays a text summary of selected items
//! and a dropdown with checkboxes when expanded. Users can toggle individual
//! items on/off. The widget emits a `selection_changed` signal with the IDs
//! of all selected items whenever the selection changes.

use crate::core::{HorizontalAlignment, Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use std::collections::HashSet;

/// An item in a MultiSelectComboBox with an identifier, display text, and enabled state.
#[derive(Debug, Clone)]
pub struct MultiSelectItem {
    /// Unique identifier for this item.
    pub id: u64,
    /// Display text shown in the dropdown.
    pub text: String,
    /// Whether this item can be selected/deselected.
    pub enabled: bool,
}

impl MultiSelectItem {
    /// Creates a new MultiSelectItem.
    pub fn new(id: u64, text: String) -> Self {
        Self { id, text, enabled: true }
    }

    /// Creates a new MultiSelectItem with explicit enabled state.
    pub fn with_enabled(id: u64, text: String, enabled: bool) -> Self {
        Self { id, text, enabled }
    }
}

/// A combo box that allows multiple selections via checkboxes.
///
/// When collapsed, shows either "N selected" or a comma-separated list of
/// selected item texts. When expanded, shows a dropdown with a checkbox
/// next to each item.
pub struct MultiSelectComboBox {
    base: BaseWidget,
    items: Vec<MultiSelectItem>,
    selected: HashSet<usize>,
    expanded: bool,
    /// Emitted when the selection changes, with the list of selected item IDs.
    pub selection_changed: Signal1<Vec<u64>>,
}

impl MultiSelectComboBox {
    /// Creates a new MultiSelectComboBox widget with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::MultiSelectComboBox, geometry, "MultiSelectComboBox"),
            items: Vec::new(),
            selected: HashSet::new(),
            expanded: false,
            selection_changed: Signal1::new(),
        }
    }

    /// Adds an item to the combo box.
    pub fn add_item(&mut self, item: MultiSelectItem) {
        self.items.push(item);
        self.base.request_redraw();
    }

    /// Removes an item by index. Returns true if the item was removed.
    pub fn remove_item(&mut self, index: usize) -> bool {
        if index < self.items.len() {
            self.items.remove(index);
            // Adjust selected indices
            let mut new_selected = HashSet::new();
            for &old_idx in &self.selected {
                if old_idx < index {
                    new_selected.insert(old_idx);
                } else if old_idx > index {
                    new_selected.insert(old_idx - 1);
                }
                // old_idx == index is dropped
            }
            self.selected = new_selected;
            self.base.request_redraw();
            true
        } else {
            false
        }
    }

    /// Removes all items and clears the selection.
    pub fn clear_items(&mut self) {
        self.items.clear();
        self.selected.clear();
        self.expanded = false;
        self.base.request_redraw();
    }

    /// Returns the number of items.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Returns a reference to the list of items.
    pub fn items(&self) -> &[MultiSelectItem] {
        &self.items
    }

    /// Selects the item at the given index (if enabled). Returns true if selection changed.
    pub fn select(&mut self, index: usize) -> bool {
        if index < self.items.len() && self.items[index].enabled
            && self.selected.insert(index) {
                self.emit_selection_changed();
                self.base.request_redraw();
                return true;
            }
        false
    }

    /// Deselects the item at the given index. Returns true if selection changed.
    pub fn deselect(&mut self, index: usize) -> bool {
        if self.selected.remove(&index) {
            self.emit_selection_changed();
            self.base.request_redraw();
            return true;
        }
        false
    }

    /// Toggles the selection state of the item at the given index.
    pub fn toggle(&mut self, index: usize) {
        if index < self.items.len() {
            if self.selected.contains(&index) {
                self.deselect(index);
            } else {
                self.select(index);
            }
        }
    }

    /// Returns whether the item at the given index is selected.
    pub fn is_selected(&self, index: usize) -> bool {
        self.selected.contains(&index)
    }

    /// Returns a sorted vector of all selected indices.
    pub fn selected_indices(&self) -> Vec<usize> {
        let mut indices: Vec<usize> = self.selected.iter().copied().collect();
        indices.sort();
        indices
    }

    /// Returns the display texts of all selected items.
    pub fn selected_texts(&self) -> Vec<String> {
        let mut texts: Vec<String> = self
            .selected
            .iter()
            .filter_map(|&idx| self.items.get(idx).map(|item| item.text.clone()))
            .collect();
        texts.sort();
        texts
    }

    /// Clears the entire selection.
    pub fn clear_selection(&mut self) {
        if !self.selected.is_empty() {
            self.selected.clear();
            self.emit_selection_changed();
            self.base.request_redraw();
        }
    }

    /// Emits the selection_changed signal with the current selected item IDs.
    fn emit_selection_changed(&self) {
        let ids: Vec<u64> = self
            .selected
            .iter()
            .filter_map(|&idx| self.items.get(idx).map(|item| item.id))
            .collect();
        self.selection_changed.emit(ids);
    }

    /// Returns the summary text shown when the dropdown is collapsed.
    fn summary_text(&self) -> String {
        let selected_count = self.selected.len();
        if selected_count == 0 {
            "Nothing selected".to_string()
        } else if selected_count <= 2 {
            self.selected_texts().join(", ")
        } else {
            format!("{} selected", selected_count)
        }
    }

    /// Toggles the expanded/collapsed state.
    fn toggle_expand(&mut self) {
        self.expanded = !self.expanded;
        self.base.request_redraw();
    }
}

impl Widget for MultiSelectComboBox {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(200, 28)
    }
}

impl Draw for MultiSelectComboBox {
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

        // Draw summary text
        let font = Font::simple("sans-serif", 13.0);
        let padding = 6i32;
        let text_x = rect.x + padding;
        let text_y = rect.y + padding + 13;
        let summary = self.summary_text();
        context.draw_text(Point::new(text_x, text_y), &summary, &font, Color::rgba(0, 0, 0, 255), HorizontalAlignment::Left);

        // Draw dropdown arrow
        let arrow_x = rect.x + rect.width as i32 - 18;
        let arrow_y = rect.y + rect.height as i32 / 2 - 2;
        context.draw_text(
            Point::new(arrow_x, arrow_y),
            if self.expanded { "▲" } else { "▼" },
            &font,
            Color::rgba(100, 100, 100, 255),
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

        // Dropdown background
        context.fill_rounded_rect(drop_rect, 2, Color::rgba(255, 255, 255, 255));
        context.draw_rounded_rect_stroke(drop_rect, 2, Color::rgba(200, 200, 200, 255), 1);

        for (i, item) in self.items.iter().enumerate() {
            let item_rect = Rect::new(
                rect.x + 1,
                drop_down_y + (i as i32) * (item_height as i32),
                rect.width - 2,
                item_height,
            );

            // Hover highlight
            context.fill_rounded_rect(item_rect, 2, Color::rgba(255, 255, 255, 255));

            // Checkbox
            let checkbox_size = 14u32;
            let checkbox_x = item_rect.x + 4;
            let checkbox_y = item_rect.y + (item_height - checkbox_size) as i32 / 2;
            let checkbox_rect = Rect::new(checkbox_x, checkbox_y, checkbox_size, checkbox_size);
            let checkbox_color = if self.selected.contains(&i) {
                Color::rgba(52, 120, 246, 255)
            } else {
                Color::rgba(200, 200, 200, 255)
            };
            context.draw_rounded_rect_stroke(checkbox_rect, 2, checkbox_color, 1);

            if self.selected.contains(&i) {
                // Draw checkmark
                let check_font = Font::simple("sans-serif", 11.0);
                context.draw_text(
                    Point::new(checkbox_x + 2, checkbox_y + 12),
                    "✓",
                    &check_font,
                    checkbox_color,
                    HorizontalAlignment::Left,
                );
            }

            // Item text
            let item_text_x = checkbox_x + checkbox_size as i32 + 6;
            let item_text_y = item_rect.y + 18;
            let item_color = if item.enabled {
                Color::rgba(0, 0, 0, 255)
            } else {
                Color::rgba(180, 180, 180, 255)
            };
            context.draw_text(Point::new(item_text_x, item_text_y), &item.text, &font, item_color, HorizontalAlignment::Left);
        }
    }
}

impl EventHandler for MultiSelectComboBox {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } if *button == 1 => {
                let rect = self.geometry();

                // Click on the main box toggles expand
                if rect.contains_point(*pos) {
                    self.toggle_expand();
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
                        if idx < self.items.len() && self.items[idx].enabled {
                            self.toggle(idx);
                        }
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
    fn multi_select_combo_box_default_creation() {
        let cb = MultiSelectComboBox::new(Rect::new(0, 0, 200, 30));
        assert_eq!(cb.kind(), WidgetKind::MultiSelectComboBox);
        assert_eq!(cb.item_count(), 0);
        assert!(cb.selected_indices().is_empty());
        assert!(!cb.expanded);
    }

    #[test]
    fn multi_select_combo_box_add_and_remove_items() {
        let mut cb = MultiSelectComboBox::new(Rect::new(0, 0, 200, 30));
        cb.add_item(MultiSelectItem::new(1, "Option A".to_string()));
        cb.add_item(MultiSelectItem::new(2, "Option B".to_string()));
        cb.add_item(MultiSelectItem::new(3, "Option C".to_string()));
        assert_eq!(cb.item_count(), 3);

        assert!(cb.remove_item(1));
        assert_eq!(cb.item_count(), 2);
        assert_eq!(cb.items()[0].text, "Option A");
        assert_eq!(cb.items()[1].text, "Option C");

        assert!(!cb.remove_item(5));
        assert_eq!(cb.item_count(), 2);
    }

    #[test]
    fn multi_select_combo_box_select_and_deselect() {
        let mut cb = MultiSelectComboBox::new(Rect::new(0, 0, 200, 30));
        cb.add_item(MultiSelectItem::new(10, "X".to_string()));
        cb.add_item(MultiSelectItem::new(20, "Y".to_string()));
        cb.add_item(MultiSelectItem::new(30, "Z".to_string()));

        assert!(cb.select(0));
        assert!(cb.is_selected(0));
        assert!(!cb.is_selected(1));

        assert!(cb.select(2));
        assert_eq!(cb.selected_indices(), vec![0, 2]);

        assert!(cb.deselect(0));
        assert!(!cb.is_selected(0));

        // Toggle
        cb.toggle(1);
        assert!(cb.is_selected(1));
        cb.toggle(1);
        assert!(!cb.is_selected(1));
    }

    #[test]
    fn multi_select_combo_box_selection_changed_signal() {
        let mut cb = MultiSelectComboBox::new(Rect::new(0, 0, 200, 30));
        cb.add_item(MultiSelectItem::new(1, "A".to_string()));
        cb.add_item(MultiSelectItem::new(2, "B".to_string()));
        cb.add_item(MultiSelectItem::new(3, "C".to_string()));

        let captured = Arc::new(Mutex::new(None::<Vec<u64>>));
        cb.selection_changed.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<Vec<u64>>| {
                *captured.lock().unwrap() = Some(val.to_vec());
            }
        });

        cb.select(0);
        assert_eq!(captured.lock().unwrap().as_deref(), Some(&[1u64][..]));

        cb.select(2);
        let ids = captured.lock().unwrap().clone();
        let ids = ids.unwrap();
        assert!(ids.contains(&1));
        assert!(ids.contains(&3));
    }

    #[test]
    fn multi_select_combo_box_clear_selection() {
        let mut cb = MultiSelectComboBox::new(Rect::new(0, 0, 200, 30));
        cb.add_item(MultiSelectItem::new(1, "A".to_string()));
        cb.add_item(MultiSelectItem::new(2, "B".to_string()));
        cb.select(0);
        cb.select(1);
        assert_eq!(cb.selected_indices().len(), 2);

        cb.clear_selection();
        assert!(cb.selected_indices().is_empty());
    }

    #[test]
    fn multi_select_combo_box_clear_items() {
        let mut cb = MultiSelectComboBox::new(Rect::new(0, 0, 200, 30));
        cb.add_item(MultiSelectItem::new(1, "A".to_string()));
        cb.add_item(MultiSelectItem::new(2, "B".to_string()));
        cb.select(0);
        assert_eq!(cb.item_count(), 2);

        cb.clear_items();
        assert_eq!(cb.item_count(), 0);
        assert!(cb.selected_indices().is_empty());
        assert!(!cb.expanded);
    }

    #[test]
    fn multi_select_combo_box_selected_texts() {
        let mut cb = MultiSelectComboBox::new(Rect::new(0, 0, 200, 30));
        cb.add_item(MultiSelectItem::new(1, "Alpha".to_string()));
        cb.add_item(MultiSelectItem::new(2, "Beta".to_string()));
        cb.add_item(MultiSelectItem::new(3, "Gamma".to_string()));

        cb.select(0);
        cb.select(2);
        let texts = cb.selected_texts();
        assert_eq!(texts, vec!["Alpha", "Gamma"]);
    }

    #[test]
    fn multi_select_combo_box_svg_output() {
        let mut cb = MultiSelectComboBox::new(Rect::new(0, 0, 200, 30));
        cb.add_item(MultiSelectItem::new(1, "Item 1".to_string()));
        cb.add_item(MultiSelectItem::new(2, "Item 2".to_string()));
        cb.select(0);
        let svg = render_to_svg(&mut cb);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }
}
