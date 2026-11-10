// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Windowed projection: filter/sort order, visible-window slicing and the caches.

use super::*;

/// A sort key normalised once per projected row.
pub(crate) fn normalise_sort_key(text: &str) -> (String, Option<f64>) {
    let trimmed = text.trim();
    (trimmed.to_lowercase(), trimmed.parse::<f64>().ok())
}

/// Compares normalised keys, preferring a numeric order when both sides are numbers.
///
/// A pure `String` comparison put `"10"` before `"2"`; a numeric column therefore painted in an
/// order no user would recognise. Parsing both sides is what makes the default comparison
/// role-aware enough for the common textual-number case, while a role explicitly supplied through
/// [`DataGrid::set_sort_role`] can expose a dedicated sort representation.
pub(crate) fn compare_sort_keys(
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

/// Rows after filtering and sorting, holding one optional cell per source column.
///
/// Only the explicit full-model mode (`DataGrid::set_full_model_projection(true)`) stores a
/// matrix of these; the default virtualized mode keeps a [`ProjectedIndex`] instead.
pub(crate) type ProjectedRows = Vec<Vec<Option<String>>>;

/// The display order of a virtualized projection: the source row for each projected row.
///
/// A sort reorders these indices and a filter drops them, so a projection costs one `usize` per
/// row and never a cloned row or cell (V-002).
pub(crate) type ProjectedIndex = Vec<usize>;

/// The sliced window most recently handed out from the full-model projection.
///
/// Keyed by the projection allocation and the window rectangle: a repeated paint returns the
/// same `Rc` rather than copying the visible slice again. The pointer comparison also makes a
/// stale window unreachable when the projection is rebuilt.
pub(crate) type FullWindowCache =
    Option<(Rc<ProjectedRows>, (usize, usize, usize, usize), Rc<Vec<Vec<Option<String>>>>)>;

/// The window most recently fetched for a virtualized projection.
///
/// Keyed by the projection allocation and the window rectangle, so a scroll reuses nothing stale
/// and a repeated paint is a refcount bump. The matrix holds **only** the window's rows: the
/// model itself is never materialised (V-001).
pub(crate) type VirtualWindowCache =
    Option<(Rc<ProjectedIndex>, (usize, usize, usize, usize), Rc<Vec<Vec<Option<String>>>>)>;

#[derive(Clone, Debug, PartialEq, Eq)]
/// Cached window of source cells, shared with every reader instead of copied per frame.
pub(crate) struct GridWindowCache {
    pub(crate) row_start: usize,
    pub(crate) row_len: usize,
    pub(crate) column_start: usize,
    pub(crate) column_len: usize,
    pub(crate) revision: u64,
    pub(crate) cells: Rc<Vec<Vec<Option<String>>>>,
    /// The filter/sort projection of `cells`, produced once and reused on later frames.
    ///
    /// The shared-window fix only covered the no-filter/no-sort path; with either active,
    /// every frame cloned the whole window and re-ran `retain` + `sort_by` over the copy.
    /// `None` means "not projected yet"; `clear_cache()` drops it together with the window.
    pub(crate) projected: Option<Rc<Vec<Vec<Option<String>>>>>,
}

/// A read-only, column-sliced view of the projected window the draw pass paints.
pub(crate) struct ProjectedWindowView {
    pub(crate) rows: Rc<ProjectedRows>,
    pub(crate) row_start: usize,
    pub(crate) row_len: usize,
    pub(crate) col_start: usize,
    pub(crate) col_len: usize,
}

impl ProjectedWindowView {
    pub(crate) fn is_empty(&self) -> bool {
        self.row_len == 0 || self.col_len == 0
    }

    pub(crate) fn len(&self) -> usize {
        self.row_len
    }

    /// The window-local row `index`, sliced to the visible column span.
    pub(crate) fn row(&self, index: usize) -> Option<&[Option<String>]> {
        if index >= self.row_len {
            return None;
        }
        let row = self.rows.get(self.row_start.checked_add(index)?)?;
        let start = self.col_start.min(row.len());
        let end = self.col_start.saturating_add(self.col_len).min(row.len());
        row.get(start..end)
    }
}

impl DataGrid {
    /// The revision that changes whenever the source's contents may have moved.
    pub(crate) fn data_projection_revision(&self) -> u64 {
        let revision = self.data_source.as_ref().map_or(0, |source| source.revision());
        if revision > 0 {
            revision
        } else {
            self.data_generation.load(Ordering::Relaxed)
        }
    }

    /// Whether the filter and sort projection covers the whole model.
    pub fn full_model_projection(&self) -> bool {
        self.full_model_projection
    }

    /// Chooses between the virtualized projection (default) and whole-model materialisation.
    ///
    /// # Default: virtualized
    ///
    /// The default path projects the model as an **index**: filtering drops source rows and
    /// sorting reorders their indices, then only the visible window is fetched. A 10k-row grid
    /// therefore no longer holds a 10k-row copy of its cells, and a scroll fetches one window
    /// (V-001/V-002). The visible order is the same as the materialised projection.
    ///
    /// `true` restores the historical whole-model fetch. It is kept as an explicit option for a
    /// source that is cheap to read once and expensive to re-read per window; it materialises
    /// every row and cell, which is why it is no longer the default.
    pub fn set_full_model_projection(&mut self, enabled: bool) {
        if self.full_model_projection == enabled {
            return;
        }
        self.full_model_projection = enabled;
        self.invalidate_projection_cache();
        self.clear_cache();
        self.base.request_redraw();
    }

    /// Drops every derived projection and window, keeping only source row identity.
    ///
    /// One place for the invalidation so a new cache cannot be forgotten at one of the sort,
    /// filter or data-change call sites.
    pub(crate) fn invalidate_projection_cache(&mut self) {
        self.full_projection_cache = None;
        self.index_projection_cache = None;
        self.full_window_cache = None;
        self.virtual_window_cache = None;
    }

    /// Returns `(row_start, row_len, col_start, col_len)` for visible+overscan window.
    ///
    /// When the grid has frozen leading columns, the fetch starts at column `0` and the
    /// returned `col_len` reaches from there through the scrolling window. Frozen columns are
    /// painted outside the scroll offset (`cell_rect`), so they cannot be omitted from the
    /// fetched matrix: the draw pass would then have no values to repaint over the scrolling
    /// cells. With no frozen columns the window is exactly the scrolling window it always was.
    pub fn visible_window(&self) -> (usize, usize, usize, usize) {
        let row_count = self.row_count();
        let column_count = self.column_count();
        if row_count == 0 || column_count == 0 {
            return (0, 0, 0, 0);
        }

        let row_start = self.scroll_row.saturating_sub(self.overscan_rows);
        let col_start = self.scroll_column.saturating_sub(self.overscan_columns);

        let visible_rows = self.visible_row_capacity();
        let visible_cols = self.visible_column_capacity();

        let row_fetch = visible_rows.saturating_add(self.overscan_rows.saturating_mul(2));
        let col_fetch = visible_cols.saturating_add(self.overscan_columns.saturating_mul(2));

        let row_end = row_start.saturating_add(row_fetch).min(row_count);
        let col_end = col_start.saturating_add(col_fetch).min(column_count);
        // A frozen prefix is part of what the draw pass needs even when it lies left of the
        // scrolling window. Reaching all the way back to column 0 also keeps the fetched
        // matrix contiguous, which is what `fetch_window` returns and what the cached window
        // is keyed on.
        let col_start = if self.frozen_columns > 0 { 0 } else { col_start };
        let col_len = col_end.saturating_sub(col_start);

        (row_start, row_end.saturating_sub(row_start), col_start, col_len)
    }

    /// The projected row/column the fetch window starts at, i.e. the scroll position pulled back by
    /// the overscan.
    ///
    /// # Why this exists (D09-VIEW-01)
    ///
    /// `visible_window()` fetches from `row_start = scroll - overscan`, so the matrix the source
    /// returns begins *before* the viewport. The draw pass used to enumerate that matrix from its
    /// local row `0`, which painted the leading overscan rows as if they were the first visible
    /// ones; and `cell_rect` already places a cell from its own projected column, so the local
    /// column reached it un-offset and the leading overscan columns overlapped the first visible
    /// column. The draw pass and `cell_at` both convert a window-local index through this origin, so
    /// a painted cell and the cell a press resolves to use one index semantics.
    pub(crate) fn window_origin(&self) -> (usize, usize) {
        let (row_start, _, col_start, _) = self.visible_window();
        (row_start, col_start)
    }

    /// Fetches visible cells, applying filters and sort rules in window scope.
    pub fn fetch_visible_cells(&mut self) -> Vec<Vec<Option<String>>> {
        (*self.fetch_visible_cells_shared()).clone()
    }

    /// The draw path's form: a cache hit with no filter/sort is the cached matrix itself.
    ///
    /// The owned form used to deep-copy the whole window on every frame, then run `retain`
    /// and `sort_by` over the copy. With no filter/sort in effect the projection is the
    /// cached matrix, so returning its `Rc` makes a hit a refcount bump.
    pub fn fetch_visible_cells_shared(&mut self) -> Rc<Vec<Vec<Option<String>>>> {
        if self.full_model_projection {
            return self.fetch_full_projected_window();
        }
        self.fetch_visible_window_view().rows
    }

    /// The draw pass's window: a borrowed, column-sliced view of the projection.
    pub(crate) fn fetch_visible_window_view(&mut self) -> ProjectedWindowView {
        self.normalize_projection_state();
        if self.full_model_projection {
            let projected = self.full_projection_rows();
            let (row_start, row_len, col_start, col_len) = self.visible_window();
            return ProjectedWindowView { rows: projected, row_start, row_len, col_start, col_len };
        }
        // With no projection in effect the window fetch *is* the virtualized path: it pulls only
        // the visible rows and caches them. A filter or sort needs the model-wide index first.
        if self.filter_expr.is_noop() && self.sort_specs.is_empty() {
            let cells = self.fetch_window_shared();
            let row_len = cells.len();
            let col_len = cells.first().map_or(0, Vec::len);
            return ProjectedWindowView {
                rows: cells,
                row_start: 0,
                row_len,
                col_start: 0,
                col_len,
            };
        }
        self.virtual_window_view()
    }

    /// Fetches only the window rows named by the virtualized projection.
    ///
    /// The sorted order is a list of source indices, so a window needs one single-row fetch per
    /// visible row rather than a whole-model fetch followed by a slice (V-001/V-002). The result
    /// is cached against the projection allocation and the window rectangle, so a repaint is a
    /// refcount bump and a stale projection cannot be served after a rebuild.
    fn virtual_window_view(&mut self) -> ProjectedWindowView {
        let Some(source) = self.data_source.clone() else {
            return ProjectedWindowView {
                rows: Rc::new(Vec::new()),
                row_start: 0,
                row_len: 0,
                col_start: 0,
                col_len: 0,
            };
        };
        let (row_start, row_len, col_start, col_len) = self.visible_window();
        if row_len == 0 || col_len == 0 {
            return ProjectedWindowView {
                rows: Rc::new(Vec::new()),
                row_start: 0,
                row_len: 0,
                col_start: 0,
                col_len: 0,
            };
        }
        let index = self.index_projection();
        let window = (row_start, row_len, col_start, col_len);
        if let Some((cached_index, cached_window, cells)) = self.virtual_window_cache.as_ref() {
            if Rc::ptr_eq(cached_index, &index) && *cached_window == window {
                let row_len = cells.len();
                return ProjectedWindowView {
                    rows: Rc::clone(cells),
                    row_start: 0,
                    row_len,
                    col_start: 0,
                    col_len,
                };
            }
        }
        let end = row_start.saturating_add(row_len).min(index.len());
        let mut rows = Vec::with_capacity(end.saturating_sub(row_start));
        for projected in row_start..end {
            let mut fetched = source.fetch_window(index[projected], 1, col_start, col_len);
            rows.push(fetched.pop().unwrap_or_default());
        }
        let cells = Rc::new(rows);
        let row_len = cells.len();
        self.virtual_window_cache = Some((Rc::clone(&index), window, Rc::clone(&cells)));
        ProjectedWindowView { rows: cells, row_start: 0, row_len, col_start: 0, col_len }
    }

    /// The window-scoped fetch used when full-model projection is off.
    fn fetch_window_shared(&mut self) -> Rc<Vec<Vec<Option<String>>>> {
        self.normalize_projection_state();

        let Some(source) = self.data_source.as_ref() else {
            return Rc::new(Vec::new());
        };

        let (row_start, row_len, col_start, col_len) = self.visible_window();
        if row_len == 0 || col_len == 0 {
            return Rc::new(Vec::new());
        }

        let revision = source.revision();
        let cache_revision =
            if revision > 0 { revision } else { self.data_generation.load(Ordering::Relaxed) };
        if self.data_is_observable {
            let mut cached_projection = None;
            let mut cached_cells = None;
            if let Some(cache) = self.window_cache.as_ref() {
                if cache.row_start == row_start
                    && cache.row_len == row_len
                    && cache.column_start == col_start
                    && cache.column_len == col_len
                    && cache.revision == cache_revision
                {
                    if self.filter_expr.is_noop() && self.sort_specs.is_empty() {
                        return Rc::clone(&cache.cells);
                    }
                    cached_projection = cache.projected.clone();
                    cached_cells = Some(Rc::clone(&cache.cells));
                }
            }
            if let Some(projected) = cached_projection {
                return projected;
            }
            if let Some(cells) = cached_cells {
                let projected = Rc::new(self.apply_filter_sort((*cells).clone(), row_start));
                if let Some(cache) = self.window_cache.as_mut() {
                    cache.projected = Some(Rc::clone(&projected));
                }
                return projected;
            }
        }

        let cells = Rc::new(source.fetch_window(row_start, row_len, col_start, col_len));
        if self.data_is_observable {
            self.window_cache = Some(GridWindowCache {
                row_start,
                row_len,
                column_start: col_start,
                column_len: col_len,
                revision: cache_revision,
                cells: Rc::clone(&cells),
                projected: None,
            });
        } else {
            self.window_cache = None;
        }

        if self.filter_expr.is_noop() && self.sort_specs.is_empty() {
            cells
        } else {
            let projected = Rc::new(self.apply_filter_sort((*cells).clone(), row_start));
            if let Some(cache) = self.window_cache.as_mut() {
                cache.projected = Some(Rc::clone(&projected));
            }
            projected
        }
    }

    /// Marks the bound source's contents as changed and drops every derived cache.
    pub fn refresh_data(&mut self) {
        self.data_generation.fetch_add(1, Ordering::Relaxed);
        self.invalidate_projection_cache();
        self.clear_cache();
        self.emit_visible_window_changed();
        self.base.request_redraw();
    }

    /// Fetches the whole model, projects it once, and slices the visible window.
    fn fetch_full_projected_window(&mut self) -> Rc<Vec<Vec<Option<String>>>> {
        let projected = self.full_projection_rows();
        if projected.is_empty() {
            return Rc::new(Vec::new());
        }
        let window = self.visible_window();
        if let Some((cached_projection, cached_window, cells)) = self.full_window_cache.as_ref() {
            if Rc::ptr_eq(cached_projection, &projected) && *cached_window == window {
                return Rc::clone(cells);
            }
        }
        let window_cells = Rc::new(Self::slice_window(&projected, window));
        self.full_window_cache = Some((Rc::clone(&projected), window, Rc::clone(&window_cells)));
        window_cells
    }

    /// The filtered/sorted full projection, rebuilt only when its cache key moves.
    fn full_projection_rows(&mut self) -> Rc<ProjectedRows> {
        let Some(source) = self.data_source.as_ref() else {
            return Rc::new(Vec::new());
        };
        let revision = source.revision();
        let cache_key =
            if revision > 0 { revision } else { self.data_generation.load(Ordering::Relaxed) };
        if let Some((key, projected)) = self.full_projection_cache.as_ref() {
            if *key == cache_key {
                return Rc::clone(projected);
            }
        }
        let rows = source.row_count();
        let columns = source.column_count();
        let cells = source.fetch_window(0, rows, 0, columns);
        let projected = Rc::new(self.apply_filter_sort(cells, 0));
        self.full_projection_cache = Some((cache_key, Rc::clone(&projected)));
        projected
    }

    /// The virtualized projection: which source row is painted at each projected row.
    ///
    /// The model is read one row at a time, and the result holds only indices. Filtering drops a
    /// row's index instead of retaining its cloned cells, and sorting reorders indices against a
    /// key table built once per spec — so a whole-model projection allocates `usize`s and keys,
    /// never a cloned row matrix (V-002).
    pub(crate) fn index_projection(&mut self) -> Rc<ProjectedIndex> {
        let Some(source) = self.data_source.clone() else {
            return Rc::new(Vec::new());
        };
        let revision = source.revision();
        let cache_key =
            if revision > 0 { revision } else { self.data_generation.load(Ordering::Relaxed) };
        if let Some((key, index)) = self.index_projection_cache.as_ref() {
            if *key == cache_key {
                return Rc::clone(index);
            }
        }

        let rows = source.row_count();
        let columns = source.column_count();
        let mut order: Vec<usize> = Vec::with_capacity(rows);
        if self.filter_expr.is_noop() {
            order.extend(0..rows);
        } else {
            // Filtering evaluates one row at a time. The row is transient: if it is rejected its
            // cells are dropped immediately rather than retained for a later slice.
            for row in 0..rows {
                let mut values = source.fetch_window(row, 1, 0, columns);
                if values.pop().is_some_and(|values| self.filter_expr.accepts(&values)) {
                    order.push(row);
                }
            }
        }

        if !self.sort_specs.is_empty() && !order.is_empty() {
            // One normalised key per spec per surviving row, computed once. The comparator reads
            // this table; it never re-reads the source (V-002).
            let mut key_sets: Vec<Vec<Option<(String, Option<f64>)>>> =
                Vec::with_capacity(self.sort_specs.len());
            for spec in &self.sort_specs {
                let role = self.sort_roles.get(&spec.column).copied();
                let mut keys = Vec::with_capacity(order.len());
                for &row in &order {
                    let role_value = role
                        .and_then(|role| source.data_role(row, spec.column, role))
                        .or_else(|| source.data(row, spec.column));
                    keys.push(Some(normalise_sort_key(role_value.as_deref().unwrap_or_default())));
                }
                key_sets.push(keys);
            }
            let empty = (String::new(), None);
            let mut positions: Vec<usize> = (0..order.len()).collect();
            positions.sort_by(|&left, &right| {
                for (index, spec) in self.sort_specs.iter().enumerate() {
                    let left_key = key_sets[index][left].as_ref().unwrap_or(&empty);
                    let right_key = key_sets[index][right].as_ref().unwrap_or(&empty);
                    let ordering = compare_sort_keys(left_key, right_key);
                    if ordering != core::cmp::Ordering::Equal {
                        return if spec.descending { ordering.reverse() } else { ordering };
                    }
                }
                // Stable tie-break on the source row, exactly as the materialised path does.
                order[left].cmp(&order[right])
            });
            order = positions.into_iter().map(|position| order[position]).collect();
        }

        let index = Rc::new(order);
        self.index_projection_cache = Some((cache_key, Rc::clone(&index)));
        index
    }

    fn slice_window(
        rows: &[Vec<Option<String>>],
        (row_start, row_len, col_start, col_len): (usize, usize, usize, usize),
    ) -> Vec<Vec<Option<String>>> {
        rows.iter()
            .skip(row_start)
            .take(row_len)
            .map(|row| row.iter().skip(col_start).take(col_len).cloned().collect::<Vec<_>>())
            .collect()
    }

    /// Filters and sorts projected rows, preserving each row's source identity.
    ///
    /// `row_origin` is the source row the first element of `cells` came from. It is what makes a
    /// role-aware sort key possible after a window fetch: the display text alone cannot tell the
    /// sorter that "10" is greater than "2", but asking the source for the configured role at the
    /// original row can. The rows stay paired with their source index through the filter pass, so a
    /// rejected row cannot shift the keys used by the survivors.
    pub(crate) fn apply_filter_sort(
        &self,
        cells: Vec<Vec<Option<String>>>,
        row_origin: usize,
    ) -> Vec<Vec<Option<String>>> {
        let row_span = cells.len();
        let mut rows: Vec<(usize, Vec<Option<String>>)> = cells
            .into_iter()
            .enumerate()
            .map(|(offset, row)| (row_origin.saturating_add(offset), row))
            .collect();
        // `is_noop` lets the whole `retain` pass be skipped when the filter cannot
        // reject anything, rather than walking every cell to conclude the same thing.
        if !self.filter_expr.is_noop() {
            rows.retain(|(_, row)| self.filter_expr.accepts(row));
        }

        if !self.sort_specs.is_empty() {
            let source = self.data_source.as_deref();
            // One key per spec per surviving row, computed once. A comparator that re-read the
            // source (and re-parsed the text) for every pair did `O(n log n)` source reads and
            // allocations for one sort.
            let mut key_sets: Vec<Vec<Option<(String, Option<f64>)>>> =
                Vec::with_capacity(self.sort_specs.len());
            for spec in &self.sort_specs {
                let role = self.sort_roles.get(&spec.column).copied();
                let mut keys: Vec<Option<(String, Option<f64>)>> = vec![None; row_span];
                for (source_row, row) in &rows {
                    let role_value = match (role, source) {
                        (Some(role), Some(source)) => {
                            source.data_role(*source_row, spec.column, role)
                        }
                        _ => None,
                    };
                    let display = row
                        .get(spec.column)
                        .and_then(|cell| cell.as_ref())
                        .cloned()
                        .unwrap_or_default();
                    if let Some(slot) = keys.get_mut(source_row.saturating_sub(row_origin)) {
                        *slot = Some(normalise_sort_key(&role_value.unwrap_or(display)));
                    }
                }
                key_sets.push(keys);
            }

            let empty = (String::new(), None);
            rows.sort_by(|(left_row, _), (right_row, _)| {
                let left = *left_row - row_origin;
                let right = *right_row - row_origin;
                for (index, spec) in self.sort_specs.iter().enumerate() {
                    let left_key = key_sets[index].get(left).and_then(|key| key.as_ref());
                    let right_key = key_sets[index].get(right).and_then(|key| key.as_ref());
                    let ordering =
                        compare_sort_keys(left_key.unwrap_or(&empty), right_key.unwrap_or(&empty));
                    if ordering != core::cmp::Ordering::Equal {
                        return if spec.descending { ordering.reverse() } else { ordering };
                    }
                }
                left_row.cmp(right_row)
            });
        }

        rows.into_iter().map(|(_, row)| row).collect()
    }

    pub(crate) fn visible_row_capacity(&self) -> usize {
        // The cells box, not the control's box: the band inset and the column-title strip are
        // not paintable rows. Dividing the control height by the row height over-counted by a
        // row here (600x300 and 20 px rows answered 15 while the paint stopped at 14), which
        // let `set_current_cell` leave a row that was never painted off-screen.
        let height = self.cells_box().height;
        if height == 0 {
            return 0;
        }
        // `div_ceil` on the max'd row height: the plain `(height + row_height - 1)` overflowed when
        // both operands were near `u32::MAX`, and the sibling `virtual_table` already used `div_ceil`.
        height.div_ceil(self.row_height.max(1)) as usize
    }

    pub(crate) fn visible_column_capacity(&self) -> usize {
        // Same cells-box reasoning as `visible_row_capacity`; the 4 px horizontal inset was the
        // equivalent error here.
        let width = self.cells_box().width;
        if width == 0 {
            return 0;
        }
        // Same `div_ceil` reasoning as `visible_row_capacity`.
        width.div_ceil(self.effective_column_width().max(1)) as usize
    }

    /// The number of rows the paint pass enumerates.
    ///
    /// The source's row count when nothing filters it (a sort reorders rows but never removes
    /// one), otherwise the cached projection's length. Hit testing reads this because a cell's box
    /// only exists for a projected row: clamping against `row_count()` named blank space below the
    /// last painted row of a filtered grid.
    pub(crate) fn projected_row_count(&self) -> usize {
        if self.filter_expr.is_noop() {
            return self.row_count();
        }
        let key = self.data_projection_revision();
        if let Some((cached_key, index)) = self.index_projection_cache.as_ref() {
            if *cached_key == key {
                return index.len();
            }
        }
        if let Some((cached_key, projected)) = self.full_projection_cache.as_ref() {
            if *cached_key == key {
                return projected.len();
            }
        }
        // No projection has been built since the last filter change; the paint that follows will
        // build it, and until then the source count is the only honest upper bound.
        self.row_count()
    }

    /// [`Self::projected_row_count`] with the projection built on demand.
    ///
    /// Keyboard navigation owns `&mut self`, so it can build the index rather than fall back to
    /// the source count; that fallback is what let a key walk onto rows the grid never painted.
    pub(crate) fn projected_row_count_mut(&mut self) -> usize {
        if self.filter_expr.is_noop() {
            self.row_count()
        } else {
            self.index_projection().len()
        }
    }

    pub(crate) fn normalize_projection_state(&mut self) {
        let row_count = self.row_count();
        let col_count = self.column_count();
        if row_count == 0 {
            self.scroll_row = 0;
        } else if self.scroll_row >= row_count {
            self.scroll_row = row_count.saturating_sub(1);
        }
        if col_count == 0 {
            self.scroll_column = 0;
            self.frozen_columns = 0;
        } else {
            if self.scroll_column >= col_count {
                self.scroll_column = col_count.saturating_sub(1);
            }
            self.frozen_columns = self.frozen_columns.min(col_count);
        }
        // Cell references that survived a shrink would point at rows or columns that no longer
        // exist, so filter them against the final projected size. `selection` is the cursor the
        // ring draws, so it is filtered with them; leaving it behind painted the ring far below a
        // shrunken grid.
        self.current_cell =
            self.current_cell.filter(|(row, column)| *row < row_count && *column < col_count);
        self.hovered_cell =
            self.hovered_cell.filter(|(row, column)| *row < row_count && *column < col_count);
        self.selection =
            self.selection.filter(|(row, column)| *row < row_count && *column < col_count);
        self.selected_cells.retain(|(row, column)| *row < row_count && *column < col_count);
    }

    /// Drops every column-indexed fact that names a column the bound source no longer has.
    ///
    /// Re-binding a source can shrink the column space, while the filter, sort, hidden and
    /// description state address columns by absolute index. An out-of-range filter rejects every
    /// row (the grid goes blank yet still reports `filter_count == 1`); an out-of-range hidden
    /// index makes `Stretch` divide the box by the wrong number of visible columns.
    ///
    /// Returns whether the filter expression changed, so the caller can report `filter_changed`.
    /// The sort removal is deliberately not reported through `sort_changed`: that signal carries
    /// `(column, descending)` and has no spelling for "removed" (the same reason an unsorted
    /// header is observable only through `sort_specs()`).
    pub(crate) fn prune_column_indexed_state(&mut self) -> bool {
        let columns = self.column_count();
        let filter_changed = {
            let conditions = self.filter_expr.conditions();
            conditions
                .iter()
                .map(|condition| condition.column)
                .max()
                .is_some_and(|max| max >= columns)
        };
        if filter_changed {
            let max_column = self
                .filter_expr
                .conditions()
                .iter()
                .map(|condition| condition.column)
                .max()
                .unwrap_or(columns);
            for column in columns..=max_column {
                self.filter_expr = self.filter_expr.without_column(column);
            }
            // Rebuild the flat mirror from the evaluated tree, so the two cannot describe
            // different filters.
            self.filters = self
                .filter_expr
                .conditions()
                .into_iter()
                .map(|condition| ColumnFilter {
                    column: condition.column,
                    query: condition.operand.clone(),
                })
                .collect();
        }
        self.sort_specs.retain(|spec| spec.column < columns);
        self.sort_roles.retain(|column, _| *column < columns);
        self.hidden_columns.retain(|column| *column < columns);
        self.column_widths.retain(|column, _| *column < columns);
        self.column_alignments.retain(|column, _| *column < columns);
        self.column_order.retain(|column| *column < columns);
        if self.header_columns.len() > columns {
            self.header_columns.truncate(columns);
            self.header_labels.truncate(columns);
            for row in &mut self.extra_header_rows {
                row.truncate(columns);
            }
            self.header_text_revision = self.header_text_revision.wrapping_add(1);
        }
        self.frozen_columns = self.frozen_columns.min(columns);
        filter_changed
    }

    pub(crate) fn clear_cache(&mut self) {
        self.window_cache = None;
    }

    pub(crate) fn emit_visible_window_changed(&self) {
        self.visible_window_changed.emit(self.visible_window());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Source {
        data: Vec<Vec<String>>,
    }

    impl IncrementalTableDataSource for Source {
        fn row_count(&self) -> usize {
            self.data.len()
        }

        fn column_count(&self) -> usize {
            self.data.first().map_or(0, Vec::len)
        }

        fn data(&self, row: usize, column: usize) -> Option<String> {
            self.data.get(row).and_then(|line| line.get(column)).cloned()
        }
    }

    /// Capacity is measured against the cells box (band inset and column-title strip removed) —
    /// the box the paint actually fills. Dividing the control's own height answered 15 where the
    /// paint stopped at 14, so `set_current_cell(14, ..)` left that row off-screen forever.
    #[test]
    fn visible_capacity_measures_the_cells_box() {
        let mut grid = DataGrid::new(Rect::new(0, 0, 600, 300));
        grid.set_data_source(Arc::new(Source {
            data: (0..20)
                .map(|row| (0..4).map(|column| format!("{row}:{column}")).collect())
                .collect(),
        }));

        let cells = grid.cells_box();
        assert_eq!(cells.height, 274, "300 minus 4 px of inset and the 22 px title strip");
        assert_eq!(grid.visible_row_capacity(), 14, "274 / 20 rounds up to 14, not 15");

        assert!(grid.set_current_cell(14, 0));
        assert_eq!(grid.scroll_row(), 1, "the capacity is what decides whether to scroll");
        let row = grid.cell_rect(14, 0);
        assert!(
            row.y >= cells.y && row.y < cells.y + cells.height as i32,
            "the scrolled-to row is inside the painted box ({row:?})"
        );

        // 596 / 199 = 2.99.. rounds up to 3; the control width (600) would answer 4.
        grid.set_default_column_width(199);
        assert_eq!(grid.visible_column_capacity(), 3);
        let cells = grid.cells_box();
        assert!(grid.cell_rect(0, 2).x < cells.x + cells.width as i32, "column 2 is painted");
        assert!(
            grid.cell_rect(0, 3).x >= cells.x + cells.width as i32,
            "column 3 starts past the last painted pixel"
        );
    }

    /// A model that shrinks under the grid must drop the `selection` cursor too; it is the box the
    /// focus ring draws, and leaving it behind painted the ring far below the grid.
    #[test]
    fn a_shrunk_model_drops_a_selection_cursor_past_the_last_row() {
        struct Shrinkable {
            rows: std::sync::atomic::AtomicUsize,
            columns: usize,
        }
        impl IncrementalTableDataSource for Shrinkable {
            fn row_count(&self) -> usize {
                self.rows.load(std::sync::atomic::Ordering::Relaxed)
            }
            fn column_count(&self) -> usize {
                self.columns
            }
            fn data(&self, row: usize, column: usize) -> Option<String> {
                Some(format!("{row}:{column}"))
            }
        }

        let source =
            Arc::new(Shrinkable { rows: std::sync::atomic::AtomicUsize::new(100), columns: 2 });
        let mut grid = DataGrid::new(Rect::new(0, 0, 600, 300));
        grid.set_data_source(source.clone());
        grid.replace_selection(50, 0);
        grid.current_cell = Some((50, 0));

        source.rows.store(10, std::sync::atomic::Ordering::Relaxed);
        grid.refresh_data();
        // The fetch is the normalization point the paint and hit test share.
        grid.fetch_visible_cells();

        assert_eq!(grid.selection(), None, "row 50 is no longer in the model");
        assert_eq!(grid.current_cell(), None);
        assert!(grid.selected_cells().is_empty());
    }
}
