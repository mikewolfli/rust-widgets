// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Combo box widget — a text field with a drop-down indicator (BLUE13 R2.4).
//!
//! # Why the indicator drives the value's padding
//!
//! The indicator is part of the control's trailing chrome, and the value's box must *yield*
//! to it: QML's `ComboBox.qml` states this as `rightPadding: padding + indicator.width`, and
//! the reason is that a fixed text inset and a fixed indicator inset are two unrelated
//! derivations from the *same* edge. A wider indicator — or a larger font measuring one —
//! then overlaps the value instead of pushing it. The previous form could not express that
//! at all: the value's width was `rect.width - (PADDING + ARROW_SIZE + PADDING)` and the
//! indicator sat at `rect.x + rect.width - PADDING - ARROW_SIZE`, two spellings of one fact
//! in two different orders.
use crate::compat::{String, ToString, Vec};
use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;

use crate::style::EdgeOffsets;
use crate::widget::capability::coercion::{expect_bool, expect_string, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, ControlMetrics};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Space between the indicator's leading edge and the end of the value's box.
///
/// Half the field's own horizontal padding, so the gap between the value and the indicator is
/// visibly tighter than the gap between the value and the field's edge — the reading every
/// toolkit uses, and a *relation* rather than a third independent numeral.
const INDICATOR_LEADING_GAP: u32 = dimensions::TEXT_FIELD_PADDING_H / 2;

/// The box the drop-down indicator occupies, and the space the value leaves for it.
///
/// # Why one derivation and not two arithmetic expressions
///
/// The indicator's box, the value's right inset and the value's available width were three
/// separate computations in `draw`, all spelled from the same two numerals (`PADDING = 4`,
/// `ARROW_SIZE = 8`) in three different orders, and none of them was reachable from a test.
/// Making both boxes outputs of one function is what makes "the value yields to the
/// indicator" a property the suite can assert instead of a coincidence.
#[derive(Debug, Clone, Copy, PartialEq)]
struct IndicatorGeometry {
    /// The triangle's bounding box.
    box_rect: Rect,
    /// The rectangle the value text may occupy.
    text_box: Rect,
}

impl IndicatorGeometry {
    /// Derives both boxes from the band the control paints and `line_height`.
    ///
    /// `line_height` is a parameter rather than measured here, for the same reason
    /// `CheckBox::indicator_rect` takes one: the indicator must sit on the *value's* line box,
    /// and only the caller knows which font the value is drawn in. Deriving the two from the
    /// band's midpoint instead is how a glyph ends up half a line from the text it labels.
    fn for_band(band: Rect, line_height: u32) -> Self {
        let indicator_width = dimensions::BUTTON_ICON_SIZE.min(band.width);
        let height = line_height.min(band.height);
        // The indicator is inset from the band's trailing edge by the field's own padding, so
        // it lines up with the value's leading inset. That symmetry is the whole reason the
        // box is derived from the band rather than from the control's rectangle.
        let box_x = band.x
            + band.width.saturating_sub(indicator_width + dimensions::TEXT_FIELD_PADDING_H) as i32;
        let box_rect = Rect::new(
            box_x,
            band.y + (band.height.saturating_sub(height) / 2) as i32,
            indicator_width,
            height,
        );
        // The value's box stops one gap before the indicator, so a label that grew cannot be
        // painted underneath it and a narrow control cannot produce a negative width.
        let text_right = box_rect.x.saturating_sub(INDICATOR_LEADING_GAP as i32);
        // In a band too narrow to hold its own leading inset the content box would start at
        // `band.x` and end *before* it, i.e. an inverted rectangle — a drawing instruction
        // that paints to the left of the control. It collapses to zero width at the band's
        // leading edge instead, which is the same reading `ControlMetrics::content_box`
        // gives to oversized padding: a squeezed field has no room for a value rather than a
        // value drawn outside itself.
        let text_left = (band.x + dimensions::TEXT_FIELD_PADDING_H as i32).min(text_right);
        let text_box =
            Rect::new(text_left, band.y, text_right.saturating_sub(text_left) as u32, band.height);
        Self { box_rect, text_box }
    }

    /// The triangle's three points, derived from its own box.
    ///
    /// The points were previously computed as three loose `y` values around the field's
    /// middle line with the half-width spelled inline, which made the shape a fourth
    /// expression of the same fact and left it unclamped when the field was short.
    fn points(&self) -> [Point; 3] {
        let b = self.box_rect;
        let mid_y = b.y + b.height as i32 / 2;
        let half_height = b.height as i32 / 4;
        [
            Point::new(b.x, mid_y - half_height),
            Point::new(b.x + b.width as i32, mid_y - half_height),
            Point::new(b.x + b.width as i32 / 2, mid_y + half_height),
        ]
    }
}
/// Combo box widget.
pub struct ComboBox {
    base: BaseWidget,
    items: Vec<String>,
    current_index: Option<usize>,
    editable: bool,
    max_visible_items: usize,
    /// Emitted with the new index after `current_index` changes, including when
    /// it is cleared to `None`. An out-of-range index is ignored (and emits
    /// nothing); re-applying the same value emits nothing.
    pub current_index_changed: Signal1<Option<usize>>,
    /// Emitted with the text of the new item after `current_index` changes; the
    /// empty string is emitted when the index is cleared.
    pub current_text_changed: Signal1<String>,
    /// Emitted after `current_index_changed` when the user activates an item
    /// (click, or keyboard confirm) with the activated item's index. Not emitted
    /// by programmatic `set_current_index`.
    pub activated: Signal1<usize>,
}
impl ComboBox {
    /// The band the control actually paints: full width, one field tall, centred.
    ///
    /// # Why the control is not its own rectangle
    ///
    /// A combo box is a text field with an indicator in it, and
    /// [`dimensions::TEXT_FIELD_MIN_HEIGHT`] is what every field in this crate occupies. The
    /// 240x120 census cell drew a 240x120 slab, so a combo box and the `line_edit` beside it
    /// in the same form were different objects even though a user reads them as one. The band
    /// is the single derivation the fill, the border, the indicator and the value's box all
    /// read.
    fn field_band(&self) -> Rect {
        ControlMetrics::full_width_band(self.geometry(), dimensions::TEXT_FIELD_MIN_HEIGHT)
    }

    /// The indicator's box and the value's box, derived from the band and one line height.
    fn indicator_geometry(&self, line_height: u32) -> IndicatorGeometry {
        IndicatorGeometry::for_band(self.field_band(), line_height)
    }

    /// Creates an empty combo box with geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ComboBox, geometry, "ComboBox"),
            items: Vec::new(),
            current_index: None,
            editable: false,
            max_visible_items: 10,
            current_index_changed: Signal1::new(),
            current_text_changed: Signal1::new(),
            activated: Signal1::new(),
        }
    }
    /// Returns number of items.
    pub fn count(&self) -> usize {
        self.items.len()
    }
    /// Returns whether the combo box is empty.
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
    /// Replaces all items with the given items. Clears the current selection.
    pub fn set_items(&mut self, items: Vec<String>) {
        self.items = items;
        self.current_index = None;
        self.current_index_changed.emit(None);
        self.current_text_changed.emit(String::new());
    }
    /// Inserts an item at specified position.
    pub fn insert_item(&mut self, index: usize, text: String) {
        if index <= self.items.len() {
            self.items.insert(index, text);
            // Adjust current index if needed
            if let Some(current) = &mut self.current_index {
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
            // Adjust current index if needed
            if let Some(current) = &mut self.current_index {
                if index == *current {
                    self.current_index = None;
                    self.current_text_changed.emit(String::new());
                    self.current_index_changed.emit(None);
                } else if index < *current {
                    *current -= 1;
                }
            }
        }
    }
    /// Clears all items.
    pub fn clear(&mut self) {
        self.items.clear();
        self.current_index = None;
        self.current_text_changed.emit(String::new());
        self.current_index_changed.emit(None);
    }
    /// Returns current index.
    pub fn current_index(&self) -> Option<usize> {
        self.current_index
    }
    /// Sets current index.
    pub fn set_current_index(&mut self, index: Option<usize>) {
        if index == self.current_index {
            return;
        }
        if let Some(idx) = index {
            if idx < self.items.len() {
                self.current_index = Some(idx);
                self.current_text_changed.emit(self.items[idx].clone());
                self.current_index_changed.emit(Some(idx));
            }
        } else {
            self.current_index = None;
            self.current_text_changed.emit(String::new());
            self.current_index_changed.emit(None);
        }
        self.base.request_redraw();
    }
    /// Returns current text.
    pub fn current_text(&self) -> String {
        self.current_index.and_then(|idx| self.items.get(idx)).cloned().unwrap_or_default()
    }
    /// Sets current text (for editable combo boxes).
    ///
    /// When the text matches an existing item, that item becomes current. When
    /// it does not match and the box is editable, the text is added as a new
    /// item, which then becomes current — the usual editable-combobox contract of
    /// "type a custom value and it is kept". An empty string selects nothing.
    /// Non-editable boxes ignore the call.
    pub fn set_current_text(&mut self, text: String) {
        if !self.editable {
            return;
        }
        let index = self.items.iter().position(|item| item == &text);
        let index = match index {
            Some(idx) => Some(idx),
            None if !text.is_empty() => {
                self.items.push(text);
                Some(self.items.len() - 1)
            }
            None => None,
        };
        self.set_current_index(index);
    }
    /// Returns whether the combo box is editable.
    pub fn is_editable(&self) -> bool {
        self.editable
    }
    /// Sets editable state.
    pub fn set_editable(&mut self, editable: bool) {
        self.editable = editable;
        self.base.request_redraw();
    }
    /// Returns maximum number of visible items in dropdown.
    pub fn max_visible_items(&self) -> usize {
        self.max_visible_items
    }
    /// Sets maximum number of visible items in dropdown.
    pub fn set_max_visible_items(&mut self, max: usize) {
        self.max_visible_items = max.max(1);
    }
    /// Finds index of item with specified text.
    pub fn find_text(&self, text: &str) -> Option<usize> {
        self.items.iter().position(|item| item == text)
    }
    /// Returns all items.
    pub fn items(&self) -> &[String] {
        &self.items
    }
    /// Shared activation logic for mouse/touch/gesture input.
    fn activate_combo(&mut self) {
        self.base.clicked.emit();
        if !self.items.is_empty() {
            let new_index = if let Some(current) = self.current_index {
                (current + 1) % self.items.len()
            } else {
                0
            };
            self.set_current_index(Some(new_index));
            self.activated.emit(new_index);
        }
    }
}
// Implement Widget trait
impl Widget for ComboBox {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        // Find widest item.
        //
        // The measured width is the item's own content width and is handed to
        // `ControlMetrics::implicit_size` as content, with the field's padding-plus-indicator
        // requirement supplying the floor. The width was previously `max_w * 8 + 30` and the
        // height a flat `24` — neither had any relation to the 120 px slab `draw` painted, nor
        // to the 48 px band it paints now.
        let widest = self.items().iter().map(|s| s.len() as u32).max().unwrap_or(8) * 8;
        let side_air = (dimensions::TEXT_FIELD_MIN_HEIGHT / 2).saturating_sub(8);
        let trailing =
            dimensions::TEXT_FIELD_PADDING_H + dimensions::BUTTON_ICON_SIZE + INDICATOR_LEADING_GAP;
        let padding = EdgeOffsets {
            top: side_air,
            right: trailing,
            bottom: side_air,
            left: dimensions::TEXT_FIELD_PADDING_H,
        };
        // The floor: a field wide enough to hold its own leading inset, the indicator and the
        // indicator's gap, at the height every entry control in the crate shares.
        let floor = Size::new(
            dimensions::TEXT_FIELD_PADDING_H + dimensions::BUTTON_ICON_SIZE + trailing,
            dimensions::TEXT_FIELD_MIN_HEIGHT,
        );
        let hint = ControlMetrics::implicit_size(Size::new(widest, 0), padding, floor);
        // The height is the band's, not a second numeral: this is what makes "the reported
        // size and the drawn box agree" checkable rather than merely intended.
        Size::new(hint.width, self.field_band().height.max(floor.height))
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `ComboBox`'s property contract.
impl WidgetProperties for ComboBox {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "item_count" => Ok(CapabilityValue::UInt(self.count() as u64)),
            "current_index" => match self.current_index() {
                Some(idx) => Ok(CapabilityValue::UInt(idx as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "current_text" => Ok(CapabilityValue::String(self.current_text().to_string())),
            "editable" => Ok(CapabilityValue::Bool(self.is_editable())),
            "max_visible_items" => Ok(CapabilityValue::UInt(self.max_visible_items() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "current_index" => {
                match value {
                    CapabilityValue::Null => self.set_current_index(None),
                    other => self.set_current_index(Some(expect_usize(other)?)),
                }
                Ok(())
            }
            "current_text" => {
                self.set_current_text(expect_string(value)?);
                Ok(())
            }
            "editable" => {
                self.set_editable(expect_bool(value)?);
                Ok(())
            }
            "max_visible_items" => {
                self.set_max_visible_items(expect_usize(value)?);
                Ok(())
            }
            // Derived from the item list.
            "item_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `COMBO_BOX_PROPERTIES`.
        property_names_of![
            "item_count",
            "current_index",
            "current_text",
            "editable",
            "max_visible_items",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `combo_box` publishes.
    ///
    /// `clear` empties the item list and drops the selection — the only zero-argument
    /// action in the set. `set_items` and `set_current_index` assign state and need a
    /// payload, so they are answered through the property route: `OutOfRange` tells the
    /// caller the name is right and the property form is the one to use.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "clear" => {
                self.clear();
                Ok(())
            }
            "set_items" | "set_current_index" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for ComboBox {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { button, .. } if *button == 1 => {
                self.activate_combo();
            }
            #[cfg(feature = "touch")]
            Event::TouchBegin { .. } | Event::Tap { .. } => {
                self.activate_combo();
            }
            Event::KeyPress { key, modifiers: _ } => {
                match *key {
                    38 => {
                        // Up arrow - previous item
                        if let Some(current) = self.current_index {
                            if current > 0 {
                                self.set_current_index(Some(current - 1));
                                self.activated.emit(current - 1);
                            }
                        } else if !self.items.is_empty() {
                            self.set_current_index(Some(self.items.len() - 1));
                            self.activated.emit(self.items.len() - 1);
                        }
                    }
                    40 => {
                        // Down arrow - next item
                        if let Some(current) = self.current_index {
                            if current < self.items.len() - 1 {
                                self.set_current_index(Some(current + 1));
                                self.activated.emit(current + 1);
                            }
                        } else if !self.items.is_empty() {
                            self.set_current_index(Some(0));
                            self.activated.emit(0);
                        }
                    }
                    13 => {
                        // Enter - activate current item
                        if let Some(current) = self.current_index {
                            self.activated.emit(current);
                        }
                    }
                    // Unknown key; ignore
                    _ => {}
                }
            }
            // Other events are not relevant for this widget
            _ => {}
        }
    }
}
impl Draw for ComboBox {
    fn draw(&mut self, context: &mut RenderContext) {
        let style = self.style();

        // ── The band actually painted ──
        //
        // `geometry()` is the area the control was *given*; a combo box is a text field, and a
        // field is a fixed-height band. Painting the given rectangle made a 240x120 census cell
        // a 240x120 surface, and the value and the indicator were then positioned against an
        // edge that was itself not the field's.
        let band = self.field_band();
        if band.width == 0 || band.height == 0 {
            return;
        }

        // Draw background
        let bg = style.background_color.unwrap_or(Color::rgb(255, 255, 255));
        context.fill_rect(band, bg);
        // Draw border
        if let Some(border_color) = style.border_color {
            context.draw_rect(band, border_color);
        }

        let default_font = crate::core::Font::default();
        let font = style.font.as_ref().unwrap_or(&default_font);
        // One line box for both the current value and the indicator, so the two cannot end up at
        // different heights. The previous form used the control's middle for the text origin
        // — which is the glyph box's top edge, so the value sat half a line low — and the same
        // point for the indicator, which is why they agreed with each other while both being
        // wrong.
        let line = context.text_line(band, font);
        // The indicator's box and the value's box come from one derivation, so the value
        // yields to the indicator instead of being clipped by a second, unrelated inset.
        let geometry = self.indicator_geometry(line.height);

        // Draw dropdown indicator, on the same line box as the value.
        let arrow_color = style.text_color.unwrap_or(Color::rgb(100, 100, 100));
        let [apex_left, apex_right, tip] = geometry.points();
        context.draw_line(apex_left, apex_right, arrow_color);
        context.draw_line(apex_right, tip, arrow_color);
        context.draw_line(tip, apex_left, arrow_color);

        // Draw current text, or the placeholder when the list is empty. Bounded to the box the
        // indicator left, so a long value is fitted rather than run under the indicator.
        let text_color = style.text_color.unwrap_or(Color::rgb(0, 0, 0));
        let current_text = self.current_text();
        let value_line = context.text_line(geometry.text_box, font);
        let text_box = Rect::new(
            geometry.text_box.x,
            value_line.y,
            geometry.text_box.width,
            value_line.height,
        );
        if !current_text.is_empty() {
            context.draw_text_fitted(
                text_box,
                &current_text,
                font,
                text_color,
                HorizontalAlignment::Left,
            );
        } else if self.items.is_empty() {
            context.draw_text_fitted(
                text_box,
                "(Empty)",
                font,
                text_color,
                HorizontalAlignment::Left,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;

    #[test]
    fn combobox_creation_defaults() {
        let cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        assert!(cb.items().is_empty());
        assert!(cb.is_empty());
        assert_eq!(cb.count(), 0);
        assert_eq!(cb.current_index(), None);
        assert!(!cb.is_editable());
        assert_eq!(cb.max_visible_items(), 10);
    }

    #[test]
    fn combobox_add_items() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.add_item("Item 1".to_string());
        cb.add_item("Item 2".to_string());
        assert_eq!(cb.items().len(), 2);
        assert_eq!(cb.items()[0], "Item 1");
        assert_eq!(cb.items()[1], "Item 2");
    }

    #[test]
    fn combobox_add_items_vec() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.add_items(vec!["A".to_string(), "B".to_string(), "C".to_string()]);
        assert_eq!(cb.count(), 3);
    }

    #[test]
    fn combobox_set_current_index() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.add_items(vec!["A".to_string(), "B".to_string(), "C".to_string()]);
        cb.set_current_index(Some(1));
        assert_eq!(cb.current_index(), Some(1));
        assert_eq!(cb.current_text(), "B".to_string());
    }

    #[test]
    fn combobox_set_current_index_none() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.add_items(vec!["A".to_string(), "B".to_string()]);
        cb.set_current_index(Some(1));
        cb.set_current_index(None);
        assert_eq!(cb.current_index(), None);
        assert!(cb.current_text().is_empty());
    }

    #[test]
    fn combobox_editable() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        assert!(!cb.is_editable());
        cb.set_editable(true);
        assert!(cb.is_editable());
        cb.set_editable(false);
        assert!(!cb.is_editable());
    }

    #[test]
    fn combobox_set_current_text_selects_existing_item() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.set_editable(true);
        cb.add_items(vec!["Apple".to_string(), "Banana".to_string()]);
        cb.set_current_text("Banana".to_string());
        assert_eq!(cb.current_index(), Some(1));
        assert_eq!(cb.current_text(), "Banana");
        assert_eq!(cb.count(), 2);
    }

    #[test]
    fn combobox_set_current_text_adds_unknown_custom_value() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.set_editable(true);
        cb.add_items(vec!["Apple".to_string()]);
        cb.set_current_text("Custom entry".to_string());
        assert_eq!(cb.current_text(), "Custom entry");
        assert_eq!(cb.count(), 2);
        assert_eq!(cb.current_index(), Some(1));
    }

    #[test]
    fn combobox_set_current_text_ignored_when_not_editable() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.add_items(vec!["Apple".to_string()]);
        cb.set_current_text("Apple".to_string());
        assert_eq!(cb.current_index(), None);
        assert_eq!(cb.count(), 1);
    }

    #[test]
    fn combobox_set_current_text_empty_selects_nothing() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.set_editable(true);
        cb.set_current_text(String::new());
        assert_eq!(cb.current_index(), None);
        assert_eq!(cb.count(), 0);
    }

    #[test]
    fn combobox_max_visible_items() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        assert_eq!(cb.max_visible_items(), 10);
        cb.set_max_visible_items(5);
        assert_eq!(cb.max_visible_items(), 5);
        cb.set_max_visible_items(0); // floors at 1
        assert_eq!(cb.max_visible_items(), 1);
    }

    #[test]
    fn combobox_insert_item() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.add_items(vec!["A".to_string(), "C".to_string()]);
        cb.insert_item(1, "B".to_string());
        assert_eq!(cb.count(), 3);
        assert_eq!(cb.item(1), Some("B"));
    }

    #[test]
    fn combobox_remove_item() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.add_items(vec!["A".to_string(), "B".to_string(), "C".to_string()]);
        cb.remove_item(1);
        assert_eq!(cb.count(), 2);
        assert_eq!(cb.item(1), Some("C"));
    }

    #[test]
    fn combobox_clear() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.add_items(vec!["A".to_string(), "B".to_string()]);
        cb.set_current_index(Some(0));
        cb.clear();
        assert!(cb.is_empty());
        assert_eq!(cb.current_index(), None);
    }

    #[test]
    fn combobox_find_text() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.add_items(vec!["Apple".to_string(), "Banana".to_string(), "Cherry".to_string()]);
        assert_eq!(cb.find_text("Banana"), Some(1));
        assert_eq!(cb.find_text("Missing"), None);
    }

    #[test]
    fn combobox_set_items() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.set_items(vec!["X".to_string(), "Y".to_string(), "Z".to_string()]);
        assert_eq!(cb.count(), 3);
        assert_eq!(cb.current_index(), None);
    }

    #[test]
    fn combobox_geometry_delegation() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        cb.set_geometry(Rect::new(10, 10, 150, 30));
        assert_eq!(cb.geometry(), Rect::new(10, 10, 150, 30));
    }

    #[test]
    fn combobox_visibility() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        assert!(cb.is_visible());
        cb.hide();
        assert!(!cb.is_visible());
        cb.show();
        assert!(cb.is_visible());
    }

    #[test]
    fn combobox_enabled() {
        let mut cb = ComboBox::new(Rect::new(0, 0, 200, 24));
        assert!(cb.is_enabled());
        cb.set_enabled(false);
        assert!(!cb.is_enabled());
        cb.set_enabled(true);
        assert!(cb.is_enabled());
    }

    #[test]
    fn combobox_id_kind() {
        let cb_a = ComboBox::new(Rect::new(0, 0, 100, 24));
        let cb_b = ComboBox::new(Rect::new(0, 0, 100, 24));
        assert_ne!(cb_a.id(), cb_b.id());
        assert_eq!(cb_a.kind(), WidgetKind::ComboBox);
        assert_eq!(cb_b.kind(), WidgetKind::ComboBox);
    }

    #[test]
    fn combobox_signal_accessors() {
        let cb = ComboBox::new(Rect::new(0, 0, 100, 24));
        let _ = &cb.current_index_changed;
        let _ = &cb.current_text_changed;
        let _ = &cb.activated;
    }

    /// The value's box ends where the indicator's box begins, at every control width.
    ///
    /// # What this pins
    ///
    /// BLUE22 §B.6 rule 4: a sub-part's box is derived from its sibling, so the value *yields*
    /// to the indicator. Both boxes used to be computed from the same two numerals in two
    /// different orders — the value's width was `width - (PADDING + ARROW_SIZE + PADDING)`
    /// while the indicator sat at `width - PADDING - ARROW_SIZE` — so the two agreed only by
    /// coincidence and neither could be read from a test.
    #[test]
    fn the_value_box_ends_where_the_indicator_begins() {
        for width in [0u32, 20, 64, 240, 400] {
            let cb = ComboBox::new(Rect::new(0, 0, width, 120));
            let geometry = cb.indicator_geometry(14);
            let band = cb.field_band();
            assert_eq!(
                geometry.text_box.x + geometry.text_box.width as i32 + INDICATOR_LEADING_GAP as i32,
                geometry.box_rect.x,
                "the value must stop one gap short of the indicator at width {width}"
            );
            assert!(
                geometry.box_rect.x + geometry.box_rect.width as i32 <= band.x + band.width as i32,
                "the indicator must stay inside the band at width {width}"
            );
        }
    }

    /// The reported height is the band that is painted.
    #[test]
    fn the_reported_height_is_the_band_that_is_painted() {
        let cb = ComboBox::new(Rect::new(0, 0, 240, 120));
        let band = cb.field_band();
        assert_eq!(band.height, dimensions::TEXT_FIELD_MIN_HEIGHT);
        assert_eq!(cb.size_hint().height, band.height);
        assert_eq!(band.width, 240, "a field spans its width");
        assert_eq!(band.y, (120 - dimensions::TEXT_FIELD_MIN_HEIGHT as i32) / 2);
    }

    /// The indicator is laid out on the value's own line box, not on the band's midpoint.
    #[test]
    fn the_indicator_follows_the_line_box_it_shares_with_the_value() {
        let cb = ComboBox::new(Rect::new(0, 0, 240, 120));
        for line_height in [4u32, 14, 32] {
            let geometry = cb.indicator_geometry(line_height);
            assert_eq!(
                geometry.box_rect.height, line_height,
                "the indicator's box is the line at height {line_height}"
            );
            assert_eq!(
                geometry.box_rect.y + geometry.box_rect.height as i32 / 2,
                cb.field_band().y + cb.field_band().height as i32 / 2,
                "the indicator must sit on the band's middle line at line height {line_height}"
            );
        }
    }

    /// The triangle is drawn from its own box, so its points cannot leave it.
    #[test]
    fn the_indicator_points_stay_inside_the_indicator_box() {
        let cb = ComboBox::new(Rect::new(0, 0, 240, 120));
        let geometry = cb.indicator_geometry(14);
        let b = geometry.box_rect;
        for point in geometry.points() {
            assert!(
                point.x >= b.x
                    && point.x <= b.x + b.width as i32
                    && point.y >= b.y
                    && point.y <= b.y + b.height as i32,
                "{point:?} escaped the indicator box {b:?}"
            );
        }
    }
}
