// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! GridTable widget — feature-rich virtualized table with grid lines, headers,
//! sorting, selection, and column resize.
//!
//! Uses the `IncrementalTableDataSource` protocol from `data_source` for data access.

#[cfg(not(alloc_frugal))]
use std::cell::RefCell;
#[cfg(not(alloc_frugal))]
use std::sync::Arc;

use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler, WheelScroll};
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, Signal1};
use crate::widget::capability::coercion::{expect_bool, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

use super::data_source::{DataRole, IncrementalTableDataSource};
use super::table_widget::TableEditTriggers;

/// Cached column spans, keyed by the inputs that decide the geometry.
///
/// `visible_column_spans` used to rebuild the run from scratch on every call, and both the painter
/// and each pointer hit test call it — so one mouse move rebuilt the same geometry several times.
/// The cache stores the run once per `(scroll, widths, data width)` and hands out slices (F-P05).
#[derive(Clone, Debug)]
struct ColumnSpanCache {
    scroll_column: usize,
    column_count: usize,
    width_revision: u64,
    data_width: u32,
    default_width: u32,
    min_width: u32,
    mode: GridSectionResizeMode,
    spans: Vec<(usize, i32, i32)>,
}

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Controls which parts of the grid respond to pointer selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridTableSelectionMode {
    /// No selection allowed.
    None,
    /// Single cell selection.
    Cell,
    /// Entire-row selection.
    Row,
    /// Entire-column selection.
    Column,
}

/// Describes a single sort direction applied to a column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridTableSortSpec {
    /// Zero-based column index.
    pub column: usize,
    /// `true` for descending order.
    pub descending: bool,
}

/// The identity of a materialised sort projection.
///
/// A sort walk reads every source row, so its result is kept until one of its inputs proves it
/// stale: the source revision/generation (in-place changes), the row count, the sort specs, or the
/// role the keys were read with.
#[derive(Clone, Debug, PartialEq, Eq)]
struct GridSortOrderKey {
    revision: u64,
    generation: u64,
    rows: usize,
    specs: Vec<(usize, bool)>,
    role: DataRole,
}

/// A sort key normalised once per row and column.
///
/// Comparing the raw text per pair re-lowercased both operands and re-parsed both numbers on every
/// comparison. A sort of `n` rows performs `O(n log n)` comparisons, so the normalised form is
/// computed once per cell instead.
fn normalise_sort_key(text: &str) -> (String, Option<f64>) {
    let trimmed = text.trim();
    (trimmed.to_lowercase(), trimmed.parse::<f64>().ok())
}

/// Compares two normalised keys, preferring a numeric order when both sides are numbers.
fn compare_sort_keys(
    left: &(String, Option<f64>),
    right: &(String, Option<f64>),
) -> core::cmp::Ordering {
    if let (Some(left), Some(right)) = (left.1, right.1) {
        if let Some(order) = left.partial_cmp(&right) {
            return order;
        }
    }
    left.0.cmp(&right.0)
}

// ---------------------------------------------------------------------------
// GridTableWidget
// ---------------------------------------------------------------------------

/// How column widths are decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GridSectionResizeMode {
    /// The caller drags a column edge; widths start at the default (the historical mode).
    #[default]
    Interactive,
    /// Every column divides the data area equally.
    Stretch,
    /// Every column uses the minimum width and never grows.
    Fixed,
    /// Columns keep their current widths and are not draggable.
    ResizeToContents,
}

/// Feature-rich virtualized table with grid lines, fixed headers, row numbers,
/// sortable columns, cell/row selection, and interactive column resizing.
pub struct GridTableWidget {
    base: BaseWidget,

    data_source: Option<Arc<dyn IncrementalTableDataSource>>,

    // Scrolling
    scroll_row: usize,
    scroll_column: usize,

    // Sizing
    row_height: u32,
    min_column_width: u32,
    header_height: u32,
    row_number_width: u32,

    // Appearance
    //
    // These are the *last-resort* literals for the grid's chrome. `Draw` resolves explicit
    // style first, then the theme's resolved style, and treats a colour that still equals one
    // of these as "nothing was configured", so they stay published for callers of the
    // setters without pinning the rendering to a light-only palette.
    grid_color: Color,
    _grid_thickness: u32,
    header_bg: Color,
    header_text_color: Color,
    selected_bg: Color,

    // Selection
    selected_cell: Option<(usize, usize)>,
    /// The full selection when extended selection is on; holds cells, not rows, so the
    /// three selection modes can each interpret membership differently.
    selected_cells: std::collections::BTreeSet<(usize, usize)>,
    /// Rows selected in row mode.
    selected_rows: std::collections::BTreeSet<usize>,
    /// Columns selected in column mode.
    selected_columns: std::collections::BTreeSet<usize>,
    /// Whether ctrl/shift extend the selection instead of replacing it.
    multi_select: bool,
    selection_mode: GridTableSelectionMode,
    /// The row the pointer is over, for the hover tint. Row-level (not cell-level) to match
    /// `table_widget`: a whole-row tint is the affordance the family shares, and a cell-sized
    /// one flickers as the pointer crosses column boundaries.
    hovered_row: Option<usize>,

    // Per-column state
    column_widths: Vec<u32>,
    /// Bumped by every column-width mutation; keys [`ColumnSpanCache`].
    column_width_revision: u64,
    /// Per-column cell alignment (V-001); an absent entry means `Left`.
    column_alignments: std::collections::HashMap<usize, HorizontalAlignment>,
    /// The cached column run shared by the painter and the hit tests (F-P05).
    span_cache: RefCell<Option<ColumnSpanCache>>,

    // Sort
    sort_specs: Vec<GridTableSortSpec>,
    /// Whether a header click is allowed to change the sort order.
    sorting_enabled: bool,
    /// The data role the sort keys are read with.
    sort_role: DataRole,
    /// Projected row -> source row when sorting is active; `None` means identity.
    sort_order: Option<Vec<usize>>,
    /// The inputs `sort_order` was built from.
    sort_order_key: Option<GridSortOrderKey>,

    // Data source change tracking
    /// Whether `data_changed_signal` refreshes the view.
    auto_refresh: bool,
    /// Scoped subscription to the bound source's change signal.
    data_source_connection_scope: ConnectionScope,
    /// Advanced by `data_changed` for sources that do not expose a revision.
    data_generation: Arc<std::sync::atomic::AtomicU64>,

    // Column resize tracking
    resizing_column: Option<usize>,
    resize_start_x: i32,
    resize_start_width: u32,

    /// Gestures that request an edit (V-01). `ALL` preserves the historical behaviour.
    edit_triggers: TableEditTriggers,

    // Cached visibility
    visible_rows: usize,

    // ── Signals ──
    /// Emitted when a cell is selected: `(row, column)`.
    pub cell_selected: Signal1<(usize, usize)>,
    /// Emitted when a cell is double-clicked: `(row, column)`.
    pub cell_double_clicked: Signal1<(usize, usize)>,
    /// Emitted when sort changes for a column: `(column, descending)`.
    pub sort_changed: Signal1<(usize, bool)>,
    /// Emitted when a header is clicked (before sort toggle): `column`.
    pub header_clicked: Signal1<usize>,
    /// How column widths are decided.
    section_resize_mode: GridSectionResizeMode,
    /// Whether odd rows receive the alternating wash.
    alternating_rows: bool,
    /// Emitted when an edit is requested for `(row, column)`.
    pub edit_requested: Signal1<(usize, usize)>,
    /// Emitted when the context menu is requested at a point.
    pub context_menu_requested: Signal1<Point>,
    /// Whether a vertical scroll-bar thumb is painted.
    show_scrollbar: bool,
}

impl GridTableWidget {
    /// The neutral-literal grid colour the constructor seeds [`Self::grid_color`] with.
    ///
    /// `Draw` compares the current value against this to tell "the caller left the
    /// appearance alone" from "the caller asked for this exact colour".
    const DEFAULT_GRID_COLOR: Color = Color::rgb(210, 215, 225);
    /// See [`Self::DEFAULT_GRID_COLOR`].
    const DEFAULT_HEADER_BG: Color = Color::rgb(240, 242, 245);
    /// See [`Self::DEFAULT_GRID_COLOR`].
    const DEFAULT_HEADER_TEXT_COLOR: Color = Color::rgb(40, 50, 70);
    /// See [`Self::DEFAULT_GRID_COLOR`].
    const DEFAULT_SELECTED_BG: Color = Color::rgb(200, 220, 250);

    /// How far either side of a column's right edge the resize handle answers, in pixels.
    ///
    /// Named because the hit test and the drawn boundary are the same edge: the handle used the
    /// literal `5` while the painter drew the column's own `x + width`, so changing one could move
    /// the grab region away from the line it is meant to be grabbing.
    const RESIZE_HANDLE_WIDTH: u32 = 5;

    /// Creates a new empty grid table with default appearance and the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::GridTable, geometry, "GridTableWidget"),
            data_source: None,
            scroll_row: 0,
            scroll_column: 0,
            row_height: 24,
            min_column_width: 30,
            header_height: 28,
            row_number_width: 48,
            grid_color: Self::DEFAULT_GRID_COLOR,
            _grid_thickness: 1,
            header_bg: Self::DEFAULT_HEADER_BG,
            header_text_color: Self::DEFAULT_HEADER_TEXT_COLOR,
            selected_bg: Self::DEFAULT_SELECTED_BG,
            selected_cell: None,
            selected_cells: std::collections::BTreeSet::new(),
            selected_rows: std::collections::BTreeSet::new(),
            selected_columns: std::collections::BTreeSet::new(),
            multi_select: false,
            selection_mode: GridTableSelectionMode::Cell,
            hovered_row: None,
            column_widths: Vec::new(),
            column_width_revision: 0,
            column_alignments: std::collections::HashMap::new(),
            span_cache: RefCell::new(None),
            sort_specs: Vec::new(),
            sorting_enabled: true,
            sort_role: DataRole::Display,
            sort_order: None,
            sort_order_key: None,
            auto_refresh: true,
            data_source_connection_scope: ConnectionScope::new(),
            data_generation: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            resizing_column: None,
            resize_start_x: 0,
            resize_start_width: 0,
            edit_triggers: TableEditTriggers::ALL,
            visible_rows: 0,
            cell_selected: Signal1::new(),
            cell_double_clicked: Signal1::new(),
            sort_changed: Signal1::new(),
            header_clicked: Signal1::new(),
            section_resize_mode: GridSectionResizeMode::Interactive,
            alternating_rows: false,
            edit_requested: Signal1::new(),
            context_menu_requested: Signal1::new(),
            show_scrollbar: false,
        }
    }

    // -----------------------------------------------------------------------
    // Data source
    // -----------------------------------------------------------------------

    /// Binds a new data source and resets scroll/selection state.
    pub fn set_data_source(&mut self, source: Arc<dyn IncrementalTableDataSource>) {
        self.data_source = Some(source);
        self.scroll_row = 0;
        self.scroll_column = 0;
        self.selected_cell = None;
        self.selected_cells.clear();
        self.selected_rows.clear();
        self.selected_columns.clear();
        self.sort_specs.clear();
        self.column_widths.clear();
        self.column_width_revision = self.column_width_revision.wrapping_add(1);
        self.sort_order = None;
        self.sort_order_key = None;
        self.data_generation.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.bind_data_source_changes();
        self.update_visibility();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Subscribes to the bound source's change signal when `auto_refresh` is on.
    ///
    /// The subscription is scoped: rebinding drops the previous one, so a replaced source cannot
    /// keep waking this control. The generation counter is the cache key for sources that signal
    /// changes but do not expose a monotonic `revision()`.
    fn bind_data_source_changes(&mut self) {
        self.data_source_connection_scope = ConnectionScope::new();
        if !self.auto_refresh {
            return;
        }
        let Some(source) = self.data_source.clone() else {
            return;
        };
        let Some(data_changed) = source.data_changed_signal() else {
            return;
        };
        let redraw = self.base.redraw_requested_signal().clone();
        let layout = self.base.layout_requested_signal().clone();
        let generation = Arc::clone(&self.data_generation);
        data_changed.connect_scoped(&self.data_source_connection_scope, move || {
            generation.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            redraw.emit();
            layout.emit();
        });
    }

    /// Whether the bound source's change signal is followed.
    pub fn auto_refresh(&self) -> bool {
        self.auto_refresh
    }

    /// Enables or disables following the bound source's change signal.
    pub fn set_auto_refresh(&mut self, auto_refresh: bool) {
        if self.auto_refresh == auto_refresh {
            return;
        }
        self.auto_refresh = auto_refresh;
        self.bind_data_source_changes();
        self.base.request_redraw();
    }

    /// Clears the data source and resets all local state.
    pub fn clear_data_source(&mut self) {
        self.data_source = None;
        self.data_source_connection_scope = ConnectionScope::new();
        self.scroll_row = 0;
        self.scroll_column = 0;
        self.selected_cell = None;
        self.selected_cells.clear();
        self.selected_rows.clear();
        self.selected_columns.clear();
        self.sort_specs.clear();
        self.column_widths.clear();
        self.column_width_revision = self.column_width_revision.wrapping_add(1);
        self.sort_order = None;
        self.sort_order_key = None;
        self.visible_rows = 0;
        self.resizing_column = None;
        self.base.request_layout();
        self.base.request_redraw();
    }

    // -----------------------------------------------------------------------
    // Row / column counts
    // -----------------------------------------------------------------------

    /// Returns the number of rows from the data source (0 if none).
    pub fn row_count(&self) -> usize {
        self.data_source.as_ref().map(|s| s.row_count()).unwrap_or(0)
    }

    /// Returns the number of columns from the data source (0 if none).
    pub fn column_count(&self) -> usize {
        self.data_source.as_ref().map(|s| s.column_count()).unwrap_or(0)
    }

    // -----------------------------------------------------------------------
    // Scrolling
    // -----------------------------------------------------------------------

    /// Returns the current scroll row offset.
    pub fn scroll_row(&self) -> usize {
        self.scroll_row
    }

    /// Sets the vertical scroll row, clamped to `[0, max_row]`.
    pub fn set_scroll_row(&mut self, row: usize) {
        let max_row = self.row_count().saturating_sub(1);
        let next = row.min(max_row);
        if next == self.scroll_row {
            return;
        }
        self.scroll_row = next;
        self.base.request_redraw();
    }

    /// Returns the current scroll column offset.
    pub fn scroll_column(&self) -> usize {
        self.scroll_column
    }

    /// Sets the horizontal scroll column, clamped to `[0, max_col]`.
    pub fn set_scroll_column(&mut self, column: usize) {
        let max_col = self.column_count().saturating_sub(1);
        let next = column.min(max_col);
        if next == self.scroll_column {
            return;
        }
        self.scroll_column = next;
        self.base.request_redraw();
    }

    // -----------------------------------------------------------------------
    // Row height
    // -----------------------------------------------------------------------

    /// Sets the uniform row height (minimum 1).
    pub fn set_row_height(&mut self, height: u32) {
        let next = height.max(1);
        if next == self.row_height {
            return;
        }
        self.row_height = next;
        self.update_visibility();
        self.base.request_layout();
        self.base.request_redraw();
    }

    // -----------------------------------------------------------------------
    // Selection mode
    // -----------------------------------------------------------------------

    /// Sets the selection mode.
    pub fn set_selection_mode(&mut self, mode: GridTableSelectionMode) {
        if self.selection_mode == mode {
            return;
        }
        self.selection_mode = mode;
        if mode == GridTableSelectionMode::None {
            self.selected_cell = None;
            self.selected_cells.clear();
            self.selected_rows.clear();
            self.selected_columns.clear();
            self.base.request_redraw();
        }
    }

    /// Whether ctrl/shift extend the selection instead of replacing it.
    pub fn multi_select(&self) -> bool {
        self.multi_select
    }

    /// Enables or disables extended selection.
    ///
    /// Off by default, which keeps a plain click replacing the selection exactly as before.
    pub fn set_multi_select(&mut self, multi_select: bool) {
        if self.multi_select == multi_select {
            return;
        }
        self.multi_select = multi_select;
        if !multi_select {
            self.selected_cells.clear();
        }
        self.base.request_redraw();
    }

    /// The extended selection, in ascending order.
    ///
    /// In row mode this lists every selected column of each row, and in column mode every
    /// selected row of each column; the mode decides the unit, so one accessor serves all
    /// three.
    pub fn selected_cells(&self) -> Vec<(usize, usize)> {
        match self.selection_mode {
            GridTableSelectionMode::Row => {
                let columns = self.column_count();
                self.selected_rows
                    .iter()
                    .flat_map(|row| (0..columns).map(move |column| (*row, column)))
                    .collect()
            }
            GridTableSelectionMode::Column => {
                let rows = self.row_count();
                self.selected_columns
                    .iter()
                    .flat_map(|column| (0..rows).map(move |row| (row, *column)))
                    .collect()
            }
            _ => self.selected_cells.iter().copied().collect(),
        }
    }

    /// The selected rows, in ascending order.
    ///
    /// A row is listed **once** even when several of its cells are selected. In cell mode the
    /// selection is a set of cells, so projecting it to rows used to repeat a row for every
    /// selected column; a host processing the returned rows hit the same row multiple times. The
    /// cell projection stays available through [`Self::selected_cells`].
    pub fn selected_rows(&self) -> Vec<usize> {
        match self.selection_mode {
            GridTableSelectionMode::Row => self.selected_rows.iter().copied().collect(),
            GridTableSelectionMode::Cell => {
                let mut rows: Vec<usize> =
                    self.selected_cells.iter().map(|(row, _)| *row).collect();
                // `selected_cells` is a `BTreeSet<(row, column)>`, so equal rows are adjacent.
                rows.dedup();
                rows
            }
            GridTableSelectionMode::Column => {
                // A selected column covers every row, which is what `selected_cells` reports too.
                if self.selected_columns.is_empty() {
                    Vec::new()
                } else {
                    (0..self.row_count()).collect()
                }
            }
            GridTableSelectionMode::None => Vec::new(),
        }
    }

    /// Whether `(row, column)` is inside the painted selection.
    fn is_cell_selected(&self, row: usize, column: usize) -> bool {
        let anchor = self.selected_cell;
        match self.selection_mode {
            GridTableSelectionMode::None => false,
            GridTableSelectionMode::Cell => {
                self.selected_cells.contains(&(row, column))
                    || (!self.multi_select && anchor == Some((row, column)))
            }
            GridTableSelectionMode::Row => {
                self.selected_rows.contains(&row)
                    || (!self.multi_select && anchor.map(|(r, _)| r) == Some(row))
            }
            GridTableSelectionMode::Column => {
                self.selected_columns.contains(&column)
                    || (!self.multi_select && anchor.map(|(_, c)| c) == Some(column))
            }
        }
    }

    /// Adds the selection unit under `(row, column)` without touching the rest.
    ///
    /// Returns whether the set actually gained a unit, so the caller can report a change without
    /// snapshotting the whole selection first (V-007).
    fn push_selection_unit(&mut self, row: usize, column: usize) -> bool {
        match self.selection_mode {
            GridTableSelectionMode::None => false,
            GridTableSelectionMode::Cell => self.selected_cells.insert((row, column)),
            GridTableSelectionMode::Row => self.selected_rows.insert(row),
            GridTableSelectionMode::Column => self.selected_columns.insert(column),
        }
    }

    /// Toggles the selection unit under `(row, column)`.
    fn toggle_selection_unit(&mut self, row: usize, column: usize) {
        match self.selection_mode {
            GridTableSelectionMode::None => {}
            GridTableSelectionMode::Cell => {
                if !self.selected_cells.remove(&(row, column)) {
                    self.selected_cells.insert((row, column));
                }
            }
            GridTableSelectionMode::Row => {
                if !self.selected_rows.remove(&row) {
                    self.selected_rows.insert(row);
                }
            }
            GridTableSelectionMode::Column => {
                if !self.selected_columns.remove(&column) {
                    self.selected_columns.insert(column);
                }
            }
        }
    }

    /// Adds every unit between the anchor and `(row, column)`.
    ///
    /// Returns whether any unit was new, which is what lets `apply_selection` decide whether to
    /// emit without cloning the previous selection (V-007).
    fn push_selection_range(
        &mut self,
        anchor_row: usize,
        anchor_column: usize,
        row: usize,
        column: usize,
    ) -> bool {
        let mut changed = false;
        match self.selection_mode {
            GridTableSelectionMode::None => {}
            GridTableSelectionMode::Cell => {
                let rows = anchor_row.min(row)..=anchor_row.max(row);
                let columns = anchor_column.min(column)..=anchor_column.max(column);
                for r in rows {
                    for c in columns.clone() {
                        changed |= self.selected_cells.insert((r, c));
                    }
                }
            }
            GridTableSelectionMode::Row => {
                for r in anchor_row.min(row)..=anchor_row.max(row) {
                    changed |= self.selected_rows.insert(r);
                }
            }
            GridTableSelectionMode::Column => {
                for c in anchor_column.min(column)..=anchor_column.max(column) {
                    changed |= self.selected_columns.insert(c);
                }
            }
        }
        changed
    }

    /// Whether the selection currently consists of exactly `(row, column)`'s unit.
    ///
    /// The comparison is done against the live sets instead of a cloned snapshot. This is the
    /// predicate a replace-click needs: if the unit is already the only thing selected, the click
    /// changes nothing and must not emit (V-007).
    fn selection_is_exactly(&self, row: usize, column: usize) -> bool {
        match self.selection_mode {
            GridTableSelectionMode::None => true,
            GridTableSelectionMode::Cell => {
                self.selected_cells.len() == 1
                    && self.selected_cells.contains(&(row, column))
                    && self.selected_rows.is_empty()
                    && self.selected_columns.is_empty()
            }
            GridTableSelectionMode::Row => {
                self.selected_rows.len() == 1
                    && self.selected_rows.contains(&row)
                    && self.selected_cells.is_empty()
                    && self.selected_columns.is_empty()
            }
            GridTableSelectionMode::Column => {
                self.selected_columns.len() == 1
                    && self.selected_columns.contains(&column)
                    && self.selected_cells.is_empty()
                    && self.selected_rows.is_empty()
            }
        }
    }

    /// Applies a pointer click to the selection, honouring ctrl/shift when extended
    /// selection is enabled. Without extended selection a click replaces the selection.
    fn apply_selection(&mut self, row: usize, column: usize, modifiers: u32) {
        use crate::shortcut::Modifiers;
        if self.selection_mode == GridTableSelectionMode::None {
            return;
        }
        let modifiers = Modifiers::from_event_bits(modifiers);
        let toggle = self.multi_select
            && (modifiers.contains(Modifiers::CTRL) || modifiers.contains(Modifiers::META));
        let extend = self.multi_select && modifiers.contains(Modifiers::SHIFT);

        // Change detection reads the live sets. The old form cloned all three sets (and the
        // anchor) on both sides of the mutation — four full copies per mouse press (V-007).
        let changed = if !self.multi_select || (!toggle && !extend) {
            let changed = self.selected_cell != Some((row, column))
                || !self.selection_is_exactly(row, column);
            self.selected_cells.clear();
            self.selected_rows.clear();
            self.selected_columns.clear();
            self.selected_cell = Some((row, column));
            let _ = self.push_selection_unit(row, column);
            changed
        } else if extend {
            match self.selected_cell {
                Some((anchor_row, anchor_column)) => {
                    self.push_selection_range(anchor_row, anchor_column, row, column)
                }
                None => {
                    self.selected_cell = Some((row, column));
                    self.push_selection_unit(row, column)
                }
            }
        } else {
            // A toggle always changes membership: either the unit was removed or it was added.
            self.toggle_selection_unit(row, column);
            self.selected_cell = Some((row, column));
            true
        };
        if changed {
            self.cell_selected.emit((row, column));
            self.base.request_redraw();
        }
    }

    /// Moves the selection to the next row whose first cell begins with `character`.
    ///
    /// The search starts after the current row and wraps once, which is the type-ahead
    /// behaviour a keyboard user expects from a table. Returns whether a row matched.
    fn type_ahead(&mut self, character: char) -> bool {
        let Some(source) = self.data_source.clone() else {
            return false;
        };
        let rows = self.row_count();
        if rows == 0 {
            return false;
        }
        let start = self.selected_cell.map(|(row, _)| row.saturating_add(1)).unwrap_or(0);
        let needle = character.to_ascii_lowercase();
        let mut text = String::new();
        let mut found = None;
        for step in 0..rows {
            let row = (start + step) % rows;
            text.clear();
            if source.fill_cell(self.source_row(row), 0, &mut text)
                && text.to_lowercase().starts_with(needle)
            {
                found = Some(row);
                break;
            }
        }
        let Some(row) = found else {
            return false;
        };
        let column = self.selected_cell.map(|(_, column)| column).unwrap_or(0);
        self.apply_selection(row, column, 0);
        if row < self.scroll_row || row >= self.scroll_row + self.visible_rows.max(1) {
            self.set_scroll_row(row);
        }
        true
    }

    /// Gestures that request an in-place edit (V-01).
    ///
    /// The default is [`TableEditTriggers::ALL`], which is the historical behaviour (a
    /// double-click and Enter both requested an edit). A host that wants "double-click selects,
    /// F2 edits" clears the triggers it does not want.
    pub fn edit_triggers(&self) -> TableEditTriggers {
        self.edit_triggers
    }

    /// Sets which gestures may request an edit; see [`Self::edit_triggers`].
    pub fn set_edit_triggers(&mut self, triggers: TableEditTriggers) {
        self.edit_triggers = triggers;
    }

    /// Moves the current cell by `(row_delta, column_delta)` and keeps it on screen (V-02).
    ///
    /// The arrow keys used to scroll the viewport without moving the current cell, so a keyboard
    /// user could never select anything but the row the pointer had last touched. The cell is
    /// clamped to the model, the selection is applied through the same `apply_selection` the
    /// pointer path uses, and the viewport follows the moved cell.
    fn move_current_cell(&mut self, row_delta: isize, column_delta: isize, modifiers: u32) {
        let rows = self.row_count();
        let columns = self.column_count();
        if rows == 0 || columns == 0 {
            return;
        }
        let (row, column) = self.selected_cell.unwrap_or((0, 0));
        let next_row = (row as isize + row_delta).clamp(0, rows as isize - 1) as usize;
        let next_column = (column as isize + column_delta).clamp(0, columns as isize - 1) as usize;
        self.apply_selection(next_row, next_column, modifiers);
        self.ensure_cell_visible(next_row, next_column);
    }

    /// Scrolls the smallest amount that puts `(row, column)` inside the viewport.
    fn ensure_cell_visible(&mut self, row: usize, column: usize) {
        let visible_rows = self.visible_rows.max(1);
        if row < self.scroll_row {
            self.set_scroll_row(row);
        } else if row >= self.scroll_row.saturating_add(visible_rows) {
            self.set_scroll_row(row.saturating_add(1).saturating_sub(visible_rows));
        }
        let rect = self.base.geometry();
        let extent = (rect.width as i32 - self.row_number_width as i32).max(1);
        let visible = self.visible_column_spans(extent);
        if !visible.iter().any(|(candidate, _, _)| *candidate == column) {
            // Touch the boundary: the column becomes the leading scrolling column. Scrolling
            // forward by one would need a per-column pass just to reach the same place.
            self.set_scroll_column(column);
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

    // -----------------------------------------------------------------------
    // Column widths
    // -----------------------------------------------------------------------

    /// Ensures the internal column-widths vector has at least `count` entries,
    /// each defaulting to a reasonable initial width.
    fn ensure_column_widths(&mut self, count: usize) {
        if self.column_widths.len() < count {
            let default_w = self.default_column_width();
            self.column_widths.resize(count, default_w);
            self.column_width_revision = self.column_width_revision.wrapping_add(1);
        }
    }

    /// Sets the width of a specific column (clamped to minimum).
    pub fn set_column_width(&mut self, col: usize, width: u32) {
        let cols = self.column_count();
        if col >= cols {
            return;
        }
        self.ensure_column_widths(cols);
        self.column_widths[col] = width.max(self.min_column_width);
        self.column_width_revision = self.column_width_revision.wrapping_add(1);
        self.update_visibility();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns the width of a specific column, or a default if unset.
    pub fn column_width(&self, col: usize) -> u32 {
        // The floor is applied on the way *out*, so a width that entered the vector below
        // `min_column_width` — or a `min_column_width` raised after the fact — is still reported as the
        // column the painters, the hit tests and the fit walk all agree on.
        self.column_widths
            .get(col)
            .copied()
            .unwrap_or_else(|| self.default_column_width())
            .max(self.min_column_width)
    }

    /// The alignment column `col` paints its cells with (V-001).
    pub fn column_alignment(&self, col: usize) -> HorizontalAlignment {
        self.column_alignments.get(&col).copied().unwrap_or(HorizontalAlignment::Left)
    }

    /// Sets one column's cell alignment, returning whether the column exists (V-001).
    pub fn set_column_alignment(&mut self, col: usize, alignment: HorizontalAlignment) -> bool {
        if col >= self.column_count() {
            return false;
        }
        if alignment == HorizontalAlignment::Left {
            self.column_alignments.remove(&col);
        } else {
            self.column_alignments.insert(col, alignment);
        }
        self.base.request_redraw();
        true
    }

    // -----------------------------------------------------------------------
    // Sorting
    // -----------------------------------------------------------------------

    /// Toggles sort for the given column. If the column is already the primary
    /// sort, the direction is flipped. Otherwise it becomes the sole sort spec
    /// with ascending order.
    ///
    /// The stored specs are not a decoration: `Self::ensure_sort_order` materialises a projected
    /// row -> source row map from them, and every read path resolves a row through it. The signal
    /// is emitted after the map is rebuilt, so a subscriber cannot observe "sorted" before the
    /// rows actually move.
    pub fn toggle_sort_column(&mut self, col: usize) {
        if !self.sorting_enabled {
            return;
        }
        let cols = self.column_count();
        if col >= cols {
            return;
        }

        if let Some(existing) = self.sort_specs.iter_mut().find(|s| s.column == col) {
            existing.descending = !existing.descending;
            let desc = existing.descending;
            self.sort_order = None;
            self.sort_order_key = None;
            self.ensure_sort_order();
            self.sort_changed.emit((col, desc));
        } else {
            self.sort_specs.clear();
            self.sort_specs.push(GridTableSortSpec { column: col, descending: false });
            self.sort_order = None;
            self.sort_order_key = None;
            self.ensure_sort_order();
            self.sort_changed.emit((col, false));
        }
        self.base.request_redraw();
    }

    /// Whether a header click is allowed to change the sort order.
    pub fn sorting_enabled(&self) -> bool {
        self.sorting_enabled
    }

    /// Enables or disables header-driven sorting.
    ///
    /// When off, [`Self::toggle_sort_column`] is a no-op and the current specs are cleared, so the
    /// header arrow cannot claim an order the rows do not have. [`Self::apply_sort`] remains a safe
    /// call: it rebuilds the identity projection, which is what "not sorted" means.
    pub fn set_sorting_enabled(&mut self, enabled: bool) {
        if self.sorting_enabled == enabled {
            return;
        }
        self.sorting_enabled = enabled;
        if !enabled {
            // The arrow is derived from the specs; keeping a spec that is not applied would show
            // a sorted header over unsorted rows.
            self.sort_specs.clear();
        }
        self.sort_order = None;
        self.sort_order_key = None;
        self.base.request_redraw();
    }

    /// The role sort keys are read with.
    pub fn sort_role(&self) -> DataRole {
        self.sort_role
    }

    /// Sets the role sort keys are read with.
    ///
    /// A model that stores a number in a display role still sorts numerically because the
    /// comparison parses a numeric key when both sides are numbers; asking for `Edit` or a
    /// `User` role lets a model expose a sort-only representation instead.
    pub fn set_sort_role(&mut self, role: DataRole) {
        if self.sort_role == role {
            return;
        }
        self.sort_role = role;
        self.sort_order = None;
        self.sort_order_key = None;
        self.base.request_redraw();
    }

    /// Rebuilds the projected row order from the current sort specs.
    ///
    /// Public because a model can change in place behind a source that only bumps `revision()`;
    /// the next data read would notice, but a host that has just written cells can force the
    /// projection immediately.
    pub fn apply_sort(&mut self) {
        self.sort_order = None;
        self.sort_order_key = None;
        self.ensure_sort_order();
        self.base.request_redraw();
    }

    /// The source row painted at projected row `row`.
    fn source_row(&self, row: usize) -> usize {
        self.sort_order.as_ref().and_then(|order| order.get(row).copied()).unwrap_or(row)
    }

    /// Whether the cached order was built from the current revision/generation/row count/role.
    ///
    /// Deliberately allocation-free: this runs from every pointer event, so building a key vector
    /// merely to compare it would make a mouse move allocate.
    fn sort_order_inputs_match(&self, revision: u64, generation: u64, rows: usize) -> bool {
        let Some(key) = self.sort_order_key.as_ref() else {
            return false;
        };
        if key.revision != revision
            || key.generation != generation
            || key.rows != rows
            || key.role != self.sort_role
        {
            return false;
        }
        let columns = self.column_count();
        let mut expected = self.sort_specs.iter().filter(|spec| spec.column < columns);
        let mut stored = key.specs.iter();
        loop {
            match (expected.next(), stored.next()) {
                (Some(spec), Some((column, descending))) => {
                    if spec.column != *column || spec.descending != *descending {
                        return false;
                    }
                }
                (None, None) => break,
                _ => return false,
            }
        }
        self.sort_order.as_ref().map(|order| order.len()).unwrap_or(rows) == rows
    }

    /// Materialises the projected row order when one of its inputs changed.
    fn ensure_sort_order(&mut self) {
        let revision = self.data_source.as_ref().map(|source| source.revision()).unwrap_or(0);
        let generation = self.data_generation.load(std::sync::atomic::Ordering::Relaxed);
        let rows = self.row_count();
        if self.sort_order_inputs_match(revision, generation, rows) {
            return;
        }

        let columns = self.column_count();
        let specs: Vec<(usize, bool)> = self
            .sort_specs
            .iter()
            .filter(|spec| spec.column < columns)
            .map(|spec| (spec.column, spec.descending))
            .collect();
        let key = GridSortOrderKey { revision, generation, rows, specs, role: self.sort_role };
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
            let mut per_row = Vec::with_capacity(key.rows);
            for row in 0..key.rows {
                let raw = source
                    .data_role(row, column, self.sort_role)
                    .or_else(|| source.data(row, column))
                    .unwrap_or_default();
                per_row.push(normalise_sort_key(&raw));
            }
            keys.push(per_row);
        }

        let mut order: Vec<usize> = (0..key.rows).collect();
        order.sort_by(|&left, &right| {
            for (index, spec) in key.specs.iter().enumerate() {
                let ordering = compare_sort_keys(&keys[index][left], &keys[index][right]);
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

    /// Returns a reference to the current sort specs.
    pub fn sort_specs(&self) -> &[GridTableSortSpec] {
        &self.sort_specs
    }

    // -----------------------------------------------------------------------
    // Selection
    // -----------------------------------------------------------------------

    /// Returns the currently selected cell, if any.
    pub fn selected_cell(&self) -> Option<(usize, usize)> {
        self.selected_cell
    }

    /// Clears the current selection.
    pub fn clear_selection(&mut self) {
        if self.selected_cell.is_some()
            || !self.selected_cells.is_empty()
            || !self.selected_rows.is_empty()
            || !self.selected_columns.is_empty()
        {
            self.selected_cell = None;
            self.selected_cells.clear();
            self.selected_rows.clear();
            self.selected_columns.clear();
            self.base.request_redraw();
        }
    }

    // -----------------------------------------------------------------------
    // Visibility helpers
    // -----------------------------------------------------------------------

    /// Recalculates how many rows and columns fit in the current viewport.
    ///
    /// # Why the walks below are the *only* place the fit is decided
    ///
    /// Five paths need to know where a column starts or ends: the header painter, the data-cell
    /// painter, the pointer hit test, the header hit test and the resize-handle hit test. Each used to
    /// walk the widths itself, with its own copy of the default width, its own `min_column_width`
    /// clamp and its own stopping rule — so "which column is under this pixel" was answered five
    /// slightly different ways. They agree only while all five are edited together, which is exactly
    /// the arrangement that put a resize handle under the neighbouring column once before.
    ///
    /// [`Self::column_span_at`] and [`Self::column_span_at_x`] are now the one derivation, and the
    /// five consumers name the offset they have.
    fn update_visibility(&mut self) {
        let rect = self.base.geometry();
        let rh = self.row_height.max(1) as i32;
        let header_h = self.header_height as i32;

        let data_h = (rect.height as i32).saturating_sub(header_h).max(0);

        self.visible_rows = if rh > 0 { (data_h / rh) as usize } else { 0 };

        // The column count is still ensured here: the table learns it here so the paint
        // loop can ask for each column's span. The former `visible_columns` count was
        // computed from a separate accumulation and never read by any drawing path (the
        // columns are bounded by the spans the paint loop walks), so it was removed
        // rather than kept in step with a second rule.
        let cols = self.column_count();
        self.ensure_column_widths(cols);

        self.visible_rows = self.visible_rows.max(1);
    }

    // -----------------------------------------------------------------------
    // Column geometry — one derivation, five consumers
    // -----------------------------------------------------------------------

    /// The width every column falls back to when the caller has not sized it.
    ///
    /// Both the default and the floor are the same fact — a column is never narrower than
    /// [`Self::min_column_width`] and its unsized width is at least 120 — and they were spelled
    /// separately in four places. Read through [`Self::column_width`] by every walk.
    fn default_column_width(&self) -> u32 {
        match self.section_resize_mode {
            GridSectionResizeMode::Stretch => {
                let columns = self.column_count().max(1) as u32;
                let usable =
                    self.geometry().width.saturating_sub(self.row_number_width).max(columns);
                (usable / columns).max(self.min_column_width)
            }
            GridSectionResizeMode::Fixed => self.min_column_width,
            GridSectionResizeMode::Interactive | GridSectionResizeMode::ResizeToContents => {
                120u32.max(self.min_column_width)
            }
        }
    }

    /// How column widths are decided.
    pub fn section_resize_mode(&self) -> GridSectionResizeMode {
        self.section_resize_mode
    }

    /// Sets the column resize mode, discarding explicit widths.
    ///
    /// A mode change is a statement about how every column is sized, so keeping the old
    /// per-column widths would make the mode apply to none of them.
    pub fn set_section_resize_mode(&mut self, mode: GridSectionResizeMode) {
        if self.section_resize_mode == mode {
            return;
        }
        self.section_resize_mode = mode;
        self.column_widths.clear();
        self.column_width_revision = self.column_width_revision.wrapping_add(1);
        self.update_visibility();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Whether odd rows receive the alternating wash.
    pub fn alternating_rows(&self) -> bool {
        self.alternating_rows
    }

    /// The height of the column-title strip.
    pub fn header_height(&self) -> u32 {
        self.header_height
    }

    /// Sets the height of the column-title strip.
    ///
    /// A caller that wants the heavier header a file browser uses raises this; the default
    /// keeps the historical 28 px strip.
    pub fn set_header_height(&mut self, height: u32) {
        let height = height.max(12);
        if self.header_height == height {
            return;
        }
        self.header_height = height;
        self.update_visibility();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Enables or disables the alternating row wash.
    pub fn set_alternating_rows(&mut self, alternating: bool) {
        if self.alternating_rows == alternating {
            return;
        }
        self.alternating_rows = alternating;
        self.base.request_redraw();
    }

    /// The columns that fit in the data area, in draw order: `(column, x, width)` with `x` **relative
    /// to the data area's left edge**.
    ///
    /// # Why the extent is a parameter and not `geometry()`
    ///
    /// The two callers measure room differently, and both are right:
    ///
    /// * a painter asks "how much room from my own left edge to the control's far edge", because every
    ///   run it draws starts at `scroll_column`; a horizontally scrolled table therefore has a *smaller*
    ///   budget, not a shifted one;
    /// * a hit test asks "how far is this pixel into the data area", which is the same extent measured
    ///   from the same edge — the pointer's x *is* the offset.
    ///
    /// Passing the extent keeps one loop for both instead of a "fit" copy and a "hit" copy that can
    /// disagree about the last column.
    ///
    /// # Why the containment test is on the *start*, not the end
    ///
    /// A column is in the run when it **begins** inside the extent, even if it ends past it. Testing
    /// `x + width <= extent` instead would drop the last column as soon as it was partly scrolled out —
    /// and for a pointer test that is fatal: [`Self::column_span_at_x`] passes the pointer's own offset
    /// as the extent, so a 100 px column would be "not in the run" for every offset below 100 and the
    /// first column would stop answering at all. The painter wants the same rule: a column that starts
    /// on screen is drawn and clipped, not skipped.
    fn visible_column_spans(&self, extent: i32) -> Vec<(usize, i32, i32)> {
        let count = self.column_count();
        let rect = self.base.geometry();
        let data_width = (rect.width as i32 - self.row_number_width as i32).max(1);
        let build_extent = extent.max(data_width).max(0);
        let mut cache = self.span_cache.borrow_mut();
        let stale = match cache.as_ref() {
            Some(entry) => {
                entry.scroll_column != self.scroll_column
                    || entry.column_count != count
                    || entry.width_revision != self.column_width_revision
                    || entry.data_width < build_extent as u32
                    || entry.default_width != self.default_column_width()
                    || entry.min_width != self.min_column_width
                    || entry.mode != self.section_resize_mode
            }
            None => true,
        };
        if stale {
            let mut spans = Vec::new();
            let mut x = 0i32;
            for ci in self.scroll_column..count {
                if x >= build_extent {
                    break;
                }
                let width = self.column_width(ci) as i32;
                spans.push((ci, x, width));
                x = x.saturating_add(width);
            }
            *cache = Some(ColumnSpanCache {
                scroll_column: self.scroll_column,
                column_count: count,
                width_revision: self.column_width_revision,
                data_width: build_extent as u32,
                default_width: self.default_column_width(),
                min_width: self.min_column_width,
                mode: self.section_resize_mode,
                spans,
            });
        }
        cache
            .as_ref()
            .expect("the span run was just built")
            .spans
            .iter()
            .copied()
            .filter(|(_, x, _)| *x < extent)
            .collect()
    }

    /// The data column occupying `x_offset` pixels into the data area, as `(column, x, width)`.
    ///
    /// The counterpart of [`Self::visible_column_spans`] for a *point*: the extent covers exactly the
    /// pixels up to and including the pointer, and the span containing it is returned.
    ///
    /// Containment is tested here, on the finished spans, rather than inside the walk. The walk asks
    /// "does this column *start* before the pointer", which is the right question for building the run
    /// but not for picking a cell: it would return the last *begun* column for a pointer sitting in a
    /// gap or past every column. Selecting the containing span means "the cell you clicked" and "the
    /// cell you can see" are the same run, which is what the old, separately-accumulated copy in the
    /// hit test could not guarantee.
    fn column_span_at_x(&self, x_offset: i32) -> Option<(usize, i32, i32)> {
        if x_offset < 0 {
            return None;
        }
        self.visible_column_spans(x_offset.saturating_add(1))
            .into_iter()
            .find(|(_, x, w)| x_offset >= *x && x_offset < x + w)
    }

    // -----------------------------------------------------------------------
    // Hit-testing helpers
    // -----------------------------------------------------------------------

    /// Returns the cell `(row, column)` at the given point, or `None` if the
    /// point falls outside the data area (including headers and row-number gutters).
    fn cell_at_point(&self, point: Point) -> Option<(usize, usize)> {
        let rect = self.base.geometry();
        let rnw = self.row_number_width as i32;
        let header_h = self.header_height as i32;

        // The data area starts below the header and to the right of the row-number gutter
        let data_x = rect.x + rnw;
        let data_y = rect.y + header_h;

        if point.x < data_x || point.y < data_y {
            return None;
        }

        let col_idx = self.horizontal_column_at(point.x - data_x)?;
        let row_idx = {
            let offset = point.y - data_y;
            let rh = self.row_height.max(1) as i32;
            if offset < 0 || rh <= 0 {
                return None;
            }
            let idx = (offset / rh) as usize;
            if idx >= self.visible_rows {
                return None;
            }
            idx
        };

        let abs_row = self.scroll_row + row_idx;
        let abs_col = self.scroll_column + col_idx;

        if abs_row >= self.row_count() || abs_col >= self.column_count() {
            return None;
        }

        Some((abs_row, abs_col))
    }

    /// Given an x-offset from the left edge of the data area (after row-number gutter),
    /// returns which visible column index (within the current scroll window) is hit,
    /// or `None` if past the last column.
    fn horizontal_column_at(&self, x_offset: i32) -> Option<usize> {
        let (cardinal, _, _) = self.column_span_at_x(x_offset)?;
        Some(cardinal - self.scroll_column)
    }

    /// Returns the column index of the header at the given point, or `None`.
    fn header_at_point(&self, point: Point) -> Option<usize> {
        let rect = self.base.geometry();
        let rnw = self.row_number_width as i32;
        let header_h = self.header_height as i32;

        if point.y < rect.y || point.y >= rect.y + header_h {
            return None;
        }
        let x_offset = point.x - rect.x - rnw;
        self.horizontal_column_at(x_offset)
    }

    /// Returns the column whose right-edge resize handle is at `point`, or `None`.
    ///
    /// # Why this reads the spans instead of accumulating widths
    ///
    /// The handle straddles a boundary: it is the `HANDLE_WIDTH` pixels either side of a column's right
    /// edge. Accumulating widths here and accumulating them again in the painter is how the handle and
    /// the line it sits on came apart — the boundary is whichever `x + width` the spans document, so
    /// asking *them* for it makes the two the same number.
    fn resize_handle_at_point(&self, point: Point) -> Option<usize> {
        let rect = self.base.geometry();
        let rnw = self.row_number_width as i32;
        let header_h = self.header_height as i32;

        if point.y < rect.y || point.y >= rect.y + header_h {
            return None;
        }

        let extent = (rect.x + rect.width as i32) - (rect.x + rnw);
        let handle_width = Self::RESIZE_HANDLE_WIDTH as i32;
        for (ci, x, cw) in self.visible_column_spans(extent) {
            if (point.x - (rect.x + rnw + x + cw)).abs() <= handle_width {
                return Some(ci);
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// Widget trait
// ---------------------------------------------------------------------------

impl Widget for GridTableWidget {
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
        let rows = self.row_count();
        let cols = self.column_count();
        let w = self
            .column_widths
            .iter()
            .take(cols)
            .copied()
            .sum::<u32>()
            .max(self.row_number_width)
            .max(self.min_column_width);
        let h = (rows as u32).saturating_mul(self.row_height).saturating_add(self.header_height);
        Size::new(w, h)
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
            "cell_selected" => {
                Some(EventSignalRef::mapped("cell_selected", &self.cell_selected, |v| {
                    CapabilityValue::Tuple(alloc::vec![
                        CapabilityValue::UInt(v.0 as u64),
                        CapabilityValue::UInt(v.1 as u64),
                    ])
                }))
            }
            "cell_double_clicked" => Some(EventSignalRef::mapped(
                "cell_double_clicked",
                &self.cell_double_clicked,
                |v| {
                    CapabilityValue::Tuple(alloc::vec![
                        CapabilityValue::UInt(v.0 as u64),
                        CapabilityValue::UInt(v.1 as u64),
                    ])
                },
            )),
            "sort_changed" => {
                Some(EventSignalRef::mapped("sort_changed", &self.sort_changed, |v| {
                    CapabilityValue::Tuple(alloc::vec![
                        CapabilityValue::UInt(v.0 as u64),
                        CapabilityValue::Bool(v.1),
                    ])
                }))
            }
            "header_clicked" => {
                Some(EventSignalRef::mapped("header_clicked", &self.header_clicked, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            "context_menu_requested" => Some(EventSignalRef::mapped(
                "context_menu_requested",
                &self.context_menu_requested,
                |v| crate::widget::capability::CapabilityValue::String(format!("{},{}", v.x, v.y)),
            )),
            "edit_requested" => {
                Some(EventSignalRef::mapped("edit_requested", &self.edit_requested, |v| {
                    crate::widget::capability::CapabilityValue::Tuple(crate::compat::Vec::from([
                        crate::widget::capability::CapabilityValue::Int(v.0 as i64),
                        crate::widget::capability::CapabilityValue::Int(v.1 as i64),
                    ]))
                }))
            }
            _ => None,
        }
    }
}

/// `GridTableWidget`'s property contract.
///
/// `selection_mode` publishes the shared lower-case tokens (`none`, `cell`,
/// `row`, `column`) rather than the type's `Debug` spelling, so a consumer can
/// treat it exactly like every other selection mode in the library. The counts
/// are derived from the data source and stay read-only.
impl WidgetProperties for GridTableWidget {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "has_data_source" => Ok(CapabilityValue::Bool(self.data_source.is_some())),
            "row_count" => Ok(CapabilityValue::UInt(self.row_count() as u64)),
            "column_count" => Ok(CapabilityValue::UInt(self.column_count() as u64)),
            "scroll_row" => Ok(CapabilityValue::UInt(self.scroll_row() as u64)),
            "scroll_column" => Ok(CapabilityValue::UInt(self.scroll_column() as u64)),
            "row_height" => Ok(CapabilityValue::UInt(self.row_height as u64)),
            "selection_mode" => Ok(CapabilityValue::String(
                grid_table_selection_mode_to_str(self.selection_mode).to_string(),
            )),
            "sort_spec_count" => Ok(CapabilityValue::UInt(self.sort_specs().len() as u64)),
            "sorting_enabled" => Ok(CapabilityValue::Bool(self.sorting_enabled())),
            "sort_role" => Ok(CapabilityValue::String(data_role_to_str(self.sort_role()))),
            "auto_refresh" => Ok(CapabilityValue::Bool(self.auto_refresh())),
            "selected_cell" => match self.selected_cell() {
                Some((row, column)) => Ok(CapabilityValue::String(format!("{row},{column}"))),
                None => Ok(CapabilityValue::Null),
            },
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
            "row_height" => match value {
                CapabilityValue::UInt(height) => {
                    let height =
                        u32::try_from(height).map_err(|_| CapabilityAccessError::TypeMismatch)?;
                    self.set_row_height(height);
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "selection_mode" => {
                self.set_selection_mode(expect_grid_table_selection_mode(value)?);
                Ok(())
            }
            "sorting_enabled" => {
                self.set_sorting_enabled(expect_bool(value)?);
                Ok(())
            }
            "auto_refresh" => {
                self.set_auto_refresh(expect_bool(value)?);
                Ok(())
            }
            "sort_role" => {
                self.set_sort_role(expect_data_role(value)?);
                Ok(())
            }
            // Counts and the selected cell are derived from the data source and
            // the live selection; there is no meaningful assignment for them.
            "has_data_source" | "row_count" | "column_count" | "sort_spec_count"
            | "selected_cell" => Err(CapabilityAccessError::ReadOnlyProperty),
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
            "selection_mode",
            "sort_spec_count",
            "selected_cell",
            "sorting_enabled",
            "sort_role",
            "auto_refresh",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `grid_table` publishes.
    ///
    /// `clear_data_source` and `clear_selection` discard state and need no argument,
    /// so a bare invocation performs them — both are already no-ops on an unbound or
    /// unselected table, which is the correct reading of "clear" either way.
    /// `toggle_sort_column` names which column to sort and is therefore
    /// [`CapabilityAccessError::OutOfRange`]: the name is valid and the column index
    /// is what is missing. `set_data_source`, `set_scroll_row` and `set_scroll_column`
    /// carry their own payloads for the same reason.
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
            "toggle_sort_column" | "set_data_source" | "set_scroll_row" | "set_scroll_column" => {
                Err(CapabilityAccessError::OutOfRange)
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

/// Publishes `GridTableSelectionMode` as the shared lower-case token.
fn grid_table_selection_mode_to_str(mode: GridTableSelectionMode) -> &'static str {
    match mode {
        GridTableSelectionMode::None => "none",
        GridTableSelectionMode::Cell => "cell",
        GridTableSelectionMode::Row => "row",
        GridTableSelectionMode::Column => "column",
    }
}

/// Parses the shared lower-case token back, rejecting anything else.
fn expect_grid_table_selection_mode(
    value: CapabilityValue,
) -> Result<GridTableSelectionMode, CapabilityAccessError> {
    match value {
        CapabilityValue::String(token) => match token.as_str() {
            "none" => Ok(GridTableSelectionMode::None),
            "cell" => Ok(GridTableSelectionMode::Cell),
            "row" => Ok(GridTableSelectionMode::Row),
            "column" => Ok(GridTableSelectionMode::Column),
            _ => Err(CapabilityAccessError::TypeMismatch),
        },
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

/// The published spelling of a [`DataRole`].
fn data_role_to_str(role: DataRole) -> String {
    match role {
        DataRole::Display => "display".to_string(),
        DataRole::AccessibleText => "accessible_text".to_string(),
        DataRole::Edit => "edit".to_string(),
        DataRole::Decoration => "decoration".to_string(),
        DataRole::CheckState => "check_state".to_string(),
        DataRole::User(id) => format!("user:{id}"),
    }
}

/// Parses a published [`DataRole`] token.
fn expect_data_role(value: CapabilityValue) -> Result<DataRole, CapabilityAccessError> {
    match value {
        CapabilityValue::String(token) => match token.as_str() {
            "display" => Ok(DataRole::Display),
            "accessible_text" => Ok(DataRole::AccessibleText),
            "edit" => Ok(DataRole::Edit),
            "decoration" => Ok(DataRole::Decoration),
            "check_state" => Ok(DataRole::CheckState),
            other => other
                .strip_prefix("user:")
                .and_then(|id| id.parse::<u32>().ok())
                .map(DataRole::User)
                .ok_or(CapabilityAccessError::TypeMismatch),
        },
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

// ---------------------------------------------------------------------------
// Draw trait
// ---------------------------------------------------------------------------

impl Draw for GridTableWidget {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then a literal. The theme step is what makes an appearance
        // switch visible; the surface, the header, the row numbers, the grid lines and all
        // four text colours used to be hardcoded literals, so light and dark rendered
        // identically.
        //
        // The theme read takes and releases the global manager's lock internally, and the palette
        // comes from the shared snapshot, so no guard is held across the draw (the mutex is not
        // re-entrant).
        // `WidgetStyle` carries a font-family `String` and a gradient `Vec`; cloning it to read
        // two `Copy` colour fields allocated both on every frame (WDGT-08). Copy the fields
        // under one short borrow instead.
        let (background_color, text_color) = {
            let style = self.base.style();
            (style.background_color, style.text_color)
        };
        let theme = crate::style::resolved_theme_style_shared("grid_table");
        let mut surface = background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let ink = text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        // `grid_table` is absent from `WidgetRole::for_kind_name`'s table, so it classifies as
        // `Surface` and resolves to `theme.colors.background` — the window's own fill. A panel
        // painted in that colour would be byte-identical to the frame behind it, so a resolved
        // surface equal to the window fill is re-derived a visible step away from it, the same
        // distinction `Colors::input_background` draws for a field.
        // One palette snapshot read for the window fill and the accent below.
        let palette = crate::style::palette_shared();
        let window_fill = palette.as_ref().map(|colors| colors.background).unwrap_or(Color::WHITE);
        if surface == window_fill {
            surface = window_fill.blend(&ink, 0.08);
        }
        // The accent is the theme's `primary`, used both for the selection fill and for the
        // resting header text; the surface and the ink supply everything else.
        let accent = palette.as_ref().map(|colors| colors.primary).unwrap_or(Color::PRIMARY);
        // Each cached appearance field is used only while it still holds its constructor
        // literal, i.e. while no caller set it explicitly. The explicit setters keep their
        // meaning; only the untouched default now follows the appearance.
        let grid_color = if self.grid_color == Self::DEFAULT_GRID_COLOR {
            surface.blend(&ink, 0.15)
        } else {
            self.grid_color
        };
        let header_bg = if self.header_bg == Self::DEFAULT_HEADER_BG {
            surface.blend(&ink, 0.06)
        } else {
            self.header_bg
        };
        let header_text_color = if self.header_text_color == Self::DEFAULT_HEADER_TEXT_COLOR {
            ink.blend(&accent, 0.60)
        } else {
            self.header_text_color
        };
        let selected_bg = if self.selected_bg == Self::DEFAULT_SELECTED_BG {
            surface.blend(&accent, 0.30)
        } else {
            self.selected_bg
        };
        // The hover tint is the same accent blend every row family member uses (`table_widget`
        // :722): visible next to the surface, clearly weaker than the selection fill.
        let hovered_bg = surface.blend(&accent, 0.12);
        // A cell's own text is the ink, not a second literal: it used to be a fixed
        // blue-black that vanished on a dark surface.
        let cell_text_color = ink;
        let outer_border = surface.blend(&ink, 0.35);

        // Background
        context.face_with_gradient_corners(
            rect,
            surface,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().resolved_corner_radii().to_array(),
            Color::BLACK,
        );

        let rnw = self.row_number_width as i32;
        let header_h = self.header_height as i32;
        let rh = self.row_height.max(1) as i32;
        let cols = self.column_count();
        let rows = self.row_count();

        // Resolve the projected row order before the paint walks read any cell, so a header click
        // that changed `sort_specs` repaints the rows in their new order rather than only moving
        // the arrow.
        self.ensure_sort_order();
        self.ensure_column_widths(cols);
        self.update_visibility();

        if rows == 0 || cols == 0 {
            // Draw at least the header / row-number area frame
            crate::widget::draw::stroke_surface_border(
                context,
                rect,
                self.style().resolved_corner_radii().to_array(),
                grid_color,
                1,
            );
            return;
        }

        let data_top = rect.y + header_h;
        let data_left = rect.x + rnw;
        // The visible column spans depend only on the control's width and the column widths,
        // not on the row being painted. Computing them per row rebuilt the same `Vec` (and
        // re-summed every width) once per visible row per frame.
        let spans = self.visible_column_spans(rect.x + rect.width as i32 - data_left);
        // The effective font is borrowed from the style, so resolving it once is also what
        // keeps the header and the cell rows on the same face.
        let cell_font = effective_font(self.style());

        // ── Draw column headers ──
        {
            use core::fmt::Write as _;
            let mut header_scratch = String::new();
            let header_rect = Rect::new(rect.x, rect.y, rect.width, self.header_height);
            context.fill_rect(header_rect, header_bg);
            context.draw_rect(header_rect, grid_color);

            let mut hx = data_left;
            for &(ci, _, cw) in &spans {
                let cell_rect = Rect::new(hx, rect.y, cw as u32, self.header_height);
                context.draw_rect(cell_rect, grid_color);

                // Which way this column is sorted, if at all.
                let sort_desc =
                    self.sort_specs.iter().find(|s| s.column == ci).map(|s| s.descending);

                // The header names a *data* column, so the label is derived from the column's
                // real index in the source — the same index the cells below are fetched with —
                // and not from the loop counter, which under a horizontal scroll would name a
                // different column than the one it sits above.
                //
                // # Why the source is asked for a name first
                //
                // `"Column {n}"` is a placeholder, and it was the *only* label this control
                // could produce: the data source protocol had no column-name method, so every
                // grid named its columns by position no matter what it was showing. Two grids
                // over different data were indistinguishable at the header row, and a user
                // reading `"Column 3"` learned nothing they did not already know from the
                // cell's x position. The source is asked first now; the index label survives
                // as the honest fallback for a projection that genuinely has no names (a
                // matrix, a headerless sheet), which is why it is not deleted.
                // The source is bound here rather than reused from an earlier block: the
                // header is drawn by a loop whose scope does not carry that binding, and a
                // re-borrow of `self.data_source` is the same value either way.
                let data_source = self.data_source.as_ref();
                let header_owned;
                let header_text: &str = match data_source.and_then(|source| source.column_name(ci))
                {
                    // A named column shows its name **alone**: the index is the fallback for a
                    // projection that has no names, not a suffix every header carries. Appending it
                    // unconditionally made a named column read "Name 3", which is the placeholder
                    // this control stopped printing once the source could be asked.
                    Some(name) if !name.is_empty() => {
                        header_owned = name;
                        header_owned.as_str()
                    }
                    _ => {
                        // Reused buffer instead of one `format!` allocation per visible
                        // column per frame.
                        header_scratch.clear();
                        let _ = write!(header_scratch, "Column {}", ci + 1);
                        header_scratch.as_str()
                    }
                };
                context.draw_text_fitted_cow(
                    context.text_line(cell_rect, cell_font),
                    header_text,
                    cell_font,
                    header_text_color,
                    HorizontalAlignment::Left,
                );

                // The direction was once a `▼`/`▲` appended to the header text, but no bundled
                // face covers the geometric-shapes block, so both glyphs degraded to an 8x8 bitmap
                // block. An icon is geometry both backends agree on, so it draws the same arrow the
                // text path could not.
                if let Some(desc) = sort_desc {
                    let icon_side = cell_rect.height.min(10);
                    let icon_rect = Rect::new(
                        cell_rect.x + cell_rect.width as i32 - icon_side as i32 - 4,
                        cell_rect.y + (cell_rect.height as i32 - icon_side as i32) / 2,
                        icon_side,
                        icon_side,
                    );
                    let icon_name = if desc {
                        crate::widget::IconName::ArrowDown
                    } else {
                        crate::widget::IconName::ArrowUp
                    };
                    crate::widget::draw_icon_at(context, icon_rect, header_text_color, icon_name);
                }

                hx += cw;
            }
        }

        // ── Draw row numbers ──
        {
            use core::fmt::Write as _;
            let mut row_scratch = String::new();
            for ri in 0..self.visible_rows.min(rows.saturating_sub(self.scroll_row)) {
                let abs_row = self.scroll_row + ri;
                let y = data_top + (ri as i32) * rh;
                if y + rh > rect.y + rect.height as i32 {
                    break;
                }

                let cell_rect = Rect::new(rect.x, y, self.row_number_width, self.row_height);
                context.fill_rect(cell_rect, header_bg);
                context.draw_rect(cell_rect, grid_color);

                // Reused buffer instead of `to_string()` per visible row per frame.
                row_scratch.clear();
                let _ = write!(row_scratch, "{}", abs_row);

                // The row-number cell is the band; centring by hand drew the digits half a line
                // low. Right alignment keeps the numbers flush with the gutter's inner edge.
                context.draw_text_fitted_cow(
                    context.text_line(cell_rect, cell_font),
                    row_scratch.as_str(),
                    cell_font,
                    header_text_color,
                    HorizontalAlignment::Right,
                );
            }
        }

        // ── Draw data cells ──
        let Some(source) = self.data_source.as_ref() else {
            return;
        };

        let mut cy = data_top;
        // The font is resolved once for the whole cell grid rather than twice per cell, and the
        // cell text is written into one reusable buffer: `data()` allocated a `String` per cell
        // per frame, which the fill path avoids for sources that override `fill_cell`.
        let font = cell_font;
        let mut cell_text = String::new();
        for ri in 0..self.visible_rows.min(rows.saturating_sub(self.scroll_row)) {
            let abs_row = self.scroll_row + ri;
            if cy + rh > rect.y + rect.height as i32 {
                break;
            }

            let mut cx = data_left;
            for &(ci, _, cw) in &spans {
                let cell_rect = Rect::new(cx, cy, cw as u32, self.row_height);

                // Selection highlight
                let is_selected = self.is_cell_selected(abs_row, ci);

                if is_selected {
                    context.fill_rect(cell_rect, selected_bg);
                } else if Some(abs_row) == self.hovered_row {
                    context.fill_rect(cell_rect, hovered_bg);
                } else if self.alternating_rows && abs_row % 2 == 1 {
                    context.fill_rect(cell_rect, header_bg);
                }

                // Grid lines
                context.draw_rect(cell_rect, grid_color);

                // Cell text
                cell_text.clear();
                if source.fill_cell(self.source_row(abs_row), ci, &mut cell_text) {
                    // The cell is the band. `cy + rh / 2` as a `draw_text` origin put the glyph
                    // box's *top* edge on the cell's middle line, drawing every value half a line
                    // low; `text_line` derives the centred box, and fitting keeps a long value
                    // inside its column instead of running under the next one.
                    context.draw_text_fitted_cow(
                        context.text_line(cell_rect, font),
                        &cell_text,
                        font,
                        cell_text_color,
                        self.column_alignment(ci),
                    );
                }

                cx += cw;
            }
            cy += rh;
        }

        // ── Outer border ──
        crate::widget::draw::stroke_surface_border(
            context,
            rect,
            self.style().resolved_corner_radii().to_array(),
            outer_border,
            1,
        );

        // The scroll-bar thumb is opt-in: the grid is virtualized, so the wheel and keyboard
        // already reach every row; the thumb is the visible statement of how much is off-screen.
        if self.show_scrollbar {
            let total = self.row_count().max(1);
            let visible = self.visible_rows.max(1).min(total);
            let track = rect.height.max(1);
            let thumb =
                ((track as u64 * visible as u64) / total as u64).max(24).min(track as u64) as u32;
            let travel = track.saturating_sub(thumb);
            let max_scroll = total.saturating_sub(visible);
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

// ---------------------------------------------------------------------------
// EventHandler trait
// ---------------------------------------------------------------------------

impl EventHandler for GridTableWidget {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        // The hit test and the keyboard paths index the *projected* rows, so the sort map has to be
        // current before either resolves a source row.
        self.ensure_sort_order();

        match event {
            Event::MousePress { pos, button, modifiers } => {
                if *button != 1 {
                    return;
                }
                // Check resize handles first
                if let Some(col) = self.resize_handle_at_point(*pos) {
                    if self.section_resize_mode == GridSectionResizeMode::Interactive {
                        self.resizing_column = Some(col);
                    }
                    self.resize_start_x = pos.x;
                    self.resize_start_width = self.column_width(col);
                    return;
                }

                // Check header click for sorting
                if let Some(col) = self.header_at_point(*pos) {
                    self.header_clicked.emit(col);
                    if self.sorting_enabled {
                        self.toggle_sort_column(col);
                    }
                    return;
                }

                // Check cell click for selection
                if let Some((row, col)) = self.cell_at_point(*pos) {
                    self.apply_selection(row, col, *modifiers);
                } else {
                    // Click outside → clear selection
                    if self.selected_cell.is_some() {
                        self.clear_selection();
                    }
                }
            }

            Event::MouseDoubleClick { pos, button: _ } => {
                if let Some((row, col)) = self.cell_at_point(*pos) {
                    self.cell_double_clicked.emit((row, col));
                    if self.edit_triggers.contains(TableEditTriggers::DOUBLE_CLICKED) {
                        self.edit_requested.emit((row, col));
                    }
                }
            }

            Event::ContextMenu { pos } => self.context_menu_requested.emit(*pos),

            Event::MouseMove { pos } => {
                // Column resize dragging
                if let Some(col) = self.resizing_column {
                    let delta = pos.x - self.resize_start_x;
                    let new_width = (self.resize_start_width as i32 + delta)
                        .max(self.min_column_width as i32)
                        as u32;
                    let cols = self.column_count();
                    if col < cols {
                        self.ensure_column_widths(cols);
                        self.column_widths[col] = new_width;
                        self.column_width_revision = self.column_width_revision.wrapping_add(1);
                        self.update_visibility();
                        self.base.request_redraw();
                    }
                } else {
                    // Row hover: read from the same hit test the press arm uses, so the tint
                    // and the click can never disagree about which row the pointer is on.
                    let hovered = self.cell_at_point(*pos).map(|(row, _)| row);
                    if hovered != self.hovered_row {
                        self.hovered_row = hovered;
                        self.base.request_redraw();
                    }
                }
            }

            Event::MouseLeave { .. } => {
                if self.hovered_row.take().is_some() {
                    self.base.request_redraw();
                }
            }

            Event::MouseRelease { pos: _, button: _ } if self.resizing_column.is_some() => {
                self.resizing_column = None;
            }

            Event::Wheel { delta, modifiers } => {
                // One notch is one row, whichever unit the host delivered it in.
                let scroll = WheelScroll::from_event(*delta, *modifiers, 1);
                let lines = scroll.lines_y.unsigned_abs() as usize;
                if scroll.lines_y < 0 {
                    let next = self.scroll_row.saturating_add(lines);
                    self.set_scroll_row(next);
                } else if scroll.lines_y > 0 {
                    let up = self.scroll_row.saturating_sub(lines);
                    self.set_scroll_row(up);
                }
            }

            Event::KeyPress { key, modifiers } => {
                let page = self.visible_rows.max(1) as isize;
                let last = self.row_count().saturating_sub(1) as isize;
                match *key {
                    // The navigation keys are named constants, the same set `tree_table` uses
                    // (V-008): a bare `37` does not say which direction it means.
                    crate::event::key_codes::LEFT => self.move_current_cell(0, -1, *modifiers),
                    crate::event::key_codes::RIGHT => self.move_current_cell(0, 1, *modifiers),
                    crate::event::key_codes::UP => self.move_current_cell(-1, 0, *modifiers),
                    crate::event::key_codes::DOWN => self.move_current_cell(1, 0, *modifiers),
                    crate::event::key_codes::HOME => {
                        let current = self.selected_cell.map_or(0, |(row, _)| row) as isize;
                        self.move_current_cell(-current, 0, *modifiers);
                    }
                    crate::event::key_codes::END => {
                        let current = self.selected_cell.map_or(0, |(row, _)| row) as isize;
                        self.move_current_cell(last - current, 0, *modifiers);
                    }
                    crate::event::key_codes::PAGE_UP => {
                        self.move_current_cell(-page, 0, *modifiers)
                    }
                    crate::event::key_codes::PAGE_DOWN => {
                        self.move_current_cell(page, 0, *modifiers)
                    }
                    crate::event::key_codes::ENTER => {
                        if let Some(cell) = self.selected_cell {
                            if self.edit_triggers.contains(TableEditTriggers::EDIT_KEY) {
                                self.edit_requested.emit(cell);
                            }
                        }
                    }
                    32..=126 => {
                        if let Some(character) = char::from_u32(*key) {
                            self.type_ahead(character);
                        }
                    }
                    _ => {}
                }
            }

            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal data source for testing.
    struct TestSource {
        rows: usize,
        cols: usize,
    }

    impl IncrementalTableDataSource for TestSource {
        fn row_count(&self) -> usize {
            self.rows
        }

        fn column_count(&self) -> usize {
            self.cols
        }

        fn data(&self, row: usize, column: usize) -> Option<String> {
            if row < self.rows && column < self.cols {
                Some(format!("{}:{}", row, column))
            } else {
                None
            }
        }
    }

    /// A source that names its columns, for the header tests.
    struct NamedSource;

    impl IncrementalTableDataSource for NamedSource {
        fn row_count(&self) -> usize {
            2
        }

        fn column_count(&self) -> usize {
            2
        }

        fn column_name(&self, column: usize) -> Option<String> {
            match column {
                0 => Some(String::from("Symbol")),
                1 => Some(String::from("Price")),
                _ => None,
            }
        }

        fn data(&self, row: usize, column: usize) -> Option<String> {
            Some(format!("{}:{}", row, column))
        }
    }

    /// A column header names the column the data source names, not its position.
    ///
    /// # The defect this pins
    ///
    /// Every header was `format!("Column {}{label}", ci + 1)`, and it could not be anything
    /// else: `IncrementalTableDataSource` had no column-name method, so a grid over a price
    /// table showed `"Column 1"` over the symbol column. Two grids over different data were
    /// indistinguishable at the header row.
    ///
    /// # Why this asserts what is *drawn*
    ///
    /// A test of `column_name` alone would prove the trait method exists, not that the control
    /// asks for it — and the original defect was precisely that nothing asked. So the header is
    /// rendered and its glyph run compared against one drawn from the literal label; the
    /// assertion fails if the control ignored the source.
    #[test]
    fn a_header_names_the_columns_source() {
        fn render(source: Option<Arc<dyn IncrementalTableDataSource>>) -> String {
            let mut tbl = GridTableWidget::new(Rect::new(0, 0, 600, 400));
            if let Some(source) = source {
                tbl.set_data_source(source);
            }
            crate::widget::svg::render_widget_to_svg(&mut tbl, Rect::new(0, 0, 600, 400))
        }

        let named = render(Some(Arc::new(NamedSource)));
        let index_labelled = render(Some(Arc::new(TestSource { rows: 2, cols: 2 })));

        assert!(
            !named.is_empty() && !index_labelled.is_empty(),
            "both renders must paint something for the comparison to mean anything"
        );
        assert_ne!(
            named, index_labelled,
            "a source that names its columns must produce different header glyphs than one that \
             does not; identical output means the header is still built from the column index"
        );
    }

    // -----------------------------------------------------------------------
    // Construction and defaults
    // -----------------------------------------------------------------------

    #[test]
    fn new_creates_default_state() {
        let tbl = GridTableWidget::new(Rect::new(0, 0, 600, 400));
        assert_eq!(tbl.row_count(), 0);
        assert_eq!(tbl.column_count(), 0);
        assert_eq!(tbl.scroll_row(), 0);
        assert_eq!(tbl.scroll_column(), 0);
        assert_eq!(tbl.row_height, 24);
        assert_eq!(tbl.header_height, 28);
        assert_eq!(tbl.row_number_width, 48);
        assert_eq!(tbl.min_column_width, 30);
        assert_eq!(tbl.selection_mode, GridTableSelectionMode::Cell);
        assert!(tbl.selected_cell().is_none());
        assert!(tbl.sort_specs().is_empty());
        assert!(tbl.resizing_column.is_none());
    }

    // -----------------------------------------------------------------------
    // Data source
    // -----------------------------------------------------------------------

    #[test]
    fn set_data_source_updates_counts() {
        let mut tbl = GridTableWidget::new(Rect::new(0, 0, 600, 400));
        let source = Arc::new(TestSource { rows: 10, cols: 5 });
        tbl.set_data_source(source);

        assert_eq!(tbl.row_count(), 10);
        assert_eq!(tbl.column_count(), 5);
        assert_eq!(tbl.scroll_row(), 0);
        assert_eq!(tbl.scroll_column(), 0);
    }

    #[test]
    fn clear_data_source_resets_state() {
        let mut tbl = GridTableWidget::new(Rect::new(0, 0, 600, 400));
        let source = Arc::new(TestSource { rows: 5, cols: 3 });
        tbl.set_data_source(source);
        assert!(tbl.data_source.is_some());

        tbl.clear_data_source();
        assert!(tbl.data_source.is_none());
        assert_eq!(tbl.row_count(), 0);
        assert_eq!(tbl.column_count(), 0);
        assert_eq!(tbl.scroll_row(), 0);
        assert_eq!(tbl.scroll_column(), 0);
        assert!(tbl.selected_cell.is_none());
    }

    // -----------------------------------------------------------------------
    // Scrolling
    // -----------------------------------------------------------------------

    #[test]
    fn scroll_row_clamps_to_source() {
        let mut tbl = GridTableWidget::new(Rect::new(0, 0, 600, 400));
        tbl.set_data_source(Arc::new(TestSource { rows: 5, cols: 3 }));

        tbl.set_scroll_row(3);
        assert_eq!(tbl.scroll_row(), 3);

        tbl.set_scroll_row(100);
        assert_eq!(tbl.scroll_row(), 4); // clamped to max

        tbl.set_scroll_row(3); // same value, no change
        assert_eq!(tbl.scroll_row(), 3);
    }

    #[test]
    fn scroll_column_clamps_to_source() {
        let mut tbl = GridTableWidget::new(Rect::new(0, 0, 600, 400));
        tbl.set_data_source(Arc::new(TestSource { rows: 5, cols: 3 }));

        tbl.set_scroll_column(1);
        assert_eq!(tbl.scroll_column(), 1);

        tbl.set_scroll_column(100);
        assert_eq!(tbl.scroll_column(), 2); // clamped
    }

    #[test]
    fn scroll_without_source_is_noop() {
        let mut tbl = GridTableWidget::new(Rect::new(0, 0, 600, 400));
        tbl.set_scroll_row(10);
        assert_eq!(tbl.scroll_row(), 0);
        tbl.set_scroll_column(10);
        assert_eq!(tbl.scroll_column(), 0);
    }

    // -----------------------------------------------------------------------
    // Column widths
    // -----------------------------------------------------------------------

    #[test]
    fn column_width_default_and_set() {
        let mut tbl = GridTableWidget::new(Rect::new(0, 0, 600, 400));
        tbl.set_data_source(Arc::new(TestSource { rows: 3, cols: 4 }));

        // Default width for any column
        assert_eq!(tbl.column_width(0), 120);
        assert_eq!(tbl.column_width(3), 120);
        assert_eq!(tbl.column_width(99), 120); // beyond count falls back to default

        tbl.set_column_width(1, 80);
        assert_eq!(tbl.column_width(1), 80);
    }

    #[test]
    fn column_width_clamps_to_minimum() {
        let mut tbl = GridTableWidget::new(Rect::new(0, 0, 600, 400));
        tbl.set_data_source(Arc::new(TestSource { rows: 3, cols: 4 }));
        tbl.set_column_width(0, 1);
        // min_column_width is 30
        assert_eq!(tbl.column_width(0), tbl.min_column_width);
    }

    #[test]
    fn set_column_width_out_of_range_is_noop() {
        let mut tbl = GridTableWidget::new(Rect::new(0, 0, 600, 400));
        tbl.set_data_source(Arc::new(TestSource { rows: 3, cols: 4 }));
        // 99 is out of range, should not panic
        tbl.set_column_width(99, 200);
        // Should still have defaults for valid columns
        assert_eq!(tbl.column_width(0), 120);
    }

    // -----------------------------------------------------------------------
    // Sorting
    // -----------------------------------------------------------------------

    #[test]
    fn toggle_sort_column_first_time_ascending() {
        let mut tbl = GridTableWidget::new(Rect::new(0, 0, 600, 400));
        tbl.set_data_source(Arc::new(TestSource { rows: 5, cols: 3 }));

        tbl.toggle_sort_column(1);
        assert_eq!(tbl.sort_specs().len(), 1);
        assert_eq!(tbl.sort_specs()[0].column, 1);
        assert!(!tbl.sort_specs()[0].descending);
    }

    #[test]
    fn toggle_sort_column_flips_direction() {
        let mut tbl = GridTableWidget::new(Rect::new(0, 0, 600, 400));
        tbl.set_data_source(Arc::new(TestSource { rows: 5, cols: 3 }));

        tbl.toggle_sort_column(1);
        assert!(!tbl.sort_specs()[0].descending);

        tbl.toggle_sort_column(1);
        assert!(tbl.sort_specs()[0].descending);

        tbl.toggle_sort_column(1);
        assert!(!tbl.sort_specs()[0].descending);
    }

    #[test]
    fn toggle_sort_column_replaces_previous_sort() {
        let mut tbl = GridTableWidget::new(Rect::new(0, 0, 600, 400));
        tbl.set_data_source(Arc::new(TestSource { rows: 5, cols: 3 }));

        tbl.toggle_sort_column(0);
        tbl.toggle_sort_column(2);
        assert_eq!(tbl.sort_specs().len(), 1);
        assert_eq!(tbl.sort_specs()[0].column, 2);
    }

    #[test]
    fn toggle_sort_column_out_of_range_is_noop() {
        let mut tbl = GridTableWidget::new(Rect::new(0, 0, 600, 400));
        tbl.set_data_source(Arc::new(TestSource { rows: 5, cols: 3 }));
        tbl.toggle_sort_column(10);
        assert!(tbl.sort_specs().is_empty());
    }

    // -----------------------------------------------------------------------
    // Selection
    // -----------------------------------------------------------------------

    #[test]
    fn selection_default_none() {
        let tbl = GridTableWidget::new(Rect::new(0, 0, 600, 400));
        assert!(tbl.selected_cell().is_none());
    }

    #[test]
    fn clear_selection_clears() {
        let mut tbl = GridTableWidget::new(Rect::new(0, 0, 600, 400));
        tbl.selected_cell = Some((2, 3));
        tbl.clear_selection();
        assert!(tbl.selected_cell().is_none());
    }

    #[test]
    fn set_selection_mode_none_clears_selection() {
        let mut tbl = GridTableWidget::new(Rect::new(0, 0, 600, 400));
        tbl.selected_cell = Some((1, 2));
        tbl.set_selection_mode(GridTableSelectionMode::None);
        assert!(tbl.selected_cell.is_none());
    }

    // -----------------------------------------------------------------------
    // Row height
    // -----------------------------------------------------------------------

    #[test]
    fn row_height_minimum_clamp() {
        let mut tbl = GridTableWidget::new(Rect::new(0, 0, 600, 400));
        tbl.set_row_height(0);
        assert_eq!(tbl.row_height, 1);

        tbl.set_row_height(50);
        assert_eq!(tbl.row_height, 50);
    }

    // -----------------------------------------------------------------------
    // Hit testing
    // -----------------------------------------------------------------------

    #[test]
    fn cell_at_point_returns_correct_cell() {
        let mut tbl = GridTableWidget::new(Rect::new(0, 0, 600, 400));
        tbl.set_data_source(Arc::new(TestSource { rows: 100, cols: 10 }));
        tbl.ensure_column_widths(10);
        tbl.column_widths[0] = 100;
        tbl.column_widths[1] = 80;
        tbl.update_visibility();

        // Point at the first data cell (after header + row number gutter)
        let cell = tbl.cell_at_point(Point::new(
            tbl.row_number_width as i32 + 10,
            tbl.header_height as i32 + 5,
        ));
        assert_eq!(cell, Some((0, 0)));

        // Point at second column, first row
        let cell2 = tbl.cell_at_point(Point::new(
            tbl.row_number_width as i32 + 110,
            tbl.header_height as i32 + 5,
        ));
        assert_eq!(cell2, Some((0, 1)));

        // Point in header area → None
        let no_cell = tbl.cell_at_point(Point::new(tbl.row_number_width as i32 + 10, 5));
        assert!(no_cell.is_none());
    }

    #[test]
    fn header_at_point_returns_correct_column() {
        let mut tbl = GridTableWidget::new(Rect::new(0, 0, 600, 400));
        tbl.set_data_source(Arc::new(TestSource { rows: 5, cols: 5 }));
        tbl.ensure_column_widths(5);
        tbl.column_widths[0] = 100;
        tbl.column_widths[1] = 80;
        tbl.update_visibility();

        let col = tbl.header_at_point(Point::new(tbl.row_number_width as i32 + 10, 5));
        assert_eq!(col, Some(0));

        let col2 = tbl.header_at_point(Point::new(tbl.row_number_width as i32 + 110, 5));
        assert_eq!(col2, Some(1));

        // Below header → None
        let none = tbl.header_at_point(Point::new(
            tbl.row_number_width as i32 + 10,
            tbl.header_height as i32 + 10,
        ));
        assert!(none.is_none());
    }

    #[test]
    fn resize_handle_at_point_detects_right_edge() {
        let mut tbl = GridTableWidget::new(Rect::new(0, 0, 600, 400));
        tbl.set_data_source(Arc::new(TestSource { rows: 5, cols: 5 }));
        tbl.ensure_column_widths(5);
        tbl.column_widths[0] = 100;
        tbl.update_visibility();

        // Near the right edge of column 0 (at x=48+100=148, within handle_width=5)
        let handle = tbl.resize_handle_at_point(Point::new(148, 5));
        assert_eq!(handle, Some(0));

        // Far from edge → None
        let none = tbl.resize_handle_at_point(Point::new(60, 5));
        assert!(none.is_none());
    }

    /// The `§B.9` judgement for this control: widening a column must move everything that follows it.
    ///
    /// # Why this is the criterion and not "the widths are stored"
    ///
    /// The defect class this replaces is *five* independent walks of the same widths — the header
    /// painter, the cell painter, the pointer hit test, the header hit test and the resize-handle hit
    /// test. A control whose paints and whose hit tests each accumulate their own x agrees with itself
    /// until one of them is edited, and the failure appears as "the resize handle is next to the line"
    /// rather than as a bad number. Asserting that a width change *propagates to every consumer* is what
    /// pins them to one derivation.
    #[test]
    fn widening_a_column_moves_every_follower() {
        fn build(first: u32) -> GridTableWidget {
            let mut tbl = GridTableWidget::new(Rect::new(0, 0, 600, 400));
            tbl.set_data_source(Arc::new(TestSource { rows: 5, cols: 5 }));
            tbl.ensure_column_widths(5);
            tbl.set_column_width(0, first);
            tbl.set_column_width(1, 80);
            tbl.update_visibility();
            tbl
        }

        let narrow = build(100);
        let wide = build(180);
        let rnw = narrow.row_number_width as i32;
        let header_y = 5;

        // The column the pointer lands in. Column 0 is 100 px wide, so 110 px in is already inside
        // column 1; growing column 0 to 180 moves the boundary past the pointer, so it lands on a
        // different *index* at the same pixel.
        //
        // # Why the index changes rather than the position
        //
        // The columns do not reflow — each keeps its own width — so widening the first one shifts every
        // follower rightward, and a fixed pixel falls into an earlier column. Asserting on both the
        // header and the cell below it is what shows the shift is shared.
        assert_eq!(narrow.header_at_point(Point::new(rnw + 110, header_y)), Some(1));
        assert_eq!(
            wide.header_at_point(Point::new(rnw + 110, header_y)),
            Some(0),
            "a wider first column must take over the pixel the follower used to own"
        );
        assert_eq!(
            wide.header_at_point(Point::new(rnw + 180, header_y)),
            Some(1),
            "and the follower's header begins at the first column's new right edge"
        );

        // The resize handle follows the same edge.
        assert_eq!(narrow.resize_handle_at_point(Point::new(rnw + 100, header_y)), Some(0));
        assert_eq!(
            wide.resize_handle_at_point(Point::new(rnw + 100, header_y)),
            None,
            "the first column's right edge moved away from x = 100"
        );
        assert_eq!(
            wide.resize_handle_at_point(Point::new(rnw + 180, header_y)),
            Some(0),
            "and the handle is now at the new edge"
        );

        // The data cells move with their headers, which is what "one derivation" has to mean.
        let row_y = narrow.header_height as i32 + 5;
        assert_eq!(narrow.cell_at_point(Point::new(rnw + 110, row_y)), Some((0, 1)));
        assert_eq!(wide.cell_at_point(Point::new(rnw + 110, row_y)), Some((0, 0)));
    }

    /// A pointer inside the data area's first pixels must hit the **first** column, not `None`.
    ///
    /// This is the case that the fit walk and the hit walk genuinely answer differently, and it was a
    /// real regression when the hit test was made to reuse the fit walk: the fit walk stops at the first
    /// column whose *end* passes the extent, so handing it the pointer's own small offset dropped the
    /// column the pointer was standing in.
    #[test]
    fn a_pointer_near_the_data_edges_hits_the_first_column_and_row() {
        let mut tbl = GridTableWidget::new(Rect::new(0, 0, 600, 400));
        tbl.set_data_source(Arc::new(TestSource { rows: 5, cols: 5 }));
        tbl.ensure_column_widths(5);
        tbl.update_visibility();

        let rnw = tbl.row_number_width as i32;
        let header_h = tbl.header_height as i32;
        // The very first pixel of the data area, and the very first column's last pixel.
        assert_eq!(tbl.cell_at_point(Point::new(rnw, header_h)), Some((0, 0)));
        assert_eq!(tbl.cell_at_point(Point::new(rnw + 119, header_h)), Some((0, 0)));
        assert_eq!(tbl.cell_at_point(Point::new(rnw + 120, header_h)), Some((0, 1)));
        // And the gutter and header themselves are not cells.
        assert_eq!(tbl.cell_at_point(Point::new(rnw - 1, header_h)), None);
        assert_eq!(tbl.cell_at_point(Point::new(rnw, header_h - 1)), None);
    }

    /// A column that only *partly* fits is still drawn and still answers — otherwise growing a column
    /// could make the table's own last column unreachable.
    #[test]
    fn a_partly_visible_column_is_still_drawn_and_hit() {
        let mut tbl = GridTableWidget::new(Rect::new(0, 0, 260, 400));
        tbl.set_data_source(Arc::new(TestSource { rows: 5, cols: 5 }));
        tbl.ensure_column_widths(5);
        for i in 0..5 {
            tbl.set_column_width(i, 100);
        }
        tbl.update_visibility();

        // 260 - 48 = 212 px of data area: two whole 100 px columns and 12 px of the third.
        let rnw = tbl.row_number_width as i32;
        let spans = tbl.visible_column_spans(212);
        assert_eq!(spans.len(), 3, "the third column starts on screen: {spans:?}");
        assert_eq!(spans[2], (2, 200, 100));
        assert_eq!(tbl.cell_at_point(Point::new(rnw + 205, 30)), Some((0, 2)));
    }

    // -----------------------------------------------------------------------
    // Visibility calculation
    // -----------------------------------------------------------------------

    #[test]
    fn update_visibility_computes_visible_cells() {
        let mut tbl = GridTableWidget::new(Rect::new(0, 0, 600, 400));
        tbl.set_data_source(Arc::new(TestSource { rows: 50, cols: 20 }));
        tbl.ensure_column_widths(20);
        tbl.column_widths.fill(120);
        tbl.update_visibility();

        // 400px height - 28px header = 372px data area, / 24px row height ≈ 15 rows
        assert_eq!(tbl.visible_rows, 15);
    }

    // -----------------------------------------------------------------------
    // Size hint
    // -----------------------------------------------------------------------

    #[test]
    fn size_hint_without_source_uses_minimum() {
        let tbl = GridTableWidget::new(Rect::new(0, 0, 600, 400));
        let hint = tbl.size_hint();
        // Without data source: width = max(row_number_width, min_column_width) = 48
        assert_eq!(hint.width, tbl.row_number_width.max(tbl.min_column_width));
        // height = header_height (no rows)
        assert_eq!(hint.height, tbl.header_height);
    }

    #[test]
    fn size_hint_with_source_uses_content() {
        let mut tbl = GridTableWidget::new(Rect::new(0, 0, 600, 400));
        tbl.set_data_source(Arc::new(TestSource { rows: 8, cols: 3 }));
        tbl.ensure_column_widths(3);
        tbl.column_widths[0] = 100;
        tbl.column_widths[1] = 150;
        tbl.column_widths[2] = 80;

        let hint = tbl.size_hint();
        // Total col width = 330, row number gutter = 48, so width = max(330,48,30) = 330
        assert_eq!(hint.width, 330);
        // Height = 8 * 24 + 28 = 220
        assert_eq!(hint.height, 220);
    }

    // -----------------------------------------------------------------------
    // ensure_column_widths
    // -----------------------------------------------------------------------

    #[test]
    fn ensure_column_widths_grows_vector() {
        let mut tbl = GridTableWidget::new(Rect::new(0, 0, 600, 400));
        assert!(tbl.column_widths.is_empty());

        tbl.ensure_column_widths(5);
        assert_eq!(tbl.column_widths.len(), 5);
        for &w in &tbl.column_widths {
            assert_eq!(w, 120); // default width
        }
    }

    #[test]
    fn ensure_column_widths_does_not_shrink() {
        let mut tbl = GridTableWidget::new(Rect::new(0, 0, 600, 400));
        tbl.column_widths = vec![50, 60, 70];
        tbl.ensure_column_widths(2);
        assert_eq!(tbl.column_widths.len(), 3); // unchanged
        assert_eq!(tbl.column_widths[0], 50);
    }

    // -----------------------------------------------------------------------
    // Empty source edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn empty_source_does_not_crash() {
        let mut tbl = GridTableWidget::new(Rect::new(0, 0, 600, 400));
        // No source set – these should all be safe
        assert_eq!(tbl.row_count(), 0);
        assert_eq!(tbl.column_count(), 0);
        tbl.set_scroll_row(5);
        assert_eq!(tbl.scroll_row(), 0);
        tbl.toggle_sort_column(0);
        assert!(tbl.sort_specs().is_empty());
        tbl.clear_selection();
        assert!(tbl.selected_cell.is_none());
    }

    /// One wheel notch scrolls one row, whether the host sends the documented
    /// notch delta or the legacy 120-unit tick.
    #[test]
    fn wheel_scrolls_one_row_per_notch() {
        let mut tbl = GridTableWidget::new(Rect::new(0, 0, 600, 400));
        tbl.set_data_source(Arc::new(TestSource { rows: 10, cols: 3 }));

        tbl.handle_event(&Event::wheel(0, -120, 0));
        assert_eq!(tbl.scroll_row(), 1, "one legacy notch is one row");

        tbl.handle_event(&Event::wheel(0, -1, 0));
        assert_eq!(tbl.scroll_row(), 2, "one documented notch is one row");

        tbl.handle_event(&Event::wheel(0, 120, 0));
        assert_eq!(tbl.scroll_row(), 1, "the opposite notch scrolls back one row");
    }

    /// Section resize modes, alternating rows, keyboard paging and the edit request are wired.
    #[test]
    fn resize_modes_alternating_rows_and_keyboard_are_wired() {
        let mut tbl = GridTableWidget::new(Rect::new(0, 0, 600, 200));
        tbl.set_data_source(Arc::new(TestSource { rows: 30, cols: 4 }));
        tbl.set_section_resize_mode(GridSectionResizeMode::Stretch);
        assert_eq!(tbl.section_resize_mode(), GridSectionResizeMode::Stretch);
        assert!(tbl.column_width(0) > 0);
        tbl.set_alternating_rows(true);
        assert!(tbl.alternating_rows());

        let edited = std::sync::Arc::new(std::sync::Mutex::new(None));
        let sink = std::sync::Arc::clone(&edited);
        tbl.edit_requested.connect(move |cell| {
            if let Ok(mut slot) = sink.lock() {
                *slot = Some(*cell);
            }
        });
        tbl.handle_event(&Event::MouseDoubleClick { pos: Point::new(200, 80), button: 1 });
        assert!(
            edited.lock().map(|slot| slot.is_some()).unwrap_or(false),
            "a double-click requests an edit"
        );

        tbl.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Tab });
        tbl.handle_event(&Event::KeyPress { key: crate::event::key_codes::END, modifiers: 0 });
        assert!(tbl.scroll_row() > 0, "End reaches the last row");
    }

    /// Extended selection, type-ahead and the scroll-bar flag are wired.
    #[test]
    fn multi_select_type_ahead_and_scrollbar_are_wired() {
        let rnw = 48;
        let header_h = 28;
        let row_h = 24;
        let mut tbl = GridTableWidget::new(Rect::new(0, 0, 600, 200));
        tbl.set_data_source(Arc::new(TestSource { rows: 6, cols: 3 }));
        tbl.set_multi_select(true);
        assert!(tbl.multi_select());

        let first = Point::new(rnw + 5, header_h + 5);
        let second = Point::new(rnw + 5, header_h + row_h + 5);
        tbl.handle_event(&Event::MousePress { pos: first, button: 1, modifiers: 0 });
        tbl.handle_event(&Event::MousePress {
            pos: second,
            button: 1,
            modifiers: crate::shortcut::Modifiers::CTRL.bits() as u32,
        });
        assert_eq!(tbl.selected_rows(), vec![0, 1], "ctrl adds a second row");

        tbl.set_selection_mode(GridTableSelectionMode::Row);
        tbl.clear_selection();
        tbl.handle_event(&Event::MousePress { pos: first, button: 1, modifiers: 0 });
        tbl.handle_event(&Event::MousePress {
            pos: second,
            button: 1,
            modifiers: crate::shortcut::Modifiers::SHIFT.bits() as u32,
        });
        assert_eq!(tbl.selected_rows(), vec![0, 1], "shift extends the row range");

        tbl.set_selection_mode(GridTableSelectionMode::Row);
        tbl.clear_selection();
        tbl.handle_event(&Event::MousePress { pos: first, button: 1, modifiers: 0 });
        tbl.handle_event(&Event::KeyPress { key: 51, modifiers: 0 });
        assert_eq!(
            tbl.selected_cell().map(|(row, _)| row),
            Some(3),
            "typing '3' jumps to the next row whose first cell starts with it"
        );

        tbl.set_show_scrollbar(true);
        assert!(tbl.show_scrollbar());
        let _ = crate::widget::svg::render_to_svg(&mut tbl);
    }

    /// F-P05: the shared span run reflects width changes and scroll position.
    #[test]
    fn column_spans_are_rebuilt_only_when_the_geometry_changes() {
        let mut tbl = GridTableWidget::new(Rect::new(0, 0, 600, 200));
        tbl.set_data_source(Arc::new(TestSource { rows: 10, cols: 8 }));
        let first = tbl.visible_column_spans(300);
        assert_eq!(first.first().map(|span| span.0), Some(0));
        assert_eq!(first.len(), 3, "two 120 px columns and the one that crosses 300 px");

        tbl.set_scroll_column(2);
        let moved = tbl.visible_column_spans(300);
        assert_eq!(moved.first().map(|span| span.0), Some(2));

        // A width change is visible immediately: the cache key includes the revision.
        tbl.set_column_width(2, 50);
        let resized = tbl.visible_column_spans(300);
        assert_eq!(
            resized.iter().find(|(column, _, _)| *column == 2).map(|(_, _, width)| *width),
            Some(50)
        );
    }

    /// V-01: the edit triggers gate the two gestures, while the plain double-click event stays.
    #[test]
    fn edit_triggers_gate_double_click_and_enter() {
        let mut tbl = GridTableWidget::new(Rect::new(0, 0, 600, 200));
        tbl.set_data_source(Arc::new(TestSource { rows: 10, cols: 4 }));
        let edits = std::sync::Arc::new(std::sync::Mutex::new(Vec::<(usize, usize)>::new()));
        let sink = std::sync::Arc::clone(&edits);
        tbl.edit_requested.connect(move |cell| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*cell);
            }
        });
        let clicks = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let click_sink = std::sync::Arc::clone(&clicks);
        tbl.cell_double_clicked.connect(move |_| {
            click_sink.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        });

        assert_eq!(tbl.edit_triggers(), TableEditTriggers::ALL);
        tbl.set_edit_triggers(TableEditTriggers::NONE);
        tbl.handle_event(&Event::MouseDoubleClick { pos: Point::new(60, 40), button: 1 });
        assert!(
            edits.lock().map(|guard| guard.is_empty()).unwrap_or(false),
            "NONE suppresses the edit request"
        );
        assert_eq!(
            clicks.load(std::sync::atomic::Ordering::Relaxed),
            1,
            "the click itself is still reported"
        );

        tbl.handle_event(&Event::MousePress { pos: Point::new(60, 40), button: 1, modifiers: 0 });
        assert!(tbl.selected_cell().is_some());
        tbl.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
        assert!(edits.lock().map(|guard| guard.is_empty()).unwrap_or(false), "EDIT_KEY is off");

        tbl.set_edit_triggers(TableEditTriggers::EDIT_KEY);
        tbl.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
        assert_eq!(
            edits.lock().map(|guard| guard.len()).unwrap_or(0),
            1,
            "Enter edits when enabled"
        );
        tbl.handle_event(&Event::MouseDoubleClick { pos: Point::new(60, 40), button: 1 });
        assert_eq!(
            edits.lock().map(|guard| guard.len()).unwrap_or(0),
            1,
            "double-click is still off"
        );
    }

    /// V-02: the arrow keys move the current cell and the viewport follows it.
    #[test]
    fn arrow_keys_move_the_current_cell() {
        use crate::event::key_codes::{DOWN, END, HOME, RIGHT};
        let mut tbl = GridTableWidget::new(Rect::new(0, 0, 600, 200));
        tbl.set_data_source(Arc::new(TestSource { rows: 30, cols: 4 }));
        tbl.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Tab });

        tbl.handle_event(&Event::KeyPress { key: DOWN, modifiers: 0 });
        assert_eq!(tbl.selected_cell(), Some((1, 0)), "Down moves one row");
        tbl.handle_event(&Event::KeyPress { key: RIGHT, modifiers: 0 });
        assert_eq!(tbl.selected_cell(), Some((1, 1)), "Right moves one column");
        tbl.handle_event(&Event::KeyPress { key: END, modifiers: 0 });
        assert_eq!(tbl.selected_cell().map(|(row, _)| row), Some(29), "End reaches the last row");
        assert!(tbl.scroll_row() > 0, "the viewport followed the cell");
        tbl.handle_event(&Event::KeyPress { key: HOME, modifiers: 0 });
        assert_eq!(
            tbl.selected_cell().map(|(row, _)| row),
            Some(0),
            "Home returns to the first row"
        );
        assert_eq!(tbl.scroll_row(), 0, "the viewport followed the cell back");
    }

    /// V-006: a sort reads each key once per row and spec, not once per comparison.
    ///
    /// The comparator used to call `data_role`/`data` for both operands of every comparison,
    /// which is `O(n log n)` source reads for one sort. The key table makes it `O(n)`; this
    /// counts the reads at the source so a future refactor cannot reintroduce the old shape.
    #[test]
    fn sorting_reads_each_sort_key_once_per_row() {
        struct CountingSource {
            rows: usize,
            reads: std::sync::Arc<std::sync::atomic::AtomicUsize>,
        }

        impl IncrementalTableDataSource for CountingSource {
            fn row_count(&self) -> usize {
                self.rows
            }
            fn column_count(&self) -> usize {
                2
            }
            fn data(&self, row: usize, column: usize) -> Option<String> {
                Some(format!("{row}:{column}"))
            }
            fn data_role(&self, row: usize, column: usize, _role: DataRole) -> Option<String> {
                self.reads.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                self.data(row, column)
            }
        }

        let reads = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut tbl = GridTableWidget::new(Rect::new(0, 0, 600, 200));
        tbl.set_data_source(Arc::new(CountingSource { rows: 50, reads: reads.clone() }));
        tbl.toggle_sort_column(0);
        assert_eq!(
            reads.load(std::sync::atomic::Ordering::Relaxed),
            50,
            "one key read per row for the single sort spec"
        );
        // The projection is cached: asking again with unchanged inputs reads nothing.
        tbl.ensure_sort_order();
        assert_eq!(
            reads.load(std::sync::atomic::Ordering::Relaxed),
            50,
            "a cache hit does not walk the source again"
        );
    }

    /// V-007: a repeat click on the already-selected cell emits nothing.
    ///
    /// The change test used to clone all three selection sets and the anchor on both sides of the
    /// mutation. The live-set predicate must reach the same answer: the first click changes the
    /// selection, the second does not.
    #[test]
    fn a_repeat_click_on_the_same_cell_does_not_re_emit() {
        let mut tbl = GridTableWidget::new(Rect::new(0, 0, 600, 200));
        tbl.set_data_source(Arc::new(TestSource { rows: 10, cols: 3 }));
        let emitted = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let sink = emitted.clone();
        tbl.cell_selected.connect(move |_| {
            sink.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        });

        tbl.apply_selection(1, 1, 0);
        assert_eq!(emitted.load(std::sync::atomic::Ordering::Relaxed), 1);
        tbl.apply_selection(1, 1, 0);
        assert_eq!(
            emitted.load(std::sync::atomic::Ordering::Relaxed),
            1,
            "selecting the same unit twice is not a change"
        );
        tbl.apply_selection(1, 2, 0);
        assert_eq!(emitted.load(std::sync::atomic::Ordering::Relaxed), 2);
    }
}
