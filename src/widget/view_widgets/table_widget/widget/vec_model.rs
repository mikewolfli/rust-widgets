// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The table's in-memory model, its source window and its small row-indexed helpers.
//!
//! Kept out of `table_widget.rs` because that file is already the largest view module: the
//! production part has to stay under the structure limit, and these pieces are a coherent seam —
//! a plain `Vec` model, the bounded window a data source is read through, and the map rotation a
//! column-structure edit needs. A child module (rather than a sibling) so the
//! `CollectionState` impl can reach `TableWidget`'s own placeholder field.

use super::*;

/// One bounded window of cell text fetched from an [`IncrementalTableDataSource`] (RW-7).
///
/// The key covers every input the window's contents depend on: the source's own revision, the
/// control's change generation, the origin and the size. A paint whose key matches reuses the
/// window instead of asking the source again, which is what keeps a 60 Hz repaint of a static
/// table from allocating a visible window per frame.
pub(super) struct TableWindow {
    pub(super) key: u64,
    pub(super) row_start: usize,
    pub(super) column_start: usize,
    pub(super) cells: Vec<Vec<Option<String>>>,
}

impl TableWindow {
    pub(super) fn cell(&self, row: usize, column: usize) -> Option<&str> {
        let row = row.checked_sub(self.row_start)?;
        let column = column.checked_sub(self.column_start)?;
        self.cells.get(row)?.get(column)?.as_deref()
    }
}

/// Shifts every key `>= at` in a column-indexed map one column right.
pub(super) fn shift_column_map<V>(map: &mut HashMap<usize, V>, at: usize) {
    if map.is_empty() {
        return;
    }
    let mut shifted = HashMap::with_capacity(map.len());
    for (key, value) in map.drain() {
        shifted.insert(if key >= at { key + 1 } else { key }, value);
    }
    *map = shifted;
}

/// Shifts every key `> at` in a column-indexed map one column left and drops `at` itself.
pub(super) fn unshift_column_map<V>(map: &mut HashMap<usize, V>, at: usize) {
    if map.is_empty() {
        return;
    }
    let mut shifted = HashMap::with_capacity(map.len());
    for (key, value) in map.drain() {
        if key == at {
            continue;
        }
        shifted.insert(if key > at { key - 1 } else { key }, value);
    }
    *map = shifted;
}

impl CollectionState for TableWidget {
    fn collection_placeholder(&self) -> &CollectionPlaceholder {
        &self.placeholder
    }

    fn collection_placeholder_mut(&mut self) -> &mut CollectionPlaceholder {
        &mut self.placeholder
    }
}

impl TableWidget {
    /// Writes a cell's text into `out`, returning whether it exists.
    ///
    /// The zero-allocation read path every internal consumer uses: it reads the bound source or
    /// model directly and never materialises the window, so a host query is cheap even while a
    /// cached window exists.
    pub(super) fn fill_cell(&self, row: usize, column: usize, out: &mut String) -> bool {
        match &self.data_source {
            Some(source) => source.fill_cell(row, column, out),
            None => match &self.model {
                Some(model) => model.fill_data(row, column, out),
                None => false,
            },
        }
    }

    /// The source's or model's name for `column`.
    pub(super) fn source_column_name(&self, column: usize) -> Option<String> {
        match &self.data_source {
            Some(source) => source.column_name(column),
            None => {
                self.model.as_ref().and_then(|model| model.column_name(column)).map(str::to_string)
            }
        }
    }

    /// The title painted above `column`.
    ///
    /// Precedence is the explicit column structure's title (when non-empty), then a host header
    /// label, then the model/source's own name. The order matters: `set_column_title` must be
    /// visible, while a table that never named a column keeps its historical blank or model name.
    pub(super) fn column_title(&self, column: usize) -> Option<String> {
        if let Some(title) = self
            .columns
            .get(column)
            .map(|spec| spec.title.as_str())
            .filter(|title| !title.is_empty())
        {
            return Some(title.to_string());
        }
        if let Some(label) = self.header_labels.get(column).filter(|label| !label.is_empty()) {
            return Some(label.clone());
        }
        self.source_column_name(column)
    }

    /// How many rows the band can show from `scroll_row`, walking the same heights the paint does.
    ///
    /// Used to size the source window (RW-7). The walk is bounded by the band's height and stops
    /// at the first row that does not fit, exactly where the paint loop stops — a multiplication
    /// by the *default* height would under-fetch as soon as one row was overridden.
    pub(super) fn visible_row_span(&self, band: Rect) -> usize {
        let mut painted = 0u32;
        let mut index = self.scroll_row;
        while index < self.row_count() {
            let height = self.resolved_row_height(index);
            if painted.saturating_add(height) > band.height {
                break;
            }
            painted = painted.saturating_add(height);
            index += 1;
        }
        index.saturating_sub(self.scroll_row).max(1)
    }

    /// How many columns the content box can show from `scroll_column`.
    ///
    /// The same composition the paint loop walks, so the fetched window always contains every
    /// column the loop can reach.
    pub(super) fn visible_column_span(&self, content_width: u32) -> usize {
        let count = self.column_count();
        let mut left = self.horizontal_offset();
        let mut visible = 0usize;
        for column in self.scroll_column..count {
            if left >= content_width {
                break;
            }
            visible += 1;
            left = left.saturating_add(self.resolved_column_width(column, content_width));
        }
        visible.max(1)
    }
}

/// A public in-memory table model (RW-8).
///
/// # Why this is public
///
/// The table had `VecListModel` and `VecTreeModel` siblings but only a test-private vector model,
/// so a host with an in-memory set of rows either implemented [`TableModel`] itself or rebuilt a
/// whole model on every edit. This model owns plain `String` cells, reports a fine-grained
/// `(row, column)` signal per cell edit and a coarse structural signal per row edit, and can be
/// handed straight to
/// [`TableModelDataSource`](crate::widget::view_widgets::data_source::TableModelDataSource) or to
/// [`TableWidget::set_model`].
///
/// The model revises its row and column vectors **in place**: an edit touches one cell and fires
/// one signal, which is what lets a bound view repaint the row that changed instead of the whole
/// control.
pub struct VecTableModel {
    columns: Vec<String>,
    rows: Vec<Vec<String>>,
    data_changed: GenericSignal,
    cell_changed: Signal1<(usize, usize)>,
    rows_changed: GenericSignal,
}

impl VecTableModel {
    /// Creates a model with column titles and no rows.
    pub fn new(columns: Vec<String>) -> Self {
        Self {
            columns,
            rows: Vec::new(),
            data_changed: GenericSignal::new(),
            cell_changed: Signal1::new(),
            rows_changed: GenericSignal::new(),
        }
    }

    /// Creates a model with column titles and initial rows.
    pub fn with_rows(columns: Vec<String>, rows: Vec<Vec<String>>) -> Self {
        let mut model = Self::new(columns);
        model.rows = rows;
        model
    }

    /// The column titles, in column order.
    pub fn column_titles(&self) -> &[String] {
        &self.columns
    }

    /// Replaces the column titles, returning whether the column count changed.
    ///
    /// Rows are not resized: a row shorter than the new column list simply has no value in the
    /// new columns, which is the honest state of a caller that has not supplied one yet.
    pub fn set_columns(&mut self, columns: Vec<String>) -> bool {
        let changed = columns.len() != self.columns.len();
        self.columns = columns;
        self.rows_changed.emit();
        self.data_changed.emit();
        changed
    }

    /// The cell text at `row`/`column`, when it exists.
    pub fn cell(&self, row: usize, column: usize) -> Option<&str> {
        self.rows.get(row)?.get(column).map(String::as_str)
    }

    /// The whole row at `row`, when it exists.
    pub fn row(&self, row: usize) -> Option<&[String]> {
        self.rows.get(row).map(Vec::as_slice)
    }

    /// Writes one cell, returning whether the address exists.
    ///
    /// The row is grown to the column count when the address is a known column but the row is
    /// short, so `set_cell` can fill a ragged row without the caller pre-sizing it.
    pub fn set_cell(&mut self, row: usize, column: usize, value: impl Into<String>) -> bool {
        if row >= self.rows.len() || column >= self.column_count() {
            return false;
        }
        let line = &mut self.rows[row];
        if line.len() <= column {
            line.resize(column + 1, String::new());
        }
        let value = value.into();
        if line[column] == value {
            return true;
        }
        line[column] = value;
        self.cell_changed.emit((row, column));
        self.data_changed.emit();
        true
    }

    /// Inserts a row at `at` (clamped), returning the index it landed at.
    pub fn insert_row(&mut self, at: usize, values: Vec<String>) -> usize {
        let at = at.min(self.rows.len());
        let width = self.column_count().max(values.len());
        let mut line = values;
        line.resize(width, String::new());
        self.rows.insert(at, line);
        self.rows_changed.emit();
        self.data_changed.emit();
        at
    }

    /// Appends a row, returning its index.
    pub fn push_row(&mut self, values: Vec<String>) -> usize {
        let at = self.rows.len();
        self.insert_row(at, values)
    }

    /// Removes the row at `at`, returning it when the index existed.
    pub fn remove_row(&mut self, at: usize) -> Option<Vec<String>> {
        if at >= self.rows.len() {
            return None;
        }
        let removed = self.rows.remove(at);
        self.rows_changed.emit();
        self.data_changed.emit();
        Some(removed)
    }

    /// Removes every row, keeping the column titles.
    pub fn clear_rows(&mut self) {
        if self.rows.is_empty() {
            return;
        }
        self.rows.clear();
        self.rows_changed.emit();
        self.data_changed.emit();
    }

    /// The signal emitted with `(row, column)` when one cell changes.
    pub fn cell_changed_signal(&self) -> &Signal1<(usize, usize)> {
        &self.cell_changed
    }

    /// The signal emitted when rows or columns are added, removed or replaced.
    pub fn rows_changed_signal(&self) -> &GenericSignal {
        &self.rows_changed
    }
}

impl TableModel for VecTableModel {
    fn row_count(&self) -> usize {
        self.rows.len()
    }

    fn column_count(&self) -> usize {
        let widest = self.rows.iter().map(Vec::len).max().unwrap_or(0);
        self.columns.len().max(widest)
    }

    fn data(&self, row: usize, column: usize) -> Option<String> {
        self.cell(row, column).map(str::to_string)
    }

    fn column_name(&self, column: usize) -> Option<&str> {
        self.columns.get(column).map(String::as_str)
    }

    fn fill_data(&self, row: usize, column: usize, out: &mut String) -> bool {
        match self.cell(row, column) {
            Some(value) => {
                out.push_str(value);
                true
            }
            None => false,
        }
    }

    fn data_changed_signal(&self) -> Option<&GenericSignal> {
        Some(&self.data_changed)
    }
}
