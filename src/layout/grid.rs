//! Grid layout manager — arranges items in a fixed row/column grid.
use super::Layout;
use crate::core::{ObjectId, Rect};
/// Fixed-grid layout manager with row/column cell placement.
pub struct GridLayout {
    rows: u32,
    cols: u32,
    spacing: u32,
    margin: u32,
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
}
impl Layout for GridLayout {
    fn as_any(&self) -> &dyn std::any::Any {
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
    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) {
        let cell_width = rect
            .width
            .saturating_sub(self.margin * 2)
            .saturating_sub((self.cols - 1) * self.spacing)
            / self.cols;
        let cell_height = rect
            .height
            .saturating_sub(self.margin * 2)
            .saturating_sub((self.rows - 1) * self.spacing)
            / self.rows;
        for row in 0..self.rows {
            for col in 0..self.cols {
                if let Some(widget_id) = self.cells[(row * self.cols + col) as usize] {
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
