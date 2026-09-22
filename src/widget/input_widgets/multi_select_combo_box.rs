// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! MultiSelectComboBox widget — a combo box that allows multiple selections.
//!
//! The MultiSelectComboBox widget displays a text summary of selected items
//! and a dropdown with checkboxes when expanded. Users can toggle individual
//! items on/off. The widget emits a `selection_changed` signal with the IDs
//! of all selected items whenever the selection changes.
//!
//! # Why the indicator drives the summary's right inset
//!
//! The indicator is the control's trailing chrome, and the summary's box must *yield* to it.
//! The summary was written at `rect.x + 6` with no upper bound while the indicator sat at
//! `rect.x + rect.width - 18`, so the two were independent derivations from the same field
//! and a longer summary — `"3 selected"` in a narrow form — was drawn underneath the arrow.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::style::EdgeOffsets;
use crate::widget::capability::coercion::expect_bool;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, ControlMetrics};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::collections::HashSet;

/// Space between the indicator's leading edge and the end of the summary's box.
///
/// Half the field's own horizontal padding, so the gap between the summary and the indicator
/// is tighter than the gap between the summary and the field's edge — a relation, not a third
/// independent numeral.
const INDICATOR_LEADING_GAP: u32 = dimensions::TEXT_FIELD_PADDING_H / 2;

/// The indicator's box and the box the summary may occupy, derived together.
///
/// Deriving both from one function is what makes "the summary yields to the indicator" an
/// assertable property rather than a coincidence of two numerals that happened to be edited
/// together.
#[derive(Debug, Clone, Copy, PartialEq)]
struct EditGeometry {
    /// The indicator glyph's bounding box.
    box_rect: Rect,
    /// The rectangle the summary text may occupy.
    text_box: Rect,
}

impl EditGeometry {
    /// Derives both boxes from the band the control paints and `line_height`.
    fn for_band(band: Rect, line_height: u32) -> Self {
        let indicator_width = dimensions::BUTTON_ICON_SIZE.min(band.width);
        let height = line_height.min(band.height);
        let box_rect = Rect::new(
            band.x
                + band.width.saturating_sub(indicator_width + dimensions::TEXT_FIELD_PADDING_H)
                    as i32,
            band.y + (band.height.saturating_sub(height) / 2) as i32,
            indicator_width,
            height,
        );
        // The summary's box stops one gap before the indicator. `min` with the leading inset
        // keeps a squeezed field from describing an inverted rectangle: a field with no room
        // for a summary draws none rather than one outside itself.
        let text_right = box_rect.x.saturating_sub(INDICATOR_LEADING_GAP as i32);
        let text_left = (band.x + dimensions::TEXT_FIELD_PADDING_H as i32).min(text_right);
        let text_box =
            Rect::new(text_left, band.y, text_right.saturating_sub(text_left) as u32, band.height);
        Self { box_rect, text_box }
    }
}

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
    /// The band the control actually paints: full width, one field tall, centred.
    ///
    /// # Why the control is not its own rectangle
    ///
    /// A multi-select combo box is a text field, and [`dimensions::TEXT_FIELD_MIN_HEIGHT`] is
    /// what every field in this crate occupies. The 240x120 census cell drew a 240x120
    /// rounded box whose summary was then positioned `padding + 13` down from an edge that
    /// was itself not the field's.
    fn field_band(&self) -> Rect {
        ControlMetrics::full_width_band(self.geometry(), dimensions::TEXT_FIELD_MIN_HEIGHT)
    }

    /// The indicator's box and the summary's box, derived from the band and one line height.
    fn indicator_geometry(&self, line_height: u32) -> EditGeometry {
        EditGeometry::for_band(self.field_band(), line_height)
    }

    /// Creates a new `MultiSelectComboBox` with the given geometry.
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
        if index < self.items.len() && self.items[index].enabled && self.selected.insert(index) {
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

    /// Returns the number of currently selected items.
    pub fn selected_count(&self) -> usize {
        self.selected.len()
    }

    /// Returns whether the dropdown is currently expanded.
    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    /// Sets the expanded/collapsed state of the dropdown.
    pub fn set_expanded(&mut self, expanded: bool) {
        if self.expanded != expanded {
            self.toggle_expand();
        }
    }

    /// Returns the summary text shown when the dropdown is collapsed.
    fn summary_text(&self) -> String {
        let selected_count = self.selected.len();
        if selected_count == 0 {
            "Nothing selected".to_string()
        } else if selected_count <= 2 {
            self.selected_texts().join(", ")
        } else {
            format!("{selected_count} selected")
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
        // Derived through the same metric the band is drawn from, so the reported size and the
        // painted box cannot describe two different controls. The width was a bare `200` while
        // `draw` painted the whole given rectangle; neither had a relation to the other.
        let side_air = (dimensions::TEXT_FIELD_MIN_HEIGHT / 2).saturating_sub(8);
        let trailing =
            dimensions::TEXT_FIELD_PADDING_H + dimensions::BUTTON_ICON_SIZE + INDICATOR_LEADING_GAP;
        let padding = EdgeOffsets {
            top: side_air,
            right: trailing,
            bottom: side_air,
            left: dimensions::TEXT_FIELD_PADDING_H,
        };
        // Content width: the summary's own text, at a nominal advance per character. A hint has
        // no `RenderContext` to measure with, and a hint a few pixels generous is the safe
        // direction — the paint path *fits* the summary into the box this produced.
        let content = Size::new(self.summary_text().len() as u32 * 8, 0);
        let floor = Size::new(
            dimensions::TEXT_FIELD_MIN_HEIGHT + trailing,
            dimensions::TEXT_FIELD_MIN_HEIGHT,
        );
        let hint = ControlMetrics::implicit_size(content, padding, floor);
        Size::new(hint.width, self.field_band().height.max(floor.height))
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `MultiSelectComboBox`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_input.in.rs` / `access_write_input.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `selected_count` is derived
/// from the selection set, so it is refused as read-only rather than reported as a
/// name this control does not know.
impl WidgetProperties for MultiSelectComboBox {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "selected_count" => Ok(CapabilityValue::UInt(self.selected_count() as u64)),
            "expanded" => Ok(CapabilityValue::Bool(self.is_expanded())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "expanded" => {
                self.set_expanded(expect_bool(value)?);
                Ok(())
            }
            "selected_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["selected_count", "expanded", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `multi_select_combo_box` publishes.
    ///
    /// `set_expanded` assigns the popup flag and needs a payload, so it is answered
    /// through the property route — the capability publishes no zero-argument action.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_expanded" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for MultiSelectComboBox {
    fn draw(&mut self, context: &mut RenderContext) {
        // ── The band actually painted ──
        //
        // `geometry()` is the area the control was *given*; a multi-select combo box is a text
        // field, and a field is a fixed-height band. The fill, the border, the summary and the
        // indicator are all placed from this one box, so none of them can be measured against
        // an edge the field does not have.
        let rect = self.field_band();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let is_enabled = self.base.is_enabled();

        // Chrome colours resolve explicit style first, then the theme's resolved style
        // for this control, and only then fall back to a literal. Every colour here used
        // to be a literal, so the control rendered identically in a light and a dark
        // theme — the rendering census reported it as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global
        // manager's mutex is not re-entrant.
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("multi_select_combo_box");
        let themed_ink = theme.as_ref().and_then(|t| t.text_color);
        let themed_border = theme.as_ref().and_then(|t| t.border_color);

        let ink = style.text_color.or(themed_ink).unwrap_or(Color::BLACK);
        let field = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let border = style
            .border_color
            .or(themed_border)
            .filter(|resolved| *resolved != field)
            .unwrap_or_else(|| field.blend(&ink, 0.3));

        // A disabled field is the resolved palette, one step toward the surface, rather
        // than a separate literal pair that could only ever suit a light theme.
        let bg_color = if is_enabled { field } else { field.blend(&ink, 0.08) };
        context.fill_rounded_rect(rect, 4, bg_color);

        // Border
        let border_color = border;
        context.draw_rounded_rect_stroke(rect, 4, border_color, 1);

        // The accent is the theme's `primary`: the hue a theme is expected to vary most,
        // which is what makes the ticks and the arrow follow the appearance. It is read as
        // its own lock acquisition, released before the draw's other theme reads.
        let accent = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.primary)
            .unwrap_or_else(|| ink.blend(&field, 0.4));

        // The summary's box and the indicator's box come from **one** derivation, so the
        // summary yields to the indicator instead of being written at an unconstrained offset.
        let font = Font::simple("sans-serif", 13.0);
        let line = context.text_line(rect, &font);
        let geometry = self.indicator_geometry(line.height);
        let summary = self.summary_text();
        // Fitted into the box the indicator left, on that box's own line box: a glyph origin is
        // the box's top edge, so the old `padding + 13` put a 13 px font's origin on the field's
        // middle line and drew the summary half a line low.
        let summary_line = context.text_line(geometry.text_box, &font);
        context.draw_text_fitted(
            Rect::new(
                geometry.text_box.x,
                summary_line.y,
                geometry.text_box.width,
                summary_line.height,
            ),
            &summary,
            &font,
            ink,
            HorizontalAlignment::Left,
        );

        // Draw the dropdown indicator inside its own derived box. It is the affordance that
        // tells a user the field opens, so it is held to the text floor rather than dimmed by a
        // fixed fraction — `0.45` toward the background measured 3.45:1 on the dark field.
        let arrow_text = if self.expanded { "▲" } else { "▼" };
        context.draw_text_fitted(
            geometry.box_rect,
            arrow_text,
            &font,
            ink.legible_on(bg_color, 4.5).blend(&bg_color, 0.15),
            HorizontalAlignment::Center,
        );

        // Draw dropdown if expanded
        if !self.expanded || self.items.is_empty() {
            return;
        }

        let drop_down_y = rect.y + rect.height as i32;
        let item_height = 28u32;
        let drop_down_height = item_height * self.items.len() as u32;
        let drop_rect = Rect::new(rect.x, drop_down_y, rect.width, drop_down_height);

        // Dropdown background. The list floats over whatever is beneath it, so it is the
        // field's own colour rather than the window's.
        context.fill_rounded_rect(drop_rect, 2, field);
        context.draw_rounded_rect_stroke(drop_rect, 2, border, 1);

        for (i, item) in self.items.iter().enumerate() {
            let item_rect = Rect::new(
                rect.x + 1,
                drop_down_y + (i as i32) * (item_height as i32),
                rect.width.saturating_sub(2),
                item_height,
            );

            // Row highlight. A row cannot be a different colour from the list it sits in,
            // so this is the list fill nudged toward the ink: on a light list that is
            // slightly darker, on a dark one slightly lighter.
            context.fill_rounded_rect(item_rect, 2, field.blend(&ink, 0.04));

            // Checkbox
            let checkbox_size = 14u32;
            let checkbox_x = item_rect.x + 4;
            let checkbox_y = item_rect.y + (item_height - checkbox_size) as i32 / 2;
            let checkbox_rect = Rect::new(checkbox_x, checkbox_y, checkbox_size, checkbox_size);
            // A selected box carries the accent; an unselected one the muted border.
            let checkbox_color = if self.selected.contains(&i) { accent } else { border };
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
            let item_color = if item.enabled { ink } else { ink.blend(&field, 0.6) };
            context.draw_text(
                Point::new(item_text_x, item_text_y),
                &item.text,
                &font,
                item_color,
                HorizontalAlignment::Left,
            );
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

    /// The summary's box ends where the indicator's box begins, at every control width.
    ///
    /// # What this pins
    ///
    /// BLUE22 §B.6 rule 4: a sub-part's box is derived from its sibling, so the summary *yields*
    /// to the indicator. The summary used to be written at `rect.x + 6` with no upper bound
    /// while the indicator sat at `rect.x + rect.width - 18`, so a longer summary or a wider
    /// indicator glyph put the two on top of each other.
    #[test]
    fn the_summary_box_ends_where_the_indicator_begins() {
        let trailing =
            dimensions::TEXT_FIELD_PADDING_H + dimensions::BUTTON_ICON_SIZE + INDICATOR_LEADING_GAP;
        for width in [0u32, 20, 64, 240, 400] {
            let cb = MultiSelectComboBox::new(Rect::new(0, 0, width, 120));
            let geometry = cb.indicator_geometry(14);
            let band = cb.field_band();
            assert_eq!(
                geometry.text_box.x + geometry.text_box.width as i32 + INDICATOR_LEADING_GAP as i32,
                geometry.box_rect.x,
                "the summary must stop one gap short of the indicator at width {width}"
            );
            assert!(
                geometry.box_rect.x + geometry.box_rect.width as i32 <= band.x + band.width as i32,
                "the indicator must stay inside the band at width {width}"
            );
            if width < trailing {
                assert_eq!(
                    geometry.text_box.width, 0,
                    "a band too narrow for its own chrome holds no summary at width {width}"
                );
            }
        }
    }

    /// The reported height is the band that is painted.
    #[test]
    fn the_reported_height_is_the_band_that_is_painted() {
        let cb = MultiSelectComboBox::new(Rect::new(0, 0, 240, 120));
        let band = cb.field_band();
        assert_eq!(band.height, dimensions::TEXT_FIELD_MIN_HEIGHT);
        assert_eq!(cb.size_hint().height, band.height);
        assert_eq!(band.y, (120 - dimensions::TEXT_FIELD_MIN_HEIGHT as i32) / 2);
    }
}
