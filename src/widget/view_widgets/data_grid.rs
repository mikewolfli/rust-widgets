//! Data grid widget backed by incremental data source protocol.

use std::sync::Arc;

use crate::core::{HorizontalAlignment, Color, Font, Point, Rect};
use crate::event::Event;
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, Signal1};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

use super::data_source::IncrementalTableDataSource;

/// Sort descriptor for a data grid column.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SortSpec {
    /// Zero-based source column index.
    pub column: usize,
    /// Descending sort when true.
    pub descending: bool,
}

/// Contains-based text filter bound to a source column.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColumnFilter {
    /// Zero-based source column index.
    pub column: usize,
    /// Query token applied with case-insensitive contains.
    pub query: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct GridWindowCache {
    row_start: usize,
    row_len: usize,
    column_start: usize,
    column_len: usize,
    revision: u64,
    cells: Vec<Vec<Option<String>>>,
}

/// Virtualized data grid with windowed pull from incremental table data source.
pub struct DataGrid {
    base: BaseWidget,
    data_source: Option<Arc<dyn IncrementalTableDataSource>>,
    data_source_connection_scope: ConnectionScope,
    scroll_row: usize,
    scroll_column: usize,
    row_height: u32,
    column_width: u32,
    overscan_rows: usize,
    overscan_columns: usize,
    frozen_columns: usize,
    sort_specs: Vec<SortSpec>,
    filters: Vec<ColumnFilter>,
    window_cache: Option<GridWindowCache>,
    /// Emitted when visible row/column window changes.
    pub visible_window_changed: Signal1<(usize, usize, usize, usize)>,
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
            window_cache: None,
            visible_window_changed: Signal1::new(),
        }
    }

    /// Binds incremental source.
    pub fn set_data_source(&mut self, data_source: Arc<dyn IncrementalTableDataSource>) {
        self.data_source_connection_scope = ConnectionScope::new();
        if let Some(changed) = data_source.data_changed_signal() {
            let redraw = self.base.redraw_requested_signal().clone();
            let layout = self.base.layout_requested_signal().clone();
            changed.connect_scoped(&self.data_source_connection_scope, move || {
                redraw.emit();
                layout.emit();
            });
        }
        self.data_source = Some(data_source);
        self.scroll_row = 0;
        self.scroll_column = 0;
        self.clear_cache();
        self.normalize_projection_state();
        self.emit_visible_window_changed();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Clears incremental source.
    pub fn clear_data_source(&mut self) {
        self.data_source_connection_scope = ConnectionScope::new();
        self.data_source = None;
        self.scroll_row = 0;
        self.scroll_column = 0;
        self.frozen_columns = 0;
        self.clear_cache();
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

    /// Sets fixed column width.
    pub fn set_column_width(&mut self, column_width: u32) {
        let next = column_width.max(1);
        if self.column_width == next {
            return;
        }
        self.column_width = next;
        self.clear_cache();
        self.emit_visible_window_changed();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns column width.
    pub fn column_width(&self) -> u32 {
        self.column_width
    }

    /// Sets frozen leading columns, clamped to source column count.
    pub fn set_frozen_columns(&mut self, frozen_columns: usize) {
        let next = frozen_columns.min(self.column_count());
        if self.frozen_columns == next {
            return;
        }
        self.frozen_columns = next;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns frozen leading column count.
    pub fn frozen_columns(&self) -> usize {
        self.frozen_columns
    }

    /// Replaces sort spec list in priority order.
    pub fn set_sort_specs(&mut self, sort_specs: Vec<SortSpec>) {
        self.sort_specs = sort_specs;
        self.clear_cache();
        self.base.request_redraw();
    }

    /// Returns active sort specs.
    pub fn sort_specs(&self) -> &[SortSpec] {
        &self.sort_specs
    }

    /// Replaces filter list.
    pub fn set_filters(&mut self, filters: Vec<ColumnFilter>) {
        self.filters = filters;
        self.clear_cache();
        self.base.request_redraw();
    }

    /// Returns active filters.
    pub fn filters(&self) -> &[ColumnFilter] {
        &self.filters
    }

    /// Returns `(row_start, row_len, col_start, col_len)` for visible+overscan window.
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

        (row_start, row_end.saturating_sub(row_start), col_start, col_end.saturating_sub(col_start))
    }

    /// Fetches visible cells, applying filters and sort rules in window scope.
    pub fn fetch_visible_cells(&mut self) -> Vec<Vec<Option<String>>> {
        self.normalize_projection_state();

        let Some(source) = self.data_source.as_ref() else {
            return Vec::new();
        };

        let (row_start, row_len, col_start, col_len) = self.visible_window();
        if row_len == 0 || col_len == 0 {
            return Vec::new();
        }

        let revision = source.revision();
        if revision > 0 {
            if let Some(cache) = self.window_cache.as_ref() {
                if cache.row_start == row_start
                    && cache.row_len == row_len
                    && cache.column_start == col_start
                    && cache.column_len == col_len
                    && cache.revision == revision
                {
                    return self.apply_filter_sort(cache.cells.clone());
                }
            }
        }

        let cells = source.fetch_window(row_start, row_len, col_start, col_len);
        if revision > 0 {
            self.window_cache = Some(GridWindowCache {
                row_start,
                row_len,
                column_start: col_start,
                column_len: col_len,
                revision,
                cells: cells.clone(),
            });
        } else {
            self.window_cache = None;
        }

        self.apply_filter_sort(cells)
    }

    fn apply_filter_sort(&self, mut cells: Vec<Vec<Option<String>>>) -> Vec<Vec<Option<String>>> {
        if !self.filters.is_empty() {
            cells.retain(|row| {
                self.filters.iter().all(|filter| {
                    let query = filter.query.to_lowercase();
                    row.get(filter.column)
                        .and_then(|cell| cell.as_ref())
                        .map(|cell| cell.to_lowercase().contains(&query))
                        .unwrap_or(false)
                })
            });
        }

        if !self.sort_specs.is_empty() {
            cells.sort_by(|a, b| {
                for spec in &self.sort_specs {
                    let left = a
                        .get(spec.column)
                        .and_then(|cell| cell.as_ref())
                        .map(String::as_str)
                        .unwrap_or("");
                    let right = b
                        .get(spec.column)
                        .and_then(|cell| cell.as_ref())
                        .map(String::as_str)
                        .unwrap_or("");
                    let order = left.cmp(right);
                    if order != std::cmp::Ordering::Equal {
                        return if spec.descending { order.reverse() } else { order };
                    }
                }
                std::cmp::Ordering::Equal
            });
        }

        cells
    }

    fn visible_row_capacity(&self) -> usize {
        let height = self.base.geometry().height;
        if height == 0 {
            return 0;
        }
        ((height + self.row_height - 1) / self.row_height.max(1)) as usize
    }

    fn visible_column_capacity(&self) -> usize {
        let width = self.base.geometry().width;
        if width == 0 {
            return 0;
        }
        ((width + self.column_width - 1) / self.column_width.max(1)) as usize
    }

    fn normalize_projection_state(&mut self) {
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
    }

    fn clear_cache(&mut self) {
        self.window_cache = None;
    }

    fn emit_visible_window_changed(&self) {
        self.visible_window_changed.emit(self.visible_window());
    }
}

impl Widget for DataGrid {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl Draw for DataGrid {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        context.fill_rect(rect, Color::rgb(255, 255, 255));
        context.draw_rect(rect, Color::rgb(200, 200, 200));

        let rows = self.fetch_visible_cells();
        if rows.is_empty() {
            return;
        }

        let row_h = self.row_height as i32;
        let col_w = self.column_width as i32;

        for (row_idx, row) in rows.iter().enumerate() {
            let y = rect.y + (row_idx as i32) * row_h;
            if y >= rect.y + rect.height as i32 {
                break;
            }

            let mut x = rect.x;
            for cell in row {
                if x >= rect.x + rect.width as i32 {
                    break;
                }
                context.draw_rect(
                    Rect::new(x, y, self.column_width, self.row_height),
                    Color::rgb(230, 230, 230),
                );
                if let Some(text) = cell {
                    context.draw_text(
                        Point::new(x + 4, y + row_h / 2),
                        text,
                        &Font::default(),
                        Color::rgb(0, 0, 0),
                        HorizontalAlignment::Left,
                    );
                }
                x += col_w;
            }
        }

        if self.frozen_columns > 0 {
            let split_x = rect.x + (self.frozen_columns as i32) * col_w;
            if split_x > rect.x {
                context.draw_line(
                    Point::new(split_x, rect.y),
                    Point::new(split_x, rect.y + rect.height as i32),
                    Color::rgb(0, 120, 215),
                );
            }
        }
    }
}

impl crate::event::EventHandler for DataGrid {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }

        if let Event::Wheel { delta, .. } = event {
            let lines = ((delta.y.abs() / 120).max(1)) as isize;
            if delta.y < 0 {
                let next = self.scroll_row.saturating_add(lines as usize);
                self.set_scroll_row(next);
            } else if delta.y > 0 {
                let up = self.scroll_row.saturating_sub(lines as usize);
                self.set_scroll_row(up);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signal::GenericSignal;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
    use std::sync::Mutex;

    struct StaticSource {
        rows: usize,
        cols: usize,
        data: Vec<Vec<String>>,
    }

    impl IncrementalTableDataSource for StaticSource {
        fn row_count(&self) -> usize {
            self.rows
        }

        fn column_count(&self) -> usize {
            self.cols
        }

        fn data(&self, row: usize, column: usize) -> Option<String> {
            self.data.get(row).and_then(|line| line.get(column)).cloned()
        }
    }

    #[test]
    fn visible_window_includes_overscan_and_clamps() {
        let mut grid = DataGrid::new(Rect::new(0, 0, 240, 60));
        grid.set_data_source(Arc::new(StaticSource {
            rows: 10,
            cols: 6,
            data: (0..10).map(|r| (0..6).map(|c| format!("{}:{}", r, c)).collect()).collect(),
        }));

        assert_eq!(grid.visible_window(), (0, 7, 0, 4));

        grid.set_scroll_row(8);
        grid.set_scroll_column(5);
        let window = grid.visible_window();
        assert_eq!(window.0, 6);
        assert_eq!(window.2, 4);
        assert!(window.1 <= 4);
        assert!(window.3 <= 2);
    }

    #[test]
    fn filter_and_sort_apply_to_window_rows() {
        let mut grid = DataGrid::new(Rect::new(0, 0, 240, 80));
        grid.set_data_source(Arc::new(StaticSource {
            rows: 4,
            cols: 2,
            data: vec![
                vec!["u1".to_string(), "alice".to_string()],
                vec!["u2".to_string(), "bob".to_string()],
                vec!["u3".to_string(), "bruce".to_string()],
                vec!["u4".to_string(), "zoe".to_string()],
            ],
        }));
        grid.set_filters(vec![ColumnFilter { column: 1, query: "b".to_string() }]);
        grid.set_sort_specs(vec![SortSpec { column: 1, descending: true }]);

        let rows = grid.fetch_visible_cells();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0][1], Some("bruce".to_string()));
        assert_eq!(rows[1][1], Some("bob".to_string()));
    }

    #[test]
    fn frozen_columns_clamp_to_column_count() {
        let mut grid = DataGrid::new(Rect::new(0, 0, 200, 60));
        grid.set_data_source(Arc::new(StaticSource {
            rows: 2,
            cols: 3,
            data: vec![
                vec!["a".to_string(), "b".to_string(), "c".to_string()],
                vec!["d".to_string(), "e".to_string(), "f".to_string()],
            ],
        }));

        grid.set_frozen_columns(99);
        assert_eq!(grid.frozen_columns(), 3);

        grid.clear_data_source();
        assert_eq!(grid.frozen_columns(), 0);
    }

    #[test]
    fn new_creates_default_state() {
        let mut grid = DataGrid::new(Rect::new(0, 0, 800, 600));
        assert!(!grid.has_data_source());
        assert_eq!(grid.scroll_row(), 0);
        assert_eq!(grid.scroll_column(), 0);
        assert_eq!(grid.row_height(), 20);
        assert_eq!(grid.column_width(), 120);
        assert_eq!(grid.frozen_columns(), 0);
        assert!(grid.sort_specs().is_empty());
        assert!(grid.filters().is_empty());
        assert_eq!(grid.row_count(), 0);
        assert_eq!(grid.column_count(), 0);
        assert!(grid.fetch_visible_cells().is_empty());
    }

    #[test]
    fn has_data_source_before_and_after() {
        let mut grid = DataGrid::new(Rect::new(0, 0, 800, 600));
        assert!(!grid.has_data_source());

        grid.set_data_source(Arc::new(StaticSource {
            rows: 3,
            cols: 3,
            data: vec![
                vec!["a".to_string(), "b".to_string(), "c".to_string()],
                vec!["d".to_string(), "e".to_string(), "f".to_string()],
                vec!["g".to_string(), "h".to_string(), "i".to_string()],
            ],
        }));
        assert!(grid.has_data_source());

        grid.clear_data_source();
        assert!(!grid.has_data_source());
    }

    #[test]
    fn row_and_column_count_queries() {
        let mut grid = DataGrid::new(Rect::new(0, 0, 800, 600));
        assert_eq!(grid.row_count(), 0);
        assert_eq!(grid.column_count(), 0);

        grid.set_data_source(Arc::new(StaticSource { rows: 5, cols: 4, data: vec![] }));
        assert_eq!(grid.row_count(), 5);
        assert_eq!(grid.column_count(), 4);

        grid.clear_data_source();
        assert_eq!(grid.row_count(), 0);
        assert_eq!(grid.column_count(), 0);
    }

    #[test]
    fn scroll_position_clamping() {
        let mut grid = DataGrid::new(Rect::new(0, 0, 800, 600));
        grid.set_data_source(Arc::new(StaticSource {
            rows: 10,
            cols: 6,
            data: (0..10).map(|r| (0..6).map(|c| format!("{}:{}", r, c)).collect()).collect(),
        }));

        grid.set_scroll_row(5);
        assert_eq!(grid.scroll_row(), 5);

        grid.set_scroll_row(999);
        assert_eq!(grid.scroll_row(), 9);

        grid.set_scroll_column(3);
        assert_eq!(grid.scroll_column(), 3);

        grid.set_scroll_column(999);
        assert_eq!(grid.scroll_column(), 5);
    }

    #[test]
    fn clear_data_source_resets_state() {
        let mut grid = DataGrid::new(Rect::new(0, 0, 800, 600));
        grid.set_data_source(Arc::new(StaticSource { rows: 5, cols: 3, data: vec![] }));
        grid.set_scroll_row(2);
        grid.set_scroll_column(1);
        grid.set_frozen_columns(2);
        grid.set_sort_specs(vec![SortSpec { column: 0, descending: false }]);

        grid.clear_data_source();
        assert!(!grid.has_data_source());
        assert_eq!(grid.scroll_row(), 0);
        assert_eq!(grid.scroll_column(), 0);
        assert_eq!(grid.frozen_columns(), 0);
        assert!(grid.fetch_visible_cells().is_empty());
    }

    #[test]
    fn sort_specs_set_get_invalidate_cache() {
        let mut grid = DataGrid::new(Rect::new(0, 0, 800, 600));
        grid.set_data_source(Arc::new(StaticSource {
            rows: 3,
            cols: 2,
            data: vec![
                vec!["c".to_string(), "x".to_string()],
                vec!["a".to_string(), "y".to_string()],
                vec!["b".to_string(), "z".to_string()],
            ],
        }));

        assert!(grid.sort_specs().is_empty());

        let specs = vec![SortSpec { column: 0, descending: false }];
        grid.set_sort_specs(specs.clone());
        assert_eq!(grid.sort_specs().len(), 1);
        assert_eq!(grid.sort_specs()[0].column, 0);

        // Fetch with sort applied
        let rows = grid.fetch_visible_cells();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0][0], Some("a".to_string()));
        assert_eq!(rows[1][0], Some("b".to_string()));
        assert_eq!(rows[2][0], Some("c".to_string()));

        // Change sort - should clear cache
        grid.set_sort_specs(vec![SortSpec { column: 0, descending: true }]);
        let rows2 = grid.fetch_visible_cells();
        assert_eq!(rows2[0][0], Some("c".to_string()));
        assert_eq!(rows2[2][0], Some("a".to_string()));

        grid.set_sort_specs(Vec::new());
        assert!(grid.sort_specs().is_empty());
    }

    #[test]
    fn filters_set_get_invalidate_cache() {
        let mut grid = DataGrid::new(Rect::new(0, 0, 800, 600));
        grid.set_data_source(Arc::new(StaticSource {
            rows: 4,
            cols: 2,
            data: vec![
                vec!["apple".to_string(), "red".to_string()],
                vec!["banana".to_string(), "yellow".to_string()],
                vec!["cherry".to_string(), "red".to_string()],
                vec!["date".to_string(), "brown".to_string()],
            ],
        }));

        assert!(grid.filters().is_empty());

        let filters = vec![ColumnFilter { column: 1, query: "red".to_string() }];
        grid.set_filters(filters.clone());
        assert_eq!(grid.filters().len(), 1);

        let rows = grid.fetch_visible_cells();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0][0], Some("apple".to_string()));
        assert_eq!(rows[1][0], Some("cherry".to_string()));

        // Change filter - should recalc
        grid.set_filters(vec![ColumnFilter { column: 0, query: "b".to_string() }]);
        let rows2 = grid.fetch_visible_cells();
        assert_eq!(rows2.len(), 1);
        assert_eq!(rows2[0][0], Some("banana".to_string()));

        grid.set_filters(Vec::new());
        assert!(grid.filters().is_empty());
    }

    #[test]
    fn frozen_columns_set_get() {
        let mut grid = DataGrid::new(Rect::new(0, 0, 800, 600));
        grid.set_data_source(Arc::new(StaticSource { rows: 2, cols: 5, data: vec![] }));

        assert_eq!(grid.frozen_columns(), 0);

        grid.set_frozen_columns(3);
        assert_eq!(grid.frozen_columns(), 3);

        // Clamped to column count
        grid.set_frozen_columns(999);
        assert_eq!(grid.frozen_columns(), 5);
    }

    #[test]
    fn visible_window_changed_signal_emission() {
        let mut grid = DataGrid::new(Rect::new(0, 0, 240, 60));
        grid.set_data_source(Arc::new(StaticSource {
            rows: 10,
            cols: 6,
            data: (0..10).map(|r| (0..6).map(|c| format!("{}:{}", r, c)).collect()).collect(),
        }));

        let emitted = Arc::new(Mutex::new(false));
        let sink = emitted.clone();
        grid.visible_window_changed.connect(move |_win| {
            *sink.lock().unwrap() = true;
        });

        grid.set_scroll_row(3);
        assert!(*emitted.lock().unwrap());
    }

    #[test]
    fn empty_source_returns_empty_window() {
        let mut grid = DataGrid::new(Rect::new(0, 0, 800, 600));
        grid.set_data_source(Arc::new(StaticSource { rows: 0, cols: 0, data: vec![] }));

        assert_eq!(grid.visible_window(), (0, 0, 0, 0));
        assert!(grid.fetch_visible_cells().is_empty());
    }

    struct RevisionSource {
        rows: usize,
        cols: usize,
        rev: AtomicU64,
        changed: GenericSignal,
        call_count: AtomicUsize,
    }

    impl RevisionSource {
        fn new(rows: usize, cols: usize) -> Self {
            Self {
                rows,
                cols,
                rev: AtomicU64::new(1),
                changed: GenericSignal::new(),
                call_count: AtomicUsize::new(0),
            }
        }

        fn bump(&self) {
            self.rev.fetch_add(1, Ordering::Relaxed);
            self.changed.emit();
        }
    }

    impl IncrementalTableDataSource for RevisionSource {
        fn row_count(&self) -> usize {
            self.rows
        }

        fn column_count(&self) -> usize {
            self.cols
        }

        fn data(&self, row: usize, column: usize) -> Option<String> {
            self.call_count.fetch_add(1, Ordering::Relaxed);
            Some(format!("{}:{}", row, column))
        }

        fn revision(&self) -> u64 {
            self.rev.load(Ordering::Relaxed)
        }

        fn data_changed_signal(&self) -> Option<&GenericSignal> {
            Some(&self.changed)
        }
    }

    #[test]
    fn cache_invalidation_on_revision_change() {
        let mut grid = DataGrid::new(Rect::new(0, 0, 240, 60));
        let source = Arc::new(RevisionSource::new(10, 4));
        grid.set_data_source(source.clone());

        let _a = grid.fetch_visible_cells();
        let calls_after_a = source.call_count.load(Ordering::Relaxed);
        assert!(calls_after_a > 0);

        // Second fetch should hit cache
        let _b = grid.fetch_visible_cells();
        assert_eq!(source.call_count.load(Ordering::Relaxed), calls_after_a);

        // Bump revision -> cache invalidated
        source.bump();
        let _c = grid.fetch_visible_cells();
        assert!(source.call_count.load(Ordering::Relaxed) > calls_after_a);
    }

    #[test]
    fn row_height_and_column_width_minimum_clamp() {
        let mut grid = DataGrid::new(Rect::new(0, 0, 800, 600));
        grid.set_data_source(Arc::new(StaticSource { rows: 3, cols: 3, data: vec![] }));

        grid.set_row_height(0);
        assert_eq!(grid.row_height(), 1);

        grid.set_row_height(25);
        assert_eq!(grid.row_height(), 25);

        grid.set_column_width(0);
        assert_eq!(grid.column_width(), 1);

        grid.set_column_width(100);
        assert_eq!(grid.column_width(), 100);
    }
}
