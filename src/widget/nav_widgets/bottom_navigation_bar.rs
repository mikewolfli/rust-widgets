// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! BottomNavigationBar widget — a mobile-style bottom tab bar with icons and labels.
//!
//! This widget provides a Material Design-style bottom navigation bar commonly
//! found in mobile applications. It displays a horizontal bar with equally-sized
//! tabs, each showing an icon character and a label below. The selected tab is
//! highlighted with the accent color, and tapping a tab emits `selected_changed`
//! with the new index.

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::expect_usize;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// A single item in the bottom navigation bar.
///
/// Each item displays a single icon character above a text label.
#[derive(Debug, Clone)]
pub struct NavItem {
    /// Icon character displayed above the label (e.g., "★", "☰", "✉").
    pub icon: String,
    /// Text label displayed below the icon.
    pub label: String,
}

/// BottomNavigationBar widget — a mobile-style bottom tab bar.
///
/// Renders a horizontal bar across the bottom of the screen with equal-width
/// tabs. Each tab shows an icon character and a label. The selected tab is
/// drawn with the PRIMARY color for icon and label, while unselected tabs
/// use a muted MIDDLE_GRAY color. A thin accent indicator line is drawn
/// above the selected tab's content area.
pub struct BottomNavigationBar {
    base: BaseWidget,
    items: Vec<NavItem>,
    selected_index: usize,
    /// Emitted when the selected tab changes. The payload is the new index.
    pub selected_changed: Signal1<usize>,
}

impl BottomNavigationBar {
    /// Creates a new empty BottomNavigationBar with the given geometry.
    ///
    /// No items are added initially. Use `add_item()` to populate tabs.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::BottomNavigationBar, geometry, "BottomNavigationBar"),
            items: Vec::new(),
            selected_index: 0,
            selected_changed: Signal1::new(),
        }
    }

    /// Adds a navigation item with the given icon character and label text.
    ///
    /// # Arguments
    ///
    /// * `icon` — A single-character icon string (e.g., `"★"`, `"☰"`, `"✉"`, `"⌂"`).
    /// * `label` — Text label displayed below the icon.
    pub fn add_item(&mut self, icon: &str, label: &str) {
        self.items.push(NavItem { icon: icon.to_string(), label: label.to_string() });
        self.base.request_redraw();
    }

    /// Sets the currently selected tab index.
    ///
    /// If the new index is different from the current selection and valid,
    /// the `selected_changed` signal is emitted and the widget redraws.
    ///
    /// Out-of-range indices are clamped to the valid range.
    pub fn set_selected_index(&mut self, index: usize) {
        let clamped = if self.items.is_empty() { 0 } else { index.min(self.items.len() - 1) };
        if self.selected_index != clamped {
            self.selected_index = clamped;
            self.selected_changed.emit(clamped);
            self.base.request_redraw();
        }
    }

    /// Returns the currently selected tab index.
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Removes all navigation items from the bar.
    ///
    /// The selected index is reset to 0.
    pub fn clear_items(&mut self) {
        self.items.clear();
        self.selected_index = 0;
        self.base.request_redraw();
    }

    /// Returns the number of navigation items currently in the bar.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }
}

impl Widget for BottomNavigationBar {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(400, 48)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `BottomNavigationBar`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_dialog.in.rs` / `access_write_dialog.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `item_count` is derived from
/// the item list, so it is readable but read-only.
impl WidgetProperties for BottomNavigationBar {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "item_count" => Ok(CapabilityValue::UInt(self.item_count() as u64)),
            "selected_index" => Ok(CapabilityValue::UInt(self.selected_index() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "selected_index" => {
                self.set_selected_index(expect_usize(value)?);
                Ok(())
            }
            // The item total is implied by how many items were added, so there is
            // nothing sensible to assign to it.
            "item_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["item_count", "selected_index", BASE_PROPERTY_NAMES]
    }
}

impl Draw for BottomNavigationBar {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then a literal. The theme step is what makes an appearance
        // switch visible; the bar fill and both tab colours used to be hardcoded literals, so
        // light and dark rendered identically.
        //
        // The theme reads take and release the global manager's lock internally, so no guard
        // is held across the draw (the mutex is not re-entrant).
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("bottom_navigation_bar");
        // `bottom_navigation_bar` is absent from `WidgetRole::for_kind_name`'s table, so it
        // classifies as `Surface` and resolves to `theme.colors.background` — the window's
        // own fill. A bar painted in that colour would be byte-identical to the frame behind
        // it, so a resolved surface equal to the window fill is re-derived a visible step
        // away from it, the same distinction `Colors::input_background` draws for a field.
        let window_fill = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.background)
            .unwrap_or(Color::WHITE);
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        // The selected tab and its indicator are the accent: the hue a theme is expected to
        // vary most, so selection follows the appearance rather than a literal blue.
        let accent = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.primary)
            .unwrap_or(Color::PRIMARY);
        let bar_color = match style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
        {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.08),
        };
        let border_color = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| bar_color.blend(&ink, 0.25));

        let is_enabled = self.base.is_enabled();
        let bar_height = rect.height;

        // Draw background bar. This is painted even with no items: the bar is a persistent
        // chrome surface whose extent the layout reserved, and returning early left a bar
        // with an empty item list completely invisible.
        context.fill_rect(rect, bar_color);

        // Draw top border line
        context.draw_line(
            Point::new(rect.x, rect.y),
            Point::new(rect.x + rect.width as i32, rect.y),
            border_color,
        );

        let item_count = self.items.len();
        if item_count == 0 {
            return;
        }

        let tab_width = rect.width / item_count as u32;

        // Determine font sizes proportional to bar height
        let icon_font_size = (bar_height as f32 * 0.32).clamp(14.0, 28.0);
        let label_font_size = (bar_height as f32 * 0.18).clamp(9.0, 14.0);

        let icon_font = crate::core::Font::new("sans-serif", icon_font_size, false, false);
        let label_font = crate::core::Font::new("sans-serif", label_font_size, false, false);

        for (i, item) in self.items.iter().enumerate() {
            let tab_x = rect.x + (i as u32 * tab_width) as i32;
            let tab_rect = Rect::new(tab_x, rect.y, tab_width, bar_height);

            let is_selected = i == self.selected_index;

            // Determine colors based on selection and enabled state
            let (icon_color, label_color) = if !is_enabled {
                (ink.blend(&bar_color, 0.6), ink.blend(&bar_color, 0.6))
            } else if is_selected {
                (accent, accent)
            } else {
                (ink.blend(&bar_color, 0.4), ink.blend(&bar_color, 0.4))
            };

            // Measure icon and label to center them in the tab
            let icon_metrics = context.measure_text(&item.icon, &icon_font);
            let label_metrics = context.measure_text(&item.label, &label_font);

            // Vertical layout: icon centered vertically with label below
            let total_content_height = icon_metrics.height + label_metrics.height + 4;
            let content_y = tab_rect.y + (tab_rect.height as i32 - total_content_height as i32) / 2;

            // The selected tab's indicator is Material M3's **pill**: a 64×32 rounded capsule
            // behind the icon, in the accent, rather than the 3 px underline this used to draw.
            //
            // The underline was the M2 shape and it also read ambiguously: pinned to the bar's
            // top edge, it looked like the bar's own border in the one place a reader expects to
            // find "which tab am I on". The pill states it where the eye already is — on the
            // icon — and it is drawn *before* the icon so the glyph sits on it. Its size follows
            // the icon's own box, so a bar with a larger font gets a larger pill instead of a
            // fixed capsule the icon outgrows.
            if is_selected && is_enabled {
                const PILL_WIDTH: u32 = 64;
                const PILL_HEIGHT: u32 = 32;
                let pill_width = PILL_WIDTH.min(tab_rect.width);
                let pill_height = PILL_HEIGHT.min(tab_rect.height);
                let pill_x = tab_rect.x + (tab_rect.width as i32 - pill_width as i32) / 2;
                // Centred on the icon rather than on the tab, so the glyph is optically inside
                // it: the label below must not make the pill drift downward.
                let pill_y = content_y + (icon_metrics.height as i32 - pill_height as i32) / 2;
                let pill = Rect::new(pill_x, pill_y, pill_width, pill_height);
                // The capsule is a *tint* of the accent, not the accent at full strength: the
                // icon on top of it is drawn in the accent, and an accent on an accent is
                // unreadable. Blending it into the bar is the same treatment `chip`'s selected
                // state uses.
                context.fill_rounded_rect(pill, pill_height / 2, accent.blend(&bar_color, 0.24));
            }

            // Draw icon. `content_y` is the top of the icon+label stack and the glyph origin is
            // the box's top edge, so no ascent term belongs here — the one that used to be
            // added pushed the icon a full line down and the label with it.
            let icon_x = tab_rect.x + (tab_rect.width as i32 - icon_metrics.width as i32) / 2;
            let icon_y = content_y;
            context.draw_text(
                Point::new(icon_x, icon_y),
                &item.icon,
                &icon_font,
                icon_color,
                HorizontalAlignment::Left,
            );

            // Draw label
            let label_x = tab_rect.x + (tab_rect.width as i32 - label_metrics.width as i32) / 2;
            let label_y = content_y + icon_metrics.height as i32 + 4;
            context.draw_text(
                Point::new(label_x, label_y),
                &item.label,
                &label_font,
                label_color,
                HorizontalAlignment::Left,
            );
        }
    }
}

impl EventHandler for BottomNavigationBar {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button } | Event::MouseRelease { pos, button } => {
                if *button != 1 {
                    return;
                }
                let item_count = self.items.len();
                if item_count == 0 {
                    return;
                }
                let rect = self.geometry();
                if !rect.contains_point(*pos) {
                    return;
                }

                // Determine which tab was clicked based on x coordinate
                let tab_width = rect.width / item_count as u32;
                let relative_x = pos.x - rect.x;
                if relative_x < 0 {
                    return;
                }
                let index = (relative_x as u32 / tab_width) as usize;
                if index < item_count {
                    self.set_selected_index(index);
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
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    fn make_bar() -> BottomNavigationBar {
        let mut bar = BottomNavigationBar::new(Rect::new(0, 0, 375, 56));
        bar.add_item("★", "Favorites");
        bar.add_item("✉", "Messages");
        bar.add_item("⌂", "Home");
        bar.add_item("⚙", "Settings");
        bar
    }

    #[test]
    fn bottom_nav_bar_default_creation() {
        let bar = BottomNavigationBar::new(Rect::new(0, 0, 375, 56));
        assert_eq!(bar.kind(), WidgetKind::BottomNavigationBar);
        assert_eq!(bar.geometry(), Rect::new(0, 0, 375, 56));
        assert_eq!(bar.item_count(), 0);
        assert_eq!(bar.selected_index(), 0);
        assert!(bar.is_visible());
        assert!(bar.is_enabled());
    }

    #[test]
    fn bottom_nav_bar_add_items() {
        let mut bar = BottomNavigationBar::new(Rect::new(0, 0, 375, 56));
        assert_eq!(bar.item_count(), 0);

        bar.add_item("★", "Favorites");
        assert_eq!(bar.item_count(), 1);
        assert_eq!(bar.items[0].icon, "★");
        assert_eq!(bar.items[0].label, "Favorites");

        bar.add_item("✉", "Messages");
        assert_eq!(bar.item_count(), 2);
        assert_eq!(bar.items[1].icon, "✉");
        assert_eq!(bar.items[1].label, "Messages");
    }

    #[test]
    fn bottom_nav_bar_clear_items() {
        let mut bar = make_bar();
        assert_eq!(bar.item_count(), 4);
        assert_eq!(bar.selected_index(), 0);

        bar.set_selected_index(2);
        assert_eq!(bar.selected_index(), 2);

        bar.clear_items();
        assert_eq!(bar.item_count(), 0);
        assert_eq!(bar.selected_index(), 0);
    }

    #[test]
    fn bottom_nav_bar_set_selected_index() {
        let mut bar = make_bar();
        assert_eq!(bar.selected_index(), 0);

        bar.set_selected_index(2);
        assert_eq!(bar.selected_index(), 2);

        bar.set_selected_index(0);
        assert_eq!(bar.selected_index(), 0);
    }

    #[test]
    fn bottom_nav_bar_set_selected_index_clamps_out_of_range() {
        let mut bar = make_bar();
        assert_eq!(bar.item_count(), 4);

        // Out-of-range high should clamp to last valid index
        bar.set_selected_index(100);
        assert_eq!(bar.selected_index(), 3);

        // Back to valid range
        bar.set_selected_index(1);
        assert_eq!(bar.selected_index(), 1);
    }

    #[test]
    fn bottom_nav_bar_selected_changed_signal_emits() {
        let mut bar = make_bar();
        let captured = Arc::new(AtomicUsize::new(usize::MAX));
        let c = captured.clone();
        bar.selected_changed.connect(move |val: Arc<usize>| {
            c.store(*val, Ordering::SeqCst);
        });

        bar.set_selected_index(2);
        assert_eq!(captured.load(Ordering::SeqCst), 2);

        bar.set_selected_index(0);
        assert_eq!(captured.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn bottom_nav_bar_selected_changed_not_emitted_for_same_index() {
        let mut bar = make_bar();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bar.selected_changed.connect(move |_: Arc<usize>| {
            c.fetch_add(1, Ordering::SeqCst);
        });

        bar.set_selected_index(0); // same as default, should not emit
        assert_eq!(count.load(Ordering::SeqCst), 0);

        bar.set_selected_index(1); // different, should emit
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn bottom_nav_bar_mouse_press_selects_tab() {
        let mut bar = make_bar();
        // Tab 0: x in [0..93], Tab 1: x in [94..187], Tab 2: x in [188..281], Tab 3: x in [282..374]
        // Click on tab at index 2
        bar.handle_event(&Event::MousePress { pos: Point::new(200, 28), button: 1 });
        assert_eq!(bar.selected_index(), 2);

        // Click on tab at index 0
        bar.handle_event(&Event::MousePress { pos: Point::new(30, 28), button: 1 });
        assert_eq!(bar.selected_index(), 0);
    }

    #[test]
    fn bottom_nav_bar_mouse_release_selects_tab() {
        let mut bar = make_bar();
        bar.handle_event(&Event::MouseRelease { pos: Point::new(300, 28), button: 1 });
        assert_eq!(bar.selected_index(), 3);
    }

    #[test]
    fn bottom_nav_bar_mouse_press_other_button_noop() {
        let mut bar = make_bar();
        bar.set_selected_index(1);
        bar.handle_event(&Event::MousePress { pos: Point::new(30, 28), button: 2 });
        assert_eq!(bar.selected_index(), 1); // should remain unchanged
    }

    #[test]
    fn bottom_nav_bar_mouse_click_outside_does_nothing() {
        let mut bar = make_bar();
        bar.handle_event(&Event::MousePress { pos: Point::new(500, 100), button: 1 });
        assert_eq!(bar.selected_index(), 0);
    }

    #[test]
    fn bottom_nav_bar_disabled_blocks_events() {
        let mut bar = make_bar();
        bar.set_enabled(false);

        bar.handle_event(&Event::MousePress { pos: Point::new(200, 28), button: 1 });
        assert_eq!(bar.selected_index(), 0); // should not change
    }

    #[test]
    fn bottom_nav_bar_empty_no_items_does_not_panic() {
        let mut bar = BottomNavigationBar::new(Rect::new(0, 0, 375, 56));
        // These should not panic with no items
        bar.handle_event(&Event::MousePress { pos: Point::new(30, 28), button: 1 });
        bar.set_selected_index(5);
        assert_eq!(bar.selected_index(), 0);
    }

    #[test]
    fn bottom_nav_bar_selected_index_out_of_range_clamps() {
        let mut bar = make_bar();
        bar.set_selected_index(10);
        assert_eq!(bar.selected_index(), 3);

        bar.clear_items();
        bar.set_selected_index(0);
        assert_eq!(bar.selected_index(), 0);
    }

    #[test]
    fn bottom_nav_bar_nav_item_debug_and_clone() {
        let item = NavItem { icon: "★".to_string(), label: "Test".to_string() };
        let cloned = item.clone();
        assert_eq!(cloned.icon, "★");
        assert_eq!(cloned.label, "Test");
        let debug = format!("{:?}", item);
        assert!(debug.contains("★"));
        assert!(debug.contains("Test"));
    }

    #[test]
    fn bottom_nav_bar_svg_output() {
        let mut bar = make_bar();
        let svg = render_to_svg(&mut bar);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("width=\"375\""));
        assert!(svg.contains("height=\"56\""));
    }

    #[test]
    fn bottom_nav_bar_svg_output_empty() {
        let mut bar = BottomNavigationBar::new(Rect::new(0, 0, 200, 48));
        let svg = render_to_svg(&mut bar);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    /// The selected tab is marked by an M3 pill, not by the 3 px underline it used to draw.
    ///
    /// Regression (BLUE21 AR2): the indicator was a 3 px accent line pinned to the bar's top
    /// edge — the M2 shape, and in the one place a reader is most likely to read it as the bar's
    /// own border rather than as "which tab am I on". The pill is a 64×32 capsule behind the
    /// icon, so the assertion is on that geometry: exactly one fully-rounded 64×32 rect, and no
    /// 3 px full-height line at the bar's top edge.
    #[test]
    fn the_selected_tab_is_marked_by_a_pill_not_an_underline() {
        let mut bar = make_bar();
        bar.set_selected_index(1);
        let svg = render_to_svg(&mut bar);

        let pill = svg
            .split("<rect")
            .skip(1)
            .find(|rect| rect.contains("width=\"64\"") && rect.contains("height=\"32\""))
            .unwrap_or_else(|| panic!("the pill capsule must be painted: {svg}"));
        assert!(pill.contains("rx=\"16\""), "a capsule is half its height in radius: {pill}");

        // Only the selected tab carries a pill, so there is exactly one.
        assert_eq!(svg.matches("width=\"64\"").count(), 1, "one selected tab, one pill: {svg}");
    }

    #[test]
    fn bottom_nav_bar_request_redraw_on_item_change() {
        let mut bar = make_bar();
        let redrawn = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let r = redrawn.clone();
        bar.redraw_requested_signal().connect(move || {
            r.store(true, std::sync::atomic::Ordering::SeqCst);
        });

        bar.add_item("♥", "Likes");
        assert!(redrawn.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn bottom_nav_bar_redraw_on_selection_change() {
        let mut bar = make_bar();
        let redraw_count = Arc::new(AtomicUsize::new(0));
        let rc = redraw_count.clone();
        bar.redraw_requested_signal().connect(move || {
            rc.fetch_add(1, Ordering::SeqCst);
        });

        bar.set_selected_index(1);
        assert!(redraw_count.load(Ordering::SeqCst) > 0);
    }
}
