//! List box widget.
use crate::core::{Alignment, Color, Font, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::object::Object;
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::{Margin, Padding, WidgetStyle};
use crate::widget::{BaseWidget, Draw, Image, Widget, WidgetKind};
/// List box widget.
pub struct ListBox {
    base: BaseWidget,
    items: Vec<String>,
    selected_indices: Vec<usize>,
    selection_mode: SelectionMode,
    current_row: Option<usize>,
    item_height: f32,
    pub item_selected: Signal1<usize>,
    pub item_activated: Signal1<usize>,
    pub selection_changed: GenericSignal,
}
/// Selection mode for list box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionMode {
    /// No selection allowed
    NoSelection,
    /// Single item selection
    SingleSelection,
    /// Multiple item selection
    MultiSelection,
    /// Extended selection with shift/ctrl
    ExtendedSelection,
}
impl Default for SelectionMode {
    fn default() -> Self {
        Self::SingleSelection
    }
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
            item_height: 20,
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
            SelectionMode::SingleSelection => {
                if self.selected_indices.len() > 1 {
                    self.selected_indices.truncate(1);
                    self.selection_changed.emit();
                }
            }
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
            SelectionMode::NoSelection => return,
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
        self.item_height = height.max(1);
    }
    /// Returns all items.
    pub fn items(&self) -> &[String] {
        &self.items
    }
    /// Returns visible item range based on scroll position.
    fn visible_range(&self) -> (usize, usize) {
        let rect = self.geometry();
        let visible_items = (rect.height / self.item_height).ceil() as usize;
        let start = 0;
        let end = self.items.len().min(start + visible_items);
        (start, end)
    }
}
// Implement Widget trait
impl Widget for ListBox {
    fn id(&self) -> ObjectId {
        self.base.id()
    }
    fn kind(&self) -> WidgetKind {
        self.base.kind()
    }
    fn geometry(&self) -> Rect {
        self.base.geometry()
    }
    fn set_geometry(&mut self, geometry: Rect) {
        self.base.set_geometry(geometry);
    }
    fn min_size(&self) -> Option<Size> {
        self.base.min_size()
    }
    fn max_size(&self) -> Option<Size> {
        self.base.max_size()
    }
    fn set_min_size(&mut self, min_size: Option<Size>) {
        self.base.set_min_size(min_size);
    }
    fn set_max_size(&mut self, max_size: Option<Size>) {
        self.base.set_max_size(max_size);
    }
    fn parent(&self) -> Option<ObjectId> {
        self.base.parent()
    }
    fn set_parent(&mut self, parent: Option<ObjectId>) {
        self.base.set_parent(parent);
    }
    fn add_child(&mut self, child: ObjectId) {
        self.base.add_child(child);
    }
    fn remove_child(&mut self, child: ObjectId) {
        self.base.remove_child(child);
    }
    fn children(&self) -> &[ObjectId] {
        self.base.children()
    }
    fn show(&mut self) {
        self.base.show();
    }
    fn hide(&mut self) {
        self.base.hide();
    }
    fn is_visible(&self) -> bool {
        self.base.is_visible()
    }
    fn set_enabled(&mut self, enabled: bool) {
        self.base.set_enabled(enabled);
    }
    fn is_enabled(&self) -> bool {
        self.base.is_enabled()
    }
    fn set_tooltip(&mut self, tooltip: String) {
        self.base.set_tooltip(tooltip);
    }
    fn tooltip(&self) -> &str {
        self.base.tooltip()
    }
    fn style(&self) -> &WidgetStyle {
        self.base.style()
    }
    fn set_style(&mut self, style: WidgetStyle) {
        self.base.set_style(style);
    }
    fn connection_scope(&self) -> &ConnectionScope {
        self.base.connection_scope()
    }
    fn hover_signal(&self) -> &Signal1<Point> {
        self.base.hover_signal()
    }
    fn mouse_down_signal(&self) -> &Signal1<(Point, u32)> {
        self.base.mouse_down_signal()
    }
    fn mouse_up_signal(&self) -> &Signal1<(Point, u32)> {
        self.base.mouse_up_signal()
    }
    fn key_down_signal(&self) -> &Signal1<(u32, u32)> {
        self.base.key_down_signal()
    }
    fn key_up_signal(&self) -> &Signal1<(u32, u32)> {
        self.base.key_up_signal()
    }
    fn focus_gained_signal(&self) -> &GenericSignal {
        self.base.focus_gained_signal()
    }
    fn focus_lost_signal(&self) -> &GenericSignal {
        self.base.focus_lost_signal()
    }
    fn redraw_requested_signal(&self) -> &GenericSignal {
        self.base.redraw_requested_signal()
    }
    fn layout_requested_signal(&self) -> &GenericSignal {
        self.base.layout_requested_signal()
    }
}
impl EventHandler for ListBox {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } => {
                if *button == 1 {
                    let rect = self.geometry();
                    if rect.contains(*pos) {
                        let item_index = ((pos.y - rect.y) / self.item_height) as usize;
                        if item_index < self.items.len() {
                            self.select(item_index);
                            self.base.clicked.emit();
                        }
                    }
                }
            }
            Event::MouseDoubleClick { pos, button } => {
                if *button == 1 {
                    let rect = self.geometry();
                    if rect.contains(*pos) {
                        let item_index = ((pos.y - rect.y) / self.item_height) as usize;
                        if item_index < self.items.len() {
                            self.select(item_index);
                            self.item_activated.emit(item_index);
                        }
                    }
                }
            }
            Event::KeyPress { key, modifiers: _ } => {
                match *key {
                    38 => {
                        // Up arrow
                        if let Some(current) = self.current_row {
                            if current > 0 {
                                self.select(current - 1);
                            }
                        } else if !self.items.is_empty() {
                            self.select(self.items.len() - 1);
                        }
                    }
                    40 => {
                        // Down arrow
                        if let Some(current) = self.current_row {
                            if current < self.items.len() - 1 {
                                self.select(current + 1);
                            }
                        } else if !self.items.is_empty() {
                            self.select(0);
                        }
                    }
                    36 => {
                        // Home
                        if !self.items.is_empty() {
                            self.select(0);
                        }
                    }
                    35 => {
                        // End
                        if !self.items.is_empty() {
                            self.select(self.items.len() - 1);
                        }
                    }
                    13 => {
                        // Enter - activate current item
                        if let Some(current) = self.current_row {
                            self.item_activated.emit(current);
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}
impl Draw for ListBox {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw base widget
        let rect = self.geometry();
        let padding = 2;
        // Draw background
        context.fill_rect(
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            Color::from_rgb(255, 255, 255),
        );
        // Draw border
        context.draw_rect(
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            Color::from_rgb(200, 200, 200),
        );
        // Draw items
        let (start, end) = self.visible_range();
        for i in start..end {
            let item_y = rect.y + (i as f32 * self.item_height);
            let item_rect = Rect::new(rect.x, item_y, rect.width, self.item_height);
            // Draw item background
            if self.is_selected(i) {
                context.fill_rect(
                    item_rect.x,
                    item_rect.y,
                    item_rect.width,
                    item_rect.height,
                    Color::from_rgb(0, 120, 215),
                );
            } else if Some(i) == self.current_row {
                context.fill_rect(
                    item_rect.x,
                    item_rect.y,
                    item_rect.width,
                    item_rect.height,
                    Color::from_rgb(240, 240, 240),
                );
            }
            // Draw item text
            if let Some(text) = self.item(i) {
                let text_color = if self.is_selected(i) {
                    Color::from_rgb(255, 255, 255)
                } else {
                    Color::from_rgb(0, 0, 0)
                };
                context.draw_text(
                    item_rect.x + padding,
                    item_rect.y + self.item_height / 2,
                    text,
                    &Font::default(),
                    text_color,
                    Alignment::Left,
                );
            }
            // Draw item separator
            if i < end - 1 {
                context.draw_line(Point::new(item_rect.x as f32, item_rect.y + item_rect.height as f32), Point::new(item_rect.x + item_rect.width as f32, item_rect.y + item_rect.height as f32), Color::from_rgb(230, 230, 230),
                );
            }
        }
    }
}
