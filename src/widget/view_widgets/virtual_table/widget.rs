// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! VirtualTable widget backed by incremental table data source.

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use crate::compat::Rc;
use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler, WheelScroll};
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, Signal1};
use crate::widget::capability::access::selection_mode_to_str;
use crate::widget::capability::coercion::{
    expect_bool, expect_selection_mode, expect_u32, expect_usize,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::view_widgets::collection_state::{
    paint_collection_placeholder, CollectionPlaceholder,
};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

use crate::widget::view_widgets::data_grid::SortSpec;
use crate::widget::view_widgets::data_source::IncrementalTableDataSource;

mod delegate;
pub use delegate::VirtualCellDelegate;

mod events;
mod properties;
mod render;
#[cfg(test)]
mod tests;

/// How one wheel notch scrolls a [`VirtualTable`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WheelScrollMode {
    /// One notch is one row (the historical behaviour).
    #[default]
    PerRow,
    /// One notch is one visible page of rows.
    PerPage,
}

fn wheel_scroll_mode_to_str(mode: WheelScrollMode) -> &'static str {
    match mode {
        WheelScrollMode::PerRow => "per_row",
        WheelScrollMode::PerPage => "per_page",
    }
}

fn expect_wheel_scroll_mode(
    value: CapabilityValue,
) -> Result<WheelScrollMode, CapabilityAccessError> {
    match value {
        CapabilityValue::String(token) => match token.as_str() {
            "per_row" => Ok(WheelScrollMode::PerRow),
            "per_page" => Ok(WheelScrollMode::PerPage),
            _ => Err(CapabilityAccessError::TypeMismatch),
        },
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

/// One cached window row, shared so a vertical scroll can reuse it instead of re-fetching.
type CachedRow = Arc<Vec<Option<String>>>;

/// The inputs a sorted projection was built from.
#[derive(Clone, Debug, PartialEq, Eq)]
struct TableSortKey {
    revision: u64,
    generation: u64,
    rows: usize,
    specs: Vec<(usize, bool)>,
}

/// Painted column geometry: a width per source column and its x offset from the cells' left edge.
///
/// A hidden column has width `0`, so it keeps its absolute index (scroll positions and the data
/// window stay absolute) while consuming no space. Built once per width/hidden change rather than
/// summed per painted cell.
#[derive(Clone, Debug)]
struct ColumnLayout {
    revision: u64,
    widths: Vec<u32>,
    offsets: Vec<i32>,
}

/// Cached header label strings, keyed by the source and label revisions (VP-007).
#[derive(Default)]
struct HeaderNameCache {
    /// The source's `revision()` the names were resolved at.
    revision: u64,
    /// The generation counter the widget advanced on `data_changed`.
    generation: u64,
    /// [`VirtualTable::header_text_revision`] the names were resolved at.
    labels_revision: u64,
    /// The source's column count when the cache was built.
    column_count: usize,
    /// The resolved, elided label per column.
    entries: HashMap<usize, Rc<String>>,
}

/// Lightweight virtual table with pull-based window fetch.
pub struct VirtualTable {
    base: BaseWidget,
    data_source: Option<Arc<dyn IncrementalTableDataSource>>,
    scroll_row: usize,
    scroll_column: usize,
    row_height: u32,
    column_width: u32,
    overscan_rows: usize,
    overscan_columns: usize,
    window_cache: Option<Arc<Vec<CachedRow>>>,
    /// The `(row_start, row_len, col_start, col_len)` window `window_cache` holds.
    ///
    /// A scroll keeps the overlap and fetches only the new side, so the cache needs to know which
    /// rows it already has (F-P06).
    window_key: Option<(usize, usize, usize, usize)>,
    /// Revision of the data source that produced `window_cache`. A source can change in
    /// place without any setter running (it bumps `revision()`), so a cache validated only
    /// by scroll position would keep painting stale cells.
    window_revision: u64,
    /// Disconnects the bound source's `data_changed` subscription; replaced on rebind.
    data_source_connection_scope: ConnectionScope,
    /// Cache key for a revision-less source, advanced by its `data_changed` signal — the same
    /// invalidation contract `VirtualList` and `DataGrid` apply.
    data_generation: Arc<AtomicU64>,
    /// `true` when the bound source has `revision() > 0` or a `data_changed` signal, i.e. the
    /// window cache has an invalidation contract and may be kept between calls.
    data_is_observable: bool,
    /// Set by a source change signal; drained before row/column state is read.
    ///
    /// The signal cannot borrow the control mutably, so it only records that a structural change is
    /// pending. The drain clamps the scroll, drops stale hover and re-encodes the selection once,
    /// which keeps the repair O(selected) instead of running in every accessor.
    data_dirty: Arc<AtomicBool>,
    /// The row the pointer is over, as the data source's own **absolute** row index.
    ///
    /// Stored absolutely like `grid_table` stores its hover, so a scroll moves the
    /// highlight with its row instead of lighting whatever viewport-local index the
    /// row used to sit at.
    hovered_row: Option<usize>,
    /// Emitted when visible window changes `(row_start,row_len,col_start,col_len)`.
    pub visible_window_changed: Signal1<(usize, usize, usize, usize)>,
    /// Whether a column-title row is painted above the first data row.
    show_header: bool,
    /// Whether a vertical scroll-bar thumb is painted.
    show_scrollbar: bool,
    /// Emitted when the selected cell changes.
    pub selection_changed: Signal1<Option<(usize, usize)>>,
    /// Emitted with the full selection set whenever it changes (V-06).
    ///
    /// `selection_changed` keeps its original single-value payload so existing subscribers keep
    /// working; a multi-select host reads the complete set from here or from
    /// [`VirtualTable::selected_cells`].
    pub selection_set_changed: Signal1<Vec<(usize, usize)>>,
    /// The selection set, stored as linearised cell indices so one model serves every mode.
    selection: crate::widget::view_widgets::list_view::SelectionModel,
    /// The column count the stored selection indices were encoded with.
    ///
    /// The model stores one `usize` per cell (`row * columns + column`), so the numbers are only
    /// meaningful together with the width that produced them. Remembering it lets a source change
    /// decode the old cell, clamp it and re-encode it under the new count instead of silently
    /// reinterpreting index 7 as a different cell.
    selection_columns: usize,
    /// Optional per-cell painter.
    delegate: Option<Arc<dyn VirtualCellDelegate>>,
    /// Labels supplied through [`VirtualTable::set_header_labels`].
    header_labels: Vec<String>,
    /// How one wheel notch scrolls.
    wheel_scroll_mode: WheelScrollMode,
    /// Per-column width overrides; absent columns use `column_width`.
    column_widths: HashMap<usize, u32>,
    /// Columns excluded from the painted order (V-013).
    hidden_columns: BTreeSet<usize>,
    /// Bumped by every width/hidden mutation; keys [`ColumnLayout`].
    column_layout_revision: u64,
    /// The cached column geometry.
    column_layout_cache: RefCell<Option<ColumnLayout>>,
    /// Sort specs in priority order (V-013). Empty means source order.
    sort_specs: Vec<SortSpec>,
    /// Whether a header click may change the sort.
    sorting_enabled: bool,
    /// Projected row -> source row when sorting is active; `None` means identity.
    sort_order: Option<Vec<usize>>,
    /// The inputs `sort_order` was built from.
    sort_order_key: Option<TableSortKey>,
    /// Window cache used while a sorted projection is active (rows are not contiguous).
    sorted_window_cache: Option<SortedWindowCache>,
    /// Whether odd rows receive the alternating wash (V-013). Off by default.
    alternating_rows: bool,
    /// Emitted with `(column, descending)` when a header click changes the sort (V-013).
    pub sort_changed: Signal1<(usize, bool)>,
    /// Emitted when the context menu is requested at a point (V-013).
    pub context_menu_requested: Signal1<Point>,
    /// Per-column cell alignment (V-001); an absent entry means `Left`.
    column_alignments: HashMap<usize, HorizontalAlignment>,
    /// Bumped by `set_header_labels`, so the header name cache can invalidate (VP-007).
    header_text_revision: u64,
    /// Elided header labels, including the `"Column N"` fallback (VP-007).
    header_text_cache: RefCell<HeaderNameCache>,
    /// Empty/error captions painted when the source has no rows (RW-17).
    placeholder: CollectionPlaceholder,
}

/// The sorted projection's window cache: the sort revision, the four window bounds, and the rows
/// that window produced.
///
/// Named because the tuple is four components plus two generic carriers — a
/// `clippy::type_complexity` finding, and unreadable at the assignment site without the name.
type SortedWindowCache = (u64, (usize, usize, usize, usize), Arc<Vec<CachedRow>>);

impl VirtualTable {
    /// Creates empty virtual table.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Table, geometry, "VirtualTable"),
            data_source: None,
            scroll_row: 0,
            scroll_column: 0,
            row_height: 20,
            column_width: 120,
            overscan_rows: 2,
            overscan_columns: 1,
            window_cache: None,
            window_key: None,
            window_revision: 0,
            data_source_connection_scope: ConnectionScope::new(),
            data_generation: Arc::new(AtomicU64::new(0)),
            data_is_observable: false,
            data_dirty: Arc::new(AtomicBool::new(false)),
            hovered_row: None,
            visible_window_changed: Signal1::new(),
            show_header: false,
            show_scrollbar: false,
            selection_changed: Signal1::new(),
            selection_set_changed: Signal1::new(),
            selection: crate::widget::view_widgets::list_view::SelectionModel::new(),
            selection_columns: 0,
            delegate: None,
            header_labels: Vec::new(),
            wheel_scroll_mode: WheelScrollMode::PerRow,
            column_widths: HashMap::new(),
            hidden_columns: BTreeSet::new(),
            column_layout_revision: 0,
            column_layout_cache: RefCell::new(None),
            sort_specs: Vec::new(),
            sorting_enabled: true,
            sort_order: None,
            sort_order_key: None,
            sorted_window_cache: None,
            alternating_rows: false,
            sort_changed: Signal1::new(),
            context_menu_requested: Signal1::new(),
            column_alignments: HashMap::new(),
            header_text_revision: 0,
            header_text_cache: RefCell::new(HeaderNameCache::default()),
            placeholder: CollectionPlaceholder::new(),
        }
    }

    /// Binds data source.
    pub fn set_data_source(&mut self, source: Arc<dyn IncrementalTableDataSource>) {
        self.data_source_connection_scope = ConnectionScope::new();
        let revision_nonzero = source.revision() > 0;
        let has_signal = if let Some(data_changed) = source.data_changed_signal() {
            let redraw = self.base.redraw_requested_signal().clone();
            let layout = self.base.layout_requested_signal().clone();
            let generation = Arc::clone(&self.data_generation);
            let dirty = Arc::clone(&self.data_dirty);
            data_changed.connect_scoped(&self.data_source_connection_scope, move || {
                // The source says its projection changed. Advancing the key is what makes the
                // next fetch miss; a revision-less source has no other way to invalidate.
                generation.fetch_add(1, Ordering::Relaxed);
                dirty.store(true, Ordering::Relaxed);
                redraw.emit();
                layout.emit();
            });
            true
        } else {
            false
        };
        self.data_generation.fetch_add(1, Ordering::Relaxed);
        self.data_is_observable = revision_nonzero || has_signal;
        self.data_source = Some(source);
        self.scroll_row = 0;
        self.scroll_column = 0;
        self.window_cache = None;
        self.sorted_window_cache = None;
        self.hovered_row = None;
        // The old selection was encoded against the previous source's columns, so it is re-encoded
        // against this one before a paint can read it.
        self.reseat_selection();
        self.emit_visible_window();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Clears data source.
    pub fn clear_data_source(&mut self) {
        self.data_source_connection_scope = ConnectionScope::new();
        self.data_is_observable = false;
        self.data_generation.fetch_add(1, Ordering::Relaxed);
        self.data_source = None;
        self.scroll_row = 0;
        self.scroll_column = 0;
        self.window_cache = None;
        self.sorted_window_cache = None;
        self.hovered_row = None;
        // The row and column counts are now zero, so the selection is empty; the re-encode reports
        // that loss to a host mirroring it instead of leaving stale linear indices behind.
        self.reseat_selection();
        self.emit_visible_window();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns row count.
    pub fn row_count(&self) -> usize {
        self.data_source.as_ref().map(|source| source.row_count()).unwrap_or(0)
    }

    /// Returns column count.
    pub fn column_count(&self) -> usize {
        self.data_source.as_ref().map(|source| source.column_count()).unwrap_or(0)
    }

    /// Returns whether source is bound.
    pub fn has_data_source(&self) -> bool {
        self.data_source.is_some()
    }

    /// Returns scroll row.
    ///
    /// Clamped to the current projection: a source can shrink without the control having run a
    /// repair pass yet, and reporting the stale index would both paint a blank body and send a host
    /// that restores it to a row that no longer exists.
    pub fn scroll_row(&self) -> usize {
        if self.data_source.is_some() {
            self.scroll_row.min(self.row_count().saturating_sub(1))
        } else {
            self.scroll_row
        }
    }

    /// Returns scroll column, clamped to the current projection for the same reason.
    pub fn scroll_column(&self) -> usize {
        if self.data_source.is_some() {
            self.scroll_column.min(self.column_count().saturating_sub(1))
        } else {
            self.scroll_column
        }
    }

    /// Sets scroll row with clamp.
    pub fn set_scroll_row(&mut self, row: usize) {
        let max_row = self.row_count().saturating_sub(1);
        let next = row.min(max_row);
        if next == self.scroll_row {
            return;
        }
        self.scroll_row = next;
        self.emit_visible_window();
        self.base.request_redraw();
    }

    /// Sets scroll column with clamp.
    pub fn set_scroll_column(&mut self, column: usize) {
        let max_column = self.column_count().saturating_sub(1);
        let next = column.min(max_column);
        if next == self.scroll_column {
            return;
        }
        self.scroll_column = next;
        self.emit_visible_window();
        self.base.request_redraw();
    }

    /// Returns row height.
    pub fn row_height(&self) -> u32 {
        self.row_height
    }

    /// Sets row height with minimum clamp.
    pub fn set_row_height(&mut self, row_height: u32) {
        let next = row_height.max(1);
        if next == self.row_height {
            return;
        }
        self.row_height = next;
        self.window_cache = None;
        self.emit_visible_window();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns column width.
    pub fn column_width(&self) -> u32 {
        self.column_width
    }

    /// Sets column width with minimum clamp.
    pub fn set_column_width(&mut self, column_width: u32) {
        let next = column_width.max(1);
        if next == self.column_width {
            return;
        }
        self.column_width = next;
        self.bump_column_layout();
        self.window_cache = None;
        self.emit_visible_window();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The width painted for `column` (V-013).
    ///
    /// A per-column override wins over [`Self::column_width`]; a hidden column answers `0`
    /// because it occupies no space.
    pub fn column_width_at(&self, column: usize) -> u32 {
        if self.hidden_columns.contains(&column) {
            return 0;
        }
        self.with_column_layout(|layout| {
            layout.widths.get(column).copied().unwrap_or(self.column_width.max(1))
        })
    }

    /// Gives `column` its own width (V-013). Returns `false` for a column the source lacks.
    pub fn set_column_width_at(&mut self, column: usize, width: u32) -> bool {
        if column >= self.column_count() {
            return false;
        }
        let width = width.max(1);
        if self.column_widths.get(&column).copied() == Some(width) {
            return true;
        }
        self.column_widths.insert(column, width);
        self.bump_column_layout();
        self.window_cache = None;
        self.emit_visible_window();
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    /// Replaces every per-column width; an entry of `0` returns that column to the default.
    pub fn set_column_widths(&mut self, widths: &[u32]) {
        self.column_widths.clear();
        for (column, width) in widths.iter().enumerate() {
            if *width > 0 {
                self.column_widths.insert(column, *width);
            }
        }
        self.bump_column_layout();
        self.window_cache = None;
        self.emit_visible_window();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Whether `column` is hidden (V-013).
    pub fn is_column_hidden(&self, column: usize) -> bool {
        self.hidden_columns.contains(&column)
    }

    /// The hidden columns, in ascending order (V-013).
    pub fn hidden_columns(&self) -> Vec<usize> {
        self.hidden_columns.iter().copied().collect()
    }

    /// Replaces the hidden-column set (V-013).
    pub fn set_hidden_columns(&mut self, columns: Vec<usize>) {
        let available = self.column_count();
        let next: BTreeSet<usize> =
            columns.into_iter().filter(|column| *column < available).collect();
        if next == self.hidden_columns {
            return;
        }
        self.hidden_columns = next;
        self.bump_column_layout();
        self.window_cache = None;
        self.emit_visible_window();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Hides or shows `column` (V-013). Returns `false` for a column the source lacks.
    pub fn set_column_hidden(&mut self, column: usize, hidden: bool) -> bool {
        if column >= self.column_count() {
            return false;
        }
        let changed = if hidden {
            self.hidden_columns.insert(column)
        } else {
            self.hidden_columns.remove(&column)
        };
        if changed {
            self.bump_column_layout();
            self.window_cache = None;
            self.emit_visible_window();
            self.base.request_layout();
            self.base.request_redraw();
        }
        true
    }

    /// Bumps the column-geometry revision so the cached layout is rebuilt.
    fn bump_column_layout(&mut self) {
        self.column_layout_revision = self.column_layout_revision.wrapping_add(1);
    }

    /// Builds the painted column geometry from the width overrides and the hidden set.
    fn build_column_layout(&self) -> ColumnLayout {
        let columns = self.column_count();
        let mut widths = Vec::with_capacity(columns);
        let mut offsets = Vec::with_capacity(columns);
        let mut offset = 0i32;
        for column in 0..columns {
            let width = if self.hidden_columns.contains(&column) {
                0
            } else {
                self.column_widths.get(&column).copied().unwrap_or(self.column_width).max(1)
            };
            widths.push(width);
            offsets.push(offset);
            offset = offset.saturating_add(width as i32);
        }
        ColumnLayout { revision: self.column_layout_revision, widths, offsets }
    }

    /// Runs `f` over the column layout, rebuilding it if it is stale.
    fn with_column_layout<R>(&self, f: impl FnOnce(&ColumnLayout) -> R) -> R {
        let mut cache = self.column_layout_cache.borrow_mut();
        let stale = cache.as_ref().is_none_or(|layout| {
            layout.revision != self.column_layout_revision
                || layout.widths.len() != self.column_count()
        });
        if stale {
            *cache = Some(self.build_column_layout());
        }
        f(cache.as_ref().expect("the column layout was just built"))
    }

    /// The x offset of `column`'s left edge, measured from the cells' left edge.
    ///
    /// The layout is the authority while the source provides columns; past its last column — and
    /// with no source at all, where the layout is empty — the uniform stride answers instead, so a
    /// cell's box stays `column * column_width` for any index.
    fn column_offset(&self, column: usize) -> i32 {
        self.with_column_layout(|layout| {
            layout
                .offsets
                .get(column)
                .copied()
                .unwrap_or_else(|| self.uniform_column_offset(column))
        })
    }

    /// The offset the uniform stride assigns to `column`, saturated against `i32`.
    fn uniform_column_offset(&self, column: usize) -> i32 {
        let stride = self.column_width.max(1) as u64;
        (column as u64).saturating_mul(stride).min(i32::MAX as u64) as i32
    }

    /// The absolute column painted at viewport-local ordinal `local`.
    ///
    /// With no hidden columns this is exactly `scroll_column + local`, the historical identity;
    /// hiding a column removes its slot without renumbering the source columns.
    fn absolute_column(&self, local: usize) -> Option<usize> {
        if self.hidden_columns.is_empty() {
            let column = self.scroll_column.saturating_add(local);
            return (column < self.column_count()).then_some(column);
        }
        self.with_column_layout(|layout| {
            layout
                .widths
                .iter()
                .enumerate()
                .skip(self.scroll_column)
                .filter(|(_, width)| **width > 0)
                .nth(local)
                .map(|(column, _)| column)
        })
    }

    /// How many visible columns of `width` fit starting at the scroll position.
    ///
    /// With no source the layout is empty, so the uniform stride decides by division — the same
    /// answer the historical `width / column_width` gave.
    fn visible_column_capacity(&self, width: u32) -> usize {
        self.with_column_layout(|layout| {
            if layout.widths.is_empty() {
                return (width / self.column_width.max(1)).max(1) as usize;
            }
            let mut used = 0u32;
            let mut count = 0usize;
            for column in self.scroll_column.min(layout.widths.len())..layout.widths.len() {
                let column_width = layout.widths[column];
                if column_width == 0 {
                    continue;
                }
                if count > 0 && used.saturating_add(column_width) > width {
                    break;
                }
                used = used.saturating_add(column_width);
                count += 1;
                if used >= width {
                    break;
                }
            }
            count.max(1)
        })
    }

    /// Returns extra fetched rows around viewport.
    pub fn overscan_rows(&self) -> usize {
        self.overscan_rows
    }

    /// Sets extra fetched rows around viewport.
    pub fn set_overscan_rows(&mut self, overscan_rows: usize) {
        if overscan_rows == self.overscan_rows {
            return;
        }
        self.overscan_rows = overscan_rows;
        self.window_cache = None;
        self.emit_visible_window();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns extra fetched columns around viewport.
    pub fn overscan_columns(&self) -> usize {
        self.overscan_columns
    }

    /// Sets extra fetched columns around viewport.
    pub fn set_overscan_columns(&mut self, overscan_columns: usize) {
        if overscan_columns == self.overscan_columns {
            return;
        }
        self.overscan_columns = overscan_columns;
        self.window_cache = None;
        self.emit_visible_window();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The sort specs, in priority order (V-013). Empty means source order.
    pub fn sort_specs(&self) -> &[SortSpec] {
        &self.sort_specs
    }

    /// Replaces the sort specs. The first spec breaks the tie for the second, and so on.
    pub fn set_sort_specs(&mut self, sort_specs: Vec<SortSpec>) {
        if self.sort_specs == sort_specs {
            return;
        }
        self.sort_specs = sort_specs;
        self.invalidate_sort_order();
    }

    /// Whether a header click may change the sort (V-013).
    pub fn sorting_enabled(&self) -> bool {
        self.sorting_enabled
    }

    /// Enables or disables header-driven sorting.
    pub fn set_sorting_enabled(&mut self, sorting_enabled: bool) {
        if self.sorting_enabled == sorting_enabled {
            return;
        }
        self.sorting_enabled = sorting_enabled;
        if !sorting_enabled {
            self.sort_specs.clear();
        }
        self.invalidate_sort_order();
    }

    /// Cycles `column` ascending -> descending -> unsorted (V-013).
    ///
    /// Returns the new direction, or `None` for the unsorted state (or a column that cannot be
    /// sorted). The third state is what restores source order; a two-state toggle cannot reach it.
    pub fn toggle_sort_column(&mut self, column: usize) -> Option<bool> {
        if !self.sorting_enabled || column >= self.column_count() {
            return None;
        }
        let existing = self.sort_specs.iter().position(|spec| spec.column == column);
        let descending = match existing {
            Some(index) if self.sort_specs[index].descending => {
                self.sort_specs.remove(index);
                None
            }
            Some(index) => {
                self.sort_specs[index].descending = true;
                Some(true)
            }
            None => {
                self.sort_specs.push(SortSpec { column, descending: false });
                Some(false)
            }
        };
        self.invalidate_sort_order();
        if let Some(descending) = descending {
            self.sort_changed.emit((column, descending));
        }
        descending
    }

    /// Whether odd rows receive the alternating wash (V-013). Off by default.
    pub fn alternating_rows(&self) -> bool {
        self.alternating_rows
    }

    /// Enables or disables the alternating row wash.
    pub fn set_alternating_rows(&mut self, alternating_rows: bool) {
        if self.alternating_rows == alternating_rows {
            return;
        }
        self.alternating_rows = alternating_rows;
        self.base.request_redraw();
    }

    /// Drops the projected order and every window derived from it.
    fn invalidate_sort_order(&mut self) {
        self.sort_order = None;
        self.sort_order_key = None;
        self.sorted_window_cache = None;
        self.window_cache = None;
        self.emit_visible_window();
        self.base.request_redraw();
    }

    /// Rebuilds the projected order when its inputs moved.
    ///
    /// The key table holds one normalised key per row per spec, computed before the sort; the
    /// comparator never re-reads the source (the same kernel `DataGrid` uses).
    fn ensure_sort_order(&mut self) {
        let revision = self.data_source.as_ref().map(|source| source.revision()).unwrap_or(0);
        let generation = self.data_generation.load(Ordering::Relaxed);
        let rows = self.row_count();
        let columns = self.column_count();
        let specs: Vec<(usize, bool)> = self
            .sort_specs
            .iter()
            .filter(|spec| spec.column < columns)
            .map(|spec| (spec.column, spec.descending))
            .collect();
        let key = TableSortKey { revision, generation, rows, specs };

        if self.sort_order_key.as_ref() == Some(&key) {
            return;
        }
        if !self.sorting_enabled || key.specs.is_empty() || rows == 0 {
            self.sort_order = None;
            self.sort_order_key = Some(key);
            return;
        }
        let Some(source) = self.data_source.clone() else {
            self.sort_order = None;
            self.sort_order_key = Some(key);
            return;
        };

        let mut keys: Vec<Vec<(String, Option<f64>)>> = Vec::with_capacity(key.specs.len());
        for &(column, _) in &key.specs {
            let mut per_row = Vec::with_capacity(rows);
            for row in 0..rows {
                let raw = source.data(row, column).unwrap_or_default();
                per_row.push(crate::widget::view_widgets::data_grid::normalise_sort_key(&raw));
            }
            keys.push(per_row);
        }

        let mut order: Vec<usize> = (0..rows).collect();
        order.sort_by(|&left, &right| {
            for (index, spec) in key.specs.iter().enumerate() {
                let ordering = crate::widget::view_widgets::data_grid::compare_sort_keys(
                    &keys[index][left],
                    &keys[index][right],
                );
                if ordering != core::cmp::Ordering::Equal {
                    return if spec.1 { ordering.reverse() } else { ordering };
                }
            }
            // Stable tie-break on the source row, so equal keys do not reorder between passes.
            left.cmp(&right)
        });
        self.sort_order = Some(order);
        self.sort_order_key = Some(key);
    }

    /// Returns `(row_start,row_len,col_start,col_len)` for visible+overscan window.
    pub fn visible_window(&self) -> (usize, usize, usize, usize) {
        let rows = self.row_count();
        let cols = self.column_count();
        if rows == 0 || cols == 0 {
            return (0, 0, 0, 0);
        }

        let row_start = self.scroll_row().saturating_sub(self.overscan_rows);
        let col_start = self.scroll_column().saturating_sub(self.overscan_columns);
        let visible_rows = (self.base.geometry().height / self.row_height.max(1)) as usize;
        // The width-aware capacity keeps the window wide enough when a column is narrower than
        // the default or a hidden column frees its slot (V-013). With the default uniform width
        // it is exactly the old `width / column_width`.
        let visible_cols = self.visible_column_capacity(self.base.geometry().width);
        let row_len = visible_rows.saturating_add(self.overscan_rows.saturating_mul(2)).max(1);
        let col_len = visible_cols.saturating_add(self.overscan_columns.saturating_mul(2)).max(1);
        (row_start, row_len, col_start, col_len)
    }

    /// Pulls currently visible window.
    ///
    /// Returns an **owned** window, preserving the established public contract. A cache hit shares
    /// the cached matrix behind an [`Arc`] and only clones the handle, not the window's rows,
    /// columns or strings (D09-VIEW-03); the draw path uses `Self::visible_window_cells` to read
    /// the same cached matrix without even cloning the handle.
    pub fn fetch_visible_window(&mut self) -> Vec<Vec<Option<String>>> {
        match self.visible_window_cells() {
            Some(window) => window.iter().map(|row| row.as_ref().clone()).collect(),
            None => Vec::new(),
        }
    }

    /// Returns the shared visible window, fetching from the source only on a cache miss.
    ///
    /// # Why the cache is shared rather than cloned (D09-VIEW-03)
    ///
    /// `fetch_visible_window` used to return `cache.clone()` on every cache hit, and `Draw::draw`
    /// calls it on every redraw: a static window therefore re-copied every row, column and
    /// `String` allocation in the visible window each frame. The cache is held behind an `Arc`, so
    /// a hit hands out a cheap shared handle and the draw loop reads the matrix in place. The
    /// fetch count is unchanged (misses still hit the source), so only the per-redraw copies are
    /// removed.
    fn visible_window_cells(&mut self) -> Option<&Arc<Vec<CachedRow>>> {
        // A pending source change may have shrunk the row/column counts, so the window is computed
        // from repaired state rather than from indices the source no longer has.
        self.drain_data_updates();
        let revision = self.data_source.as_ref()?.revision();
        // A source with a real revision uses it; one without falls back to the generation this
        // widget advances from `data_changed` — the same contract as `VirtualList`/`DataGrid`.
        // A source with neither has no invalidation contract, so its window is refetched on
        // every call instead of being served stale.
        let cache_revision =
            if revision > 0 { revision } else { self.data_generation.load(Ordering::Relaxed) };
        let window = self.visible_window();

        // A sorted projection paints non-contiguous source rows, so the sliding overlap cache
        // below (which assumes a contiguous row run) cannot serve it. Its own window cache is
        // keyed by revision and window; per-row fetches happen only on a miss (V-013).
        self.ensure_sort_order();
        if self.sort_order.is_some() {
            let stale = match self.sorted_window_cache.as_ref() {
                Some((cached_revision, cached_window, _)) => {
                    *cached_revision != cache_revision || *cached_window != window
                }
                None => true,
            };
            if !stale {
                return self.sorted_window_cache.as_ref().map(|(_, _, rows)| rows);
            }
            let source = self.data_source.clone()?;
            let mut rows: Vec<CachedRow> = Vec::with_capacity(window.1);
            if let Some(order) = self.sort_order.as_ref() {
                for projected in window.0..window.0.saturating_add(window.1) {
                    let Some(&source_row) = order.get(projected) else { break };
                    let mut fetched = source.fetch_window(source_row, 1, window.2, window.3);
                    rows.push(Arc::new(fetched.pop().unwrap_or_default()));
                }
            }
            self.sorted_window_cache = Some((cache_revision, window, Arc::new(rows)));
            return self.sorted_window_cache.as_ref().map(|(_, _, rows)| rows);
        }

        // F-P06: a scroll that keeps the column span slides the row cache. The overlapping rows
        // are reused behind their `Arc`, and only the newly exposed side is fetched.
        if self.data_is_observable
            && self.window_cache.is_some()
            && self.window_revision == cache_revision
        {
            if self.window_key == Some(window) {
                return self.window_cache.as_ref();
            }
            if let Some((old_start, old_len, old_col_start, old_col_len)) = self.window_key {
                if old_col_start == window.2 && old_col_len == window.3 {
                    let (new_start, new_len, _, _) = window;
                    let old_end = old_start.saturating_add(old_len);
                    let new_end = new_start.saturating_add(new_len);
                    let overlap_start = old_start.max(new_start);
                    let overlap_end = old_end.min(new_end);
                    if overlap_start < overlap_end {
                        let source = self.data_source.as_ref()?;
                        let mut rows: Vec<CachedRow> = Vec::with_capacity(new_len);
                        if new_start < overlap_start {
                            let gap = overlap_start - new_start;
                            rows.extend(
                                source
                                    .fetch_window(new_start, gap, window.2, window.3)
                                    .into_iter()
                                    .map(Arc::new),
                            );
                        }
                        {
                            let cached = self.window_cache.as_ref()?;
                            for row in overlap_start..overlap_end {
                                if let Some(cached_row) = cached.get(row.saturating_sub(old_start))
                                {
                                    rows.push(Arc::clone(cached_row));
                                }
                            }
                        }
                        if overlap_end < new_end {
                            let gap = new_end - overlap_end;
                            rows.extend(
                                source
                                    .fetch_window(overlap_end, gap, window.2, window.3)
                                    .into_iter()
                                    .map(Arc::new),
                            );
                        }
                        self.window_cache = Some(Arc::new(rows));
                        self.window_key = Some(window);
                        return self.window_cache.as_ref();
                    }
                }
            }
        }

        if !self.data_is_observable
            || self.window_cache.is_none()
            || self.window_revision != cache_revision
            || self.window_key != Some(window)
        {
            let source = self.data_source.as_ref()?;
            let data = source.fetch_window(window.0, window.1, window.2, window.3);
            self.window_cache = Some(Arc::new(data.into_iter().map(Arc::new).collect()));
            self.window_revision = cache_revision;
            self.window_key = Some(window);
        }
        self.window_cache.as_ref()
    }

    fn emit_visible_window(&self) {
        self.visible_window_changed.emit(self.visible_window());
    }

    /// The first row/column the fetch window starts at, i.e. the scroll position pulled back by
    /// the overscan.
    ///
    /// # Why this exists (D09-VIEW-01)
    ///
    /// `visible_window()` fetches from `scroll - overscan`, so the data it returns begins *before*
    /// the viewport. The draw pass used to treat the fetched data's local `(0, 0)` as the visible
    /// top-left, which painted the leading overscan rows/columns as if they were the first visible
    /// ones — non-zero scroll showed content shifted by the overscan amount. `cell_at` and `draw`
    /// both convert a window-local index through this origin so the cell that is painted is the
    /// cell that is hit, at the correct global position.
    fn window_origin(&self) -> (usize, usize) {
        let (row_start, _, col_start, _) = self.visible_window();
        (row_start, col_start)
    }

    /// The box a data cell occupies, in the table's own coordinates.
    ///
    /// # Why the cell position is a function rather than an accumulator
    ///
    /// The draw loop used to advance `x += column_width` and `y += row_height` as it went,
    /// which makes every cell's position depend on how many cells were drawn *before* it in
    /// that particular loop. Two consequences follow, and both are real: a later pass that
    /// wanted one cell's box (a hit test, a tooltip, a selection rectangle) had to replay the
    /// whole accumulator, and any change to the leading inset moved every cell without the
    /// later passes knowing. Expressing the position as `origin + index * stride` makes a
    /// cell's box answerable for any index, independently of what was drawn.
    ///
    /// `row` and `column` are **viewport-relative** indices: `(0, 0)` is the first cell inside the
    /// visible area, with the leading overscan rows/columns already skipped (D09-VIEW-01). That is
    /// the semantics `cell_at` answers with, so the box a press resolves to is the box that was
    /// painted. The ordinal is mapped through the column layout, so per-column widths and hidden
    /// columns shift the painted positions without changing this contract (V-013).
    ///
    /// The two insets are `2` rather than `0` so the first row and column do not sit on the
    /// table's own border, and they are shared with the cell's *size* below.
    fn cell_rect(&self, origin: Rect, row: usize, column: usize) -> Rect {
        const INSET: i32 = 2;
        let absolute = self
            .absolute_column(column)
            .unwrap_or_else(|| self.scroll_column.saturating_add(column));
        // The offset is measured from the first painted column rather than from column `0`, so a
        // horizontal scroll moves the cells exactly as the old `column * stride` did.
        let x = self.column_offset(absolute) - self.column_offset(self.scroll_column);
        let width = self.column_width_at(absolute);
        // The cell is 2 px smaller than its stride so the grid lines are the surface showing
        // through rather than strokes that would double up where cells meet. The shrink is
        // applied **before** mirroring: in RTL the trailing edge is the left one, so the gap must
        // land on the left exactly as `column_at` tests it (T3-16).
        let cell_width = width.saturating_sub(2);
        let stride_y = self.row_height as i32;
        // Mirror the whole column band inside the inset content area (U2-06). `origin` is the
        // control's geometry, so the reflection band is what `column_at` measures against.
        let band = Rect::new(
            origin.x + INSET,
            origin.y,
            origin.width.saturating_sub(INSET as u32 * 2),
            origin.height,
        );
        let x = if crate::widget::view_widgets::rtl_columns::is_rtl() {
            crate::widget::view_widgets::rtl_columns::mirror_span(
                origin.x + INSET + x,
                cell_width,
                band,
            )
        } else {
            origin.x + INSET + x
        };
        Rect::new(
            x,
            origin.y + INSET + self.header_offset() + row as i32 * stride_y,
            cell_width,
            self.row_height.saturating_sub(2),
        )
    }

    /// The vertical room the column-title row takes, when it is enabled.
    fn header_offset(&self) -> i32 {
        if self.show_header {
            self.row_height as i32
        } else {
            0
        }
    }

    /// The selected absolute cell (the current cell of the selection set).
    pub fn selected_cell(&self) -> Option<(usize, usize)> {
        self.selection.current_row().and_then(|index| self.decode_cell_index(index))
    }

    /// The number of columns a linearised cell index is divided by.
    fn cell_columns(&self) -> usize {
        self.column_count().max(1)
    }

    /// Maps an absolute cell to the linear index the selection model stores.
    fn encode_cell_index(&self, row: usize, column: usize) -> Option<usize> {
        if row >= self.row_count() || column >= self.column_count() {
            return None;
        }
        Some(row.saturating_mul(self.cell_columns()).saturating_add(column))
    }

    /// Maps a linearised selection index back to `(row, column)`.
    ///
    /// The division uses the column count the index was **encoded with**, not the count the source
    /// happens to have now: when a source changes its columns, the same number means a different
    /// cell under the new width, so decoding with the live count renamed the user's selection.
    fn decode_cell_index(&self, index: usize) -> Option<(usize, usize)> {
        let columns = self.column_count();
        if columns == 0 {
            return None;
        }
        let encoded_columns =
            if self.selection_columns == 0 { columns } else { self.selection_columns };
        let row = index / encoded_columns;
        let column = index % encoded_columns;
        (row < self.row_count() && column < columns).then_some((row, column))
    }

    /// Re-encodes the linearised selection against the current row/column counts.
    ///
    /// Returns whether the stored set changed. An index that names a row the source no longer has
    /// is dropped (the cell is gone); a column past the new end is clamped to the last column, so
    /// the cell the user picked is preserved when only the column count shrank. The single-value
    /// and set-value signals are emitted when the set really changed, so a host mirroring the
    /// selection hears the repair rather than keeping an index it can no longer interpret.
    fn reseat_selection(&mut self) -> bool {
        let before = self.selection.rows();
        let old_columns = self.selection_columns;
        let rows = self.row_count();
        let columns = self.column_count();
        let remap = |index: usize| -> Option<usize> {
            if old_columns == 0 || rows == 0 || columns == 0 {
                return None;
            }
            let row = index / old_columns;
            if row >= rows {
                return None;
            }
            let column = (index % old_columns).min(columns - 1);
            Some(row.saturating_mul(columns).saturating_add(column))
        };
        let current = self.selection.current_row().and_then(remap);
        let mut reseated: Vec<usize> = Vec::with_capacity(before.len());
        for index in &before {
            if let Some(index) = remap(*index) {
                if !reseated.contains(&index) {
                    reseated.push(index);
                }
            }
        }
        self.selection_columns = columns;
        // `before` is in ascending order and the remap is monotonic in the index, so `reseated` is
        // too: an equal vector means the set survived the change, and no signal is due.
        if reseated == before {
            return false;
        }
        // The current cell is restored last so a mode that records the current row from the last
        // `select_row` (Multi/Extended) keeps pointing at the cell the user was on.
        if let Some(current) = current {
            if let Some(position) = reseated.iter().position(|index| *index == current) {
                let current = reseated.remove(position);
                reseated.push(current);
            }
        }
        self.selection.clear();
        match self.selection.mode() {
            crate::widget::view_widgets::list_view::SelectionMode::Single => {
                if let Some(index) = reseated.first().copied() {
                    self.selection.select_row(index);
                }
            }
            crate::widget::view_widgets::list_view::SelectionMode::Multi => {
                for index in &reseated {
                    self.selection.select_row(*index);
                }
            }
            crate::widget::view_widgets::list_view::SelectionMode::Extended => {
                for index in &reseated {
                    self.selection.select_with_modifiers(*index, crate::shortcut::Modifiers::CTRL);
                }
            }
            crate::widget::view_widgets::list_view::SelectionMode::None => {}
        }
        self.emit_selection_changed();
        self.base.request_redraw();
        true
    }

    /// Applies a pending source change before a paint, an input or a query reads row state.
    ///
    /// The change signal only raises a flag (it cannot borrow the control mutably); this is the
    /// one place that clamps the scroll to the surviving rows, clears a hover that named a removed
    /// row and re-encodes the selection, then reports the new visible window.
    fn drain_data_updates(&mut self) {
        if !self.data_dirty.swap(false, Ordering::Relaxed) {
            return;
        }
        self.window_cache = None;
        self.sorted_window_cache = None;
        let rows = self.row_count();
        let columns = self.column_count();
        let mut bounds_changed = false;
        if self.scroll_row > rows.saturating_sub(1) {
            self.scroll_row = rows.saturating_sub(1);
            bounds_changed = true;
        }
        if self.scroll_column > columns.saturating_sub(1) {
            self.scroll_column = columns.saturating_sub(1);
            bounds_changed = true;
        }
        let hover_cleared = self.hovered_row.is_some_and(|row| row >= rows);
        if hover_cleared {
            self.hovered_row = None;
        }
        self.reseat_selection();
        if bounds_changed {
            self.emit_visible_window();
        }
        if bounds_changed || hover_cleared {
            self.base.request_redraw();
        }
    }

    /// The selection mode (V-06).
    pub fn selection_mode(&self) -> crate::widget::view_widgets::list_view::SelectionMode {
        self.selection.mode()
    }

    /// Sets how many cells one gesture may select (V-06).
    pub fn set_selection_mode(
        &mut self,
        mode: crate::widget::view_widgets::list_view::SelectionMode,
    ) {
        if self.selection.mode() == mode {
            return;
        }
        let before_current = self.selection.current_row();
        let before_set = self.selection.rows();
        self.selection.set_mode(mode);
        if mode == crate::widget::view_widgets::list_view::SelectionMode::None {
            // `None` accepts no selection, so the surviving indices are not a selection any more.
            self.selection.clear();
        }
        if self.selection.current_row() != before_current || self.selection.rows() != before_set {
            // `set_mode` trims a Multi set when switching to Single; the host has to hear that the
            // surviving cell changed, not discover it by polling.
            self.emit_selection_changed();
        }
        self.base.request_redraw();
    }

    /// Every selected cell, in row-major order.
    pub fn selected_cells(&self) -> Vec<(usize, usize)> {
        self.selection
            .rows()
            .into_iter()
            .filter_map(|index| self.decode_cell_index(index))
            .collect()
    }

    /// Selects an absolute cell. Out-of-range cells are ignored.
    pub fn select_cell(&mut self, row: usize, column: usize) -> bool {
        let Some(index) = self.encode_cell_index(row, column) else {
            return false;
        };
        if self.selection.current_row() == Some(index) && self.selection.rows() == vec![index] {
            self.selection_columns = self.column_count();
            return true;
        }
        self.selection.select_row(index);
        self.selection_columns = self.column_count();
        self.emit_selection_changed();
        self.base.request_redraw();
        true
    }

    /// Selects a cell honouring keyboard modifiers (V-06).
    ///
    /// `Shift` extends the contiguous cell range from the anchor, `Ctrl`/`Meta` toggles the cell,
    /// and an unmodified call replaces the selection — the same model `ListView` uses.
    pub fn select_cell_with_modifiers(
        &mut self,
        row: usize,
        column: usize,
        modifiers: crate::shortcut::Modifiers,
    ) -> bool {
        let Some(index) = self.encode_cell_index(row, column) else {
            return false;
        };
        self.selection.select_with_modifiers(index, modifiers);
        self.selection_columns = self.column_count();
        self.emit_selection_changed();
        self.base.request_redraw();
        true
    }

    /// Emits the single-value and set-value selection signals.
    fn emit_selection_changed(&mut self) {
        let current = self.selected_cell();
        self.selection_changed.emit(current);
        self.selection_set_changed.emit(self.selected_cells());
    }

    /// Moves the current cell by `(row_delta, column_delta)` and keeps it on screen (V-07).
    ///
    /// The arrow keys used to scroll the viewport without selecting anything, so a keyboard user
    /// could not move or create a selection at all. Selection goes through the same model the
    /// pointer uses, so `Shift` extends in extended mode for free.
    fn move_current_cell(&mut self, row_delta: isize, column_delta: isize, modifiers: u32) {
        let rows = self.row_count();
        let columns = self.column_count();
        if rows == 0 || columns == 0 {
            return;
        }
        let (row, column) = self.selected_cell().unwrap_or((0, 0));
        let next_row = (row as isize + row_delta).clamp(0, rows as isize - 1) as usize;
        let next_column = (column as isize + column_delta).clamp(0, columns as isize - 1) as usize;
        let modifiers = crate::shortcut::Modifiers::from_event_bits(modifiers);
        self.select_cell_with_modifiers(next_row, next_column, modifiers);
        self.ensure_cell_visible(next_row, next_column);
    }

    /// Scrolls the smallest amount that puts `(row, column)` inside the viewport.
    fn ensure_cell_visible(&mut self, row: usize, column: usize) {
        let (visible_rows, visible_columns) = self.visible_grid(self.geometry());
        let visible_rows = visible_rows.max(1);
        if row < self.scroll_row {
            self.set_scroll_row(row);
        } else if row >= self.scroll_row.saturating_add(visible_rows) {
            self.set_scroll_row(row.saturating_add(1).saturating_sub(visible_rows));
        }
        let visible_columns = visible_columns.max(1);
        if column < self.scroll_column {
            self.set_scroll_column(column);
        } else if column >= self.scroll_column.saturating_add(visible_columns) {
            self.set_scroll_column(column.saturating_add(1).saturating_sub(visible_columns));
        }
    }

    /// Clears the selection.
    pub fn clear_selection(&mut self) {
        if self.selection.current_row().is_some() || !self.selection.rows().is_empty() {
            self.selection.clear();
            self.selection_columns = self.column_count();
            self.selection_changed.emit(None);
            self.selection_set_changed.emit(Vec::new());
            self.base.request_redraw();
        }
    }

    /// Whether the column-title row is painted.
    pub fn show_header(&self) -> bool {
        self.show_header
    }

    /// Enables or disables the column-title row.
    pub fn set_show_header(&mut self, show: bool) {
        if self.show_header == show {
            return;
        }
        self.show_header = show;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The header labels supplied by the host.
    pub fn header_labels(&self) -> &[String] {
        &self.header_labels
    }

    /// Replaces the header labels, taking precedence over [`IncrementalTableDataSource::column_name`].
    pub fn set_header_labels(&mut self, labels: &[&str]) {
        self.header_labels = labels.iter().map(|label| (*label).to_string()).collect();
        self.header_text_revision = self.header_text_revision.wrapping_add(1);
        self.base.request_redraw();
    }

    /// The alignment `column` paints its cells with (V-001).
    pub fn column_alignment(&self, column: usize) -> HorizontalAlignment {
        self.column_alignments
            .get(&column)
            .copied()
            .unwrap_or_else(crate::widget::view_widgets::rtl_columns::leading_alignment)
    }

    /// Sets one column's cell alignment, returning whether the column exists (V-001).
    pub fn set_column_alignment(&mut self, column: usize, alignment: HorizontalAlignment) -> bool {
        if column >= self.column_count() {
            return false;
        }
        if alignment == HorizontalAlignment::Left {
            self.column_alignments.remove(&column);
        } else {
            self.column_alignments.insert(column, alignment);
        }
        self.base.request_redraw();
        true
    }

    /// Resolves the header label for `column` from the cache, building it on a miss (VP-007).
    fn header_name(&self, column: usize, source: &dyn IncrementalTableDataSource) -> Rc<String> {
        let revision = source.revision();
        let generation = self.data_generation.load(Ordering::Relaxed);
        let column_count = self.column_count();
        let mut cache = self.header_text_cache.borrow_mut();
        if cache.revision != revision
            || cache.generation != generation
            || cache.labels_revision != self.header_text_revision
            || cache.column_count != column_count
        {
            cache.entries.clear();
            cache.revision = revision;
            cache.generation = generation;
            cache.labels_revision = self.header_text_revision;
            cache.column_count = column_count;
        }
        if let Some(label) = cache.entries.get(&column) {
            return Rc::clone(label);
        }
        let name = self
            .header_labels
            .get(column)
            .map(String::as_str)
            .filter(|label| !label.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| source.column_name(column))
            .unwrap_or_else(|| std::format!("Column {}", column + 1));
        let name = Rc::new(crate::widget::text_utils::elide_end(&name, 20).into_owned());
        cache.entries.insert(column, Rc::clone(&name));
        name
    }

    /// How one wheel notch scrolls.
    pub fn wheel_scroll_mode(&self) -> WheelScrollMode {
        self.wheel_scroll_mode
    }

    /// Sets how one wheel notch scrolls.
    pub fn set_wheel_scroll_mode(&mut self, mode: WheelScrollMode) {
        self.wheel_scroll_mode = mode;
    }

    /// Whether the vertical scroll-bar thumb is painted.
    ///
    /// Since UX-04 the thumb is *derived*: it appears whenever rows are off-screen, whether or
    /// not a caller asked. This flag is a **force-on** override, never a way to hide an overflow.
    pub fn show_scrollbar(&self) -> bool {
        self.show_scrollbar
    }

    /// Installs a per-cell painter, replacing any previous delegate.
    pub fn set_delegate(&mut self, delegate: Arc<dyn VirtualCellDelegate>) {
        self.delegate = Some(delegate);
        self.base.request_redraw();
    }

    /// Removes the per-cell painter.
    pub fn clear_delegate(&mut self) {
        if self.delegate.take().is_some() {
            self.base.request_redraw();
        }
    }

    /// Whether a delegate is installed.
    pub fn has_delegate(&self) -> bool {
        self.delegate.is_some()
    }

    /// Forces the vertical scroll-bar thumb on even when the rows fit.
    ///
    /// An overflowing table paints the thumb regardless; this only makes it permanent.
    pub fn set_show_scrollbar(&mut self, show: bool) {
        if self.show_scrollbar == show {
            return;
        }
        self.show_scrollbar = show;
        self.base.request_redraw();
    }
    /// How many rows and columns fit inside `rect`.
    ///
    /// The counts the draw loop uses, and the ones a hit test would need, so neither derives
    /// its own "how many fit" rule. At least one of each: a table clipped to a sliver still
    /// paints its first cell, which is what makes a degenerate rectangle visibly degenerate
    /// rather than blank.
    fn visible_grid(&self, rect: Rect) -> (usize, usize) {
        const INSET: i32 = 2;
        let usable_w = (rect.width as i32 - INSET * 2).max(0) as u32;
        let usable_h = (rect.height as i32 - INSET * 2 - self.header_offset()).max(0) as u32;
        // Width-aware, so per-column widths and hidden columns decide how many columns fit
        // (V-013). With the uniform default this is the historical division.
        let columns = self.visible_column_capacity(usable_w);
        let rows = (usable_h / self.row_height.max(1)).max(1) as usize;
        (rows, columns)
    }

    /// The **global** cell whose box contains `pos`, if any.
    ///
    /// With a fixed row height and column width a cell is `origin + index * stride`, so the cell
    /// under a point is a division rather than a walk. The previous version nested a loop over the
    /// visible grid and tested every cell's rectangle, which ran on every `MouseMove` and cost
    /// `O(visible cells)`. The 2 px inter-cell gap is rejected explicitly, matching the box
    /// `Self::cell_rect` paints.
    pub fn cell_at(&self, pos: crate::core::Point) -> Option<(usize, usize)> {
        Some((self.row_at(pos.y)?, self.column_at(pos.x)?))
    }

    /// The **global** row at screen y `y`, if `y` is inside a cell rather than a gap.
    pub fn row_at(&self, y: i32) -> Option<usize> {
        const INSET: i32 = 2;
        let rect = self.geometry();
        let top = rect.y + INSET + self.header_offset();
        if y < top {
            return None;
        }
        let (rows, _) = self.visible_grid(rect);
        let stride = self.row_height.max(1) as i32;
        let offset = y - top;
        let local = offset / stride;
        if local < 0 || local as usize >= rows {
            return None;
        }
        // The painted cell is 2 px shorter than its stride; the remainder is the gap.
        if offset % stride >= stride.saturating_sub(2) {
            return None;
        }
        let row = self.scroll_row + local as usize;
        // A point inside the rectangle but below the last row is empty space: without the bound a
        // 3-row source in a 300 px control answered row 9 and painted a hover stripe over nothing.
        if self.data_source.as_ref().is_some_and(|source| row >= source.row_count()) {
            return None;
        }
        Some(row)
    }

    /// The **global** column in the header strip under `pos`, when the header is shown.
    fn header_column_at(&self, pos: Point) -> Option<usize> {
        if !self.show_header {
            return None;
        }
        let rect = self.geometry();
        const INSET: i32 = 2;
        let top = rect.y + INSET;
        let bottom = top + self.row_height.saturating_sub(2) as i32;
        if pos.y < top || pos.y >= bottom {
            return None;
        }
        self.column_at(pos.x)
    }

    /// The **global** column at screen x `x`, if `x` is inside a cell rather than a gap.
    pub fn column_at(&self, x: i32) -> Option<usize> {
        const INSET: i32 = 2;
        let rect = self.geometry();
        let left = rect.x + INSET;
        if x < left {
            return None;
        }
        // Mirror the probe inside the inset content area; the walk below stays the LTR one, so
        // the painted column and the hit-tested column keep one derivation (U2-06).
        let offset = if crate::widget::view_widgets::rtl_columns::is_rtl() {
            let content_width = rect.width as i32 - INSET * 2;
            crate::widget::view_widgets::rtl_columns::mirror_offset(x - left, content_width)
        } else {
            x - left
        };
        if offset < 0 {
            return None;
        }
        let scroll_offset = self.column_offset(self.scroll_column);
        self.with_column_layout(|layout| {
            if layout.widths.is_empty() {
                // No source: the uniform stride is the only geometry, so the hit test is the
                // division that inverts `Self::cell_rect`, rejecting the gap the same way.
                let stride = self.column_width.max(1) as i32;
                let local = (offset / stride) as usize;
                return (offset % stride < stride - 2)
                    .then_some(self.scroll_column.saturating_add(local));
            }
            let start = self.scroll_column.min(layout.widths.len());
            for column in start..layout.widths.len() {
                let width = layout.widths[column];
                // A hidden column occupies no slot, so the pointer can never land in it.
                if width == 0 {
                    continue;
                }
                let start_x = layout.offsets[column] - scroll_offset;
                if offset < start_x {
                    return None;
                }
                if offset < start_x + width as i32 {
                    // The painted cell is 2 px narrower than its slot; the rest is the gap.
                    return (offset - start_x < width as i32 - 2).then_some(column);
                }
            }
            None
        })
    }
}

impl Widget for VirtualTable {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn scrollable_rows(&self) -> Option<&dyn crate::widget::container_widgets::ScrollableRows> {
        Some(self)
    }
    fn scrollable_rows_mut(
        &mut self,
    ) -> Option<&mut dyn crate::widget::container_widgets::ScrollableRows> {
        Some(self)
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(400, 300)
    }

    /// Selects the cell under the pointer, then asks the host for a menu about it.
    ///
    /// The old handler emitted the request on a secondary press **and** on the synthesized
    /// context-menu event, so one right-click produced two requests, neither of which moved the
    /// selection (feedback C-9). One hook, selection first, one request. A point outside a cell
    /// answers `false`.
    fn context_menu_event(&mut self, pos: Point) -> bool {
        let Some((row, column)) = self.cell_at(pos) else {
            return false;
        };
        if !self.select_cell(row, column) {
            return false;
        }
        self.context_menu_requested.emit(pos);
        true
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
            "visible_window_changed" => Some(EventSignalRef::mapped(
                "visible_window_changed",
                &self.visible_window_changed,
                // The declared shape is `Tuple4` of `UInt`, so the payload arrives as the
                // (row_start, row_len, col_start, col_len) tuple rather than its `Debug` spelling.
                |v| {
                    CapabilityValue::Tuple(crate::compat::Vec::from([
                        CapabilityValue::UInt(v.0 as u64),
                        CapabilityValue::UInt(v.1 as u64),
                        CapabilityValue::UInt(v.2 as u64),
                        CapabilityValue::UInt(v.3 as u64),
                    ]))
                },
            )),
            "selection_changed" => {
                Some(EventSignalRef::mapped("selection_changed", &self.selection_changed, |v| {
                    match v {
                        Some(value) => crate::widget::capability::CapabilityValue::Tuple(
                            crate::compat::Vec::from([
                                crate::widget::capability::CapabilityValue::UInt(value.0 as u64),
                                crate::widget::capability::CapabilityValue::UInt(value.1 as u64),
                            ]),
                        ),
                        None => crate::widget::capability::CapabilityValue::Null,
                    }
                }))
            }
            "selection_set_changed" => Some(EventSignalRef::mapped(
                "selection_set_changed",
                &self.selection_set_changed,
                // A list of `(row, column)` cells: each pair travels as its own `Tuple`, so a
                // subscriber can read the cell boundaries instead of re-parsing a `Debug` string.
                |v| {
                    crate::widget::capability::CapabilityValue::Tuple(
                        v.iter()
                            .map(|cell| {
                                crate::widget::capability::CapabilityValue::Tuple(
                                    crate::compat::Vec::from([
                                        crate::widget::capability::CapabilityValue::UInt(
                                            cell.0 as u64,
                                        ),
                                        crate::widget::capability::CapabilityValue::UInt(
                                            cell.1 as u64,
                                        ),
                                    ]),
                                )
                            })
                            .collect(),
                    )
                },
            )),
            "context_menu_requested" => Some(EventSignalRef::mapped(
                "context_menu_requested",
                &self.context_menu_requested,
                // The `Point` has no property kind of its own; the published payload is its
                // `"x,y"` spelling, not a second numeric carrier.
                |v| CapabilityValue::String(format!("{v:?}")),
            )),
            "sort_changed" => {
                Some(EventSignalRef::mapped("sort_changed", &self.sort_changed, |v| {
                    // The declared shape is `Mixed`, so the column and the direction arrive as a
                    // structured pair rather than its `Debug` spelling.
                    CapabilityValue::Tuple(crate::compat::Vec::from([
                        CapabilityValue::UInt(v.0 as u64),
                        CapabilityValue::Bool(v.1),
                    ]))
                }))
            }
            _ => None,
        }
    }
}
