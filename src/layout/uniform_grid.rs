// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Uniform grid layout manager — arranges items in a grid with equal cell sizes.
use super::grid::GridLayout;
use super::{ChildInfo, Layout, LayoutContext};
use crate::compat::{Any, Vec};
use crate::core::{ObjectId, Rect};

/// One widget's cell, recorded so the inner grid can be rebuilt when the spacing or margin
/// changes. `GridLayout` has no post-construction spacing setter, so the uniform wrapper keeps
/// the placement map and replays it into a fresh inner grid.
#[derive(Debug, Clone, Copy)]
struct UniformPlacement {
    widget_id: ObjectId,
    row: u32,
    col: u32,
    col_span: u32,
    row_span: u32,
}

/// Fixed-grid layout manager where all cells are the same size.
///
/// This is a thin wrapper around [`GridLayout`] with the stretch factors
/// fixed to `1` so that every cell receives an equal share of the
/// available space.
///
/// Cells are distributed evenly across the available area, respecting
/// outer margin and inter-cell spacing. Each cell has identical
/// width and height, computed from the container rect.
pub struct UniformGridLayout {
    inner: GridLayout,
    /// Placements in insertion order, replayed when the inner grid is rebuilt.
    placements: Vec<UniformPlacement>,
}

impl UniformGridLayout {
    /// Create a uniform grid layout with fixed rows/columns.
    ///
    /// * `rows`    – Number of rows (minimum 1).
    /// * `cols`    – Number of columns (minimum 1).
    /// * `spacing` – Gap between adjacent cells in pixels.
    /// * `margin`  – Outer padding around the entire grid in pixels.
    pub fn new(rows: u32, cols: u32, spacing: u32, margin: u32) -> Self {
        let mut inner = GridLayout::new(rows, cols, spacing, margin);
        // Uniform grid means every cell is the same size, so stretch is 1.
        inner.set_column_stretch(1);
        inner.set_row_stretch(1);
        Self { inner, placements: Vec::new() }
    }

    /// The wrapped grid, for a caller that needs a grid-specific capability.
    ///
    /// Changing a placement through this reference is not reflected in the placement map used
    /// by [`Self::set_spacing`] / [`Self::set_margin`]; use the wrapper's own placement methods
    /// when the spacing is also expected to change later.
    pub fn inner(&self) -> &GridLayout {
        &self.inner
    }

    /// Mutable access to the wrapped grid; see [`Self::inner`] for the placement caveat.
    pub fn inner_mut(&mut self) -> &mut GridLayout {
        &mut self.inner
    }

    /// Replaces the inner grid with one built from `spacing`/`margin`, replaying the recorded
    /// placements. The grid dimensions and the uniform stretch factors are preserved.
    fn rebuild(&mut self, spacing: u32, margin: u32) {
        let margins = self.inner.margins();
        let mut inner = GridLayout::new(self.inner.rows(), self.inner.cols(), spacing, margin);
        inner.set_column_stretch(1);
        inner.set_row_stretch(1);
        if let Some(margins) = margins {
            inner.set_margins(margins);
        }
        for placement in &self.placements {
            let _ = inner.place_within_extent(
                placement.row,
                placement.col,
                placement.col_span,
                placement.row_span,
                placement.widget_id,
            );
        }
        self.inner = inner;
    }

    /// Records `widget_id`'s footprint. A previous placement of the same widget is dropped, so
    /// the replay writes its cells exactly once, in the order the original calls made them.
    fn record_placement(
        &mut self,
        widget_id: ObjectId,
        row: u32,
        col: u32,
        col_span: u32,
        row_span: u32,
    ) {
        self.placements.retain(|existing| existing.widget_id != widget_id);
        self.placements.push(UniformPlacement { widget_id, row, col, col_span, row_span });
    }

    /// Assign a widget to a specific cell position.
    ///
    /// Does nothing if `(row, col)` is out of range.
    pub fn set_widget(&mut self, row: u32, col: u32, widget_id: ObjectId) {
        // `place_within_extent`, not `set_widget_spanning`: a uniform grid's dimensions are what
        // the caller asked for, so an out-of-range cell is refused rather than grown up to.
        // Growing here would silently rewrite the row and column counts the caller chose, which
        // is the one thing this type exists to keep fixed.
        if self.inner.place_within_extent(row, col, 1, 1, widget_id) {
            self.record_placement(widget_id, row, col, 1, 1);
        }
    }

    /// Assigns a widget to an explicit cell, returning whether the cell existed.
    ///
    /// # Why a span and a return value
    ///
    /// The declarative engine exposes one placement door for every cell-based layout. A
    /// uniform grid is cell-based, so it has to answer the same call as a plain grid — its
    /// dimensions stay fixed, but a cell that exists accepts the placement. Returning the
    /// answer lets the loader report a refused cell instead of claiming it placed a child.
    pub fn set_widget_spanning(
        &mut self,
        row: u32,
        col: u32,
        col_span: u32,
        row_span: u32,
        widget_id: ObjectId,
    ) -> bool {
        let placed = self.inner.place_within_extent(row, col, col_span, row_span, widget_id);
        if placed {
            // Mirror the clamp `place_within_extent` applies, so a replayed placement covers
            // exactly the cells the inner grid wrote.
            let col_span = col_span.max(1).min(self.inner.cols().saturating_sub(col));
            let row_span = row_span.max(1).min(self.inner.rows().saturating_sub(row));
            self.record_placement(widget_id, row, col, col_span, row_span);
        }
        placed
    }

    /// Returns the number of occupied cells (widgets placed in the grid).
    pub fn cell_count(&self) -> usize {
        self.inner.cell_count()
    }

    /// Returns the total number of cells in the grid (rows × cols).
    pub fn total_cells(&self) -> usize {
        self.inner.total_cells()
    }

    /// Returns the number of rows.
    pub fn rows(&self) -> u32 {
        self.inner.rows()
    }

    /// Returns the number of columns.
    pub fn cols(&self) -> u32 {
        self.inner.cols()
    }

    /// Returns the spacing between cells.
    pub fn spacing(&self) -> u32 {
        self.inner.spacing()
    }

    /// Returns the outer margin.
    pub fn margin(&self) -> u32 {
        self.inner.margin()
    }

    /// Sets the gap between adjacent cells.
    ///
    /// The inner grid is rebuilt with the new gap; every recorded placement is replayed, so the
    /// cell map is unchanged.
    pub fn set_spacing(&mut self, spacing: u32) {
        if self.inner.spacing() == spacing {
            return;
        }
        let margin = self.inner.margin();
        self.rebuild(spacing, margin);
    }

    /// Sets the outer margin around the entire grid.
    ///
    /// The inner grid is rebuilt with the new margin; every recorded placement is replayed.
    pub fn set_margin(&mut self, margin: u32) {
        if self.inner.margin() == margin {
            return;
        }
        let spacing = self.inner.spacing();
        self.rebuild(spacing, margin);
    }
}

impl Layout for UniformGridLayout {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn add_widget(&mut self, widget_id: ObjectId, _stretch: u32) {
        // A uniform grid's dimensions are fixed, so auto-placement must use the
        // non-growing path. Delegating to `GridLayout::add_widget` would grow the grid
        // once it is full, silently undoing the row/column counts the caller chose (S-43).
        if self.inner.place_next_free_cell(widget_id) {
            // `cell_of` now reads the grid's reverse index, so recording the placement does
            // not re-scan the cell array (G-151).
            if let Some((row, col)) = self.inner.cell_of(widget_id) {
                self.record_placement(widget_id, row, col, 1, 1);
            }
        }
    }

    fn remove_widget(&mut self, widget_id: ObjectId) {
        self.inner.remove_widget(widget_id);
        self.placements.retain(|placement| placement.widget_id != widget_id);
    }

    fn child_ids(&self) -> Vec<ObjectId> {
        self.inner.child_ids()
    }

    fn has_child(&self, id: ObjectId) -> bool {
        self.inner.has_child(id)
    }

    fn clear(&mut self) {
        self.inner.clear();
        self.placements.clear();
    }

    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) {
        self.inner.update(rect, widgets);
    }

    // G-403: the trait no longer defaults these entries; each forward reproduces the previous fallback behaviour.
    fn update_with_context(
        &self,
        rect: Rect,
        context: &LayoutContext,
        widgets: &mut dyn FnMut(ObjectId, Rect),
    ) {
        let _ = context;
        self.update(rect, widgets);
    }

    fn arrange(&self, rect: Rect, children: &[ChildInfo], widgets: &mut dyn FnMut(ObjectId, Rect)) {
        let _ = children;
        self.update(rect, widgets);
    }

    fn arrange_with_context(
        &self,
        rect: Rect,
        children: &[ChildInfo],
        context: &LayoutContext,
        widgets: &mut dyn FnMut(ObjectId, Rect),
    ) {
        let _ = context;
        self.arrange(rect, children, widgets);
    }

    fn widget_count(&self) -> usize {
        self.inner.widget_count()
    }

    fn index_of(&self, id: ObjectId) -> Option<usize> {
        self.inner.index_of(id)
    }

    fn child_rect(&self, id: ObjectId, rect: Rect) -> Option<Rect> {
        self.inner.child_rect(id, rect)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::HashMap;

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

        let mut rects = HashMap::new();
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

        let mut rects = HashMap::new();
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
    fn uniform_grid_explicit_placement_is_supported_and_stays_inside_the_extent() {
        let mut grid = UniformGridLayout::new(2, 2, 0, 0);
        // The declarative engine's one placement door works for a uniform grid too; a cell
        // that exists is accepted, a cell outside the fixed extent is refused.
        assert!(grid.set_widget_spanning(1, 1, 1, 1, 7));
        assert_eq!(grid.inner.cell_of(7), Some((1, 1)));
        assert!(!grid.set_widget_spanning(9, 9, 1, 1, 8), "a fixed grid does not grow");
        assert_eq!(grid.rows(), 2);
        assert_eq!(grid.cols(), 2);
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

        let mut sizes = alloc::collections::BTreeSet::new();
        grid.update(Rect::new(0, 0, 100, 80), &mut |_id, rect| {
            sizes.insert((rect.width, rect.height));
        });

        // All cells must report the same (width, height) — uniform guarantee.
        assert_eq!(sizes.len(), 1, "all cells must be the same size");
    }

    /// S-43: a full fixed-dimension grid must refuse `add_widget` instead of growing.
    #[test]
    fn uniform_grid_add_widget_does_not_grow_a_full_grid() {
        let mut grid = UniformGridLayout::new(1, 1, 0, 0);
        grid.add_widget(1, 1);
        assert_eq!(grid.rows(), 1);
        assert_eq!(grid.cols(), 1);
        assert_eq!(grid.total_cells(), 1);
        assert_eq!(grid.cell_count(), 1);

        // The grid is full: the second widget is refused without changing the dimensions.
        grid.add_widget(2, 1);
        assert_eq!(grid.rows(), 1, "a uniform grid never grows past its declared rows");
        assert_eq!(grid.cols(), 1, "a uniform grid never grows past its declared cols");
        assert_eq!(grid.total_cells(), 1);
        assert_eq!(grid.cell_count(), 1, "the second widget was rejected");
        assert!(grid.has_child(1));
        assert!(!grid.has_child(2));
    }
}
