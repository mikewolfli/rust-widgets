//! List box widget.
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};

use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// List box widget.
pub struct ListBox {
    base: BaseWidget,
    items: Vec<String>,
    selected_indices: Vec<usize>,
    selection_mode: SelectionMode,
    current_row: Option<usize>,
    item_height: f32,
    scroll_offset: usize,
    pub item_selected: Signal1<usize>,
    pub item_activated: Signal1<usize>,
    pub selection_changed: GenericSignal,
}
/// Selection mode for list box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectionMode {
    /// No selection allowed
    NoSelection,
    /// Single item selection
    #[default]
    SingleSelection,
    /// Multiple item selection
    MultiSelection,
    /// Extended selection with shift/ctrl
    ExtendedSelection,
}
impl ListBox {
    /// Creates an empty list box.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ListBox, geometry, "ListBox"),
            items: Vec::new(),
            selected_indices: Vec::new(),
            selection_mode: SelectionMode::SingleSelection,
            current_row: None,
            item_height: 20.0,
            scroll_offset: 0,
            item_selected: Signal1::new(),
            item_activated: Signal1::new(),
            selection_changed: GenericSignal::new(),
        }
    }
    /// Returns number of items.
    pub fn count(&self) -> usize {
        self.items.len()
    }
    /// Returns whether the list box is empty.
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
    /// Inserts an item at specified position.
    pub fn insert_item(&mut self, index: usize, text: String) {
        if index <= self.items.len() {
            self.items.insert(index, text);
            // Adjust selected indices
            for selected in &mut self.selected_indices {
                if index <= *selected {
                    *selected += 1;
                }
            }
            // Adjust current row
            if let Some(current) = &mut self.current_row {
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
            // Remove from selected indices
            self.selected_indices.retain(|&i| i != index);
            // Adjust remaining indices
            for selected in &mut self.selected_indices {
                if index < *selected {
                    *selected -= 1;
                }
            }
            // Adjust current row
            if let Some(current) = &mut self.current_row {
                if index == *current {
                    self.current_row = None;
                } else if index < *current {
                    *current -= 1;
                }
            }
            self.selection_changed.emit();
        }
    }
    /// Clears all items.
    pub fn clear(&mut self) {
        self.items.clear();
        self.selected_indices.clear();
        self.current_row = None;
        self.selection_changed.emit();
    }
    /// Returns selection mode.
    pub fn selection_mode(&self) -> SelectionMode {
        self.selection_mode
    }
    /// Sets selection mode.
    pub fn set_selection_mode(&mut self, mode: SelectionMode) {
        self.selection_mode = mode;
        // Clear selection if mode doesn't allow current selection
        match mode {
            SelectionMode::NoSelection => {
                self.selected_indices.clear();
                self.current_row = None;
                self.selection_changed.emit();
            }
            SelectionMode::SingleSelection if self.selected_indices.len() > 1 => {
                self.selected_indices.truncate(1);
                self.selection_changed.emit();
            }
            // No action needed for this transition
            _ => {}
        }
    }
    /// Returns selected indices.
    pub fn selected_indices(&self) -> &[usize] {
        &self.selected_indices
    }
    /// Returns whether an item is selected.
    pub fn is_selected(&self, index: usize) -> bool {
        self.selected_indices.contains(&index)
    }
    /// Selects an item.
    pub fn select(&mut self, index: usize) {
        if index >= self.items.len() {
            return;
        }
        match self.selection_mode {
            SelectionMode::NoSelection => (),
            SelectionMode::SingleSelection => {
                self.selected_indices.clear();
                self.selected_indices.push(index);
                self.current_row = Some(index);
                self.item_selected.emit(index);
                self.selection_changed.emit();
            }
            SelectionMode::MultiSelection => {
                if !self.selected_indices.contains(&index) {
                    self.selected_indices.push(index);
                    self.current_row = Some(index);
                    self.item_selected.emit(index);
                    self.selection_changed.emit();
                }
            }
            SelectionMode::ExtendedSelection => {
                // Similar to multi for now
                if !self.selected_indices.contains(&index) {
                    self.selected_indices.push(index);
                    self.current_row = Some(index);
                    self.item_selected.emit(index);
                    self.selection_changed.emit();
                }
            }
        }
    }
    /// Deselects an item.
    pub fn deselect(&mut self, index: usize) {
        if let Some(pos) = self.selected_indices.iter().position(|&i| i == index) {
            self.selected_indices.remove(pos);
            if self.current_row == Some(index) {
                self.current_row = None;
            }
            self.selection_changed.emit();
        }
    }
    /// Clears selection.
    pub fn clear_selection(&mut self) {
        if !self.selected_indices.is_empty() {
            self.selected_indices.clear();
            self.current_row = None;
            self.selection_changed.emit();
        }
    }
    /// Selects all items.
    pub fn select_all(&mut self) {
        if self.selection_mode == SelectionMode::NoSelection {
            return;
        }
        self.selected_indices.clear();
        for i in 0..self.items.len() {
            self.selected_indices.push(i);
        }
        if !self.items.is_empty() {
            self.current_row = Some(0);
        }
        self.selection_changed.emit();
    }
    /// Returns current row.
    pub fn current_row(&self) -> Option<usize> {
        self.current_row
    }
    /// Selects item at a pixel position (maps screen coords to item index).
    fn select_at_pos(&mut self, pos: Point) {
        let rect = self.geometry();
        if rect.contains(pos) {
            let rel_index = ((pos.y - rect.y) as f32 / self.item_height) as usize;
            let item_index = self.scroll_offset + rel_index;
            if item_index < self.items.len() {
                self.select(item_index);
                self.base.clicked.emit();
            }
        }
    }
    /// Activates item at a pixel position (select + activate signal).
    fn activate_at_pos(&mut self, pos: Point) {
        let rect = self.geometry();
        if rect.contains(pos) {
            let rel_index = ((pos.y - rect.y) as f32 / self.item_height) as usize;
            let item_index = self.scroll_offset + rel_index;
            if item_index < self.items.len() {
                self.select(item_index);
                self.item_activated.emit(item_index);
            }
        }
    }
    /// Sets current row.
    pub fn set_current_row(&mut self, row: Option<usize>) {
        if let Some(r) = row {
            if r < self.items.len() {
                self.current_row = Some(r);
            }
        } else {
            self.current_row = None;
        }
    }
    /// Returns item height.
    pub fn item_height(&self) -> f32 {
        self.item_height
    }
    /// Sets item height.
    pub fn set_item_height(&mut self, height: f32) {
        self.item_height = height.max(1.0);
    }
    /// Returns all items.
    pub fn items(&self) -> &[String] {
        &self.items
    }
    /// Returns visible item range based on scroll position.
    fn visible_range(&self) -> (usize, usize) {
        let rect = self.geometry();
        let visible_items = (rect.height as f32 / self.item_height).ceil() as usize;
        let start = self.scroll_offset.min(self.items.len().saturating_sub(1));
        let end = self.items.len().min(start + visible_items);
        (start, end)
    }
    /// Scrolls the list by the given delta (positive = down, negative = up).
    pub fn scroll(&mut self, delta: i32) {
        if self.items.is_empty() {
            return;
        }
        let max_offset = self.items.len().saturating_sub(1);
        if delta > 0 {
            self.scroll_offset = self.scroll_offset.saturating_add(delta as usize).min(max_offset);
        } else {
            self.scroll_offset = self.scroll_offset.saturating_sub((-delta) as usize);
        }
        self.base.request_redraw();
    }
}
// Implement Widget trait
impl Widget for ListBox {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(120, 100)
    }
}
impl EventHandler for ListBox {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } if *button == 1 => {
                self.select_at_pos(*pos);
            }
            Event::MouseDoubleClick { pos, button } if *button == 1 => {
                self.activate_at_pos(*pos);
            }
            #[cfg(feature = "touch")]
            Event::TouchBegin { pos, .. } => {
                self.select_at_pos(*pos);
            }
            #[cfg(feature = "touch")]
            Event::Tap { pos } => {
                self.activate_at_pos(*pos);
            }
            Event::KeyPress { key, modifiers } => {
                match *key {
                    38 if *modifiers == 0 => {
                        // Up arrow
                        if let Some(current) = self.current_row {
                            if current > 0 {
                                self.select(current - 1);
                            }
                        } else if !self.items.is_empty() {
                            self.select(self.items.len() - 1);
                        }
                    }
                    40 if *modifiers == 0 => {
                        // Down arrow
                        if let Some(current) = self.current_row {
                            if current < self.items.len() - 1 {
                                self.select(current + 1);
                            }
                        } else if !self.items.is_empty() {
                            self.select(0);
                        }
                    }
                    36
                        // Home
                        if !self.items.is_empty() => {
                            self.select(0);
                        }
                    35
                        // End
                        if !self.items.is_empty() => {
                            self.select(self.items.len() - 1);
                        }
                    13 => {
                        // Enter - activate current item
                        if let Some(current) = self.current_row {
                            self.item_activated.emit(current);
                        }
                    }
                    // Unknown key; ignore
                    _ => {}
                }
            }
            Event::Wheel { delta, .. } => {
                // delta.y > 0 = scroll down, delta.y < 0 = scroll up
                self.scroll(-delta.y);
            }
            // Other events are not relevant for this widget
            _ => {}
        }
    }
}

impl Draw for ListBox {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw base widget
        let rect = self.geometry();
        let padding = 2;
        let style = self.style();
        let bg = style.background_color.unwrap_or(Color::rgb(255, 255, 255));
        let text_color = style.text_color.unwrap_or(Color::rgb(0, 0, 0));
        // Draw background
        context.fill_rect(Rect::new(rect.x, rect.y, rect.width, rect.height), bg);
        // Draw border
        if let Some(border_color) = style.border_color {
            context.draw_rect(Rect::new(rect.x, rect.y, rect.width, rect.height), border_color);
        }
        // Draw items
        let (start, end) = self.visible_range();
        for i in start..end {
            let item_y_f = rect.y as f32 + (i as f32 * self.item_height);
            let item_rect =
                Rect::from_f32(rect.x as f32, item_y_f, rect.width as f32, self.item_height);
            // Draw item background
            if self.is_selected(i) {
                context.fill_rect(
                    Rect::new(item_rect.x, item_rect.y, item_rect.width, item_rect.height),
                    Color::rgb(0, 120, 215),
                );
            } else if Some(i) == self.current_row {
                context.fill_rect(
                    Rect::new(item_rect.x, item_rect.y, item_rect.width, item_rect.height),
                    Color::rgb(240, 240, 240),
                );
            }
            // Draw item text
            if let Some(text) = self.item(i) {
                let text_color =
                    if self.is_selected(i) { Color::rgb(255, 255, 255) } else { text_color };
                context.draw_text(
                    Point::new(
                        item_rect.x + padding,
                        (item_rect.y as f32 + self.item_height / 2.0) as i32,
                    ),
                    text,
                    &Font::default(),
                    text_color,
                    HorizontalAlignment::Left,
                );
            }
            // Draw item separator
            if i < end - 1 {
                let sep_y = item_rect.y + item_rect.height as i32;
                context.draw_line(
                    Point::new(item_rect.x, sep_y),
                    Point::new(item_rect.x + item_rect.width as i32, sep_y),
                    Color::rgb(230, 230, 230),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;

    #[test]
    fn listbox_creation_defaults() {
        let lb = ListBox::new(Rect::new(0, 0, 200, 200));
        assert!(lb.items().is_empty());
        assert!(lb.is_empty());
        assert_eq!(lb.count(), 0);
        assert_eq!(lb.current_row(), None);
        assert!(lb.selected_indices().is_empty());
        assert_eq!(lb.selection_mode(), SelectionMode::SingleSelection);
        assert!((lb.item_height() - 20.0).abs() < f32::EPSILON);
    }

    #[test]
    fn listbox_add_items() {
        let mut lb = ListBox::new(Rect::new(0, 0, 200, 200));
        lb.add_item("Item A".to_string());
        lb.add_item("Item B".to_string());
        assert_eq!(lb.count(), 2);
        assert_eq!(lb.item(0), Some("Item A"));
        assert_eq!(lb.item(1), Some("Item B"));
    }

    #[test]
    fn listbox_add_items_vec() {
        let mut lb = ListBox::new(Rect::new(0, 0, 200, 200));
        lb.add_items(vec!["X".to_string(), "Y".to_string(), "Z".to_string()]);
        assert_eq!(lb.count(), 3);
        assert!(!lb.is_empty());
    }

    #[test]
    fn listbox_insert_item() {
        let mut lb = ListBox::new(Rect::new(0, 0, 200, 200));
        lb.add_items(vec!["A".to_string(), "C".to_string()]);
        lb.insert_item(1, "B".to_string());
        assert_eq!(lb.count(), 3);
        assert_eq!(lb.item(1), Some("B"));
    }

    #[test]
    fn listbox_remove_item() {
        let mut lb = ListBox::new(Rect::new(0, 0, 200, 200));
        lb.add_items(vec!["A".to_string(), "B".to_string(), "C".to_string()]);
        lb.remove_item(1);
        assert_eq!(lb.count(), 2);
        assert_eq!(lb.item(1), Some("C"));
    }

    #[test]
    fn listbox_clear() {
        let mut lb = ListBox::new(Rect::new(0, 0, 200, 200));
        lb.add_items(vec!["A".to_string(), "B".to_string()]);
        lb.select(0);
        lb.clear();
        assert!(lb.is_empty());
        assert!(lb.selected_indices().is_empty());
        assert_eq!(lb.current_row(), None);
    }

    #[test]
    fn listbox_current_row() {
        let mut lb = ListBox::new(Rect::new(0, 0, 200, 200));
        lb.add_items(vec!["A".to_string(), "B".to_string(), "C".to_string()]);
        assert_eq!(lb.current_row(), None);
        lb.set_current_row(Some(1));
        assert_eq!(lb.current_row(), Some(1));
        lb.set_current_row(None);
        assert_eq!(lb.current_row(), None);
    }

    #[test]
    fn listbox_select() {
        let mut lb = ListBox::new(Rect::new(0, 0, 200, 200));
        lb.add_items(vec!["A".to_string(), "B".to_string(), "C".to_string()]);
        lb.select(1);
        assert!(lb.is_selected(1));
        assert_eq!(lb.selected_indices().len(), 1);
    }

    #[test]
    fn listbox_deselect() {
        let mut lb = ListBox::new(Rect::new(0, 0, 200, 200));
        lb.add_items(vec!["A".to_string(), "B".to_string()]);
        lb.select(0);
        assert!(lb.is_selected(0));
        lb.deselect(0);
        assert!(!lb.is_selected(0));
    }

    #[test]
    fn listbox_clear_selection() {
        let mut lb = ListBox::new(Rect::new(0, 0, 200, 200));
        lb.add_items(vec!["A".to_string(), "B".to_string()]);
        lb.select(0);
        lb.select(1);
        lb.clear_selection();
        assert!(lb.selected_indices().is_empty());
    }

    #[test]
    fn listbox_select_all() {
        let mut lb = ListBox::new(Rect::new(0, 0, 200, 200));
        lb.add_items(vec!["A".to_string(), "B".to_string(), "C".to_string()]);
        lb.set_selection_mode(SelectionMode::MultiSelection);
        lb.select_all();
        assert_eq!(lb.selected_indices().len(), 3);
    }

    #[test]
    fn listbox_selection_mode() {
        let mut lb = ListBox::new(Rect::new(0, 0, 200, 200));
        assert_eq!(lb.selection_mode(), SelectionMode::SingleSelection);
        lb.set_selection_mode(SelectionMode::MultiSelection);
        assert_eq!(lb.selection_mode(), SelectionMode::MultiSelection);
        lb.set_selection_mode(SelectionMode::NoSelection);
        assert_eq!(lb.selection_mode(), SelectionMode::NoSelection);
        lb.set_selection_mode(SelectionMode::ExtendedSelection);
        assert_eq!(lb.selection_mode(), SelectionMode::ExtendedSelection);
        lb.set_selection_mode(SelectionMode::SingleSelection);
        assert_eq!(lb.selection_mode(), SelectionMode::SingleSelection);
    }

    #[test]
    fn listbox_item_height() {
        let mut lb = ListBox::new(Rect::new(0, 0, 200, 200));
        lb.set_item_height(32.0);
        assert!((lb.item_height() - 32.0).abs() < f32::EPSILON);
        lb.set_item_height(0.0);
        assert!((lb.item_height() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn listbox_geometry_delegation() {
        let mut lb = ListBox::new(Rect::new(0, 0, 200, 200));
        lb.set_geometry(Rect::new(10, 10, 250, 300));
        assert_eq!(lb.geometry(), Rect::new(10, 10, 250, 300));
    }

    #[test]
    fn listbox_visibility() {
        let mut lb = ListBox::new(Rect::new(0, 0, 200, 200));
        assert!(lb.is_visible());
        lb.hide();
        assert!(!lb.is_visible());
        lb.show();
        assert!(lb.is_visible());
    }

    #[test]
    fn listbox_enabled() {
        let mut lb = ListBox::new(Rect::new(0, 0, 200, 200));
        assert!(lb.is_enabled());
        lb.set_enabled(false);
        assert!(!lb.is_enabled());
        lb.set_enabled(true);
        assert!(lb.is_enabled());
    }

    #[test]
    fn listbox_id_kind() {
        let lb_a = ListBox::new(Rect::new(0, 0, 100, 100));
        let lb_b = ListBox::new(Rect::new(0, 0, 100, 100));
        assert_ne!(lb_a.id(), lb_b.id());
        assert_eq!(lb_a.kind(), WidgetKind::ListBox);
        assert_eq!(lb_b.kind(), WidgetKind::ListBox);
    }

    #[test]
    fn listbox_signal_accessors() {
        let lb = ListBox::new(Rect::new(0, 0, 100, 100));
        let _ = &lb.item_selected;
        let _ = &lb.item_activated;
        let _ = &lb.selection_changed;
    }
}
