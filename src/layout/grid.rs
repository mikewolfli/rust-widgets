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

/// A child placed at an explicit cell, occupying `col_span` columns and `row_span` rows.
///
/// # Why a placement carries spans
///
/// The grid's cell array holds one widget per cell, so a widget that spans two columns
/// has no single cell to sit in. The span was previously expressible in JSON (`col_span`,
/// `row_span`) and parsed, but the layout had no way to receive it — so the keys were
/// read and dropped. Keeping the span next to the origin makes "this widget occupies
/// (row..row+row_span, col..col+col_span)" one fact rather than three that can disagree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GridPlacement {
    /// The widget placed at the origin cell.
    widget_id: ObjectId,
    /// Columns covered — at least 1, clamped to the grid's column count at insertion.
    col_span: u32,
    /// Rows covered — at least 1, clamped to the grid's row count at insertion.
    row_span: u32,
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
    cells: Vec<Option<GridPlacement>>,
}
impl GridLayout {
    /// The maximum number of cells the grid will allocate storage for.
    ///
    /// `rows`/`cols` may come from untrusted input (JSON) and their product can overflow
    /// `u32`, so the cell `Vec` is capped here. The cap is a property of the *storage*:
    /// the logical extent (`rows`/`cols`) must never exceed what this array can address,
    /// or a placement would pass the range check and then have no slot to be written to.
    const MAX_CELLS: usize = 1_000_000;

    /// Create a grid layout with fixed rows/columns.
    ///
    /// The logical extent is reduced until it is exactly addressable by the capped cell
    /// array, so `rows * cols` always equals the number of storage slots. A `rows × cols`
    /// product above [`Self::MAX_CELLS`] is therefore refused deterministically rather than
    /// leaving a logical range with no backing storage.
    pub fn new(rows: u32, cols: u32, spacing: u32, margin: u32) -> Self {
        let safe_rows = rows.max(1);
        let safe_cols = cols.max(1);
        let (safe_rows, safe_cols) = Self::fit_extent(safe_rows, safe_cols);
        let cell_count = safe_rows as usize * safe_cols as usize;
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

    /// Reduces `(rows, cols)` until `rows * cols <= MAX_CELLS`, preserving the aspect
    /// ratio as far as the integer grid allows and never dropping below one row/column.
    ///
    /// Returns the reduced pair, guaranteed to satisfy `rows * cols <= MAX_CELLS`.
    fn fit_extent(rows: u32, cols: u32) -> (u32, u32) {
        let rows = rows.max(1);
        let cols = cols.max(1);
        if rows as u64 * cols as u64 <= Self::MAX_CELLS as u64 {
            return (rows, cols);
        }
        // The grid's dimensions start uniform, so scale both axes by the same factor that
        // brings the product within the cap, using the integer square root as the target.
        let allowed = (Self::MAX_CELLS as f64).sqrt() as u32;
        let scaled_rows = rows.min(allowed.max(1));
        let scaled_cols = cols.min((Self::MAX_CELLS as u64 / scaled_rows.max(1) as u64) as u32);
        (scaled_rows.max(1), scaled_cols.max(1))
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

    /// Grows the grid so `row` is addressable, if it is not already.
    ///
    /// # Why a grid needs this at all
    ///
    /// A `Grid` is constructed with one row and "the row count grows as children are added"
    /// — but the growth used to happen only inside `add_widget`'s own placement. A caller
    /// placing a widget at an **explicit** cell bypasses that path entirely, so a document
    /// saying `"row": 1` against the freshly built one-row grid was refused: the cell it named
    /// did not exist yet, and the widget silently vanished from the arrangement. That is the
    /// same class of defect as the dropped `col`/`row` keys — a placement the caller asked for
    /// and did not get, with nothing said about it.
    ///
    /// Existing children and their stretches are preserved; a new column's stretch defaults to
    /// the same `1` a fresh grid gives it. Shrinking is not offered, because discarding occupied
    /// cells is a decision the caller must make explicitly through `remove_widget`.
    ///
    /// The logical extent stays exactly addressable: a requested extent whose product would
    /// exceed [`Self::MAX_CELLS`] is reduced to the cap, so `rows * cols` always equals the
    /// cell array's length and no logical cell can lack a storage slot.
    ///
    /// Returns whether the grid grew.
    pub fn grow_to_fit(&mut self, row: u32, col: u32) -> bool {
        let needed_rows = row.saturating_add(1);
        let needed_cols = col.saturating_add(1);
        if needed_rows <= self.rows && needed_cols <= self.cols {
            return false;
        }
        let (new_rows, new_cols) =
            Self::fit_extent(self.rows.max(needed_rows), self.cols.max(needed_cols));
        if new_rows == self.rows && new_cols == self.cols {
            return false;
        }
        let mut grown = vec![None; new_rows as usize * new_cols as usize];
        for r in 0..self.rows {
            for c in 0..self.cols {
                let from = r.saturating_mul(self.cols).saturating_add(c) as usize;
                let to = r.saturating_mul(new_cols).saturating_add(c) as usize;
                if let (Some(placement), Some(slot)) =
                    (self.cells.get(from).copied().flatten(), grown.get_mut(to))
                {
                    *slot = Some(placement);
                }
            }
        }
        self.cells = grown;
        self.rows = new_rows;
        self.cols = new_cols;
        self.row_stretches.resize(new_rows as usize, 1);
        self.column_stretches.resize(new_cols as usize, 1);
        true
    }

    /// Returns the current row-sizing rule.
    pub fn row_sizing(&self) -> RowSizing {
        self.row_sizing
    }

    /// Assign widget to explicit cell, occupying one column and one row.
    pub fn set_widget(&mut self, row: u32, col: u32, widget_id: ObjectId) {
        self.set_widget_spanning(row, col, 1, 1, widget_id);
    }

    /// Assign widget to an explicit cell that spans `col_span` columns and `row_span` rows.
    ///
    /// A span is clamped to the space **still available** from the origin, not to the grid's
    /// total: a widget declared at the last column with a `col_span` of 3 cannot reach past the
    /// edge, and clamping to the total instead would silently place it over cells that belong to
    /// other widgets. A span of `0` is read as `1` — "span no columns" is not a placement, and the
    /// one honest reading of it is the single-cell default.
    ///
    /// A wide widget must occupy its whole rectangle, not just its origin cell, or a later
    /// `set_widget` would place a second widget in the area the first already covers.
    ///
    /// # An out-of-range cell grows the grid
    ///
    /// A grid starts as one row and grows as children arrive; a caller that places at an
    /// explicit cell is doing the growing itself, so an unreachable cell is grown up to rather
    /// than refused. Refusing was the behaviour that lost the widget entirely.
    ///
    /// A **uniform** grid must not grow — its dimensions are the point of the type — so
    /// [`UniformGridLayout`](crate::layout::UniformGridLayout) uses
    /// [`place_within_extent`](Self::place_within_extent) instead.
    pub fn set_widget_spanning(
        &mut self,
        row: u32,
        col: u32,
        col_span: u32,
        row_span: u32,
        widget_id: ObjectId,
    ) {
        self.grow_to_fit(row, col);
        if !self.has_storage_slot(row, col) {
            // `grow_to_fit` failed to cover the cell because the requested extent's product
            // exceeds the cell cap, so there is no slot to write. The cell is genuinely
            // unaddressable, not merely out of the current logical range, so the placement
            // is refused and the caller is told through the log.
            log::warn!(
                "GridLayout: cell ({row}, {col}) is past the grid's cell cap of 1,000,000; the \
                 placement is dropped"
            );
            return;
        }
        let col_span = col_span.max(1).min(self.cols - col);
        let row_span = row_span.max(1).min(self.rows - row);
        let placement = GridPlacement { widget_id, col_span, row_span };
        for r in row..row + row_span {
            for c in col..col + col_span {
                self.cells[Self::cell_index(self.cols, r, c)] = Some(placement);
            }
        }
    }

    /// The storage index of cell `(row, col)` under a given column count.
    ///
    /// `rows * cols == cells.len()` is an invariant of this type (enforced by
    /// [`Self::fit_extent`]), so this index is always in bounds for a logical cell.
    fn cell_index(cols: u32, row: u32, col: u32) -> usize {
        (row as usize) * (cols as usize) + (col as usize)
    }

    /// Whether `(row, col)` is inside the logical extent **and** has a storage slot.
    ///
    /// The two are one fact here: the logical extent never exceeds the capped array, so a
    /// cell inside `rows × cols` is always addressable. This predicate makes that explicit
    /// at every write site so a placement can never claim success without storing anything.
    fn has_storage_slot(&self, row: u32, col: u32) -> bool {
        row < self.rows
            && col < self.cols
            && Self::cell_index(self.cols, row, col) < self.cells.len()
    }

    /// Places a widget at an explicit cell that must already exist.
    ///
    /// # Why a caller would want the refusing variant
    ///
    /// [`set_widget_spanning`](Self::set_widget_spanning) grows the grid to reach the cell it was
    /// given, which is right for a grid whose row count is "as many as there are children". It is
    /// wrong for a grid whose dimensions are fixed by the caller — a uniform grid, where growing
    /// would silently undo the sizes the caller chose. This variant refuses an out-of-range cell so
    /// the fixed-dimension case can share the cell array without sharing the growth.
    ///
    /// Returns whether the placement was made. A cell with no storage slot — outside the
    /// logical extent, or (hypothetically) past the capped array — returns `false` rather
    /// than reporting a success that wrote nothing. A refusal leaves the layout unchanged.
    pub fn place_within_extent(
        &mut self,
        row: u32,
        col: u32,
        col_span: u32,
        row_span: u32,
        widget_id: ObjectId,
    ) -> bool {
        if !self.has_storage_slot(row, col) {
            return false;
        }
        let col_span = col_span.max(1).min(self.cols - col);
        let row_span = row_span.max(1).min(self.rows - row);
        let placement = GridPlacement { widget_id, col_span, row_span };
        for r in row..row + row_span {
            for c in col..col + col_span {
                self.cells[Self::cell_index(self.cols, r, c)] = Some(placement);
            }
        }
        true
    }

    /// Places a widget at the next free cell **without** growing the grid.
    ///
    /// Returns `false` when every cell is occupied. This is the fixed-dimension
    /// auto-placement a uniform grid needs: its row/column counts are the point of
    /// the type, so a full grid must refuse rather than grow (S-43).
    pub fn place_next_free_cell(&mut self, widget_id: ObjectId) -> bool {
        let Some(index) = self.cells.iter().position(|cell| cell.is_none()) else {
            log::warn!(
                "GridLayout: no free cell for widget {widget_id}; a fixed-dimension grid does \
                 not grow"
            );
            return false;
        };
        self.cells[index] = Some(GridPlacement { widget_id, col_span: 1, row_span: 1 });
        true
    }

    /// The cell `widget_id` is anchored at, as `(row, col)`, when it was placed.
    ///
    /// A spanning widget occupies several cells; this reports the top-left one — the position
    /// its geometry is derived from — rather than whichever cell happens to be scanned first.
    pub fn cell_of(&self, widget_id: ObjectId) -> Option<(u32, u32)> {
        self.cells.iter().enumerate().find_map(|(index, cell)| match cell {
            Some(placement) if placement.widget_id == widget_id => {
                Some((index as u32 / self.cols, index as u32 % self.cols))
            }
            _ => None,
        })
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
    /// Auto-placement: the next free cell, one cell in size. A child that wants an explicit
    /// cell or a span is placed with `set_widget`/`set_widget_spanning` before the layout runs;
    /// the declarative loader does exactly that for `col`/`row`/`col_span`/`row_span`.
    ///
    /// # Why this grows the grid
    ///
    /// A grid of `columns` columns and one row grows as children arrive, so a sixth child in a
    /// two-column grid needs a third row to exist. Without the growth the cell it should occupy
    /// is outside the array, and the child would be reported in `child_ids` (which reads the
    /// array, so no) — it would simply disappear from the arrangement. The growth is here rather
    /// than in the caller because auto-placement is the one path that *chooses* the cell.
    fn add_widget(&mut self, widget_id: ObjectId, _stretch: u32) {
        if let Some(index) = self.cells.iter().position(|cell| cell.is_none()) {
            self.cells[index] = Some(GridPlacement { widget_id, col_span: 1, row_span: 1 });
            return;
        }
        // Every existing cell is taken: append a fresh row and use its first cell. The new row
        // is one cell wide per column, so the widget lands at `(rows - 1, 0)` after the grow.
        let grown = self.grow_to_fit(self.rows, 0);
        if !grown {
            log::warn!(
                "GridLayout: no free cell for widget {widget_id} and the grid cannot grow past its \
                 cell cap; the widget was not placed"
            );
            return;
        }
        let index = self.rows.saturating_sub(1).saturating_mul(self.cols) as usize;
        if let Some(slot) = self.cells.get_mut(index) {
            *slot = Some(GridPlacement { widget_id, col_span: 1, row_span: 1 });
        }
    }
    fn remove_widget(&mut self, widget_id: ObjectId) {
        // Every cell the widget occupies is cleared, not just its origin: a spanning widget
        // would otherwise leave its other cells holding a placement for a widget that is no
        // longer a child, so the cell would never be offered to an auto-placed sibling.
        for cell in &mut self.cells {
            if cell.is_some_and(|placement| placement.widget_id == widget_id) {
                *cell = None;
            }
        }
    }
    fn child_ids(&self) -> Vec<ObjectId> {
        // One id per **widget**, not per occupied cell: a spanning widget covers several cells
        // and must not be reported several times, which is what turns a child count into a
        // number larger than the number of children.
        let mut ids: Vec<ObjectId> = Vec::new();
        for cell in self.cells.iter().flatten() {
            if !ids.contains(&cell.widget_id) {
                ids.push(cell.widget_id);
            }
        }
        ids
    }
    fn has_child(&self, id: ObjectId) -> bool {
        self.cells.iter().flatten().any(|placement| placement.widget_id == id)
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

        // Calculate column widths and x-offsets based on per-column stretch factors.
        // The total is summed in `u64`: two columns of weight `u32::MAX` must not overflow a
        // `u32` (a debug panic today, and a wrapped total that over-allocated two ~100px columns
        // into a 100px container in release).
        let total_col_stretch: u64 = self.column_stretches.iter().map(|&s| s as u64).sum();
        // Calculate row heights and y-offsets based on per-row stretch factors.
        let total_row_stretch: u64 = self.row_stretches.iter().map(|&s| s as u64).sum();

        // Precompute cumulative column width sums for x-offsets (fraction-aware)
        let mut col_widths: Vec<u32> = Vec::with_capacity(self.cols as usize);
        let mut col_x_offsets: Vec<i32> = Vec::with_capacity(self.cols as usize);
        let mut current_x: i32 = 0;
        for col in 0..self.cols {
            let cell_width = (available_width as u64 * self.column_stretches[col as usize] as u64)
                .checked_div(total_col_stretch)
                .map(|width| width as u32)
                .unwrap_or_else(|| available_width / self.cols);
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
                    (available_height as i32 - current_y).max(0).min(height as i32) as u32
                }
                RowSizing::Fill => (available_height as u64
                    * self.row_stretches[row as usize] as u64)
                    .checked_div(total_row_stretch)
                    .map(|height| height as u32)
                    .unwrap_or_else(|| available_height / self.rows),
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

        // A spanning widget covers several cells and therefore appears in each of them; it is
        // emitted once, from its origin (top-left) cell, with the widths and heights of the
        // cells it covers summed plus the spacing it bridges. Emitting it from every covered
        // cell would report one widget several times at several positions, and the last write
        // would win arbitrarily.
        let mut emitted: Vec<ObjectId> = Vec::new();
        for row in 0..self.rows {
            for col in 0..self.cols {
                let Some(placement) =
                    self.cells.get((row * self.cols + col) as usize).copied().flatten()
                else {
                    continue;
                };
                if placement.widget_id == 0 || emitted.contains(&placement.widget_id) {
                    continue;
                }
                if self.cell_of(placement.widget_id) != Some((row, col)) {
                    continue;
                }
                emitted.push(placement.widget_id);

                let last_col = (col + placement.col_span - 1).min(self.cols - 1);
                let last_row = (row + placement.row_span - 1).min(self.rows - 1);
                let cell_width: u32 = (col..=last_col).map(|c| col_widths[c as usize]).sum::<u32>()
                    + spacing_x * (last_col - col);
                let cell_height: u32 =
                    (row..=last_row).map(|r| row_heights[r as usize]).sum::<u32>()
                        + spacing_y * (last_row - row);
                let x = rect.x + margin as i32 + col_x_offsets[col as usize];
                let y = rect.y + margin as i32 + row_y_offsets[row as usize];
                widgets(placement.widget_id, Rect::new(x, y, cell_width, cell_height));
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

    /// **The defect this pins.** A widget placed with `col_span` must be as wide as the
    /// cells it covers, plus the spacing it bridges.
    ///
    /// The span was previously unrepresentable: a cell held one `ObjectId`, so the only
    /// possible answer for a spanning child was the width of its origin cell. A document
    /// could write `col_span: 3` and get a third of the width it asked for, with nothing
    /// reporting that the key had been discarded.
    #[test]
    fn a_spanning_widget_covers_every_cell_it_was_given() {
        let spacing = 10;
        let mut grid = GridLayout::new(2, 3, spacing, 0);
        grid.set_widget_spanning(0, 0, 3, 1, 7);
        grid.set_widget_spanning(1, 0, 1, 1, 8);
        grid.set_widget_spanning(1, 1, 1, 1, 9);
        grid.set_widget_spanning(1, 2, 1, 1, 10);

        let page = Rect::new(0, 0, 320, 100);
        let out = placed(&grid, page);

        let span = rect_of(&out, 7);
        let single_a = rect_of(&out, 8);
        let single_c = rect_of(&out, 10);

        assert_eq!(span.x, single_a.x, "a span starts at the left edge of its origin cell");
        // Three cells plus the two gaps they bridge: the right edge of the last covered
        // cell. Comparing edges rather than widths is what makes this a statement about
        // coverage instead of about arithmetic that could be wrong in one place only.
        assert_eq!(
            span.x + span.width as i32,
            single_c.x + single_c.width as i32,
            "a three-column span must reach the right edge of the third column"
        );
        assert_eq!(
            span.width,
            3 * single_a.width + 2 * spacing,
            "the span bridges the spacing between the cells it covers"
        );
    }

    /// A widget is reported once, at its origin, however many cells it covers.
    ///
    /// A spanning widget occupies several cells; emitting it from each of them would
    /// report one widget several times at overlapping positions, and whichever write
    /// landed last would win arbitrarily.
    #[test]
    fn a_spanning_widget_is_reported_once_at_its_origin() {
        let mut grid = GridLayout::new(2, 2, 0, 0);
        grid.set_widget_spanning(0, 0, 2, 2, 42);

        let out = placed(&grid, Rect::new(0, 0, 200, 200));
        assert_eq!(out.len(), 1, "one widget, one geometry: {out:?}");
        assert_eq!(out[0].0, 42);
        assert_eq!(out[0].1, Rect::new(0, 0, 200, 200), "a 2x2 span fills the whole rect");

        assert_eq!(grid.child_ids(), vec![42], "a spanning widget is one child, not four");
        assert!(grid.has_child(42));
        assert_eq!(grid.cell_of(42), Some((0, 0)));
    }

    /// A span is clamped to the space still available from its origin.
    ///
    /// Clamping to the grid's total instead let a widget at the last column claim three
    /// columns' worth of width, so it overlapped cells that belong to other widgets.
    #[test]
    fn a_span_is_clamped_to_the_remaining_cells() {
        let mut grid = GridLayout::new(1, 3, 0, 0);
        grid.set_widget_spanning(0, 2, 5, 1, 5);
        grid.set_widget(0, 0, 1);
        grid.set_widget(0, 1, 2);

        let out = placed(&grid, Rect::new(0, 0, 300, 100));
        assert_eq!(rect_of(&out, 5), Rect::new(200, 0, 100, 100), "one column was left");
        assert_eq!(rect_of(&out, 2).x + rect_of(&out, 2).width as i32, 200, "no overlap");
    }

    /// A zero span is read as one cell: "cover no cells" is not a placement.
    #[test]
    fn a_zero_span_is_read_as_one_cell() {
        let mut grid = GridLayout::new(1, 2, 0, 0);
        grid.set_widget_spanning(0, 0, 0, 0, 3);
        let out = placed(&grid, Rect::new(0, 0, 200, 50));
        assert_eq!(rect_of(&out, 3).width, 100);
        assert_eq!(grid.cell_of(3), Some((0, 0)));
    }

    /// Removing a spanning widget frees every cell it covered.
    ///
    /// Clearing only the origin left the other cells holding a placement for a widget
    /// that is no longer a child, so the cell was never offered to an auto-placed sibling.
    #[test]
    fn removing_a_spanning_widget_frees_all_of_its_cells() {
        let mut grid = GridLayout::new(1, 3, 0, 0);
        grid.set_widget_spanning(0, 0, 3, 1, 77);
        grid.remove_widget(77);

        assert_eq!(grid.cell_count(), 0, "every covered cell must be free again");
        assert!(!grid.has_child(77));
        assert!(grid.child_ids().is_empty());

        // The freed cells are usable: an auto-placed child lands in the first of them.
        grid.add_widget(78, 0);
        assert_eq!(grid.cell_of(78), Some((0, 0)));
    }

    /// A placement past the current extent **grows** the grid rather than being refused.
    ///
    /// # The defect this pins
    ///
    /// A grid is built with one row and grows as children arrive — but the growth lived only in
    /// `add_widget`'s auto-placement. A caller placing at an **explicit** cell bypasses that path,
    /// so `"row": 1` against the freshly built one-row grid named a cell that did not exist and
    /// the widget vanished from the arrangement without a diagnostic. That is the same failure as
    /// the dropped `col`/`row` keys: a placement that was asked for and not delivered.
    #[test]
    fn a_placement_past_the_extent_grows_the_grid_instead_of_vanishing() {
        let mut grid = GridLayout::new(1, 2, 0, 0);
        grid.set_widget_spanning(3, 1, 1, 1, 99);

        assert_eq!(grid.rows(), 4, "the grid must reach row 3");
        assert_eq!(grid.cols(), 2, "the column count is already sufficient");
        assert_eq!(grid.cell_of(99), Some((3, 1)), "the widget lands where it was asked to");
        assert!(grid.has_child(99));
        assert_eq!(grid.child_ids(), vec![99]);
    }

    /// The defect this pins: `cells` was capped at 1,000,000 while `rows`/`cols` kept the
    /// larger logical range, so a placement inside the logical range could have no storage
    /// slot — `place_within_extent` returned `true` having written nothing. The logical
    /// extent is now reduced to exactly what the capped array addresses, and a cell with no
    /// slot is refused.
    #[test]
    fn an_oversized_extent_is_reduced_to_exactly_addressable_storage() {
        // 1 × 1,000,001 would need more slots than the cap allows.
        let grid = GridLayout::new(1, 1_000_001, 0, 0);
        assert_eq!(grid.total_cells(), grid.rows() as usize * grid.cols() as usize);
        assert!(grid.total_cells() <= 1_000_000, "the extent must fit the cap");

        // An oversized square is reduced on both axes while staying addressable.
        let grid = GridLayout::new(2000, 2000, 0, 0);
        assert_eq!(grid.total_cells(), grid.rows() as usize * grid.cols() as usize);
        assert!(grid.total_cells() <= 1_000_000);
    }

    /// A `place_within_extent` at a cell that has no storage slot must report `false` and
    /// leave the layout unchanged — the success it used to report wrote nothing.
    #[test]
    fn place_within_extent_refuses_a_cell_with_no_storage_slot() {
        let mut grid = GridLayout::new(2, 2, 0, 0);
        // Out of the logical extent: refused, and no cell is touched.
        assert!(!grid.place_within_extent(5, 5, 1, 1, 99));
        assert_eq!(grid.cell_count(), 0, "a refusal must not mutate the layout");
        assert!(!grid.has_child(99));

        // In extent: accepted, and the storage slot is written.
        assert!(grid.place_within_extent(1, 1, 1, 1, 7));
        assert_eq!(grid.cell_count(), 1);
        assert!(grid.has_child(7));
    }

    /// The far side of the cap: a grid just under the cap keeps its full extent and stores
    /// a placement at its last addressable cell.
    #[test]
    fn the_cell_cap_boundary_is_consistent() {
        // Exactly under the cap: 1000 × 1000 = 1,000,000 slots.
        let mut grid = GridLayout::new(1000, 1000, 0, 0);
        assert_eq!(grid.total_cells(), 1_000_000);
        assert_eq!(grid.rows(), 1000);
        assert_eq!(grid.cols(), 1000);
        assert!(grid.place_within_extent(999, 999, 1, 1, 5), "the last slot must be writable");
        assert_eq!(grid.cell_of(5), Some((999, 999)));

        // Just over the cap: the extent is reduced so no logical cell lacks a slot.
        let grid = GridLayout::new(1001, 1000, 0, 0);
        assert!(grid.total_cells() <= 1_000_000);
        assert_eq!(grid.total_cells(), grid.rows() as usize * grid.cols() as usize);
    }

    /// A growing placement cannot silently drop a widget: after `set_widget_spanning` the
    /// widget is either placed somewhere addressable or refused outright.
    #[test]
    fn a_placement_that_cannot_be_stored_is_not_claimed_as_placed() {
        let mut grid = GridLayout::new(1, 1, 0, 0);
        // A request reaching column 1,000,001 exceeds the capped storage; the reduced
        // extent has no slot for it, so the widget must not be reported as placed.
        grid.set_widget_spanning(0, 1_000_001, 1, 1, 42);
        assert!(!grid.has_child(42), "a placement with no slot must be refused");
        assert_eq!(grid.cell_count(), 0);
    }

    /// Auto-placement appends a row when every existing cell is occupied.
    ///
    /// A six-child two-column grid needs three rows, and used to drop the children that had no
    /// cell left — `add_widget`'s `find(|cell| cell.is_none())` simply found nothing and returned.
    #[test]
    fn auto_placement_grows_a_row_when_the_grid_is_full() {
        let mut grid = GridLayout::new(1, 2, 0, 0);
        for id in 1..=6 {
            grid.add_widget(id, 1);
        }

        assert_eq!(grid.child_ids(), vec![1, 2, 3, 4, 5, 6]);
        assert_eq!(grid.rows(), 3, "six children in two columns need three rows");
        assert_eq!(grid.cell_of(5), Some((2, 0)));
        assert_eq!(grid.cell_of(6), Some((2, 1)));
    }

    /// Two columns of weight `u32::MAX` must not overflow the stretch sum.
    ///
    /// The sum was a `u32`, so this case panicked in debug and, in release, wrapped to a total
    /// just under `2 * u32::MAX` — which over-allocated two ~100px columns into a 100px container.
    #[test]
    fn max_column_stretches_do_not_overflow_or_overrun_the_container() {
        let mut grid = GridLayout::new(1, 2, 0, 0);
        grid.set_widget(0, 0, 1);
        grid.set_widget(0, 1, 2);
        grid.set_column_stretch(u32::MAX);

        let rect = Rect::new(0, 0, 100, 50);
        let out = placed(&grid, rect);

        let left = rect_of(&out, 1);
        let right = rect_of(&out, 2);
        assert_eq!(left.width, 50, "equal max weights split the container evenly");
        assert_eq!(right.width, 50, "equal max weights split the container evenly");
        assert_eq!(left.width + right.width, 100, "the columns must not exceed the container");
        assert!(
            right.x + right.width as i32 <= rect.x + rect.width as i32,
            "right column: {right:?}"
        );
    }

    /// Mixed max and minimal row weights must share the container without overrunning it.
    #[test]
    fn mixed_row_stretches_do_not_overflow_or_overrun_the_container() {
        let mut grid = GridLayout::new(2, 1, 0, 0);
        grid.set_widget(0, 0, 1);
        grid.set_widget(1, 0, 2);
        grid.set_row_stretch_for_row(0, u32::MAX);
        grid.set_row_stretch_for_row(1, 1);

        let rect = Rect::new(0, 0, 50, 60);
        let out = placed(&grid, rect);

        let top = rect_of(&out, 1);
        let bottom = rect_of(&out, 2);
        assert!(top.height >= bottom.height, "the max-weight row gets the larger share");
        assert_eq!(top.height + bottom.height, 60, "the rows must not exceed the container");
        assert!(top.y + top.height as i32 <= rect.y + rect.height as i32, "top row: {top:?}");
        assert!(
            bottom.y + bottom.height as i32 <= rect.y + rect.height as i32,
            "bottom row: {bottom:?}"
        );
    }
}
