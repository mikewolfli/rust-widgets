// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! GridTable widget — feature-rich virtualized table with grid lines, headers,
//! sorting, selection, and column resize.
//!
//! Uses the `IncrementalTableDataSource` protocol from `data_source` for data access.

#[cfg(not(alloc_frugal))]
use std::sync::Arc;

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::expect_usize;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

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
    //
    // These are the *last-resort* literals for the grid's chrome. `Draw` resolves explicit
    // style first, then the theme's resolved style, and treats a colour that still equals one
    // of these as "nothing was configured", so they stay published for callers of the
    // setters without pinning the rendering to a light-only palette.
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
    /// The neutral-literal grid colour the constructor seeds [`Self::grid_color`] with.
    ///
    /// `Draw` compares the current value against this to tell "the caller left the
    /// appearance alone" from "the caller asked for this exact colour".
    const DEFAULT_GRID_COLOR: Color = Color::rgb(210, 215, 225);
    /// See [`Self::DEFAULT_GRID_COLOR`].
    const DEFAULT_HEADER_BG: Color = Color::rgb(240, 242, 245);
    /// See [`Self::DEFAULT_GRID_COLOR`].
    const DEFAULT_HEADER_TEXT_COLOR: Color = Color::rgb(40, 50, 70);
    /// See [`Self::DEFAULT_GRID_COLOR`].
    const DEFAULT_SELECTED_BG: Color = Color::rgb(200, 220, 250);

    /// How far either side of a column's right edge the resize handle answers, in pixels.
    ///
    /// Named because the hit test and the drawn boundary are the same edge: the handle used the
    /// literal `5` while the painter drew the column's own `x + width`, so changing one could move
    /// the grab region away from the line it is meant to be grabbing.
    const RESIZE_HANDLE_WIDTH: u32 = 5;

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
            grid_color: Self::DEFAULT_GRID_COLOR,
            _grid_thickness: 1,
            header_bg: Self::DEFAULT_HEADER_BG,
            header_text_color: Self::DEFAULT_HEADER_TEXT_COLOR,
            selected_bg: Self::DEFAULT_SELECTED_BG,
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
            let default_w = self.default_column_width();
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
        // The floor is applied on the way *out*, so a width that entered the vector below
        // `min_column_width` — or a `min_column_width` raised after the fact — is still reported as the
        // column the painters, the hit tests and the fit walk all agree on.
        self.column_widths
            .get(col)
            .copied()
            .unwrap_or_else(|| self.default_column_width())
            .max(self.min_column_width)
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
    ///
    /// # Why the walks below are the *only* place the fit is decided
    ///
    /// Five paths need to know where a column starts or ends: the header painter, the data-cell
    /// painter, the pointer hit test, the header hit test and the resize-handle hit test. Each used to
    /// walk the widths itself, with its own copy of the default width, its own `min_column_width`
    /// clamp and its own stopping rule — so "which column is under this pixel" was answered five
    /// slightly different ways. They agree only while all five are edited together, which is exactly
    /// the arrangement that put a resize handle under the neighbouring column once before.
    ///
    /// [`Self::column_span_at`] and [`Self::column_span_at_x`] are now the one derivation, and the
    /// five consumers name the offset they have.
    fn update_visibility(&mut self) {
        let rect = self.base.geometry();
        let rh = self.row_height.max(1) as i32;
        let header_h = self.header_height as i32;
        let rnw = self.row_number_width as i32;

        let data_h = (rect.height as i32).saturating_sub(header_h).max(0);
        let data_w = (rect.width as i32).saturating_sub(rnw).max(0);

        self.visible_rows = if rh > 0 { (data_h / rh) as usize } else { 0 };
        self.visible_columns = 0;

        // `data_w` is an *extent*, which is what the fit test wants: "would this column end past the
        // data area".
        let mut acc = 0i32;
        let cols = self.column_count();
        self.ensure_column_widths(cols);
        for ci in 0..cols {
            let iw = self.column_width(ci) as i32;
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
    // Column geometry — one derivation, five consumers
    // -----------------------------------------------------------------------

    /// The width every column falls back to when the caller has not sized it.
    ///
    /// Both the default and the floor are the same fact — a column is never narrower than
    /// [`Self::min_column_width`] and its unsized width is at least 120 — and they were spelled
    /// separately in four places. Read through [`Self::column_width`] by every walk.
    fn default_column_width(&self) -> u32 {
        120u32.max(self.min_column_width)
    }

    /// The columns that fit in the data area, in draw order: `(column, x, width)` with `x` **relative
    /// to the data area's left edge**.
    ///
    /// # Why the extent is a parameter and not `geometry()`
    ///
    /// The two callers measure room differently, and both are right:
    ///
    /// * a painter asks "how much room from my own left edge to the control's far edge", because every
    ///   run it draws starts at `scroll_column`; a horizontally scrolled table therefore has a *smaller*
    ///   budget, not a shifted one;
    /// * a hit test asks "how far is this pixel into the data area", which is the same extent measured
    ///   from the same edge — the pointer's x *is* the offset.
    ///
    /// Passing the extent keeps one loop for both instead of a "fit" copy and a "hit" copy that can
    /// disagree about the last column.
    ///
    /// # Why the containment test is on the *start*, not the end
    ///
    /// A column is in the run when it **begins** inside the extent, even if it ends past it. Testing
    /// `x + width <= extent` instead would drop the last column as soon as it was partly scrolled out —
    /// and for a pointer test that is fatal: [`Self::column_span_at_x`] passes the pointer's own offset
    /// as the extent, so a 100 px column would be "not in the run" for every offset below 100 and the
    /// first column would stop answering at all. The painter wants the same rule: a column that starts
    /// on screen is drawn and clipped, not skipped.
    fn visible_column_spans(&self, extent: i32) -> Vec<(usize, i32, i32)> {
        let cols = self.column_count();
        let mut spans = Vec::new();
        let mut x = 0i32;
        for ci in self.scroll_column..cols {
            if x >= extent {
                break;
            }
            spans.push((ci, x, self.column_width(ci) as i32));
            x += self.column_width(ci) as i32;
        }
        spans
    }

    /// The data column occupying `x_offset` pixels into the data area, as `(column, x, width)`.
    ///
    /// The counterpart of [`Self::visible_column_spans`] for a *point*: the extent covers exactly the
    /// pixels up to and including the pointer, and the span containing it is returned.
    ///
    /// Containment is tested here, on the finished spans, rather than inside the walk. The walk asks
    /// "does this column *start* before the pointer", which is the right question for building the run
    /// but not for picking a cell: it would return the last *begun* column for a pointer sitting in a
    /// gap or past every column. Selecting the containing span means "the cell you clicked" and "the
    /// cell you can see" are the same run, which is what the old, separately-accumulated copy in the
    /// hit test could not guarantee.
    fn column_span_at_x(&self, x_offset: i32) -> Option<(usize, i32, i32)> {
        if x_offset < 0 {
            return None;
        }
        self.visible_column_spans(x_offset.saturating_add(1))
            .into_iter()
            .find(|(_, x, w)| x_offset >= *x && x_offset < x + w)
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
        let (cardinal, _, _) = self.column_span_at_x(x_offset)?;
        Some(cardinal - self.scroll_column)
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
    ///
    /// # Why this reads the spans instead of accumulating widths
    ///
    /// The handle straddles a boundary: it is the `HANDLE_WIDTH` pixels either side of a column's right
    /// edge. Accumulating widths here and accumulating them again in the painter is how the handle and
    /// the line it sits on came apart — the boundary is whichever `x + width` the spans document, so
    /// asking *them* for it makes the two the same number.
    fn resize_handle_at_point(&self, point: Point) -> Option<usize> {
        let rect = self.base.geometry();
        let rnw = self.row_number_width as i32;
        let header_h = self.header_height as i32;

        if point.y < rect.y || point.y >= rect.y + header_h {
            return None;
        }

        let extent = (rect.x + rect.width as i32) - (rect.x + rnw);
        let handle_width = Self::RESIZE_HANDLE_WIDTH as i32;
        for (ci, x, cw) in self.visible_column_spans(extent) {
            if (point.x - (rect.x + rnw + x + cw)).abs() <= handle_width {
                return Some(ci);
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
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `GridTableWidget`'s property contract.
///
/// `selection_mode` publishes the shared lower-case tokens (`none`, `cell`,
/// `row`, `column`) rather than the type's `Debug` spelling, so a consumer can
/// treat it exactly like every other selection mode in the library. The counts
/// are derived from the data source and stay read-only.
impl WidgetProperties for GridTableWidget {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "has_data_source" => Ok(CapabilityValue::Bool(self.data_source.is_some())),
            "row_count" => Ok(CapabilityValue::UInt(self.row_count() as u64)),
            "column_count" => Ok(CapabilityValue::UInt(self.column_count() as u64)),
            "scroll_row" => Ok(CapabilityValue::UInt(self.scroll_row() as u64)),
            "scroll_column" => Ok(CapabilityValue::UInt(self.scroll_column() as u64)),
            "row_height" => Ok(CapabilityValue::UInt(self.row_height as u64)),
            "selection_mode" => Ok(CapabilityValue::String(
                grid_table_selection_mode_to_str(self.selection_mode).to_string(),
            )),
            "sort_spec_count" => Ok(CapabilityValue::UInt(self.sort_specs().len() as u64)),
            "selected_cell" => match self.selected_cell() {
                Some((row, column)) => Ok(CapabilityValue::String(format!("{row},{column}"))),
                None => Ok(CapabilityValue::Null),
            },
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "scroll_row" => {
                self.set_scroll_row(expect_usize(value)?);
                Ok(())
            }
            "scroll_column" => {
                self.set_scroll_column(expect_usize(value)?);
                Ok(())
            }
            "row_height" => match value {
                CapabilityValue::UInt(height) => {
                    let height =
                        u32::try_from(height).map_err(|_| CapabilityAccessError::TypeMismatch)?;
                    self.set_row_height(height);
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "selection_mode" => {
                self.set_selection_mode(expect_grid_table_selection_mode(value)?);
                Ok(())
            }
            // Counts and the selected cell are derived from the data source and
            // the live selection; there is no meaningful assignment for them.
            "has_data_source" | "row_count" | "column_count" | "sort_spec_count"
            | "selected_cell" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "has_data_source",
            "row_count",
            "column_count",
            "scroll_row",
            "scroll_column",
            "row_height",
            "selection_mode",
            "sort_spec_count",
            "selected_cell",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `grid_table` publishes.
    ///
    /// `clear_data_source` and `clear_selection` discard state and need no argument,
    /// so a bare invocation performs them — both are already no-ops on an unbound or
    /// unselected table, which is the correct reading of "clear" either way.
    /// `toggle_sort_column` names which column to sort and is therefore
    /// [`CapabilityAccessError::OutOfRange`]: the name is valid and the column index
    /// is what is missing. `set_data_source`, `set_scroll_row` and `set_scroll_column`
    /// carry their own payloads for the same reason.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "clear_data_source" => {
                self.clear_data_source();
                Ok(())
            }
            "clear_selection" => {
                self.clear_selection();
                Ok(())
            }
            "toggle_sort_column" | "set_data_source" | "set_scroll_row" | "set_scroll_column" => {
                Err(CapabilityAccessError::OutOfRange)
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

/// Publishes `GridTableSelectionMode` as the shared lower-case token.
fn grid_table_selection_mode_to_str(mode: GridTableSelectionMode) -> &'static str {
    match mode {
        GridTableSelectionMode::None => "none",
        GridTableSelectionMode::Cell => "cell",
        GridTableSelectionMode::Row => "row",
        GridTableSelectionMode::Column => "column",
    }
}

/// Parses the shared lower-case token back, rejecting anything else.
fn expect_grid_table_selection_mode(
    value: CapabilityValue,
) -> Result<GridTableSelectionMode, CapabilityAccessError> {
    match value {
        CapabilityValue::String(token) => match token.as_str() {
            "none" => Ok(GridTableSelectionMode::None),
            "cell" => Ok(GridTableSelectionMode::Cell),
            "row" => Ok(GridTableSelectionMode::Row),
            "column" => Ok(GridTableSelectionMode::Column),
            _ => Err(CapabilityAccessError::TypeMismatch),
        },
        _ => Err(CapabilityAccessError::TypeMismatch),
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

        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then a literal. The theme step is what makes an appearance
        // switch visible; the surface, the header, the row numbers, the grid lines and all
        // four text colours used to be hardcoded literals, so light and dark rendered
        // identically.
        //
        // The theme reads take and release the global manager's lock internally, so no guard
        // is held across the draw (the mutex is not re-entrant).
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("grid_table");
        let mut surface = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        // `grid_table` is absent from `WidgetRole::for_kind_name`'s table, so it classifies as
        // `Surface` and resolves to `theme.colors.background` — the window's own fill. A panel
        // painted in that colour would be byte-identical to the frame behind it, so a resolved
        // surface equal to the window fill is re-derived a visible step away from it, the same
        // distinction `Colors::input_background` draws for a field.
        let window_fill = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.background)
            .unwrap_or(Color::WHITE);
        if surface == window_fill {
            surface = window_fill.blend(&ink, 0.08);
        }
        // The accent is the theme's `primary`, used both for the selection fill and for the
        // resting header text; the surface and the ink supply everything else.
        let accent = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.primary)
            .unwrap_or(Color::PRIMARY);
        // Each cached appearance field is used only while it still holds its constructor
        // literal, i.e. while no caller set it explicitly. The explicit setters keep their
        // meaning; only the untouched default now follows the appearance.
        let grid_color = if self.grid_color == Self::DEFAULT_GRID_COLOR {
            surface.blend(&ink, 0.15)
        } else {
            self.grid_color
        };
        let header_bg = if self.header_bg == Self::DEFAULT_HEADER_BG {
            surface.blend(&ink, 0.06)
        } else {
            self.header_bg
        };
        let header_text_color = if self.header_text_color == Self::DEFAULT_HEADER_TEXT_COLOR {
            ink.blend(&accent, 0.60)
        } else {
            self.header_text_color
        };
        let selected_bg = if self.selected_bg == Self::DEFAULT_SELECTED_BG {
            surface.blend(&accent, 0.30)
        } else {
            self.selected_bg
        };
        // A cell's own text is the ink, not a second literal: it used to be a fixed
        // blue-black that vanished on a dark surface.
        let cell_text_color = ink;
        let outer_border = surface.blend(&ink, 0.35);

        // Background
        context.fill_rect(rect, surface);

        let rnw = self.row_number_width as i32;
        let header_h = self.header_height as i32;
        let rh = self.row_height.max(1) as i32;
        let cols = self.column_count();
        let rows = self.row_count();

        self.ensure_column_widths(cols);
        self.update_visibility();

        if rows == 0 || cols == 0 {
            // Draw at least the header / row-number area frame
            context.draw_rect(rect, grid_color);
            return;
        }

        let data_top = rect.y + header_h;
        let data_left = rect.x + rnw;

        // ── Draw column headers ──
        {
            let header_rect = Rect::new(rect.x, rect.y, rect.width, self.header_height);
            context.fill_rect(header_rect, header_bg);
            context.draw_rect(header_rect, grid_color);

            let mut hx = data_left;
            for (ci, _, cw) in self.visible_column_spans(rect.x + rect.width as i32 - data_left) {
                let cell_rect = Rect::new(hx, rect.y, cw as u32, self.header_height);
                context.draw_rect(cell_rect, grid_color);

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

                // The header names a *data* column, so the label is derived from the column's
                // real index in the source — the same index the cells below are fetched with —
                // and not from the loop counter, which under a horizontal scroll would name a
                // different column than the one it sits above.
                let header_text = format!("Column {}{label}", ci + 1);
                context.draw_text_fitted(
                    context.text_line(cell_rect, &Font::default()),
                    &header_text,
                    &Font::default(),
                    header_text_color,
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
                context.fill_rect(cell_rect, header_bg);
                context.draw_rect(cell_rect, grid_color);

                // The row-number cell is the band; centring by hand drew the digits half a line
                // low. Right alignment keeps the numbers flush with the gutter's inner edge.
                context.draw_text_fitted(
                    context.text_line(cell_rect, &Font::default()),
                    &abs_row.to_string(),
                    &Font::default(),
                    header_text_color,
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
            for (ci, _, cw) in self.visible_column_spans(rect.x + rect.width as i32 - data_left) {
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
                    context.fill_rect(cell_rect, selected_bg);
                }

                // Grid lines
                context.draw_rect(cell_rect, grid_color);

                // Cell text
                if let Some(text) = source.data(abs_row, ci) {
                    // The cell is the band. `cy + rh / 2` as a `draw_text` origin put the glyph
                    // box's *top* edge on the cell's middle line, drawing every value half a line
                    // low; `text_line` derives the centred box, and fitting keeps a long value
                    // inside its column instead of running under the next one.
                    context.draw_text_fitted(
                        context.text_line(cell_rect, &Font::default()),
                        &text,
                        &Font::default(),
                        cell_text_color,
                        HorizontalAlignment::Left,
                    );
                }

                cx += cw;
            }
            cy += rh;
        }

        // ── Outer border ──
        context.draw_rect(rect, outer_border);
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

            Event::MouseRelease { pos: _, button: _ } if self.resizing_column.is_some() => {
                self.resizing_column = None;
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

    /// The `§B.9` judgement for this control: widening a column must move everything that follows it.
    ///
    /// # Why this is the criterion and not "the widths are stored"
    ///
    /// The defect class this replaces is *five* independent walks of the same widths — the header
    /// painter, the cell painter, the pointer hit test, the header hit test and the resize-handle hit
    /// test. A control whose paints and whose hit tests each accumulate their own x agrees with itself
    /// until one of them is edited, and the failure appears as "the resize handle is next to the line"
    /// rather than as a bad number. Asserting that a width change *propagates to every consumer* is what
    /// pins them to one derivation.
    #[test]
    fn widening_a_column_moves_every_follower() {
        fn build(first: u32) -> GridTableWidget {
            let mut tbl = GridTableWidget::new(Rect::new(0, 0, 600, 400));
            tbl.set_data_source(Arc::new(TestSource { rows: 5, cols: 5 }));
            tbl.ensure_column_widths(5);
            tbl.set_column_width(0, first);
            tbl.set_column_width(1, 80);
            tbl.update_visibility();
            tbl
        }

        let narrow = build(100);
        let wide = build(180);
        let rnw = narrow.row_number_width as i32;
        let header_y = 5;

        // The column the pointer lands in. Column 0 is 100 px wide, so 110 px in is already inside
        // column 1; growing column 0 to 180 moves the boundary past the pointer, so it lands on a
        // different *index* at the same pixel.
        //
        // # Why the index changes rather than the position
        //
        // The columns do not reflow — each keeps its own width — so widening the first one shifts every
        // follower rightward, and a fixed pixel falls into an earlier column. Asserting on both the
        // header and the cell below it is what shows the shift is shared.
        assert_eq!(narrow.header_at_point(Point::new(rnw + 110, header_y)), Some(1));
        assert_eq!(
            wide.header_at_point(Point::new(rnw + 110, header_y)),
            Some(0),
            "a wider first column must take over the pixel the follower used to own"
        );
        assert_eq!(
            wide.header_at_point(Point::new(rnw + 180, header_y)),
            Some(1),
            "and the follower's header begins at the first column's new right edge"
        );

        // The resize handle follows the same edge.
        assert_eq!(narrow.resize_handle_at_point(Point::new(rnw + 100, header_y)), Some(0));
        assert_eq!(
            wide.resize_handle_at_point(Point::new(rnw + 100, header_y)),
            None,
            "the first column's right edge moved away from x = 100"
        );
        assert_eq!(
            wide.resize_handle_at_point(Point::new(rnw + 180, header_y)),
            Some(0),
            "and the handle is now at the new edge"
        );

        // The data cells move with their headers, which is what "one derivation" has to mean.
        let row_y = narrow.header_height as i32 + 5;
        assert_eq!(narrow.cell_at_point(Point::new(rnw + 110, row_y)), Some((0, 1)));
        assert_eq!(wide.cell_at_point(Point::new(rnw + 110, row_y)), Some((0, 0)));
    }

    /// A pointer inside the data area's first pixels must hit the **first** column, not `None`.
    ///
    /// This is the case that the fit walk and the hit walk genuinely answer differently, and it was a
    /// real regression when the hit test was made to reuse the fit walk: the fit walk stops at the first
    /// column whose *end* passes the extent, so handing it the pointer's own small offset dropped the
    /// column the pointer was standing in.
    #[test]
    fn a_pointer_near_the_data_edges_hits_the_first_column_and_row() {
        let mut tbl = GridTableWidget::new(Rect::new(0, 0, 600, 400));
        tbl.set_data_source(Arc::new(TestSource { rows: 5, cols: 5 }));
        tbl.ensure_column_widths(5);
        tbl.update_visibility();

        let rnw = tbl.row_number_width as i32;
        let header_h = tbl.header_height as i32;
        // The very first pixel of the data area, and the very first column's last pixel.
        assert_eq!(tbl.cell_at_point(Point::new(rnw, header_h)), Some((0, 0)));
        assert_eq!(tbl.cell_at_point(Point::new(rnw + 119, header_h)), Some((0, 0)));
        assert_eq!(tbl.cell_at_point(Point::new(rnw + 120, header_h)), Some((0, 1)));
        // And the gutter and header themselves are not cells.
        assert_eq!(tbl.cell_at_point(Point::new(rnw - 1, header_h)), None);
        assert_eq!(tbl.cell_at_point(Point::new(rnw, header_h - 1)), None);
    }

    /// A column that only *partly* fits is still drawn and still answers — otherwise growing a column
    /// could make the table's own last column unreachable.
    #[test]
    fn a_partly_visible_column_is_still_drawn_and_hit() {
        let mut tbl = GridTableWidget::new(Rect::new(0, 0, 260, 400));
        tbl.set_data_source(Arc::new(TestSource { rows: 5, cols: 5 }));
        tbl.ensure_column_widths(5);
        for i in 0..5 {
            tbl.set_column_width(i, 100);
        }
        tbl.update_visibility();

        // 260 - 48 = 212 px of data area: two whole 100 px columns and 12 px of the third.
        let rnw = tbl.row_number_width as i32;
        let spans = tbl.visible_column_spans(212);
        assert_eq!(spans.len(), 3, "the third column starts on screen: {spans:?}");
        assert_eq!(spans[2], (2, 200, 100));
        assert_eq!(tbl.cell_at_point(Point::new(rnw + 205, 30)), Some((0, 2)));
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
        tbl.column_widths.fill(120);
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
