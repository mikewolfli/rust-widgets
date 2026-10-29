// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Column layout: widths, prefix sums, hidden/frozen columns, spans, alignment and filters.

use super::*;

/// Prefix sums of column widths, rebuilt only when a width input changes.
///
/// `column_prefix_width` used to sum `0..upto` on every call, and `cell_rect` called it twice
/// per painted cell — an interactive-width grid therefore cost O(rows x columns^2) per frame.
/// The table is O(columns) to build and O(1) to read.
#[derive(Clone, Debug)]
pub(crate) struct ColumnPrefixCache {
    pub(crate) revision: u64,
    pub(crate) column_count: usize,
    cells_width: u32,
    pub(crate) mode: ColumnWidthMode,
    pub(crate) default_width: u32,
    pub(crate) prefix: Vec<i32>,
}

/// Elided header labels, keyed by `(column, painted width)`.
#[derive(Clone, Debug, Default)]
pub(crate) struct HeaderTextCache {
    pub(crate) labels_revision: u64,
    pub(crate) generation: u64,
    pub(crate) column_count: usize,
    pub(crate) entries: std::collections::HashMap<(usize, u32), String>,
}

/// The row count above which a span is left out of the per-row index (WGT-G3).
///
/// `CellSpan::row_span` is caller-supplied and `covers` accepts any `usize`; registering a span
/// that tall would cost more than the scan it replaces. Such a span is answered by the exact
/// linear fallback instead, so the index never changes an answer.
const MAX_INDEXED_SPAN_ROWS: usize = 1024;

impl DataGrid {
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
        self.column_alignments.clear();
        for (column, spec) in self.header_columns.iter().enumerate() {
            if spec.alignment != HorizontalAlignment::Left {
                self.column_alignments.insert(column, spec.alignment);
            }
        }
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
    pub(crate) fn column_token_maps(
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
        let replaced = {
            let existing = self
                .cell_spans
                .iter_mut()
                .find(|existing| existing.row == row && existing.column == column);
            match existing {
                Some(existing) => {
                    let previous = *existing;
                    *existing = span;
                    Some(previous)
                }
                None => None,
            }
        };
        match replaced {
            Some(previous) => {
                self.unindex_span(previous);
                self.index_span(span);
            }
            None => {
                self.cell_spans.push(span);
                self.index_span(span);
            }
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
        self.span_rows.clear();
        self.base.request_redraw();
    }

    /// The span covering `(row, column)`, when one does.
    pub fn cell_span(&self, row: usize, column: usize) -> Option<CellSpan> {
        self.span_covering(row, column)
    }

    /// Registers `span` under every row it covers.
    fn index_span(&mut self, span: CellSpan) {
        if span.row_span > MAX_INDEXED_SPAN_ROWS {
            // Answered by the exact fallback in `span_covering`; indexing it would make the
            // index itself the expensive part for a caller-supplied `usize` span.
            return;
        }
        for row in span.row..span.row.saturating_add(span.row_span) {
            self.span_rows.entry(row).or_default().push(span);
        }
    }

    /// Removes `span` from every row bucket it was registered in.
    fn unindex_span(&mut self, span: CellSpan) {
        if span.row_span > MAX_INDEXED_SPAN_ROWS {
            return;
        }
        for row in span.row..span.row.saturating_add(span.row_span) {
            let mut empty = false;
            if let Some(spans) = self.span_rows.get_mut(&row) {
                spans.retain(|candidate| *candidate != span);
                empty = spans.is_empty();
            }
            if empty {
                self.span_rows.remove(&row);
            }
        }
    }

    pub(crate) fn span_covering(&self, row: usize, column: usize) -> Option<CellSpan> {
        if let Some(spans) = self.span_rows.get(&row) {
            if let Some(span) = spans.iter().copied().find(|span| span.covers(row, column)) {
                return Some(span);
            }
        }
        // Spans too tall to index are still answered exactly, just by scanning.
        self.cell_spans
            .iter()
            .copied()
            .find(|span| span.row_span > MAX_INDEXED_SPAN_ROWS && span.covers(row, column))
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
    pub(crate) fn header_strip_height(&self) -> u32 {
        GRID_HEADER_HEIGHT.saturating_mul(1 + self.extra_header_rows.len() as u32)
    }

    // ── Column alignment (V-001) ──

    /// The alignment `column` paints its values with.
    ///
    /// A column with no explicit alignment answers `Left`, the historical behaviour, so setting
    /// alignment on one numeric column leaves the rest unchanged.
    pub fn column_alignment(&self, column: usize) -> HorizontalAlignment {
        self.column_alignments.get(&column).copied().unwrap_or(HorizontalAlignment::Left)
    }

    /// Sets `column`'s cell alignment, returning whether the column exists.
    ///
    /// This is the per-column counterpart of the header spec's
    /// [`ColumnSpec::alignment`]: the grid paints it and the header only carries it.
    pub fn set_column_alignment(&mut self, column: usize, alignment: HorizontalAlignment) -> bool {
        if column >= self.column_count() {
            return false;
        }
        if alignment == HorizontalAlignment::Left {
            self.column_alignments.remove(&column);
        } else {
            self.column_alignments.insert(column, alignment);
        }
        self.base.request_redraw();
        true
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

    /// The width a column falls back to under the current mode.
    ///
    /// Per-column overrides live in [`Self::column_width_at`]; this is the nominal width used for
    /// capacity estimates and for columns that have no override.
    pub(crate) fn effective_column_width(&self) -> u32 {
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
    pub(crate) fn store_column_width(&mut self, column: usize, width: u32) -> bool {
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
    pub(crate) fn resize_handle_at(&self, point: Point) -> Option<usize> {
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
    pub(crate) fn column_view_x(&self, column: usize) -> i32 {
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
    pub(crate) fn column_prefix_width(&self, upto: usize) -> i32 {
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
    pub(crate) fn column_at_dx(&self, dx: i32) -> Option<usize> {
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
}
