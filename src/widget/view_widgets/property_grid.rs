//! PropertyGrid widget — a two-column property editor table (inspector-style).
//!
//! Displays key-value pairs in a two-column layout similar to VS/Unity Inspector:
//! - Name column (left): bold text on gray background
//! - Value column (right): editable text with alternating row colors
//!   Supports row selection via click and emits a `selected` signal.

use crate::core::{HorizontalAlignment, Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// A single property row in the grid.
#[derive(Debug, Clone)]
pub struct PropertyItem {
    /// Display name shown in the left column.
    pub name: String,
    /// Current value shown in the right column.
    pub value: String,
    /// Whether the value can be edited by the user.
    pub editable: bool,
}

impl PropertyItem {
    /// Creates a new property item.
    pub fn new(name: impl Into<String>, value: impl Into<String>, editable: bool) -> Self {
        Self { name: name.into(), value: value.into(), editable }
    }
}

/// PropertyGrid widget — a two-column property editor table.
pub struct PropertyGrid {
    base: BaseWidget,
    properties: Vec<PropertyItem>,
    selected_index: Option<usize>,
    scroll_offset: u32,
    /// Emitted when a property row is selected via click.
    pub selected: Signal1<usize>,
}

impl PropertyGrid {
    /// Creates a new PropertyGrid with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::PropertyGrid, geometry, "PropertyGrid"),
            properties: Vec::new(),
            selected_index: None,
            scroll_offset: 0,
            selected: Signal1::new(),
        }
    }

    /// Adds a property item to the grid.
    pub fn add_property(
        &mut self,
        name: impl Into<String>,
        value: impl Into<String>,
        editable: bool,
    ) {
        self.properties.push(PropertyItem::new(name, value, editable));
        self.base.request_redraw();
    }

    /// Sets the value of the property at the given index.
    /// Returns `true` if the index was valid and the value was updated.
    pub fn set_value(&mut self, index: usize, value: impl Into<String>) -> bool {
        if let Some(item) = self.properties.get_mut(index) {
            item.value = value.into();
            self.base.changed.emit();
            self.base.request_redraw();
            true
        } else {
            false
        }
    }

    /// Returns a reference to the value of the property at the given index, or `None`.
    pub fn value(&self, index: usize) -> Option<&str> {
        self.properties.get(index).map(|item| item.value.as_str())
    }

    /// Removes all properties from the grid.
    pub fn clear(&mut self) {
        self.properties.clear();
        self.selected_index = None;
        self.scroll_offset = 0;
        self.base.request_redraw();
    }

    /// Returns the number of properties in the grid.
    pub fn property_count(&self) -> usize {
        self.properties.len()
    }

    /// Returns the currently selected index, if any.
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    /// Sets the selected row index. Clamps to valid range.
    pub fn set_selected_index(&mut self, index: Option<usize>) {
        let clamped = index.filter(|&i| i < self.properties.len());
        if self.selected_index != clamped {
            self.selected_index = clamped;
            if let Some(idx) = clamped {
                self.selected.emit(idx);
            }
            self.base.request_redraw();
        }
    }

    /// Returns a reference to the properties slice.
    pub fn properties(&self) -> &[PropertyItem] {
        &self.properties
    }

    /// Returns a mutable reference to the properties slice.
    pub fn properties_mut(&mut self) -> &mut Vec<PropertyItem> {
        &mut self.properties
    }
}

impl Widget for PropertyGrid {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(300, 300)
    }
}

impl Draw for PropertyGrid {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let row_height = 24u32;
        let name_col_width = rect.width / 3;
        let is_enabled = self.base.is_enabled();

        // Background
        context.fill_rect(rect, Color::WHITE);

        // Header row
        let header_font = Font::bold("Arial", 12.0);
        let header_rect = Rect::new(rect.x, rect.y, rect.width, row_height);
        context.fill_rect(header_rect, Color::rgba(60, 60, 60, 200));
        context.draw_text(
            Point::new(rect.x + 4, rect.y + 6),
            "Property",
            &header_font,
            Color::WHITE,
            HorizontalAlignment::Left,
        );
        context.draw_text(
            Point::new(rect.x + name_col_width as i32 + 4, rect.y + 6),
            "Value",
            &header_font,
            Color::WHITE,
            HorizontalAlignment::Left,
        );

        // Draw a separator line under header
        let separator_y = rect.y + row_height as i32;
        context.draw_line(
            Point::new(rect.x, separator_y),
            Point::new(rect.x + rect.width as i32, separator_y),
            Color::rgba(100, 100, 100, 200),
        );

        // Property rows
        let value_font = Font::new("Arial", 12.0, false, false);
        let mut y = separator_y + 1;

        #[allow(clippy::manual_checked_ops)]
        let visible_count = if row_height > 0 {
            (rect.height.saturating_sub(row_height + 1)) / row_height
        } else {
            0
        };

        let start_idx = self.scroll_offset as usize;
        let end_idx = (start_idx + visible_count as usize).min(self.properties.len());

        for i in start_idx..end_idx {
            let row_rect = Rect::new(rect.x, y, rect.width, row_height);
            let is_selected = self.selected_index == Some(i);

            // Alternating row background
            if is_selected {
                context.fill_rect(row_rect, Color::rgba(51, 153, 255, 80));
            } else if i % 2 == 0 {
                context.fill_rect(row_rect, Color::rgba(240, 240, 240, 200));
            } else {
                context.fill_rect(row_rect, Color::WHITE);
            }

            // Name column background
            let name_rect = Rect::new(rect.x, y, name_col_width, row_height);
            context.fill_rect(name_rect, Color::rgba(220, 220, 220, 200));

            // Name text (bold)
            let name_text_color =
                if !is_enabled { Color::GRAY } else { Color::rgba(30, 30, 30, 255) };
            context.draw_text(
                Point::new(rect.x + 4, y + 6),
                &self.properties[i].name,
                &Font::bold("Arial", 12.0),
                name_text_color,
                HorizontalAlignment::Left,
            );

            // Value text
            let value_color = if !is_enabled {
                Color::GRAY
            } else if self.properties[i].editable {
                Color::rgba(0, 0, 139, 255) // dark blue for editable
            } else {
                Color::rgba(30, 30, 30, 255)
            };
            context.draw_text(
                Point::new(rect.x + name_col_width as i32 + 4, y + 6),
                &self.properties[i].value,
                &value_font,
                value_color,
                HorizontalAlignment::Left,
            );

            // Row separator
            context.draw_line(
                Point::new(rect.x, y + row_height as i32 - 1),
                Point::new(rect.x + rect.width as i32, y + row_height as i32 - 1),
                Color::rgba(200, 200, 200, 150),
            );

            y += row_height as i32;
        }
    }
}

impl EventHandler for PropertyGrid {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } | Event::MouseRelease { pos, button } => {
                if *button == 1 {
                    let rect = self.geometry();
                    let row_height = 24u32;
                    let header_height = row_height + 1;

                    // Check if click is below header
                    let click_y = pos.y - rect.y;
                    if click_y > header_height as i32 {
                        let row_index =
                            (click_y as u32 - header_height) / row_height + self.scroll_offset;
                        let row_index = row_index as usize;
                        if row_index < self.properties.len() {
                            self.selected_index = Some(row_index);
                            self.selected.emit(row_index);
                            self.base.clicked.emit();
                            self.base.request_redraw();
                            return;
                        }
                    }

                    // Click outside rows — deselect
                    if self.selected_index.is_some() {
                        self.selected_index = None;
                        self.base.request_redraw();
                    }
                }
            }
            Event::Wheel { delta, .. } => {
                let row_height = 24u32;
                let max_scroll = (self.properties.len() as u32).saturating_sub(
                    (self.geometry().height.saturating_sub(row_height + 1)) / row_height,
                );
                if delta.y > 0 {
                    self.scroll_offset = self.scroll_offset.saturating_sub(1);
                } else if delta.y < 0 {
                    self.scroll_offset = (self.scroll_offset + 1).min(max_scroll);
                }
                self.base.request_redraw();
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
    use crate::core::Point;
    use std::sync::{Arc, Mutex};

    #[test]
    fn property_grid_new_is_empty() {
        let pg = PropertyGrid::new(Rect::new(0, 0, 300, 200));
        assert_eq!(pg.property_count(), 0);
        assert_eq!(pg.selected_index(), None);
        assert_eq!(pg.kind(), WidgetKind::PropertyGrid);
    }

    #[test]
    fn property_grid_add_property() {
        let mut pg = PropertyGrid::new(Rect::new(0, 0, 300, 200));
        pg.add_property("Name", "John Doe", true);
        pg.add_property("Age", "30", false);
        assert_eq!(pg.property_count(), 2);
        assert_eq!(pg.value(0), Some("John Doe"));
        assert_eq!(pg.value(1), Some("30"));
    }

    #[test]
    fn property_grid_set_value() {
        let mut pg = PropertyGrid::new(Rect::new(0, 0, 300, 200));
        pg.add_property("Name", "", true);
        assert!(pg.set_value(0, "Alice"));
        assert_eq!(pg.value(0), Some("Alice"));
    }

    #[test]
    fn property_grid_set_value_invalid_index() {
        let mut pg = PropertyGrid::new(Rect::new(0, 0, 300, 200));
        assert!(!pg.set_value(0, "test"));
    }

    #[test]
    fn property_grid_clear() {
        let mut pg = PropertyGrid::new(Rect::new(0, 0, 300, 200));
        pg.add_property("A", "1", true);
        pg.add_property("B", "2", true);
        pg.set_selected_index(Some(0));
        pg.clear();
        assert_eq!(pg.property_count(), 0);
        assert_eq!(pg.selected_index(), None);
    }

    #[test]
    fn property_grid_selected_signal() {
        let mut pg = PropertyGrid::new(Rect::new(0, 0, 300, 200));
        pg.add_property("X", "10", true);
        pg.add_property("Y", "20", true);

        let captured = Arc::new(Mutex::new(None));
        pg.selected.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<usize>| {
                *captured.lock().unwrap() = Some(*val);
            }
        });

        pg.set_selected_index(Some(1));
        assert_eq!(*captured.lock().unwrap(), Some(1));
        assert_eq!(pg.selected_index(), Some(1));
    }

    #[test]
    fn property_grid_mouse_click_selects_row() {
        let mut pg = PropertyGrid::new(Rect::new(0, 0, 300, 200));
        pg.add_property("A", "1", true);
        pg.add_property("B", "2", true);
        pg.add_property("C", "3", true);

        // First row is at y=25 (header at 0..24, then y=25+1=26 is row 0 start, so
        // click on row 0: y=26..49, clicking at y=30 should select index 0
        pg.handle_event(&Event::MousePress { pos: Point::new(10, 35), button: 1 });
        assert_eq!(pg.selected_index(), Some(0));
    }

    #[test]
    fn property_grid_mouse_click_header_does_not_select() {
        let mut pg = PropertyGrid::new(Rect::new(0, 0, 300, 200));
        pg.add_property("A", "1", true);

        // Click in header area (y < row_height + 1 = 25)
        pg.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
        assert_eq!(pg.selected_index(), None);
    }

    #[test]
    fn property_grid_property_item_accessors() {
        let item = PropertyItem::new("Name", "value", true);
        assert_eq!(item.name, "Name");
        assert_eq!(item.value, "value");
        assert!(item.editable);
    }

    #[test]
    fn property_grid_svg_output() {
        let mut pg = PropertyGrid::new(Rect::new(0, 0, 300, 200));
        pg.add_property("Name", "Alice", true);
        pg.add_property("Age", "30", false);
        let svg = crate::widget::svg::render_to_svg(&mut pg);
        assert!(svg.starts_with("<svg"));
    }
}
