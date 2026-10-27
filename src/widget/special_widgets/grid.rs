// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Grid widget — a visual container that arranges children in a
//! fixed row/column matrix with optional spacing, grid-line rendering,
//! and per-cell hover/click detection.
//!
//! # JSON Example
//! ```json
//! {
//!     "grid": {
//!         "rows": 3,
//!         "columns": 4,
//!         "spacing": 4,
//!         "line_color": "#DCDCDC",
//!         "children": [
//!             ...children placed via layout row/col attrs...
//!         ]
//!     }
//! }
//! ```

use crate::core::{Color, HorizontalAlignment};
use crate::core::{Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::expect_u32;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// The height reserved for the column-heading band, when headings are present.
const GRID_HEADER_HEIGHT: u32 = 22;

/// The width reserved for the row-heading column, when row headings are present.
const GRID_ROW_HEADER_WIDTH: u32 = 60;

/// The frame tables derived from the model and the viewport (SP-114).
///
/// They are a pure function of the span list, the selection, the column widths, the page and the
/// geometry, so a frame that changes none of them reuses the previous tables instead of allocating
/// three new ones.
struct GridFrameTables {
    /// The model revision the tables were built from.
    revision: u64,
    /// The geometry the tables were built for.
    geometry: Rect,
    /// Cell spans keyed by `(row, column)`.
    spans: crate::compat::HashMap<(u32, u32), (u32, u32)>,
    /// Selected cells as a lookup set.
    selected: crate::compat::HashMap<(u32, u32), ()>,
    /// Every column's offset from the body's left edge.
    column_offsets: Vec<i32>,
}

/// Grid widget for layout management.
///
/// Displays a visual grid with configurable rows, columns, spacing,
/// and grid-line rendering. Children can be assigned to cells via
/// the parent's layout manager (typically a `GridLayout`), or
/// positioned absolutely within the grid area.
///
/// The control also owns the text each cell shows: use
/// [`GridWidget::set_cell_text`] for a cell and [`GridWidget::set_headers`] for the
/// heading band. A grid used purely as a layout guide simply leaves both empty.
pub struct GridWidget {
    base: BaseWidget,
    /// Number of rows in the grid (minimum 1).
    rows: u32,
    /// Number of columns in the grid (minimum 1).
    columns: u32,
    /// Spacing between cells in pixels.
    spacing: u32,
    /// Color of grid separator lines (`None` = no lines drawn).
    ///
    /// A caller that never sets this gets the **themed** separator, derived from the resolved border
    /// so it moves with the appearance. That used to be expressed as `Some(Color::rgb(220,220,220))`
    /// plus a sentinel comparison in the draw (`if line_color == that literal`), which had two
    /// defects: the value `#DCDCDC` was **impossible to ask for** — a caller who wanted exactly that
    /// grey silently got the themed border — and the getter reported `Some(…)` for a control whose
    /// caller had set nothing, so the property round-trip could not tell "unset" from "set to the
    /// default". `None` here means "follow the theme", and `Explicit` means the caller chose.
    line_color: Option<Option<Color>>,
    /// Cached cell dimensions computed during the last draw pass.
    cell_width: u32,
    cell_height: u32,
    hovered_cell: Option<(u32, u32)>,
    /// The cell keyboard focus is on.
    focused_cell: Option<(u32, u32)>,
    /// Selected cells; more than one when multi-select is enabled.
    selected_cells: Vec<(u32, u32)>,
    /// Whether a click adds to the selection instead of replacing it.
    multi_select: bool,
    /// Cell spans as `(row, column, row_span, column_span)`.
    spans: Vec<(u32, u32, u32, u32)>,
    /// Per-column widths, index-aligned with the columns; `None` uses the computed share (SP-69).
    column_widths: Vec<Option<u32>>,
    /// Rows shown per page, or `None` to show every row.
    page_rows: Option<u32>,
    /// The page currently drawn when paging is enabled.
    page: u32,
    /// Per-cell text, indexed by `[row][column]`.
    cells: Vec<Vec<String>>,
    /// Optional column headings, one per column.
    headers: Vec<String>,
    /// Optional row headings, one per row. A non-empty list reserves a leading band (SP-115).
    row_headers: Vec<String>,
    /// Whether a cell edit is currently in progress (SP-115).
    editing_cell: Option<(u32, u32)>,
    /// The text being edited while [`Self::editing_cell`] is set.
    edit_buffer: String,
    /// The editor's insertion point, as a character index into `edit_buffer`.
    edit_caret: usize,
    /// Whether a double-click or Enter starts an in-place edit. Off preserves the historical
    /// read-only grid.
    editable: bool,
    /// The model revision the frame tables are invalidated with (SP-114).
    frame_revision: u64,
    /// How many times the frame tables have been rebuilt, for the cache test.
    #[cfg_attr(not(test), allow(dead_code))]
    frame_builds: u64,
    /// The cached frame tables; `None` until the first draw after an invalidation.
    frame_cache: Option<GridFrameTables>,
    /// Emitted when an in-place edit starts, with the cell.
    pub edit_requested: Signal1<(u32, u32)>,
    /// Emitted with `(row, column, text)` when an edit is committed.
    pub cell_edited: Signal1<(u32, u32, String)>,
    /// Emitted when a cell is clicked.
    pub cell_clicked: Signal1<(u32, u32)>,
    /// Emitted when pointer hover changes to another cell.
    pub cell_hovered: Signal1<(u32, u32)>,
    /// Emitted when a context menu is requested over a cell.
    pub context_menu_requested: Signal1<(u32, u32)>,
    /// Emitted with the new page index when paging moves.
    pub page_changed: Signal1<u32>,
    /// Emitted with the column index when the heading band is clicked (SP-69).
    pub header_clicked: Signal1<u32>,
}

impl GridWidget {
    /// Creates a new grid widget with default 1x1 layout.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Grid, geometry, "GridWidget"),
            rows: 1,
            columns: 1,
            spacing: 0,
            // `Some(None)` is "follow the theme": the caller has not chosen a separator colour,
            // and the draw derives one from the resolved border. `None` would mean "draw no lines".
            line_color: Some(None),
            cell_width: geometry.width,
            cell_height: geometry.height,
            hovered_cell: None,
            focused_cell: None,
            selected_cells: Vec::new(),
            multi_select: false,
            spans: Vec::new(),
            column_widths: Vec::new(),
            page_rows: None,
            page: 0,
            cells: Vec::new(),
            headers: Vec::new(),
            row_headers: Vec::new(),
            editing_cell: None,
            edit_buffer: String::new(),
            edit_caret: 0,
            editable: false,
            frame_revision: 0,
            frame_builds: 0,
            frame_cache: None,
            edit_requested: Signal1::new(),
            cell_edited: Signal1::new(),
            cell_clicked: Signal1::new(),
            cell_hovered: Signal1::new(),
            context_menu_requested: Signal1::new(),
            page_changed: Signal1::new(),
            header_clicked: Signal1::new(),
        }
    }

    /// Creates a new grid widget with specified dimensions.
    pub fn with_dimensions(geometry: Rect, rows: u32, columns: u32) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Grid, geometry, "GridWidget"),
            rows: rows.max(1),
            columns: columns.max(1),
            spacing: 0,
            // `Some(None)` is "follow the theme": the caller has not chosen a separator colour,
            // and the draw derives one from the resolved border. `None` would mean "draw no lines".
            line_color: Some(None),
            cell_width: geometry.width / columns.max(1),
            cell_height: geometry.height / rows.max(1),
            hovered_cell: None,
            focused_cell: None,
            selected_cells: Vec::new(),
            multi_select: false,
            spans: Vec::new(),
            column_widths: Vec::new(),
            page_rows: None,
            page: 0,
            cells: Vec::new(),
            headers: Vec::new(),
            row_headers: Vec::new(),
            editing_cell: None,
            edit_buffer: String::new(),
            edit_caret: 0,
            editable: false,
            frame_revision: 0,
            frame_builds: 0,
            frame_cache: None,
            edit_requested: Signal1::new(),
            cell_edited: Signal1::new(),
            cell_clicked: Signal1::new(),
            cell_hovered: Signal1::new(),
            context_menu_requested: Signal1::new(),
            page_changed: Signal1::new(),
            header_clicked: Signal1::new(),
        }
    }

    // ── Row / Column accessors ─────────────────────────────

    /// Returns the number of rows.
    pub fn rows(&self) -> u32 {
        self.rows
    }
    /// Sets the number of rows (minimum 1). Triggers a redraw request.
    pub fn set_rows(&mut self, rows: u32) {
        let rows = rows.max(1);
        if self.rows == rows {
            return;
        }
        self.rows = rows;
        self.sync_cells();
        self.update_cell_dimensions();
        self.invalidate_frame_cache();
        self.base.request_redraw();
    }
    /// Returns the number of columns.
    pub fn columns(&self) -> u32 {
        self.columns
    }
    /// Sets the number of columns (minimum 1). Triggers a redraw request.
    pub fn set_columns(&mut self, columns: u32) {
        let columns = columns.max(1);
        if self.columns == columns {
            return;
        }
        self.columns = columns;
        self.sync_cells();
        self.update_cell_dimensions();
        self.invalidate_frame_cache();
        self.base.request_redraw();
    }

    // ── Spacing ────────────────────────────────────────────

    /// Returns spacing between cells in pixels.
    pub fn spacing(&self) -> u32 {
        self.spacing
    }
    /// Sets spacing between cells in pixels. Triggers a redraw request.
    pub fn set_spacing(&mut self, spacing: u32) {
        if self.spacing == spacing {
            return;
        }
        self.spacing = spacing;
        self.update_cell_dimensions();
        self.invalidate_frame_cache();
        self.base.request_redraw();
    }

    // ── Grid line color ────────────────────────────────────

    /// Returns the grid line color, `None` if lines are disabled, or `Some(None)` when the caller
    /// has not chosen one and the separator follows the theme.
    ///
    /// The outer option is the enabled/disabled question and the inner is "did the caller pick a
    /// colour". Collapsing the two is what made the old default indistinguishable from an explicit
    /// `#DCDCDC`.
    pub fn line_color(&self) -> Option<Option<Color>> {
        self.line_color
    }
    /// Sets the grid line color. Pass `None` to disable grid lines, or `Some(None)` to follow the
    /// theme's border colour.
    pub fn set_line_color(&mut self, color: Option<Option<Color>>) {
        if self.line_color == color {
            return;
        }
        self.line_color = color;
        self.base.request_redraw();
    }

    // ── Cell content ───────────────────────────────────────

    /// Returns the text of `(row, column)`, or an empty string outside the grid.
    pub fn cell_text(&self, row: u32, column: u32) -> &str {
        self.cells
            .get(row as usize)
            .and_then(|cells| cells.get(column as usize))
            .map(String::as_str)
            .unwrap_or("")
    }

    /// Sets the text of `(row, column)`.
    ///
    /// The backing store is grown to the grid's dimensions on demand, so a caller can fill
    /// cells in any order. An out-of-range address is refused rather than silently stored,
    /// because a cell that the control will never paint is not a model.
    pub fn set_cell_text(&mut self, row: u32, column: u32, text: impl Into<String>) -> bool {
        if row >= self.rows || column >= self.columns {
            return false;
        }
        self.cells.resize_with(self.rows as usize, || vec![String::new(); self.columns as usize]);
        for cells in &mut self.cells {
            cells.resize_with(self.columns as usize, String::new);
        }
        self.cells[row as usize][column as usize] = text.into();
        self.base.request_redraw();
        true
    }

    // ── Cell editing (SP-115) ──────────────────────────────

    /// Whether a double-click or Enter starts an in-place edit.
    pub fn is_editable(&self) -> bool {
        self.editable
    }

    /// Enables or disables in-place cell editing.
    ///
    /// Editing is off by default, so a grid that was a read-only layout guide stays one.
    pub fn set_editable(&mut self, editable: bool) {
        if self.editable == editable {
            return;
        }
        self.editable = editable;
        if !editable {
            self.cancel_edit();
        }
        self.base.request_redraw();
    }

    /// The cell being edited, if any.
    pub fn editing_cell(&self) -> Option<(u32, u32)> {
        self.editing_cell
    }

    /// Starts editing `(row, column)`, seeding the buffer with the cell's current text.
    ///
    /// Returns `false` when editing is disabled or the address is outside the grid.
    pub fn begin_edit(&mut self, row: u32, column: u32) -> bool {
        if !self.editable || row >= self.rows || column >= self.columns {
            return false;
        }
        if self.editing_cell == Some((row, column)) {
            return true;
        }
        self.edit_buffer = self.cell_text(row, column).to_string();
        self.edit_caret = self.edit_buffer.chars().count();
        self.editing_cell = Some((row, column));
        self.focused_cell = Some((row, column));
        self.edit_requested.emit((row, column));
        self.base.request_redraw();
        true
    }

    /// Commits the text being edited into its cell and emits `cell_edited`.
    ///
    /// Returns `false` when no edit is active.
    pub fn commit_edit(&mut self) -> bool {
        let Some((row, column)) = self.editing_cell.take() else {
            return false;
        };
        let text = core::mem::take(&mut self.edit_buffer);
        self.edit_caret = 0;
        if row < self.rows && column < self.columns {
            self.set_cell_text(row, column, text.clone());
            self.cell_edited.emit((row, column, text));
        }
        self.base.request_redraw();
        true
    }

    /// Abandons the edit without writing anything.
    pub fn cancel_edit(&mut self) {
        if self.editing_cell.is_none() {
            return;
        }
        self.editing_cell = None;
        self.edit_buffer.clear();
        self.edit_caret = 0;
        self.base.request_redraw();
    }

    /// The text in the active edit buffer.
    pub fn edit_text(&self) -> &str {
        &self.edit_buffer
    }

    fn invalidate_frame_cache(&mut self) {
        self.frame_revision = self.frame_revision.wrapping_add(1);
        self.frame_cache = None;
    }

    /// Rebuilds the frame tables when the model or the geometry has moved (SP-114).
    fn ensure_frame_tables(&mut self) {
        let geometry = self.base.geometry();
        if self.frame_cache.as_ref().is_some_and(|cache| {
            cache.revision == self.frame_revision && cache.geometry == geometry
        }) {
            return;
        }
        self.update_cell_dimensions();
        let spans = self
            .spans
            .iter()
            .map(|(row, column, row_span, column_span)| {
                ((*row, *column), (*row_span, *column_span))
            })
            .collect();
        let selected = self.selected_cells.iter().map(|cell| (*cell, ())).collect();
        let column_offsets = self.column_offsets();
        self.frame_builds = self.frame_builds.wrapping_add(1);
        self.frame_cache = Some(GridFrameTables {
            revision: self.frame_revision,
            geometry,
            spans,
            selected,
            column_offsets,
        });
    }

    /// The column headings, one per column.
    pub fn headers(&self) -> &[String] {
        &self.headers
    }

    /// Replaces the column headings.
    ///
    /// Headings reserve a band above the matrix; an empty list removes that band and
    /// restores the previous cell geometry.
    pub fn set_headers(&mut self, headers: Vec<String>) {
        if self.headers == headers {
            return;
        }
        self.headers = headers;
        self.update_cell_dimensions();
        self.invalidate_frame_cache();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The row headings, one per row; empty means no heading column.
    pub fn row_headers(&self) -> &[String] {
        &self.row_headers
    }

    /// Replaces the row headings.
    ///
    /// A non-empty list reserves a leading band the same way the column headings reserve a top
    /// band, so a row's title cannot be mistaken for a cell's data. Passing an empty list restores
    /// the previous geometry, which is the historical picture (SP-115).
    pub fn set_row_headers(&mut self, headers: Vec<String>) {
        if self.row_headers == headers {
            return;
        }
        self.row_headers = headers;
        self.update_cell_dimensions();
        self.invalidate_frame_cache();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The width of the row-heading column, or zero when no row headings are set.
    pub fn row_header_width(&self) -> u32 {
        if self.row_headers.is_empty() {
            0
        } else {
            GRID_ROW_HEADER_WIDTH.min(self.base.geometry().width)
        }
    }

    fn sync_cells(&mut self) {
        let columns = self.columns as usize;
        self.cells.resize_with(self.rows as usize, || vec![String::new(); columns]);
        for cells in &mut self.cells {
            cells.resize_with(columns, String::new);
        }
    }

    /// The band the matrix itself occupies, below the heading row when one is present and right of
    /// the row-heading column when one is present.
    fn body_rect(&self) -> Rect {
        let rect = self.base.geometry();
        let header = if self.headers.is_empty() { 0 } else { GRID_HEADER_HEIGHT.min(rect.height) };
        let row_header = self.row_header_width();
        Rect::new(
            rect.x + row_header as i32,
            rect.y + header as i32,
            rect.width.saturating_sub(row_header),
            rect.height.saturating_sub(header),
        )
    }

    /// Whether `pos` lands in the column-heading band.
    fn hit_header(&self, pos: Point) -> bool {
        let rect = self.base.geometry();
        let body = self.body_rect();
        body.y > rect.y
            && pos.y >= rect.y
            && pos.y < body.y
            && pos.x >= body.x
            && pos.x < rect.x + rect.width as i32
    }

    // ── Cell info ──────────────────────────────────────────

    /// Returns the cached cell width computed during the last draw.
    pub fn cell_width(&self) -> u32 {
        self.cell_width
    }
    /// Returns the cached cell height computed during the last draw.
    pub fn cell_height(&self) -> u32 {
        self.cell_height
    }

    /// Returns the cell row for a given y-coordinate, or `None` if outside.
    fn cell_at_y(&self, y: i32) -> Option<u32> {
        let rect = self.body_rect();
        if y < rect.y || y >= rect.y + rect.height as i32 {
            return None;
        }
        if self.rows == 0 || self.cell_height == 0 {
            return None;
        }
        let local_y = (y - rect.y) as u32;
        let step = (self.cell_height + self.spacing).max(1);
        let visible_row = local_y / step;
        if visible_row < self.visible_rows() {
            Some(self.first_row() + visible_row)
        } else {
            None
        }
    }

    /// Returns the cell column for a given x-coordinate, or `None` if outside.
    fn cell_at_x(&self, x: i32) -> Option<u32> {
        let rect = self.body_rect();
        if x < rect.x || x >= rect.x + rect.width as i32 {
            return None;
        }
        if self.columns == 0 || self.cell_width == 0 {
            return None;
        }
        // Per-column widths mean the column is no longer `local_x / step`; the walk is over the
        // column count, not over the cells, so it stays bounded by the grid's shape.
        let mut left = rect.x;
        for column in 0..self.columns {
            let width = self.column_width(column) as i32;
            if x >= left && x < left + width {
                return Some(column);
            }
            left += width + self.spacing as i32;
        }
        None
    }

    /// The painted width of column `column`: its configured width, or the computed share.
    pub fn column_width(&self, column: u32) -> u32 {
        self.column_widths.get(column as usize).copied().flatten().unwrap_or(self.cell_width).max(1)
    }

    /// Sets one column's width; `None` restores the computed share (SP-69).
    pub fn set_column_width(&mut self, column: u32, width: Option<u32>) -> bool {
        if column >= self.columns {
            return false;
        }
        if self.column_widths.len() <= column as usize {
            self.column_widths.resize(column as usize + 1, None);
        }
        self.column_widths[column as usize] = width.map(|width| width.max(1));
        self.update_cell_dimensions();
        self.invalidate_frame_cache();
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    /// Replaces the whole per-column width list.
    pub fn set_column_widths(&mut self, widths: Vec<Option<u32>>) {
        if self.column_widths == widths {
            return;
        }
        self.column_widths = widths;
        self.update_cell_dimensions();
        self.invalidate_frame_cache();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The x offset of column `column` from the body's left edge, spacing included.
    fn column_offset(&self, column: u32) -> i32 {
        let mut offset = 0i32;
        for index in 0..column {
            offset += self.column_width(index) as i32 + self.spacing as i32;
        }
        offset
    }

    /// Every column's offset from the body's left edge, so a frame builds it once.
    fn column_offsets(&self) -> Vec<i32> {
        let mut offsets = Vec::with_capacity(self.columns as usize);
        let mut offset = 0i32;
        for column in 0..self.columns {
            offsets.push(offset);
            offset += self.column_width(column) as i32 + self.spacing as i32;
        }
        offsets
    }

    /// Returns the cell position `(row, col)` for a given point, or `None`.
    pub fn cell_at(&self, point: Point) -> Option<(u32, u32)> {
        let row = self.cell_at_y(point.y)?;
        let col = self.cell_at_x(point.x)?;
        Some((row, col))
    }

    /// Returns the bounding rectangle of a specific cell, including any span.
    pub fn cell_rect(&self, row: u32, col: u32) -> Option<Rect> {
        self.cell_rect_in(row, col, None, None)
    }

    /// [`Self::cell_rect`], reading spans from a table the caller already built.
    ///
    /// The paint builds the span table once per frame instead of letting every cell run a linear
    /// scan of the whole span list, which made a spanned grid `O(cells × spans)` (SP-P74). The
    /// column offsets follow the same rule: one vector per frame rather than a cumulative walk per
    /// cell.
    fn cell_rect_in(
        &self,
        row: u32,
        col: u32,
        spans: Option<&crate::compat::HashMap<(u32, u32), (u32, u32)>>,
        offsets: Option<&[i32]>,
    ) -> Option<Rect> {
        if row >= self.rows || col >= self.columns {
            return None;
        }
        let rect = self.body_rect();
        let visible_row = row.saturating_sub(self.first_row());
        if row < self.first_row() || visible_row >= self.visible_rows() {
            return None;
        }
        let (row_span, column_span) = match spans {
            Some(spans) => spans.get(&(row, col)).copied().unwrap_or((1, 1)),
            None => self.cell_span(row, col),
        };
        let x = rect.x
            + match offsets {
                Some(offsets) => offsets.get(col as usize).copied().unwrap_or(0),
                None => self.column_offset(col),
            };
        let y = rect.y + (visible_row * (self.cell_height + self.spacing)) as i32;
        // A spanning cell covers its columns' widths plus the gaps it crosses.
        let width: u32 = (0..column_span)
            .map(|index| self.column_width(col + index))
            .sum::<u32>()
            .saturating_add(self.spacing.saturating_mul(column_span.saturating_sub(1)));
        let height = self.cell_height.saturating_mul(row_span)
            + self.spacing.saturating_mul(row_span.saturating_sub(1));
        Some(Rect::new(x, y, width.max(1), height))
    }

    /// Sets the span of a cell.
    ///
    /// A span of `(1, 1)` is an ordinary cell and removes the entry. The span is clipped to
    /// the grid so it can never address a cell that does not exist.
    pub fn set_cell_span(
        &mut self,
        row: u32,
        column: u32,
        row_span: u32,
        column_span: u32,
    ) -> bool {
        if row >= self.rows || column >= self.columns {
            return false;
        }
        let row_span = row_span.max(1).min(self.rows - row);
        let column_span = column_span.max(1).min(self.columns - column);
        self.spans.retain(|(r, c, _, _)| *r != row || *c != column);
        if row_span > 1 || column_span > 1 {
            self.spans.push((row, column, row_span, column_span));
        }
        self.invalidate_frame_cache();
        self.base.request_redraw();
        true
    }

    /// Returns the span of a cell as `(row_span, column_span)`; an ordinary cell answers `(1, 1)`.
    pub fn cell_span(&self, row: u32, column: u32) -> (u32, u32) {
        self.spans
            .iter()
            .find(|(r, c, _, _)| *r == row && *c == column)
            .map_or((1, 1), |(_, _, row_span, column_span)| (*row_span, *column_span))
    }

    /// Enables or disables multi-cell selection.
    pub fn set_multi_select(&mut self, multi: bool) {
        if self.multi_select == multi {
            return;
        }
        self.multi_select = multi;
        if !multi && self.selected_cells.len() > 1 {
            let keep = self.selected_cells[0];
            self.selected_cells.clear();
            self.selected_cells.push(keep);
        }
        self.invalidate_frame_cache();
        self.base.request_redraw();
    }

    /// Returns whether multi-cell selection is enabled.
    pub fn multi_select(&self) -> bool {
        self.multi_select
    }

    /// Returns the selected cells in selection order.
    pub fn selected_cells(&self) -> &[(u32, u32)] {
        &self.selected_cells
    }

    /// Toggles `cell` in the selection, replacing it when multi-select is off.
    pub fn select_cell(&mut self, cell: (u32, u32)) {
        if cell.0 >= self.rows || cell.1 >= self.columns {
            return;
        }
        if self.multi_select {
            if let Some(index) = self.selected_cells.iter().position(|selected| *selected == cell) {
                self.selected_cells.remove(index);
            } else {
                self.selected_cells.push(cell);
            }
        } else {
            self.selected_cells.clear();
            self.selected_cells.push(cell);
        }
        self.focused_cell = Some(cell);
        // The same page follow-through as the arrow keys: a selection that lands on another page
        // must take the page with it (SP-68).
        if let Some(page_rows) = self.page_rows {
            if let Some(page) = cell.0.checked_div(page_rows) {
                if page != self.page {
                    self.set_page(page);
                }
            }
        }
        self.invalidate_frame_cache();
        self.base.request_redraw();
    }

    /// Clears the selection.
    pub fn clear_selection(&mut self) {
        if self.selected_cells.is_empty() {
            return;
        }
        self.selected_cells.clear();
        self.invalidate_frame_cache();
        self.base.request_redraw();
    }

    /// Enables paging with `rows_per_page` rows per page.
    pub fn set_page_rows(&mut self, rows_per_page: Option<u32>) {
        let rows_per_page = rows_per_page.map(|rows| rows.max(1));
        if self.page_rows == rows_per_page {
            return;
        }
        self.page_rows = rows_per_page;
        self.page = 0;
        self.update_cell_dimensions();
        self.invalidate_frame_cache();
        self.base.request_redraw();
    }

    /// Returns the rows shown per page, if paging is enabled.
    pub fn page_rows(&self) -> Option<u32> {
        self.page_rows
    }

    /// Returns the current page index.
    pub fn page(&self) -> u32 {
        self.page
    }

    /// Sets the current page, clamped to the available pages.
    pub fn set_page(&mut self, page: u32) {
        let pages = self.page_count();
        let page = page.min(pages.saturating_sub(1));
        if self.page == page {
            return;
        }
        self.page = page;
        self.update_cell_dimensions();
        self.invalidate_frame_cache();
        self.page_changed.emit(page);
        self.base.request_redraw();
    }

    /// Returns how many pages the grid has.
    pub fn page_count(&self) -> u32 {
        match self.page_rows {
            Some(rows) if rows > 0 => self.rows.div_ceil(rows).max(1),
            _ => 1,
        }
    }

    /// The first row drawn on the current page.
    fn first_row(&self) -> u32 {
        self.page.saturating_mul(self.page_rows.unwrap_or(0))
    }

    /// The number of rows drawn on the current page.
    fn visible_rows(&self) -> u32 {
        match self.page_rows {
            Some(rows) => rows.min(self.rows.saturating_sub(self.first_row())),
            None => self.rows,
        }
    }

    // ── Recalculate cell dimensions ────────────────────────
    fn update_cell_dimensions(&mut self) {
        let rect = self.body_rect();
        let cols = self.columns.max(1);
        // The matrix is divided over the rows **drawn on this page**, not over every row in
        // the model. Dividing by `rows` made a paged grid occupy only its first `page_rows`
        // rows' worth of the control, leaving the rest of the rectangle empty.
        let rows = self.visible_rows().max(1);
        let total_spacing_h = self.spacing.saturating_mul(rows.saturating_sub(1));
        self.cell_height = (rect.height.saturating_sub(total_spacing_h)) / rows;
        // Columns with an explicit width take it out of the pool first; the remaining ones share
        // what is left, so setting one column never squeezes the grid past its own width (SP-69).
        let mut fixed = 0u32;
        let mut auto = 0u32;
        for column in 0..cols {
            match self.column_widths.get(column as usize).copied().flatten() {
                Some(width) => fixed = fixed.saturating_add(width.max(1)),
                None => auto += 1,
            }
        }
        let total_spacing_w = self.spacing.saturating_mul(cols.saturating_sub(1));
        let free = rect.width.saturating_sub(total_spacing_w).saturating_sub(fixed);
        let divisor = if auto == 0 { cols } else { auto };
        self.cell_width = free.checked_div(divisor).unwrap_or(0).max(1);
    }
}

// ── Widget trait ──────────────────────────────────────────

impl Widget for GridWidget {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn set_geometry(&mut self, geometry: Rect) {
        self.base.set_geometry(geometry);
        self.update_cell_dimensions();
        self.invalidate_frame_cache();
    }

    /// Returns a size hint proportional to rows x columns.
    fn size_hint(&self) -> Size {
        // Each cell at least one `GRID_CELL_SIZE` square, times row/col count, plus spacing.
        let cell = crate::widget::metrics::dimensions::GRID_CELL_SIZE;
        let floor = crate::widget::metrics::dimensions::GRID_DEFAULT_FLOOR;
        let w = self.columns * cell + self.spacing.saturating_mul(self.columns.saturating_sub(1));
        let h = self.rows * cell + self.spacing.saturating_mul(self.rows.saturating_sub(1));
        Size::new(w.max(floor), h.max(floor))
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually **emits**. Without an
    /// arm a published name is valid and inert, which is the silent failure
    /// `tools/check_event_signal_dyn.sh` exists to make impossible: the arm set is compared against
    /// the capability's published names, so the two cannot drift.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "cell_clicked" => {
                Some(EventSignalRef::mapped("cell_clicked", &self.cell_clicked, |v| {
                    CapabilityValue::Tuple(alloc::vec![
                        CapabilityValue::UInt(v.0 as u64),
                        CapabilityValue::UInt(v.1 as u64),
                    ])
                }))
            }
            "cell_hovered" => {
                Some(EventSignalRef::mapped("cell_hovered", &self.cell_hovered, |v| {
                    CapabilityValue::Tuple(alloc::vec![
                        CapabilityValue::UInt(v.0 as u64),
                        CapabilityValue::UInt(v.1 as u64),
                    ])
                }))
            }
            "context_menu_requested" => Some(EventSignalRef::mapped(
                "context_menu_requested",
                &self.context_menu_requested,
                |v| {
                    crate::widget::capability::CapabilityValue::Tuple(crate::compat::Vec::from([
                        crate::widget::capability::CapabilityValue::Int(v.0 as i64),
                        crate::widget::capability::CapabilityValue::Int(v.1 as i64),
                    ]))
                },
            )),
            "page_changed" => {
                Some(EventSignalRef::mapped("page_changed", &self.page_changed, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            "header_clicked" => {
                Some(EventSignalRef::mapped("header_clicked", &self.header_clicked, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            _ => None,
        }
    }
}

/// `GridWidget`'s property contract, published under the `Grid` kind.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, including
/// the `Null` handling of `line_color` and the `TypeMismatch` an unparsable hex
/// colour produced.
impl WidgetProperties for GridWidget {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "rows" => Ok(CapabilityValue::UInt(self.rows() as u64)),
            "columns" => Ok(CapabilityValue::UInt(self.columns() as u64)),
            "spacing" => Ok(CapabilityValue::UInt(self.spacing() as u64)),
            "line_color" => Ok(match self.line_color() {
                // No lines at all: the property reports null, which is the documented spelling for
                // "disabled".
                None => CapabilityValue::Null,
                // "Follow the theme": there is no caller-chosen colour to report, so this is null
                // as well. Reporting a concrete hex here was the old defect -- it made "unset"
                // read back as a specific colour a caller could then fail to reproduce.
                Some(None) => CapabilityValue::Null,
                Some(Some(color)) => CapabilityValue::String(color.to_hex_rgba()),
            }),
            "cell_width" => Ok(CapabilityValue::UInt(self.cell_width() as u64)),
            "cell_height" => Ok(CapabilityValue::UInt(self.cell_height() as u64)),
            "selected_cell" => Ok(match self.selected_cells.first().copied() {
                Some((row, column)) => CapabilityValue::UInt(
                    u64::from(row) * u64::from(self.columns.max(1)) + u64::from(column),
                ),
                None => CapabilityValue::Null,
            }),
            "selected_count" => Ok(CapabilityValue::UInt(self.selected_cells.len() as u64)),
            "page" => Ok(CapabilityValue::UInt(u64::from(self.page))),
            "page_count" => Ok(CapabilityValue::UInt(u64::from(self.page_count()))),
            "headers" => Ok(CapabilityValue::Tuple(
                self.headers.iter().cloned().map(CapabilityValue::String).collect(),
            )),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "rows" => {
                self.set_rows(expect_u32(value)?);
                Ok(())
            }
            "columns" => {
                self.set_columns(expect_u32(value)?);
                Ok(())
            }
            "spacing" => {
                self.set_spacing(expect_u32(value)?);
                Ok(())
            }
            "line_color" => {
                match value {
                    CapabilityValue::Null => self.set_line_color(None),
                    CapabilityValue::String(raw) => {
                        let Some(color) = crate::core::Color::parse_hex(&raw) else {
                            return Err(CapabilityAccessError::TypeMismatch);
                        };
                        self.set_line_color(Some(Some(color)));
                    }
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                }
                Ok(())
            }
            // Cell extents are recomputed from the geometry and the row/column
            // counts, so they are derived reads rather than assignments.
            "cell_width" | "cell_height" | "selected_count" => {
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
            "selected_cell" => {
                match value {
                    CapabilityValue::Null => self.clear_selection(),
                    CapabilityValue::UInt(index) => {
                        let columns = self.columns.max(1);
                        let row = (index / u64::from(columns)) as u32;
                        let column = (index % u64::from(columns)) as u32;
                        if row >= self.rows || column >= self.columns {
                            return Err(CapabilityAccessError::OutOfRange);
                        }
                        self.select_cell((row, column));
                    }
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                }
                Ok(())
            }
            "page" => {
                let CapabilityValue::UInt(page) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                let page = u32::try_from(page).map_err(|_| CapabilityAccessError::OutOfRange)?;
                if page >= self.page_count() {
                    return Err(CapabilityAccessError::OutOfRange);
                }
                self.set_page(page);
                Ok(())
            }
            "headers" => {
                let CapabilityValue::Tuple(items) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                let mut headers = Vec::with_capacity(items.len());
                for item in items {
                    let CapabilityValue::String(text) = item else {
                        return Err(CapabilityAccessError::TypeMismatch);
                    };
                    headers.push(text);
                }
                self.set_headers(headers);
                Ok(())
            }
            "page_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "rows",
            "columns",
            "spacing",
            "line_color",
            "cell_width",
            "cell_height",
            "selected_cell",
            "selected_count",
            "page",
            "page_count",
            "headers",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `grid` publishes.
    ///
    /// The grid addresses cells by *hover*, not by a stored selection, so
    /// `select_cell` and `clear_selection` are names this control publishes through
    /// the shared `GridLayout` schema but cannot itself perform; they are reported as
    /// [`CapabilityAccessError::UnsupportedOnWidget`]. Note that the dispatcher
    /// already folds the trait default's `UnknownCommand` into exactly that error
    /// before the caller sees it, so the two spellings differ only in intent here.
    ///
    /// # Why the `set_*` arm
    ///
    /// `set_rows` / `set_columns` / `set_spacing` / `set_line_color` are published by
    /// this capability and are payload-carrying writes. An earlier revision answered
    /// them with `UnknownCommand`, which was a bug rather than a policy: the trait
    /// default deliberately accepts a bare `set_foo` as "needs its value", and a
    /// hand-written override that named only the non-`set_` commands silently
    /// contradicted it. `set_spacing` in particular slips past the default's
    /// `is_ascii_lowercase` prefix test, so `set_` names are matched here as a class
    /// instead of being enumerated one by one.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "clear_selection" => {
                self.clear_selection();
                Ok(())
            }
            // `select_cell` needs the cell to select, which a bare command cannot carry; the
            // `selected_cell` property is the value-carrying spelling.
            "select_cell" => Err(CapabilityAccessError::OutOfRange),
            _ if name.starts_with("set_") => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

// ── Draw ──────────────────────────────────────────────────

impl Draw for GridWidget {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        self.ensure_frame_tables();
        let body = self.body_rect();

        // Chrome colours resolve the explicit style first, then the theme's resolved style for
        // this control, and only then a literal. Every colour below used to be a literal, so a
        // light/dark switch left the matrix and its separators unchanged — the rendering census
        // reported the control as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's mutex
        // is not re-entrant.
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("grid");
        // Read as its own lock acquisition and copied out as values, so the guard is dropped
        // before anything else touches the theme.
        let (window_fill, foreground, secondary) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => {
                    (active.colors.background, active.colors.foreground, active.colors.secondary)
                }
                None => (Color::rgb(240, 240, 240), Color::BLACK, Color::rgb(158, 158, 158)),
            }
        };

        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(foreground);
        // The matrix: one step from the window fill toward the text colour, so it is a distinct
        // element on a light theme and on a dark one. `grid` is absent from
        // `WidgetRole::for_kind_name`'s table, so it classifies as `Surface` and the active theme
        // has already written the window fill into `style.background_color`; the resolved value is
        // filtered so a byte-identical-to-the-frame fill cannot be let through unfiltered, while a
        // colour the caller set still wins.
        let surface = window_fill.blend(&ink, 0.08);
        let cell_fill = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => surface,
        };
        context.face_with_gradient_corners(
            rect,
            cell_fill,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().resolved_corner_radii().to_array(),
            Color::BLACK,
        );

        // Border
        let border_color = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != cell_fill)
            .unwrap_or_else(|| cell_fill.blend(&secondary, 0.45));
        crate::widget::draw::stroke_surface_border(
            context,
            rect,
            self.style().resolved_corner_radii().to_array(),
            border_color,
            1,
        );

        // Selection and hover are chrome states, so they are tints of the resolved ink rather
        // than colours of their own. They are painted before the separators, so a line stays
        // visible over a filled cell.
        let selection_fill = ink.blend(&cell_fill, 0.72);
        let hover_fill = ink.blend(&cell_fill, 0.88);
        let cell_font = crate::widget::metrics::role_font(self.style(), 12.0);
        // The frame tables come from the cache: a still frame reuses the span table, the selection
        // set and the column offsets instead of rebuilding all three (SP-114).
        let tables =
            self.frame_cache.as_ref().expect("ensure_frame_tables() populates the frame cache");
        for row in self.first_row()..self.first_row() + self.visible_rows() {
            for column in 0..self.columns {
                let Some(cell) = self.cell_rect_in(
                    row,
                    column,
                    Some(&tables.spans),
                    Some(&tables.column_offsets),
                ) else {
                    continue;
                };
                if tables.selected.contains_key(&(row, column)) {
                    context.fill_rect(cell, selection_fill);
                } else if self.hovered_cell == Some((row, column)) {
                    context.fill_rect(cell, hover_fill);
                }
                let editing = self.editing_cell == Some((row, column));
                let text =
                    if editing { self.edit_buffer.as_str() } else { self.cell_text(row, column) };
                if !text.is_empty() {
                    let line = context.text_line(cell, &cell_font);
                    let inset = 3u32.min(cell.width / 2);
                    context.draw_text_fitted_cow(
                        Rect::new(
                            cell.x + inset as i32,
                            line.y,
                            cell.width.saturating_sub(inset * 2).max(1),
                            line.height,
                        ),
                        text,
                        &cell_font,
                        ink,
                        HorizontalAlignment::Left,
                    );
                }
                if editing {
                    // The caret follows the buffer, so a mid-string edit is visible rather than
                    // only committed on Enter.
                    let prefix: String = self.edit_buffer.chars().take(self.edit_caret).collect();
                    let caret_x =
                        cell.x + 3 + context.measure_text(&prefix, &cell_font).width as i32;
                    context.fill_rect(
                        Rect::new(
                            caret_x.min(cell.x + cell.width as i32 - 1),
                            cell.y + 3,
                            1,
                            cell.height.saturating_sub(6).max(1),
                        ),
                        ink,
                    );
                }
            }
        }

        // ── Column headings ──
        //
        // Headings reserve their own band above the matrix, so a heading cannot be mistaken
        // for row 0's data and the cell geometry stays a single derivation (`body_rect`).
        if !self.headers.is_empty() && body.y > rect.y {
            let band = Rect::new(body.x, rect.y, body.width, (body.y - rect.y) as u32);
            context.fill_rect(band, ink.blend(&cell_fill, 0.9));
            // The corner cell above the row headings belongs to neither band; painting it with the
            // heading tint keeps the two bands from looking like one continuous strip.
            if self.row_header_width() > 0 {
                context.fill_rect(
                    Rect::new(rect.x, rect.y, self.row_header_width(), (body.y - rect.y) as u32),
                    ink.blend(&cell_fill, 0.82),
                );
            }
            let font = crate::widget::metrics::role_font(self.style(), 11.0);
            for column in 0..self.columns {
                let Some(title) = self.headers.get(column as usize) else {
                    continue;
                };
                let x = body.x + self.column_offset(column);
                let width = self.column_width(column);
                let line = context.text_line(Rect::new(x, band.y, width, band.height), &font);
                context.draw_text_fitted_cow(
                    Rect::new(x + 3, line.y, width.saturating_sub(6).max(1), line.height),
                    title,
                    &font,
                    ink,
                    HorizontalAlignment::Left,
                );
            }
            context.draw_line(
                Point::new(band.x, band.y + band.height as i32),
                Point::new(band.x + band.width as i32, band.y + band.height as i32),
                border_color,
            );
        }

        // ── Row headings (SP-115) ──
        // A non-empty list reserves a leading column, so a row's title cannot be mistaken for the
        // first cell's data. The band shares the body's vertical derivation, so a page that shows
        // rows 4..8 labels those rows and not rows 0..4.
        let row_header_width = self.row_header_width();
        if row_header_width > 0 && !self.row_headers.is_empty() {
            let band_x = rect.x;
            context.fill_rect(
                Rect::new(band_x, body.y, row_header_width, body.height),
                ink.blend(&cell_fill, 0.9),
            );
            let font = crate::widget::metrics::role_font(self.style(), 11.0);
            let rows = self.visible_rows();
            for visible in 0..rows {
                let row = self.first_row() + visible;
                let Some(title) = self.row_headers.get(row as usize) else {
                    continue;
                };
                let y = body.y + (visible * (self.cell_height + self.spacing)) as i32;
                if y >= body.y + body.height as i32 {
                    break;
                }
                let height = self.cell_height.min((body.y + body.height as i32 - y).max(0) as u32);
                let line = context.text_line(
                    Rect::new(band_x + 3, y, row_header_width.saturating_sub(6), height),
                    &font,
                );
                context.draw_text_fitted_cow(
                    Rect::new(
                        band_x + 3,
                        line.y,
                        row_header_width.saturating_sub(6).max(1),
                        line.height,
                    ),
                    title,
                    &font,
                    ink,
                    HorizontalAlignment::Left,
                );
            }
            context.draw_line(
                Point::new(body.x, body.y),
                Point::new(body.x, body.y + body.height as i32),
                border_color,
            );
        }
        if !self.selected_cells.is_empty() {
            // A focused cell keeps a visible ring after the pointer leaves, so keyboard
            // navigation has something to show for itself.
            if let Some(cell) = self.focused_cell.and_then(|(r, c)| self.cell_rect(r, c)) {
                context.draw_rect(cell, ink);
            }
        }

        // Grid lines (skip for 1×1, also skip if color is None). A caller-set line colour still
        // wins; `Some(None)` means the caller chose nothing, so the separators are derived from the
        // themed border and move with it. There is no sentinel value here: the old form compared
        // against `Color::rgb(220,220,220)`, which made that exact grey impossible to request.
        let Some(chosen) = self.line_color else {
            return;
        };
        let line_color = chosen.unwrap_or(border_color);
        if self.rows <= 1 && self.columns <= 1 {
            return;
        }

        let total_w: u32 = (0..self.columns)
            .map(|column| self.column_width(column))
            .sum::<u32>()
            .saturating_add(self.spacing.saturating_mul(self.columns.saturating_sub(1)));
        let total_h = self.visible_rows() * self.cell_height
            + self.spacing.saturating_mul(self.visible_rows().saturating_sub(1));

        // Vertical lines
        for col in 1..self.columns {
            let x = body.x + self.column_offset(col) - (self.spacing / 2) as i32;
            let x = x.max(body.x).min(body.x + total_w as i32);
            context.draw_line(
                Point::new(x, body.y),
                Point::new(x, body.y + total_h as i32),
                line_color,
            );
        }

        // Horizontal lines
        for row in 1..self.visible_rows() {
            let y = body.y + (row * (self.cell_height + self.spacing)) as i32
                - (self.spacing / 2) as i32;
            let y = y.max(body.y).min(body.y + total_h as i32);
            context.draw_line(
                Point::new(body.x, y),
                Point::new(body.x + total_w as i32, y),
                line_color,
            );
        }
    }
}

// ── EventHandler ──────────────────────────────────────────

impl EventHandler for GridWidget {
    fn handle_event(&mut self, event: &crate::event::Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        // An active in-place edit owns the keyboard until it commits or cancels (SP-115).
        if self.editing_cell.is_some() {
            match event {
                Event::TextInput { text } | Event::ImeCommit { text } => {
                    let filtered: String = text.chars().filter(|ch| !ch.is_control()).collect();
                    if filtered.is_empty() {
                        return;
                    }
                    let mut chars: Vec<char> = self.edit_buffer.chars().collect();
                    let at = self.edit_caret.min(chars.len());
                    chars.splice(at..at, filtered.chars());
                    self.edit_buffer = chars.into_iter().collect();
                    self.edit_caret = at + filtered.chars().count();
                    self.base.request_redraw();
                }
                Event::KeyPress { key, .. } => match crate::shortcut::Key::from_key_code(*key) {
                    Some(crate::shortcut::Key::Enter) => {
                        self.commit_edit();
                    }
                    Some(crate::shortcut::Key::Escape) => self.cancel_edit(),
                    Some(crate::shortcut::Key::Backspace) => {
                        if self.edit_caret > 0 {
                            let mut chars: Vec<char> = self.edit_buffer.chars().collect();
                            chars.remove(self.edit_caret - 1);
                            self.edit_caret -= 1;
                            self.edit_buffer = chars.into_iter().collect();
                            self.base.request_redraw();
                        }
                    }
                    Some(crate::shortcut::Key::Delete) => {
                        let length = self.edit_buffer.chars().count();
                        if self.edit_caret < length {
                            let mut chars: Vec<char> = self.edit_buffer.chars().collect();
                            chars.remove(self.edit_caret);
                            self.edit_buffer = chars.into_iter().collect();
                            self.base.request_redraw();
                        }
                    }
                    Some(crate::shortcut::Key::Left) => {
                        self.edit_caret = self.edit_caret.saturating_sub(1);
                        self.base.request_redraw();
                    }
                    Some(crate::shortcut::Key::Right) => {
                        self.edit_caret =
                            (self.edit_caret + 1).min(self.edit_buffer.chars().count());
                        self.base.request_redraw();
                    }
                    Some(crate::shortcut::Key::Home) => {
                        self.edit_caret = 0;
                        self.base.request_redraw();
                    }
                    Some(crate::shortcut::Key::End) => {
                        self.edit_caret = self.edit_buffer.chars().count();
                        self.base.request_redraw();
                    }
                    _ => {}
                },
                _ => {}
            }
            return;
        }

        match *event {
            Event::MouseMove { pos } => {
                if let Some(cell) = self.cell_at(pos) {
                    if self.hovered_cell != Some(cell) {
                        self.hovered_cell = Some(cell);
                        self.cell_hovered.emit(cell);
                    }
                } else {
                    self.hovered_cell = None;
                }
            }
            Event::MousePress { pos, button: 1, .. } => {
                self.base.set_mouse_pressed(true);
                // The heading band is a target of its own: a press there reports the column
                // instead of selecting the first cell under it (SP-69).
                if !self.headers.is_empty() && self.hit_header(pos) {
                    if let Some(column) = self.cell_at_x(pos.x) {
                        self.header_clicked.emit(column);
                        self.base.clicked.emit();
                    }
                    return;
                }
                if let Some(cell) = self.cell_at(pos) {
                    self.base.clicked.emit();
                    self.select_cell(cell);
                    self.cell_clicked.emit(cell);
                }
            }
            Event::MouseDoubleClick { pos, button: 1 } => {
                // Double-click is the reachable edit gesture when the control is editable; with
                // editing off it stays an ordinary pair of presses (SP-115).
                if self.editable {
                    if let Some((row, column)) = self.cell_at(pos) {
                        self.select_cell((row, column));
                        self.begin_edit(row, column);
                    }
                }
            }
            Event::MouseRelease { pos: _, button: 1 } => {
                self.base.set_mouse_pressed(false);
            }
            Event::ContextMenu { pos } => {
                if let Some(cell) = self.cell_at(pos) {
                    self.context_menu_requested.emit(cell);
                }
            }
            Event::KeyPress { key, .. } => {
                let pressed = crate::shortcut::Key::from_key_code(key);
                let current = self.focused_cell.unwrap_or((0, 0));
                let (mut row, mut column) = current;
                match pressed {
                    Some(crate::shortcut::Key::Left) => column = column.saturating_sub(1),
                    Some(crate::shortcut::Key::Right) => {
                        column = (column + 1).min(self.columns.saturating_sub(1));
                    }
                    Some(crate::shortcut::Key::Up) => row = row.saturating_sub(1),
                    Some(crate::shortcut::Key::Down) => {
                        row = (row + 1).min(self.rows.saturating_sub(1));
                    }
                    Some(crate::shortcut::Key::Home) => column = 0,
                    Some(crate::shortcut::Key::End) => column = self.columns.saturating_sub(1),
                    Some(crate::shortcut::Key::Enter) | Some(crate::shortcut::Key::Space) => {
                        // Enter into an editable cell starts the edit rather than re-committing
                        // the selection; with editing off the historical behaviour is unchanged.
                        if self.editable && pressed == Some(crate::shortcut::Key::Enter) {
                            self.begin_edit(current.0, current.1);
                            return;
                        }
                        self.select_cell(current);
                        self.cell_clicked.emit(current);
                        return;
                    }
                    Some(crate::shortcut::Key::Escape) => {
                        self.clear_selection();
                        return;
                    }
                    _ => return,
                }
                self.focused_cell = Some((row, column));
                // A focus that scrolls off the current page is invisible: the ring stays on a page
                // the user is not looking at and the arrow keys appear dead. Bring the page the
                // focused row belongs to into view (SP-68).
                if let Some(page_rows) = self.page_rows {
                    if let Some(page) = row.checked_div(page_rows) {
                        if page != self.page {
                            self.set_page(page);
                        }
                    }
                }
                if self.multi_select {
                    // Arrow navigation moves the focus only; Space/Enter commits it.
                } else {
                    self.select_cell((row, column));
                }
                self.base.request_redraw();
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn with_dimensions_uses_columns_for_width_and_rows_for_height() {
        let grid = GridWidget::with_dimensions(Rect::new(0, 0, 120, 80), 2, 4);
        assert_eq!(grid.cell_width(), 30);
        assert_eq!(grid.cell_height(), 40);
    }

    #[test]
    fn grid_mouse_interaction_emits_cell_signals() {
        let mut grid = GridWidget::with_dimensions(Rect::new(0, 0, 100, 100), 2, 2);

        let clicked = Arc::new(Mutex::new(Vec::<(u32, u32)>::new()));
        let hovered = Arc::new(Mutex::new(Vec::<(u32, u32)>::new()));

        let clicked_sink = clicked.clone();
        grid.cell_clicked.connect(move |cell| {
            if let Ok(mut guard) = clicked_sink.lock() {
                guard.push(*cell);
            }
        });

        let hovered_sink = hovered.clone();
        grid.cell_hovered.connect(move |cell| {
            if let Ok(mut guard) = hovered_sink.lock() {
                guard.push(*cell);
            }
        });

        grid.handle_event(&Event::mouse_move(75, 25));
        grid.handle_event(&Event::mouse_press(75, 25, 1));

        let hovered_values = hovered.lock().expect("hovered lock poisoned").clone();
        let clicked_values = clicked.lock().expect("clicked lock poisoned").clone();

        assert_eq!(hovered_values, vec![(0, 1)]);
        assert_eq!(clicked_values, vec![(0, 1)]);
    }

    /// A press outside the grid reports no cell, in any direction.
    ///
    /// # Why this test exists even though the code is already right
    ///
    /// `cell_at` bounds-checks both axes (`cell_at_y` and `cell_at_x` each return `None` outside
    /// `base.geometry()`), so a point outside cannot resolve to a cell and the press arm's
    /// `clicked.emit()` is unreachable. That was an **argument**, and this file is listed in
    /// `tools/click_release_exemptions.txt` on the strength of it -- but the exemption table's own
    /// header is explicit that the vocabulary-based scan "does not prove the containment test guards
    /// the specific `clicked.emit()`", and that a mechanism the scan cannot name shows up as a
    /// finding precisely so it gets looked at.
    ///
    /// The risk this pins is drift, not a present defect: `cell_at_y`'s bound and `cell_at_x`'s bound
    /// are two separate lines, and deleting either one leaves the file's vocabulary unchanged and the
    /// gate green, while restoring the exact bug class the gate exists for.
    ///
    /// # Why the grid is built with a spacing
    ///
    /// At `spacing == 0` the cells tile the rectangle exactly and the column bound
    /// (`col < self.columns`) rejects every outside point on its own -- so a test written that way
    /// passes even with the rectangle bound deleted, proving nothing. It did: the first version of
    /// this test used `with_dimensions`, whose spacing defaults to `0`, and survived that injection.
    ///
    /// A spacing opens a gutter between the columns, and the gutter is exactly where the two bounds
    /// disagree: `x` past the last cell but still under `rect.right` maps to a *valid* column index,
    /// so only the rectangle bound rejects it. `rect.right` itself is that case -- `cell_at_x` admits
    /// it by division and the bound is what excludes it.
    #[test]
    fn a_press_outside_the_grid_reports_no_cell() {
        let mut grid = GridWidget::with_dimensions(Rect::new(0, 0, 100, 100), 2, 2);
        // The gutter is what makes the rectangle bound observable; see this test's rustdoc.
        grid.set_spacing(10);

        let clicked = Arc::new(Mutex::new(Vec::<(u32, u32)>::new()));
        let sink = clicked.clone();
        grid.cell_clicked.connect(move |cell| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*cell);
            }
        });

        // A point inside the first cell, to prove the coordinates below are the only difference.
        assert_eq!(grid.cell_at(Point::new(10, 10)), Some((0, 0)));

        // `rect.right` and `rect.bottom` are excluded (`>=`), and the far corner is 4000 px away.
        let rect = grid.geometry();
        let right = rect.x + rect.width as i32;
        let bottom = rect.y + rect.height as i32;
        for (label, point) in [
            ("left", Point::new(-1, 10)),
            ("right", Point::new(right, 10)),
            ("above", Point::new(10, -1)),
            ("below", Point::new(10, bottom)),
            ("far away", Point::new(5000, 5000)),
        ] {
            assert_eq!(grid.cell_at(point), None, "{label} of the grid must resolve to no cell");
            grid.handle_event(&Event::mouse_press(point.x, point.y, 1));
        }
        assert!(
            clicked.lock().expect("clicked lock poisoned").is_empty(),
            "a press outside the grid reported a cell"
        );
    }

    /// `#DCDCDC` is a colour a caller can actually ask for.
    ///
    /// # The defect this pins
    ///
    /// The separator colour was `Some(Color::rgb(220,220,220))` and the draw compared against that
    /// exact value as a **sentinel**: `if line_color == Color::rgb(220,220,220) { themed }`. Two
    /// consequences, and the first is the one that is a correctness problem rather than a smell —
    /// an application that wanted its grid lines in `#DCDCDC` asked for them and got the theme's
    /// border colour instead, with no error and nothing to report. The second is that the property
    /// route could not distinguish "the caller set nothing" from "the caller set that grey".
    ///
    /// The assertions cover both: the requested grey is painted as requested, and it is *different*
    /// from what an unset grid paints. The second half is what makes this a test of the sentinel
    /// rather than of a single value.
    #[test]
    fn a_caller_can_ask_for_the_separator_colour_that_used_to_be_the_sentinel() {
        use crate::widget::svg::render_to_svg;
        let _theme_guard = crate::style::theme_test_guard();

        // The sentinel's own value, in both the accessor and the property route.
        use crate::widget::capability::types::CapabilityValue as V;
        use crate::widget::capability::write_widget_property_by_name;
        let sentinel_hex = Color::rgb(220, 220, 220).to_hex_rgba();
        let mut asked = GridWidget::with_dimensions(Rect::new(0, 0, 120, 120), 2, 2);
        write_widget_property_by_name(&mut asked, "line_color", V::String(sentinel_hex.clone()))
            .expect("an explicit separator colour is a valid write");
        assert_eq!(
            asked.line_color(),
            Some(Some(Color::rgb(220, 220, 220))),
            "the caller's colour is stored as an explicit choice, not collapsed into the default"
        );

        // A grid that was never given a colour follows the theme.
        let unset = GridWidget::with_dimensions(Rect::new(0, 0, 120, 120), 2, 2);
        assert_eq!(
            unset.line_color(),
            Some(None),
            "an unset separator is \"follow the theme\", which is distinct from any colour"
        );

        // And the two paint differently -- which is the whole point: the old code made them paint
        // the same, so the requested grey was unreachable.
        let asked_svg = render_to_svg(&mut asked);
        let mut unset = unset;
        let unset_svg = render_to_svg(&mut unset);
        assert_ne!(
            asked_svg, unset_svg,
            "a requested separator colour must change the picture, or it was silently ignored"
        );
    }

    /// Disabling the separators is still expressible, and is distinct from "follow the theme".
    #[test]
    fn no_separators_is_still_the_null_write() {
        use crate::widget::capability::types::CapabilityValue as V;
        use crate::widget::capability::{
            read_widget_property_by_name, write_widget_property_by_name,
        };
        let mut grid = GridWidget::with_dimensions(Rect::new(0, 0, 120, 120), 2, 2);
        write_widget_property_by_name(&mut grid, "line_color", V::Null)
            .expect("null disables the lines");
        assert_eq!(grid.line_color(), None, "disabled is the outer none");
        assert_eq!(
            read_widget_property_by_name(&grid, "line_color").expect("publishable"),
            V::Null,
            "and it reads back as null"
        );
    }

    /// A paged matrix fills the control with the rows it shows, not with a share of every
    /// row in the model.
    #[test]
    fn a_paged_grid_fills_the_control_with_its_visible_rows() {
        let mut grid = GridWidget::with_dimensions(Rect::new(0, 0, 100, 100), 8, 2);
        grid.set_page_rows(Some(2));
        assert_eq!(grid.page_count(), 4);
        assert_eq!(grid.cell_height(), 50, "two visible rows share the 100 px body");

        let changed = std::sync::Arc::new(std::sync::Mutex::new(Vec::<u32>::new()));
        let sink = std::sync::Arc::clone(&changed);
        grid.page_changed.connect(move |page| sink.lock().unwrap().push(*page));
        grid.set_page(2);
        assert_eq!(grid.page(), 2);
        assert_eq!(*changed.lock().unwrap(), vec![2]);
    }

    /// The grid owns per-cell text and a heading band; both are painted from that model.
    #[test]
    fn cell_text_and_headers_are_drawn_and_round_trip() {
        use crate::widget::svg::render_to_svg;
        let mut grid = GridWidget::with_dimensions(Rect::new(0, 0, 200, 100), 2, 2);
        let empty = render_to_svg(&mut grid);
        assert!(grid.set_cell_text(0, 0, "Alpha"));
        assert!(grid.set_cell_text(1, 1, "Beta"));
        assert!(!grid.set_cell_text(4, 0, "out of range"));
        assert_eq!(grid.cell_text(0, 0), "Alpha");
        grid.set_headers(vec!["One".to_string(), "Two".to_string()]);

        let painted = render_to_svg(&mut grid);
        assert_ne!(empty, painted, "cell text and headings change the picture");

        // Escape clears the selection without clearing the focus.
        grid.select_cell((1, 1));
        assert_eq!(grid.selected_cells().len(), 1);
        grid.handle_event(&Event::key_press(27, 0));
        assert!(grid.selected_cells().is_empty());
    }

    #[test]
    fn set_geometry_updates_cell_dimensions() {
        let mut grid = GridWidget::with_dimensions(Rect::new(0, 0, 100, 100), 2, 2);
        assert_eq!(grid.cell_width(), 50);
        assert_eq!(grid.cell_height(), 50);

        grid.set_geometry(Rect::new(0, 0, 200, 300));
        assert_eq!(grid.cell_width(), 100);
        assert_eq!(grid.cell_height(), 150);
    }

    /// A still frame reuses the span/selection/offset tables; a model or viewport change rebuilds
    /// them (SP-114).
    #[test]
    fn the_frame_tables_are_reused_until_the_model_or_viewport_moves() {
        use crate::widget::svg::render_to_svg;
        let mut grid = GridWidget::with_dimensions(Rect::new(0, 0, 120, 120), 2, 2);

        let _ = render_to_svg(&mut grid);
        assert_eq!(grid.frame_builds, 1, "the first draw builds the tables");

        // The same model and geometry: no second build.
        let _ = render_to_svg(&mut grid);
        assert_eq!(grid.frame_builds, 1, "a still frame rebuilt the frame tables");

        // A selection change invalidates the selected-cell set.
        grid.select_cell((0, 1));
        let _ = render_to_svg(&mut grid);
        assert_eq!(grid.frame_builds, 2);

        // A viewport change invalidates the column offsets and the cell dimensions.
        grid.set_geometry(Rect::new(0, 0, 200, 200));
        let _ = render_to_svg(&mut grid);
        assert_eq!(grid.frame_builds, 3);
    }

    /// Row headings reserve a leading band, so the matrix starts to the right of it and every cell
    /// shares what is left (SP-115).
    #[test]
    fn row_headers_reserve_a_leading_band() {
        let mut grid = GridWidget::with_dimensions(Rect::new(0, 0, 200, 100), 2, 2);
        assert_eq!(grid.cell_at(Point::new(10, 10)), Some((0, 0)));
        assert_eq!(grid.column_width(0), 100);

        grid.set_row_headers(vec![String::from("Alpha"), String::from("Beta")]);
        assert_eq!(grid.row_header_width(), GRID_ROW_HEADER_WIDTH);
        // The heading column is not part of the matrix any more.
        assert_eq!(grid.cell_at(Point::new(10, 10)), None);
        assert_eq!(grid.cell_at(Point::new(GRID_ROW_HEADER_WIDTH as i32 + 10, 10)), Some((0, 0)));
        // Two columns share the 140 px left of the heading band.
        assert_eq!(grid.column_width(0), 70);
        assert_eq!(grid.column_width(1), 70);

        grid.set_row_headers(Vec::new());
        assert_eq!(grid.row_header_width(), 0);
        assert_eq!(grid.cell_at(Point::new(10, 10)), Some((0, 0)));
    }

    /// Editing is off by default; when enabled, a double-click starts it and Enter commits the
    /// buffer into the cell, emitting `cell_edited` (SP-115).
    #[test]
    fn in_place_editing_is_opt_in_and_commits_the_buffer() {
        let mut grid = GridWidget::with_dimensions(Rect::new(0, 0, 100, 100), 2, 2);
        grid.handle_event(&Event::mouse_double_click(10, 10, 1));
        assert_eq!(grid.editing_cell(), None, "a read-only grid must not start an edit");
        assert!(!grid.begin_edit(0, 0));

        let edited = Arc::new(Mutex::new(Vec::<(u32, u32, String)>::new()));
        let sink = edited.clone();
        grid.cell_edited.connect(move |cell| {
            if let Ok(mut guard) = sink.lock() {
                guard.push((*cell).clone());
            }
        });

        grid.set_editable(true);
        grid.handle_event(&Event::mouse_double_click(10, 10, 1));
        assert_eq!(grid.editing_cell(), Some((0, 0)));
        grid.handle_event(&Event::text_input("Hello"));
        grid.handle_event(&Event::key_press(13, 0));

        assert_eq!(grid.editing_cell(), None);
        assert_eq!(grid.cell_text(0, 0), "Hello");
        assert_eq!(
            edited.lock().expect("edited lock poisoned").clone(),
            vec![(0, 0, String::from("Hello"))]
        );
    }
}
