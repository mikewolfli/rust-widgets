// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Data grid widget backed by incremental data source protocol.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::compat::Rc;
use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, WheelScroll};
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{ConnectionHandle, ConnectionScope, Signal1};
use crate::widget::capability::access::{column_filters_to_string, sort_specs_to_string};
use crate::widget::capability::coercion::{
    expect_bool, expect_column_filters, expect_sort_specs, expect_string_list,
    expect_u32_saturating, expect_usize,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::companions::FilterRow;
use crate::widget::metrics::{effective_font, ControlMetrics};
use crate::widget::nav_widgets::pagination::Pagination;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

use super::column_chooser::ColumnChooser;
use super::data_source::{DataRole, IncrementalTableDataSource};

/// The margin a data grid leaves between its own frame and its cells: 2 px on every edge.
///
/// Named once so the first cell and the frozen-column divider are both measured from the
/// same inset. The cells used to start at the control's literal `rect.y`, which put the first
/// row's border on the frame's own stroke.
const GRID_INSET: u32 = 2;

/// The strip a data grid reserves above its first cell for column titles: 22 px, one row.
///
/// A grid reads as one whether or not a header row is supplied, so the first cell is always
/// one header below the frame rather than pinned to the top edge.
const GRID_HEADER_HEIGHT: u32 = 22;

/// What a selection gesture selects in a [`DataGrid`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DataGridSelectionBehavior {
    /// Individual cells (the historical behaviour).
    #[default]
    Cells,
    /// Whole rows, whichever cell was clicked.
    Rows,
    /// Whole columns, whichever cell was clicked.
    Columns,
}

fn data_grid_selection_mode_to_str(
    mode: crate::widget::view_widgets::list_view::SelectionMode,
) -> &'static str {
    use crate::widget::view_widgets::list_view::SelectionMode;
    match mode {
        SelectionMode::Single => "single",
        SelectionMode::Multi => "multi",
        SelectionMode::Extended => "extended",
        SelectionMode::None => "none",
    }
}

/// The published spelling of a column sizing mode.
fn column_width_mode_to_str(mode: ColumnWidthMode) -> &'static str {
    match mode {
        ColumnWidthMode::Fixed => "fixed",
        ColumnWidthMode::Stretch => "stretch",
        ColumnWidthMode::StretchLast => "stretch_last",
        ColumnWidthMode::Interactive => "interactive",
    }
}

/// Parses the published column sizing token.
fn expect_column_width_mode(
    value: CapabilityValue,
) -> Result<ColumnWidthMode, CapabilityAccessError> {
    match crate::widget::capability::coercion::expect_string(value)?.to_ascii_lowercase().as_str() {
        "fixed" => Ok(ColumnWidthMode::Fixed),
        "stretch" => Ok(ColumnWidthMode::Stretch),
        "stretch_last" | "stretchlast" => Ok(ColumnWidthMode::StretchLast),
        "interactive" => Ok(ColumnWidthMode::Interactive),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

fn expect_data_grid_selection_mode(
    value: CapabilityValue,
) -> Result<crate::widget::view_widgets::list_view::SelectionMode, CapabilityAccessError> {
    use crate::widget::view_widgets::list_view::SelectionMode;
    match value {
        CapabilityValue::String(token) => match token.as_str() {
            "single" => Ok(SelectionMode::Single),
            "multi" => Ok(SelectionMode::Multi),
            "extended" => Ok(SelectionMode::Extended),
            "none" => Ok(SelectionMode::None),
            _ => Err(CapabilityAccessError::TypeMismatch),
        },
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

fn selection_behavior_to_str(behavior: DataGridSelectionBehavior) -> &'static str {
    match behavior {
        DataGridSelectionBehavior::Cells => "cells",
        DataGridSelectionBehavior::Rows => "rows",
        DataGridSelectionBehavior::Columns => "columns",
    }
}

fn expect_selection_behavior(
    value: CapabilityValue,
) -> Result<DataGridSelectionBehavior, CapabilityAccessError> {
    match value {
        CapabilityValue::String(token) => match token.as_str() {
            "cells" => Ok(DataGridSelectionBehavior::Cells),
            "rows" => Ok(DataGridSelectionBehavior::Rows),
            "columns" => Ok(DataGridSelectionBehavior::Columns),
            _ => Err(CapabilityAccessError::TypeMismatch),
        },
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

/// A sort key normalised once per projected row.
pub(crate) fn normalise_sort_key(text: &str) -> (String, Option<f64>) {
    let trimmed = text.trim();
    (trimmed.to_lowercase(), trimmed.parse::<f64>().ok())
}

/// Compares normalised keys, preferring a numeric order when both sides are numbers.
///
/// A pure `String` comparison put `"10"` before `"2"`; a numeric column therefore painted in an
/// order no user would recognise. Parsing both sides is what makes the default comparison
/// role-aware enough for the common textual-number case, while a role explicitly supplied through
/// [`DataGrid::set_sort_role`] can expose a dedicated sort representation.
pub(crate) fn compare_sort_keys(
    left: &(String, Option<f64>),
    right: &(String, Option<f64>),
) -> core::cmp::Ordering {
    if let (Some(left), Some(right)) = (left.1, right.1) {
        if let Some(order) = left.partial_cmp(&right) {
            return order;
        }
    }
    left.0.cmp(&right.0)
}

use super::filter_expr::{FilterCondition, FilterExpr};

/// Rows after filtering and sorting, holding one optional cell per source column.
///
/// Only the explicit full-model mode (`DataGrid::set_full_model_projection(true)`) stores a
/// matrix of these; the default virtualized mode keeps a [`ProjectedIndex`] instead.
type ProjectedRows = Vec<Vec<Option<String>>>;

/// The display order of a virtualized projection: the source row for each projected row.
///
/// A sort reorders these indices and a filter drops them, so a projection costs one `usize` per
/// row and never a cloned row or cell (V-002).
type ProjectedIndex = Vec<usize>;

/// The sliced window most recently handed out from the full-model projection.
///
/// Keyed by the projection allocation and the window rectangle: a repeated paint returns the
/// same `Rc` rather than copying the visible slice again. The pointer comparison also makes a
/// stale window unreachable when the projection is rebuilt.
type FullWindowCache =
    Option<(Rc<ProjectedRows>, (usize, usize, usize, usize), Rc<Vec<Vec<Option<String>>>>)>;

/// The window most recently fetched for a virtualized projection.
///
/// Keyed by the projection allocation and the window rectangle, so a scroll reuses nothing stale
/// and a repeated paint is a refcount bump. The matrix holds **only** the window's rows: the
/// model itself is never materialised (V-001).
type VirtualWindowCache =
    Option<(Rc<ProjectedIndex>, (usize, usize, usize, usize), Rc<Vec<Vec<Option<String>>>>)>;

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
/// Cached window of source cells, shared with every reader instead of copied per frame.
struct GridWindowCache {
    row_start: usize,
    row_len: usize,
    column_start: usize,
    column_len: usize,
    revision: u64,
    cells: Rc<Vec<Vec<Option<String>>>>,
    /// The filter/sort projection of `cells`, produced once and reused on later frames.
    ///
    /// The shared-window fix only covered the no-filter/no-sort path; with either active,
    /// every frame cloned the whole window and re-ran `retain` + `sort_by` over the copy.
    /// `None` means "not projected yet"; `clear_cache()` drops it together with the window.
    projected: Option<Rc<Vec<Vec<Option<String>>>>>,
}

/// Prefix sums of column widths, rebuilt only when a width input changes.
///
/// `column_prefix_width` used to sum `0..upto` on every call, and `cell_rect` called it twice
/// per painted cell — an interactive-width grid therefore cost O(rows x columns^2) per frame.
/// The table is O(columns) to build and O(1) to read.
#[derive(Clone, Debug)]
struct ColumnPrefixCache {
    revision: u64,
    column_count: usize,
    cells_width: u32,
    mode: ColumnWidthMode,
    default_width: u32,
    prefix: Vec<i32>,
}

/// Elided header labels, keyed by `(column, painted width)`.
#[derive(Clone, Debug, Default)]
struct HeaderTextCache {
    labels_revision: u64,
    generation: u64,
    column_count: usize,
    entries: std::collections::HashMap<(usize, u32), String>,
}

/// A read-only, column-sliced view of the projected window the draw pass paints.
struct ProjectedWindowView {
    rows: Rc<ProjectedRows>,
    row_start: usize,
    row_len: usize,
    col_start: usize,
    col_len: usize,
}

impl ProjectedWindowView {
    fn is_empty(&self) -> bool {
        self.row_len == 0 || self.col_len == 0
    }

    fn len(&self) -> usize {
        self.row_len
    }

    /// The window-local row `index`, sliced to the visible column span.
    fn row(&self, index: usize) -> Option<&[Option<String>]> {
        if index >= self.row_len {
            return None;
        }
        let row = self.rows.get(self.row_start.checked_add(index)?)?;
        let start = self.col_start.min(row.len());
        let end = self.col_start.saturating_add(self.col_len).min(row.len());
        row.get(start..end)
    }
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
    /// The filter as a recursive expression.
    ///
    /// This is what `apply_filter_sort` evaluates; `filters` is derived from it, so
    /// the two cannot describe different filters.
    filter_expr: FilterExpr,
    window_cache: Option<GridWindowCache>,
    /// Prefix sums of the current column widths; see [`ColumnPrefixCache`].
    column_prefix_cache: RefCell<Option<ColumnPrefixCache>>,
    /// Bumped by every width-affecting setter (F-P01).
    column_geometry_revision: u64,
    /// Number of prefix-table rebuilds, published so a test can prove the table is reused.
    column_prefix_rebuilds: Cell<u64>,
    /// Elided header labels; see [`HeaderTextCache`] (F-P04).
    header_text_cache: RefCell<HeaderTextCache>,
    /// Bumped by `set_header_labels` / `set_header`.
    header_text_revision: u64,
    /// Bumped by the source's `data_changed` signal and by `set_data_source`.
    ///
    /// A source whose `revision()` stays `0` still changes, and the signal is how it says so.
    /// Keying the window cache on this counter is what lets such a source be cached at all; a
    /// source that offers neither a revision nor a signal is not cached, because then there is no
    /// evidence a cached window is still current.
    data_generation: Arc<AtomicU64>,
    /// Whether the bound source can report change (`revision > 0` or a `data_changed` signal).
    data_is_observable: bool,
    /// The projected `(row, column)` of the last press, or `None` when the press missed.
    ///
    /// Set through [`Self::cell_at`], so a selection can only ever name a cell that was
    /// actually painted.
    selection: Option<(usize, usize)>,
    /// How many units a press may add to the selection.
    selection_mode: crate::widget::view_widgets::list_view::SelectionMode,
    /// What a selection gesture selects (cells, rows or columns).
    selection_behavior: DataGridSelectionBehavior,
    /// The selected units, stored as cells: `(row, 0)` for a row, `(0, column)` for a column and
    /// the exact cell for cell selection. Keeping one set means the three behaviours share the
    /// paint and query paths.
    selected_cells: BTreeSet<(usize, usize)>,
    /// The cell a range selection extends from.
    selection_anchor: Option<(usize, usize)>,
    /// The cell keyboard navigation currently points at.
    current_cell: Option<(usize, usize)>,
    /// A `select_all` that is represented by a flag instead of a per-cell set.
    all_selected: bool,
    /// Roles used for a column's sort key, when it differs from the display text.
    sort_roles: std::collections::HashMap<usize, DataRole>,
    /// Whether the header paints a sort arrow.
    sort_indicator_visible: bool,
    /// Explicit per-column widths used by [`ColumnWidthMode::Interactive`].
    column_widths: std::collections::HashMap<usize, u32>,
    /// The column currently being dragged wider or narrower.
    resizing_column: Option<usize>,
    /// The pointer x where the current resize drag began.
    resize_start_x: i32,
    /// The width the column had when the current resize drag began.
    resize_start_width: u32,
    /// Labels supplied through [`DataGrid::set_header`] or `set_header_labels`.
    header_labels: Vec<String>,
    /// The column specs a bound [`GridViewHeader`] contributed.
    ///
    /// Held as specs rather than as a header control because a widget cannot own a borrow of
    /// another widget: the header is copied in at `set_header`, and its per-column capabilities
    /// (sortable, filterable) keep driving this grid's header row afterwards.
    header_columns: Vec<ColumnSpec>,
    /// Whether filter glyphs are painted on the filterable header columns.
    header_filter_visible: bool,
    /// The pagination bar this grid is bound to, if any.
    pagination: Option<PaginationBinding>,
    /// The last page the binding applied, so polling twice does not re-emit.
    pagination_applied_page: usize,
    /// The `FilterRow` this grid follows, when one is attached (chapter 3, gap 2).
    filter_row: Option<FilterBinding>,
    /// Emitted when the selection changes; query [`DataGrid::selected_cells`] for the units.
    selection_changed: Signal1<()>,
    /// Emitted with the cell keyboard navigation moved to.
    current_changed: Signal1<(usize, usize)>,
    /// Emitted when a cell is pressed.
    cell_clicked: Signal1<(usize, usize)>,
    /// Emitted when a cell is double-clicked.
    cell_double_clicked: Signal1<(usize, usize)>,
    /// Emitted after a user resize with `(column, width)`.
    column_resized: Signal1<(usize, u32)>,
    /// The projected `(row, column)` under the pointer, or `None` when it is elsewhere.
    ///
    /// Set through the **same** [`Self::cell_at`] the press uses, so the cell a hover points at is
    /// the cell a click would select — the two cannot disagree about where a cell begins. A grid is a
    /// matrix of independent targets, so this has to name a cell rather than light the control.
    hovered_cell: Option<(usize, usize)>,
    /// Emitted when visible row/column window changes.
    pub visible_window_changed: Signal1<(usize, usize, usize, usize)>,
    /// Whether filtering and sorting project the whole model instead of the visible window.
    ///
    /// Off by default: the default path is virtualized and materialises only the visible window
    /// (V-001). `true` keeps the historical whole-model materialisation as an explicit option.
    full_model_projection: bool,
    /// Cache for the full-model projection, keyed by the source generation/revision.
    full_projection_cache: Option<(u64, Rc<ProjectedRows>)>,
    /// Cache for the virtualized projection's row order, keyed by the source generation/revision.
    index_projection_cache: Option<(u64, Rc<ProjectedIndex>)>,
    /// The sliced window most recently handed out from `full_projection_cache`.
    full_window_cache: FullWindowCache,
    /// The window most recently fetched for the virtualized projection.
    virtual_window_cache: VirtualWindowCache,
    /// How column widths are decided.
    column_width_mode: ColumnWidthMode,
    /// Whether odd rows get the alternating wash.
    alternating_rows: bool,
    /// What gestures start an edit.
    edit_triggers: EditTriggers,
    /// Whether horizontal/vertical scroll-bar thumbs are painted.
    show_scrollbars: bool,
    /// Emitted when a header cell is clicked, with the column index.
    pub header_clicked: Signal1<usize>,
    /// Emitted when sorting changes for a column.
    pub sort_changed: Signal1<(usize, bool)>,
    /// Emitted when an edit is requested, with `(row, column)`.
    pub edit_requested: Signal1<(usize, usize)>,
    /// Emitted when the context menu is requested at a point.
    pub context_menu_requested: Signal1<Point>,
    /// Emitted with the new expression whenever the filter changes.
    pub filter_changed: Signal1<FilterExpr>,
    /// Emitted with the zero-based page when a page from the bound [`Pagination`] is applied.
    pub page_applied: Signal1<usize>,
    /// Columns excluded from painting and hit-testing (V-014).
    hidden_columns: BTreeSet<usize>,
    /// The `ColumnChooser` this grid follows, when one is attached (V-015).
    column_chooser: Option<ColumnChooserBinding>,
    /// The column display order the chooser last reported, as source column indices.
    ///
    /// Empty means the source order. The paint and hit-test paths address columns by their
    /// absolute source index, so this records the reported order without repainting the grid in
    /// a second coordinate space.
    column_order: Vec<usize>,
    /// Merged cells, expressed in projected coordinates (V-014).
    cell_spans: Vec<CellSpan>,
    /// Additional header rows painted above the title row (V-014).
    extra_header_rows: Vec<Vec<String>>,
    /// Per-cell tooltip source; `None` keeps the historical no-tooltip behaviour (V-014).
    cell_tooltip: Option<Arc<dyn Fn(usize, usize) -> Option<String> + Send + Sync>>,
    /// Per-cell paint delegate; `None` draws every cell directly (V-014).
    cell_delegate: Option<Arc<dyn DataGridCellDelegate>>,
}

/// How a data grid decides column widths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColumnWidthMode {
    /// Every column uses [`DataGrid::column_width`] (the historical behaviour).
    #[default]
    Fixed,
    /// Columns divide the visible cells box equally.
    Stretch,
    /// Every column keeps its width except the last, which takes whatever room is left.
    ///
    /// This is the classic table-header mode for a list whose final column is a note or action
    /// column: stretching every column would move the one the reader is reading.
    StretchLast,
    /// Columns use the fixed width until the caller resizes one (interactive mode).
    Interactive,
}

/// Gestures that start an in-place edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EditTriggers(u8);

impl EditTriggers {
    /// No gesture starts an edit.
    pub const NONE: Self = Self(0);
    /// A double-click starts an edit.
    pub const DOUBLE_CLICKED: Self = Self(1 << 0);
    /// A selected cell starts an edit when the user presses Enter.
    pub const EDIT_KEY: Self = Self(1 << 1);
    /// Any of the above.
    pub const ALL: Self = Self(0b11);

    /// Whether every flag in `other` is present.
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// The union of two trigger sets.
    pub fn or(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// One column as a header describes it.
///
/// A header is shared by more than one table implementation, so this is deliberately the whole
/// of a column's presentation: the title, its width, and the three capabilities a header row can
/// offer. Nothing here refers to a cell's data, which is the table's own business.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnSpec {
    /// Column title.
    pub title: String,
    /// Width in logical pixels when the header is not stretching the column.
    pub width: u32,
    /// Whether clicking the section cycles its sort order.
    pub sortable: bool,
    /// Whether the section shows a filter affordance.
    pub filterable: bool,
    /// Whether the section's right edge may be dragged.
    pub resizable: bool,
}

impl ColumnSpec {
    /// Creates a plain column: fixed width, no sort or filter affordance, resizable.
    pub fn new(title: impl Into<String>, width: u32) -> Self {
        Self {
            title: title.into(),
            width: width.max(1),
            sortable: false,
            filterable: false,
            resizable: true,
        }
    }

    /// Marks the column sortable.
    pub fn with_sortable(mut self, sortable: bool) -> Self {
        self.sortable = sortable;
        self
    }

    /// Marks the column filterable.
    pub fn with_filterable(mut self, filterable: bool) -> Self {
        self.filterable = filterable;
        self
    }

    /// Sets whether the column's edge may be dragged.
    pub fn with_resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    /// The published property spelling: `title|width|sortable|filterable|resizable`.
    pub fn to_token(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}",
            self.title,
            self.width,
            u8::from(self.sortable),
            u8::from(self.filterable),
            u8::from(self.resizable)
        )
    }

    /// Parses [`Self::to_token`]; flags are `0`/`1`.
    pub fn from_token(token: &str) -> Option<Self> {
        let mut parts = token.splitn(5, '|');
        let title = parts.next()?.to_string();
        let width = parts.next()?.trim().parse::<u32>().ok()?.max(1);
        let flag = |text: Option<&str>| matches!(text.map(str::trim), Some("1" | "true"));
        Some(Self {
            title,
            width,
            sortable: flag(parts.next()),
            filterable: flag(parts.next()),
            resizable: parts.next().is_none_or(|text| !matches!(text.trim(), "0" | "false")),
        })
    }
}

/// Which way a column's sort runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    /// Smallest first.
    Ascending,
    /// Largest first.
    Descending,
}

impl SortOrder {
    /// The token spelling published in `sort_columns`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ascending => "ascending",
            Self::Descending => "descending",
        }
    }

    /// Parses a published token spelling.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "ascending" | "asc" => Some(Self::Ascending),
            "descending" | "desc" => Some(Self::Descending),
            _ => None,
        }
    }

    /// Whether the order is descending, the flag `DataGrid::set_sort_specs` takes.
    pub fn is_descending(self) -> bool {
        matches!(self, Self::Descending)
    }

    /// The order a click on an already-sorted section advances to.
    pub fn next(self) -> Self {
        match self {
            Self::Ascending => Self::Descending,
            Self::Descending => Self::Ascending,
        }
    }
}

/// A merged cell: its origin and how many rows and columns it covers (V-014).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellSpan {
    /// The origin's projected row.
    pub row: usize,
    /// The origin's projected column.
    pub column: usize,
    /// Rows covered, at least 1.
    pub row_span: usize,
    /// Columns covered, at least 1.
    pub column_span: usize,
}

impl CellSpan {
    /// A span of at least one row and one column.
    pub fn new(row: usize, column: usize, row_span: usize, column_span: usize) -> Self {
        Self { row, column, row_span: row_span.max(1), column_span: column_span.max(1) }
    }

    /// Whether `(row, column)` is covered by this span.
    pub fn covers(&self, row: usize, column: usize) -> bool {
        row >= self.row
            && row < self.row.saturating_add(self.row_span)
            && column >= self.column
            && column < self.column.saturating_add(self.column_span)
    }
}

/// Paint hook for a data grid cell (V-014).
///
/// A delegate owns a cell's ink when `paint` returns `true`; returning `false` keeps the grid's
/// own text rendering, which is also the behaviour when no delegate is installed.
pub trait DataGridCellDelegate: Send + Sync {
    /// Paints `cell`, returning whether the delegate took over the cell.
    fn paint(
        &self,
        context: &mut RenderContext,
        cell: Rect,
        row: usize,
        column: usize,
        text: Option<&str>,
    ) -> bool {
        let _ = (context, cell, row, column, text);
        false
    }
}

/// The height of a header strip: 28 px, one row of titles.
const HEADER_STRIP_HEIGHT: u32 = 28;
/// How close to a section's right edge a press must land to start a resize.
const HEADER_RESIZE_GRAB: i32 = 4;
/// The side of a sort arrow or filter glyph.
const HEADER_ICON_SIDE: u32 = 10;

/// A column header strip shared by the grid-family controls.
///
/// `DataGrid`, `GridTable` and `Pagination` each used to draw their own header, which is why
/// one showed sort arrows without filter affordances and another the reverse. This control is
/// the single implementation: it owns the section geometry, paints the arrows and filter glyphs,
/// and reports the three gestures a header can receive (click, resize, filter).
///
/// It is **not** a data control: `set_sort_indicator` records what the table asked to show, and
/// `section_clicked` says what the reader did. Sorting the rows remains the table's job.
pub struct GridViewHeader {
    base: BaseWidget,
    columns: Vec<ColumnSpec>,
    /// The order shown per section, `None` when unsorted.
    sorts: Vec<Option<SortOrder>>,
    /// Whether sort arrows are painted.
    sort_indicator_visible: bool,
    /// How section widths are decided.
    resize_mode: ColumnWidthMode,
    /// Whether filter glyphs are painted on filterable sections.
    filter_icon_visible: bool,
    /// The section the pointer is over.
    hovered: Option<usize>,
    /// The drag in progress: `(section, press x, width at the press)`.
    resizing: Option<(usize, i32, u32)>,
    /// Emitted with the section index when its body is clicked.
    pub section_clicked: Signal1<usize>,
    /// Emitted with `(section, width)` after a drag that changed a width.
    pub section_resized: Signal1<(usize, u32)>,
    /// Emitted with the section index when its filter glyph is clicked.
    pub filter_clicked: Signal1<usize>,
}

impl GridViewHeader {
    /// Creates an empty header.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::GridViewHeader, geometry, "GridViewHeader"),
            columns: Vec::new(),
            sorts: Vec::new(),
            sort_indicator_visible: true,
            resize_mode: ColumnWidthMode::Fixed,
            filter_icon_visible: true,
            hovered: None,
            resizing: None,
            section_clicked: Signal1::new(),
            section_resized: Signal1::new(),
            filter_clicked: Signal1::new(),
        }
    }

    /// The configured columns.
    pub fn columns(&self) -> &[ColumnSpec] {
        &self.columns
    }

    /// Replaces the columns.
    ///
    /// Sort indicators survive for the sections that still exist; a section that is gone cannot
    /// keep an arrow nothing paints.
    pub fn set_columns(&mut self, columns: Vec<ColumnSpec>) {
        let mut sorts = core::mem::take(&mut self.sorts);
        sorts.resize(columns.len(), None);
        self.sorts = sorts;
        self.columns = columns;
        self.hovered = None;
        self.resizing = None;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Whether sort arrows are painted.
    pub fn sort_indicator_visible(&self) -> bool {
        self.sort_indicator_visible
    }

    /// Shows or hides sort arrows.
    pub fn set_sort_indicator_visible(&mut self, visible: bool) {
        if self.sort_indicator_visible == visible {
            return;
        }
        self.sort_indicator_visible = visible;
        self.base.request_redraw();
    }

    /// Whether filter glyphs are painted.
    pub fn filter_icon_visible(&self) -> bool {
        self.filter_icon_visible
    }

    /// Shows or hides the filter glyphs of filterable sections.
    pub fn set_filter_icon_visible(&mut self, visible: bool) {
        if self.filter_icon_visible == visible {
            return;
        }
        self.filter_icon_visible = visible;
        self.base.request_redraw();
    }

    /// How section widths are decided.
    pub fn resize_mode(&self) -> ColumnWidthMode {
        self.resize_mode
    }

    /// Sets how section widths are decided.
    pub fn set_resize_mode(&mut self, mode: ColumnWidthMode) {
        if self.resize_mode == mode {
            return;
        }
        self.resize_mode = mode;
        self.base.request_redraw();
    }

    /// The width painted for section `column`.
    pub fn section_width(&self, column: usize) -> u32 {
        let count = self.columns.len().max(1) as u32;
        let fixed = |column: usize| self.columns.get(column).map_or(1, |spec| spec.width.max(1));
        match self.resize_mode {
            ColumnWidthMode::Fixed | ColumnWidthMode::Interactive => fixed(column),
            ColumnWidthMode::Stretch => (self.geometry().width / count).max(1),
            ColumnWidthMode::StretchLast => {
                if column + 1 != count as usize {
                    return fixed(column);
                }
                let used: u32 = (0..self.columns.len().saturating_sub(1)).map(fixed).sum();
                self.geometry().width.saturating_sub(used).max(1)
            }
        }
    }

    /// The painted box of section `column`.
    ///
    /// Paint, hit-testing and the resize handle all read this one derivation, so a press can
    /// never land in a section other than the one it is drawn in.
    pub fn section_rect(&self, column: usize) -> Option<Rect> {
        if column >= self.columns.len() {
            return None;
        }
        let rect = self.geometry();
        let x: i32 = (0..column).map(|index| self.section_width(index) as i32).sum();
        let width = self.section_width(column);
        let trimmed = width.min((rect.width as i32 - x).max(0) as u32);
        (trimmed > 0).then(|| Rect::new(rect.x + x, rect.y, trimmed, rect.height))
    }

    /// The painted box of section `column`'s filter glyph, when it has one.
    pub fn filter_rect(&self, column: usize) -> Option<Rect> {
        if !self.filter_icon_visible {
            return None;
        }
        if !self.columns.get(column).is_some_and(|spec| spec.filterable) {
            return None;
        }
        let section = self.section_rect(column)?;
        if section.width < HEADER_ICON_SIDE + 4 {
            return None;
        }
        Some(Rect::new(
            section.x + section.width as i32 - HEADER_ICON_SIDE as i32 - 3,
            section.y + (section.height as i32 - HEADER_ICON_SIDE as i32) / 2,
            HEADER_ICON_SIDE,
            HEADER_ICON_SIDE,
        ))
    }

    /// The order shown for section `column`.
    pub fn sort_indicator(&self, column: usize) -> Option<SortOrder> {
        self.sorts.get(column).copied().flatten()
    }

    /// Records the sort order to paint on section `column`.
    ///
    /// `None` clears the arrow. Returns `false` for a section that does not exist: a caller
    /// asking to mark a column the header does not have has made a mistake worth reporting.
    pub fn set_sort_indicator(&mut self, column: usize, order: Option<SortOrder>) -> bool {
        if column >= self.columns.len() {
            return false;
        }
        if self.sorts[column] == order {
            return true;
        }
        self.sorts[column] = order;
        self.base.request_redraw();
        true
    }

    /// Cycles section `column` between ascending, descending and unsorted.
    ///
    /// Returns the order it landed on, or `None` when the section is not sortable or does not
    /// exist — a header that reported a sort it never painted would be lying to its table.
    pub fn toggle_sort(&mut self, column: usize) -> Option<SortOrder> {
        let spec = self.columns.get(column)?;
        if !spec.sortable {
            return None;
        }
        let next = match self.sorts.get(column).copied().flatten() {
            None => Some(SortOrder::Ascending),
            Some(SortOrder::Ascending) => Some(SortOrder::Descending),
            Some(SortOrder::Descending) => None,
        };
        let _ = self.set_sort_indicator(column, next);
        next
    }

    /// The section under `pos`.
    fn section_at(&self, pos: Point) -> Option<usize> {
        (0..self.columns.len())
            .find(|column| self.section_rect(*column).is_some_and(|rect| rect.contains_point(pos)))
    }

    /// The section whose right edge `pos` is on, when that section may be resized.
    fn resize_handle_at(&self, pos: Point) -> Option<usize> {
        let column = self.section_at(pos)?;
        let spec = self.columns.get(column)?;
        if !spec.resizable || !matches!(self.resize_mode, ColumnWidthMode::Interactive) {
            return None;
        }
        let section = self.section_rect(column)?;
        let right = section.x + section.width as i32;
        ((pos.x - right).abs() <= HEADER_RESIZE_GRAB).then_some(column)
    }

    /// Sets one section's width, switching to interactive sizing.
    pub fn set_section_width(&mut self, column: usize, width: u32) -> bool {
        let Some(spec) = self.columns.get_mut(column) else {
            return false;
        };
        let width = width.max(8);
        if spec.width == width {
            return true;
        }
        spec.width = width;
        self.resize_mode = ColumnWidthMode::Interactive;
        self.base.request_layout();
        self.base.request_redraw();
        true
    }
}

impl Widget for GridViewHeader {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(320, HEADER_STRIP_HEIGHT)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves the three header gestures; see `DiffViewer`'s note for why the arms are explicit.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "section_clicked" => {
                Some(EventSignalRef::mapped("section_clicked", &self.section_clicked, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            "section_resized" => {
                Some(EventSignalRef::mapped("section_resized", &self.section_resized, |v| {
                    CapabilityValue::Tuple(crate::compat::Vec::from([
                        CapabilityValue::UInt(v.0 as u64),
                        CapabilityValue::UInt(v.1 as u64),
                    ]))
                }))
            }
            "filter_clicked" => {
                Some(EventSignalRef::mapped("filter_clicked", &self.filter_clicked, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for GridViewHeader {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "columns" => Ok(CapabilityValue::Tuple(
                self.columns.iter().map(|spec| CapabilityValue::String(spec.to_token())).collect(),
            )),
            "column_count" => Ok(CapabilityValue::UInt(self.columns.len() as u64)),
            "sort_indicator_visible" => Ok(CapabilityValue::Bool(self.sort_indicator_visible)),
            "resize_mode" => {
                Ok(CapabilityValue::String(column_width_mode_to_str(self.resize_mode).to_string()))
            }
            "filter_icon_visible" => Ok(CapabilityValue::Bool(self.filter_icon_visible)),
            "sort_columns" => Ok(CapabilityValue::String(
                self.sorts
                    .iter()
                    .enumerate()
                    .filter_map(|(column, order)| {
                        order.map(|order| format!("{column}:{}", order.as_str()))
                    })
                    .collect::<Vec<_>>()
                    .join(","),
            )),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "columns" => {
                let items = expect_string_list(value)?;
                let mut columns = Vec::with_capacity(items.len());
                for item in items {
                    columns.push(
                        ColumnSpec::from_token(&item).ok_or(CapabilityAccessError::TypeMismatch)?,
                    );
                }
                self.set_columns(columns);
                Ok(())
            }
            "column_count" | "sort_columns" => Err(CapabilityAccessError::ReadOnlyProperty),
            "sort_indicator_visible" => {
                self.set_sort_indicator_visible(expect_bool(value)?);
                Ok(())
            }
            "resize_mode" => {
                self.set_resize_mode(expect_column_width_mode(value)?);
                Ok(())
            }
            "filter_icon_visible" => {
                self.set_filter_icon_visible(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "columns",
            "column_count",
            "sort_indicator_visible",
            "resize_mode",
            "filter_icon_visible",
            "sort_columns",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl crate::event::EventHandler for GridViewHeader {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button: 1, .. } => {
                if let Some(column) = self.resize_handle_at(*pos) {
                    let width = self.section_width(column);
                    self.resizing = Some((column, pos.x, width));
                    return;
                }
                if let Some(column) = (0..self.columns.len()).rev().find(|column| {
                    self.filter_rect(*column).is_some_and(|rect| rect.contains_point(*pos))
                }) {
                    self.filter_clicked.emit(column);
                    return;
                }
                if let Some(column) = self.section_at(*pos) {
                    let _ = self.toggle_sort(column);
                    self.section_clicked.emit(column);
                }
            }
            Event::MouseMove { pos } => {
                if let Some((column, press_x, start_width)) = self.resizing {
                    let width = (start_width as i32 + (pos.x - press_x)).max(8) as u32;
                    if let Some(spec) = self.columns.get_mut(column) {
                        if spec.width != width {
                            spec.width = width;
                            self.base.request_redraw();
                        }
                    }
                    return;
                }
                let hovered = self.section_at(*pos);
                if hovered != self.hovered {
                    self.hovered = hovered;
                    self.base.request_redraw();
                }
            }
            Event::MouseRelease { button: 1, .. } => {
                if let Some((column, _, start_width)) = self.resizing.take() {
                    let width = self.section_width(column);
                    if width != start_width {
                        self.section_resized.emit((column, width));
                    }
                    self.base.request_redraw();
                }
            }
            _ => {}
        }
    }
}

impl Draw for GridViewHeader {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("data_grid");
        let surface = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| surface.blend(&ink, 0.20));
        let font = effective_font(style);

        context.face_with_gradient_corners(
            rect,
            surface.blend(&ink, 0.06),
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().resolved_corner_radii().to_array(),
            Color::BLACK,
        );
        crate::widget::draw::stroke_surface_border(
            context,
            rect,
            self.style().resolved_corner_radii().to_array(),
            border,
            1,
        );

        for column in 0..self.columns.len() {
            let Some(section) = self.section_rect(column) else {
                continue;
            };
            if self.hovered == Some(column) {
                context.fill_rect(section, surface.blend(&ink, 0.03));
            }
            let spec = &self.columns[column];
            let filter_slot = if self.filter_rect(column).is_some() { 12i32 } else { 0 };
            let sort_slot = if self.sort_indicator_visible && self.sort_indicator(column).is_some()
            {
                12
            } else {
                0
            };
            let title_width =
                section.width.saturating_sub(8 + filter_slot.max(0) as u32 + sort_slot).max(8);
            let label = crate::widget::text_utils::elide_end(
                &spec.title,
                (title_width as usize / 7).max(1),
            );
            context.draw_text(
                Point::new(section.x + 4, section.y + (section.height as i32 - 12) / 2),
                &label,
                font,
                ink,
                HorizontalAlignment::Left,
            );
            if self.sort_indicator_visible {
                if let Some(order) = self.sort_indicator(column) {
                    let icon = if order.is_descending() {
                        crate::widget::IconName::ArrowDown
                    } else {
                        crate::widget::IconName::ArrowUp
                    };
                    let icon_rect = Rect::new(
                        section.x + section.width as i32
                            - filter_slot
                            - HEADER_ICON_SIDE as i32
                            - 3,
                        section.y + (section.height as i32 - HEADER_ICON_SIDE as i32) / 2,
                        HEADER_ICON_SIDE,
                        HEADER_ICON_SIDE,
                    );
                    crate::widget::draw_icon_at(context, icon_rect, ink, icon);
                }
            }
            if let Some(filter_rect) = self.filter_rect(column) {
                crate::widget::draw_icon_at(
                    context,
                    filter_rect,
                    ink.blend(&surface, 0.25),
                    crate::widget::IconName::Filter,
                );
            }
            context.draw_line(
                Point::new(section.x + section.width as i32 - 1, section.y),
                Point::new(section.x + section.width as i32 - 1, section.y + section.height as i32),
                surface.blend(&ink, 0.12),
            );
        }
    }
}

/// The live link between a [`DataGrid`] and a [`Pagination`] bar.
///
/// A widget cannot hold a borrow of another widget, so the link is the pagination's
/// `page_changed` slot plus the shared cell it writes. [`DataGrid::poll_pagination`] reads that
/// cell, which is what keeps a page click from requiring the host to forward it by hand.
struct PaginationBinding {
    /// The page the bound bar most recently reported.
    page: Arc<AtomicU64>,
    /// Items per page, copied when the bar was bound.
    page_size: usize,
    /// The subscription, dropped with the binding so a rebound grid leaves no slot behind.
    _connection: ConnectionHandle,
}

/// The live link between a [`DataGrid`] and a [`FilterRow`] (chapter 3, gap 2).
///
/// A row publishes the complete condition list after every change, so the link parks the latest
/// report in a shared slot and [`DataGrid::poll_filter_row`] folds it into the grid's filter model.
/// This is the same host-free shape as [`PaginationBinding`]: a widget cannot hold a borrow of
/// another, so the signal writes state and the grid reads it on its next paint or interaction.
struct FilterBinding {
    /// The condition list the row published since the last poll.
    conditions: Arc<Mutex<Option<Vec<FilterCondition>>>>,
    /// The subscription, dropped with the binding so a detached grid leaves no slot behind.
    _scope: ConnectionScope,
}

/// The live link between a [`DataGrid`] and a [`ColumnChooser`] (V-015).
///
/// The chooser's signals carry persisted column tokens rather than source indices, so the link
/// resolves each token through the column set both controls were configured with and parks the
/// answer in a shared queue. [`DataGrid::poll_column_chooser`] drains that queue, which is the
/// same host-free pattern the pagination binding uses.
struct ColumnChooserBinding {
    /// `(source column, visible)` changes reported since the last poll.
    visibility: Arc<Mutex<Vec<(usize, bool)>>>,
    /// The display order the chooser last reported, mapped to source columns.
    order: Arc<Mutex<Vec<usize>>>,
    /// The subscriptions, dropped with the binding so a re-attach leaves no slot behind.
    _scope: ConnectionScope,
}

/// Resolves a chooser token to the grid's source column.
///
/// The exact token is preferred; when a width or capability differs between the two controls, the
/// title alone still identifies the column, so a host that changed a width after attaching keeps
/// its visibility wiring.
fn resolve_column_token(
    token: &str,
    tokens: &HashMap<String, usize>,
    titles: &HashMap<String, usize>,
) -> Option<usize> {
    tokens
        .get(token)
        .copied()
        .or_else(|| ColumnSpec::from_token(token).and_then(|spec| titles.get(&spec.title).copied()))
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
            filter_expr: FilterExpr::MatchAll,
            window_cache: None,
            column_prefix_cache: RefCell::new(None),
            column_geometry_revision: 0,
            column_prefix_rebuilds: Cell::new(0),
            header_text_cache: RefCell::new(HeaderTextCache::default()),
            header_text_revision: 0,
            data_generation: Arc::new(AtomicU64::new(0)),
            data_is_observable: false,
            selection: None,
            selection_mode: crate::widget::view_widgets::list_view::SelectionMode::Single,
            selection_behavior: DataGridSelectionBehavior::Cells,
            selected_cells: BTreeSet::new(),
            selection_anchor: None,
            current_cell: None,
            all_selected: false,
            sort_roles: std::collections::HashMap::new(),
            sort_indicator_visible: true,
            column_widths: std::collections::HashMap::new(),
            resizing_column: None,
            resize_start_x: 0,
            resize_start_width: 0,
            header_labels: Vec::new(),
            header_columns: Vec::new(),
            header_filter_visible: false,
            pagination: None,
            pagination_applied_page: 0,
            filter_row: None,
            selection_changed: Signal1::new(),
            current_changed: Signal1::new(),
            cell_clicked: Signal1::new(),
            cell_double_clicked: Signal1::new(),
            column_resized: Signal1::new(),
            hovered_cell: None,
            visible_window_changed: Signal1::new(),
            full_model_projection: false,
            full_projection_cache: None,
            index_projection_cache: None,
            full_window_cache: None,
            virtual_window_cache: None,
            column_width_mode: ColumnWidthMode::Fixed,
            alternating_rows: false,
            edit_triggers: EditTriggers::NONE,
            show_scrollbars: false,
            header_clicked: Signal1::new(),
            sort_changed: Signal1::new(),
            edit_requested: Signal1::new(),
            context_menu_requested: Signal1::new(),
            filter_changed: Signal1::new(),
            page_applied: Signal1::new(),
            hidden_columns: BTreeSet::new(),
            column_chooser: None,
            column_order: Vec::new(),
            cell_spans: Vec::new(),
            extra_header_rows: Vec::new(),
            cell_tooltip: None,
            cell_delegate: None,
        }
    }

    /// Sets the column titles the grid paints, without binding a header control.
    pub fn set_header_labels(&mut self, labels: &[&str]) {
        self.header_labels = labels.iter().map(|label| (*label).to_string()).collect();
        self.header_text_revision = self.header_text_revision.wrapping_add(1);
        self.base.request_redraw();
    }

    /// The column specs contributed by the last [`Self::set_header`], or empty.
    pub fn header_columns(&self) -> &[ColumnSpec] {
        &self.header_columns
    }

    /// Adopts `header`'s columns and presentation settings.
    ///
    /// The header is copied, not borrowed: a widget cannot hold a reference to another widget, and
    /// a table must keep painting correctly whether or not the host that installed the header is
    /// still alive. What carries over is everything the header is authoritative for — titles,
    /// widths, sort arrows, filter affordances and the resize mode — and what does **not** carry
    /// over is the row data's sorting, which this grid derives from the same indicator rules it
    /// always used.
    pub fn set_header(&mut self, header: &GridViewHeader) {
        self.header_columns = header.columns().to_vec();
        self.header_labels = header.columns().iter().map(|spec| spec.title.clone()).collect();
        self.header_filter_visible = header.filter_icon_visible();
        self.sort_indicator_visible = header.sort_indicator_visible();
        // The header's `Fixed` means "each section fixed at its own spec width"; the grid's
        // `Fixed` means "every column the same width". Per-column widths can only be expressed by
        // `Interactive`, so that is what a fixed header maps onto.
        self.column_width_mode = match header.resize_mode() {
            ColumnWidthMode::Fixed => ColumnWidthMode::Interactive,
            mode => mode,
        };
        self.column_widths.clear();
        self.sort_specs.clear();
        for (column, spec) in self.header_columns.iter().enumerate() {
            // Every spec width is stored, not only the resizable ones: `resizable` gates the drag
            // handle, while the width is what the header actually draws.
            self.column_widths.insert(column, spec.width.max(1));
            if let Some(order) = header.sort_indicator(column) {
                self.sort_specs.push(SortSpec { column, descending: order.is_descending() });
            }
        }
        self.clear_cache();
        self.invalidate_projection_cache();
        self.column_geometry_revision = self.column_geometry_revision.wrapping_add(1);
        self.header_text_revision = self.header_text_revision.wrapping_add(1);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Binds `pagination` to this grid.
    ///
    /// Binding does three things and each one is needed for the pair to be usable: the bar learns
    /// the grid's row count, the grid adopts the bar's page size and current page, and the bar's
    /// `page_changed` stream is subscribed so a page click reaches [`Self::poll_pagination`].
    ///
    /// The bar's page size is authoritative at bind time. Changing it afterwards needs a second
    /// `set_pagination`, because a widget cannot hold a borrow of another and so cannot re-read a
    /// value it was never signalled about.
    pub fn set_pagination(&mut self, pagination: &mut Pagination) {
        pagination.set_total(self.row_count());
        let page_size = pagination.page_size().max(1);
        let page = pagination.page();
        let shared = Arc::new(AtomicU64::new(page as u64));
        let slot = Arc::clone(&shared);
        let connection = pagination.page_changed.connect(move |page| {
            slot.store(*page as u64, Ordering::Relaxed);
        });
        self.pagination =
            Some(PaginationBinding { page: shared, page_size, _connection: connection });
        self.scroll_to_page(page);
        self.pagination_applied_page = page;
    }

    /// Applies a page the bound bar reported since the last poll.
    ///
    /// Called from `draw` and `handle_event`, so a click on the bar is picked up on the next frame
    /// or the next interaction without the host forwarding it. Returns the page that was applied.
    pub fn poll_pagination(&mut self) -> Option<usize> {
        let page = self.pagination.as_ref()?.page.load(Ordering::Relaxed) as usize;
        if page == self.pagination_applied_page {
            return None;
        }
        self.apply_page(page);
        Some(page)
    }

    /// Applies `page` to the scroll position and emits `page_applied`.
    ///
    /// Returns whether the visible rows moved. The signal fires even when the page was already
    /// current: a caller that asked to go to page 3 wants the acknowledgement, not silence.
    pub fn apply_page(&mut self, page: usize) -> bool {
        let moved = self.scroll_to_page(page);
        self.pagination_applied_page = page;
        self.page_applied.emit(page);
        moved
    }

    /// Scrolls so `page`'s first row is at the top, without emitting.
    fn scroll_to_page(&mut self, page: usize) -> bool {
        let page_size = self
            .pagination
            .as_ref()
            .map_or_else(|| self.visible_row_capacity().max(1), |binding| binding.page_size.max(1));
        let first_row = page.saturating_mul(page_size);
        let clamped = first_row.min(self.row_count().saturating_sub(1));
        if self.scroll_row == clamped {
            return false;
        }
        self.set_scroll_row(clamped);
        true
    }

    /// Pushes the grid's current page and row count back into `pagination`.
    ///
    /// This is the other half of the binding: `set_pagination` is bar-to-grid, this is
    /// grid-to-bar. Returns whether the bar's page changed.
    pub fn sync_pagination(&mut self, pagination: &mut Pagination) -> bool {
        let Some(binding) = self.pagination.as_ref() else {
            return false;
        };
        let page_size = binding.page_size.max(1);
        let page = self.scroll_row / page_size;
        pagination.set_total(self.row_count());
        let before = pagination.page();
        pagination.set_page(page);
        binding.page.store(page as u64, Ordering::Relaxed);
        self.pagination_applied_page = page;
        before != page
    }

    /// The bound page and page size, when a pagination bar is bound.
    pub fn pagination_state(&self) -> Option<(usize, usize)> {
        self.pagination.as_ref().map(|binding| (self.pagination_applied_page, binding.page_size))
    }

    // ── FilterRow wiring (chapter 3, gap 2) ──

    /// Follows `row`'s `filter_changed` stream (chapter 3, gap 2).
    ///
    /// A filter row publishes complete condition lists — a commit, a clear and `filter_cleared`'s
    /// empty list all arrive on `filter_changed` — so the link parks the newest list and
    /// [`Self::poll_filter_row`] installs it as one conjunction, the same semantics
    /// [`Self::set_filters`] has. Attaching makes the row authoritative: its current conditions
    /// replace the grid's filter immediately, and every later report does the same. Conditions
    /// address source columns exactly as the row reports them; the grid does not remap them.
    ///
    /// Programmatic filtering is untouched: [`Self::set_filters`] and [`Self::set_filter_expr`]
    /// keep their behaviour, and a grid with no row attached never polls. Attaching replaces any
    /// previous row, so calling this twice leaves one subscription rather than two.
    pub fn attach_filter_row(&mut self, row: &FilterRow) {
        let conditions = Arc::new(Mutex::new(None));
        let sink = Arc::clone(&conditions);
        let scope = ConnectionScope::new();
        let redraw = self.base.redraw_requested_signal().clone();
        row.filter_changed.connect_scoped(&scope, move |reported| {
            *sink.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) =
                Some((*reported).clone());
            // A row edit redraws the grid, so the poll that applies it happens next frame rather
            // than on whatever event arrives afterwards.
            redraw.emit();
        });
        self.filter_row = Some(FilterBinding { conditions, _scope: scope });
        // Adopt the row's current state now, so attaching does not wait for the next edit.
        self.apply_filter_row_conditions(row.conditions().to_vec());
    }

    /// Stops following the attached row; the last applied filter stays.
    pub fn detach_filter_row(&mut self) {
        if self.filter_row.take().is_some() {
            self.base.request_redraw();
        }
    }

    /// Whether a [`FilterRow`] is attached.
    pub fn is_filter_row_attached(&self) -> bool {
        self.filter_row.is_some()
    }

    /// Applies the condition list the attached row published since the last poll.
    ///
    /// Called from `draw` and `handle_event`, so a row edit is reflected by the next frame or
    /// interaction without the host forwarding it. Returns whether a report was applied.
    pub fn poll_filter_row(&mut self) -> bool {
        let reported = {
            let Some(binding) = self.filter_row.as_ref() else {
                return false;
            };
            binding.conditions.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).take()
        };
        match reported {
            Some(conditions) => {
                self.apply_filter_row_conditions(conditions);
                true
            }
            None => false,
        }
    }

    /// Installs `conditions` as one conjunction and refreshes the projection.
    ///
    /// The flat list is mirrored into `filters` so `summary_text` and the `filters` property report
    /// the grid as filtered, then installed as the tree `apply_filter_sort` evaluates. Going
    /// through [`Self::set_filter_expr`] is what invalidates the projection caches and announces
    /// the change on `filter_changed`.
    fn apply_filter_row_conditions(&mut self, conditions: Vec<FilterCondition>) {
        self.filters = conditions
            .iter()
            .map(|condition| ColumnFilter {
                column: condition.column,
                query: condition.operand.clone(),
            })
            .collect();
        self.set_filter_expr(FilterExpr::from_conditions(conditions));
    }

    /// A one-line overview of the grid's data state.
    ///
    /// Row count, selection count and whether a filter is active — the three facts a status strip
    /// below a table needs, written once here rather than re-derived by every host.
    pub fn summary_text(&self) -> String {
        let rows = self.row_count();
        let mut text = format!("{rows} rows");
        let selected = self.selected_indexes().len();
        if selected > 0 {
            text.push_str(&format!(" \u{00b7} {selected} selected"));
        }
        if !self.filters.is_empty() {
            text.push_str(" \u{00b7} filtered");
        }
        text
    }

    /// Binds incremental source.
    pub fn set_data_source(&mut self, data_source: Arc<dyn IncrementalTableDataSource>) {
        self.data_source_connection_scope = ConnectionScope::new();
        let revision_nonzero = data_source.revision() > 0;
        let has_signal = if let Some(changed) = data_source.data_changed_signal() {
            let redraw = self.base.redraw_requested_signal().clone();
            let layout = self.base.layout_requested_signal().clone();
            let generation = Arc::clone(&self.data_generation);
            changed.connect_scoped(&self.data_source_connection_scope, move || {
                generation.fetch_add(1, Ordering::Relaxed);
                redraw.emit();
                layout.emit();
            });
            true
        } else {
            false
        };
        self.data_generation.fetch_add(1, Ordering::Relaxed);
        self.data_is_observable = revision_nonzero || has_signal;
        self.data_source = Some(data_source);
        self.scroll_row = 0;
        self.scroll_column = 0;
        self.clear_cache();
        self.invalidate_projection_cache();
        self.clear_selection_units();
        self.current_cell = None;
        self.normalize_projection_state();
        self.emit_visible_window_changed();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Clears incremental source.
    pub fn clear_data_source(&mut self) {
        self.data_source_connection_scope = ConnectionScope::new();
        self.data_source = None;
        self.data_is_observable = false;
        self.data_generation.fetch_add(1, Ordering::Relaxed);
        self.scroll_row = 0;
        self.scroll_column = 0;
        self.frozen_columns = 0;
        self.clear_cache();
        self.invalidate_projection_cache();
        self.clear_selection_units();
        self.current_cell = None;
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

    /// The projected `(row, column)` of the current selection, if any.
    pub fn selection(&self) -> Option<(usize, usize)> {
        self.selection
    }

    /// The selection's cardinality mode.
    pub fn selection_mode(&self) -> crate::widget::view_widgets::list_view::SelectionMode {
        self.selection_mode
    }

    /// Sets how many units a press may add to the selection.
    pub fn set_selection_mode(
        &mut self,
        mode: crate::widget::view_widgets::list_view::SelectionMode,
    ) {
        if self.selection_mode == mode {
            return;
        }
        self.selection_mode = mode;
        if mode == crate::widget::view_widgets::list_view::SelectionMode::None {
            self.clear_selection();
            return;
        }
        if mode == crate::widget::view_widgets::list_view::SelectionMode::Single
            && (self.selected_cells.len() > 1 || self.all_selected)
        {
            self.all_selected = false;
            let keep = self.selection.or_else(|| self.selected_cells.iter().next().copied());
            self.selected_cells.clear();
            if let Some((row, column)) = keep {
                self.push_selection_unit(row, column);
                self.selection = Some((row, column));
            }
            self.selection_changed.emit(());
        }
        self.base.request_redraw();
    }

    /// What a selection gesture selects.
    pub fn selection_behavior(&self) -> DataGridSelectionBehavior {
        self.selection_behavior
    }

    /// Sets what a selection gesture selects, clearing the old units.
    pub fn set_selection_behavior(&mut self, behavior: DataGridSelectionBehavior) {
        if self.selection_behavior == behavior {
            return;
        }
        self.selection_behavior = behavior;
        self.clear_selection_units();
        self.base.request_redraw();
    }

    /// The current navigation cell, if any.
    pub fn current_cell(&self) -> Option<(usize, usize)> {
        self.current_cell
    }

    /// Moves keyboard navigation to `(row, column)`, selecting it in single mode.
    pub fn set_current_cell(&mut self, row: usize, column: usize) -> bool {
        if row >= self.row_count() || column >= self.column_count() {
            return false;
        }
        self.current_cell = Some((row, column));
        self.current_changed.emit((row, column));
        if self.selection_mode == crate::widget::view_widgets::list_view::SelectionMode::Single {
            self.replace_selection(row, column);
        }
        self.scroll_to_cell(row, column);
        self.base.request_redraw();
        true
    }

    /// Moves the current cell, optionally extending the selection from its anchor.
    ///
    /// Keyboard navigation uses this so the arrow keys move a real current cell instead of only
    /// scrolling. The cell is scrolled into view with the minimum movement, which keeps the
    /// pointer/keyboard viewport behaviour the grid already had.
    pub fn move_current_cell(&mut self, row: usize, column: usize, extend: bool) -> bool {
        if row >= self.row_count() || column >= self.column_count() {
            return false;
        }
        if self.selection_mode == crate::widget::view_widgets::list_view::SelectionMode::None {
            self.current_cell = Some((row, column));
            self.current_changed.emit((row, column));
            self.scroll_to_cell(row, column);
            self.base.request_redraw();
            return true;
        }
        let anchor =
            self.selection_anchor.or(self.selection).or(self.current_cell).unwrap_or((row, column));
        self.current_cell = Some((row, column));
        if extend {
            self.all_selected = false;
            self.push_selection_range(anchor, (row, column));
            self.selection = Some((row, column));
            self.selection_changed.emit(());
        } else {
            self.replace_selection(row, column);
        }
        self.current_changed.emit((row, column));
        self.scroll_to_cell(row, column);
        self.base.request_redraw();
        true
    }

    /// The signal emitted when the selection changes.
    ///
    /// Exposed as accessors rather than `pub` fields: the capability table owns the published
    /// event names, and these names are not in it, so they stay Rust-level events. Query
    /// [`Self::selected_cells`] or [`Self::selected_indexes`] for the new units.
    pub fn selection_changed_signal(&self) -> &Signal1<()> {
        &self.selection_changed
    }

    /// The signal emitted with the current navigation cell.
    pub fn current_changed_signal(&self) -> &Signal1<(usize, usize)> {
        &self.current_changed
    }

    /// The signal emitted with a pressed cell.
    pub fn cell_clicked_signal(&self) -> &Signal1<(usize, usize)> {
        &self.cell_clicked
    }

    /// The signal emitted with a double-clicked cell.
    pub fn cell_double_clicked_signal(&self) -> &Signal1<(usize, usize)> {
        &self.cell_double_clicked
    }

    /// The signal emitted after a user resize with `(column, width)`.
    pub fn column_resized_signal(&self) -> &Signal1<(usize, u32)> {
        &self.column_resized
    }

    /// The selected units, in insertion order.
    ///
    /// The stored unit is the cell itself in `Cells` behaviour, `(row, 0)` in `Rows` behaviour and
    /// `(0, column)` in `Columns` behaviour (V-004): one accessor serves all three by handing back
    /// what the selection stores, and whether a whole row or column is covered is
    /// [`Self::selection_contains`]'s lookup. A `select_all` keeps no units, so it is expanded
    /// here; a caller that needs the distinct rows or columns should prefer
    /// [`Self::selected_indexes`].
    pub fn selected_cells(&self) -> Vec<(usize, usize)> {
        if self.all_selected {
            let columns = self.column_count();
            return (0..self.row_count())
                .flat_map(|row| (0..columns).map(move |column| (row, column)))
                .collect();
        }
        self.selected_cells.iter().copied().collect()
    }

    /// The distinct rows (`Cells`/`Rows`) or columns (`Columns`) in the selection.
    pub fn selected_indexes(&self) -> Vec<usize> {
        if self.all_selected {
            return match self.selection_behavior {
                DataGridSelectionBehavior::Columns => (0..self.column_count()).collect(),
                _ => (0..self.row_count()).collect(),
            };
        }
        match self.selection_behavior {
            DataGridSelectionBehavior::Columns => {
                let mut columns: Vec<usize> =
                    self.selected_cells.iter().map(|(_, column)| *column).collect();
                columns.dedup();
                columns
            }
            _ => {
                let mut rows: Vec<usize> =
                    self.selected_cells.iter().map(|(row, _)| *row).collect();
                rows.dedup();
                rows
            }
        }
    }

    /// Selects every unit of the current behaviour.
    pub fn select_all(&mut self) {
        if self.row_count() == 0 || self.column_count() == 0 {
            return;
        }
        self.all_selected = true;
        self.selected_cells.clear();
        self.selection = Some((0, 0));
        self.current_cell.get_or_insert((0, 0));
        self.selection_changed.emit(());
        self.base.request_redraw();
    }

    /// Clears the current selection.
    pub fn clear_selection(&mut self) {
        if self.selection.is_none() && self.selected_cells.is_empty() && !self.all_selected {
            return;
        }
        self.clear_selection_units();
        self.selection_changed.emit(());
        self.base.request_redraw();
    }

    fn clear_selection_units(&mut self) {
        self.selection = None;
        self.selected_cells.clear();
        self.all_selected = false;
        self.selection_anchor = None;
    }

    /// Whether `(row, column)` belongs to the painted selection.
    fn selection_contains(&self, row: usize, column: usize) -> bool {
        if self.all_selected {
            return true;
        }
        match self.selection_behavior {
            DataGridSelectionBehavior::Cells => self.selected_cells.contains(&(row, column)),
            DataGridSelectionBehavior::Rows => {
                // Row units are stored as `(row, 0)`, so a row query is an ordered-set lookup
                // rather than a scan of every selected cell (V-004).
                self.selected_cells.contains(&(row, 0))
            }
            DataGridSelectionBehavior::Columns => {
                // Column units are stored as `(0, column)`, the same convention (V-004).
                self.selected_cells.contains(&(0, column))
            }
        }
    }

    /// Adds the behaviour's unit for `(row, column)`.
    fn push_selection_unit(&mut self, row: usize, column: usize) {
        let unit = match self.selection_behavior {
            DataGridSelectionBehavior::Cells => (row, column),
            DataGridSelectionBehavior::Rows => (row, 0),
            DataGridSelectionBehavior::Columns => (0, column),
        };
        self.selected_cells.insert(unit);
    }

    /// Adds every unit between two cells.
    fn push_selection_range(&mut self, from: (usize, usize), to: (usize, usize)) {
        match self.selection_behavior {
            DataGridSelectionBehavior::Cells => {
                for row in from.0.min(to.0)..=from.0.max(to.0) {
                    for column in from.1.min(to.1)..=from.1.max(to.1) {
                        self.selected_cells.insert((row, column));
                    }
                }
            }
            DataGridSelectionBehavior::Rows => {
                for row in from.0.min(to.0)..=from.0.max(to.0) {
                    self.selected_cells.insert((row, 0));
                }
            }
            DataGridSelectionBehavior::Columns => {
                for column in from.1.min(to.1)..=from.1.max(to.1) {
                    self.selected_cells.insert((0, column));
                }
            }
        }
    }

    /// Replaces the selection with `(row, column)`'s unit.
    fn replace_selection(&mut self, row: usize, column: usize) {
        self.selected_cells.clear();
        self.all_selected = false;
        self.selection = Some((row, column));
        self.selection_anchor = Some((row, column));
        self.push_selection_unit(row, column);
        self.selection_changed.emit(());
    }

    /// Applies a press to the selection, honouring ctrl/shift in extended mode.
    fn apply_selection_click(&mut self, row: usize, column: usize, modifiers: u32) {
        use crate::shortcut::Modifiers;
        if self.selection_mode == crate::widget::view_widgets::list_view::SelectionMode::None {
            return;
        }
        let modifiers = Modifiers::from_event_bits(modifiers);
        let extended =
            self.selection_mode == crate::widget::view_widgets::list_view::SelectionMode::Extended;
        let shift = extended && modifiers.contains(Modifiers::SHIFT);
        let toggle = extended
            && (modifiers.contains(Modifiers::CTRL) || modifiers.contains(Modifiers::META));

        if shift {
            let anchor = self.selection_anchor.unwrap_or((row, column));
            if !self.all_selected {
                // Keep the anchor's unit and add the range, which is what shift means.
                self.push_selection_range(anchor, (row, column));
            }
            self.selection = Some((row, column));
            self.selection_changed.emit(());
        } else if toggle {
            self.all_selected = false;
            let unit = match self.selection_behavior {
                DataGridSelectionBehavior::Cells => (row, column),
                DataGridSelectionBehavior::Rows => (row, 0),
                DataGridSelectionBehavior::Columns => (0, column),
            };
            if !self.selected_cells.remove(&unit) {
                self.selected_cells.insert(unit);
            }
            self.selection = Some((row, column));
            self.selection_anchor = Some((row, column));
            self.selection_changed.emit(());
        } else {
            self.replace_selection(row, column);
        }
    }

    /// Scrolls the minimum amount that brings `(row, column)` into view.
    fn scroll_to_cell(&mut self, row: usize, column: usize) {
        let rows = self.visible_row_capacity().max(1);
        if row < self.scroll_row {
            self.set_scroll_row(row);
        } else if row >= self.scroll_row.saturating_add(rows) {
            self.set_scroll_row(row.saturating_add(1).saturating_sub(rows));
        }
        let columns = self.visible_column_capacity().max(1);
        if column < self.scroll_column {
            self.set_scroll_column(column);
        } else if column >= self.scroll_column.saturating_add(columns) {
            self.set_scroll_column(column.saturating_add(1).saturating_sub(columns));
        }
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

    /// Sets the fallback width every column uses when it has no override.
    ///
    /// This is the historical single-width setter, renamed from `set_column_width` so the
    /// per-column form can carry the name the issue asks for (V-07).
    pub fn set_default_column_width(&mut self, column_width: u32) {
        let next = column_width.max(1);
        if self.column_width == next {
            return;
        }
        self.column_width = next;
        self.column_geometry_revision = self.column_geometry_revision.wrapping_add(1);
        self.clear_cache();
        self.emit_visible_window_changed();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns the fallback column width, i.e. the width of a column without an override.
    pub fn default_column_width(&self) -> u32 {
        self.column_width
    }

    /// The width painted for `column` under the current sizing mode.
    ///
    /// The issue's `column_width(col)` name. It is the same answer as
    /// [`Self::column_width_at`], which remains as an explicit alias so existing callers keep
    /// compiling; the zero-argument getter is now [`Self::default_column_width`].
    pub fn column_width(&self, column: usize) -> u32 {
        self.column_width_at(column)
    }

    /// Sets one column's width and switches to interactive sizing, returning whether the
    /// column exists.
    ///
    /// The issue's `set_column_width(col, w)` name. It is the same operation as
    /// [`Self::set_column_width_at`] (which emits `column_resized`).
    pub fn set_column_width(&mut self, column: usize, width: u32) -> bool {
        self.set_column_width_at(column, width)
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

    // ── Hidden columns (V-014) ──

    /// Whether `column` is excluded from painting and hit-testing.
    pub fn is_column_hidden(&self, column: usize) -> bool {
        self.hidden_columns.contains(&column)
    }

    /// The hidden columns, in ascending order.
    pub fn hidden_columns(&self) -> Vec<usize> {
        self.hidden_columns.iter().copied().collect()
    }

    /// Replaces the hidden-column set.
    ///
    /// A hidden column keeps its absolute index (sort specs, filters and the data window all stay
    /// absolute) and contributes zero width, so the columns after it close the gap. The companion
    /// wiring that lets a `ColumnChooser` drive this is chapter 3's A32/A24 (V-014): this is the
    /// in-widget capability only.
    pub fn set_hidden_columns(&mut self, columns: Vec<usize>) {
        let available = self.column_count();
        let next: BTreeSet<usize> =
            columns.into_iter().filter(|column| *column < available).collect();
        if next == self.hidden_columns {
            return;
        }
        self.hidden_columns = next;
        self.column_geometry_revision = self.column_geometry_revision.wrapping_add(1);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Hides or shows `column`. Returns `false` for a column the source lacks.
    pub fn set_column_hidden(&mut self, column: usize, hidden: bool) -> bool {
        if column >= self.column_count() {
            return false;
        }
        let changed = if hidden {
            self.hidden_columns.insert(column)
        } else {
            self.hidden_columns.remove(&column)
        };
        if changed {
            self.column_geometry_revision = self.column_geometry_revision.wrapping_add(1);
            self.base.request_layout();
            self.base.request_redraw();
        }
        true
    }

    // ── ColumnChooser wiring (V-015) ──

    /// Follows `chooser`'s visibility and order signals (V-015).
    ///
    /// A widget cannot hold a borrow of another, so the chooser writes into a shared queue and
    /// [`Self::poll_column_chooser`] drains it from the paint and event paths. Attaching replaces
    /// any previous chooser, so calling this twice leaves one subscription rather than two.
    ///
    /// The column set both controls were configured with is captured at attach time: the chooser
    /// reports persisted tokens, and the grid needs their source indices. A token this grid does
    /// not know is ignored rather than guessed at, so a chooser covering more columns than the
    /// bound source cannot hide an unrelated column.
    pub fn attach_column_chooser(&mut self, chooser: &ColumnChooser) {
        let (token_map, title_map) = self.column_token_maps(chooser);
        let token_map = Arc::new(token_map);
        let title_map = Arc::new(title_map);
        let visibility = Arc::new(Mutex::new(Vec::new()));
        let order = Arc::new(Mutex::new(Vec::new()));
        let scope = ConnectionScope::new();

        let visibility_sink = Arc::clone(&visibility);
        let lookup_tokens = Arc::clone(&token_map);
        let lookup_titles = Arc::clone(&title_map);
        chooser.column_visibility_changed.connect_scoped(&scope, move |change| {
            let (token, visible) = &*change;
            let Some(column) = resolve_column_token(token, &lookup_tokens, &lookup_titles) else {
                return;
            };
            let mut guard = visibility_sink.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            guard.push((column, *visible));
        });

        let order_sink = Arc::clone(&order);
        let lookup_tokens = Arc::clone(&token_map);
        let lookup_titles = Arc::clone(&title_map);
        chooser.column_order_changed.connect_scoped(&scope, move |reported| {
            let mapped: Vec<usize> = reported
                .iter()
                .filter_map(|token| resolve_column_token(token, &lookup_tokens, &lookup_titles))
                .collect();
            let mut guard = order_sink.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            *guard = mapped;
        });

        self.column_chooser = Some(ColumnChooserBinding { visibility, order, _scope: scope });
        self.base.request_redraw();
    }

    /// Stops following the attached chooser.
    pub fn detach_column_chooser(&mut self) {
        if self.column_chooser.take().is_some() {
            self.base.request_redraw();
        }
    }

    /// Whether a [`ColumnChooser`] is attached.
    pub fn is_column_chooser_attached(&self) -> bool {
        self.column_chooser.is_some()
    }

    /// Applies every chooser change reported since the last poll.
    ///
    /// Called from `draw` and `handle_event`, so a chooser click is reflected by the next frame
    /// without the host forwarding it. Returns whether any grid state changed.
    pub fn poll_column_chooser(&mut self) -> bool {
        let Some(binding) = self.column_chooser.as_ref() else {
            return false;
        };
        let changes: Vec<(usize, bool)> = {
            let mut guard =
                binding.visibility.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            core::mem::take(&mut *guard)
        };
        let reported_order: Vec<usize> =
            binding.order.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone();

        let mut changed = false;
        for (column, visible) in changes {
            changed |= self.set_column_hidden(column, !visible);
        }
        if !reported_order.is_empty() && reported_order != self.column_order {
            self.column_order = reported_order;
            self.base.request_redraw();
            changed = true;
        }
        changed
    }

    /// The column order the attached chooser last reported, as source column indices.
    ///
    /// An empty report returns the natural order. The grid's paint and hit-test paths address
    /// columns by absolute source index, so this is the reported order as state; it does not
    /// restate the paint geometry.
    pub fn column_display_order(&self) -> Vec<usize> {
        if self.column_order.is_empty() {
            (0..self.column_count()).collect()
        } else {
            self.column_order.clone()
        }
    }

    /// The token and title maps used to resolve a chooser's tokens.
    fn column_token_maps(
        &self,
        chooser: &ColumnChooser,
    ) -> (HashMap<String, usize>, HashMap<String, usize>) {
        let mut tokens = HashMap::new();
        let mut titles = HashMap::new();
        // A bound header is authoritative because it describes the same source columns this grid
        // paints. Without one, the chooser's own column list is the only description available.
        let specs: Vec<&ColumnSpec> = if self.header_columns.is_empty() {
            chooser.columns().iter().collect()
        } else {
            self.header_columns.iter().collect()
        };
        for (index, spec) in specs.into_iter().enumerate() {
            tokens.insert(spec.to_token(), index);
            titles.entry(spec.title.clone()).or_insert(index);
        }
        (tokens, titles)
    }

    // ── Merged cells (V-014) ──

    /// The configured spans, in insertion order.
    pub fn cell_spans(&self) -> &[CellSpan] {
        &self.cell_spans
    }

    /// Merges `row_span` x `column_span` cells anchored at `(row, column)`.
    ///
    /// The span is expressed in projected coordinates (the same space selection uses). Returns
    /// `false` when the anchor is outside the current row/column counts.
    pub fn set_cell_span(
        &mut self,
        row: usize,
        column: usize,
        row_span: usize,
        column_span: usize,
    ) -> bool {
        if row >= self.row_count() || column >= self.column_count() {
            return false;
        }
        let span = CellSpan::new(row, column, row_span, column_span);
        // One span per origin: re-setting the same anchor replaces it rather than stacking two
        // answers for the same cell.
        if let Some(existing) = self
            .cell_spans
            .iter_mut()
            .find(|existing| existing.row == row && existing.column == column)
        {
            *existing = span;
        } else {
            self.cell_spans.push(span);
        }
        self.base.request_redraw();
        true
    }

    /// Removes every merged cell.
    pub fn clear_cell_spans(&mut self) {
        if self.cell_spans.is_empty() {
            return;
        }
        self.cell_spans.clear();
        self.base.request_redraw();
    }

    /// The span covering `(row, column)`, when one does.
    pub fn cell_span(&self, row: usize, column: usize) -> Option<CellSpan> {
        self.span_covering(row, column)
    }

    fn span_covering(&self, row: usize, column: usize) -> Option<CellSpan> {
        self.cell_spans.iter().copied().find(|span| span.covers(row, column))
    }

    // ── Multi-level header (V-014) ──

    /// The additional header rows painted above the title row.
    pub fn extra_header_rows(&self) -> &[Vec<String>] {
        &self.extra_header_rows
    }

    /// Sets additional header rows above the title row.
    ///
    /// Row `0` is the topmost level and the last row is adjacent to the titles. An empty list is
    /// the historical single-row header. The cells box shrinks by one title strip per row.
    pub fn set_extra_header_rows(&mut self, rows: Vec<Vec<String>>) {
        let rows: Vec<Vec<String>> =
            rows.into_iter().filter(|row| row.iter().any(|label| !label.is_empty())).collect();
        if rows == self.extra_header_rows {
            return;
        }
        self.extra_header_rows = rows;
        self.header_text_revision = self.header_text_revision.wrapping_add(1);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The total header strip height: the title row plus every extra level.
    fn header_strip_height(&self) -> u32 {
        GRID_HEADER_HEIGHT.saturating_mul(1 + self.extra_header_rows.len() as u32)
    }

    // ── Cell tooltip (V-014) ──

    /// The tooltip text for `(row, column)`, from the installed source.
    pub fn cell_tooltip(&self, row: usize, column: usize) -> Option<String> {
        self.cell_tooltip.as_ref().and_then(|source| source(row, column))
    }

    /// Installs the per-cell tooltip source. The callback runs only for the hovered cell.
    pub fn set_cell_tooltip(
        &mut self,
        source: Arc<dyn Fn(usize, usize) -> Option<String> + Send + Sync>,
    ) {
        self.cell_tooltip = Some(source);
        self.base.request_redraw();
    }

    /// Removes the tooltip source.
    pub fn clear_cell_tooltip(&mut self) {
        if self.cell_tooltip.take().is_some() {
            self.base.request_redraw();
        }
    }

    // ── Cell delegate (V-014) ──

    /// Installs a per-cell paint delegate. Without one, cells draw directly (the default).
    pub fn set_cell_delegate(&mut self, delegate: Arc<dyn DataGridCellDelegate>) {
        self.cell_delegate = Some(delegate);
        self.base.request_redraw();
    }

    /// Removes the paint delegate.
    pub fn clear_cell_delegate(&mut self) {
        if self.cell_delegate.take().is_some() {
            self.base.request_redraw();
        }
    }

    /// Whether a paint delegate is installed.
    pub fn has_cell_delegate(&self) -> bool {
        self.cell_delegate.is_some()
    }

    /// Whether the filter and sort projection covers the whole model.
    pub fn full_model_projection(&self) -> bool {
        self.full_model_projection
    }

    /// Chooses between the virtualized projection (default) and whole-model materialisation.
    ///
    /// # Default: virtualized
    ///
    /// The default path projects the model as an **index**: filtering drops source rows and
    /// sorting reorders their indices, then only the visible window is fetched. A 10k-row grid
    /// therefore no longer holds a 10k-row copy of its cells, and a scroll fetches one window
    /// (V-001/V-002). The visible order is the same as the materialised projection.
    ///
    /// `true` restores the historical whole-model fetch. It is kept as an explicit option for a
    /// source that is cheap to read once and expensive to re-read per window; it materialises
    /// every row and cell, which is why it is no longer the default.
    pub fn set_full_model_projection(&mut self, enabled: bool) {
        if self.full_model_projection == enabled {
            return;
        }
        self.full_model_projection = enabled;
        self.invalidate_projection_cache();
        self.clear_cache();
        self.base.request_redraw();
    }

    /// Drops every derived projection and window, keeping only source row identity.
    ///
    /// One place for the invalidation so a new cache cannot be forgotten at one of the sort,
    /// filter or data-change call sites.
    fn invalidate_projection_cache(&mut self) {
        self.full_projection_cache = None;
        self.index_projection_cache = None;
        self.full_window_cache = None;
        self.virtual_window_cache = None;
    }

    /// How columns are sized.
    pub fn column_width_mode(&self) -> ColumnWidthMode {
        self.column_width_mode
    }

    /// How columns are sized (`column_width_mode`'s grid-family alias).
    pub fn section_resize_mode(&self) -> ColumnWidthMode {
        self.column_width_mode
    }

    /// Sets the column sizing mode (`set_column_width_mode`'s grid-family alias).
    pub fn set_section_resize_mode(&mut self, mode: ColumnWidthMode) {
        self.set_column_width_mode(mode);
    }

    /// Sets the column sizing mode.
    pub fn set_column_width_mode(&mut self, mode: ColumnWidthMode) {
        if self.column_width_mode == mode {
            return;
        }
        self.column_width_mode = mode;
        self.column_geometry_revision = self.column_geometry_revision.wrapping_add(1);
        self.clear_cache();
        self.base.request_redraw();
    }

    /// Whether odd rows are painted with the alternating wash.
    pub fn alternating_rows(&self) -> bool {
        self.alternating_rows
    }

    /// Enables or disables the alternating row wash.
    pub fn set_alternating_rows(&mut self, alternating: bool) {
        if self.alternating_rows == alternating {
            return;
        }
        self.alternating_rows = alternating;
        self.base.request_redraw();
    }

    /// The gestures that start an edit.
    pub fn edit_triggers(&self) -> EditTriggers {
        self.edit_triggers
    }

    /// Sets the gestures that start an edit.
    pub fn set_edit_triggers(&mut self, triggers: EditTriggers) {
        if self.edit_triggers == triggers {
            return;
        }
        self.edit_triggers = triggers;
    }

    /// Whether scroll-bar thumbs are painted.
    pub fn show_scrollbars(&self) -> bool {
        self.show_scrollbars
    }

    /// Enables or disables the scroll-bar thumbs.
    pub fn set_show_scrollbars(&mut self, show: bool) {
        if self.show_scrollbars == show {
            return;
        }
        self.show_scrollbars = show;
        self.base.request_redraw();
    }

    /// The width a column falls back to under the current mode.
    ///
    /// Per-column overrides live in [`Self::column_width_at`]; this is the nominal width used for
    /// capacity estimates and for columns that have no override.
    fn effective_column_width(&self) -> u32 {
        match self.column_width_mode {
            ColumnWidthMode::Fixed | ColumnWidthMode::Interactive => self.column_width,
            ColumnWidthMode::Stretch => {
                let count = self.column_count().max(1) as u32;
                (self.cells_box().width / count).max(1)
            }
            // Only the final column stretches, so the nominal width is the honest estimate for
            // the rest; the capacity helpers that read this do not need the last column's share.
            ColumnWidthMode::StretchLast => self.column_width,
        }
    }

    /// Replaces sort spec list in priority order.
    pub fn set_sort_specs(&mut self, sort_specs: Vec<SortSpec>) {
        self.sort_specs = sort_specs;
        self.invalidate_projection_cache();
        self.clear_cache();
        self.base.request_redraw();
    }

    /// Returns active sort specs.
    pub fn sort_specs(&self) -> &[SortSpec] {
        &self.sort_specs
    }

    /// The role `column` is sorted with, when one was set.
    pub fn sort_role(&self, column: usize) -> Option<DataRole> {
        self.sort_roles.get(&column).copied()
    }

    /// Sets the role a column's sort keys are read with.
    ///
    /// The display text is the default source of a sort key. A model that stores a timestamp or a
    /// number in another role can expose it to the sorter without changing what the cell paints.
    pub fn set_sort_role(&mut self, column: usize, role: DataRole) {
        if self.sort_roles.insert(column, role) == Some(role) {
            return;
        }
        self.invalidate_projection_cache();
        self.clear_cache();
        self.base.request_redraw();
    }

    /// Removes a column's sort-role override, returning to the display text.
    pub fn clear_sort_role(&mut self, column: usize) {
        if self.sort_roles.remove(&column).is_some() {
            self.invalidate_projection_cache();
            self.clear_cache();
            self.base.request_redraw();
        }
    }

    /// Whether the header paints a sort arrow.
    pub fn sort_indicator_visible(&self) -> bool {
        self.sort_indicator_visible
    }

    /// Enables or disables the header's sort arrow.
    pub fn set_sort_indicator_visible(&mut self, visible: bool) {
        if self.sort_indicator_visible == visible {
            return;
        }
        self.sort_indicator_visible = visible;
        self.base.request_redraw();
    }

    /// The width painted for `column` under the current mode.
    ///
    /// The historical single `column_width` remains [`Self::column_width`]; this is its per-column
    /// counterpart. `Fixed` deliberately ignores the overrides, `Stretch` divides the cells box and
    /// `Interactive` honours an explicit width before falling back to the fixed one.
    pub fn column_width_at(&self, column: usize) -> u32 {
        // A hidden column consumes no space; every offset and hit test below reads this one
        // answer, so hiding is a width of zero rather than a second geometry path (V-014).
        if self.hidden_columns.contains(&column) {
            return 0;
        }
        match self.column_width_mode {
            ColumnWidthMode::Fixed => self.column_width,
            ColumnWidthMode::Interactive => {
                self.column_widths.get(&column).copied().unwrap_or(self.column_width)
            }
            ColumnWidthMode::Stretch => {
                // Hidden columns do not take a share: the visible ones divide the whole box.
                let visible = self.column_count().saturating_sub(self.hidden_columns.len()).max(1);
                (self.cells_box().width / visible as u32).max(1)
            }
            ColumnWidthMode::StretchLast => {
                let count = self.column_count().max(1);
                if column + 1 != count {
                    return self.column_widths.get(&column).copied().unwrap_or(self.column_width);
                }
                let used: u32 = (0..count - 1)
                    .filter(|index| !self.hidden_columns.contains(index))
                    .map(|index| {
                        self.column_widths.get(&index).copied().unwrap_or(self.column_width)
                    })
                    .sum();
                self.cells_box().width.saturating_sub(used).max(1)
            }
        }
    }

    /// Sets one column's width and switches to interactive sizing.
    ///
    /// The mode change is part of the request: an explicit per-column width that `Fixed` ignored
    /// would be a setter with no effect. Returns `false` for a column the source does not have.
    pub fn set_column_width_at(&mut self, column: usize, width: u32) -> bool {
        if !self.store_column_width(column, width) {
            return false;
        }
        self.column_resized.emit((column, self.column_width_at(column)));
        true
    }

    /// Stores a per-column width without emitting the user-resize signal.
    fn store_column_width(&mut self, column: usize, width: u32) -> bool {
        if column >= self.column_count() {
            return false;
        }
        let width = width.max(1);
        self.column_width_mode = ColumnWidthMode::Interactive;
        self.column_geometry_revision = self.column_geometry_revision.wrapping_add(1);
        if self.column_widths.insert(column, width) == Some(width) {
            return true;
        }
        self.clear_cache();
        self.emit_visible_window_changed();
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    /// Sizes `column` to its widest painted value plus a readable inset.
    ///
    /// The widest cell is measured with the same font the painter uses, so the result is the width
    /// at which the value stops being elided. A source whose rows are expensive to read can still
    /// use [`Self::set_column_width_at`] with its own answer.
    pub fn resize_column_to_contents(&mut self, column: usize) -> bool {
        let Some(source) = self.data_source.clone() else {
            return false;
        };
        if column >= source.column_count() {
            return false;
        }
        let font = effective_font(self.style());
        let mut widest = 0u32;
        let mut text = String::new();
        for row in 0..source.row_count() {
            text.clear();
            if source.fill_cell(row, column, &mut text) {
                widest = widest.max(crate::widget::metrics::estimate_text_width(&text, font, 1.0));
            }
        }
        self.set_column_width_at(column, widest.saturating_add(16).max(24))
    }

    /// Whether a header resize handle lies at `point`.
    fn resize_handle_at(&self, point: Point) -> Option<usize> {
        let band = self.header_band();
        if !band.contains_point(point) {
            return None;
        }
        let cells = self.cells_box();
        for column in 0..self.column_count() {
            let x = cells.x + self.column_view_x(column);
            let right = x + self.column_width_at(column) as i32;
            if right <= band.x {
                continue;
            }
            if x >= band.x + band.width as i32 {
                break;
            }
            if (point.x - right).abs() <= 4 {
                return Some(column);
            }
        }
        None
    }

    /// The x of `column`'s left edge relative to the cells box's left edge.
    fn column_view_x(&self, column: usize) -> i32 {
        let frozen = self.frozen_columns.min(self.column_count());
        let frozen_width = self.column_prefix_width(frozen);
        if column < frozen {
            self.column_prefix_width(column)
        } else {
            // The scrolling band starts after the pinned prefix, not at the leading edge: a
            // scrolling column must never paint over the frozen one it sits behind.
            let start = self.scroll_column.max(frozen);
            frozen_width + (self.column_prefix_width(column) - self.column_prefix_width(start))
        }
    }

    /// The summed width of columns `0..upto`, read from a cached prefix table.
    ///
    /// The table is rebuilt only when a width input changes (mode, default width, an explicit
    /// per-column width or the column count); a frame therefore pays O(columns) once instead of
    /// summing per cell (F-P01).
    fn column_prefix_width(&self, upto: usize) -> i32 {
        let count = self.column_count();
        let cells_width = self.cells_box().width;
        let mut cache = self.column_prefix_cache.borrow_mut();
        let stale = match cache.as_ref() {
            Some(entry) => {
                entry.revision != self.column_geometry_revision
                    || entry.column_count != count
                    || entry.cells_width != cells_width
                    || entry.mode != self.column_width_mode
                    || entry.default_width != self.column_width
            }
            None => true,
        };
        if stale {
            *cache = Some(self.build_column_prefixes(count, cells_width));
            self.column_prefix_rebuilds.set(self.column_prefix_rebuilds.get().saturating_add(1));
        }
        let entry = cache.as_ref().expect("the prefix table was just built");
        if upto <= entry.column_count {
            return entry.prefix.get(upto).copied().unwrap_or(0);
        }
        // Deliberately not clamped to `column_count()`: `cell_rect` is a pure function of its
        // indices, and a grid with no source yet still has to answer where column 2 would be.
        // Only the (rare) beyond-the-model tail is summed on demand.
        let base = entry.prefix.get(entry.column_count).copied().unwrap_or(0);
        base + (entry.column_count..upto)
            .map(|column| self.column_width_at(column) as i32)
            .sum::<i32>()
    }

    /// Builds the prefix table for the current widths.
    fn build_column_prefixes(&self, count: usize, cells_width: u32) -> ColumnPrefixCache {
        let mut prefix = Vec::with_capacity(count.saturating_add(1));
        let mut sum = 0i32;
        prefix.push(0);
        for column in 0..count {
            sum = sum.saturating_add(self.column_width_at(column) as i32);
            prefix.push(sum);
        }
        ColumnPrefixCache {
            revision: self.column_geometry_revision,
            column_count: count,
            cells_width,
            mode: self.column_width_mode,
            default_width: self.column_width,
            prefix,
        }
    }

    /// The column at `dx` pixels into the cells box.
    fn column_at_dx(&self, dx: i32) -> Option<usize> {
        if dx < 0 {
            return None;
        }
        let frozen = self.frozen_columns.min(self.column_count());
        let frozen_width = self.column_prefix_width(frozen);
        let (mut column, mut x) = if dx < frozen_width {
            (0, 0)
        } else {
            // The band begins at the frozen prefix; its first column is the further of the
            // scroll position and the frozen count.
            (self.scroll_column.max(frozen), frozen_width)
        };
        while column < self.column_count() {
            let width = self.column_width_at(column) as i32;
            if dx < x + width {
                return Some(column);
            }
            x += width;
            column += 1;
        }
        None
    }

    /// Replaces the filter list with a flat conjunction of `contains` conditions.
    ///
    /// Kept as the simple spelling: a caller that wants "contains this text in this
    /// column" does not have to build a tree. Delegates to
    /// [`Self::set_filter_expr`] so the two spellings cannot diverge — the list is
    /// converted to one `And` of predicates, which is the semantics this method
    /// always had (an implicit AND, `apply_filter_sort`).
    pub fn set_filters(&mut self, filters: Vec<ColumnFilter>) {
        let expr = FilterExpr::from_conditions(
            filters
                .iter()
                .map(|filter| FilterCondition::contains(filter.column, filter.query.clone()))
                .collect(),
        );
        self.set_filter_expr(expr);
        // Kept in step so `filters()` reports what was set rather than what the
        // tree happens to flatten to (a caller may have set an `Or`/`Not` tree, in
        // which case the list is the predicates in reading order).
        self.filters = filters;
    }

    /// Returns the active filters as a flat list.
    ///
    /// The conditions of the **tree**, in reading order, so a grid whose filter was
    /// set through [`Self::set_filter_expr`] still reports them. Each condition's
    /// operator is dropped, because `ColumnFilter` has no operator field and
    /// inventing one here would be a second definition of the same thing.
    pub fn filters(&self) -> Vec<ColumnFilter> {
        self.filter_expr
            .conditions()
            .iter()
            .map(|condition| ColumnFilter {
                column: condition.column,
                query: condition.operand.clone(),
            })
            .collect()
    }

    /// Replaces the filter with a recursive expression.
    ///
    /// This is the model a query-builder UI produces (nested AND/OR, non-text
    /// operators), so "code sets a filter" and "a user builds one" share one
    /// evaluation path (principle #54).
    pub fn set_filter_expr(&mut self, filter_expr: FilterExpr) {
        self.filter_expr = filter_expr.clone();
        self.invalidate_projection_cache();
        self.clear_cache();
        self.filter_changed.emit(filter_expr);
        self.base.request_redraw();
    }

    /// Returns the active filter expression.
    pub fn filter_expr(&self) -> &FilterExpr {
        &self.filter_expr
    }

    /// Returns `(row_start, row_len, col_start, col_len)` for visible+overscan window.
    ///
    /// When the grid has frozen leading columns, the fetch starts at column `0` and the
    /// returned `col_len` reaches from there through the scrolling window. Frozen columns are
    /// painted outside the scroll offset (`cell_rect`), so they cannot be omitted from the
    /// fetched matrix: the draw pass would then have no values to repaint over the scrolling
    /// cells. With no frozen columns the window is exactly the scrolling window it always was.
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
        // A frozen prefix is part of what the draw pass needs even when it lies left of the
        // scrolling window. Reaching all the way back to column 0 also keeps the fetched
        // matrix contiguous, which is what `fetch_window` returns and what the cached window
        // is keyed on.
        let col_start = if self.frozen_columns > 0 { 0 } else { col_start };
        let col_len = col_end.saturating_sub(col_start);

        (row_start, row_end.saturating_sub(row_start), col_start, col_len)
    }

    /// The projected row/column the fetch window starts at, i.e. the scroll position pulled back by
    /// the overscan.
    ///
    /// # Why this exists (D09-VIEW-01)
    ///
    /// `visible_window()` fetches from `row_start = scroll - overscan`, so the matrix the source
    /// returns begins *before* the viewport. The draw pass used to enumerate that matrix from its
    /// local row `0`, which painted the leading overscan rows as if they were the first visible
    /// ones; and `cell_rect` already places a cell from its own projected column, so the local
    /// column reached it un-offset and the leading overscan columns overlapped the first visible
    /// column. The draw pass and `cell_at` both convert a window-local index through this origin, so
    /// a painted cell and the cell a press resolves to use one index semantics.
    fn window_origin(&self) -> (usize, usize) {
        let (row_start, _, col_start, _) = self.visible_window();
        (row_start, col_start)
    }

    /// Fetches visible cells, applying filters and sort rules in window scope.
    pub fn fetch_visible_cells(&mut self) -> Vec<Vec<Option<String>>> {
        (*self.fetch_visible_cells_shared()).clone()
    }

    /// The draw path's form: a cache hit with no filter/sort is the cached matrix itself.
    ///
    /// The owned form used to deep-copy the whole window on every frame, then run `retain`
    /// and `sort_by` over the copy. With no filter/sort in effect the projection is the
    /// cached matrix, so returning its `Rc` makes a hit a refcount bump.
    pub fn fetch_visible_cells_shared(&mut self) -> Rc<Vec<Vec<Option<String>>>> {
        if self.full_model_projection {
            return self.fetch_full_projected_window();
        }
        self.fetch_visible_window_view().rows
    }

    /// The draw pass's window: a borrowed, column-sliced view of the projection.
    fn fetch_visible_window_view(&mut self) -> ProjectedWindowView {
        self.normalize_projection_state();
        if self.full_model_projection {
            let projected = self.full_projection_rows();
            let (row_start, row_len, col_start, col_len) = self.visible_window();
            return ProjectedWindowView { rows: projected, row_start, row_len, col_start, col_len };
        }
        // With no projection in effect the window fetch *is* the virtualized path: it pulls only
        // the visible rows and caches them. A filter or sort needs the model-wide index first.
        if self.filter_expr.is_noop() && self.sort_specs.is_empty() {
            let cells = self.fetch_window_shared();
            let row_len = cells.len();
            let col_len = cells.first().map_or(0, Vec::len);
            return ProjectedWindowView {
                rows: cells,
                row_start: 0,
                row_len,
                col_start: 0,
                col_len,
            };
        }
        self.virtual_window_view()
    }

    /// Fetches only the window rows named by the virtualized projection.
    ///
    /// The sorted order is a list of source indices, so a window needs one single-row fetch per
    /// visible row rather than a whole-model fetch followed by a slice (V-001/V-002). The result
    /// is cached against the projection allocation and the window rectangle, so a repaint is a
    /// refcount bump and a stale projection cannot be served after a rebuild.
    fn virtual_window_view(&mut self) -> ProjectedWindowView {
        let Some(source) = self.data_source.clone() else {
            return ProjectedWindowView {
                rows: Rc::new(Vec::new()),
                row_start: 0,
                row_len: 0,
                col_start: 0,
                col_len: 0,
            };
        };
        let (row_start, row_len, col_start, col_len) = self.visible_window();
        if row_len == 0 || col_len == 0 {
            return ProjectedWindowView {
                rows: Rc::new(Vec::new()),
                row_start: 0,
                row_len: 0,
                col_start: 0,
                col_len: 0,
            };
        }
        let index = self.index_projection();
        let window = (row_start, row_len, col_start, col_len);
        if let Some((cached_index, cached_window, cells)) = self.virtual_window_cache.as_ref() {
            if Rc::ptr_eq(cached_index, &index) && *cached_window == window {
                let row_len = cells.len();
                return ProjectedWindowView {
                    rows: Rc::clone(cells),
                    row_start: 0,
                    row_len,
                    col_start: 0,
                    col_len,
                };
            }
        }
        let end = row_start.saturating_add(row_len).min(index.len());
        let mut rows = Vec::with_capacity(end.saturating_sub(row_start));
        for projected in row_start..end {
            let mut fetched = source.fetch_window(index[projected], 1, col_start, col_len);
            rows.push(fetched.pop().unwrap_or_default());
        }
        let cells = Rc::new(rows);
        let row_len = cells.len();
        self.virtual_window_cache = Some((Rc::clone(&index), window, Rc::clone(&cells)));
        ProjectedWindowView { rows: cells, row_start: 0, row_len, col_start: 0, col_len }
    }

    /// The window-scoped fetch used when full-model projection is off.
    fn fetch_window_shared(&mut self) -> Rc<Vec<Vec<Option<String>>>> {
        self.normalize_projection_state();

        let Some(source) = self.data_source.as_ref() else {
            return Rc::new(Vec::new());
        };

        let (row_start, row_len, col_start, col_len) = self.visible_window();
        if row_len == 0 || col_len == 0 {
            return Rc::new(Vec::new());
        }

        let revision = source.revision();
        let cache_revision =
            if revision > 0 { revision } else { self.data_generation.load(Ordering::Relaxed) };
        if self.data_is_observable {
            let mut cached_projection = None;
            let mut cached_cells = None;
            if let Some(cache) = self.window_cache.as_ref() {
                if cache.row_start == row_start
                    && cache.row_len == row_len
                    && cache.column_start == col_start
                    && cache.column_len == col_len
                    && cache.revision == cache_revision
                {
                    if self.filter_expr.is_noop() && self.sort_specs.is_empty() {
                        return Rc::clone(&cache.cells);
                    }
                    cached_projection = cache.projected.clone();
                    cached_cells = Some(Rc::clone(&cache.cells));
                }
            }
            if let Some(projected) = cached_projection {
                return projected;
            }
            if let Some(cells) = cached_cells {
                let projected = Rc::new(self.apply_filter_sort((*cells).clone(), row_start));
                if let Some(cache) = self.window_cache.as_mut() {
                    cache.projected = Some(Rc::clone(&projected));
                }
                return projected;
            }
        }

        let cells = Rc::new(source.fetch_window(row_start, row_len, col_start, col_len));
        if self.data_is_observable {
            self.window_cache = Some(GridWindowCache {
                row_start,
                row_len,
                column_start: col_start,
                column_len: col_len,
                revision: cache_revision,
                cells: Rc::clone(&cells),
                projected: None,
            });
        } else {
            self.window_cache = None;
        }

        if self.filter_expr.is_noop() && self.sort_specs.is_empty() {
            cells
        } else {
            let projected = Rc::new(self.apply_filter_sort((*cells).clone(), row_start));
            if let Some(cache) = self.window_cache.as_mut() {
                cache.projected = Some(Rc::clone(&projected));
            }
            projected
        }
    }

    /// Marks the bound source's contents as changed and drops every derived cache.
    pub fn refresh_data(&mut self) {
        self.data_generation.fetch_add(1, Ordering::Relaxed);
        self.invalidate_projection_cache();
        self.clear_cache();
        self.emit_visible_window_changed();
        self.base.request_redraw();
    }

    /// Fetches the whole model, projects it once, and slices the visible window.
    fn fetch_full_projected_window(&mut self) -> Rc<Vec<Vec<Option<String>>>> {
        let projected = self.full_projection_rows();
        if projected.is_empty() {
            return Rc::new(Vec::new());
        }
        let window = self.visible_window();
        if let Some((cached_projection, cached_window, cells)) = self.full_window_cache.as_ref() {
            if Rc::ptr_eq(cached_projection, &projected) && *cached_window == window {
                return Rc::clone(cells);
            }
        }
        let window_cells = Rc::new(Self::slice_window(&projected, window));
        self.full_window_cache = Some((Rc::clone(&projected), window, Rc::clone(&window_cells)));
        window_cells
    }

    /// The filtered/sorted full projection, rebuilt only when its cache key moves.
    fn full_projection_rows(&mut self) -> Rc<ProjectedRows> {
        let Some(source) = self.data_source.as_ref() else {
            return Rc::new(Vec::new());
        };
        let revision = source.revision();
        let cache_key =
            if revision > 0 { revision } else { self.data_generation.load(Ordering::Relaxed) };
        if let Some((key, projected)) = self.full_projection_cache.as_ref() {
            if *key == cache_key {
                return Rc::clone(projected);
            }
        }
        let rows = source.row_count();
        let columns = source.column_count();
        let cells = source.fetch_window(0, rows, 0, columns);
        let projected = Rc::new(self.apply_filter_sort(cells, 0));
        self.full_projection_cache = Some((cache_key, Rc::clone(&projected)));
        projected
    }

    /// The virtualized projection: which source row is painted at each projected row.
    ///
    /// The model is read one row at a time, and the result holds only indices. Filtering drops a
    /// row's index instead of retaining its cloned cells, and sorting reorders indices against a
    /// key table built once per spec — so a whole-model projection allocates `usize`s and keys,
    /// never a cloned row matrix (V-002).
    fn index_projection(&mut self) -> Rc<ProjectedIndex> {
        let Some(source) = self.data_source.clone() else {
            return Rc::new(Vec::new());
        };
        let revision = source.revision();
        let cache_key =
            if revision > 0 { revision } else { self.data_generation.load(Ordering::Relaxed) };
        if let Some((key, index)) = self.index_projection_cache.as_ref() {
            if *key == cache_key {
                return Rc::clone(index);
            }
        }

        let rows = source.row_count();
        let columns = source.column_count();
        let mut order: Vec<usize> = Vec::with_capacity(rows);
        if self.filter_expr.is_noop() {
            order.extend(0..rows);
        } else {
            // Filtering evaluates one row at a time. The row is transient: if it is rejected its
            // cells are dropped immediately rather than retained for a later slice.
            for row in 0..rows {
                let mut values = source.fetch_window(row, 1, 0, columns);
                if values.pop().is_some_and(|values| self.filter_expr.accepts(&values)) {
                    order.push(row);
                }
            }
        }

        if !self.sort_specs.is_empty() && !order.is_empty() {
            // One normalised key per spec per surviving row, computed once. The comparator reads
            // this table; it never re-reads the source (V-002).
            let mut key_sets: Vec<Vec<Option<(String, Option<f64>)>>> =
                Vec::with_capacity(self.sort_specs.len());
            for spec in &self.sort_specs {
                let role = self.sort_roles.get(&spec.column).copied();
                let mut keys = Vec::with_capacity(order.len());
                for &row in &order {
                    let role_value = role
                        .and_then(|role| source.data_role(row, spec.column, role))
                        .or_else(|| source.data(row, spec.column));
                    keys.push(Some(normalise_sort_key(role_value.as_deref().unwrap_or_default())));
                }
                key_sets.push(keys);
            }
            let empty = (String::new(), None);
            let mut positions: Vec<usize> = (0..order.len()).collect();
            positions.sort_by(|&left, &right| {
                for (index, spec) in self.sort_specs.iter().enumerate() {
                    let left_key = key_sets[index][left].as_ref().unwrap_or(&empty);
                    let right_key = key_sets[index][right].as_ref().unwrap_or(&empty);
                    let ordering = compare_sort_keys(left_key, right_key);
                    if ordering != core::cmp::Ordering::Equal {
                        return if spec.descending { ordering.reverse() } else { ordering };
                    }
                }
                // Stable tie-break on the source row, exactly as the materialised path does.
                order[left].cmp(&order[right])
            });
            order = positions.into_iter().map(|position| order[position]).collect();
        }

        let index = Rc::new(order);
        self.index_projection_cache = Some((cache_key, Rc::clone(&index)));
        index
    }

    fn slice_window(
        rows: &[Vec<Option<String>>],
        (row_start, row_len, col_start, col_len): (usize, usize, usize, usize),
    ) -> Vec<Vec<Option<String>>> {
        rows.iter()
            .skip(row_start)
            .take(row_len)
            .map(|row| row.iter().skip(col_start).take(col_len).cloned().collect::<Vec<_>>())
            .collect()
    }

    /// Filters and sorts projected rows, preserving each row's source identity.
    ///
    /// `row_origin` is the source row the first element of `cells` came from. It is what makes a
    /// role-aware sort key possible after a window fetch: the display text alone cannot tell the
    /// sorter that "10" is greater than "2", but asking the source for the configured role at the
    /// original row can. The rows stay paired with their source index through the filter pass, so a
    /// rejected row cannot shift the keys used by the survivors.
    fn apply_filter_sort(
        &self,
        cells: Vec<Vec<Option<String>>>,
        row_origin: usize,
    ) -> Vec<Vec<Option<String>>> {
        let row_span = cells.len();
        let mut rows: Vec<(usize, Vec<Option<String>>)> = cells
            .into_iter()
            .enumerate()
            .map(|(offset, row)| (row_origin.saturating_add(offset), row))
            .collect();
        // `is_noop` lets the whole `retain` pass be skipped when the filter cannot
        // reject anything, rather than walking every cell to conclude the same thing.
        if !self.filter_expr.is_noop() {
            rows.retain(|(_, row)| self.filter_expr.accepts(row));
        }

        if !self.sort_specs.is_empty() {
            let source = self.data_source.as_deref();
            // One key per spec per surviving row, computed once. A comparator that re-read the
            // source (and re-parsed the text) for every pair did `O(n log n)` source reads and
            // allocations for one sort.
            let mut key_sets: Vec<Vec<Option<(String, Option<f64>)>>> =
                Vec::with_capacity(self.sort_specs.len());
            for spec in &self.sort_specs {
                let role = self.sort_roles.get(&spec.column).copied();
                let mut keys: Vec<Option<(String, Option<f64>)>> = vec![None; row_span];
                for (source_row, row) in &rows {
                    let role_value = match (role, source) {
                        (Some(role), Some(source)) => {
                            source.data_role(*source_row, spec.column, role)
                        }
                        _ => None,
                    };
                    let display = row
                        .get(spec.column)
                        .and_then(|cell| cell.as_ref())
                        .cloned()
                        .unwrap_or_default();
                    if let Some(slot) = keys.get_mut(source_row.saturating_sub(row_origin)) {
                        *slot = Some(normalise_sort_key(&role_value.unwrap_or(display)));
                    }
                }
                key_sets.push(keys);
            }

            let empty = (String::new(), None);
            rows.sort_by(|(left_row, _), (right_row, _)| {
                let left = *left_row - row_origin;
                let right = *right_row - row_origin;
                for (index, spec) in self.sort_specs.iter().enumerate() {
                    let left_key = key_sets[index].get(left).and_then(|key| key.as_ref());
                    let right_key = key_sets[index].get(right).and_then(|key| key.as_ref());
                    let ordering =
                        compare_sort_keys(left_key.unwrap_or(&empty), right_key.unwrap_or(&empty));
                    if ordering != core::cmp::Ordering::Equal {
                        return if spec.descending { ordering.reverse() } else { ordering };
                    }
                }
                left_row.cmp(right_row)
            });
        }

        rows.into_iter().map(|(_, row)| row).collect()
    }

    fn visible_row_capacity(&self) -> usize {
        let height = self.base.geometry().height;
        if height == 0 {
            return 0;
        }
        // `div_ceil` on the max'd row height: the plain `(height + row_height - 1)` overflowed when
        // both operands were near `u32::MAX`, and the sibling `virtual_table` already used `div_ceil`.
        height.div_ceil(self.row_height.max(1)) as usize
    }

    /// The area the cells may occupy: the grid's box minus the margin that keeps ink off the
    /// frame's stroke, with the column-title strip removed from the top.
    ///
    /// Both the draw pass and the hit test start here, so a cell's painted box and the box a
    /// press resolves to are the same rectangle by construction.
    fn cells_box(&self) -> Rect {
        let content = ControlMetrics::band_inset(self.base.geometry(), GRID_INSET);
        let strip = self.header_strip_height();
        let height = content.height.saturating_sub(strip);
        Rect::new(content.x, content.y + strip as i32, content.width, height)
    }

    /// The box of the cell at `(row, column)` in the *projection* — the index the data source
    /// serves, including the scroll offset.
    ///
    /// This is a pure function of the indices, so the column a press lands in is the column
    /// that was painted for it. The draw loop used to advance its own `x += column_width`
    /// cursor, which made a cell's box depend on how many cells happened to precede it and
    /// left the two derivations free to disagree.
    fn cell_rect(&self, row: usize, column: usize) -> Rect {
        let cells = self.cells_box();
        // The row also travels the scroll: `row` is a **global** projected index, so its screen y is
        // measured from the current scroll row, the same way the column is measured from
        // `scroll_column`. Before this the row was used raw, so the first window row always painted
        // at the top of the grid regardless of scroll (D09-VIEW-01).
        let row = row.saturating_sub(self.scroll_row);
        // A frozen column is pinned to the leading edge by its **own** index. The scrolling band
        // starts after the frozen prefix, and each column travels by the sum of the widths before
        // it — not by `index * width`, which is only true while every column is the same size.
        Rect::new(
            cells.x + self.column_view_x(column),
            cells.y + row as i32 * self.row_height as i32,
            self.column_width_at(column),
            self.row_height,
        )
    }

    /// Paints one cell's border and value.
    ///
    /// Extracted so the scrolling pass and the frozen repaint use the **same** ink, font and
    /// alignment; two copies would let the pinned band drift away from the cells it sits over.
    fn paint_cell(
        &self,
        context: &mut RenderContext,
        cell_rect: Rect,
        value: Option<&String>,
        cell_border: Color,
        ink: Color,
    ) {
        context.draw_rect(cell_rect, cell_border);
        let Some(text) = value else {
            return;
        };
        // Guarded on the text being non-empty: an unguarded draw of an empty cell emits
        // `<text …></text>`, an element the rasteriser never produces.
        if text.is_empty() {
            return;
        }
        // The cell is the band. Centring by handing `y + row_h / 2` to `draw_text` put the glyph
        // box's *top* edge on the cell's middle line, so every value sat half a line low;
        // `text_line` derives the real centred box. Fitting as well keeps a long value inside its
        // own column instead of bleeding right.
        let font = effective_font(self.style());
        context.draw_text_fitted_cow(
            context.text_line(cell_rect, font),
            text,
            font,
            ink,
            HorizontalAlignment::Left,
        );
    }

    /// Paints a cell through the delegate when one is installed, otherwise directly.
    ///
    /// The delegate's `false` answer is the documented "keep the grid's own rendering" path, so
    /// a host can decorate selected cells and leave the rest to the control (V-014).
    fn paint_cell_ink(
        &self,
        context: &mut RenderContext,
        cell_rect: Rect,
        value: Option<&String>,
        row: usize,
        column: usize,
        cell_border: Color,
        ink: Color,
    ) {
        if let Some(delegate) = self.cell_delegate.as_ref() {
            if delegate.paint(context, cell_rect, row, column, value.map(String::as_str)) {
                return;
            }
        }
        self.paint_cell(context, cell_rect, value, cell_border, ink);
    }

    /// The union box of a merged run (V-014).
    fn span_rect(&self, span: CellSpan) -> Rect {
        let origin = self.cell_rect(span.row, span.column);
        let last_row = span.row.saturating_add(span.row_span.saturating_sub(1));
        let last_column = span.column.saturating_add(span.column_span.saturating_sub(1));
        let last = self.cell_rect(last_row, last_column);
        let right = last.x + last.width as i32;
        let bottom = last.y + last.height as i32;
        Rect::new(
            origin.x,
            origin.y,
            (right - origin.x).max(0) as u32,
            (bottom - origin.y).max(0) as u32,
        )
    }

    /// Paints `(row, column)`, merging it with the run it anchors (V-014).
    ///
    /// Returns `true` when ink was laid down. A cell covered by another cell's span answers
    /// `false`: the origin paints the whole run, including this cell's area.
    fn paint_cell_at(
        &self,
        context: &mut RenderContext,
        value: Option<&String>,
        row: usize,
        column: usize,
        cell_border: Color,
        ink: Color,
        selected_bg: Color,
    ) -> bool {
        match self.span_covering(row, column) {
            Some(span) if span.row == row && span.column == column => {
                let rect = self.span_rect(span);
                if self.selection_contains(row, column) {
                    context.fill_rect(rect, selected_bg);
                }
                self.paint_cell_ink(context, rect, value, row, column, cell_border, ink);
                true
            }
            Some(_) => false,
            None => {
                let rect = self.cell_rect(row, column);
                if self.selection_contains(row, column) {
                    context.fill_rect(rect, selected_bg);
                }
                self.paint_cell_ink(context, rect, value, row, column, cell_border, ink);
                true
            }
        }
    }

    /// The projected `(row, column)` a point falls in, or `None` when it misses every cell
    /// (including a press in the column-title strip or outside the grid).
    pub fn cell_at(&self, point: Point) -> Option<(usize, usize)> {
        let cells = self.cells_box();
        if !cells.contains_point(point) {
            return None;
        }
        let dx = point.x - cells.x;
        let dy = point.y - cells.y;
        if dx < 0 || dy < 0 {
            return None;
        }
        let column = self.column_at_dx(dx)?;
        let row = self.scroll_row + dy as usize / self.row_height.max(1) as usize;
        if column >= self.column_count() || row >= self.row_count() {
            return None;
        }
        if !self.cell_rect(row, column).contains_point(point) {
            return None;
        }
        // A merged cell answers as its origin, so a press anywhere inside the run selects (and
        // draws the focus ring around) the cell that actually owns the value (V-014).
        if let Some(span) = self.span_covering(row, column) {
            return Some((span.row, span.column));
        }
        Some((row, column))
    }

    fn visible_column_capacity(&self) -> usize {
        let width = self.base.geometry().width;
        if width == 0 {
            return 0;
        }
        // Same `div_ceil` reasoning as `visible_row_capacity`.
        width.div_ceil(self.effective_column_width().max(1)) as usize
    }

    /// The column-title strip.
    fn header_band(&self) -> Rect {
        let cells = self.cells_box();
        let height = self.header_strip_height();
        Rect::new(cells.x, cells.y - height as i32, cells.width, height)
    }

    /// The column index in the title strip under `point`.
    fn header_column_at(&self, point: Point) -> Option<usize> {
        let band = self.header_band();
        if !band.contains_point(point) {
            return None;
        }
        let dx = point.x - band.x;
        if dx < 0 {
            return None;
        }
        self.column_at_dx(dx).filter(|column| *column < self.column_count())
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
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(
            crate::widget::metrics::dimensions::DATA_GRID_DEFAULT_WIDTH,
            crate::widget::metrics::dimensions::DATA_GRID_DEFAULT_HEIGHT,
        )
    }

    /// Reports this widget as the object that paints it.
    ///
    /// `DataGrid` implements `Draw`, so `Some(self)` is total and cannot be wrong.
    fn as_draw_mut(&mut self) -> Option<&mut dyn crate::widget::Draw> {
        Some(self)
    }

    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why the field is named per arm
    ///
    /// The published name and the Rust field name are not always the same (`find_next` is backed by
    /// `find_next_signal`, `dismissed` by a `Signal1<()>` field). `connect_event` validates a name
    /// against the capability table and registers a hub slot; only `event_signal_dyn` joins that
    /// name to the signal the control actually **emits**. A wrong arm is worse than no arm, because
    /// it reports a wire as live and never fires it, so each field is named explicitly here rather
    /// than derived from the published name.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "visible_window_changed" => Some(EventSignalRef::mapped(
                "visible_window_changed",
                &self.visible_window_changed,
                |v| {
                    CapabilityValue::Tuple(alloc::vec![
                        CapabilityValue::UInt(v.0 as u64),
                        CapabilityValue::UInt(v.1 as u64),
                        CapabilityValue::UInt(v.2 as u64),
                        CapabilityValue::UInt(v.3 as u64),
                    ])
                },
            )),
            "context_menu_requested" => Some(EventSignalRef::mapped(
                "context_menu_requested",
                &self.context_menu_requested,
                |v| crate::widget::capability::CapabilityValue::String(format!("{},{}", v.x, v.y)),
            )),
            "edit_requested" => {
                Some(EventSignalRef::mapped("edit_requested", &self.edit_requested, |v| {
                    crate::widget::capability::CapabilityValue::Tuple(crate::compat::Vec::from([
                        crate::widget::capability::CapabilityValue::Int(v.0 as i64),
                        crate::widget::capability::CapabilityValue::Int(v.1 as i64),
                    ]))
                }))
            }
            "filter_changed" => {
                Some(EventSignalRef::mapped("filter_changed", &self.filter_changed, |v| {
                    crate::widget::capability::CapabilityValue::String(format!("{v:?}"))
                }))
            }
            "header_clicked" => {
                Some(EventSignalRef::mapped("header_clicked", &self.header_clicked, |v| {
                    crate::widget::capability::CapabilityValue::UInt(*v as u64)
                }))
            }
            "page_applied" => {
                Some(EventSignalRef::mapped("page_applied", &self.page_applied, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            "sort_changed" => {
                Some(EventSignalRef::mapped("sort_changed", &self.sort_changed, |v| {
                    crate::widget::capability::CapabilityValue::Tuple(crate::compat::Vec::from([
                        crate::widget::capability::CapabilityValue::UInt(v.0 as u64),
                        crate::widget::capability::CapabilityValue::Bool(v.1),
                    ]))
                }))
            }
            // The five interaction signals the control has always emitted natively; without
            // these arms the capability surface could observe none of them (the schema gate
            // answers `UnknownProperty` for any name absent here).
            "selection_changed" => Some(EventSignalRef::mapped(
                "selection_changed",
                self.selection_changed_signal(),
                |_v| CapabilityValue::Null,
            )),
            "current_changed" => Some(EventSignalRef::mapped(
                "current_changed",
                self.current_changed_signal(),
                |v| {
                    crate::widget::capability::CapabilityValue::Tuple(crate::compat::Vec::from([
                        crate::widget::capability::CapabilityValue::UInt(v.0 as u64),
                        crate::widget::capability::CapabilityValue::UInt(v.1 as u64),
                    ]))
                },
            )),
            "cell_clicked" => {
                Some(EventSignalRef::mapped("cell_clicked", self.cell_clicked_signal(), |v| {
                    crate::widget::capability::CapabilityValue::Tuple(crate::compat::Vec::from([
                        crate::widget::capability::CapabilityValue::UInt(v.0 as u64),
                        crate::widget::capability::CapabilityValue::UInt(v.1 as u64),
                    ]))
                }))
            }
            "cell_double_clicked" => Some(EventSignalRef::mapped(
                "cell_double_clicked",
                self.cell_double_clicked_signal(),
                |v| {
                    crate::widget::capability::CapabilityValue::Tuple(crate::compat::Vec::from([
                        crate::widget::capability::CapabilityValue::UInt(v.0 as u64),
                        crate::widget::capability::CapabilityValue::UInt(v.1 as u64),
                    ]))
                },
            )),
            "column_resized" => {
                Some(EventSignalRef::mapped("column_resized", self.column_resized_signal(), |v| {
                    crate::widget::capability::CapabilityValue::Tuple(crate::compat::Vec::from([
                        crate::widget::capability::CapabilityValue::UInt(v.0 as u64),
                        crate::widget::capability::CapabilityValue::UInt(v.1 as u64),
                    ]))
                }))
            }
            _ => None,
        }
    }
}

/// `DataGrid`'s property contract.
///
/// This control is one of the fourteen the old centralised dispatch could not
/// reach. It shares `WidgetKind::Table` with `TableWidget`, so the
/// `WidgetKind::Table` arms downcast to `TableWidget` and a `DataGrid` answered
/// `UnsupportedOnWidget` for **every** property — despite
/// `DATA_GRID_PROPERTIES` describing a full contract for it. Declaring the
/// contract here makes it reachable, which is the whole point of moving the
/// contract onto the control (BLUE15 rule #67).
///
/// The property set follows `DATA_GRID_PROPERTIES`: the two counts and the two
/// item collections are derived state and therefore read-only, while the viewport
/// and sizing knobs are writable.
impl WidgetProperties for DataGrid {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "has_data_source" => Ok(CapabilityValue::Bool(self.has_data_source())),
            "row_count" => Ok(CapabilityValue::UInt(self.row_count() as u64)),
            "column_count" => Ok(CapabilityValue::UInt(self.column_count() as u64)),
            "scroll_row" => Ok(CapabilityValue::UInt(self.scroll_row() as u64)),
            "scroll_column" => Ok(CapabilityValue::UInt(self.scroll_column() as u64)),
            "row_height" => Ok(CapabilityValue::UInt(self.row_height() as u64)),
            "column_width" => Ok(CapabilityValue::UInt(self.default_column_width() as u64)),
            "frozen_columns" => Ok(CapabilityValue::UInt(self.frozen_columns() as u64)),
            "sort_spec_count" => Ok(CapabilityValue::UInt(self.sort_specs().len() as u64)),
            // The count is of *conditions*, not of `ColumnFilter`s: a tree with a
            // nested `And` holds more conditions than the flat list would, and the
            // number a query-builder UI shows is the condition count.
            "filter_count" => {
                Ok(CapabilityValue::UInt(self.filter_expr().condition_count() as u64))
            }
            "sort_specs" => Ok(CapabilityValue::String(sort_specs_to_string(self.sort_specs()))),
            "filters" => Ok(CapabilityValue::String(column_filters_to_string(&self.filters()))),
            "selection_mode" => Ok(CapabilityValue::String(
                data_grid_selection_mode_to_str(self.selection_mode()).to_string(),
            )),
            "selection_behavior" => Ok(CapabilityValue::String(
                selection_behavior_to_str(self.selection_behavior()).to_string(),
            )),
            "sort_indicator_visible" => Ok(CapabilityValue::Bool(self.sort_indicator_visible())),
            "section_resize_mode" => Ok(CapabilityValue::String(
                column_width_mode_to_str(self.section_resize_mode()).to_string(),
            )),
            "current_cell" => match self.current_cell() {
                Some((row, column)) => Ok(CapabilityValue::String(format!("{row},{column}"))),
                None => Ok(CapabilityValue::Null),
            },
            "summary_text" => Ok(CapabilityValue::String(self.summary_text())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "summary_text" => Err(CapabilityAccessError::ReadOnlyProperty),
            "scroll_row" => {
                self.set_scroll_row(expect_usize(value)?);
                Ok(())
            }
            "scroll_column" => {
                self.set_scroll_column(expect_usize(value)?);
                Ok(())
            }
            "row_height" => {
                self.set_row_height(expect_u32_saturating(value)?);
                Ok(())
            }
            "column_width" => {
                self.set_default_column_width(expect_u32_saturating(value)?);
                Ok(())
            }
            "frozen_columns" => {
                self.set_frozen_columns(expect_usize(value)?);
                Ok(())
            }
            "sort_specs" => {
                self.set_sort_specs(expect_sort_specs(value)?);
                Ok(())
            }
            "filters" => {
                self.set_filters(expect_column_filters(value)?);
                Ok(())
            }
            "selection_mode" => {
                self.set_selection_mode(expect_data_grid_selection_mode(value)?);
                Ok(())
            }
            "selection_behavior" => {
                self.set_selection_behavior(expect_selection_behavior(value)?);
                Ok(())
            }
            "sort_indicator_visible" => {
                self.set_sort_indicator_visible(crate::widget::capability::coercion::expect_bool(
                    value,
                )?);
                Ok(())
            }
            "section_resize_mode" => {
                self.set_section_resize_mode(expect_column_width_mode(value)?);
                Ok(())
            }
            "current_cell" => Err(CapabilityAccessError::ReadOnlyProperty),
            // Derived and structural state: presented but not settable, matching
            // `DATA_GRID_PROPERTIES`' `writable: false` entries.
            "has_data_source" | "row_count" | "column_count" | "sort_spec_count"
            | "filter_count" => Err(CapabilityAccessError::ReadOnlyProperty),
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
            "column_width",
            "frozen_columns",
            "sort_spec_count",
            "filter_count",
            "sort_specs",
            "filters",
            "selection_mode",
            "selection_behavior",
            "sort_indicator_visible",
            "section_resize_mode",
            "current_cell",
            "summary_text",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `data_grid` publishes.
    ///
    /// `clear_data_source` is payload-free and executes here. The remaining names
    /// need a payload (a source, an index, a size) and are answered through the
    /// property route.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "clear_data_source" => {
                self.clear_data_source();
                Ok(())
            }
            // `select_all` takes every cell of the current behaviour, so it is one of the
            // payload-free commands the arm can honour outright.
            "select_all" => {
                self.select_all();
                Ok(())
            }
            "set_data_source" | "set_scroll_row" | "set_scroll_column" | "set_row_height"
            | "set_column_width" | "set_frozen_columns" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for DataGrid {
    fn draw(&mut self, context: &mut RenderContext) {
        // Drain a page the bound bar reported, so the paint shows the page the reader asked for.
        self.poll_pagination();
        // Drain chooser visibility/order changes for the same reason: the frame that follows the
        // chooser click already reflects it (V-015).
        self.poll_column_chooser();
        // A bound filter row reports through the same shared-cell route (chapter 3, gap 2).
        self.poll_filter_row();
        let rect = self.base.geometry();

        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then a literal. The theme step is what makes an appearance
        // switch visible; the surface, the cell fills, the borders and the text colour used
        // to be hardcoded literals, so light and dark rendered identically.
        //
        // The theme reads take and release the global manager's lock internally, so no guard
        // is held across the draw (the mutex is not re-entrant). `data_grid` reaches the
        // theme through its `table` classification, which is `Input` — an interior that
        // moves with the appearance.
        // `WidgetStyle` carries a font-family `String` and a gradient `Vec`; cloning it to read
        // three `Copy` colour fields allocated both on every frame (WDGT-08). Copy the fields
        // under one short borrow instead.
        let (background_color, text_color, border_color) = {
            let style = self.base.style();
            (style.background_color, style.text_color, style.border_color)
        };
        let theme = crate::style::resolved_theme_style_shared("data_grid");
        let mut surface = background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let ink = text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        // `data_grid` is absent from `WidgetRole::for_kind_name`'s table, so it classifies as
        // `Surface` and resolves to `theme.colors.background` — the window's own fill. A panel
        // painted in that colour would be byte-identical to the frame behind it, so a resolved
        // surface equal to the window fill is re-derived a visible step away from it, the same
        // distinction `Colors::input_background` draws for a field.
        // One palette read for the surface, grid and accent roles (three theme locks before).
        let palette = crate::style::palette_shared();
        let window_fill = palette.as_ref().map(|colors| colors.background).unwrap_or(Color::WHITE);
        if surface == window_fill {
            surface = window_fill.blend(&ink, 0.08);
        }
        let border = border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != surface)
            .unwrap_or_else(|| surface.blend(&ink, 0.20));
        // Cell chrome is the theme's `outline_variant` — the **weak** separator role — rather than a
        // local blend, so the grid is visibly weaker than the `outline`-strength focus ring (§5.4)
        // and a theme that tunes its separators moves this control with the rest of the table family
        // (`table_widget`, `virtual_table`, `tree_table`). The blend is the last rung for a build with
        // no theme.
        let cell_border = palette
            .as_ref()
            .map(|colors| colors.outline_variant)
            .filter(|resolved| *resolved != surface)
            .unwrap_or_else(|| surface.blend(&ink, 0.10));
        // The accent is the theme's `primary`: the hue a theme is expected to vary most, so
        // the frozen-column indicator follows the appearance rather than staying a literal blue.
        let accent = palette.as_ref().map(|colors| colors.primary).unwrap_or(Color::PRIMARY);
        let selected_bg = surface.blend(&accent, 0.22);

        context.face_with_gradient_corners(
            rect,
            surface,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().resolved_corner_radii().to_array(),
            Color::BLACK,
        );
        crate::widget::draw::stroke_surface_border(
            context,
            rect,
            self.style().resolved_corner_radii().to_array(),
            border,
            1,
        );

        // The draw pass reads a borrowed view of the projection (F-P02): no per-row/`String`
        // clone happens on a scroll, because the view addresses the cached matrix by offset.
        let window_view = self.fetch_visible_window_view();
        if window_view.is_empty() {
            return;
        }

        // The column-title strip: the grid has always reserved it, and this is where it is
        // finally used. Names come from `set_header_labels` first, then the source's own
        // `column_name`, so two grids over different sources are distinguishable at the header row.
        let cells = self.cells_box();
        let header_font = effective_font(self.style());
        let header_y = cells.y - GRID_HEADER_HEIGHT as i32;
        if let Some(source) = self.data_source.as_ref() {
            let cells_right = cells.x + cells.width as i32;
            // Header titles are static; elide each one once and reuse it (F-P04).
            let generation = self.data_generation.load(Ordering::Relaxed);
            let column_count = self.column_count();
            // Extra header levels are drawn above the title row, one strip each (V-014). The
            // levels share the title row's column geometry, so a hidden column leaves no gap
            // here either.
            for (level, labels) in self.extra_header_rows.iter().enumerate() {
                let y = cells.y
                    - GRID_HEADER_HEIGHT as i32 * (self.extra_header_rows.len() - level + 1) as i32;
                for column in self.scroll_column..column_count {
                    if self.is_column_hidden(column) {
                        continue;
                    }
                    let x = cells.x + self.column_view_x(column);
                    if x >= cells_right {
                        break;
                    }
                    let width = self.column_width_at(column).max(1);
                    let band = Rect::new(x, y, width, GRID_HEADER_HEIGHT);
                    context.fill_rect(band, surface.blend(&ink, 0.05));
                    if let Some(label) = labels.get(column).filter(|label| !label.is_empty()) {
                        context.draw_text_fitted_cow(
                            context.text_line(band, header_font),
                            label,
                            header_font,
                            ink,
                            HorizontalAlignment::Left,
                        );
                    }
                }
            }
            let mut text_cache = self.header_text_cache.borrow_mut();
            if text_cache.labels_revision != self.header_text_revision
                || text_cache.generation != generation
                || text_cache.column_count != column_count
            {
                text_cache.entries.clear();
                text_cache.labels_revision = self.header_text_revision;
                text_cache.generation = generation;
                text_cache.column_count = column_count;
            }
            for column in self.scroll_column..column_count {
                // A hidden column has no painted header section (V-014).
                if self.is_column_hidden(column) {
                    continue;
                }
                let x = cells.x + self.column_view_x(column);
                if x >= cells_right {
                    break;
                }
                let width = self.column_width_at(column).max(1);
                let band = Rect::new(x, header_y, width, GRID_HEADER_HEIGHT);
                context.fill_rect(band, surface.blend(&ink, 0.06));
                let name = text_cache.entries.entry((column, width)).or_insert_with(|| {
                    let raw = self
                        .header_labels
                        .get(column)
                        .filter(|label| !label.is_empty())
                        .cloned()
                        .or_else(|| source.column_name(column));
                    raw.map_or_else(String::new, |name| {
                        crate::widget::text_utils::elide_end(&name, 24).into_owned()
                    })
                });
                if !name.is_empty() {
                    // Leave room for the sort arrow and the filter glyph before fitting the label.
                    let filter_slot = if self.header_filter_visible
                        && self.header_columns.get(column).is_some_and(|spec| spec.filterable)
                    {
                        12
                    } else {
                        0
                    };
                    let sort_slot = if self.sort_indicator_visible
                        && self.sort_specs.iter().any(|spec| spec.column == column)
                    {
                        12
                    } else {
                        0
                    };
                    let arrow = filter_slot + sort_slot;
                    let text_band = Rect::new(
                        band.x + 2,
                        band.y,
                        band.width.saturating_sub(4 + arrow),
                        band.height,
                    );
                    context.draw_text_fitted_cow(
                        context.text_line(text_band, header_font),
                        name.as_str(),
                        header_font,
                        ink,
                        HorizontalAlignment::Left,
                    );
                }
                if self.sort_indicator_visible {
                    if let Some(spec) = self.sort_specs.iter().find(|spec| spec.column == column) {
                        let icon_side = band.height.min(10);
                        let filter_slot = if self.header_filter_visible
                            && self.header_columns.get(column).is_some_and(|spec| spec.filterable)
                        {
                            12
                        } else {
                            0
                        };
                        let icon_rect = Rect::new(
                            band.x + band.width as i32 - filter_slot - icon_side as i32 - 3,
                            band.y + (band.height as i32 - icon_side as i32) / 2,
                            icon_side,
                            icon_side,
                        );
                        let icon_name = if spec.descending {
                            crate::widget::IconName::ArrowDown
                        } else {
                            crate::widget::IconName::ArrowUp
                        };
                        crate::widget::draw_icon_at(context, icon_rect, ink, icon_name);
                    }
                }
                // A filterable column advertises the affordance its header spec declares; the
                // glyph is the same one the standalone `GridViewHeader` paints.
                if self.header_filter_visible
                    && self.header_columns.get(column).is_some_and(|spec| spec.filterable)
                {
                    let icon_side = band.height.min(10);
                    let icon_rect = Rect::new(
                        band.x + band.width as i32 - icon_side as i32 - 3,
                        band.y + (band.height as i32 - icon_side as i32) / 2,
                        icon_side,
                        icon_side,
                    );
                    crate::widget::draw_icon_at(
                        context,
                        icon_rect,
                        ink.blend(&surface, 0.3),
                        crate::widget::IconName::Filter,
                    );
                }
            }
        }

        // The cells are laid out from the control's inset content box, not from its literal
        // top edge. The first row used to start at `rect.y`, so its border and its glyph box
        // sat on the frame's own stroke; the inset also leaves a header row's worth of room
        // above the first cell, which is what a grid reads as its column-title strip.
        let cells = self.cells_box();
        let cells_right = cells.x + cells.width as i32;
        let cells_bottom = cells.y + cells.height as i32;

        // Every cell's box comes from `cell_rect`, the same derivation `cell_at` reads, so a
        // painted cell and the cell a press resolves to can never drift apart.
        //
        // The fetched window starts at `scroll - overscan`, so its leading rows/columns are above/
        // left of the viewport. Convert each window-local index to its **global** projected index
        // before `cell_rect` (which subtracts the scroll) (D09-VIEW-01). The filtered/sorted
        // projection is left as-is: its row identity is not the source's projected index, so no
        // correction is applied here.
        let (row_origin, col_origin) = self.window_origin();
        if self.alternating_rows {
            for row_idx in 0..window_view.len() {
                let global_row = row_origin + row_idx;
                if global_row % 2 == 1 {
                    let y = self.cell_rect(global_row, 0).y;
                    context.fill_rect(
                        Rect::new(cells.x, y, cells.width, self.row_height),
                        surface.blend(&accent, 0.05),
                    );
                }
            }
        }
        for row_idx in 0..window_view.len() {
            if self.cell_rect(row_origin + row_idx, 0).y >= cells_bottom {
                break;
            }
            let Some(row) = window_view.row(row_idx) else { continue };
            for (col_idx, cell) in row.iter().enumerate() {
                let global_column = col_origin + col_idx;
                // Frozen columns are painted by the overdraw pass below, after every scrolling
                // cell. Skipping them here is what makes that pass the single authority for the
                // pinned band: painting them twice would also lay their text down twice.
                if global_column < self.frozen_columns || self.is_column_hidden(global_column) {
                    continue;
                }
                let cell_rect = self.cell_rect(row_origin + row_idx, global_column);
                if cell_rect.x >= cells_right {
                    break;
                }
                let _ = self.paint_cell_at(
                    context,
                    cell.as_ref(),
                    row_origin + row_idx,
                    global_column,
                    cell_border,
                    ink,
                    selected_bg,
                );
            }
        }

        // ── Frozen columns, repainted over the scrolling cells ──
        //
        // The render layer has no clip primitive (there is no `clip_rect` in `src/render`), so
        // the compatible way to pin a leading band is the same one the rest of the family uses:
        // paint the scrolling grid first and overdraw the frozen cells on top of it. Their boxes
        // come from `cell_rect`, which ignores `scroll_column` for a frozen column, so this pass
        // is also the proof that the pinned band and the hit test agree.
        if self.frozen_columns > 0 {
            for row_idx in 0..window_view.len() {
                let Some(row) = window_view.row(row_idx) else { continue };
                let global_row = row_origin + row_idx;
                if self.cell_rect(global_row, 0).y >= cells_bottom {
                    break;
                }
                for column in 0..self.frozen_columns {
                    if self.is_column_hidden(column) {
                        continue;
                    }
                    let local = column.saturating_sub(col_origin);
                    let cell_rect = self.cell_rect(global_row, column);
                    if cell_rect.x >= cells_right {
                        break;
                    }
                    // `draw_rect` is a stroke, so the scrolling cell's ink underneath must be
                    // covered with the surface before this cell's own border and value go down.
                    context.fill_rect(cell_rect, surface);
                    let value = row.get(local).and_then(|cell| cell.as_ref());
                    let _ = self.paint_cell_at(
                        context,
                        value,
                        global_row,
                        column,
                        cell_border,
                        ink,
                        selected_bg,
                    );
                }
            }
        }

        // ── Selection and hover, painted **over** the cells ──
        //
        // The selection was stored and never drawn: a press set `self.selection`, re-requested a
        // redraw, and the grid came back looking exactly the same. A stored state with no consumer
        // is the same shape as a declared token with no consumer — the user cannot tell a selected
        // cell from any other. It is drawn here, over the cell borders, so the marker is not
        // overpainted by the next row's line.
        //
        // The hover is the accent at **lower** alpha: pointer position rather than a committed
        // choice, so it points at the cell a click would select without competing with the one
        // already selected. Both come from `cell_rect`, the same derivation `cell_at` reads, so a
        // highlighted cell is always a cell that can be hit.
        if let Some((row, column)) = self.hovered_cell {
            if !self.selection_contains(row, column) {
                let box_rect = self.cell_rect(row, column);
                context.draw_rect_stroke(box_rect, accent.with_alpha(120), 1);
            }
            // A tooltip is drawn only for the cell under the pointer, so a large model costs
            // nothing: the callback runs once per frame at most, and only when installed
            // (V-014).
            if let Some(text) = self.cell_tooltip(row, column).filter(|text| !text.is_empty()) {
                let cell = self.cell_rect(row, column);
                let tip_height = GRID_HEADER_HEIGHT;
                let tip_y = (cell.y + cell.height as i32 + 2)
                    .min(cells.y + cells.height as i32 - tip_height as i32)
                    .max(cells.y);
                let tip = Rect::new(cell.x, tip_y, cell.width.max(80).min(cells.width), tip_height);
                context.fill_rect(tip, surface.blend(&ink, 0.06));
                context.draw_rect(tip, cell_border);
                let font = effective_font(self.style());
                context.draw_text_fitted_cow(
                    context.text_line(tip, font),
                    &text,
                    font,
                    ink,
                    HorizontalAlignment::Left,
                );
            }
        }
        if let Some((row, column)) = self.current_cell.or(self.selection) {
            let box_rect = self.cell_rect(row, column);
            context.draw_rect_stroke(box_rect, accent, 2);
        }

        // The divider is the frozen band's **right edge**. That is the same arithmetic
        // `cell_rect` gives a frozen column's last trailing pixel, so the rule and the pinned
        // cells cannot disagree about where the band ends.
        if self.frozen_columns > 0 {
            let split_x = cells.x + self.column_prefix_width(self.frozen_columns);
            if split_x > cells.x {
                context.draw_line(
                    Point::new(split_x, cells.y),
                    Point::new(split_x, cells.y + cells.height as i32),
                    accent,
                );
            }
        }

        // Scroll-bar thumbs are opt-in. The grid is virtualized, so a grid without them can
        // still be scrolled by wheel/keyboard; the thumbs are the visible affordance that
        // says how much of the model is off-screen.
        if self.show_scrollbars {
            let total_rows = self.row_count().max(1);
            let visible_rows = self.visible_row_capacity().max(1).min(total_rows);
            let track = cells.height.max(1);
            let thumb = ((track as u64 * visible_rows as u64) / total_rows as u64)
                .max(24)
                .min(track as u64) as u32;
            let travel = track.saturating_sub(thumb);
            let max_scroll = total_rows.saturating_sub(visible_rows);
            let offset = if max_scroll == 0 {
                0
            } else {
                (travel as u64 * self.scroll_row.min(max_scroll) as u64 / max_scroll as u64) as u32
            };
            let band = Rect::new(rect.x + rect.width as i32 - 6, cells.y + offset as i32, 6, thumb);
            context.fill_rect(band, accent.with_alpha(120));
        }
    }
}

impl crate::event::EventHandler for DataGrid {
    fn handle_event(&mut self, event: &Event) {
        // A page click on the bound bar is a shared-cell write, so it is drained here before the
        // event is routed: the frame that follows the click already shows the new page.
        self.poll_pagination();
        // A chooser click is likewise a shared write, drained before routing (V-015).
        self.poll_column_chooser();
        // A filter-row edit is a shared write as well (chapter 3, gap 2).
        self.poll_filter_row();
        // The base keeps the control-level hover/press facts and answers `widget_state()`. This
        // handler did not forward to it, so `"data_grid:hover"` could never fire.
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        if let Event::Wheel { delta, modifiers } = event {
            // One notch is one row. `WheelScroll` owns the notch normalization, so a
            // legacy 120-unit delta and a modern notch delta both mean a single row.
            let scroll = WheelScroll::from_event(*delta, *modifiers, 1);
            let lines = scroll.lines_y.unsigned_abs() as usize;
            if scroll.lines_y < 0 {
                let next = self.scroll_row.saturating_add(lines);
                self.set_scroll_row(next);
            } else if scroll.lines_y > 0 {
                let up = self.scroll_row.saturating_sub(lines);
                self.set_scroll_row(up);
            }
            if scroll.columns_x != 0 {
                let next = self.scroll_column.saturating_add_signed(scroll.columns_x as isize);
                self.set_scroll_column(next);
            }
            let mods = crate::shortcut::Modifiers::from_event_bits(*modifiers);
            if mods.contains(crate::shortcut::Modifiers::SHIFT) {
                let next = self.scroll_column.saturating_add_signed(-(scroll.lines_y as isize));
                self.set_scroll_column(next);
            }
        }

        if let Event::MousePress { pos, button, modifiers } = event {
            if *button == 3 {
                self.context_menu_requested.emit(*pos);
                return;
            }
            if *button != 1 {
                return;
            }
            if let Some(column) = self.resize_handle_at(*pos) {
                self.resizing_column = Some(column);
                self.resize_start_x = pos.x;
                self.resize_start_width = self.column_width_at(column);
                return;
            }
            if let Some(column) = self.header_column_at(*pos) {
                self.header_clicked.emit(column);
                // The header cycles ascending -> descending -> unsorted, matching
                // `GridViewHeader::toggle_sort` (V-003). The third state restores the source
                // order, which a two-state toggle could never reach.
                let existing = self.sort_specs.iter().position(|spec| spec.column == column);
                let descending = match existing {
                    Some(index) => {
                        if self.sort_specs[index].descending {
                            self.sort_specs.remove(index);
                            None
                        } else {
                            self.sort_specs[index].descending = true;
                            Some(true)
                        }
                    }
                    None => {
                        self.sort_specs.push(SortSpec { column, descending: false });
                        Some(false)
                    }
                };
                self.invalidate_projection_cache();
                self.clear_cache();
                // `sort_changed` still carries `(column, descending)`; the unsorted state is
                // observable through `sort_specs()` and the arrow's absence. It is deliberately
                // not signalled as `(column, false)`, which would claim an ascending sort.
                if let Some(descending) = descending {
                    self.sort_changed.emit((column, descending));
                }
                self.base.request_redraw();
                return;
            }
            // The press resolves through `cell_at`, which reads `cell_rect` — the same
            // derivation the draw pass uses — so a selection can only name a cell that was
            // actually painted. A press in the column-title strip or in a partial trailing
            // column resolves to nothing.
            let hit = self.cell_at(*pos);
            self.current_cell = hit;
            match hit {
                Some((row, column)) => {
                    self.cell_clicked.emit((row, column));
                    self.apply_selection_click(row, column, *modifiers);
                    self.current_changed.emit((row, column));
                }
                None => {
                    if self.selection.is_some() || !self.selected_cells.is_empty() {
                        self.clear_selection();
                    }
                }
            }
            self.base.request_redraw();
        }

        match event {
            Event::MouseDoubleClick { pos, button: 1 } => {
                if let Some((row, column)) = self.cell_at(*pos) {
                    self.cell_double_clicked.emit((row, column));
                    self.apply_selection_click(row, column, 0);
                    self.current_cell = Some((row, column));
                    self.current_changed.emit((row, column));
                    if self.edit_triggers.contains(EditTriggers::DOUBLE_CLICKED) {
                        self.edit_requested.emit((row, column));
                    }
                    self.base.request_redraw();
                }
            }
            Event::ContextMenu { pos } => self.context_menu_requested.emit(*pos),
            Event::KeyPress { key, modifiers } if self.base.focus_reason().is_some() => {
                let page = self.visible_row_capacity().max(1);
                let last_row = self.row_count().saturating_sub(1);
                let last_column = self.column_count().saturating_sub(1);
                if self.row_count() == 0 || self.column_count() == 0 {
                    return;
                }
                let (mut row, mut column) = self
                    .current_cell
                    .or(self.selection)
                    .unwrap_or((self.scroll_row, self.scroll_column));
                let modifiers = crate::shortcut::Modifiers::from_event_bits(*modifiers);
                let shift = modifiers.contains(crate::shortcut::Modifiers::SHIFT)
                    && self.selection_mode
                        == crate::widget::view_widgets::list_view::SelectionMode::Extended;
                match *key {
                    crate::event::key_codes::UP => row = row.saturating_sub(1),
                    crate::event::key_codes::DOWN => row = row.saturating_add(1).min(last_row),
                    crate::event::key_codes::LEFT => column = column.saturating_sub(1),
                    crate::event::key_codes::RIGHT => {
                        column = column.saturating_add(1).min(last_column)
                    }
                    crate::event::key_codes::PAGE_UP => row = row.saturating_sub(page),
                    crate::event::key_codes::PAGE_DOWN => {
                        row = row.saturating_add(page).min(last_row)
                    }
                    crate::event::key_codes::HOME => column = 0,
                    crate::event::key_codes::END => column = last_column,
                    crate::event::key_codes::ENTER
                        if self.edit_triggers.contains(EditTriggers::EDIT_KEY) =>
                    {
                        if let Some(cell) = self.current_cell.or(self.selection) {
                            self.edit_requested.emit(cell);
                        }
                        return;
                    }
                    _ => return,
                }
                self.move_current_cell(row, column, shift);
            }
            // Cell hover, from the same `cell_at` the press reads: the cell that is outlined is the
            // cell a click would select. A pointer that leaves the cells (into the title strip, past
            // the last column, or off the control) clears it rather than latching the last cell it
            // crossed.
            Event::MouseMove { pos } => {
                if let Some(column) = self.resizing_column {
                    let delta = pos.x - self.resize_start_x;
                    let width = (self.resize_start_width as i32 + delta).max(1) as u32;
                    if self.store_column_width(column, width) {
                        self.base.request_redraw();
                    }
                    return;
                }
                let hovered = self.cell_at(*pos);
                if hovered != self.hovered_cell {
                    self.hovered_cell = hovered;
                    self.base.request_redraw();
                }
            }
            Event::MouseRelease { button: 1, .. } => {
                if let Some(column) = self.resizing_column.take() {
                    self.column_resized.emit((column, self.column_width_at(column)));
                    self.base.request_redraw();
                }
            }
            Event::MouseLeave { .. } => {
                let had_hover = self.hovered_cell.take().is_some();
                if had_hover {
                    self.base.request_redraw();
                }
            }
            _ => { /* The wheel and press arms above already handled those. */ }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::types::EventHandler;
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

    fn static_grid(rows: usize, cols: usize) -> DataGrid {
        let mut grid = DataGrid::new(Rect::new(0, 0, 600, 300));
        grid.set_data_source(Arc::new(StaticSource {
            rows,
            cols,
            data: (0..rows).map(|r| (0..cols).map(|c| format!("{r}:{c}")).collect()).collect(),
        }));
        grid
    }

    /// V-015: a chooser toggle reaches the grid's hidden-column state.
    #[test]
    fn a_chooser_toggle_hides_the_column_in_the_grid() {
        use crate::widget::view_widgets::column_chooser::ColumnChooser;

        let mut grid = static_grid(2, 3);
        let mut chooser = ColumnChooser::new(Rect::new(0, 0, 220, 260));
        chooser.set_columns(vec![
            ColumnSpec::new("Name", 100),
            ColumnSpec::new("Size", 80),
            ColumnSpec::new("Owner", 120),
        ]);
        let tokens: Vec<String> = chooser.columns().iter().map(ColumnSpec::to_token).collect();

        grid.attach_column_chooser(&chooser);
        assert!(grid.is_column_chooser_attached());
        assert!(!grid.is_column_hidden(1));

        assert!(chooser.set_column_visible(&tokens[1], false));
        assert!(grid.poll_column_chooser(), "the reported change is applied");
        assert!(grid.is_column_hidden(1), "the chooser hid the second column");

        assert!(chooser.set_column_visible(&tokens[1], true));
        assert!(grid.poll_column_chooser());
        assert!(!grid.is_column_hidden(1), "showing the column restores it");

        // A reorder is reported as source indices as well (V-015).
        assert!(chooser.move_column(0, 2));
        assert!(grid.poll_column_chooser());
        assert_eq!(grid.column_display_order(), vec![1, 2, 0]);

        grid.detach_column_chooser();
        assert!(!grid.is_column_chooser_attached());
        assert!(chooser.set_column_visible(&tokens[1], false));
        assert!(!grid.poll_column_chooser(), "a detached chooser reports nothing");
        assert!(!grid.is_column_hidden(1));
    }

    /// Chapter 3, gap 2: a condition committed on the bound row filters the grid, and detaching
    /// stops the wiring without undoing the last filter.
    #[test]
    fn a_condition_from_the_filter_row_filters_the_grid() {
        let mut grid = static_grid(4, 2);
        let mut row = FilterRow::new(Rect::new(0, 0, 300, 28));
        row.set_columns(vec!["Id".to_string(), "Name".to_string()]);

        grid.attach_filter_row(&row);
        assert!(grid.is_filter_row_attached());
        assert_eq!(grid.fetch_visible_cells().len(), 4, "an empty row filters nothing");

        assert!(row.set_condition(0, FilterCondition::contains(0, "2")));
        assert!(grid.poll_filter_row(), "the reported conditions are applied");
        let rows = grid.fetch_visible_cells();
        assert_eq!(rows.len(), 1, "the condition filters the grid rows");
        assert_eq!(rows[0][0], Some("2:0".to_string()));
        assert!(!grid.poll_filter_row(), "an unchanged report is not re-applied");

        grid.detach_filter_row();
        assert!(!grid.is_filter_row_attached());
        assert!(row.set_condition(0, FilterCondition::contains(0, "3")));
        assert!(!grid.poll_filter_row(), "a detached row reports nothing");
        let rows = grid.fetch_visible_cells();
        assert_eq!(rows.len(), 1, "the last applied filter stays");
        assert_eq!(rows[0][0], Some("2:0".to_string()));
    }

    #[test]
    fn header_sections_split_by_width_and_report_clicks() {
        let mut header = GridViewHeader::new(Rect::new(0, 0, 300, 28));
        header.set_columns(vec![
            ColumnSpec::new("Name", 100).with_sortable(true),
            ColumnSpec::new("Size", 100).with_filterable(true),
            ColumnSpec::new("Owner", 100),
        ]);
        let clicked = Arc::new(Mutex::new(Vec::<usize>::new()));
        let sink = clicked.clone();
        header.section_clicked.connect(move |index| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*index);
            }
        });

        assert_eq!(header.section_rect(1), Some(Rect::new(100, 0, 100, 28)));
        let middle = header.section_rect(1).expect("section 1");
        header.handle_event(&Event::mouse_press(middle.x + 10, middle.y + 10, 1));
        assert_eq!(*clicked.lock().expect("clicked lock"), vec![1]);
    }

    #[test]
    fn header_sort_indicator_cycles_and_reports_the_order() {
        let mut header = GridViewHeader::new(Rect::new(0, 0, 200, 28));
        header.set_columns(vec![
            ColumnSpec::new("Name", 100).with_sortable(true),
            ColumnSpec::new("Fixed", 100),
        ]);
        assert!(header.set_sort_indicator(0, Some(SortOrder::Ascending)));
        assert_eq!(header.sort_indicator(0), Some(SortOrder::Ascending));
        assert_eq!(header.toggle_sort(0), Some(SortOrder::Descending));
        assert_eq!(header.toggle_sort(0), None);
        assert_eq!(header.sort_indicator(0), None);
        assert_eq!(header.toggle_sort(1), None, "an unsortable section never sorts");
        assert!(!header.set_sort_indicator(5, None), "there is no section 5");
    }

    #[test]
    fn header_resize_emits_the_new_width() {
        let mut header = GridViewHeader::new(Rect::new(0, 0, 300, 28));
        header.set_columns(vec![ColumnSpec::new("Name", 100)]);
        header.set_resize_mode(ColumnWidthMode::Interactive);
        let resized = Arc::new(Mutex::new(Vec::<(usize, u32)>::new()));
        let sink = resized.clone();
        header.section_resized.connect(move |pair| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*pair);
            }
        });

        header.handle_event(&Event::mouse_press(99, 10, 1));
        header.handle_event(&Event::mouse_move(139, 10));
        header.handle_event(&Event::mouse_release(139, 10, 1));

        assert_eq!(header.section_width(0), 140);
        assert_eq!(*resized.lock().expect("resized lock"), vec![(0, 140)]);
    }

    #[test]
    fn set_header_adopts_titles_widths_and_arrows() {
        let mut grid = static_grid(3, 3);
        let mut header = GridViewHeader::new(Rect::new(0, 0, 400, 28));
        header.set_columns(vec![
            ColumnSpec::new("Alpha", 90).with_sortable(true),
            ColumnSpec::new("Beta", 110).with_filterable(true),
            ColumnSpec::new("Gamma", 120),
        ]);
        header.set_sort_indicator(1, Some(SortOrder::Descending));

        grid.set_header(&header);

        assert_eq!(grid.header_columns().len(), 3);
        assert_eq!(grid.header_columns()[0].title, "Alpha");
        assert_eq!(grid.column_width_at(1), 110);
        assert_eq!(grid.sort_specs().len(), 1);
        assert_eq!(grid.sort_specs()[0], SortSpec { column: 1, descending: true });
        assert_eq!(grid.get("summary_text"), Ok(CapabilityValue::String("3 rows".to_string())));
    }

    #[test]
    fn a_page_click_on_the_bound_bar_reaches_the_grid() {
        let mut grid = static_grid(100, 2);
        let mut pagination =
            crate::widget::nav_widgets::pagination::Pagination::new(Rect::new(0, 0, 300, 32));
        pagination.set_page_size(10);
        grid.set_pagination(&mut pagination);
        assert_eq!(pagination.total(), 100, "the bar learns the row count");
        assert_eq!(grid.pagination_state(), Some((0, 10)));

        let applied = Arc::new(Mutex::new(Vec::<usize>::new()));
        let sink = applied.clone();
        grid.page_applied.connect(move |page| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*page);
            }
        });

        assert!(pagination.next_page());
        assert_eq!(grid.poll_pagination(), Some(1));
        assert_eq!(grid.scroll_row(), 10);
        assert_eq!(*applied.lock().expect("applied lock"), vec![1]);
        assert_eq!(grid.poll_pagination(), None, "an unchanged page is not re-applied");

        // Pushing the grid's page back into the bar is the other half of the binding.
        grid.set_scroll_row(30);
        assert!(grid.sync_pagination(&mut pagination));
        assert_eq!(pagination.page(), 3);
    }

    #[test]
    fn summary_text_reports_rows_selection_and_filter() {
        let mut grid = static_grid(10, 2);
        assert_eq!(grid.summary_text(), "10 rows");
        grid.set_current_cell(0, 0);
        assert!(grid.summary_text().contains("selected"));
        grid.set_filters(vec![ColumnFilter { column: 0, query: "1".to_string() }]);
        assert!(grid.summary_text().contains("filtered"));
    }

    #[test]
    fn stretch_last_gives_the_remaining_room_to_the_last_column() {
        let mut grid = static_grid(3, 3);
        grid.set_column_width_mode(ColumnWidthMode::StretchLast);
        let width = grid.column_width_at(2);
        assert!(width > 0);
        // The first two columns keep the base width, so the last takes the rest.
        assert_eq!(grid.column_width_at(0), grid.default_column_width());
        assert_eq!(grid.column_width_at(1), grid.default_column_width());
        assert_eq!(
            width as i32,
            grid.cells_box().width as i32 - 2 * grid.default_column_width() as i32
        );
    }

    /// A cell's box must be a pure function of its own projected indices.
    ///
    /// The draw loop used to advance its own `x += column_width` cursor, so a cell's box
    /// depended on how many cells happened to precede it in the fetched window. Both the draw
    /// pass and `cell_at` now read `cell_rect`, so this pins the closed form.
    #[test]
    fn a_cells_box_is_a_function_of_its_own_indices() {
        let grid = DataGrid::new(Rect::new(0, 0, 400, 300));
        let cells = grid.cells_box();

        assert_eq!(grid.cell_rect(0, 0), Rect::new(cells.x, cells.y, 120, 20));
        assert_eq!(grid.cell_rect(0, 2).x, cells.x + 240);
        assert_eq!(grid.cell_rect(3, 0).y, cells.y + 60);
        assert_eq!(grid.cell_rect(3, 0).width, 120);
        assert_eq!(grid.cell_rect(3, 0).height, 20);

        // Idempotent: asking twice names the same box.
        assert_eq!(grid.cell_rect(1, 1), grid.cell_rect(1, 1));

        // The cells start below the column-title strip, never on the frame's own stroke.
        assert!(grid.cells_box().y > 0, "cells must clear the column-title strip");
        assert!(grid.cells_box().height < 300, "the title strip must be removed");
    }

    /// The box a press resolves to must be the box that was painted for that cell.
    #[test]
    fn the_cell_a_press_resolves_to_is_the_cell_that_was_painted() {
        // The grid is wider than four 120 px columns so that column 3's centre falls inside
        // the control — a cell whose centre is past the right edge cannot be probed at all,
        // because nothing is painted there.
        let mut grid = DataGrid::new(Rect::new(0, 0, 900, 300));
        grid.set_data_source(Arc::new(StaticSource {
            rows: 4,
            cols: 4,
            data: (0..4).map(|r| (0..4).map(|c| format!("{}:{}", r, c)).collect()).collect(),
        }));

        let target = grid.cell_rect(2, 3);
        let centre =
            Point::new(target.x + target.width as i32 / 2, target.y + target.height as i32 / 2);
        assert_eq!(grid.cell_at(centre), Some((2, 3)));
        assert_eq!(grid.selection(), None, "merely probing must not select");

        // The box that *was* painted is the box a press resolves to, so the same point names
        // the same cell for probing and for pressing.
        grid.handle_event(&Event::MousePress { pos: centre, button: 1, modifiers: 0 });
        assert_eq!(grid.selection(), Some((2, 3)));

        // A press in the column-title strip or outside the grid resolves to nothing rather
        // than to a phantom cell.
        assert_eq!(grid.cell_at(Point::new(centre.x, 1)), None);
        assert_eq!(grid.cell_at(Point::new(-5, -5)), None);

        let below = Point::new(centre.x, target.y + 20 * 100);
        grid.handle_event(&Event::MousePress { pos: below, button: 1, modifiers: 0 });
        assert_eq!(grid.selection(), None, "a press on empty space must clear the selection");
    }

    /// A selected cell is **painted** as selected, and a hovered cell is marked more faintly.
    ///
    /// # The defect this pins
    ///
    /// `selection` was stored and never drawn: a press set the field, re-requested a redraw, and the
    /// grid came back byte-identical. A stored state with no consumer is the same shape as a declared
    /// token with no consumer, and this one is a *user-visible* one — the user cannot tell the cell
    /// they clicked from any other.
    ///
    /// # Why the assertion counts accent strokes rather than comparing documents
    ///
    /// "The document changed" would also be satisfied by the hover outline or by the frozen-column
    /// rule. The marker is emitted as a `<rect ... stroke="..." stroke-width="2" />` in the accent
    /// colour, so counting elements carrying that stroke names exactly the element under test.
    #[test]
    #[cfg(all(device_profile, feature = "desktop"))]
    fn a_selected_cell_is_painted_as_selected() {
        let _guard = crate::style::theme_test_guard();
        crate::widget::census::install_preset_appearances();
        crate::theme::global_theme_manager().set_appearance(crate::theme::AppearanceMode::Light);

        let mut grid = DataGrid::new(Rect::new(0, 0, 400, 300));
        grid.set_data_source(Arc::new(StaticSource {
            rows: 4,
            cols: 4,
            data: (0..4).map(|r| (0..4).map(|c| format!("{r}:{c}")).collect()).collect(),
        }));

        let target = grid.cell_rect(1, 1);
        let centre =
            Point::new(target.x + target.width as i32 / 2, target.y + target.height as i32 / 2);

        let before = width_two_strokes(&mut grid);
        assert_eq!(before, 0, "an untouched grid draws no selection marker");

        grid.handle_event(&Event::MousePress { pos: centre, button: 1, modifiers: 0 });
        assert_eq!(grid.selection(), Some((1, 1)));
        let after = width_two_strokes(&mut grid);
        assert_eq!(
            after,
            before + 1,
            "selecting a cell must paint exactly one selection marker (had {before}, now {after})"
        );
    }

    /// How many SVG `<rect>` strokes are drawn at width 2 — how the selection marker is emitted.
    #[cfg(all(device_profile, feature = "desktop"))]
    fn width_two_strokes(grid: &mut DataGrid) -> usize {
        let rect = grid.geometry();
        let svg = crate::widget::svg::render_widget_to_svg(grid, rect);
        svg.match_indices("stroke-width=\"2\"").count()
    }

    /// Scrolling moves the mapping from a point to a cell along with the cells themselves.
    ///
    /// The scroll offset is applied inside `cell_rect`, so a press at a fixed point names a
    /// later column once the grid has scrolled left; the painted box and the resolved cell
    /// move together.
    #[test]
    fn scrolling_moves_the_point_to_cell_mapping_with_the_cells() {
        let mut grid = DataGrid::new(Rect::new(0, 0, 400, 300));
        grid.set_data_source(Arc::new(StaticSource {
            rows: 4,
            cols: 8,
            data: (0..4).map(|r| (0..8).map(|c| format!("{}:{}", r, c)).collect()).collect(),
        }));

        let first = grid.cell_rect(0, 0);
        let probe = Point::new(first.x + 5, first.y + 5);
        assert_eq!(grid.cell_at(probe), Some((0, 0)));

        grid.set_scroll_column(2);
        // The first painted column is now projected column 2, so the same point names it.
        assert_eq!(grid.cell_at(probe), Some((0, 2)));
        // The first visible scrolling column occupies the band's leading edge; the columns
        // hidden to its left are off-screen rather than stacked on top of it.
        assert_eq!(grid.cell_rect(0, 2).x, first.x);
        assert!(grid.cell_rect(0, 0).x < first.x, "scrolled-away columns stay off-screen");

        // The projected column just right of the probe is one further along, so the mapping
        // really is continuous across the column boundary rather than pinned to a single cell.
        assert_eq!(grid.cell_at(Point::new(first.x + 120 + 5, probe.y)), Some((0, 3)));

        // A point left of the cells box — the frame margin the control does not paint in —
        // resolves to nothing. This is where truncating `usize` division used to go wrong: it
        // floored a negative distance to `0` and named a phantom column.
        assert_eq!(
            grid.cell_at(Point::new(first.x - 1, probe.y)),
            None,
            "the frame margin is not a cell"
        );
    }

    /// A frozen column keeps its leading position when the grid scrolls horizontally.
    ///
    /// `cell_rect` used to apply `scroll_column` to every column, so the "frozen" prefix slid
    /// away with the rest of the grid and the only trace of the feature was the divider line.
    /// The pinned band is now a fixed function of the column index; the scrolling columns on
    /// its right still travel the scroll.
    #[test]
    fn frozen_columns_ignore_horizontal_scroll() {
        let mut grid = DataGrid::new(Rect::new(0, 0, 900, 300));
        grid.set_data_source(Arc::new(StaticSource {
            rows: 4,
            cols: 8,
            data: (0..4).map(|r| (0..8).map(|c| format!("{r}:{c}")).collect()).collect(),
        }));
        grid.set_frozen_columns(2);

        let cells = grid.cells_box();
        let first = grid.cell_rect(0, 0);
        let second = grid.cell_rect(0, 1);
        assert_eq!(first.x, cells.x);
        assert_eq!(second.x, cells.x + grid.column_width as i32);

        grid.set_scroll_column(5);

        assert_eq!(grid.cell_rect(0, 0), first, "the frozen column must not travel the scroll");
        assert_eq!(grid.cell_rect(0, 1), second, "nor the next frozen column");
        // Rows still scroll: only the column offset is pinned.
        assert_eq!(grid.cell_rect(2, 0).y, cells.y + 2 * grid.row_height as i32);

        // A point over the frozen band names a frozen column, not the scrolling column the old
        // inverse arithmetic would have named.
        let frozen_centre =
            Point::new(second.x + second.width as i32 / 2, second.y + second.height as i32 / 2);
        assert_eq!(grid.cell_at(frozen_centre), Some((0, 1)));

        // To the right of the band the original scroll mapping is unchanged.
        let scrolling = grid.cell_rect(2, 7);
        assert_eq!(
            scrolling.x,
            cells.x + 2 * grid.column_width as i32 + 2 * grid.column_width as i32,
            "the frozen prefix occupies its own band before the scrolled columns"
        );
        let scrolling_centre = Point::new(
            scrolling.x + scrolling.width as i32 / 2,
            scrolling.y + scrolling.height as i32 / 2,
        );
        assert_eq!(grid.cell_at(scrolling_centre), Some((2, 7)));

        // The fetched window must include the frozen prefix, or the draw pass would have no
        // values to repaint over the scrolling cells.
        assert_eq!(grid.visible_window().2, 0);
    }

    /// A press inside the frozen band selects the frozen cell it is over.
    #[test]
    fn a_click_in_the_frozen_band_selects_the_frozen_column() {
        let mut grid = DataGrid::new(Rect::new(0, 0, 400, 300));
        grid.set_data_source(Arc::new(StaticSource {
            rows: 4,
            cols: 8,
            data: (0..4).map(|r| (0..8).map(|c| format!("{r}:{c}")).collect()).collect(),
        }));
        grid.set_frozen_columns(2);
        grid.set_scroll_column(5);

        let target = grid.cell_rect(3, 1);
        let centre =
            Point::new(target.x + target.width as i32 / 2, target.y + target.height as i32 / 2);
        grid.handle_event(&Event::MousePress { pos: centre, button: 1, modifiers: 0 });

        assert_eq!(grid.selection(), Some((3, 1)));
    }

    /// The frozen band is **painted** at the leading edge after a horizontal scroll.
    ///
    /// The model fix alone is not enough: with the fetch window starting at `scroll - overscan`,
    /// a frozen column that lies before it never reaches the draw pass at all. Only the frozen
    /// cell is made non-empty, so exactly one text run must appear — and it must land in the
    /// leading column.
    #[test]
    #[cfg(all(device_profile, feature = "desktop"))]
    fn a_frozen_column_is_painted_at_the_leading_edge_after_scrolling() {
        let mut grid = DataGrid::new(Rect::new(0, 0, 400, 300));
        let mut data: Vec<Vec<String>> = vec![vec![String::new(); 8]; 4];
        data[1][0] = "P".to_string();
        grid.set_data_source(Arc::new(StaticSource { rows: 4, cols: 8, data }));
        grid.set_frozen_columns(2);
        grid.set_scroll_column(5);

        let rect = grid.geometry();
        let svg = crate::widget::svg::render_widget_to_svg(&mut grid, rect);
        let boxes = crate::widget::svg::text_ink_boxes(&svg);
        assert_eq!(
            boxes.len(),
            1,
            "only the frozen cell is non-empty; a missing frozen window paints nothing"
        );
        let frozen = grid.cell_rect(1, 0);
        let (bx, by, _, _) = boxes[0];
        assert!(
            bx >= frozen.x && bx < frozen.x + frozen.width as i32,
            "the frozen value must land in the leading column ({frozen:?}), got x={bx}"
        );
        assert!(
            by >= frozen.y && by < frozen.y + frozen.height as i32,
            "and in its own row ({frozen:?}), got y={by}"
        );
    }

    // ── D09-VIEW-01: the leading overscan is skipped, not painted as the first cell ──

    /// After a vertical scroll the first visible box must name the scrolled-to row, and a press in
    /// that box must select that same global row.
    ///
    /// # The defect this pins
    ///
    /// `visible_window()` fetches from `row_start = scroll - overscan`, but the draw loop enumerated
    /// the fetched matrix from its local row `0` — so at row 8 the first painted row was global row 6
    /// (the overscan lead), not row 8. The row also never travelled the scroll in `cell_at`. Both
    /// now measure the row from `scroll_row`, so the box a press resolves to is the box that was
    /// painted for the scrolled-to row.
    #[test]
    fn a_vertical_scroll_keeps_the_first_visible_row_at_the_scroll_position() {
        let mut grid = DataGrid::new(Rect::new(0, 0, 400, 300));
        grid.set_data_source(Arc::new(StaticSource {
            rows: 30,
            cols: 6,
            data: (0..30).map(|r| (0..6).map(|c| format!("{r}:{c}")).collect()).collect(),
        }));

        grid.set_scroll_row(8);
        // The default row overscan (2) means the fetch begins two rows early.
        assert_eq!(grid.visible_window().0, 6, "the fetch origin leads the scroll by the overscan");

        // The first visible box must name global row 8, not the overscan lead row 6.
        let first = grid.cell_rect(8, 0);
        let centre =
            Point::new(first.x + first.width as i32 / 2, first.y + first.height as i32 / 2);
        assert_eq!(grid.cell_at(centre), Some((8, 0)));

        // And a press there selects the same global cell.
        grid.handle_event(&Event::MousePress { pos: centre, button: 1, modifiers: 0 });
        assert_eq!(grid.selection(), Some((8, 0)));
    }

    /// A scrolled grid paints its visible rows, not the leading overscan rows — the model-versus-
    /// pixels form. As with `virtual_table`, the SVG carries glyph outlines rather than the text
    /// string, so exactly one cell is made non-empty and its ink box is checked instead.
    #[test]
    #[cfg(all(device_profile, feature = "desktop"))]
    fn the_visible_rows_are_drawn_blocked_at_the_scroll_position() {
        // The scrolled-to cell is the only non-empty one; if the leading overscan were painted first
        // the first visible box would be blank.
        let mut grid = DataGrid::new(Rect::new(0, 0, 400, 300));
        let rows = 30usize;
        let mut data: Vec<Vec<String>> = vec![vec![String::new(); 6]; rows];
        data[8][0] = "W".to_string();
        grid.set_data_source(Arc::new(StaticSource { rows, cols: 6, data }));

        grid.set_scroll_row(8);
        let first = grid.cell_rect(8, 0);
        let rect = grid.geometry();
        let svg = crate::widget::svg::render_widget_to_svg(&mut grid, rect);
        let boxes = crate::widget::svg::text_ink_boxes(&svg);
        assert_eq!(
            boxes.len(),
            1,
            "only the scrolled-to cell must be painted as text; a missing row-scroll offset paints \
             the blank lead row instead"
        );
        let (bx, by, _, _) = boxes[0];
        assert!(
            bx >= first.x && bx < first.x + first.width as i32,
            "the value must land in the scrolled-to row's first column ({first:?}), got x={bx}"
        );
        assert!(
            by >= first.y && by < first.y + first.height as i32,
            "the value must land in the scrolled-to row's box ({first:?}), got y={by}"
        );
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
        assert_eq!(grid.default_column_width(), 120);
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

        /// A revision-less source: announces change without versioning the projection.
        fn with_revision(rows: usize, cols: usize, revision: u64) -> Self {
            let source = Self::new(rows, cols);
            source.rev.store(revision, Ordering::Relaxed);
            source
        }

        fn touch(&self) {
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

    /// The trait's `revision()` default is `0`; a source that reports change only through its
    /// signal must still be cacheable, and that signal must invalidate the window.
    #[test]
    fn a_revision_less_source_is_cached_and_invalidated_by_its_signal() {
        let mut grid = DataGrid::new(Rect::new(0, 0, 240, 60));
        let source = Arc::new(RevisionSource::with_revision(10, 4, 0));
        grid.set_data_source(source.clone());

        let first = grid.fetch_visible_cells_shared();
        let calls_after_first = source.call_count.load(Ordering::Relaxed);
        assert!(calls_after_first > 0);

        let second = grid.fetch_visible_cells_shared();
        assert!(Rc::ptr_eq(&first, &second), "the cached window is shared, not re-fetched");
        assert_eq!(
            source.call_count.load(Ordering::Relaxed),
            calls_after_first,
            "the signal makes this source observable"
        );

        source.touch();
        let _ = grid.fetch_visible_cells_shared();
        assert!(
            source.call_count.load(Ordering::Relaxed) > calls_after_first,
            "a reported change must invalidate the window"
        );
    }

    #[test]
    fn row_height_and_column_width_minimum_clamp() {
        let mut grid = DataGrid::new(Rect::new(0, 0, 800, 600));
        grid.set_data_source(Arc::new(StaticSource { rows: 3, cols: 3, data: vec![] }));

        grid.set_row_height(0);
        assert_eq!(grid.row_height(), 1);

        grid.set_row_height(25);
        assert_eq!(grid.row_height(), 25);

        grid.set_default_column_width(0);
        assert_eq!(grid.default_column_width(), 1);

        grid.set_default_column_width(100);
        assert_eq!(grid.default_column_width(), 100);
    }

    /// N-V-03 / N-V-04: a full-width control with a full-height row must not overflow the
    /// `height + row_height - 1` capacity computation.
    #[test]
    fn extreme_dimensions_do_not_overflow_visible_capacity() {
        // A very wide/tall control with an enormous row/column height: the old
        // `height + row_height - 1` and `width + column_width - 1` overflowed (debug panic).
        let mut grid = DataGrid::new(Rect::new(0, 0, u32::MAX, u32::MAX));
        grid.set_row_height(u32::MAX);
        grid.set_default_column_width(u32::MAX);
        // Reaching the assertions means neither capacity calculation panicked.
        assert_eq!(grid.visible_row_capacity(), 1);
        assert_eq!(grid.visible_column_capacity(), 1);
    }

    /// One wheel notch scrolls one row, whether the host sends the documented
    /// notch delta or the legacy 120-unit tick.
    #[test]
    fn wheel_scrolls_one_row_per_notch() {
        let mut grid = DataGrid::new(Rect::new(0, 0, 400, 300));
        grid.set_data_source(Arc::new(StaticSource { rows: 10, cols: 2, data: vec![] }));

        grid.handle_event(&Event::wheel(0, -120, 0));
        assert_eq!(grid.scroll_row(), 1, "one legacy notch is one row");

        grid.handle_event(&Event::wheel(0, -1, 0));
        assert_eq!(grid.scroll_row(), 2, "one documented notch is one row");

        grid.handle_event(&Event::wheel(0, 120, 0));
        assert_eq!(grid.scroll_row(), 1, "the opposite notch scrolls back one row");
    }

    /// Full-model projection, header sorting, column modes, alternating rows and keyboard
    /// navigation are wired.
    #[test]
    fn full_model_projection_header_sort_and_keyboard_are_wired() {
        let data = vec![vec!["b".to_string()], vec!["c".to_string()], vec!["a".to_string()]];
        let mut grid = DataGrid::new(Rect::new(0, 0, 300, 120));
        grid.set_data_source(Arc::new(StaticSource { rows: 3, cols: 1, data }));
        grid.set_full_model_projection(true);
        grid.set_sort_specs(vec![SortSpec { column: 0, descending: false }]);
        let rows = grid.fetch_visible_cells();
        assert_eq!(rows.first().and_then(|row| row.first().cloned()), Some(Some("a".to_string())));

        grid.set_column_width_mode(ColumnWidthMode::Stretch);
        assert_eq!(grid.column_width_mode(), ColumnWidthMode::Stretch);
        grid.set_alternating_rows(true);
        assert!(grid.alternating_rows());
        grid.set_show_scrollbars(true);
        assert!(grid.show_scrollbars());

        let header = grid.header_band();
        let clicked = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(usize::MAX));
        let sink = std::sync::Arc::clone(&clicked);
        grid.header_clicked.connect(move |column| {
            sink.store(*column, std::sync::atomic::Ordering::SeqCst);
        });
        grid.handle_event(&Event::MousePress {
            pos: Point::new(header.x + 1, header.y + 1),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(
            clicked.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "a header click names its column"
        );

        grid.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Tab });
        grid.handle_event(&Event::KeyPress { key: crate::event::key_codes::DOWN, modifiers: 0 });
        assert_eq!(grid.current_cell(), Some((1, 0)), "Down moves the current cell one row");
        grid.handle_event(&Event::KeyPress { key: crate::event::key_codes::DOWN, modifiers: 0 });
        assert_eq!(grid.current_cell(), Some((2, 0)), "Down reaches the last row");
        let _ = grid.fetch_visible_cells();
    }

    /// F-P01: the column prefix table is built once per width revision, not summed per cell.
    #[test]
    fn column_prefix_width_is_cached_until_a_width_changes() {
        let mut grid = static_grid(50, 5);
        grid.set_column_width_mode(ColumnWidthMode::Interactive);
        assert!(grid.set_column_width(1, 80));
        let expected = 120 + 80 + 120 + 120;
        assert_eq!(grid.column_prefix_width(4), expected);
        let builds = grid.column_prefix_rebuilds.get();
        // Repeated reads are table lookups: no rebuild.
        for _ in 0..100 {
            assert_eq!(grid.column_prefix_width(4), expected);
            assert_eq!(grid.column_prefix_width(2), 200);
        }
        assert_eq!(grid.column_prefix_rebuilds.get(), builds, "the table was reused");

        // A width change rebuilds exactly once, and the new widths are visible immediately.
        assert!(grid.set_column_width(1, 50));
        assert_eq!(grid.column_prefix_width(4), 120 + 50 + 120 + 120);
        assert_eq!(grid.column_prefix_rebuilds.get(), builds + 1);
    }

    /// F-P02: the draw path's view addresses the cached projection instead of cloning the window.
    #[test]
    fn draw_view_shares_the_projection_across_scroll_positions() {
        let mut grid = static_grid(200, 4);
        // The shared-projection view is the explicit full-model mode; the default virtualized
        // mode fetches only the window (V-001).
        grid.set_full_model_projection(true);
        let first = grid.fetch_visible_window_view();
        assert!(!first.is_empty());
        let same_projection = Rc::clone(&first.rows);

        grid.set_scroll_row(10);
        let second = grid.fetch_visible_window_view();
        assert!(
            Rc::ptr_eq(&same_projection, &second.rows),
            "a scroll reuses the same projected matrix instead of slicing a copy"
        );
        assert_ne!(first.row_start, second.row_start, "the window did move");
        // The window starts at `scroll - overscan`, so the viewport's first row is index 2.
        assert_eq!(second.row(2).and_then(|row| row[0].clone()), Some("10:0".to_string()));
    }

    /// F-P03: a source with neither a revision nor a signal is projected once, and
    /// `refresh_data` is the explicit invalidation point.
    #[test]
    fn non_observable_projection_is_cached_until_refresh_data() {
        struct CountingSource {
            rows: usize,
            columns: usize,
            fetches: Arc<AtomicU64>,
        }
        impl IncrementalTableDataSource for CountingSource {
            fn row_count(&self) -> usize {
                self.rows
            }
            fn column_count(&self) -> usize {
                self.columns
            }
            fn data(&self, row: usize, column: usize) -> Option<String> {
                Some(format!("{row}:{column}"))
            }
            fn fetch_window(
                &self,
                row_start: usize,
                row_len: usize,
                column_start: usize,
                column_len: usize,
            ) -> Vec<Vec<Option<String>>> {
                self.fetches.fetch_add(1, Ordering::Relaxed);
                (row_start..row_start.saturating_add(row_len).min(self.rows))
                    .map(|row| {
                        (column_start..column_start.saturating_add(column_len).min(self.columns))
                            .map(|column| Some(format!("{row}:{column}")))
                            .collect()
                    })
                    .collect()
            }
        }

        let fetches = Arc::new(AtomicU64::new(0));
        let mut grid = DataGrid::new(Rect::new(0, 0, 400, 160));
        // The full-model projection is what caches a non-observable source; without it the
        // virtualized mode refetches its window by design (V-001).
        grid.set_full_model_projection(true);
        grid.set_data_source(Arc::new(CountingSource {
            rows: 500,
            columns: 3,
            fetches: Arc::clone(&fetches),
        }));
        assert!(!grid.data_is_observable, "the fixture has no invalidation contract");

        let _ = grid.fetch_visible_cells();
        let after_first = fetches.load(Ordering::Relaxed);
        assert!(after_first > 0);

        grid.set_scroll_row(5);
        let _ = grid.fetch_visible_cells();
        grid.set_scroll_row(9);
        let _ = grid.fetch_visible_cells();
        assert_eq!(
            fetches.load(Ordering::Relaxed),
            after_first,
            "scrolling must hit the cached projection"
        );

        grid.refresh_data();
        let _ = grid.fetch_visible_cells();
        assert!(
            fetches.load(Ordering::Relaxed) > after_first,
            "refresh_data is how a non-observable source reports change"
        );
    }

    /// F-P04: elided header labels are cached and reused across frames.
    #[test]
    fn header_labels_are_elided_once_and_cached() {
        let mut grid = static_grid(4, 3);
        grid.set_header_labels(&["Alpha", "Beta", "Gamma"]);
        let _ = crate::widget::svg::render_to_svg(&mut grid);
        let cached = grid.header_text_cache.borrow();
        assert_eq!(cached.entries.get(&(0, 120)).map(String::as_str), Some("Alpha"));
        assert_eq!(cached.entries.get(&(2, 120)).map(String::as_str), Some("Gamma"));

        // A label change invalidates the cache; the next paint stores the new title.
        drop(cached);
        grid.set_header_labels(&["Renamed", "Beta", "Gamma"]);
        let _ = crate::widget::svg::render_to_svg(&mut grid);
        let cached = grid.header_text_cache.borrow();
        assert_eq!(cached.entries.get(&(0, 120)).map(String::as_str), Some("Renamed"));
    }

    /// V-001/V-002: the default is a virtualized index projection, and its visible order is the
    /// same as the explicit full-model projection.
    #[test]
    fn the_default_projection_is_virtualized_and_matches_the_full_order() {
        let data = vec![vec!["c".to_string()], vec!["a".to_string()], vec!["b".to_string()]];
        let mut grid = DataGrid::new(Rect::new(0, 0, 300, 120));
        grid.set_data_source(Arc::new(StaticSource { rows: 3, cols: 1, data }));
        assert!(!grid.full_model_projection(), "virtualized rendering is the default");
        grid.set_sort_specs(vec![SortSpec { column: 0, descending: false }]);

        let virtualized = grid.fetch_visible_cells();
        assert!(grid.full_projection_cache.is_none(), "no whole-model row matrix is held");
        let index = Rc::clone(&grid.index_projection_cache.as_ref().expect("index cache").1);
        assert_eq!(*index, vec![1, 2, 0], "the projection is an index, not cloned rows");
        assert_eq!(virtualized[0][0], Some("a".to_string()));

        grid.set_full_model_projection(true);
        let materialised = grid.fetch_visible_cells();
        assert_eq!(
            virtualized, materialised,
            "both strategies paint the same rows in the same order"
        );
    }

    /// V-003: a header click cycles ascending -> descending -> unsorted.
    #[test]
    fn a_header_click_cycles_three_sort_states() {
        let mut grid = static_grid(3, 1);
        let header = grid.header_band();
        let pos = Point::new(header.x + 2, header.y + 2);

        grid.handle_event(&Event::MousePress { pos, button: 1, modifiers: 0 });
        assert_eq!(grid.sort_specs(), &[SortSpec { column: 0, descending: false }]);
        grid.handle_event(&Event::MousePress { pos, button: 1, modifiers: 0 });
        assert_eq!(grid.sort_specs(), &[SortSpec { column: 0, descending: true }]);
        grid.handle_event(&Event::MousePress { pos, button: 1, modifiers: 0 });
        assert!(grid.sort_specs().is_empty(), "the third state restores the source order");
    }

    /// V-004: row and column membership is a set lookup, and the stored unit is the row or the
    /// column rather than every covered cell.
    #[test]
    fn row_and_column_selection_are_one_unit_lookups() {
        let mut grid = static_grid(4, 3);
        grid.set_selection_behavior(DataGridSelectionBehavior::Rows);
        grid.apply_selection_click(1, 2, 0);
        assert_eq!(grid.selected_cells(), vec![(1, 0)], "a row unit is (row, 0)");
        assert!(grid.selection_contains(1, 0));
        assert!(grid.selection_contains(1, 2), "every column of the row is selected");
        assert!(!grid.selection_contains(2, 0));

        grid.set_selection_behavior(DataGridSelectionBehavior::Columns);
        grid.apply_selection_click(2, 1, 0);
        assert_eq!(grid.selected_cells(), vec![(0, 1)], "a column unit is (0, column)");
        assert!(grid.selection_contains(0, 1));
        assert!(grid.selection_contains(3, 1));
        assert!(!grid.selection_contains(0, 2));
    }

    /// V-014: a hidden column occupies no width, closes the gap, and is skipped by the hit test.
    #[test]
    fn hiding_a_column_removes_its_slot_without_renumbering() {
        let mut grid = static_grid(3, 3);
        assert_eq!(grid.column_view_x(2), 240);
        assert!(grid.set_column_hidden(1, true));
        assert_eq!(grid.column_width_at(1), 0);
        assert_eq!(grid.column_view_x(2), 120, "the following column closes the gap");
        assert_eq!(grid.column_at_dx(130), Some(2), "a press past the gap names column 2");
        assert_eq!(grid.hidden_columns(), vec![1]);
        assert!(grid.set_column_hidden(1, false));
        assert_eq!(grid.column_view_x(2), 240);
        assert_eq!(grid.hidden_columns(), Vec::<usize>::new());
    }

    /// V-014: a merged run answers as its origin, at the paint geometry and the hit test.
    #[test]
    fn a_merged_run_resolves_to_its_origin() {
        let mut grid = static_grid(3, 3);
        assert!(grid.set_cell_span(0, 0, 2, 2));
        assert_eq!(grid.cell_spans().len(), 1);
        let inside = grid.cell_rect(1, 1);
        let centre = Point::new(inside.x + 1, inside.y + 1);
        assert_eq!(grid.cell_at(centre), Some((0, 0)));
        assert_eq!(grid.cell_span(1, 1), Some(CellSpan::new(0, 0, 2, 2)));
        assert_eq!(grid.cell_span(2, 2), None);
        assert!(!grid.set_cell_span(99, 0, 1, 1), "an anchor outside the model is refused");
        grid.clear_cell_spans();
        assert!(grid.cell_spans().is_empty());
    }

    /// V-014: each extra header level takes another title strip, and removing it restores the
    /// historical geometry.
    #[test]
    fn extra_header_levels_take_their_own_strips() {
        let mut grid = static_grid(3, 3);
        let base_y = grid.cells_box().y;
        grid.set_extra_header_rows(vec![vec!["Top".to_string()]]);
        assert_eq!(grid.header_strip_height(), GRID_HEADER_HEIGHT * 2);
        assert_eq!(grid.cells_box().y, base_y + GRID_HEADER_HEIGHT as i32);
        assert_eq!(grid.header_band().height, GRID_HEADER_HEIGHT * 2);
        grid.set_extra_header_rows(Vec::new());
        assert_eq!(grid.cells_box().y, base_y);
        assert_eq!(grid.header_band().height, GRID_HEADER_HEIGHT);
    }

    /// V-014: the tooltip callback is asked per cell and is removed by `clear_cell_tooltip`.
    #[test]
    fn a_cell_tooltip_comes_from_the_installed_callback() {
        let mut grid = static_grid(3, 3);
        grid.set_cell_tooltip(Arc::new(|row, column| {
            (row == 1 && column == 2).then(|| "the tip".to_string())
        }));
        assert_eq!(grid.cell_tooltip(1, 2).as_deref(), Some("the tip"));
        assert_eq!(grid.cell_tooltip(0, 0), None);
        grid.clear_cell_tooltip();
        assert_eq!(grid.cell_tooltip(1, 2), None);
    }

    /// V-014: a delegate is consulted for each painted cell, and its `false` answer keeps the
    /// direct rendering.
    #[test]
    fn a_cell_delegate_is_invoked_for_painted_cells() {
        struct CountingDelegate(std::sync::Arc<AtomicUsize>);
        impl DataGridCellDelegate for CountingDelegate {
            fn paint(
                &self,
                _context: &mut RenderContext,
                _cell: Rect,
                _row: usize,
                _column: usize,
                _text: Option<&str>,
            ) -> bool {
                self.0.fetch_add(1, Ordering::Relaxed);
                false
            }
        }

        let calls = Arc::new(AtomicUsize::new(0));
        let mut grid = static_grid(3, 3);
        grid.set_cell_delegate(Arc::new(CountingDelegate(Arc::clone(&calls))));
        assert!(grid.has_cell_delegate());
        let _ = crate::widget::svg::render_to_svg(&mut grid);
        assert!(calls.load(Ordering::Relaxed) > 0, "the delegate is called by the paint path");
        grid.clear_cell_delegate();
        assert!(!grid.has_cell_delegate());
    }
}
