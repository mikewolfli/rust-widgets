// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Unified incremental data source protocol for modern data views.
//!
//! BLUE9 R3 groundwork: provides a common pull-based window API that can back
//! virtualized list/table/tree widgets without duplicating model traversal logic.

use std::sync::Arc;

use crate::signal::{GenericSignal, Signal};
use crate::widget::view_widgets::list_view::ListModel;
use crate::widget::view_widgets::table_widget::TableModel;
use crate::widget::view_widgets::tree_view::TreeModel;

/// The deepest inline ancestor chain a [`ModelIndex`] can carry.
///
/// The chain is inline (and `Copy`) rather than a heap-allocated `Vec`, because a `ModelIndex`
/// is produced per cell by window fetches and by hit tests; an allocation per address would be
/// paid on every pointer move. A model deeper than this cap still resolves its nearer ancestors
/// through [`IncrementalTableDataSource::parent_index`]; the inline chain keeps the *outermost*
/// rows that still fit, so `path()` never silently drops the segment next to `parent`.
pub const MODEL_INDEX_MAX_DEPTH: usize = 8;

/// A logical cell address: an optional parent row plus a row and column.
///
/// The row index is relative to `parent` (`None` means a root row). This is the addressing
/// form a tree-shaped model needs and a flat row/column pair cannot express.
///
/// # Why there is an ancestor chain
///
/// A single `parent` link could name the immediate parent only, so a caller could not address a
/// cell *by its full path* — the grandparent was unreachable, and two cells at the same
/// `(parent, row)` under different ancestors were indistinguishable. [`Self::ancestry`] carries
/// the ancestors above `parent`, outermost first, and [`Self::path`] returns the complete row
/// path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ModelIndex {
    /// The parent row, or `None` for a root row.
    pub parent: Option<usize>,
    /// Row within the parent (or the root list).
    pub row: usize,
    /// Column within the row.
    pub column: usize,
    /// The rows above `parent`, outermost first; `None` marks the unused tail of the chain.
    pub ancestry: [Option<usize>; MODEL_INDEX_MAX_DEPTH],
}

impl ModelIndex {
    /// A root cell at `row`/`column`.
    pub fn root(row: usize, column: usize) -> Self {
        Self { parent: None, row, column, ancestry: [None; MODEL_INDEX_MAX_DEPTH] }
    }

    /// A cell below `parent`.
    pub fn child(parent: usize, row: usize, column: usize) -> Self {
        Self { parent: Some(parent), row, column, ancestry: [None; MODEL_INDEX_MAX_DEPTH] }
    }

    /// A cell addressed by a full row `path`.
    ///
    /// The last element is the row itself, the one before it is `parent`, and the preceding
    /// elements are the ancestors. An empty path addresses the root list at `column`, i.e. row
    /// `0` with no parent. A path longer than [`MODEL_INDEX_MAX_DEPTH`] keeps the nearest
    /// ancestors (the suffix), because those are the ones that disambiguate `parent`.
    pub fn nested(path: &[usize], column: usize) -> Self {
        match path.split_last() {
            None => Self::root(0, column),
            Some((row, rest)) => {
                let parent = rest.last().copied();
                let above_parent = &rest[..rest.len().saturating_sub(1)];
                let start = above_parent.len().saturating_sub(MODEL_INDEX_MAX_DEPTH);
                let mut ancestry = [None; MODEL_INDEX_MAX_DEPTH];
                for (slot, value) in ancestry.iter_mut().zip(above_parent[start..].iter().copied())
                {
                    *slot = Some(value);
                }
                Self { parent, row: *row, column, ancestry }
            }
        }
    }

    /// The complete row path, outermost first, ending with [`Self::row`].
    ///
    /// A root cell answers `vec![row]`; a child answers `vec![grandparent, parent, row]` and so
    /// on. The expansion allocates, so a hot path that only needs the immediate parent reads
    /// [`Self::parent`] directly.
    pub fn path(&self) -> Vec<usize> {
        let mut path = Vec::with_capacity(1 + self.ancestry.len());
        path.extend(self.ancestry.iter().flatten().copied());
        if let Some(parent) = self.parent {
            path.push(parent);
        }
        path.push(self.row);
        path
    }

    /// How many links sit between this cell and the root list: `0` for a root cell, `1` for a
    /// child, and so on.
    pub fn depth(&self) -> usize {
        self.ancestry.iter().flatten().count() + usize::from(self.parent.is_some())
    }

    /// Whether this cell is a root cell, i.e. it has no parent and no ancestors.
    pub fn is_root(&self) -> bool {
        self.parent.is_none() && self.ancestry.iter().all(Option::is_none)
    }
}

/// The order a sortable source was last sorted in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SortOrder {
    /// Smallest/root-first ordering.
    Ascending,
    /// Largest/leaf-first ordering.
    Descending,
}

/// A payload-carrying fine-grained change: one cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CellChange {
    /// The row the change landed on.
    pub row: usize,
    /// The column the change landed on.
    pub column: usize,
    /// The role whose value changed, when the model can name it.
    pub role: Option<DataRole>,
}

impl CellChange {
    /// A change to the display value of `row`/`column`.
    pub fn display(row: usize, column: usize) -> Self {
        Self { row, column, role: Some(DataRole::Display) }
    }
}

/// A payload-carrying structural change: a contiguous run of rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RowRange {
    /// The first row the change starts at.
    pub start: usize,
    /// How many rows the change covers.
    pub count: usize,
}

impl RowRange {
    /// A run of `count` rows beginning at `start`.
    pub fn new(start: usize, count: usize) -> Self {
        Self { start, count }
    }
}

/// The purpose a value is being read for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DataRole {
    /// The text a table cell paints.
    Display,
    /// The text an assistive technology announces.
    AccessibleText,
    /// The text an editor starts with.
    Edit,
    /// A decoration token (an icon name, for example).
    Decoration,
    /// The checked state, spelled `"true"`/`"false"`.
    CheckState,
    /// A host-defined role.
    User(u32),
}

/// What a cell allows.
///
/// A plain struct of booleans rather than a bitmask: models are written by callers, and
/// four named flags are harder to mis-combine than an integer whose bit positions must be
/// remembered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellFlags {
    /// The cell can be selected.
    pub selectable: bool,
    /// The cell can be edited.
    pub editable: bool,
    /// The cell is enabled.
    pub enabled: bool,
    /// The cell carries a check state.
    pub checkable: bool,
}

impl Default for CellFlags {
    fn default() -> Self {
        Self { selectable: true, editable: false, enabled: true, checkable: false }
    }
}

impl CellFlags {
    /// The permissive set: selectable, editable and enabled.
    pub const EDITABLE: Self =
        Self { selectable: true, editable: true, enabled: true, checkable: false };
}

/// Pull-based data source protocol used by modern data controls.
pub trait IncrementalTableDataSource: Send + Sync {
    /// Total rows available in source.
    fn row_count(&self) -> usize;
    /// Total columns available in source.
    fn column_count(&self) -> usize;
    /// The heading a column should be labelled with, if the source names its columns.
    ///
    /// # Why this is optional
    ///
    /// Not every projection has column names — a matrix, a sparse grid or a sheet without a
    /// header row genuinely has none — but a *table* usually does, and without this the
    /// consumer had no way to ask. `grid_table` therefore labelled every column from its index
    /// (`"Column 1"`, `"Column 2"`, ...) regardless of the data, which is a placeholder a user
    /// cannot act on: two grids over different data were indistinguishable at the header row.
    ///
    /// Returning `None` is the honest answer for a source that has no names, and lets the
    /// consumer fall back to an index label rather than inventing one. The default is `None`,
    /// so an existing implementor keeps working unchanged (principle #21).
    fn column_name(&self, column: usize) -> Option<String> {
        let _ = column;
        None
    }
    /// Returns cell text for a row/column if available.
    fn data(&self, row: usize, column: usize) -> Option<String>;

    /// Returns a cell value for a specific purpose.
    ///
    /// The default answers [`DataRole::Display`] from [`Self::data`] and `None` for every
    /// other role, so a model that has only display text behaves exactly as before.
    fn data_role(&self, row: usize, column: usize, role: DataRole) -> Option<String> {
        match role {
            DataRole::Display => self.data(row, column),
            _ => None,
        }
    }

    /// Returns a cell value addressed by a [`ModelIndex`].
    ///
    /// The default delegates to [`Self::data_role`] for a root index and returns `None` for
    /// a child index, so a flat model needs no change and a tree model overrides this. An index
    /// carrying any ancestor is hierarchical even when its immediate `parent` is `None`, so the
    /// check reads the whole path rather than the single parent link.
    fn data_at(&self, index: ModelIndex, role: DataRole) -> Option<String> {
        if index.is_root() {
            self.data_role(index.row, index.column, role)
        } else {
            None
        }
    }

    /// Returns the parent row of `row`, if the model is hierarchical.
    fn parent_index(&self, row: usize) -> Option<usize> {
        let _ = row;
        None
    }

    /// Writes a cell value.
    ///
    /// Returns `false` by default, which is the honest answer for a read-only source. A
    /// writable model overrides this and emits a fine-grained change signal afterwards.
    fn set_data(&mut self, row: usize, column: usize, value: &str) -> bool {
        let _ = (row, column, value);
        false
    }

    /// What the cell allows.
    fn flags(&self, row: usize, column: usize) -> CellFlags {
        let _ = (row, column);
        CellFlags::default()
    }

    /// A drag payload for the cell, if the model supplies one.
    fn mime_data(&self, row: usize, column: usize) -> Option<String> {
        let _ = (row, column);
        None
    }

    /// A signal emitted when one cell changes.
    ///
    /// # Why the unit signal is kept and a payload signal added
    ///
    /// A `GenericSignal` cannot say *which* cell changed, so a view that listens to it can only
    /// repaint everything. [`Self::cell_changed_detail_signal`] is the additive, payload-carrying
    /// counterpart: a source that can name the address publishes both (or migrates to the detail
    /// signal), while every existing implementor and every existing listener keeps working.
    fn cell_changed_signal(&self) -> Option<&GenericSignal> {
        None
    }

    /// A signal emitted when one cell changes, carrying its address and optional role.
    fn cell_changed_detail_signal(&self) -> Option<&Signal<CellChange>> {
        None
    }

    /// A signal emitted when rows are inserted.
    fn rows_inserted_signal(&self) -> Option<&GenericSignal> {
        None
    }

    /// A signal emitted when rows are inserted, carrying the affected run.
    fn rows_inserted_detail_signal(&self) -> Option<&Signal<RowRange>> {
        None
    }

    /// A signal emitted when rows are removed.
    fn rows_removed_signal(&self) -> Option<&GenericSignal> {
        None
    }

    /// A signal emitted when rows are removed, carrying the affected run.
    fn rows_removed_detail_signal(&self) -> Option<&Signal<RowRange>> {
        None
    }

    /// A signal emitted when the whole model is reset.
    fn rows_reset_signal(&self) -> Option<&GenericSignal> {
        None
    }

    /// A signal emitted when the whole model is reset, carrying no payload.
    ///
    /// A `Signal<()>` rather than a bare `GenericSignal`: the hub can key both, but a host that
    /// collects change notifications into one typed stream needs the `Signal` spelling.
    fn rows_reset_detail_signal(&self) -> Option<&Signal<()>> {
        None
    }

    /// Inserts an empty row at `row`, returning whether the source accepted the edit.
    ///
    /// The default refuses: a pull-only projection (a filtered view, a file listing) genuinely
    /// cannot be written through. A writable source overrides this and emits
    /// [`Self::rows_inserted_detail_signal`] so a view can update its projection incrementally.
    fn insert_row(&mut self, row: usize) -> bool {
        let _ = row;
        false
    }

    /// Removes the row at `row`, returning whether the source accepted the edit.
    fn remove_row(&mut self, row: usize) -> bool {
        let _ = row;
        false
    }

    /// Moves the row at `from` to `to`, returning whether the source accepted the edit.
    ///
    /// The move is expressed against the source's own row space. A source that only owns a
    /// display projection refuses it and lets the view reorder locally, which is what
    /// `VirtualList::move_row` already does.
    fn move_row(&mut self, from: usize, to: usize) -> bool {
        let _ = (from, to);
        false
    }

    /// Whether the source can produce rows beyond the ones it currently exposes.
    ///
    /// A paged/streaming source answers `true` so a view can keep its "load more" affordance
    /// live; the default is `false`, which is the honest answer for a finite in-memory model.
    fn can_fetch_more(&self) -> bool {
        false
    }

    /// Whether more rows can be fetched *below* `path` in a hierarchical projection.
    ///
    /// A flat source cannot address a node, so the default defers to the source-wide
    /// [`Self::can_fetch_more`]. The path form is a distinct name rather than an overload because
    /// Rust has no method overloading and the zero-argument form is a published API: renaming it
    /// would break every existing implementor for no behavioural gain.
    fn can_fetch_more_path(&self, path: &[usize]) -> bool {
        let _ = path;
        self.can_fetch_more()
    }

    /// Asks the source to load more rows below `path`.
    ///
    /// **Synchronous and host-driven.** There is no async runtime here: a source that must wait
    /// returns immediately and reports completion through its revision/change signal. `&mut self`
    /// matches [`Self::insert_row`] and the other editors; a source behind an `Arc<dyn …>` that
    /// needs interior mutation can guard it internally.
    fn fetch_more(&mut self, path: &[usize]) {
        let _ = path;
    }

    /// Whether the source accepts [`Self::insert_row`].
    fn can_insert_rows(&self) -> bool {
        false
    }

    /// Whether the source accepts [`Self::remove_row`].
    fn can_remove_rows(&self) -> bool {
        false
    }

    /// Whether the source accepts [`Self::move_row`].
    fn can_move_rows(&self) -> bool {
        false
    }

    /// Whether the source can sort itself.
    fn can_sort(&self) -> bool {
        false
    }

    /// Sorts the source by `column` in `order`, returning whether the source accepted it.
    ///
    /// Sorting the source (rather than the view's projection) is what keeps row indices stable
    /// across every consumer of the same model. A view that only reorders its own display keeps
    /// using `VirtualList::move_row`.
    fn sort(&mut self, column: usize, order: SortOrder) -> bool {
        let _ = (column, order);
        false
    }

    /// The column and order the source is currently sorted by, when it can say.
    fn sort_indicator(&self) -> Option<(usize, SortOrder)> {
        None
    }

    /// Writes cell text for a row/column into `out`, returning whether the cell exists.
    ///
    /// # Why this exists
    ///
    /// [`Self::data`] returns an owned `String`, so a draw pass over an R×C window allocated
    /// one string per cell per frame. A consumer that only reads a cell (the grid's paint loop,
    /// for example) reuses a single buffer through this method instead. The default keeps
    /// existing implementors working unchanged: it delegates to `data` and copies. An implementor
    /// that already holds its cells can override it and allocate nothing.
    fn fill_cell(&self, row: usize, column: usize, out: &mut String) -> bool {
        match self.data(row, column) {
            Some(value) => {
                out.push_str(&value);
                true
            }
            None => false,
        }
    }
    /// Optional monotonic revision for cache invalidation.
    fn revision(&self) -> u64 {
        0
    }
    /// Optional signal emitted when source projection changes.
    fn data_changed_signal(&self) -> Option<&GenericSignal> {
        None
    }
    /// Fetches a bounded window from the source.
    fn fetch_window(
        &self,
        row_start: usize,
        row_len: usize,
        column_start: usize,
        column_len: usize,
    ) -> Vec<Vec<Option<String>>> {
        let total_rows = self.row_count();
        let total_columns = self.column_count();

        if row_start >= total_rows || column_start >= total_columns {
            return Vec::new();
        }

        let row_end = row_start.saturating_add(row_len).min(total_rows);
        let col_end = column_start.saturating_add(column_len).min(total_columns);
        let mut window = Vec::with_capacity(row_end.saturating_sub(row_start));

        for row in row_start..row_end {
            let mut line = Vec::with_capacity(col_end.saturating_sub(column_start));
            for column in column_start..col_end {
                line.push(self.data(row, column));
            }
            window.push(line);
        }

        window
    }
}

/// Adapter that projects `ListModel` into the unified tabular data source protocol.
pub struct ListModelDataSource {
    model: Arc<dyn ListModel>,
}

impl ListModelDataSource {
    /// Creates a new list model adapter.
    pub fn new(model: Arc<dyn ListModel>) -> Self {
        Self { model }
    }

    /// Returns underlying list model.
    pub fn model_ref(&self) -> &Arc<dyn ListModel> {
        &self.model
    }
}

impl IncrementalTableDataSource for ListModelDataSource {
    fn row_count(&self) -> usize {
        self.model.row_count()
    }

    fn column_count(&self) -> usize {
        1
    }

    fn data(&self, row: usize, column: usize) -> Option<String> {
        if column == 0 {
            self.model.data(row)
        } else {
            None
        }
    }

    fn fill_cell(&self, row: usize, column: usize, out: &mut String) -> bool {
        if column == 0 {
            self.model.fill_data(row, out)
        } else {
            false
        }
    }

    fn data_changed_signal(&self) -> Option<&GenericSignal> {
        self.model.data_changed_signal()
    }
}

/// Adapter that projects `TreeModel` into the unified tabular data source protocol.
pub struct TreeModelDataSource {
    model: Arc<dyn TreeModel>,
}

impl TreeModelDataSource {
    /// Creates a new tree model adapter.
    pub fn new(model: Arc<dyn TreeModel>) -> Self {
        Self { model }
    }

    /// Returns underlying tree model.
    pub fn model_ref(&self) -> &Arc<dyn TreeModel> {
        &self.model
    }
}

impl IncrementalTableDataSource for TreeModelDataSource {
    fn row_count(&self) -> usize {
        self.model.node_count()
    }

    fn column_count(&self) -> usize {
        1
    }

    fn data(&self, row: usize, column: usize) -> Option<String> {
        if column == 0 {
            self.model.node_path(row)
        } else {
            None
        }
    }

    fn fill_cell(&self, row: usize, column: usize, out: &mut String) -> bool {
        if column == 0 {
            self.model.fill_node_path(row, out)
        } else {
            false
        }
    }

    fn data_changed_signal(&self) -> Option<&GenericSignal> {
        self.model.data_changed_signal()
    }
}

/// Adapter that projects `TableModel` into the unified tabular data source protocol.
pub struct TableModelDataSource {
    model: Arc<dyn TableModel>,
}

impl TableModelDataSource {
    /// Creates a new table model adapter.
    pub fn new(model: Arc<dyn TableModel>) -> Self {
        Self { model }
    }

    /// Returns underlying table model.
    pub fn model_ref(&self) -> &Arc<dyn TableModel> {
        &self.model
    }
}

impl IncrementalTableDataSource for TableModelDataSource {
    fn row_count(&self) -> usize {
        self.model.row_count()
    }

    fn column_count(&self) -> usize {
        self.model.column_count()
    }

    fn data(&self, row: usize, column: usize) -> Option<String> {
        self.model.data(row, column)
    }

    fn fill_cell(&self, row: usize, column: usize, out: &mut String) -> bool {
        self.model.fill_data(row, column, out)
    }

    fn data_changed_signal(&self) -> Option<&GenericSignal> {
        self.model.data_changed_signal()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StaticList;

    impl ListModel for StaticList {
        fn row_count(&self) -> usize {
            3
        }

        fn data(&self, row: usize) -> Option<String> {
            match row {
                0 => Some("A".to_string()),
                1 => Some("B".to_string()),
                2 => Some("C".to_string()),
                _ => None,
            }
        }
    }

    struct StaticTree;

    impl TreeModel for StaticTree {
        fn node_count(&self) -> usize {
            2
        }

        fn node_path(&self, index: usize) -> Option<String> {
            match index {
                0 => Some("root".to_string()),
                1 => Some("root/child".to_string()),
                _ => None,
            }
        }
    }

    struct StaticTable;

    impl TableModel for StaticTable {
        fn row_count(&self) -> usize {
            2
        }

        fn column_count(&self) -> usize {
            3
        }

        fn data(&self, row: usize, column: usize) -> Option<String> {
            Some(format!("{}:{}", row, column))
        }
    }

    #[test]
    fn list_adapter_projects_single_column_window() {
        let source = ListModelDataSource::new(Arc::new(StaticList));

        assert_eq!(source.row_count(), 3);
        assert_eq!(source.column_count(), 1);
        assert_eq!(source.data(1, 0), Some("B".to_string()));
        assert_eq!(source.data(1, 1), None);

        let window = source.fetch_window(1, 4, 0, 1);
        assert_eq!(window.len(), 2);
        assert_eq!(window[0][0], Some("B".to_string()));
        assert_eq!(window[1][0], Some("C".to_string()));
    }

    /// `fill_cell` is the zero-allocation counterpart of `data`: it must report the same
    /// presence and write the same text, so a paint loop can switch to it safely (VW-N02).
    #[test]
    fn fill_cell_agrees_with_data() {
        let source = ListModelDataSource::new(Arc::new(StaticList));
        let mut out = String::new();

        assert!(source.fill_cell(1, 0, &mut out));
        assert_eq!(out, "B");

        out.clear();
        assert!(!source.fill_cell(9, 0, &mut out));
        assert!(out.is_empty(), "an absent cell must not append anything");

        out.clear();
        assert!(!source.fill_cell(0, 1, &mut out));
        assert!(out.is_empty(), "a column the adapter does not project is absent");

        let table = TableModelDataSource::new(Arc::new(StaticTable));
        out.clear();
        assert!(table.fill_cell(1, 2, &mut out));
        assert_eq!(out, "1:2");
    }

    #[test]
    fn tree_adapter_projects_single_column_window() {
        let source = TreeModelDataSource::new(Arc::new(StaticTree));

        assert_eq!(source.row_count(), 2);
        assert_eq!(source.column_count(), 1);
        assert_eq!(source.data(0, 0), Some("root".to_string()));

        let window = source.fetch_window(0, 2, 0, 1);
        assert_eq!(window.len(), 2);
        assert_eq!(window[1][0], Some("root/child".to_string()));
    }

    #[test]
    fn table_adapter_fetches_bounded_window() {
        let source = TableModelDataSource::new(Arc::new(StaticTable));

        assert_eq!(source.row_count(), 2);
        assert_eq!(source.column_count(), 3);
        assert_eq!(source.data(1, 2), Some("1:2".to_string()));

        let window = source.fetch_window(0, 2, 1, 5);
        assert_eq!(window.len(), 2);
        assert_eq!(window[0].len(), 2);
        assert_eq!(window[0][0], Some("0:1".to_string()));
        assert_eq!(window[1][1], Some("1:2".to_string()));

        let empty = source.fetch_window(10, 2, 0, 1);
        assert!(empty.is_empty());
    }

    #[test]
    fn default_revision_is_zero() {
        struct NoRevision;
        impl IncrementalTableDataSource for NoRevision {
            fn row_count(&self) -> usize {
                5
            }
            fn column_count(&self) -> usize {
                3
            }
            fn data(&self, row: usize, column: usize) -> Option<String> {
                Some(format!("{}:{}", row, column))
            }
        }

        let source = NoRevision;
        assert_eq!(source.revision(), 0);
        assert!(source.data_changed_signal().is_none());
    }

    #[test]
    fn data_changed_signal_default_is_none() {
        struct NoSignalSource;
        impl IncrementalTableDataSource for NoSignalSource {
            fn row_count(&self) -> usize {
                3
            }
            fn column_count(&self) -> usize {
                1
            }
            fn data(&self, row: usize, _: usize) -> Option<String> {
                Some(format!("row-{}", row))
            }
        }

        let source = NoSignalSource;
        assert!(source.data_changed_signal().is_none());
    }

    #[test]
    fn fetch_window_bounds_min_window() {
        struct SmallSource;
        impl IncrementalTableDataSource for SmallSource {
            fn row_count(&self) -> usize {
                1
            }
            fn column_count(&self) -> usize {
                1
            }
            fn data(&self, row: usize, column: usize) -> Option<String> {
                Some(format!("{}:{}", row, column))
            }
        }

        let source = SmallSource;

        // Full window within bounds
        let window = source.fetch_window(0, 1, 0, 1);
        assert_eq!(window.len(), 1);
        assert_eq!(window[0][0], Some("0:0".to_string()));

        // Window exceeding bounds should clamp
        let window2 = source.fetch_window(0, 10, 0, 10);
        assert_eq!(window2.len(), 1);
        assert_eq!(window2[0].len(), 1);
    }

    #[test]
    fn fetch_window_column_offset_window() {
        struct WideSource;
        impl IncrementalTableDataSource for WideSource {
            fn row_count(&self) -> usize {
                2
            }
            fn column_count(&self) -> usize {
                5
            }
            fn data(&self, row: usize, column: usize) -> Option<String> {
                Some(format!("{}:{}", row, column))
            }
        }

        let source = WideSource;
        let window = source.fetch_window(0, 2, 2, 2);
        assert_eq!(window.len(), 2);
        assert_eq!(window[0].len(), 2);
        assert_eq!(window[0][0], Some("0:2".to_string()));
        assert_eq!(window[1][1], Some("1:3".to_string()));

        // Column start out of bounds
        let empty = source.fetch_window(0, 2, 10, 2);
        assert!(empty.is_empty());
    }

    #[test]
    fn list_model_adapter_model_ref() {
        use crate::widget::view_widgets::list_view::ListModel;

        struct MyList;
        impl ListModel for MyList {
            fn row_count(&self) -> usize {
                2
            }
            fn data(&self, row: usize) -> Option<String> {
                Some(format!("item-{}", row))
            }
        }

        let inner = Arc::new(MyList);
        let adapter = ListModelDataSource::new(inner.clone());
        let recovered = adapter.model_ref();
        assert_eq!(recovered.row_count(), 2);
        assert_eq!(recovered.data(0), Some("item-0".to_string()));
    }

    #[test]
    fn table_model_adapter_model_ref() {
        use crate::widget::view_widgets::table_widget::TableModel;

        struct MyTable;
        impl TableModel for MyTable {
            fn row_count(&self) -> usize {
                3
            }
            fn column_count(&self) -> usize {
                2
            }
            fn data(&self, row: usize, column: usize) -> Option<String> {
                Some(format!("{}-{}", row, column))
            }
        }

        let inner = Arc::new(MyTable);
        let adapter = TableModelDataSource::new(inner.clone());
        let recovered = adapter.model_ref();
        assert_eq!(recovered.row_count(), 3);
        assert_eq!(recovered.data(1, 1), Some("1-1".to_string()));
    }

    #[test]
    fn tree_model_adapter_model_ref() {
        use crate::widget::view_widgets::tree_view::TreeModel;

        struct MyTree;
        impl TreeModel for MyTree {
            fn node_count(&self) -> usize {
                2
            }
            fn node_path(&self, index: usize) -> Option<String> {
                Some(format!("node-{}", index))
            }
        }

        let inner = Arc::new(MyTree);
        let adapter = TreeModelDataSource::new(inner.clone());
        let recovered = adapter.model_ref();
        assert_eq!(recovered.node_count(), 2);
        assert_eq!(recovered.node_path(1), Some("node-1".to_string()));
    }

    /// Role-addressed reads, index addressing, flags and writes are part of the protocol,
    /// and a flat read-only source keeps every default.
    #[test]
    fn roles_and_model_indices_are_addressable() {
        use super::{CellFlags, DataRole, ModelIndex};

        struct Editable {
            cells: Vec<Vec<String>>,
        }
        impl IncrementalTableDataSource for Editable {
            fn row_count(&self) -> usize {
                self.cells.len()
            }
            fn column_count(&self) -> usize {
                self.cells.first().map(Vec::len).unwrap_or(0)
            }
            fn data(&self, row: usize, column: usize) -> Option<String> {
                self.cells.get(row).and_then(|line| line.get(column)).cloned()
            }
            fn flags(&self, _row: usize, _column: usize) -> CellFlags {
                CellFlags::EDITABLE
            }
            fn set_data(&mut self, row: usize, column: usize, value: &str) -> bool {
                match self.cells.get_mut(row).and_then(|line| line.get_mut(column)) {
                    Some(cell) => {
                        *cell = value.to_string();
                        true
                    }
                    None => false,
                }
            }
        }

        let mut source = Editable { cells: vec![vec!["a".into(), "b".into()]] };
        assert_eq!(source.data_at(ModelIndex::root(0, 1), DataRole::Display), Some("b".into()));
        assert_eq!(source.data_at(ModelIndex::child(0, 0, 0), DataRole::Display), None);
        assert_eq!(source.data_role(0, 0, DataRole::AccessibleText), None);
        assert!(source.flags(0, 0).editable);
        assert!(source.set_data(0, 0, "changed"));
        assert_eq!(source.data(0, 0), Some("changed".into()));
        assert!(!source.set_data(9, 9, "nope"));
        assert!(source.parent_index(0).is_none());
        assert!(source.mime_data(0, 0).is_none());
        assert!(source.cell_changed_signal().is_none());
        assert!(source.cell_changed_detail_signal().is_none());
        assert!(!source.can_fetch_more());
        assert!(!source.can_sort());
        assert!(!source.insert_row(0));
        assert!(!source.remove_row(0));
        assert!(!source.move_row(0, 1));
    }

    /// A deep index names its full ancestor chain, not just the immediate parent.
    #[test]
    fn a_model_index_carries_the_whole_ancestor_path() {
        use super::ModelIndex;

        let deep = ModelIndex::nested(&[9, 4, 2, 7], 3);
        assert_eq!(deep.path(), vec![9, 4, 2, 7]);
        assert_eq!(deep.parent, Some(2));
        assert_eq!(deep.row, 7);
        assert_eq!(deep.column, 3);
        assert_eq!(deep.depth(), 3);
        assert!(!deep.is_root());

        // Ancestors live in the index itself, so a grandparent is addressable without asking
        // the model to walk `parent_index`.
        assert_eq!(deep.ancestry[0], Some(9));
        assert_eq!(deep.ancestry[1], Some(4));
        assert_eq!(deep.ancestry[2], None);

        let root = ModelIndex::root(5, 1);
        assert_eq!(root.path(), vec![5]);
        assert!(root.is_root());
        assert_eq!(root.depth(), 0);

        // A path longer than the inline cap keeps the nearest ancestors.
        let long: Vec<usize> = (0..20).collect();
        let capped = ModelIndex::nested(&long, 0);
        assert_eq!(capped.row, 19);
        assert_eq!(capped.parent, Some(18));
        assert_eq!(capped.path().last().copied(), Some(19));
        assert_eq!(capped.depth(), super::MODEL_INDEX_MAX_DEPTH + 1);
    }

    /// A minimal flat source that opts into the source-wide fetch flag.
    struct FlatSource {
        can_fetch: bool,
    }

    impl IncrementalTableDataSource for FlatSource {
        fn row_count(&self) -> usize {
            1
        }

        fn column_count(&self) -> usize {
            1
        }

        fn data(&self, row: usize, column: usize) -> Option<String> {
            (row == 0 && column == 0).then(|| "cell".to_string())
        }

        fn can_fetch_more(&self) -> bool {
            self.can_fetch
        }
    }

    #[test]
    fn path_fetch_defaults_follow_the_source_wide_flag() {
        let finite = FlatSource { can_fetch: false };
        assert!(!finite.can_fetch_more_path(&[0]));

        let mut streaming = FlatSource { can_fetch: true };
        assert!(streaming.can_fetch_more_path(&[0]));
        // The default is a documented no-op, not a fabricated success: a source that does not
        // override it accepts the call and reports completion through its own change signal.
        streaming.fetch_more(&[0]);
        assert_eq!(streaming.row_count(), 1);
    }
}
