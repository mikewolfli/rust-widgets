// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Table widget.
use crate::core::Color;
use crate::core::HorizontalAlignment;
use crate::core::Rect;
use crate::event::WheelScroll;
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::widget::capability::access::selection_mode_to_str;
use crate::widget::capability::coercion::{expect_bool, expect_selection_mode, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{effective_font, ControlMetrics};
use crate::widget::view_widgets::collection_state::{
    paint_collection_placeholder, CollectionPlaceholder, CollectionState,
};
use crate::widget::view_widgets::data_grid::{
    ColumnChange, ColumnChangeKind, ColumnDataType, ColumnSpec,
};
use crate::widget::view_widgets::data_source::IncrementalTableDataSource;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

mod events;
mod properties;
mod render;
#[cfg(test)]
mod tests;

/// The margin a table leaves between its own frame and its rows: 2 px on every edge.
///
/// Named once so the header row and the first content row are both measured from the same
/// inset. The rows used to start at the control's literal `rect.y`, so the first row's glyph
/// box sat flush on the frame's own stroke with no header row above it.
const TABLE_INSET: u32 = 2;

/// The height of the table's header row: 20, the same row the model's data rows use.
///
/// A table has a header whether or not the model supplies column titles, so the first data
/// row is always one header below the frame rather than pinned to the top edge.
const HEADER_ROW_HEIGHT: u32 = 20;

/// The height of a content row: 20, the same as the header's.
///
/// Extracted because it was a `let row_h = 20;` in **two** places — the paint loop and the press
/// arm — and they did not even measure from the same origin. [`TableWidget::row_rect`] is now the
/// one derivation, which is what makes the row a click selects the row a hover highlights.
const TABLE_ROW_HEIGHT: u32 = 20;

/// Table model abstraction for table-like views.
pub trait TableModel: Send + Sync {
    /// Number of rows exposed by model.
    fn row_count(&self) -> usize;
    /// Number of columns exposed by model.
    fn column_count(&self) -> usize;
    /// Data for row and column index, if present.
    fn data(&self, row: usize, column: usize) -> Option<String>;
    /// The heading a column is painted with, if the model names its columns.
    ///
    /// The table reserved a header band but had no way to ask for a title, so the band was blank
    /// for every model. Returning `None` keeps a model that does not name its columns unchanged.
    fn column_name(&self, column: usize) -> Option<&str> {
        let _ = column;
        None
    }
    /// The declared data type of `column`, when the model knows one (RW-6).
    ///
    /// A model that stores typed cells can describe them here, and a [`TableWidget`] bound to it
    /// consults this when its own column structure does not declare a type. The default answers
    /// `None`, so an untyped model keeps working unchanged.
    fn column_type(&self, column: usize) -> Option<&ColumnDataType> {
        let _ = column;
        None
    }
    /// Writes the cell at `row`/`column` into `out`, returning whether it exists.
    ///
    /// # Why this exists
    ///
    /// [`Self::data`] hands out an owned `String`, so a paint loop over a window allocates one
    /// string per cell per frame. A consumer that only reads the cell can reuse a single buffer
    /// through this method. The default delegates to `data`, so existing implementors compile and
    /// behave unchanged (principle #21); a model that stores its cells overrides it and allocates
    /// nothing.
    fn fill_data(&self, row: usize, column: usize, out: &mut String) -> bool {
        match self.data(row, column) {
            Some(value) => {
                out.push_str(&value);
                true
            }
            None => false,
        }
    }
    /// Optional signal emitted when model data projection changes.
    fn data_changed_signal(&self) -> Option<&GenericSignal> {
        None
    }
}
/// Item delegate for custom display/editing.
pub trait ItemDelegate: Send + Sync {
    /// Creates editor for given cell.
    fn create_editor(
        &self,
        parent: &mut BaseWidget,
        row: usize,
        column: usize,
    ) -> Option<Box<dyn Widget>>;
    /// Sets editor data.
    fn set_editor_data(&self, editor: &mut dyn Widget, row: usize, column: usize);
    /// Gets editor data.
    fn get_editor_data(&self, editor: &dyn Widget, row: usize, column: usize) -> Option<String>;

    /// Paints a cell, returning whether the delegate took over the cell.
    ///
    /// The three-method trait could only supply an editor, so a checkbox column, a
    /// progress-bar cell or rich text had no place to draw. A delegate now owns the cell's
    /// ink when it answers `true`; `false` keeps the table's own text rendering.
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

    /// The size a cell would like to be, if the delegate has an opinion.
    fn size_hint(&self, row: usize, column: usize) -> Option<crate::core::Size> {
        let _ = (row, column);
        None
    }

    /// Places `editor` over `cell`.
    fn update_editor_geometry(&self, editor: &mut dyn Widget, cell: Rect) {
        editor.set_geometry(cell);
    }

    /// Called when an edit ends, so a delegate can commit or discard its own state.
    fn close_editor(&self, editor: &dyn Widget) {
        let _ = editor;
    }
}
/// Table widget with model/view helpers and selection state.
pub struct TableWidget {
    base: BaseWidget,
    /// Optional bound data model.
    model: Option<Arc<dyn TableModel>>,
    /// Scoped model-to-view signal subscriptions.
    model_connection_scope: ConnectionScope,
    /// View-side selection state.
    selection: crate::widget::view_widgets::list_view::SelectionModel,
    /// View-side focused row.
    focused_row: Option<usize>,
    /// The content row under the pointer, or `None` when the pointer is elsewhere.
    ///
    /// The same reasoning as `ListView::hovered_row`: a table is a stack of independent rows, so
    /// the hover has to name a row rather than the control, and the row it names is the one a click
    /// would affect. Held here rather than derived from `focused_row` because a focus row is
    /// persistent while this is transient pointer position.
    hovered_row: Option<usize>,
    /// Explicit column width overrides in logical pixels.
    column_widths: HashMap<usize, u32>,
    /// Explicit row height overrides in logical pixels.
    row_heights: HashMap<usize, u32>,
    /// Optional display/editor delegate.
    ///
    /// Driven by [`TableWidget::begin_edit`] / [`TableWidget::commit_edit`]; see those for why the
    /// three-method trait is an operation rather than a rendering hook.
    delegate: Option<Arc<dyn ItemDelegate>>,
    /// The cell an in-place edit is currently open on, if any.
    editing: Option<(usize, usize)>,
    /// Reused buffer for the cell text being painted.
    ///
    /// The paint path used to declare this inside `draw`, so a model that lends its cells still
    /// grew a fresh `String` from zero every frame (measured: two allocations per frame on a
    /// 240x120 sample table). Kept here, the capacity survives across frames.
    cell_text: String,
    /// Emitted when selected row changes.
    pub selection_changed: Signal1<usize>,
    /// Emitted when focused row changes.
    pub focused_row_changed: Signal1<Option<usize>>,
    /// First content row shown; wheel, keyboard and programmatic scrolling move it.
    scroll_row: usize,
    /// First scrolling column shown (V-03).
    ///
    /// The column widths have a 40 px floor, so a multi-column table is wider than its content
    /// box by construction. Without a horizontal offset the columns past the right edge were
    /// neither painted nor hit-testable.
    scroll_column: usize,
    /// The column the pointer last pressed or double-clicked (V-04).
    ///
    /// Enter used to edit column 0 regardless of where the user was; this records the column the
    /// pointer last named so the keyboard edit lands there.
    last_pointed_column: Option<usize>,
    /// What gestures start an edit.
    edit_triggers: TableEditTriggers,
    /// Emitted when an edit is requested for `(row, column)`.
    pub edit_requested: Signal1<(usize, usize)>,
    /// Emitted when a row is double-clicked.
    pub row_activated: Signal1<usize>,
    /// Emitted with the cell a double-click landed on.
    cell_double_clicked: Signal1<(usize, usize)>,
    /// Emitted when a header section is pressed.
    section_clicked: Signal1<usize>,
    /// Emitted when the context menu is requested at a point.
    pub context_menu_requested: Signal1<crate::core::Point>,
    /// Labels supplied through [`TableWidget::set_header_labels`].
    header_labels: Vec<String>,
    /// Whether the header band is painted and reserved.
    header_visible: bool,
    /// Whether a double click may request an edit.
    editable: bool,
    /// The explicit column structure (RW-5).
    ///
    /// Empty means "the bound model or source owns the column list", which is the historical
    /// behaviour: the count and the titles come from the model. Once a host inserts, removes or
    /// renames a column the structure exists and is authoritative for the count, the titles, the
    /// widths and the declared types; rebinding a model or source re-shapes it to that count so
    /// the control cannot describe columns the data does not have.
    columns: Vec<ColumnSpec>,
    /// Emitted when a column is inserted, removed, renamed or re-typed (RW-5).
    pub column_changed: Signal1<ColumnChange>,
    /// The incremental data source (RW-7), when one is bound instead of a model.
    ///
    /// **One storage slot.** [`Self::set_model`] and [`Self::set_data_source`] are exclusive: the
    /// most recent binder wins and the other slot is cleared, so a table never has two sources of
    /// truth for its row/column count.
    data_source: Option<Arc<dyn IncrementalTableDataSource>>,
    /// Scoped source-to-view signal subscriptions.
    data_source_connection_scope: ConnectionScope,
    /// Bumped by every source change signal; keys [`Self::window_cache`].
    data_generation: Arc<AtomicU64>,
    /// Set by a source change signal, drained before a paint to repair row/column state.
    data_dirty: Arc<AtomicBool>,
    /// The window fetched through [`IncrementalTableDataSource::fetch_window`] (RW-7).
    window_cache: Option<TableWindow>,
    /// How many windows were fetched; test-visible proof that the source is read through its
    /// window API rather than cell by cell.
    window_fetches: u64,
    /// Empty/error captions painted when there are no rows (RW-17).
    placeholder: CollectionPlaceholder,
}

/// The table's in-memory model, bounded source window and column-map helpers.
mod vec_model;

pub use vec_model::VecTableModel;
use vec_model::{shift_column_map, unshift_column_map, TableWindow};

/// Gestures that start an in-place edit in a [`TableWidget`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TableEditTriggers(u8);

impl TableEditTriggers {
    /// No gesture starts an edit.
    pub const NONE: Self = Self(0);
    /// A double-click starts an edit.
    pub const DOUBLE_CLICKED: Self = Self(1 << 0);
    /// The edit key (Enter) starts an edit on the focused row.
    pub const EDIT_KEY: Self = Self(1 << 1);
    /// Any of the above.
    pub const ALL: Self = Self(0b11);

    /// Whether `other`'s flags are all present.
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// The union of two sets.
    pub fn or(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl TableWidget {
    /// Creates an empty table widget.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Table, geometry, "TableWidget"),
            model: None,
            model_connection_scope: ConnectionScope::new(),
            selection: crate::widget::view_widgets::list_view::SelectionModel::new(),
            focused_row: None,
            hovered_row: None,
            column_widths: HashMap::new(),
            row_heights: HashMap::new(),
            delegate: None,
            editing: None,
            cell_text: String::new(),
            selection_changed: Signal1::new(),
            focused_row_changed: Signal1::new(),
            scroll_row: 0,
            scroll_column: 0,
            last_pointed_column: None,
            edit_triggers: TableEditTriggers::NONE,
            edit_requested: Signal1::new(),
            row_activated: Signal1::new(),
            cell_double_clicked: Signal1::new(),
            section_clicked: Signal1::new(),
            context_menu_requested: Signal1::new(),
            header_labels: Vec::new(),
            header_visible: true,
            editable: true,
            columns: Vec::new(),
            column_changed: Signal1::new(),
            data_source: None,
            data_source_connection_scope: ConnectionScope::new(),
            data_generation: Arc::new(AtomicU64::new(0)),
            data_dirty: Arc::new(AtomicBool::new(false)),
            window_cache: None,
            window_fetches: 0,
            placeholder: CollectionPlaceholder::new(),
        }
    }
    /// Binds an external table model.
    ///
    /// **One storage slot.** Binding a model releases any [`Self::set_data_source`] binding, so the
    /// row/column count has exactly one owner. The explicit column structure, if a host created
    /// one, is re-shaped to the model's column count.
    pub fn set_model(&mut self, model: Arc<dyn TableModel>) {
        self.data_source_connection_scope = ConnectionScope::new();
        self.data_source = None;
        self.model_connection_scope = ConnectionScope::new();
        if let Some(data_changed) = model.data_changed_signal() {
            let redraw = self.base.redraw_requested_signal().clone();
            let layout = self.base.layout_requested_signal().clone();
            let dirty = Arc::clone(&self.data_dirty);
            let generation = Arc::clone(&self.data_generation);
            data_changed.connect_scoped(&self.model_connection_scope, move || {
                generation.fetch_add(1, Ordering::Relaxed);
                dirty.store(true, Ordering::Relaxed);
                redraw.emit();
                layout.emit();
            });
        }
        self.model = Some(model);
        self.reconcile_columns();
        self.invalidate_window();
        self.normalize_projection_state();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Binds an incremental data source (RW-7).
    ///
    /// The source replaces the model: [`Self::set_model`] and this method share one storage slot,
    /// and the most recent call wins. Cells are painted from one bounded
    /// [`IncrementalTableDataSource::fetch_window`] per frame (cached until the source reports a
    /// change), and the source's fine-grained change signals drive repaints and repair a selection
    /// that a row removal made stale.
    pub fn set_data_source(&mut self, data_source: Arc<dyn IncrementalTableDataSource>) {
        self.data_source_connection_scope = ConnectionScope::new();
        self.model_connection_scope = ConnectionScope::new();
        self.model = None;

        let redraw = self.base.redraw_requested_signal().clone();
        let layout = self.base.layout_requested_signal().clone();
        let generation = Arc::clone(&self.data_generation);
        let dirty = Arc::clone(&self.data_dirty);
        // Every change channel the protocol offers, so a source that reports one cell, a row run
        // or a reset all reach the same repair path. Each arm does the same three things: advance
        // the window key, mark the projection dirty and ask for a repaint.
        if let Some(changed) = data_source.data_changed_signal() {
            let (redraw, layout) = (redraw.clone(), layout.clone());
            let (generation, dirty) = (Arc::clone(&generation), Arc::clone(&dirty));
            changed.connect_scoped(&self.data_source_connection_scope, move || {
                generation.fetch_add(1, Ordering::Relaxed);
                dirty.store(true, Ordering::Relaxed);
                redraw.emit();
                layout.emit();
            });
        }
        if let Some(changed) = data_source.cell_changed_signal() {
            let (redraw, generation) = (redraw.clone(), Arc::clone(&generation));
            let dirty = Arc::clone(&dirty);
            changed.connect_scoped(&self.data_source_connection_scope, move || {
                generation.fetch_add(1, Ordering::Relaxed);
                dirty.store(true, Ordering::Relaxed);
                redraw.emit();
            });
        }
        if let Some(changed) = data_source.cell_changed_detail_signal() {
            let (redraw, generation) = (redraw.clone(), Arc::clone(&generation));
            let dirty = Arc::clone(&dirty);
            changed.connect_scoped(&self.data_source_connection_scope, move |_| {
                generation.fetch_add(1, Ordering::Relaxed);
                dirty.store(true, Ordering::Relaxed);
                redraw.emit();
            });
        }
        for detail in
            [data_source.rows_inserted_detail_signal(), data_source.rows_removed_detail_signal()]
                .into_iter()
                .flatten()
        {
            let (redraw, layout) = (redraw.clone(), layout.clone());
            let (generation, dirty) = (Arc::clone(&generation), Arc::clone(&dirty));
            detail.connect_scoped(&self.data_source_connection_scope, move |_| {
                generation.fetch_add(1, Ordering::Relaxed);
                dirty.store(true, Ordering::Relaxed);
                redraw.emit();
                layout.emit();
            });
        }
        for unit in [
            data_source.rows_inserted_signal(),
            data_source.rows_removed_signal(),
            data_source.rows_reset_signal(),
        ]
        .into_iter()
        .flatten()
        {
            let (redraw, layout) = (redraw.clone(), layout.clone());
            let (generation, dirty) = (Arc::clone(&generation), Arc::clone(&dirty));
            unit.connect_scoped(&self.data_source_connection_scope, move || {
                generation.fetch_add(1, Ordering::Relaxed);
                dirty.store(true, Ordering::Relaxed);
                redraw.emit();
                layout.emit();
            });
        }
        if let Some(reset) = data_source.rows_reset_detail_signal() {
            let (redraw, layout) = (redraw.clone(), layout.clone());
            let (generation, dirty) = (Arc::clone(&generation), Arc::clone(&dirty));
            reset.connect_scoped(&self.data_source_connection_scope, move |_| {
                generation.fetch_add(1, Ordering::Relaxed);
                dirty.store(true, Ordering::Relaxed);
                redraw.emit();
                layout.emit();
            });
        }

        self.data_source = Some(data_source);
        self.reconcile_columns();
        self.invalidate_window();
        self.scroll_row = 0;
        self.scroll_column = 0;
        self.normalize_projection_state();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Releases the bound data source (RW-7).
    ///
    /// The table keeps its column structure and its selection is repaired against the now-empty
    /// row set; a model is not restored, because a host that wants one binds it explicitly.
    pub fn clear_data_source(&mut self) {
        if self.data_source.is_none() {
            return;
        }
        self.data_source_connection_scope = ConnectionScope::new();
        self.data_source = None;
        self.data_generation.fetch_add(1, Ordering::Relaxed);
        self.invalidate_window();
        self.normalize_projection_state();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns whether a model is currently bound.
    pub fn has_model(&self) -> bool {
        self.model.is_some()
    }

    /// Returns the bound table model, if present.
    pub fn model_ref(&self) -> Option<&Arc<dyn TableModel>> {
        self.model.as_ref()
    }

    /// Returns the bound incremental data source, if present (RW-7).
    pub fn data_source_ref(&self) -> Option<&Arc<dyn IncrementalTableDataSource>> {
        self.data_source.as_ref()
    }

    /// Returns whether an incremental data source is bound.
    pub fn has_data_source(&self) -> bool {
        self.data_source.is_some()
    }

    /// Returns visible row count.
    pub fn row_count(&self) -> usize {
        match (&self.data_source, &self.model) {
            (Some(source), _) => source.row_count(),
            (None, Some(model)) => model.row_count(),
            (None, None) => 0,
        }
    }

    /// Returns visible column count.
    ///
    /// The explicit column structure wins when one exists (RW-5): a host that inserted a column
    /// is describing the list this table paints. Without one the count comes from the bound model
    /// or source, which is the historical answer.
    pub fn column_count(&self) -> usize {
        if !self.columns.is_empty() {
            return self.columns.len();
        }
        match (&self.data_source, &self.model) {
            (Some(source), _) => source.column_count(),
            (None, Some(model)) => model.column_count(),
            (None, None) => 0,
        }
    }

    /// Returns item text by row and column index.
    pub fn item(&self, row: usize, column: usize) -> Option<String> {
        match &self.data_source {
            Some(source) => source.data(row, column),
            None => self.model.as_ref().and_then(|m| m.data(row, column)),
        }
    }

    /// Brings an existing explicit column structure in line with the bound model/source.
    ///
    /// A structure the host built is kept field for field; only its length changes, because the
    /// data owns the column count and a table must not describe a column it cannot paint.
    fn reconcile_columns(&mut self) {
        if self.columns.is_empty() {
            return;
        }
        let count = match (&self.data_source, &self.model) {
            (Some(source), _) => source.column_count(),
            (None, Some(model)) => model.column_count(),
            (None, None) => self.columns.len(),
        };
        if self.columns.len() > count {
            self.columns.truncate(count);
        }
        if self.columns.len() < count {
            let content = ControlMetrics::band_inset(self.base.geometry(), TABLE_INSET);
            // The share comes from the bound model's column count, not from the structure while it
            // is being extended (see `ensure_columns`).
            let share = (content.width / count.max(1) as u32).max(40);
            while self.columns.len() < count {
                let column = self.columns.len();
                let title = self.source_column_name(column).unwrap_or_default();
                let width = self.column_widths.get(&column).copied().unwrap_or(share);
                self.columns.push(ColumnSpec::new(title, width));
            }
        }
    }

    /// Drops the cached window, so the next paint reads the source again.
    fn invalidate_window(&mut self) {
        self.window_cache = None;
    }

    /// Applies a pending source change before a paint or an input.
    ///
    /// The change signals cannot borrow the table mutably, so they only set a flag; this drains it
    /// once per frame and repairs every piece of row/column state that a structural change can
    /// invalidate. Doing it here (rather than in each accessor) keeps the repair O(selected) once
    /// instead of once per painted cell.
    fn drain_data_updates(&mut self) {
        if !self.data_dirty.swap(false, Ordering::Relaxed) {
            return;
        }
        self.invalidate_window();
        self.normalize_projection_state();
        let last_row = self.row_count().saturating_sub(1);
        let last_column = self.column_count().saturating_sub(1);
        self.scroll_row = self.scroll_row.min(last_row);
        self.scroll_column = self.scroll_column.min(last_column);
        self.last_pointed_column = self.last_pointed_column.filter(|column| *column <= last_column);
        self.editing =
            self.editing.filter(|(row, column)| *row <= last_row && *column <= last_column);
    }

    /// The content box the rows live in: the control inset, less the header band.
    ///
    /// The one derivation both the paint loop and the hit test read. They used to disagree: the loop
    /// measured from the content box while the press arm measured from `rect.y`, so a press on the
    /// first content row selected the row **above** it — the same defect `ListView::row_rect` was
    /// extracted to fix, which this control's sibling had kept.
    fn rows_band(&self) -> Rect {
        let content = ControlMetrics::band_inset(self.base.geometry(), TABLE_INSET);
        if !self.header_visible {
            return content;
        }
        let header = ControlMetrics::top_band(content, HEADER_ROW_HEIGHT);
        Rect::new(
            content.x,
            content.y + header.height as i32,
            content.width,
            content.height.saturating_sub(header.height),
        )
    }

    /// Whether the header band is painted and reserved.
    pub fn header_visible(&self) -> bool {
        self.header_visible
    }

    /// Shows or hides the header band.
    ///
    /// Hiding it gives the rows the header's height back, rather than leaving an empty strip; the
    /// paint loop, the hit test and an editor's geometry all read `Self::rows_band`, so they move
    /// together.
    pub fn set_header_visible(&mut self, visible: bool) {
        if self.header_visible == visible {
            return;
        }
        self.header_visible = visible;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The header labels supplied by the host.
    pub fn header_labels(&self) -> &[String] {
        &self.header_labels
    }

    /// Replaces the header labels.
    ///
    /// A label here takes precedence over [`TableModel::column_name`], which lets a host relabel a
    /// model's columns without owning the model. A missing index falls back to the model's name;
    /// a model that names nothing keeps the strip's historical blank section.
    pub fn set_header_labels(&mut self, labels: &[&str]) {
        self.header_labels = labels.iter().map(|label| (*label).to_string()).collect();
        self.base.request_redraw();
    }

    // ── Column structure (RW-5, RW-6) ──

    /// The explicit column structure, empty when the bound model owns the column list.
    pub fn columns(&self) -> &[ColumnSpec] {
        &self.columns
    }

    /// The structural column-change signal (RW-5).
    pub fn column_changed_signal(&self) -> &Signal1<ColumnChange> {
        &self.column_changed
    }

    /// Materialises the column structure from the bound model, when there is none.
    ///
    /// Called before the first structural edit so `insert_column(3, …)` on a three-column model
    /// addresses the columns that model actually has instead of an empty list.
    fn ensure_columns(&mut self) {
        if !self.columns.is_empty() {
            return;
        }
        let count = match (&self.data_source, &self.model) {
            (Some(source), _) => source.column_count(),
            (None, Some(model)) => model.column_count(),
            (None, None) => 0,
        };
        let content = ControlMetrics::band_inset(self.base.geometry(), TABLE_INSET);
        // The share is computed from the *model's* column count, not from the list being built:
        // reading `column_count()` inside the loop would divide by the structure's growing length
        // and give later columns a wider share than earlier ones.
        let share = (content.width / count.max(1) as u32).max(40);
        for column in 0..count {
            let title = self
                .header_labels
                .get(column)
                .filter(|label| !label.is_empty())
                .cloned()
                .or_else(|| self.source_column_name(column))
                .unwrap_or_default();
            let width = self.column_widths.get(&column).copied().unwrap_or(share);
            self.columns.push(ColumnSpec::new(title, width));
        }
    }

    /// Inserts a column description at `at`, returning the index it landed at.
    ///
    /// `at` is clamped to the structure's length, the per-column width and height overrides are
    /// shifted with it, and the focused/scroll column is clamped afterwards — a structural change
    /// can shrink the column range under a selection that no longer exists.
    ///
    /// The structure becomes authoritative for the painted column count ([`Self::column_count`]),
    /// so a host that inserts into a table bound to a read-only model adds a column that paints
    /// empty cells until the model grows a matching column. `VecTableModel` (RW-8) supports that
    /// growth through [`VecTableModel::set_columns`].
    pub fn insert_column(&mut self, at: usize, spec: ColumnSpec) -> usize {
        self.ensure_columns();
        let at = at.min(self.columns.len());
        let width = spec.width.max(1);
        let title = spec.title.clone();
        self.columns.insert(at, spec);
        shift_column_map(&mut self.column_widths, at);
        self.column_widths.insert(at, width);
        if at <= self.header_labels.len() {
            self.header_labels.insert(at, title);
        }
        let count = self.columns.len();
        self.scroll_column = self.scroll_column.min(count.saturating_sub(1));
        self.last_pointed_column =
            self.last_pointed_column.map(|column| if column >= at { column + 1 } else { column });
        self.editing =
            self.editing.map(|(row, column)| (row, if column >= at { column + 1 } else { column }));
        self.finish_column_change();
        self.column_changed.emit(ColumnChange::new(ColumnChangeKind::Inserted, at));
        at
    }

    /// Removes the column description at `at`, returning it when the index existed.
    ///
    /// The width overrides shift left with their columns, a focus on the removed column moves to
    /// its left neighbour (or is cleared at column 0), and the scroll offset is clamped so the
    /// view stays over a column that exists.
    pub fn remove_column(&mut self, at: usize) -> Option<ColumnSpec> {
        self.ensure_columns();
        if at >= self.columns.len() {
            return None;
        }
        let removed = self.columns.remove(at);
        unshift_column_map(&mut self.column_widths, at);
        if at < self.header_labels.len() {
            self.header_labels.remove(at);
        }
        let count = self.columns.len();
        self.scroll_column = self.scroll_column.min(count.saturating_sub(1));
        self.last_pointed_column = self.last_pointed_column.and_then(|column| {
            if column < at {
                Some(column)
            } else if column == at {
                None
            } else {
                Some(column - 1)
            }
        });
        self.editing = self
            .editing
            .filter(|(_, column)| *column != at)
            .map(|(row, column)| (row, if column > at { column - 1 } else { column }));
        self.finish_column_change();
        self.column_changed.emit(ColumnChange::new(ColumnChangeKind::Removed, at));
        Some(removed)
    }

    /// Sets the title of column `column`, returning whether it exists.
    ///
    /// The title lives in the explicit structure, which takes precedence over a host header label
    /// and over the model's own column name; an empty title falls back to those.
    pub fn set_column_title(&mut self, column: usize, title: impl Into<String>) -> bool {
        self.ensure_columns();
        let title = title.into();
        let Some(spec) = self.columns.get_mut(column) else {
            return false;
        };
        if spec.title == title {
            return true;
        }
        spec.title = title;
        self.finish_column_change();
        self.column_changed.emit(ColumnChange::new(ColumnChangeKind::TitleChanged, column));
        true
    }

    /// Declares the data type of column `column` (RW-6), returning whether it exists.
    pub fn set_column_type(&mut self, column: usize, data_type: ColumnDataType) -> bool {
        self.ensure_columns();
        let Some(spec) = self.columns.get_mut(column) else {
            return false;
        };
        if spec.data_type == data_type {
            return true;
        }
        spec.data_type = data_type;
        self.finish_column_change();
        self.column_changed.emit(ColumnChange::new(ColumnChangeKind::TypeChanged, column));
        true
    }

    /// The declared data type of column `column` (RW-6).
    ///
    /// The table's own structure wins when it declares a type; otherwise the bound model's answer
    /// is used, so a typed model does not have to be copied into a column structure first.
    pub fn column_type(&self, column: usize) -> Option<&ColumnDataType> {
        if let Some(data_type) = self
            .columns
            .get(column)
            .map(|spec| &spec.data_type)
            .filter(|data_type| **data_type != ColumnDataType::Auto)
        {
            return Some(data_type);
        }
        self.model.as_ref().and_then(|model| (**model).column_type(column))
    }

    /// The empty/error captions painted when there are no rows (RW-17).
    pub fn placeholder(&self) -> &CollectionPlaceholder {
        &self.placeholder
    }

    /// Invalidates everything a column-structure change affects.
    fn finish_column_change(&mut self) {
        self.invalidate_window();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Whether a double-click may request an edit.
    pub fn editable(&self) -> bool {
        self.editable
    }

    /// Enables or disables double-click editing.
    pub fn set_editable(&mut self, editable: bool) {
        if self.editable == editable {
            return;
        }
        self.editable = editable;
    }

    /// The column at screen x `x`, if it is inside the content box.
    pub fn column_at(&self, x: i32) -> Option<usize> {
        let content = ControlMetrics::band_inset(self.base.geometry(), TABLE_INSET);
        if x < content.x || x >= content.x + content.width as i32 {
            return None;
        }
        let mut left = content.x - self.horizontal_offset() as i32;
        for column in 0..self.column_count() {
            let width = self.resolved_column_width(column, content.width) as i32;
            if column < self.scroll_column {
                left += width;
                continue;
            }
            if x < left + width {
                return Some(column);
            }
            left += width;
        }
        None
    }

    /// The pixel offset the scrolling column band starts at, i.e. the summed widths before
    /// `scroll_column` (V-03).
    fn horizontal_offset(&self) -> u32 {
        let content = ControlMetrics::band_inset(self.base.geometry(), TABLE_INSET);
        (0..self.scroll_column.min(self.column_count()))
            .map(|column| self.resolved_column_width(column, content.width))
            .sum()
    }

    /// The first scrolling column.
    pub fn scroll_column(&self) -> usize {
        self.scroll_column
    }

    /// Sets the first scrolling column, clamped to the model (V-03).
    pub fn set_scroll_column(&mut self, column: usize) {
        let next = column.min(self.column_count().saturating_sub(1));
        if next == self.scroll_column {
            return;
        }
        self.scroll_column = next;
        self.base.request_redraw();
    }

    /// The header column at `point`, when the header is visible.
    fn header_column_at(&self, point: crate::core::Point) -> Option<usize> {
        if !self.header_visible {
            return None;
        }
        let content = ControlMetrics::band_inset(self.base.geometry(), TABLE_INSET);
        let header = ControlMetrics::top_band(content, HEADER_ROW_HEIGHT);
        if !header.contains_point(point) {
            return None;
        }
        self.column_at(point.x)
    }

    /// The height of content row `index`: its override, or the default.
    ///
    /// # Why a row's height is resolved per row rather than per table
    ///
    /// `row_heights` is an override map, so a table may have a tall header row, a set of normal ones
    /// and a tall summary row. The `TABLE_ROW_HEIGHT` constant is the *default*, and a table with no
    /// overrides must lay out exactly as it did before this existed — which is the property its
    /// snapshot and the hit-test tests pin.
    fn resolved_row_height(&self, index: usize) -> u32 {
        if let Some(height) = self.row_heights.get(&index) {
            return *height;
        }
        // A delegate that wants a taller row (a two-line cell, an editor, a thumbnail) says so
        // through its size hint. Without a delegate, and without a hint, the default stands.
        self.delegate
            .as_ref()
            .and_then(|delegate| delegate.size_hint(index, 0))
            .map(|size| size.height)
            .unwrap_or(TABLE_ROW_HEIGHT)
    }

    /// The y offset of content row `index` from the top of the rows band.
    ///
    /// # Why the rows are accumulated rather than multiplied
    ///
    /// `TABLE_ROW_HEIGHT * index` is the same thing only while every row is the default height. With
    /// per-row overrides the rows **compose**: row `n` starts where rows `0..n` end, and multiplying
    /// would overlap a tall row with its successors. The accumulation is over the overrides that are
    /// actually set, so a table with none takes the same arithmetic as before (`default * index`) and
    /// cannot drift from it.
    fn row_offset(&self, index: usize) -> u32 {
        if self.row_heights.is_empty() {
            return TABLE_ROW_HEIGHT * index as u32;
        }
        (0..index).map(|i| self.resolved_row_height(i)).sum()
    }

    /// The width of column `index`: its override, or an equal share of the content box.
    ///
    /// # Why the override replaces the share rather than trimming it
    ///
    /// A host that sets a column width is saying how wide it wants that column. Clamping the result
    /// back into the box would make `set_column_width(0, 400)` on a 240-wide table give neither 400
    /// nor a sensible share, and the table must not silently disagree with the value it stored -- the
    /// same defect `grid`'s sentinel had, in a different form.
    fn resolved_column_width(&self, index: usize, content_width: u32) -> u32 {
        if let Some(width) = self.column_widths.get(&index) {
            return *width;
        }
        // A spec's own `width` is deliberately *not* read here: it is the width recorded when the
        // structure was created, and treating it as a constraint would freeze the layout at that
        // width forever (a resize would no longer reflow columns). An explicit width — a host's
        // `insert_column` spec, a resize, or `fit_column_to_contents` — lives in `column_widths`
        // and is read above.
        if self.column_count() == 0 {
            return content_width;
        }
        (content_width / self.column_count() as u32).max(40)
    }

    /// The x offset of column `index` from the content box's left edge.
    ///
    /// # Why the columns are accumulated rather than multiplied
    ///
    /// `col_w * index` is the same thing only while every column is the same width. With overrides the
    /// columns **compose**: column `n` starts where columns `0..n` end. The multiplication was also
    /// the reason a wide column could overlap its neighbour rather than push it, which is the same
    /// defect the crate records for the footer's button row.
    ///
    /// The fast path keeps a table with no overrides on the original arithmetic, so its snapshot and
    /// every geometry assertion are unchanged.
    ///
    /// Test-only now: the paint path accumulates the same sum in one pass, and the hit test
    /// (`row_at_point`) walks its own cursor, so nothing in production re-derives a column offset
    /// from zero per cell. Kept as the reference the accumulation is asserted against.
    #[cfg(test)]
    fn column_offset(&self, index: usize, content_width: u32) -> u32 {
        if self.column_widths.is_empty() {
            let col_w = if self.column_count() > 0 {
                (content_width / self.column_count() as u32).max(40)
            } else {
                content_width
            };
            return col_w * index as u32;
        }
        (0..index).map(|c| self.resolved_column_width(c, content_width)).sum()
    }

    /// The rectangle of content row `index`, or `None` when it is not fully visible.
    fn row_rect(&self, index: usize) -> Option<Rect> {
        let band = self.rows_band();
        let height = self.resolved_row_height(index);
        let y = band.y + self.row_offset(index) as i32 - self.scroll_offset_px() as i32;
        // A row that would extend past the content box is not painted at all rather than painted
        // truncated: a half-height row reads as a rendering error. A row scrolled partly above
        // the band is hidden for the same reason.
        if y + height as i32 > band.y + band.height as i32 || y < band.y {
            return None;
        }
        Some(Rect::new(band.x, y, band.width, height))
    }

    /// The pixel offset the first visible row starts at.
    fn scroll_offset_px(&self) -> u32 {
        self.row_offset(self.scroll_row.min(self.row_count()))
    }

    /// The first visible content row.
    pub fn scroll_row(&self) -> usize {
        self.scroll_row
    }

    /// Sets the first visible content row, clamped to the model.
    pub fn set_scroll_row(&mut self, row: usize) {
        let next = row.min(self.row_count().saturating_sub(1));
        if next == self.scroll_row {
            return;
        }
        self.scroll_row = next;
        self.base.request_redraw();
    }

    /// The gestures that start an edit.
    pub fn edit_triggers(&self) -> TableEditTriggers {
        self.edit_triggers
    }

    /// Sets the gestures that start an edit.
    pub fn set_edit_triggers(&mut self, triggers: TableEditTriggers) {
        if self.edit_triggers == triggers {
            return;
        }
        self.edit_triggers = triggers;
    }

    /// The content row a point falls on, if it falls on a visible one.
    ///
    /// The public form of the hit test the press handling uses, so a host can resolve the row a
    /// context-menu request addressed (feedback C-9).
    pub fn row_index_at(&self, point: crate::core::Point) -> Option<usize> {
        self.row_at_point(point)
    }

    /// The content row a point falls on, if it falls on a visible one.
    ///
    /// The inverse of [`Self::row_rect`] and built on it, so the row a click selects and the row a
    /// hover highlights cannot disagree about where a row begins.
    fn row_at_point(&self, point: crate::core::Point) -> Option<usize> {
        let band = self.rows_band();
        if !band.contains_point(point) {
            return None;
        }
        let offset = (point.y - band.y) as u32;
        // Walk the rows rather than dividing: with per-row heights the row a y belongs to is the one
        // whose offset range contains it, and a division by the *default* height would be wrong from
        // the first overridden row down. The walk is over the rows the band can show, which is bounded
        // by the band's height, so it cannot run away on a long table.
        let mut y = 0u32;
        let mut index = 0usize;
        let target = offset + self.scroll_offset_px();
        while index < self.row_count() {
            let height = self.resolved_row_height(index);
            if target < y + height {
                break;
            }
            y += height;
            index += 1;
        }
        // The last partial row is not a target, matching `row_rect`'s refusal to paint it.
        (index < self.row_count() && self.row_rect(index).is_some()).then_some(index)
    }
    /// Select one row in the current view projection.
    ///
    /// Repaints: the selected row's highlight is painted, so without the request it
    /// would only appear once some unrelated event repainted the control.
    pub fn select_row(&mut self, row: usize) -> bool {
        if row < self.row_count() {
            self.selection.select_row(row);
            self.selection_changed.emit(row);
            self.set_focused_row(row);
            self.base.request_redraw();
            true
        } else {
            false
        }
    }
    /// Selects `row` honouring the modifier keys held during the click.
    ///
    /// In [`SelectionMode::Extended`](crate::widget::input_widgets::listbox::SelectionMode::Extended)
    /// this is what implements the documented interaction model: `Shift` selects the
    /// range from the anchor, `Ctrl`/`Primary` toggles the row, and no modifier replaces
    /// the selection. In every other mode it is [`Self::select_row`].
    pub fn select_row_with_modifiers(
        &mut self,
        row: usize,
        modifiers: crate::shortcut::Modifiers,
    ) -> bool {
        if row >= self.row_count() {
            return false;
        }
        self.selection.select_with_modifiers(row, modifiers);
        self.selection_changed.emit(row);
        self.set_focused_row(row);
        self.base.request_redraw();
        true
    }
    /// Clear current row selection.
    ///
    /// Repaints: the selection highlight is painted from the selection model.
    pub fn clear_selection(&mut self) {
        if self.selection.rows().is_empty() {
            return;
        }
        self.selection.clear();
        self.base.request_redraw();
    }
    /// Sets focused row in current projection.
    ///
    /// Repaints: the focus highlight is painted from this field.
    pub fn set_focused_row(&mut self, row: usize) -> bool {
        if row >= self.row_count() {
            return false;
        }
        if self.focused_row == Some(row) {
            return true;
        }
        self.focused_row = Some(row);
        self.focused_row_changed.emit(self.focused_row);
        self.base.request_redraw();
        true
    }
    /// Clears focused row.
    ///
    /// Repaints: the focus highlight is painted from this field.
    pub fn clear_focused_row(&mut self) {
        if self.focused_row.is_none() {
            return;
        }
        self.focused_row = None;
        self.focused_row_changed.emit(None);
        self.base.request_redraw();
    }
    /// Returns focused row when still visible in projection.
    pub fn focused_row(&self) -> Option<usize> {
        self.focused_row.filter(|row| *row < self.row_count())
    }
    /// Current selected row index.
    pub fn selected_row(&self) -> Option<usize> {
        self.selection.current_row().filter(|row| *row < self.row_count())
    }
    /// All selected rows in stable order.
    pub fn selected_rows(&self) -> Vec<usize> {
        self.selection.rows().into_iter().filter(|row| *row < self.row_count()).collect()
    }
    /// Sets row selection mode.
    pub fn set_selection_mode(
        &mut self,
        mode: crate::widget::view_widgets::list_view::SelectionMode,
    ) {
        self.selection.set_mode(mode);
    }
    /// Returns current selection mode.
    pub fn selection_mode(&self) -> crate::widget::view_widgets::list_view::SelectionMode {
        self.selection.mode()
    }
    /// Sets column width.
    pub fn set_column_width(&mut self, column: usize, width: u32) {
        self.column_widths.insert(column, width);
        self.base.request_layout();
    }
    /// Returns explicit column width override when present.
    pub fn column_width(&self, column: usize) -> Option<u32> {
        self.column_widths.get(&column).copied()
    }

    /// Sizes `column` to its widest sampled content (V-05).
    ///
    /// The model is sampled rather than fully walked: every `row_count / 128`-th row plus rows
    /// the viewport shows, capped at 256 samples. A 100k-row table therefore costs a bounded
    /// measurement pass, and a column whose content is uniform still gets the same answer as a
    /// full walk. The width is the estimate plus 16 px of padding, floored at the 40 px minimum.
    pub fn fit_column_to_contents(&mut self, column: usize) -> bool {
        if column >= self.column_count() {
            return false;
        }
        let font = crate::widget::metrics::effective_font(self.base.style());
        let rows = self.row_count();
        let step = (rows / 128).max(1);
        let mut widest = 0u32;
        let mut text = String::new();
        let mut row = 0usize;
        let mut samples = 0usize;
        while row < rows && samples < 256 {
            text.clear();
            if self.fill_cell(row, column, &mut text) {
                widest = widest.max(crate::widget::metrics::estimate_text_width(&text, font, 1.0));
            }
            row = row.saturating_add(step);
            samples += 1;
        }
        let width = widest.saturating_add(16).max(40);
        self.column_widths.insert(column, width);
        if let Some(spec) = self.columns.get_mut(column) {
            spec.width = width;
        }
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    /// Sizes every column to its content (V-05).
    pub fn fit_columns_to_contents(&mut self) {
        for column in 0..self.column_count() {
            let _ = self.fit_column_to_contents(column);
        }
    }
    /// Sets row height.
    pub fn set_row_height(&mut self, row: usize, height: u32) {
        self.row_heights.insert(row, height);
        self.base.request_layout();
    }
    /// Returns explicit row height override when present.
    pub fn row_height(&self, row: usize) -> Option<u32> {
        self.row_heights.get(&row).copied()
    }
    /// Sets item delegate.
    pub fn set_delegate(&mut self, delegate: Arc<dyn ItemDelegate>) {
        self.delegate = Some(delegate);
    }

    /// Begins an in-place edit of cell `(row, column)`, returning the editor the delegate built.
    ///
    /// # Why this is the delegate's *only* entry point
    ///
    /// `ItemDelegate` declares three methods -- `create_editor`, `set_editor_data`,
    /// `get_editor_data` -- and **none of them was ever called**. The delegate was stored, published
    /// (`has_delegate`), reachable through `delegate_ref`, covered by a test that set one and read it
    /// back, and had no effect on anything: a host that supplied a delegate to get custom editing got
    /// exactly the read-only table it would have had without one.
    ///
    /// The three methods are one operation, so they are driven from one place and in order:
    /// create, then seed from the model. `begin_edit` is that place, and it is public because the
    /// host decides *when* an edit starts (a double click, a key, a toolbar command) -- the widget has
    /// no opinion about the gesture, only about the sequence.
    ///
    /// Returns `None` when there is no delegate, no model, or the cell is out of range; those are
    /// three ways for the request to be meaningless rather than errors.
    pub fn begin_edit(&mut self, row: usize, column: usize) -> Option<Box<dyn Widget>> {
        if !self.editable {
            return None;
        }
        let delegate = self.delegate.clone()?;
        let model = self.model.clone()?;
        if row >= model.row_count() || column >= model.column_count() {
            return None;
        }
        let mut editor = delegate.create_editor(&mut self.base, row, column)?;
        // Seeded from the delegate rather than from the model: the delegate owns the mapping between
        // a cell value and an editor's own text form (a number's units, a date's spelling), and asking
        // the model for a `String` here would bypass exactly the customisation the delegate exists for.
        delegate.set_editor_data(editor.as_mut(), row, column);
        // The editor is a real widget; its geometry is the cell it edits, derived from the same
        // row/column composition the paint loop uses.
        let band = self.rows_band();
        let scroll = self.scroll_offset_px();
        let y = band.y + self.row_offset(row) as i32 - scroll as i32;
        let content = ControlMetrics::band_inset(self.base.geometry(), TABLE_INSET);
        let x: i32 = (0..column)
            .map(|index| self.resolved_column_width(index, content.width) as i32)
            .sum::<i32>()
            + content.x;
        let width = self.resolved_column_width(column, content.width);
        let height = self.resolved_row_height(row);
        delegate.update_editor_geometry(editor.as_mut(), Rect::new(x, y, width, height));
        self.editing = Some((row, column));
        self.base.request_redraw();
        Some(editor)
    }

    /// Reads the current text back out of an editor and commits it.
    ///
    /// The counterpart to [`Self::begin_edit`], and the reason `get_editor_data` exists: the editor's
    /// own representation is the delegate's business, so the value is read back through the delegate
    /// rather than by downcasting the editor to a known control. Returns the text that was committed,
    /// or `None` when there is no edit in progress or the delegate declines the cell.
    pub fn commit_edit(&mut self, editor: &dyn Widget) -> Option<String> {
        let delegate = self.delegate.clone()?;
        let (row, column) = self.editing?;
        let text = delegate.get_editor_data(editor, row, column);
        delegate.close_editor(editor);
        self.editing = None;
        self.base.request_redraw();
        text
    }

    /// The cell currently being edited, if any.
    pub fn editing_cell(&self) -> Option<(usize, usize)> {
        self.editing
    }

    /// Abandons an in-place edit without committing it.
    pub fn cancel_edit(&mut self) {
        if self.editing.take().is_some() {
            self.base.request_redraw();
        }
    }
    /// Returns whether a delegate is currently bound.
    pub fn has_delegate(&self) -> bool {
        self.delegate.is_some()
    }
    /// Returns the bound item delegate, if present.
    pub fn delegate_ref(&self) -> Option<&Arc<dyn ItemDelegate>> {
        self.delegate.as_ref()
    }

    /// The signal emitted with the cell a double-click landed on.
    ///
    /// Exposed as an accessor rather than a `pub` field so it stays a Rust-level event: the
    /// capability table owns the published event names, and a name it does not publish cannot be
    /// wired through `connect_event`.
    pub fn cell_double_clicked_signal(&self) -> &Signal1<(usize, usize)> {
        &self.cell_double_clicked
    }

    /// The signal emitted when a header section is pressed.
    pub fn section_clicked_signal(&self) -> &Signal1<usize> {
        &self.section_clicked
    }
    fn normalize_projection_state(&mut self) {
        let row_count = self.row_count();
        // Get current selection and filter out invalid rows
        let mut selected_rows = self.selection.rows();
        selected_rows.retain(|row| *row < row_count);
        // Clear and re-add valid rows
        self.selection.clear();
        for row in selected_rows {
            self.selection.select_row(row);
        }
        // Update current row if invalid
        if let Some(current_row) = self.selection.current_row() {
            if current_row >= row_count {
                self.selection.clear();
            }
        }
        self.focused_row = self.focused_row.filter(|row| *row < row_count);
    }
}
impl Widget for TableWidget {
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
            crate::widget::metrics::dimensions::TABLE_WIDGET_DEFAULT_WIDTH,
            crate::widget::metrics::dimensions::TABLE_WIDGET_DEFAULT_HEIGHT,
        )
    }

    /// Selects the row under the pointer, then asks the host for a menu about it.
    ///
    /// The right press used to emit the request while leaving the selection where it was, so the
    /// host's menu acted on the previous row (feedback C-9). Selection first makes both describe
    /// the same row; a point outside the rows answers `false`.
    fn context_menu_event(&mut self, pos: crate::core::Point) -> bool {
        let Some(row) = self.row_at_point(pos) else {
            return false;
        };
        self.select_row(row);
        self.context_menu_requested.emit(pos);
        true
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
    ///
    /// # Payload mapping
    ///
    /// A hub name carries no value, so a `Signal1<T>` needs a conversion. The one used here is the
    /// same spelling the property side uses, so a designer reads one representation per kind: a
    /// number as `Int`, a float as `Float`, a flag as `Bool`, text as `String`, and a structured
    /// payload as its debug spelling. A `None` maps to `Null`, which is the same value an absent
    /// optional property uses — so "no selection" is distinguishable from "selection 0".
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "selection_changed" => {
                Some(EventSignalRef::mapped("selection_changed", &self.selection_changed, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            "focused_row_changed" => Some(EventSignalRef::mapped(
                "focused_row_changed",
                &self.focused_row_changed,
                |v| match v {
                    Some(value) => CapabilityValue::UInt(*value as u64),
                    None => CapabilityValue::Null,
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
            "row_activated" => {
                Some(EventSignalRef::mapped("row_activated", &self.row_activated, |v| {
                    crate::widget::capability::CapabilityValue::UInt(*v as u64)
                }))
            }
            "column_changed" => {
                Some(EventSignalRef::mapped("column_changed", &self.column_changed, |v| {
                    crate::widget::capability::CapabilityValue::Tuple(crate::compat::Vec::from([
                        crate::widget::capability::CapabilityValue::String(String::from(
                            v.kind.as_str(),
                        )),
                        crate::widget::capability::CapabilityValue::UInt(v.index as u64),
                    ]))
                }))
            }
            _ => None,
        }
    }
}

/// The published spelling of an edit-trigger set.
///
/// The bitset has four reachable states, so a closed vocabulary is enough; `all` is the
/// combination rather than a fifth gesture.
fn edit_triggers_to_str(triggers: TableEditTriggers) -> &'static str {
    match triggers {
        TableEditTriggers::NONE => "none",
        TableEditTriggers::DOUBLE_CLICKED => "double_clicked",
        TableEditTriggers::EDIT_KEY => "edit_key",
        _ => "all",
    }
}

/// Parses the published edit-trigger spelling, accepting the union spellings a designer writes.
fn expect_edit_triggers(
    value: CapabilityValue,
) -> Result<TableEditTriggers, CapabilityAccessError> {
    let text = expect_string(value)?;
    let mut triggers = TableEditTriggers::NONE;
    for token in text.split(['+', ',', '|', ' ']).filter(|token| !token.is_empty()) {
        match token.to_ascii_lowercase().as_str() {
            "none" => {}
            "double_clicked" | "double-clicked" | "doubleclick" => {
                triggers = triggers.or(TableEditTriggers::DOUBLE_CLICKED);
            }
            "edit_key" | "edit-key" | "enter" => {
                triggers = triggers.or(TableEditTriggers::EDIT_KEY);
            }
            "all" => triggers = TableEditTriggers::ALL,
            _ => return Err(CapabilityAccessError::TypeMismatch),
        }
    }
    Ok(triggers)
}
