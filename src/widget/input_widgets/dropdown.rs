// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Dropdown widget — standalone dropdown list selector (BLUE13 R2.4).
//!
//! A button-like label showing the current selection. When clicked, it
//! expands to show a scrollable list of options. Selecting an item emits
//! a `changed` signal and collapses the list; a row the user confirms also
//! emits `activated`, and the row the pointer or keyboard marks while the
//! list is open is reported through `highlighted`.

use crate::compat::{Instant, String, ToString, Vec};
use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::key_codes;
use crate::event::{Event, EventHandler, WheelScroll};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::style::EdgeOffsets;
use crate::widget::capability::coercion::{
    expect_bool, expect_horizontal_alignment, expect_string, expect_usize,
    horizontal_alignment_to_str,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, effective_font, estimate_text_width, ControlMetrics};
use crate::widget::{BaseWidget, Draw, IconName, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Height of each item row in the expanded dropdown list (pixels).
const ITEM_HEIGHT: u32 = 20;
/// Width of the leading cell a row's icon occupies, when it has one.
///
/// A fixed cell rather than a per-frame measurement: the row's label starts at the same
/// offset whatever glyph the icon is, so a list whose icons differ does not shimmer and
/// showing a row costs no text shaping.
const ITEM_ICON_WIDTH: u32 = 16;
/// Padding between the button text and the widget edges.
const PADDING: i32 = 4;

/// Width of the indicator cell at the field's trailing edge.
///
/// It was the literal `12` at the draw site while the value was written at `geo.x + PADDING`
/// with no bound, so the two were independent derivations from the same field width and a
/// long selection ran under the arrow. Naming it is what lets the value's box be derived
/// from it instead of from a second numeral.
const INDICATOR_WIDTH: i32 = 12;

/// Space between the indicator's leading edge and the end of the value's box: 2.
///
/// Half of [`PADDING`], written as a literal because `Ord::max` is not available in a
/// `const` expression on this toolchain; the relation is documented rather than computed so
/// the constant table stays where a reader can find it.
const INDICATOR_LEADING_GAP: i32 = 2;

/// The indicator's box and the box the value may occupy, derived together.
///
/// # Why the two are computed together
///
/// A `▼` and the label beside it are one row: the label must *yield* to the indicator rather
/// than be clipped by an inset that was chosen independently of it. The shared relation reads
/// `rightPadding: padding + indicator.width`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct FieldGeometry {
    /// The indicator cell at the field's trailing edge.
    indicator_box: Rect,
    /// The rectangle the selected label may occupy.
    label_box: Rect,
}

impl FieldGeometry {
    /// Derives both boxes from the band the control paints and `line_height`.
    fn for_band(band: Rect, line_height: u32) -> Self {
        let indicator_width = INDICATOR_WIDTH.min(band.width as i32);
        let height = line_height.min(band.height);
        let indicator_x = band.x + band.width as i32 - PADDING - indicator_width;
        // Keep the cell inside the band when the field is narrower than its own padding: a
        // field with no room for its chrome shows no indicator rather than one outside itself.
        let indicator_x =
            indicator_x.min(band.x + band.width.saturating_sub(indicator_width as u32) as i32);
        let indicator_box = Rect::new(
            indicator_x.max(band.x),
            band.y + (band.height.saturating_sub(height) / 2) as i32,
            indicator_width.max(0) as u32,
            height,
        );
        let label_right = indicator_box.x.saturating_sub(INDICATOR_LEADING_GAP);
        let label_left = (band.x + PADDING).min(label_right);
        let label_box = Rect::new(
            label_left,
            band.y,
            label_right.saturating_sub(label_left) as u32,
            band.height,
        );
        Self { indicator_box, label_box }
    }
}

/// One row of a [`Dropdown`]'s list.
///
/// The plain model is still `Vec<String>` for callers that only have labels (through
/// [`Dropdown::set_items`]), but a real select carries more: a disabled row, a leading glyph,
/// and a caller-owned token. One struct holds all four rather than four parallel vectors, so
/// the row cannot be half-updated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DropdownItem {
    text: String,
    enabled: bool,
    icon: Option<String>,
    data: Option<String>,
}

impl Default for DropdownItem {
    fn default() -> Self {
        Self { text: String::new(), enabled: true, icon: None, data: None }
    }
}

impl DropdownItem {
    /// Creates an enabled row with the given label and no icon or data.
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), enabled: true, icon: None, data: None }
    }

    /// Marks the row as enabled or disabled.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Sets the row's icon text (a glyph or short marker) drawn before the label.
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Attaches a caller-owned data token to the row.
    pub fn with_data(mut self, data: impl Into<String>) -> Self {
        self.data = Some(data.into());
        self
    }

    /// The row's label — what the field shows when the row is selected.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Whether the row can be selected and activated.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// The row's icon text, if it has one.
    pub fn icon(&self) -> Option<&str> {
        self.icon.as_deref()
    }

    /// The caller-owned data token, if one was attached.
    pub fn data(&self) -> Option<&str> {
        self.data.as_deref()
    }
}

/// Dropdown widget for selecting from a list of options.
///
/// # Behaviour
/// - Displays the currently selected item (or a placeholder) inside a button-like
///   rectangle with a ▼ arrow.
/// - On click the list expands below the button; clicking an item selects it and
///   collapses the list.
/// - Losing focus also collapses the list.
pub struct Dropdown {
    base: BaseWidget,
    /// The rows: label, enabled flag, icon and caller data.
    items: Vec<DropdownItem>,
    /// The labels, in row order.
    ///
    /// A **derived** cache, not a second source of truth: [`Self::sync_labels`] rebuilds it after
    /// every row mutation, so [`Self::items`] can keep the `&[String]` signature it has always
    /// had without the labels and the rows being editable independently.
    labels: Vec<String>,
    /// Currently selected index. Defaults to `0` when items is non-empty.
    selected_index: usize,
    /// Whether the dropdown list is expanded.
    expanded: bool,
    /// How the selected label is aligned within its own box.
    ///
    /// Horizontal only: the label is centred vertically in the field, so a `top`/`bottom` value
    /// would be one this control could never honour —
    /// [`crate::widget::capability::coercion::expect_horizontal_alignment`] refuses those rather
    /// than accepting a write that does nothing. Defaults to left.
    alignment: crate::core::Alignment,
    /// Signal emitted when the selection changes.
    pub changed: GenericSignal,
    /// Emitted with the row the user confirms (click or Enter).
    ///
    /// Distinct from [`Self::changed`], which also fires for programmatic selections and for
    /// keyboard navigation *before* the user commits: this one says the user chose the row.
    pub activated: Signal1<usize>,
    /// Emitted with the row the pointer or keyboard currently marks while the list is open.
    ///
    /// Clearing the mark (the pointer leaving the list, or the list closing) has no row to
    /// name, so it is not emitted; [`Self::highlighted_index`] reports the cleared state.
    pub highlighted: Signal1<usize>,
    /// The row `highlighted` last named, or `None` when nothing is marked.
    highlighted_index: Option<usize>,
    /// Maximum number of rows the expanded list shows at once. `None` shows them all.
    max_visible: Option<usize>,
    /// Index of the first row painted in the popup.
    scroll_offset: usize,
    /// Prefix typed since the last navigation, for first-letter selection.
    type_ahead: String,
    /// The text painted when nothing is selected (I-09).
    ///
    /// Defaults to the `"(Select)"` the control used to hardcode, so an existing caller paints
    /// exactly what it always did; a host with business wording replaces it.
    placeholder: String,
    /// The selection read when the list opened, restored when Escape cancels (I-10).
    expanded_selection: Option<usize>,
    /// When the current type-ahead prefix began, for the shared 800 ms rule (I-P06).
    type_ahead_started: Option<Instant>,
}

impl Dropdown {
    /// Creates a new `Dropdown` with the given items and geometry.
    ///
    /// The first item is automatically selected when `items` is non-empty.
    /// The widget's height should accommodate the collapsed button area.
    pub fn new(items: Vec<String>, rect: Rect) -> Self {
        let rows: Vec<DropdownItem> = items.into_iter().map(DropdownItem::new).collect();
        let labels = rows.iter().map(|item| item.text.clone()).collect();
        Self {
            base: BaseWidget::new(WidgetKind::Dropdown, rect, "Dropdown"),
            items: rows,
            labels,
            selected_index: 0,
            expanded: false,
            alignment: crate::core::Alignment::Left,
            changed: GenericSignal::new(),
            activated: Signal1::new(),
            highlighted: Signal1::new(),
            highlighted_index: None,
            max_visible: None,
            scroll_offset: 0,
            type_ahead: String::new(),
            placeholder: String::from("(Select)"),
            expanded_selection: None,
            type_ahead_started: None,
        }
    }

    /// The text painted while nothing is selected (I-09).
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }

    /// Sets the text painted while nothing is selected.
    ///
    /// The default is `"(Select)"`, which is what the control painted before the string was
    /// configurable (I-09).
    pub fn set_placeholder(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.placeholder == text {
            return;
        }
        self.placeholder = text;
        self.base.request_redraw();
    }

    /// Rebuilds the label cache from the row model.
    fn sync_labels(&mut self) {
        self.labels.clear();
        self.labels.extend(self.items.iter().map(|item| item.text.clone()));
    }

    /// Returns the full list of option labels.
    pub fn items(&self) -> &[String] {
        &self.labels
    }

    /// The row at `index`, or `None` when the index is out of range.
    pub fn item(&self, index: usize) -> Option<&DropdownItem> {
        self.items.get(index)
    }

    /// Whether a row exists and is enabled.
    pub fn is_item_enabled(&self, index: usize) -> bool {
        self.items.get(index).is_some_and(|item| item.is_enabled())
    }

    /// How the selected label is aligned within its own box.
    pub fn alignment(&self) -> crate::core::Alignment {
        self.alignment
    }

    /// Sets how the selected label is aligned within its own box.
    ///
    /// Horizontal only. A `top`/`bottom` alignment is **ignored**, because the label is centred
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

    /// Replaces all rows with plain labels and resets the selection to the first item
    /// (or index 0 if the list is empty).
    ///
    /// Rows built with [`DropdownItem`] keep their enabled flag, icon and data through
    /// [`Self::insert_item`]; this writer is the `Vec<String>` shape the property route carries.
    pub fn set_items(&mut self, new_items: Vec<String>) {
        self.items = new_items.into_iter().map(DropdownItem::new).collect();
        self.sync_labels();
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.highlighted_index = None;
        self.changed.emit();
        self.base.request_redraw();
    }

    /// Inserts a row at `index`, shifting the rows after it.
    ///
    /// An index past the end is ignored, matching the list mutators elsewhere. Inserting before
    /// the selected row keeps the selection on the row it named rather than on the row number it
    /// happened to occupy.
    pub fn insert_item(&mut self, index: usize, item: DropdownItem) {
        if index > self.items.len() {
            return;
        }
        let had_items = !self.items.is_empty();
        self.items.insert(index, item);
        self.sync_labels();
        if had_items && index <= self.selected_index {
            self.selected_index += 1;
        }
        self.scroll_offset = self.clamp_scroll(self.scroll_offset);
        self.base.request_redraw();
    }

    /// Removes the row at `index`, returning it.
    ///
    /// The selection follows the rows: removing a row before it shifts it down, and removing
    /// the selected row moves the selection to the nearest remaining row (or `0` when the list
    /// is now empty). `changed` is emitted whenever the row the selection names is no longer
    /// the one it named.
    pub fn remove_item(&mut self, index: usize) -> Option<DropdownItem> {
        if index >= self.items.len() {
            return None;
        }
        let removed = self.items.remove(index);
        self.sync_labels();
        let mut selection_moved = false;
        if index < self.selected_index {
            self.selected_index -= 1;
            selection_moved = true;
        } else if index == self.selected_index {
            self.selected_index = self.selected_index.min(self.items.len().saturating_sub(1));
            selection_moved = true;
        }
        self.scroll_offset = self.clamp_scroll(self.scroll_offset);
        self.highlighted_index = None;
        if selection_moved {
            self.changed.emit();
        }
        self.base.request_redraw();
        Some(removed)
    }

    /// Enables or disables a row. A disabled row cannot be chosen by pointer or keyboard.
    pub fn set_item_enabled(&mut self, index: usize, enabled: bool) {
        let Some(item) = self.items.get_mut(index) else {
            return;
        };
        if item.enabled == enabled {
            return;
        }
        item.enabled = enabled;
        self.base.request_redraw();
    }

    /// Sets a row's icon text, or clears it with `None`.
    pub fn set_item_icon(&mut self, index: usize, icon: Option<String>) {
        let Some(item) = self.items.get_mut(index) else {
            return;
        };
        if item.icon == icon {
            return;
        }
        item.icon = icon;
        self.base.request_redraw();
    }

    /// Attaches a caller-owned token to a row, or clears it with `None`.
    pub fn set_item_data(&mut self, index: usize, data: Option<String>) {
        let Some(item) = self.items.get_mut(index) else {
            return;
        };
        if item.data == data {
            return;
        }
        item.data = data;
    }

    /// Relabels a row, keeping the derived label cache in step.
    pub fn set_item_text(&mut self, index: usize, text: impl Into<String>) {
        let text = text.into();
        let Some(item) = self.items.get_mut(index) else {
            return;
        };
        if item.text == text {
            return;
        }
        item.text = text;
        self.sync_labels();
        self.base.request_redraw();
    }

    /// Returns the currently-selected index (may be out of range if the list
    /// was shrunk after selection).
    pub fn selected_index(&self) -> Option<usize> {
        self.items.get(self.selected_index).map(|_| self.selected_index)
    }

    /// Sets the selected index, clamping it to the list bounds.
    ///
    /// Emits the `changed` signal whenever the index actually changes or is
    /// clamped to a different value.
    pub fn set_selected_index(&mut self, index: usize) {
        let clamped =
            if self.items.is_empty() { 0 } else { index.min(self.items.len().saturating_sub(1)) };
        // While the list is open, the row the keyboard moves to is the row that is marked — the
        // model's selection and the popup's current row are the same index here, so the mark is
        // reported from the one place they meet.
        if self.expanded && !self.items.is_empty() {
            self.set_highlighted(Some(clamped));
        }
        if self.selected_index != clamped {
            self.selected_index = clamped;
            self.changed.emit();
            self.base.request_redraw();
        }
    }

    /// The row the pointer or keyboard currently marks, or `None` when nothing is marked.
    pub fn highlighted_index(&self) -> Option<usize> {
        self.highlighted_index
    }

    /// Marks `index` as the popup's current row, reporting a change once.
    fn set_highlighted(&mut self, index: Option<usize>) {
        if self.highlighted_index == index {
            return;
        }
        self.highlighted_index = index;
        if let Some(index) = index {
            self.highlighted.emit(index);
        }
    }

    /// Returns the text of the currently-selected item, or `None` if the list
    /// is empty.
    pub fn selected_text(&self) -> Option<&str> {
        if self.selected_index < self.items.len() {
            Some(self.items[self.selected_index].text.as_str())
        } else {
            None
        }
    }

    /// Returns `true` when the dropdown list is currently expanded.
    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    /// Expands or collapses the dropdown list.
    pub fn set_expanded(&mut self, expanded: bool) {
        if self.expanded == expanded {
            return;
        }
        self.expanded = expanded;
        if expanded {
            // The selection at open time is what Escape restores (I-10).
            self.expanded_selection = Some(self.selected_index);
            self.ensure_selected_visible();
            // Opening marks the selected row, so the popup says where Enter would land before
            // the user moves anything.
            let marked = self.selected_index();
            self.set_highlighted(marked);
            #[cfg(not(alloc_frugal))]
            crate::widget::runtime::set_popup_grab(self.id());
        } else {
            // A closing popup has no marked row; clearing the mark has no index to report.
            self.highlighted_index = None;
            #[cfg(not(alloc_frugal))]
            crate::widget::runtime::clear_popup_grab_for(self.id());
        }
        self.base.request_redraw();
    }

    /// Toggles the expanded / collapsed state.
    pub fn toggle(&mut self) {
        self.set_expanded(!self.expanded);
    }

    // ─── helpers ────────────────────────────────────────────────────────

    /// The band the collapsed field actually occupies: full width, one field tall, centred.
    ///
    /// # Why the field is not the control's rectangle
    ///
    /// A dropdown is a text field with an indicator in it, and
    /// [`dimensions::TEXT_FIELD_MIN_HEIGHT`] is what every field in this crate occupies. The
    /// 240x120 census cell drew a 240x120 slab, so the collapsed field and the `line_edit`
    /// beside it were different objects. The band is the single derivation the fill, the
    /// border, the label and the indicator all read — and the expanded list still hangs from
    /// the control's own bottom edge, because the list is not part of the field.
    fn field_band(&self) -> Rect {
        ControlMetrics::full_width_band(self.geometry(), dimensions::TEXT_FIELD_MIN_HEIGHT)
    }

    /// The indicator's cell and the label's box, derived from the band and one line height.
    fn field_geometry(&self, line_height: u32) -> FieldGeometry {
        FieldGeometry::for_band(self.field_band(), line_height)
    }

    /// Compute the bounding `Rect` of the n-th list item in screen coordinates.
    /// Only meaningful when `expanded == true`.
    fn item_rect(&self, index: usize) -> Rect {
        let geo = self.geometry();
        // `index` is a row in the model; the paint stacks visible rows from the scroll
        // offset, so this maps it into the window the popup actually shows.
        let row = index.saturating_sub(self.clamp_scroll(self.scroll_offset));
        let item_y = geo.y + geo.height as i32 + (row as u32 * ITEM_HEIGHT) as i32;
        Rect::new(geo.x, item_y, geo.width, ITEM_HEIGHT)
    }

    /// Determine which list item (if any) a point falls on.
    fn hit_test_item(&self, pos: Point) -> Option<usize> {
        if !self.expanded {
            return None;
        }
        let (start, end) = self.visible_range();
        (start..end).find(|&index| self.item_rect(index).contains_point(pos))
    }

    /// The maximum number of rows the popup shows at once.
    pub fn max_visible(&self) -> Option<usize> {
        self.max_visible
    }

    /// Caps the number of rows the popup paints.
    ///
    /// `None` restores full expansion. A cap of zero is treated as one row, because an open
    /// popup with no rows reads as a glitch rather than as a setting.
    pub fn set_max_visible(&mut self, max_visible: Option<usize>) {
        let max_visible = max_visible.map(|value| value.max(1));
        if self.max_visible == max_visible {
            return;
        }
        self.max_visible = max_visible;
        self.ensure_selected_visible();
        self.base.request_redraw();
    }

    /// Index of the first row painted in the popup.
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Sets the first painted row, clamped so the popup stays full when it can.
    pub fn set_scroll_offset(&mut self, offset: usize) {
        let next = self.clamp_scroll(offset);
        if self.scroll_offset == next {
            return;
        }
        self.scroll_offset = next;
        self.base.request_redraw();
    }

    /// Number of rows the popup shows at once.
    pub fn page_size(&self) -> usize {
        self.max_visible.unwrap_or(self.items.len()).max(1)
    }

    fn clamp_scroll(&self, offset: usize) -> usize {
        let page = self.page_size();
        let max_offset = self.items.len().saturating_sub(page.min(self.items.len()));
        offset.min(max_offset)
    }

    /// Scrolls the popup so `index` is inside the visible window.
    fn ensure_row_visible(&mut self, index: usize) {
        if self.items.is_empty() {
            self.scroll_offset = 0;
            return;
        }
        let page = self.page_size();
        if index < self.scroll_offset {
            self.scroll_offset = index;
        } else if index >= self.scroll_offset + page {
            self.scroll_offset = index + 1 - page;
        }
        self.scroll_offset = self.clamp_scroll(self.scroll_offset);
    }

    /// Scrolls the popup so the selected row is inside the visible window.
    pub fn ensure_selected_visible(&mut self) {
        self.ensure_row_visible(self.selected_index);
    }

    /// Moves the popup's **mark** to `index`, keeping it in view.
    ///
    /// The mark is the row Enter would confirm; moving it does not change the selection, which
    /// is what makes the open list a two-stage gesture — browse, then confirm (I-10).
    fn move_highlight(&mut self, index: usize) {
        self.set_highlighted(Some(index));
        self.ensure_row_visible(index);
        self.base.request_redraw();
    }

    /// Rows currently inside the popup window.
    fn visible_range(&self) -> (usize, usize) {
        let start = self.clamp_scroll(self.scroll_offset);
        let end = (start + self.page_size()).min(self.items.len());
        (start, end)
    }

    /// Selects the first enabled row at or after `start` whose label begins with `prefix`.
    ///
    /// The prefix is folded once and each label compared byte-wise, instead of allocating a
    /// lowercased copy of every row (I-P05).
    fn find_prefix(&self, prefix: &str, start: usize) -> Option<usize> {
        let needle = crate::widget::input_widgets::text_match::fold(prefix);
        let count = self.items.len();
        if count == 0 {
            return None;
        }
        (0..count).map(|offset| (start + offset) % count).find(|&index| {
            self.is_item_enabled(index)
                && crate::widget::input_widgets::text_match::starts_with(
                    &self.items[index].text,
                    &needle,
                )
        })
    }

    /// The nearest enabled row before or after `from`, or `None` when there is none.
    fn neighbour_enabled(&self, from: usize, forward: bool) -> Option<usize> {
        if forward {
            (from.saturating_add(1)..self.items.len()).find(|&index| self.is_item_enabled(index))
        } else {
            (0..from.min(self.items.len())).rev().find(|&index| self.is_item_enabled(index))
        }
    }
    /// The row the popup currently marks, falling back to the committed selection.
    fn marked_row(&self) -> usize {
        self.highlighted_index.unwrap_or(self.selected_index)
    }
}

impl Widget for Dropdown {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    /// The field **and** the expanded list accept the pointer.
    ///
    /// The list is painted below this control's own rectangle; the default hit rule would reject
    /// every row, so a click on an item fell through to the window.
    fn contains_point(&self, point: Point) -> bool {
        self.base().contains_point_with_touch_expansion(point)
            || (self.expanded
                && (0..self.items.len()).any(|index| self.item_rect(index).contains_point(point)))
    }

    fn has_open_popup(&self) -> bool {
        self.expanded
    }

    fn size_hint(&self) -> crate::core::Size {
        // Derived through the same metric the band is drawn from, so the reported size and the
        // painted box cannot describe two different controls. The width was a bare
        // `max(text.len()) * 8` — a **byte** count, so a CJK label asked for roughly three times
        // the width it draws (I-P15). The measurement is the shared per-cluster estimate
        // `editable_combo_box` already uses.
        let font = effective_font(self.style());
        let widest = self
            .items
            .iter()
            .map(|item| estimate_text_width(&item.text, font, 1.0))
            .max()
            .unwrap_or(6);
        let side_air = (dimensions::TEXT_FIELD_MIN_HEIGHT / 2).saturating_sub(8);
        let trailing = (PADDING.max(0) as u32)
            + INDICATOR_WIDTH.max(0) as u32
            + INDICATOR_LEADING_GAP.max(0) as u32;
        let padding = EdgeOffsets {
            top: side_air,
            right: trailing,
            bottom: side_air,
            left: PADDING.max(0) as u32,
        };
        let floor = Size::new(
            PADDING.max(0) as u32 + INDICATOR_WIDTH.max(0) as u32 + trailing,
            dimensions::TEXT_FIELD_MIN_HEIGHT,
        );
        let hint = ControlMetrics::implicit_size(Size::new(widest, 0), padding, floor);
        // The height is the band's, not a second numeral: this is what makes "the reported size
        // and the drawn box agree" checkable rather than merely intended. A `select` with no
        // items is still a field, so the height does not depend on the list.
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
            "changed" => Some(EventSignalRef::unit("changed", &self.changed)),
            "activated" => Some(EventSignalRef::mapped("activated", &self.activated, |v| {
                CapabilityValue::UInt(*v as u64)
            })),
            "highlighted" => Some(EventSignalRef::mapped("highlighted", &self.highlighted, |v| {
                CapabilityValue::UInt(*v as u64)
            })),
            _ => None,
        }
    }
}

/// `Dropdown`'s property contract.
///
/// `text` is the selected item's label and follows the old arm's shape: an empty
/// string (not `Null`) when nothing is selected, so callers do not need a
/// two-way `Option` check for a display-only value.
impl WidgetProperties for Dropdown {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => match self.selected_text() {
                Some(text) => Ok(CapabilityValue::String(text.to_string())),
                None => Ok(CapabilityValue::String(String::new())),
            },
            "selected_index" => {
                Ok(CapabilityValue::UInt(self.selected_index().unwrap_or(0) as u64))
            }
            "item_count" => Ok(CapabilityValue::UInt(self.items().len() as u64)),
            "expanded" => Ok(CapabilityValue::Bool(self.is_expanded())),
            "max_visible" => match self.max_visible() {
                Some(max) => Ok(CapabilityValue::UInt(max as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "scroll_offset" => Ok(CapabilityValue::UInt(self.scroll_offset() as u64)),
            "placeholder" => Ok(CapabilityValue::String(self.placeholder().to_string())),
            "alignment" => Ok(CapabilityValue::String(
                horizontal_alignment_to_str(self.alignment()).to_string(),
            )),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "selected_index" => {
                self.set_selected_index(expect_usize(value)?);
                Ok(())
            }
            "expanded" => {
                self.set_expanded(expect_bool(value)?);
                Ok(())
            }
            "alignment" => {
                self.set_alignment(expect_horizontal_alignment(value)?);
                Ok(())
            }
            "max_visible" => {
                match value {
                    CapabilityValue::Null => self.set_max_visible(None),
                    other => self.set_max_visible(Some(expect_usize(other)?)),
                }
                Ok(())
            }
            "scroll_offset" => {
                self.set_scroll_offset(expect_usize(value)?);
                Ok(())
            }
            "placeholder" => {
                self.set_placeholder(expect_string(value)?);
                Ok(())
            }
            // `text` follows the selection and `item_count` the item list.
            "text" | "item_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `DROPDOWN_PROPERTIES`.
        property_names_of![
            "text",
            "selected_index",
            "item_count",
            "expanded",
            "alignment",
            "max_visible",
            "scroll_offset",
            "placeholder",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `dropdown` publishes.
    ///
    /// `toggle` flips the expanded state through the control's own `toggle()`, so the
    /// flag and the redraw request stay in one place. `set_items`,
    /// `set_selected_index` and `set_expanded` assign state and need a payload, so they
    /// are answered through the property route.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "toggle" => {
                self.toggle();
                Ok(())
            }
            "set_items" | "set_selected_index" | "set_expanded" => {
                Err(CapabilityAccessError::OutOfRange)
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for Dropdown {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button, .. } if *button == 1 => {
                if self.expanded {
                    // Clicked on one of the list items?
                    if let Some(idx) = self.hit_test_item(*pos) {
                        // A disabled row is not a choice: the press is swallowed (the popup stays
                        // open) rather than selecting a row the user cannot have.
                        if self.is_item_enabled(idx) {
                            self.set_selected_index(idx);
                            self.activated.emit(idx);
                            self.set_expanded(false);
                        }
                        return;
                    }

                    // Clicked back on the button header, or outside the control — collapse.
                    self.set_expanded(false);
                } else {
                    // Collapsed: open the list through the same entry point the keyboard uses, so
                    // the popup grab and the marked row are set here too.
                    let geo = self.geometry();
                    if geo.contains_point(*pos) {
                        self.set_expanded(true);
                    }
                }
            }
            // Hovering the open list marks the row under the pointer. This is what makes
            // `highlighted` a live signal rather than only a keyboard echo.
            Event::MouseMove { pos } => {
                if self.expanded {
                    self.set_highlighted(self.hit_test_item(*pos));
                }
            }

            Event::FocusLost => {
                self.set_expanded(false);
            }

            Event::KeyPress { key, modifiers: _ } => {
                if !self.expanded {
                    // Up / Down / Home / End / Enter / Space open the list.
                    if matches!(*key, key_codes::UP | key_codes::DOWN | 36 | 35 | 13 | 32)
                        && !self.items.is_empty()
                    {
                        self.set_expanded(true);
                    }
                    return;
                }

                let page = self.page_size();
                // The open list is a **two-stage** gesture: these keys move the mark, Enter (or a
                // click) commits it, Escape restores the selection the list opened with. Driving
                // the selection directly made every arrow press a value change (I-10).
                match *key {
                    // The arrows, paging and Home/End skip disabled rows: a row that cannot be
                    // chosen is not a navigation target either.
                    38 => {
                        let from = self.marked_row();
                        if let Some(target) = self.neighbour_enabled(from, false) {
                            self.move_highlight(target);
                        }
                    }
                    40 => {
                        let from = self.marked_row();
                        if let Some(target) = self.neighbour_enabled(from, true) {
                            self.move_highlight(target);
                        }
                    }
                    33 => {
                        // Page Up: the nearest enabled row at or before one page up.
                        let from = self.marked_row();
                        let floor = from.saturating_sub(page);
                        if let Some(target) =
                            (floor..from).rev().find(|&index| self.is_item_enabled(index))
                        {
                            self.move_highlight(target);
                        }
                    }
                    34 => {
                        // Page Down: the nearest enabled row at or before one page down.
                        let from = self.marked_row();
                        let ceiling = (from + page).min(self.items.len().saturating_sub(1));
                        if let Some(target) =
                            (from + 1..=ceiling).rev().find(|&index| self.is_item_enabled(index))
                        {
                            self.move_highlight(target);
                        }
                    }
                    36 => {
                        // Home: the first enabled row.
                        if let Some(target) =
                            (0..self.items.len()).find(|&index| self.is_item_enabled(index))
                        {
                            self.move_highlight(target);
                        }
                    }
                    35 => {
                        // End: the last enabled row.
                        if let Some(target) =
                            (0..self.items.len()).rev().find(|&index| self.is_item_enabled(index))
                        {
                            self.move_highlight(target);
                        }
                    }
                    13 => {
                        // Enter commits the marked row, then closes. Activation is what the user
                        // chose; the selection only changes here.
                        let target = self.marked_row();
                        if self.is_item_enabled(target) {
                            self.set_selected_index(target);
                            self.activated.emit(target);
                        }
                        self.set_expanded(false);
                    }
                    27 => {
                        // Escape — cancel: drop the mark and restore the selection the list opened
                        // with, then collapse (I-10).
                        self.type_ahead.clear();
                        self.type_ahead_started = None;
                        if let Some(previous) = self.expanded_selection.take() {
                            self.selected_index = previous;
                        }
                        self.set_expanded(false);
                    }
                    32..=126 => {
                        // First-letter selection, wrapping from the current row.
                        if let Some(ch) = char::from_u32(*key) {
                            // A prefix older than the shared timeout no longer describes what the
                            // user is typing, so it is dropped before this key extends it — the
                            // stale buffer used to intercept the first letter of the next search
                            // forever (I-P06). Clearing queues no frame by itself.
                            let now = Instant::now();
                            let expired = self.type_ahead_started.is_none_or(|started| {
                                now.duration_since(started).as_millis()
                                    > u128::from(
                                        crate::widget::input_widgets::text_match::TYPE_AHEAD_TIMEOUT_MS,
                                    )
                            });
                            if expired {
                                self.type_ahead.clear();
                            }
                            self.type_ahead_started = Some(now);
                            self.type_ahead.push(ch);
                            let start = (self.selected_index + 1).min(self.items.len());
                            if let Some(index) = self.find_prefix(&self.type_ahead, start) {
                                self.move_highlight(index);
                            }
                        }
                    }
                    _ => {}
                }
            }
            Event::Wheel { delta, modifiers } => {
                if self.expanded {
                    let scroll = WheelScroll::from_event(*delta, *modifiers, 1);
                    if scroll.lines_y != 0 {
                        let offset = (self.scroll_offset as i32 + scroll.lines_y).max(0) as usize;
                        self.set_scroll_offset(offset);
                    }
                } else {
                    self.base.handle_event(event);
                }
            }

            _ => {}
        }
    }
}

impl Draw for Dropdown {
    fn draw(&mut self, context: &mut RenderContext) {
        // ── The collapsed field actually painted ──
        //
        // `geometry()` is the area the control was *given*; the **button** part of a dropdown
        // is a text field and a field is a fixed-height band. The expanded list below it is a
        // popup rather than part of the field, so it still hangs from the control's own bottom
        // edge, which is a separate derivation and is left as one.
        let geo = self.field_band();
        if geo.width == 0 || geo.height == 0 {
            return;
        }

        // ── style-derived colours ───────────────────────────────────────
        //
        // # The five literals that used to be here
        //
        // `bg`/`border`/`text_color` already read `style`, but the other five were constants:
        // `placeholder_color`, `highlight_bg`, `highlight_text`, `list_border`, and the list row's
        // own `rgb(248,248,248)`. So the *collapsed field* followed the appearance while everything
        // **inside the popup** did not — the list kept a pale blue highlight and a near-white row on
        // a dark palette, which is exactly the "drawn but unreachable by the theme" shape the census
        // reports. They now read roles:
        //
        //   placeholder  -> the theme's weak ink, so it recedes on both appearances
        //   highlight    -> the accent pair (an emphasised row is a *selection*, not a lighter grey)
        //   list border  -> `outline_variant`, the weak separator, so it is visibly weaker than the
        //                   field's own focus ring
        //   list row     -> `surface_container`, one step above the page, which is what a popup *is*
        //
        // The guard is released before drawing: `theme_manager()` is a non-reentrant mutex and the
        // accessors below take the same one — the rule `slider.rs` documents.
        let (weak_ink, accent, on_accent, separator, popup_surface) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => {
                    let accent = active.colors.primary;
                    (
                        Some(active.colors.secondary),
                        Some(accent),
                        Some(accent.contrast_color()),
                        Some(active.colors.outline_variant),
                        Some(active.colors.surface_container),
                    )
                }
                None => (None, None, None, None, None),
            }
        };
        let bg = self.style().background_color.unwrap_or(Color::rgb(255, 255, 255));
        let border = self.style().border_color.unwrap_or(Color::rgb(180, 180, 180));
        let text_color = self.style().text_color.unwrap_or(Color::rgb(0, 0, 0));
        let placeholder_color = weak_ink.unwrap_or(Color::rgb(160, 160, 160));
        let highlight_bg = accent.unwrap_or(Color::rgb(200, 220, 255));
        // The highlighted row's ink is the accent's contrast colour rather than the field's ink: a
        // selected row is filled *with the accent*, so the ink has to be legible on that, not on the
        // page. Reading `text_color` here was legible only while the fill happened to be pale.
        let highlight_text = on_accent.unwrap_or(Color::rgb(0, 0, 0));
        let list_border = separator.unwrap_or(Color::rgb(150, 150, 150));
        let list_row = popup_surface.unwrap_or(Color::rgb(248, 248, 248));

        // ── Collapsed / button area ─────────────────────────────────────
        // Background
        context.fill_rect(geo, bg);
        // Border
        context.draw_rect(geo, border);

        // The indicator's cell and the label's box come from one derivation, so the selection
        // yields to the indicator instead of being written at an unconstrained offset.
        // Both labels on the collapsed row sit on the field's own line box: a glyph origin is
        // the box's top-left edge, so the old `geo.y + geo.height / 2` put that edge on the
        // field's middle line and drew the value and the indicator half a line low.
        let font = effective_font(self.style());
        let field_line = context.text_line(geo, font);
        let geometry = self.field_geometry(field_line.height);
        let label_line = context.text_line(geometry.label_box, font);

        let (label, color) = match self.selected_text() {
            Some(text) => (text, text_color),
            None => (self.placeholder.as_str(), placeholder_color),
        };
        // Fitted, so a selection longer than the field is elided rather than drawn under the
        // indicator — the box it is given is the one the indicator left.
        context.draw_text_fitted_cow(
            Rect::new(
                geometry.label_box.x,
                label_line.y,
                geometry.label_box.width,
                label_line.height,
            ),
            label,
            font,
            color,
            self.alignment.to_horizontal().unwrap_or(HorizontalAlignment::Left),
        );
        // The disclosure arrow is the `ChevronDown` **icon outline**, not the `▼` text glyph: no
        // bundled face covers U+25BC, so the glyph fell back to an 8x8 bitmap block. The icon is
        // centred in the indicator box the field geometry reserved for it.
        crate::widget::draw_icon_centered(
            context,
            geometry.indicator_box,
            geometry.indicator_box.height.min(geometry.indicator_box.width),
            color,
            IconName::ChevronDown,
        );

        // ── Expanded list ───────────────────────────────────────────────
        if !self.expanded || self.items.is_empty() {
            return;
        }

        let (first, last) = self.visible_range();
        for i in first..last {
            let item_geo = self.item_rect(i);
            let row = &self.items[i];

            // Background. The selected row is the committed choice; the marked row (the one the
            // pointer or keyboard is on) gets a lighter wash, so `highlighted` is visible rather
            // than only observable.
            let is_selected = i == self.selected_index;
            let is_highlighted = self.highlighted_index == Some(i);
            if is_selected {
                context.fill_rect(item_geo, highlight_bg);
            } else if is_highlighted {
                context.fill_rect(item_geo, list_row.blend(&highlight_bg, 0.35));
            } else {
                context.fill_rect(item_geo, list_row);
            }

            // Border (bottom line)
            context.draw_rect(item_geo, list_border);

            // Item text. A disabled row uses the weak ink: it is present and readable, but it
            // cannot be chosen, so it must not read as an available option.
            let item_color = if !row.enabled {
                placeholder_color
            } else if is_selected {
                highlight_text
            } else {
                text_color
            };
            let item_x = item_geo.x + PADDING;
            // The row's own line box, for the same reason as the collapsed field above: a
            // halved height here places the glyph box's top edge on the row's middle line.
            let item_line = context.text_line(item_geo, font);
            // A row with an icon reserves one fixed cell for it, so the label of every row starts
            // at the same offset and showing a row costs no per-frame text measurement.
            let icon_cell = row.icon.is_some() as u32 * ITEM_ICON_WIDTH;
            if let Some(icon) = row.icon() {
                context.draw_text(
                    Point::new(item_x, item_line.y),
                    icon,
                    font,
                    item_color,
                    HorizontalAlignment::Left,
                );
            }
            // Fitted to the row: the collapsed value was fitted but the expanded rows were not,
            // so a long item label painted past the popup (which is not clipped).
            context.draw_text_fitted_cow(
                Rect::new(
                    item_x + icon_cell as i32,
                    item_line.y,
                    item_geo.width.saturating_sub(PADDING as u32 * 2 + icon_cell),
                    item_line.height.max(1),
                ),
                &row.text,
                font,
                item_color,
                HorizontalAlignment::Left,
            );
        }

        // A capped popup shows a thin thumb, so a window of a longer list cannot look like
        // the whole list.
        if self.max_visible.is_some() && self.items.len() > self.page_size() {
            let page = self.page_size();
            let track_top = self.item_rect(first).y;
            let track_height = (page * ITEM_HEIGHT as usize) as i32;
            let thumb_height =
                ((track_height as usize * page) / self.items.len().max(1)).max(8) as i32;
            let max_offset = self.items.len().saturating_sub(page).max(1);
            let thumb_top = track_top
                + ((track_height - thumb_height) * self.scroll_offset as i32 / max_offset as i32);
            context.fill_rect(
                Rect::new(geo.x + geo.width as i32 - 4, thumb_top, 3, thumb_height as u32),
                list_border,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;

    fn make_dropdown() -> Dropdown {
        Dropdown::new(
            vec!["Option A".to_string(), "Option B".to_string(), "Option C".to_string()],
            Rect::new(10, 10, 160, 24),
        )
    }

    #[test]
    fn dropdown_creation_defaults() {
        let dd = make_dropdown();
        assert_eq!(dd.items().len(), 3);
        assert_eq!(dd.items()[0], "Option A");
        assert_eq!(dd.items()[1], "Option B");
        assert_eq!(dd.items()[2], "Option C");
        assert_eq!(dd.selected_index().unwrap_or(0), 0);
        assert_eq!(dd.selected_text(), Some("Option A"));
        assert!(!dd.is_expanded());
    }

    #[test]
    fn dropdown_set_items() {
        let mut dd = make_dropdown();
        dd.set_items(vec!["X".to_string(), "Y".to_string()]);
        assert_eq!(dd.items().len(), 2);
        assert_eq!(dd.items()[0], "X");
        assert_eq!(dd.items()[1], "Y");
        assert_eq!(dd.selected_index().unwrap_or(0), 0);
    }

    #[test]
    fn dropdown_set_items_empty_resets_selection() {
        let mut dd = make_dropdown();
        dd.set_selected_index(2);
        dd.set_items(Vec::new());
        assert!(dd.items().is_empty());
        assert_eq!(dd.selected_index().unwrap_or(0), 0);
        assert!(dd.selected_text().is_none());
    }

    #[test]
    fn dropdown_select_item() {
        let mut dd = make_dropdown();
        dd.set_selected_index(1);
        assert_eq!(dd.selected_index().unwrap_or(0), 1);
        assert_eq!(dd.selected_text(), Some("Option B"));
    }

    #[test]
    fn dropdown_select_item_clamps_out_of_range() {
        let mut dd = make_dropdown();
        dd.set_selected_index(100);
        assert_eq!(dd.selected_index().unwrap_or(0), 2);
        assert_eq!(dd.selected_text(), Some("Option C"));
    }

    #[test]
    fn dropdown_toggle_expand() {
        let mut dd = make_dropdown();
        assert!(!dd.is_expanded());
        dd.toggle();
        assert!(dd.is_expanded());
        dd.toggle();
        assert!(!dd.is_expanded());
    }

    #[test]
    fn dropdown_set_expanded() {
        let mut dd = make_dropdown();
        dd.set_expanded(true);
        assert!(dd.is_expanded());
        dd.set_expanded(false);
        assert!(!dd.is_expanded());
    }

    #[test]
    fn dropdown_focus_lost_collapses() {
        let mut dd = make_dropdown();
        dd.set_expanded(true);
        dd.handle_event(&Event::FocusLost);
        assert!(!dd.is_expanded());
    }

    #[test]
    fn dropdown_mouse_press_toggles() {
        let mut dd = make_dropdown();
        // Click inside the widget geometry to expand
        dd.handle_event(&Event::mouse_press(20, 20, 1));
        assert!(dd.is_expanded());

        // Click inside the widget geometry again to collapse
        dd.handle_event(&Event::mouse_press(20, 20, 1));
        assert!(!dd.is_expanded());
    }

    #[test]
    fn dropdown_mouse_press_outside_collapses_when_expanded() {
        let mut dd = make_dropdown();
        dd.set_expanded(true);
        // Click far outside
        dd.handle_event(&Event::mouse_press(999, 999, 1));
        assert!(!dd.is_expanded());
    }

    #[test]
    fn dropdown_mouse_select_item() {
        let mut dd = make_dropdown();
        dd.set_expanded(true);

        // The first item is at y = 10 + 24 + 0*20 = 34
        dd.handle_event(&Event::mouse_press(15, 35, 1));
        assert_eq!(dd.selected_index().unwrap_or(0), 0);
        assert!(!dd.is_expanded()); // selection collapses

        // Expand again and select the 3rd item (index 2)
        dd.set_expanded(true);
        dd.handle_event(&Event::mouse_press(15, 35 + 2 * 20, 1));
        assert_eq!(dd.selected_index().unwrap_or(0), 2);
        assert!(!dd.is_expanded());
    }

    #[test]
    fn dropdown_keyboard_navigation() {
        let mut dd = make_dropdown();
        // Down arrow opens the expanded list
        dd.handle_event(&Event::key_press(40, 0));
        assert!(dd.is_expanded());

        // The open list is two-stage: an arrow moves the **mark**, not the committed selection
        // (I-10).
        dd.handle_event(&Event::key_press(40, 0));
        assert_eq!(dd.highlighted_index(), Some(1));
        assert_eq!(dd.selected_index().unwrap_or(0), 0);

        dd.handle_event(&Event::key_press(40, 0));
        assert_eq!(dd.highlighted_index(), Some(2));

        // Already at last — should stay
        dd.handle_event(&Event::key_press(40, 0));
        assert_eq!(dd.highlighted_index(), Some(2));

        // Up arrow
        dd.handle_event(&Event::key_press(38, 0));
        assert_eq!(dd.highlighted_index(), Some(1));

        // Enter commits the marked row and collapses
        dd.handle_event(&Event::key_press(13, 0));
        assert_eq!(dd.selected_index(), Some(1));
        assert!(!dd.is_expanded());
    }

    #[test]
    fn escape_restores_the_selection_the_list_opened_with() {
        let mut dd = make_dropdown();
        dd.handle_event(&Event::key_press(40, 0));
        dd.handle_event(&Event::key_press(40, 0));
        dd.handle_event(&Event::key_press(40, 0));
        assert_eq!(dd.highlighted_index(), Some(2));
        assert!(!dd.is_expanded() || dd.selected_index() == Some(0));

        dd.handle_event(&Event::key_press(27, 0));
        assert!(!dd.is_expanded());
        assert_eq!(dd.selected_index(), Some(0));
        assert_eq!(dd.highlighted_index(), None);
    }

    #[test]
    fn the_placeholder_is_configurable() {
        let mut dd = Dropdown::new(Vec::new(), Rect::new(0, 0, 200, 30));
        assert_eq!(dd.placeholder(), "(Select)");
        dd.set_placeholder("Pick a city");
        assert_eq!(dd.placeholder(), "Pick a city");
    }

    #[test]
    fn dropdown_escape_collapses() {
        let mut dd = make_dropdown();
        dd.set_expanded(true);
        dd.handle_event(&Event::key_press(27, 0));
        assert!(!dd.is_expanded());
    }

    #[test]
    fn dropdown_disabled_ignores_events() {
        let mut dd = make_dropdown();
        dd.set_expanded(false);
        dd.set_enabled(false);
        dd.handle_event(&Event::mouse_press(20, 20, 1));
        assert!(!dd.is_expanded(), "disabled widget should not expand");
    }

    #[test]
    fn dropdown_draw_does_not_panic() {
        // Minimal smoke test — we only verify that the draw method runs
        // without panicking. A real render backend would be needed for
        // meaningful pixel tests.
        let mut dd = make_dropdown();

        // Create a minimal software render backend for testing
        use crate::render::PaintBackend;
        let mut backend =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(200, 200), 1.0);
        backend.begin_frame(crate::core::Color::WHITE);
        let mut ctx = crate::render::RenderContext::new(&mut backend);

        // Collapsed draw
        dd.draw(&mut ctx);

        // Expanded draw
        dd.set_expanded(true);
        dd.draw(&mut ctx);
    }

    #[test]
    fn dropdown_geometry_delegation() {
        let mut dd = make_dropdown();
        dd.set_geometry(Rect::new(0, 0, 200, 30));
        assert_eq!(dd.geometry(), Rect::new(0, 0, 200, 30));
    }

    #[test]
    fn dropdown_visibility() {
        let mut dd = make_dropdown();
        assert!(dd.is_visible());
        dd.hide();
        assert!(!dd.is_visible());
        dd.show();
        assert!(dd.is_visible());
    }

    #[test]
    fn dropdown_enabled() {
        let mut dd = make_dropdown();
        assert!(dd.is_enabled());
        dd.set_enabled(false);
        assert!(!dd.is_enabled());
        dd.set_enabled(true);
        assert!(dd.is_enabled());
    }

    #[test]
    fn dropdown_id_kind() {
        let dd_a = make_dropdown();
        let dd_b = make_dropdown();
        assert_ne!(dd_a.id(), dd_b.id());
        assert_eq!(dd_a.kind(), WidgetKind::Dropdown);
        assert_eq!(dd_b.kind(), WidgetKind::Dropdown);
    }

    #[test]
    fn dropdown_signal_accessors() {
        let dd = make_dropdown();
        let _ = &dd.changed;
        let _ = dd.changed_signal();
        let _ = dd.clicked_signal();
    }

    #[test]
    fn dropdown_item_rect_computation() {
        let dd = make_dropdown(); // Rect(10, 10, 160, 24)
        let r = dd.item_rect(0);
        assert_eq!(r, Rect::new(10, 34, 160, 20));
        let r = dd.item_rect(2);
        assert_eq!(r, Rect::new(10, 74, 160, 20));
    }

    #[test]
    fn dropdown_selected_text_none_when_empty() {
        let dd = Dropdown::new(Vec::new(), Rect::new(0, 0, 100, 24));
        assert!(dd.selected_text().is_none());
    }

    /// The label's box ends where the indicator's cell begins, at every control width.
    ///
    /// # What this pins
    ///
    /// BLUE22 §B.6 rule 4: a sub-part's box is derived from its sibling, so the selection
    /// *yields* to the indicator. The label used to be written at `geo.x + PADDING` with no
    /// upper bound while the indicator sat at `geo.x + geo.width - PADDING - 12`, so a long
    /// selection ran underneath the arrow.
    #[test]
    fn the_label_box_ends_where_the_indicator_begins() {
        let trailing = PADDING.max(0) as u32
            + INDICATOR_WIDTH.max(0) as u32
            + INDICATOR_LEADING_GAP.max(0) as u32;
        for width in [0u32, 20, 64, 240, 400] {
            let dd = Dropdown::new(vec!["Sample".to_string()], Rect::new(0, 0, width, 120));
            let geometry = dd.field_geometry(14);
            let band = dd.field_band();
            assert_eq!(
                geometry.label_box.x + geometry.label_box.width as i32 + INDICATOR_LEADING_GAP,
                geometry.indicator_box.x,
                "the label must stop one gap short of the indicator at width {width}"
            );
            assert!(
                geometry.indicator_box.x + geometry.indicator_box.width as i32
                    <= band.x + band.width as i32,
                "the indicator must stay inside the band at width {width}"
            );
            if width < trailing {
                assert_eq!(
                    geometry.label_box.width, 0,
                    "a band too narrow for its own chrome holds no label at width {width}"
                );
            }
        }
    }

    /// The reported height is the band that is painted.
    #[test]
    fn the_reported_height_is_the_band_that_is_painted() {
        let dd = Dropdown::new(Vec::new(), Rect::new(0, 0, 240, 120));
        let band = dd.field_band();
        assert_eq!(band.height, dimensions::TEXT_FIELD_MIN_HEIGHT);
        assert_eq!(dd.size_hint().height, band.height);
        assert_eq!(band.y, (120 - dimensions::TEXT_FIELD_MIN_HEIGHT as i32) / 2);
    }

    /// The **expanded list** follows the appearance, not just the collapsed field.
    ///
    /// # The defect this pins
    ///
    /// `bg` / `border` / `text_color` read `style`, but the five colours *inside the popup* were
    /// constants: a pale-blue highlight, a near-white row, a grey placeholder and two greys for the
    /// list's border. So the field tracked the theme while the thing it opened did not — a dark
    /// build showed a near-white list under a dark field.
    ///
    /// # Why the assertion is about one specific row, and not the whole document
    ///
    /// A first version compared the *set of every fill* in the two documents. It passed even with
    /// the literals restored — measured — because the field's own background still differs between
    /// appearances, so a document-wide comparison is satisfied by a colour this test is not about.
    /// The assertion has to name the element: the popup's **first unselected row** is the
    /// `surface_container` fill, and that is the one that must move.
    #[test]
    #[cfg(device_profile)]
    fn the_expanded_list_follows_the_appearance() {
        let _guard = crate::style::theme_test_guard();
        crate::widget::census::install_preset_appearances();
        let rect = Rect::new(0, 0, 200, 60);

        let sample = |appearance| -> (String, String) {
            crate::theme::global_theme_manager().set_appearance(appearance);
            let mut dd = Dropdown::new(vec!["One".to_string(), "Two".to_string()], rect);
            dd.set_expanded(true);
            crate::theme::apply_theme_to_widget(&mut dd);
            let backdrop = crate::style::theme_manager()
                .current_theme()
                .map(|active| active.colors.background)
                .expect("a preset is active");
            let svg = crate::widget::svg::render_widget_to_svg_on(&mut dd, rect, backdrop);
            // The rows live below the field's own band, so a fill whose `y` is past it belongs to
            // the popup. Reading the geometry rather than counting elements is what keeps this from
            // depending on how many surfaces the field happens to paint.
            let field_bottom = crate::widget::metrics::dimensions::TEXT_FIELD_MIN_HEIGHT as i32;
            let rows = row_fill(&svg, field_bottom);
            (rows[0].clone(), rows[1].clone())
        };

        // `sample` returns (first row, second row). The first row is the **selected** one — a fresh
        // dropdown with items selects index 0 — so the pair is (selected, unselected) rather than
        // the other way round. Reading them by position is what the helper documents; getting the
        // order wrong is what the first draft of this test did, and what the panic message made
        // obvious.
        let (dark_sel, dark_row) = sample(crate::theme::AppearanceMode::Dark);
        let (light_sel, light_row) = sample(crate::theme::AppearanceMode::Light);
        assert_ne!(
            dark_row, light_row,
            "an unselected popup row must follow the appearance; both were {dark_row}"
        );
        assert_ne!(
            dark_sel, light_sel,
            "a selected popup row must follow the appearance; both were {dark_sel}"
        );
        // And the highlighted row must differ from the plain one in each appearance, or the
        // selection is not visible at all.
        assert_ne!(dark_sel, dark_row, "the selected row must stand out on the dark appearance");
        assert_ne!(light_sel, light_row, "the selected row must stand out on the light one");
    }

    /// The fills of the first two popup rows below `field_bottom`, in document order.
    ///
    /// Rows are emitted in order and the first one is the configured selection, so the pair is
    /// `(selected, unselected)`. Walking the rects rather than guessing an index is what keeps this
    /// independent of how many surfaces the collapsed field paints — the count differs between the
    /// two appearances, which is exactly the kind of thing an index would silently track.
    #[cfg(device_profile)]
    fn row_fill(svg: &str, field_bottom: i32) -> Vec<String> {
        let mut rows = Vec::new();
        let mut rest = svg;
        while let Some(rect_at) = rest.find("<rect ") {
            let rect = &rest[rect_at..];
            let end = rect.find("/>").map(|e| e + 2).unwrap_or(rect.len());
            let element = &rect[..end];
            let attr = |name: &str| -> Option<i32> {
                let key = format!(" {name}=\"");
                let at = element.find(&key)? + key.len();
                let to = element[at..].find('"')? + at;
                element[at..to].parse().ok()
            };
            if let (Some(y), Some(key)) =
                (attr("y"), element.find("fill=\"rgba(").map(|i| i + "fill=\"rgba(".len()))
            {
                if y >= field_bottom {
                    let to = element[key..].find(')').map(|e| e + key).unwrap_or(key);
                    rows.push(element[key..to].to_string());
                    if rows.len() == 2 {
                        return rows;
                    }
                }
            }
            rest = &rect[end..];
        }
        panic!("fewer than two popup rows at or below y={field_bottom}");
    }

    /// A capped popup scrolls with the wheel and pages with the keyboard.
    #[test]
    fn a_capped_list_scrolls_and_pages() {
        let items: Vec<String> = (0..10).map(|index| format!("Item {index}")).collect();
        let mut dropdown = Dropdown::new(items, Rect::new(0, 0, 160, 24));
        dropdown.set_max_visible(Some(3));
        assert_eq!(dropdown.max_visible(), Some(3));
        dropdown.set_expanded(true);
        dropdown.set_selected_index(0);

        dropdown.handle_event(&Event::Wheel { delta: Point::new(0, 1), modifiers: 0 });
        assert_eq!(dropdown.scroll_offset(), 1, "a notch scrolls the popup");

        // The open list is two-stage (I-10): paging and End move the **mark**, not the value.
        dropdown.handle_event(&Event::KeyPress { key: 34, modifiers: 0 }); // PageDown
        assert!(dropdown.highlighted_index().unwrap() >= 3);
        assert!(dropdown.scroll_offset() >= 1);
        assert_eq!(dropdown.selected_index(), Some(0), "nothing is committed by navigating");

        dropdown.handle_event(&Event::KeyPress { key: 35, modifiers: 0 }); // End
        assert_eq!(dropdown.highlighted_index(), Some(9));
        assert_eq!(dropdown.scroll_offset(), 7, "the mark is kept visible");

        // Type-ahead wraps and marks the first matching label.
        dropdown.handle_event(&Event::KeyPress { key: 73, modifiers: 0 }); // 'I'
        dropdown.handle_event(&Event::KeyPress { key: 116, modifiers: 0 }); // 't'
        assert_eq!(dropdown.highlighted_index(), Some(1));

        // Enter is what commits the mark.
        dropdown.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
        assert_eq!(dropdown.selected_text(), Some("Item 1"));
    }

    /// The row cap and the scroll offset are published and writable.
    #[test]
    fn the_popup_window_is_published() {
        let items: Vec<String> = (0..8).map(|index| format!("Item {index}")).collect();
        let mut dropdown = Dropdown::new(items, Rect::new(0, 0, 160, 24));
        assert_eq!(dropdown.get("max_visible").unwrap(), CapabilityValue::Null);
        dropdown.set("max_visible", CapabilityValue::UInt(3)).unwrap();
        assert_eq!(dropdown.max_visible(), Some(3));
        dropdown.set("max_visible", CapabilityValue::Null).unwrap();
        assert_eq!(dropdown.max_visible(), None);

        dropdown.set("max_visible", CapabilityValue::UInt(2)).unwrap();
        dropdown.set("scroll_offset", CapabilityValue::UInt(1)).unwrap();
        assert_eq!(dropdown.get("scroll_offset").unwrap(), CapabilityValue::UInt(1));
        // The offset is clamped to what the window can show rather than stored raw.
        dropdown.set("scroll_offset", CapabilityValue::UInt(99)).unwrap();
        assert_eq!(dropdown.scroll_offset(), 6);
    }
}
