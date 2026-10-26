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

use core::fmt::Write as _;

use crate::core::{Color, HorizontalAlignment, Rect, Size};
use crate::event::{Event, EventHandler, WheelScroll};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::style::EdgeOffsets;
use crate::widget::capability::coercion::{
    expect_bool, expect_horizontal_alignment, expect_string, horizontal_alignment_to_str,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, effective_font, estimate_text_width, ControlMetrics};
use crate::widget::{BaseWidget, Draw, IconName, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::cell::Ref;
use std::cell::RefCell;
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
    /// How the field's summary is aligned within its own box.
    ///
    /// Horizontal only: the summary is centred vertically in the field, so a `top`/`bottom`
    /// value would be one this control could never honour —
    /// [`crate::widget::capability::coercion::expect_horizontal_alignment`] refuses those rather
    /// than accepting a write that does nothing. Defaults to left.
    alignment: crate::core::Alignment,
    /// Emitted when the selection changes, with the list of selected item IDs.
    pub selection_changed: Signal1<Vec<u64>>,
    /// The row keyboard input acts on while the list is open.
    active_index: usize,
    /// Maximum number of rows the open list shows at once. `None` shows them all.
    max_visible: Option<usize>,
    /// Index of the first row painted in the open list.
    scroll_offset: usize,
    /// Placeholder shown instead of the empty-selection summary (I-104).
    placeholder: String,
    /// Bumped whenever the items or the selection change (I-P68).
    summary_revision: u64,
    /// Memoised summary text, keyed by `summary_revision` (I-P68).
    summary_cache: RefCell<Option<(u64, String)>>,
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
            alignment: crate::core::Alignment::Left,
            selection_changed: Signal1::new(),
            active_index: 0,
            max_visible: None,
            scroll_offset: 0,
            placeholder: String::new(),
            summary_revision: 0,
            summary_cache: RefCell::new(None),
        }
    }

    /// The text shown when nothing is selected (I-104).
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }

    /// Sets the empty-selection placeholder (I-104).
    pub fn set_placeholder(&mut self, placeholder: &str) {
        if self.placeholder == placeholder {
            return;
        }
        self.placeholder = placeholder.to_string();
        self.summary_cache.borrow_mut().take();
        self.base.request_redraw();
    }

    /// Bumps the summary cache key after an item or selection change (I-P68).
    fn bump_summary(&mut self) {
        self.summary_revision = self.summary_revision.wrapping_add(1);
        self.summary_cache.borrow_mut().take();
    }

    /// Adds an item to the combo box.
    pub fn add_item(&mut self, item: MultiSelectItem) {
        self.items.push(item);
        self.bump_summary();
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
            self.bump_summary();
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
        self.bump_summary();
        self.base.request_redraw();
    }

    /// Returns the number of items.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// How the field's summary is aligned within its own box.
    pub fn alignment(&self) -> crate::core::Alignment {
        self.alignment
    }

    /// Sets how the field's summary is aligned within its own box.
    ///
    /// Horizontal only. A `top`/`bottom` alignment is **ignored**, because the summary is centred
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

    /// Returns a reference to the list of items.
    pub fn items(&self) -> &[MultiSelectItem] {
        &self.items
    }

    /// Selects the item at the given index (if enabled). Returns true if selection changed.
    pub fn select(&mut self, index: usize) -> bool {
        if index < self.items.len() && self.items[index].enabled && self.selected.insert(index) {
            self.bump_summary();
            self.emit_selection_changed();
            self.base.request_redraw();
            return true;
        }
        false
    }

    /// Deselects the item at the given index. Returns true if selection changed.
    pub fn deselect(&mut self, index: usize) -> bool {
        if self.selected.remove(&index) {
            self.bump_summary();
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
            self.bump_summary();
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

    /// Writes the summary shown when the dropdown is collapsed into `out`.
    ///
    /// Into-style (the calendar-title arrangement): the draw pass renders into its own
    /// buffer, so the common literal case (`"Nothing selected"`) costs no string at all.
    fn write_summary_text(&self, out: &mut String) {
        let selected_count = self.selected.len();
        if selected_count == 0 {
            if !self.placeholder.is_empty() {
                out.push_str(&self.placeholder);
                return;
            }
            out.push_str("Nothing selected");
        } else if selected_count <= 2 {
            // Borrowed in place: `selected_texts` cloned every selected label into a fresh
            // `Vec<String>` just to join it. At most two labels here, sorted borrowed and
            // joined with the same order the owned version sorts by.
            let mut texts: Vec<&str> = self
                .selected
                .iter()
                .filter_map(|&idx| self.items.get(idx).map(|item| item.text.as_str()))
                .collect();
            texts.sort_unstable();
            for (i, text) in texts.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(text);
            }
        } else {
            let _ = write!(out, "{selected_count} selected");
        }
    }

    /// The summary text; see [`Self::write_summary_text`]. Kept for `size_hint`, which
    /// measures the widest summary the selection can currently produce.
    fn summary_text(&self) -> String {
        self.summary_cached().as_str().to_string()
    }

    /// The summary string, rebuilt only when the items or selection change (I-P68).
    fn summary_cached(&self) -> Ref<'_, String> {
        let stale = self
            .summary_cache
            .borrow()
            .as_ref()
            .is_none_or(|(revision, _)| *revision != self.summary_revision);
        if stale {
            let mut text = String::new();
            self.write_summary_text(&mut text);
            *self.summary_cache.borrow_mut() = Some((self.summary_revision, text));
        }
        Ref::map(self.summary_cache.borrow(), |cache| {
            &cache.as_ref().expect("the summary cache was just filled").1
        })
    }

    /// Toggles the expanded/collapsed state.
    fn toggle_expand(&mut self) {
        self.expanded = !self.expanded;
        if self.expanded {
            self.active_index = self.active_index.min(self.items.len().saturating_sub(1));
            self.ensure_active_visible();
        }
        self.base.request_redraw();
    }

    /// The maximum number of rows the open list shows at once.
    pub fn max_visible(&self) -> Option<usize> {
        self.max_visible
    }

    /// Caps the number of rows the open list paints.
    ///
    /// A zero cap is treated as one row: an open list with no rows reads as a broken control.
    pub fn set_max_visible(&mut self, max_visible: Option<usize>) {
        let max_visible = max_visible.map(|value| value.max(1));
        if self.max_visible == max_visible {
            return;
        }
        self.max_visible = max_visible;
        self.ensure_active_visible();
        self.base.request_redraw();
    }

    /// Index of the first row painted in the open list.
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Sets the first painted row, clamped so the window stays full when the list allows.
    pub fn set_scroll_offset(&mut self, offset: usize) {
        let next = self.clamp_scroll(offset);
        if self.scroll_offset == next {
            return;
        }
        self.scroll_offset = next;
        self.base.request_redraw();
    }

    /// The row keyboard input acts on.
    pub fn active_index(&self) -> usize {
        self.active_index
    }

    /// Sets the keyboard row, clamped into the list.
    pub fn set_active_index(&mut self, index: usize) {
        let next = index.min(self.items.len().saturating_sub(1));
        if self.active_index == next {
            return;
        }
        self.active_index = next;
        self.ensure_active_visible();
        self.base.request_redraw();
    }

    /// Number of rows the open list shows at once.
    pub fn page_size(&self) -> usize {
        self.max_visible.unwrap_or(self.items.len()).max(1)
    }

    fn clamp_scroll(&self, offset: usize) -> usize {
        let page = self.page_size();
        let max_offset = self.items.len().saturating_sub(page.min(self.items.len()));
        offset.min(max_offset)
    }

    /// Scrolls the list so the keyboard row is inside the visible window.
    pub fn ensure_active_visible(&mut self) {
        if self.items.is_empty() {
            self.scroll_offset = 0;
            return;
        }
        let page = self.page_size();
        if self.active_index < self.scroll_offset {
            self.scroll_offset = self.active_index;
        } else if self.active_index >= self.scroll_offset + page {
            self.scroll_offset = self.active_index + 1 - page;
        }
        self.scroll_offset = self.clamp_scroll(self.scroll_offset);
    }

    /// Rows currently painted in the open list.
    fn visible_range(&self) -> (usize, usize) {
        let start = self.clamp_scroll(self.scroll_offset);
        let end = (start + self.page_size()).min(self.items.len());
        (start, end)
    }

    /// Selects every enabled item.
    ///
    /// Returns whether the selection changed. Disabled rows stay unselected, so "select
    /// all" never selects something the control refuses to select individually.
    pub fn select_all(&mut self) -> bool {
        let before = self.selected.len();
        for (index, item) in self.items.iter().enumerate() {
            if item.enabled {
                self.selected.insert(index);
            }
        }
        if self.selected.len() != before {
            self.bump_summary();
            self.emit_selection_changed();
            self.base.request_redraw();
            return true;
        }
        false
    }
}

impl Widget for MultiSelectComboBox {
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
        // Content width: the summary's own text, through the shared estimate. A hint has no
        // `RenderContext` to measure with, and a hint a few pixels generous is the safe direction
        // — the paint path *fits* the summary into the box this produced.
        //
        // It used to be `len() as u32 * 8` (UTF-8 bytes); see `editable_combo_box` for why a byte
        // count is not the generous direction but a wrong one for any non-ASCII summary.
        let content = Size::new(
            estimate_text_width(&self.summary_text(), effective_font(self.style()), 1.0),
            0,
        );
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
    ///
    /// # Payload mapping
    ///
    /// A hub name carries no value, so a `Signal1<T>` needs a conversion. The one used here is the
    /// same spelling the property side uses, so a designer reads one representation per kind: a
    /// number as `Int`, a float as `Float`, a flag as `Bool`, text as `String`, and a structured
    /// payload as its debug spelling. A `None` maps to `Null`, which is the same value an absent
    /// optional property uses — so "no selection" is distinguishable from "selection 0".
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "selection_changed" => {
                Some(EventSignalRef::mapped("selection_changed", &self.selection_changed, |v| {
                    let mut items = crate::compat::Vec::with_capacity(v.len());
                    for id in v.iter() {
                        items.push(CapabilityValue::UInt(*id));
                    }
                    CapabilityValue::Tuple(items)
                }))
            }
            _ => None,
        }
    }
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
            // I-104: the selected values are readable as a comma-joined string (the same
            // list-as-string spelling `tag_input` publishes).
            "selected_items" => Ok(CapabilityValue::String(self.selected_texts().join(","))),
            "placeholder" => Ok(CapabilityValue::String(self.placeholder().to_string())),
            "expanded" => Ok(CapabilityValue::Bool(self.is_expanded())),
            "alignment" => Ok(CapabilityValue::String(
                horizontal_alignment_to_str(self.alignment()).to_string(),
            )),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "expanded" => {
                self.set_expanded(expect_bool(value)?);
                Ok(())
            }
            "selected_count" | "selected_items" => Err(CapabilityAccessError::ReadOnlyProperty),
            "placeholder" => {
                self.set_placeholder(&expect_string(value)?);
                Ok(())
            }
            "alignment" => {
                self.set_alignment(expect_horizontal_alignment(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "selected_count",
            "selected_items",
            "expanded",
            "placeholder",
            "alignment",
            BASE_PROPERTY_NAMES
        ]
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
        let style = self.base.style();
        // I-P68: one theme read serves the ink, the border, the fill and the accent. The draw
        // used to take the manager lock twice (the resolved style and then the primary).
        let (themed_ink, themed_border, themed_field, themed_accent) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => (
                    Some(active.colors.foreground),
                    Some(active.colors.outline_variant),
                    Some(active.colors.background),
                    Some(active.colors.primary),
                ),
                None => (None, None, None, None),
            }
        };

        let ink = style.text_color.or(themed_ink).unwrap_or(Color::BLACK);
        let field = style.background_color.or(themed_field).unwrap_or(Color::WHITE);
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
        // The same M1 gap as its `editable` sibling and `combo_box`: a closed multi-select is a
        // field the user presses to open a list, and it painted one colour in every pointer
        // state. `StateOverlay::fill_blend` is read so the weight matches the theme's own
        // `"<kind>:hover"` key rather than a private constant that could drift from it.
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

        // The accent is the theme's `primary`: the hue a theme is expected to vary most,
        // which is what makes the ticks and the arrow follow the appearance. It comes from the
        // same single theme read as the inks above (I-P68).
        let accent = themed_accent.unwrap_or_else(|| ink.blend(&field, 0.4));

        // The summary's box and the indicator's box come from **one** derivation, so the
        // summary yields to the indicator instead of being written at an unconstrained offset.
        // The font is the **effective** one so the theme body font and the user's text scale
        // reach the summary and the list rows (D09-STYLE-01).
        let font = effective_font(style);
        let line = context.text_line(rect, font);
        let geometry = self.indicator_geometry(line.height);
        // I-P68: the summary is rebuilt only on a selection/item change.
        let summary = self.summary_cached();
        // Fitted into the box the indicator left, on that box's own line box: a glyph origin is
        // the box's top edge, so the old `padding + 13` put a 13 px font's origin on the field's
        // middle line and drew the summary half a line low.
        let summary_line = context.text_line(geometry.text_box, font);
        context.draw_text_fitted_cow(
            Rect::new(
                geometry.text_box.x,
                summary_line.y,
                geometry.text_box.width,
                summary_line.height,
            ),
            summary.as_str(),
            font,
            ink,
            self.alignment.to_horizontal().unwrap_or(HorizontalAlignment::Left),
        );

        // Draw the dropdown indicator inside its own derived box. It is the affordance that
        // tells a user the field opens, so it is held to the text floor rather than dimmed by a
        // fixed fraction — `0.45` toward the background measured 3.45:1 on the dark field.
        // The disclosure arrow is a `ChevronUp`/`ChevronDown` **icon outline** rather than the
        // `▲`/`▼` text glyph: no bundled face covers U+25B2/U+25BC, so those fell back to 8x8
        // bitmap blocks. It is centred in the box the field geometry reserved for it.
        let arrow_icon = if self.expanded { IconName::ChevronUp } else { IconName::ChevronDown };
        let arrow_color = ink.legible_on(bg_color, 4.5).blend(&bg_color, 0.15);
        crate::widget::draw_icon_centered(
            context,
            geometry.box_rect,
            geometry.box_rect.height.min(geometry.box_rect.width),
            arrow_color,
            arrow_icon,
        );

        // Draw dropdown if expanded
        if !self.expanded || self.items.is_empty() {
            return;
        }

        let drop_down_y = rect.y + rect.height as i32;
        let item_height = 28u32;
        let (first, last) = self.visible_range();
        let drop_down_height = item_height * (last - first) as u32;
        let drop_rect = Rect::new(rect.x, drop_down_y, rect.width, drop_down_height);

        // Dropdown background. The list floats over whatever is beneath it, so it is the
        // field's own colour rather than the window's.
        context.fill_rounded_rect(drop_rect, 2, field);
        context.draw_rounded_rect_stroke(drop_rect, 2, border, 1);
        // I-103: the list is clipped to its own rectangle, so a narrow list cannot paint over
        // the control's neighbours.
        context.push_clip(drop_rect.x, drop_rect.y, drop_rect.width, drop_rect.height);

        for (row, i) in (first..last).enumerate() {
            let item = &self.items[i];
            let item_rect = Rect::new(
                rect.x + 1,
                drop_down_y + (row as i32) * (item_height as i32),
                rect.width.saturating_sub(2),
                item_height,
            );

            // Row highlight. A row cannot be a different colour from the list it sits in,
            // so this is the list fill nudged toward the ink: on a light list that is
            // slightly darker, on a dark one slightly lighter. The keyboard row is a
            // separate, stronger band so the arrow keys have a visible target.
            let row_fill = if i == self.active_index {
                field.blend(&accent, 0.18)
            } else {
                field.blend(&ink, 0.04)
            };
            context.fill_rounded_rect(item_rect, 2, row_fill);

            // Checkbox
            let checkbox_size = 14u32;
            let checkbox_x = item_rect.x + 4;
            let checkbox_y = item_rect.y + (item_height - checkbox_size) as i32 / 2;
            let checkbox_rect = Rect::new(checkbox_x, checkbox_y, checkbox_size, checkbox_size);
            // A selected box carries the accent; an unselected one the muted border.
            let checkbox_color = if self.selected.contains(&i) { accent } else { border };
            context.draw_rounded_rect_stroke(checkbox_rect, 2, checkbox_color, 1);

            if self.selected.contains(&i) {
                // The tick is the `Check` **icon outline**, not the `✓` text glyph: no bundled face
                // covers U+2713, so the glyph fell back to an 8x8 bitmap block. It is inset inside
                // the box just drawn so the outline reads as a mark *in* the box.
                let inset = checkbox_rect.width / 4;
                let tick = Rect::new(
                    checkbox_rect.x + inset as i32,
                    checkbox_rect.y + inset as i32,
                    checkbox_rect.width - inset * 2,
                    checkbox_rect.height - inset * 2,
                );
                crate::widget::draw_icon_at(context, tick, checkbox_color, IconName::Check);
            }

            // Item text: line-box centred in the row (the fixed `+18` sat the label a
            // half line below the checkbox, which is centred), and fitted between the
            // checkbox and the row's right edge.
            let item_text_x = checkbox_x + checkbox_size as i32 + 6;
            let item_line = context.text_line(item_rect, font);
            let item_color = if item.enabled { ink } else { ink.blend(&field, 0.6) };
            let text_band_width =
                ((item_rect.x + item_rect.width as i32 - 8) - item_text_x).max(0) as u32;
            context.draw_text_fitted_cow(
                Rect::new(item_text_x, item_line.y, text_band_width, item_line.height.max(1)),
                &item.text,
                font,
                item_color,
                HorizontalAlignment::Left,
            );
        }

        context.pop_clip();
        // A capped list shows a thin thumb, so a window of a longer list cannot look like
        // the whole list.
        if self.max_visible.is_some() && self.items.len() > self.page_size() && last > first {
            let page = self.page_size();
            let track_height = drop_down_height as i32;
            let thumb_height =
                ((track_height as usize * page) / self.items.len().max(1)).max(8) as i32;
            let max_offset = self.items.len().saturating_sub(page).max(1);
            let thumb_top = drop_down_y
                + ((track_height - thumb_height) * self.scroll_offset as i32 / max_offset as i32);
            context.fill_rect(
                Rect::new(rect.x + rect.width as i32 - 4, thumb_top, 3, thumb_height as u32),
                border,
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
            Event::MousePress { pos, button, .. } if *button == 1 => {
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
                    let (first, last) = self.visible_range();
                    let drop_down_height = item_height * (last - first) as i32;
                    let drop_rect =
                        Rect::new(rect.x, drop_down_y, rect.width, drop_down_height as u32);

                    if drop_rect.contains_point(*pos) {
                        let rel_y = pos.y - drop_down_y;
                        let row = (rel_y / item_height) as usize;
                        let idx = first + row;
                        if idx < last && self.items[idx].enabled {
                            self.active_index = idx;
                            self.toggle(idx);
                        }
                    } else {
                        // I-103: a press outside both the field and the open list dismisses it.
                        self.expanded = false;
                        self.base.request_redraw();
                    }
                }
            }
            Event::KeyPress { key, modifiers } => {
                // Keyboard support: Escape closes, arrows move the active row, Space toggles
                // it, Page keys move a screenful, and Primary+A selects every enabled row.
                if !self.expanded {
                    if matches!(*key, 13 | 32 | 38 | 40) && !self.items.is_empty() {
                        self.expanded = true;
                        self.ensure_active_visible();
                        self.base.request_redraw();
                    }
                    return;
                }
                let mods = crate::shortcut::Modifiers::from_event_bits(*modifiers);
                if (mods.contains(crate::shortcut::Modifiers::CTRL)
                    || crate::shortcut::event_carries_primary(*modifiers))
                    && (*key == 65 || *key == 97)
                {
                    // The two table-wide actions: Primary+A selects every enabled row and
                    // Primary+Shift+A clears the selection again, so an open list can be
                    // emptied without toggling each row by hand.
                    if mods.contains(crate::shortcut::Modifiers::SHIFT) {
                        self.clear_selection();
                    } else {
                        self.select_all();
                    }
                    return;
                }
                let page = self.page_size();
                match *key {
                    27 => {
                        self.expanded = false;
                        self.base.request_redraw();
                    }
                    38 => self.set_active_index(self.active_index.saturating_sub(1)),
                    40 => {
                        if self.active_index + 1 < self.items.len() {
                            self.set_active_index(self.active_index + 1);
                        }
                    }
                    33 => self.set_active_index(self.active_index.saturating_sub(page)),
                    34 => self.set_active_index(
                        (self.active_index + page).min(self.items.len().saturating_sub(1)),
                    ),
                    36 => self.set_active_index(0),
                    35 => {
                        self.set_active_index(self.items.len().saturating_sub(1));
                    }
                    32 => self.toggle(self.active_index),
                    13 => {
                        self.toggle(self.active_index);
                        self.expanded = false;
                        self.base.request_redraw();
                    }
                    _ => {}
                }
            }
            Event::Wheel { delta, modifiers } => {
                if !self.expanded {
                    self.base.handle_event(event);
                    return;
                }
                let scroll = WheelScroll::from_event(*delta, *modifiers, 1);
                if scroll.lines_y != 0 {
                    let offset = (self.scroll_offset as i32 + scroll.lines_y).max(0) as usize;
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
    use crate::core::Point;
    use crate::widget::input_widgets::shortcut_editor::primary_modifier_event_bits;
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

    /// The open list has a keyboard model and a bounded, scrollable window.
    #[test]
    fn keyboard_navigation_and_a_capped_list() {
        let mut combo = MultiSelectComboBox::new(Rect::new(0, 0, 200, 40));
        for index in 0..8 {
            combo.add_item(MultiSelectItem::new(index as u64 + 1, format!("Item {index}")));
        }
        combo.set_max_visible(Some(3));
        combo.set_expanded(true);
        assert_eq!(combo.page_size(), 3);

        combo.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
        assert_eq!(combo.active_index(), 1);
        combo.handle_event(&Event::KeyPress { key: 32, modifiers: 0 });
        assert!(combo.is_selected(1));
        assert_eq!(combo.selected_count(), 1);

        combo.handle_event(&Event::KeyPress { key: 35, modifiers: 0 }); // End
        assert_eq!(combo.active_index(), 7);
        assert_eq!(combo.scroll_offset(), 5, "the active row is kept visible");

        combo.handle_event(&Event::Wheel { delta: Point::new(0, -1), modifiers: 0 });
        assert_eq!(combo.scroll_offset(), 4);
        combo.handle_event(&Event::KeyPress { key: 27, modifiers: 0 });
        assert!(!combo.is_expanded());
    }

    /// Primary+A selects every enabled row in one action.
    #[test]
    fn select_all_respects_disabled_rows() {
        let mut combo = MultiSelectComboBox::new(Rect::new(0, 0, 200, 40));
        combo.add_item(MultiSelectItem::new(1, "A".to_string()));
        combo.add_item(MultiSelectItem::with_enabled(2, "B".to_string(), false));
        combo.add_item(MultiSelectItem::new(3, "C".to_string()));
        assert!(combo.select_all());
        assert_eq!(combo.selected_indices(), vec![0, 2]);
        assert_eq!(combo.selected_count(), 2);
    }

    /// Primary+Shift+A clears the open list's selection in one action.
    #[test]
    fn primary_shift_a_clears_the_open_list() {
        let mut combo = MultiSelectComboBox::new(Rect::new(0, 0, 200, 40));
        combo.add_item(MultiSelectItem::new(1, "A".to_string()));
        combo.add_item(MultiSelectItem::new(2, "B".to_string()));
        combo.set_expanded(true);
        combo.select_all();
        assert_eq!(combo.selected_count(), 2);

        // Shift is event bit 1; the accelerator bit comes from the host's own convention.
        combo.handle_event(&Event::KeyPress {
            key: 65,
            modifiers: primary_modifier_event_bits() | 0b0001,
        });
        assert_eq!(combo.selected_count(), 0, "the chord must empty the selection");
    }
}
