//! Uniform grid layout manager — arranges items in a grid with equal cell sizes.
use super::Layout;
use crate::core::{ObjectId, Rect};

/// Fixed-grid layout manager where all cells are the same size.
///
/// Cells are distributed evenly across the available area, respecting
/// outer margin and inter-cell spacing. Each cell has identical
/// width and height, computed from the container rect.
pub struct UniformGridLayout {
    rows: u32,
    cols: u32,
    spacing: u32,
    margin: u32,
    cells: Vec<Option<ObjectId>>,
}

impl UniformGridLayout {
    /// Create a uniform grid layout with fixed rows/columns.
    ///
    /// * `rows`    – Number of rows (minimum 1).
    /// * `cols`    – Number of columns (minimum 1).
    /// * `spacing` – Gap between adjacent cells in pixels.
    /// * `margin`  – Outer padding around the entire grid in pixels.
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

    /// Assign a widget to a specific cell position.
    ///
    /// Does nothing if `(row, col)` is out of range.
    pub fn set_widget(&mut self, row: u32, col: u32, widget_id: ObjectId) {
        if row < self.rows && col < self.cols {
            self.cells[(row * self.cols + col) as usize] = Some(widget_id);
        }
    }

    /// Returns the number of occupied cells (widgets placed in the grid).
    pub fn cell_count(&self) -> usize {
        self.cells.iter().filter(|cell| cell.is_some()).count()
    }

    /// Returns the total number of cells in the grid (rows × cols).
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
}

impl Layout for UniformGridLayout {
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
        // Avoid division by zero when computing uniform cell size.
        if self.rows == 0 || self.cols == 0 {
            return;
        }

        // Total gap consumed by inter-cell spacing (cols - 1 horizontal,
        // rows - 1 vertical).
        let spacing_h = (self.cols - 1) * self.spacing;
        let spacing_v = (self.rows - 1) * self.spacing;

        // Available inner area after subtracting margins and spacing.
        let available_w = rect
            .width
            .saturating_sub(self.margin * 2)
            .saturating_sub(spacing_h);
        let available_h = rect
            .height
            .saturating_sub(self.margin * 2)
            .saturating_sub(spacing_v);

        // Uniform cell size — all cells share the available space equally.
        let cell_width = available_w / self.cols;
        let cell_height = available_h / self.rows;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_grid_has_correct_cell_count() {
        let grid = UniformGridLayout::new(3, 4, 2, 1);
        assert_eq!(grid.rows(), 3);
        assert_eq!(grid.cols(), 4);
        assert_eq!(grid.total_cells(), 12);
        assert_eq!(grid.cell_count(), 0);
    }

    #[test]
    fn uniform_grid_places_widgets_in_uniform_cells() {
        let mut grid = UniformGridLayout::new(2, 2, 0, 0);
        grid.set_widget(0, 0, 1);
        grid.set_widget(1, 1, 2);

        let mut rects = std::collections::HashMap::new();
        grid.update(Rect::new(0, 0, 40, 20), &mut |id, rect| {
            rects.insert(id, rect);
        });

        // 2x2 grid in 40x20 with no margin/spacing => each cell is 20x10.
        assert_eq!(rects.get(&1), Some(&Rect::new(0, 0, 20, 10)));
        assert_eq!(rects.get(&2), Some(&Rect::new(20, 10, 20, 10)));
    }

    #[test]
    fn uniform_grid_respects_margin_and_spacing() {
        let mut grid = UniformGridLayout::new(2, 2, 4, 2);
        grid.set_widget(0, 0, 1);
        grid.set_widget(0, 1, 2);

        let mut rects = std::collections::HashMap::new();
        // Available inner: 100 - 2*2(margin) - (2-1)*4(spacing) = 92.
        // Cell width: 92 / 2 = 46.
        // Available height: 60 - 2*2 - (2-1)*4 = 52.
        // Cell height: 52 / 2 = 26.
        grid.update(Rect::new(0, 0, 100, 60), &mut |id, rect| {
            rects.insert(id, rect);
        });

        // Cell (0,0): x=0+2, y=0+2, w=46, h=26
        assert_eq!(rects.get(&1), Some(&Rect::new(2, 2, 46, 26)));
        // Cell (0,1): x=0+2+1*(46+4)=52, y=0+2
        assert_eq!(rects.get(&2), Some(&Rect::new(52, 2, 46, 26)));
    }

    #[test]
    fn uniform_grid_add_widget_fills_empty_cell() {
        let mut grid = UniformGridLayout::new(2, 2, 0, 0);
        grid.add_widget(42, 1);
        assert!(grid.has_child(42));
        assert_eq!(grid.cell_count(), 1);
    }

    #[test]
    fn uniform_grid_remove_widget_empties_cell() {
        let mut grid = UniformGridLayout::new(2, 2, 0, 0);
        grid.set_widget(0, 0, 1);
        assert!(grid.has_child(1));
        grid.remove_widget(1);
        assert!(!grid.has_child(1));
        assert_eq!(grid.cell_count(), 0);
    }

    #[test]
    fn uniform_grid_clear_empties_all_cells() {
        let mut grid = UniformGridLayout::new(2, 2, 0, 0);
        grid.set_widget(0, 0, 1);
        grid.set_widget(1, 1, 2);
        assert_eq!(grid.cell_count(), 2);
        grid.clear();
        assert_eq!(grid.cell_count(), 0);
    }

    #[test]
    fn uniform_grid_child_ids_returns_only_occupied() {
        let mut grid = UniformGridLayout::new(3, 3, 0, 0);
        grid.set_widget(1, 1, 10);
        grid.set_widget(2, 0, 20);
        let ids = grid.child_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&10));
        assert!(ids.contains(&20));
    }

    #[test]
    fn uniform_grid_out_of_bounds_set_widget_is_noop() {
        let mut grid = UniformGridLayout::new(2, 2, 0, 0);
        grid.set_widget(5, 5, 99); // out of bounds
        assert_eq!(grid.cell_count(), 0);
    }

    #[test]
    fn uniform_grid_minimum_one_row_col() {
        let grid = UniformGridLayout::new(0, 0, 0, 0);
        assert_eq!(grid.rows(), 1);
        assert_eq!(grid.cols(), 1);
        assert_eq!(grid.total_cells(), 1);
    }

    #[test]
    fn uniform_grid_all_cells_same_size() {
        let mut grid = UniformGridLayout::new(3, 4, 2, 0);
        for r in 0..3 {
            for c in 0..4 {
                grid.set_widget(r, c, (r * 4 + c + 1) as ObjectId);
            }
        }

        let mut sizes = std::collections::HashSet::new();
        grid.update(Rect::new(0, 0, 100, 80), &mut |_id, rect| {
            sizes.insert((rect.width, rect.height));
        });

        // All cells must report the same (width, height) — uniform guarantee.
        assert_eq!(sizes.len(), 1, "all cells must be the same size");
    }
}
