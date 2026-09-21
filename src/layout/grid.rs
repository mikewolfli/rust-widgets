// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Grid layout manager — arranges items in a fixed row/column grid.
use super::Layout;
use crate::compat::{vec, Any, Vec};
use crate::core::{ObjectId, Rect};
/// How a grid decides each row's height.
///
/// The distinction exists because the two are both correct for different callers,
/// and the grid previously offered only the first:
///
/// * [`Self::Fill`] divides the whole available height across the rows by stretch
///   factor. Right for a grid whose cells should occupy the container eventually.
/// * [`Self::Fixed`] gives every row the same explicit height, and leaves the rest of
///   the container unused. Right for a grid of **controls**, which have a natural size:
///   a button stretched to a third of a page is not a taller button, it is a layout
///   mistake that reads as a blank panel.
///
/// The uniform-stretch default is [`Self::Fill`], so no existing caller changes
/// behaviour by this type's introduction.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RowSizing {
    /// Rows share the container height in proportion to their stretch factors.
    Fill,
    /// Every row is exactly this many pixels tall.
    Fixed(u32),
}

/// Fixed-grid layout manager with row/column cell placement.
pub struct GridLayout {
    rows: u32,
    cols: u32,
    spacing: u32,
    margin: u32,
    column_stretches: Vec<u32>,
    row_stretches: Vec<u32>,
    row_sizing: RowSizing,
    cells: Vec<Option<ObjectId>>,
}
impl GridLayout {
    /// Create a grid layout with fixed rows/columns.
    pub fn new(rows: u32, cols: u32, spacing: u32, margin: u32) -> Self {
        let safe_rows = rows.max(1);
        let safe_cols = cols.max(1);
        // `rows`/`cols` may come from untrusted input (JSON), and their product can
        // overflow `u32`. Cap the product so the cell Vec is always sized correctly
        // and later `row * self.cols + col` indexing stays in bounds.
        let cell_count = safe_rows.saturating_mul(safe_cols).min(1_000_000) as usize;
        Self {
            rows: safe_rows,
            cols: safe_cols,
            spacing,
            margin,
            column_stretches: vec![1; safe_cols as usize],
            row_stretches: vec![1; safe_rows as usize],
            row_sizing: RowSizing::Fill,
            cells: vec![None; cell_count],
        }
    }

    /// Sets how rows are sized; see [`RowSizing`].
    ///
    /// Returns the grid so a caller can chain it onto [`Self::new`], matching the
    /// builder style of the rest of this module's callers.
    pub fn with_row_sizing(mut self, sizing: RowSizing) -> Self {
        self.row_sizing = sizing;
        self
    }

    /// Sets how rows are sized, in place.
    pub fn set_row_sizing(&mut self, sizing: RowSizing) {
        self.row_sizing = sizing;
    }

    /// Returns the current row-sizing rule.
    pub fn row_sizing(&self) -> RowSizing {
        self.row_sizing
    }

    /// Assign widget to explicit cell.
    pub fn set_widget(&mut self, row: u32, col: u32, widget_id: ObjectId) {
        if row < self.rows && col < self.cols {
            let index = row.saturating_mul(self.cols).saturating_add(col) as usize;
            if index < self.cells.len() {
                self.cells[index] = Some(widget_id);
            }
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

    /// Returns the uniform column stretch factor (first column's value).
    pub fn column_stretch(&self) -> u32 {
        self.column_stretches.first().copied().unwrap_or(1)
    }

    /// Sets the uniform column stretch factor (applied to all columns).
    pub fn set_column_stretch(&mut self, stretch: u32) {
        let stretch = stretch.max(1);
        self.column_stretches.fill(stretch);
    }

    /// Returns the stretch factor for a specific column.
    pub fn column_stretch_for_col(&self, col: u32) -> u32 {
        self.column_stretches.get(col as usize).copied().unwrap_or(1)
    }

    /// Sets the stretch factor for a specific column.
    pub fn set_column_stretch_for_col(&mut self, col: u32, stretch: u32) {
        if col < self.cols {
            self.column_stretches[col as usize] = stretch.max(1);
        }
    }

    /// Returns a slice of all column stretch factors.
    pub fn column_stretches(&self) -> &[u32] {
        &self.column_stretches
    }

    /// Returns the uniform row stretch factor (first row's value).
    pub fn row_stretch(&self) -> u32 {
        self.row_stretches.first().copied().unwrap_or(1)
    }

    /// Sets the uniform row stretch factor (applied to all rows).
    pub fn set_row_stretch(&mut self, stretch: u32) {
        let stretch = stretch.max(1);
        self.row_stretches.fill(stretch);
    }

    /// Returns the stretch factor for a specific row.
    pub fn row_stretch_for_row(&self, row: u32) -> u32 {
        self.row_stretches.get(row as usize).copied().unwrap_or(1)
    }

    /// Sets the stretch factor for a specific row.
    pub fn set_row_stretch_for_row(&mut self, row: u32, stretch: u32) {
        if row < self.rows {
            self.row_stretches[row as usize] = stretch.max(1);
        }
    }

    /// Returns a slice of all row stretch factors.
    pub fn row_stretches(&self) -> &[u32] {
        &self.row_stretches
    }
}
impl Layout for GridLayout {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
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
        self.cells.fill(None);
    }
    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) {
        // The margins and the inter-cell spacing are honoured only as far as the rect can
        // pay for them. When it cannot, the spacing is reduced *before* the available
        // extent is derived, so the cursor walk and the extent can never disagree: the
        // previous version saturated `available_width` to zero while still stepping by the
        // full spacing, which placed later columns outside a parent that had allocated
        // nothing for them. A control resized small is the case this exists for.
        let margin = self.margin.min((rect.width / 2).min(rect.height / 2));
        let inner_width = rect.width.saturating_sub(margin * 2);
        let inner_height = rect.height.saturating_sub(margin * 2);
        let spacing_x =
            if self.cols > 1 { self.spacing.min(inner_width / (self.cols - 1)) } else { 0 };
        let spacing_y =
            if self.rows > 1 { self.spacing.min(inner_height / (self.rows - 1)) } else { 0 };

        let available_width = inner_width.saturating_sub(spacing_x * (self.cols - 1));
        let available_height = inner_height.saturating_sub(spacing_y * (self.rows - 1));

        // Calculate column widths and x-offsets based on per-column stretch factors
        let total_col_stretch: u32 = self.column_stretches.iter().sum();
        // Calculate row heights and y-offsets based on per-row stretch factors
        let total_row_stretch: u32 = self.row_stretches.iter().sum();

        // Precompute cumulative column width sums for x-offsets (fraction-aware)
        let mut col_widths: Vec<u32> = Vec::with_capacity(self.cols as usize);
        let mut col_x_offsets: Vec<i32> = Vec::with_capacity(self.cols as usize);
        let mut current_x: i32 = 0;
        for col in 0..self.cols {
            let cell_width = if total_col_stretch > 0 {
                (available_width as u64 * self.column_stretches[col as usize] as u64
                    / total_col_stretch as u64) as u32
            } else {
                available_width / self.cols
            };
            col_widths.push(cell_width);
            col_x_offsets.push(current_x);
            current_x += cell_width as i32 + spacing_x as i32;
        }

        // Precompute cumulative row height sums for y-offsets (fraction-aware)
        let mut row_heights: Vec<u32> = Vec::with_capacity(self.rows as usize);
        let mut row_y_offsets: Vec<i32> = Vec::with_capacity(self.rows as usize);
        let mut current_y: i32 = 0;
        for row in 0..self.rows {
            // A fixed row height is taken as given: the caller has said what a row is,
            // so the grid does not divide the container among the rows. It is clamped to
            // the height **still available**, not to the container's total: clamping each
            // row independently let every row claim the full height, so the later rows
            // were placed past the bottom edge (a 60px container with two 500px rows
            // put the second row at y=60 with a height of 60).
            let cell_height = match self.row_sizing {
                RowSizing::Fixed(height) => {
                    // Clamp to the height **still available**, not to the container's total:
                    // clamping each row independently let every row claim the full height,
                    // so the later rows were placed past the bottom edge (a 60px container
                    // with two 500px rows put the second row at y=60 with a height of 60).
                    let remaining =
                        (available_height as i32 - current_y).max(0).min(height as i32) as u32;
                    remaining
                }
                RowSizing::Fill => {
                    if total_row_stretch > 0 {
                        (available_height as u64 * self.row_stretches[row as usize] as u64
                            / total_row_stretch as u64) as u32
                    } else {
                        available_height / self.rows
                    }
                }
            };
            row_heights.push(cell_height);
            row_y_offsets.push(current_y);
            current_y += cell_height as i32 + spacing_y as i32;
        }

        // Distribute the remainder width/height across columns/rows (biggest-bucket algorithm)
        let total_width: u32 = col_widths.iter().sum::<u32>() + spacing_x * (self.cols - 1);
        let remainder_w = available_width.saturating_sub(total_width);
        if remainder_w > 0 && !col_widths.is_empty() {
            // Distribute remainder to columns with largest stretch first
            let mut indices: Vec<usize> = (0..self.cols as usize).collect();
            indices.sort_by(|&a, &b| self.column_stretches[b].cmp(&self.column_stretches[a]));
            let mut remaining = remainder_w;
            for &idx in &indices {
                let add = remaining / (self.cols - idx as u32).max(1);
                if add > 0 {
                    col_widths[idx] += add;
                    remaining -= add;
                }
            }
            if remaining > 0 {
                col_widths[indices[0]] += remaining;
            }
        }

        let total_height: u32 = row_heights.iter().sum::<u32>() + spacing_y * (self.rows - 1);
        let remainder_h = available_height.saturating_sub(total_height);
        // The remainder is only shared out in `Fill` mode. In `Fixed` mode the leftover
        // height is deliberately left unused: distributing it is exactly the stretch
        // that mode exists to avoid, and doing it here would silently undo the caller's
        // choice on every frame the container grew.
        if remainder_h > 0 && !row_heights.is_empty() && self.row_sizing == RowSizing::Fill {
            let mut indices: Vec<usize> = (0..self.rows as usize).collect();
            indices.sort_by(|&a, &b| self.row_stretches[b].cmp(&self.row_stretches[a]));
            let mut remaining = remainder_h;
            for &idx in &indices {
                let add = remaining / (self.rows - idx as u32).max(1);
                if add > 0 {
                    row_heights[idx] += add;
                    remaining -= add;
                }
            }
            if remaining > 0 {
                row_heights[indices[0]] += remaining;
            }
        }

        // Recompute offsets after remainder distribution
        current_x = 0;
        for col in 0..self.cols {
            col_x_offsets[col as usize] = current_x;
            current_x += col_widths[col as usize] as i32 + spacing_x as i32;
        }
        current_y = 0;
        for row in 0..self.rows {
            row_y_offsets[row as usize] = current_y;
            current_y += row_heights[row as usize] as i32 + spacing_y as i32;
        }

        for row in 0..self.rows {
            for col in 0..self.cols {
                if let Some(widget_id) =
                    self.cells.get((row * self.cols + col) as usize).copied().flatten()
                {
                    let cell_width = col_widths[col as usize];
                    let cell_height = row_heights[row as usize];
                    let x = rect.x + margin as i32 + col_x_offsets[col as usize];
                    let y = rect.y + margin as i32 + row_y_offsets[row as usize];
                    widgets(widget_id, Rect::new(x, y, cell_width, cell_height));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::Vec;

    /// Collects the rects a layout produces, keyed by widget id.
    fn placed(layout: &GridLayout, rect: Rect) -> Vec<(ObjectId, Rect)> {
        let mut out = Vec::new();
        layout.update(rect, &mut |id, r| out.push((id, r)));
        out
    }

    fn rect_of(out: &[(ObjectId, Rect)], id: ObjectId) -> Rect {
        out.iter().find(|(i, _)| *i == id).map(|(_, r)| *r).expect("the id was placed")
    }

    /// **The defect this pins.** A grid of controls in a tall container.
    ///
    /// `Fill` (the default) divides the container height across the rows, so a
    /// two-row grid in a 582px page gives every control a 291px cell and stretches it
    /// to fill — a button becomes a tall blank panel. `Fixed` preserves the row height
    /// the caller asked for and leaves the surplus unused.
    #[test]
    fn fixed_row_sizing_does_not_stretch_controls_to_fill_the_container() {
        let mut fill = GridLayout::new(2, 3, 6, 0);
        for col in 0..3 {
            fill.set_widget(0, col, 10 + col as ObjectId);
            fill.set_widget(1, col, 20 + col as ObjectId);
        }
        let mut fixed = GridLayout::new(2, 3, 6, 0).with_row_sizing(RowSizing::Fixed(30));
        for col in 0..3 {
            fixed.set_widget(0, col, 10 + col as ObjectId);
            fixed.set_widget(1, col, 20 + col as ObjectId);
        }

        // A page-sized rect: the case the control demo hits.
        let page = Rect::new(8, 82, 1144, 582);
        let fill_out = placed(&fill, page);
        let fixed_out = placed(&fixed, page);

        // `Fill` gives a row half the page, which is what stretches a control.
        assert!(
            rect_of(&fill_out, 10).height > 200,
            "the default must still fill, or this test would not be pinning the defect: {:?}",
            rect_of(&fill_out, 10)
        );

        // `Fixed` keeps the requested height on every row.
        for id in [10, 11, 12, 20, 21, 22] {
            assert_eq!(
                rect_of(&fixed_out, id).height,
                30,
                "a fixed row must be exactly the requested height for id {id}"
            );
        }

        // The rows stay inside the container: the surplus is left unused, not pushed
        // past the bottom edge.
        for id in [10, 20] {
            let r = rect_of(&fixed_out, id);
            assert!(
                r.y + r.height as i32 <= page.y + page.height as i32,
                "a fixed row must stay inside the rect: {r:?}"
            );
        }
    }

    /// The rows must not drift apart as the container grows: a fixed row's height is
    /// a property of the row, not of how much room happens to be available.
    #[test]
    fn fixed_rows_keep_their_height_across_container_sizes() {
        let mut grid = GridLayout::new(3, 1, 6, 0).with_row_sizing(RowSizing::Fixed(24));
        grid.set_widget(0, 0, 1);
        grid.set_widget(1, 0, 2);
        grid.set_widget(2, 0, 3);

        for height in [200u32, 400, 900] {
            let out = placed(&grid, Rect::new(0, 0, 300, height));
            for id in [1, 2, 3] {
                assert_eq!(
                    rect_of(&out, id).height,
                    24,
                    "row height must not follow the container at height {height}"
                );
            }
        }
    }

    /// A fixed row taller than the container is clamped, so it cannot place later rows
    /// outside the rect.
    #[test]
    fn a_fixed_row_taller_than_the_container_is_clamped() {
        let mut grid = GridLayout::new(2, 1, 0, 0).with_row_sizing(RowSizing::Fixed(500));
        grid.set_widget(0, 0, 1);
        grid.set_widget(1, 0, 2);

        let rect = Rect::new(0, 0, 100, 60);
        let out = placed(&grid, rect);
        for id in [1, 2] {
            let r = rect_of(&out, id);
            assert!(
                r.y + r.height as i32 <= rect.y + rect.height as i32,
                "a clamped row must stay inside the rect: {r:?}"
            );
        }
    }

    /// The default is unchanged, so introducing the mode did not alter existing
    /// callers.
    #[test]
    fn the_default_row_sizing_is_fill() {
        assert_eq!(GridLayout::new(2, 2, 0, 0).row_sizing(), RowSizing::Fill);
    }
}
