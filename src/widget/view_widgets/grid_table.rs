//! GridTable widget — feature-rich virtualized table with grid lines, headers,
//! sorting, selection, and column resize.
//!
//! Uses the `IncrementalTableDataSource` protocol from `data_source` for data access.

#[cfg(not(feature = "mini"))]
use std::sync::Arc;

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

use super::data_source::IncrementalTableDataSource;

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

// ---------------------------------------------------------------------------
// GridTableWidget
// ---------------------------------------------------------------------------

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
    grid_color: Color,
    _grid_thickness: u32,
    header_bg: Color,
    header_text_color: Color,
    selected_bg: Color,

    // Selection
    selected_cell: Option<(usize, usize)>,
    selection_mode: GridTableSelectionMode,

    // Per-column state
    column_widths: Vec<u32>,

    // Sort
    sort_specs: Vec<GridTableSortSpec>,

    // Column resize tracking
    resizing_column: Option<usize>,
    resize_start_x: i32,
    resize_start_width: u32,

    // Cached visibility
    visible_rows: usize,
    visible_columns: usize,

    // ── Signals ──
    /// Emitted when a cell is selected: `(row, column)`.
    pub cell_selected: Signal1<(usize, usize)>,
    /// Emitted when a cell is double-clicked: `(row, column)`.
    pub cell_double_clicked: Signal1<(usize, usize)>,
    /// Emitted when sort changes for a column: `(column, descending)`.
    pub sort_changed: Signal1<(usize, bool)>,
    /// Emitted when a header is clicked (before sort toggle): `column`.
    pub header_clicked: Signal1<usize>,
}

impl GridTableWidget {
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
            grid_color: Color::rgb(210, 215, 225),
            _grid_thickness: 1,
            header_bg: Color::rgb(240, 242, 245),
            header_text_color: Color::rgb(40, 50, 70),
            selected_bg: Color::rgb(200, 220, 250),
            selected_cell: None,
            selection_mode: GridTableSelectionMode::Cell,
            column_widths: Vec::new(),
            sort_specs: Vec::new(),
            resizing_column: None,
            resize_start_x: 0,
            resize_start_width: 0,
            visible_rows: 0,
            visible_columns: 0,
            cell_selected: Signal1::new(),
            cell_double_clicked: Signal1::new(),
            sort_changed: Signal1::new(),
            header_clicked: Signal1::new(),
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
        self.sort_specs.clear();
        self.column_widths.clear();
        self.update_visibility();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Clears the data source and resets all local state.
    pub fn clear_data_source(&mut self) {
        self.data_source = None;
        self.scroll_row = 0;
        self.scroll_column = 0;
        self.selected_cell = None;
        self.sort_specs.clear();
        self.column_widths.clear();
        self.visible_rows = 0;
        self.visible_columns = 0;
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
        self.selection_mode = mode;
        if mode == GridTableSelectionMode::None {
            self.selected_cell = None;
            self.base.request_redraw();
        }
    }

    // -----------------------------------------------------------------------
    // Column widths
    // -----------------------------------------------------------------------

    /// Ensures the internal column-widths vector has at least `count` entries,
    /// each defaulting to a reasonable initial width.
    fn ensure_column_widths(&mut self, count: usize) {
        if self.column_widths.len() < count {
            let default_w = 120u32.max(self.min_column_width);
            self.column_widths.resize(count, default_w);
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
        self.update_visibility();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns the width of a specific column, or a default if unset.
    pub fn column_width(&self, col: usize) -> u32 {
        self.column_widths.get(col).copied().unwrap_or(120u32.max(self.min_column_width))
    }

    // -----------------------------------------------------------------------
    // Sorting
    // -----------------------------------------------------------------------

    /// Toggles sort for the given column. If the column is already the primary
    /// sort, the direction is flipped. Otherwise it becomes the sole sort spec
    /// with ascending order.
    pub fn toggle_sort_column(&mut self, col: usize) {
        let cols = self.column_count();
        if col >= cols {
            return;
        }

        if let Some(existing) = self.sort_specs.iter_mut().find(|s| s.column == col) {
            existing.descending = !existing.descending;
            let desc = existing.descending;
            self.sort_changed.emit((col, desc));
        } else {
            self.sort_specs.clear();
            self.sort_specs.push(GridTableSortSpec { column: col, descending: false });
            self.sort_changed.emit((col, false));
        }
        self.base.request_redraw();
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
        if self.selected_cell.is_some() {
            self.selected_cell = None;
            self.base.request_redraw();
        }
    }

    // -----------------------------------------------------------------------
    // Visibility helpers
    // -----------------------------------------------------------------------

    /// Recalculates how many rows and columns fit in the current viewport.
    fn update_visibility(&mut self) {
        let rect = self.base.geometry();
        let rh = self.row_height.max(1) as i32;
        let header_h = self.header_height as i32;
        let rnw = self.row_number_width as i32;

        let data_h = (rect.height as i32).saturating_sub(header_h).max(0);
        let data_w = (rect.width as i32).saturating_sub(rnw).max(0);

        self.visible_rows = if rh > 0 { (data_h / rh) as usize } else { 0 };
        self.visible_columns = 0;

        // Walk column widths to count how many fit horizontally
        let mut acc = 0i32;
        let cols = self.column_count();
        self.ensure_column_widths(cols);
        for &w in &self.column_widths {
            let iw = w as i32;
            if acc + iw > data_w {
                break;
            }
            acc += iw;
            self.visible_columns += 1;
        }

        self.visible_rows = self.visible_rows.max(1);
        self.visible_columns = self.visible_columns.max(1);
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
        if x_offset < 0 {
            return None;
        }
        let cols = self.column_count();
        let default_w = 120u32.max(self.min_column_width) as i32;
        let mut acc = 0i32;
        for i in self.scroll_column..cols {
            let iw = self
                .column_widths
                .get(i)
                .copied()
                .unwrap_or(default_w as u32)
                .max(self.min_column_width) as i32;
            if x_offset >= acc && x_offset < acc + iw {
                return Some(i - self.scroll_column);
            }
            acc += iw;
        }
        None
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
    fn resize_handle_at_point(&self, point: Point) -> Option<usize> {
        let rect = self.base.geometry();
        let rnw = self.row_number_width as i32;
        let handle_width = 5i32;

        if point.y < rect.y || point.y >= rect.y + (self.header_height as i32) {
            return None;
        }

        let cols = self.column_count();
        let default_w = 120u32.max(self.min_column_width) as i32;
        let mut acc = rect.x + rnw;
        for i in self.scroll_column..cols {
            let iw = self
                .column_widths
                .get(i)
                .copied()
                .unwrap_or(default_w as u32)
                .max(self.min_column_width) as i32;
            let right_edge = acc + iw;
            // The handle area is the last `handle_width` pixels before the right edge
            if (point.x - right_edge).abs() <= handle_width {
                return Some(i);
            }
            acc += iw;
            if acc > rect.x + rect.width as i32 {
                break;
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

        // Background
        context.fill_rect(rect, Color::rgb(255, 255, 255));

        let rnw = self.row_number_width as i32;
        let header_h = self.header_height as i32;
        let rh = self.row_height.max(1) as i32;
        let cols = self.column_count();
        let rows = self.row_count();

        self.ensure_column_widths(cols);
        self.update_visibility();

        if rows == 0 || cols == 0 {
            // Draw at least the header / row-number area frame
            context.draw_rect(rect, Color::rgb(190, 198, 210));
            return;
        }

        let data_top = rect.y + header_h;
        let data_left = rect.x + rnw;

        // ── Draw column headers ──
        {
            let header_rect = Rect::new(rect.x, rect.y, rect.width, self.header_height);
            context.fill_rect(header_rect, self.header_bg);
            context.draw_rect(header_rect, self.grid_color);

            let mut hx = data_left;
            for ci in self.scroll_column..cols {
                let cw = self.column_width(ci) as i32;
                if hx >= rect.x + rect.width as i32 {
                    break;
                }

                let cell_rect = Rect::new(hx, rect.y, cw as u32, self.header_height);
                context.draw_rect(cell_rect, self.grid_color);

                // Sort indicator
                let sort_desc =
                    self.sort_specs.iter().find(|s| s.column == ci).map(|s| s.descending);
                let label = if let Some(desc) = sort_desc {
                    if desc {
                        " ▼"
                    } else {
                        " ▲"
                    }
                } else {
                    ""
                };

                let header_text = format!("Col {}{}", ci, label);
                context.draw_text(
                    Point::new(hx + 4, rect.y + header_h / 2),
                    &header_text,
                    &Font::default(),
                    self.header_text_color,
                    HorizontalAlignment::Left,
                );

                hx += cw;
            }
        }

        // ── Draw row numbers ──
        {
            for ri in 0..self.visible_rows.min(rows.saturating_sub(self.scroll_row)) {
                let abs_row = self.scroll_row + ri;
                let y = data_top + (ri as i32) * rh;
                if y + rh > rect.y + rect.height as i32 {
                    break;
                }

                let cell_rect = Rect::new(rect.x, y, self.row_number_width, self.row_height);
                context.fill_rect(cell_rect, self.header_bg);
                context.draw_rect(cell_rect, self.grid_color);

                context.draw_text(
                    Point::new(rect.x + rnw - 6, y + rh / 2),
                    &abs_row.to_string(),
                    &Font::default(),
                    self.header_text_color,
                    HorizontalAlignment::Right,
                );
            }
        }

        // ── Draw data cells ──
        let data_source = self.data_source.as_ref();
        if data_source.is_none() {
            return;
        }
        let source = data_source.unwrap();

        let mut cy = data_top;
        for ri in 0..self.visible_rows.min(rows.saturating_sub(self.scroll_row)) {
            let abs_row = self.scroll_row + ri;
            if cy + rh > rect.y + rect.height as i32 {
                break;
            }

            let mut cx = data_left;
            for ci in self.scroll_column..cols.min(self.scroll_column + self.visible_columns) {
                let cw = self.column_width(ci) as i32;
                if cx + cw > rect.x + rect.width as i32 {
                    break;
                }

                let cell_rect = Rect::new(cx, cy, cw as u32, self.row_height);

                // Selection highlight
                let is_selected = match self.selection_mode {
                    GridTableSelectionMode::None => false,
                    GridTableSelectionMode::Cell => self.selected_cell == Some((abs_row, ci)),
                    GridTableSelectionMode::Row => {
                        self.selected_cell.map(|(r, _)| r == abs_row).unwrap_or(false)
                    }
                    GridTableSelectionMode::Column => {
                        self.selected_cell.map(|(_, c)| c == ci).unwrap_or(false)
                    }
                };

                if is_selected {
                    context.fill_rect(cell_rect, self.selected_bg);
                }

                // Grid lines
                context.draw_rect(cell_rect, self.grid_color);

                // Cell text
                if let Some(text) = source.data(abs_row, ci) {
                    // Offset text slightly inside the cell
                    let text_x = cx + 3;
                    let text_y = cy + rh / 2;
                    context.draw_text(
                        Point::new(text_x, text_y),
                        &text,
                        &Font::default(),
                        Color::rgb(30, 40, 55),
                        HorizontalAlignment::Left,
                    );
                }

                cx += cw;
            }
            cy += rh;
        }

        // ── Outer border ──
        context.draw_rect(rect, Color::rgb(170, 180, 195));
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

        match event {
            Event::MousePress { pos, button: _ } => {
                // Check resize handles first
                if let Some(col) = self.resize_handle_at_point(*pos) {
                    self.resizing_column = Some(col);
                    self.resize_start_x = pos.x;
                    self.resize_start_width = self.column_width(col);
                    return;
                }

                // Check header click for sorting
                if let Some(col) = self.header_at_point(*pos) {
                    self.header_clicked.emit(col);
                    self.toggle_sort_column(col);
                    return;
                }

                // Check cell click for selection
                if let Some((row, col)) = self.cell_at_point(*pos) {
                    // Apply selection based on mode
                    let new_sel = match self.selection_mode {
                        GridTableSelectionMode::None => None,
                        GridTableSelectionMode::Cell => Some((row, col)),
                        GridTableSelectionMode::Row => Some((row, col)),
                        GridTableSelectionMode::Column => Some((row, col)),
                    };

                    if self.selected_cell != new_sel {
                        self.selected_cell = new_sel;
                        if let Some((r, c)) = new_sel {
                            self.cell_selected.emit((r, c));
                        }
                        self.base.request_redraw();
                    }
                } else {
                    // Click outside → clear selection
                    if self.selected_cell.is_some() {
                        self.selected_cell = None;
                        self.base.request_redraw();
                    }
                }
            }

            Event::MouseDoubleClick { pos, button: _ } => {
                if let Some((row, col)) = self.cell_at_point(*pos) {
                    self.cell_double_clicked.emit((row, col));
                }
            }

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
                        self.update_visibility();
                        self.base.request_redraw();
                    }
                }
            }

            Event::MouseRelease { pos: _, button: _ } => {
                if self.resizing_column.is_some() {
                    self.resizing_column = None;
                }
            }

            Event::Wheel { delta, modifiers: _ } => {
                let lines = ((delta.y.abs() / 120).max(1)) as isize;
                if delta.y < 0 {
                    let next = self.scroll_row.saturating_add(lines as usize);
                    self.set_scroll_row(next);
                } else if delta.y > 0 {
                    let up = self.scroll_row.saturating_sub(lines as usize);
                    self.set_scroll_row(up);
                }
            }

            Event::KeyPress { key, modifiers: _ } => match *key {
                // Arrow keys for scrolling
                37 => self.set_scroll_column(self.scroll_column.saturating_sub(1)),
                39 => self.set_scroll_column(self.scroll_column.saturating_add(1)),
                38 => self.set_scroll_row(self.scroll_row.saturating_sub(1)),
                40 => self.set_scroll_row(self.scroll_row.saturating_add(1)),
                _ => {}
            },

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

    // -----------------------------------------------------------------------
    // Visibility calculation
    // -----------------------------------------------------------------------

    #[test]
    fn update_visibility_computes_visible_cells() {
        let mut tbl = GridTableWidget::new(Rect::new(0, 0, 600, 400));
        tbl.set_data_source(Arc::new(TestSource { rows: 50, cols: 20 }));
        tbl.ensure_column_widths(20);
        // All cols default to 120, so with 600px width and 48px row-number gutter,
        // we should fit 4 columns (552/120 ≈ 4)
        for w in &mut tbl.column_widths {
            *w = 120;
        }
        tbl.update_visibility();

        // 400px height - 28px header = 372px data area, / 24px row height ≈ 15 rows
        assert_eq!(tbl.visible_rows, 15);
        // 600px - 48px = 552px data width, / 120px per col = 4 full cols
        assert_eq!(tbl.visible_columns, 4);
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
}
