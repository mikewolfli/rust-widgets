// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Virtualized list widget backed by incremental data source protocol.

use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::dnd::{DragPayload, DragSource, DropEffect, DropTarget};
use crate::event::{Event, WheelScroll};
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, Signal1};
use crate::widget::capability::access::selection_mode_to_str;
use crate::widget::capability::coercion::{expect_selection_mode, expect_u32, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

use super::data_source::{IncrementalTableDataSource, ListModelDataSource};
use super::list_view::{ScrollHint, SelectionMode, VecListModel};

#[derive(Clone, Debug, PartialEq, Eq)]
struct WindowCache {
    start: usize,
    len: usize,
    revision: u64,
    /// Shared with the draw path so a cache hit is a refcount bump. The owned vector was
    /// cloned per frame, duplicating every visible label.
    rows: Rc<Vec<(usize, String)>>,
}

/// The index a row takes after the row at `from` is moved to `to`.
///
/// The display projection and the selected row are both addressed by display index, so they
/// use this one rule and cannot disagree about where a row ended up.
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

/// Virtualized list view that renders only a visible row window.
pub struct VirtualList {
    base: BaseWidget,
    data_source: Option<Arc<dyn IncrementalTableDataSource>>,
    data_source_connection_scope: ConnectionScope,
    scroll_row: usize,
    /// Display order as indices into the bound source.
    ///
    /// Empty means identity: display row `n` is source row `n`. A reorder fills the vector
    /// with a permutation, so the source is never mutated — the control owns the arrangement.
    /// [`Self::set_data_source`] and a row-count change reset it to identity.
    row_order: Vec<usize>,
    /// Whether `row_order` is still the identity arrangement (F-P08).
    ///
    /// Checking that with an O(n) scan on every window fetch made a scroll step follow the document
    /// size. The flag is maintained by the two places that change the arrangement and read O(1).
    order_is_identity: bool,
    /// Whether a programmatic selection scrolls its row into view (V-18).
    selection_scrolls_into_view: bool,
    row_height: u32,
    overscan: usize,
    selected_row: Option<usize>,
    /// Every selected row, in visual order.
    ///
    /// `selected_row` remains the *current* row (the one keyboard navigation and the anchor use)
    /// and is kept inside this set, so a single-selection caller sees exactly the value it always
    /// did. The set is what makes `Multi`/`Extended` expressible at all.
    selected_rows: std::collections::BTreeSet<usize>,
    /// How clicks and selections combine.
    selection_mode: SelectionMode,
    /// The row a `Shift`-extend grows from.
    selection_anchor: Option<usize>,
    /// Emitted with the whole selection whenever it changes.
    ///
    /// `selection_changed` keeps its single-row payload for compatibility; this is the
    /// set-valued signal a multi-selection host needs.
    pub selected_rows_changed: Signal1<Vec<usize>>,
    /// The row the pointer is currently over, or `None` when it is between/outside the rows.
    ///
    /// Filled by `MouseMove` through the same [`Self::row_at`] the click uses, so the highlighted
    /// row is the row a click would affect. Held here rather than re-derived in `draw`, because
    /// the draw path does not see the pointer.
    hovered_row: Option<usize>,
    window_cache: Option<WindowCache>,
    /// Bumped by the source's `data_changed` signal and by `set_data_source`.
    ///
    /// A source whose `revision()` stays `0` still changes, and the signal is how it says so.
    /// Keying the window cache on this counter is what lets such a source be cached at all; a
    /// source that offers neither a revision nor a signal is not cached, because then there is
    /// no evidence a cached window is still current.
    data_generation: Arc<AtomicU64>,
    /// Whether the bound source can report change (`revision > 0` or a `data_changed` signal).
    data_is_observable: bool,
    /// Emitted when selected row changes.
    pub selection_changed: Signal1<Option<usize>>,
    /// Emitted when visible row window changes `(start, len)`.
    pub visible_window_changed: Signal1<(usize, usize)>,
    /// Emitted with the effect of a drag that began in this list.
    ///
    /// The coordinator reports the same effect through `DragSource::on_drag_finished`; this
    /// signal is the observable form for a host that holds the control by id.
    pub drag_finished: Signal1<DropEffect>,
    /// Whether a vertical scroll-bar thumb is painted.
    show_scrollbar: bool,
}

impl VirtualList {
    /// Creates an empty virtual list.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::DataView, geometry, "VirtualList"),
            data_source: None,
            data_source_connection_scope: ConnectionScope::new(),
            scroll_row: 0,
            row_order: Vec::new(),
            order_is_identity: true,
            selection_scrolls_into_view: true,
            row_height: 20,
            overscan: 2,
            selected_row: None,
            selected_rows: std::collections::BTreeSet::new(),
            selection_mode: SelectionMode::Single,
            selection_anchor: None,
            selected_rows_changed: Signal1::new(),
            hovered_row: None,
            window_cache: None,
            data_generation: Arc::new(AtomicU64::new(0)),
            data_is_observable: false,
            selection_changed: Signal1::new(),
            visible_window_changed: Signal1::new(),
            drag_finished: Signal1::new(),
            show_scrollbar: false,
        }
    }

    /// Whether a vertical scroll-bar thumb is painted.
    pub fn show_scrollbar(&self) -> bool {
        self.show_scrollbar
    }

    /// Enables or disables the vertical scroll-bar thumb.
    pub fn set_show_scrollbar(&mut self, show: bool) {
        if self.show_scrollbar == show {
            return;
        }
        self.show_scrollbar = show;
        self.base.request_redraw();
    }

    /// How many rows fit in the viewport.
    fn visible_row_capacity(&self) -> usize {
        (self.base.geometry().height / self.row_height.max(1)) as usize
    }

    /// Binds incremental data source.
    pub fn set_data_source(&mut self, data_source: Arc<dyn IncrementalTableDataSource>) {
        self.data_source_connection_scope = ConnectionScope::new();
        let revision_nonzero = data_source.revision() > 0;
        let has_signal = if let Some(data_changed) = data_source.data_changed_signal() {
            let redraw = self.base.redraw_requested_signal().clone();
            let layout = self.base.layout_requested_signal().clone();
            let generation = Arc::clone(&self.data_generation);
            data_changed.connect_scoped(&self.data_source_connection_scope, move || {
                // The source says its projection changed. Advancing the key is what makes the
                // next fetch miss; a revision-less source has no other way to invalidate.
                generation.fetch_add(1, Ordering::Relaxed);
                redraw.emit();
                layout.emit();
            });
            true
        } else {
            false
        };
        self.data_generation.fetch_add(1, Ordering::Relaxed);
        self.data_is_observable = revision_nonzero || has_signal;
        self.data_source = Some(data_source);
        self.scroll_row = 0;
        self.selected_row = None;
        self.selected_rows.clear();
        self.selection_anchor = None;
        self.row_order.clear();
        self.order_is_identity = true;
        self.clear_window_cache();
        self.normalize_projection_state();
        self.emit_visible_window_changed();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Binds a flat list of rows without requiring a trait implementation.
    ///
    /// # Why this exists
    ///
    /// [`Self::set_data_source`] is the only fill entry, and it takes a full
    /// [`IncrementalTableDataSource`] implementation plus its signal wiring. A host or a
    /// snapshot that has three strings therefore had to implement a trait to show them, which
    /// is why these controls were the blank ones (VIEW-N05). This builds the in-memory model
    /// and delegates to `set_data_source`, so invalidation, scrolling and signals behave
    /// exactly as they do for any other source.
    pub fn set_rows(&mut self, rows: crate::compat::Vec<String>) {
        let source = ListModelDataSource::new(Arc::new(VecListModel::new(rows)));
        self.set_data_source(Arc::new(source));
    }

    /// Clears bound data source.
    pub fn clear_data_source(&mut self) {
        self.data_source_connection_scope = ConnectionScope::new();
        self.data_source = None;
        self.data_is_observable = false;
        self.data_generation.fetch_add(1, Ordering::Relaxed);
        self.scroll_row = 0;
        self.selected_row = None;
        self.selected_rows.clear();
        self.selection_anchor = None;
        self.row_order.clear();
        self.order_is_identity = true;
        self.clear_window_cache();
        self.emit_visible_window_changed();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns whether a data source is bound.
    pub fn has_data_source(&self) -> bool {
        self.data_source.is_some()
    }

    /// Returns bound data source when present.
    pub fn data_source_ref(&self) -> Option<&Arc<dyn IncrementalTableDataSource>> {
        self.data_source.as_ref()
    }

    /// Returns total row count.
    pub fn row_count(&self) -> usize {
        self.data_source.as_ref().map(|s| s.row_count()).unwrap_or(0)
    }

    /// Returns current top visible row.
    pub fn scroll_row(&self) -> usize {
        self.scroll_row
    }

    /// Sets top visible row with clamping.
    pub fn set_scroll_row(&mut self, row: usize) {
        self.normalize_projection_state();
        let max_row = self.row_count().saturating_sub(1);
        let next = row.min(max_row);
        if next == self.scroll_row {
            return;
        }
        self.scroll_row = next;
        self.clear_window_cache();
        self.emit_visible_window_changed();
        self.base.request_redraw();
    }

    /// Scrolls by signed row delta.
    pub fn scroll_by_rows(&mut self, delta: isize) {
        if delta == 0 {
            return;
        }
        let current = self.scroll_row as isize;
        let next = (current + delta).max(0) as usize;
        self.set_scroll_row(next);
    }

    /// Scrolls so `row` is placed according to `hint` (V-18).
    ///
    /// Returns `false` for a row the model does not have. `Nearest` moves only when the row is
    /// outside the viewport, which is what an `ensure_visible` call wants.
    pub fn scroll_to_row(&mut self, row: usize, hint: ScrollHint) -> bool {
        if row >= self.row_count() {
            return false;
        }
        let visible = self.visible_capacity().max(1);
        let target = match hint {
            ScrollHint::Top => row,
            ScrollHint::Center => row.saturating_sub(visible / 2),
            ScrollHint::Bottom => row.saturating_add(1).saturating_sub(visible),
            ScrollHint::Nearest => {
                if row < self.scroll_row {
                    row
                } else if row >= self.scroll_row.saturating_add(visible) {
                    row.saturating_add(1).saturating_sub(visible)
                } else {
                    self.scroll_row
                }
            }
        };
        self.set_scroll_row(target);
        true
    }

    /// Scrolls the smallest amount that makes `row` visible (V-18).
    pub fn ensure_visible(&mut self, row: usize) -> bool {
        self.scroll_to_row(row, ScrollHint::Nearest)
    }

    /// Whether a programmatic selection scrolls its row into view.
    pub fn selection_scrolls_into_view(&self) -> bool {
        self.selection_scrolls_into_view
    }

    /// Enables or disables the follow-on scroll of a selection (V-18).
    pub fn set_selection_scrolls_into_view(&mut self, follow: bool) {
        self.selection_scrolls_into_view = follow;
    }

    /// Returns row height in pixels.
    pub fn row_height(&self) -> u32 {
        self.row_height
    }

    /// Sets row height in pixels.
    pub fn set_row_height(&mut self, row_height: u32) {
        let next = row_height.max(1);
        if self.row_height == next {
            return;
        }
        self.row_height = next;
        self.clear_window_cache();
        self.emit_visible_window_changed();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns overscan row count.
    pub fn overscan(&self) -> usize {
        self.overscan
    }

    /// Sets overscan row count.
    pub fn set_overscan(&mut self, overscan: usize) {
        if self.overscan == overscan {
            return;
        }
        self.overscan = overscan;
        self.clear_window_cache();
        self.emit_visible_window_changed();
        self.base.request_redraw();
    }

    /// Returns selected row when present.
    pub fn selected_row(&self) -> Option<usize> {
        self.selected_row.filter(|row| *row < self.row_count())
    }

    /// The row at a screen point, or `None` when the point is not over a real row.
    ///
    /// # Why this is one derivation
    ///
    /// Two readers ask "which row is here": the press handler decides what to select, and the
    /// move handler decides what to highlight. Deriving it twice is how `table_widget` came to
    /// select the row *above* the one the user clicked (it counted rows from the content box while
    /// hit-testing from the control's top edge, with the offset written in two places). Both now
    /// call this, so a rect that changes moves both answers together.
    pub fn row_at(&self, pos: Point) -> Option<usize> {
        let rect = self.base.geometry();
        if pos.y < rect.y || pos.y >= rect.y + rect.height as i32 {
            return None;
        }
        let local_row = ((pos.y - rect.y) / self.row_height.max(1) as i32).max(0) as usize;
        let row = self.scroll_row.saturating_add(local_row);
        (row < self.row_count()).then_some(row)
    }

    /// Selects a row.
    ///
    /// The mode decides how the selection changes; this is the un-modified path (a plain click or
    /// a programmatic select), and it still works exactly as before in [`SelectionMode::Single`].
    pub fn select_row(&mut self, row: usize) -> bool {
        self.select_row_with_modifiers(row, crate::shortcut::Modifiers::empty())
    }

    /// Selects `row` honouring keyboard modifiers.
    ///
    /// `Shift` extends from the anchor, `Ctrl`/`Primary` toggles one row, and no modifier is a
    /// plain selection. In `Single` mode every call replaces the selection, which is what keeps a
    /// pre-multi-selection caller unchanged.
    pub fn select_row_with_modifiers(
        &mut self,
        row: usize,
        modifiers: crate::shortcut::Modifiers,
    ) -> bool {
        if row >= self.row_count() {
            return false;
        }
        let unchanged = self.selection_mode == SelectionMode::Single
            && self.selected_row == Some(row)
            && self.selected_rows.len() == 1
            && self.selected_rows.contains(&row);
        if unchanged {
            return true;
        }
        match self.selection_mode {
            SelectionMode::None => {
                self.selected_rows.clear();
                self.selected_rows.insert(row);
                self.selection_anchor = Some(row);
            }
            SelectionMode::Single => {
                self.selected_rows.clear();
                self.selected_rows.insert(row);
                self.selection_anchor = Some(row);
            }
            SelectionMode::Multi => {
                let toggle = modifiers.contains(crate::shortcut::Modifiers::CTRL)
                    || modifiers.contains(crate::shortcut::Modifiers::PRIMARY);
                if toggle && !self.selected_rows.insert(row) {
                    self.selected_rows.remove(&row);
                } else {
                    self.selected_rows.insert(row);
                }
                self.selection_anchor = Some(row);
            }
            SelectionMode::Extended => {
                let shift = modifiers.contains(crate::shortcut::Modifiers::SHIFT);
                let toggle = modifiers.contains(crate::shortcut::Modifiers::CTRL)
                    || modifiers.contains(crate::shortcut::Modifiers::PRIMARY);
                if shift {
                    let anchor = self.selection_anchor.unwrap_or(row);
                    let (low, high) = if anchor <= row { (anchor, row) } else { (row, anchor) };
                    self.selected_rows = (low..=high).collect();
                } else if toggle {
                    if !self.selected_rows.insert(row) {
                        self.selected_rows.remove(&row);
                    }
                    self.selection_anchor = Some(row);
                } else {
                    self.selected_rows.clear();
                    self.selected_rows.insert(row);
                    self.selection_anchor = Some(row);
                }
            }
        }
        self.selected_row = Some(row);
        self.emit_selection();
        if self.selection_scrolls_into_view {
            let _ = self.ensure_visible(row);
        }
        self.base.request_redraw();
        true
    }

    /// Selects the inclusive range `from..=to`.
    ///
    /// In single-selection mode a range cannot be expressed, so the selection collapses to `to`
    /// (the row a range gesture would end on) rather than contradicting the mode.
    pub fn select_range(&mut self, from: usize, to: usize) -> bool {
        let count = self.row_count();
        if from >= count || to >= count {
            return false;
        }
        if self.selection_mode == SelectionMode::Single
            || self.selection_mode == SelectionMode::None
        {
            return self.select_row(to);
        }
        let (low, high) = if from <= to { (from, to) } else { (to, from) };
        self.selected_rows = (low..=high).collect();
        self.selected_row = Some(high);
        if self.selection_anchor.is_none() {
            self.selection_anchor = Some(low);
        }
        self.emit_selection();
        if self.selection_scrolls_into_view {
            let _ = self.ensure_visible(high);
        }
        self.base.request_redraw();
        true
    }

    /// The selected rows, in ascending (visual) order.
    pub fn selected_rows(&self) -> Vec<usize> {
        self.selected_rows.iter().copied().filter(|row| *row < self.row_count()).collect()
    }

    /// Clears the selection, leaving no current row.
    pub fn clear_selection(&mut self) {
        if self.selected_rows.is_empty() && self.selected_row.is_none() {
            return;
        }
        self.selected_rows.clear();
        self.selected_row = None;
        self.selection_anchor = None;
        self.selection_changed.emit(None);
        self.selected_rows_changed.emit(Vec::new());
        self.base.request_redraw();
    }

    /// How clicks and selections combine.
    pub fn selection_mode(&self) -> SelectionMode {
        self.selection_mode
    }

    /// Sets the selection mode.
    ///
    /// Leaving `Multi`/`Extended` keeps only the current row, because a caller that switched to
    /// `Single` expects exactly one selected row.
    pub fn set_selection_mode(&mut self, mode: SelectionMode) {
        if self.selection_mode == mode {
            return;
        }
        self.selection_mode = mode;
        if mode == SelectionMode::Single || mode == SelectionMode::None {
            self.selected_rows.clear();
            if let Some(row) = self.selected_row.filter(|row| *row < self.row_count()) {
                self.selected_rows.insert(row);
                self.selection_anchor = Some(row);
            } else {
                self.selection_anchor = None;
            }
        }
        self.emit_selection();
        self.base.request_redraw();
    }

    /// Emits both the legacy single-row signal and the set-valued one.
    fn emit_selection(&self) {
        self.selection_changed.emit(self.selected_row);
        self.selected_rows_changed.emit(self.selected_rows());
    }

    /// Moves the displayed row at `from` to `to`, keeping the selection on the item it names.
    ///
    /// The bound data source is read-only, so this rearranges the display projection rather
    /// than the source itself. A move onto the same row is accepted without changing
    /// anything, which lets a drop aimed at the dragged row still report that it was taken.
    /// Returns `false` only for an out-of-range index.
    pub fn move_row(&mut self, from: usize, to: usize) -> bool {
        let count = self.row_count();
        if from >= count || to >= count {
            return false;
        }
        self.normalize_row_order();
        if from == to {
            return true;
        }
        let row = self.row_order.remove(from);
        self.row_order.insert(to, row);
        // A move can in principle restore the identity order; marking the flag dirty is the
        // conservative answer and costs one extra per-row fetch, not a wrong screen.
        self.order_is_identity = false;
        let before = self.selected_row;
        self.selected_row = self.selected_row.map(|index| index_after_move(index, from, to));
        self.selected_rows =
            self.selected_rows.iter().map(|index| index_after_move(*index, from, to)).collect();
        self.selection_anchor =
            self.selection_anchor.map(|index| index_after_move(index, from, to));
        // The cache key counts the source revision, not this local arrangement, so the cached
        // window has to be dropped explicitly or a repaint would serve the old order.
        self.clear_window_cache();
        self.data_generation.fetch_add(1, Ordering::Relaxed);
        if before != self.selected_row {
            self.emit_selection();
        }
        self.base.request_redraw();
        true
    }

    /// Returns `(start, len)` for visible + overscan row window.
    pub fn visible_window(&self) -> (usize, usize) {
        let row_count = self.row_count();
        if row_count == 0 {
            return (0, 0);
        }

        let start = self.scroll_row.saturating_sub(self.overscan);
        let visible_capacity = self.visible_capacity();
        let fetch_len = visible_capacity.saturating_add(self.overscan.saturating_mul(2));
        let end = start.saturating_add(fetch_len).min(row_count);
        (start, end.saturating_sub(start))
    }

    /// Fetches visible rows as `(row_index, text)` pairs.
    pub fn fetch_visible_rows(&mut self) -> Vec<(usize, String)> {
        (*self.fetch_visible_rows_shared()).clone()
    }

    /// The draw path's form of [`Self::fetch_visible_rows`]: a cache hit returns the shared
    /// window instead of cloning every row label.
    pub fn fetch_visible_rows_shared(&mut self) -> Rc<Vec<(usize, String)>> {
        self.normalize_projection_state();

        let Some(source) = self.data_source.as_ref() else {
            return Rc::new(Vec::new());
        };

        let (start, len) = self.visible_window();
        if len == 0 {
            return Rc::new(Vec::new());
        }

        let revision = source.revision();
        // A source with a real revision uses it; one that only raises `data_changed` uses the
        // generation this widget advances from that signal. When neither exists the window is
        // fetched afresh — a cache with no invalidation contract would serve stale rows.
        let cache_revision =
            if revision > 0 { revision } else { self.data_generation.load(Ordering::Relaxed) };
        if self.data_is_observable {
            if let Some(cache) = self.window_cache.as_ref() {
                if cache.start == start && cache.len == len && cache.revision == cache_revision {
                    return Rc::clone(&cache.rows);
                }
            }
        }

        let rows = Rc::new(self.fetch_window_rows(start, len));
        if self.data_is_observable {
            self.window_cache =
                Some(WindowCache { start, len, revision: cache_revision, rows: Rc::clone(&rows) });
        } else {
            self.window_cache = None;
        }

        rows
    }

    fn fetch_window_rows(&self, start: usize, len: usize) -> Vec<(usize, String)> {
        let Some(source) = self.data_source.as_ref() else {
            return Vec::new();
        };

        // The identity case keeps the batched window fetch. A reordered projection has to ask
        // for one source row per display row, because the window is no longer contiguous in
        // the source.
        if self.row_order_is_identity() {
            return source
                .fetch_window(start, len, 0, 1)
                .into_iter()
                .enumerate()
                .map(|(offset, row)| {
                    let text = row.into_iter().next().flatten().unwrap_or_default();
                    (start + offset, text)
                })
                .collect();
        }

        (start..start.saturating_add(len))
            .map(|display| {
                let source_row = self.source_row(display);
                (display, source.data(source_row, 0).unwrap_or_default())
            })
            .collect()
    }

    /// Turns the identity-encoded order into a real permutation so it can be rearranged.
    fn normalize_row_order(&mut self) {
        let count = self.row_count();
        if self.row_order.len() != count {
            self.row_order = (0..count).collect();
            self.order_is_identity = true;
        }
    }

    /// Whether display row `n` is still source row `n` (F-P08: O(1), not an O(n) scan).
    fn row_order_is_identity(&self) -> bool {
        self.order_is_identity || self.row_order.len() != self.row_count()
    }

    /// The source row a display row reads its text from.
    fn source_row(&self, display: usize) -> usize {
        if self.row_order.len() == self.row_count() {
            self.row_order.get(display).copied().unwrap_or(display)
        } else {
            display
        }
    }

    /// The display row a drop at `pos` would land on.
    ///
    /// Dropping past the last row means "move to the end", the same clamp the tab strip and
    /// the list box apply. A point outside the control is not a drop at all.
    fn drop_row_at(&self, pos: Point) -> Option<usize> {
        if let Some(row) = self.row_at(pos) {
            return Some(row);
        }
        let count = self.row_count();
        if count > 0 && self.base.geometry().contains(pos) {
            return Some(count - 1);
        }
        None
    }

    fn clear_window_cache(&mut self) {
        self.window_cache = None;
    }

    fn normalize_projection_state(&mut self) {
        let row_count = self.row_count();
        if self.row_order.len() != row_count {
            self.row_order.clear();
            self.order_is_identity = true;
        }
        if row_count == 0 {
            self.scroll_row = 0;
            self.selected_row = None;
            self.selected_rows.clear();
            self.selection_anchor = None;
            return;
        }

        let max_row = row_count.saturating_sub(1);
        if self.scroll_row > max_row {
            self.scroll_row = max_row;
        }
        self.selected_row = self.selected_row.filter(|row| *row <= max_row);
        self.selected_rows.retain(|row| *row <= max_row);
        if let Some(row) = self.selected_row {
            self.selected_rows.insert(row);
        }
    }

    fn visible_capacity(&self) -> usize {
        let h = self.base.geometry().height;
        if h == 0 {
            return 0;
        }
        let rh = self.row_height.max(1);
        h.div_ceil(rh) as usize
    }

    fn emit_visible_window_changed(&self) {
        self.visible_window_changed.emit(self.visible_window());
    }
}

impl Widget for VirtualList {
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
        crate::core::Size::new(
            crate::widget::metrics::dimensions::VIRTUAL_LIST_DEFAULT_WIDTH,
            crate::widget::metrics::dimensions::VIRTUAL_LIST_DEFAULT_HEIGHT,
        )
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
    /// # Why the field is named per arm
    ///
    /// The published name and the Rust field name are not always the same (`find_next` is backed by
    /// `find_next_signal`, `dismissed` by a `Signal1<()>` field). `connect_event` validates a name
    /// against the capability table and registers a hub slot; only `event_signal_dyn` joins that
    /// name to the signal the control actually **emits**. A wrong arm is worse than no arm, because
    /// it reports a wire as live and never fires it, so each field is named explicitly here rather
    /// than derived from the published name.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "selection_changed" => {
                Some(EventSignalRef::mapped("selection_changed", &self.selection_changed, |v| {
                    match v {
                        Some(value) => CapabilityValue::UInt(*value as u64),
                        None => CapabilityValue::Null,
                    }
                }))
            }
            "visible_window_changed" => Some(EventSignalRef::mapped(
                "visible_window_changed",
                &self.visible_window_changed,
                |v| {
                    CapabilityValue::Tuple(alloc::vec![
                        CapabilityValue::UInt(v.0 as u64),
                        CapabilityValue::UInt(v.1 as u64),
                    ])
                },
            )),
            "drag_finished" => {
                Some(EventSignalRef::mapped("drag_finished", &self.drag_finished, |v| {
                    CapabilityValue::String(v.as_str().to_string())
                }))
            }
            "selected_rows_changed" => Some(EventSignalRef::mapped(
                "selected_rows_changed",
                &self.selected_rows_changed,
                |rows| {
                    CapabilityValue::Tuple(
                        rows.iter().map(|row| CapabilityValue::UInt(*row as u64)).collect(),
                    )
                },
            )),
            _ => None,
        }
    }
}

/// A row drag carries its display index in the item id and its text in the label.
///
/// The index addresses the control's own display order, which is what a target reorders; the
/// bound data source is never written through this payload.
impl DragSource for VirtualList {
    fn drag_payload(&self, pos: Point) -> Option<DragPayload> {
        if !self.base.is_enabled() {
            return None;
        }
        let row = self.row_at(pos)?;
        let text = self.data_source.as_ref()?.data(self.source_row(row), 0)?;
        Some(DragPayload::new("text/plain", row.to_string()).with_label(text).with_origin(pos))
    }

    /// Reports the outcome to the host. The display order is already committed by the drop
    /// target, so the source has nothing left to remove.
    fn on_drag_finished(&mut self, effect: DropEffect) {
        self.drag_finished.emit(effect);
    }
}

/// A row dropped on the list moves to the row under the pointer.
impl DropTarget for VirtualList {
    fn can_accept(&self, payload: &DragPayload) -> bool {
        if !self.base.is_enabled() || !payload.is_type("text/plain") {
            return false;
        }
        // # Why a row from another view is refused
        //
        // The payload's item id is a row index, and an index only addresses the view that
        // produced it. This view also cannot *insert*: its rows are a projection of the read-only
        // `DataSource` bound to it, so accepting a foreign row would have to write into a binding
        // it does not own. A same-view payload is the only one this control can honour.
        if !payload.came_from(self.base.id()) {
            return false;
        }
        payload.item_id.parse::<usize>().is_ok_and(|row| row < self.row_count())
    }

    fn on_drop(&mut self, payload: &DragPayload, pos: Point) -> DropEffect {
        if !self.can_accept(payload) {
            return DropEffect::None;
        }
        let Ok(from) = payload.item_id.parse::<usize>() else {
            return DropEffect::None;
        };
        let Some(to) = self.drop_row_at(pos) else {
            return DropEffect::None;
        };
        if self.move_row(from, to) {
            DropEffect::Move
        } else {
            DropEffect::None
        }
    }

    /// Highlights the band the row would land on.
    fn preview_rect(&self, payload: &DragPayload, pos: Point) -> Option<Rect> {
        if !self.can_accept(payload) {
            return None;
        }
        let row = self.drop_row_at(pos)?;
        let rect = self.base.geometry();
        let height = self.row_height.max(1) as i32;
        let y = rect.y + (row as i32 - self.scroll_row as i32) * height;
        Some(Rect::new(rect.x, y, rect.width, height as u32))
    }
}

/// `VirtualList`'s property contract.
///
/// `DataView` is a type alias for `VirtualList` (`src/widget/mod.rs`), so this one
/// impl answers for both kinds. Read/write semantics are carried over unchanged
/// from the centralised `access_read_view.in.rs` / `access_write_view.in.rs`
/// dispatch, so callers see the same coercions and the same errors as before.
impl WidgetProperties for VirtualList {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "has_data_source" => Ok(CapabilityValue::Bool(self.has_data_source())),
            "row_count" => Ok(CapabilityValue::UInt(self.row_count() as u64)),
            "scroll_row" => Ok(CapabilityValue::UInt(self.scroll_row() as u64)),
            "row_height" => Ok(CapabilityValue::UInt(self.row_height() as u64)),
            "overscan" => Ok(CapabilityValue::UInt(self.overscan() as u64)),
            "selected_row" => match self.selected_row() {
                Some(row) => Ok(CapabilityValue::UInt(row as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "selected_rows" => Ok(CapabilityValue::Tuple(
                self.selected_rows()
                    .into_iter()
                    .map(|row| CapabilityValue::UInt(row as u64))
                    .collect(),
            )),
            "selection_mode" => Ok(CapabilityValue::String(
                selection_mode_to_str(self.selection_mode()).to_string(),
            )),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "scroll_row" => {
                self.set_scroll_row(expect_usize(value)?);
                Ok(())
            }
            "row_height" => {
                let row_height = expect_u32(value)?;
                self.set_row_height(row_height);
                Ok(())
            }
            "overscan" => {
                self.set_overscan(expect_usize(value)?);
                Ok(())
            }
            "selection_mode" => {
                self.set_selection_mode(expect_selection_mode(value)?);
                Ok(())
            }
            "has_data_source" => Err(CapabilityAccessError::ReadOnlyProperty),
            "row_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            "selected_row" => Err(CapabilityAccessError::ReadOnlyProperty),
            "selected_rows" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "has_data_source",
            "row_count",
            "scroll_row",
            "row_height",
            "overscan",
            "selected_row",
            "selected_rows",
            "selection_mode",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `virtual_list` publishes.
    ///
    /// `clear_data_source` is payload-free and executes here;
    /// `fetch_visible_rows` is a query and belongs to the read path.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "clear_data_source" => {
                self.clear_data_source();
                Ok(())
            }
            "fetch_visible_rows" => {
                let _ = self.fetch_visible_rows();
                Ok(())
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for VirtualList {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();

        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then a literal. The theme step is what makes an appearance
        // switch visible; the surface, the border, the selected-row highlight and the text
        // colour used to be hardcoded literals, so light and dark rendered identically.
        //
        // The theme reads take and release the global manager's lock internally, so no guard
        // is held across the draw (the mutex is not re-entrant).
        // `WidgetStyle` carries a font-family `String` and a gradient `Vec`; cloning it to read
        // three `Copy` colour fields allocated both on every frame (WDGT-08). Copy the fields
        // under one short borrow instead.
        let (background_color, text_color, border_color) = {
            let style = self.base.style();
            (style.background_color, style.text_color, style.border_color)
        };
        let theme = crate::style::resolved_theme_style_shared("virtual_list");
        let mut surface = background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let ink = text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        // `virtual_list` is absent from `WidgetRole::for_kind_name`'s table, so it classifies as
        // `Surface` and resolves to `theme.colors.background` — the window's own fill. A panel
        // painted in that colour would be byte-identical to the frame behind it, so a resolved
        // surface equal to the window fill is re-derived a visible step away from it, the same
        // distinction `Colors::input_background` draws for a field. `data_view` is the same
        // control under its other spelling, so the derivation covers both names.
        let window_fill = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.background)
            .unwrap_or(Color::WHITE);
        if surface == window_fill {
            surface = window_fill.blend(&ink, 0.08);
        }
        let border = border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != surface)
            .unwrap_or_else(|| surface.blend(&ink, 0.20));
        // The selected row is a selection state, so it reads the theme's accent token and is
        // laid over the surface, which keeps it legible in either appearance.
        let accent = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.primary)
            .unwrap_or(Color::PRIMARY);
        let selected_bg = surface.blend(&accent, 0.30);

        context.face_with_gradient(
            rect,
            surface,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );
        context.draw_rect(rect, border);

        let rows = self.fetch_visible_rows_shared();
        if rows.is_empty() {
            return;
        }

        let rh = self.row_height as i32;
        let row_font = effective_font(self.style());
        for &(row_index, ref text) in rows.iter() {
            let y = rect.y + ((row_index as i32 - self.scroll_row as i32) * rh);
            if y + rh <= rect.y || y >= rect.y + rect.height as i32 {
                continue;
            }

            if self.selected_row == Some(row_index) || self.selected_rows.contains(&row_index) {
                context.fill_rect(Rect::new(rect.x, y, rect.width, self.row_height), selected_bg);
            } else if self.hovered_row == Some(row_index) {
                // A weaker wash than the selection, from the same accent: a row the pointer is over
                // is a *preview* of the row a click would select, so it must read as the same
                // affordance at lower weight rather than as a second, unrelated highlight.
                context.fill_rect(
                    Rect::new(rect.x, y, rect.width, self.row_height),
                    surface.blend(&accent, 0.12),
                );
            }

            // The row text is a host-supplied `DataSource` string, so it is fitted into the row's
            // own box rather than drawn at an unbounded length. `draw_text` places a string and
            // never asks whether it fits; the sibling `list_view` already fits its rows, and this
            // one let a long label run past the control's right edge.
            let row_box = Rect::new(rect.x + 4, y, rect.width.saturating_sub(8), self.row_height);
            context.draw_text_line_cow(row_box, text, row_font, ink, HorizontalAlignment::Left);
        }

        if self.show_scrollbar {
            let total = self.row_count().max(1);
            let visible = self.visible_row_capacity().max(1).min(total);
            let track = rect.height.max(1);
            let thumb =
                ((track as u64 * visible as u64) / total as u64).max(24).min(track as u64) as u32;
            let max_scroll = total.saturating_sub(visible);
            let travel = track.saturating_sub(thumb);
            let offset = if max_scroll == 0 {
                0
            } else {
                (travel as u64 * self.scroll_row.min(max_scroll) as u64 / max_scroll as u64) as u32
            };
            context.fill_rect(
                Rect::new(rect.x + rect.width as i32 - 6, rect.y + offset as i32, 4, thumb),
                accent.with_alpha(120),
            );
        }
    }
}

impl crate::event::EventHandler for VirtualList {
    fn handle_event(&mut self, event: &Event) {
        // The base keeps the control-level facts (`hovered`, `pressed`, `focus_reason`) and its
        // `MouseEnter`/`MouseLeave` arms are what make `widget_state()` answer `Hover` here. This
        // handler did not forward to it at all, so the theme's `"data_view:hover"` override could
        // never fire and the control-level hover was always false — the same defect `list_view`
        // records, in the sibling control that never inherited its fix.
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        self.normalize_projection_state();

        match event {
            Event::MousePress { pos, button, modifiers } if *button == 1 => {
                // The same derivation the hover uses, so the row a click selects and the row the
                // pointer highlights cannot disagree. Modifiers choose between replace, toggle
                // and range-extend in the multi-row modes.
                if let Some(row) = self.row_at(*pos) {
                    let _ = self.select_row_with_modifiers(
                        row,
                        crate::shortcut::Modifiers::from_event_bits(*modifiers),
                    );
                }
            }
            // Row hover, derived from the same `row_at` the click uses, so the row that is
            // highlighted is the row a click would affect. A pointer that leaves the rows entirely
            // clears it rather than latching the last row it crossed.
            Event::MouseMove { pos } => {
                let hovered = self.row_at(*pos);
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
            Event::Wheel { delta, modifiers } => {
                // One notch is one row. `scroll_by_rows` takes a positive delta as
                // moving toward later rows, which is the negative event direction.
                let scroll = WheelScroll::from_event(*delta, *modifiers, 1);
                if scroll.lines_y != 0 {
                    self.scroll_by_rows(-(scroll.lines_y as isize));
                }
            }
            Event::KeyPress { key, modifiers } if self.base.focus_reason().is_some() => {
                let page = self.visible_row_capacity().max(1);
                let last = self.row_count().saturating_sub(1);
                let current = self.selected_row.unwrap_or(0);
                let mods = crate::shortcut::Modifiers::from_event_bits(*modifiers);
                // Ctrl/Cmd+A selects the whole projection, which is the shortcut every list
                // responds to and the one a multi-selection host expects.
                if (mods.contains(crate::shortcut::Modifiers::CTRL)
                    || mods.contains(crate::shortcut::Modifiers::PRIMARY))
                    && crate::event::key_codes::ascii_key_code('a').is_some_and(|code| code == *key)
                {
                    let mut rows: Vec<usize> = (0..self.row_count()).collect();
                    if self.selection_mode == SelectionMode::Single {
                        rows.truncate(1);
                    }
                    if let Some(&last_row) = rows.last() {
                        self.selected_rows = rows.into_iter().collect();
                        self.selected_row = Some(last_row);
                        self.selection_anchor = self.selected_rows.iter().next().copied();
                        self.emit_selection();
                        self.base.request_redraw();
                    }
                    return;
                }
                let next = match *key {
                    crate::event::key_codes::UP => current.saturating_sub(1),
                    crate::event::key_codes::DOWN => current.saturating_add(1).min(last),
                    crate::event::key_codes::HOME => 0,
                    crate::event::key_codes::END => last,
                    crate::event::key_codes::PAGE_UP => current.saturating_sub(page),
                    crate::event::key_codes::PAGE_DOWN => current.saturating_add(page).min(last),
                    _ => current,
                };
                if self.select_row_with_modifiers(next, mods) {
                    if next < self.scroll_row {
                        self.set_scroll_row(next);
                    } else if next >= self.scroll_row + page {
                        self.set_scroll_row(next + 1 - page);
                    }
                }
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::EventHandler;
    use crate::signal::GenericSignal;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

    struct StaticSource {
        rows: usize,
    }

    impl IncrementalTableDataSource for StaticSource {
        fn row_count(&self) -> usize {
            self.rows
        }

        fn column_count(&self) -> usize {
            1
        }

        fn data(&self, row: usize, column: usize) -> Option<String> {
            if column == 0 && row < self.rows {
                Some(format!("row-{}", row))
            } else {
                None
            }
        }
    }

    struct CountingSource {
        rows: usize,
        calls: AtomicUsize,
        revision: AtomicU64,
        changed: GenericSignal,
    }

    impl CountingSource {
        fn new(rows: usize, revision: u64) -> Self {
            Self {
                rows,
                calls: AtomicUsize::new(0),
                revision: AtomicU64::new(revision),
                changed: GenericSignal::new(),
            }
        }

        fn calls(&self) -> usize {
            self.calls.load(Ordering::Relaxed)
        }

        fn bump_revision(&self) {
            self.revision.fetch_add(1, Ordering::Relaxed);
            self.changed.emit();
        }

        /// Announces a change without touching the revision, which is the shape a source that
        /// does not implement `revision()` has.
        fn touch(&self) {
            self.changed.emit();
        }
    }

    impl IncrementalTableDataSource for CountingSource {
        fn row_count(&self) -> usize {
            self.rows
        }

        fn column_count(&self) -> usize {
            1
        }

        fn data(&self, row: usize, column: usize) -> Option<String> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            if column == 0 && row < self.rows {
                Some(format!("cached-{}", row))
            } else {
                None
            }
        }

        fn revision(&self) -> u64 {
            self.revision.load(Ordering::Relaxed)
        }

        fn data_changed_signal(&self) -> Option<&GenericSignal> {
            Some(&self.changed)
        }
    }

    #[test]
    fn visible_window_includes_overscan_and_clamps() {
        let mut list = VirtualList::new(Rect::new(0, 0, 120, 45));
        list.set_row_height(20);
        list.set_overscan(2);
        list.set_data_source(Arc::new(StaticSource { rows: 10 }));

        assert_eq!(list.visible_window(), (0, 7));

        list.set_scroll_row(4);
        assert_eq!(list.visible_window(), (2, 7));

        list.set_scroll_row(999);
        assert_eq!(list.scroll_row(), 9);
        assert_eq!(list.visible_window(), (7, 3));
    }

    #[test]
    fn fetch_visible_rows_projects_window() {
        let mut list = VirtualList::new(Rect::new(0, 0, 120, 40));
        list.set_data_source(Arc::new(StaticSource { rows: 5 }));
        list.set_overscan(0);

        let rows = list.fetch_visible_rows();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0], (0, "row-0".to_string()));
        assert_eq!(rows[1], (1, "row-1".to_string()));

        list.set_scroll_row(3);
        let rows2 = list.fetch_visible_rows();
        assert_eq!(rows2.len(), 2);
        assert_eq!(rows2[0], (3, "row-3".to_string()));
        assert_eq!(rows2[1], (4, "row-4".to_string()));
    }

    #[test]
    fn visible_rows_cache_invalidation_uses_revision() {
        let mut list = VirtualList::new(Rect::new(0, 0, 120, 40));
        list.set_overscan(0);

        let source = Arc::new(CountingSource::new(6, 1));
        list.set_data_source(source.clone());

        let a = list.fetch_visible_rows();
        let calls_after_a = source.calls();
        assert_eq!(a.len(), 2);
        assert!(calls_after_a > 0);

        let b = list.fetch_visible_rows();
        let calls_after_b = source.calls();
        assert_eq!(a, b);
        assert_eq!(calls_after_b, calls_after_a);

        source.bump_revision();
        let _ = list.fetch_visible_rows();
        assert!(source.calls() > calls_after_b);
    }

    #[test]
    fn a_revision_less_source_is_cached_and_invalidated_by_its_signal() {
        let mut list = VirtualList::new(Rect::new(0, 0, 120, 40));
        list.set_overscan(0);

        // Revision 0 = the trait's default, i.e. "I do not version my projection".
        let source = Arc::new(CountingSource::new(6, 0));
        list.set_data_source(source.clone());

        let first = list.fetch_visible_rows_shared();
        let calls_after_first = source.calls();
        assert!(calls_after_first > 0);

        let second = list.fetch_visible_rows_shared();
        assert!(Rc::ptr_eq(&first, &second), "the cached window is shared, not re-fetched");
        assert_eq!(source.calls(), calls_after_first, "the signal makes this source observable");

        source.touch();
        let _ = list.fetch_visible_rows_shared();
        assert!(source.calls() > calls_after_first, "a reported change must invalidate the window");
    }

    #[test]
    fn a_source_with_no_change_contract_is_not_cached() {
        /// A source that has neither a revision nor a `data_changed` signal: caching it would
        /// have no way to learn that the projection moved.
        struct SilentSource {
            rows: usize,
            calls: AtomicUsize,
        }

        impl IncrementalTableDataSource for SilentSource {
            fn row_count(&self) -> usize {
                self.rows
            }
            fn column_count(&self) -> usize {
                1
            }
            fn data(&self, row: usize, column: usize) -> Option<String> {
                self.calls.fetch_add(1, Ordering::Relaxed);
                (column == 0 && row < self.rows).then(|| format!("row-{row}"))
            }
        }

        let mut list = VirtualList::new(Rect::new(0, 0, 120, 40));
        list.set_overscan(0);
        let source = Arc::new(SilentSource { rows: 6, calls: AtomicUsize::new(0) });
        list.set_data_source(source.clone());

        let _ = list.fetch_visible_rows_shared();
        let calls_after_first = source.calls.load(Ordering::Relaxed);
        let _ = list.fetch_visible_rows_shared();
        assert!(
            source.calls.load(Ordering::Relaxed) > calls_after_first,
            "without a change contract every fetch must read the source"
        );
    }

    #[test]
    fn wheel_scroll_and_selection_behave() {
        let mut list = VirtualList::new(Rect::new(0, 0, 120, 60));
        list.set_data_source(Arc::new(StaticSource { rows: 20 }));

        list.handle_event(&Event::wheel(0, -120, 0));
        assert_eq!(list.scroll_row(), 1);

        list.handle_event(&Event::wheel(0, 120, 0));
        assert_eq!(list.scroll_row(), 0);

        list.handle_event(&Event::mouse_press(2, 42, 1));
        assert_eq!(list.selected_row(), Some(2));

        assert!(!list.select_row(999));
        assert_eq!(list.selected_row(), Some(2));
    }

    #[test]
    fn new_creates_default_state() {
        let mut list = VirtualList::new(Rect::new(0, 0, 800, 600));
        assert!(!list.has_data_source());
        assert!(list.data_source_ref().is_none());
        assert_eq!(list.scroll_row(), 0);
        assert_eq!(list.selected_row(), None);
        assert_eq!(list.row_height(), 20);
        assert_eq!(list.overscan(), 2);
        assert_eq!(list.row_count(), 0);
        assert_eq!(list.visible_window(), (0, 0));
        assert!(list.fetch_visible_rows().is_empty());
    }

    #[test]
    fn has_data_source_and_data_source_ref_work() {
        let mut list = VirtualList::new(Rect::new(0, 0, 800, 600));
        assert!(!list.has_data_source());
        assert!(list.data_source_ref().is_none());

        let source = Arc::new(StaticSource { rows: 5 });
        list.set_data_source(source.clone());
        assert!(list.has_data_source());
        assert!(list.data_source_ref().is_some());
        assert_eq!(list.data_source_ref().unwrap().row_count(), 5);

        list.clear_data_source();
        assert!(!list.has_data_source());
        assert!(list.data_source_ref().is_none());
    }

    #[test]
    fn clear_data_source_resets_state() {
        let mut list = VirtualList::new(Rect::new(0, 0, 800, 600));
        list.set_data_source(Arc::new(StaticSource { rows: 10 }));
        list.set_scroll_row(5);
        list.select_row(3);

        list.clear_data_source();
        assert!(!list.has_data_source());
        assert_eq!(list.scroll_row(), 0);
        assert_eq!(list.selected_row(), None);
        assert_eq!(list.row_count(), 0);
        assert_eq!(list.visible_window(), (0, 0));
        assert!(list.fetch_visible_rows().is_empty());
    }

    #[test]
    fn row_count_reflects_source() {
        let mut list = VirtualList::new(Rect::new(0, 0, 800, 600));
        assert_eq!(list.row_count(), 0);

        list.set_data_source(Arc::new(StaticSource { rows: 0 }));
        assert_eq!(list.row_count(), 0);

        list.set_data_source(Arc::new(StaticSource { rows: 7 }));
        assert_eq!(list.row_count(), 7);
    }

    #[test]
    fn scroll_row_clamps_and_invalid_positions_are_safe() {
        let mut list = VirtualList::new(Rect::new(0, 0, 800, 600));
        // No data source - scroll_row stays 0
        list.set_scroll_row(100);
        assert_eq!(list.scroll_row(), 0);

        list.set_data_source(Arc::new(StaticSource { rows: 5 }));
        list.set_scroll_row(3);
        assert_eq!(list.scroll_row(), 3);

        // Clamp to max index
        list.set_scroll_row(999);
        assert_eq!(list.scroll_row(), 4);

        // scroll_by_rows edge cases
        list.scroll_by_rows(-10);
        assert_eq!(list.scroll_row(), 0);

        list.scroll_by_rows(100);
        assert_eq!(list.scroll_row(), 4);
    }

    #[test]
    fn overscan_affects_visible_window() {
        let mut list = VirtualList::new(Rect::new(0, 0, 120, 40));
        list.set_data_source(Arc::new(StaticSource { rows: 20 }));

        // With overscan=2 (default), visible window extends beyond viewport
        let (start, len) = list.visible_window();
        assert_eq!(start, 0);
        assert!(len > 2);

        // Reduce overscan to 0
        list.set_overscan(0);
        let (start2, len2) = list.visible_window();
        assert_eq!(start2, 0);
        assert_eq!(len2, 2);

        assert_eq!(list.overscan(), 0);
    }

    #[test]
    fn signal_emission_on_selection_and_window_change() {
        let mut list = VirtualList::new(Rect::new(0, 0, 800, 600));
        list.set_data_source(Arc::new(StaticSource { rows: 5 }));

        let last_selection = Arc::new(std::sync::Mutex::new(None::<Option<usize>>));
        let sink = last_selection.clone();
        list.selection_changed.connect(move |sel| {
            *sink.lock().unwrap() = Some(*sel);
        });

        list.select_row(2);
        assert_eq!(*last_selection.lock().unwrap(), Some(Some(2)));
    }

    #[test]
    fn empty_source_handling() {
        let mut list = VirtualList::new(Rect::new(0, 0, 800, 600));
        list.set_data_source(Arc::new(StaticSource { rows: 0 }));

        assert_eq!(list.row_count(), 0);
        assert_eq!(list.visible_window(), (0, 0));
        assert!(list.fetch_visible_rows().is_empty());
        assert!(!list.select_row(0));
        assert_eq!(list.selected_row(), None);

        // Scrolling does nothing on empty source
        list.set_scroll_row(10);
        assert_eq!(list.scroll_row(), 0);
    }

    /// The row under the pointer is the row a click would select, and its hover reaches the base.
    ///
    /// # The two defects this pins
    ///
    /// This handler never forwarded to the base at all, so `widget_state()` answered `Normal` for
    /// the whole interaction and the theme's `"data_view:hover"` override could never fire — the
    /// same defect `list_view` records, in the sibling control that never inherited its fix. It also
    /// kept no row hover, so a virtual list gave no feedback about which row a click would affect.
    ///
    /// The round trip is asserted through the *same* derivation the click uses, so the two cannot
    /// drift apart: ask [`VirtualList::row_at`] which row a point is over, then click that point and
    /// require the same row to be selected.
    #[test]
    fn the_row_under_the_pointer_is_the_row_a_click_selects() {
        use crate::style::WidgetState;

        let mut list = VirtualList::new(Rect::new(0, 0, 120, 60));
        list.set_data_source(Arc::new(StaticSource { rows: 20 }));

        let probe = Point::new(4, 22);
        let row = list.row_at(probe).expect("the second row is under this point");
        assert_eq!(row, 1);

        list.handle_event(&Event::MouseEnter { pos: probe });
        list.handle_event(&Event::MouseMove { pos: probe });
        assert_eq!(list.hovered_row, Some(row), "the row under the pointer is the one lit");
        assert_eq!(
            list.widget_state(),
            WidgetState::Hover,
            "the base must have been told the pointer arrived"
        );

        list.handle_event(&Event::mouse_press(4, 22, 1));
        assert_eq!(
            list.selected_row(),
            Some(row),
            "the highlighted row must be the row a click selects"
        );
        list.handle_event(&Event::mouse_release(4, 22, 1));

        // A point past the last row is over nothing, on both readings.
        let below = Point::new(4, 1000);
        assert_eq!(list.row_at(below), None);
        list.handle_event(&Event::MouseLeave { pos: below });
        assert_eq!(list.hovered_row, None, "leaving clears the row rather than latching it");
        assert_eq!(list.widget_state(), WidgetState::Normal);
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

    /// A press, a move past the threshold and a release inside the list reorder the displayed
    /// rows, keep the selection on the moved item and report `Move` to the source.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn a_drag_within_the_list_reorders_rows_and_reports_move() {
        use crate::event::mouse_button::PRIMARY;
        use crate::widget::runtime::{register, with_widget_mut, DragCoordinator};
        use std::sync::{Arc, Mutex};

        let mut list = VirtualList::new(Rect::new(0, 0, 200, 100));
        list.set_rows(vec!["A".to_string(), "B".to_string(), "C".to_string()]);
        assert!(list.select_row(0));
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
                .downcast_mut::<VirtualList>()
                .expect("the mounted control is a virtual list");
            let rows =
                list.fetch_visible_rows().into_iter().map(|(_, text)| text).collect::<Vec<_>>();
            (rows, list.selected_row())
        })
        .expect("the list is still mounted");
        assert_eq!(observed.0, vec!["B", "C", "A"]);
        assert_eq!(observed.1, Some(2), "the selection follows the row that moved");
        assert_eq!(*finished.lock().unwrap(), Some(DropEffect::Move));
    }

    /// Keyboard navigation and the scroll-bar thumb are wired.
    #[test]
    fn keyboard_and_scrollbar_are_wired() {
        use crate::event::EventHandler;
        let mut list = VirtualList::new(Rect::new(0, 0, 200, 80));
        list.set_rows((0..50).map(|index| std::format!("row {index}")).collect());
        list.set_show_scrollbar(true);
        assert!(list.show_scrollbar());
        list.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Tab });
        list.handle_event(&Event::KeyPress { key: crate::event::key_codes::DOWN, modifiers: 0 });
        assert_eq!(list.selected_row(), Some(1));
        list.handle_event(&Event::KeyPress {
            key: crate::event::key_codes::PAGE_DOWN,
            modifiers: 0,
        });
        assert!(list.selected_row().unwrap_or(0) > 1);
        list.handle_event(&Event::KeyPress { key: crate::event::key_codes::HOME, modifiers: 0 });
        assert_eq!(list.selected_row(), Some(0));
    }

    /// F-P08: the identity check is a maintained flag, not a scan, and a reorder clears it.
    #[test]
    fn row_order_identity_is_maintained_incrementally() {
        let mut list = VirtualList::new(Rect::new(0, 0, 400, 200));
        list.set_rows((0..20).map(|row| format!("row {row}")).collect());
        assert!(list.row_order_is_identity(), "a fresh projection is the identity");

        assert!(list.move_row(0, 2));
        assert!(!list.row_order_is_identity(), "a move breaks the identity");

        // Rebinding the source resets the arrangement to identity.
        list.set_rows((0..20).map(|row| format!("row {row}")).collect());
        assert!(list.row_order_is_identity());
    }

    /// V-18: `scroll_to_row` honours the alignment hint and `ensure_visible` moves minimally.
    #[test]
    fn scroll_to_row_aligns_and_ensure_visible_moves_minimally() {
        let mut list = VirtualList::new(Rect::new(0, 0, 400, 200));
        list.set_rows((0..100).map(|row| format!("row {row}")).collect());
        let visible = list.visible_capacity().max(1);

        assert!(list.scroll_to_row(50, ScrollHint::Top));
        assert_eq!(list.scroll_row(), 50);
        assert!(list.scroll_to_row(50, ScrollHint::Center));
        assert_eq!(list.scroll_row(), 50usize.saturating_sub(visible / 2));
        assert!(list.scroll_to_row(50, ScrollHint::Bottom));
        assert_eq!(list.scroll_row(), 51usize.saturating_sub(visible));

        // Nearest: a row already on screen does not scroll; one past the end scrolls just enough.
        list.set_scroll_row(10);
        assert!(list.ensure_visible(12));
        assert_eq!(list.scroll_row(), 10, "already visible");
        assert!(list.ensure_visible(10 + visible + 3));
        assert_eq!(list.scroll_row(), 10 + 4, "moved by exactly the overflow");
        assert!(!list.scroll_to_row(10_000, ScrollHint::Top), "there is no such row");
    }

    /// V-18: selecting a row follows it into view unless the follow-on scroll is disabled.
    #[test]
    fn selecting_a_row_scrolls_it_into_view() {
        let mut list = VirtualList::new(Rect::new(0, 0, 400, 200));
        list.set_rows((0..100).map(|row| format!("row {row}")).collect());

        assert!(list.select_row(60));
        assert!(list.scroll_row() > 0, "the selected row was brought on screen");

        list.set_scroll_row(0);
        list.set_selection_scrolls_into_view(false);
        assert!(list.select_row(60));
        assert_eq!(list.scroll_row(), 0, "the follow-on scroll is off");
    }
}
