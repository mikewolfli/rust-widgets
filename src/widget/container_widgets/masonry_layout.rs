// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! MasonryLayout widget — a Pinterest-style waterfall grid layout.
//!
//! Items are arranged in a vertical waterfall across a configurable number of
//! columns. Each item is drawn as a filled rounded rectangle with a label.

use crate::core::{Color, Font, HorizontalAlignment, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::capability::coercion::expect_u32;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// An individual item in the masonry layout.
#[derive(Debug, Clone)]
pub struct MasonryItem {
    /// Display label text.
    pub label: String,
    /// Height of the item in logical pixels.
    pub height: u32,
    /// Fill color of the item.
    pub color: Color,
}

/// MasonryLayout widget — a Pinterest-style waterfall grid.
pub struct MasonryLayout {
    base: BaseWidget,
    columns: u32,
    items: Vec<MasonryItem>,
}

impl MasonryLayout {
    /// Creates a new MasonryLayout widget with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::MasonryLayout, geometry, "MasonryLayout"),
            columns: 2,
            items: Vec::new(),
        }
    }

    /// Returns the number of columns.
    pub fn columns(&self) -> u32 {
        self.columns
    }

    /// Sets the number of columns (minimum 1).
    pub fn set_columns(&mut self, columns: u32) {
        self.columns = columns.max(1);
        self.base.request_redraw();
    }

    /// Returns a reference to the items slice.
    pub fn items(&self) -> &[MasonryItem] {
        &self.items
    }

    /// Adds an item to the masonry layout.
    pub fn add_item(&mut self, label: &str, height: u32, color: Color) {
        self.items.push(MasonryItem { label: label.to_string(), height, color });
        self.base.request_redraw();
    }

    /// Removes all items from the layout.
    pub fn clear_items(&mut self) {
        self.items.clear();
        self.base.request_redraw();
    }

    /// Calculate positions for all items (waterfall layout).
    fn layout_items(&self) -> Vec<(MasonryItem, Rect)> {
        if self.items.is_empty() || self.columns == 0 {
            return Vec::new();
        }

        let rect = self.base.geometry();
        let col_w = rect.width / self.columns;
        let spacing: u32 = 4; // gap between items
        let _corner_radius: u32 = 4;

        // Track the current y-offset for each column.
        let mut col_heights = vec![0u32; self.columns as usize];
        let mut result = Vec::with_capacity(self.items.len());

        for item in &self.items {
            // Find the column with the smallest current height.
            let (min_col, _) = col_heights.iter().enumerate().min_by_key(|&(_, h)| *h).unwrap();

            let x = rect.x + (min_col as u32 * col_w) as i32;
            let y = rect.y + col_heights[min_col] as i32;

            let item_height = item.height.max(20);
            let item_rect = Rect::new(x, y, col_w, item_height);

            result.push((item.clone(), item_rect));

            col_heights[min_col] += item_height + spacing;
        }

        result
    }
}

impl Widget for MasonryLayout {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(300, 300)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `MasonryLayout`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_dialog.in.rs` / `access_write_dialog.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `item_count` is derived from
/// the item list, so it is readable but read-only.
impl WidgetProperties for MasonryLayout {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "column_count" => Ok(CapabilityValue::UInt(self.columns() as u64)),
            "item_count" => Ok(CapabilityValue::UInt(self.items().len() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "column_count" => {
                self.set_columns(expect_u32(value)?);
                Ok(())
            }
            // The item total is implied by how many items were added, so there is
            // nothing sensible to assign to it.
            "item_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["column_count", "item_count", BASE_PROPERTY_NAMES]
    }
}

impl Draw for MasonryLayout {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let corner_radius: u32 = 6;
        let font = Font::simple("Arial", 12.0);

        // The container behind the cards is this control's chrome, so it follows the
        // theme. `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not re-entrant).
        //
        // `masonry_layout` is absent from `WidgetRole::for_kind_name`'s table, so it
        // classifies as `Surface` and resolves to `theme.colors.background` — the window's
        // own fill. Painting that leaves the panel byte-identical to the frame behind it,
        // so the resolved surface is re-derived a visible step away from the window's ink
        // colour; a colour the caller set still wins.
        let style = self.base.style().clone();
        let themed = crate::style::resolved_theme_style("masonry_layout");
        let themed_bg = themed.as_ref().and_then(|r| r.background_color);
        let themed_text = themed.as_ref().and_then(|r| r.text_color);
        // Precedence: explicit style → theme → the ORIGINAL LITERAL. The literal has to
        // stay: an inactive theme still needs a defined appearance, and keeping the old
        // value means the existing pixel baselines cannot regress.
        let base = style.background_color.or(themed_bg).unwrap_or(Color::rgba(180, 180, 180, 200));
        let ink = style.text_color.or(themed_text).unwrap_or_else(|| base.contrast_color());
        let container_bg = base.blend(&ink, 0.04);

        let layout = self.layout_items();

        // The panel is drawn first so an empty layout still shows where the control is
        // rather than being indistinguishable from the frame behind it.
        context.fill_rect(rect, container_bg);

        for (_item, item_rect) in &layout {
            // Draw the item card background. `_item.color` is the per-card colour the
            // CALLER supplied — it identifies the card, so it is data and is passed
            // through untouched rather than resolved from the theme.
            context.fill_rounded_rect(*item_rect, corner_radius, _item.color);

            // Draw the label text centered in the item, in whichever ink is legible on
            // that card's own colour.
            let text_color = _item.color.contrast_color();
            let text_x = item_rect.x + 6;
            let text_y = item_rect.y + (item_rect.height as i32 / 2) - 6;
            context.draw_text(
                crate::core::Point::new(text_x, text_y),
                &_item.label,
                &font,
                text_color,
                HorizontalAlignment::Left,
            );
        }

        // If there are no items, draw an empty-state hint. The hint is chrome, so it reads
        // the resolved ink instead of a literal grey.
        if self.items.is_empty() {
            let hint = "No items";
            let hint_color = ink.blend(&base, 0.35);
            let hint_font = Font::simple("Arial", 16.0);
            let hint_x = rect.x + (rect.width / 4) as i32;
            let hint_y = rect.y + (rect.height / 3) as i32;
            context.draw_text(
                crate::core::Point::new(hint_x, hint_y),
                hint,
                &hint_font,
                hint_color,
                HorizontalAlignment::Left,
            );
        }
    }
}

impl EventHandler for MasonryLayout {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;

    #[test]
    fn masonry_creation() {
        let ml = MasonryLayout::new(Rect::new(0, 0, 300, 500));
        assert_eq!(ml.columns(), 2);
        assert!(ml.items().is_empty());
        assert_eq!(ml.kind(), WidgetKind::MasonryLayout);
    }

    #[test]
    fn masonry_set_columns() {
        let mut ml = MasonryLayout::new(Rect::new(0, 0, 300, 500));
        ml.set_columns(3);
        assert_eq!(ml.columns(), 3);
        ml.set_columns(0); // should clamp to 1
        assert_eq!(ml.columns(), 1);
    }

    #[test]
    fn masonry_add_item() {
        let mut ml = MasonryLayout::new(Rect::new(0, 0, 300, 500));
        ml.add_item("Item A", 80, Color::rgba(52, 152, 219, 255));
        ml.add_item("Item B", 120, Color::rgba(231, 76, 60, 255));
        assert_eq!(ml.items().len(), 2);
        assert_eq!(ml.items()[0].label, "Item A");
        assert_eq!(ml.items()[0].height, 80);
        assert_eq!(ml.items()[1].label, "Item B");
        assert_eq!(ml.items()[1].height, 120);
    }

    #[test]
    fn masonry_clear_items() {
        let mut ml = MasonryLayout::new(Rect::new(0, 0, 300, 500));
        ml.add_item("Item", 80, Color::rgba(52, 152, 219, 255));
        assert_eq!(ml.items().len(), 1);
        ml.clear_items();
        assert!(ml.items().is_empty());
    }

    #[test]
    fn masonry_layout_items_returns_correct_count() {
        let mut ml = MasonryLayout::new(Rect::new(0, 0, 300, 500));
        ml.add_item("A", 60, Color::RED);
        ml.add_item("B", 80, Color::GREEN);
        ml.add_item("C", 100, Color::BLUE);
        let layout = ml.layout_items();
        assert_eq!(layout.len(), 3);
    }

    #[test]
    fn masonry_svg_output() {
        let mut ml = MasonryLayout::new(Rect::new(0, 0, 300, 400));
        ml.add_item("Card 1", 80, Color::rgba(52, 152, 219, 255));
        ml.add_item("Card 2", 120, Color::rgba(231, 76, 60, 255));
        let svg = crate::widget::svg::render_to_svg(&mut ml);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        // Should contain fill operations for the rounded rects.
        assert!(svg.contains("fill="));
    }

    #[test]
    fn masonry_event_handler_delegates() {
        let mut ml = MasonryLayout::new(Rect::new(0, 0, 300, 400));
        // Should not panic.
        ml.handle_event(&Event::MouseMove { pos: Point::new(50, 50) });
        ml.handle_event(&Event::KeyDown((65, 0)));
    }
}
