// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `SuggestionPopup` — the reusable candidate list the input family hangs under a field
//! (COMP-B01, the single home for COMP-C01 as well).
//!
//! # Why one popup instead of three
//!
//! Three controls in this family need to show candidates: `combo_box`'s completer had nowhere to
//! paint them at all, while `auto_complete_edit` and `editable_combo_box` each implemented an
//! inline popup — with different row heights (24 px vs 28 px) and slightly different key
//! behaviour. This companion is that list, once: same rows, same two-stage navigation, same empty
//! state.
//!
//! # Two-stage navigation
//!
//! Arrow keys move the **highlight**; Enter or a click commits it; Escape dismisses. That is the
//! contract `dropdown` now follows too, so browsing a list never changes a value by accident.
//!
//! The control is mounted beside the field it serves — it is not in the capability table, like the
//! other companions — and is told where to hang by [`SuggestionPopup::set_anchor`].
//!
//! # The COMP-C01 capabilities live here too
//!
//! `input_companions` originally carried a second `SuggestionPopup` (COMP-C01) with filtering,
//! flipping, a scroll window and outside dismissal — two implementations of one concept, which
//! principle #101 forbids. The two are merged here, per principle #101: this file is the one
//! implementation, and `input_companions` re-exports [`SuggestionPopup`], [`PopupItem`] and
//! [`MatchMode`] so COMP-C01's documented path still resolves.
//!
//! One consequence is worth stating because it is what keeps the two halves from drifting again:
//! the filtered rows, the painted window and the hit test all read the same facts —
//! [`SuggestionPopup::visible_count`], `scroll_offset`, `window_count`, `row_rect` and
//! `row_at` — so a row cannot be painted at one place and hit-tested at another. The two signal
//! vocabularies are raised from one path as well: [`SuggestionPopup::move_highlight`] (and
//! `set_selected_index` / `highlight_at`) emits both `suggestion_highlighted` (COMP-B01) and
//! `highlight_changed` (COMP-C01), and the commit path emits `suggestion_committed` plus
//! `item_selected`.

use crate::compat::{String, ToString, Vec};
use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{EventSignalRef, GenericSignal, Signal1};
use crate::widget::capability::coercion::{
    expect_bool, expect_string, expect_u32, expect_usize, normalize_key,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::input_widgets::combobox::PopupWindow;
use crate::widget::input_widgets::popup_support::{
    expect_rect, expect_strings, rect_value, row_height_for, strings_value,
};
use crate::widget::input_widgets::text_match;
use crate::widget::metrics::{effective_font, estimate_text_width};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// The widest number of rows an unconfigured popup shows at once.
const DEFAULT_MAX_VISIBLE: usize = 8;
/// Horizontal inset inside a row.
const ROW_INSET: i32 = 8;
/// Space left between a row's label and its trailing hint.
const ROW_GAP: u32 = 6;

/// One row a [`SuggestionPopup`] can offer (COMP-C01's row description).
///
/// The popup's own row list is the label list [`SuggestionPopup::items`] returns; a `PopupItem`
/// is what a host hands in through [`SuggestionPopup::set_popup_items`] and what
/// [`SuggestionPopup::item_selected`] reports back, so the two views cannot disagree about a row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PopupItem {
    /// The primary label.
    pub label: String,
    /// Optional leading icon name, carried for hosts that paint icons beside the row.
    pub icon: Option<String>,
    /// Whether the row can be chosen.
    pub enabled: bool,
    /// Secondary hint shown beside the label.
    pub hint: String,
}

impl PopupItem {
    /// Creates an enabled item with no icon or hint.
    pub fn new(label: impl Into<String>) -> Self {
        Self { label: label.into(), icon: None, enabled: true, hint: String::new() }
    }

    /// Adds a secondary hint.
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = hint.into();
        self
    }

    /// Adds a leading icon name.
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }
}

/// How [`SuggestionPopup::apply_filter`] matches the current filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MatchMode {
    /// A row matches when its label starts with the filter (case-insensitive).
    #[default]
    Prefix,
    /// A row matches when its label contains the filter (case-insensitive).
    Contains,
}

impl MatchMode {
    /// The property representation of this mode.
    fn as_name(self) -> &'static str {
        match self {
            Self::Prefix => "prefix",
            Self::Contains => "contains",
        }
    }
}

/// The parts of a [`PopupItem`] that are not its label, index-aligned with the label rows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct RowDetail {
    /// The leading icon name, if any.
    icon: Option<String>,
    /// The trailing hint, empty when there is none.
    hint: String,
    /// Whether the row refuses to be chosen; the inverse of [`PopupItem::enabled`].
    disabled: bool,
}

impl RowDetail {
    /// Drops a row description down to the parts the label list does not already carry.
    fn from_item(item: &PopupItem) -> Self {
        Self { icon: item.icon.clone(), hint: item.hint.clone(), disabled: !item.enabled }
    }
}

/// A candidate list anchored under an input field.
///
/// The popup owns its rows, filters them with [`Self::apply_filter`], and keeps the highlight
/// inside the painted window as the rows scroll. The host places it against a field band with
/// [`Self::set_anchor`], tells it how much screen is available with [`Self::set_viewport`], and
/// listens to `suggestion_committed` / `item_selected` / `dismissed`.
pub struct SuggestionPopup {
    base: BaseWidget,
    /// The candidate labels; the canonical row list and the view COMP-B01 publishes.
    items: Vec<String>,
    /// Index-aligned with `items`: the icon, hint and enabled state each row carries.
    details: Vec<RowDetail>,
    /// Item indices that pass the filter, in row order. Rebuilt by `refresh_visible`.
    visible: Vec<usize>,
    /// The field band this popup hangs from, or `None` to paint at its own geometry.
    anchor: Option<Rect>,
    /// The host's available area, which is what makes flipping above the anchor measurable.
    viewport: Option<Rect>,
    /// Whether the popup may flip above the anchor when the viewport has no room below it.
    flip_allowed: bool,
    /// The painted row window: row cap and first painted row.
    ///
    /// This is the same kernel a combo box and a dropdown use for their own lists
    /// ([`PopupWindow`]), so "which rows are painted and how they scroll" is answered once.
    window: PopupWindow,
    /// A caller-chosen row height, or `None` to derive it from the font.
    row_height: Option<u32>,
    selected_index: Option<usize>,
    filter: String,
    match_mode: MatchMode,
    empty_text: String,
    alignment: HorizontalAlignment,
    shown: bool,
    /// Emitted when the highlight moves, with the row index (COMP-B01).
    pub suggestion_highlighted: Signal1<usize>,
    /// Emitted when the highlighted row is taken by Enter or a click (COMP-B01).
    pub suggestion_committed: Signal1<usize>,
    /// Emitted with the new highlight index (COMP-C01; the same move as `suggestion_highlighted`).
    pub highlight_changed: Signal1<usize>,
    /// Emitted with the chosen row (COMP-C01; the same commit as `suggestion_committed`).
    pub item_selected: Signal1<PopupItem>,
    /// Emitted when the popup dismisses without committing (Escape, outside press, focus loss).
    pub dismissed: GenericSignal,
}

impl SuggestionPopup {
    /// Creates a hidden popup with no candidates.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Panel, geometry, "SuggestionPopup"),
            items: Vec::new(),
            details: Vec::new(),
            visible: Vec::new(),
            anchor: None,
            viewport: None,
            flip_allowed: true,
            window: {
                let mut window = PopupWindow::new();
                window.set_max_visible(Some(DEFAULT_MAX_VISIBLE));
                window
            },
            row_height: None,
            selected_index: None,
            filter: String::new(),
            match_mode: MatchMode::Prefix,
            empty_text: String::from("(No matches)"),
            alignment: HorizontalAlignment::Left,
            shown: false,
            suggestion_highlighted: Signal1::new(),
            suggestion_committed: Signal1::new(),
            highlight_changed: Signal1::new(),
            item_selected: Signal1::new(),
            dismissed: GenericSignal::new(),
        }
    }

    /// Replaces the candidate list with plain labels.
    ///
    /// The highlight is clamped into the new list rather than kept by value: row 5 of the old
    /// list is a different row of the new one.
    pub fn set_items(&mut self, items: Vec<String>) {
        // One row-setter path: the label-only form is the rich form with default decorations, so
        // the two setters cannot drift in how they clamp the highlight or rebuild the filter.
        self.set_popup_items(items.into_iter().map(PopupItem::new).collect());
    }

    /// Replaces the candidate list with described rows ([`PopupItem`]).
    pub fn set_popup_items(&mut self, items: Vec<PopupItem>) {
        self.items = items.iter().map(|item| item.label.clone()).collect();
        self.details = items.iter().map(RowDetail::from_item).collect();
        self.refresh_visible();
        self.base.request_redraw();
    }

    /// The candidate labels.
    pub fn items(&self) -> &[String] {
        &self.items
    }

    /// Anchors the popup under `anchor`, the field band it serves.
    pub fn set_anchor(&mut self, anchor: Rect) {
        if self.anchor == Some(anchor) {
            return;
        }
        self.anchor = Some(anchor);
        self.base.request_redraw();
    }

    /// The field band the popup hangs from, if one was set.
    pub fn anchor(&self) -> Option<Rect> {
        self.anchor
    }

    /// Supplies the host's available area, which is what makes flipping measurable.
    pub fn set_viewport(&mut self, viewport: Option<Rect>) {
        if self.viewport == viewport {
            return;
        }
        self.viewport = viewport;
        self.base.request_redraw();
    }

    /// The host's available area, if one was supplied.
    pub fn viewport(&self) -> Option<Rect> {
        self.viewport
    }

    /// Allows or forbids flipping the popup above the anchor.
    pub fn set_flip_allowed(&mut self, allowed: bool) {
        if self.flip_allowed == allowed {
            return;
        }
        self.flip_allowed = allowed;
        self.base.request_redraw();
    }

    /// Whether flipping above the anchor is allowed.
    pub fn flip_allowed(&self) -> bool {
        self.flip_allowed
    }

    /// Caps how many rows are painted at once.
    pub fn set_max_visible(&mut self, max_visible: usize) {
        let max_visible = max_visible.max(1);
        if self.max_visible() == max_visible {
            return;
        }
        self.window.set_max_visible(Some(max_visible));
        self.scroll_to_highlight();
        self.base.request_redraw();
    }

    /// How many rows are painted at once.
    pub fn max_visible(&self) -> usize {
        self.window.max_visible().unwrap_or(DEFAULT_MAX_VISIBLE)
    }

    /// Sets the row cap and the row height together.
    pub fn set_metrics(&mut self, max_visible: usize, row_height: u32) {
        self.window.set_max_visible(Some(max_visible.max(1)));
        self.row_height = Some(row_height.max(1));
        self.scroll_to_highlight();
        self.base.request_redraw();
    }

    /// Highlights `index`, or clears the highlight for a row that does not exist.
    pub fn set_selected_index(&mut self, index: Option<usize>) {
        self.set_highlight(index);
    }

    /// The highlighted row, if any.
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    /// Places the highlight on `index` within the rows that pass the filter.
    pub fn highlight_at(&mut self, index: usize) {
        self.set_highlight(Some(index));
    }

    /// The filter string the rows are matched against.
    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// Applies a filter string, re-derives the visible rows and highlights the first match.
    pub fn apply_filter(&mut self, text: &str) {
        if self.filter == text {
            return;
        }
        self.filter = text.to_string();
        self.refresh_visible();
        self.selected_index = self.visible.first().map(|_| 0);
        self.window.set_offset(0, self.visible.len());
        self.scroll_to_highlight();
        self.base.request_redraw();
    }

    /// How [`Self::apply_filter`] matches a row.
    pub fn match_mode(&self) -> MatchMode {
        self.match_mode
    }

    /// Sets how [`Self::apply_filter`] matches a row.
    pub fn set_match_mode(&mut self, mode: MatchMode) {
        if self.match_mode == mode {
            return;
        }
        self.match_mode = mode;
        self.refresh_visible();
        self.base.request_redraw();
    }

    /// The rows that pass the current filter, in row order.
    pub fn visible_items(&self) -> Vec<PopupItem> {
        self.visible.iter().filter_map(|index| self.item(*index)).collect()
    }

    /// How many rows pass the current filter.
    pub fn visible_count(&self) -> usize {
        self.visible.len()
    }

    /// The first painted row within the filtered rows.
    pub fn scroll_offset(&self) -> usize {
        self.window.offset()
    }

    /// The candidate indices that pass the filter, in painted row order.
    ///
    /// A hosting control keeps its own candidate source (a combo box's items or completer rows,
    /// a tag field's suggestions), so it needs the original index of each painted row rather than
    /// a copy of the labels. Exposing the map is what lets the host paint from its own storage
    /// while the filter and window stay here (I-129).
    // `widgets_unstripped`, not `full_widgets`: the caller is `editable_combo_box`, which is
    // compiled whenever the widget set is unstripped. A device profile is not part of what this
    // method needs — it only reads the filtered row map.
    #[cfg(widgets_unstripped)]
    pub(crate) fn visible_indices(&self) -> &[usize] {
        &self.visible
    }

    /// How many rows the painted window holds.
    ///
    /// This is the row count [`Self::row_rect`], [`Self::draw`] and the hit test share; a host
    /// that paints with its own colours still asks the popup which rows are in the window.
    pub(crate) fn window_rows(&self) -> usize {
        self.window_count()
    }

    /// Sets the first painted row from outside the popup's own navigation.
    ///
    /// A hosting combo box opens its list centred on the selected row, which is a window rule the
    /// popup's two-stage navigation does not need. The offset is still clamped to the rows that
    /// exist, so a host cannot scroll the window past its own end.
    pub(crate) fn set_scroll_offset(&mut self, offset: usize) {
        if self.window.set_offset(offset, self.visible.len()) {
            self.base.request_redraw();
        }
    }

    /// Scrolls the painted window the minimum amount that shows `row`.
    ///
    /// The same rule [`Self::scroll_to_highlight`] applies, exposed for hosts that own the
    /// highlight in their own model (an editable combo box keeps an item index, not a row index).
    pub(crate) fn scroll_into_view(&mut self, row: usize) {
        let before = self.window.offset();
        self.window.ensure_visible(row, self.visible.len());
        if self.window.offset() != before {
            self.base.request_redraw();
        }
    }

    /// The painted row under `pos`, whether or not this widget is the shown popup.
    ///
    /// A hosting control owns the open flag and the popup grab (the popup is mounted beside the
    /// field rather than replacing its event handling), so it needs the row mapping without
    /// [`Self::show`] being called on an unregistered widget.
    pub(crate) fn window_row_at(&self, pos: Point) -> Option<usize> {
        let popup = self.popup_rect();
        if !popup.contains_point(pos) {
            return None;
        }
        let row_height = self.row_height().max(1) as i32;
        let window_row = ((pos.y - popup.y) / row_height) as usize;
        if window_row >= self.window_count() {
            return None;
        }
        let row = self.window.offset() + window_row;
        (row < self.visible.len()).then_some(row)
    }

    /// Moves the highlight by `delta` rows within the filtered rows.
    ///
    /// Returns whether the highlight moved. Only the mark moves: nothing is committed here, which
    /// is what makes browsing safe.
    pub fn move_highlight(&mut self, delta: i32) -> bool {
        let rows = self.visible.len();
        if rows == 0 {
            return false;
        }
        let last = rows - 1;
        let next = match self.selected_index {
            Some(index) => (index as i32 + delta).clamp(0, last as i32) as usize,
            None => {
                if delta < 0 {
                    last
                } else {
                    0
                }
            }
        };
        self.set_highlight(Some(next))
    }

    /// Scrolls the painted window so the highlight stays inside it.
    pub fn scroll_to_highlight(&mut self) {
        let Some(highlight) = self.selected_index else {
            self.set_scroll_offset(0);
            return;
        };
        self.scroll_into_view(highlight);
    }

    /// Takes the highlighted row: emits `suggestion_committed`, hides, and reports whether a row
    /// was committed.
    pub fn commit_highlighted(&mut self) -> bool {
        self.commit_selected().is_some()
    }

    /// Takes the highlighted row and returns it, or `None` when nothing can be taken.
    ///
    /// This is COMP-C01's spelling of [`Self::commit_highlighted`]; both run the same commit path,
    /// so the two signals cannot disagree about which row was taken.
    pub fn select_highlighted(&mut self) -> Option<PopupItem> {
        self.commit_selected().map(|(_, item)| item)
    }

    /// Shows the popup, highlighting the first row when nothing is highlighted yet.
    pub fn show(&mut self) {
        if self.shown {
            return;
        }
        self.shown = true;
        // The popup owns the press that dismisses it, matching the combo box's
        // popup-grab contract (I-127).
        #[cfg(not(alloc_frugal))]
        crate::widget::runtime::set_popup_grab(self.base.id());
        if self.selected_index.is_none() && !self.visible.is_empty() {
            self.set_highlight(Some(0));
        }
        self.base.request_redraw();
    }

    /// Hides the popup without committing.
    pub fn hide(&mut self) {
        if !self.shown {
            return;
        }
        self.shown = false;
        #[cfg(not(alloc_frugal))]
        crate::widget::runtime::clear_popup_grab_for(self.base.id());
        self.base.request_redraw();
    }

    /// Whether the popup is showing.
    pub fn is_shown(&self) -> bool {
        self.shown
    }

    /// Hides the popup and reports the dismissal (Escape, outside press, focus loss).
    pub fn dismiss(&mut self) {
        if self.shown {
            self.hide();
            self.dismissed.emit();
        }
    }

    /// Sets a fixed row height, or restores the font-derived one with `None`.
    pub fn set_row_height(&mut self, row_height: u32) {
        let next = Some(row_height.max(1));
        if self.row_height == next {
            return;
        }
        self.row_height = next;
        self.base.request_redraw();
    }

    /// Replaces the text painted when there are no candidates.
    pub fn set_empty_text(&mut self, text: &str) {
        if self.empty_text == text {
            return;
        }
        self.empty_text = text.to_string();
        self.base.request_redraw();
    }

    /// The text painted when there are no candidates.
    pub fn empty_text(&self) -> &str {
        &self.empty_text
    }

    /// How the row text is aligned within its row.
    pub fn alignment(&self) -> HorizontalAlignment {
        self.alignment
    }

    /// Sets how the row text is aligned within its row.
    pub fn set_alignment(&mut self, alignment: HorizontalAlignment) {
        if self.alignment == alignment {
            return;
        }
        self.alignment = alignment;
        self.base.request_redraw();
    }

    /// The height of one painted row.
    pub fn row_height(&self) -> u32 {
        self.row_height.unwrap_or_else(|| row_height_for(effective_font(self.base.style())))
    }

    /// The popup's box: window rows tall, hanging under (or flipped above) the anchor.
    pub fn popup_rect(&self) -> Rect {
        let rows = self.window_count().max(1) as u32;
        let height = rows.saturating_mul(self.row_height());
        match self.anchor {
            Some(anchor) => {
                let below = anchor.y + anchor.height as i32;
                let fits_below = !self.flip_allowed
                    || self.viewport.is_none_or(|viewport| {
                        below + height as i32 <= viewport.y + viewport.height as i32
                    });
                let y = if fits_below { below } else { anchor.y - height as i32 };
                Rect::new(anchor.x, y, anchor.width, height)
            }
            None => {
                let geometry = self.geometry();
                Rect::new(geometry.x, geometry.y, geometry.width, height)
            }
        }
    }

    /// The box of one filtered row, when that row is inside the painted window.
    pub fn row_rect(&self, row: usize) -> Option<Rect> {
        let start = self.window.offset();
        if row < start || row >= start + self.window_count() {
            return None;
        }
        let popup = self.popup_rect();
        let offset = (row - start) as i32 * self.row_height() as i32;
        Some(Rect::new(popup.x, popup.y + offset, popup.width, self.row_height()))
    }

    /// The filtered row under `pos`, when the popup is showing and the point is on one.
    pub fn row_at(&self, pos: Point) -> Option<usize> {
        if !self.shown {
            return None;
        }
        self.window_row_at(pos)
    }

    /// How many rows the painted window holds.
    fn window_count(&self) -> usize {
        self.window.window_rows(self.visible.len())
    }

    /// The described row at `index` in `items`.
    fn item(&self, index: usize) -> Option<PopupItem> {
        let label = self.items.get(index)?;
        let detail = self.details.get(index);
        Some(PopupItem {
            label: label.clone(),
            icon: detail.and_then(|detail| detail.icon.clone()),
            hint: detail.map(|detail| detail.hint.clone()).unwrap_or_default(),
            enabled: detail.is_none_or(|detail| !detail.disabled),
        })
    }

    /// The described row at `visible_index` in the filtered rows.
    fn item_at(&self, visible_index: usize) -> Option<PopupItem> {
        self.item(*self.visible.get(visible_index)?)
    }

    /// The highlighted row, when it exists and is enabled.
    fn selected_item(&self) -> Option<PopupItem> {
        self.selected_index.and_then(|index| self.item_at(index)).filter(|item| item.enabled)
    }

    /// One highlight path: clamp, keep it inside the window, redraw, and report the move.
    fn set_highlight(&mut self, index: Option<usize>) -> bool {
        let next = index.filter(|index| *index < self.visible.len());
        if self.selected_index == next {
            return false;
        }
        self.selected_index = next;
        // Clearing the mark is not a scroll request: a host that moves the pointer off the rows
        // (or closes its list) must not send the window back to its first row. A filter that
        // leaves no rows still resets the offset through `refresh_visible` -> `scroll_to_highlight`.
        if next.is_some() {
            self.scroll_to_highlight();
        }
        if let Some(index) = next {
            self.suggestion_highlighted.emit(index);
            self.highlight_changed.emit(index);
        }
        self.base.request_redraw();
        true
    }

    /// One commit path: both signal vocabularies report the same row.
    fn commit_selected(&mut self) -> Option<(usize, PopupItem)> {
        // A hidden popup has nothing on screen to take.
        if !self.shown {
            return None;
        }
        let index = self.selected_index?;
        let item = self.selected_item()?;
        self.suggestion_committed.emit(index);
        self.item_selected.emit(item.clone());
        self.hide();
        Some((index, item))
    }

    /// Rebuilds the filtered view and keeps the highlight inside it.
    fn refresh_visible(&mut self) {
        let needle = text_match::fold(&self.filter);
        self.visible.clear();
        for (index, label) in self.items.iter().enumerate() {
            let matched = if needle.is_empty() {
                true
            } else {
                match self.match_mode {
                    MatchMode::Prefix => text_match::starts_with(label, &needle),
                    MatchMode::Contains => text_match::contains(label, &needle),
                }
            };
            if matched {
                self.visible.push(index);
            }
        }
        self.selected_index = match self.selected_index {
            Some(_) if self.visible.is_empty() => None,
            Some(index) => Some(index.min(self.visible.len() - 1)),
            None => None,
        };
        self.scroll_to_highlight();
    }
}

impl EventHandler for SuggestionPopup {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() || !self.shown {
            return;
        }
        match event {
            Event::KeyPress { key, .. } => match *key {
                38 => {
                    self.move_highlight(-1);
                }
                40 => {
                    self.move_highlight(1);
                }
                36 => {
                    self.set_highlight(self.visible.first().map(|_| 0));
                }
                35 => {
                    self.set_highlight(self.visible.len().checked_sub(1));
                }
                13 => {
                    self.commit_highlighted();
                }
                27 => self.dismiss(),
                _ => {}
            },
            Event::MousePress { pos, button: 1, .. } => {
                if let Some(row) = self.row_at(*pos) {
                    self.set_highlight(Some(row));
                    self.commit_highlighted();
                } else if !self.popup_rect().contains_point(*pos) {
                    self.dismiss();
                }
            }
            Event::FocusLost => self.dismiss(),
            _ => {}
        }
    }
}

impl Draw for SuggestionPopup {
    fn draw(&mut self, context: &mut RenderContext) {
        if !self.shown {
            return;
        }
        let popup = self.popup_rect();
        if popup.width == 0 || popup.height == 0 {
            return;
        }
        let style = self.style();
        let font = effective_font(style);
        let surface = style.background_color.unwrap_or(Color::WHITE);
        let ink = style.text_color.unwrap_or_else(|| surface.contrast_color());
        let border = style.border_color.unwrap_or_else(|| surface.blend(&ink, 0.25));
        let accent = crate::style::resolved_theme_style_shared("suggestion_popup")
            .and_then(|resolved| resolved.background_color)
            .or_else(|| crate::style::palette_shared().map(|colors| colors.primary))
            .unwrap_or(Color::PRIMARY);

        context.fill_rounded_rect(popup, 2, surface);
        context.draw_rounded_rect_stroke(popup, 2, border, 1);

        if self.visible.is_empty() {
            let row = Rect::new(popup.x, popup.y, popup.width, self.row_height());
            let line = context.text_line(row, font);
            context.draw_text_fitted_cow(
                Rect::new(
                    row.x + ROW_INSET,
                    line.y,
                    row.width.saturating_sub(ROW_INSET as u32 * 2),
                    line.height.max(1),
                ),
                &self.empty_text,
                font,
                ink.with_alpha(120),
                self.alignment,
            );
            return;
        }

        // The paint loop and `row_at` read the same window: a row's box comes from `row_rect`,
        // which is indexed in filtered-row space, so scrolling cannot paint one row and hit-test
        // another.
        for window_row in 0..self.window_count() {
            let row = self.window.offset() + window_row;
            let Some(rect) = self.row_rect(row) else { continue };
            let Some(&item_index) = self.visible.get(row) else { continue };
            let Some(label) = self.items.get(item_index) else { continue };
            let detail = self.details.get(item_index);
            let enabled = detail.is_none_or(|detail| !detail.disabled);
            let hint = detail.map_or("", |detail| detail.hint.as_str());
            let highlighted = self.selected_index == Some(row);
            if highlighted {
                context.fill_rect(rect, accent.with_alpha(40));
            }
            let line = context.text_line(rect, font);
            let text_width = rect.width.saturating_sub(ROW_INSET as u32 * 2);
            let hint_width = if hint.is_empty() {
                0
            } else {
                estimate_text_width(hint, font, 1.0).min(text_width / 2)
            };
            let label_width = if hint_width == 0 {
                text_width
            } else {
                text_width.saturating_sub(hint_width + ROW_GAP)
            };
            let label_ink = if !enabled {
                ink.with_alpha(90)
            } else if highlighted {
                ink
            } else {
                ink.with_alpha(230)
            };
            context.draw_text_fitted_cow(
                Rect::new(rect.x + ROW_INSET, line.y, label_width.max(1), line.height.max(1)),
                label,
                font,
                label_ink,
                self.alignment,
            );
            if hint_width > 0 {
                context.draw_text_fitted_cow(
                    Rect::new(rect.x + ROW_INSET, line.y, text_width, line.height.max(1)),
                    hint,
                    font,
                    ink.with_alpha(120),
                    HorizontalAlignment::Right,
                );
            }
        }
    }
}

impl Widget for SuggestionPopup {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn size_hint(&self) -> Size {
        Size::new(200, self.row_height().saturating_mul(self.window_count().max(1) as u32))
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    fn event_signal_dyn(&self, name: &str) -> Option<EventSignalRef> {
        match name {
            "suggestion_highlighted" => Some(EventSignalRef::mapped(
                "suggestion_highlighted",
                &self.suggestion_highlighted,
                |value| CapabilityValue::UInt(*value as u64),
            )),
            "suggestion_committed" => Some(EventSignalRef::mapped(
                "suggestion_committed",
                &self.suggestion_committed,
                |value| CapabilityValue::UInt(*value as u64),
            )),
            "highlight_changed" => Some(EventSignalRef::mapped(
                "highlight_changed",
                &self.highlight_changed,
                |value| CapabilityValue::UInt(*value as u64),
            )),
            "item_selected" => {
                Some(EventSignalRef::mapped("item_selected", &self.item_selected, |item| {
                    CapabilityValue::String(item.label.clone())
                }))
            }
            "dismissed" => Some(EventSignalRef::unit("dismissed", &self.dismissed)),
            _ => None,
        }
    }
}

impl WidgetProperties for SuggestionPopup {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "items" => Ok(strings_value(&self.items)),
            "anchor" => match self.anchor {
                Some(anchor) => Ok(rect_value(anchor)),
                None => Ok(CapabilityValue::Null),
            },
            "max_visible" => Ok(CapabilityValue::UInt(self.max_visible() as u64)),
            "row_height" => Ok(CapabilityValue::UInt(u64::from(self.row_height()))),
            "selected_index" => match self.selected_index {
                Some(index) => Ok(CapabilityValue::UInt(index as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "empty_text" => Ok(CapabilityValue::String(self.empty_text.clone())),
            "shown" => Ok(CapabilityValue::Bool(self.shown)),
            "filter" => Ok(CapabilityValue::String(self.filter.clone())),
            "match_mode" => Ok(CapabilityValue::String(self.match_mode.as_name().to_string())),
            "viewport" => match self.viewport {
                Some(viewport) => Ok(rect_value(viewport)),
                None => Ok(CapabilityValue::Null),
            },
            "flip_allowed" => Ok(CapabilityValue::Bool(self.flip_allowed)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "items" => {
                self.set_items(expect_strings(value)?);
                Ok(())
            }
            "anchor" => match value {
                CapabilityValue::Null => {
                    self.anchor = None;
                    self.base.request_redraw();
                    Ok(())
                }
                other => {
                    self.set_anchor(expect_rect(other)?);
                    Ok(())
                }
            },
            "max_visible" => {
                self.set_max_visible(expect_usize(value)?);
                Ok(())
            }
            "row_height" => {
                self.set_row_height(expect_u32(value)?);
                Ok(())
            }
            "selected_index" => match value {
                CapabilityValue::Null => {
                    self.set_selected_index(None);
                    Ok(())
                }
                other => {
                    self.set_selected_index(Some(expect_usize(other)?));
                    Ok(())
                }
            },
            "empty_text" => {
                let text = expect_string(value)?;
                self.set_empty_text(&text);
                Ok(())
            }
            "shown" => {
                let shown = expect_bool(value)?;
                if shown {
                    self.show();
                } else {
                    self.dismiss();
                }
                Ok(())
            }
            "filter" => {
                let text = expect_string(value)?;
                self.apply_filter(&text);
                Ok(())
            }
            "match_mode" => {
                let token = normalize_key(&expect_string(value)?);
                let mode = match token.as_str() {
                    "prefix" => MatchMode::Prefix,
                    "contains" => MatchMode::Contains,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_match_mode(mode);
                Ok(())
            }
            "viewport" => match value {
                CapabilityValue::Null => {
                    self.set_viewport(None);
                    Ok(())
                }
                other => {
                    self.set_viewport(Some(expect_rect(other)?));
                    Ok(())
                }
            },
            "flip_allowed" => {
                self.set_flip_allowed(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "items",
            "anchor",
            "max_visible",
            "row_height",
            "selected_index",
            "empty_text",
            "shown",
            "filter",
            "match_mode",
            "viewport",
            "flip_allowed",
            BASE_PROPERTY_NAMES
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::{lock, Arc, Mutex};
    use core::sync::atomic::{AtomicUsize, Ordering};

    fn popup() -> SuggestionPopup {
        SuggestionPopup::new(Rect::new(0, 0, 200, 1))
    }

    fn labels(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    #[test]
    fn arrows_move_the_mark_and_only_enter_commits() {
        let mut popup = popup();
        popup.set_items(vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()]);
        popup.show();
        assert_eq!(popup.selected_index(), Some(0));
        assert!(popup.move_highlight(1));
        assert_eq!(popup.selected_index(), Some(1));
        // Moving did not commit anything: `commit_highlighted` is the only commit path.
        assert!(popup.commit_highlighted());
        assert!(!popup.is_shown());
        assert!(!popup.commit_highlighted());
    }

    #[test]
    fn escape_dismisses_without_committing() {
        let mut popup = popup();
        popup.set_items(vec!["alpha".to_string()]);
        popup.show();
        popup.handle_event(&Event::key_press(27, 0));
        assert!(!popup.is_shown());
    }

    #[test]
    fn the_empty_state_row_is_painted_and_configurable() {
        let mut popup = popup();
        popup.show();
        assert_eq!(popup.empty_text(), "(No matches)");
        popup.set_empty_text("Nothing here");
        assert_eq!(popup.empty_text(), "Nothing here");
    }

    #[test]
    fn a_row_is_hit_tested_from_the_anchor_band() {
        let mut popup = popup();
        popup.set_items(vec!["a".to_string(), "b".to_string()]);
        popup.set_anchor(Rect::new(10, 20, 120, 24));
        popup.show();
        let height = popup.row_height() as i32;
        assert_eq!(popup.row_at(Point::new(20, 44 + height / 2)), Some(0));
        assert_eq!(popup.row_at(Point::new(20, 44 + height + height / 2)), Some(1));
        assert_eq!(popup.row_at(Point::new(20, 44 + height * 2)), None);
    }

    #[test]
    fn the_property_contract_round_trips_the_rows() {
        let mut popup = popup();
        popup
            .set("items", CapabilityValue::Tuple(vec![CapabilityValue::String("x".to_string())]))
            .unwrap();
        assert_eq!(popup.items(), ["x".to_string()]);
        assert_eq!(popup.get("selected_index").unwrap(), CapabilityValue::Null);
    }

    #[test]
    fn a_filter_matches_by_prefix_or_by_substring() {
        let mut popup = popup();
        popup.set_items(labels(&["alpha", "beta", "alphabet"]));
        popup.apply_filter("ALP");
        assert_eq!(popup.match_mode(), MatchMode::Prefix);
        assert_eq!(popup.visible_count(), 2);
        assert_eq!(
            popup.visible_items().iter().map(|item| item.label.as_str()).collect::<Vec<_>>(),
            vec!["alpha", "alphabet"]
        );
        // Filtering narrows the view without destroying the rows the host supplied.
        assert_eq!(popup.items().len(), 3);

        popup.set_match_mode(MatchMode::Contains);
        popup.apply_filter("PHA");
        assert_eq!(popup.visible_count(), 2);

        popup.apply_filter("eta");
        assert_eq!(popup.visible_count(), 1);
        assert_eq!(popup.visible_items()[0].label, "beta");

        popup.apply_filter("");
        assert_eq!(popup.visible_count(), 3);
    }

    #[test]
    fn the_popup_flips_above_the_anchor_when_the_viewport_has_no_room_below() {
        let mut popup = popup();
        popup.set_items(labels(&["a", "b"]));
        popup.set_row_height(20);
        popup.set_anchor(Rect::new(10, 20, 120, 24));
        // The anchor ends at y = 44 and two rows need 40 px, so a viewport 44 px tall flips.
        popup.set_viewport(Some(Rect::new(0, 0, 300, 44)));
        assert_eq!(popup.popup_rect().y, 20 - 40);
        // Room below keeps the popup under the anchor.
        popup.set_viewport(Some(Rect::new(0, 0, 300, 200)));
        assert_eq!(popup.popup_rect().y, 44);
        // A host that forbids flipping always gets the under-anchor placement.
        popup.set_viewport(Some(Rect::new(0, 0, 300, 44)));
        popup.set_flip_allowed(false);
        assert_eq!(popup.popup_rect().y, 44);
    }

    #[test]
    fn the_scroll_window_is_shared_by_the_paint_order_and_the_hit_test() {
        let mut popup = popup();
        popup.set_items(labels(&["r0", "r1", "r2", "r3", "r4"]));
        popup.set_max_visible(2);
        popup.set_row_height(10);
        popup.set_anchor(Rect::new(0, 0, 100, 10));
        popup.show();
        assert_eq!(popup.scroll_offset(), 0);

        popup.set_selected_index(Some(3));
        assert_eq!(popup.scroll_offset(), 2);
        assert_eq!(popup.popup_rect(), Rect::new(0, 10, 100, 20));
        // The painted window starts at filtered row 2, so that row owns the top slot...
        assert_eq!(popup.row_rect(2), Some(Rect::new(0, 10, 100, 10)));
        assert_eq!(popup.row_rect(3), Some(Rect::new(0, 20, 100, 10)));
        // ...and the rows scrolled out have no box at all.
        assert_eq!(popup.row_rect(0), None);
        assert_eq!(popup.row_rect(4), None);
        // The hit test reads the same window: y = 15 is the top painted row, which is row 2.
        assert_eq!(popup.row_at(Point::new(5, 15)), Some(2));
        assert_eq!(popup.row_at(Point::new(5, 25)), Some(3));
        assert_eq!(popup.row_at(Point::new(5, 35)), None);
    }

    #[test]
    fn an_outside_press_dismisses_but_a_press_inside_the_empty_state_does_not() {
        let mut popup = popup();
        popup.set_anchor(Rect::new(10, 20, 120, 24));
        popup.show();
        let dismissals = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&dismissals);
        popup.dismissed.connect(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        });

        // Inside the popup's empty-state row: nothing to take, but not an outside press.
        popup.handle_event(&Event::mouse_press(20, 50, 1));
        assert!(popup.is_shown());
        assert_eq!(dismissals.load(Ordering::SeqCst), 0);

        popup.handle_event(&Event::mouse_press(5, 5, 1));
        assert!(!popup.is_shown());
        assert_eq!(dismissals.load(Ordering::SeqCst), 1);

        // A press while already hidden is not a second dismissal.
        popup.handle_event(&Event::mouse_press(5, 5, 1));
        assert_eq!(dismissals.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_press_on_a_row_commits_it_and_a_disabled_row_refuses() {
        let mut popup = popup();
        popup.set_popup_items(vec![
            PopupItem::new("alpha"),
            PopupItem::new("beta").with_hint("second"),
        ]);
        popup.set_anchor(Rect::new(10, 20, 120, 24));
        popup.show();
        let row_height = popup.row_height() as i32;
        popup.handle_event(&Event::mouse_press(20, 44 + row_height + 2, 1));
        assert!(!popup.is_shown());

        let mut disabled = PopupItem::new("locked");
        disabled.enabled = false;
        popup.set_popup_items(vec![disabled]);
        popup.show();
        assert!(!popup.commit_highlighted());
        assert!(popup.is_shown());
    }

    #[test]
    fn both_signal_vocabularies_report_one_highlight_and_one_commit() {
        let mut popup = popup();
        popup.set_popup_items(vec![PopupItem::new("alpha").with_hint("first")]);

        let legacy = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&legacy);
        popup.suggestion_highlighted.connect_ref(move |_| {
            counter.fetch_add(1, Ordering::SeqCst);
        });
        let changed = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&changed);
        popup.highlight_changed.connect_ref(move |_| {
            counter.fetch_add(1, Ordering::SeqCst);
        });
        let picked: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let sink = Arc::clone(&picked);
        popup.item_selected.connect_ref(move |item: &PopupItem| {
            *lock(&sink) = Some(item.label.clone());
        });

        popup.show();
        assert_eq!(legacy.load(Ordering::SeqCst), 1);
        assert_eq!(changed.load(Ordering::SeqCst), 1);

        let item = popup.select_highlighted().unwrap();
        assert_eq!(item.label, "alpha");
        assert_eq!(*lock(&picked), Some("alpha".to_string()));
        assert!(!popup.is_shown());
    }
}
