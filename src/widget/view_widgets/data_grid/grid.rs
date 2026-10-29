// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The `DataGrid` control: state, selection, editing, row details and viewport accessors.

use super::*;

/// Virtualized data grid with windowed pull from incremental table data source.
pub struct DataGrid {
    pub(crate) base: BaseWidget,
    pub(crate) data_source: Option<Arc<dyn IncrementalTableDataSource>>,
    pub(crate) data_source_connection_scope: ConnectionScope,
    pub(crate) scroll_row: usize,
    pub(crate) scroll_column: usize,
    pub(crate) row_height: u32,
    pub(crate) column_width: u32,
    pub(crate) overscan_rows: usize,
    pub(crate) overscan_columns: usize,
    pub(crate) frozen_columns: usize,
    pub(crate) sort_specs: Vec<SortSpec>,
    pub(crate) filters: Vec<ColumnFilter>,
    /// The filter as a recursive expression.
    ///
    /// This is what `apply_filter_sort` evaluates; `filters` is derived from it, so
    /// the two cannot describe different filters.
    pub(crate) filter_expr: FilterExpr,
    pub(crate) window_cache: Option<GridWindowCache>,
    /// Prefix sums of the current column widths; see [`ColumnPrefixCache`].
    pub(crate) column_prefix_cache: RefCell<Option<ColumnPrefixCache>>,
    /// Bumped by every width-affecting setter (F-P01).
    pub(crate) column_geometry_revision: u64,
    /// Number of prefix-table rebuilds, published so a test can prove the table is reused.
    pub(crate) column_prefix_rebuilds: Cell<u64>,
    /// Elided header labels; see [`HeaderTextCache`] (F-P04).
    pub(crate) header_text_cache: RefCell<HeaderTextCache>,
    /// Bumped by `set_header_labels` / `set_header`.
    pub(crate) header_text_revision: u64,
    /// Bumped by the source's `data_changed` signal and by `set_data_source`.
    ///
    /// A source whose `revision()` stays `0` still changes, and the signal is how it says so.
    /// Keying the window cache on this counter is what lets such a source be cached at all; a
    /// source that offers neither a revision nor a signal is not cached, because then there is no
    /// evidence a cached window is still current.
    pub(crate) data_generation: Arc<AtomicU64>,
    /// Whether the bound source can report change (`revision > 0` or a `data_changed` signal).
    pub(crate) data_is_observable: bool,
    /// The projected `(row, column)` of the last press, or `None` when the press missed.
    ///
    /// Set through [`Self::cell_at`], so a selection can only ever name a cell that was
    /// actually painted.
    pub(crate) selection: Option<(usize, usize)>,
    /// How many units a press may add to the selection.
    pub(crate) selection_mode: crate::widget::view_widgets::list_view::SelectionMode,
    /// What a selection gesture selects (cells, rows or columns).
    pub(crate) selection_behavior: DataGridSelectionBehavior,
    /// The selected units, stored as cells: `(row, 0)` for a row, `(0, column)` for a column and
    /// the exact cell for cell selection. Keeping one set means the three behaviours share the
    /// paint and query paths.
    pub(crate) selected_cells: BTreeSet<(usize, usize)>,
    /// The cell a range selection extends from.
    pub(crate) selection_anchor: Option<(usize, usize)>,
    /// The cell keyboard navigation currently points at.
    pub(crate) current_cell: Option<(usize, usize)>,
    /// A `select_all` that is represented by a flag instead of a per-cell set.
    all_selected: bool,
    /// Roles used for a column's sort key, when it differs from the display text.
    pub(crate) sort_roles: std::collections::HashMap<usize, DataRole>,
    /// Whether the header paints a sort arrow.
    pub(crate) sort_indicator_visible: bool,
    /// Explicit per-column widths used by [`ColumnWidthMode::Interactive`].
    pub(crate) column_widths: std::collections::HashMap<usize, u32>,
    /// The column currently being dragged wider or narrower.
    pub(crate) resizing_column: Option<usize>,
    /// The pointer x where the current resize drag began.
    pub(crate) resize_start_x: i32,
    /// The width the column had when the current resize drag began.
    pub(crate) resize_start_width: u32,
    /// Labels supplied through [`DataGrid::set_header`] or `set_header_labels`.
    pub(crate) header_labels: Vec<String>,
    /// The column specs a bound [`GridViewHeader`] contributed.
    ///
    /// Held as specs rather than as a header control because a widget cannot own a borrow of
    /// another widget: the header is copied in at `set_header`, and its per-column capabilities
    /// (sortable, filterable) keep driving this grid's header row afterwards.
    pub(crate) header_columns: Vec<ColumnSpec>,
    /// Whether filter glyphs are painted on the filterable header columns.
    pub(crate) header_filter_visible: bool,
    /// The pagination bar this grid is bound to, if any.
    pub(crate) pagination: Option<PaginationBinding>,
    /// The last page the binding applied, so polling twice does not re-emit.
    pub(crate) pagination_applied_page: usize,
    /// The `FilterRow` this grid follows, when one is attached (chapter 3, gap 2).
    pub(crate) filter_row: Option<FilterBinding>,
    /// The `GridStatusBar` this grid follows, when one is attached (COMP-A8).
    pub(crate) grid_status_bar: Option<GridStatusBarBinding>,
    /// The `SummaryRow` this grid follows, when one is attached (COMP-A8).
    pub(crate) summary_row: Option<SummaryRowBinding>,
    /// The `HeaderMenu` this grid follows, when one is attached (COMP-A8).
    pub(crate) header_menu: Option<HeaderMenuBinding>,
    /// Whether the attached header menu asked the host to open a column chooser (COMP-A8).
    pub(crate) column_chooser_requested: bool,
    /// Emitted when the selection changes; query [`DataGrid::selected_cells`] for the units.
    pub(crate) selection_changed: Signal1<()>,
    /// Emitted with the cell keyboard navigation moved to.
    pub(crate) current_changed: Signal1<(usize, usize)>,
    /// Emitted when a cell is pressed.
    pub(crate) cell_clicked: Signal1<(usize, usize)>,
    /// Emitted when a cell is double-clicked.
    pub(crate) cell_double_clicked: Signal1<(usize, usize)>,
    /// Emitted after a user resize with `(column, width)`.
    pub(crate) column_resized: Signal1<(usize, u32)>,
    /// The projected `(row, column)` under the pointer, or `None` when it is elsewhere.
    ///
    /// Set through the **same** [`Self::cell_at`] the press uses, so the cell a hover points at is
    /// the cell a click would select — the two cannot disagree about where a cell begins. A grid is a
    /// matrix of independent targets, so this has to name a cell rather than light the control.
    pub(crate) hovered_cell: Option<(usize, usize)>,
    /// Emitted when visible row/column window changes.
    pub visible_window_changed: Signal1<(usize, usize, usize, usize)>,
    /// Whether filtering and sorting project the whole model instead of the visible window.
    ///
    /// Off by default: the default path is virtualized and materialises only the visible window
    /// (V-001). `true` keeps the historical whole-model materialisation as an explicit option.
    pub(crate) full_model_projection: bool,
    /// Cache for the full-model projection, keyed by the source generation/revision.
    pub(crate) full_projection_cache: Option<(u64, Rc<ProjectedRows>)>,
    /// Cache for the virtualized projection's row order, keyed by the source generation/revision.
    pub(crate) index_projection_cache: Option<(u64, Rc<ProjectedIndex>)>,
    /// The sliced window most recently handed out from `full_projection_cache`.
    pub(crate) full_window_cache: FullWindowCache,
    /// The window most recently fetched for the virtualized projection.
    pub(crate) virtual_window_cache: VirtualWindowCache,
    /// How column widths are decided.
    pub(crate) column_width_mode: ColumnWidthMode,
    /// Whether odd rows get the alternating wash.
    pub(crate) alternating_rows: bool,
    /// What gestures start an edit.
    pub(crate) edit_triggers: EditTriggers,
    /// Whether horizontal/vertical scroll-bar thumbs are painted.
    pub(crate) show_scrollbars: bool,
    /// Emitted when a header cell is clicked, with the column index.
    pub header_clicked: Signal1<usize>,
    /// Emitted when sorting changes for a column.
    pub sort_changed: Signal1<(usize, bool)>,
    /// Emitted when an edit is requested, with `(row, column)`.
    pub edit_requested: Signal1<(usize, usize)>,
    /// Emitted when the context menu is requested at a point.
    pub context_menu_requested: Signal1<Point>,
    /// Emitted with the new expression whenever the filter changes.
    pub filter_changed: Signal1<FilterExpr>,
    /// Emitted with the zero-based page when a page from the bound [`Pagination`] is applied.
    pub page_applied: Signal1<usize>,
    /// Columns excluded from painting and hit-testing (V-014).
    pub(crate) hidden_columns: BTreeSet<usize>,
    /// The `ColumnChooser` this grid follows, when one is attached (V-015).
    pub(crate) column_chooser: Option<ColumnChooserBinding>,
    /// The column display order the chooser last reported, as source column indices.
    ///
    /// Empty means the source order. The paint and hit-test paths address columns by their
    /// absolute source index, so this records the reported order without repainting the grid in
    /// a second coordinate space.
    pub(crate) column_order: Vec<usize>,
    /// Merged cells, expressed in projected coordinates (V-014).
    pub(crate) cell_spans: Vec<CellSpan>,
    /// The spans that cover each row, so a cell's span is a bucket lookup rather than a scan of
    /// every span on the grid (WGT-G3).
    ///
    /// `paint_cell_at` asks this for every visible cell and `cell_at` asks it on every pointer
    /// sample, so the previous linear scan made both costs grow with the span count. Derived
    /// data, maintained only by `set_cell_span` / `clear_cell_spans`; `cell_spans` stays the
    /// authoritative list.
    pub(crate) span_rows: HashMap<usize, Vec<CellSpan>>,
    /// Additional header rows painted above the title row (V-014).
    pub(crate) extra_header_rows: Vec<Vec<String>>,
    /// Per-cell tooltip source; `None` keeps the historical no-tooltip behaviour (V-014).
    pub(crate) cell_tooltip: Option<Arc<dyn Fn(usize, usize) -> Option<String> + Send + Sync>>,
    /// Per-cell paint delegate; `None` draws every cell directly (V-014).
    pub(crate) cell_delegate: Option<Arc<dyn DataGridCellDelegate>>,
    /// Per-column cell alignment (V-001); an absent entry means `Left`.
    pub(crate) column_alignments: HashMap<usize, HorizontalAlignment>,
    /// The cell an in-place edit is open on, if any (V-002).
    pub(crate) editing: Option<(usize, usize)>,
    /// Detail text source for expanded rows (V-009).
    row_detail_provider: Option<RowDetailProvider>,
    /// Rows whose detail band is open (V-009).
    pub(crate) expanded_detail_rows: BTreeSet<usize>,
    /// Explicit detail-band heights (V-009); a row without one uses
    /// [`DEFAULT_ROW_DETAIL_HEIGHT`].
    detail_heights: HashMap<usize, u32>,
}

impl DataGrid {
    /// Creates an empty data grid.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Table, geometry, "DataGrid"),
            data_source: None,
            data_source_connection_scope: ConnectionScope::new(),
            scroll_row: 0,
            scroll_column: 0,
            row_height: 20,
            column_width: 120,
            overscan_rows: 2,
            overscan_columns: 1,
            frozen_columns: 0,
            sort_specs: Vec::new(),
            filters: Vec::new(),
            filter_expr: FilterExpr::MatchAll,
            window_cache: None,
            column_prefix_cache: RefCell::new(None),
            column_geometry_revision: 0,
            column_prefix_rebuilds: Cell::new(0),
            header_text_cache: RefCell::new(HeaderTextCache::default()),
            header_text_revision: 0,
            data_generation: Arc::new(AtomicU64::new(0)),
            data_is_observable: false,
            selection: None,
            selection_mode: crate::widget::view_widgets::list_view::SelectionMode::Single,
            selection_behavior: DataGridSelectionBehavior::Cells,
            selected_cells: BTreeSet::new(),
            selection_anchor: None,
            current_cell: None,
            all_selected: false,
            sort_roles: std::collections::HashMap::new(),
            sort_indicator_visible: true,
            column_widths: std::collections::HashMap::new(),
            resizing_column: None,
            resize_start_x: 0,
            resize_start_width: 0,
            header_labels: Vec::new(),
            header_columns: Vec::new(),
            header_filter_visible: false,
            pagination: None,
            pagination_applied_page: 0,
            filter_row: None,
            grid_status_bar: None,
            summary_row: None,
            header_menu: None,
            column_chooser_requested: false,
            selection_changed: Signal1::new(),
            current_changed: Signal1::new(),
            cell_clicked: Signal1::new(),
            cell_double_clicked: Signal1::new(),
            column_resized: Signal1::new(),
            hovered_cell: None,
            visible_window_changed: Signal1::new(),
            full_model_projection: false,
            full_projection_cache: None,
            index_projection_cache: None,
            full_window_cache: None,
            virtual_window_cache: None,
            column_width_mode: ColumnWidthMode::Fixed,
            alternating_rows: false,
            edit_triggers: EditTriggers::NONE,
            show_scrollbars: false,
            header_clicked: Signal1::new(),
            sort_changed: Signal1::new(),
            edit_requested: Signal1::new(),
            context_menu_requested: Signal1::new(),
            filter_changed: Signal1::new(),
            page_applied: Signal1::new(),
            hidden_columns: BTreeSet::new(),
            column_chooser: None,
            column_order: Vec::new(),
            cell_spans: Vec::new(),
            span_rows: HashMap::new(),
            extra_header_rows: Vec::new(),
            cell_tooltip: None,
            cell_delegate: None,
            column_alignments: HashMap::new(),
            editing: None,
            row_detail_provider: None,
            expanded_detail_rows: BTreeSet::new(),
            detail_heights: HashMap::new(),
        }
    }

    /// Binds incremental source.
    pub fn set_data_source(&mut self, data_source: Arc<dyn IncrementalTableDataSource>) {
        self.data_source_connection_scope = ConnectionScope::new();
        let revision_nonzero = data_source.revision() > 0;
        let has_signal = if let Some(changed) = data_source.data_changed_signal() {
            let redraw = self.base.redraw_requested_signal().clone();
            let layout = self.base.layout_requested_signal().clone();
            let generation = Arc::clone(&self.data_generation);
            changed.connect_scoped(&self.data_source_connection_scope, move || {
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
        self.scroll_column = 0;
        self.clear_cache();
        self.invalidate_projection_cache();
        self.clear_selection_units();
        self.current_cell = None;
        self.hovered_cell = None;
        self.normalize_projection_state();
        self.emit_visible_window_changed();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Clears incremental source.
    pub fn clear_data_source(&mut self) {
        self.data_source_connection_scope = ConnectionScope::new();
        self.data_source = None;
        self.data_is_observable = false;
        self.data_generation.fetch_add(1, Ordering::Relaxed);
        self.scroll_row = 0;
        self.scroll_column = 0;
        self.frozen_columns = 0;
        self.clear_cache();
        self.invalidate_projection_cache();
        self.clear_selection_units();
        self.current_cell = None;
        self.emit_visible_window_changed();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns whether source is bound.
    pub fn has_data_source(&self) -> bool {
        self.data_source.is_some()
    }

    /// Returns source row count.
    pub fn row_count(&self) -> usize {
        self.data_source.as_ref().map(|source| source.row_count()).unwrap_or(0)
    }

    /// Returns source column count.
    pub fn column_count(&self) -> usize {
        self.data_source.as_ref().map(|source| source.column_count()).unwrap_or(0)
    }

    /// Returns current vertical scroll row.
    pub fn scroll_row(&self) -> usize {
        self.scroll_row
    }

    /// The projected `(row, column)` of the current selection, if any.
    pub fn selection(&self) -> Option<(usize, usize)> {
        self.selection
    }

    /// The selection's cardinality mode.
    pub fn selection_mode(&self) -> crate::widget::view_widgets::list_view::SelectionMode {
        self.selection_mode
    }

    /// Sets how many units a press may add to the selection.
    pub fn set_selection_mode(
        &mut self,
        mode: crate::widget::view_widgets::list_view::SelectionMode,
    ) {
        if self.selection_mode == mode {
            return;
        }
        self.selection_mode = mode;
        if mode == crate::widget::view_widgets::list_view::SelectionMode::None {
            self.clear_selection();
            return;
        }
        if mode == crate::widget::view_widgets::list_view::SelectionMode::Single
            && (self.selected_cells.len() > 1 || self.all_selected)
        {
            self.all_selected = false;
            let keep = self.selection.or_else(|| self.selected_cells.iter().next().copied());
            self.selected_cells.clear();
            if let Some((row, column)) = keep {
                self.push_selection_unit(row, column);
                self.selection = Some((row, column));
            }
            self.selection_changed.emit(());
        }
        self.base.request_redraw();
    }

    /// What a selection gesture selects.
    pub fn selection_behavior(&self) -> DataGridSelectionBehavior {
        self.selection_behavior
    }

    /// Sets what a selection gesture selects, clearing the old units.
    pub fn set_selection_behavior(&mut self, behavior: DataGridSelectionBehavior) {
        if self.selection_behavior == behavior {
            return;
        }
        self.selection_behavior = behavior;
        self.clear_selection_units();
        self.base.request_redraw();
    }

    /// The current navigation cell, if any.
    pub fn current_cell(&self) -> Option<(usize, usize)> {
        self.current_cell
    }

    /// Moves keyboard navigation to `(row, column)`, selecting it in single mode.
    pub fn set_current_cell(&mut self, row: usize, column: usize) -> bool {
        if row >= self.row_count() || column >= self.column_count() {
            return false;
        }
        self.current_cell = Some((row, column));
        self.current_changed.emit((row, column));
        if self.selection_mode == crate::widget::view_widgets::list_view::SelectionMode::Single {
            self.replace_selection(row, column);
        }
        self.scroll_to_cell(row, column);
        self.base.request_redraw();
        true
    }

    /// Moves the current cell, optionally extending the selection from its anchor.
    ///
    /// Keyboard navigation uses this so the arrow keys move a real current cell instead of only
    /// scrolling. The cell is scrolled into view with the minimum movement, which keeps the
    /// pointer/keyboard viewport behaviour the grid already had.
    pub fn move_current_cell(&mut self, row: usize, column: usize, extend: bool) -> bool {
        if row >= self.row_count() || column >= self.column_count() {
            return false;
        }
        if self.selection_mode == crate::widget::view_widgets::list_view::SelectionMode::None {
            self.current_cell = Some((row, column));
            self.current_changed.emit((row, column));
            self.scroll_to_cell(row, column);
            self.base.request_redraw();
            return true;
        }
        let anchor =
            self.selection_anchor.or(self.selection).or(self.current_cell).unwrap_or((row, column));
        self.current_cell = Some((row, column));
        if extend {
            self.all_selected = false;
            self.push_selection_range(anchor, (row, column));
            self.selection = Some((row, column));
            self.selection_changed.emit(());
        } else {
            self.replace_selection(row, column);
        }
        self.current_changed.emit((row, column));
        self.scroll_to_cell(row, column);
        self.base.request_redraw();
        true
    }

    /// The signal emitted when the selection changes.
    ///
    /// Exposed as accessors rather than `pub` fields: the capability table owns the published
    /// event names, and these names are not in it, so they stay Rust-level events. Query
    /// [`Self::selected_cells`] or [`Self::selected_indexes`] for the new units.
    pub fn selection_changed_signal(&self) -> &Signal1<()> {
        &self.selection_changed
    }

    /// The signal emitted with the current navigation cell.
    pub fn current_changed_signal(&self) -> &Signal1<(usize, usize)> {
        &self.current_changed
    }

    /// The signal emitted with a pressed cell.
    pub fn cell_clicked_signal(&self) -> &Signal1<(usize, usize)> {
        &self.cell_clicked
    }

    /// The signal emitted with a double-clicked cell.
    pub fn cell_double_clicked_signal(&self) -> &Signal1<(usize, usize)> {
        &self.cell_double_clicked
    }

    /// The signal emitted after a user resize with `(column, width)`.
    pub fn column_resized_signal(&self) -> &Signal1<(usize, u32)> {
        &self.column_resized
    }

    /// The selected units, in insertion order.
    ///
    /// The stored unit is the cell itself in `Cells` behaviour, `(row, 0)` in `Rows` behaviour and
    /// `(0, column)` in `Columns` behaviour (V-004): one accessor serves all three by handing back
    /// what the selection stores, and whether a whole row or column is covered is
    /// [`Self::selection_contains`]'s lookup. A `select_all` keeps no units, so it is expanded
    /// here; a caller that needs the distinct rows or columns should prefer
    /// [`Self::selected_indexes`].
    pub fn selected_cells(&self) -> Vec<(usize, usize)> {
        if self.all_selected {
            let columns = self.column_count();
            return (0..self.row_count())
                .flat_map(|row| (0..columns).map(move |column| (row, column)))
                .collect();
        }
        self.selected_cells.iter().copied().collect()
    }

    /// The distinct rows (`Cells`/`Rows`) or columns (`Columns`) in the selection.
    pub fn selected_indexes(&self) -> Vec<usize> {
        if self.all_selected {
            return match self.selection_behavior {
                DataGridSelectionBehavior::Columns => (0..self.column_count()).collect(),
                _ => (0..self.row_count()).collect(),
            };
        }
        match self.selection_behavior {
            DataGridSelectionBehavior::Columns => {
                let mut columns: Vec<usize> =
                    self.selected_cells.iter().map(|(_, column)| *column).collect();
                columns.dedup();
                columns
            }
            _ => {
                let mut rows: Vec<usize> =
                    self.selected_cells.iter().map(|(row, _)| *row).collect();
                rows.dedup();
                rows
            }
        }
    }

    /// Selects every unit of the current behaviour.
    pub fn select_all(&mut self) {
        if self.row_count() == 0 || self.column_count() == 0 {
            return;
        }
        self.all_selected = true;
        self.selected_cells.clear();
        self.selection = Some((0, 0));
        self.current_cell.get_or_insert((0, 0));
        self.selection_changed.emit(());
        self.base.request_redraw();
    }

    /// Clears the current selection.
    pub fn clear_selection(&mut self) {
        if self.selection.is_none() && self.selected_cells.is_empty() && !self.all_selected {
            return;
        }
        self.clear_selection_units();
        self.selection_changed.emit(());
        self.base.request_redraw();
    }

    fn clear_selection_units(&mut self) {
        self.selection = None;
        self.selected_cells.clear();
        self.all_selected = false;
        self.selection_anchor = None;
    }

    /// Whether `(row, column)` belongs to the painted selection.
    pub(crate) fn selection_contains(&self, row: usize, column: usize) -> bool {
        if self.all_selected {
            return true;
        }
        match self.selection_behavior {
            DataGridSelectionBehavior::Cells => self.selected_cells.contains(&(row, column)),
            DataGridSelectionBehavior::Rows => {
                // Row units are stored as `(row, 0)`, so a row query is an ordered-set lookup
                // rather than a scan of every selected cell (V-004).
                self.selected_cells.contains(&(row, 0))
            }
            DataGridSelectionBehavior::Columns => {
                // Column units are stored as `(0, column)`, the same convention (V-004).
                self.selected_cells.contains(&(0, column))
            }
        }
    }

    /// Adds the behaviour's unit for `(row, column)`.
    pub(crate) fn push_selection_unit(&mut self, row: usize, column: usize) {
        let unit = match self.selection_behavior {
            DataGridSelectionBehavior::Cells => (row, column),
            DataGridSelectionBehavior::Rows => (row, 0),
            DataGridSelectionBehavior::Columns => (0, column),
        };
        self.selected_cells.insert(unit);
    }

    /// Adds every unit between two cells.
    pub(crate) fn push_selection_range(&mut self, from: (usize, usize), to: (usize, usize)) {
        match self.selection_behavior {
            DataGridSelectionBehavior::Cells => {
                for row in from.0.min(to.0)..=from.0.max(to.0) {
                    for column in from.1.min(to.1)..=from.1.max(to.1) {
                        self.selected_cells.insert((row, column));
                    }
                }
            }
            DataGridSelectionBehavior::Rows => {
                for row in from.0.min(to.0)..=from.0.max(to.0) {
                    self.selected_cells.insert((row, 0));
                }
            }
            DataGridSelectionBehavior::Columns => {
                for column in from.1.min(to.1)..=from.1.max(to.1) {
                    self.selected_cells.insert((0, column));
                }
            }
        }
    }

    /// Replaces the selection with `(row, column)`'s unit.
    pub(crate) fn replace_selection(&mut self, row: usize, column: usize) {
        self.selected_cells.clear();
        self.all_selected = false;
        self.selection = Some((row, column));
        self.selection_anchor = Some((row, column));
        self.push_selection_unit(row, column);
        self.selection_changed.emit(());
    }

    /// Applies a press to the selection, honouring ctrl/shift in extended mode.
    pub(crate) fn apply_selection_click(&mut self, row: usize, column: usize, modifiers: u32) {
        use crate::shortcut::Modifiers;
        if self.selection_mode == crate::widget::view_widgets::list_view::SelectionMode::None {
            return;
        }
        let modifiers = Modifiers::from_event_bits(modifiers);
        let extended =
            self.selection_mode == crate::widget::view_widgets::list_view::SelectionMode::Extended;
        let shift = extended && modifiers.contains(Modifiers::SHIFT);
        let toggle = extended
            && (modifiers.contains(Modifiers::CTRL) || modifiers.contains(Modifiers::META));

        if shift {
            let anchor = self.selection_anchor.unwrap_or((row, column));
            if !self.all_selected {
                // Keep the anchor's unit and add the range, which is what shift means.
                self.push_selection_range(anchor, (row, column));
            }
            self.selection = Some((row, column));
            self.selection_changed.emit(());
        } else if toggle {
            self.all_selected = false;
            let unit = match self.selection_behavior {
                DataGridSelectionBehavior::Cells => (row, column),
                DataGridSelectionBehavior::Rows => (row, 0),
                DataGridSelectionBehavior::Columns => (0, column),
            };
            if !self.selected_cells.remove(&unit) {
                self.selected_cells.insert(unit);
            }
            self.selection = Some((row, column));
            self.selection_anchor = Some((row, column));
            self.selection_changed.emit(());
        } else {
            self.replace_selection(row, column);
        }
    }

    /// Scrolls the minimum amount that brings `(row, column)` into view.
    fn scroll_to_cell(&mut self, row: usize, column: usize) {
        let rows = self.visible_row_capacity().max(1);
        if row < self.scroll_row {
            self.set_scroll_row(row);
        } else if row >= self.scroll_row.saturating_add(rows) {
            self.set_scroll_row(row.saturating_add(1).saturating_sub(rows));
        }
        let columns = self.visible_column_capacity().max(1);
        if column < self.scroll_column {
            self.set_scroll_column(column);
        } else if column >= self.scroll_column.saturating_add(columns) {
            self.set_scroll_column(column.saturating_add(1).saturating_sub(columns));
        }
    }

    /// Returns current horizontal scroll column.
    pub fn scroll_column(&self) -> usize {
        self.scroll_column
    }

    /// Sets vertical scroll row with clamping.
    pub fn set_scroll_row(&mut self, row: usize) {
        self.normalize_projection_state();
        let max_row = self.row_count().saturating_sub(1);
        let next = row.min(max_row);
        if next == self.scroll_row {
            return;
        }
        self.scroll_row = next;
        self.clear_cache();
        self.emit_visible_window_changed();
        self.base.request_redraw();
    }

    /// Sets horizontal scroll column with clamping.
    pub fn set_scroll_column(&mut self, column: usize) {
        self.normalize_projection_state();
        let max_column = self.column_count().saturating_sub(1);
        let next = column.min(max_column);
        if next == self.scroll_column {
            return;
        }
        self.scroll_column = next;
        self.clear_cache();
        self.emit_visible_window_changed();
        self.base.request_redraw();
    }

    /// Sets fixed row height.
    pub fn set_row_height(&mut self, row_height: u32) {
        let next = row_height.max(1);
        if self.row_height == next {
            return;
        }
        self.row_height = next;
        self.clear_cache();
        self.emit_visible_window_changed();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns row height.
    pub fn row_height(&self) -> u32 {
        self.row_height
    }

    // ── Cell tooltip (V-014) ──

    /// The tooltip text for `(row, column)`, from the installed source.
    pub fn cell_tooltip(&self, row: usize, column: usize) -> Option<String> {
        self.cell_tooltip.as_ref().and_then(|source| source(row, column))
    }

    /// Installs the per-cell tooltip source. The callback runs only for the hovered cell.
    pub fn set_cell_tooltip(
        &mut self,
        source: Arc<dyn Fn(usize, usize) -> Option<String> + Send + Sync>,
    ) {
        self.cell_tooltip = Some(source);
        self.base.request_redraw();
    }

    /// Removes the tooltip source.
    pub fn clear_cell_tooltip(&mut self) {
        if self.cell_tooltip.take().is_some() {
            self.base.request_redraw();
        }
    }

    // ── Cell delegate (V-014) ──

    /// Installs a per-cell paint delegate. Without one, cells draw directly (the default).
    pub fn set_cell_delegate(&mut self, delegate: Arc<dyn DataGridCellDelegate>) {
        self.cell_delegate = Some(delegate);
        self.base.request_redraw();
    }

    /// Removes the paint delegate.
    pub fn clear_cell_delegate(&mut self) {
        if self.cell_delegate.take().is_some() {
            self.base.request_redraw();
        }
    }

    /// Whether a paint delegate is installed.
    pub fn has_cell_delegate(&self) -> bool {
        self.cell_delegate.is_some()
    }

    // ── In-place editing (V-002) ──

    /// Begins an in-place edit of `(row, column)` and returns the editor.
    ///
    /// The grid owns the *lifecycle* — which cell is open, and what a commit means — while the
    /// host owns the gesture. [`EditTriggers`] and `edit_requested` remain the way a gesture is
    /// reported; a host that wants a built-in editor answers the signal with this call. The
    /// returned editor is seeded with the cell's current text and placed over the cell's box, so
    /// placing it needs no second geometry derivation.
    ///
    /// Returns `None` when the coordinate is outside the model: three ways for the request to be
    /// meaningless rather than errors.
    pub fn begin_edit(&mut self, row: usize, column: usize) -> Option<Box<dyn Widget>> {
        if row >= self.row_count() || column >= self.column_count() {
            return None;
        }
        let mut text = String::new();
        if let Some(source) = self.data_source.as_ref() {
            let _ = source.fill_cell(row, column, &mut text);
        }
        self.editing = Some((row, column));
        self.base.request_redraw();
        let editor = DataGridCellEditor::new(self.cell_rect(row, column), text);
        Some(Box::new(editor))
    }

    /// Reads the editor's text back and closes the edit, returning the committed value.
    ///
    /// Returns `None` when no edit is open or `editor` was not built by [`Self::begin_edit`].
    pub fn commit_edit(&mut self, editor: &dyn Widget) -> Option<String> {
        let editor = (editor as &dyn core::any::Any).downcast_ref::<DataGridCellEditor>()?;
        self.editing.take()?;
        let text = editor.text().to_string();
        self.base.request_redraw();
        Some(text)
    }

    /// Closes the open edit without committing it.
    pub fn cancel_edit(&mut self) {
        if self.editing.take().is_some() {
            self.base.request_redraw();
        }
    }

    /// The cell currently being edited, if any.
    pub fn editing_cell(&self) -> Option<(usize, usize)> {
        self.editing
    }

    // ── Row detail (V-009) ──

    /// Installs the detail-text source shown under an expanded row.
    pub fn set_row_detail_provider(&mut self, provider: RowDetailProvider) {
        self.row_detail_provider = Some(provider);
        self.base.request_redraw();
    }

    /// Removes the detail-text source; expanded rows keep their reserved band.
    pub fn clear_row_detail_provider(&mut self) {
        if self.row_detail_provider.take().is_some() {
            self.base.request_redraw();
        }
    }

    /// The detail text for `row`, from the installed provider, or `None`.
    pub fn row_detail_text(&self, row: usize) -> Option<String> {
        self.row_detail_provider.as_ref().and_then(|provider| provider(row))
    }

    /// Sets the height of `row`'s detail band when it is open.
    ///
    /// A height of zero is clamped to one pixel: an expanded row that reserves no room would be
    /// indistinguishable from a collapsed one.
    pub fn set_detail_height(&mut self, row: usize, height: u32) {
        let height = height.max(1);
        if self.detail_heights.insert(row, height) == Some(height) {
            return;
        }
        if self.expanded_detail_rows.contains(&row) {
            self.base.request_layout();
            self.base.request_redraw();
        }
    }

    /// The height of `row`'s detail band when it is open.
    pub fn detail_height(&self, row: usize) -> u32 {
        self.detail_heights.get(&row).copied().unwrap_or(DEFAULT_ROW_DETAIL_HEIGHT)
    }

    /// Whether `row`'s detail band is open.
    pub fn is_row_detail_expanded(&self, row: usize) -> bool {
        self.expanded_detail_rows.contains(&row)
    }

    /// Opens or closes `row`'s detail band, returning the new state.
    ///
    /// Returns `false` when `row` is outside the model; a toggle that changes nothing would claim
    /// a state the grid cannot paint.
    pub fn toggle_row_detail(&mut self, row: usize) -> bool {
        if row >= self.row_count() {
            return false;
        }
        let expanded = if self.expanded_detail_rows.remove(&row) {
            false
        } else {
            self.expanded_detail_rows.insert(row);
            true
        };
        self.base.request_layout();
        self.base.request_redraw();
        expanded
    }

    /// The height of `row` including its open detail band.
    pub fn row_height_at(&self, row: usize) -> u32 {
        if self.expanded_detail_rows.contains(&row) {
            self.row_height.saturating_add(self.detail_height(row))
        } else {
            self.row_height
        }
    }

    /// The top of `row` relative to the first visible row, accumulating every open detail band
    /// between the scroll position and the row (V-009).
    pub(crate) fn row_view_offset(&self, row: usize) -> i32 {
        let rows = row.saturating_sub(self.scroll_row) as u64;
        let mut offset = rows.saturating_mul(self.row_height as u64);
        for detail_row in self.expanded_detail_rows.range(self.scroll_row..row) {
            offset = offset.saturating_add(self.detail_height(*detail_row) as u64);
        }
        offset.min(i32::MAX as u64) as i32
    }

    /// The projected row whose band contains `dy`, or `None` past the model.
    pub(crate) fn row_at_dy(&self, dy: i32) -> Option<usize> {
        if dy < 0 {
            return None;
        }
        let mut row = self.scroll_row;
        let mut top = 0i64;
        let target = dy as i64;
        let count = self.row_count();
        while row < count {
            let height = self.row_height_at(row) as i64;
            if target < top.saturating_add(height) {
                return Some(row);
            }
            top = top.saturating_add(height);
            row += 1;
        }
        None
    }

    /// Whether odd rows are painted with the alternating wash.
    pub fn alternating_rows(&self) -> bool {
        self.alternating_rows
    }

    /// Enables or disables the alternating row wash.
    pub fn set_alternating_rows(&mut self, alternating: bool) {
        if self.alternating_rows == alternating {
            return;
        }
        self.alternating_rows = alternating;
        self.base.request_redraw();
    }

    /// The gestures that start an edit.
    pub fn edit_triggers(&self) -> EditTriggers {
        self.edit_triggers
    }

    /// Sets the gestures that start an edit.
    pub fn set_edit_triggers(&mut self, triggers: EditTriggers) {
        if self.edit_triggers == triggers {
            return;
        }
        self.edit_triggers = triggers;
    }

    /// Whether scroll-bar thumbs are painted.
    pub fn show_scrollbars(&self) -> bool {
        self.show_scrollbars
    }

    /// Enables or disables the scroll-bar thumbs.
    pub fn set_show_scrollbars(&mut self, show: bool) {
        if self.show_scrollbars == show {
            return;
        }
        self.show_scrollbars = show;
        self.base.request_redraw();
    }
}
