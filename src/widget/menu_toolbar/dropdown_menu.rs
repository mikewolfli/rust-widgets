//! DropdownMenu widget — a cascading/linked dropdown selector.
//!
//! The DropdownMenu displays a text field with a dropdown arrow. When clicked,
//! it expands to show a scrollable list of items. Each item has a value, label,
//! optional icon, enabled state, and optional children for submenu support.
//! Selecting an item emits an `item_selected` signal with the item's value.

use crate::core::{HorizontalAlignment, Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::{RenderCommand, RenderContext};
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Default height of the dropdown list when expanded.
const DEFAULT_DROPDOWN_HEIGHT: u32 = 200;
/// Height of each item row in the dropdown list.
const ITEM_HEIGHT: i32 = 30;
/// Padding inside the control.
const PADDING: i32 = 8;
/// Maximum number of items visible before scrolling.
const MAX_VISIBLE_ITEMS: usize = 8;

/// A single item in a DropdownMenu.
///
/// Each item has a value (string identifier), a display label, an optional
/// icon, an enabled state, and optional children for cascading submenus.
pub struct DropdownItem {
    /// Unique value identifier for this item.
    pub value: String,
    /// Display label shown in the dropdown.
    pub label: String,
    /// Optional icon for additional visual context.
    pub icon: Option<String>,
    /// Whether this item is selectable.
    pub enabled: bool,
    /// Optional child items for submenu support.
    pub children: Vec<DropdownItem>,
}

impl DropdownItem {
    /// Creates a new DropdownItem with the given value and label.
    pub fn new(value: &str, label: &str) -> Self {
        Self {
            value: value.to_string(),
            label: label.to_string(),
            icon: None,
            enabled: true,
            children: Vec::new(),
        }
    }

    /// Sets the icon for this item.
    pub fn with_icon(mut self, icon: &str) -> Self {
        self.icon = Some(icon.to_string());
        self
    }

    /// Sets the enabled state for this item.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Sets the children for this item (submenu support).
    pub fn with_children(mut self, children: Vec<DropdownItem>) -> Self {
        self.children = children;
        self
    }
}

/// DropdownMenu widget — a text field with a dropdown list.
///
/// Displays the currently selected value (or placeholder) with a dropdown
/// arrow. When expanded, shows a scrollable list of items. Selecting an
/// item updates the selected value and emits the `item_selected` signal.
pub struct DropdownMenu {
    base: BaseWidget,
    selected_value: Option<String>,
    items: Vec<DropdownItem>,
    expanded: bool,
    /// Scroll offset for the item list.
    scroll_offset: usize,
    /// Emitted when an item is selected, providing the item's value.
    pub item_selected: Signal1<String>,
}

impl DropdownMenu {
    /// Creates a new DropdownMenu widget with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::DropdownMenu, geometry, "DropdownMenu"),
            selected_value: None,
            items: Vec::new(),
            expanded: false,
            scroll_offset: 0,
            item_selected: Signal1::new(),
        }
    }

    /// Sets the selected item by value. If the value does not match any
    /// item, the selection is cleared.
    pub fn select(&mut self, value: &str) {
        if self.items.iter().any(|item| item.value == value && item.enabled) {
            self.selected_value = Some(value.to_string());
            self.base.request_redraw();
        }
    }

    /// Returns the currently selected value, if any.
    pub fn selected_value(&self) -> Option<String> {
        self.selected_value.clone()
    }

    /// Returns the display label for the currently selected value.
    pub fn selected_label(&self) -> Option<String> {
        self.selected_value.as_ref().and_then(|val| {
            self.items.iter().find(|item| item.value == *val).map(|item| item.label.clone())
        })
    }

    /// Adds an item to the dropdown.
    pub fn add_item(&mut self, item: DropdownItem) {
        self.items.push(item);
        self.base.request_redraw();
    }

    /// Removes an item by value. Returns true if an item was removed.
    pub fn remove_item(&mut self, value: &str) -> bool {
        let len_before = self.items.len();
        self.items.retain(|item| item.value != value);
        let removed = self.items.len() < len_before;
        if removed {
            // Clear selection if the removed item was selected
            if self.selected_value.as_deref() == Some(value) {
                self.selected_value = None;
            }
            self.base.request_redraw();
        }
        removed
    }

    /// Returns a reference to all items.
    pub fn items(&self) -> &[DropdownItem] {
        &self.items
    }

    /// Returns a mutable reference to all items.
    pub fn items_mut(&mut self) -> &mut Vec<DropdownItem> {
        &mut self.items
    }

    /// Returns the number of items.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Returns whether the dropdown is currently expanded.
    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    /// Expands the dropdown to show the item list.
    pub fn expand(&mut self) {
        self.expanded = true;
        self.scroll_offset = 0;
        self.base.request_redraw();
    }

    /// Collapses the dropdown to hide the item list.
    pub fn collapse(&mut self) {
        self.expanded = false;
        self.base.request_redraw();
    }

    /// Toggles the expanded state.
    pub fn toggle(&mut self) {
        if self.expanded {
            self.collapse();
        } else {
            self.expand();
        }
    }

    /// Computes the dropdown list rectangle below the text field.
    fn dropdown_rect(&self) -> Rect {
        let geom = self.geometry();
        let item_count = self.items.len().min(MAX_VISIBLE_ITEMS);
        let height = (item_count as u32 * ITEM_HEIGHT as u32).min(DEFAULT_DROPDOWN_HEIGHT);
        Rect::new(geom.x, geom.y + geom.height as i32, geom.width, height)
    }

    /// Computes the rectangle for an item at the given index.
    fn item_rect(&self, index: usize) -> Rect {
        let drop = self.dropdown_rect();
        Rect::new(
            drop.x,
            drop.y + (index - self.scroll_offset) as i32 * ITEM_HEIGHT,
            drop.width,
            ITEM_HEIGHT as u32,
        )
    }

    /// Finds the item index at a given position, if any.
    fn hit_test_item(&self, pos: Point) -> Option<usize> {
        if !self.expanded {
            return None;
        }
        let drop = self.dropdown_rect();
        if !drop.contains_point(pos) {
            return None;
        }
        let visible_end = (self.scroll_offset + MAX_VISIBLE_ITEMS).min(self.items.len());
        for i in self.scroll_offset..visible_end {
            let ir = self.item_rect(i);
            if ir.contains_point(pos) && self.items[i].enabled {
                return Some(i);
            }
        }
        None
    }
}

impl Widget for DropdownMenu {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(200, 200)
    }
}

impl Draw for DropdownMenu {
    fn draw(&mut self, context: &mut RenderContext) {
        let geom = self.geometry();
        let is_enabled = self.base.is_enabled();
        let font = Font::simple("sans-serif", 13.0);

        // ── Draw text field background ──
        let bg_color = if !is_enabled { Color::rgba(240, 240, 240, 180) } else { Color::WHITE };
        context.fill_rounded_rect(geom, 4, bg_color);
        context.draw_rounded_rect_stroke(geom, 4, Color::rgba(190, 190, 200, 200), 1);

        // ── Draw selected label or placeholder ──
        let display_text = self.selected_label().unwrap_or_else(|| "Select...".to_string());
        let text_color = if !is_enabled {
            Color::rgba(150, 150, 150, 200)
        } else if self.selected_value.is_some() {
            Color::rgb(33, 33, 33)
        } else {
            Color::rgba(180, 180, 180, 200)
        };
        context.draw_text(
            Point::new(geom.x + PADDING, geom.y + geom.height as i32 / 2),
            &display_text,
            &font,
            text_color,
            HorizontalAlignment::Left,
        );

        // ── Draw dropdown arrow ──
        let arrow_x = geom.x + geom.width as i32 - PADDING - 10;
        let arrow_y = geom.y + geom.height as i32 / 2 - 2;
        let arrow_color = if !is_enabled {
            Color::rgba(150, 150, 150, 180)
        } else {
            Color::rgba(100, 100, 100, 220)
        };
        if self.expanded {
            // Upward-pointing triangle when expanded
            context.execute_command(RenderCommand::DrawPath {
                points: vec![
                    Point::new(arrow_x, arrow_y + 6),
                    Point::new(arrow_x + 10, arrow_y + 6),
                    Point::new(arrow_x + 5, arrow_y),
                ],
                closed: true,
                color: arrow_color,
                filled: true,
                width: 1,
            });
        } else {
            // Downward-pointing triangle when collapsed
            context.execute_command(RenderCommand::DrawPath {
                points: vec![
                    Point::new(arrow_x, arrow_y),
                    Point::new(arrow_x + 10, arrow_y),
                    Point::new(arrow_x + 5, arrow_y + 6),
                ],
                closed: true,
                color: arrow_color,
                filled: true,
                width: 1,
            });
        }

        // ── Draw dropdown list when expanded ──
        if self.expanded && !self.items.is_empty() {
            let drop = self.dropdown_rect();
            // Dropdown background with shadow
            context.fill_rounded_rect(drop, 4, Color::WHITE);
            context.draw_rounded_rect_stroke(drop, 4, Color::rgba(180, 180, 190, 200), 2);

            // Draw visible items
            let end = (self.scroll_offset + MAX_VISIBLE_ITEMS).min(self.items.len());
            for i in self.scroll_offset..end {
                let item = &self.items[i];
                let ir = self.item_rect(i);

                // Highlight selected item
                let is_selected = self.selected_value.as_deref() == Some(&item.value);
                if is_selected {
                    context.fill_rounded_rect(ir, 2, Color::rgba(220, 235, 255, 200));
                }

                let item_text_color = if !item.enabled {
                    Color::rgba(180, 180, 180, 200)
                } else {
                    Color::rgb(33, 33, 33)
                };
                let item_font = Font::simple("sans-serif", 12.0);
                let mut item_x = ir.x + PADDING;

                // Item icon
                if let Some(ref item_icon) = item.icon {
                    let icon_font = Font::simple("sans-serif", 13.0);
                    context.draw_text(
                        Point::new(item_x, ir.y + ir.height as i32 / 2),
                        item_icon,
                        &icon_font,
                        item_text_color,
                        HorizontalAlignment::Left,
                    );
                    item_x += 20;
                }

                // Item label
                context.draw_text(
                    Point::new(item_x, ir.y + ir.height as i32 / 2),
                    &item.label,
                    &item_font,
                    item_text_color,
                    HorizontalAlignment::Left,
                );

                // Submenu indicator if item has children
                if !item.children.is_empty() && item.enabled {
                    let sub_x = ir.x + ir.width as i32 - PADDING - 8;
                    let sub_y = ir.y + ir.height as i32 / 2 - 4;
                    context.execute_command(RenderCommand::DrawPath {
                        points: vec![
                            Point::new(sub_x, sub_y),
                            Point::new(sub_x, sub_y + 8),
                            Point::new(sub_x + 5, sub_y + 4),
                        ],
                        closed: true,
                        color: Color::rgba(120, 120, 120, 200),
                        filled: true,
                        width: 1,
                    });
                }

                // Separator line
                if i + 1 < end {
                    context.draw_line(
                        Point::new(ir.x + 4, ir.y + ir.height as i32),
                        Point::new(ir.x + ir.width as i32 - 4, ir.y + ir.height as i32),
                        Color::rgba(230, 230, 235, 200),
                    );
                }
            }

            // Scroll indicators if content is scrollable
            if self.scroll_offset > 0 {
                let scroll_font = Font::simple("sans-serif", 10.0);
                context.draw_text(
                    Point::new(drop.x + drop.width as i32 / 2, drop.y + 2),
                    "▲",
                    &scroll_font,
                    Color::rgba(150, 150, 150, 180),
                    HorizontalAlignment::Left,
                );
            }
            if end < self.items.len() {
                let scroll_font = Font::simple("sans-serif", 10.0);
                context.draw_text(
                    Point::new(drop.x + drop.width as i32 / 2, drop.y + drop.height as i32 - 12),
                    "▼",
                    &scroll_font,
                    Color::rgba(150, 150, 150, 180),
                    HorizontalAlignment::Left,
                );
            }
        }
    }
}

impl EventHandler for DropdownMenu {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button } => {
                if *button == 1 {
                    if self.geometry().contains_point(*pos) {
                        // Click on the text field — toggle expand
                        self.toggle();
                    } else if self.expanded {
                        // Check if click is on a dropdown item
                        if let Some(idx) = self.hit_test_item(*pos) {
                            let value = self.items[idx].value.clone();
                            self.selected_value = Some(value.clone());
                            self.item_selected.emit(value);
                            self.collapse();
                        } else {
                            // Click outside the dropdown — collapse
                            self.collapse();
                        }
                    }
                }
            }
            Event::KeyPress { key, modifiers: _ } => {
                if *key == 27 && self.expanded {
                    // Escape — collapse
                    self.collapse();
                }
            }
            Event::Wheel { delta, modifiers: _ } => {
                if self.expanded && self.items.len() > MAX_VISIBLE_ITEMS {
                    let max_offset = self.items.len() - MAX_VISIBLE_ITEMS;
                    if delta.y > 0 {
                        self.scroll_offset = self.scroll_offset.saturating_sub(1);
                    } else if delta.y < 0 {
                        self.scroll_offset = (self.scroll_offset + 1).min(max_offset);
                    }
                    self.base.request_redraw();
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
    fn dropdown_menu_default_creation() {
        let dm = DropdownMenu::new(Rect::new(0, 0, 200, 30));
        assert_eq!(dm.selected_value(), None);
        assert_eq!(dm.item_count(), 0);
        assert!(!dm.is_expanded());
    }

    #[test]
    fn dropdown_menu_add_remove_items() {
        let mut dm = DropdownMenu::new(Rect::new(0, 0, 200, 30));
        dm.add_item(DropdownItem::new("us", "United States"));
        dm.add_item(DropdownItem::new("ca", "Canada"));
        dm.add_item(DropdownItem::new("mx", "Mexico"));
        assert_eq!(dm.item_count(), 3);

        assert!(dm.remove_item("ca"));
        assert_eq!(dm.item_count(), 2);

        assert!(!dm.remove_item("nonexistent"));
        assert_eq!(dm.item_count(), 2);
    }

    #[test]
    fn dropdown_menu_select_and_signal() {
        let mut dm = DropdownMenu::new(Rect::new(0, 0, 200, 30));
        dm.add_item(DropdownItem::new("us", "United States"));
        dm.add_item(DropdownItem::new("ca", "Canada"));

        let selected = Arc::new(Mutex::new(None));
        let sel = selected.clone();
        dm.item_selected.connect(move |val| {
            *sel.lock().unwrap() = Some(val.to_string());
        });

        // Click on the text field to expand
        dm.handle_event(&Event::mouse_press(10, 10, 1));
        assert!(dm.is_expanded());

        // Click on the first item
        let ir = dm.item_rect(0);
        dm.handle_event(&Event::mouse_press(ir.x + 5, ir.y + 5, 1));

        assert_eq!(dm.selected_value(), Some("us".to_string()));
        assert_eq!(selected.lock().unwrap().as_deref(), Some("us"));
        assert!(!dm.is_expanded());
    }

    #[test]
    fn dropdown_menu_click_outside_collapses() {
        let mut dm = DropdownMenu::new(Rect::new(0, 0, 200, 30));
        dm.add_item(DropdownItem::new("a", "Item A"));
        dm.expand();
        assert!(dm.is_expanded());

        dm.handle_event(&Event::mouse_press(500, 500, 1));
        assert!(!dm.is_expanded());
    }

    #[test]
    fn dropdown_menu_escape_collapses() {
        let mut dm = DropdownMenu::new(Rect::new(0, 0, 200, 30));
        dm.expand();
        assert!(dm.is_expanded());

        dm.handle_event(&Event::key_press(27, 0));
        assert!(!dm.is_expanded());
    }

    #[test]
    fn dropdown_menu_selected_label() {
        let mut dm = DropdownMenu::new(Rect::new(0, 0, 200, 30));
        dm.add_item(DropdownItem::new("fr", "France"));
        dm.select("fr");
        assert_eq!(dm.selected_label(), Some("France".to_string()));
        assert_eq!(dm.selected_value(), Some("fr".to_string()));
    }

    #[test]
    fn dropdown_menu_remove_item_clears_selection() {
        let mut dm = DropdownMenu::new(Rect::new(0, 0, 200, 30));
        dm.add_item(DropdownItem::new("x", "Item X"));
        dm.select("x");
        assert_eq!(dm.selected_value(), Some("x".to_string()));

        dm.remove_item("x");
        assert_eq!(dm.selected_value(), None);
    }

    #[test]
    fn dropdown_menu_toggle() {
        let mut dm = DropdownMenu::new(Rect::new(0, 0, 200, 30));
        assert!(!dm.is_expanded());

        dm.toggle();
        assert!(dm.is_expanded());

        dm.toggle();
        assert!(!dm.is_expanded());
    }

    #[test]
    fn dropdown_menu_svg_output() {
        let mut dm = DropdownMenu::new(Rect::new(0, 0, 200, 30));
        dm.add_item(DropdownItem::new("op1", "Option 1").with_icon("★"));
        dm.add_item(DropdownItem::new("op2", "Option 2").with_enabled(false));
        dm.add_item(DropdownItem::new("op3", "Option 3"));
        dm.select("op1");
        dm.expand();

        let svg = render_to_svg(&mut dm);
        assert!(svg.starts_with("<svg"), "SVG should start with <svg, got: {svg:.60}");
        assert!(svg.ends_with("</svg>"), "SVG should end with </svg>");
    }
}
