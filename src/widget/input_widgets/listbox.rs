// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! List box widget.
use crate::compat::{String, ToString, Vec};
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};

use crate::widget::capability::coercion::{
    expect_f32, expect_list_box_selection_mode, expect_usize,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Formats a list-box [`SelectionMode`] as its published token.
///
/// Kept as a local free function rather than importing
/// `capability::access::list_box_selection_mode_to_str`: that helper lives behind
/// the device-profile gate, and this control must answer its own contract in
/// every profile, including `embedded` and `mini`.
fn list_box_selection_mode_to_str(mode: SelectionMode) -> &'static str {
    match mode {
        SelectionMode::Single => "single",
        SelectionMode::Multi => "multi",
        SelectionMode::Extended => "extended",
        SelectionMode::None => "none",
    }
}

/// List box widget.
pub struct ListBox {
    base: BaseWidget,
    items: Vec<String>,
    selected_indices: Vec<usize>,
    selection_mode: SelectionMode,
    current_row: Option<usize>,
    /// The row a Shift-extend grows from, set by the last non-extending selection.
    ///
    /// Only [`SelectionMode::Extended`] reads it, but it is maintained on every
    /// selection so switching into that mode mid-session starts from the row the user
    /// last touched rather than from nothing.
    anchor: Option<usize>,
    item_height: f32,
    scroll_offset: usize,
    /// Emitted when the cursor row changes to a valid item, with that item's
    /// index. Also emitted when programmatically setting the current row.
    pub item_selected: Signal1<usize>,
    /// Emitted when the user confirms an item (double-click or Enter), with the
    /// item's index. Not emitted by programmatic selection.
    pub item_activated: Signal1<usize>,
    /// Emitted without a payload after any selection-affecting change —
    /// including mode switches and `clear()`, which can alter the selection
    /// without changing the cursor row.
    pub selection_changed: GenericSignal,
}
/// Selection mode for list, tree, table and list-box views.
///
/// This is the **canonical definition**, placed at the always-available input
/// layer because [`ListBox`] needs it in every profile while the view widgets are
/// `full_widgets`-gated. `view_widgets::list_view::SelectionMode` and
/// `app::SelectionMode` re-export it, so a mode read from a handle, a `ListView`
/// or a `ListBox` is the same type and can be passed between them with no
/// conversion (principle #54).
///
/// `None` means the view accepts no selection at all, which is distinct from an
/// empty selection in `Single`/`Multi` mode: it is a property of the view, not a
/// state of the data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SelectionMode {
    /// At most one row can be selected.
    #[default]
    Single,
    /// Multiple rows can be selected (toggle behaviour).
    Multi,
    /// Multiple rows can be selected with modifier keys: a plain click replaces the
    /// selection, `Ctrl`/`Primary` toggles one row, and `Shift` extends from the
    /// anchor.
    ///
    /// This is the Windows-explorer interaction model. It differs from [`Self::Multi`]
    /// in that a *plain* click starts a new selection rather than adding to the old one,
    /// which is why the two are not the same mode spelled twice.
    Extended,
    /// No row can be selected.
    None,
}
impl ListBox {
    /// Creates an empty list box.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ListBox, geometry, "ListBox"),
            items: Vec::new(),
            selected_indices: Vec::new(),
            selection_mode: SelectionMode::Single,
            current_row: None,
            anchor: None,
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
            // The anchor is a row number too, so it shifts with the rows it points at.
            // Leaving it behind would make a later Shift-extend grow from the wrong row.
            if let Some(anchor) = &mut self.anchor {
                if index <= *anchor {
                    *anchor += 1;
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
            // An anchor on the removed row no longer addresses anything, and one after
            // it moves down with the rows.
            self.anchor = match self.anchor {
                Some(anchor) if anchor == index => None,
                Some(anchor) if anchor > index => Some(anchor - 1),
                other => other,
            };
            self.selection_changed.emit();
        }
    }
    /// Clears all items.
    pub fn clear(&mut self) {
        self.items.clear();
        self.selected_indices.clear();
        self.current_row = None;
        self.anchor = None;
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
            SelectionMode::None => {
                self.selected_indices.clear();
                self.current_row = None;
                self.anchor = None;
                self.selection_changed.emit();
            }
            SelectionMode::Single if self.selected_indices.len() > 1 => {
                self.selected_indices.truncate(1);
                self.selection_changed.emit();
            }
            SelectionMode::Single => {
                // A single selection has no range to extend from, so an anchor left over
                // from `Extended` would be a stale row number nothing can use.
                self.anchor = self.current_row;
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
    ///
    /// The mode decides what a plain selection means:
    ///
    /// * `Single` / `Multi` — as before.
    /// * `Extended` — a plain call behaves like `Single` (a new selection), because in
    ///   that mode the *modifiers* are what extend. Use [`ListBox::select_with_modifiers`]
    ///   to express ctrl-toggle and shift-extend; a plain `select` cannot know them.
    pub fn select(&mut self, index: usize) {
        if index >= self.items.len() {
            return;
        }
        match self.selection_mode {
            SelectionMode::None => (),
            SelectionMode::Single | SelectionMode::Extended => {
                self.anchor = Some(index);
                self.selected_indices.clear();
                self.selected_indices.push(index);
                self.current_row = Some(index);
                self.item_selected.emit(index);
                self.selection_changed.emit();
            }
            SelectionMode::Multi => {
                if !self.selected_indices.contains(&index) {
                    self.selected_indices.push(index);
                    self.current_row = Some(index);
                    self.item_selected.emit(index);
                    self.selection_changed.emit();
                }
            }
        }
    }

    /// Selects `index` honouring the modifier keys held at the time.
    ///
    /// # What each modifier does
    ///
    /// * **Shift** — extends from the anchor to `index` (the range is inclusive and
    ///   works in either direction), replacing the selection with that range.
    /// * **Ctrl / Primary** — toggles `index` without touching the rest, and moves the
    ///   anchor to it so a following Shift extends from there.
    /// * **Neither** — a plain selection, identical to [`ListBox::select`].
    ///
    /// # Why this is a separate entry point
    ///
    /// `Event::MousePress` carries no modifier mask, so the widget layer cannot derive
    /// the modifiers from the event. A caller that *does* have them (a keyboard-driven
    /// host, or a backend that folds modifiers into the press) calls this instead. Before
    /// it existed, `Extended` was documented as modifier-driven but its implementation
    /// was a copy of `Multi`: no anchor, no range, no toggle.
    ///
    /// # Modes other than `Extended`
    ///
    /// This is only meaningful for [`SelectionMode::Extended`]. In the other modes it
    /// delegates to [`ListBox::select`], so a caller does not have to know the current
    /// mode to call it safely.
    pub fn select_with_modifiers(&mut self, index: usize, modifiers: crate::shortcut::Modifiers) {
        if index >= self.items.len() {
            return;
        }
        if self.selection_mode != SelectionMode::Extended {
            self.select(index);
            return;
        }
        if modifiers.contains(crate::shortcut::Modifiers::SHIFT) {
            // The anchor is the row the user last selected without extending. With no
            // anchor (a Shift-click on a never-touched list) the range is just this row,
            // which is also the anchor it leaves behind.
            let anchor = self.anchor.unwrap_or(index).min(self.items.len() - 1);
            let (low, high) = if anchor <= index { (anchor, index) } else { (index, anchor) };
            self.selected_indices.clear();
            self.selected_indices.extend(low..=high);
            self.current_row = Some(index);
            self.item_selected.emit(index);
            self.selection_changed.emit();
            return;
        }
        if modifiers.contains(crate::shortcut::Modifiers::CTRL) {
            if let Some(pos) = self.selected_indices.iter().position(|&i| i == index) {
                self.selected_indices.remove(pos);
            } else {
                self.selected_indices.push(index);
                // Keep the list ordered so `selected_indices` reads as a set of rows in
                // visual order rather than in click order.
                self.selected_indices.sort_unstable();
                self.item_selected.emit(index);
            }
            self.current_row = Some(index);
            self.anchor = Some(index);
            self.selection_changed.emit();
            return;
        }
        self.select(index);
    }

    /// Returns the anchor a Shift-extend grows from, if one has been set.
    pub fn selection_anchor(&self) -> Option<usize> {
        self.anchor
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
            self.anchor = None;
            self.selection_changed.emit();
        }
    }
    /// Selects all items.
    pub fn select_all(&mut self) {
        if self.selection_mode == SelectionMode::None {
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
        if let Some(item_index) = self.item_index_at_y(pos) {
            // `Event::MousePress` carries no modifier mask, so a backend that wants
            // ctrl-toggle / shift-extend folds the modifiers into the event it
            // translates and calls `select_with_modifiers` instead of relying on
            // this path. What a press *can* express is reaching here.
            self.select(item_index);
            self.base.clicked.emit();
        }
    }
    /// Activates item at a pixel position (select + activate signal).
    fn activate_at_pos(&mut self, pos: Point) {
        if let Some(item_index) = self.item_index_at_y(pos) {
            self.select(item_index);
            self.item_activated.emit(item_index);
        }
    }
    /// Sets current row.
    pub fn set_current_row(&mut self, row: Option<usize>) {
        let old = self.current_row;
        let new = match row {
            Some(r) if r < self.items.len() => Some(r),
            Some(_) => old,
            None => None,
        };

        if old == new {
            return;
        }

        self.current_row = new;
        self.base.request_redraw();
        if let Some(index) = new {
            self.item_selected.emit(index);
        }
        self.selection_changed.emit();
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
    /// Returns the absolute index of the item drawn at widget-relative `y`.
    ///
    /// `draw` maps absolute item `i` to `y = i * item_height` (see
    /// [`visible_range`](Self::visible_range), which yields absolute indices and is
    /// what `draw` iterates). Scrolling therefore pushes items off the **top** of the
    /// widget rather than shifting the remainder down, so the row index derived from a
    /// click *is* the absolute item index and `scroll_offset` must not be added again.
    ///
    /// It used to be: with `scroll_offset = 3`, a click on the row painted as item 3
    /// selected item 6 — every click off by exactly `scroll_offset` rows.
    fn item_index_at_y(&self, pos: Point) -> Option<usize> {
        let rect = self.geometry();
        if !rect.contains(pos) || self.item_height <= 0.0 {
            return None;
        }
        let row = ((pos.y - rect.y) as f32 / self.item_height).floor();
        // A click in the widget's bottom padding, or above its top edge, addresses no
        // row. Without the lower-bound check a negative offset casts to a huge index.
        if !(0.0..).contains(&row) {
            return None;
        }
        let index = row as usize;
        if index < self.items.len() {
            Some(index)
        } else {
            None
        }
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
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `ListBox`'s property contract.
///
/// `selection_mode` publishes the shared lower-case tokens (`single`, `multi`, …)
/// rather than the type's `Debug` spelling, because that is what the previous
/// reader produced and what `expect_list_box_selection_mode` accepts.
impl WidgetProperties for ListBox {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "item_count" => Ok(CapabilityValue::UInt(self.count() as u64)),
            "selection_mode" => Ok(CapabilityValue::String(
                list_box_selection_mode_to_str(self.selection_mode()).to_string(),
            )),
            "current_row" => match self.current_row() {
                Some(row) => Ok(CapabilityValue::UInt(row as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "item_height" => Ok(CapabilityValue::Float(self.item_height() as f64)),
            "selected_count" => Ok(CapabilityValue::UInt(self.selected_indices().len() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "selection_mode" => {
                self.set_selection_mode(expect_list_box_selection_mode(value)?);
                Ok(())
            }
            "current_row" => {
                match value {
                    CapabilityValue::Null => self.set_current_row(None),
                    other => self.set_current_row(Some(expect_usize(other)?)),
                }
                Ok(())
            }
            "item_height" => {
                self.set_item_height(expect_f32(value)?);
                Ok(())
            }
            // `item_count` and `selected_count` are derived from the item list.
            "item_count" | "selected_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `LIST_BOX_PROPERTIES`.
        property_names_of![
            "item_count",
            "selection_mode",
            "current_row",
            "item_height",
            "selected_count",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `list_box` publishes.
    ///
    /// `clear` and `clear_selection` are payload-free and operate on live state,
    /// so they execute here. `add_item` / `remove_item` need text or an index and
    /// `set_selection_mode` needs a mode; those are answered through the property
    /// route, so a payload-less call reports `OutOfRange` as elsewhere.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "clear" => {
                self.clear();
                Ok(())
            }
            "clear_selection" => {
                self.clear_selection();
                Ok(())
            }
            "add_item" | "remove_item" | "set_selection_mode" => {
                Err(CapabilityAccessError::OutOfRange)
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
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

    /// A list box holding `count` items named `A`, `B`, …
    fn listbox_with(count: usize) -> ListBox {
        let mut lb = ListBox::new(Rect::new(0, 0, 200, 200));
        for index in 0..count {
            lb.add_item(format!("Item {index}"));
        }
        lb
    }

    #[test]
    fn listbox_creation_defaults() {
        let lb = ListBox::new(Rect::new(0, 0, 200, 200));
        assert!(lb.items().is_empty());
        assert!(lb.is_empty());
        assert_eq!(lb.count(), 0);
        assert_eq!(lb.current_row(), None);
        assert!(lb.selected_indices().is_empty());
        assert_eq!(lb.selection_mode(), SelectionMode::Single);
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
    fn listbox_set_current_row_requests_redraw_and_signal() {
        let mut lb = ListBox::new(Rect::new(0, 0, 200, 200));
        lb.add_items(vec!["A".to_string(), "B".to_string()]);

        let redraw = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        lb.base.redraw_requested.connect({
            let flag = std::sync::Arc::clone(&redraw);
            move || flag.store(true, std::sync::atomic::Ordering::SeqCst)
        });

        let selected = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(usize::MAX));
        lb.item_selected.connect({
            let flag = std::sync::Arc::clone(&selected);
            move |val| flag.store(*val, std::sync::atomic::Ordering::SeqCst)
        });

        lb.set_current_row(Some(1));

        assert_eq!(lb.current_row(), Some(1));
        assert!(redraw.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(selected.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn listbox_select() {
        let mut lb = ListBox::new(Rect::new(0, 0, 200, 200));
        lb.add_items(vec!["A".to_string(), "B".to_string(), "C".to_string()]);
        lb.select(1);
        assert!(lb.is_selected(1));
        assert_eq!(lb.selected_indices().len(), 1);
    }

    /// A click must select the row `draw` paints under the pointer.
    ///
    /// `draw` maps absolute item `i` to `y = i * item_height`, so scrolling pushes
    /// items off the top and the row index derived from a click is already the absolute
    /// item index. The hit-test added `scroll_offset` a second time, so every click on a
    /// scrolled list selected an item `scroll_offset` rows away from the one visible.
    #[test]
    fn listbox_click_selects_the_row_that_is_drawn_there() {
        let geometry = Rect::new(0, 0, 200, 100);
        let mut lb = ListBox::new(geometry);
        for i in 0..20 {
            lb.add_item(format!("item {i}"));
        }
        lb.set_item_height(20.0);
        lb.scroll(3);

        // `draw` paints item 3 at y = 60 and item 6 at y = 120 (below the widget).
        let click = |y: i32| {
            let mut list = ListBox::new(geometry);
            for i in 0..20 {
                list.add_item(format!("item {i}"));
            }
            list.set_item_height(20.0);
            list.scroll(3);
            list.handle_event(&Event::MousePress { pos: Point::new(50, y), button: 1 });
            list.current_row()
        };

        assert_eq!(click(10), Some(0), "y=10 is row 0, painted as item 0");
        assert_eq!(click(30), Some(1), "y=30 is row 1, painted as item 1");
        assert_eq!(click(60), Some(3), "y=60 is row 3, painted as item 3");
        // Below the last drawn row: no selection rather than a wrapped index.
        assert_eq!(click(100), None);
        assert_eq!(click(120), None);
        // Above the widget's own top edge must not wrap through `as usize`.
        assert_eq!(click(-5), None);
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
        lb.set_selection_mode(SelectionMode::Multi);
        lb.select_all();
        assert_eq!(lb.selected_indices().len(), 3);
    }

    #[test]
    fn listbox_selection_mode() {
        let mut lb = ListBox::new(Rect::new(0, 0, 200, 200));
        assert_eq!(lb.selection_mode(), SelectionMode::Single);
        lb.set_selection_mode(SelectionMode::Multi);
        assert_eq!(lb.selection_mode(), SelectionMode::Multi);
        lb.set_selection_mode(SelectionMode::None);
        assert_eq!(lb.selection_mode(), SelectionMode::None);
        lb.set_selection_mode(SelectionMode::Extended);
        assert_eq!(lb.selection_mode(), SelectionMode::Extended);
        lb.set_selection_mode(SelectionMode::Single);
        assert_eq!(lb.selection_mode(), SelectionMode::Single);
    }

    /// `Extended` must behave like the Windows-explorer model, not like `Multi`.
    ///
    /// The two differ in what a **plain** selection does: `Multi` adds, `Extended`
    /// replaces. Before this was fixed `Extended` was a literal copy of `Multi` (the
    /// comment said "Similar to multi for now"), so the mode could not be told apart
    /// from `Multi` by any caller.
    #[test]
    fn listbox_extended_plain_selection_replaces_rather_than_adds() {
        let mut lb = listbox_with(5);
        lb.set_selection_mode(SelectionMode::Extended);
        lb.select(1);
        lb.select(3);
        assert_eq!(lb.selected_indices(), &[3], "a plain select replaces the selection");

        let mut multi = listbox_with(5);
        multi.set_selection_mode(SelectionMode::Multi);
        multi.select(1);
        multi.select(3);
        assert_eq!(multi.selected_indices(), &[1, 3], "Multi still adds");
    }

    /// Shift must select the inclusive range from the anchor, in either direction.
    #[test]
    fn listbox_extended_shift_selects_the_anchor_range() {
        use crate::shortcut::Modifiers;
        let mut lb = listbox_with(6);
        lb.set_selection_mode(SelectionMode::Extended);

        lb.select(2);
        assert_eq!(lb.selection_anchor(), Some(2));
        lb.select_with_modifiers(4, Modifiers::SHIFT);
        assert_eq!(lb.selected_indices(), &[2, 3, 4], "forward range from the anchor");

        // Extending backwards keeps the rows in ascending order and moves the current row.
        lb.select_with_modifiers(0, Modifiers::SHIFT);
        assert_eq!(lb.selected_indices(), &[0, 1, 2], "backward range from the anchor");
        assert_eq!(lb.current_row(), Some(0));
    }

    /// Ctrl must toggle one row without disturbing the rest, and re-arm the anchor.
    #[test]
    fn listbox_extended_ctrl_toggles_a_row() {
        use crate::shortcut::Modifiers;
        let mut lb = listbox_with(5);
        lb.set_selection_mode(SelectionMode::Extended);
        lb.select(0);
        lb.select_with_modifiers(2, Modifiers::CTRL);
        assert_eq!(lb.selected_indices(), &[0, 2], "ctrl adds without clearing");
        lb.select_with_modifiers(0, Modifiers::CTRL);
        assert_eq!(lb.selected_indices(), &[2], "ctrl on a selected row removes it");
        assert_eq!(
            lb.selection_anchor(),
            Some(0),
            "ctrl moves the anchor to the row it toggled, so a following shift extends from it"
        );
    }

    /// A row removed from underneath the anchor must not leave a stale anchor behind.
    #[test]
    fn listbox_insert_and_remove_keep_the_anchor_addressable() {
        use crate::shortcut::Modifiers;
        let mut lb = listbox_with(5);
        lb.set_selection_mode(SelectionMode::Extended);
        lb.select(2);

        lb.insert_item(0, "new".to_string());
        assert_eq!(lb.selection_anchor(), Some(3), "an insert before the anchor shifts it");
        lb.select_with_modifiers(4, Modifiers::SHIFT);
        assert_eq!(lb.selected_indices(), &[3, 4]);

        // Removing the anchored row drops the anchor: there is no row left to grow from.
        lb.select(0);
        assert_eq!(lb.selection_anchor(), Some(0));
        lb.remove_item(0);
        assert_eq!(lb.selection_anchor(), None);
    }

    /// The modifier entry point must be safe to call in any mode.
    #[test]
    fn listbox_select_with_modifiers_delegates_outside_extended_mode() {
        use crate::shortcut::Modifiers;
        let mut lb = listbox_with(4);
        lb.set_selection_mode(SelectionMode::Single);
        lb.select_with_modifiers(1, Modifiers::SHIFT);
        lb.select_with_modifiers(2, Modifiers::SHIFT);
        assert_eq!(lb.selected_indices(), &[2], "Single keeps its one-row rule");

        let mut none = listbox_with(4);
        none.set_selection_mode(SelectionMode::None);
        none.select_with_modifiers(1, Modifiers::CTRL);
        assert!(none.selected_indices().is_empty(), "None selects nothing");
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
