// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! MenuButton widget — a button that shows a dropdown menu when clicked.
//!
//! The MenuButton displays as a button with text and an optional icon. When
//! clicked, it toggles a dropdown menu containing items. Each item supports
//! an icon, enabled/disabled state, checked state, and optional submenu items.
//! Selecting a menu item emits an `item_triggered` signal with the item's ID.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::{RenderCommand, RenderContext};
use crate::signal::Signal1;
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Default width of the dropdown menu panel.
const DEFAULT_MENU_WIDTH: u32 = 180;
/// Height of each menu item row.
const ITEM_HEIGHT: i32 = 28;
/// Padding inside the button and menu.
const PADDING: i32 = 8;

/// A single item inside a MenuButton dropdown.
///
/// Each item has a unique ID, display text, optional icon, and flags for
/// enabled/disabled and checked state. Items can also have an optional
/// submenu (nested `MenuItem` list) for cascading menus.
pub struct MenuItem {
    /// Unique identifier for this item.
    pub id: u64,
    /// Display text.
    pub text: String,
    /// Optional icon character/emoji.
    pub icon: Option<String>,
    /// Whether this item is enabled.
    pub enabled: bool,
    /// Whether this item is checked (shows a checkmark).
    pub checked: bool,
    /// Optional submenu items for cascading menus.
    pub submenu: Option<Vec<MenuItem>>,
}

impl MenuItem {
    /// Creates a new MenuItem with the given id and text.
    pub fn new(id: u64, text: &str) -> Self {
        Self {
            id,
            text: text.to_string(),
            icon: None,
            enabled: true,
            checked: false,
            submenu: None,
        }
    }

    /// Sets the icon for this menu item.
    pub fn with_icon(mut self, icon: &str) -> Self {
        self.icon = Some(icon.to_string());
        self
    }

    /// Sets the enabled state for this menu item.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Sets the checked state for this menu item.
    pub fn with_checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// Sets the submenu items for this menu item.
    pub fn with_submenu(mut self, items: Vec<MenuItem>) -> Self {
        self.submenu = Some(items);
        self
    }
}

/// MenuButton widget — a button that opens a dropdown menu on click.
///
/// Displays a button with text and an optional icon. When clicked, it toggles
/// a dropdown menu positioned below the button. Selecting a menu item emits
/// the `item_triggered` signal with the item's ID.
pub struct MenuButton {
    base: BaseWidget,
    text: String,
    menu_items: Vec<MenuItem>,
    icon: Option<String>,
    /// Whether the dropdown menu is currently visible.
    menu_open: bool,
    /// Emitted when a menu item is clicked, providing the item ID.
    pub item_triggered: Signal1<u64>,
}

impl MenuButton {
    /// Creates a new MenuButton widget with the given geometry.
    pub fn new(text: &str, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::MenuButton, geometry, "MenuButton"),
            text: text.to_string(),
            menu_items: Vec::new(),
            icon: None,
            menu_open: false,
            item_triggered: Signal1::new(),
        }
    }

    /// Returns the button text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Sets the button text.
    pub fn set_text(&mut self, text: &str) {
        self.text = text.to_string();
        self.base.request_redraw();
    }

    /// Returns the optional icon.
    pub fn icon(&self) -> Option<&str> {
        self.icon.as_deref()
    }

    /// Sets the button icon.
    pub fn set_icon(&mut self, icon: &str) {
        self.icon = Some(icon.to_string());
        self.base.request_redraw();
    }

    /// Clears the icon.
    pub fn clear_icon(&mut self) {
        self.icon = None;
        self.base.request_redraw();
    }

    /// Adds a menu item to the dropdown.
    pub fn add_item(&mut self, item: MenuItem) {
        self.menu_items.push(item);
        self.base.request_redraw();
    }

    /// Removes a menu item by its ID.
    pub fn remove_item(&mut self, id: u64) -> bool {
        let len_before = self.menu_items.len();
        self.menu_items.retain(|item| item.id != id);
        let removed = self.menu_items.len() < len_before;
        if removed {
            self.base.request_redraw();
        }
        removed
    }

    /// Removes all menu items.
    pub fn clear_items(&mut self) {
        self.menu_items.clear();
        self.base.request_redraw();
    }

    /// Returns the number of menu items.
    pub fn item_count(&self) -> usize {
        self.menu_items.len()
    }

    /// Returns a reference to the menu items.
    pub fn items(&self) -> &[MenuItem] {
        &self.menu_items
    }

    /// Returns a mutable reference to the menu items.
    pub fn items_mut(&mut self) -> &mut Vec<MenuItem> {
        &mut self.menu_items
    }

    /// Returns whether the dropdown menu is currently open.
    pub fn is_menu_open(&self) -> bool {
        self.menu_open
    }

    /// Toggles the dropdown menu visibility.
    pub fn toggle_menu(&mut self) {
        self.menu_open = !self.menu_open;
        self.base.request_redraw();
    }

    /// Opens the dropdown menu.
    pub fn open_menu(&mut self) {
        self.menu_open = true;
        self.base.request_redraw();
    }

    /// Closes the dropdown menu.
    pub fn close_menu(&mut self) {
        self.menu_open = false;
        self.base.request_redraw();
    }

    /// Computes the menu rectangle positioned below the button.
    fn menu_rect(&self) -> Rect {
        let geom = self.geometry();
        let menu_width = geom.width.max(DEFAULT_MENU_WIDTH);
        let menu_height = (self.menu_items.len() as u32 * ITEM_HEIGHT as u32).max(1);
        Rect::new(geom.x, geom.y + geom.height as i32, menu_width, menu_height)
    }

    /// Computes the rectangle for a specific menu item at the given index.
    fn item_rect(&self, index: usize) -> Rect {
        let menu_rect = self.menu_rect();
        Rect::new(
            menu_rect.x,
            menu_rect.y + index as i32 * ITEM_HEIGHT,
            menu_rect.width,
            ITEM_HEIGHT as u32,
        )
    }

    /// Finds the index of the menu item at the given position, if any.
    fn hit_test_item(&self, pos: Point) -> Option<usize> {
        if !self.menu_open {
            return None;
        }
        let menu = self.menu_rect();
        if !menu.contains_point(pos) {
            return None;
        }
        for (i, item) in self.menu_items.iter().enumerate() {
            let ir = self.item_rect(i);
            if ir.contains_point(pos) && item.enabled {
                return Some(i);
            }
        }
        None
    }
}

impl Widget for MenuButton {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(120, 28)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `MenuButton`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `expanded` is the published
/// name for the open/closed state [`MenuButton::is_menu_open`] reports, because
/// that is the name the schema declares. `item_count` is derived from the menu
/// item list and has no setter, so writes are refused with
/// [`CapabilityAccessError::ReadOnlyProperty`].
impl WidgetProperties for MenuButton {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "item_count" => Ok(CapabilityValue::UInt(self.item_count() as u64)),
            "expanded" => Ok(CapabilityValue::Bool(self.is_menu_open())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(&expect_string(value)?);
                Ok(())
            }
            "expanded" => {
                if expect_bool(value)? {
                    self.open_menu();
                } else {
                    self.close_menu();
                }
                Ok(())
            }
            // Derived from the menu item list, which owns it.
            "item_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["text", "item_count", "expanded", BASE_PROPERTY_NAMES]
    }
}

impl Draw for MenuButton {
    fn draw(&mut self, context: &mut RenderContext) {
        let geom = self.geometry();
        let is_enabled = self.base.is_enabled();
        let font = Font::simple("sans-serif", 13.0);

        // Chrome colours resolve the explicit style first, then the theme's resolved style
        // for this control, and only then the original literal. The literal stays as the
        // fallback so an inactive theme still has a defined appearance, and because keeping
        // the old value means the existing pixel baselines cannot regress. The theme read is
        // a separate manager lock, taken and released inside `resolved_theme_style`, so no
        // guard is held across the draw (the mutex is not re-entrant).
        let style = self.base.style().clone();
        let themed = crate::style::resolved_theme_style("menu_button");
        let themed_bg = themed.as_ref().and_then(|r| r.background_color);
        let themed_border = themed.as_ref().and_then(|r| r.border_color);
        let themed_text = themed.as_ref().and_then(|r| r.text_color);
        // `menu_button` is absent from `WidgetRole::for_kind_name`'s table, so it classifies
        // as `Surface` and resolves to `theme.colors.background`; the fallback is the button's
        // own resting grey rather than that surface, because the stroke below needs a fill to
        // stand against.
        let background =
            style.background_color.or(themed_bg).unwrap_or(Color::rgba(235, 235, 240, 200));
        let border =
            style.border_color.or(themed_border).unwrap_or(Color::rgba(180, 180, 190, 200));
        let ink = style.text_color.or(themed_text).unwrap_or(Color::rgb(33, 33, 33));
        // The three button states are derived from the resolved pair instead of from three
        // separate literals, so they keep their relationship in either appearance: a disabled
        // button recedes towards the surface, an open-menu button steps towards the accent.
        let accent = crate::style::theme_manager()
            .current_theme()
            .map(|theme| theme.colors.accent)
            .unwrap_or(Color::rgba(200, 200, 220, 200));
        let disabled_ink = ink.blend(&background, 0.55);

        // ── Draw button background ──
        let bg_color = if !is_enabled {
            background.blend(&ink, 0.08)
        } else if self.menu_open {
            background.blend(&accent, 0.25)
        } else {
            background
        };
        context.fill_rounded_rect(geom, 4, bg_color);
        context.draw_rounded_rect_stroke(geom, 4, border, 1);

        // ── Draw icon if present ──
        let mut text_offset_x = geom.x + PADDING;
        if let Some(ref icon_str) = self.icon {
            let icon_font = Font::simple("sans-serif", 14.0);
            context.draw_text(
                Point::new(text_offset_x, geom.y + geom.height as i32 / 2),
                icon_str,
                &icon_font,
                if is_enabled { ink } else { disabled_ink },
                HorizontalAlignment::Left,
            );
            text_offset_x += 22; // space for icon
        }

        // ── Draw button text ──
        let text_color = if !is_enabled { disabled_ink } else { ink };
        let text_y = geom.y + geom.height as i32 / 2;
        context.draw_text(
            Point::new(text_offset_x, text_y),
            &self.text,
            &font,
            text_color,
            HorizontalAlignment::Left,
        );

        // ── Draw dropdown arrow ──
        let arrow_x = geom.x + geom.width as i32 - PADDING - 8;
        let arrow_y = geom.y + geom.height as i32 / 2 - 2;
        let arrow_color = if !is_enabled { disabled_ink } else { ink };
        // Draw a small triangle pointing down (filled path)
        context.execute_command(RenderCommand::DrawPath {
            points: vec![
                Point::new(arrow_x, arrow_y),
                Point::new(arrow_x + 8, arrow_y),
                Point::new(arrow_x + 4, arrow_y + 6),
            ],
            closed: true,
            color: arrow_color,
            filled: true,
            width: 1,
        });

        // ── Draw dropdown menu ──
        if self.menu_open {
            let menu = self.menu_rect();
            // The popup panel is chrome: a popup that stayed white in a dark theme would be
            // the brightest object on screen. It reads the resolved surface, and the literal
            // it used to be stays as the fallback.
            let panel = crate::style::resolved_theme_style("menu_button")
                .and_then(|r| r.background_color)
                .or(themed_bg)
                .unwrap_or(Color::WHITE);
            let panel_border =
                style.border_color.or(themed_border).unwrap_or(Color::rgba(190, 190, 200, 200));
            let item_ink = style.text_color.or(themed_text).unwrap_or(Color::rgb(33, 33, 33));
            // A selected row's highlight is chrome, not data, so it is resolved from the
            // theme's accent rather than from a literal tint of blue.
            let item_highlight = panel.blend(&accent, 0.3);
            let item_rule = panel.blend(&item_ink, 0.08);
            // Menu background
            context.fill_rounded_rect(menu, 4, panel);
            context.draw_rounded_rect_stroke(menu, 4, panel_border, 1);

            for (i, item) in self.menu_items.iter().enumerate() {
                let item_rect = self.item_rect(i);

                // Highlight on hover/checked
                if item.checked {
                    context.fill_rounded_rect(item_rect, 2, item_highlight);
                }

                // Item text
                let item_text_color =
                    if !item.enabled { item_ink.blend(&panel, 0.5) } else { item_ink };
                let item_font = Font::simple("sans-serif", 12.0);
                let mut item_x = item_rect.x + PADDING;

                // Item icon
                if let Some(ref item_icon) = item.icon {
                    let icon_font = Font::simple("sans-serif", 13.0);
                    context.draw_text(
                        Point::new(item_x, item_rect.y + item_rect.height as i32 / 2),
                        item_icon,
                        &icon_font,
                        item_text_color,
                        HorizontalAlignment::Left,
                    );
                    item_x += 20;
                }

                context.draw_text(
                    Point::new(item_x, item_rect.y + item_rect.height as i32 / 2),
                    &item.text,
                    &item_font,
                    item_text_color,
                    HorizontalAlignment::Left,
                );

                // Checkmark for checked items
                if item.checked {
                    let check_x = item_rect.x + item_rect.width as i32 - PADDING - 12;
                    let check_font = Font::simple("sans-serif", 12.0);
                    context.draw_text(
                        Point::new(check_x, item_rect.y + item_rect.height as i32 / 2),
                        "\u{2713}",
                        &check_font,
                        accent.contrast_color(),
                        HorizontalAlignment::Left,
                    );
                }

                // Submenu indicator
                if item.submenu.is_some() && item.enabled {
                    let sub_x = item_rect.x + item_rect.width as i32 - PADDING - 8;
                    let sub_y = item_rect.y + item_rect.height as i32 / 2 - 4;
                    context.execute_command(RenderCommand::DrawPath {
                        points: vec![
                            Point::new(sub_x, sub_y),
                            Point::new(sub_x, sub_y + 8),
                            Point::new(sub_x + 5, sub_y + 4),
                        ],
                        closed: true,
                        color: item_ink.blend(&panel, 0.45),
                        filled: true,
                        width: 1,
                    });
                }

                // Separator between items (except last)
                if i + 1 < self.menu_items.len() {
                    context.draw_line(
                        Point::new(item_rect.x + 4, item_rect.y + item_rect.height as i32),
                        Point::new(
                            item_rect.x + item_rect.width as i32 - 4,
                            item_rect.y + item_rect.height as i32,
                        ),
                        item_rule,
                    );
                }
            }
        }
    }
}

impl EventHandler for MenuButton {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button } => {
                if *button == 1 {
                    if self.geometry().contains_point(*pos) {
                        // Click on the button itself — toggle menu
                        self.toggle_menu();
                    } else if self.menu_open {
                        // Check if click is on a menu item
                        if let Some(idx) = self.hit_test_item(*pos) {
                            let item_id = self.menu_items[idx].id;
                            self.item_triggered.emit(item_id);
                            self.close_menu();
                        } else {
                            // Click outside menu — close it
                            self.close_menu();
                        }
                    }
                }
            }
            Event::KeyPress { key, modifiers: _ } => {
                if *key == 27 && self.menu_open {
                    // Escape — close menu
                    self.close_menu();
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
    fn menu_button_default_creation() {
        let mb = MenuButton::new("File", Rect::new(0, 0, 100, 30));
        assert_eq!(mb.text(), "File");
        assert_eq!(mb.item_count(), 0);
        assert!(!mb.is_menu_open());
        assert!(mb.icon().is_none());
    }

    #[test]
    fn menu_button_add_remove_items() {
        let mut mb = MenuButton::new("Edit", Rect::new(0, 0, 100, 30));
        assert_eq!(mb.item_count(), 0);

        mb.add_item(MenuItem::new(1, "Cut"));
        mb.add_item(MenuItem::new(2, "Copy"));
        mb.add_item(MenuItem::new(3, "Paste"));
        assert_eq!(mb.item_count(), 3);

        assert!(mb.remove_item(2));
        assert_eq!(mb.item_count(), 2);

        mb.clear_items();
        assert_eq!(mb.item_count(), 0);
    }

    #[test]
    fn menu_button_toggle_menu() {
        let mut mb = MenuButton::new("File", Rect::new(0, 0, 100, 30));
        assert!(!mb.is_menu_open());

        mb.toggle_menu();
        assert!(mb.is_menu_open());

        mb.toggle_menu();
        assert!(!mb.is_menu_open());
    }

    #[test]
    fn menu_button_item_triggered_signal() {
        let mut mb = MenuButton::new("File", Rect::new(0, 0, 100, 30));
        mb.add_item(MenuItem::new(42, "Open"));
        mb.add_item(MenuItem::new(99, "Save"));

        let triggered_id = Arc::new(Mutex::new(0u64));
        let tid = triggered_id.clone();
        mb.item_triggered.connect(move |val| {
            *tid.lock().unwrap() = *val;
        });

        // Open the menu
        mb.open_menu();
        assert!(mb.is_menu_open());

        // Click on the first menu item
        let item_rect = mb.item_rect(0);
        let click_pos = Point::new(item_rect.x + 5, item_rect.y + 5);
        mb.handle_event(&Event::mouse_press(click_pos.x, click_pos.y, 1));

        assert_eq!(*triggered_id.lock().unwrap(), 42);
        assert!(!mb.is_menu_open());
    }

    #[test]
    fn menu_button_click_outside_closes_menu() {
        let mut mb = MenuButton::new("File", Rect::new(0, 0, 100, 30));
        mb.add_item(MenuItem::new(1, "Item"));
        mb.open_menu();
        assert!(mb.is_menu_open());

        // Click far outside
        mb.handle_event(&Event::mouse_press(500, 500, 1));
        assert!(!mb.is_menu_open());
    }

    #[test]
    fn menu_button_escape_closes_menu() {
        let mut mb = MenuButton::new("File", Rect::new(0, 0, 100, 30));
        mb.open_menu();
        assert!(mb.is_menu_open());

        mb.handle_event(&Event::key_press(27, 0));
        assert!(!mb.is_menu_open());
    }

    #[test]
    fn menu_button_disabled_blocks_events() {
        let mut mb = MenuButton::new("File", Rect::new(0, 0, 100, 30));
        mb.set_enabled(false);

        let triggered_id = Arc::new(Mutex::new(0u64));
        let tid = triggered_id.clone();
        mb.item_triggered.connect(move |val| {
            *tid.lock().unwrap() = *val;
        });

        mb.handle_event(&Event::mouse_press(10, 10, 1));
        assert!(!mb.is_menu_open());
        assert_eq!(*triggered_id.lock().unwrap(), 0);
    }

    #[test]
    fn menu_button_icon() {
        let mut mb = MenuButton::new("Menu", Rect::new(0, 0, 100, 30));
        assert!(mb.icon().is_none());

        mb.set_icon("🔍");
        assert_eq!(mb.icon(), Some("🔍"));

        mb.clear_icon();
        assert!(mb.icon().is_none());
    }

    #[test]
    fn menu_button_svg_output() {
        let mut mb = MenuButton::new("File", Rect::new(0, 0, 120, 30));
        mb.add_item(MenuItem::new(1, "Open").with_icon("📂"));
        mb.add_item(MenuItem::new(2, "Save").with_checked(true));
        mb.open_menu();

        let svg = render_to_svg(&mut mb);
        assert!(svg.starts_with("<svg"), "SVG should start with <svg, got: {svg:.60}");
        assert!(svg.ends_with("</svg>"), "SVG should end with </svg>");
    }
}
