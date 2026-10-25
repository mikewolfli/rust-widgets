// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! TreeTable widget combining hierarchical rows with table columns.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::Event;
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::widget::capability::coercion::{expect_bool, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Hierarchical model abstraction for TreeTable.
pub trait TreeTableModel: Send + Sync {
    /// Returns number of root nodes.
    fn root_count(&self) -> usize;
    /// Returns number of children for node path.
    fn child_count(&self, path: &[usize]) -> usize;
    /// Returns total visible columns.
    fn column_count(&self) -> usize;
    /// Returns cell text for node path and column.
    fn data(&self, path: &[usize], column: usize) -> Option<String>;
    /// The heading a column is painted with, if the model names its columns.
    fn column_name(&self, column: usize) -> Option<&str> {
        let _ = column;
        None
    }
    /// The value a sibling sort compares for `path`/`column`.
    ///
    /// Defaults to [`Self::data`], so an existing model sorts by its painted text. A model that
    /// wants a dedicated ordering key (a date, a numeric id, a rank) can return it without changing
    /// what the cell paints.
    fn sort_key(&self, path: &[usize], column: usize) -> Option<String> {
        self.data(path, column)
    }
    /// Writes the cell text into `out`, returning whether the cell exists.
    ///
    /// The default implementation delegates to [`Self::data`], so existing models keep working.
    /// A model that can format directly should override this: the paint path asks for one cell
    /// per visible row per column per frame, and `data`'s owned `String` is a heap allocation for
    /// every one of them (the same reason `TableModel::fill_data` exists).
    fn fill_data(&self, path: &[usize], column: usize, out: &mut String) -> bool {
        match self.data(path, column) {
            Some(text) => {
                out.push_str(&text);
                true
            }
            None => false,
        }
    }
    /// Optional signal emitted when model projection changes.
    fn data_changed_signal(&self) -> Option<&GenericSignal> {
        None
    }

    /// Whether more children can be fetched below `path`.
    ///
    /// `child_count` describes what the model already knows; a lazily loaded model uses this to
    /// answer "there may be more here". The default is `false`, which is the honest answer for an
    /// eager in-memory model and leaves existing models unchanged.
    fn can_fetch_more(&self, path: &[usize]) -> bool {
        let _ = path;
        false
    }

    /// Asks the model to load more children below `path`, returning whether its child count grew.
    ///
    /// **Synchronous and host-driven.** The crate does not own an async runtime, a thread pool or
    /// a timer: this hook is called on the UI thread by
    /// [`TreeTable::fetch_more`](super::tree_table::TreeTable::fetch_more), and a model that
    /// cannot answer immediately returns `false` and later raises its `data_changed` signal when
    /// the work completes. `&self` rather than `&mut self` is deliberate: the control holds the
    /// model in an `Arc<dyn TreeTableModel>`, so a model that mutates must own its storage behind
    /// interior mutability (a `Mutex` or an atomic) — exactly the shape a host's background
    /// loader already has.
    fn fetch_more(&self, path: &[usize]) -> bool {
        let _ = path;
        false
    }
}

/// A single-column, flat model used by [`TreeTable::set_rows`].
///
/// Every row is a root with no children, which is the honest shape for "here are some
/// lines": a caller that has a tree supplies its own [`TreeTableModel`].
struct FlatTreeTableModel {
    rows: Vec<String>,
}

impl TreeTableModel for FlatTreeTableModel {
    fn root_count(&self) -> usize {
        self.rows.len()
    }

    fn child_count(&self, _path: &[usize]) -> usize {
        0
    }

    fn column_count(&self) -> usize {
        1
    }

    fn data(&self, path: &[usize], column: usize) -> Option<String> {
        if column != 0 {
            return None;
        }
        path.first().and_then(|&index| self.rows.get(index)).cloned()
    }

    fn fill_data(&self, path: &[usize], column: usize, out: &mut String) -> bool {
        if column != 0 {
            return false;
        }
        let Some(row) = path.first().and_then(|&index| self.rows.get(index)) else {
            return false;
        };
        out.push_str(row);
        true
    }
}

/// How sibling sort keys are compared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TreeTableSortRole {
    /// Text order, case-folded unless `case_sensitive` is on.
    #[default]
    Text,
    /// Numeric order when both sides parse as a number, text order otherwise.
    Number,
}

/// How much of the tree a projection materialises.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TreeTableProjectionStrategy {
    /// Only the rows around the viewport (the default).
    #[default]
    Windowed,
    /// Every visible row, however long the tree.
    Eager,
}

fn projection_strategy_to_str(strategy: TreeTableProjectionStrategy) -> &'static str {
    match strategy {
        TreeTableProjectionStrategy::Windowed => "windowed",
        TreeTableProjectionStrategy::Eager => "eager",
    }
}

fn expect_projection_strategy(
    value: CapabilityValue,
) -> Result<TreeTableProjectionStrategy, CapabilityAccessError> {
    match value {
        CapabilityValue::String(token) => match token.as_str() {
            "windowed" => Ok(TreeTableProjectionStrategy::Windowed),
            "eager" => Ok(TreeTableProjectionStrategy::Eager),
            _ => Err(CapabilityAccessError::TypeMismatch),
        },
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

/// A normalised sibling sort key, cached for the life of one sort generation.
type SortKey = (String, Option<f64>);

fn normalise_sort_key(text: &str, role: TreeTableSortRole, case_sensitive: bool) -> SortKey {
    let trimmed = text.trim();
    let text = if case_sensitive { trimmed.to_string() } else { trimmed.to_lowercase() };
    let number = match role {
        TreeTableSortRole::Number => trimmed.parse::<f64>().ok(),
        TreeTableSortRole::Text => None,
    };
    (text, number)
}

fn compare_sort_keys(left: &SortKey, right: &SortKey) -> core::cmp::Ordering {
    if let (Some(left), Some(right)) = (left.1, right.1) {
        if let Some(order) = left.partial_cmp(&right) {
            return order;
        }
    }
    left.0.cmp(&right.0)
}

fn tree_table_sort_role_to_str(role: TreeTableSortRole) -> &'static str {
    match role {
        TreeTableSortRole::Text => "text",
        TreeTableSortRole::Number => "number",
    }
}

fn expect_tree_table_sort_role(
    value: CapabilityValue,
) -> Result<TreeTableSortRole, CapabilityAccessError> {
    match value {
        CapabilityValue::String(token) => match token.as_str() {
            "text" => Ok(TreeTableSortRole::Text),
            "number" => Ok(TreeTableSortRole::Number),
            _ => Err(CapabilityAccessError::TypeMismatch),
        },
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct VisibleRow {
    path: Vec<usize>,
    depth: usize,
}

/// TreeTable control with expand/collapse and flattened visible projection.
pub struct TreeTable {
    base: BaseWidget,
    model: Option<Arc<dyn TreeTableModel>>,
    model_connection_scope: ConnectionScope,
    expanded_paths: HashSet<Vec<usize>>,
    visible_rows: Vec<VisibleRow>,
    row_height: u32,
    column_width: u32,
    selected_row: Option<usize>,
    /// The row the pointer is currently over, or `None` when it is between/outside the rows.
    ///
    /// Filled by `MouseMove` through the same [`Self::row_at`] the clicks use, so the highlighted
    /// row is the row a click would affect.
    hovered_row: Option<usize>,
    /// Emits visible row count changes.
    pub projection_changed: Signal1<usize>,
    /// Emits selected visible row index changes.
    pub selection_changed: Signal1<Option<usize>>,
    /// Emitted when a row's expansion changes: `(path, expanded)`.
    pub expansion_changed: Signal1<(Vec<usize>, bool)>,
    /// First visible row.
    scroll_row: usize,
    /// Indent per depth level, in pixels.
    indent: u32,
    /// Whether a column-title row is painted above the data.
    show_header: bool,
    /// The column whose sibling order is sorted, if any.
    sort_column: Option<usize>,
    /// Whether the sort is descending.
    sort_descending: bool,
    /// Emitted when the header is clicked and the sort changed: `(column, descending)`.
    pub sort_changed: Signal1<(usize, bool)>,
    /// Emitted when the context menu is requested at a point.
    pub context_menu_requested: Signal1<Point>,
    /// Whether the projection materializes only the rows around the viewport.
    windowed_projection: bool,
    /// Rows materialized past the viewport in windowed mode.
    window_overscan: usize,
    /// Whether the last projection stopped before the end of the tree.
    projection_truncated: bool,
    /// How sibling sort keys are compared.
    sort_role: TreeTableSortRole,
    /// Whether text sort keys are compared case-sensitively.
    case_sensitive: bool,
    /// Sort keys cached for the current sort column/role/case while the data generation holds.
    sort_key_cache: HashMap<Vec<usize>, SortKey>,
    /// The data generation `sort_key_cache` was populated from.
    sort_key_generation: u64,
    /// Advanced by the model's `data_changed` signal.
    data_generation: Arc<AtomicU64>,
    /// Whether a double-click on a branch expands or collapses it.
    expands_on_double_click: bool,
    /// Whether the middle button still toggles a branch.
    middle_click_expands: bool,
    /// Labels supplied through [`TreeTable::set_header_labels`].
    header_labels: Vec<String>,
    /// Emitted with the visible row a double-click activated.
    row_activated: Signal1<usize>,
    /// Whether expanding a branch asks the model for more children.
    ///
    /// Off by default, so an eager model keeps the historical expand-and-show behaviour. A host
    /// that binds a lazy model sets this and implements
    /// [`TreeTableModel::can_fetch_more`]/[`TreeTableModel::fetch_more`].
    lazy_loading: bool,
    /// Whether the loading placeholder row is painted.
    ///
    /// `fetch_more` maintains it, but it is also settable so a host that completes a fetch on its
    /// own thread can keep the placeholder up until it raises `data_changed`.
    loading_indicator_visible: bool,
    /// Advisory ceiling, in milliseconds, for a host-driven fetch.
    ///
    /// The control never starts a timer (there is no async runtime here), so this value is
    /// published for the host's loader: a loader that exceeds it should report failure through
    /// [`TreeTable::finish_fetch`] rather than leaving the placeholder up forever.
    fetch_timeout_ms: u32,
    /// Paths whose children are currently being fetched.
    loading_paths: Vec<Vec<usize>>,
    /// Emitted when [`TreeTable::fetch_more`] asks the model for `path`.
    pub fetch_started: Signal1<Vec<usize>>,
    /// Emitted when a fetch for `path` finishes; the `bool` is whether the model grew.
    pub fetch_finished: Signal1<(Vec<usize>, bool)>,
}

impl TreeTable {
    /// Creates an empty tree table.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::TreeTable, geometry, "TreeTable"),
            model: None,
            model_connection_scope: ConnectionScope::new(),
            expanded_paths: HashSet::new(),
            visible_rows: Vec::new(),
            row_height: 20,
            column_width: 140,
            selected_row: None,
            hovered_row: None,
            projection_changed: Signal1::new(),
            selection_changed: Signal1::new(),
            expansion_changed: Signal1::new(),
            scroll_row: 0,
            indent: 14,
            show_header: false,
            sort_column: None,
            sort_descending: false,
            sort_changed: Signal1::new(),
            context_menu_requested: Signal1::new(),
            windowed_projection: true,
            window_overscan: 8,
            projection_truncated: false,
            sort_role: TreeTableSortRole::Text,
            case_sensitive: false,
            sort_key_cache: HashMap::new(),
            sort_key_generation: 0,
            data_generation: Arc::new(AtomicU64::new(0)),
            expands_on_double_click: true,
            middle_click_expands: true,
            header_labels: Vec::new(),
            row_activated: Signal1::new(),
            lazy_loading: false,
            loading_indicator_visible: false,
            fetch_timeout_ms: 5000,
            loading_paths: Vec::new(),
            fetch_started: Signal1::new(),
            fetch_finished: Signal1::new(),
        }
    }

    /// Binds flat, single-column rows without requiring a trait implementation.
    ///
    /// The lightweight counterpart of [`Self::set_model`]: a host or snapshot that only
    /// has lines of text no longer has to implement [`TreeTableModel`] to show them
    /// (VIEW-N05). It delegates to `set_model`, so expansion state, selection signalling
    /// and projection rebuilds behave exactly as for any other model.
    pub fn set_rows(&mut self, rows: Vec<String>) {
        self.set_model(Arc::new(FlatTreeTableModel { rows }));
    }

    /// Binds model and rebuilds visible projection.
    pub fn set_model(&mut self, model: Arc<dyn TreeTableModel>) {
        self.model_connection_scope = ConnectionScope::new();
        if let Some(data_changed) = model.data_changed_signal() {
            let redraw = self.base.redraw_requested_signal().clone();
            let layout = self.base.layout_requested_signal().clone();
            let generation = Arc::clone(&self.data_generation);
            data_changed.connect_scoped(&self.model_connection_scope, move || {
                generation.fetch_add(1, Ordering::Relaxed);
                redraw.emit();
                layout.emit();
            });
        }

        self.model = Some(model);
        self.data_generation.fetch_add(1, Ordering::Relaxed);
        self.sort_key_cache.clear();
        self.sort_key_generation = self.data_generation.load(Ordering::Relaxed);
        self.expanded_paths.clear();
        // A new model invalidates the old selection: the visible rows no longer refer to the same
        // nodes. The clear is captured before it happens so it can be **announced** afterwards
        // (BLUE-issue E-20): a subscriber must see the selection go from its old value to `None`, not
        // have it vanish silently.
        let previous_selection = self.selected_row.take();
        self.rebuild_projection();
        self.announce_selection_change(previous_selection);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Clears model and resets projection.
    pub fn clear_model(&mut self) {
        self.model_connection_scope = ConnectionScope::new();
        self.model = None;
        self.expanded_paths.clear();
        self.visible_rows.clear();
        self.selected_row = None;
        self.projection_changed.emit(0);
        self.selection_changed.emit(None);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns whether model is bound.
    pub fn has_model(&self) -> bool {
        self.model.is_some()
    }

    /// Returns visible row count.
    pub fn row_count(&self) -> usize {
        self.visible_rows.len()
    }

    /// Returns model column count.
    pub fn column_count(&self) -> usize {
        self.model.as_ref().map(|m| m.column_count()).unwrap_or(0)
    }

    /// Returns selected visible row.
    pub fn selected_row(&self) -> Option<usize> {
        self.selected_row.filter(|row| *row < self.visible_rows.len())
    }

    /// Sets fixed row height.
    pub fn set_row_height(&mut self, row_height: u32) {
        let row_height = row_height.max(1);
        if self.row_height == row_height {
            return;
        }
        self.row_height = row_height;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns row height.
    pub fn row_height(&self) -> u32 {
        self.row_height
    }

    /// Sets fixed column width.
    pub fn set_column_width(&mut self, column_width: u32) {
        let column_width = column_width.max(1);
        if self.column_width == column_width {
            return;
        }
        self.column_width = column_width;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns column width.
    pub fn column_width(&self) -> u32 {
        self.column_width
    }

    /// Returns the node path for visible row.
    pub fn row_path(&self, row: usize) -> Option<&[usize]> {
        self.visible_rows.get(row).map(|visible| visible.path.as_slice())
    }

    /// Returns whether visible row is expanded.
    pub fn is_row_expanded(&self, row: usize) -> Option<bool> {
        let path = self.row_path(row)?;
        Some(self.expanded_paths.contains(path))
    }

    /// Returns depth for visible row.
    pub fn row_depth(&self, row: usize) -> Option<usize> {
        self.visible_rows.get(row).map(|visible| visible.depth)
    }

    /// Returns a cell value from visible projection.
    pub fn item(&self, row: usize, column: usize) -> Option<String> {
        let model = self.model.as_ref()?;
        let path = self.row_path(row)?;
        model.data(path, column)
    }

    /// Writes a visible cell's text into `out`, avoiding `item`'s per-cell allocation.
    fn fill_item(&self, row: usize, column: usize, out: &mut String) -> bool {
        let Some(model) = self.model.as_ref() else {
            return false;
        };
        let Some(path) = self.row_path(row) else {
            return false;
        };
        model.fill_data(path, column, out)
    }

    /// Expands row when it has children.
    pub fn expand_row(&mut self, row: usize) -> bool {
        let Some(path) = self.row_path(row).map(ToOwned::to_owned) else {
            return false;
        };
        // A lazy branch is expandable before its children exist: that is the whole point of
        // `can_fetch_more`. An eager branch keeps the historical "only with children" rule.
        let fetchable = self.lazy_loading && self.can_fetch_more(&path);
        if !self.path_has_children(&path) && !fetchable {
            return false;
        }
        if !self.expanded_paths.insert(path.clone()) {
            return true;
        }
        self.expansion_changed.emit((path.clone(), true));
        // Insert only the rows the branch reveals when the projection is complete. A truncated
        // windowed projection cannot prove where the tail belongs, so it falls back to one full
        // (budget-limited) rebuild rather than splicing a row into the wrong slot.
        if !self.insert_expanded_rows(row, &path) {
            self.rebuild_projection();
        }
        if fetchable {
            // Host-driven and synchronous; see `TreeTableModel::fetch_more`.
            let _ = self.fetch_more(&path);
        }
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    /// Whether expanding a branch asks the model for more children.
    pub fn lazy_loading(&self) -> bool {
        self.lazy_loading
    }

    /// Enables or disables fetch-on-expand.
    pub fn set_lazy_loading(&mut self, enabled: bool) {
        if self.lazy_loading == enabled {
            return;
        }
        self.lazy_loading = enabled;
        self.base.request_redraw();
    }

    /// Whether the loading placeholder row is being painted.
    pub fn loading_indicator_visible(&self) -> bool {
        self.loading_indicator_visible
    }

    /// Shows or hides the loading placeholder row.
    pub fn set_loading_indicator_visible(&mut self, visible: bool) {
        if self.loading_indicator_visible == visible {
            return;
        }
        self.loading_indicator_visible = visible;
        self.base.request_redraw();
    }

    /// The advisory fetch ceiling, in milliseconds. See the field docs for why it is advisory.
    pub fn fetch_timeout_ms(&self) -> u32 {
        self.fetch_timeout_ms
    }

    /// Sets the advisory fetch ceiling.
    pub fn set_fetch_timeout_ms(&mut self, timeout_ms: u32) {
        self.fetch_timeout_ms = timeout_ms;
    }

    /// Paths whose children are currently being fetched.
    pub fn loading_paths(&self) -> &[Vec<usize>] {
        &self.loading_paths
    }

    /// Whether the model can load more children below `path`.
    ///
    /// False while [`Self::lazy_loading`] is off: the flag is the host's statement that the model
    /// is lazy, and a control that fetched anyway would surprise an eager model.
    pub fn can_fetch_more(&self, path: &[usize]) -> bool {
        self.lazy_loading && self.model.as_ref().is_some_and(|model| model.can_fetch_more(path))
    }

    /// Asks the model for more children below `path` and reports the outcome.
    ///
    /// **Synchronous and host-driven.** `fetch_started` is emitted before the model is called and
    /// `fetch_finished` after it returns, so a subscriber sees a complete pair even when the
    /// model answers immediately. A model that cannot answer synchronously returns `false` here
    /// (announcing a failed fetch), keeps the path pending, and calls [`Self::finish_fetch`] when
    /// its background work completes — the control does not start an async runtime for it.
    pub fn fetch_more(&mut self, path: &[usize]) -> bool {
        if !self.can_fetch_more(path) {
            return false;
        }
        if !self.loading_paths.iter().any(|pending| pending == path) {
            self.loading_paths.push(path.to_vec());
        }
        self.loading_indicator_visible = true;
        self.fetch_started.emit(path.to_vec());
        let changed = self.model.as_ref().is_some_and(|model| model.fetch_more(path));
        self.finish_fetch(path, changed);
        changed
    }

    /// Completes a fetch the host performed on its own schedule.
    ///
    /// Removes `path` from [`Self::loading_paths`], hides the placeholder once the last pending
    /// path finishes, and emits `fetch_finished`. `changed` says whether the model grew, so a
    /// successful background load rebuilds the visible projection exactly once.
    pub fn finish_fetch(&mut self, path: &[usize], changed: bool) {
        self.loading_paths.retain(|pending| pending != path);
        self.loading_indicator_visible = !self.loading_paths.is_empty();
        if changed {
            self.rebuild_projection();
            self.base.request_layout();
        }
        self.fetch_finished.emit((path.to_vec(), changed));
        self.base.request_redraw();
    }

    /// Collapses row.
    pub fn collapse_row(&mut self, row: usize) -> bool {
        let Some(path) = self.row_path(row).map(ToOwned::to_owned) else {
            return false;
        };
        if !self.expanded_paths.remove(&path) {
            return true;
        }
        self.expanded_paths.retain(|expanded| !Self::is_descendant_of(expanded, &path));
        self.expansion_changed.emit((path.clone(), false));
        if !self.remove_expanded_rows(&path) {
            self.rebuild_projection();
        }
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    /// Toggles row expansion state.
    pub fn toggle_row_expanded(&mut self, row: usize) -> bool {
        match self.is_row_expanded(row) {
            Some(true) => self.collapse_row(row),
            Some(false) => self.expand_row(row),
            None => false,
        }
    }

    /// Expands every branch in the tree.
    ///
    /// Returns how many branches changed state. The projection is rebuilt once after the state is
    /// collected, rather than once per `expand_row` call.
    pub fn expand_all(&mut self) -> usize {
        let Some(model) = self.model.clone() else {
            return 0;
        };
        let mut paths = Vec::new();
        self.collect_paths(model.as_ref(), None, model.root_count(), &mut paths);
        let mut changed = 0usize;
        for path in paths {
            if self.path_has_children(&path) && self.expanded_paths.insert(path.clone()) {
                self.expansion_changed.emit((path, true));
                changed += 1;
            }
        }
        if changed > 0 {
            self.rebuild_projection();
            self.base.request_layout();
            self.base.request_redraw();
        }
        changed
    }

    /// Collapses every branch in the tree.
    pub fn collapse_all(&mut self) -> usize {
        let mut paths: Vec<Vec<usize>> = self.expanded_paths.iter().cloned().collect();
        // Deepest first keeps the signal order consistent with a sequence of individual collapses.
        paths.sort_by_key(|left| core::cmp::Reverse(left.len()));
        let changed = paths.len();
        self.expanded_paths.clear();
        for path in paths {
            self.expansion_changed.emit((path, false));
        }
        if changed > 0 {
            self.rebuild_projection();
            self.base.request_layout();
            self.base.request_redraw();
        }
        changed
    }

    /// Collects every node path in model order, depth-first.
    fn collect_paths(
        &mut self,
        model: &dyn TreeTableModel,
        parent: Option<&[usize]>,
        count: usize,
        out: &mut Vec<Vec<usize>>,
    ) {
        for child in self.ordered_children(model, parent, count) {
            let mut path = parent.map(ToOwned::to_owned).unwrap_or_default();
            path.push(child);
            out.push(path.clone());
            let child_count = model.child_count(&path);
            if child_count > 0 {
                self.collect_paths(model, Some(&path), child_count, out);
            }
        }
    }

    /// Inserts the rows below an expanded branch, returning whether it could do so incrementally.
    fn insert_expanded_rows(&mut self, row: usize, path: &[usize]) -> bool {
        let Some(model) = self.model.clone() else {
            return false;
        };
        let Some(depth) = self.row_depth(row) else {
            return false;
        };
        let remaining = if self.windowed_projection {
            self.projection_budget().saturating_sub(self.visible_rows.len()).saturating_add(1)
        } else {
            usize::MAX
        };
        let mut added = Vec::new();
        let mut child_path = path.to_vec();
        let child_count = model.child_count(path);
        let complete = self.flatten_children(
            model.as_ref(),
            &mut added,
            &mut child_path,
            depth + 1,
            Some(path),
            child_count,
            remaining,
        );
        if !complete && self.windowed_projection {
            return false;
        }
        let previous_path = self.selected_path();
        self.visible_rows.splice(row.saturating_add(1)..row.saturating_add(1), added);
        self.projection_truncated = false;
        self.restore_selection_by_path(previous_path);
        self.projection_changed.emit(self.visible_rows.len());
        true
    }

    /// Removes the rows below a collapsed branch, returning whether it could do so incrementally.
    fn remove_expanded_rows(&mut self, path: &[usize]) -> bool {
        let previous_path = self.selected_path();
        let before = self.visible_rows.len();
        self.visible_rows.retain(|row| !Self::is_descendant_of(&row.path, path));
        let removed = before.saturating_sub(self.visible_rows.len());
        if self.windowed_projection && self.projection_truncated {
            // The rows below the removed section were never materialized; refill the window.
            return false;
        }
        self.restore_selection_by_path(previous_path);
        if removed > 0 {
            self.projection_changed.emit(self.visible_rows.len());
        }
        true
    }

    /// Flattens the children of `parent` into `acc`, with the same budget rule as a full rebuild.
    #[allow(clippy::too_many_arguments)]
    fn flatten_children(
        &mut self,
        model: &dyn TreeTableModel,
        acc: &mut Vec<VisibleRow>,
        path: &mut Vec<usize>,
        depth: usize,
        parent: Option<&[usize]>,
        child_count: usize,
        budget: usize,
    ) -> bool {
        for child in self.ordered_children(model, parent, child_count) {
            path.push(child);
            let keep_going = self.flatten_path(model, acc, path, depth, budget);
            path.pop();
            if !keep_going {
                return false;
            }
        }
        true
    }

    /// The selected row's model path, used to hold its identity across a splice.
    fn selected_path(&self) -> Option<Vec<usize>> {
        self.selected_row.and_then(|row| self.visible_rows.get(row).map(|row| row.path.clone()))
    }

    /// Re-points the selection at the same path after rows were inserted or removed.
    fn restore_selection_by_path(&mut self, previous: Option<Vec<usize>>) {
        let restored =
            previous.and_then(|path| self.visible_rows.iter().position(|row| row.path == path));
        if restored != self.selected_row {
            self.selected_row = restored;
            self.selection_changed.emit(self.selected_row);
        }
    }

    /// Selects a visible row.
    pub fn select_row(&mut self, row: usize) -> bool {
        if row >= self.visible_rows.len() {
            return false;
        }
        if self.selected_row == Some(row) {
            return true;
        }
        self.selected_row = Some(row);
        self.selection_changed.emit(self.selected_row);
        self.base.request_redraw();
        true
    }

    fn path_has_children(&self, path: &[usize]) -> bool {
        self.model.as_ref().map(|model| model.child_count(path) > 0).unwrap_or(false)
    }

    fn rebuild_projection(&mut self) {
        let previous_selection = self.selected_row;
        let mut next = Vec::new();
        let Some(model) = self.model.clone() else {
            self.visible_rows = next;
            self.projection_truncated = false;
            self.selected_row = None;
            self.projection_changed.emit(0);
            self.announce_selection_change(previous_selection);
            return;
        };

        let budget = self.projection_budget();
        let mut truncated = false;
        for root in self.ordered_children(model.as_ref(), None, model.root_count()) {
            let mut path = vec![root];
            if !self.flatten_path(model.as_ref(), &mut next, &mut path, 0, budget) {
                truncated = true;
                break;
            }
        }

        self.visible_rows = next;
        self.projection_truncated = truncated;
        self.selected_row = self.selected_row.filter(|row| *row < self.visible_rows.len());
        self.projection_changed.emit(self.visible_rows.len());
        self.announce_selection_change(previous_selection);
    }

    /// Emits `selection_changed` when a projection rebuild cleared or moved the selection.
    ///
    /// # The defect this closes (BLUE-issue E-20)
    ///
    /// Collapsing a row (or replacing the model) can remove the selected row from the visible
    /// projection, so `selected_row` silently becomes `None` while only `projection_changed` fires.
    /// A subscriber saw the selection disappear with no `selection_changed`, which is inconsistent
    /// with `clear_model`, which *does* announce it. Announcing from the one place the selection can
    /// change keeps every path \u2014 collapse, model swap, clear \u2014 reporting the same fact.
    fn announce_selection_change(&self, previous: Option<usize>) {
        if self.selected_row != previous {
            self.selection_changed.emit(self.selected_row);
        }
    }

    /// Flattens `path` into `acc`, stopping at `budget` rows.
    ///
    /// Returns `false` when the budget was reached, which lets the caller stop walking the
    /// remaining roots and report a truncated projection.
    fn flatten_path(
        &mut self,
        model: &dyn TreeTableModel,
        acc: &mut Vec<VisibleRow>,
        path: &mut Vec<usize>,
        depth: usize,
        budget: usize,
    ) -> bool {
        if acc.len() >= budget {
            return false;
        }
        acc.push(VisibleRow { path: path.clone(), depth });

        if !self.expanded_paths.contains(path) {
            return true;
        }

        let child_count = model.child_count(path);
        let children = self.ordered_children(model, Some(path.as_slice()), child_count);
        for child_index in children {
            path.push(child_index);
            let keep_going = self.flatten_path(model, acc, path, depth + 1, budget);
            let _ = path.pop();
            if !keep_going {
                return false;
            }
        }
        true
    }

    /// The child indices below `parent` in painted order.
    ///
    /// Model order is preserved unless a sort column is set. Keys are normalised through the
    /// configured role and case policy and cached by path, so a rebuild does not re-lowercase (and
    /// re-parse) every child again; the cache is dropped when the sort inputs or the data
    /// generation change.
    fn ordered_children(
        &mut self,
        model: &dyn TreeTableModel,
        parent: Option<&[usize]>,
        count: usize,
    ) -> Vec<usize> {
        let children: Vec<usize> = (0..count).collect();
        let Some(column) = self.sort_column else {
            return children;
        };
        let generation = self.data_generation.load(Ordering::Relaxed);
        if generation != self.sort_key_generation {
            self.sort_key_cache.clear();
            self.sort_key_generation = generation;
        }
        let mut keyed: Vec<(SortKey, usize)> = children
            .iter()
            .map(|child| {
                let mut path = parent.map(ToOwned::to_owned).unwrap_or_default();
                path.push(*child);
                let key = match self.sort_key_cache.get(path.as_slice()) {
                    Some(key) => key.clone(),
                    None => {
                        let raw = model.sort_key(&path, column).unwrap_or_default();
                        let key = normalise_sort_key(&raw, self.sort_role, self.case_sensitive);
                        self.sort_key_cache.insert(path, key.clone());
                        key
                    }
                };
                (key, *child)
            })
            .collect();
        keyed.sort_by(|a, b| compare_sort_keys(&a.0, &b.0).then(a.1.cmp(&b.1)));
        if self.sort_descending {
            keyed.reverse();
        }
        keyed.into_iter().map(|(_, child)| child).collect()
    }

    fn row_at(&self, y: i32) -> Option<usize> {
        let rect = self.base.geometry();
        let top = rect.y + self.header_offset();
        if y < top || y >= rect.y + rect.height as i32 {
            return None;
        }
        let local = (y - top).max(0) as usize;
        let index = local / self.row_height as usize + self.scroll_row;
        (index < self.visible_rows.len()).then_some(index)
    }

    /// The vertical room the column-title row takes.
    fn header_offset(&self) -> i32 {
        if self.show_header {
            self.row_height as i32
        } else {
            0
        }
    }

    /// The first visible row.
    pub fn scroll_row(&self) -> usize {
        self.scroll_row
    }

    /// Sets the first visible row, clamped to the projection.
    pub fn set_scroll_row(&mut self, row: usize) {
        // In windowed mode the projection stops near the viewport, so a scroll past its end
        // re-materializes it with a budget that reaches the requested row.
        if self.windowed_projection
            && self.projection_truncated
            && row >= self.visible_rows.len().saturating_sub(self.window_overscan)
        {
            self.scroll_row = row;
            self.rebuild_projection();
        }
        let max = self.visible_rows.len().saturating_sub(1);
        let next = row.min(max);
        if next == self.scroll_row {
            return;
        }
        self.scroll_row = next;
        self.base.request_redraw();
    }

    /// Indent per depth level in pixels.
    pub fn indent(&self) -> u32 {
        self.indent
    }

    /// Sets the indent per depth level.
    pub fn set_indent(&mut self, indent: u32) {
        let indent = indent.min(64);
        if self.indent == indent {
            return;
        }
        self.indent = indent;
        self.base.request_redraw();
    }

    /// Whether the column-title row is painted.
    pub fn show_header(&self) -> bool {
        self.show_header
    }

    /// Enables or disables the column-title row.
    pub fn set_show_header(&mut self, show: bool) {
        if self.show_header == show {
            return;
        }
        self.show_header = show;
        self.base.request_redraw();
    }

    /// Whether the column-title row is painted (`show_header`'s alias).
    pub fn header_visible(&self) -> bool {
        self.show_header
    }

    /// Shows or hides the column-title row (`set_show_header`'s alias).
    pub fn set_header_visible(&mut self, visible: bool) {
        self.set_show_header(visible);
    }

    /// The header labels supplied by the host.
    pub fn header_labels(&self) -> &[String] {
        &self.header_labels
    }

    /// Replaces the header labels, taking precedence over [`TreeTableModel::column_name`].
    pub fn set_header_labels(&mut self, labels: &[&str]) {
        self.header_labels = labels.iter().map(|label| (*label).to_string()).collect();
        self.base.request_redraw();
    }

    /// Whether a double-click on a branch expands or collapses it.
    pub fn expands_on_double_click(&self) -> bool {
        self.expands_on_double_click
    }

    /// Enables or disables double-click expansion.
    pub fn set_expands_on_double_click(&mut self, expands: bool) {
        self.expands_on_double_click = expands;
    }

    /// Whether the middle button toggles a branch.
    pub fn middle_click_expands(&self) -> bool {
        self.middle_click_expands
    }

    /// Enables or disables middle-button expansion.
    pub fn set_middle_click_expands(&mut self, expands: bool) {
        self.middle_click_expands = expands;
    }

    /// The signal emitted with the row a double-click activated.
    pub fn row_activated_signal(&self) -> &Signal1<usize> {
        &self.row_activated
    }

    /// The column whose sibling order is sorted, if any.
    pub fn sort_column(&self) -> Option<usize> {
        self.sort_column
    }

    /// Whether the sibling sort is descending.
    pub fn sort_descending(&self) -> bool {
        self.sort_descending
    }

    /// Sorts siblings by `column`, or restores model order with `None`.
    pub fn set_sort_column(&mut self, column: Option<usize>) {
        let column = column.filter(|column| *column < self.column_count());
        if self.sort_column == column {
            return;
        }
        self.sort_column = column;
        self.sort_descending = false;
        self.sort_key_cache.clear();
        self.sort_key_generation = self.data_generation.load(Ordering::Relaxed);
        self.rebuild_projection();
        self.base.request_redraw();
    }

    /// Toggles the sort direction of `column`, selecting it when it was not sorted.
    pub fn toggle_sort_column(&mut self, column: usize) {
        if column >= self.column_count() {
            return;
        }
        if self.sort_column == Some(column) {
            self.sort_descending = !self.sort_descending;
        } else {
            self.sort_column = Some(column);
            self.sort_descending = false;
        }
        let descending = self.sort_descending;
        self.sort_key_cache.clear();
        self.sort_key_generation = self.data_generation.load(Ordering::Relaxed);
        self.rebuild_projection();
        self.sort_changed.emit((column, descending));
        self.base.request_redraw();
    }

    /// How sibling sort keys are compared.
    pub fn sort_role(&self) -> TreeTableSortRole {
        self.sort_role
    }

    /// Sets how sibling sort keys are compared.
    pub fn set_sort_role(&mut self, role: TreeTableSortRole) {
        if self.sort_role == role {
            return;
        }
        self.sort_role = role;
        self.sort_key_cache.clear();
        self.rebuild_projection();
        self.base.request_redraw();
    }

    /// Whether text sort keys are compared case-sensitively.
    pub fn case_sensitive(&self) -> bool {
        self.case_sensitive
    }

    /// Sets the case policy for text sort keys.
    pub fn set_case_sensitive(&mut self, case_sensitive: bool) {
        if self.case_sensitive == case_sensitive {
            return;
        }
        self.case_sensitive = case_sensitive;
        self.sort_key_cache.clear();
        self.rebuild_projection();
        self.base.request_redraw();
    }

    /// Whether the projection materializes only the rows around the viewport.
    pub fn windowed_projection(&self) -> bool {
        self.windowed_projection
    }

    /// The materialisation strategy in its named form.
    pub fn projection_strategy(&self) -> TreeTableProjectionStrategy {
        if self.windowed_projection {
            TreeTableProjectionStrategy::Windowed
        } else {
            TreeTableProjectionStrategy::Eager
        }
    }

    /// Sets the materialisation strategy.
    pub fn set_projection_strategy(&mut self, strategy: TreeTableProjectionStrategy) {
        self.set_windowed_projection(strategy == TreeTableProjectionStrategy::Windowed);
    }

    /// Enables or disables the windowed projection.
    ///
    /// On by default: a large tree's expansion must not flatten the whole document to paint the
    /// viewport. When on, a rebuild materializes only the rows the viewport can show plus
    /// overscan, so opening a branch of a large tree stops flattening as soon as the visible
    /// window is full. [`Self::projection_truncated`] says whether more rows remain, and
    /// scrolling extends the projection on demand. Turning it off restores the eager projection
    /// for a caller that needs `row_count` to describe the whole tree.
    pub fn set_windowed_projection(&mut self, windowed: bool) {
        if self.windowed_projection == windowed {
            return;
        }
        self.windowed_projection = windowed;
        self.rebuild_projection();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Whether the last projection stopped before the end of the tree.
    pub fn projection_truncated(&self) -> bool {
        self.projection_truncated
    }

    /// The column whose header band covers `pos`, when the header row is shown.
    fn header_column_at(&self, pos: Point) -> Option<usize> {
        if !self.show_header {
            return None;
        }
        let rect = self.base.geometry();
        let band = Rect::new(rect.x, rect.y, rect.width, self.row_height);
        if !band.contains_point(pos) {
            return None;
        }
        let column = (pos.x - rect.x).max(0) as usize / self.column_width.max(1) as usize;
        (column < self.column_count()).then_some(column)
    }

    /// How many rows the projection may materialize.
    fn projection_budget(&self) -> usize {
        if !self.windowed_projection {
            return usize::MAX;
        }
        let capacity = (self.base.geometry().height / self.row_height.max(1)) as usize;
        self.scroll_row
            .saturating_add(capacity)
            .saturating_add(self.window_overscan)
            .max(capacity)
            .max(1)
    }

    fn is_descendant_of(path: &[usize], ancestor: &[usize]) -> bool {
        path.len() > ancestor.len() && path[..ancestor.len()] == *ancestor
    }
}

impl Widget for TreeTable {
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
            crate::widget::metrics::dimensions::TREE_TABLE_DEFAULT_WIDTH,
            crate::widget::metrics::dimensions::TREE_TABLE_DEFAULT_HEIGHT,
        )
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
            "projection_changed" => {
                Some(EventSignalRef::mapped("projection_changed", &self.projection_changed, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            "selection_changed" => {
                Some(EventSignalRef::mapped("selection_changed", &self.selection_changed, |v| {
                    match v {
                        Some(value) => CapabilityValue::UInt(*value as u64),
                        None => CapabilityValue::Null,
                    }
                }))
            }
            "context_menu_requested" => Some(EventSignalRef::mapped(
                "context_menu_requested",
                &self.context_menu_requested,
                |v| crate::widget::capability::CapabilityValue::String(format!("{},{}", v.x, v.y)),
            )),
            "expansion_changed" => {
                Some(EventSignalRef::mapped("expansion_changed", &self.expansion_changed, |v| {
                    crate::widget::capability::CapabilityValue::Tuple(crate::compat::Vec::from([
                        crate::widget::capability::CapabilityValue::Tuple(
                            v.0.iter()
                                .map(|item| {
                                    crate::widget::capability::CapabilityValue::UInt(*item as u64)
                                })
                                .collect(),
                        ),
                        crate::widget::capability::CapabilityValue::Bool(v.1),
                    ]))
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
            "fetch_started" => {
                Some(EventSignalRef::mapped("fetch_started", &self.fetch_started, |v| {
                    crate::widget::capability::CapabilityValue::Tuple(
                        v.iter()
                            .map(|item| {
                                crate::widget::capability::CapabilityValue::UInt(*item as u64)
                            })
                            .collect(),
                    )
                }))
            }
            "fetch_finished" => {
                Some(EventSignalRef::mapped("fetch_finished", &self.fetch_finished, |v| {
                    crate::widget::capability::CapabilityValue::Tuple(crate::compat::Vec::from([
                        crate::widget::capability::CapabilityValue::Tuple(
                            v.0.iter()
                                .map(|item| {
                                    crate::widget::capability::CapabilityValue::UInt(*item as u64)
                                })
                                .collect(),
                        ),
                        crate::widget::capability::CapabilityValue::Bool(v.1),
                    ]))
                }))
            }
            _ => None,
        }
    }
}

/// `TreeTable`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_view.in.rs` / `access_write_view.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `TreeTable` reports
/// `WidgetKind::TreeView`, which it shares with `TreeView`; dispatching on the
/// concrete type here keeps the two contracts from answering for each other.
impl WidgetProperties for TreeTable {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "has_model" => Ok(CapabilityValue::Bool(self.has_model())),
            "row_count" => Ok(CapabilityValue::UInt(self.row_count() as u64)),
            "column_count" => Ok(CapabilityValue::UInt(self.column_count() as u64)),
            "selected_row" => match self.selected_row() {
                Some(row) => Ok(CapabilityValue::UInt(row as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "row_height" => Ok(CapabilityValue::UInt(self.row_height() as u64)),
            "column_width" => Ok(CapabilityValue::UInt(self.column_width() as u64)),
            "header_visible" => Ok(CapabilityValue::Bool(self.header_visible())),
            "expands_on_double_click" => Ok(CapabilityValue::Bool(self.expands_on_double_click())),
            "middle_click_expands" => Ok(CapabilityValue::Bool(self.middle_click_expands())),
            "sort_role" => Ok(CapabilityValue::String(
                tree_table_sort_role_to_str(self.sort_role()).to_string(),
            )),
            "case_sensitive" => Ok(CapabilityValue::Bool(self.case_sensitive())),
            "projection_strategy" => Ok(CapabilityValue::String(
                projection_strategy_to_str(self.projection_strategy()).to_string(),
            )),
            "lazy_loading" => Ok(CapabilityValue::Bool(self.lazy_loading())),
            "loading_indicator_visible" => {
                Ok(CapabilityValue::Bool(self.loading_indicator_visible()))
            }
            "fetch_timeout_ms" => Ok(CapabilityValue::UInt(self.fetch_timeout_ms() as u64)),
            "projection_state" => {
                let selected = match self.selected_row() {
                    Some(row) => format!("Some({row})"),
                    None => "None".to_string(),
                };
                Ok(CapabilityValue::String(format!(
                    "rows={},selected={}",
                    self.row_count(),
                    selected
                )))
            }
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "selected_row" => match value {
                CapabilityValue::Null => {
                    if let Some(selected) = self.selected_row() {
                        let _ = self.select_row(selected);
                    }
                    Ok(())
                }
                other => {
                    let row = expect_usize(other)?;
                    if self.select_row(row) || self.row_count() == 0 {
                        Ok(())
                    } else {
                        // `select_row` answers `false` for a row past the last one;
                        // the empty-table case is already accepted above, so what
                        // remains is an out-of-range index.
                        Err(CapabilityAccessError::OutOfRange)
                    }
                }
            },
            "row_height" => {
                self.set_row_height(crate::widget::capability::coercion::expect_u32(value)?);
                Ok(())
            }
            "column_width" => {
                self.set_column_width(crate::widget::capability::coercion::expect_u32(value)?);
                Ok(())
            }
            "header_visible" => {
                self.set_header_visible(expect_bool(value)?);
                Ok(())
            }
            "expands_on_double_click" => {
                self.set_expands_on_double_click(expect_bool(value)?);
                Ok(())
            }
            "middle_click_expands" => {
                self.set_middle_click_expands(expect_bool(value)?);
                Ok(())
            }
            "sort_role" => {
                self.set_sort_role(expect_tree_table_sort_role(value)?);
                Ok(())
            }
            "case_sensitive" => {
                self.set_case_sensitive(expect_bool(value)?);
                Ok(())
            }
            "projection_strategy" => {
                self.set_projection_strategy(expect_projection_strategy(value)?);
                Ok(())
            }
            "lazy_loading" => {
                self.set_lazy_loading(expect_bool(value)?);
                Ok(())
            }
            "loading_indicator_visible" => {
                self.set_loading_indicator_visible(expect_bool(value)?);
                Ok(())
            }
            "fetch_timeout_ms" => {
                self.set_fetch_timeout_ms(crate::widget::capability::coercion::expect_u32(value)?);
                Ok(())
            }
            // Derived counts and the projection summary are computed from the
            // model, so they are refused as read-only rather than reported as
            // names this control does not know.
            "has_model" | "row_count" | "column_count" | "projection_state" => {
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "has_model",
            "row_count",
            "column_count",
            "selected_row",
            "row_height",
            "column_width",
            "header_visible",
            "expands_on_double_click",
            "middle_click_expands",
            "sort_role",
            "case_sensitive",
            "projection_strategy",
            "lazy_loading",
            "loading_indicator_visible",
            "fetch_timeout_ms",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `tree_table` publishes.
    ///
    /// `clear_model` is payload-free and executes here. `expand_row` /
    /// `collapse_row` / `select_row` need a row index and `set_model` needs a
    /// model object, so those are answered through the property route.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "clear_model" => {
                self.clear_model();
                Ok(())
            }
            "set_model" | "expand_row" | "collapse_row" | "select_row" => {
                Err(CapabilityAccessError::OutOfRange)
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for TreeTable {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();

        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then a literal. The theme step is what makes an appearance
        // switch visible; the surface, the border, the selected-row highlight, the cell
        // outlines and the text colour used to be hardcoded literals, so light and dark
        // rendered identically.
        //
        // The theme reads take and release the global manager's lock internally, so no guard
        // is held across the draw (the mutex is not re-entrant).
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("tree_table");
        let mut surface = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        // `tree_table` is absent from `WidgetRole::for_kind_name`'s table, so it classifies as
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
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != surface)
            .unwrap_or_else(|| surface.blend(&ink, 0.20));
        // The cell grid is the theme's `outline_variant` — the **weak** separator role — so a
        // tree table's grid is visibly weaker than its `outline`-strength focus ring (§5.4), and so
        // it moves with a theme that tunes its separators. The local blend stays as the last rung for
        // a build with no theme.
        //
        // One source for the whole table family: `table_widget`, `virtual_table` and this control all
        // share the `Table`/`DataView` grid, so a theme cannot end up tuning one and not the others.
        let cell_border = palette
            .as_ref()
            .map(|colors| colors.outline_variant)
            .filter(|resolved| *resolved != surface)
            .unwrap_or_else(|| surface.blend(&ink, 0.10));
        // The selected row is a selection state, so it reads the theme's accent token and is
        // laid over the surface, which keeps it legible in either appearance.
        let accent = palette.as_ref().map(|colors| colors.primary).unwrap_or(Color::PRIMARY);
        let selected_bg = surface.blend(&accent, 0.30);

        context.face_with_gradient(
            rect,
            surface,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );
        context.draw_rect(rect, border);

        if self.visible_rows.is_empty() {
            return;
        }

        let columns = self.column_count();
        if columns == 0 {
            return;
        }

        let row_h = self.row_height as i32;
        let col_w = self.column_width as i32;
        // One font and one text buffer for the whole grid (see the cell loop below).
        let cell_font = effective_font(style);
        let mut cell_text = String::new();

        if self.show_header {
            let columns = self.column_count();
            let model_names = self.model.as_ref();
            for col in 0..columns {
                let x = rect.x + col as i32 * col_w;
                if x >= rect.x + rect.width as i32 {
                    break;
                }
                let band = Rect::new(x, rect.y, self.column_width, self.row_height);
                context.fill_rect(band, surface.blend(&ink, 0.08));
                let label = self
                    .header_labels
                    .get(col)
                    .map(String::as_str)
                    .filter(|label| !label.is_empty())
                    .or_else(|| model_names.and_then(|model| model.column_name(col)));
                if let Some(label) = label {
                    context.draw_text_fitted_cow(
                        context.text_line(band, cell_font),
                        label,
                        cell_font,
                        ink,
                        HorizontalAlignment::Left,
                    );
                }
            }
        }

        let top = rect.y + self.header_offset();
        for row in self.scroll_row..self.visible_rows.len() {
            let y = top + (row - self.scroll_row) as i32 * row_h;
            if y >= rect.y + rect.height as i32 {
                break;
            }

            if self.selected_row == Some(row) {
                context.fill_rect(Rect::new(rect.x, y, rect.width, self.row_height), selected_bg);
            } else if self.hovered_row == Some(row) {
                // A weaker wash than the selection, from the same accent: a row the pointer is over
                // is a *preview* of the row a click would select.
                context.fill_rect(
                    Rect::new(rect.x, y, rect.width, self.row_height),
                    surface.blend(&accent, 0.12),
                );
            }

            for col in 0..columns {
                let x = rect.x + col as i32 * col_w;
                if x >= rect.x + rect.width as i32 {
                    break;
                }

                context.draw_rect(Rect::new(x, y, self.column_width, self.row_height), cell_border);

                // One buffer and one font for the whole grid: this used to build a `String` per
                // visible cell (via `item`) and resolve the font twice per cell.
                cell_text.clear();
                if self.fill_item(row, col, &mut cell_text) {
                    let indent = if col == 0 {
                        self.row_depth(row).unwrap_or(0) as i32 * self.indent as i32
                    } else {
                        0
                    };
                    // The indented cell is the band: `y + row_h / 2` as a `draw_text` origin put
                    // the glyph box's *top* edge on the row's middle line, drawing every label
                    // half a line low. The band starts after the indent, so the tree's hierarchy
                    // still reads while the line box is derived correctly. Fitting keeps a deep
                    // node's value inside its column.
                    let text_x = x + 4 + indent;
                    let band =
                        Rect::new(text_x, y, (x + col_w - text_x).max(0) as u32, self.row_height);
                    context.draw_text_fitted_cow(
                        context.text_line(band, cell_font),
                        &cell_text,
                        cell_font,
                        ink,
                        HorizontalAlignment::Left,
                    );
                }
            }
        }

        // The loading placeholder occupies the row immediately below the branch being fetched,
        // which is where its first child will appear. Without it an expanded lazy branch is
        // indistinguishable from an empty one until the host's fetch completes.
        if self.loading_indicator_visible {
            if let Some(path) = self.loading_paths.first() {
                let slot = match self.visible_rows.iter().position(|row| &row.path == path) {
                    Some(row) if row >= self.scroll_row => (row - self.scroll_row + 1) as i32,
                    Some(_) => 0,
                    None => self.visible_rows.len().saturating_sub(self.scroll_row) as i32,
                };
                let y = top + slot * row_h;
                if y + row_h <= rect.y + rect.height as i32 {
                    let band = Rect::new(rect.x, y, rect.width, self.row_height);
                    context.fill_rect(band, surface.blend(&accent, 0.06));
                    context.draw_rect(band, cell_border);
                    let indent = path.len() as i32 * self.indent as i32;
                    let text_x = rect.x + 4 + indent;
                    let text_band = Rect::new(
                        text_x,
                        y,
                        (rect.right() - text_x).max(0) as u32,
                        self.row_height,
                    );
                    context.draw_text_fitted_cow(
                        context.text_line(text_band, cell_font),
                        "Loading...",
                        cell_font,
                        ink.with_alpha_f32(0.65),
                        HorizontalAlignment::Left,
                    );
                }
            }
        }
    }
}

impl crate::event::EventHandler for TreeTable {
    fn handle_event(&mut self, event: &Event) {
        // The base keeps the control-level facts (`hovered`, `pressed`, `focus_reason`) and its
        // `MouseEnter`/`MouseLeave` arms are what make `widget_state()` answer `Hover` here. This
        // handler did not forward to it at all, so the theme's `"tree_table:hover"` override could
        // never fire — the same defect `list_view` records, in the sibling that never inherited it.
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button, .. } if *button == 1 => {
                if let Some(column) = self.header_column_at(*pos) {
                    self.toggle_sort_column(column);
                    return;
                }
                if let Some(row) = self.row_at(pos.y) {
                    let _ = self.select_row(row);
                }
            }
            Event::ContextMenu { pos } => self.context_menu_requested.emit(*pos),
            Event::MousePress { pos, button, .. } if *button == 2 => {
                if self.middle_click_expands {
                    if let Some(row) = self.row_at(pos.y) {
                        let _ = self.toggle_row_expanded(row);
                    }
                }
            }
            Event::MouseDoubleClick { pos, button: 1 } => {
                if let Some(row) = self.row_at(pos.y) {
                    let _ = self.select_row(row);
                    self.row_activated.emit(row);
                    if self.expands_on_double_click && self.is_row_expanded(row).is_some() {
                        let _ = self.toggle_row_expanded(row);
                    }
                }
            }
            // Row hover, derived from the same `row_at` the clicks use, so the row that is
            // highlighted is the row a click would affect.
            Event::MouseMove { pos } => {
                let hovered = self.row_at(pos.y);
                if hovered != self.hovered_row {
                    self.hovered_row = hovered;
                    self.base.request_redraw();
                }
            }
            // The leave is tested in the arm's **pattern**: `take()` both clears the row and tells
            // whether there was one to clear, so "leaving a highlighted row" is one condition.
            Event::MouseLeave { .. } if self.hovered_row.take().is_some() => {
                self.base.request_redraw();
            }
            // A leave with nothing highlighted still runs `take()`, which is what the bare arm is
            // for: it must not fall through to the catch-all and leave a stale row set.
            Event::MouseLeave { .. } => {}
            Event::Wheel { delta, modifiers } => {
                let scroll = crate::event::WheelScroll::from_event(*delta, *modifiers, 1);
                let next = self.scroll_row as isize - scroll.lines_y as isize;
                self.set_scroll_row(next.max(0) as usize);
            }
            Event::KeyPress { key, .. } if self.base.focus_reason().is_some() => {
                let page = (self.base.geometry().height / self.row_height.max(1)) as usize;
                let last = self.visible_rows.len().saturating_sub(1);
                let current = self.selected_row.unwrap_or(0);
                let next = match *key {
                    crate::event::key_codes::UP => current.saturating_sub(1),
                    crate::event::key_codes::DOWN => current.saturating_add(1).min(last),
                    crate::event::key_codes::HOME => 0,
                    crate::event::key_codes::END => last,
                    crate::event::key_codes::PAGE_UP => current.saturating_sub(page.max(1)),
                    crate::event::key_codes::PAGE_DOWN => {
                        current.saturating_add(page.max(1)).min(last)
                    }
                    crate::event::key_codes::ENTER | crate::event::key_codes::SPACE => {
                        if self.selected_row.is_some() {
                            let _ = self.toggle_row_expanded(current);
                        }
                        current
                    }
                    _ => current,
                };
                if self.select_row(next) {
                    if next < self.scroll_row {
                        self.set_scroll_row(next);
                    } else if next >= self.scroll_row + page.max(1) {
                        self.set_scroll_row(next + 1 - page.max(1));
                    }
                }
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct SampleTreeTableModel;

    impl TreeTableModel for SampleTreeTableModel {
        fn root_count(&self) -> usize {
            2
        }

        fn child_count(&self, path: &[usize]) -> usize {
            match path {
                [0] => 2,
                [1] => 1,
                [0, 0] => 1,
                _ => 0,
            }
        }

        fn column_count(&self) -> usize {
            2
        }

        fn data(&self, path: &[usize], column: usize) -> Option<String> {
            let id = path.iter().map(|part| part.to_string()).collect::<Vec<_>>().join("/");
            Some(format!("{}:{}", id, column))
        }
    }

    #[test]
    fn out_of_range_row_is_reported_as_out_of_range_not_unsupported() {
        // Same misreport as `list_view`/`tree_view`: the empty-table case is already
        // accepted, so the only remaining `false` is a row past the last visible one —
        // an argument fault, not a capability gap.
        use crate::widget::capability::types::CapabilityAccessError;

        let mut table = TreeTable::new(Rect::new(0, 0, 200, 120));
        table.set_model(std::sync::Arc::new(SampleTreeTableModel));
        assert!(table.row_count() > 0, "the fixture must have rows to be out of range of");

        assert_eq!(
            table.set("selected_row", CapabilityValue::UInt(9999)),
            Err(CapabilityAccessError::OutOfRange),
            "a row past the last visible one is the caller's argument"
        );
        // A valid row still works, which is what separates the two errors.
        assert_eq!(table.set("selected_row", CapabilityValue::UInt(0)), Ok(()));
        assert_eq!(table.selected_row(), Some(0));
    }

    #[test]
    fn projection_expands_and_collapses_hierarchical_rows() {
        let mut tree_table = TreeTable::new(Rect::new(0, 0, 320, 120));
        tree_table.set_model(Arc::new(SampleTreeTableModel));

        assert_eq!(tree_table.row_count(), 2);
        assert_eq!(tree_table.row_path(0), Some([0].as_slice()));
        assert_eq!(tree_table.row_path(1), Some([1].as_slice()));

        assert!(tree_table.expand_row(0));
        assert_eq!(tree_table.row_count(), 4);
        assert_eq!(tree_table.row_path(1), Some([0, 0].as_slice()));
        assert_eq!(tree_table.row_depth(1), Some(1));

        assert!(tree_table.expand_row(1));
        assert_eq!(tree_table.row_count(), 5);
        assert_eq!(tree_table.row_path(2), Some([0, 0, 0].as_slice()));

        assert!(tree_table.collapse_row(0));
        assert_eq!(tree_table.row_count(), 2);
    }

    /// Collapsing the row that holds the selection must announce the cleared selection.
    ///
    /// # The defect this pins (BLUE-issue E-20)
    ///
    /// Collapsing a row rebuilds the visible projection and drops a now-invisible `selected_row`,
    /// but only `projection_changed` was emitted. A subscriber saw `selected_row` go from `Some` to
    /// `None` with **zero** `selection_changed` notifications \u2014 inconsistent with `clear_model`,
    /// which does announce it. This pins that the clear is reported with the `Null` payload an absent
    /// optional selection uses.
    #[test]
    fn collapsing_the_selected_row_announces_the_cleared_selection() {
        // A single root with one child, so collapsing the root shrinks the projection to just the
        // root and the previously-selected child (row 1) is no longer a valid index \u2014 the exact
        // shape the issue reproduced.
        struct OneBranch;
        impl TreeTableModel for OneBranch {
            fn root_count(&self) -> usize {
                1
            }
            fn child_count(&self, path: &[usize]) -> usize {
                match path {
                    [0] => 1,
                    _ => 0,
                }
            }
            fn column_count(&self) -> usize {
                1
            }
            fn data(&self, path: &[usize], _column: usize) -> Option<String> {
                Some(format!("{path:?}"))
            }
        }

        let mut tree = TreeTable::new(Rect::new(0, 0, 320, 120));
        tree.set_model(Arc::new(OneBranch));

        // Expand the root and select its child (visible row 1).
        assert!(tree.expand_row(0));
        assert!(tree.select_row(1));
        assert_eq!(tree.selected_row(), Some(1));

        let seen = Arc::new(std::sync::Mutex::new(Vec::<Option<usize>>::new()));
        let sink = Arc::clone(&seen);
        tree.selection_changed.connect(move |value: Arc<Option<usize>>| {
            sink.lock().expect("lock").push(*value);
        });

        // Collapsing the root hides the selected child, so the selection must clear **and** be reported.
        assert!(tree.collapse_row(0));
        assert_eq!(tree.selected_row(), None, "the hidden selection must be cleared");
        assert_eq!(
            *seen.lock().expect("lock"),
            vec![None],
            "clearing the selection on collapse must emit exactly one `selection_changed(None)`"
        );
    }

    /// Replacing the model must announce the cleared selection, not drop it silently.
    #[test]
    fn replacing_the_model_announces_the_cleared_selection() {
        let mut tree = TreeTable::new(Rect::new(0, 0, 320, 120));
        tree.set_model(Arc::new(SampleTreeTableModel));
        assert!(tree.select_row(1));

        let seen = Arc::new(std::sync::Mutex::new(Vec::<Option<usize>>::new()));
        let sink = Arc::clone(&seen);
        tree.selection_changed.connect(move |value: Arc<Option<usize>>| {
            sink.lock().expect("lock").push(*value);
        });

        tree.set_model(Arc::new(SampleTreeTableModel));
        assert_eq!(tree.selected_row(), None);
        assert_eq!(
            *seen.lock().expect("lock"),
            vec![None],
            "a model swap that clears the selection must announce it"
        );
    }

    #[test]
    fn row_selection_and_item_lookup_follow_visible_projection() {
        let mut tree_table = TreeTable::new(Rect::new(0, 0, 320, 120));
        tree_table.set_model(Arc::new(SampleTreeTableModel));

        assert!(tree_table.select_row(1));
        assert_eq!(tree_table.selected_row(), Some(1));
        assert_eq!(tree_table.item(1, 0), Some("1:0".to_string()));

        assert!(tree_table.expand_row(0));
        assert_eq!(tree_table.item(1, 0), Some("0/0:0".to_string()));
        assert!(!tree_table.select_row(99));
    }

    #[test]
    fn clear_model_resets_projection_and_selection() {
        let mut tree_table = TreeTable::new(Rect::new(0, 0, 320, 120));
        tree_table.set_model(Arc::new(SampleTreeTableModel));
        assert!(tree_table.select_row(0));

        tree_table.clear_model();
        assert_eq!(tree_table.row_count(), 0);
        assert_eq!(tree_table.selected_row(), None);
        assert!(!tree_table.has_model());
    }

    #[test]
    fn new_creates_default_state() {
        let tree = TreeTable::new(Rect::new(0, 0, 800, 600));
        assert!(!tree.has_model());
        assert_eq!(tree.row_count(), 0);
        assert_eq!(tree.column_count(), 0);
        assert_eq!(tree.selected_row(), None);
        assert_eq!(tree.row_height(), 20);
        assert_eq!(tree.column_width(), 140);
        assert!(tree.row_path(0).is_none());
        assert!(tree.row_depth(0).is_none());
        assert!(tree.is_row_expanded(0).is_none());
    }

    #[test]
    fn has_model_before_and_after() {
        let mut tree = TreeTable::new(Rect::new(0, 0, 800, 600));
        assert!(!tree.has_model());

        tree.set_model(Arc::new(SampleTreeTableModel));
        assert!(tree.has_model());

        tree.clear_model();
        assert!(!tree.has_model());
    }

    #[test]
    fn set_model_resets_expansion_and_selection() {
        let mut tree = TreeTable::new(Rect::new(0, 0, 320, 120));
        tree.set_model(Arc::new(SampleTreeTableModel));
        tree.expand_row(0);
        tree.select_row(2);
        assert_eq!(tree.row_count(), 4);

        // Re-binding should reset
        tree.set_model(Arc::new(SampleTreeTableModel));
        assert_eq!(tree.row_count(), 2);
        assert_eq!(tree.selected_row(), None);
    }

    #[test]
    fn toggle_row_expanded_works() {
        let mut tree = TreeTable::new(Rect::new(0, 0, 320, 120));
        tree.set_model(Arc::new(SampleTreeTableModel));

        // Toggle expands
        assert!(tree.toggle_row_expanded(0));
        assert!(tree.is_row_expanded(0) == Some(true));
        assert_eq!(tree.row_count(), 4);

        // Toggle collapses
        assert!(tree.toggle_row_expanded(0));
        assert!(tree.is_row_expanded(0) == Some(false));
        assert_eq!(tree.row_count(), 2);

        // Toggle on leaf node (no children)
        tree.expand_row(0);
        tree.expand_row(1); // path [0,0] has 1 child
        assert_eq!(tree.row_count(), 5);
        // Row with path [0,0,0] has no children
        assert!(!tree.toggle_row_expanded(2));
    }

    #[test]
    fn row_depth_returns_correct_values() {
        let mut tree = TreeTable::new(Rect::new(0, 0, 320, 120));
        tree.set_model(Arc::new(SampleTreeTableModel));

        assert_eq!(tree.row_depth(0), Some(0));
        assert_eq!(tree.row_depth(1), Some(0));

        tree.expand_row(0);
        assert_eq!(tree.row_depth(0), Some(0));
        assert_eq!(tree.row_depth(1), Some(1)); // child of root 0
        assert_eq!(tree.row_depth(2), Some(1)); // child of root 0
        assert_eq!(tree.row_depth(3), Some(0)); // root 1
    }

    #[test]
    fn signal_emission_on_projection_and_selection() {
        let mut tree = TreeTable::new(Rect::new(0, 0, 320, 120));

        let proj_emitted = Arc::new(std::sync::Mutex::new(Vec::new()));
        let sel_emitted = Arc::new(std::sync::Mutex::new(Vec::new()));

        let p_sink = proj_emitted.clone();
        tree.projection_changed.connect(move |count| {
            p_sink.lock().unwrap().push(*count);
        });

        let s_sink = sel_emitted.clone();
        tree.selection_changed.connect(move |sel| {
            s_sink.lock().unwrap().push(*sel);
        });

        tree.set_model(Arc::new(SampleTreeTableModel));
        assert!(proj_emitted.lock().unwrap().contains(&2));

        tree.select_row(0);
        assert!(sel_emitted.lock().unwrap().contains(&Some(0)));

        tree.clear_model();
        assert!(proj_emitted.lock().unwrap().contains(&0));
    }

    #[test]
    fn invalid_row_indices_handled_gracefully() {
        let mut tree = TreeTable::new(Rect::new(0, 0, 320, 120));
        tree.set_model(Arc::new(SampleTreeTableModel));

        // Out-of-bounds row operations
        assert!(!tree.expand_row(999));
        assert!(!tree.collapse_row(999));
        assert!(!tree.select_row(999));
        assert_eq!(tree.row_path(999), None);
        assert_eq!(tree.row_depth(999), None);
        assert_eq!(tree.is_row_expanded(999), None);
        assert_eq!(tree.item(999, 0), None);
    }

    #[test]
    fn empty_model_handled_gracefully() {
        struct EmptyModel;
        impl TreeTableModel for EmptyModel {
            fn root_count(&self) -> usize {
                0
            }
            fn child_count(&self, _: &[usize]) -> usize {
                0
            }
            fn column_count(&self) -> usize {
                0
            }
            fn data(&self, _: &[usize], _: usize) -> Option<String> {
                None
            }
        }

        let mut tree = TreeTable::new(Rect::new(0, 0, 320, 120));
        tree.set_model(Arc::new(EmptyModel));

        assert_eq!(tree.row_count(), 0);
        assert_eq!(tree.column_count(), 0);
        assert_eq!(tree.selected_row(), None);
        assert!(!tree.select_row(0));
        assert!(!tree.expand_row(0));
        assert!(!tree.collapse_row(0));
    }

    #[test]
    fn item_lookup_on_expanded_rows() {
        let mut tree = TreeTable::new(Rect::new(0, 0, 320, 120));
        tree.set_model(Arc::new(SampleTreeTableModel));

        // Root child full path
        assert_eq!(tree.item(0, 0), Some("0:0".to_string()));
        assert_eq!(tree.item(1, 1), Some("1:1".to_string()));

        tree.expand_row(0);
        // Expanded rows use full path
        assert_eq!(tree.item(1, 0), Some("0/0:0".to_string()));
        assert_eq!(tree.item(2, 1), Some("0/1:1".to_string()));

        // Row out of bounds returns None (model handles column)
        assert_eq!(tree.item(999, 0), None);
    }

    #[test]
    fn set_row_and_column_width() {
        let mut tree = TreeTable::new(Rect::new(0, 0, 800, 600));

        tree.set_row_height(30);
        assert_eq!(tree.row_height(), 30);

        tree.set_row_height(0);
        assert_eq!(tree.row_height(), 1);

        tree.set_column_width(200);
        assert_eq!(tree.column_width(), 200);

        tree.set_column_width(0);
        assert_eq!(tree.column_width(), 1);
    }

    #[test]
    fn collapse_descendants_are_removed() {
        let mut tree = TreeTable::new(Rect::new(0, 0, 320, 120));
        tree.set_model(Arc::new(SampleTreeTableModel));

        // Expand root 0 -> shows children
        tree.expand_row(0);
        // Expand [0,0] -> shows [0,0,0]
        tree.expand_row(1);
        assert_eq!(tree.row_count(), 5);

        // Collapse root 0 -> all descendants removed
        tree.collapse_row(0);
        assert_eq!(tree.row_count(), 2);
        assert_eq!(tree.row_path(0), Some([0].as_slice()));
        assert_eq!(tree.row_path(1), Some([1].as_slice()));
    }

    /// The row under the pointer is highlighted, and its hover reaches the base.
    ///
    /// This handler never forwarded to the base, so `widget_state()` answered `Normal` throughout
    /// and the theme's `"tree_table:hover"` override could never fire — the same defect
    /// `list_view` records, in a sibling control that never inherited its fix. It also kept no row
    /// hover, so a hierarchical table gave no feedback about which row a click would act on.
    #[test]
    fn the_row_under_the_pointer_is_highlighted_and_reaches_the_base() {
        use crate::core::Point;
        use crate::event::{Event, EventHandler};
        use crate::style::WidgetState;

        let mut tree = TreeTable::new(Rect::new(0, 0, 320, 120));
        tree.set_model(Arc::new(SampleTreeTableModel));

        // The fixture's rows are 20 px tall, so y = 22 is inside the second row.
        let probe = Point::new(8, 22);
        let row = tree.row_at(probe.y).expect("the second row is under this point");
        assert_eq!(row, 1);

        tree.handle_event(&Event::MouseEnter { pos: probe });
        tree.handle_event(&Event::MouseMove { pos: probe });
        assert_eq!(tree.hovered_row, Some(row), "the row under the pointer is the one lit");
        assert_eq!(
            tree.widget_state(),
            WidgetState::Hover,
            "the base must have been told the pointer arrived"
        );

        let below = Point::new(8, 1000);
        assert_eq!(tree.row_at(below.y), None);
        tree.handle_event(&Event::MouseLeave { pos: below });
        assert_eq!(tree.hovered_row, None, "leaving clears the row rather than latching it");
        assert_eq!(tree.widget_state(), WidgetState::Normal);
    }

    /// Scroll, keyboard paging, the header row, indent and the expansion signal are wired.
    #[test]
    fn scrolling_header_keyboard_and_expansion_signal_are_wired() {
        use crate::event::EventHandler;
        let mut tree = TreeTable::new(Rect::new(0, 0, 300, 100));
        tree.set_model(Arc::new(SampleTreeTableModel));
        tree.set_row_height(24);
        tree.set_indent(20);
        assert_eq!(tree.indent(), 20);
        tree.set_show_header(true);
        assert!(tree.show_header());
        let expansion = std::sync::Arc::new(std::sync::Mutex::new(None));
        let sink = std::sync::Arc::clone(&expansion);
        tree.expansion_changed.connect(move |event| {
            if let Ok(mut slot) = sink.lock() {
                *slot = Some((*event).clone());
            }
        });
        assert!(tree.expand_row(0));
        assert!(
            expansion.lock().map(|slot| slot.is_some()).unwrap_or(false),
            "opening a row broadcasts its path"
        );

        tree.handle_event(&Event::Wheel { delta: crate::core::Point::new(0, -1), modifiers: 0 });
        assert!(tree.scroll_row() > 0 || tree.row_count() < 2);
        tree.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Tab });
        tree.handle_event(&Event::KeyPress { key: crate::event::key_codes::END, modifiers: 0 });
        assert!(tree.selected_row().is_some());
        let _ = crate::widget::svg::render_to_svg(&mut tree);
    }

    /// A windowed projection stops at the viewport and extends on demand.
    #[test]
    fn a_windowed_projection_stops_at_the_viewport_and_extends() {
        struct ManyRoots;
        impl TreeTableModel for ManyRoots {
            fn root_count(&self) -> usize {
                2_000
            }
            fn child_count(&self, _path: &[usize]) -> usize {
                0
            }
            fn column_count(&self) -> usize {
                1
            }
            fn data(&self, path: &[usize], _column: usize) -> Option<String> {
                Some(std::format!("{:04}", path[0]))
            }
        }

        let mut tree = TreeTable::new(Rect::new(0, 0, 300, 200));
        tree.set_row_height(24);
        tree.set_model(Arc::new(ManyRoots));
        // Windowed is the default; prove the eager projection still reports the whole tree.
        tree.set_windowed_projection(false);
        assert!(!tree.windowed_projection());
        assert_eq!(tree.row_count(), 2_000, "the eager projection is complete");

        tree.set_windowed_projection(true);
        assert!(tree.windowed_projection());
        assert!(tree.projection_truncated());
        let materialized = tree.row_count();
        assert!(materialized < 2_000, "only the window is materialized: {materialized}");

        tree.set_scroll_row(900);
        assert!(tree.row_count() > 900, "scrolling extends the projection");
        assert_eq!(tree.scroll_row(), 900);
        assert_eq!(tree.row_path(900).map(|path| path.to_vec()), Some(vec![900]));
    }

    /// Clicking a header sorts siblings and broadcasts the new direction.
    #[test]
    fn a_header_click_sorts_siblings_and_broadcasts() {
        use crate::event::EventHandler;
        let mut tree = TreeTable::new(Rect::new(0, 0, 300, 200));
        tree.set_rows(vec!["charlie".to_string(), "alpha".to_string(), "bravo".to_string()]);
        tree.set_show_header(true);
        let sorted = std::sync::Arc::new(std::sync::Mutex::new(None));
        let sink = std::sync::Arc::clone(&sorted);
        tree.sort_changed.connect(move |spec| {
            if let Ok(mut slot) = sink.lock() {
                *slot = Some(*spec);
            }
        });
        tree.handle_event(&Event::MousePress {
            pos: crate::core::Point::new(10, 5),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(tree.sort_column(), Some(0));
        assert_eq!(tree.item(0, 0), Some("alpha".to_string()));
        assert_eq!(tree.item(2, 0), Some("charlie".to_string()));
        assert_eq!(sorted.lock().map(|slot| *slot).unwrap_or(None), Some((0, false)));

        tree.handle_event(&Event::MousePress {
            pos: crate::core::Point::new(10, 5),
            button: 1,
            modifiers: 0,
        });
        assert!(tree.sort_descending());
        assert_eq!(tree.item(0, 0), Some("charlie".to_string()));

        let context = std::sync::Arc::new(std::sync::Mutex::new(None));
        let sink = std::sync::Arc::clone(&context);
        tree.context_menu_requested.connect(move |point| {
            if let Ok(mut slot) = sink.lock() {
                *slot = Some(*point);
            }
        });
        tree.handle_event(&Event::ContextMenu { pos: crate::core::Point::new(40, 60) });
        assert!(context.lock().map(|slot| slot.is_some()).unwrap_or(false));
    }

    /// A lazy model: child_count reports nothing until `fetch_more` loads the children. The
    /// model owns its storage behind a `Mutex`, which is the shape `&self` fetch hooks require.
    struct LazyTreeTableModel {
        loaded: std::sync::Mutex<bool>,
    }

    impl TreeTableModel for LazyTreeTableModel {
        fn root_count(&self) -> usize {
            1
        }

        fn child_count(&self, path: &[usize]) -> usize {
            match path {
                [0] if *self.loaded.lock().expect("lazy lock") => 2,
                _ => 0,
            }
        }

        fn column_count(&self) -> usize {
            1
        }

        fn data(&self, path: &[usize], _column: usize) -> Option<String> {
            Some(path.iter().map(|part| part.to_string()).collect::<Vec<_>>().join("/"))
        }

        fn can_fetch_more(&self, path: &[usize]) -> bool {
            path == [0] && !*self.loaded.lock().expect("lazy lock")
        }

        fn fetch_more(&self, path: &[usize]) -> bool {
            if path != [0] {
                return false;
            }
            let mut loaded = self.loaded.lock().expect("lazy lock");
            if *loaded {
                return false;
            }
            *loaded = true;
            true
        }
    }

    #[test]
    fn lazy_fetch_more_expands_signals_and_properties_are_wired() {
        use crate::widget::capability::types::CapabilityValue;
        use crate::widget::capability::WidgetProperties;

        let model =
            std::sync::Arc::new(LazyTreeTableModel { loaded: std::sync::Mutex::new(false) });
        let mut tree = TreeTable::new(Rect::new(0, 0, 320, 120));
        tree.set_model(model.clone());

        let started = std::sync::Arc::new(std::sync::Mutex::new(Vec::<Vec<usize>>::new()));
        let finished = std::sync::Arc::new(std::sync::Mutex::new(Vec::<(Vec<usize>, bool)>::new()));
        let started_sink = std::sync::Arc::clone(&started);
        let finished_sink = std::sync::Arc::clone(&finished);
        tree.fetch_started.connect(move |path| {
            if let Ok(mut seen) = started_sink.lock() {
                seen.push(path.as_ref().clone());
            }
        });
        tree.fetch_finished.connect(move |payload| {
            if let Ok(mut seen) = finished_sink.lock() {
                seen.push((*payload).clone());
            }
        });

        assert!(!tree.can_fetch_more(&[0]), "lazy loading is off by default");
        tree.set("lazy_loading", CapabilityValue::Bool(true)).expect("writable");
        assert!(tree.lazy_loading());
        assert!(tree.can_fetch_more(&[0]));

        assert_eq!(tree.row_count(), 1, "the root is the only known row");
        assert!(tree.expand_row(0), "a fetchable branch is expandable before its children exist");
        assert_eq!(tree.row_count(), 3, "the fetch revealed the two children");
        assert_eq!(*started.lock().expect("started lock"), vec![vec![0]]);
        assert_eq!(*finished.lock().expect("finished lock"), vec![(vec![0], true)]);
        assert!(!tree.loading_indicator_visible(), "a synchronous fetch settles immediately");
        assert!(tree.loading_paths().is_empty());

        assert_eq!(tree.fetch_timeout_ms(), 5000);
        tree.set("fetch_timeout_ms", CapabilityValue::UInt(1500)).expect("writable");
        assert_eq!(tree.fetch_timeout_ms(), 1500);
    }
}
