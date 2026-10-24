// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! DropdownMenu widget — a cascading/linked dropdown selector.
//!
//! The DropdownMenu displays a text field with a dropdown arrow. When clicked,
//! it expands to show a scrollable list of items. Each item has a value, label,
//! optional icon, enabled state, and optional children for submenu support.
//! Selecting an item emits an `item_selected` signal with the item's value.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::key_codes;
use crate::event::{Event, EventHandler};
use crate::render::{RenderCommand, RenderContext};
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::{
    expect_bool, expect_horizontal_alignment, expect_usize, horizontal_alignment_to_str,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::menu_toolbar::popup_reveal::{PopupReveal, RevealDirection};
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, IconName, IconSource, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Default height of the dropdown list when expanded.
const DEFAULT_DROPDOWN_HEIGHT: u32 = 200;
/// Height of each item row in the dropdown list.
const ITEM_HEIGHT: i32 = 30;
/// Padding inside the control.
const PADDING: i32 = 8;
/// Maximum number of items visible before scrolling.
const MAX_VISIBLE_ITEMS: usize = 8;

/// The record form of the `items` property: one row per **top-level** dropdown item.
///
/// # Why this is read-only, and why the rows stop at the top level
///
/// `DropdownItem` is a public-field struct with a nested `children` vector, and a record row cannot
/// carry a subtree. A *writable* `items` property would therefore have to invent children or silently
/// drop them — and a write that loses a submenu is precisely the silent-loss shape §19.6 forbids. The
/// rows publish what a document can hold faithfully (identity, label, enabled, typed icon) and a write
/// is refused with a reason instead of a lossy success; behaviour changes through the control's own
/// API (`add_item` / `set_item_icon_source` / `items_mut`), which is what "public-field collections
/// are a version boundary" prescribes.
pub const DROPDOWN_ITEM_SCHEMA: crate::widget::capability::RecordSchema =
    crate::widget::capability::RecordSchema {
        fields: &[
            crate::widget::capability::RecordField::required(
                "value",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::required(
                "label",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::optional(
                "enabled",
                crate::widget::capability::PropertyValueKind::Bool,
            ),
            crate::widget::capability::RecordField::optional(
                "icon_source",
                crate::widget::capability::PropertyValueKind::IconSpec,
            ),
        ],
        id_field: "value",
        ids_unique: true,
        ids_required: true,
    };

/// One row of [`DROPDOWN_ITEM_SCHEMA`]; refuses a source with no document form.
fn dropdown_item_record(
    item: &DropdownItem,
    icon_source: Option<IconSource>,
) -> Result<crate::widget::capability::RecordValue, CapabilityAccessError> {
    let mut record = crate::widget::capability::RecordValue::new()
        .with("value", CapabilityValue::String(item.value.clone()))
        .with("label", CapabilityValue::String(item.label.clone()))
        .with("enabled", CapabilityValue::Bool(item.enabled));
    match icon_source {
        // "No `icon_source` field" is how a document says "this row has no typed icon", and a row
        // that never had one is the normal case.
        None => {}
        Some(IconSource::Builtin(name)) => {
            record = record.with(
                "icon_source",
                CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Builtin(name)),
            );
        }
        // A borrowed static geometry cannot be written as a document value, and omitting it would be
        // indistinguishable from "no icon" — the same rule the six control-level owners follow.
        Some(IconSource::StaticGeometry(_)) => {
            return Err(CapabilityAccessError::IconSourceNotPersistable)
        }
    }
    Ok(record)
}

/// A single item in a DropdownMenu.
///
/// Each item has a value (string identifier), a display label, an optional
/// icon, an enabled state, and optional children for cascading submenus.
///
/// `Clone` is what lets an open child panel hold its own rows while the item tree it came from
/// stays owned by the control: a submenu that borrowed the tree could not outlive a mutation of
/// the parent list.
#[derive(Clone, Debug)]
pub struct DropdownItem {
    /// Unique value identifier for this item.
    pub value: String,
    /// Display label shown in the dropdown.
    pub label: String,
    /// Optional icon for additional visual context.
    pub icon: Option<String>,
    /// Whether this item is selectable.
    pub enabled: bool,
    /// Whether this item shows a check mark and toggles when chosen.
    pub checkable: bool,
    /// The current check state, meaningful only when [`Self::checkable`] is set.
    pub checked: bool,
    /// Optional child items for submenu support.
    pub children: Vec<DropdownItem>,
}

impl DropdownItem {
    /// Creates a new DropdownItem with the given value and label.
    pub fn new(value: &str, label: &str) -> Self {
        Self {
            value: value.to_string(),
            label: label.to_string(),
            icon: None,
            enabled: true,
            checkable: false,
            checked: false,
            children: Vec::new(),
        }
    }

    /// Sets the icon for this item.
    pub fn with_icon(mut self, icon: &str) -> Self {
        self.icon = Some(icon.to_string());
        self
    }

    /// Sets the enabled state for this item.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Sets the children for this item (submenu support).
    pub fn with_children(mut self, children: Vec<DropdownItem>) -> Self {
        self.children = children;
        self
    }

    /// Makes the item checkable and sets its initial check state.
    pub fn with_checked(mut self, checked: bool) -> Self {
        self.checkable = true;
        self.checked = checked;
        self
    }

    /// Toggles the check state, ignoring the call for a non-checkable item.
    pub fn toggle_checked(&mut self) {
        if self.checkable {
            self.checked = !self.checked;
        }
    }
}

/// The maximum number of **nested** submenu panels below the field's own list.
///
/// Re-exported from [`crate::widget::menu_toolbar::MAX_SUBMENU_DEPTH`] so the whole menu
/// family shares one depth contract. The field's own list is level 0, so the deepest visible
/// row sits at level 4.
pub use crate::widget::menu_toolbar::MAX_SUBMENU_DEPTH;

/// One open child panel: the rows it shows, its rectangle and its keyboard-active row.
///
/// The stack keeps one of these per open level, which is what makes a four-deep cascade one
/// control state instead of a chain of mutually exclusive `Option` fields.
struct SubmenuPanel {
    items: Vec<DropdownItem>,
    rect: Rect,
    active: Option<usize>,
}

/// The colours one dropdown list is painted with.
///
/// Resolved once per frame so the field's list and a child panel cannot drift apart: both read
/// the same palette, and a theme change moves them together.
#[derive(Clone, Copy)]
struct ListPalette {
    text: Color,
    disabled_text: Color,
    highlight: Color,
    separator: Color,
}

/// One list paint: the panel, the rows it shows, and what to mark as selected.
struct ListPaint<'a> {
    panel: Rect,
    items: &'a [DropdownItem],
    first: usize,
    end: usize,
    alignment: crate::core::Alignment,
    icon_sources: &'a [(crate::compat::String, IconSource)],
    selected_value: Option<&'a str>,
    highlight_selection: bool,
    /// Whether a row with children paints its disclosure arrow.
    ///
    /// Only the level that is allowed to open a deeper panel says `true`; the deepest
    /// panel says `false` because a fifth level is refused by [`MAX_SUBMENU_DEPTH`], and
    /// an arrow that opens nothing would be a promise the control cannot keep.
    branches_open: bool,
}

/// Paints the rows of one dropdown list.
///
/// The field's own list and a branch's child panel both come through here, so a submenu row is
/// the same row the parent list would draw — including its icon, alignment and disclosure arrow.
fn draw_item_list(
    context: &mut RenderContext,
    paint: ListPaint<'_>,
    style: &crate::style::WidgetStyle,
    palette: ListPalette,
) {
    let item_font = effective_font(style);
    for index in paint.first..paint.end {
        let Some(item) = paint.items.get(index) else {
            continue;
        };
        let row = Rect::new(
            paint.panel.x,
            paint.panel.y + (index - paint.first) as i32 * ITEM_HEIGHT,
            paint.panel.width,
            ITEM_HEIGHT as u32,
        );

        let is_selected = paint.highlight_selection
            && paint.selected_value.is_some_and(|value| item.value.as_str() == value);
        if is_selected {
            context.fill_rounded_rect(row, 2, palette.highlight);
        }

        let item_text_color = if !item.enabled { palette.disabled_text } else { palette.text };
        let mut item_x = row.x + PADDING;

        // A checkable row reserves its tick column whether or not it is checked, so the labels
        // of a checkable list stay in one column — the same rule a menu row follows.
        if item.checkable {
            let line = context.text_line(row, item_font);
            let side = line.height.clamp(1, 16);
            if item.checked {
                crate::widget::draw_icon_at(
                    context,
                    Rect::new(item_x, line.y, side, side),
                    item_text_color,
                    IconName::Check,
                );
            }
            item_x += 18;
        }

        // Item icon
        let icon_source =
            paint.icon_sources.iter().find(|(id, _)| id == &item.value).map(|(_, s)| *s);
        if let Some(source) = icon_source {
            let icon_line = context.text_line(row, item_font);
            let side = icon_line.height.clamp(1, 16);
            let icon_rect = Rect::new(item_x, icon_line.y, side, side);
            if let Err(error) =
                crate::widget::paint_icon(context, source.into(), icon_rect, item_text_color)
            {
                log::warn!("dropdown menu item icon failed: {error:?}");
            }
            item_x += 20;
        } else if let Some(ref item_icon) = item.icon {
            let icon_font = Font::simple("sans-serif", 13.0);
            // The icon and the label share the row, so both take their origin from the line
            // box centred in it rather than from the row's raw midpoint.
            let icon_line = context.text_line(row, &icon_font);
            context.draw_text(
                Point::new(item_x, icon_line.y),
                item_icon,
                &icon_font,
                item_text_color,
                HorizontalAlignment::Left,
            );
            item_x += 20;
        }

        // Item label, aligned in the same inner box the field's value uses.
        let item_line = context.text_line(row, item_font);
        let label_band = Rect::new(
            item_x,
            item_line.y,
            (row.x + row.width as i32 - PADDING - item_x).max(0) as u32,
            item_line.height,
        );
        let label_align = paint.alignment.to_horizontal().unwrap_or(HorizontalAlignment::Left);
        context.draw_text_fitted_cow(
            label_band,
            &item.label,
            item_font,
            item_text_color,
            label_align,
        );

        // Submenu indicator if item has children
        if paint.branches_open && !item.children.is_empty() && item.enabled {
            let sub_x = row.x + row.width as i32 - PADDING - 8;
            let sub_y = row.y + row.height as i32 / 2 - 4;
            context.execute_command(RenderCommand::DrawPath {
                points: vec![
                    Point::new(sub_x, sub_y),
                    Point::new(sub_x, sub_y + 8),
                    Point::new(sub_x + 5, sub_y + 4),
                ],
                closed: true,
                color: palette.disabled_text,
                filled: true,
                width: 1,
            });
        }

        // Separator line
        if index + 1 < paint.end {
            context.draw_line(
                Point::new(row.x + 4, row.y + row.height as i32),
                Point::new(row.x + row.width as i32 - 4, row.y + row.height as i32),
                palette.separator,
            );
        }
    }
}

/// DropdownMenu widget — a text field with a dropdown list.
///
/// Displays the currently selected value (or placeholder) with a dropdown
/// arrow. When expanded, shows a scrollable list of items. Selecting an
/// item updates the selected value and emits the `item_selected` signal.
pub struct DropdownMenu {
    base: BaseWidget,
    selected_value: Option<String>,
    items: Vec<DropdownItem>,
    /// Typed icon overrides, keyed by [`DropdownItem::value`] — the identity the item type declares.
    item_icon_sources: Vec<(crate::compat::String, IconSource)>,
    expanded: bool,
    /// The list's open/close reveal, so the list grows out of the field rather than appearing.
    ///
    /// A `bool` cannot express a popup mid-open, and the draw path needs the fraction. The flag
    /// above stays the *authority* (it is what `get("expanded")` answers, and what hit testing
    /// consults); this only says how far the transition has got.
    reveal: PopupReveal,
    /// Scroll offset for the item list.
    scroll_offset: usize,
    /// The open child panels, deepest last. One panel per level, up to [`MAX_SUBMENU_DEPTH`].
    submenus: Vec<SubmenuPanel>,
    /// The top-level row the pointer last addressed, so Right can open its submenu.
    active_index: Option<usize>,
    /// How the field's value and each item's label are aligned within their own row.
    ///
    /// Both runs take the same value because they are the same column of text seen twice -- what the
    /// field shows and what the list offers below it -- so aligning one and not the other would make
    /// the selected value and the row it was chosen from disagree about where the column starts.
    /// Horizontal only: a row is centred vertically in its band by the row's own layout, so a
    /// `top`/`bottom` value would be one this control could never honour --
    /// [`crate::widget::capability::coercion::expect_horizontal_alignment`] refuses those rather
    /// than accepting a write that does nothing. Defaults to left, so a caller that never asks
    /// behaves exactly as it did.
    alignment: crate::core::Alignment,
    /// Emitted when an item is selected, providing the item's value.
    pub item_selected: Signal1<String>,
    /// Emitted when a checkable item's check state changes, carrying its value.
    pub item_toggled: Signal1<String>,
    /// Emitted when a branch row opens its child panel, carrying the branch's label.
    pub submenu_requested: Signal1<String>,
    /// Emitted just before the list expands.
    pub about_to_show: GenericSignal,
    /// Emitted just after the list collapses.
    pub about_to_hide: GenericSignal,
}

impl DropdownMenu {
    /// Creates a new DropdownMenu widget with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::DropdownMenu, geometry, "DropdownMenu"),
            selected_value: None,
            items: Vec::new(),
            item_icon_sources: Vec::new(),
            expanded: false,
            reveal: PopupReveal::new(false),
            scroll_offset: 0,
            submenus: Vec::new(),
            active_index: None,
            alignment: crate::core::Alignment::Left,
            item_selected: Signal1::new(),
            item_toggled: Signal1::new(),
            submenu_requested: Signal1::new(),
            about_to_show: GenericSignal::new(),
            about_to_hide: GenericSignal::new(),
        }
    }

    /// Sets the selected item by value. If the value does not match any
    /// item, the selection is cleared.
    pub fn select(&mut self, value: &str) {
        if self.items.iter().any(|item| item.value == value && item.enabled) {
            self.selected_value = Some(value.to_string());
            self.base.request_redraw();
        }
    }

    /// Returns the currently selected value, if any.
    pub fn selected_value(&self) -> Option<String> {
        self.selected_value.clone()
    }

    /// The check state of the item identified by `value`, at any nesting depth.
    pub fn item_checked(&self, value: &str) -> Option<bool> {
        fn find(items: &[DropdownItem], value: &str) -> Option<bool> {
            for item in items {
                if item.value == value {
                    return Some(item.checked);
                }
                if let Some(found) = find(&item.children, value) {
                    return Some(found);
                }
            }
            None
        }
        find(&self.items, value)
    }

    /// Sets the check state of the item identified by `value`, at any nesting depth.
    ///
    /// Returns `false` when no item carries that value, or when the item is not
    /// checkable. Requests a repaint when the state actually changed.
    pub fn set_item_checked(&mut self, value: &str, checked: bool) -> bool {
        fn apply(items: &mut [DropdownItem], value: &str, checked: bool) -> Option<bool> {
            for item in items {
                if item.value == value {
                    if !item.checkable {
                        return Some(false);
                    }
                    if item.checked == checked {
                        return Some(false);
                    }
                    item.checked = checked;
                    return Some(true);
                }
                if let Some(changed) = apply(&mut item.children, value, checked) {
                    return Some(changed);
                }
            }
            None
        }
        let changed = apply(&mut self.items, value, checked).unwrap_or(false);
        if changed {
            // Every open child panel holds a copy of the branch's rows, so mirror the new
            // state into all of them or a panel would show a stale tick until it is reopened.
            for panel in &mut self.submenus {
                apply(&mut panel.items, value, checked);
            }
            self.item_toggled.emit(value.to_string());
            self.base.request_redraw();
        }
        changed
    }

    /// Every checked item value in the tree, in document order.
    pub fn checked_values(&self) -> Vec<String> {
        fn collect(items: &[DropdownItem], out: &mut Vec<String>) {
            for item in items {
                if item.checkable && item.checked {
                    out.push(item.value.clone());
                }
                collect(&item.children, out);
            }
        }
        let mut out = Vec::new();
        collect(&self.items, &mut out);
        out
    }

    /// The level that owns keyboard focus: the deepest open child panel, or the field's
    /// own list when no panel is open.
    fn focus_level(&self) -> usize {
        self.submenus.len()
    }

    /// The rows shown at `level`: the field's list at level 0, a panel's rows below it.
    fn items_at(&self, level: usize) -> Option<&[DropdownItem]> {
        if level == 0 {
            Some(&self.items)
        } else {
            self.submenus.get(level - 1).map(|panel| panel.items.as_slice())
        }
    }

    /// The visible window of `level` as `(first, end)` plus the level's row count.
    ///
    /// Only the field's own list scrolls; a child panel shows every row it holds, so its
    /// window is the whole list. Expressing both through one helper is what lets the
    /// navigation below run unchanged at every level.
    fn active_window(&self, level: usize) -> Option<(usize, usize, usize)> {
        let len = self.items_at(level)?.len();
        if level == 0 {
            let end = (self.scroll_offset + self.max_visible_items()).min(len);
            Some((self.scroll_offset, end, len))
        } else {
            Some((0, len, len))
        }
    }

    /// The keyboard-active row of `level`.
    fn active_at(&self, level: usize) -> Option<usize> {
        if level == 0 {
            self.active_index
        } else {
            self.submenus.get(level - 1).and_then(|panel| panel.active)
        }
    }

    /// Sets the keyboard-active row of `level`.
    fn set_active_at(&mut self, level: usize, index: Option<usize>) {
        if level == 0 {
            self.active_index = index;
        } else if let Some(panel) = self.submenus.get_mut(level - 1) {
            panel.active = index;
        }
    }

    /// Whether row `index` of `level` is present and enabled.
    fn row_enabled_at(&self, level: usize, index: usize) -> bool {
        self.items_at(level).and_then(|items| items.get(index)).is_some_and(|item| item.enabled)
    }

    /// Moves the keyboard-active row of `level` by `delta`, skipping disabled rows and
    /// rows outside the visible window.
    fn move_active_at(&mut self, level: usize, delta: i32) {
        if !self.expanded || delta == 0 {
            return;
        }
        let Some((first, end, _)) = self.active_window(level) else {
            return;
        };
        let count = end.saturating_sub(first);
        if count == 0 {
            return;
        }
        let start = match self.active_at(level) {
            Some(index) if index >= first && index < end => index as i32,
            _ if delta > 0 => first as i32 - 1,
            _ => end as i32,
        };
        let mut index = start;
        for _ in 0..count {
            index += delta;
            if index < first as i32 {
                index = end as i32 - 1;
            } else if index >= end as i32 {
                index = first as i32;
            }
            if self.row_enabled_at(level, index as usize) {
                self.set_active_at(level, Some(index as usize));
                self.base.request_redraw();
                return;
            }
        }
    }

    /// Jumps the keyboard-active row of `level` to one end of its visible window.
    fn jump_active_at(&mut self, level: usize, to_end: bool) {
        if !self.expanded {
            return;
        }
        let Some((first, end, _)) = self.active_window(level) else {
            return;
        };
        let found = if to_end {
            (first..end).rev().find(|index| self.row_enabled_at(level, *index))
        } else {
            (first..end).find(|index| self.row_enabled_at(level, *index))
        };
        if let Some(index) = found {
            self.set_active_at(level, Some(index));
            self.base.request_redraw();
        }
    }

    /// Selects the next enabled row of `level` whose label starts with `letter`.
    ///
    /// Repeated presses of the same letter cycle forward from the current row, which is
    /// the behaviour a list with no timing state can still promise.
    fn type_ahead_at(&mut self, level: usize, letter: char) {
        if !self.expanded {
            return;
        }
        let wanted = letter.to_ascii_lowercase();
        let Some(len) = self.items_at(level).map(|items| items.len()) else {
            return;
        };
        if len == 0 {
            return;
        }
        let start = match self.active_at(level) {
            Some(index) if index + 1 < len => index + 1,
            Some(_) => 0,
            None if level == 0 => self.scroll_offset,
            None => 0,
        };
        let mut landed = None;
        for offset in 0..len {
            let index = (start + offset) % len;
            let Some(item) = self.items_at(level).and_then(|items| items.get(index)) else {
                continue;
            };
            if item.enabled
                && item
                    .label
                    .chars()
                    .next()
                    .is_some_and(|first| first.to_ascii_lowercase() == wanted)
            {
                landed = Some(index);
                break;
            }
        }
        let Some(index) = landed else {
            return;
        };
        self.set_active_at(level, Some(index));
        if level == 0 {
            self.scroll_offset = self
                .scroll_offset
                .min(index)
                .max(index.saturating_add(1).saturating_sub(self.max_visible_items()));
        }
        self.base.request_redraw();
    }

    /// Moves the keyboard-active row of `level` by one page inside its visible window.
    ///
    /// Paging does not wrap: the point of PageDown is to move a screenful in one
    /// direction, and a page that wrapped to the top would undo itself on the last
    /// press. The landing row is the nearest enabled one, searching toward the target.
    fn page_active_at(&mut self, level: usize, down: bool) {
        if !self.expanded {
            return;
        }
        let Some((visible_start, visible_end, _)) = self.active_window(level) else {
            return;
        };
        if visible_end <= visible_start {
            return;
        }
        let page = self.max_visible_items().max(1);
        let current = self.active_at(level).unwrap_or(visible_start);
        let target = if down {
            current.saturating_add(page).min(visible_end - 1)
        } else {
            current.saturating_sub(page).max(visible_start)
        };
        let candidates: Vec<usize> = if down {
            (target..visible_end).collect()
        } else {
            (visible_start..=target).rev().collect()
        };
        if let Some(index) = candidates.into_iter().find(|index| self.row_enabled_at(level, *index))
        {
            self.set_active_at(level, Some(index));
            self.base.request_redraw();
        }
    }

    /// Returns the display label for the currently selected value.
    pub fn selected_label(&self) -> Option<String> {
        self.selected_value.as_ref().and_then(|val| {
            self.items.iter().find(|item| item.value == *val).map(|item| item.label.clone())
        })
    }

    /// Returns the index of the selected item, or `None` when nothing is selected.
    ///
    /// The index addresses the item list as a whole, so it matches the index of
    /// the entry whose value is currently selected in
    /// [`selected_value`](Self::selected_value).
    pub fn selected_index(&self) -> Option<usize> {
        let selected = self.selected_value.as_deref()?;
        self.items.iter().position(|item| item.value == selected)
    }

    /// Clears the selection, so no item reads as chosen.
    ///
    /// The `None` half of the `selected_index` property: the getter answers `Null`
    /// when nothing is selected, so the setter must be able to return to that state.
    /// No `item_selected` fires — nothing was selected.
    pub fn clear_selection(&mut self) {
        if self.selected_value.is_none() {
            return;
        }
        self.selected_value = None;
        self.base.request_redraw();
    }

    /// Selects the item at `index`.
    ///
    /// Mirrors what a click on that row does: an in-range, enabled item becomes
    /// the selection and emits [`item_selected`](Self::item_selected); an
    /// out-of-range index or a disabled item leaves the selection untouched.
    /// Returns whether the selection changed as a result.
    pub fn set_selected_index(&mut self, index: usize) -> Result<(), CapabilityAccessError> {
        let Some(item) = self.items.get(index) else {
            return Err(CapabilityAccessError::TypeMismatch);
        };
        if !item.enabled {
            // Disabled rows are presented but deliberately not selectable.
            return Ok(());
        }
        let value = item.value.clone();
        if self.selected_value.as_deref() == Some(value.as_str()) {
            return Ok(());
        }

        self.selected_value = Some(value.clone());
        self.item_selected.emit(value);
        self.base.request_redraw();
        Ok(())
    }

    /// Adds an item to the dropdown.
    pub fn add_item(&mut self, item: DropdownItem) {
        // A re-added `value` must not inherit the previous item's icon: the override is keyed by
        // identity, so a stale entry would silently attach the old glyph to a new row.
        self.item_icon_sources.retain(|(value, _)| *value != item.value);
        self.items.push(item);
        self.base.request_redraw();
    }

    /// Assigns a typed icon source to an item by its **value**.
    ///
    /// A typed source takes precedence while set, without discarding the item's legacy text icon.
    /// Passing `None` restores that text icon.
    ///
    /// # Why the item's value and not its index
    ///
    /// `DropdownItem::value` is documented as a *unique* identifier, so it is the identity the
    /// collection really has, and §19.6 requires the typed channel to be keyed on real identity
    /// rather than a position: an index association silently reattaches to whatever row later moves
    /// into that slot. The index-keyed form this replaced was justified by a claim that the
    /// collection had only positional identity, which its own item type disproves.
    pub fn set_item_icon_source(
        &mut self,
        value: &str,
        source: Option<IconSource>,
    ) -> Result<(), crate::widget::IconItemError> {
        match self.items.iter().filter(|item| item.value == value).count() {
            0 => {
                return Err(crate::widget::IconItemError::UnknownItemId {
                    id: crate::compat::String::from(value),
                })
            }
            1 => {}
            // Two rows sharing a value cannot be addressed apart, and picking the first would be a
            // silent coin flip (the same reasoning `IconItemError::AmbiguousItemId` records).
            _ => {
                return Err(crate::widget::IconItemError::DuplicateItemId {
                    id: crate::compat::String::from(value),
                })
            }
        }
        self.item_icon_sources.retain(|(existing, _)| existing != value);
        if let Some(source) = source {
            self.item_icon_sources.push((crate::compat::String::from(value), source));
        }
        self.base.request_redraw();
        Ok(())
    }

    /// The typed icon source assigned to `value`, if any.
    pub fn item_icon_source(&self, value: &str) -> Option<IconSource> {
        self.item_icon_sources.iter().find(|(id, _)| id == value).map(|(_, source)| *source)
    }

    /// Removes an item by value. Returns true if an item was removed.
    pub fn remove_item(&mut self, value: &str) -> bool {
        let len_before = self.items.len();
        self.items.retain(|item| item.value != value);
        let removed = self.items.len() < len_before;
        if removed {
            self.item_icon_sources.retain(|(id, _)| id != value);
            // Every open panel holds a snapshot of the removed row's subtree, so none of
            // them can be trusted to describe the list that is still there.
            self.close_all_submenus();
            // Clear selection if the removed item was selected
            if self.selected_value.as_deref() == Some(value) {
                self.selected_value = None;
            }
            self.base.request_redraw();
        }
        removed
    }

    /// Returns a reference to all items.
    pub fn items(&self) -> &[DropdownItem] {
        &self.items
    }

    /// Returns mutable access to the items, clearing typed icon assignments first.
    ///
    /// A caller can replace an item's `value` through this vector, which would leave an override
    /// pointing at identity the control can no longer verify as the same row; clearing is the honest
    /// answer rather than keeping an association the caller may have broken.
    pub fn items_mut(&mut self) -> &mut Vec<DropdownItem> {
        self.item_icon_sources.clear();
        self.close_all_submenus();
        &mut self.items
    }

    /// Returns the number of items.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// The configured visible-item cap, falling back to this control's historical
    /// constant when `menu_config` is off.
    fn max_visible_items(&self) -> usize {
        super::effects::current().visible_items_or(MAX_VISIBLE_ITEMS as u32).max(1) as usize
    }

    /// Clamps the scroll offset after the list or the cap changed.
    fn clamp_scroll(&mut self) {
        let max_offset = self.items.len().saturating_sub(self.max_visible_items());
        self.scroll_offset = self.scroll_offset.min(max_offset);
    }

    /// Returns whether the dropdown is currently expanded.
    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    /// Expands the dropdown to show the item list.
    pub fn expand(&mut self) {
        if self.expanded {
            return;
        }
        self.about_to_show.emit();
        self.expanded = true;
        self.reveal.set_open(true);
        self.scroll_offset = 0;
        self.submenus.clear();
        self.active_index = None;
        self.clamp_scroll();
        #[cfg(not(alloc_frugal))]
        crate::widget::runtime::set_popup_grab(self.id());
        self.base.request_redraw();
    }

    /// Collapses the dropdown, including any open child panel.
    pub fn collapse(&mut self) {
        if !self.expanded {
            return;
        }
        self.expanded = false;
        self.reveal.set_open(false);
        self.submenus.clear();
        self.active_index = None;
        #[cfg(not(alloc_frugal))]
        crate::widget::runtime::clear_popup_grab_for(self.id());
        self.base.request_redraw();
        self.about_to_hide.emit();
    }

    /// Whether a branch's child panel is currently showing.
    pub fn is_submenu_open(&self) -> bool {
        !self.submenus.is_empty()
    }

    /// How many nested panels are open below the field's list.
    ///
    /// `0` when nothing is open and at most [`MAX_SUBMENU_DEPTH`], because a branch at
    /// that depth is drawn with its arrow but refused by [`Self::open_submenu_at`].
    pub fn submenu_depth(&self) -> usize {
        self.submenus.len()
    }

    /// Toggles the expanded state.
    pub fn toggle(&mut self) {
        if self.expanded {
            self.collapse();
        } else {
            self.expand();
        }
    }

    /// How the field's value and each item's label are aligned within their own row.
    pub fn alignment(&self) -> crate::core::Alignment {
        self.alignment
    }

    /// Sets how the field's value and each item's label are aligned within their own row.
    ///
    /// Horizontal only. A `top`/`bottom` alignment is **ignored** because a row is centred
    /// vertically in its band by the row's own layout -- the property route refuses it through
    /// [`crate::widget::capability::coercion::expect_horizontal_alignment`], and this setter
    /// matching that keeps the two entry points from disagreeing.
    pub fn set_alignment(&mut self, alignment: crate::core::Alignment) {
        if alignment.to_horizontal().is_none() || self.alignment == alignment {
            return;
        }
        self.alignment = alignment;
        self.base.request_redraw();
    }

    /// The child panel's rectangle for `item_count` rows anchored to `anchor_row`.
    fn child_panel_rect(parent: Rect, anchor_row: Rect, item_count: usize) -> Rect {
        let height = (item_count as u32 * ITEM_HEIGHT as u32).max(1);
        Rect::new(parent.x + parent.width as i32, anchor_row.y, parent.width, height)
    }

    /// Row `index` inside a child panel.
    fn submenu_row_rect(panel: Rect, index: usize) -> Rect {
        Rect::new(panel.x, panel.y + index as i32 * ITEM_HEIGHT, panel.width, ITEM_HEIGHT as u32)
    }

    /// Collapses the deepest open child panel, leaving its parent level open.
    fn collapse_submenu(&mut self) {
        if self.submenus.pop().is_none() {
            return;
        }
        // The runtime clears the grab before delivering the outside press that collapsed
        // this level. The parent level is still open, so it must take the grab back or a
        // second outside press would reach the control behind it.
        #[cfg(not(alloc_frugal))]
        if self.expanded {
            crate::widget::runtime::set_popup_grab(self.id());
        }
        self.base.request_redraw();
    }

    /// Drops every open child panel, leaving the field's list open.
    fn close_all_submenus(&mut self) {
        if self.submenus.is_empty() {
            return;
        }
        self.submenus.clear();
        #[cfg(not(alloc_frugal))]
        if self.expanded {
            crate::widget::runtime::set_popup_grab(self.id());
        }
        self.base.request_redraw();
    }

    /// The panel rectangle at `level`: level 0 is the field's own list.
    fn panel_rect_at(&self, level: usize) -> Option<Rect> {
        if level == 0 {
            Some(self.dropdown_rect())
        } else {
            self.submenus.get(level - 1).map(|panel| panel.rect)
        }
    }

    /// Row `index` of `level`, when that row is inside the level's visible window.
    fn row_rect_at(&self, level: usize, index: usize) -> Option<Rect> {
        let panel = self.panel_rect_at(level)?;
        if level == 0 {
            let (first, end, _) = self.active_window(0)?;
            (index >= first && index < end).then(|| self.item_rect(index))
        } else {
            let len = self.items_at(level)?.len();
            (index < len).then(|| Self::submenu_row_rect(panel, index))
        }
    }

    /// Opens the child panel of the row at `index` in `level`.
    ///
    /// `level` is the level the row belongs to: 0 is the field's list, `n` is the panel
    /// opened by `submenus[n - 1]`. Opening replaces that level and every deeper one, so
    /// picking a sibling branch closes the branch that was open beside it.
    ///
    /// Activating a branch is not a selection: no [`Self::item_selected`] fires and the list
    /// stays expanded, which is the difference between a row that leads somewhere and a row
    /// that does something.
    ///
    /// A `level` at or beyond [`MAX_SUBMENU_DEPTH`] is refused: the row keeps its
    /// disclosure arrow but no fifth panel opens, which is the depth contract this control
    /// advertises.
    fn open_submenu_at(&mut self, level: usize, index: usize) {
        if level >= MAX_SUBMENU_DEPTH {
            return;
        }
        let Some(item) = self.items_at(level).and_then(|items| items.get(index)) else {
            return;
        };
        if !item.enabled || item.children.is_empty() {
            return;
        }
        let Some(parent) = self.panel_rect_at(level) else {
            return;
        };
        let Some(anchor_row) = self.row_rect_at(level, index) else {
            return;
        };
        let label = item.label.clone();
        let children = item.children.clone();
        let count = children.len();
        self.submenu_requested.emit(label);
        self.submenus.truncate(level);
        self.submenus.push(SubmenuPanel {
            items: children,
            rect: Self::child_panel_rect(parent, anchor_row, count),
            active: None,
        });
        self.set_active_at(level, Some(index));
        #[cfg(not(alloc_frugal))]
        crate::widget::runtime::set_popup_grab(self.id());
        self.base.request_redraw();
    }

    /// Activates the row at `index` in `level`: a branch opens the next panel, a leaf
    /// selects and closes the whole popup.
    fn activate_row_at(&mut self, level: usize, index: usize) {
        let Some(item) = self.items_at(level).and_then(|items| items.get(index)) else {
            return;
        };
        if !item.enabled {
            return;
        }
        let value = item.value.clone();
        let label = item.label.clone();
        let is_branch = !item.children.is_empty();
        let checkable = item.checkable;
        if is_branch {
            if level >= MAX_SUBMENU_DEPTH {
                // Depth contract: the row still announces that it leads somewhere, but a
                // fifth panel is refused. The signal is the host's way to show the rest.
                self.submenu_requested.emit(label);
            } else {
                self.open_submenu_at(level, index);
            }
            return;
        }
        if checkable {
            let next = !self.item_checked(&value).unwrap_or(false);
            self.set_item_checked(&value, next);
        }
        self.selected_value = Some(value.clone());
        self.item_selected.emit(value);
        self.collapse();
    }

    /// Activates a top-level row: a branch opens its child panel, a leaf selects.
    fn activate_top_level(&mut self, index: usize) {
        self.activate_row_at(0, index);
    }

    /// The index of the enabled row in `level` that a Right key should open.
    fn keyboard_branch_index_at(&self, level: usize) -> Option<usize> {
        let is_branch = |index: usize| {
            self.items_at(level)
                .and_then(|items| items.get(index))
                .is_some_and(|item| item.enabled && !item.children.is_empty())
        };
        self.active_at(level).filter(|index| is_branch(*index)).or_else(|| {
            // Only rows the level is actually showing can be opened: a branch scrolled out
            // of the window has no row to anchor its child panel to.
            let (first, end, _) = self.active_window(level)?;
            (first..end).find(|index| is_branch(*index))
        })
    }

    /// The deepest enabled submenu row at `pos`, as `(level, index)`.
    fn hit_test_submenu(&self, pos: Point) -> Option<(usize, usize)> {
        for (offset, panel) in self.submenus.iter().enumerate().rev() {
            if !panel.rect.contains_point(pos) {
                continue;
            }
            let level = offset + 1;
            for (index, item) in panel.items.iter().enumerate() {
                let row = Self::submenu_row_rect(panel.rect, index);
                if item.enabled && row.contains_point(pos) {
                    return Some((level, index));
                }
            }
        }
        None
    }

    /// Computes the dropdown list rectangle below the text field.
    fn dropdown_rect(&self) -> Rect {
        let geom = self.geometry();
        let item_count = self.items.len().min(self.max_visible_items());
        let height = (item_count as u32 * ITEM_HEIGHT as u32).min(DEFAULT_DROPDOWN_HEIGHT);
        Rect::new(geom.x, geom.y + geom.height as i32, geom.width, height)
    }

    /// Computes the rectangle for an item at the given index.
    fn item_rect(&self, index: usize) -> Rect {
        let drop = self.dropdown_rect();
        Rect::new(
            drop.x,
            drop.y + index.saturating_sub(self.scroll_offset) as i32 * ITEM_HEIGHT,
            drop.width,
            ITEM_HEIGHT as u32,
        )
    }

    /// Finds the item index at a given position, if any.
    fn hit_test_item(&self, pos: Point) -> Option<usize> {
        if !self.expanded {
            return None;
        }
        let drop = self.dropdown_rect();
        if !drop.contains_point(pos) {
            return None;
        }
        let visible_end = (self.scroll_offset + self.max_visible_items()).min(self.items.len());
        for i in self.scroll_offset..visible_end {
            let ir = self.item_rect(i);
            if ir.contains_point(pos) && self.items[i].enabled {
                return Some(i);
            }
        }
        None
    }
}

impl Widget for DropdownMenu {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    /// The field, the expanded list, and every child panel accept the pointer.
    ///
    /// The list hangs below the field's own rectangle and each panel sits to the right of
    /// the one that opened it; without this override a click on any of them was rejected by
    /// the hit test and never reached `hit_test_item`.
    fn contains_point(&self, point: crate::core::Point) -> bool {
        self.base().contains_point_with_touch_expansion(point)
            || (self.expanded && self.dropdown_rect().contains_point(point))
            || self.submenus.iter().any(|panel| panel.rect.contains_point(point))
    }

    fn has_open_popup(&self) -> bool {
        self.expanded
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(200, 200)
    }

    /// Advances the list's open/close reveal; `true` while it still owes frames.
    ///
    /// Forwarded to the trait so the animation bus drives it: `tick_animations` sweeps every
    /// mounted control and asks `tick`/`is_animating`, so a control that keeps its tick inherent
    /// is a control the host cannot advance — the defect the trait method exists to remove.
    fn tick(&mut self, delta_ms: u32) -> bool {
        self.reveal.tick(delta_ms)
    }

    fn is_animating(&self) -> bool {
        self.reveal.is_animating()
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit per control
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually emits. Without it a name
    /// is valid and inert, which is the silent failure `tools/check_event_signal_dyn.sh` exists to
    /// make impossible. The arm set is checked against the capability's published names, so this
    /// list cannot drift from what the control advertises.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        #[allow(unused_imports)]
        use crate::widget::capability::CapabilityValue;
        match name {
            "item_selected" => {
                Some(EventSignalRef::mapped("item_selected", &self.item_selected, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "submenu_requested" => {
                Some(EventSignalRef::mapped("submenu_requested", &self.submenu_requested, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "about_to_hide" => Some(EventSignalRef::unit("about_to_hide", &self.about_to_hide)),
            "about_to_show" => Some(EventSignalRef::unit("about_to_show", &self.about_to_show)),
            "item_toggled" => {
                Some(EventSignalRef::mapped("item_toggled", &self.item_toggled, |v| {
                    crate::widget::capability::CapabilityValue::String(v.clone())
                }))
            }
            _ => None,
        }
    }
}

/// `DropdownMenu`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `item_count` is derived from
/// the item list and has no setter, so writes are refused with
/// [`CapabilityAccessError::ReadOnlyProperty`].
impl WidgetProperties for DropdownMenu {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "items" => {
                Ok(CapabilityValue::Records(crate::widget::capability::RecordList::from_items(
                    &DROPDOWN_ITEM_SCHEMA,
                    self.items
                        .iter()
                        .map(|item| dropdown_item_record(item, self.item_icon_source(&item.value)))
                        .collect::<Result<Vec<_>, CapabilityAccessError>>()?,
                )))
            }
            "item_count" => Ok(CapabilityValue::UInt(self.item_count() as u64)),
            "selected_index" => match self.selected_index() {
                Some(index) => Ok(CapabilityValue::UInt(index as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "expanded" => Ok(CapabilityValue::Bool(self.is_expanded())),
            "alignment" => Ok(CapabilityValue::String(
                horizontal_alignment_to_str(self.alignment()).to_string(),
            )),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            // `Null` is the getter's "nothing selected" answer and the schema default,
            // so it must be writable back.
            "selected_index" => match value {
                CapabilityValue::Null => {
                    self.clear_selection();
                    Ok(())
                }
                other => self.set_selected_index(expect_usize(other)?),
            },
            "expanded" => {
                if expect_bool(value)? {
                    self.expand();
                } else {
                    self.collapse();
                }
                Ok(())
            }
            // Derived from the item list, which owns it.
            "item_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            // The rows are published so a document can **read** every item's typed icon, but a row
            // cannot express `DropdownItem::children`, so accepting a write would silently discard
            // subtrees. Refusing with the standard read-only error is the non-destructive answer;
            // `set_item_icon_source` and `add_item` are the write paths. See `DROPDOWN_ITEM_SCHEMA`.
            "items" => Err(CapabilityAccessError::ReadOnlyProperty),
            "alignment" => {
                self.set_alignment(expect_horizontal_alignment(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "items",
            "item_count",
            "selected_index",
            "expanded",
            "alignment",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for DropdownMenu {
    fn draw(&mut self, context: &mut RenderContext) {
        let geom = self.geometry();
        let is_enabled = self.base.is_enabled();
        // The effect policy is read once per frame; with `menu_config` disabled this is
        // `MenuEffects::legacy()` and every paint below takes the pre-wiring path.
        let effects = super::effects::current();

        // Chrome colours resolve explicit style first, then the theme's resolved
        // style for this control, and only then a literal. The theme step is what
        // makes an appearance switch visible; previously every colour below was a
        // hardcoded literal, so light and dark rendered identically.
        //
        // `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style();
        // The **effective font** — the resolved theme/caller font — for the field's value;
        // the list rows take the same font below (D09-STYLE-01).
        let font = effective_font(style);
        let theme = crate::style::resolved_theme_style_shared("dropdown_menu");
        // `dropdown_menu` is not a control kind in the role table, so it classifies as
        // `Surface`, whose background is `theme.colors.background` — the colour the
        // window paints. The field is therefore a step toward the foreground, so it
        // reads as a field rather than as a hole in the window.
        let resolved = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| crate::core::Color::border_of(resolved));
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        let background = resolved.blend(&text_color, 0.08);
        // A disabled field is a chrome state, so it is derived from the resolved
        // colours rather than from a second literal grey.
        let disabled_background = background.blend(&border, 0.25);
        let placeholder_color = text_color.blend(&background, 0.45);
        // Derived from the popup's own fill rather than blended toward it: `text_color.blend(
        // &background, 0.6)` moved the ink 0.6 of the way to the surface it sits on, which is the
        // lowest-contrast point available and read near-invisibly on a light popup. This is the
        // one binding every disabled row, arrow and label below reads, so the recede is one
        // decision rather than six.
        let disabled_text_color = self.base.disabled_ink_on(text_color, background);
        // The selected row and the separators are secondary chrome, derived from the
        // resolved pair so they follow the appearance.
        let item_highlight = background.blend(&text_color, 0.14);
        let separator = background.blend(&text_color, 0.1);

        // ── Draw text field background ──
        let bg_color = if !is_enabled { disabled_background } else { background };
        context.fill_rounded_rect(geom, 4, bg_color);
        context.draw_rounded_rect_stroke(geom, 4, border, 1);

        // ── Draw selected label or placeholder ──
        let display_text = self.selected_label().unwrap_or_else(|| "Select...".to_string());
        let text_color = if !is_enabled {
            disabled_text_color
        } else if self.selected_value.is_some() {
            text_color
        } else {
            placeholder_color
        };
        // `draw_text`'s origin is the glyph box's top-left, so `geom.y + geom.height / 2` put
        // that top edge on the field's middle line and drew the label half a line low. The
        // shared primitive returns the line box itself, centred in the field.
        let line = context.text_line(geom, font);
        // The value is aligned within the field's inner box (its own padding inset at both ends),
        // so a right- or centre-aligned value moves the run while the field it sits in does not.
        // Fitted rather than drawn at a point, because an anchor alone cannot express centre/right
        // over a width.
        let value_align = self.alignment.to_horizontal().unwrap_or(HorizontalAlignment::Left);
        context.draw_text_fitted_cow(
            Rect::new(
                geom.x + PADDING,
                line.y,
                geom.width.saturating_sub(PADDING as u32 * 2),
                line.height,
            ),
            &display_text,
            font,
            text_color,
            value_align,
        );

        // ── Draw dropdown arrow ──
        let arrow_x = geom.x + geom.width as i32 - PADDING - 10;
        let arrow_y = geom.y + geom.height as i32 / 2 - 2;
        let arrow_color =
            if !is_enabled { disabled_text_color } else { text_color.blend(&background, 0.25) };
        if self.expanded {
            // Upward-pointing triangle when expanded
            context.execute_command(RenderCommand::DrawPath {
                points: vec![
                    Point::new(arrow_x, arrow_y + 6),
                    Point::new(arrow_x + 10, arrow_y + 6),
                    Point::new(arrow_x + 5, arrow_y),
                ],
                closed: true,
                color: arrow_color,
                filled: true,
                width: 1,
            });
        } else {
            // Downward-pointing triangle when collapsed
            context.execute_command(RenderCommand::DrawPath {
                points: vec![
                    Point::new(arrow_x, arrow_y),
                    Point::new(arrow_x + 10, arrow_y),
                    Point::new(arrow_x + 5, arrow_y + 6),
                ],
                closed: true,
                color: arrow_color,
                filled: true,
                width: 1,
            });
        }

        // ── Draw dropdown list when expanded ──
        //
        // # Why the list is drawn clipped to a revealed rectangle
        //
        // The list used to appear at its full height on the frame `expanded` flipped, so opening a
        // dropdown was a hard cut — no sense of the list coming *out of* the field. It is now drawn
        // twice: once to establish the clip at the revealed extent, and once inside it. Clipping is
        // what makes the reveal honest: a list that only translated would, mid-open, still print
        // rows below its own edge, so the user would see items that are not yet reachable.
        //
        // `reveal.is_closed()` is checked rather than just the flag, so a list that is closing
        // keeps painting until it has finished shrinking instead of vanishing on the click.
        let palette = ListPalette {
            text: text_color,
            disabled_text: disabled_text_color,
            highlight: item_highlight,
            separator,
        };
        if self.expanded && !self.items.is_empty() {
            let drop = self.dropdown_rect();
            let revealed = self.reveal.revealed(
                drop,
                RevealDirection::Down,
                PopupReveal::travel_cap_for(crate::core::Size::new(drop.width, drop.height)),
            );
            if !self.reveal.is_closed() {
                // The shadow extends past the panel, so it is painted before the clip.
                if effects.shadows {
                    super::effects::paint_popup_shadow(context, revealed, &effects);
                }
                context.push_clip(revealed.x, revealed.y, revealed.width, revealed.height);
            }
            // Dropdown background with shadow
            let panel_fill = super::effects::popup_face(background, &effects);
            context.fill_rounded_rect(drop, 4, panel_fill);
            context.draw_rounded_rect_stroke(drop, 4, border, 2);

            // Draw visible items
            let end = (self.scroll_offset + self.max_visible_items()).min(self.items.len());
            draw_item_list(
                context,
                ListPaint {
                    panel: drop,
                    items: &self.items,
                    first: self.scroll_offset,
                    end,
                    alignment: self.alignment,
                    icon_sources: &self.item_icon_sources,
                    selected_value: self.selected_value.as_deref(),
                    highlight_selection: true,
                    branches_open: true,
                },
                style,
                palette,
            );

            // Scroll indicators if content is scrollable. Each is a `ChevronUp`/`ChevronDown`
            // **icon outline**, not the `▲`/`▼` text glyph (U+25B2/U+25BC are covered by no
            // bundled face, so those drew as 8x8 bitmap blocks).
            if self.scroll_offset > 0 {
                let box_ = Rect::new(drop.x, drop.y, drop.width.max(1), 12);
                crate::widget::draw_icon_centered(
                    context,
                    box_,
                    10,
                    disabled_text_color,
                    IconName::ChevronUp,
                );
            }
            if end < self.items.len() {
                let box_ =
                    Rect::new(drop.x, drop.y + drop.height as i32 - 12, drop.width.max(1), 12);
                crate::widget::draw_icon_centered(
                    context,
                    box_,
                    10,
                    disabled_text_color,
                    IconName::ChevronDown,
                );
            }
            // The clip is popped on the same path it was pushed, so the reveal cannot leak a clip
            // onto whatever the host draws next.
            if !self.reveal.is_closed() {
                context.pop_clip();
            }
        }

        // ── Draw the child panels ──
        //
        // Each panel hangs to the right of the level that opened it, outside the list's
        // reveal clip, so they are painted after the clip is popped. One panel per open
        // level, deepest last: opening another branch replaces that level and everything
        // below it, which `open_submenu_at` does by truncating the stack first.
        for (offset, panel) in self.submenus.iter().enumerate() {
            let panel_fill = super::effects::popup_face(background, &effects);
            context.fill_rounded_rect(panel.rect, 4, panel_fill);
            context.draw_rounded_rect_stroke(panel.rect, 4, border, 2);
            draw_item_list(
                context,
                ListPaint {
                    panel: panel.rect,
                    items: &panel.items,
                    first: 0,
                    end: panel.items.len(),
                    alignment: self.alignment,
                    icon_sources: &self.item_icon_sources,
                    selected_value: None,
                    highlight_selection: false,
                    // `offset + 1` is the level these rows sit on; the deepest panel
                    // cannot open another, so it paints no arrow.
                    branches_open: offset + 1 < MAX_SUBMENU_DEPTH,
                },
                style,
                palette,
            );
        }
    }
}

impl EventHandler for DropdownMenu {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button, .. } => {
                if *button != 1 {
                    return;
                }
                if self.geometry().contains_point(*pos) {
                    // Click on the text field — toggle expand
                    self.toggle();
                } else if self.expanded {
                    // The deepest panel wins: panels are laid out side by side, so only one
                    // can contain the point, and asking deepest-first keeps the order honest
                    // if a future layout ever overlaps them.
                    if let Some((level, index)) = self.hit_test_submenu(*pos) {
                        self.activate_row_at(level, index);
                    } else if let Some(index) = self.hit_test_item(*pos) {
                        self.active_index = Some(index);
                        self.activate_top_level(index);
                    } else if !self.submenus.is_empty() {
                        // An outside click collapses one level: the deepest panel first, the
                        // list itself on the click after the last panel is gone.
                        self.collapse_submenu();
                    } else {
                        // Click outside the dropdown — collapse
                        self.collapse();
                    }
                }
            }
            Event::KeyPress { key, modifiers } => match *key {
                key_codes::ESCAPE if !self.submenus.is_empty() => self.collapse_submenu(),
                key_codes::ESCAPE if self.expanded => self.collapse(),
                key_codes::RIGHT if self.expanded => {
                    // The deepest open panel owns the focus, so Right always means "go one
                    // level deeper from here". The depth gate lives in `open_submenu_at`.
                    let level = self.focus_level();
                    if let Some(index) = self.keyboard_branch_index_at(level) {
                        self.open_submenu_at(level, index);
                    }
                }
                key_codes::LEFT if !self.submenus.is_empty() => self.collapse_submenu(),
                key_codes::DOWN if self.expanded => self.move_active_at(self.focus_level(), 1),
                key_codes::UP if self.expanded => self.move_active_at(self.focus_level(), -1),
                key_codes::HOME if self.expanded => self.jump_active_at(self.focus_level(), false),
                key_codes::END if self.expanded => self.jump_active_at(self.focus_level(), true),
                key_codes::PAGE_DOWN if self.expanded => {
                    self.page_active_at(self.focus_level(), true)
                }
                key_codes::PAGE_UP if self.expanded => {
                    self.page_active_at(self.focus_level(), false)
                }
                key_codes::ENTER | key_codes::LINE_FEED if self.expanded => {
                    let level = self.focus_level();
                    if let Some(index) = self.active_at(level) {
                        self.activate_row_at(level, index);
                    }
                }
                // Type-ahead: a bare letter or digit jumps to the next row starting with it.
                // Chorded presses belong to the application's shortcuts, not to the list.
                key if *modifiers == 0 && self.expanded => {
                    if let Some(letter) = char::from_u32(key).filter(char::is_ascii_alphanumeric) {
                        self.type_ahead_at(self.focus_level(), letter);
                    }
                }
                _ => { /* Other keys are not relevant */ }
            },
            Event::Wheel { delta, modifiers: _ } => {
                let capacity = self.max_visible_items();
                if self.expanded && self.items.len() > capacity {
                    let max_offset = self.items.len() - capacity;
                    if delta.y > 0 {
                        self.scroll_offset = self.scroll_offset.saturating_sub(1);
                    } else if delta.y < 0 {
                        self.scroll_offset = (self.scroll_offset + 1).min(max_offset);
                    }
                    self.base.request_redraw();
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
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    #[test]
    fn dropdown_menu_default_creation() {
        let dm = DropdownMenu::new(Rect::new(0, 0, 200, 30));
        assert_eq!(dm.selected_value(), None);
        assert_eq!(dm.item_count(), 0);
        assert!(!dm.is_expanded());
    }

    #[test]
    fn dropdown_menu_add_remove_items() {
        let mut dm = DropdownMenu::new(Rect::new(0, 0, 200, 30));
        dm.add_item(DropdownItem::new("us", "United States"));
        dm.add_item(DropdownItem::new("ca", "Canada"));
        dm.add_item(DropdownItem::new("mx", "Mexico"));
        assert_eq!(dm.item_count(), 3);

        assert!(dm.remove_item("ca"));
        assert_eq!(dm.item_count(), 2);

        assert!(!dm.remove_item("nonexistent"));
        assert_eq!(dm.item_count(), 2);
    }

    #[test]
    fn dropdown_menu_typed_icons_follow_items_and_validate_identity() {
        let mut dm = DropdownMenu::new(Rect::new(0, 0, 200, 30));
        dm.add_item(DropdownItem::new("first", "First"));
        dm.add_item(DropdownItem::new("second", "Second").with_icon("legacy"));
        let source = IconSource::Builtin(IconName::Check);

        assert_eq!(
            dm.set_item_icon_source("missing", Some(source)),
            Err(crate::widget::IconItemError::UnknownItemId {
                id: crate::compat::String::from("missing")
            })
        );
        dm.set_item_icon_source("second", Some(source)).unwrap();
        assert_eq!(dm.items()[1].icon.as_deref(), Some("legacy"));
        assert!(dm.remove_item("first"));
        // The override follows the item's identity through the removal of another row.
        assert_eq!(dm.item_icon_source("second"), Some(source));

        dm.expand();
        dm.reveal.jump_to(true);
        let with_typed_source = render_to_svg(&mut dm);
        dm.set_item_icon_source("second", None).unwrap();
        let with_legacy_text = render_to_svg(&mut dm);
        let mut plain = DropdownMenu::new(Rect::new(0, 0, 200, 30));
        plain.add_item(DropdownItem::new("second", "Second"));
        plain.expand();
        plain.reveal.jump_to(true);
        let without_icon = render_to_svg(&mut plain);
        assert_eq!(
            with_typed_source.matches("<path").count(),
            without_icon.matches("<path").count() + 1,
            "the typed source adds one geometry path to the item"
        );
        assert!(with_legacy_text.matches("<path").count() > without_icon.matches("<path").count());

        dm.items_mut().reverse();
        assert_eq!(dm.item_icon_source("second"), None);
    }

    /// The rows publish each item's typed icon, and a source with no document form refuses the read.
    ///
    /// blue27 §19.10 asks for a structured `icon_source` on the entries that carry an icon, and
    /// §19.13 case I13 requires a runtime-only source to be either faithful or refused. The write side
    /// is read-only *on purpose* — see `DROPDOWN_ITEM_SCHEMA` — so both halves are asserted here.
    #[test]
    fn dropdown_menu_publishes_item_icons_as_rows_and_refuses_the_write() {
        use crate::widget::capability::{CapabilityValue, RecordList};
        use crate::widget::display_widgets::icon::IconData;

        static SOLID: IconData =
            IconData { name: "dd-solid", grid: 24, paths: &["M4-4H20V-20H4Z"] };

        let mut dm = DropdownMenu::new(Rect::new(0, 0, 200, 30));
        dm.add_item(DropdownItem::new("open", "Open").with_icon("legacy"));
        dm.add_item(DropdownItem::new("close", "Close"));
        dm.set_item_icon_source("open", Some(IconSource::Builtin(IconName::Save))).unwrap();

        let CapabilityValue::Records(list) = dm.get("items").expect("the rows read back") else {
            panic!("`items` must read as a record list");
        };
        assert_eq!(list.len(), 2);
        assert!(matches!(
            list.get_by_id("open").expect("present").get("icon_source"),
            Some(CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Builtin(
                IconName::Save
            )))
        ));
        assert!(
            list.get_by_id("close").expect("present").get("icon_source").is_none(),
            "a row with no typed icon omits the field rather than inventing one"
        );

        // The row list is a *view*: writing it would have to drop `children`, so it is refused.
        let empty: RecordList = RecordList::new(&super::DROPDOWN_ITEM_SCHEMA);
        assert_eq!(
            dm.set("items", CapabilityValue::Records(empty)),
            Err(crate::widget::capability::CapabilityAccessError::ReadOnlyProperty)
        );
        // ...and a refused write leaves the items exactly as they were.
        assert_eq!(dm.item_count(), 2);
        assert_eq!(dm.item_icon_source("open"), Some(IconSource::Builtin(IconName::Save)));

        // A borrowed source has no document form, so the read refuses instead of reporting "no icon".
        dm.set_item_icon_source("close", Some(IconSource::static_geometry(&SOLID))).unwrap();
        assert_eq!(
            dm.get("items"),
            Err(crate::widget::capability::CapabilityAccessError::IconSourceNotPersistable)
        );
    }

    #[test]
    fn dropdown_menu_select_and_signal() {
        let mut dm = DropdownMenu::new(Rect::new(0, 0, 200, 30));
        dm.add_item(DropdownItem::new("us", "United States"));
        dm.add_item(DropdownItem::new("ca", "Canada"));

        let selected = Arc::new(Mutex::new(None));
        let sel = selected.clone();
        dm.item_selected.connect(move |val| {
            *sel.lock().unwrap() = Some(val.to_string());
        });

        // Click on the text field to expand
        dm.handle_event(&Event::mouse_press(10, 10, 1));
        assert!(dm.is_expanded());

        // Click on the first item
        let ir = dm.item_rect(0);
        dm.handle_event(&Event::mouse_press(ir.x + 5, ir.y + 5, 1));

        assert_eq!(dm.selected_value(), Some("us".to_string()));
        assert_eq!(selected.lock().unwrap().as_deref(), Some("us"));
        assert!(!dm.is_expanded());
    }

    #[test]
    fn dropdown_menu_click_outside_collapses() {
        let mut dm = DropdownMenu::new(Rect::new(0, 0, 200, 30));
        dm.add_item(DropdownItem::new("a", "Item A"));
        dm.expand();
        assert!(dm.is_expanded());

        dm.handle_event(&Event::mouse_press(500, 500, 1));
        assert!(!dm.is_expanded());
    }

    #[test]
    fn dropdown_menu_escape_collapses() {
        let mut dm = DropdownMenu::new(Rect::new(0, 0, 200, 30));
        dm.expand();
        assert!(dm.is_expanded());

        dm.handle_event(&Event::key_press(27, 0));
        assert!(!dm.is_expanded());
    }

    /// A dropdown with one branch row ("Documents") and one leaf row ("Quit").
    fn dropdown_with_branch() -> DropdownMenu {
        let mut dm = DropdownMenu::new(Rect::new(0, 0, 200, 30));
        dm.add_item(DropdownItem::new("docs", "Documents").with_children(vec![
            DropdownItem::new("pdf", "PDF"),
            DropdownItem::new("txt", "Text"),
        ]));
        dm.add_item(DropdownItem::new("quit", "Quit"));
        dm
    }

    /// A dropdown whose branch chain is deeper than the control may open.
    ///
    /// Every level holds a branch named after the level below it plus a leaf named after
    /// its own level, and a second top-level branch exercises sibling replacement. The
    /// deepest branch ("Level 5") sits on level [`MAX_SUBMENU_DEPTH`], where opening is
    /// refused.
    fn dropdown_with_deep_chain() -> DropdownMenu {
        let l5 = DropdownItem::new("l5", "Level 5")
            .with_children(vec![DropdownItem::new("leaf5", "Leaf 5")]);
        let l4 = DropdownItem::new("l4", "Level 4")
            .with_children(vec![l5, DropdownItem::new("leaf4", "Leaf 4")]);
        let l3 = DropdownItem::new("l3", "Level 3")
            .with_children(vec![l4, DropdownItem::new("leaf3", "Leaf 3")]);
        let l2 = DropdownItem::new("l2", "Level 2")
            .with_children(vec![l3, DropdownItem::new("leaf2", "Leaf 2")]);
        let l1 = DropdownItem::new("l1", "Level 1")
            .with_children(vec![l2, DropdownItem::new("leaf1", "Leaf 1")]);
        let mut dm = DropdownMenu::new(Rect::new(0, 0, 200, 30));
        dm.add_item(l1);
        dm.add_item(
            DropdownItem::new("other", "Other")
                .with_children(vec![DropdownItem::new("other1", "Other 1")]),
        );
        dm
    }

    /// Clicks row `index` of `level` (0 is the field's list).
    fn click_row(dm: &mut DropdownMenu, level: usize, index: usize) {
        let row = if level == 0 {
            dm.item_rect(index)
        } else {
            DropdownMenu::submenu_row_rect(dm.submenus[level - 1].rect, index)
        };
        dm.handle_event(&Event::mouse_press(row.x + 5, row.y + 5, 1));
    }

    /// Activating a branch opens the child panel to the right and leaves the list expanded.
    #[test]
    fn dropdown_menu_branch_opens_its_child_panel_without_selecting() {
        let mut dm = dropdown_with_branch();
        let selected = Arc::new(AtomicUsize::new(0));
        let count = selected.clone();
        dm.item_selected.connect(move |_| {
            count.fetch_add(1, Ordering::SeqCst);
        });

        dm.expand();
        let row = dm.item_rect(0);
        dm.handle_event(&Event::mouse_press(row.x + 5, row.y + 5, 1));

        assert!(dm.is_expanded(), "a branch activation must not collapse the list");
        assert_eq!(selected.load(Ordering::SeqCst), 0, "a branch is not a selection");
        assert!(dm.is_submenu_open());
        let panel = dm.submenus[0].rect;
        assert!(panel.x >= dm.dropdown_rect().x + dm.dropdown_rect().width as i32);
        assert!(dm.contains_point(Point::new(panel.x + 5, panel.y + 5)));
    }

    /// The branch's own label is what `submenu_requested` carries.
    #[test]
    fn dropdown_menu_submenu_requested_carries_the_branch_label() {
        let mut dm = dropdown_with_branch();
        let label = Arc::new(Mutex::new(String::new()));
        let captured = label.clone();
        dm.submenu_requested.connect(move |text| {
            *captured.lock().unwrap() = text.to_string();
        });

        dm.expand();
        let row = dm.item_rect(0);
        dm.handle_event(&Event::mouse_press(row.x + 5, row.y + 5, 1));
        assert_eq!(*label.lock().unwrap(), "Documents");
    }

    /// Escape collapses one level: the child panel first, then the list.
    #[test]
    fn dropdown_menu_escape_collapses_the_child_panel_first() {
        let mut dm = dropdown_with_branch();
        dm.expand();
        let row = dm.item_rect(0);
        dm.handle_event(&Event::mouse_press(row.x + 5, row.y + 5, 1));
        assert!(dm.is_submenu_open());

        dm.handle_event(&Event::key_press(27, 0));
        assert!(dm.is_expanded(), "the first Escape returns to the parent list");
        assert!(!dm.is_submenu_open());

        dm.handle_event(&Event::key_press(27, 0));
        assert!(!dm.is_expanded());
    }

    /// Choosing a leaf in the child panel emits its value and closes the whole popup.
    #[test]
    fn dropdown_menu_child_leaf_selection_emits_item_selected() {
        let mut dm = dropdown_with_branch();
        let selected = Arc::new(Mutex::new(None));
        let captured = selected.clone();
        dm.item_selected.connect(move |value| {
            *captured.lock().unwrap() = Some(value.to_string());
        });

        dm.expand();
        let branch = dm.item_rect(0);
        dm.handle_event(&Event::mouse_press(branch.x + 5, branch.y + 5, 1));
        let panel = dm.submenus[0].rect;
        dm.handle_event(&Event::mouse_press(panel.x + 5, panel.y + 5, 1));

        assert_eq!(selected.lock().unwrap().as_deref(), Some("pdf"));
        assert_eq!(dm.selected_value(), Some("pdf".to_string()));
        assert!(!dm.is_expanded(), "a leaf selection closes the whole popup");
        assert!(!dm.is_submenu_open());
    }

    /// Right opens the branch, Left returns to the parent list.
    #[test]
    fn dropdown_menu_right_opens_a_branch_and_left_returns_to_the_list() {
        let mut dm = dropdown_with_branch();
        dm.expand();

        dm.handle_event(&Event::key_press(39, 0));
        assert!(dm.is_submenu_open());
        dm.handle_event(&Event::key_press(37, 0));
        assert!(!dm.is_submenu_open());
        assert!(dm.is_expanded());
    }

    /// Four Rights open four panels, each to the right of the level that opened it.
    #[test]
    fn dropdown_menu_opens_four_nested_child_panels() {
        let mut dm = dropdown_with_deep_chain();
        dm.expand();
        for _ in 0..MAX_SUBMENU_DEPTH {
            dm.handle_event(&Event::key_press(key_codes::RIGHT, 0));
        }

        assert_eq!(dm.submenu_depth(), MAX_SUBMENU_DEPTH);
        assert!(dm.is_expanded(), "opening panels must never collapse the list");
        let mut parent = dm.dropdown_rect();
        for panel in &dm.submenus {
            assert_eq!(panel.rect.x, parent.x + parent.width as i32);
            assert!(
                dm.contains_point(Point::new(panel.rect.x + 2, panel.rect.y + 2)),
                "every open panel accepts the pointer"
            );
            parent = panel.rect;
        }
        assert_eq!(
            dm.submenus[3].items[0].value, "l5",
            "the fourth panel holds the deepest branch"
        );
    }

    /// The depth contract: a branch on the deepest level announces itself but opens nothing.
    #[test]
    fn dropdown_menu_refuses_a_fifth_panel_but_announces_the_branch() {
        let mut dm = dropdown_with_deep_chain();
        let requested = Arc::new(Mutex::new(Vec::new()));
        let captured = requested.clone();
        dm.submenu_requested.connect(move |label| {
            captured.lock().unwrap().push(label.to_string());
        });
        dm.expand();
        for _ in 0..MAX_SUBMENU_DEPTH {
            dm.handle_event(&Event::key_press(key_codes::RIGHT, 0));
        }
        assert_eq!(dm.submenu_depth(), MAX_SUBMENU_DEPTH);
        let before = requested.lock().unwrap().clone();

        // "Level 5" is a branch, but it sits on the deepest allowed level.
        click_row(&mut dm, MAX_SUBMENU_DEPTH, 0);

        assert_eq!(dm.submenu_depth(), MAX_SUBMENU_DEPTH, "no fifth panel may open");
        assert!(dm.is_expanded());
        let after = requested.lock().unwrap().clone();
        assert_eq!(after.len(), before.len() + 1);
        assert_eq!(after.last().map(String::as_str), Some("Level 5"));
    }

    /// Escape closes one panel per press, deepest first, then the list itself.
    #[test]
    fn dropdown_menu_escape_unwinds_one_panel_per_press() {
        let mut dm = dropdown_with_deep_chain();
        dm.expand();
        for _ in 0..MAX_SUBMENU_DEPTH {
            dm.handle_event(&Event::key_press(key_codes::RIGHT, 0));
        }
        assert_eq!(dm.submenu_depth(), MAX_SUBMENU_DEPTH);

        for expected in (0..MAX_SUBMENU_DEPTH).rev() {
            dm.handle_event(&Event::key_press(key_codes::ESCAPE, 0));
            assert_eq!(dm.submenu_depth(), expected);
            assert!(dm.is_expanded(), "Escape leaves the field's list open until it is last");
        }
        dm.handle_event(&Event::key_press(key_codes::ESCAPE, 0));
        assert!(!dm.is_expanded());
    }

    /// A leaf in the deepest panel is selectable and closes the whole popup.
    #[test]
    fn dropdown_menu_deep_leaf_selection_emits_item_selected() {
        let mut dm = dropdown_with_deep_chain();
        let selected = Arc::new(Mutex::new(None));
        let captured = selected.clone();
        dm.item_selected.connect(move |value| {
            *captured.lock().unwrap() = Some(value.to_string());
        });
        dm.expand();
        for level in 0..MAX_SUBMENU_DEPTH {
            click_row(&mut dm, level, 0);
        }
        assert_eq!(dm.submenu_depth(), MAX_SUBMENU_DEPTH);

        click_row(&mut dm, MAX_SUBMENU_DEPTH, 1);

        assert_eq!(selected.lock().unwrap().as_deref(), Some("leaf4"));
        assert_eq!(dm.selected_value(), Some("leaf4".to_string()));
        assert!(!dm.is_expanded());
        assert_eq!(dm.submenu_depth(), 0);
    }

    /// Choosing a sibling branch replaces the deeper panels instead of stacking them.
    #[test]
    fn dropdown_menu_opening_a_sibling_branch_discards_the_deeper_panels() {
        let mut dm = dropdown_with_deep_chain();
        dm.expand();
        click_row(&mut dm, 0, 0);
        click_row(&mut dm, 1, 0);
        assert_eq!(dm.submenu_depth(), 2);

        click_row(&mut dm, 0, 1);

        assert_eq!(dm.submenu_depth(), 1);
        assert_eq!(dm.submenus[0].items[0].value, "other1");
    }

    /// Down and Enter act on the deepest panel, not on the field's list.
    #[test]
    fn dropdown_menu_keyboard_selects_a_leaf_inside_a_panel() {
        let mut dm = dropdown_with_deep_chain();
        let selected = Arc::new(Mutex::new(None));
        let captured = selected.clone();
        dm.item_selected.connect(move |value| {
            *captured.lock().unwrap() = Some(value.to_string());
        });
        dm.expand();
        dm.handle_event(&Event::key_press(key_codes::RIGHT, 0));
        assert_eq!(dm.submenu_depth(), 1);

        dm.handle_event(&Event::key_press(key_codes::DOWN, 0));
        // The first Down lands on the branch row, the second on the leaf beside it.
        dm.handle_event(&Event::key_press(key_codes::DOWN, 0));
        dm.handle_event(&Event::key_press(key_codes::ENTER, 0));

        assert_eq!(selected.lock().unwrap().as_deref(), Some("leaf1"));
        assert!(!dm.is_expanded());
    }

    #[test]
    fn dropdown_menu_selected_label() {
        let mut dm = DropdownMenu::new(Rect::new(0, 0, 200, 30));
        dm.add_item(DropdownItem::new("fr", "France"));
        dm.select("fr");
        assert_eq!(dm.selected_label(), Some("France".to_string()));
        assert_eq!(dm.selected_value(), Some("fr".to_string()));
    }

    #[test]
    fn dropdown_menu_remove_item_clears_selection() {
        let mut dm = DropdownMenu::new(Rect::new(0, 0, 200, 30));
        dm.add_item(DropdownItem::new("x", "Item X"));
        dm.select("x");
        assert_eq!(dm.selected_value(), Some("x".to_string()));

        dm.remove_item("x");
        assert_eq!(dm.selected_value(), None);
    }

    #[test]
    fn dropdown_menu_toggle() {
        let mut dm = DropdownMenu::new(Rect::new(0, 0, 200, 30));
        assert!(!dm.is_expanded());

        dm.toggle();
        assert!(dm.is_expanded());

        dm.toggle();
        assert!(!dm.is_expanded());
    }

    #[test]
    fn dropdown_menu_svg_output() {
        let mut dm = DropdownMenu::new(Rect::new(0, 0, 200, 30));
        dm.add_item(DropdownItem::new("op1", "Option 1").with_icon("★"));
        dm.add_item(DropdownItem::new("op2", "Option 2").with_enabled(false));
        dm.add_item(DropdownItem::new("op3", "Option 3"));
        dm.select("op1");
        dm.expand();

        let svg = render_to_svg(&mut dm);
        assert!(svg.starts_with("<svg"), "SVG should start with <svg, got: {svg:.60}");
        assert!(svg.ends_with("</svg>"), "SVG should end with </svg>");
    }

    /// The list is reachable from the keyboard: arrows skip disabled rows, digits/letters
    /// jump by first letter, and Enter commits the active row.
    #[test]
    fn dropdown_menu_keyboard_walks_and_commits() {
        let mut dm = DropdownMenu::new(Rect::new(0, 0, 200, 30));
        dm.add_item(DropdownItem::new("a", "Alpha"));
        dm.add_item(DropdownItem::new("b", "Beta").with_enabled(false));
        dm.add_item(DropdownItem::new("c", "Charlie"));
        dm.expand();

        dm.handle_event(&Event::key_press(40, 0));
        assert_eq!(dm.active_index, Some(0));
        dm.handle_event(&Event::key_press(40, 0));
        assert_eq!(dm.active_index, Some(2), "Down must step over the disabled row");
        dm.handle_event(&Event::key_press(38, 0));
        assert_eq!(dm.active_index, Some(0));

        // Type-ahead selects the next label beginning with the letter.
        dm.handle_event(&Event::key_press(67, 0));
        assert_eq!(dm.active_index, Some(2), "C jumps to Charlie");

        let selected = Arc::new(Mutex::new(None));
        let captured = selected.clone();
        dm.item_selected.connect(move |value| {
            *captured.lock().unwrap() = Some(value.to_string());
        });
        dm.handle_event(&Event::key_press(13, 0));
        assert_eq!(selected.lock().unwrap().as_deref(), Some("c"));
        assert!(!dm.is_expanded());
    }

    /// Home/End and PageUp/PageDown address the visible window, not the whole list.
    #[test]
    fn dropdown_menu_keyboard_home_end_and_paging() {
        let mut dm = DropdownMenu::new(Rect::new(0, 0, 200, 30));
        for index in 0..10 {
            dm.add_item(DropdownItem::new(&format!("v{index}"), &format!("Item {index}")));
        }
        dm.expand();
        dm.handle_event(&Event::key_press(35, 0));
        assert_eq!(dm.active_index, Some(7), "End stops at the last visible row");
        dm.handle_event(&Event::key_press(36, 0));
        assert_eq!(dm.active_index, Some(0));
        dm.handle_event(&Event::key_press(34, 0));
        assert_eq!(dm.active_index, Some(7), "PageDown moves within the visible window");
    }

    /// about_to_show / about_to_hide bracket the expansion exactly once per transition.
    #[test]
    fn dropdown_menu_announces_show_and_hide() {
        let mut dm = DropdownMenu::new(Rect::new(0, 0, 200, 30));
        dm.add_item(DropdownItem::new("a", "Alpha"));
        let shown = Arc::new(AtomicUsize::new(0));
        let hidden = Arc::new(AtomicUsize::new(0));
        let s = shown.clone();
        let h = hidden.clone();
        dm.about_to_show.connect(move || {
            s.fetch_add(1, Ordering::SeqCst);
        });
        dm.about_to_hide.connect(move || {
            h.fetch_add(1, Ordering::SeqCst);
        });

        dm.expand();
        dm.expand();
        assert_eq!(shown.load(Ordering::SeqCst), 1, "an already-open list must not re-announce");
        dm.collapse();
        dm.collapse();
        assert_eq!(hidden.load(Ordering::SeqCst), 1);
    }

    /// A checkable row toggles on selection and reports its value through `item_toggled`.
    #[test]
    fn dropdown_menu_checkable_rows_toggle() {
        let mut dm = DropdownMenu::new(Rect::new(0, 0, 200, 30));
        dm.add_item(DropdownItem::new("w", "Wrap").with_checked(false));
        dm.add_item(DropdownItem::new("plain", "Plain"));
        assert!(dm.items()[0].checkable);
        assert_eq!(dm.item_checked("w"), Some(false));

        let toggled = Arc::new(Mutex::new(Vec::new()));
        let sink = toggled.clone();
        dm.item_toggled.connect(move |value| {
            sink.lock().unwrap().push(value.to_string());
        });
        dm.expand();
        let row = dm.item_rect(0);
        dm.handle_event(&Event::mouse_press(row.x + 5, row.y + 5, 1));
        assert_eq!(dm.item_checked("w"), Some(true));
        assert_eq!(dm.checked_values(), vec!["w".to_string()]);
        assert_eq!(&*toggled.lock().unwrap(), &["w".to_string()]);

        assert!(!dm.set_item_checked("plain", true), "a non-checkable row refuses a check");
        assert_eq!(dm.item_checked("plain"), Some(false));
        assert_eq!(dm.item_checked("missing"), None);
    }
}
