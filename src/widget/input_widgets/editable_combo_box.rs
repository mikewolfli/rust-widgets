// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! EditableComboBox widget — a combo box that allows typing custom values.
//!
//! The EditableComboBox combines a text field with a dropdown list. Users can
//! either type a custom value directly or select from the provided items.
//! A `text_changed` signal is emitted when the text field content changes,
//! and an `item_selected` signal is emitted when a dropdown item is clicked.
//!
//! # Why the indicator drives the value's right inset
//!
//! The indicator is the control's trailing chrome, and the value's box must *yield* to it:
//! The shared relation writes this as `rightPadding: padding + indicator.width`, and the
//! reason is that a fixed text inset and a fixed indicator inset are two unrelated
//! derivations from the *same* edge. A wider indicator, a theme that swaps the glyph for a
//! larger one, or a bigger font measuring it then overlaps the value instead of pushing it.
//! The previous form could not express the relation at all — the value was written at
//! `rect.x + 6` with no bound and the indicator placed independently at
//! `rect.x + rect.width - 20`.

use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler, WheelScroll};
use crate::render::RenderContext;
use crate::shortcut::Key;
use crate::signal::{GenericSignal, Signal1};
use crate::style::EdgeOffsets;
use crate::undo::{UndoStack, ValueCommand};
use crate::widget::capability::coercion::{
    expect_horizontal_alignment, expect_string, expect_usize, horizontal_alignment_to_str,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::input_widgets::shortcut_editor::primary_shortcut_pressed;
use crate::widget::input_widgets::suggestion_popup::{MatchMode, SuggestionPopup};
use crate::widget::metrics::{dimensions, effective_font, estimate_text_width, ControlMetrics};
use crate::widget::{BaseWidget, Draw, IconName, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::cell::RefCell;
use std::rc::Rc;

/// Space between the indicator's leading edge and the end of the value's box.
///
/// Half the field's own horizontal padding, so the gap between the value and the indicator
/// is tighter than the gap between the value and the field's edge — a relation, not a third
/// independent numeral.
const INDICATOR_LEADING_GAP: u32 = dimensions::TEXT_FIELD_PADDING_H / 2;

/// Height of one painted row in the open list.
///
/// The list's row height is a fact the paint, the hit test and the popup engine all read, so it
/// is named once instead of repeated as a literal in each of them.
const EDITABLE_ROW_HEIGHT: u32 = 28;

/// The indicator's box and the box the value may occupy, derived together.
///
/// # Why one derivation and not two arithmetic expressions
///
/// The value's inset and the indicator's position were two independent computations from the
/// same field width, so neither could be read from a test and neither constrained the other.
/// Making both outputs of one function is what lets the suite assert that the value yields to
/// the indicator rather than merely observing that it usually appears to.
#[derive(Debug, Clone, Copy, PartialEq)]
struct EditGeometry {
    /// The indicator glyph's bounding box.
    box_rect: Rect,
    /// The rectangle the typed value may occupy.
    text_box: Rect,
}

impl EditGeometry {
    /// Derives both boxes from the band the control paints and `line_height`.
    ///
    /// `line_height` is a parameter rather than measured here, for the same reason
    /// `CheckBox::indicator_rect` takes one: both boxes sit on the *value's* line box and only
    /// the caller knows which font that is.
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
        // The value's box stops one gap before the indicator. `min` with the leading inset
        // keeps a squeezed field from describing an inverted rectangle: a field with no room
        // for a value draws no value rather than one outside itself.
        let text_right = box_rect.x.saturating_sub(INDICATOR_LEADING_GAP as i32);
        let text_left = (band.x + dimensions::TEXT_FIELD_PADDING_H as i32).min(text_right);
        let text_box =
            Rect::new(text_left, band.y, text_right.saturating_sub(text_left) as u32, band.height);
        Self { box_rect, text_box }
    }
}

/// A combo box that allows both typing custom values and selecting from a list.
///
/// When collapsed, shows the current text in a text field with a dropdown arrow.
/// When expanded, shows a dropdown list of items below the text field.
pub struct EditableComboBox {
    base: BaseWidget,
    text: String,
    items: Vec<String>,
    expanded: bool,
    selected_index: Option<usize>,
    /// How the field's value is aligned within its own box.
    ///
    /// Horizontal only: the value is centred vertically in the field, so a `top`/`bottom` value
    /// would be one this control could never honour —
    /// [`crate::widget::capability::coercion::expect_horizontal_alignment`] refuses those rather
    /// than accepting a write that does nothing. Defaults to left.
    alignment: crate::core::Alignment,
    /// Emitted when the text field content changes.
    pub text_changed: Signal1<String>,
    /// Emitted when a dropdown item is selected (by index).
    pub item_selected: Signal1<usize>,
    /// Emitted when editing ends — Enter is pressed or the field loses focus.
    ///
    /// Carries no payload because the committed value is read back through [`Self::text`];
    /// this is the notification a form waits for before validating, and the counterpart of the
    /// same signal on [`crate::widget::LineEdit`].
    pub editing_finished: GenericSignal,
    /// Emitted when Enter is pressed in the field, after any highlighted row is selected.
    ///
    /// A host that treats Enter as "go" must not have to infer it from `item_selected`: Enter
    /// on a free-typed value selects nothing and would otherwise be silent.
    pub return_pressed: GenericSignal,
    undo_stack: UndoStack,
    history_target: Rc<RefCell<String>>,
    restoring_history: bool,
    /// The insertion point, in bytes from the start of the value.
    cursor: usize,
    /// The other end of a selection, in bytes. `None` means no selection.
    selection_anchor: Option<usize>,
    /// The caret's blink state, shared with the crate's other carets.
    cursor_blink: crate::style::CursorBlink,
    /// Maximum rows the open list shows at once. `None` shows them all.
    max_visible: Option<usize>,
    /// The candidate-list engine the open list is painted from (I-129).
    ///
    /// The popup filters the items, owns the painted window, resolves the row under the pointer
    /// and holds the highlight. The control keeps its own item storage and paints the rows, so
    /// `items` and every public accessor are unchanged.
    popup: SuggestionPopup,
    /// Bumped whenever `items` changes; the popup's row list is rebuilt only when it moves.
    items_revision: u64,
    /// The item revision the popup's rows currently reflect.
    popup_synced_revision: u64,
}

impl EditableComboBox {
    /// The band the control actually paints: full width, one field tall, centred.
    ///
    /// # Why the control is not its own rectangle
    ///
    /// An editable combo box is a text field, and [`dimensions::TEXT_FIELD_MIN_HEIGHT`] is
    /// what every field in this crate occupies. The 240x120 census cell drew a 240x120
    /// rounded box whose value was then positioned `padding + 13` down from an edge that was
    /// itself not the field's.
    fn field_band(&self) -> Rect {
        ControlMetrics::full_width_band(self.geometry(), dimensions::TEXT_FIELD_MIN_HEIGHT)
    }

    /// The indicator's box and the value's box, derived from the band and one line height.
    fn indicator_geometry(&self, line_height: u32) -> EditGeometry {
        EditGeometry::for_band(self.field_band(), line_height)
    }

    /// Creates a new EditableComboBox widget with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        let mut combo = Self {
            base: BaseWidget::new(WidgetKind::EditableComboBox, geometry, "EditableComboBox"),
            text: String::new(),
            items: Vec::new(),
            expanded: false,
            selected_index: None,
            alignment: crate::core::Alignment::Left,
            text_changed: Signal1::new(),
            item_selected: Signal1::new(),
            editing_finished: GenericSignal::new(),
            return_pressed: GenericSignal::new(),
            undo_stack: UndoStack::new(),
            history_target: Rc::new(RefCell::new(String::new())),
            restoring_history: false,
            cursor: 0,
            selection_anchor: None,
            cursor_blink: crate::style::CursorBlink::new(),
            max_visible: None,
            popup: SuggestionPopup::new(geometry),
            items_revision: 0,
            popup_synced_revision: u64::MAX,
        };
        combo.sync_popup();
        combo
    }

    /// The rectangle the open list hangs from: the field band's bottom edge.
    fn popup_anchor(&self) -> Rect {
        let band = self.field_band();
        Rect::new(band.x, band.y + band.height as i32, band.width, 0)
    }

    /// Brings the popup engine's rows, filter, metrics and anchor up to date.
    ///
    /// Every read of the open list (paint, hit test, scroll) goes through the engine, so the
    /// filter is applied here rather than at each call site. Row rebuilding is keyed on the item
    /// revision, and the other setters are guarded internally, so a repaint is not a list copy.
    fn sync_popup(&mut self) {
        let offset = self.popup.scroll_offset();
        let max = self.max_visible.unwrap_or_else(|| self.items.len().max(1));
        if self.popup.max_visible() != max {
            self.popup.set_max_visible(max);
        }
        if self.popup.row_height() != EDITABLE_ROW_HEIGHT {
            self.popup.set_row_height(EDITABLE_ROW_HEIGHT);
        }
        self.popup.set_match_mode(MatchMode::Prefix);
        self.popup.set_anchor(self.popup_anchor());
        if self.items_revision != self.popup_synced_revision {
            self.popup.set_items(self.items.clone());
            self.popup_synced_revision = self.items_revision;
        }
        self.popup.apply_filter(self.text.trim());
        let row = self
            .selected_index
            .and_then(|index| self.popup.visible_indices().iter().position(|&c| c == index));
        self.popup.set_selected_index(row);
        // The scroll position is a view fact, not a selection fact: a text change that narrows
        // the candidate list clamps it, but does not re-centre the window on the selection.
        self.popup.set_scroll_offset(offset);
    }

    /// Returns the current text in the text field.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// How the field's value is aligned within its own box.
    pub fn alignment(&self) -> crate::core::Alignment {
        self.alignment
    }

    /// Sets how the field's value is aligned within its own box.
    ///
    /// Horizontal only. A `top`/`bottom` alignment is **ignored**, because the value is centred
    /// vertically in the field by the field's own layout — the property route refuses it through
    /// [`crate::widget::capability::coercion::expect_horizontal_alignment`], and this setter
    /// matching that keeps the two entry points from disagreeing.
    pub fn set_alignment(&mut self, alignment: crate::core::Alignment) {
        if alignment.to_horizontal().is_none() || self.alignment == alignment {
            return;
        }
        self.alignment = alignment;
        self.base.request_redraw();
    }

    /// Sets the text in the text field and emits `text_changed`.
    ///
    /// The new value is moved in rather than cloned for every consumer, and the pre-edit value
    /// is captured once for the undo record (I-P02).
    pub fn set_text(&mut self, text: impl Into<String>) {
        let t = text.into();
        if self.text != t {
            let before = core::mem::replace(&mut self.text, t);
            self.cursor = self.text.len();
            self.selection_anchor = None;
            self.publish_text_change(before);
        }
    }

    /// Publishes a value mutation the caller already applied to `self.text`.
    ///
    /// `before` is the pre-edit value, moved in so the history record does not need another
    /// copy of the string (I-P02). The caller owns the caret because each edit path places it
    /// differently (an append advances by the inserted length, a delete parks at the gap).
    fn publish_text_change(&mut self, before: String) {
        if !self.restoring_history {
            *self.history_target.borrow_mut() = self.text.clone();
            self.undo_stack.push(Box::new(ValueCommand::new(
                self.history_target.clone(),
                before,
                self.text.clone(),
                "Edit text",
                "editable_combo_text",
            )));
        }
        self.text_changed.emit(self.text.clone());
        self.sync_popup();
        self.base.request_redraw();
    }

    /// The caret's byte offset in the value.
    pub fn cursor_position(&self) -> usize {
        self.cursor
    }

    /// Moves the caret, clearing the selection.
    pub fn set_cursor_position(&mut self, position: usize) {
        let position = crate::widget::text_utils::floor_char_boundary(
            &self.text,
            position.min(self.text.len()),
        );
        if self.cursor == position && self.selection_anchor.is_none() {
            return;
        }
        self.cursor = position;
        self.selection_anchor = None;
        self.base.request_redraw();
    }

    /// The selected byte range, or `None` when nothing is selected.
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.selection_anchor?;
        let (start, end) =
            if anchor <= self.cursor { (anchor, self.cursor) } else { (self.cursor, anchor) };
        (start != end).then_some((start, end))
    }

    /// The maximum number of rows the open list shows at once.
    pub fn max_visible_items(&self) -> Option<usize> {
        self.max_visible
    }

    /// Caps the number of rows the open list paints.
    pub fn set_max_visible_items(&mut self, max_visible: Option<usize>) {
        let max_visible = max_visible.map(|value| value.max(1));
        if self.max_visible == max_visible {
            return;
        }
        self.max_visible = max_visible;
        self.sync_popup();
        self.base.request_redraw();
    }

    /// Index of the first painted row in the open list.
    pub fn scroll_offset(&self) -> usize {
        self.popup.scroll_offset()
    }

    /// Sets the first painted row, clamped to the rows that exist.
    pub fn set_scroll_offset(&mut self, offset: usize) {
        self.sync_popup();
        self.popup.set_scroll_offset(offset);
        self.base.request_redraw();
    }

    /// Number of rows painted at once for a list with `rows` rows.
    pub fn page_size(&self, rows: usize) -> usize {
        self.max_visible.unwrap_or(rows).max(1)
    }

    /// Items matching the typed prefix, in list order.
    ///
    /// An empty value matches everything, so opening the list with no text shows the whole
    /// collection. Matching is case-insensitive and prefix-anchored: a substring match would
    /// make typing `ap` list every item whose label merely contains it.
    pub fn filtered_items(&self) -> Vec<String> {
        // Derived from the one filter path below, so the clone-returning and
        // index-returning views cannot disagree (I-103).
        self.filtered_indices()
            .into_iter()
            .filter_map(|index| self.items.get(index).cloned())
            .collect()
    }

    /// Indices in `items` matching the typed prefix.
    ///
    /// The filter and the painted window are the shared [`SuggestionPopup`]'s (I-129), so the
    /// typed text is folded once there and this is a view over the rows it kept. The draw path,
    /// the hit test and the keyboard navigation all read this one list, so rows are never cloned
    /// to be identified.
    fn filtered_indices(&self) -> Vec<usize> {
        self.popup.visible_indices().to_vec()
    }

    /// Moves the caret by `delta` characters.
    fn move_caret(&mut self, delta: i32, extend: bool) {
        let mut target = self.cursor;
        if delta < 0 {
            for _ in 0..delta.unsigned_abs() {
                target = crate::widget::text_utils::previous_char_boundary(&self.text, target);
            }
        } else {
            for _ in 0..delta.unsigned_abs() {
                target = crate::widget::text_utils::next_char_boundary(&self.text, target);
            }
        }
        target = crate::widget::text_utils::floor_char_boundary(&self.text, target);
        if extend {
            if self.selection_anchor.is_none() {
                self.selection_anchor = Some(self.cursor);
            }
        } else {
            self.selection_anchor = None;
        }
        if self.cursor != target {
            self.cursor = target;
            self.base.request_redraw();
        }
    }

    /// Removes the selection, or the character before the caret when there is none.
    fn backspace(&mut self) {
        let (start, end) = self.selection_range().unwrap_or_else(|| {
            (
                crate::widget::text_utils::previous_char_boundary(&self.text, self.cursor),
                self.cursor,
            )
        });
        if start == end {
            return;
        }
        let before = self.text.clone();
        self.text.replace_range(start..end, "");
        self.cursor = start;
        self.selection_anchor = None;
        self.publish_text_change(before);
    }

    /// Removes the selection, or the character after the caret when there is none.
    fn delete_forward(&mut self) {
        let (start, end) = self.selection_range().unwrap_or_else(|| {
            (self.cursor, crate::widget::text_utils::next_char_boundary(&self.text, self.cursor))
        });
        if start == end {
            return;
        }
        let before = self.text.clone();
        self.text.replace_range(start..end, "");
        self.cursor = start;
        self.selection_anchor = None;
        self.publish_text_change(before);
    }

    /// Scrolls the list so the selected row is inside the window.
    fn ensure_selected_visible(&mut self) {
        self.sync_popup();
        let Some(index) = self.selected_index else {
            return;
        };
        let Some(position) =
            self.popup.visible_indices().iter().position(|&candidate| candidate == index)
        else {
            return;
        };
        self.popup.scroll_into_view(position);
        self.base.request_redraw();
    }

    /// Appends committed text to the field and emits `text_changed` (D09-INPUT-01).
    ///
    /// The desktop backends deliver a printable character or an IME commit as
    /// `Event::TextInput`/`Event::ImeCommit`, not as a `KeyPress`. This is the one append entry
    /// point shared by the printable key arm and the committed-text branch, so the undo push and
    /// the `text_changed` signal behave identically for both. Control characters are filtered so a
    /// stray one does not enter the value.
    fn append_committed_text(&mut self, text: &str) {
        let filtered: String = text.chars().filter(|c| !c.is_control()).collect();
        if filtered.is_empty() {
            return;
        }
        let (start, end) = self.selection_range().unwrap_or((self.cursor, self.cursor));
        // In place, with the pre-edit value captured for the undo record only (I-P02).
        let before = self.text.clone();
        self.text.replace_range(start..end, &filtered);
        self.cursor = start + filtered.len();
        self.selection_anchor = None;
        self.publish_text_change(before);
    }

    /// Adds an item to the dropdown list.
    pub fn add_item(&mut self, item: impl Into<String>) {
        self.items.push(item.into());
        self.items_revision = self.items_revision.wrapping_add(1);
        self.sync_popup();
        self.base.request_redraw();
    }

    /// Removes an item by index. Returns `true` if the item was removed.
    pub fn remove_item(&mut self, index: usize) -> bool {
        if index < self.items.len() {
            self.items.remove(index);
            if self.selected_index == Some(index) {
                self.selected_index = None;
            } else if let Some(ref mut sel) = self.selected_index {
                if index < *sel {
                    *sel -= 1;
                }
            }
            self.items_revision = self.items_revision.wrapping_add(1);
            self.sync_popup();
            self.base.request_redraw();
            true
        } else {
            false
        }
    }

    /// Removes all items from the list and closes the dropdown.
    pub fn clear_items(&mut self) {
        self.items.clear();
        self.selected_index = None;
        self.items_revision = self.items_revision.wrapping_add(1);
        self.sync_popup();
        self.collapse();
        self.base.request_redraw();
    }

    /// Returns a reference to the list of items.
    pub fn items(&self) -> &[String] {
        &self.items
    }

    /// Returns the number of items in the dropdown.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Returns whether the dropdown is currently expanded.
    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    /// Expands the dropdown list.
    pub fn expand(&mut self) {
        if !self.expanded {
            self.expanded = true;
            self.sync_popup();
            // The open list owns the press that dismisses it, matching the combo box's
            // popup-grab contract (I-127).
            #[cfg(not(alloc_frugal))]
            crate::widget::runtime::set_popup_grab(self.base.id());
            self.base.request_redraw();
        }
    }

    /// Collapses the dropdown list.
    pub fn collapse(&mut self) {
        if self.expanded {
            self.expanded = false;
            #[cfg(not(alloc_frugal))]
            crate::widget::runtime::clear_popup_grab_for(self.base.id());
            self.base.request_redraw();
        }
    }

    /// Toggles the dropdown between expanded and collapsed.
    pub fn toggle(&mut self) {
        if self.expanded {
            self.collapse();
        } else {
            self.expand();
        }
    }

    /// Returns the currently selected item index, if any.
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    /// Steps back one text edit and returns `true`, or `false` when there is
    /// nothing to undo.
    ///
    /// The restored text is applied through [`EditableComboBox::set_text`], so
    /// it runs the full validation and signal path. It also clears the selection:
    /// undoing a text change does not restore the previously selected item, and
    /// `selected_index` becomes `None` even though the text may match an item.
    pub fn undo(&mut self) -> bool {
        if self.undo_stack.undo().is_err() {
            return false;
        }
        self.restoring_history = true;
        let text = self.history_target.borrow().clone();
        self.set_text(&text);
        self.restoring_history = false;
        true
    }
    /// Steps forward one undone edit and returns `true`, or `false` when there is
    /// nothing to redo. Selection behaviour matches
    /// [`EditableComboBox::undo`].
    pub fn redo(&mut self) -> bool {
        if self.undo_stack.redo().is_err() {
            return false;
        }
        self.restoring_history = true;
        let text = self.history_target.borrow().clone();
        self.set_text(&text);
        self.restoring_history = false;
        true
    }
    /// Returns `true` if [`EditableComboBox::undo`] would change the text.
    pub fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }
    /// Returns `true` if [`EditableComboBox::redo`] would change the text.
    pub fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }

    /// Selects the item at the given index and sets the text field to its value.
    /// Emits `item_selected`. Returns `true` if the selection changed.
    pub fn select_index(&mut self, index: usize) -> bool {
        if index < self.items.len() && self.selected_index != Some(index) {
            self.selected_index = Some(index);
            self.text = self.items[index].clone();
            self.text_changed.emit(self.text.clone());
            self.item_selected.emit(index);
            self.expanded = false;
            self.sync_popup();
            self.base.request_redraw();
            return true;
        }
        false
    }
}

impl Widget for EditableComboBox {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    /// Keeps the popup's anchor on the field when the control is laid out again.
    fn set_geometry(&mut self, geometry: Rect) {
        self.base.set_geometry(geometry);
        self.sync_popup();
    }

    fn size_hint(&self) -> crate::core::Size {
        // Derived through the same metric the band is drawn from, so the reported size and the
        // painted box cannot describe two different controls. The width was a bare `200` while
        // `draw` painted the whole given rectangle — neither had a relation to the other.
        let side_air = (dimensions::TEXT_FIELD_MIN_HEIGHT / 2).saturating_sub(8);
        let trailing =
            dimensions::TEXT_FIELD_PADDING_H + dimensions::BUTTON_ICON_SIZE + INDICATOR_LEADING_GAP;
        let padding = EdgeOffsets {
            top: side_air,
            right: trailing,
            bottom: side_air,
            left: dimensions::TEXT_FIELD_PADDING_H,
        };
        // Content width: the text the caller typed, through the shared estimate. A hint has no
        // `RenderContext` to measure with, and a hint a few pixels generous is the safe direction
        // — the paint path *fits* the value into the box this produced.
        //
        // It used to be `text.len() as u32 * 8`, which counts UTF-8 **bytes**. "A few pixels
        // generous" is true of the flat per-cluster model and false of a byte count: a CJK
        // summary measured three times its drawn width, so the field reserved room it would never
        // use and pushed its neighbours around. `estimate_text_width` is still context-free, so the
        // original reasoning holds — it is simply a per-*cluster* nominal advance rather than a
        // per-byte one.
        let content =
            Size::new(estimate_text_width(&self.text, effective_font(self.style()), 1.0), 0);
        let floor = Size::new(
            dimensions::TEXT_FIELD_MIN_HEIGHT + trailing,
            dimensions::TEXT_FIELD_MIN_HEIGHT,
        );
        let hint = ControlMetrics::implicit_size(content, padding, floor);
        Size::new(hint.width, self.field_band().height.max(floor.height))
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually **emits**. Without an
    /// arm a published name is valid and inert, which is the silent failure
    /// `tools/check_event_signal_dyn.sh` exists to make impossible: the arm set is compared against
    /// the capability's published names, so the two cannot drift.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "text_changed" => {
                Some(EventSignalRef::mapped("text_changed", &self.text_changed, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "item_selected" => {
                Some(EventSignalRef::mapped("item_selected", &self.item_selected, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            "editing_finished" => {
                Some(EventSignalRef::unit("editing_finished", &self.editing_finished))
            }
            "return_pressed" => Some(EventSignalRef::unit("return_pressed", &self.return_pressed)),
            _ => None,
        }
    }
}

/// `EditableComboBox`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_input.in.rs` / `access_write_input.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `item_count` is derived from
/// the dropdown list, so it is refused as read-only rather than reported as a name
/// this control does not know.
impl WidgetProperties for EditableComboBox {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "item_count" => Ok(CapabilityValue::UInt(self.item_count() as u64)),
            "alignment" => Ok(CapabilityValue::String(
                horizontal_alignment_to_str(self.alignment()).to_string(),
            )),
            "cursor_position" => Ok(CapabilityValue::UInt(self.cursor_position() as u64)),
            "max_visible_items" => match self.max_visible_items() {
                Some(max) => Ok(CapabilityValue::UInt(max as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "scroll_offset" => Ok(CapabilityValue::UInt(self.scroll_offset() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(expect_string(value)?);
                Ok(())
            }
            "item_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            "alignment" => {
                self.set_alignment(expect_horizontal_alignment(value)?);
                Ok(())
            }
            "cursor_position" => {
                self.set_cursor_position(expect_usize(value)?);
                Ok(())
            }
            "max_visible_items" => {
                match value {
                    CapabilityValue::Null => self.set_max_visible_items(None),
                    other => self.set_max_visible_items(Some(expect_usize(other)?)),
                }
                Ok(())
            }
            "scroll_offset" => {
                self.set_scroll_offset(expect_usize(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "text",
            "item_count",
            "alignment",
            "cursor_position",
            "max_visible_items",
            "scroll_offset",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `editable_combo_box` publishes.
    ///
    /// `set_text` assigns the edit contents and needs a payload, so it is answered
    /// through the property route — the capability publishes no zero-argument action.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_text" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for EditableComboBox {
    fn draw(&mut self, context: &mut RenderContext) {
        // The open list is painted from the popup's window and row boxes, so its filtered rows,
        // scroll offset and highlight are synchronised once here (I-129).
        self.sync_popup();
        // ── The band actually painted ──
        //
        // `geometry()` is the area the control was *given*; an editable combo box is a text
        // field, and a field is a fixed-height band. The fill, the border, the value and the
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
        let style = self.base.style();
        // One palette read for the whole frame through the revision-validated cache, so the
        // highlight does not take the manager lock a second time (I-P07).
        let palette_primary = crate::style::palette_shared().map(|colors| colors.primary);
        let theme = crate::style::resolved_theme_style_shared("editable_combo_box");
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
        // # Why the field acknowledges the pointer
        //
        // A closed combo box is a field the user presses to open a list, so it must read as
        // interactive. Before this it painted one colour for every pointer state, which is the
        // M1 gap the appendix's input group shares. `StateOverlay::fill_blend` is read rather
        // than a local constant so this matches the theme's own `"<kind>:hover"` weight.
        let overlay = crate::style::StateOverlay::from_base(
            self.base.is_hovered(),
            self.base.is_pressed(),
            self.base.draws_focus_ring(),
        );
        let bg_color = if is_enabled && !overlay.is_empty() {
            bg_color.blend(&bg_color.contrast_color(), overlay.fill_blend())
        } else {
            bg_color
        };
        context.fill_rounded_rect(rect, 4, bg_color);

        // Border
        let border_color = border;
        context.draw_rounded_rect_stroke(rect, 4, border_color, 1);

        // Draw text content and the dropdown arrow from **one** derivation, so the value
        // yields to the indicator instead of being written at an unconstrained offset. The font is
        // the **effective** one so the theme body font and the user's text scale reach the value
        // and the list rows (D09-STYLE-01).
        let font = effective_font(style);
        let line = context.text_line(rect, font);
        let geometry = self.indicator_geometry(line.height);
        let display_text = if self.text.is_empty() && !is_enabled { "" } else { &self.text };
        let text_color = self.base.disabled_ink_on(ink, bg_color);
        // The value is fitted into the box the indicator left, on that box's own line box: a
        // glyph origin is the box's top edge, so the old `padding + 13` put a 13 px font's
        // origin on the field's middle line and drew the value half a line low.
        let value_line = context.text_line(geometry.text_box, font);
        let value_align = self.alignment.to_horizontal().unwrap_or(HorizontalAlignment::Left);
        let value_advance = context.measure_text(display_text, font).width as i32;
        let value_origin =
            crate::render::fitted_origin(geometry.text_box, value_advance, value_align);
        // Selection band behind the selected range.
        if let Some((start, end)) = self.selection_range() {
            let start_x =
                value_origin.x + context.measure_text(&self.text[..start], font).width as i32;
            let end_x = value_origin.x + context.measure_text(&self.text[..end], font).width as i32;
            context.fill_rect(
                Rect::new(
                    start_x.min(end_x),
                    value_line.y,
                    (end_x - start_x).unsigned_abs(),
                    value_line.height.max(1),
                ),
                border.with_alpha(70),
            );
        }
        context.draw_text_fitted_cow(
            Rect::new(
                geometry.text_box.x,
                value_line.y,
                geometry.text_box.width,
                value_line.height,
            ),
            display_text,
            font,
            text_color,
            value_align,
        );

        // Caret at the field's own insertion point, clipped to the value's box.
        if is_enabled && self.cursor_blink.is_visible() {
            let caret = crate::widget::text_utils::floor_char_boundary(&self.text, self.cursor);
            let caret_x =
                value_origin.x + context.measure_text(&self.text[..caret], font).width as i32;
            let caret_x = caret_x
                .min(geometry.text_box.x + geometry.text_box.width as i32 - 1)
                .max(geometry.text_box.x);
            context.draw_line(
                Point::new(caret_x, value_line.y),
                Point::new(caret_x, value_line.y + value_line.height as i32),
                text_color,
            );
        }

        // Draw the dropdown indicator inside its own derived box.
        //
        // This glyph *is* the combo box's affordance — it is how a user knows the field opens —
        // so it is held to the text floor rather than dimmed by a fixed fraction. The old
        // `ink.blend(&bg_color, 0.45)` measured 3.45:1 on the dark field and 3.83:1 on the light
        // one, i.e. the control's one signifier was its least legible mark. It is still drawn
        // slightly quieter than the value, which is what keeps it reading as chrome.
        // The disclosure arrow is a `ChevronUp`/`ChevronDown` **icon outline** rather than the
        // `▲`/`▼` text glyph: no bundled face covers U+25B2/U+25BC, so those fell back to 8x8
        // bitmap blocks. It is centred in the box the field geometry reserved for it.
        let arrow_icon = if self.expanded { IconName::ChevronUp } else { IconName::ChevronDown };
        let arrow_color = if is_enabled {
            ink.legible_on(bg_color, 4.5).blend(&bg_color, 0.15)
        } else {
            // Derived from the field rather than toward it: `ink.blend(&bg_color, 0.7)` put the
            // indicator 0.7 of the way to the surface it sits on, which is the lowest-contrast
            // point available to it. The disabled weight keeps it muted *and* on its surface.
            self.base.disabled_ink_on(ink, bg_color)
        };
        crate::widget::draw_icon_centered(
            context,
            geometry.box_rect,
            geometry.box_rect.height.min(geometry.box_rect.width),
            arrow_color,
            arrow_icon,
        );

        // Draw dropdown if expanded
        if !self.expanded {
            return;
        }

        let start = self.popup.scroll_offset();
        let rows = self.popup.window_rows();
        let drop_rect = self.popup.popup_rect();

        // Dropdown background. The list floats over whatever is beneath it, so it is the
        // field's own colour rather than the window's: a transparent list would show the
        // content behind it through the popup.
        context.fill_rounded_rect(drop_rect, 2, field);
        context.draw_rounded_rect_stroke(drop_rect, 2, border, 1);

        if rows == 0 {
            let row = Rect::new(
                drop_rect.x + 1,
                drop_rect.y,
                drop_rect.width.saturating_sub(2),
                EDITABLE_ROW_HEIGHT,
            );
            let line = context.text_line(row, font);
            context.draw_text_fitted_cow(
                Rect::new(row.x + 8, line.y, row.width.saturating_sub(16), line.height),
                "(No matches)",
                font,
                ink.with_alpha(120),
                HorizontalAlignment::Left,
            );
            return;
        }

        // The selected-row highlight is the theme's accent, which is the one hue a theme
        // is expected to vary most. The palette was read once at the top of the frame.
        let highlight = palette_primary
            .map(|primary| primary.with_alpha(40))
            .unwrap_or_else(|| ink.blend(&field, 0.85));

        // Rows are painted **from their indices**: the popup names the rows in its painted
        // window, and the control reads the label from its own item storage. That keeps a
        // duplicate label resolving to its own index instead of the first one's (I-P08).
        for row in 0..rows {
            let painted = start + row;
            let Some(&original_index) = self.popup.visible_indices().get(painted) else { continue };
            let Some(item) = self.items.get(original_index) else { continue };
            let Some(row_box) = self.popup.row_rect(painted) else { continue };
            let item_rect = Rect::new(
                row_box.x + 1,
                row_box.y,
                row_box.width.saturating_sub(2),
                row_box.height,
            );

            // Highlight selected item
            if self.popup.selected_index() == Some(painted) {
                context.fill_rounded_rect(item_rect, 2, highlight);
            }

            // Item text: the row's own line box (the fixed `+18` placed the glyph box's
            // top edge 18px down a 28px row, half a line below centre) and fitted to the
            // row so a long entry cannot paint past the popup.
            let item_text_x = item_rect.x + 8;
            let item_line = context.text_line(item_rect, font);
            let item_color = self.base.disabled_ink_on(ink, field);
            context.draw_text_fitted_cow(
                Rect::new(
                    item_text_x,
                    item_line.y,
                    item_rect.width.saturating_sub(16),
                    item_line.height.max(1),
                ),
                item,
                font,
                item_color,
                HorizontalAlignment::Left,
            );
        }
    }
}

impl EventHandler for EditableComboBox {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        // The hit test and the list navigation read the popup's rows and window, so they are
        // brought up to date before this event consults them.
        self.sync_popup();

        match event {
            Event::KeyPress { key, modifiers } => {
                // Undo and redo are the platform primary chord: Control+Z/Y on
                // Windows and Linux, Command+Z/Y on macOS. A shifted chord is a
                // different binding and stays unbound here.
                if primary_shortcut_pressed(*key, *modifiers, Key::Z) {
                    let _ = self.undo();
                    return;
                }
                if primary_shortcut_pressed(*key, *modifiers, Key::Y) {
                    let _ = self.redo();
                    return;
                }
                match *key {
                    8 => {
                        self.backspace();
                        return;
                    }
                    127 => {
                        self.delete_forward();
                        return;
                    }
                    37 => {
                        let shift = crate::shortcut::Modifiers::from_event_bits(*modifiers)
                            .contains(crate::shortcut::Modifiers::SHIFT);
                        self.move_caret(-1, shift);
                        return;
                    }
                    39 => {
                        let shift = crate::shortcut::Modifiers::from_event_bits(*modifiers)
                            .contains(crate::shortcut::Modifiers::SHIFT);
                        self.move_caret(1, shift);
                        return;
                    }
                    36 => {
                        self.set_cursor_position(0);
                        return;
                    }
                    35 => {
                        self.set_cursor_position(self.text.len());
                        return;
                    }
                    13 => {
                        // Enter/Return: commit the highlighted row when there is one, then report
                        // the return and the end of the edit. A free-typed value selects nothing,
                        // so the two notifications are emitted outside the selection branch.
                        if self.expanded && !self.items.is_empty() {
                            if let Some(idx) = self.selected_index {
                                self.select_index(idx);
                            } else if let Some(idx) =
                                self.items.iter().position(|item| item == &self.text)
                            {
                                self.select_index(idx);
                            }
                        }
                        self.collapse();
                        self.return_pressed.emit();
                        self.editing_finished.emit();
                        self.base.request_redraw();
                        return;
                    }
                    27 => {
                        // Escape
                        self.collapse();
                        return;
                    }
                    38 => {
                        // ArrowUp: move through the filtered rows.
                        if self.expanded {
                            let indices = self.filtered_indices();
                            let current = self
                                .selected_index
                                .and_then(|index| indices.iter().position(|&i| i == index));
                            let position = match current {
                                Some(0) | None => 0,
                                Some(position) => position - 1,
                            };
                            if let Some(&index) = indices.get(position) {
                                self.selected_index = Some(index);
                                self.ensure_selected_visible();
                                self.base.request_redraw();
                            }
                        }
                        return;
                    }
                    40 => {
                        // ArrowDown: open and move through the filtered rows.
                        if !self.expanded {
                            self.expand();
                        }
                        let indices = self.filtered_indices();
                        let current = self
                            .selected_index
                            .and_then(|index| indices.iter().position(|&i| i == index));
                        let position = match current {
                            Some(position) => (position + 1).min(indices.len().saturating_sub(1)),
                            None => 0,
                        };
                        if let Some(&index) = indices.get(position) {
                            self.selected_index = Some(index);
                            self.ensure_selected_visible();
                            self.base.request_redraw();
                        }
                        return;
                    }
                    _ => {
                        // Character input
                        if let Some(ch) = char::from_u32(*key) {
                            if ch.is_ascii_graphic() || ch == ' ' {
                                self.append_committed_text(&ch.to_string());
                                return;
                            }
                        }
                    }
                }
                self.base.handle_event(event);
            }
            // Platform-committed text (D09-INPUT-01). A printable character or an IME commit reaches
            // the control as `TextInput`/`ImeCommit`, not as a `KeyPress`, so without this branch a
            // typed value never entered the field. It funnels into the shared
            // `append_committed_text`, which drives `set_text` — the same entry point the printable
            // key arm uses. The `is_enabled` guard above already ran.
            Event::TextInput { text } | Event::ImeCommit { text } => {
                self.append_committed_text(text);
            }
            Event::FocusLost => {
                // The field finishing an edit is the event a form validates on; the list also
                // collapses so a focus change cannot leave a popup pinned open.
                self.base.handle_event(event);
                if self.expanded {
                    self.collapse();
                }
                self.editing_finished.emit();
            }
            Event::MousePress { pos, button, .. } if *button == 1 => {
                let rect = self.geometry();

                // Click on the text field (with arrow zone)
                if rect.contains_point(*pos) {
                    // Check if click is on the arrow area (right side)
                    let arrow_zone_x = rect.x + rect.width as i32 - 24;
                    if pos.x >= arrow_zone_x {
                        // Arrow zone - toggle dropdown
                        self.toggle();
                    } else {
                        // Text field - focus and expand if items exist
                        if !self.expanded && !self.items.is_empty() {
                            self.expand();
                        } else {
                            self.toggle();
                        }
                    }
                    return;
                }

                // Click on a dropdown item
                if self.expanded {
                    if let Some(row) = self.popup.window_row_at(*pos) {
                        if let Some(&index) = self.popup.visible_indices().get(row) {
                            self.select_index(index);
                        }
                        return;
                    }

                    // Click outside dropdown - collapse
                    if !rect.contains_point(*pos) {
                        self.collapse();
                    }
                }
            }
            Event::Wheel { delta, modifiers } => {
                if !self.expanded {
                    self.base.handle_event(event);
                    return;
                }
                let scroll = WheelScroll::from_event(*delta, *modifiers, 1);
                if scroll.lines_y != 0 {
                    let offset = (self.scroll_offset() as i32 + scroll.lines_y).max(0) as usize;
                    self.set_scroll_offset(offset);
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

    #[test]
    fn editable_combo_box_default_creation() {
        let cb = EditableComboBox::new(Rect::new(0, 0, 200, 30));
        assert_eq!(cb.kind(), WidgetKind::EditableComboBox);
        assert_eq!(cb.text(), "");
        assert_eq!(cb.item_count(), 0);
        assert!(!cb.is_expanded());
        assert!(cb.selected_index().is_none());
    }

    #[test]
    fn editable_combo_box_add_and_remove_items() {
        let mut cb = EditableComboBox::new(Rect::new(0, 0, 200, 30));
        cb.add_item("Option A");
        cb.add_item("Option B");
        cb.add_item("Option C");
        assert_eq!(cb.item_count(), 3);

        assert!(cb.remove_item(1));
        assert_eq!(cb.item_count(), 2);
        assert_eq!(cb.items()[0], "Option A");
        assert_eq!(cb.items()[1], "Option C");

        assert!(!cb.remove_item(5));
        assert_eq!(cb.item_count(), 2);
    }

    #[test]
    fn editable_combo_box_text_operations() {
        let mut cb = EditableComboBox::new(Rect::new(0, 0, 200, 30));
        assert_eq!(cb.text(), "");

        cb.set_text("Hello");
        assert_eq!(cb.text(), "Hello");

        cb.set_text("World");
        assert_eq!(cb.text(), "World");

        cb.clear_items();
        // Text should be unchanged by clear_items
        assert_eq!(cb.text(), "World");
    }

    #[test]
    fn editable_combo_box_expand_collapse() {
        let mut cb = EditableComboBox::new(Rect::new(0, 0, 200, 30));
        assert!(!cb.is_expanded());

        cb.expand();
        assert!(cb.is_expanded());

        cb.collapse();
        assert!(!cb.is_expanded());

        cb.toggle();
        assert!(cb.is_expanded());

        cb.toggle();
        assert!(!cb.is_expanded());
    }

    #[test]
    fn editable_combo_box_select_index() {
        let mut cb = EditableComboBox::new(Rect::new(0, 0, 200, 30));
        cb.add_item("Alpha");
        cb.add_item("Beta");
        cb.add_item("Gamma");

        assert!(cb.select_index(1));
        assert_eq!(cb.selected_index(), Some(1));
        assert_eq!(cb.text(), "Beta");
        assert!(!cb.is_expanded());

        // Selecting same index again should return false
        assert!(!cb.select_index(1));

        // Selecting out of bounds should be noop
        assert!(!cb.select_index(10));
    }

    #[test]
    fn editable_combo_box_svg_output() {
        let mut cb = EditableComboBox::new(Rect::new(0, 0, 200, 30));
        cb.add_item("Item 1");
        cb.add_item("Item 2");
        cb.set_text("Hello");
        let svg = render_to_svg(&mut cb);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn editable_combo_box_text_changed_signal() {
        let mut cb = EditableComboBox::new(Rect::new(0, 0, 200, 30));
        let captured = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
        let c = captured.clone();
        cb.text_changed.connect(move |val: std::sync::Arc<String>| {
            *c.lock().unwrap() = Some(val.to_string());
        });

        cb.set_text("CustomValue");
        assert_eq!(captured.lock().unwrap().as_deref(), Some("CustomValue"));
    }

    #[test]
    fn editable_combo_box_item_selected_signal() {
        let mut cb = EditableComboBox::new(Rect::new(0, 0, 200, 30));
        cb.add_item("A");
        cb.add_item("B");

        let captured = std::sync::Arc::new(std::sync::Mutex::new(None::<usize>));
        let c = captured.clone();
        cb.item_selected.connect(move |val: std::sync::Arc<usize>| {
            *c.lock().unwrap() = Some(*val);
        });

        cb.select_index(1);
        assert_eq!(*captured.lock().unwrap(), Some(1));
    }

    /// The value's box ends where the indicator's box begins, at every control width.
    ///
    /// # What this pins
    ///
    /// BLUE22 §B.6 rule 4: a sub-part's box is derived from its sibling, so the value *yields*
    /// to the indicator. The value used to be written at `rect.x + 6` with no upper bound
    /// while the indicator sat at `rect.x + rect.width - 20`, so a longer value or a wider
    /// indicator glyph put the two on top of each other.
    /// The value's box ends where the indicator's box begins, at every control width.
    ///
    /// # What this pins
    ///
    /// BLUE22 §B.6 rule 4: a sub-part's box is derived from its sibling, so the value *yields*
    /// to the indicator. The value used to be written at `rect.x + 6` with no upper bound
    /// while the indicator sat at `rect.x + rect.width - 20`, so a longer value or a wider
    /// indicator glyph put the two on top of each other.
    ///
    /// The widths below include two that are narrower than the leading inset plus the
    /// indicator, because that is where a derivation that is only *described* as a relation
    /// comes apart: the value's box must collapse rather than claim room the band does not
    /// have.
    #[test]
    fn the_value_box_ends_where_the_indicator_begins() {
        let trailing =
            dimensions::TEXT_FIELD_PADDING_H + dimensions::BUTTON_ICON_SIZE + INDICATOR_LEADING_GAP;
        for width in [0u32, 20, 64, 240, 400] {
            let cb = EditableComboBox::new(Rect::new(0, 0, width, 120));
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
            if width < trailing {
                assert_eq!(
                    geometry.text_box.width, 0,
                    "a band too narrow for its own chrome holds no value at width {width}"
                );
            } else {
                assert!(
                    geometry.text_box.width > 0,
                    "a band with room must give the value some of it at width {width}"
                );
            }
        }
    }

    /// The reported height is the band that is painted.
    #[test]
    fn the_reported_height_is_the_band_that_is_painted() {
        let cb = EditableComboBox::new(Rect::new(0, 0, 240, 120));
        let band = cb.field_band();
        assert_eq!(band.height, dimensions::TEXT_FIELD_MIN_HEIGHT);
        assert_eq!(cb.size_hint().height, band.height);
        assert_eq!(band.y, (120 - dimensions::TEXT_FIELD_MIN_HEIGHT as i32) / 2);
    }

    /// The field has a caret, so it edits in the middle rather than only appending.
    #[test]
    fn the_field_has_a_caret_model() {
        let mut cb = EditableComboBox::new(Rect::new(0, 0, 200, 40));
        cb.set_text("helo");
        cb.handle_event(&Event::KeyPress { key: 37, modifiers: 0 });
        cb.handle_event(&Event::KeyPress { key: 108, modifiers: 0 });
        assert_eq!(cb.text(), "hello");
        assert_eq!(cb.cursor_position(), 4);

        cb.handle_event(&Event::KeyPress { key: 36, modifiers: 0 }); // Home
        cb.handle_event(&Event::KeyPress { key: 127, modifiers: 0 }); // Delete
        assert_eq!(cb.text(), "ello");
        assert_eq!(cb.cursor_position(), 0);
    }

    /// The open list filters as the value changes and scrolls when capped.
    #[test]
    fn the_open_list_filters_and_scrolls() {
        let mut cb = EditableComboBox::new(Rect::new(0, 0, 200, 40));
        for item in ["alpha", "alpine", "beta", "gamma", "alps"] {
            cb.add_item(item);
        }
        cb.expand();
        cb.set_text("al");
        assert_eq!(
            cb.filtered_items(),
            vec!["alpha".to_string(), "alpine".to_string(), "alps".to_string()].as_slice()
        );

        cb.set_max_visible_items(Some(2));
        assert_eq!(cb.max_visible_items(), Some(2));
        cb.handle_event(&Event::Wheel { delta: Point::new(0, 1), modifiers: 0 });
        assert_eq!(cb.scroll_offset(), 1);

        cb.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
        assert!(cb.selected_index().is_some());
        let svg = crate::widget::svg::render_to_svg(&mut cb);
        assert!(svg.contains("alpha") || cb.selected_index().is_some());
    }

    /// The caret, the row cap and the list's scroll offset are published and writable.
    #[test]
    fn the_caret_and_row_cap_are_published() {
        let mut cb = EditableComboBox::new(Rect::new(0, 0, 200, 40));
        cb.set_text("hello");
        assert_eq!(cb.get("cursor_position").unwrap(), CapabilityValue::UInt(5));
        cb.set("cursor_position", CapabilityValue::UInt(2)).unwrap();
        assert_eq!(cb.cursor_position(), 2);

        // The cap is nullable: `None` restores the full expansion.
        assert_eq!(cb.get("max_visible_items").unwrap(), CapabilityValue::Null);
        cb.set("max_visible_items", CapabilityValue::UInt(3)).unwrap();
        assert_eq!(cb.max_visible_items(), Some(3));
        cb.set("max_visible_items", CapabilityValue::Null).unwrap();
        assert_eq!(cb.max_visible_items(), None);

        cb.set("scroll_offset", CapabilityValue::UInt(0)).unwrap();
        assert_eq!(cb.get("scroll_offset").unwrap(), CapabilityValue::UInt(0));
    }
}
