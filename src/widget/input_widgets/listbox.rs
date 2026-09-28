// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! List box widget.
use crate::compat::{String, ToString, Vec};
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};

use crate::widget::capability::coercion::{
    expect_f32, expect_horizontal_alignment, expect_list_box_selection_mode, expect_usize,
    horizontal_alignment_to_str,
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
    /// The row the pointer is currently over, or `None`.
    ///
    /// Transient pointer state, deliberately separate from [`Self::current_row`] (the committed
    /// selection the keyboard moves): the draw gives them different weights for exactly that
    /// reason, the same distinction `list_view` makes.
    hovered_row: Option<usize>,
    /// How each row's label is aligned within its row.
    ///
    /// Horizontal only: a row's label is vertically centred in its band by the row's own layout, so
    /// a `top`/`bottom` value would be one this control could never honour —
    /// [`crate::widget::capability::coercion::expect_horizontal_alignment`] refuses those. Defaults
    /// to left, so a caller that never asks behaves exactly as it did.
    alignment: crate::core::Alignment,
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
            hovered_row: None,
            alignment: crate::core::Alignment::Left,
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
    ///
    /// Repaints, because the mode change can also *clear* visible state: switching to [`SelectionMode::None`]
    /// empties the selection and the cursor row, and switching to `Single` can truncate a multi-row
    /// selection. Both are what `draw` paints, so without this the previously highlighted rows stayed
    /// highlighted until something unrelated repainted.
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
        self.base.request_redraw();
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
        self.base.request_redraw();
    }
    /// Returns all items.
    pub fn items(&self) -> &[String] {
        &self.items
    }

    /// The row the pointer is currently over, or `None`.
    pub fn hovered_row(&self) -> Option<usize> {
        self.hovered_row
    }

    /// How each row's label is aligned within its row.
    pub fn alignment(&self) -> crate::core::Alignment {
        self.alignment
    }

    /// Sets how each row's label is aligned within its row.
    ///
    /// Horizontal only. A `top`/`bottom` alignment is **ignored** because a row's label is centred
    /// vertically in its band by the row's own layout; the property route refuses those through
    /// [`crate::widget::capability::coercion::expect_horizontal_alignment`], and this setter
    /// matching that keeps the two entry points from disagreeing.
    pub fn set_alignment(&mut self, alignment: crate::core::Alignment) {
        if alignment.to_horizontal().is_none() || self.alignment == alignment {
            return;
        }
        self.alignment = alignment;
        self.base.request_redraw();
    }

    /// The inset between the control's border and the rows it draws.
    ///
    /// # Why the rows are inset at all
    ///
    /// The border stroke and a rounded `face` are drawn on the control's outer rectangle, and a
    /// row painted from `rect.y` covers both — the first row sat on the border and the rounded
    /// corners clipped nothing. One inset applied to every edge keeps the ink wholly inside the
    /// chrome, which is the "never paint outside your own frame" rule the other controls follow.
    const CONTENT_INSET: i32 = 1;

    /// The rectangle the rows occupy: the control's box, inset past its own border.
    fn content_rect(&self) -> Rect {
        let rect = self.geometry();
        let inset = Self::CONTENT_INSET.min(rect.width as i32 / 2).min(rect.height as i32 / 2);
        Rect::new(
            rect.x + inset,
            rect.y + inset,
            rect.width.saturating_sub(2 * inset as u32),
            rect.height.saturating_sub(2 * inset as u32),
        )
    }

    /// The row height actually used to draw and hit-test, floored so at least one row is legible.
    fn row_height(&self) -> f32 {
        self.item_height.max(1.0)
    }

    /// The font the rows are drawn with, **scaled to the row height** so a larger `item_height`
    /// gets larger text and a small one does not overflow the row.
    ///
    /// The scaling is a fraction of the row height rather than a fixed point size, so a control
    /// resized or given a different `item_height` keeps its text in proportion — which is what
    /// "auto-scaling" means for a list whose row height is settable. The fraction leaves a small
    /// vertical margin so a descender is not clipped at the row's own edge.
    fn row_font(&self) -> Font {
        let size = (self.row_height() * 0.7).clamp(8.0, 32.0);
        Font::new("sans-serif", size, false, false)
    }

    /// The band item `index` occupies, in device space, or `None` when it is scrolled out.
    ///
    /// # One derivation, three consumers
    ///
    /// `draw`, [`Self::item_index_at_y`] and the hover test all read this, so the row a click or a
    /// hover resolves to is the row that was painted. Before this they each recomputed
    /// `rect.y + i * item_height` and the origin they measured from had already drifted once (a
    /// press selected the row `scroll_offset` below the one under the cursor).
    fn row_rect(&self, index: usize) -> Option<Rect> {
        let content = self.content_rect();
        let height = self.row_height();
        let y = content.y as f32 + (index as f32 - self.scroll_offset as f32) * height;
        let row = Rect::from_f32(content.x as f32, y, content.width as f32, height);
        // A row wholly outside the content box is not visible at all rather than drawn clipped to
        // a sliver: the same "refuse rather than truncate" rule the sibling list controls use.
        if row.y + row.height as i32 <= content.y || row.y >= content.y + content.height as i32 {
            return None;
        }
        Some(row)
    }

    /// Returns the absolute index of the item drawn at widget-relative `y`.
    ///
    /// The inverse of [`Self::row_rect`], deliberately built on it: a point is a row's when it is
    /// inside that row's band, so the two directions cannot disagree about where a row begins.
    fn item_index_at_y(&self, pos: Point) -> Option<usize> {
        let content = self.content_rect();
        if !content.contains(pos) || self.row_height() <= 0.0 {
            return None;
        }
        let height = self.row_height();
        let row = ((pos.y - content.y) as f32 / height).floor() + self.scroll_offset as f32;
        if !(0.0..).contains(&row) {
            return None;
        }
        let index = row as usize;
        (index < self.items.len()).then_some(index)
    }

    /// Returns visible item range based on scroll position.
    fn visible_range(&self) -> (usize, usize) {
        let content = self.content_rect();
        let height = self.row_height();
        let visible_items = (content.height as f32 / height).ceil().max(1.0) as usize;
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
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
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
            // The selected rows as a comma-joined list of indices, in visual order.
            "selected_indices" => Ok(CapabilityValue::String(
                self.selected_indices()
                    .iter()
                    .map(|index| index.to_string())
                    .collect::<Vec<_>>()
                    .join(","),
            )),
            "hovered_row" => match self.hovered_row() {
                Some(row) => Ok(CapabilityValue::UInt(row as u64)),
                None => Ok(CapabilityValue::Null),
            },
            // The size the rows draw at, which scales with `item_height`; reporting the value the
            // draw uses keeps the contract and the picture from disagreeing.
            "font_size" => Ok(CapabilityValue::Float(f64::from(self.row_font().size()))),
            "alignment" => Ok(CapabilityValue::String(
                horizontal_alignment_to_str(self.alignment()).to_string(),
            )),
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
            "alignment" => {
                self.set_alignment(expect_horizontal_alignment(value)?);
                Ok(())
            }
            // `item_count`, `selected_count`, `selected_indices`, `hovered_row` and `font_size`
            // are derived from the item list, the selection or the row height.
            "item_count" | "selected_count" | "selected_indices" | "hovered_row" | "font_size" => {
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
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
            "selected_indices",
            "hovered_row",
            "font_size",
            "alignment",
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
            Event::MouseMove { pos } => {
                // Hover is the row the pointer is over, from the same `row_rect` the paint and the
                // press use, so a highlight cannot land on a different row than a click would.
                // `MouseMove` fires far more often than the highlight changes, so the redraw is
                // requested only on an actual change.
                let hovered = self.item_index_at_y(*pos);
                if hovered != self.hovered_row {
                    self.hovered_row = hovered;
                    self.base.request_redraw();
                }
            }
            Event::MouseLeave { .. } => {
                if self.hovered_row.take().is_some() {
                    self.base.request_redraw();
                }
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
        let rect = self.geometry();
        // Chrome resolved style-first, then the theme role for this control, then a literal. The
        // theme step is what makes an appearance switch visible; the previous form hardcoded the
        // selection blue, the current-row fill and the separator, so the control was theme-blind.
        let style = self.style().clone();
        let theme = crate::style::resolved_theme_style("list_box");
        let surface = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        // A border that would coincide with the surface is a border nobody can see, so it steps
        // off the surface instead of being painted invisible.
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != surface)
            .unwrap_or_else(|| surface.blend(&ink, 0.20));
        // A selected row is a committed selection, so it reads the theme's accent token rather
        // than a second literal blue; a focused row is the same accent at a lighter weight, and a
        // hovered row lighter still — a pointer position rather than a selection, so it must read
        // as "a click would land here" without competing with the row actually chosen.
        let accent = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.primary)
            .unwrap_or(Color::PRIMARY);
        let selected_bg = surface.blend(&accent, 0.85);
        let focused_bg = surface.blend(&accent, 0.30);
        let hovered_bg = surface.blend(&accent, 0.12);
        let separator = surface.blend(&ink, 0.14);

        context.face(
            rect,
            surface,
            style.surface.unwrap_or_default(),
            style.border_radius.unwrap_or(0),
            Color::BLACK,
        );
        // The border is drawn on the control's outer rectangle, so the rows must not paint over it
        // — `row_rect` insets them by `CONTENT_INSET` inside this stroke.
        context.draw_rect(rect, border);

        let font = self.row_font();
        let text_inset = 6;
        // Read once so the loop body only compares integers.
        let current_row = self.current_row;
        let hovered_row = self.hovered_row;
        for i in self.visible_range().0..self.visible_range().1 {
            let Some(row) = self.row_rect(i) else { continue };
            // Hover is painted *under* focus and selection: a row that is both hovered and chosen
            // keeps the stronger weight, so the persistent fact is not displaced by the pointer.
            let fill = if self.is_selected(i) {
                Some(selected_bg)
            } else if Some(i) == current_row {
                Some(focused_bg)
            } else if Some(i) == hovered_row {
                Some(hovered_bg)
            } else {
                None
            };
            if let Some(fill) = fill {
                context.fill_rect(row, fill);
            }
            if let Some(text) = self.item(i) {
                if !text.is_empty() {
                    // The label's colour is taken from the row's own fill so the two cannot
                    // collide when a theme changes hue, and falls back to the control's ink on a
                    // row with no highlight.
                    let label_ink = fill.map_or(ink, |fill| fill.contrast_color());
                    // The line box is measured from the same font the ink is drawn with and centred
                    // in the row, so the glyph box's top edge lands where the text renders. The old
                    // `row.y + item_height / 2` put that edge on the row's middle line, half a line
                    // low, and ignored the font entirely.
                    let line = context.text_line(row, &font);
                    // Fitted into the row's inner width, so an item longer than the row is elided
                    // rather than drawn across the border and out of the control. The alignment is
                    // the control's own; a right-aligned row keeps the same inner inset on both
                    // sides, so the elision bound and the anchor move together.
                    let label_align =
                        self.alignment.to_horizontal().unwrap_or(HorizontalAlignment::Left);
                    context.draw_text_fitted(
                        Rect::new(
                            row.x + text_inset,
                            line.y,
                            row.width.saturating_sub(text_inset as u32 * 2),
                            line.height,
                        ),
                        text,
                        &font,
                        label_ink,
                        label_align,
                    );
                }
            }
            // A separator only belongs *between* rows, so it is skipped on the last visible one.
            if i + 1 < self.visible_range().1 {
                let sep_y = row.y + row.height as i32;
                context.draw_line(
                    Point::new(row.x, sep_y),
                    Point::new(row.x + row.width as i32, sep_y),
                    separator,
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
    /// The assertion drives the test off [`ListBox::row_rect`] — the same derivation `draw` uses —
    /// rather than off hand-computed `y` values. A row's `y` therefore follows whatever the real
    /// content inset and scroll are, so the test cannot rot when those change (it did: it used to
    /// hard-code the pre-inset geometry and assert a mapping the paint loop no longer performed).
    #[test]
    fn listbox_click_selects_the_row_that_is_drawn_there() {
        let geometry = Rect::new(0, 0, 200, 100);
        let build = || {
            let mut lb = ListBox::new(geometry);
            for i in 0..20 {
                lb.add_item(format!("item {i}"));
            }
            lb.set_item_height(20.0);
            lb.scroll(3);
            lb
        };

        let click = |y: i32| {
            let mut list = build();
            list.handle_event(&Event::MousePress { pos: Point::new(50, y), button: 1 });
            list.current_row()
        };

        // Every visible row resolves to its own index when clicked at its centre — the row the
        // press lands on is the row that was painted.
        let probe = build();
        for index in probe.visible_range().0..probe.visible_range().1 {
            let row = probe.row_rect(index).expect("a visible index has a row");
            let mid = row.y + row.height as i32 / 2;
            assert_eq!(click(mid), Some(index), "the centre of painted row {index} selects it");
        }
        // Below the content box, above the control, and below the last drawn row: no selection
        // rather than a wrapped or off-by-scroll index.
        assert_eq!(click(geometry.height as i32 + 5), None);
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
