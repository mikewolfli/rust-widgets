//! Grid layout manager — arranges items in a fixed row/column grid.
use super::Layout;
use crate::core::{ObjectId, Rect};
/// Fixed-grid layout manager with row/column cell placement.
pub struct GridLayout {
    rows: u32,
    cols: u32,
    spacing: u32,
    margin: u32,
    column_stretch: u32,
    row_stretch: u32,
    cells: Vec<Option<ObjectId>>,
}
impl GridLayout {
    /// Create a grid layout with fixed rows/columns.
    pub fn new(rows: u32, cols: u32, spacing: u32, margin: u32) -> Self {
        let safe_rows = rows.max(1);
        let safe_cols = cols.max(1);
        Self {
            rows: safe_rows,
            cols: safe_cols,
            spacing,
            margin,
            column_stretch: 1,
            row_stretch: 1,
            cells: vec![None; (safe_rows * safe_cols) as usize],
        }
    }
    /// Assign widget to explicit cell.
    pub fn set_widget(&mut self, row: u32, col: u32, widget_id: ObjectId) {
        if row < self.rows && col < self.cols {
            self.cells[(row * self.cols + col) as usize] = Some(widget_id);
        }
    }
    /// Returns the number of occupied cells (widgets placed in grid).
    pub fn cell_count(&self) -> usize {
        self.cells.iter().filter(|cell| cell.is_some()).count()
    }
    /// Returns the total number of cells in the grid.
    pub fn total_cells(&self) -> usize {
        self.cells.len()
    }
    /// Returns the number of rows.
    pub fn rows(&self) -> u32 {
        self.rows
    }
    /// Returns the number of columns.
    pub fn cols(&self) -> u32 {
        self.cols
    }
    /// Returns the spacing between cells.
    pub fn spacing(&self) -> u32 {
        self.spacing
    }
    /// Returns the outer margin.
    pub fn margin(&self) -> u32 {
        self.margin
    }

    /// Returns the column stretch factor.
    pub fn column_stretch(&self) -> u32 {
        self.column_stretch
    }

    /// Sets the column stretch factor.
    pub fn set_column_stretch(&mut self, stretch: u32) {
        self.column_stretch = stretch.max(1);
    }

    /// Returns the row stretch factor.
    pub fn row_stretch(&self) -> u32 {
        self.row_stretch
    }

    /// Sets the row stretch factor.
    pub fn set_row_stretch(&mut self, stretch: u32) {
        self.row_stretch = stretch.max(1);
    }
}
impl Layout for GridLayout {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn add_widget(&mut self, widget_id: ObjectId, _stretch: u32) {
        if let Some(slot) = self.cells.iter_mut().find(|cell| cell.is_none()) {
            *slot = Some(widget_id);
        }
    }
    fn remove_widget(&mut self, widget_id: ObjectId) {
        for cell in &mut self.cells {
            if *cell == Some(widget_id) {
                *cell = None;
            }
        }
    }
    fn child_ids(&self) -> Vec<ObjectId> {
        self.cells.iter().filter_map(|cell| *cell).collect()
    }
    fn has_child(&self, id: ObjectId) -> bool {
        self.cells.contains(&Some(id))
    }
    fn clear(&mut self) {
        for cell in &mut self.cells {
            *cell = None;
        }
    }
    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) {
        let available_width = rect
            .width
            .saturating_sub(self.margin * 2)
            .saturating_sub((self.cols - 1) * self.spacing);
        let available_height = rect
            .height
            .saturating_sub(self.margin * 2)
            .saturating_sub((self.rows - 1) * self.spacing);

        // Calculate column widths proportionally by stretch factor
        let total_col_stretch = self.cols * self.column_stretch;
        let total_row_stretch = self.rows * self.row_stretch;

        for row in 0..self.rows {
            for col in 0..self.cols {
                if let Some(widget_id) = self.cells[(row * self.cols + col) as usize] {
                    // Proportional width/height based on stretch
                    let cell_width = if total_col_stretch > 0 {
                        (available_width as u64 * self.column_stretch as u64
                            / total_col_stretch as u64) as u32
                    } else {
                        available_width / self.cols
                    };
                    let cell_height = if total_row_stretch > 0 {
                        (available_height as u64 * self.row_stretch as u64
                            / total_row_stretch as u64) as u32
                    } else {
                        available_height / self.rows
                    };

                    let x =
                        rect.x + self.margin as i32 + (col * (cell_width + self.spacing)) as i32;
                    let y =
                        rect.y + self.margin as i32 + (row * (cell_height + self.spacing)) as i32;
                    widgets(widget_id, Rect::new(x, y, cell_width, cell_height));
                }
            }
        }
    }
}
