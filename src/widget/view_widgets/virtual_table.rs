// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! VirtualTable widget backed by incremental table data source.

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

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
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

use super::data_grid::SortSpec;
use super::data_source::IncrementalTableDataSource;

/// Paint hook for a virtual table cell.
///
/// The table itself only knows how to draw text. A host that wants a checkbox column, a
/// progress bar or rich text installs a delegate, which owns a cell's ink when `paint`
/// returns `true`; returning `false` keeps the table's own text rendering.
pub trait VirtualCellDelegate: Send + Sync {
    /// Paints `cell`, returning whether the delegate took over the cell.
    fn paint(
        &self,
        context: &mut RenderContext,
        cell: Rect,
        row: usize,
        column: usize,
        text: Option<&str>,
    ) -> bool {
        let _ = (context, cell, row, column, text);
        false
    }
}

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
    sorted_window_cache: Option<(u64, (usize, usize, usize, usize), Arc<Vec<CachedRow>>)>,
    /// Whether odd rows receive the alternating wash (V-013). Off by default.
    alternating_rows: bool,
    /// Emitted with `(column, descending)` when a header click changes the sort (V-013).
    pub sort_changed: Signal1<(usize, bool)>,
    /// Emitted when the context menu is requested at a point (V-013).
    pub context_menu_requested: Signal1<Point>,
}

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
            hovered_row: None,
            visible_window_changed: Signal1::new(),
            show_header: false,
            show_scrollbar: false,
            selection_changed: Signal1::new(),
            selection_set_changed: Signal1::new(),
            selection: crate::widget::view_widgets::list_view::SelectionModel::new(),
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
        self.data_source = Some(source);
        self.scroll_row = 0;
        self.scroll_column = 0;
        self.window_cache = None;
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
    pub fn scroll_row(&self) -> usize {
        self.scroll_row
    }

    /// Returns scroll column.
    pub fn scroll_column(&self) -> usize {
        self.scroll_column
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
                per_row.push(super::data_grid::normalise_sort_key(&raw));
            }
            keys.push(per_row);
        }

        let mut order: Vec<usize> = (0..rows).collect();
        order.sort_by(|&left, &right| {
            for (index, spec) in key.specs.iter().enumerate() {
                let ordering =
                    super::data_grid::compare_sort_keys(&keys[index][left], &keys[index][right]);
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

        let row_start = self.scroll_row.saturating_sub(self.overscan_rows);
        let col_start = self.scroll_column.saturating_sub(self.overscan_columns);
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
        let stride_y = self.row_height as i32;
        Rect::new(
            origin.x + INSET + x,
            origin.y + INSET + self.header_offset() + row as i32 * stride_y,
            // The cell is 2 px smaller than its stride on each axis, so the grid lines are the
            // surface showing through rather than strokes that would double up where cells meet.
            width.saturating_sub(2),
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
    fn decode_cell_index(&self, index: usize) -> Option<(usize, usize)> {
        let columns = self.column_count();
        if columns == 0 {
            return None;
        }
        let row = index / columns;
        let column = index % columns;
        (row < self.row_count()).then_some((row, column))
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
        self.selection.set_mode(mode);
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
            return true;
        }
        self.selection.select_row(index);
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
        self.base.request_redraw();
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

    /// Enables or disables the vertical scroll-bar thumb.
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
        let offset = x - left;
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
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(400, 300)
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
            _ => None,
        }
    }
}

/// `VirtualTable`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_view.in.rs` / `access_write_view.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `VirtualTable` reports
/// `WidgetKind::Table`, shared with `TableWidget` and `DataGrid`; dispatching on
/// the concrete type here is what keeps the three contracts separate.
impl WidgetProperties for VirtualTable {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "has_data_source" => Ok(CapabilityValue::Bool(self.has_data_source())),
            "row_count" => Ok(CapabilityValue::UInt(self.row_count() as u64)),
            "column_count" => Ok(CapabilityValue::UInt(self.column_count() as u64)),
            "scroll_row" => Ok(CapabilityValue::UInt(self.scroll_row() as u64)),
            "scroll_column" => Ok(CapabilityValue::UInt(self.scroll_column() as u64)),
            "row_height" => Ok(CapabilityValue::UInt(self.row_height() as u64)),
            "column_width" => Ok(CapabilityValue::UInt(self.column_width() as u64)),
            "overscan_rows" => Ok(CapabilityValue::UInt(self.overscan_rows as u64)),
            "overscan_columns" => Ok(CapabilityValue::UInt(self.overscan_columns as u64)),
            "wheel_scroll_mode" => Ok(CapabilityValue::String(
                wheel_scroll_mode_to_str(self.wheel_scroll_mode()).to_string(),
            )),
            "header_visible" => Ok(CapabilityValue::Bool(self.show_header())),
            "selection_mode" => Ok(CapabilityValue::String(
                selection_mode_to_str(self.selection_mode()).to_string(),
            )),
            // Row-major `(row, column)` pairs of the live selection, flattened
            // into one number list — the same carrier the timeline axis `range`
            // pair uses. Empty when nothing is selected.
            "selected_cells" => Ok(CapabilityValue::Tuple(
                self.selected_cells()
                    .iter()
                    .map(|(row, column)| {
                        CapabilityValue::Tuple(crate::compat::Vec::from([
                            CapabilityValue::UInt(*row as u64),
                            CapabilityValue::UInt(*column as u64),
                        ]))
                    })
                    .collect(),
            )),
            "visible_window" => {
                let (row_start, row_len, column_start, column_len) = self.visible_window();
                Ok(CapabilityValue::String(format!(
                    "rows={row_start}..{},{},columns={column_start}..{}",
                    row_start.saturating_add(row_len),
                    row_len,
                    column_start.saturating_add(column_len)
                )))
            }
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "scroll_row" => {
                self.set_scroll_row(expect_usize(value)?);
                Ok(())
            }
            "scroll_column" => {
                self.set_scroll_column(expect_usize(value)?);
                Ok(())
            }
            "row_height" => {
                self.set_row_height(expect_u32(value)?);
                Ok(())
            }
            "column_width" => {
                self.set_column_width(expect_u32(value)?);
                Ok(())
            }
            "overscan_rows" => {
                self.set_overscan_rows(expect_usize(value)?);
                Ok(())
            }
            "overscan_columns" => {
                self.set_overscan_columns(expect_usize(value)?);
                Ok(())
            }
            "wheel_scroll_mode" => {
                self.set_wheel_scroll_mode(expect_wheel_scroll_mode(value)?);
                Ok(())
            }
            "header_visible" => {
                self.set_show_header(expect_bool(value)?);
                Ok(())
            }
            "selection_mode" => {
                self.set_selection_mode(expect_selection_mode(value)?);
                Ok(())
            }
            // Derived counts and the visible-window summary are computed from the
            // data source and layout, so they are refused as read-only rather
            // than reported as names this control does not know. `selected_cells`
            // is the live selection: it is changed by selecting, not by writing.
            "has_data_source" | "row_count" | "column_count" | "selected_cells"
            | "visible_window" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "has_data_source",
            "row_count",
            "column_count",
            "scroll_row",
            "scroll_column",
            "row_height",
            "column_width",
            "overscan_rows",
            "overscan_columns",
            "wheel_scroll_mode",
            "header_visible",
            // `get` answers this name; leaving it out of the list made the schema
            // declare a readable property no contract published.
            "visible_window",
            "selection_mode",
            "selected_cells",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `virtual_table` publishes.
    ///
    /// `clear_data_source` and `clear_selection` are payload-free and execute
    /// here; `fetch_visible_window` is a *query*, so it belongs to the read path
    /// (the `visible_window` property) rather than an action. The remaining
    /// names need a payload and are answered through the property route.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "clear_data_source" => {
                self.clear_data_source();
                Ok(())
            }
            "clear_selection" => {
                self.clear_selection();
                Ok(())
            }
            "fetch_visible_window" => {
                // This is a query with no result channel. Warming the cache is the only
                // useful effect here; the owned fetch copied the whole window and then
                // discarded it.
                let _ = self.visible_window_cells();
                Ok(())
            }
            "set_data_source"
            | "set_scroll_row"
            | "set_scroll_column"
            | "set_row_height"
            | "set_column_width"
            | "set_overscan_rows"
            | "set_overscan_columns" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for VirtualTable {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            // The context menu is a request signal, exactly like the rest of the table family
            // (V-013). Both the right-button press and the platform's own menu event are
            // reported, so a host does not have to guess which one its backend delivers.
            Event::MousePress { pos, button: 3, .. } => self.context_menu_requested.emit(*pos),
            Event::ContextMenu { pos } => self.context_menu_requested.emit(*pos),
            Event::MousePress { pos, button: 1, modifiers } => {
                // A press in the column-title strip cycles that column's sort; only a body press
                // selects a cell (V-013).
                if let Some(column) = self.header_column_at(*pos) {
                    let _ = self.toggle_sort_column(column);
                    return;
                }
                if let Some((row, column)) = self.cell_at(*pos) {
                    let modifiers = crate::shortcut::Modifiers::from_event_bits(*modifiers);
                    self.select_cell_with_modifiers(row, column, modifiers);
                } else {
                    self.clear_selection();
                }
            }
            Event::MouseMove { pos, .. } => {
                // Hover is the absolute row under the pointer, from the same `cell_at`
                // derivation `cell_rect` paints through, so a highlight cannot land on a
                // row a click would not reach. `MouseMove` fires far more often than the
                // highlight changes, so the redraw is requested only on an actual change.
                let hovered = self.cell_at(*pos).map(|(row, _)| row);
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
                // One shared normalisation for wheel units and modifiers, the same one the rest of
                // the table family uses. The old arm read `delta.y` raw and ignored modifiers, so a
                // model-level delta scrolled hundreds of rows and Shift could not pan sideways.
                let scroll = WheelScroll::from_event(*delta, *modifiers, 1);
                let step = match self.wheel_scroll_mode {
                    WheelScrollMode::PerRow => 1,
                    WheelScrollMode::PerPage => self.visible_grid(self.geometry()).0.max(1),
                };
                let lines = scroll.lines_y.unsigned_abs() as usize * step;
                if scroll.lines_y < 0 {
                    self.set_scroll_row(self.scroll_row.saturating_add(lines));
                } else if scroll.lines_y > 0 {
                    self.set_scroll_row(self.scroll_row.saturating_sub(lines));
                }
                let columns = scroll.columns_x as isize * step as isize;
                if columns != 0 {
                    let next = self.scroll_column.saturating_add_signed(columns);
                    self.set_scroll_column(next);
                }
                let mods = crate::shortcut::Modifiers::from_event_bits(*modifiers);
                if mods.contains(crate::shortcut::Modifiers::SHIFT) {
                    let next = self
                        .scroll_column
                        .saturating_add_signed(-(scroll.lines_y as isize * step as isize));
                    self.set_scroll_column(next);
                }
            }
            Event::KeyPress { key, modifiers } => {
                let (page_rows, _) = self.visible_grid(self.geometry());
                let page = page_rows.max(1) as isize;
                let last = self.row_count().saturating_sub(1) as isize;
                match *key {
                    37 => self.move_current_cell(0, -1, *modifiers),
                    39 => self.move_current_cell(0, 1, *modifiers),
                    38 => self.move_current_cell(-1, 0, *modifiers),
                    40 => self.move_current_cell(1, 0, *modifiers),
                    crate::event::key_codes::PAGE_UP => {
                        self.move_current_cell(-page, 0, *modifiers)
                    }
                    crate::event::key_codes::PAGE_DOWN => {
                        self.move_current_cell(page, 0, *modifiers)
                    }
                    crate::event::key_codes::HOME => {
                        let current = self.selected_cell().map_or(0, |(row, _)| row) as isize;
                        self.move_current_cell(-current, 0, *modifiers);
                    }
                    crate::event::key_codes::END => {
                        let current = self.selected_cell().map_or(0, |(row, _)| row) as isize;
                        self.move_current_cell(last - current, 0, *modifiers);
                    }
                    _ => { /* Other keys are not relevant */ }
                }
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}

impl Draw for VirtualTable {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then a literal. The theme step is what makes an appearance
        // switch visible; the surface, the border, the cell fills and the cell text colour
        // used to be hardcoded literals, so light and dark rendered identically.
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
        let theme = crate::style::resolved_theme_style_shared("virtual_table");
        let mut surface = background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let ink = text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        // `virtual_table` is absent from `WidgetRole::for_kind_name`'s table, so it classifies
        // as `Surface` and resolves to `theme.colors.background` — the window's own fill. A
        // panel painted in that colour would be byte-identical to the frame behind it, so a
        // resolved surface equal to the window fill is re-derived a visible step away from it,
        // the same distinction `Colors::input_background` draws for a field.
        // One palette read for the surface, grid and accent roles (three theme locks before).
        let palette = crate::style::palette_shared();
        let window_fill = palette.as_ref().map(|colors| colors.background).unwrap_or(Color::WHITE);
        if surface == window_fill {
            surface = window_fill.blend(&ink, 0.08);
        }
        let border = border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != surface)
            .unwrap_or_else(|| surface.blend(&ink, 0.20));
        // The cell grid is the theme's `outline_variant` — the **weak** separator role — so a
        // table's grid lines are visibly weaker than its `outline`-strength focus ring, which is
        // the separation §5.4 of the plan asks for. The blind `surface.blend(&ink, 0.10)` fallback
        // remains for a build with no theme; it is the last rung, not the derivation.
        //
        // Read the role rather than blending locally so this control and `table_widget` (same
        // `Table` kind, so the same grid) cannot drift apart: a theme that tunes its grid cannot
        // tune one without the other.
        let cell_border = palette
            .as_ref()
            .map(|colors| colors.outline_variant)
            .filter(|resolved| *resolved != surface)
            .unwrap_or_else(|| surface.blend(&ink, 0.10));
        // A hovered row reads the same accent at a **lighter** weight than a selection:
        // it is pointer position, not a committed state. The accent comes from the same
        // palette read above, so the hover costs no extra theme lock.
        let accent = palette.as_ref().map(|colors| colors.primary).unwrap_or(Color::PRIMARY);
        let hovered_bg = surface.blend(&accent, 0.12);

        context.face_with_gradient_corners(
            rect,
            surface,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().resolved_corner_radii().to_array(),
            Color::BLACK,
        );
        crate::widget::draw::stroke_surface_border(
            context,
            rect,
            self.style().resolved_corner_radii().to_array(),
            border,
            1,
        );

        if self.show_header {
            if let Some(source) = self.data_source.as_ref() {
                let font = effective_font(self.style());
                let (_, columns) = self.visible_grid(rect);
                let scroll_offset = self.column_offset(self.scroll_column);
                let mut painted = 0usize;
                for global in self.scroll_column..self.column_count() {
                    // A hidden column keeps its source index but has no slot (V-013).
                    if self.is_column_hidden(global) {
                        continue;
                    }
                    if painted >= columns {
                        break;
                    }
                    let width = self.column_width_at(global);
                    let band = Rect::new(
                        rect.x + 2 + self.column_offset(global) - scroll_offset,
                        rect.y + 2,
                        width.saturating_sub(2),
                        self.row_height.saturating_sub(2),
                    );
                    context.fill_rect(band, surface.blend(&ink, 0.08));
                    let name = self
                        .header_labels
                        .get(global)
                        .map(String::as_str)
                        .filter(|label| !label.is_empty())
                        .map(ToOwned::to_owned)
                        .or_else(|| source.column_name(global))
                        .unwrap_or_else(|| std::format!("Column {}", global + 1));
                    let name = crate::widget::text_utils::elide_end(&name, 20);
                    context.draw_text_fitted_cow(
                        context.text_line(band, font),
                        &name,
                        font,
                        ink,
                        HorizontalAlignment::Left,
                    );
                    painted += 1;
                }
            }
        }

        // Read the shared cached window (D09-VIEW-03): a cache hit hands back the existing `Arc`
        // rather than deep-cloning every row, column and `String` on every redraw. All the geometry
        // the layout needs is computed *before* the window borrow is taken, and the shared handle is
        // cloned out so the loop below reads it without holding a borrow of `self`.
        let (row_origin, col_origin) = self.window_origin();
        let (visible_rows, visible_columns) = self.visible_grid(rect);
        let Some(data) = self.visible_window_cells().cloned() else {
            return;
        };
        if data.is_empty() {
            return;
        }

        // The fetched window starts at `scroll - overscan`, so its first `overscan` rows/columns are
        // *above/left of* the viewport. Skip exactly that many before laying out, otherwise the
        // leading overscan data is painted as if it were the first visible row/column and non-zero
        // scroll shows shifted content (D09-VIEW-01). The skipped amount is `scroll - row_start`,
        // the same quantity the draw loop's local index must be offset by to reach the viewport.
        let leading_rows = self.scroll_row.saturating_sub(row_origin);
        let leading_cols = self.scroll_column.saturating_sub(col_origin);

        // The alternating wash is a row band under the cells, so it is painted before the cell
        // strokes. Off by default, which keeps the historical pixels (V-013).
        if self.alternating_rows {
            const INSET: i32 = 2;
            for local_row in 0..visible_rows {
                if (self.scroll_row + local_row) % 2 == 1 {
                    context.fill_rect(
                        Rect::new(
                            rect.x + INSET,
                            rect.y
                                + INSET
                                + self.header_offset()
                                + local_row as i32 * self.row_height as i32,
                            rect.width.saturating_sub(4),
                            self.row_height.saturating_sub(2),
                        ),
                        surface.blend(&accent, 0.05),
                    );
                }
            }
        }

        // The grid's extent and each cell's box come from `visible_grid`/`cell_rect`, the same
        // derivation `cell_at` reads, so the cell a press resolves to is the cell that was
        // painted. The loop no longer carries `x`/`y` accumulators — a cell's position is a
        // function of its own indices, which is what makes it answerable outside this loop.
        let visible = data
            .iter()
            .skip(leading_rows)
            .take(visible_rows)
            .map(|row| row.iter().skip(leading_cols).take(visible_columns));
        // The hovered row's viewport-local index, when the absolute hover is inside the
        // window actually painted. The band is drawn once, spanning the full row, before
        // the cells stroke over it — a whole-row tint is the affordance the family shares,
        // and a cell-sized one flickers as the pointer crosses column boundaries.
        let hovered_local = self
            .hovered_row
            .and_then(|abs| abs.checked_sub(self.scroll_row))
            .filter(|local| *local < visible_rows);
        if let Some(local_row) = hovered_local {
            // The band's top and height are the cell band's own: `INSET + row * stride`
            // tall and `stride - 2` high, so the tint covers the row's cells and the
            // 2 px gaps between them as one continuous stripe.
            const INSET: i32 = 2;
            context.fill_rect(
                Rect::new(
                    rect.x + INSET,
                    rect.y + INSET + local_row as i32 * self.row_height as i32,
                    rect.width.saturating_sub(4),
                    self.row_height.saturating_sub(2),
                ),
                hovered_bg,
            );
        }
        // One font resolution for the whole grid (F-P07): the loop used to resolve the face twice
        // per visible cell, so a full screen paid hundreds of resolutions per frame.
        let cell_font = effective_font(self.style());
        for (row_index, row) in visible.enumerate() {
            // `local_column` counts painted slots, so a hidden column leaves no gap and the
            // ordinal still maps back to the absolute column `cell_at` answers with (V-013).
            let mut local_column = 0usize;
            for (column_index, cell) in row.enumerate() {
                let absolute_column = self.scroll_column + column_index;
                if self.is_column_hidden(absolute_column) {
                    continue;
                }
                let cell_rect = self.cell_rect(rect, row_index, local_column);
                local_column += 1;
                let absolute = (self.scroll_row + row_index, absolute_column);
                if self
                    .encode_cell_index(absolute.0, absolute.1)
                    .is_some_and(|index| self.selection.is_selected(index))
                {
                    context.fill_rect(cell_rect, surface.blend(&accent, 0.30));
                }
                context.draw_rect(cell_rect, cell_border);
                let painted_by_delegate = match self.delegate.as_ref() {
                    Some(delegate) => {
                        delegate.paint(context, cell_rect, absolute.0, absolute.1, cell.as_deref())
                    }
                    None => false,
                };
                if let Some(value) = cell.as_ref().filter(|_| !painted_by_delegate) {
                    // The cell is the band. `y + row_height / 2` as a `draw_text` origin placed
                    // the glyph box's *top* edge on the cell's middle line, so the value drew half
                    // a line low; `text_line` derives the centred box. Fitting keeps a wide value
                    // from spilling into the next column.
                    context.draw_text_fitted_cow(
                        context.text_line(cell_rect, cell_font),
                        value.as_str(),
                        cell_font,
                        ink,
                        HorizontalAlignment::Left,
                    );
                }
            }
        }

        if self.show_scrollbar {
            let total_rows = self.row_count().max(1);
            let (rows, _) = self.visible_grid(rect);
            let visible_rows = rows.max(1).min(total_rows);
            let track = rect.height.saturating_sub(4).max(1);
            let thumb = ((track as u64 * visible_rows as u64) / total_rows as u64)
                .max(24)
                .min(track as u64) as u32;
            let max_scroll = total_rows.saturating_sub(visible_rows);
            let travel = track.saturating_sub(thumb);
            let offset = if max_scroll == 0 {
                0
            } else {
                (travel as u64 * self.scroll_row.min(max_scroll) as u64 / max_scroll as u64) as u32
            };
            context.fill_rect(
                Rect::new(rect.x + rect.width as i32 - 6, rect.y + 2 + offset as i32, 4, thumb),
                accent.with_alpha(120),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // The tests below export SVG on every profile this module is built for, so the import is
    // unconditional: gating it on `desktop` made the tablet profile compile the tests without
    // the function they call.
    use crate::signal::GenericSignal;
    use crate::widget::svg::render_to_svg;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    /// The cell grid uses the weak separator role, not the frame's strength.
    ///
    /// # Why this is the judgement rather than a style preference
    ///
    /// Plan §5.4 asks for separators to be visibly weaker than the focus ring, and its stated
    /// criterion is "the grid line's colour differs from the ring's". The control drew its grid with
    /// a **local** `surface.blend(&ink, 0.10)` while `table_widget` — the same `Table` kind, so the
    /// same grid — read `colors.outline_variant`. Both satisfied the criterion, which is why neither
    /// was a defect, but they were two derivations of one decision: a theme could tune the grid for
    /// one control and not the other. This pins them to one source.
    #[test]
    #[cfg(all(device_profile, feature = "desktop"))]
    fn the_cell_grid_reads_the_weak_separator_role() {
        let _guard = crate::theme::theme_test_guard();
        crate::widget::census::install_preset_appearances();
        crate::theme::global_theme_manager().set_appearance(crate::theme::AppearanceMode::Dark);

        let mut table = VirtualTable::new(Rect::new(0, 0, 320, 200));
        table.set_data_source(Arc::new(StaticSource));
        crate::theme::apply_theme_to_widget(&mut table);
        let svg = render_to_svg(&mut table);

        let manager = crate::style::theme_manager();
        let variant = manager.current_theme().expect("a preset is active").colors.outline_variant;
        let background = manager.current_theme().expect("a preset is active").colors.background;
        let expected = format!("rgba({},{},{},", variant.r, variant.g, variant.b);
        assert!(
            svg.contains(&expected),
            "the grid must be stroked in `outline_variant` ({expected}…), so a theme can tune it \
             for every `Table`-kind control at once"
        );
        // And it must be the **weak** role: a grid drawn in the frame's own `outline` strength would
        // satisfy the previous assertion if the two roles happened to be equal, which is exactly the
        // "two names, one colour" state §5.4 exists to end.
        assert_ne!(variant, background, "the separator role must not coincide with the page");
    }

    struct StaticSource;

    impl IncrementalTableDataSource for StaticSource {
        fn row_count(&self) -> usize {
            100
        }

        fn column_count(&self) -> usize {
            20
        }

        fn data(&self, row: usize, column: usize) -> Option<String> {
            Some(format!("{}:{}", row, column))
        }
    }

    #[test]
    fn visible_window_tracks_scroll() {
        let mut table = VirtualTable::new(Rect::new(0, 0, 240, 80));
        table.set_data_source(Arc::new(StaticSource));

        table.set_scroll_row(12);
        table.set_scroll_column(5);
        let (row_start, _, col_start, _) = table.visible_window();

        assert!(row_start <= 12);
        assert!(col_start <= 5);
        assert!(table.row_count() == 100);
    }

    #[test]
    fn fetch_visible_window_reads_cells() {
        let mut table = VirtualTable::new(Rect::new(0, 0, 240, 80));
        table.set_data_source(Arc::new(StaticSource));

        let data = table.fetch_visible_window();
        assert!(!data.is_empty());
        assert_eq!(data[0][0], Some("0:0".to_string()));
    }

    #[test]
    fn signal_emits_when_window_changes() {
        let mut table = VirtualTable::new(Rect::new(0, 0, 240, 80));
        table.set_data_source(Arc::new(StaticSource));

        let emitted = Arc::new(Mutex::new(Vec::<(usize, usize, usize, usize)>::new()));
        let sink = emitted.clone();
        table.visible_window_changed.connect(move |window| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*window);
            }
        });

        table.set_scroll_row(10);

        let got = emitted.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert!(!got.is_empty());
    }

    #[test]
    fn overscan_settings_resize_visible_window() {
        let mut table = VirtualTable::new(Rect::new(0, 0, 240, 80));
        table.set_data_source(Arc::new(StaticSource));
        table.set_scroll_row(10);
        table.set_scroll_column(5);

        table.set_overscan_rows(0);
        table.set_overscan_columns(0);
        let (_, row_len_small, _, col_len_small) = table.visible_window();

        table.set_overscan_rows(3);
        table.set_overscan_columns(2);
        let (_, row_len_large, _, col_len_large) = table.visible_window();

        assert!(row_len_large >= row_len_small);
        assert!(col_len_large >= col_len_small);
        assert_eq!(table.overscan_rows(), 3);
        assert_eq!(table.overscan_columns(), 2);
    }

    #[test]
    fn new_creates_default_state() {
        let mut table = VirtualTable::new(Rect::new(0, 0, 800, 600));
        assert!(!table.has_data_source());
        assert_eq!(table.scroll_row(), 0);
        assert_eq!(table.scroll_column(), 0);
        assert_eq!(table.row_height(), 20);
        assert_eq!(table.column_width(), 120);
        assert_eq!(table.overscan_rows(), 2);
        assert_eq!(table.overscan_columns(), 1);
        assert_eq!(table.row_count(), 0);
        assert_eq!(table.column_count(), 0);
        let data = table.fetch_visible_window();
        assert!(data.is_empty());
    }

    #[test]
    fn has_data_source_before_and_after() {
        let mut table = VirtualTable::new(Rect::new(0, 0, 800, 600));
        assert!(!table.has_data_source());

        table.set_data_source(Arc::new(StaticSource));
        assert!(table.has_data_source());

        table.clear_data_source();
        assert!(!table.has_data_source());
    }

    #[test]
    fn row_and_column_counts() {
        let mut table = VirtualTable::new(Rect::new(0, 0, 800, 600));
        assert_eq!(table.row_count(), 0);
        assert_eq!(table.column_count(), 0);

        table.set_data_source(Arc::new(StaticSource));
        assert_eq!(table.row_count(), 100);
        assert_eq!(table.column_count(), 20);

        table.clear_data_source();
        assert_eq!(table.row_count(), 0);
        assert_eq!(table.column_count(), 0);
    }

    #[test]
    fn scroll_position_clamping() {
        let mut table = VirtualTable::new(Rect::new(0, 0, 800, 600));
        // Without source, scroll still accumulates (normalize not called after source set)
        table.set_data_source(Arc::new(StaticSource));

        table.set_scroll_row(50);
        assert_eq!(table.scroll_row(), 50);

        // Clamp to max
        table.set_scroll_row(999);
        assert_eq!(table.scroll_row(), 99);

        // Clamped to max column index (19)
        table.set_scroll_column(30);
        assert_eq!(table.scroll_column(), 19);

        // Already at max
        table.set_scroll_column(999);
        assert_eq!(table.scroll_column(), 19);
    }

    #[test]
    fn clear_data_source_resets_state() {
        let mut table = VirtualTable::new(Rect::new(0, 0, 800, 600));
        table.set_data_source(Arc::new(StaticSource));
        table.set_scroll_row(10);
        table.set_scroll_column(5);

        table.clear_data_source();
        assert!(!table.has_data_source());
        assert_eq!(table.scroll_row(), 0);
        assert_eq!(table.scroll_column(), 0);
        assert_eq!(table.row_count(), 0);
        assert_eq!(table.column_count(), 0);
        assert!(table.fetch_visible_window().is_empty());
    }

    #[test]
    fn cached_window_returns_same_data_without_refetch() {
        struct TrackingSource;
        impl IncrementalTableDataSource for TrackingSource {
            fn row_count(&self) -> usize {
                50
            }
            fn column_count(&self) -> usize {
                5
            }
            fn data(&self, row: usize, column: usize) -> Option<String> {
                Some(format!("data-{}:{}", row, column))
            }
            fn revision(&self) -> u64 {
                1
            }
        }

        let mut table = VirtualTable::new(Rect::new(0, 0, 240, 80));
        table.set_data_source(Arc::new(TrackingSource));

        let a = table.fetch_visible_window();
        let b = table.fetch_visible_window();
        assert_eq!(a, b);
    }

    #[test]
    fn a_revisionless_source_without_a_contract_is_refetched_not_served_stale() {
        struct InPlaceSource {
            generation: AtomicU64,
            calls: AtomicUsize,
        }
        impl IncrementalTableDataSource for InPlaceSource {
            fn row_count(&self) -> usize {
                6
            }
            fn column_count(&self) -> usize {
                2
            }
            fn data(&self, row: usize, column: usize) -> Option<String> {
                self.calls.fetch_add(1, Ordering::Relaxed);
                Some(format!("gen-{}:{}:{}", self.generation.load(Ordering::Relaxed), row, column))
            }
            // `revision()` and `data_changed_signal()` keep the trait defaults: no versioning
            // and no signal — no invalidation contract, so the window must not be cached.
        }

        let source =
            Arc::new(InPlaceSource { generation: AtomicU64::new(0), calls: AtomicUsize::new(0) });
        let mut table = VirtualTable::new(Rect::new(0, 0, 240, 80));
        table.set_data_source(source.clone());

        let first = table.fetch_visible_window();
        let calls_after_first = source.calls.load(Ordering::Relaxed);
        assert!(calls_after_first > 0);

        // The source mutates in place. With no revision and no signal there is no way to be
        // told, so the next fetch must go back to the source instead of serving the window
        // built from the old content.
        source.generation.fetch_add(1, Ordering::Relaxed);
        let second = table.fetch_visible_window();
        assert_ne!(first, second);
        assert!(source.calls.load(Ordering::Relaxed) > calls_after_first);
    }

    #[test]
    fn a_revisionless_source_is_cached_and_invalidated_by_its_signal() {
        struct SignalSource {
            generation: AtomicU64,
            calls: AtomicUsize,
            changed: GenericSignal,
        }
        impl IncrementalTableDataSource for SignalSource {
            fn row_count(&self) -> usize {
                6
            }
            fn column_count(&self) -> usize {
                2
            }
            fn data(&self, row: usize, column: usize) -> Option<String> {
                self.calls.fetch_add(1, Ordering::Relaxed);
                Some(format!("gen-{}:{}:{}", self.generation.load(Ordering::Relaxed), row, column))
            }
            fn data_changed_signal(&self) -> Option<&GenericSignal> {
                Some(&self.changed)
            }
        }

        let source = Arc::new(SignalSource {
            generation: AtomicU64::new(0),
            calls: AtomicUsize::new(0),
            changed: GenericSignal::new(),
        });
        let mut table = VirtualTable::new(Rect::new(0, 0, 240, 80));
        table.set_data_source(source.clone());

        let _ = table.fetch_visible_window();
        let calls_after_first = source.calls.load(Ordering::Relaxed);

        // No signal fired: the cached window is reused.
        let cached = table.fetch_visible_window();
        assert_eq!(source.calls.load(Ordering::Relaxed), calls_after_first);

        // The source mutates in place and raises `data_changed`; the generation advances and
        // the next fetch rebuilds from the new content.
        source.generation.fetch_add(1, Ordering::Relaxed);
        source.changed.emit();
        let fresh = table.fetch_visible_window();
        assert_ne!(cached, fresh);
        assert!(source.calls.load(Ordering::Relaxed) > calls_after_first);
        assert!(fresh[0][0].as_deref().is_some_and(|cell| cell.starts_with("gen-1")));
    }

    #[test]
    fn empty_source_handling() {
        struct EmptySource;
        impl IncrementalTableDataSource for EmptySource {
            fn row_count(&self) -> usize {
                0
            }
            fn column_count(&self) -> usize {
                0
            }
            fn data(&self, _: usize, _: usize) -> Option<String> {
                None
            }
        }

        let mut table = VirtualTable::new(Rect::new(0, 0, 800, 600));
        table.set_data_source(Arc::new(EmptySource));

        assert_eq!(table.row_count(), 0);
        assert_eq!(table.column_count(), 0);
        assert_eq!(table.visible_window(), (0, 0, 0, 0));
        assert!(table.fetch_visible_window().is_empty());

        // Scrolling on empty source
        table.set_scroll_row(10);
        assert_eq!(table.scroll_row(), 0);
    }

    #[test]
    fn overscan_default_values() {
        let table = VirtualTable::new(Rect::new(0, 0, 800, 600));
        assert_eq!(table.overscan_rows(), 2);
        assert_eq!(table.overscan_columns(), 1);
    }

    #[test]
    fn set_row_height_and_column_width_minimum_clamp() {
        let mut table = VirtualTable::new(Rect::new(0, 0, 800, 600));
        table.set_data_source(Arc::new(StaticSource));

        table.set_row_height(0);
        assert_eq!(table.row_height(), 1);

        table.set_row_height(30);
        assert_eq!(table.row_height(), 30);

        table.set_column_width(0);
        assert_eq!(table.column_width(), 1);

        table.set_column_width(80);
        assert_eq!(table.column_width(), 80);
    }

    // ── The shared cell derivation ───────────────────────────────────────

    #[test]
    fn a_cells_box_is_a_function_of_its_own_indices() {
        // The defect this pins: the cell position used to be an accumulator inside the draw
        // loop (`x += column_width`), so a cell's box depended on how many cells happened to
        // be drawn before it and could not be asked for independently. It is now
        // `origin + index * stride`.
        let mut table = VirtualTable::new(Rect::new(10, 20, 400, 300));
        table.set_column_width(80);
        table.set_row_height(40);
        let origin = table.geometry();

        // (0,0) sits at the inset, and each step is exactly one stride.
        let first = table.cell_rect(origin, 0, 0);
        assert_eq!(first.x, origin.x + 2);
        assert_eq!(first.y, origin.y + 2);
        assert_eq!(first.width, 78);
        assert_eq!(first.height, 38);

        let third_column = table.cell_rect(origin, 0, 3);
        assert_eq!(third_column.x, first.x + 3 * 80, "a column is one stride apart");
        let second_row = table.cell_rect(origin, 2, 0);
        assert_eq!(second_row.y, first.y + 2 * 40, "a row is one stride apart");

        // Asking for a box twice gives the same answer: no accumulated state.
        assert_eq!(table.cell_rect(origin, 2, 3), table.cell_rect(origin, 2, 3));
    }

    #[test]
    fn the_box_a_press_resolves_to_is_the_box_that_was_painted() {
        // The draw loop and the hit test now read one derivation. Before, the only record of a
        // cell's position was the draw loop's local variable, so the table could not answer
        // "which cell is under this point?" at all.
        let mut table = VirtualTable::new(Rect::new(0, 0, 400, 300));
        table.set_column_width(80);
        table.set_row_height(40);

        let cell = table.cell_rect(table.geometry(), 1, 2);
        let centre = crate::core::Point::new(
            cell.x + cell.width as i32 / 2,
            cell.y + cell.height as i32 / 2,
        );
        assert_eq!(table.cell_at(centre), Some((1, 2)));

        // A point on the table's own border, outside every inset cell, resolves to no cell
        // rather than to a neighbour — the boxes are the authority, not a division of the
        // whole rectangle.
        assert_eq!(table.cell_at(crate::core::Point::new(0, 0)), None);
    }

    #[test]
    fn the_visible_grid_counts_what_fits_and_never_reports_zero() {
        let mut table = VirtualTable::new(Rect::new(0, 0, 400, 300));
        table.set_column_width(100);
        table.set_row_height(50);

        // Usable extent is the rectangle minus the 2 px inset on each side: 396 x 296, so
        // three 100 px columns and five 50 px rows fit.
        assert_eq!(table.visible_grid(table.geometry()), (5, 3));

        // A table clipped to a sliver still reports one of each, so its first cell is drawn
        // rather than the control coming out blank.
        let sliver = Rect::new(0, 0, 3, 3);
        assert_eq!(table.visible_grid(sliver), (1, 1));
    }

    /// N-V-05: a huge overscan must saturate `overscan * 2`, not wrap the window length.
    #[test]
    fn a_huge_overscan_does_not_wrap_the_window() {
        let mut table = VirtualTable::new(Rect::new(0, 0, 320, 200));
        table.set_data_source(Arc::new(StaticSource));
        table.set_row_height(20);
        table.set_column_width(20);
        // Debug builds panic on the wrapping `overscan_rows * 2`; reaching the assert is the check.
        table.set_overscan_rows(usize::MAX);
        table.set_overscan_columns(usize::MAX);
        let (_row_start, row_len, _col_start, col_len) = table.visible_window();
        // A wrapped multiply could have produced a tiny length; saturation keeps it large.
        assert!(row_len > 1, "row length must not wrap to a small value, got {row_len}");
        assert!(col_len > 1, "column length must not wrap to a small value, got {col_len}");
    }

    // ── D09-VIEW-01: the leading overscan is skipped, not painted as the first cell ──

    /// The cell a press resolves to must be the **global** cell at that box, not a window-local
    /// index, once the fetch window begins before the viewport.
    ///
    /// # The defect this pins
    ///
    /// `visible_window()` fetches from `scroll - overscan`, so the returned data's local `(0, 0)` is
    /// *above/left of* the viewport. The draw loop treated that `(0, 0)` as the visible top-left, so
    /// at row 12 / column 5 the cell painted in the first box was actually global (10, 4). `cell_at`
    /// now converts through the window origin, so the box the user sees reports the global index that
    /// was fetched for it.
    #[test]
    fn a_visible_cell_maps_to_its_global_index_after_scrolling() {
        let mut table = VirtualTable::new(Rect::new(0, 0, 320, 200));
        table.set_data_source(Arc::new(StaticSource));
        table.set_row_height(20);
        table.set_column_width(40);

        table.set_scroll_row(12);
        table.set_scroll_column(5);

        // The first visible box (viewport-relative (0, 0)) must name the scrolled-to cell, because
        // that is the cell the draw pass now paints there.
        let first = table.cell_rect(table.geometry(), 0, 0);
        let centre = crate::core::Point::new(
            first.x + first.width as i32 / 2,
            first.y + first.height as i32 / 2,
        );
        assert_eq!(
            table.cell_at(centre),
            Some((12, 5)),
            "the first visible box must report the global scrolled-to cell"
        );

        // And the local index the box is addressed by is exactly `scroll - overscan`, so the drawn
        // loop's leading skip and the hit test's origin share one derivation.
        let (row_origin, col_origin) = table.window_origin();
        assert_eq!(table.scroll_row - row_origin, 2, "default row overscan is skipped");
        assert_eq!(table.scroll_column - col_origin, 1, "default column overscan is skipped");
    }

    /// Overscan 0 makes the window origin the scroll position itself, so the skip is a no-op.
    #[test]
    fn zero_overscan_leaves_the_window_origin_at_the_scroll_position() {
        let mut table = VirtualTable::new(Rect::new(0, 0, 320, 200));
        table.set_data_source(Arc::new(StaticSource));
        table.set_row_height(20);
        table.set_column_width(40);
        table.set_overscan_rows(0);
        table.set_overscan_columns(0);
        table.set_scroll_row(7);
        table.set_scroll_column(3);

        assert_eq!(table.window_origin(), (7, 3));
        let first = table.cell_rect(table.geometry(), 0, 0);
        let centre = crate::core::Point::new(
            first.x + first.width as i32 / 2,
            first.y + first.height as i32 / 2,
        );
        assert_eq!(table.cell_at(centre), Some((7, 3)));
    }

    /// At the top-left edge there is no leading overscan to skip, so the origin is `(0, 0)` and the
    /// first box names the data source's own first cell.
    #[test]
    fn the_top_left_corner_has_no_leading_overscan_to_skip() {
        let mut table = VirtualTable::new(Rect::new(0, 0, 320, 200));
        table.set_data_source(Arc::new(StaticSource));
        table.set_row_height(20);
        table.set_column_width(40);

        assert_eq!(table.visible_window().0, 0);
        assert_eq!(table.visible_window().2, 0);
        let first = table.cell_rect(table.geometry(), 0, 0);
        let centre = crate::core::Point::new(
            first.x + first.width as i32 / 2,
            first.y + first.height as i32 / 2,
        );
        assert_eq!(table.cell_at(centre), Some((0, 0)));
    }

    /// The **drawn** values must be the global cells for the visible boxes — the model-versus-pixels
    /// form of the defect.
    ///
    /// # Why only the scrolled-to cell is non-empty
    ///
    /// The SVG does not embed the cell's text string (each glyph is an outline `<path>`), so the
    /// content cannot be read back as a literal. Instead the data source makes exactly one cell
    /// non-empty — the cell the viewport's top-left box must hold after scrolling. If the draw pass
    /// skipped the leading overscan correctly, that one value lands at the first visible box; if it
    /// did not, the leading overscan cell would be painted first and the visible box would be blank.
    #[test]
    #[cfg(all(device_profile, feature = "desktop"))]
    fn the_drawn_cells_are_the_global_cells_after_scrolling() {
        struct MarkedSource;
        impl IncrementalTableDataSource for MarkedSource {
            fn row_count(&self) -> usize {
                50
            }
            fn column_count(&self) -> usize {
                10
            }
            fn data(&self, row: usize, column: usize) -> Option<String> {
                if (row, column) == (12, 5) {
                    Some("W".to_string())
                } else {
                    Some(String::new())
                }
            }
        }

        let mut table = VirtualTable::new(Rect::new(0, 0, 320, 200));
        table.set_data_source(Arc::new(MarkedSource));
        table.set_row_height(20);
        table.set_column_width(40);
        table.set_scroll_row(12);
        table.set_scroll_column(5);

        // The first visible box is where the scrolled-to global cell must be painted.
        let first = table.cell_rect(table.geometry(), 0, 0);
        let boxes = crate::widget::svg::text_ink_boxes(&render_to_svg(&mut table));
        assert_eq!(
            boxes.len(),
            1,
            "only the one non-empty cell must be drawn as text; a leading overscan skip paints it, \
             a missing skip paints the blank lead cell instead"
        );
        let (bx, by, _, _) = boxes[0];
        assert!(
            bx >= first.x && bx < first.x + first.width as i32,
            "the value must be painted inside the first visible box ({first:?}), got x={bx}"
        );
        assert!(
            by >= first.y && by < first.y + first.height as i32,
            "the value must be painted inside the first visible box ({first:?}), got y={by}"
        );
    }

    // ── D09-VIEW-03: the cache-hit draw path must not deep-clone the window ──

    /// A cache hit must be shared, not deep-cloned, and the draw path must read the same window
    /// across consecutive redraws without another fetch.
    ///
    /// # The defect this pins
    ///
    /// `fetch_visible_window()` returned `cache.clone()` on every hit, and `Draw::draw` called it on
    /// every redraw — so a static window re-copied every row, column and `String` each frame. The
    /// cache is now shared behind an `Arc`: a hit hands out the same allocation, which is observable
    /// as a stable pointer identity across calls, and the source is still fetched exactly once.
    #[test]
    fn a_cache_hit_shares_the_window_instead_of_deep_cloning() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct CountingSource {
            fetches: Arc<AtomicUsize>,
        }
        impl IncrementalTableDataSource for CountingSource {
            fn row_count(&self) -> usize {
                50
            }
            fn column_count(&self) -> usize {
                5
            }
            fn data(&self, row: usize, column: usize) -> Option<String> {
                Some(format!("data-{}:{}", row, column))
            }
            // The shared-cache guarantee under test applies to sources with an invalidation
            // contract; a contract-less source is refetched instead of served stale.
            fn revision(&self) -> u64 {
                1
            }
            fn fetch_window(
                &self,
                row_start: usize,
                row_len: usize,
                column_start: usize,
                column_len: usize,
            ) -> Vec<Vec<Option<String>>> {
                self.fetches.fetch_add(1, Ordering::SeqCst);
                let row_end = row_start.saturating_add(row_len).min(self.row_count());
                let col_end = column_start.saturating_add(column_len).min(self.column_count());
                (row_start..row_end)
                    .map(|row| (column_start..col_end).map(|c| self.data(row, c)).collect())
                    .collect()
            }
        }

        let fetches = Arc::new(AtomicUsize::new(0));
        let mut table = VirtualTable::new(Rect::new(0, 0, 240, 80));
        table.set_data_source(Arc::new(CountingSource { fetches: fetches.clone() }));

        // Two cache hits in a row must be served from the cache, so the source is fetched exactly
        // once across both; the data each call returns is identical and complete.
        let first = table.fetch_visible_window();
        let second = table.fetch_visible_window();
        assert_eq!(first, second, "a cache hit must return the same window contents");
        assert!(!first.is_empty());
        assert_eq!(fetches.load(Ordering::SeqCst), 1, "a cache hit must not re-fetch the source");

        // A redraw (the draw path reads the shared window) is likewise a cache hit, so no extra
        // fetch happens between frames.
        #[cfg(all(device_profile, feature = "desktop"))]
        {
            let _ = crate::widget::svg::render_to_svg(&mut table);
            assert_eq!(
                fetches.load(Ordering::SeqCst),
                1,
                "a redraw must read the cached window, not re-fetch"
            );
        }

        // Scrolling invalidates the cache, and the next fetch returns the window that begins at
        // the new fetch origin (`scroll - overscan`).
        table.set_scroll_row(10);
        let scrolled = table.fetch_visible_window();
        let (row_start, _, _, _) = table.visible_window();
        assert_eq!(row_start, 8, "row 10 with overscan 2 fetches from row 8");
        assert_eq!(
            scrolled[0][0],
            Some(format!("data-{}:0", row_start)),
            "after invalidation the window must begin at the new fetch origin"
        );
        assert_eq!(fetches.load(Ordering::SeqCst), 2, "a scroll must invalidate and re-fetch once");
    }

    /// Hover is the absolute row under the pointer, resolved through the same `cell_at`
    /// derivation the paint reads, and leaving the table clears the highlight.
    #[test]
    fn virtual_table_hover_follows_the_pointer() {
        let mut table = VirtualTable::new(Rect::new(0, 0, 320, 200));
        table.set_data_source(Arc::new(StaticSource));

        let cell = table.cell_rect(table.geometry(), 0, 0);
        table.handle_event(&Event::MouseMove {
            pos: crate::core::Point::new(cell.x + 5, cell.y + 5),
        });
        assert_eq!(table.hovered_row, Some(0));

        // A lower viewport row lights its own absolute index.
        let cell = table.cell_rect(table.geometry(), 2, 1);
        table.handle_event(&Event::MouseMove {
            pos: crate::core::Point::new(cell.x + 5, cell.y + 5),
        });
        assert_eq!(table.hovered_row, Some(2));

        table.handle_event(&Event::MouseLeave { pos: crate::core::Point::new(-1, -1) });
        assert_eq!(table.hovered_row, None);
    }

    /// The hover is stored absolutely, so scrolling moves the highlight with its row:
    /// the same pointer position after a scroll names a different absolute row.
    #[test]
    fn virtual_table_hover_is_absolute_across_a_scroll() {
        let mut table = VirtualTable::new(Rect::new(0, 0, 320, 200));
        table.set_data_source(Arc::new(StaticSource));

        let cell = table.cell_rect(table.geometry(), 0, 0);
        table.handle_event(&Event::MouseMove {
            pos: crate::core::Point::new(cell.x + 5, cell.y + 5),
        });
        assert_eq!(table.hovered_row, Some(0));

        table.set_scroll_row(5);
        assert_eq!(table.hovered_row, Some(0), "the row the pointer was on did not change");

        // The pointer has not moved, but the row now under it is global row 5.
        table.handle_event(&Event::MouseMove {
            pos: crate::core::Point::new(cell.x + 5, cell.y + 5),
        });
        assert_eq!(table.hovered_row, Some(5));
    }

    /// Cell selection, the header row, page keys and the scroll-bar thumb are wired.
    #[test]
    fn selection_header_paging_and_scrollbar_are_wired() {
        use crate::event::types::EventHandler;
        let mut table = VirtualTable::new(Rect::new(0, 0, 320, 200));
        table.set_data_source(Arc::new(StaticSource));
        let cell = table.cell_rect(Rect::new(0, 0, 320, 200), 1, 1);
        table.handle_event(&Event::MousePress {
            pos: crate::core::Point::new(cell.x + 2, cell.y + 2),
            button: 1,
            modifiers: 0,
        });
        assert!(table.selected_cell().is_some());
        table.set_show_header(true);
        assert!(table.show_header());
        table.set_show_scrollbar(true);
        assert!(table.show_scrollbar());
        table.handle_event(&Event::KeyPress {
            key: crate::event::key_codes::PAGE_DOWN,
            modifiers: 0,
        });
        assert!(table.scroll_row() > 0, "PageDown advances one viewport");
        let _ = render_to_svg(&mut table);
    }

    /// A delegate can take over a cell's ink, and the table reports that it is installed.
    #[test]
    fn a_cell_delegate_can_paint_a_cell() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct CountingDelegate {
            painted: AtomicUsize,
        }
        impl VirtualCellDelegate for CountingDelegate {
            fn paint(
                &self,
                context: &mut RenderContext,
                cell: Rect,
                _row: usize,
                _column: usize,
                _text: Option<&str>,
            ) -> bool {
                context.fill_rect(cell, Color::PRIMARY);
                self.painted.fetch_add(1, Ordering::Relaxed);
                true
            }
        }

        let delegate = Arc::new(CountingDelegate { painted: AtomicUsize::new(0) });
        let mut table = VirtualTable::new(Rect::new(0, 0, 240, 120));
        table.set_data_source(Arc::new(StaticSource));
        table.set_delegate(delegate.clone());
        assert!(table.has_delegate());
        let _ = render_to_svg(&mut table);
        assert!(delegate.painted.load(Ordering::Relaxed) > 0, "the delegate owned the cells");
        table.clear_delegate();
        assert!(!table.has_delegate());
    }

    /// F-P06: a one-row scroll keeps the overlapping rows and fetches only the new row.
    #[test]
    fn a_small_scroll_fetches_only_the_new_rows() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct RowCountingSource {
            rows: Arc<AtomicUsize>,
        }
        impl IncrementalTableDataSource for RowCountingSource {
            fn row_count(&self) -> usize {
                200
            }
            fn column_count(&self) -> usize {
                3
            }
            fn data(&self, row: usize, column: usize) -> Option<String> {
                Some(format!("{row}:{column}"))
            }
            fn revision(&self) -> u64 {
                1
            }
            fn fetch_window(
                &self,
                row_start: usize,
                row_len: usize,
                column_start: usize,
                column_len: usize,
            ) -> Vec<Vec<Option<String>>> {
                self.rows.fetch_add(row_len, Ordering::SeqCst);
                (row_start..row_start.saturating_add(row_len).min(self.row_count()))
                    .map(|row| {
                        (column_start..column_start.saturating_add(column_len))
                            .map(|column| self.data(row, column))
                            .collect()
                    })
                    .collect()
            }
        }

        let fetched_rows = Arc::new(AtomicUsize::new(0));
        let mut table = VirtualTable::new(Rect::new(0, 0, 240, 80));
        table.set_overscan_rows(0);
        table.set_data_source(Arc::new(RowCountingSource { rows: Arc::clone(&fetched_rows) }));

        let first = table.fetch_visible_window();
        assert_eq!(first.len(), 4, "80 px / 20 px shows four rows");
        assert_eq!(fetched_rows.load(Ordering::SeqCst), 4);

        table.set_scroll_row(1);
        let scrolled = table.fetch_visible_window();
        assert_eq!(scrolled.len(), 4);
        assert_eq!(scrolled[0][0], Some("1:0".to_string()), "the window slid by one row");
        assert_eq!(
            fetched_rows.load(Ordering::SeqCst),
            5,
            "only the newly exposed row was fetched from the source"
        );
    }

    /// V-06: the selection model supports extended ranges and ctrl toggles.
    #[test]
    fn multi_select_ranges_and_toggles_cells() {
        use crate::shortcut::Modifiers;
        use crate::widget::view_widgets::list_view::SelectionMode;

        let mut table = VirtualTable::new(Rect::new(0, 0, 320, 200));
        table.set_data_source(Arc::new(StaticSource));
        table.set_selection_mode(SelectionMode::Extended);
        assert_eq!(table.selection_mode(), SelectionMode::Extended);

        assert!(table.select_cell(0, 0));
        assert!(table.select_cell_with_modifiers(0, 2, Modifiers::SHIFT));
        assert_eq!(table.selected_cells(), vec![(0, 0), (0, 1), (0, 2)]);

        // Ctrl toggles the middle cell off, and the current cell follows the gesture.
        assert!(table.select_cell_with_modifiers(0, 1, Modifiers::CTRL));
        assert_eq!(table.selected_cells(), vec![(0, 0), (0, 2)]);
        assert_eq!(table.selected_cell(), Some((0, 1)));
    }

    /// V-07: the arrow keys move the current cell and the viewport follows it.
    #[test]
    fn arrow_keys_move_the_current_cell() {
        use crate::event::key_codes::{DOWN, END, RIGHT};

        let mut table = VirtualTable::new(Rect::new(0, 0, 320, 200));
        table.set_data_source(Arc::new(StaticSource));
        table.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Tab });

        table.handle_event(&Event::KeyPress { key: DOWN, modifiers: 0 });
        assert_eq!(table.selected_cell(), Some((1, 0)), "Down selects the next row");
        table.handle_event(&Event::KeyPress { key: RIGHT, modifiers: 0 });
        assert_eq!(table.selected_cell(), Some((1, 1)), "Right selects the next column");
        table.handle_event(&Event::KeyPress { key: END, modifiers: 0 });
        assert_eq!(table.selected_cell().map(|(row, _)| row), Some(99));
        assert!(table.scroll_row() > 0, "the viewport followed the cell");
    }

    /// V-013: per-column widths and hidden columns change the painted geometry, while the
    /// absolute column indices stay stable.
    #[test]
    fn per_column_widths_and_hidden_columns_drive_the_geometry() {
        let mut table = VirtualTable::new(Rect::new(0, 0, 400, 200));
        table.set_data_source(Arc::new(StaticSource));

        assert!(table.set_column_width_at(1, 40));
        let rect0 = table.cell_rect(table.geometry(), 0, 0);
        let rect1 = table.cell_rect(table.geometry(), 0, 1);
        let rect2 = table.cell_rect(table.geometry(), 0, 2);
        assert_eq!(rect1.x - rect0.x, 120);
        assert_eq!(rect2.x - rect0.x, 160, "column 2 starts after column 1's own width");

        assert!(table.set_column_hidden(1, true));
        assert_eq!(table.column_width_at(1), 0, "a hidden column occupies no width");
        let hidden_slot = table.cell_rect(table.geometry(), 0, 1);
        assert_eq!(hidden_slot.x - rect0.x, 120, "column 2 moved into the freed slot");
        assert_eq!(table.column_at(hidden_slot.x + 1), Some(2));
        assert_eq!(table.hidden_columns(), vec![1]);
        assert!(!table.set_column_hidden(99, true), "there is no column 99");
    }

    /// V-013: header clicks sort the window three-state, and each state paints its own order.
    #[test]
    fn sorting_orders_the_window_and_cycles_three_states() {
        struct SortSource;
        impl IncrementalTableDataSource for SortSource {
            fn row_count(&self) -> usize {
                3
            }
            fn column_count(&self) -> usize {
                1
            }
            fn data(&self, row: usize, column: usize) -> Option<String> {
                let _ = column;
                ["c", "a", "b"].get(row).map(|value| (*value).to_string())
            }
        }

        let first_cell = |table: &mut VirtualTable| {
            table.fetch_visible_window().first().and_then(|row| row.first().cloned()).flatten()
        };

        let mut table = VirtualTable::new(Rect::new(0, 0, 300, 120));
        table.set_data_source(Arc::new(SortSource));
        table.set_show_header(true);

        assert_eq!(table.toggle_sort_column(0), Some(false));
        assert_eq!(first_cell(&mut table).as_deref(), Some("a"));
        assert_eq!(table.toggle_sort_column(0), Some(true));
        assert_eq!(first_cell(&mut table).as_deref(), Some("c"));
        assert_eq!(table.toggle_sort_column(0), None, "the third state is unsorted");
        assert!(table.sort_specs().is_empty());
        assert_eq!(first_cell(&mut table).as_deref(), Some("c"), "source order is restored");

        // A header press is the same gesture.
        let header = Point::new(table.geometry().x + 3, table.geometry().y + 3);
        table.handle_event(&Event::MousePress { pos: header, button: 1, modifiers: 0 });
        assert_eq!(table.sort_specs(), &[SortSpec { column: 0, descending: false }]);
    }

    /// V-013: both the right-button press and the platform context-menu event are reported.
    #[test]
    fn the_context_menu_request_is_reported() {
        use std::sync::atomic::AtomicUsize;

        let mut table = VirtualTable::new(Rect::new(0, 0, 300, 120));
        table.set_data_source(Arc::new(StaticSource));
        let hits = Arc::new(AtomicUsize::new(0));
        let sink = Arc::clone(&hits);
        table.context_menu_requested.connect(move |_| {
            sink.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        });

        let pos = Point::new(10, 10);
        table.handle_event(&Event::MousePress { pos, button: 3, modifiers: 0 });
        table.handle_event(&Event::ContextMenu { pos });
        assert_eq!(hits.load(std::sync::atomic::Ordering::Relaxed), 2);
    }

    /// V-013: the alternating wash changes the painted output and is off by default.
    #[test]
    fn alternating_rows_change_the_painted_output() {
        let mut table = VirtualTable::new(Rect::new(0, 0, 300, 120));
        table.set_data_source(Arc::new(StaticSource));
        assert!(!table.alternating_rows(), "off by default keeps the historical pixels");
        let plain = render_to_svg(&mut table);
        table.set_alternating_rows(true);
        assert!(table.alternating_rows());
        let striped = render_to_svg(&mut table);
        assert_ne!(plain, striped, "the odd-row band is actually painted");
    }
}
