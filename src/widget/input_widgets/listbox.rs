// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! List box widget.
use crate::compat::{Instant, RefCell, String, ToString, Vec};
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::dnd::{DragPayload, DragSource, DropEffect, DropTarget};
use crate::event::{Event, EventHandler, WheelScroll};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};

use crate::widget::capability::coercion::{
    expect_f32, expect_horizontal_alignment, expect_list_box_selection_mode, expect_usize,
    horizontal_alignment_to_str,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{focus_ring_color, FocusRing, FOCUS_RING_WIDTH};
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

/// The index a row takes after the row at `from` is moved to `to`.
///
/// Every index-based piece of state the list keeps — the selection, the cursor row and the
/// range anchor — uses this one rule, so a reorder cannot leave one of them pointing at a
/// different item than the other two.
fn index_after_move(index: usize, from: usize, to: usize) -> usize {
    if index == from {
        to
    } else if from < index && index <= to {
        index - 1
    } else if to <= index && index < from {
        index + 1
    } else {
        index
    }
}

/// List box widget.
pub struct ListBox {
    base: BaseWidget,
    items: Vec<String>,
    /// Per-row presentation metadata, parallel to [`Self::items`].
    ///
    /// The label vector is kept as the public item accessor so existing callers (and the
    /// property bridge, which publishes `Vec<String>`) do not have to unwrap a struct to read a
    /// row. The metadata vector is maintained by every mutation beside it.
    item_meta: Vec<ListBoxItem>,
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
    /// Emitted with the effect of a drag that began in this list.
    ///
    /// The coordinator reports the same effect through `DragSource::on_drag_finished`; this
    /// signal is the observable form for a host that holds the control by id.
    pub drag_finished: Signal1<DropEffect>,
    /// The row a drag taken from this list started on.
    ///
    /// Taken in `drag_payload` and consumed by `on_drag_finished`: a transfer to another list
    /// inserts the row there, and this is the copy the source then removes. An index is only
    /// meaningful inside the list that produced it, which is why the payload's item id is not
    /// reused for the removal — the rows may have shifted by then.
    pending_drag_row: core::cell::Cell<Option<usize>>,
    /// Whether the drop that ended the current gesture was committed **by this list**.
    ///
    /// A reorder is already finished when `on_drop` returns, so the source half must not remove
    /// the row a second time. A transfer commits in the target's list, leaving this false, and
    /// that is exactly when the source's copy has to go.
    drop_committed_here: core::cell::Cell<bool>,
    /// Number of columns a row is split into. One is a plain list.
    columns: usize,
    /// Which gestures may start an item rename.
    edit_triggers: EditTriggers,
    /// The prefix typed since the last navigation, for first-letter type-ahead.
    type_ahead: String,
    /// When the current type-ahead prefix began, for the shared 800 ms rule (I-P06).
    type_ahead_started: Option<Instant>,
    /// Emitted when a rename gesture lands on a row. The host owns the editor.
    pub rename_requested: Signal1<usize>,
    /// Emitted with the row whose label or presentation metadata changed.
    ///
    /// A host that mirrors this list in its own model could previously only learn that *some*
    /// change had happened through `selection_changed` (which is about selection) or by
    /// rebuilding the whole list. This names the row, so an item update is one notification
    /// about one index.
    pub item_changed: Signal1<usize>,
    /// Bumped whenever `items` or `item_meta` changes.
    ///
    /// The row icons' advances are parallel to `item_meta`, so a reorder, an insertion or a
    /// removal has to make every stored advance unreachable even when no icon text changed.
    item_revision: u64,
    /// Memoised advances of the row icons, parallel to `item_meta`.
    icon_advance_cache: RefCell<Option<IconAdvanceCache>>,
}

/// Memoised per-row icon advances, parallel to `item_meta`.
///
/// `None` means "not measured yet"; a row without an icon never reads its slot.
struct IconAdvanceCache {
    revision: u64,
    font: Font,
    dpi_bits: u32,
    advances: Vec<Option<i32>>,
}

/// Presentation metadata for one list row.
///
/// The row's text stays in the list's label vector; this carries what a list tends to grow next:
/// an icon, an optional check state, a disabled flag and a caller-owned data token. Every field
/// is optional or defaults to the plain state, so a list that never touches the type paints
/// exactly as it did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListBoxItem {
    icon: Option<String>,
    checked: Option<bool>,
    enabled: bool,
    data: Option<String>,
}

impl Default for ListBoxItem {
    fn default() -> Self {
        Self { icon: None, checked: None, enabled: true, data: None }
    }
}

impl ListBoxItem {
    /// Creates a plain, enabled row with no icon and no check box.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the row's icon text (a glyph or short marker) drawn before the label.
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Gives the row a check box with the given state.
    pub fn with_checked(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
        self
    }

    /// Marks the row as disabled, so it cannot be selected or activated.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Attaches a caller-owned data token to the row.
    pub fn with_data(mut self, data: impl Into<String>) -> Self {
        self.data = Some(data.into());
        self
    }

    /// The row's icon text, if it has one.
    pub fn icon(&self) -> Option<&str> {
        self.icon.as_deref()
    }

    /// The row's check state, or `None` when it has no check box.
    pub fn checked(&self) -> Option<bool> {
        self.checked
    }

    /// Whether the row can be selected and activated.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// The caller-owned data token, if one was attached.
    pub fn data(&self) -> Option<&str> {
        self.data.as_deref()
    }
}

/// How a [`ListBox::scroll_to_item`] call places the requested row.
///
/// The three "position at" modes are Qt's `ScrollHint` spellings: a caller that wants row 7
/// pinned to the top and a caller that only wants it *visible* are asking different
/// questions, and the old private `scroll(delta)` could express neither.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollToMode {
    /// Scroll only as far as needed to bring the row inside the band.
    #[default]
    EnsureVisible,
    /// Put the row at the top of the band.
    PositionAtTop,
    /// Centre the row in the band where the list allows it.
    PositionAtCenter,
    /// Put the row at the bottom of the band.
    PositionAtBottom,
}

/// Which gestures may begin renaming a list row.
///
/// A rename is a **request**, not an in-place editor: the control emits
/// [`ListBox::rename_requested`] and the host decides whether to mount an editor over the row.
/// That is the only shape that works when the host owns undo, validation and the commit target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditTriggers(u8);

impl EditTriggers {
    /// No gesture starts a rename.
    pub const NONE: Self = Self(0);
    /// A double-click on a row starts a rename.
    pub const DOUBLE_CLICK: Self = Self(1);
    /// The F2 key on the current row starts a rename.
    pub const F2: Self = Self(2);
    /// Both common gestures start a rename.
    pub const DEFAULT: Self = Self(Self::DOUBLE_CLICK.0 | Self::F2.0);

    /// Whether every trigger in `other` is enabled here.
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// The union of two trigger sets.
    pub fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl Default for EditTriggers {
    fn default() -> Self {
        Self::NONE
    }
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
            item_meta: Vec::new(),
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
            drag_finished: Signal1::new(),
            pending_drag_row: core::cell::Cell::new(None),
            drop_committed_here: core::cell::Cell::new(false),
            columns: 1,
            edit_triggers: EditTriggers::NONE,
            type_ahead: String::new(),
            type_ahead_started: None,
            rename_requested: Signal1::new(),
            item_changed: Signal1::new(),
            item_revision: 0,
            icon_advance_cache: RefCell::new(None),
        }
    }
    /// Bumped whenever `items` or `item_meta` changes, so the icon-advance cache invalidates.
    fn invalidate_item_caches(&mut self) {
        self.item_revision = self.item_revision.wrapping_add(1);
    }

    /// The measured advance of a row's icon, memoised for the current items, font and scale.
    fn icon_advance(&self, context: &RenderContext, index: usize, icon: &str, font: &Font) -> i32 {
        let dpi_bits = context.dpi_scale().to_bits();
        if let Ok(cache) = self.icon_advance_cache.try_borrow() {
            if let Some(cache) = cache.as_ref() {
                if cache.revision == self.item_revision
                    && cache.font == *font
                    && cache.dpi_bits == dpi_bits
                {
                    if let Some(Some(advance)) = cache.advances.get(index) {
                        return *advance;
                    }
                }
            }
        }
        let advance = context.measure_text(icon, font).width as i32;
        if let Ok(mut cache) = self.icon_advance_cache.try_borrow_mut() {
            let stale = cache.as_ref().is_none_or(|cache| {
                cache.revision != self.item_revision
                    || cache.font != *font
                    || cache.dpi_bits != dpi_bits
            });
            if stale {
                let mut advances = Vec::new();
                advances.resize(self.item_meta.len(), None);
                *cache = Some(IconAdvanceCache {
                    revision: self.item_revision,
                    font: font.clone(),
                    dpi_bits,
                    advances,
                });
            }
            if let Some(cache) = cache.as_mut() {
                if cache.advances.len() < self.item_meta.len() {
                    cache.advances.resize(self.item_meta.len(), None);
                }
                if let Some(slot) = cache.advances.get_mut(index) {
                    *slot = Some(advance);
                }
            }
        }
        advance
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
        let index = self.items.len();
        self.items.push(text);
        self.item_meta.push(ListBoxItem::default());
        self.invalidate_item_caches();
        self.item_changed.emit(index);
    }
    /// Adds multiple items.
    pub fn add_items(&mut self, items: Vec<String>) {
        let first = self.items.len();
        let added = items.len();
        self.items.extend(items);
        self.item_meta.resize(self.item_meta.len() + added, ListBoxItem::default());
        self.invalidate_item_caches();
        for index in first..first + added {
            self.item_changed.emit(index);
        }
    }
    /// Inserts an item at specified position.
    pub fn insert_item(&mut self, index: usize, text: String) {
        if index <= self.items.len() {
            self.items.insert(index, text);
            self.item_meta.insert(index.min(self.item_meta.len()), ListBoxItem::default());
            self.invalidate_item_caches();
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
            self.item_changed.emit(index);
        }
    }
    /// Adds `index` to the selection, keeping `selected_indices` sorted. Returns whether it was new.
    ///
    /// # Why the order is an invariant
    ///
    /// [`ListBox::selected_indices`] reads as "a set of rows in row order" (its own test asserts
    /// `&[1, 3]`, not `[3, 1]`), and [`ListBox::is_selected`] answers from that order with a binary
    /// search. The `Multi` path used to `push` straight onto the end, so selecting rows 3 then 1
    /// published `[3, 1]` *and* made the lookup miss row 1 on the next repaint — the row was
    /// selected and drawn unselected. `select_with_modifiers` already sorted after its push; this is
    /// the same rule for the plain path.
    fn add_to_selection(&mut self, index: usize) -> bool {
        match self.selected_indices.binary_search(&index) {
            Ok(_) => false,
            Err(position) => {
                self.selected_indices.insert(position, index);
                true
            }
        }
    }

    /// Removes item at specified index.
    pub fn remove_item(&mut self, index: usize) {
        if index < self.items.len() {
            self.items.remove(index);
            if index < self.item_meta.len() {
                self.item_meta.remove(index);
            }
            self.invalidate_item_caches();
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
            self.item_changed.emit(index);
        }
    }

    /// Moves the item at `from` to `to`, keeping the selection on the items it names.
    ///
    /// A move onto the same row is accepted without changing anything, so a drop aimed at
    /// the dragged row still reports that the list accepted it. Returns `false` only for an
    /// out-of-range index, which can only be a caller error.
    pub fn move_row(&mut self, from: usize, to: usize) -> bool {
        if from >= self.items.len() || to >= self.items.len() {
            return false;
        }
        if from == to {
            return true;
        }
        let item = self.items.remove(from);
        self.items.insert(to, item);
        if from < self.item_meta.len() {
            let meta = self.item_meta.remove(from);
            self.item_meta.insert(to.min(self.item_meta.len()), meta);
        }
        self.invalidate_item_caches();
        for index in &mut self.selected_indices {
            *index = index_after_move(*index, from, to);
        }
        // The mapping moves the dragged row ahead of rows that were behind it, so the sorted
        // order `is_selected` binary-searches has to be restored after the transformation.
        self.selected_indices.sort_unstable();
        self.current_row = self.current_row.map(|index| index_after_move(index, from, to));
        self.anchor = self.anchor.map(|index| index_after_move(index, from, to));
        self.selection_changed.emit();
        // Both ends of the move name a row whose content changed; the rows in between moved
        // too, but a move is one gesture and these are the two the caller asked about.
        self.item_changed.emit(from);
        self.item_changed.emit(to);
        self.base.request_redraw();
        true
    }

    /// Clears all items.
    pub fn clear(&mut self) {
        self.items.clear();
        self.item_meta.clear();
        self.invalidate_item_caches();
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
    ///
    /// A binary search rather than a linear scan: `draw` asks this once per visible row, so the scan
    /// made one frame cost O(visible rows × selected rows) (WDGT-09). The list is kept sorted by
    /// every mutation — see [`ListBox::add_to_selection`].
    pub fn is_selected(&self, index: usize) -> bool {
        self.selected_indices.binary_search(&index).is_ok()
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
        if index >= self.items.len() || !self.is_item_enabled(index) {
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
                if self.add_to_selection(index) {
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
        if index >= self.items.len() || !self.is_item_enabled(index) {
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
        let height = height.max(1.0);
        if self.item_height == height {
            return;
        }
        self.item_height = height;
        self.base.request_redraw();
    }
    /// Returns all items.
    pub fn items(&self) -> &[String] {
        &self.items
    }

    /// The presentation metadata for a row, or `None` when the index is out of range.
    pub fn item_meta(&self, index: usize) -> Option<&ListBoxItem> {
        self.item_meta.get(index)
    }

    /// Replaces a row's presentation metadata.
    ///
    /// An out-of-range index is ignored, matching the label mutators. Repaints only when the
    /// metadata actually changes.
    pub fn set_item_meta(&mut self, index: usize, meta: ListBoxItem) {
        let Some(slot) = self.item_meta.get_mut(index) else {
            return;
        };
        if *slot == meta {
            return;
        }
        *slot = meta;
        self.invalidate_item_caches();
        self.item_changed.emit(index);
        self.base.request_redraw();
    }

    /// Sets a row's icon text, or clears it with `None`.
    pub fn set_item_icon(&mut self, index: usize, icon: Option<String>) {
        if let Some(meta) = self.item_meta.get_mut(index) {
            if meta.icon == icon {
                return;
            }
            meta.icon = icon;
            self.invalidate_item_caches();
            self.item_changed.emit(index);
            self.base.request_redraw();
        }
    }

    /// Gives a row a check box in the given state, or removes it with `None`.
    pub fn set_item_checked(&mut self, index: usize, checked: Option<bool>) {
        if let Some(meta) = self.item_meta.get_mut(index) {
            if meta.checked == checked {
                return;
            }
            meta.checked = checked;
            self.item_changed.emit(index);
            self.base.request_redraw();
        }
    }

    /// Enables or disables a row. A disabled row cannot be selected or activated.
    pub fn set_item_enabled(&mut self, index: usize, enabled: bool) {
        if let Some(meta) = self.item_meta.get_mut(index) {
            if meta.enabled == enabled {
                return;
            }
            meta.enabled = enabled;
            self.item_changed.emit(index);
            self.base.request_redraw();
        }
    }

    /// Attaches a caller-owned data token to a row, or clears it with `None`.
    pub fn set_item_data(&mut self, index: usize, data: Option<String>) {
        if let Some(meta) = self.item_meta.get_mut(index) {
            if meta.data == data {
                return;
            }
            meta.data = data;
            self.item_changed.emit(index);
            self.base.request_redraw();
        }
    }

    /// Whether a row can be selected. An out-of-range row is not selectable.
    pub fn is_item_enabled(&self, index: usize) -> bool {
        self.item_meta.get(index).is_none_or(|meta| meta.enabled)
    }

    /// The indices whose check boxes are checked, in row order.
    pub fn checked_indices(&self) -> Vec<usize> {
        self.item_meta
            .iter()
            .enumerate()
            .filter_map(|(index, meta)| (meta.checked == Some(true)).then_some(index))
            .collect()
    }

    /// Flips a row's check box when it has one.
    ///
    /// Returns whether a check box was there to flip. A check box is presentation state, so
    /// this does not touch the selection and does not emit `selection_changed`.
    pub fn toggle_item_checked(&mut self, index: usize) -> bool {
        let Some(meta) = self.item_meta.get_mut(index) else {
            return false;
        };
        let Some(checked) = meta.checked else {
            return false;
        };
        meta.checked = Some(!checked);
        self.item_changed.emit(index);
        self.base.request_redraw();
        true
    }

    /// Sorts the rows by label.
    ///
    /// # Why the selection is cleared
    ///
    /// A selected row is an index into the old order. Sorting moves the row the index names,
    /// so remapping it would silently select a different label. Clearing is the only outcome
    /// that cannot lie; a caller that needs to preserve a selection should sort its own model
    /// and call [`ListBox::set_items`] equivalent mutators itself.
    pub fn sort_items(&mut self) {
        // Ascending byte order is the convenient default, not the only order: `sort_items_by`
        // carries the comparison, so a descending or case-insensitive sort is one call away
        // (I-23).
        self.sort_items_by(|left, right| left.cmp(right));
    }

    /// Sorts the rows with a caller-supplied comparison.
    ///
    /// The comparison receives the two labels; the per-row metadata travels with its label, so
    /// icons, check states and data tokens stay attached to the rows they belong to. Selection
    /// is cleared for the reason [`Self::sort_items`] documents: an index into the old order
    /// names a different row after the sort.
    pub fn sort_items_by(&mut self, compare: impl Fn(&str, &str) -> core::cmp::Ordering) {
        if self.items.len() < 2 {
            return;
        }
        let mut rows: Vec<(String, ListBoxItem)> = {
            let metas = self.item_meta.iter().cloned();
            self.items.drain(..).zip(metas).collect()
        };
        rows.sort_by(|left, right| compare(&left.0, &right.0));
        let old_len = rows.len();
        self.items = rows.iter().map(|(text, _)| text.clone()).collect();
        self.item_meta = rows.into_iter().map(|(_, meta)| meta).collect();
        if self.item_meta.len() < old_len {
            self.item_meta.resize(old_len, ListBoxItem::default());
        }
        self.invalidate_item_caches();
        self.selected_indices.clear();
        self.current_row = None;
        self.anchor = None;
        self.selection_changed.emit();
        // Every row's label moved, so every row is reported. A caller that keeps its own order
        // rebuilds from this list rather than comparing two snapshots.
        for index in 0..self.items.len() {
            self.item_changed.emit(index);
        }
        self.base.request_redraw();
    }

    /// Number of columns a row is split into.
    ///
    /// A row's label is split on `\t`; columns beyond the first are placed at equal widths
    /// across the row. One column is a plain list, which is the default, so a caller that
    /// never asks paints exactly the label it always did.
    pub fn columns(&self) -> usize {
        self.columns
    }

    /// Sets the number of columns, clamped to at least one.
    pub fn set_columns(&mut self, columns: usize) {
        let columns = columns.max(1);
        if self.columns == columns {
            return;
        }
        self.columns = columns;
        self.base.request_redraw();
    }

    /// Which gestures may start a rename.
    pub fn edit_triggers(&self) -> EditTriggers {
        self.edit_triggers
    }

    /// Sets which gestures may start a rename.
    ///
    /// The control never mutates its own label: it emits [`Self::rename_requested`] so the
    /// host can mount an editor that owns validation and undo.
    pub fn set_edit_triggers(&mut self, triggers: EditTriggers) {
        if self.edit_triggers == triggers {
            return;
        }
        self.edit_triggers = triggers;
    }

    /// The rows visible in the content band, used by PageUp/PageDown.
    fn page_rows(&self) -> usize {
        let content = self.content_rect();
        ((content.height as f32 / self.row_height()).floor() as usize).max(1)
    }

    /// Selects the first row at or after `start` whose label begins with `prefix`.
    ///
    /// The prefix is folded once and each label compared byte-wise, instead of allocating a
    /// lowercased copy of every row (I-P05).
    fn find_prefix(&self, prefix: &str, start: usize) -> Option<usize> {
        let needle = crate::widget::input_widgets::text_match::fold(prefix);
        let count = self.items.len();
        (0..count).map(|offset| (start + offset) % count).find(|&index| {
            self.is_item_enabled(index)
                && crate::widget::input_widgets::text_match::starts_with(
                    &self.items[index],
                    &needle,
                )
        })
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
        // Prefer the resolved theme/caller font when the style names one, so the list honours the
        // theme body font and the user's text scale; otherwise keep the row-height-derived face
        // this control has always used (D09-STYLE-01). One method, so `draw`, the `font_size`
        // property and the hit tests all read the same font.
        if let Some(font) = self.style().font.as_ref() {
            return font.clone();
        }
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

    /// The row a drop at `pos` would land on.
    ///
    /// Dropping past the last row means "move to the end", the same way a drop past the last
    /// tab is clamped by the tab strip. A point outside the content box is not a drop at all.
    fn drop_index_at(&self, pos: Point) -> Option<usize> {
        if let Some(index) = self.item_index_at_y(pos) {
            return Some(index);
        }
        if self.content_rect().contains(pos) && !self.items.is_empty() {
            return Some(self.items.len() - 1);
        }
        None
    }

    /// The rectangle the keyboard focus ring follows.
    ///
    /// A list box's keyboard target is a *row*, so the ring frames the current row rather
    /// than the whole box. With no row selected there is no row to frame, so the content
    /// box is the fallback: keyboard focus must remain visible either way.
    fn focus_ring_rect(&self) -> Rect {
        self.current_row
            .and_then(|index| self.row_rect(index))
            .unwrap_or_else(|| self.content_rect())
    }

    /// Returns visible item range based on scroll position.
    fn visible_range(&self) -> (usize, usize) {
        let content = self.content_rect();
        let height = self.row_height();
        // `floor`, not `ceil`: a row that only partly fits must not be drawn, because the paint
        // path has no way to clip a glyph run to the content box on every backend (the SVG
        // backend emits absolute coordinates). `ceil` counted that partial row, so the demo's
        // 90 px list painted its fifth 20 px row below its own border. At least one row is kept
        // for a control shorter than a single row, which is the only case where a row can still
        // exceed the box.
        let visible_items = (content.height as f32 / height).floor().max(1.0) as usize;
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

    /// Index of the first row painted in the content band.
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Number of whole rows the content band can show, floored at one.
    fn viewport_rows(&self) -> usize {
        let content = self.content_rect();
        ((content.height as f32 / self.row_height()).floor() as usize).max(1)
    }

    /// The greatest first-row index that still fills the band.
    ///
    /// Distinct from [`Self::scroll`]'s `len - 1` bound on purpose: the wheel path lets the
    /// last row reach the top because a notch is a small movement, while a *positioned*
    /// scroll to the bottom has no reason to leave the band empty below the last row.
    fn bottom_scroll_offset(&self) -> usize {
        self.items.len().saturating_sub(self.viewport_rows())
    }

    /// Sets the first painted row, clamped so the band stays as full as it can.
    fn set_scroll_offset(&mut self, offset: usize) -> bool {
        let next = offset.min(self.bottom_scroll_offset());
        if next == self.scroll_offset {
            return false;
        }
        self.scroll_offset = next;
        self.base.request_redraw();
        true
    }

    /// Scrolls a row into view, or pins it, according to `mode`.
    ///
    /// A programmatic selection does not move the viewport by itself, so a host that calls
    /// [`Self::select`] on row 200 of a long list left the user looking at row 0. This is the
    /// public form of the navigation the keyboard and wheel paths already perform. An
    /// out-of-range index is ignored, matching the other row mutators.
    pub fn scroll_to_item(&mut self, index: usize, mode: ScrollToMode) {
        if index >= self.items.len() {
            return;
        }
        let rows = self.viewport_rows();
        let target = match mode {
            ScrollToMode::EnsureVisible => {
                if index < self.scroll_offset {
                    index
                } else if index >= self.scroll_offset + rows {
                    index + 1 - rows
                } else {
                    self.scroll_offset
                }
            }
            ScrollToMode::PositionAtTop => index,
            ScrollToMode::PositionAtCenter => index.saturating_sub(rows / 2),
            ScrollToMode::PositionAtBottom => (index + 1).saturating_sub(rows),
        };
        self.set_scroll_offset(target);
    }

    /// Scrolls to the first row.
    pub fn scroll_to_top(&mut self) {
        self.set_scroll_offset(0);
    }

    /// Scrolls to the last full page, so the final row is visible.
    pub fn scroll_to_bottom(&mut self) {
        let target = self.bottom_scroll_offset();
        self.set_scroll_offset(target);
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

    /// Opts the list into the runtime's drag coordinator as a source.
    fn drag_source_mut(&mut self) -> Option<&mut dyn DragSource> {
        Some(self)
    }

    /// Opts the list into the runtime's drag coordinator as a drop target.
    fn drop_target_mut(&mut self) -> Option<&mut dyn DropTarget> {
        Some(self)
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
            "item_selected" => {
                Some(EventSignalRef::mapped("item_selected", &self.item_selected, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            "item_activated" => {
                Some(EventSignalRef::mapped("item_activated", &self.item_activated, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            "selection_changed" => {
                Some(EventSignalRef::unit("selection_changed", &self.selection_changed))
            }
            "drag_finished" => {
                Some(EventSignalRef::mapped("drag_finished", &self.drag_finished, |v| {
                    CapabilityValue::String(v.as_str().to_string())
                }))
            }
            "rename_requested" => {
                Some(EventSignalRef::mapped("rename_requested", &self.rename_requested, |v| {
                    crate::widget::capability::CapabilityValue::UInt(*v as u64)
                }))
            }
            "item_changed" => {
                Some(EventSignalRef::mapped("item_changed", &self.item_changed, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            _ => None,
        }
    }
}

/// A row drag carries its display index in the item id and its text in the label.
///
/// The index is the reorder key: a target reads it back with `parse`, and the text is the
/// human-readable form a preview or an accessibility announcement uses. The label is also what
/// a *different* list inserts, because the index addresses this list's rows and nothing else.
impl DragSource for ListBox {
    fn drag_payload(&self, pos: Point) -> Option<DragPayload> {
        if !self.base.is_enabled() {
            return None;
        }
        let index = self.item_index_at_y(pos)?;
        let text = self.items.get(index)?.clone();
        // A fresh gesture: the previous one's bookkeeping cannot describe this drag.
        self.pending_drag_row.set(Some(index));
        self.drop_committed_here.set(false);
        Some(DragPayload::new("text/plain", index.to_string()).with_label(text).with_origin(pos))
    }

    /// Ends the gesture, removing the source copy of a row that was transferred away.
    ///
    /// A reorder inside this list already committed in `on_drop`, so nothing is removed. A
    /// transfer inserted the row in another list and reported `Move`, which is the signal that
    /// this list's copy has to go; a refusal reports `None` and the row stays where it was.
    fn on_drag_finished(&mut self, effect: DropEffect) {
        let moved_away = effect.consumes_source() && !self.drop_committed_here.get();
        if moved_away {
            if let Some(row) = self.pending_drag_row.take() {
                self.remove_item(row);
            }
        }
        self.pending_drag_row.set(None);
        self.drop_committed_here.set(false);
        self.drag_finished.emit(effect);
    }
}

/// A row dropped on the list moves to the row under the pointer.
///
/// # Reorder versus transfer
///
/// A payload taken from this list names one of its rows, so the drop moves it. A payload taken
/// from *another* list names an index that means nothing here, so the drop inserts the carried
/// text at the row under the pointer instead — and the source list, told the effect was `Move`,
/// removes its copy. Without that split a cross-list drag would move an unrelated row here and
/// keep a stale copy there.
impl DropTarget for ListBox {
    fn can_accept(&self, payload: &DragPayload) -> bool {
        if !self.base.is_enabled() || !payload.is_type("text/plain") {
            return false;
        }
        if payload.came_from(self.base.id()) {
            return payload.item_id.parse::<usize>().is_ok_and(|index| index < self.items.len());
        }
        !payload.label.is_empty()
    }

    fn on_drop(&mut self, payload: &DragPayload, pos: Point) -> DropEffect {
        if !self.can_accept(payload) {
            return DropEffect::None;
        }
        let Some(to) = self.drop_index_at(pos) else {
            return DropEffect::None;
        };
        if payload.came_from(self.base.id()) {
            let Ok(from) = payload.item_id.parse::<usize>() else {
                return DropEffect::None;
            };
            if !self.move_row(from, to) {
                return DropEffect::None;
            }
        } else {
            // The carried text is inserted *before* the row under the pointer, which is what a
            // drop on that row means; a drop past the last row lands after it.
            self.insert_item(to, payload.label.clone());
            self.selection_changed.emit();
        }
        // Either arm finished the move here, so the source half of this gesture must not remove
        // a row when the coordinator reports the effect back.
        self.drop_committed_here.set(true);
        self.base.request_redraw();
        DropEffect::Move
    }

    /// Highlights the band the row would land on.
    fn preview_rect(&self, payload: &DragPayload, pos: Point) -> Option<Rect> {
        if !self.can_accept(payload) {
            return None;
        }
        self.row_rect(self.drop_index_at(pos)?)
    }
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
                    other => {
                        let row = expect_usize(other)?;
                        // See `ComboBox`'s `current_index` arm: the model setter ignores an
                        // out-of-range row, so the *property* setter has to refuse it. Reporting
                        // `Ok` for a write the control discarded is what made
                        // `rw_list_box_set_current_index(99)` return `true` on a three-item box
                        // and then read back as unselected.
                        if row >= self.items().len() {
                            return Err(CapabilityAccessError::OutOfRange);
                        }
                        self.set_current_row(Some(row));
                    }
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
            Event::MousePress { pos, button, .. } if *button == 1 => {
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
                // A double-click on a rename-enabled row asks the host for an editor instead of
                // activating the row, so the two gestures do not fight over one click stream.
                if self.edit_triggers.contains(EditTriggers::DOUBLE_CLICK) {
                    if let Some(index) = self.item_index_at_y(*pos) {
                        if self.is_item_enabled(index) {
                            self.select(index);
                            self.rename_requested.emit(index);
                            return;
                        }
                    }
                }
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
                // Modifier-aware navigation: Shift extends from the anchor, Primary/Ctrl
                // toggles, and a plain arrow replaces the selection. The old handler only ran
                // when `modifiers == 0`, so a keyboard user had no range selection at all while
                // the pointer path supported it.
                let mods = crate::shortcut::Modifiers::from_event_bits(*modifiers);
                let extend = mods.contains(crate::shortcut::Modifiers::SHIFT)
                    || mods.contains(crate::shortcut::Modifiers::CTRL);
                let select_target = |list: &mut Self, index: usize| {
                    if extend {
                        list.select_with_modifiers(index, mods);
                    } else {
                        list.select(index);
                    }
                };
                match *key {
                    38 => {
                        // Up arrow: move toward earlier selectable rows.
                        let start = self.current_row.unwrap_or(self.items.len());
                        if let Some(target) = (0..start).rev().find(|&i| self.is_item_enabled(i)) {
                            select_target(self, target);
                        }
                    }
                    40 => {
                        // Down arrow.
                        let start = self.current_row.map_or(0, |row| row + 1);
                        if let Some(target) =
                            (start..self.items.len()).find(|&i| self.is_item_enabled(i))
                        {
                            select_target(self, target);
                        }
                    }
                    36 => {
                        // Home; Primary+Home is the conventional "first row" chord as well.
                        if let Some(target) =
                            (0..self.items.len()).find(|&i| self.is_item_enabled(i))
                        {
                            select_target(self, target);
                        }
                    }
                    35 => {
                        // End.
                        if let Some(target) =
                            (0..self.items.len()).rev().find(|&i| self.is_item_enabled(i))
                        {
                            select_target(self, target);
                        }
                    }
                    33 => {
                        // Page Up.
                        let stride = self.page_rows();
                        let start = self.current_row.unwrap_or(self.items.len());
                        let floor = start.saturating_sub(stride);
                        if let Some(target) =
                            (floor..start).rev().find(|&i| self.is_item_enabled(i))
                        {
                            select_target(self, target);
                        }
                    }
                    34 => {
                        // Page Down.
                        let stride = self.page_rows();
                        let start = self.current_row.map_or(0, |row| row + 1);
                        let ceiling = (start + stride).min(self.items.len());
                        if let Some(target) = (start..ceiling).find(|&i| self.is_item_enabled(i)) {
                            select_target(self, target);
                        }
                    }
                    13 => {
                        // Enter activates the current row.
                        if let Some(current) = self.current_row {
                            if self.is_item_enabled(current) {
                                self.item_activated.emit(current);
                            }
                        }
                    }
                    113 => {
                        // F2 requests a rename when that trigger is enabled.
                        if self.edit_triggers.contains(EditTriggers::F2) {
                            if let Some(current) = self.current_row {
                                if self.is_item_enabled(current) {
                                    self.rename_requested.emit(current);
                                }
                            }
                        }
                    }
                    // Type-ahead: a printable character extends the prefix and jumps to the
                    // first matching row from the current one. Without it a long list could
                    // only be traversed with the arrows or the pointer.
                    32..=126 => {
                        if let Some(ch) = char::from_u32(*key) {
                            // A prefix older than the shared timeout is dropped before this key
                            // extends it, so a stale search cannot keep intercepting the first
                            // letter of the next one (I-P06). Clearing queues no frame.
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
                            let start = self.current_row.map_or(0, |row| row + 1);
                            if let Some(target) = self.find_prefix(&self.type_ahead, start) {
                                self.select(target);
                            }
                        }
                    }
                    _ => {
                        self.type_ahead.clear();
                        self.type_ahead_started = None;
                    }
                }
            }
            Event::Wheel { delta, modifiers } => {
                // One notch is one row. The event's positive y means "content moves
                // down", so it scrolls toward earlier rows, hence the negation.
                let scroll = WheelScroll::from_event(*delta, *modifiers, 1);
                self.scroll(-scroll.lines_y);
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
        let style = self.style();
        let theme = crate::style::resolved_theme_style_shared("list_box");
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
        //
        // The palette is the frame's **one** manager lock (I-P07); `resolved_theme_style_shared`
        // above is a thread-local cache hit after the first frame and takes none.
        let accent = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.primary)
            .unwrap_or(Color::PRIMARY);
        let selected_bg = surface.blend(&accent, 0.85);
        let focused_bg = surface.blend(&accent, 0.30);
        let hovered_bg = surface.blend(&accent, 0.12);
        let separator = surface.blend(&ink, 0.14);

        context.face_with_gradient(
            rect,
            surface,
            style.background_gradient.as_ref(),
            style.surface.unwrap_or_default(),
            style.border_radius.unwrap_or(0),
            Color::BLACK,
        );
        // The border is drawn on the control's outer rectangle, so the rows must not paint over it
        // — `row_rect` insets them by `CONTENT_INSET` inside this stroke.
        context.draw_rect(rect, border);

        // Partial rows are clipped to the content box. `visible_range` deliberately includes the
        // row that is only partly visible (`ceil`), because a list that hid it would leave a band
        // of empty surface above its bottom edge. What it must not do is paint past that edge,
        // which is what happened when the last row's fixed height exceeded the remaining space:
        // the demo's 90 px list drew its fifth 20 px row outside its own border.
        let content = self.content_rect();
        context.push_clip(content.x, content.y, content.width, content.height);

        let font = self.row_font();
        let text_inset = 6;
        // Read once so the loop body only compares integers.
        let current_row = self.current_row;
        let hovered_row = self.hovered_row;
        // One window read for the whole pass: the loop bound and the separator's last-row test
        // both used to re-derive the same range.
        let (visible_start, visible_end) = self.visible_range();
        for i in visible_start..visible_end {
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
                    let meta = self.item_meta.get(i);
                    let enabled = meta.is_none_or(|meta| meta.enabled);
                    let label_ink = if enabled { label_ink } else { label_ink.with_alpha(110) };
                    // The line box is measured from the same font the ink is drawn with and centred
                    // in the row, so the glyph box's top edge lands where the text renders.
                    let line = context.text_line(row, &font);
                    let label_align =
                        self.alignment.to_horizontal().unwrap_or(HorizontalAlignment::Left);
                    let mut left = row.x + text_inset;

                    // A check box, when the row has one, is drawn first and reserves its own
                    // leading space. Clicking it is handled by the row press (the whole row is
                    // the target), which is the forgiving behaviour a list needs.
                    if let Some(checked) = meta.and_then(|meta| meta.checked) {
                        let box_size = 10i32;
                        let box_y = row.y + (row.height as i32 - box_size) / 2;
                        let box_rect = Rect::new(left, box_y, box_size as u32, box_size as u32);
                        context.draw_rounded_rect_stroke(box_rect, 2, label_ink.with_alpha(180), 1);
                        if checked {
                            context.fill_rounded_rect(box_rect, 2, label_ink.with_alpha(200));
                            context.draw_line(
                                Point::new(left + 2, box_y + 5),
                                Point::new(left + 4, box_y + 8),
                                fill.map_or(surface, |fill| fill.contrast_color()),
                            );
                            context.draw_line(
                                Point::new(left + 4, box_y + 8),
                                Point::new(left + 8, box_y + 2),
                                fill.map_or(surface, |fill| fill.contrast_color()),
                            );
                        }
                        left += box_size + 6;
                    }

                    // The row's icon, when present, occupies the slot before the label.
                    if let Some(icon) = meta.and_then(|meta| meta.icon()) {
                        if !icon.is_empty() {
                            let icon_width = self.icon_advance(context, i, icon, &font);
                            context.draw_text(
                                Point::new(left, line.y),
                                icon,
                                &font,
                                label_ink,
                                HorizontalAlignment::Left,
                            );
                            left += icon_width + 6;
                        }
                    }

                    let inner_width = (row.x + row.width as i32 - text_inset - left).max(1) as u32;
                    if self.columns > 1 {
                        // Multi-column rows split the label on `\t` and place each part in an
                        // equal-width cell, which is the cheap presentation of a table a list
                        // needs before it becomes a real grid.
                        let parts: Vec<&str> = text.split('\t').collect();
                        let cell_width = (inner_width / self.columns as u32).max(1);
                        for (column, part) in parts.iter().take(self.columns).enumerate() {
                            let cell_x = left + (column as i32) * cell_width as i32;
                            let width = if column + 1 == self.columns {
                                (left + inner_width as i32 - cell_x).max(1) as u32
                            } else {
                                cell_width
                            };
                            context.draw_text_fitted_cow(
                                Rect::new(cell_x, line.y, width, line.height),
                                part,
                                &font,
                                label_ink,
                                HorizontalAlignment::Left,
                            );
                        }
                    } else {
                        context.draw_text_fitted_cow(
                            Rect::new(left, line.y, inner_width, line.height),
                            text,
                            &font,
                            label_ink,
                            label_align,
                        );
                    }
                }
            }
            // A separator only belongs *between* rows, so it is skipped on the last visible one.
            if i + 1 < visible_end {
                let sep_y = row.y + row.height as i32;
                context.draw_line(
                    Point::new(row.x, sep_y),
                    Point::new(row.x + row.width as i32, sep_y),
                    separator,
                );
            }
        }

        // ── Scroll position indicator (I-22) ──
        //
        // A long list scrolled only by wheel or keyboard gave no on-screen position feedback.
        // This is the dropdown's popup thumb at the same 4 px width, drawn on the content box's
        // trailing edge: the track is the surface mixed toward the border, the thumb the
        // separator tone. It appears only when there is something to scroll.
        let bottom = self.bottom_scroll_offset();
        if bottom > 0 {
            let page = self.viewport_rows().max(1);
            let total = self.items.len().max(1);
            let track_height = content.height as i32;
            let thumb_height = ((track_height as usize * page) / total)
                .max(8)
                .min(track_height.max(8) as usize) as i32;
            let max_offset = bottom.max(1) as i32;
            let thumb_top = content.y
                + ((track_height - thumb_height) * self.scroll_offset.min(bottom) as i32
                    / max_offset);
            let rail_x = content.x + content.width as i32 - 4;
            context.fill_rect(
                Rect::new(rail_x, content.y, 4, content.height),
                surface.blend(&border, 0.35),
            );
            context
                .fill_rect(Rect::new(rail_x, thumb_top, 4, thumb_height.max(1) as u32), separator);
        }

        // ── Keyboard focus ring ──
        //
        // Inside the content clip, because the ring is drawn on a row that is itself clipped
        // to the rows' box. Gated on the *reason* focus arrived: a pointer press selects a
        // row without painting a ring under the cursor, while Tab and Shortcut draw one.
        if self.base.draws_focus_ring() {
            let ring = FocusRing::for_control(self.focus_ring_rect(), 0);
            if ring.is_drawable() {
                // The fallback is the surface's contrasting tone; an active theme overrides
                // it with its declared `outline` token.
                context.draw_rounded_rect_stroke(
                    ring.rect,
                    ring.radius,
                    focus_ring_color(surface),
                    FOCUS_RING_WIDTH,
                );
            }
        }
        context.pop_clip();
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

    /// The selection is published in **row order** however it was built.
    ///
    /// # The defect this pins
    ///
    /// [`ListBox::is_selected`] now answers with a binary search over `selected_indices`, so the
    /// list has to stay sorted — and `selected_indices()` is a public accessor whose own test
    /// asserts row order (`&[1, 3]`). The `Multi` path used to `push` straight onto the end, which
    /// both published `[3, 1]` and made the lookup miss row 1: the row stayed selected while it was
    /// painted unselected. `select_with_modifiers` already sorted after its push; the plain path
    /// did not.
    #[test]
    fn the_selection_is_published_in_row_order_however_it_was_built() {
        let mut lb = ListBox::new(Rect::new(0, 0, 200, 200));
        for i in 0..5 {
            lb.add_item(format!("item {i}"));
        }
        lb.set_selection_mode(SelectionMode::Multi);

        // Out of order on purpose: the later row first.
        lb.select(3);
        lb.select(1);
        lb.select(4);
        assert_eq!(
            lb.selected_indices(),
            &[1, 3, 4],
            "plain Multi selections are published in row order"
        );
        for index in [1, 3, 4] {
            assert!(lb.is_selected(index), "row {index} must read as selected");
        }
        assert!(!lb.is_selected(0) && !lb.is_selected(2), "and the others must not");

        // Re-selecting a selected row must not duplicate it.
        lb.select(3);
        assert_eq!(lb.selected_indices(), &[1, 3, 4]);

        // The modifier path keeps the same invariant when it extends.
        lb.select_with_modifiers(2, crate::shortcut::Modifiers::CTRL);
        assert_eq!(lb.selected_indices(), &[1, 2, 3, 4], "ctrl-add lands in row order too");
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
            list.handle_event(&Event::MousePress {
                pos: Point::new(50, y),
                button: 1,
                modifiers: 0,
            });
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

    /// A row that is only partly visible is clipped to the control's content box.
    ///
    /// `visible_range` uses `ceil`, deliberately: a list whose last row was wholly hidden would
    /// leave an empty band above its bottom edge. The row must then be *clipped*, not painted all
    /// the way out — measured in the control demo, where a 90 px list drew its fifth 20 px row
    /// below its own border.
    #[test]
    fn a_partial_last_row_is_clipped_to_the_content_box() {
        let mut list = ListBox::new(Rect::new(0, 0, 200, 50));
        for index in 0..4 {
            list.add_item(format!("row {index}"));
        }
        let svg = crate::widget::svg::render_to_svg(&mut list);
        let boxes = crate::widget::svg::text_ink_boxes(&svg);
        assert!(!boxes.is_empty(), "the visible rows must paint text");
        for (x, y, right, bottom) in boxes {
            assert!(
                bottom <= list.geometry().y + list.geometry().height as i32,
                "text at ({x},{y})-({right},{bottom}) escaped the list's bottom edge"
            );
        }
    }

    /// One wheel notch scrolls one row, whether the host sends the documented
    /// notch delta or the legacy 120-unit tick.
    #[test]
    fn wheel_scrolls_one_row_per_notch() {
        let mut list = listbox_with(10);

        list.handle_event(&Event::wheel(0, -120, 0));
        assert_eq!(list.scroll_offset, 1, "one legacy notch is one row");

        list.handle_event(&Event::wheel(0, -1, 0));
        assert_eq!(list.scroll_offset, 2, "one documented notch is one row");

        list.handle_event(&Event::wheel(0, 120, 0));
        assert_eq!(list.scroll_offset, 1, "the opposite notch scrolls back one row");
    }

    /// Keyboard focus paints the ring on the current row; a pointer press does not.
    ///
    /// The list reported `WidgetState::Focused` for keyboard focus but painted no ring, so
    /// the state had no visible consumer. The ring is the only 2 px stroke a list with no
    /// other chrome emits, which makes its presence observable in the render.
    #[test]
    fn a_keyboard_focused_list_paints_the_ring_and_pointer_focus_does_not() {
        // The ring's colour reads the process-wide theme; hold the guard so a concurrent
        // appearance switch cannot change this render mid-assertion.
        let _guard = crate::style::theme_test_guard();
        let make = || {
            let mut list = ListBox::new(Rect::new(0, 0, 120, 80));
            list.add_items(vec!["one".to_string(), "two".to_string()]);
            list.set_current_row(Some(0));
            list
        };

        let mut keyboard = make();
        keyboard.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Tab });
        assert!(keyboard.base.draws_focus_ring(), "Tab focus must ask for a ring");
        let focused = crate::widget::svg::render_to_svg(&mut keyboard);
        assert!(
            focused.lines().any(|line| line.contains("stroke-width=\"2\"")),
            "keyboard focus must paint the ring around the current row: {focused}"
        );

        let mut pointer = make();
        pointer.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Pointer });
        let unfocused = crate::widget::svg::render_to_svg(&mut pointer);
        assert!(
            !unfocused.lines().any(|line| line.contains("stroke-width=\"2\"")),
            "pointer focus must not paint a ring: {unfocused}"
        );
    }

    /// With no current row the ring falls back to the content box rather than vanishing.
    #[test]
    fn keyboard_focus_without_a_current_row_still_paints_a_ring() {
        let _guard = crate::style::theme_test_guard();
        let mut list = ListBox::new(Rect::new(0, 0, 120, 80));
        list.add_item("one".to_string());
        list.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Tab });
        let svg = crate::widget::svg::render_to_svg(&mut list);
        assert!(
            svg.lines().any(|line| line.contains("stroke-width=\"2\"")),
            "keyboard focus must be visible even with no row selected: {svg}"
        );
    }

    // ── Drag and drop ─────────────────────────────────────────────────────

    /// Unmounts a widget registered by a test, even on panic.
    #[cfg(not(alloc_frugal))]
    struct MountGuard(crate::core::ObjectId);

    #[cfg(not(alloc_frugal))]
    impl Drop for MountGuard {
        fn drop(&mut self) {
            crate::widget::runtime::unregister(self.0);
        }
    }

    /// A press, a move past the threshold and a release inside the list move the pressed row
    /// to the release row, and the source learns that the drop was a `Move`.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn a_drag_within_the_list_reorders_rows_and_reports_move() {
        use crate::event::mouse_button::PRIMARY;
        use crate::widget::runtime::{register, with_widget_mut, DragCoordinator};
        use std::sync::{Arc, Mutex};

        let mut list = ListBox::new(Rect::new(0, 0, 200, 100));
        list.add_items(vec!["A".to_string(), "B".to_string(), "C".to_string()]);
        list.select(0);
        let finished = Arc::new(Mutex::new(None));
        let sink = Arc::clone(&finished);
        list.drag_finished.connect_ref(move |effect: &DropEffect| {
            *sink.lock().unwrap() = Some(*effect);
        });
        let id = register(crate::compat::Box::new(list)).expect("mount the list");
        let _guard = MountGuard(id);

        let viewport = Rect::new(0, 0, 400, 200);
        let mut coordinator = DragCoordinator::new();
        assert_eq!(coordinator.on_event(&Event::mouse_press(10, 5, PRIMARY), viewport), None);
        assert_eq!(coordinator.on_event(&Event::mouse_move(10, 50), viewport), None);
        assert_eq!(
            coordinator.on_event(&Event::mouse_release(10, 50, PRIMARY), viewport),
            Some(DropEffect::Move)
        );

        let observed = with_widget_mut(id, |widget| {
            let list = (widget as &mut dyn core::any::Any)
                .downcast_mut::<ListBox>()
                .expect("the mounted control is a list box");
            (list.items().to_vec(), list.selected_indices().to_vec())
        })
        .expect("the list is still mounted");
        assert_eq!(observed.0, vec!["B", "C", "A"]);
        assert_eq!(observed.1, vec![2], "the selection follows the row that moved");
        assert_eq!(*finished.lock().unwrap(), Some(DropEffect::Move));
    }

    /// A row dragged onto a *second* list is inserted there and removed from the list it came
    /// from.
    ///
    /// This is the difference between a reorder and a transfer: the payload's index addresses the
    /// source's rows, so the target inserts the carried text instead of moving one of its own, and
    /// the source — told the effect was `Move` — drops its copy.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn a_row_dragged_to_another_list_is_inserted_there_and_removed_from_the_source() {
        use crate::event::mouse_button::PRIMARY;
        use crate::widget::runtime::{register, with_widget_mut, DragCoordinator};

        let mut source = ListBox::new(Rect::new(0, 0, 200, 100));
        source.add_items(vec!["A".to_string(), "B".to_string(), "C".to_string()]);
        let mut target = ListBox::new(Rect::new(300, 0, 200, 100));
        target.add_items(vec!["X".to_string(), "Y".to_string()]);
        let source_id = register(crate::compat::Box::new(source)).expect("mount the source");
        let _source_guard = MountGuard(source_id);
        let target_id = register(crate::compat::Box::new(target)).expect("mount the target");
        let _target_guard = MountGuard(target_id);

        let viewport = Rect::new(0, 0, 600, 200);
        let mut coordinator = DragCoordinator::new();
        // Row 0 of the source, row 1 of the target (`item_height` is 20 by default).
        assert_eq!(coordinator.on_event(&Event::mouse_press(10, 5, PRIMARY), viewport), None);
        assert_eq!(coordinator.on_event(&Event::mouse_move(310, 25), viewport), None);
        assert_eq!(
            coordinator.on_event(&Event::mouse_release(310, 25, PRIMARY), viewport),
            Some(DropEffect::Move)
        );

        let items = |id| {
            with_widget_mut(id, |widget| {
                (widget as &mut dyn core::any::Any)
                    .downcast_mut::<ListBox>()
                    .expect("the mounted control is a list box")
                    .items()
                    .to_vec()
            })
            .expect("the list is still mounted")
        };
        assert_eq!(items(source_id), vec!["B", "C"], "the source copy is gone");
        assert_eq!(items(target_id), vec!["X", "A", "Y"], "the row landed before row 1");
    }

    /// A press that never travels far enough is still a click: no row moves and the selection
    /// the press established is left exactly as it was.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn a_press_below_the_drag_threshold_keeps_the_selection() {
        use crate::event::mouse_button::PRIMARY;
        use crate::widget::runtime::{register, with_widget_mut, DragCoordinator};
        use std::sync::{Arc, Mutex};

        let mut list = ListBox::new(Rect::new(0, 0, 200, 100));
        list.add_items(vec!["A".to_string(), "B".to_string(), "C".to_string()]);
        list.select(1);
        let finished = Arc::new(Mutex::new(None));
        let sink = Arc::clone(&finished);
        list.drag_finished.connect_ref(move |effect: &DropEffect| {
            *sink.lock().unwrap() = Some(*effect);
        });
        let id = register(crate::compat::Box::new(list)).expect("mount the list");
        let _guard = MountGuard(id);

        let viewport = Rect::new(0, 0, 400, 200);
        let mut coordinator = DragCoordinator::new();
        let _ = coordinator.on_event(&Event::mouse_press(10, 5, PRIMARY), viewport);
        let _ = coordinator.on_event(&Event::mouse_move(10, 7), viewport);
        assert_eq!(
            coordinator.on_event(&Event::mouse_release(10, 7, PRIMARY), viewport),
            Some(DropEffect::None),
            "a gesture below the threshold is a click, not a drop"
        );

        let observed = with_widget_mut(id, |widget| {
            let list = (widget as &mut dyn core::any::Any)
                .downcast_mut::<ListBox>()
                .expect("the mounted control is a list box");
            (list.items().to_vec(), list.selected_indices().to_vec())
        })
        .expect("the list is still mounted");
        assert_eq!(observed.0, vec!["A", "B", "C"]);
        assert_eq!(observed.1, vec![1], "the click selection must not be disturbed");
        assert_eq!(*finished.lock().unwrap(), Some(DropEffect::None));
    }

    /// Text dragged from a text area lands as a new row: a `text/plain` payload from *another*
    /// control carries text, not one of this list's row indices, so the drop inserts it.
    ///
    /// This replaces the earlier expectation that such a payload was refused. Refusing it was
    /// right while every drop was a reorder; with cross-control transfers the payload's label *is*
    /// the row, and the source — told `Move` — removes its own copy.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn text_dragged_from_another_control_is_inserted_as_a_row() {
        use crate::event::mouse_button::PRIMARY;
        use crate::widget::runtime::{register, with_widget_mut, DragCoordinator};

        let mut area =
            crate::TextArea::new("not a row index".to_string(), Rect::new(0, 0, 100, 40));
        area.select_all();
        let area_id = register(crate::compat::Box::new(area)).expect("mount the text area");
        let _area_guard = MountGuard(area_id);

        let mut list = ListBox::new(Rect::new(200, 0, 200, 100));
        list.add_items(vec!["A".to_string(), "B".to_string(), "C".to_string()]);
        list.select(1);
        let list_id = register(crate::compat::Box::new(list)).expect("mount the list");
        let _list_guard = MountGuard(list_id);

        let viewport = Rect::new(0, 0, 400, 200);
        let mut coordinator = DragCoordinator::new();
        let _ = coordinator.on_event(&Event::mouse_press(10, 10, PRIMARY), viewport);
        let _ = coordinator.on_event(&Event::mouse_move(210, 50), viewport);
        assert_eq!(
            coordinator.on_event(&Event::mouse_release(210, 50, PRIMARY), viewport),
            Some(DropEffect::Move),
            "a foreign text payload is inserted at the row under the pointer"
        );

        let observed = with_widget_mut(list_id, |widget| {
            let list = (widget as &mut dyn core::any::Any)
                .downcast_mut::<ListBox>()
                .expect("the mounted control is a list box");
            (list.items().to_vec(), list.selected_indices().to_vec())
        })
        .expect("the list is still mounted");
        assert_eq!(
            observed.0,
            vec!["A", "B", "not a row index", "C"],
            "the carried text became a row before the row under the pointer"
        );
        assert_eq!(observed.1, vec![1], "an insert after the selected row leaves its index alone");
    }

    /// Structured rows carry an icon, a check state, a disabled flag and a data token.
    #[test]
    fn rows_carry_icons_checks_and_data() {
        let mut list = listbox_with(3);
        list.set_item_meta(
            0,
            ListBoxItem::new().with_icon("*").with_checked(true).with_data("row-a"),
        );
        list.set_item_enabled(1, false);

        assert_eq!(list.item_meta(0).and_then(ListBoxItem::icon), Some("*"));
        assert_eq!(list.item_meta(0).and_then(ListBoxItem::checked), Some(true));
        assert_eq!(list.item_meta(0).and_then(ListBoxItem::data), Some("row-a"));
        assert!(!list.is_item_enabled(1));
        assert_eq!(list.checked_indices(), vec![0]);

        let svg = crate::widget::svg::render_to_svg(&mut list);
        assert!(!svg.contains("row-a"), "the data token is not painted");
        assert_eq!(list.item_meta(0).and_then(|meta| meta.icon()), Some("*"));
    }

    /// Shift and Ctrl modify keyboard movement, exactly as they modify a pointer press.
    #[test]
    fn modified_arrow_keys_extend_and_toggle_the_selection() {
        let mut list = listbox_with(4);
        list.set_selection_mode(SelectionMode::Extended);
        list.select(0);

        list.handle_event(&Event::KeyPress { key: 40, modifiers: 1 });
        assert_eq!(list.selected_indices(), &[0, 1], "Shift+Down extends the range");

        list.handle_event(&Event::KeyPress { key: 40, modifiers: 2 });
        assert_eq!(list.selected_indices(), &[0, 1, 2], "Ctrl+Down adds a row");
    }

    /// Page keys, type-ahead and disabled rows are all reachable from the keyboard.
    #[test]
    fn page_keys_type_ahead_and_disabled_rows_are_honoured() {
        let mut list = ListBox::new(Rect::new(0, 0, 200, 100));
        for label in ["Alpha", "Bravo", "Charlie", "Delta"] {
            list.add_item(label.to_string());
        }
        list.set_current_row(Some(0));

        list.handle_event(&Event::KeyPress { key: 34, modifiers: 0 });
        assert_ne!(list.current_row(), Some(0), "PageDown moves by a screenful");

        list.handle_event(&Event::KeyPress { key: 67, modifiers: 0 }); // 'C'
        assert_eq!(list.current_row(), Some(2), "type-ahead jumps to the matching row");

        list.set_item_enabled(2, false);
        list.handle_event(&Event::KeyPress { key: 67, modifiers: 0 });
        assert_eq!(list.current_row(), Some(2), "a disabled row is not selectable");

        list.set_current_row(Some(0));
        list.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
        assert_eq!(list.current_row(), Some(1));
        list.set_item_enabled(1, false);
        list.set_current_row(Some(0));
        list.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
        assert_eq!(list.current_row(), Some(3), "arrows skip the disabled rows");
    }

    /// A row with tabs paints as columns, and sorting keeps its metadata with its label.
    #[test]
    fn columns_and_sorting_keep_rows_together() {
        let mut list = ListBox::new(Rect::new(0, 0, 200, 100));
        list.add_item("b\t2".to_string());
        list.add_item("a\t1".to_string());
        list.set_item_data(0, Some("second".to_string()));
        list.set_item_data(1, Some("first".to_string()));
        list.set_columns(2);
        assert_eq!(list.columns(), 2);

        let svg = crate::widget::svg::render_to_svg(&mut list);
        assert!(svg.contains("b") && svg.contains("2"));

        list.sort_items();
        assert_eq!(list.items(), &["a\t1".to_string(), "b\t2".to_string()]);
        assert_eq!(list.item_meta(0).and_then(ListBoxItem::data), Some("first"));
        assert!(list.selected_indices().is_empty(), "sorting clears an index-based selection");
    }

    /// A rename gesture asks the host rather than mutating the label itself.
    #[test]
    fn rename_gestures_emit_a_request() {
        let mut list = listbox_with(2);
        list.set_edit_triggers(EditTriggers::DEFAULT);
        let requested = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(usize::MAX));
        list.rename_requested.connect({
            let requested = requested.clone();
            move |index: std::sync::Arc<usize>| {
                requested.store(*index, std::sync::atomic::Ordering::SeqCst);
            }
        });

        list.set_current_row(Some(1));
        list.handle_event(&Event::KeyPress { key: 113, modifiers: 0 }); // F2
        assert_eq!(requested.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(list.item(1), Some("Item 1"), "the label is the host's to change");
    }
}
