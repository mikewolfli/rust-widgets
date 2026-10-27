// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Tree view widget.
use crate::core::Color;
use crate::core::HorizontalAlignment;
use crate::core::Point;
use crate::core::Rect;
use crate::event::dnd::{DragPayload, DragSource, DropEffect, DropTarget};
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::{MotionSlot, PropertyDriver};
use crate::widget::capability::coercion::{expect_bool, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{effective_font, ControlMetrics};
use crate::widget::view_widgets::list_view::SelectionMode;
use crate::widget::view_widgets::table_widget::ItemDelegate;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// The margin a tree leaves between its own frame and its rows: 2 px on every edge.
///
/// Named once so every node is measured from the same inset. The nodes used to start at the
/// control's literal `rect.y`, which pinned the first node's text to y=0 on the frame's
/// stroke.
const TREE_INSET: u32 = 2;

/// A tree row's height: 20 px.
///
/// One fact for the three readers that place a row: the draw loop, the hit test and the hover test.
/// It was the literal `20` written in the draw loop *and* again in each press arm — two copies of
/// one number describing one band. This is the same defect `table_widget` records (it counted rows
/// from the content box while hit-testing from the control's top edge), and it is fixed the same
/// way: the offset lives in one function and all three readers call it.
const TREE_ROW_HEIGHT: i32 = 20;

/// One level of indentation, in pixels: 15.
///
/// The indent of a row at depth `d` is `(d + 1) * TREE_INDENT`, not `d * TREE_INDENT`: a root
/// node is one level in from the frame. That is the convention every tree control uses (it keeps
/// the first level off the border), and it is also what this control drew before it understood
/// depth at all — so a model that reports no hierarchy renders exactly as it always did.
const TREE_INDENT: i32 = 15;

/// The width of a disclosure glyph's box, in pixels.
const TREE_DISCLOSURE_SIZE: i32 = 6;

/// Half-open point-in-rect test, matching the convention the rest of the widget layer uses.
fn point_in_rect(point: Point, rect: Rect) -> bool {
    let right = rect.x + rect.width as i32;
    let bottom = rect.y + rect.height as i32;
    point.x >= rect.x && point.x < right && point.y >= rect.y && point.y < bottom
}

/// The disclosure glyph's box for a row, given the node's own depth and whether it is a branch.
///
/// A free function taking the two facts it needs, rather than a method that looks them up: the draw
/// loop already holds the row and the node, and a method spelling would re-derive the **visible
/// projection** per row — which turns one frame's O(nodes) walk into one walk per visible row.
fn disclosure_box(row: Rect, depth: usize, has_children: bool) -> Option<Rect> {
    if !has_children {
        return None;
    }
    let depth = i32::try_from(depth).ok()?;
    // The glyph sits in the row's own indent gutter, immediately left of its label.
    let gutter_x = row.x + TREE_INDENT.saturating_mul(depth.saturating_add(1));
    let size = TREE_DISCLOSURE_SIZE;
    let top = row.y + (TREE_ROW_HEIGHT - size) / 2;
    Some(Rect::new(
        gutter_x - TREE_INDENT + (TREE_INDENT - size) / 2,
        top,
        size as u32,
        size as u32,
    ))
}

/// Everything the view needs to know about one node.
///
/// # Why the model must be able to describe *shape*
///
/// [`TreeModel::node_count`] and [`TreeModel::node_path`] describe a **flat list**: one string
/// per row, with no parent/child relation and no expansion state. A tree drawn from that can only
/// be a list with a tree's chrome around it — every row the same indent, no disclosure glyph, no
/// way to fold a branch, and (because the view decides visibility but could not report it) a
/// "visible index" a host could not check. The sample data worked around it by putting the
/// hierarchy into the label as leading spaces, which is what a flat model forces you to do.
///
/// `NodeView` is that missing shape. The fields are exactly the four facts a renderer needs; the
/// view owns the *interaction* (which branches are open) and the model owns the *structure*
/// (which nodes exist, how deep, and whether they have children).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeView {
    /// The text drawn for the node.
    pub label: String,
    /// How deep the node sits; `0` is a root, `1` its child, and so on.
    pub depth: usize,
    /// Whether the node has children — i.e. whether it can be expanded or collapsed.
    ///
    /// A node with no children is a leaf, and its disclosure glyph is not drawn; the view uses
    /// this to reject [`TreeView::expand`] on a leaf instead of pretending the state changed.
    pub has_children: bool,
    /// The model's **initial** expansion state for the node.
    ///
    /// Read only until the user (or the host) changes it: [`TreeView::is_expanded`] prefers an
    /// explicit override and falls back to this. A model that rebuilds itself can therefore
    /// declare "branches start closed" without the view keeping any state of its own.
    pub expanded: bool,
}

impl NodeView {
    /// Builds a flat node: depth 0, no children, expanded.
    ///
    /// The shape a model that only knows labels produces, and the one
    /// [`TreeModel::node`]'s default derives from [`TreeModel::node_path`].
    pub fn leaf(label: impl Into<String>) -> Self {
        Self { label: label.into(), depth: 0, has_children: false, expanded: true }
    }
}

/// One column of a multi-column tree presentation (V-016).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeColumn {
    /// The column title.
    pub title: String,
    /// Width in logical pixels.
    pub width: u32,
}

impl TreeColumn {
    /// A column with a title and a width of at least one pixel.
    pub fn new(title: impl Into<String>, width: u32) -> Self {
        Self { title: title.into(), width: width.max(1) }
    }
}

/// Tree model abstraction for tree-like views.
pub trait TreeModel: Send + Sync {
    /// Number of nodes exposed by model.
    ///
    /// This is the count of **all** nodes, including those under a collapsed parent: the model
    /// describes the tree, and the view decides which rows are shown. Use
    /// [`TreeView::visible_indices`] to recover the mapping from rows to model nodes.
    fn node_count(&self) -> usize;
    /// Node path by model index, if present.
    fn node_path(&self, index: usize) -> Option<String>;
    /// Writes the node's label into `out`, returning whether the node exists.
    ///
    /// The zero-allocation counterpart of [`Self::node_path`]; the default delegates to it so
    /// existing implementors keep working unchanged.
    fn fill_node_path(&self, index: usize, out: &mut String) -> bool {
        match self.node_path(index) {
            Some(value) => {
                out.push_str(&value);
                true
            }
            None => false,
        }
    }
    /// The shape of node `index`: its label, depth, whether it has children, and its initial
    /// expansion state.
    ///
    /// # Why this has a default
    ///
    /// The default answers a **flat** node derived from [`TreeModel::node_path`] — depth 0, no
    /// children, expanded — which is exactly the model this trait used to describe. A model that
    /// only knows labels therefore compiles and renders unchanged, and a model that *does* know
    /// its structure overrides this one method. There is deliberately no second abstract
    /// "flat" trait: one trait, one default, and one behaviour (principle #101).
    fn node(&self, index: usize) -> Option<NodeView> {
        self.node_path(index).map(NodeView::leaf)
    }
    /// Optional signal emitted when model data projection changes.
    fn data_changed_signal(&self) -> Option<&GenericSignal> {
        None
    }

    /// Whether the model can fetch more children below `index`.
    ///
    /// The flat `node_count`/`node` protocol describes what is already known; a lazy tree
    /// needs to say "there may be more here". The default answers `false`, so an eager model
    /// is unchanged.
    fn can_fetch_more(&self, index: usize) -> bool {
        let _ = index;
        false
    }

    /// Asks the model to fetch more children for `index`.
    ///
    /// Takes `&self` so a model can own its storage behind a `Mutex`; returns whether the
    /// node's child count changed. The default is a no-op `false`.
    fn fetch_more(&self, index: usize) -> bool {
        let _ = index;
        false
    }

    /// Path-addressed form of [`Self::can_fetch_more`].
    ///
    /// A [`TreeView`] model is a flat node list, so a path's meaningful component is its last
    /// element (the node index); a richer model can override this to interpret the full ancestor
    /// chain. The default keeps the two entry points one implementation.
    fn can_fetch_more_path(&self, path: &[usize]) -> bool {
        path.last().is_some_and(|index| self.can_fetch_more(*index))
    }

    /// Path-addressed form of [`Self::fetch_more`]; synchronous and host-driven, like its
    /// node-addressed counterpart.
    fn fetch_more_path(&self, path: &[usize]) -> bool {
        path.last().is_some_and(|index| self.fetch_more(*index))
    }

    /// The text painted in `column` of node `index` (V-016).
    ///
    /// Column `0` is the label column and defaults to [`Self::node_path`]; every other column
    /// defaults to `None`, so a model that does not describe columns keeps rendering exactly the
    /// historical single label. A multi-column model overrides this to supply the extra cells.
    fn node_column_text(&self, index: usize, column: usize) -> Option<String> {
        if column == 0 {
            self.node_path(index)
        } else {
            None
        }
    }
}
/// In-memory tree model backed by a vector of nodes.
pub struct VecTreeModel {
    nodes: Vec<NodeView>,
    data_changed: GenericSignal,
}
impl VecTreeModel {
    /// Creates a model of flat nodes from a list of labels.
    pub fn new(nodes: Vec<String>) -> Self {
        Self {
            nodes: nodes.into_iter().map(NodeView::leaf).collect(),
            data_changed: GenericSignal::new(),
        }
    }
    /// Creates a model from nodes that already carry their shape.
    pub fn from_views(nodes: Vec<NodeView>) -> Self {
        Self { nodes, data_changed: GenericSignal::new() }
    }
    /// Returns a reference to the data changed signal.
    pub fn data_changed_signal(&self) -> &GenericSignal {
        &self.data_changed
    }
    /// Appends a flat node to the model.
    pub fn append(&mut self, node: String) {
        self.nodes.push(NodeView::leaf(node));
        self.data_changed.emit();
    }
    /// Appends a node with an explicit shape.
    pub fn append_view(&mut self, node: NodeView) {
        self.nodes.push(node);
        self.data_changed.emit();
    }
    /// Removes a node at given index.
    pub fn remove(&mut self, index: usize) -> Option<String> {
        if index < self.nodes.len() {
            let node = self.nodes.remove(index);
            self.data_changed.emit();
            Some(node.label)
        } else {
            None
        }
    }
    /// Clears all nodes.
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.data_changed.emit();
    }
}
impl TreeModel for VecTreeModel {
    fn node_count(&self) -> usize {
        self.nodes.len()
    }
    fn node_path(&self, index: usize) -> Option<String> {
        self.nodes.get(index).map(|node| node.label.clone())
    }
    /// The model already owns its labels, so drawing can borrow them instead of cloning.
    fn fill_node_path(&self, index: usize, out: &mut String) -> bool {
        match self.nodes.get(index) {
            Some(node) => {
                out.push_str(&node.label);
                true
            }
            None => false,
        }
    }
    fn node(&self, index: usize) -> Option<NodeView> {
        self.nodes.get(index).cloned()
    }
    fn data_changed_signal(&self) -> Option<&GenericSignal> {
        Some(&self.data_changed)
    }
}
/// Tree view widget with optional external model binding.
///
/// # Index spaces
///
/// Two different indices name a node, and the distinction is the whole reason expansion could not
/// be added to a flat model:
///
/// * a **model index** addresses a node in [`TreeModel`], and never moves while the model is
///   unchanged;
/// * a **visible index** addresses a *row on screen*, and moves whenever a branch above it is
///   expanded or collapsed.
///
/// The widget keeps its own state (selection, focus, hover, the open edit) in **model indices**,
/// so collapsing a branch cannot silently re-point the selection at a different node. The public
/// accessors and the published `selection_changed` / `focused_node_changed` payloads speak
/// **visible indices**, because that is the space a host's own hit testing works in — and
/// [`TreeView::visible_indices`] is the published mapping between the two, so a host can check
/// what a visible index means instead of taking it on trust.
pub struct TreeView {
    base: BaseWidget,
    /// Optional bound tree model.
    model: Option<Arc<dyn TreeModel>>,
    /// Scoped model-to-view signal subscriptions.
    model_connection_scope: ConnectionScope,
    /// Selected node, as a **model** index.
    selected_node: Option<usize>,
    /// Every selected node, in **model** indices (V-10).
    ///
    /// `selected_node` stays the current node; the set makes `Multi`/`Extended` expressible
    /// without changing the single-value accessor or the published `selection_changed` payload.
    selected_nodes: std::collections::BTreeSet<usize>,
    /// How clicks and keyboard selection combine (V-10).
    selection_mode: SelectionMode,
    /// The node a `Shift`-extend grows from, in model indices.
    selection_anchor: Option<usize>,
    /// Emitted with every selected node whenever the set changes (V-10).
    pub selection_set_changed: Signal1<Vec<usize>>,
    /// Whether expanding a branch scrolls the expanded row into view (V-11).
    auto_scroll_to_expanded: bool,
    /// Focused node, as a **model** index.
    focused_node: Option<usize>,
    /// The row the pointer is currently over, as a **model** index; `None` when the pointer is
    /// between/outside the rows.
    ///
    /// Filled by `MouseMove` through the same [`Self::node_at`] the click uses, so the highlighted
    /// row is the row a click would affect.
    hovered_node: Option<usize>,
    /// Expansion state the host or user has set, keyed by **model** index.
    ///
    /// A model index is the key because it is the node identity this widget has; the alternative
    /// (keying by visible index) would make "which branch is open" change meaning every time an
    /// unrelated branch is toggled. Entries here take precedence over [`NodeView::expanded`];
    /// absent means "use the model's answer".
    expanded_override: HashMap<usize, bool>,
    /// Optional display/editor delegate, shared with [`TableWidget`](super::table_widget::TableWidget).
    ///
    /// Driven by [`TreeView::begin_edit`] / [`TreeView::commit_edit`]; see those for why the
    /// three-method trait is an operation rather than a rendering hook.
    delegate: Option<Arc<dyn ItemDelegate>>,
    /// The node an in-place edit is currently open on, as a **model** index.
    editing: Option<usize>,
    /// Emitted when selected node changes.
    pub selection_changed: Signal1<usize>,
    /// Emitted when focused node changed.
    pub focused_node_changed: Signal1<Option<usize>>,
    /// Emitted with the model index when a branch opens.
    pub expanded: Signal1<usize>,
    /// Emitted with the model index when a branch closes.
    pub collapsed: Signal1<usize>,
    /// Emitted when the user activates a node (Enter or double-click).
    pub activated: Signal1<usize>,
    /// Emitted when a node is double-clicked.
    pub node_double_clicked: Signal1<usize>,
    /// First visible row; wheel and keyboard scrolling move it.
    scroll_row: usize,
    /// Whether the source-list (sidebar) presentation is used.
    source_list: bool,
    /// Whether an expansion fades its newly revealed rows in.
    animated: bool,
    /// The fade applied to rows revealed by the most recent expansion.
    expansion_progress: PropertyDriver,
    /// The first visible row the fade applies to.
    animation_start: Option<usize>,
    /// How many rows the fade covers.
    animation_len: usize,
    /// Whether a vertical scroll-bar thumb is painted.
    show_scrollbar: bool,
    /// The visible projection of the current expansion state.
    ///
    /// A state change used to materialise the projection three or four times (once to resolve the
    /// row, once for the before-count, once for the after-count and once more on the next access).
    /// One state change now rebuilds this cache once and every reader slices it.
    projection_cache: Vec<usize>,
    /// Whether `projection_cache` still describes the model and the expansion state.
    projection_valid: bool,
    /// The data generation `projection_cache` was built from.
    projection_generation: u64,
    /// The inverse of `projection_cache`: model index -> visible row (F-P10).
    ///
    /// Built alongside the projection (same O(nodes) pass) so keyboard navigation can locate the
    /// focused node in O(1) instead of materialising and scanning the whole projection per key.
    projection_positions: HashMap<usize, usize>,
    /// Advanced by the model's `data_changed` signal, which a `&self` projection reader cannot
    /// observe by rebuilding. A mismatch makes the reader fall back to a bounded walk.
    data_generation: Arc<AtomicU64>,
    /// Whether expanding a branch asks the model for more children.
    lazy_loading: bool,
    /// Whether the loading placeholder row is painted.
    loading_indicator_visible: bool,
    /// Advisory fetch ceiling for the host's loader; see the field docs on `TreeTable`.
    fetch_timeout_ms: u32,
    /// Paths whose children are currently being fetched.
    loading_paths: Vec<Vec<usize>>,
    /// Emitted when [`TreeView::fetch_more`] asks the model for `path`.
    pub fetch_started: Signal1<Vec<usize>>,
    /// Emitted when a fetch for `path` finishes; the `bool` is whether the model grew.
    pub fetch_finished: Signal1<(Vec<usize>, bool)>,
    /// Whether vertical indent guide lines are painted (V-016). Off by default.
    indent_lines: bool,
    /// Column descriptors for the multi-column presentation (V-016). Empty means the historical
    /// single label.
    columns: Vec<TreeColumn>,
    /// Emitted with `(from_visible_row, to_visible_row)` when a row drag is dropped (V-016).
    ///
    /// The model is host-owned and read-only, so the drag cannot commit the move itself; the
    /// request is the contract, and the host applies it to the model it owns.
    pub row_move_requested: Signal1<(usize, usize)>,
    /// Emitted with the effect of a drag that began in this tree (V-016).
    pub drag_finished: Signal1<DropEffect>,
}
impl TreeView {
    /// Creates an empty tree view.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::TreeView, geometry, "TreeView"),
            model: None,
            model_connection_scope: ConnectionScope::new(),
            selected_node: None,
            selected_nodes: std::collections::BTreeSet::new(),
            selection_mode: SelectionMode::Single,
            selection_anchor: None,
            selection_set_changed: Signal1::new(),
            auto_scroll_to_expanded: true,
            focused_node: None,
            hovered_node: None,
            expanded_override: HashMap::new(),
            delegate: None,
            editing: None,
            selection_changed: Signal1::new(),
            focused_node_changed: Signal1::new(),
            expanded: Signal1::new(),
            collapsed: Signal1::new(),
            activated: Signal1::new(),
            node_double_clicked: Signal1::new(),
            scroll_row: 0,
            source_list: false,
            animated: false,
            expansion_progress: PropertyDriver::at(1.0, MotionSlot::Normal),
            animation_start: None,
            animation_len: 0,
            show_scrollbar: false,
            projection_cache: Vec::new(),
            projection_positions: HashMap::new(),
            projection_valid: false,
            projection_generation: 0,
            data_generation: Arc::new(AtomicU64::new(0)),
            lazy_loading: false,
            loading_indicator_visible: false,
            fetch_timeout_ms: 5000,
            loading_paths: Vec::new(),
            fetch_started: Signal1::new(),
            fetch_finished: Signal1::new(),
            indent_lines: false,
            columns: Vec::new(),
            row_move_requested: Signal1::new(),
            drag_finished: Signal1::new(),
        }
    }
    /// Binds an external tree model.
    ///
    /// Any expansion state the *previous* model owned is dropped: the overrides are keyed by model
    /// index, and a different model's indices name different nodes.
    pub fn set_model(&mut self, model: Arc<dyn TreeModel>) {
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
        self.expanded_override.clear();
        self.editing = None;
        self.normalize_projection_state();
        self.data_generation.fetch_add(1, Ordering::Relaxed);
        self.rebuild_projection();
        self.base.request_layout();
        self.base.request_redraw();
    }
    /// Returns whether a model is currently bound.
    pub fn has_model(&self) -> bool {
        self.model.is_some()
    }
    /// Returns the bound tree model, if present.
    pub fn model_ref(&self) -> Option<&Arc<dyn TreeModel>> {
        self.model.as_ref()
    }

    /// The **model** indices of the rows currently shown, in display order.
    ///
    /// # What "visible" means, and why this is public
    ///
    /// A node is shown only when every one of its ancestors is expanded. That rule is applied
    /// here, once, and every row index the widget publishes — [`Self::node_count`],
    /// [`Self::node_row_rect`], [`Self::node_at`], [`Self::select_node`], and the payloads of
    /// `selection_changed` / `focused_node_changed` — is an index into **this** list. Exposing the
    /// mapping is what makes a visible index checkable by the host: before it existed, the widget
    /// decided visibility internally and a host holding a "visible index" from
    /// [`Self::node_at`] had no way to find out which model node it named.
    ///
    /// The walk is a single pass over the model with a stack of the open ancestors, so it is
    /// O(nodes) and allocates one `Vec`.
    pub fn visible_indices(&self) -> Vec<usize> {
        self.visible_indices_limited(usize::MAX)
    }

    /// The visible projection, stopped after `limit` rows.
    ///
    /// The draw loop only needs the rows its content box can hold, and the pointer paths only
    /// need one row. Walking the whole model for either made the cost follow the document
    /// rather than the viewport. A current cache is sliced instead of walked.
    pub fn visible_indices_limited(&self, limit: usize) -> Vec<usize> {
        if limit == 0 {
            return Vec::new();
        }
        if self.projection_is_current() {
            return self.projection_cache.iter().take(limit).copied().collect();
        }
        self.walk_visible_indices(limit)
    }

    /// The visible rows from `start`, capped at `limit`.
    ///
    /// The paint loop used to ask for `0..scroll_row + capacity` and then `skip(scroll_row)`, so a
    /// deep scroll copied every row above the viewport into a fresh `Vec` each frame. This cursor
    /// starts at the first visible row: a current cache is sliced from `start`, and an uncached
    /// walk stops after `start + limit` rows instead of materialising the prefix (V-010).
    fn visible_indices_from(&self, start: usize, limit: usize) -> Vec<usize> {
        if limit == 0 {
            return Vec::new();
        }
        if self.projection_is_current() {
            return self.projection_cache.iter().skip(start).take(limit).copied().collect();
        }
        self.walk_visible_indices(start.saturating_add(limit)).into_iter().skip(start).collect()
    }

    /// Whether the cached projection still describes the current model and expansion state.
    fn projection_is_current(&self) -> bool {
        self.projection_valid
            && self.projection_generation == self.data_generation.load(Ordering::Relaxed)
    }

    /// Rebuilds the cached projection exactly once.
    fn rebuild_projection(&mut self) {
        self.projection_generation = self.data_generation.load(Ordering::Relaxed);
        self.projection_cache = self.walk_visible_indices(usize::MAX);
        self.projection_positions.clear();
        for (row, model_index) in self.projection_cache.iter().enumerate() {
            self.projection_positions.insert(*model_index, row);
        }
        self.projection_valid = true;
    }

    /// The one walk over the model, applying the ancestor-expansion rule.
    fn walk_visible_indices(&self, limit: usize) -> Vec<usize> {
        let Some(model) = self.model.as_ref() else {
            return Vec::new();
        };
        if limit == 0 {
            return Vec::new();
        }
        let count = model.node_count();
        let mut rows = Vec::with_capacity(count.min(limit));
        // `(depth, expanded)` of the open-able ancestors of the node being considered. The stack
        // holds only nodes that *have* children; a leaf can never hide anything.
        let mut ancestors: Vec<(usize, bool)> = Vec::new();
        for index in 0..count {
            let Some(node) = model.node(index) else { continue };
            // Everything at or below the node's own depth is a sibling or a previous subtree, so
            // it is no longer an ancestor.
            while ancestors.last().is_some_and(|(depth, _)| *depth >= node.depth) {
                ancestors.pop();
            }
            if ancestors.iter().all(|(_, expanded)| *expanded) {
                rows.push(index);
                if rows.len() >= limit {
                    break;
                }
            }
            if node.has_children {
                ancestors.push((node.depth, self.is_expanded(index)));
            }
        }
        rows
    }

    /// The **model** index of visible row `target`, or `None` when the projection is shorter.
    ///
    /// Single-row lookup that stops as soon as the row is found; the pointer paths used to
    /// materialise the whole projection to read one entry from it. A current cache makes it an
    /// index.
    /// The **model** index of visible row `target`, or `None` when the projection is shorter.
    ///
    /// Single-row lookup that stops as soon as the row is found; the pointer paths used to
    /// materialise the whole projection to read one entry from it. A current cache makes it an
    /// index.
    fn visible_index_at(&self, target: usize) -> Option<usize> {
        if self.projection_is_current() {
            return self.projection_cache.get(target).copied();
        }
        let model = self.model.as_ref()?;
        let count = model.node_count();
        let mut shown = 0usize;
        let mut ancestors: Vec<(usize, bool)> = Vec::new();
        for index in 0..count {
            let Some(node) = model.node(index) else { continue };
            while ancestors.last().is_some_and(|(depth, _)| *depth >= node.depth) {
                ancestors.pop();
            }
            if ancestors.iter().all(|(_, expanded)| *expanded) {
                if shown == target {
                    return Some(index);
                }
                shown += 1;
            }
            if node.has_children {
                ancestors.push((node.depth, self.is_expanded(index)));
            }
        }
        None
    }

    /// Whether the node at **model** index `index` is expanded.
    ///
    /// Prefers an explicit [`Self::expand`] / [`Self::collapse`], then the model's own
    /// [`NodeView::expanded`]. An index the model does not have answers `false`.
    pub fn is_expanded(&self, index: usize) -> bool {
        if let Some(value) = self.expanded_override.get(&index) {
            return *value;
        }
        self.model
            .as_ref()
            .and_then(|model| model.node(index))
            .map(|node| node.expanded)
            .unwrap_or(false)
    }

    /// Expands the node at **visible** index `index`.
    ///
    /// Returns `false` when `index` is not a visible row, or names a leaf — a node with nothing to
    /// show cannot be expanded, and reporting success for that would be the "success-shaped answer
    /// for something that did not happen" this project forbids.
    pub fn expand(&mut self, index: usize) -> bool {
        self.set_expanded(index, true)
    }

    /// Collapses the node at **visible** index `index`.
    ///
    /// See [`Self::expand`] for the return contract.
    pub fn collapse(&mut self, index: usize) -> bool {
        self.set_expanded(index, false)
    }

    /// Toggles the node at **visible** index `index`.
    ///
    /// The gesture a disclosure glyph carries: one entry point so a click and a keyboard action
    /// cannot disagree about what "toggle" means. The projection is resolved once here rather than
    /// once in the toggle and again in the setter.
    pub fn toggle_expanded(&mut self, index: usize) -> bool {
        self.ensure_projection();
        let Some(model_index) = self.projection_cache.get(index).copied() else {
            return false;
        };
        let expanded = !self.is_expanded(model_index);
        self.apply_expansion(index, model_index, expanded)
    }

    /// Expands the node at **visible** index `index`.
    ///
    /// Returns `false` when `index` is not a visible row, or names a leaf.
    pub fn set_expanded(&mut self, visible_index: usize, expanded: bool) -> bool {
        self.ensure_projection();
        let Some(model_index) = self.projection_cache.get(visible_index).copied() else {
            return false;
        };
        self.apply_expansion(visible_index, model_index, expanded)
    }

    /// Expands the node at **model** index `model_index`.
    ///
    /// A model index is stable across the expansion changes that move visible rows, so this is
    /// the form a host holding a node identity should use. A node hidden under a collapsed
    /// ancestor can still be marked expanded; nothing is painted for it until it is revealed.
    pub fn set_model_expanded(&mut self, model_index: usize, expanded: bool) -> bool {
        let Some(node) = self.model.as_ref().and_then(|model| model.node(model_index)) else {
            return false;
        };
        let fetchable = self.lazy_loading && self.can_fetch_more(&[model_index]);
        if !node.has_children && !fetchable {
            return false;
        }
        if self.is_expanded(model_index) == expanded {
            return true;
        }
        self.expanded_override.insert(model_index, expanded);
        if expanded {
            self.expanded.emit(model_index);
        } else {
            self.collapsed.emit(model_index);
        }
        if expanded && fetchable {
            let _ = self.fetch_more(&[model_index]);
        }
        self.normalize_projection_state();
        self.rebuild_projection();
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    /// Applies one expansion decision and rebuilds the projection exactly once.
    fn apply_expansion(
        &mut self,
        visible_index: usize,
        model_index: usize,
        expanded: bool,
    ) -> bool {
        let has_children = self
            .model
            .as_ref()
            .and_then(|model| model.node(model_index))
            .map(|node| node.has_children)
            .unwrap_or(false);
        let fetchable = expanded && self.lazy_loading && self.can_fetch_more(&[model_index]);
        if !has_children && !fetchable {
            return false;
        }
        if self.is_expanded(model_index) == expanded {
            return true;
        }
        let before = self.projection_cache.len();
        self.expanded_override.insert(model_index, expanded);
        if expanded {
            self.expanded.emit(model_index);
        } else {
            self.collapsed.emit(model_index);
        }
        if fetchable {
            // Host-driven and synchronous; the fetch rebuilds the projection when the model grew.
            let _ = self.fetch_more(&[model_index]);
        }
        // A collapse can hide the selected or focused node; the projection state has to follow, the
        // same way it follows a model swap.
        self.normalize_projection_state();
        self.rebuild_projection();
        if expanded && self.auto_scroll_to_expanded {
            let _ = self.ensure_visible(model_index);
        }
        if self.animated {
            let after = self.projection_cache.len();
            if expanded && after > before {
                // Only the rows that appeared fade; the rows that were already painted keep
                // their resting ink, so the tree does not flash on every expansion.
                self.animation_start = Some(visible_index + 1);
                self.animation_len = after - before;
                self.expansion_progress.jump_to(0.0);
                self.expansion_progress.set_target(1.0);
            } else {
                self.animation_start = None;
                self.animation_len = 0;
            }
        }
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    /// Rebuilds the cached projection when a model change invalidated it.
    fn ensure_projection(&mut self) {
        if !self.projection_is_current() {
            self.rebuild_projection();
        }
    }

    /// Expands every branch in the model.
    ///
    /// Returns how many branch nodes changed state. The projection is rebuilt once, not once per
    /// node; `expanded` is emitted for each branch that actually changed.
    pub fn expand_all(&mut self) -> usize {
        let Some(count) = self.model.as_ref().map(|model| model.node_count()) else {
            return 0;
        };
        let mut changed = 0usize;
        for index in 0..count {
            let Some(node) = self.model.as_ref().and_then(|model| model.node(index)) else {
                continue;
            };
            if !node.has_children || self.is_expanded(index) {
                continue;
            }
            self.expanded_override.insert(index, true);
            self.expanded.emit(index);
            changed += 1;
        }
        if changed > 0 {
            self.normalize_projection_state();
            self.rebuild_projection();
            self.base.request_layout();
            self.base.request_redraw();
        }
        changed
    }

    /// Collapses every branch in the model.
    pub fn collapse_all(&mut self) -> usize {
        let Some(count) = self.model.as_ref().map(|model| model.node_count()) else {
            return 0;
        };
        let mut changed = 0usize;
        for index in 0..count {
            let Some(node) = self.model.as_ref().and_then(|model| model.node(index)) else {
                continue;
            };
            if !node.has_children || !self.is_expanded(index) {
                continue;
            }
            self.expanded_override.insert(index, false);
            self.collapsed.emit(index);
            changed += 1;
        }
        if changed > 0 {
            self.normalize_projection_state();
            self.rebuild_projection();
            self.base.request_layout();
            self.base.request_redraw();
        }
        changed
    }

    /// Expands every branch shallower than `depth`.
    ///
    /// `expand_to_depth(1)` therefore reveals each root's direct children and leaves deeper
    /// branches closed. Returns how many branches changed state.
    pub fn expand_to_depth(&mut self, depth: usize) -> usize {
        let Some(count) = self.model.as_ref().map(|model| model.node_count()) else {
            return 0;
        };
        let mut changed = 0usize;
        for index in 0..count {
            let Some(node) = self.model.as_ref().and_then(|model| model.node(index)) else {
                continue;
            };
            if !node.has_children || node.depth >= depth || self.is_expanded(index) {
                continue;
            }
            self.expanded_override.insert(index, true);
            self.expanded.emit(index);
            changed += 1;
        }
        if changed > 0 {
            self.normalize_projection_state();
            self.rebuild_projection();
            self.base.request_layout();
            self.base.request_redraw();
        }
        changed
    }

    /// Whether an expansion fades its newly revealed rows in (`animated`'s alias).
    pub fn expansion_animated(&self) -> bool {
        self.animated
    }

    /// Enables or disables the expansion fade (`set_animated`'s alias).
    pub fn set_expansion_animated(&mut self, animated: bool) {
        self.set_animated(animated);
    }

    /// Returns the current visible node count.
    pub fn node_count(&self) -> usize {
        self.visible_row_count()
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

    /// The advisory fetch ceiling, in milliseconds.
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

    /// Asks the model whether more children can be fetched below `path`.
    ///
    /// `TreeModel` is a flat node protocol, so a path's meaningful component is its last element;
    /// a model that overrides [`TreeModel::can_fetch_more_path`] can interpret the full chain.
    /// False while [`Self::lazy_loading`] is off, which keeps an eager model's behaviour unchanged.
    pub fn can_fetch_more(&self, path: &[usize]) -> bool {
        self.lazy_loading
            && self.model.as_ref().is_some_and(|model| model.can_fetch_more_path(path))
    }

    /// Asks the model for more children below `path` and reports the outcome.
    ///
    /// **Synchronous and host-driven.** `fetch_started` is emitted before the model is called and
    /// `fetch_finished` after it returns. A model that cannot answer immediately returns `false`
    /// and calls [`Self::finish_fetch`] when its background work completes; the control never
    /// starts an async runtime on the model's behalf.
    pub fn fetch_more(&mut self, path: &[usize]) -> bool {
        if !self.can_fetch_more(path) {
            return false;
        }
        if !self.loading_paths.iter().any(|pending| pending == path) {
            self.loading_paths.push(path.to_vec());
        }
        self.loading_indicator_visible = true;
        self.fetch_started.emit(path.to_vec());
        let changed = self.model.as_ref().is_some_and(|model| model.fetch_more_path(path));
        self.finish_fetch(path, changed);
        changed
    }

    /// Completes a fetch the host performed on its own schedule.
    pub fn finish_fetch(&mut self, path: &[usize], changed: bool) {
        self.loading_paths.retain(|pending| pending != path);
        self.loading_indicator_visible = !self.loading_paths.is_empty();
        if changed {
            // A successful fetch can add children without raising the model's change signal, so
            // the cached projection is stale even though the generation did not move.
            self.rebuild_projection();
            self.base.request_layout();
        }
        self.fetch_finished.emit((path.to_vec(), changed));
        self.base.request_redraw();
    }

    /// The first visible row.
    pub fn scroll_row(&self) -> usize {
        self.scroll_row
    }

    /// Sets the first visible row, clamped to the projection.
    pub fn set_scroll_row(&mut self, row: usize) {
        let lines = self.visible_row_count();
        let max = lines.saturating_sub(1);
        let next = row.min(max);
        if next == self.scroll_row {
            return;
        }
        self.scroll_row = next;
        self.base.request_redraw();
    }

    /// Scrolls so `model_index` is the first visible row (V-11).
    ///
    /// Returns `false` when the node is hidden under a collapsed ancestor, because there is no
    /// row to scroll to.
    pub fn scroll_to_node(&mut self, model_index: usize) -> bool {
        let Some(position) = self.visible_position_of(model_index) else {
            return false;
        };
        self.set_scroll_row(position);
        true
    }

    /// Scrolls the smallest amount that makes `model_index` visible (V-11).
    pub fn ensure_visible(&mut self, model_index: usize) -> bool {
        let Some(position) = self.visible_position_of(model_index) else {
            return false;
        };
        let page = (self.base.geometry().height / TREE_ROW_HEIGHT.max(1) as u32).max(1) as usize;
        if position < self.scroll_row {
            self.set_scroll_row(position);
        } else if position >= self.scroll_row.saturating_add(page) {
            self.set_scroll_row(position.saturating_add(1).saturating_sub(page));
        }
        true
    }

    /// Whether expanding a branch scrolls the expanded row into view.
    pub fn auto_scroll_to_expanded(&self) -> bool {
        self.auto_scroll_to_expanded
    }

    /// Enables or disables the follow-on scroll of an expansion (V-11).
    pub fn set_auto_scroll_to_expanded(&mut self, follow: bool) {
        self.auto_scroll_to_expanded = follow;
    }

    /// Whether the source-list (sidebar) presentation is used.
    pub fn source_list(&self) -> bool {
        self.source_list
    }

    /// Enables or disables the source-list presentation.
    pub fn set_source_list(&mut self, source_list: bool) {
        if self.source_list == source_list {
            return;
        }
        self.source_list = source_list;
        self.base.request_redraw();
    }

    /// Whether an expansion fades its newly revealed rows in.
    pub fn animated(&self) -> bool {
        self.animated
    }

    /// Enables or disables the expansion fade.
    ///
    /// Off by default: the settled drawing is the historical pixels, and the fade only changes
    /// the frames between an expansion and its end.
    pub fn set_animated(&mut self, animated: bool) {
        if self.animated == animated {
            return;
        }
        self.animated = animated;
        if !animated {
            self.animation_start = None;
            self.animation_len = 0;
            self.expansion_progress.jump_to(1.0);
        }
        self.base.request_redraw();
    }

    /// The progress of the running expansion fade, `0.0..=1.0`.
    pub fn expansion_progress(&self) -> f32 {
        if self.animated {
            self.expansion_progress.value()
        } else {
            1.0
        }
    }

    /// Whether the vertical scroll-bar thumb is painted.
    pub fn show_scrollbar(&self) -> bool {
        self.show_scrollbar
    }

    /// Enables or disables the vertical scroll-bar thumb.
    pub fn set_show_scrollbar(&mut self, show: bool) {
        if self.show_scrollbar == show {
            return;
        }
        self.show_scrollbar = show;
        self.base.request_redraw();
    }

    /// Whether vertical indent guide lines are painted (V-016).
    pub fn indent_lines(&self) -> bool {
        self.indent_lines
    }

    /// Enables or disables the indent guide lines. Off by default, which keeps the historical
    /// pixels.
    pub fn set_indent_lines(&mut self, indent_lines: bool) {
        if self.indent_lines == indent_lines {
            return;
        }
        self.indent_lines = indent_lines;
        self.base.request_redraw();
    }

    /// The multi-column descriptors (V-016). Empty means the single label column.
    pub fn columns(&self) -> &[TreeColumn] {
        &self.columns
    }

    /// Replaces the multi-column descriptors.
    ///
    /// Column `0` carries the disclosure glyph and the label; each later column reads
    /// [`TreeModel::node_column_text`]. An empty list restores the single-label presentation.
    pub fn set_columns(&mut self, columns: Vec<TreeColumn>) {
        let columns: Vec<TreeColumn> =
            columns.into_iter().map(|column| TreeColumn::new(column.title, column.width)).collect();
        if columns == self.columns {
            return;
        }
        self.columns = columns;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns the label of the visible row `index`.
    pub fn node_path(&self, index: usize) -> Option<String> {
        let model_index = self.visible_index_at(index)?;
        self.model.as_ref()?.node(model_index).map(|node| node.label)
    }

    /// Returns the shape of the visible row `index`.
    ///
    /// The row's own view, so a host can ask the same questions the renderer does (is this row a
    /// branch, how deep is it) without recomputing the visible projection itself.
    pub fn node_view(&self, index: usize) -> Option<NodeView> {
        let model_index = self.visible_index_at(index)?;
        self.model.as_ref()?.node(model_index)
    }

    /// The **model** index a visible row names, if the row exists.
    pub fn model_index(&self, index: usize) -> Option<usize> {
        self.visible_index_at(index)
    }

    /// The visible row `model_index` occupies, or `None` when it is hidden (F-P10).
    ///
    /// Reads the inverse map built with the projection, so keyboard navigation and the selection
    /// accessors are O(1) on a current cache rather than materialising and scanning every row.
    fn visible_position_of(&self, model_index: usize) -> Option<usize> {
        if self.projection_is_current() {
            return self.projection_positions.get(&model_index).copied();
        }
        self.walk_visible_indices(usize::MAX).iter().position(|index| *index == model_index)
    }

    /// The visible row of `model_index` as `Some`, or `None` when it is hidden.
    pub fn visible_row_of(&self, model_index: usize) -> Option<usize> {
        self.visible_position_of(model_index)
    }

    /// Selects a node by **visible** index.
    ///
    /// Repaints: the selected row's highlight is painted, so without the request the
    /// selection would only appear once some unrelated event repainted the control.
    pub fn select_node(&mut self, index: usize) -> bool {
        self.select_node_with_modifiers(index, crate::shortcut::Modifiers::empty())
    }

    /// Selects a node honoring keyboard modifiers (V-10).
    ///
    /// `Shift` extends the contiguous visible range from the anchor, `Ctrl`/`Meta` toggles the
    /// node, and an unmodified call replaces the selection — the model `ListView` uses. Selection
    /// is kept in **model** indices, so collapsing a branch cannot re-point it at another node.
    pub fn select_node_with_modifiers(
        &mut self,
        index: usize,
        modifiers: crate::shortcut::Modifiers,
    ) -> bool {
        let Some(model_index) = self.visible_index_at(index) else {
            return false;
        };
        self.apply_node_selection(index, model_index, modifiers);
        true
    }

    /// Applies one selection gesture to the node set.
    fn apply_node_selection(
        &mut self,
        visible_index: usize,
        model_index: usize,
        modifiers: crate::shortcut::Modifiers,
    ) {
        use crate::shortcut::Modifiers;
        if self.selection_mode == SelectionMode::None {
            return;
        }
        let shift = modifiers.contains(Modifiers::SHIFT);
        let toggle = modifiers.contains(Modifiers::CTRL) || modifiers.contains(Modifiers::PRIMARY);
        match self.selection_mode {
            SelectionMode::None => return,
            SelectionMode::Single => {
                self.selected_nodes.clear();
                self.selected_nodes.insert(model_index);
                self.selection_anchor = Some(model_index);
            }
            SelectionMode::Multi => {
                if !self.selected_nodes.insert(model_index) {
                    self.selected_nodes.remove(&model_index);
                }
                self.selection_anchor = Some(model_index);
            }
            SelectionMode::Extended => {
                if shift {
                    let anchor = self.selection_anchor.unwrap_or(model_index);
                    let (Some(anchor_row), Some(target_row)) =
                        (self.visible_position_of(anchor), self.visible_position_of(model_index))
                    else {
                        self.selected_nodes.clear();
                        self.selected_nodes.insert(model_index);
                        self.selection_anchor = Some(model_index);
                        self.finish_node_selection(visible_index, model_index);
                        return;
                    };
                    let (low, high) = if anchor_row <= target_row {
                        (anchor_row, target_row)
                    } else {
                        (target_row, anchor_row)
                    };
                    self.ensure_projection();
                    let range: std::collections::BTreeSet<usize> = self
                        .projection_cache
                        .get(low..=high)
                        .map(|rows| rows.iter().copied().collect())
                        .unwrap_or_default();
                    self.selected_nodes = range;
                } else if toggle {
                    if !self.selected_nodes.remove(&model_index) {
                        self.selected_nodes.insert(model_index);
                    }
                    self.selection_anchor = Some(model_index);
                } else {
                    self.selected_nodes.clear();
                    self.selected_nodes.insert(model_index);
                    self.selection_anchor = Some(model_index);
                }
            }
        }
        self.finish_node_selection(visible_index, model_index);
    }

    /// Publishes a completed selection gesture and moves focus to it.
    fn finish_node_selection(&mut self, visible_index: usize, model_index: usize) {
        self.selected_node = Some(model_index);
        self.selection_changed.emit(visible_index);
        self.selection_set_changed.emit(self.selected_nodes());
        self.set_focused_node(visible_index);
        self.base.request_redraw();
    }

    /// Every selected node, in visual order (V-10).
    pub fn selected_nodes(&self) -> Vec<usize> {
        if self.projection_is_current() {
            self.projection_cache
                .iter()
                .copied()
                .filter(|model_index| self.selected_nodes.contains(model_index))
                .collect()
        } else {
            self.selected_nodes.iter().copied().collect()
        }
    }

    /// The selection mode (V-10).
    pub fn selection_mode(&self) -> SelectionMode {
        self.selection_mode
    }

    /// Sets how many nodes one gesture may select (V-10).
    pub fn set_selection_mode(&mut self, mode: SelectionMode) {
        if self.selection_mode == mode {
            return;
        }
        self.selection_mode = mode;
        if mode != SelectionMode::Extended {
            self.selection_anchor = None;
        }
        if mode == SelectionMode::Single || mode == SelectionMode::None {
            let current = self.selected_node;
            self.selected_nodes.clear();
            if let Some(node) = current {
                self.selected_nodes.insert(node);
                self.selection_anchor = Some(node);
            }
        }
        self.selection_set_changed.emit(self.selected_nodes());
        self.base.request_redraw();
    }

    /// Clears node selection.
    ///
    /// Repaints: the highlight for the previously selected row is painted from this
    /// field, so clearing it has to mark damage.
    pub fn clear_selection(&mut self) {
        if self.selected_node.is_none() && self.selected_nodes.is_empty() {
            return;
        }
        self.selected_node = None;
        self.selected_nodes.clear();
        self.selection_anchor = None;
        self.selection_set_changed.emit(Vec::new());
        self.base.request_redraw();
    }
    /// Sets focused node by **visible** index.
    ///
    /// Repaints: the focus ring is painted from this field.
    pub fn set_focused_node(&mut self, index: usize) -> bool {
        let Some(model_index) = self.visible_index_at(index) else {
            return false;
        };
        if self.focused_node == Some(model_index) {
            return true;
        }
        self.focused_node = Some(model_index);
        self.focused_node_changed.emit(Some(index));
        self.base.request_redraw();
        true
    }
    /// Clears node focus.
    ///
    /// Repaints: the focus ring is painted from this field.
    pub fn clear_focused_node(&mut self) {
        if self.focused_node.is_none() {
            return;
        }
        self.focused_node = None;
        self.focused_node_changed.emit(None);
        self.base.request_redraw();
    }
    /// Returns the focused node's **visible** index when it is on screen.
    ///
    /// `None` also covers the case where the focused node is currently hidden under a collapsed
    /// ancestor — the focus is kept, but there is no row to point at.
    pub fn focused_node(&self) -> Option<usize> {
        let focused = self.focused_node?;
        self.visible_position_of(focused)
    }

    /// The rectangle a node's own row occupies, or `None` when the index is not shown.
    ///
    /// # Why this is one derivation
    ///
    /// Three readers ask "where is node N": the draw loop, the press handler and the hover test.
    /// They used to derive it separately — the draw from the inset content box, the press from the
    /// control's literal top edge — so a click landed one row off whenever the inset was non-zero,
    /// which is exactly the defect `table_widget` records. One function, three readers.
    pub fn node_row_rect(&self, index: usize) -> Option<Rect> {
        let content = ControlMetrics::band_inset(self.base.geometry(), TREE_INSET);
        if index < self.scroll_row {
            return None;
        }
        let line = index - self.scroll_row;
        // `index as i32` narrowed a `usize` above `i32::MAX` to a negative row offset, and the
        // multiplication then wrapped — a row that scrolls nowhere near the control could map onto a
        // visible position. Reject an index that cannot be represented before multiplying; such an
        // index is far past any renderable row, so `None` is the honest answer.
        let row_offset = i32::try_from(line).ok()?;
        let y = content.y + TREE_ROW_HEIGHT.saturating_mul(row_offset);
        // A row that would extend past the content box is not shown, matching the draw loop's own
        // bound so the row the pointer can hit is exactly the row that was painted.
        if y + TREE_ROW_HEIGHT > content.y + content.height as i32 {
            return None;
        }
        Some(Rect::new(content.x, y, content.width, TREE_ROW_HEIGHT as u32))
    }

    /// The visible row at a screen point, or `None` when the point is not over a painted row.
    pub fn node_at(&self, pos: crate::core::Point) -> Option<usize> {
        let content = ControlMetrics::band_inset(self.base.geometry(), TREE_INSET);
        if pos.y < content.y || pos.y >= content.y + content.height as i32 {
            return None;
        }
        let index = ((pos.y - content.y) / TREE_ROW_HEIGHT).max(0) as usize + self.scroll_row;
        // A single-row lookup, not the whole projection: this runs on every pointer move, and
        // counting or materialising the document made hover cost follow the model size.
        self.visible_index_at(index).map(|_| index)
    }

    /// How many rows the visible projection has, without building the list.
    ///
    /// The same walk as [`Self::visible_indices`] with nothing pushed, for the readers that only
    /// need the bound: the hit test runs per pointer move, and a `Vec` per move would make a large
    /// tree's hover cost follow the document rather than the viewport.
    pub fn visible_row_count(&self) -> usize {
        if self.projection_is_current() {
            return self.projection_cache.len();
        }
        let Some(model) = self.model.as_ref() else {
            return 0;
        };
        let mut shown = 0usize;
        let mut ancestors: Vec<(usize, bool)> = Vec::new();
        for index in 0..model.node_count() {
            let Some(node) = model.node(index) else { continue };
            while ancestors.last().is_some_and(|(depth, _)| *depth >= node.depth) {
                ancestors.pop();
            }
            if ancestors.iter().all(|(_, expanded)| *expanded) {
                shown += 1;
            }
            if node.has_children {
                ancestors.push((node.depth, self.is_expanded(index)));
            }
        }
        shown
    }

    /// The disclosure glyph's box on the visible row `index`, when that row is a branch.
    ///
    /// One derivation for the two readers that must agree: the draw loop paints the glyph here and
    /// `handle_event` tests a click against it. It is derived from the same `(depth + 1) *
    /// TREE_INDENT` gutter the label is placed after, so the glyph always sits immediately left of
    /// its own label rather than at a fixed offset from the control's edge.
    pub fn disclosure_rect(&self, index: usize) -> Option<Rect> {
        let node = self.node_view(index)?;
        let row = self.node_row_rect(index)?;
        disclosure_box(row, node.depth, node.has_children)
    }

    /// Returns the selected node's **visible** index when it is on screen.
    pub fn selected_node(&self) -> Option<usize> {
        let selected = self.selected_node?;
        self.visible_position_of(selected)
    }

    fn normalize_projection_state(&mut self) {
        let model_count = self.model.as_ref().map(|model| model.node_count()).unwrap_or(0);
        // Selection, focus and the open edit are **model** indices, so they are filtered against
        // the model's size — not against the visible row count, which a collapse can shrink without
        // the node having gone away.
        self.selected_node = self.selected_node.filter(|index| *index < model_count);
        self.selected_nodes.retain(|index| *index < model_count);
        self.focused_node = self.focused_node.filter(|index| *index < model_count);
        self.editing = self.editing.filter(|index| *index < model_count);
        self.expanded_override.retain(|index, _| *index < model_count);
    }

    // ── In-place editing ─────────────────────────────────────────────────────

    /// Sets the in-place edit delegate.
    ///
    /// The same [`ItemDelegate`] seam [`TableWidget`](super::table_widget::TableWidget) uses, not a
    /// tree-specific twin: a host that has written an editor factory for its tables should not have
    /// to write a second one for its tree.
    pub fn set_delegate(&mut self, delegate: Arc<dyn ItemDelegate>) {
        self.delegate = Some(delegate);
    }

    /// Returns whether an edit delegate is bound.
    pub fn has_delegate(&self) -> bool {
        self.delegate.is_some()
    }

    /// Returns the bound edit delegate, if present.
    pub fn delegate_ref(&self) -> Option<&Arc<dyn ItemDelegate>> {
        self.delegate.as_ref()
    }

    /// Begins an in-place edit of the visible row `index`, returning the editor the delegate built.
    ///
    /// # Why the tree needs the same seam the table has
    ///
    /// Renaming a file in a project tree is a basic operation, and without this the host had to
    /// build it from pieces it could not get right: take [`Self::node_row_rect`], create its own
    /// `TextEdit`, place it over the row, and then keep it aligned through scrolling and selection
    /// changes. The table sibling already solved that with one operation, so the tree mirrors it.
    ///
    /// The delegate is addressed with `row = the node's **model** index` and `column = 0`: a tree
    /// has one column, and a model index is stable across the expansion changes that would move a
    /// visible index mid-edit. Hosts that need their own addressing can map it through
    /// [`Self::model_index`].
    ///
    /// Returns `None` when there is no delegate or model, or the row does not exist — three ways
    /// for the request to be meaningless rather than errors.
    pub fn begin_edit(&mut self, index: usize) -> Option<Box<dyn Widget>> {
        let delegate = self.delegate.clone()?;
        let model_index = self.model_index(index)?;
        let mut editor = delegate.create_editor(&mut self.base, model_index, 0)?;
        delegate.set_editor_data(editor.as_mut(), model_index, 0);
        self.editing = Some(model_index);
        self.base.request_redraw();
        Some(editor)
    }

    /// Reads the current text back out of an editor and commits it.
    ///
    /// The counterpart to [`Self::begin_edit`], and the reason `get_editor_data` exists: the
    /// editor's own representation is the delegate's business, so the value is read back through
    /// the delegate rather than by downcasting the editor to a known control. Returns the committed
    /// text, or `None` when there is no edit in progress or the delegate declines the node.
    pub fn commit_edit(&mut self, editor: &dyn Widget) -> Option<String> {
        let delegate = self.delegate.clone()?;
        let model_index = self.editing?;
        let text = delegate.get_editor_data(editor, model_index, 0);
        self.editing = None;
        self.base.request_redraw();
        text
    }

    /// The node an in-place edit is open on, as a **model** index, if any.
    pub fn editing_node(&self) -> Option<usize> {
        self.editing
    }

    /// Abandons an in-place edit without committing it.
    pub fn cancel_edit(&mut self) {
        if self.editing.take().is_some() {
            self.base.request_redraw();
        }
    }

    /// Whether the focused node is currently expanded, or `None` when no visible branch is focused.
    ///
    /// `None` covers both "nothing is focused" and "the focused node is a leaf". Both are "there is
    /// no branch state to report", which a `bool` would have to spell as `false` — the same
    /// collapse of two facts that the `expanded_focused_node` property would otherwise inherit.
    pub fn focused_node_expanded(&self) -> Option<bool> {
        let visible = self.focused_node()?;
        let model_index = self.model_index(visible)?;
        let node = self.node_view(visible)?;
        node.has_children.then(|| self.is_expanded(model_index))
    }

    /// Expands the focused node, if it is a branch.
    pub fn expand_focused(&mut self) -> bool {
        match self.focused_node() {
            Some(index) => self.expand(index),
            None => false,
        }
    }

    /// Collapses the focused node, if it is a branch.
    pub fn collapse_focused(&mut self) -> bool {
        match self.focused_node() {
            Some(index) => self.collapse(index),
            None => false,
        }
    }

    /// Toggles the focused node, if it is a branch.
    pub fn toggle_focused(&mut self) -> bool {
        match self.focused_node() {
            Some(index) => self.toggle_expanded(index),
            None => false,
        }
    }
}
impl Widget for TreeView {
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
            crate::widget::metrics::dimensions::TREE_VIEW_DEFAULT_WIDTH,
            crate::widget::metrics::dimensions::TREE_VIEW_DEFAULT_HEIGHT,
        )
    }

    /// Advances the expansion fade by `delta_ms`; `true` while it is still moving.
    fn tick(&mut self, delta_ms: u32) -> bool {
        self.expansion_progress.tick(delta_ms)
    }

    fn is_animating(&self) -> bool {
        self.expansion_progress.is_moving()
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();

    /// Opts the tree into the runtime's drag coordinator as a source (V-016).
    fn drag_source_mut(&mut self) -> Option<&mut dyn DragSource> {
        Some(self)
    }

    /// Opts the tree into the runtime's drag coordinator as a drop target (V-016).
    fn drop_target_mut(&mut self) -> Option<&mut dyn DropTarget> {
        Some(self)
    }
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
            "focused_node_changed" => Some(EventSignalRef::mapped(
                "focused_node_changed",
                &self.focused_node_changed,
                |v| match v {
                    Some(value) => CapabilityValue::UInt(*value as u64),
                    None => CapabilityValue::Null,
                },
            )),
            "activated" => Some(EventSignalRef::mapped("activated", &self.activated, |v| {
                crate::widget::capability::CapabilityValue::UInt(*v as u64)
            })),
            "collapsed" => Some(EventSignalRef::mapped("collapsed", &self.collapsed, |v| {
                crate::widget::capability::CapabilityValue::UInt(*v as u64)
            })),
            "expanded" => Some(EventSignalRef::mapped("expanded", &self.expanded, |v| {
                crate::widget::capability::CapabilityValue::UInt(*v as u64)
            })),
            "node_double_clicked" => Some(EventSignalRef::mapped(
                "node_double_clicked",
                &self.node_double_clicked,
                |v| crate::widget::capability::CapabilityValue::UInt(*v as u64),
            )),
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
            "selection_set_changed" => Some(EventSignalRef::mapped(
                "selection_set_changed",
                &self.selection_set_changed,
                |v| {
                    crate::widget::capability::CapabilityValue::Tuple(
                        v.iter()
                            .map(|item| {
                                crate::widget::capability::CapabilityValue::UInt(*item as u64)
                            })
                            .collect(),
                    )
                },
            )),
            _ => None,
        }
    }
}

/// A row drag carries its **visible** row index; the label is the node's own text (V-016).
impl DragSource for TreeView {
    fn drag_payload(&self, pos: Point) -> Option<DragPayload> {
        if !self.base.is_enabled() {
            return None;
        }
        let row = self.node_at(pos)?;
        let model_index = self.visible_index_at(row)?;
        let label =
            self.model.as_ref()?.node(model_index).map(|node| node.label).unwrap_or_default();
        Some(DragPayload::new("text/plain", row.to_string()).with_label(label).with_origin(pos))
    }

    /// Reports the outcome to the host.
    fn on_drag_finished(&mut self, effect: DropEffect) {
        self.drag_finished.emit(effect);
    }
}

/// A row dropped on the tree requests a move (V-016).
///
/// `TreeModel` is read-only by design (`Arc<dyn TreeModel>`), so the control cannot write the
/// reorder through to the model it does not own. The drop therefore reports a request, which is
/// the same ghost/drop path `VirtualList` uses, with the commit point moved to the model owner.
impl DropTarget for TreeView {
    fn can_accept(&self, payload: &DragPayload) -> bool {
        if !self.base.is_enabled() || !payload.is_type("text/plain") {
            return false;
        }
        // A row index only addresses the view that produced it.
        if !payload.came_from(self.base.id()) {
            return false;
        }
        payload.item_id.parse::<usize>().is_ok_and(|row| row < self.visible_row_count())
    }

    fn on_drop(&mut self, payload: &DragPayload, pos: Point) -> DropEffect {
        if !self.can_accept(payload) {
            return DropEffect::None;
        }
        let Ok(from) = payload.item_id.parse::<usize>() else {
            return DropEffect::None;
        };
        let Some(to) = self.node_at(pos) else {
            return DropEffect::None;
        };
        if from != to {
            self.row_move_requested.emit((from, to));
        }
        DropEffect::Move
    }

    /// Highlights the row a drop would land on.
    fn preview_rect(&self, payload: &DragPayload, pos: Point) -> Option<Rect> {
        if !self.can_accept(payload) {
            return None;
        }
        let row = self.node_at(pos)?;
        self.node_row_rect(row)
    }
}

/// `TreeView`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_view.in.rs` / `access_write_view.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before.
///
/// The old dispatch also answered the `TreeView`-kind names that really belong to
/// `TreeTable` (`row_count`, `column_count`, `selected_row`, `row_height`,
/// `column_width`, `projection_state`): the names were reachable because
/// `WidgetKind::TreeView` also carries `TreeTable` instances. Those stay with
/// `TreeTable`'s own contract so the two controls keep owning their own fields.
impl WidgetProperties for TreeView {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "has_model" => Ok(CapabilityValue::Bool(self.has_model())),
            "node_count" => Ok(CapabilityValue::UInt(self.node_count() as u64)),
            "has_delegate" => Ok(CapabilityValue::Bool(self.has_delegate())),
            "expansion_animated" => Ok(CapabilityValue::Bool(self.expansion_animated())),
            // The focused node's expansion state, or `Null` when it is not a visible branch: a
            // `bool` could not say "no branch is focused", which is a different fact from
            // "collapsed".
            "expanded_focused_node" => match self.focused_node_expanded() {
                Some(expanded) => Ok(CapabilityValue::Bool(expanded)),
                None => Ok(CapabilityValue::Null),
            },
            "focused_node" => match self.focused_node() {
                Some(node) => Ok(CapabilityValue::UInt(node as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "selected_node" => match self.selected_node() {
                Some(node) => Ok(CapabilityValue::UInt(node as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "lazy_loading" => Ok(CapabilityValue::Bool(self.lazy_loading())),
            "loading_indicator_visible" => {
                Ok(CapabilityValue::Bool(self.loading_indicator_visible()))
            }
            "fetch_timeout_ms" => Ok(CapabilityValue::UInt(self.fetch_timeout_ms() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "expansion_animated" => {
                self.set_expansion_animated(expect_bool(value)?);
                Ok(())
            }
            "focused_node" => match value {
                CapabilityValue::Null => {
                    self.clear_focused_node();
                    Ok(())
                }
                other => {
                    let node = expect_usize(other)?;
                    if self.set_focused_node(node) {
                        Ok(())
                    } else {
                        // `set_focused_node` answers `false` when `node` is not a live
                        // node index — an argument fault, not a capability gap.
                        Err(CapabilityAccessError::OutOfRange)
                    }
                }
            },
            "has_model" => Err(CapabilityAccessError::ReadOnlyProperty),
            "node_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            "selected_node" => Err(CapabilityAccessError::ReadOnlyProperty),
            "has_delegate" => Err(CapabilityAccessError::ReadOnlyProperty),
            "expanded_focused_node" => Err(CapabilityAccessError::ReadOnlyProperty),
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
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "has_model",
            "node_count",
            "focused_node",
            "selected_node",
            "has_delegate",
            "expansion_animated",
            "expanded_focused_node",
            "lazy_loading",
            "loading_indicator_visible",
            "fetch_timeout_ms",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `tree_view` publishes. All are payload-free.
    ///
    /// The expand/collapse commands act on the **focused** node, because a payload-free command
    /// cannot name another one. A host that wants to open a specific row calls
    /// [`TreeView::expand`] with a visible index directly.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "clear_selection" => {
                self.clear_selection();
                Ok(())
            }
            "clear_focused_node" => {
                self.clear_focused_node();
                Ok(())
            }
            "expand" => {
                // `false` here means "there is no visible focused branch", which is an invocation
                // that could not complete rather than an unknown name.
                if self.expand_focused() {
                    Ok(())
                } else {
                    Err(CapabilityAccessError::OutOfRange)
                }
            }
            "collapse" => {
                if self.collapse_focused() {
                    Ok(())
                } else {
                    Err(CapabilityAccessError::OutOfRange)
                }
            }
            "toggle" => {
                if self.toggle_focused() {
                    Ok(())
                } else {
                    Err(CapabilityAccessError::OutOfRange)
                }
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for TreeView {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        // Rows are laid out from the control's inset content box rather than from its
        // literal top edge. Every node used to start at `rect.y` and draw its label at
        // `y + item_height / 2` with a top-left origin, so the first node's glyph box began on
        // the frame's own stroke. The inset is now applied by `node_row_rect`, which the hit test
        // reads too, so the drawn row and the clickable row are one derivation.
        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then a literal. The theme step is what makes an appearance
        // switch visible; the surface, the border, the focused-node highlight and the text
        // colour used to be hardcoded literals, so light and dark rendered identically.
        //
        // `tree_view` reaches the theme through the `TreeView` classification, which is
        // `Input` — a scrolling field whose whole rectangle is the control. The theme reads
        // take and release the global manager's lock internally, so no guard is held across
        // the draw (the mutex is not re-entrant).
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("tree_view");
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
            .filter(|resolved| *resolved != surface)
            .unwrap_or_else(|| surface.blend(&ink, 0.20));
        // The focused node is a selection state, so it reads the theme's accent token and is
        // laid over the surface, which keeps it legible in either appearance.
        let accent = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.primary)
            .unwrap_or(Color::PRIMARY);
        let focused_bg = surface.blend(&accent, 0.30);
        let selected_bg = surface.blend(&accent, 0.30);

        // Draw background
        context.face_with_gradient_corners(
            rect,
            surface,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().resolved_corner_radii().to_array(),
            Color::BLACK,
        );
        // Draw border
        crate::widget::draw::stroke_surface_border(
            context,
            rect,
            self.style().resolved_corner_radii().to_array(),
            border,
            1,
        );
        // A source list is a sidebar: an accent-tinted face separates it from the content area
        // behind it, and the tint is inside the frame so the border keeps its own ink. Off by
        // default, which is what keeps the historical pixels.
        if self.source_list {
            context.fill_rect(
                Rect::new(
                    rect.x + 1,
                    rect.y + 1,
                    rect.width.saturating_sub(2),
                    rect.height.saturating_sub(2),
                ),
                surface.blend(&accent, 0.06),
            );
        }
        // Draw nodes from model. Each node is a band of the content box, never of the
        // control, and the label's line box is derived from the row rather than from
        // `y + item_height / 2` — which put the glyph box's top edge on the row's middle
        // line and left the first node pinned to y=0.
        if let Some(ref model) = self.model {
            // The **visible projection**, computed only as far as the viewport can show. The
            // full projection materialised one entry per node — a 10k-node tree allocated a
            // 10k-entry `Vec` per frame to paint ~30 rows. The limit is the row count the
            // content box can hold plus one (the loop stops at the first row that does not
            // fit), so the projection stops as soon as painting will.
            let content = ControlMetrics::band_inset(rect, TREE_INSET);
            let row_capacity = (content.height / TREE_ROW_HEIGHT as u32) as usize + 1;
            // Start the cursor at the first visible row: the prefix above the viewport is never
            // materialised, however deep the scroll is (V-010).
            let rows = self.visible_indices_from(self.scroll_row, row_capacity);
            let font = effective_font(style);
            let reveal = self.expansion_progress();
            for (offset, &model_index) in rows.iter().enumerate() {
                let visible = self.scroll_row + offset;
                // The row's own rectangle comes from the same derivation the hit test uses, so a
                // click cannot land on a row other than the one that was painted.
                let Some(row) = self.node_row_rect(visible) else {
                    break;
                };
                // Rows the most recent expansion revealed fade in; every other row keeps the
                // resting ink, so a settled tree paints exactly the historical pixels.
                let fading = self.animated
                    && self.animation_start.is_some_and(|start| {
                        visible >= start && visible < start + self.animation_len
                    });
                let row_ink =
                    if fading { ink.with_alpha((ink.a as f32 * reveal) as u8) } else { ink };
                // Highlighting compares **model** indices: the selection is stable across an
                // expansion change, so the wash follows the node and not the row number.
                if Some(model_index) == self.focused_node {
                    context.fill_rect(row, focused_bg);
                } else if self.selected_nodes.contains(&model_index) {
                    // A selected non-focused node keeps the selection wash (V-10); before the set
                    // existed, only the focused node was ever painted.
                    context.fill_rect(row, selected_bg);
                } else if Some(model_index) == self.hovered_node {
                    // A weaker wash than the focus, from the same accent: a row the pointer is over
                    // is a *preview* of the row a click would focus.
                    context.fill_rect(row, surface.blend(&accent, 0.12));
                }
                let Some(node) = model.node(model_index) else { continue };
                // One level of indent for a root, one more per depth below it. `saturating_mul`
                // because a model is free to report an absurd depth, and a row that overflows
                // `i32` must draw off-screen rather than wrap onto another row.
                let depth = i32::try_from(node.depth).unwrap_or(i32::MAX);
                let indent = TREE_INDENT.saturating_mul(depth.saturating_add(1));
                // One guide per open ancestor level, from the parent's label origin down to the
                // row. Off by default, so a settled tree keeps the historical pixels (V-016).
                if self.indent_lines {
                    for level in 1..=node.depth {
                        let guide_x = row.x + TREE_INDENT.saturating_mul(level as i32);
                        context.draw_line(
                            Point::new(guide_x, row.y),
                            Point::new(guide_x, row.y + row.height as i32),
                            ink.with_alpha(60),
                        );
                    }
                }
                if node.has_children {
                    // The disclosure glyph is the only thing that says a row can be opened, so it
                    // is drawn from the same derivation the click test uses — the free helper,
                    // which takes the facts this loop already has instead of re-walking the model
                    // once per row.
                    if let Some(box_rect) = disclosure_box(row, node.depth, node.has_children) {
                        let center_y = box_rect.y + box_rect.height as i32 / 2;
                        let left = box_rect.x;
                        let size = box_rect.width as i32;
                        let (a, b, c) = if self.is_expanded(model_index) {
                            // Expanded: points down.
                            (
                                Point::new(left, center_y - size / 2),
                                Point::new(left + size, center_y - size / 2),
                                Point::new(left + size / 2, center_y + size / 2),
                            )
                        } else {
                            // Collapsed: points right.
                            (
                                Point::new(left, center_y - size / 2),
                                Point::new(left, center_y + size / 2),
                                Point::new(left + size, center_y),
                            )
                        };
                        context.draw_line(a, b, row_ink);
                        context.draw_line(b, c, row_ink);
                        context.draw_line(c, a, row_ink);
                    }
                }
                if self.columns.is_empty() {
                    if !node.label.is_empty() {
                        let cell = crate::core::Rect::new(
                            row.x + indent,
                            row.y,
                            row.width.saturating_sub(indent.max(0) as u32),
                            row.height,
                        );
                        context.draw_text_fitted_cow(
                            context.text_line(cell, font),
                            &node.label,
                            font,
                            row_ink,
                            HorizontalAlignment::Left,
                        );
                    }
                } else {
                    // Column 0 carries the indent and the label; every later column is placed
                    // after the previous descriptor's width (V-016). A model that does not
                    // describe a column answers `None` and simply paints nothing there.
                    let mut column_x = row.x;
                    for (column, descriptor) in self.columns.iter().enumerate() {
                        let width = descriptor.width.max(1);
                        if column == 0 {
                            let cell_width = width.saturating_sub(indent.max(0) as u32);
                            if cell_width > 0 && !node.label.is_empty() {
                                let cell = crate::core::Rect::new(
                                    row.x + indent,
                                    row.y,
                                    cell_width,
                                    row.height,
                                );
                                context.draw_text_fitted_cow(
                                    context.text_line(cell, font),
                                    &node.label,
                                    font,
                                    row_ink,
                                    HorizontalAlignment::Left,
                                );
                            }
                        } else if let Some(text) = model.node_column_text(model_index, column) {
                            if !text.is_empty() {
                                let cell = crate::core::Rect::new(
                                    column_x,
                                    row.y,
                                    width.min(row.width),
                                    row.height,
                                );
                                context.draw_text_fitted_cow(
                                    context.text_line(cell, font),
                                    &text,
                                    font,
                                    row_ink,
                                    HorizontalAlignment::Left,
                                );
                            }
                        }
                        column_x = column_x.saturating_add(width as i32);
                    }
                }
            }

            // The loading placeholder sits immediately below the branch being fetched, where its
            // first child will appear. An expanded lazy branch is otherwise indistinguishable
            // from an empty one until the host's fetch completes.
            if self.loading_indicator_visible {
                if let Some(path) = self.loading_paths.first() {
                    let pending = path.last().copied().unwrap_or(0);
                    // The inverse map answers the row directly; the old form materialised the
                    // whole projection just to call `position` on it (V-010).
                    let visible = self.visible_position_of(pending);
                    let slot = match visible {
                        Some(row) if row >= self.scroll_row => row - self.scroll_row + 1,
                        Some(_) => 0,
                        None => self.visible_row_count().saturating_sub(self.scroll_row),
                    };
                    let y = content.y + slot as i32 * TREE_ROW_HEIGHT;
                    if y + TREE_ROW_HEIGHT <= content.bottom() {
                        let band = Rect::new(content.x, y, content.width, TREE_ROW_HEIGHT as u32);
                        context.fill_rect(band, surface.blend(&accent, 0.06));
                        let indent = TREE_INDENT.saturating_mul(path.len() as i32 + 1);
                        let text_x = band.x + indent;
                        let text_band = Rect::new(
                            text_x,
                            y,
                            (band.right() - text_x).max(0) as u32,
                            TREE_ROW_HEIGHT as u32,
                        );
                        context.draw_text_fitted_cow(
                            context.text_line(text_band, font),
                            "Loading...",
                            font,
                            ink.with_alpha_f32(0.65),
                            HorizontalAlignment::Left,
                        );
                    }
                }
            }
        }

        // The thumb is opt-in: wheel and keyboard already reach every row, so the bar is the
        // visible statement of how much of the tree is off-screen.
        if self.show_scrollbar {
            let total = self.visible_row_count().max(1);
            let visible = (rect.height / TREE_ROW_HEIGHT.max(1) as u32).max(1) as usize;
            let track = rect.height.max(1);
            let thumb =
                ((track as u64 * visible as u64) / total as u64).max(24).min(track as u64) as u32;
            let travel = track.saturating_sub(thumb);
            let max_scroll = total.saturating_sub(visible);
            let offset = if max_scroll == 0 {
                0
            } else {
                (travel as u64 * self.scroll_row.min(max_scroll) as u64 / max_scroll as u64) as u32
            };
            context.fill_rect(
                Rect::new(rect.x + rect.width as i32 - 6, rect.y + offset as i32, 4, thumb),
                accent.with_alpha(120),
            );
        }
    }
}
impl crate::event::EventHandler for TreeView {
    fn handle_event(&mut self, event: &crate::event::Event) {
        // The base keeps the control-level facts (`hovered`, `pressed`, `focus_reason`) and its
        // `MouseEnter`/`MouseLeave` arms are what make `widget_state()` answer `Hover` here. This
        // handler did not forward to it at all, so the theme's `"tree_view:hover"` override could
        // never fire — the same defect `list_view` records, in the sibling that never inherited it.
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            crate::event::Event::MousePress { pos, button, modifiers } if *button == 1 => {
                // The same index derivation the draw uses, so the row a click selects is the row
                // the user pointed at.
                if let Some(index) = self.node_at(*pos) {
                    // A press on the disclosure glyph opens or closes the branch instead of just
                    // selecting it: that is the one gesture the glyph exists for, and testing it
                    // here (rather than in the label band) keeps the two hit areas from overlapping.
                    let on_glyph =
                        self.disclosure_rect(index).is_some_and(|glyph| point_in_rect(*pos, glyph));
                    let modifiers = crate::shortcut::Modifiers::from_event_bits(*modifiers);
                    self.select_node_with_modifiers(index, modifiers);
                    if on_glyph {
                        self.toggle_expanded(index);
                    }
                }
            }
            crate::event::Event::MouseMove { pos } => {
                // Hover is stored as a **model** index so it survives the expansion changes that
                // move row numbers; `None` when the pointer is off the rows.
                let hovered = self.node_at(*pos).and_then(|visible| self.model_index(visible));
                if hovered != self.hovered_node {
                    self.hovered_node = hovered;
                    self.base.request_redraw();
                }
            }
            crate::event::Event::MouseLeave { .. } => {
                if self.hovered_node.take().is_some() {
                    self.base.request_redraw();
                }
            }
            crate::event::Event::KeyPress { key, .. } => {
                // The platform tree convention: Right opens a branch (or moves into it), Left
                // closes it. Without these a keyboard user could select a node but never reach the
                // nodes inside a collapsed branch.
                let changed = match *key {
                    crate::event::key_codes::RIGHT => self.expand_focused(),
                    crate::event::key_codes::LEFT => self.collapse_focused(),
                    crate::event::key_codes::UP
                    | crate::event::key_codes::DOWN
                    | crate::event::key_codes::HOME
                    | crate::event::key_codes::END
                    | crate::event::key_codes::PAGE_UP
                    | crate::event::key_codes::PAGE_DOWN => {
                        let visible = self.visible_row_count();
                        if visible == 0 {
                            false
                        } else {
                            // O(1): the inverse map built with the projection, rather than
                            // materialising and scanning every visible row per key (F-P10).
                            self.ensure_projection();
                            let current = self
                                .focused_node
                                .and_then(|model_index| {
                                    self.projection_positions.get(&model_index).copied()
                                })
                                .unwrap_or(0);
                            let page = (self.base.geometry().height / TREE_ROW_HEIGHT.max(1) as u32)
                                .max(1) as usize;
                            let next = match *key {
                                crate::event::key_codes::UP => current.saturating_sub(1),
                                crate::event::key_codes::DOWN => {
                                    current.saturating_add(1).min(visible - 1)
                                }
                                crate::event::key_codes::HOME => 0,
                                crate::event::key_codes::END => visible - 1,
                                crate::event::key_codes::PAGE_UP => current.saturating_sub(page),
                                _ => current.saturating_add(page).min(visible - 1),
                            };
                            self.select_node(next);
                            self.set_focused_node(next);
                            if next < self.scroll_row {
                                self.set_scroll_row(next);
                            } else if next >= self.scroll_row + page {
                                self.set_scroll_row(next + 1 - page);
                            }
                            true
                        }
                    }
                    _ => false,
                };
                if changed {
                    self.base.request_redraw();
                }
            }
            crate::event::Event::MouseDoubleClick { pos, button: 1 } => {
                if let Some(index) = self.node_at(*pos) {
                    let model_index = self.model_index(index).unwrap_or(index);
                    self.node_double_clicked.emit(model_index);
                    self.activated.emit(model_index);
                    let _ = self.toggle_expanded(index);
                }
            }
            crate::event::Event::Wheel { delta, modifiers } => {
                let scroll = crate::event::WheelScroll::from_event(*delta, *modifiers, 1);
                let next = self.scroll_row as isize - scroll.lines_y as isize;
                self.set_scroll_row(next.max(0) as usize);
            }
            #[cfg(feature = "touch")]
            crate::event::Event::Tap { pos } => {
                if let Some(index) = self.node_at(*pos) {
                    self.select_node(index);
                }
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use std::sync::Arc;

    struct StaticTreeModel;

    impl TreeModel for StaticTreeModel {
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

    #[test]
    fn out_of_range_node_is_reported_as_out_of_range_not_unsupported() {
        // Same misreport as `list_view` (see the guard there): an index that addresses
        // no node is the caller's argument, and saying "unsupported on this widget"
        // would send them looking for a different control.
        use crate::widget::capability::types::CapabilityAccessError;

        let mut view = TreeView::new(Rect::new(0, 0, 120, 100));
        view.set_model(Arc::new(StaticTreeModel));
        assert_eq!(view.node_count(), 2, "the fixture must have nodes to be out of range of");

        assert_eq!(
            view.set("focused_node", CapabilityValue::UInt(99)),
            Err(CapabilityAccessError::OutOfRange),
            "an index past the last node is the caller's argument, not a capability gap"
        );
        // A valid index still works, which is what separates the two errors.
        assert_eq!(view.set("focused_node", CapabilityValue::UInt(1)), Ok(()));
        assert_eq!(view.focused_node(), Some(1));
    }

    #[test]
    fn tree_view_model_binding_roundtrip() {
        let mut view = TreeView::new(Rect::new(0, 0, 120, 100));
        assert!(!view.has_model());
        assert!(view.model_ref().is_none());

        view.set_model(Arc::new(StaticTreeModel));

        assert!(view.has_model());
        assert!(view.model_ref().is_some());
        assert_eq!(view.node_count(), 2);
        assert_eq!(view.node_path(99), None);
    }

    /// A click selects the node it landed on, and the frame's inset margin is not a row.
    ///
    /// # The defect this pins
    ///
    /// The draw loop laid rows out from the inset content box (`rect` shrunk by `TREE_INSET` on
    /// every edge) while the press handler computed the index from the control's literal top edge.
    /// The two agree for most points — which is why the defect could survive — and diverge exactly
    /// in the inset margins: a click on the 2 px frame counted as row 0, so clicking the border
    /// *selected the first node*, and a click in the bottom margin could address a row that was
    /// never painted. The same family as `table_widget`'s one-row offset, with a different surface.
    ///
    /// A first version of this test only clicked the *centre* of a row, where both derivations
    /// agree; reverse injection therefore passed. The assertion that carries the defect is the one
    /// in the margin, which is what a real user hits when they click near the edge of a list.
    #[test]
    fn a_click_selects_the_node_it_landed_on() {
        use crate::event::{Event, EventHandler};

        let mut view = TreeView::new(Rect::new(0, 0, 120, 100));
        view.set_model(Arc::new(StaticTreeModel));

        let row = view.node_row_rect(1).expect("a roomy view paints node 1");
        let centre = crate::core::Point::new(row.x + 4, row.y + row.height as i32 / 2);
        view.handle_event(&Event::MousePress { pos: centre, button: 1, modifiers: 0 });
        assert_eq!(
            view.selected_node(),
            Some(1),
            "a click inside node 1's own painted band must select node 1"
        );

        // And a click in the frame's own inset margin selects nothing rather than clamping to the
        // first node — the assertion that actually carries the defect (see the doc comment).
        let mut fresh = TreeView::new(Rect::new(0, 0, 120, 100));
        fresh.set_model(Arc::new(StaticTreeModel));
        fresh.handle_event(&Event::MousePress {
            pos: crate::core::Point::new(4, 0),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(fresh.selected_node(), None, "the inset margin is not a row");
    }

    /// The pointer reaching a row highlights it, and the base learns the pointer arrived.
    ///
    /// Two facts in one gesture, because they were two different omissions: the handler never
    /// forwarded to the base (so `"tree_view:hover"` was a dead key) and it never tracked a row
    /// (so a list of rows gave no feedback about which one a click would act on).
    #[test]
    fn hovering_a_row_highlights_it_and_reaches_the_base() {
        use crate::event::{Event, EventHandler};
        use crate::style::WidgetState;

        let mut view = TreeView::new(Rect::new(0, 0, 120, 100));
        view.set_model(Arc::new(StaticTreeModel));

        let row = view.node_row_rect(1).expect("a roomy view paints node 1");
        let centre = crate::core::Point::new(row.x + 4, row.y + row.height as i32 / 2);
        view.handle_event(&Event::MouseEnter { pos: centre });
        view.handle_event(&Event::MouseMove { pos: centre });
        assert_eq!(view.hovered_node, Some(1), "the row under the pointer is the one lit");
        assert_eq!(
            view.widget_state(),
            WidgetState::Hover,
            "the base must have been told the pointer arrived"
        );

        view.handle_event(&Event::MouseLeave { pos: crate::core::Point::new(500, 500) });
        assert_eq!(view.hovered_node, None, "leaving clears the row rather than latching it");
        assert_eq!(view.widget_state(), WidgetState::Normal);
    }

    /// A model that counts how many times it is asked for a node.
    ///
    /// The count is the *shape* of one frame's work, which is what a per-frame cost assertion can
    /// check without becoming a wall-clock test (the same reason `code_editor`'s tests count lines
    /// rather than milliseconds).
    struct CountingModel {
        nodes: Vec<NodeView>,
        lookups: std::sync::atomic::AtomicUsize,
    }

    impl CountingModel {
        fn nested(count: usize) -> Self {
            // A root **and** `count` child branches, so the visible projection is `count + 1` rows
            // and *every* painted row asks for a disclosure glyph.
            //
            // # Why the children must be branches, not leaves
            //
            // A first version made the children leaves, and the cost test then passed even with the
            // per-row re-derivation reinstated: the glyph is only computed for a node with
            // children, so exactly one row was affected and the injection had nothing to multiply.
            // A cost assertion whose fixture cannot reach the code under test is the same
            // "green because it never ran" shape the gate rules warn about — so the fixture is
            // branch-dense on purpose.
            let mut nodes = vec![NodeView {
                label: "root".to_string(),
                depth: 0,
                has_children: true,
                expanded: true,
            }];
            for i in 0..count {
                nodes.push(NodeView {
                    label: format!("branch {i}"),
                    depth: 1,
                    has_children: true,
                    expanded: true,
                });
            }
            Self { nodes, lookups: std::sync::atomic::AtomicUsize::new(0) }
        }
        fn lookups(&self) -> usize {
            self.lookups.load(std::sync::atomic::Ordering::SeqCst)
        }
        /// Zeroes the counter, so a frame's cost is measured on its own.
        fn reset_lookups(&self) {
            self.lookups.store(0, std::sync::atomic::Ordering::SeqCst);
        }
    }

    impl TreeModel for CountingModel {
        fn node_count(&self) -> usize {
            self.nodes.len()
        }
        fn node_path(&self, index: usize) -> Option<String> {
            self.nodes.get(index).map(|node| node.label.clone())
        }
        fn node(&self, index: usize) -> Option<NodeView> {
            self.lookups.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            self.nodes.get(index).cloned()
        }
    }

    /// One frame's model lookups must be proportional to the **node count**, not nodes × rows.
    ///
    /// # The defect this pins
    ///
    /// The first version of the depth-aware draw loop asked a per-row helper for each row's
    /// disclosure glyph, and that helper resolved the row's node by walking the whole visible
    /// projection. One frame therefore cost `rows × nodes` model lookups: a large tree's repaint
    /// followed the document instead of the viewport, which is the invariant the rest of this crate
    /// holds to. The fix hoists the projection out of the loop (it is computed once, before the
    /// rows) and derives each glyph from facts the loop already holds.
    ///
    /// The bound is generous on purpose — it is there to separate "linear in nodes" from
    /// "nodes times rows", not to pin an exact count.
    #[test]
    fn a_frame_does_not_look_up_the_model_once_per_row() {
        use crate::render::{PaintBackend, SoftwarePaintBackend};

        const LEAVES: usize = 400;
        let model = Arc::new(CountingModel::nested(LEAVES));
        let mut view = TreeView::new(Rect::new(0, 0, 400, 300));
        view.set_model(model.clone());
        assert_eq!(view.node_count(), LEAVES + 1, "the fixture must be fully expanded");

        // The setup above legitimately walks the projection; only the frame is under test, so the
        // counter starts here rather than at construction (a count that included the setup would
        // measure the fixture, not the draw).
        model.reset_lookups();

        let mut backend = SoftwarePaintBackend::new(crate::core::Size::new(400, 300), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut context = RenderContext::new(&mut backend);
        view.draw(&mut context);
        backend.end_frame();

        let lookups = model.lookups();
        let rows = view.node_count();
        assert!(
            lookups <= 4 * (LEAVES + 1),
            "one frame made {lookups} model lookups for {} nodes and {rows} rows; a budget of \
             nodes × rows would be {}, so this is per-row re-derivation rather than one walk",
            LEAVES + 1,
            rows * (LEAVES + 1)
        );
    }

    /// A small hierarchy fixture: `root` → `child_a`, `child_b` → `grandchild`, plus `other`.
    ///
    /// The model indices are stable and the labels name them, so a test can assert *which* node a
    /// visible index names instead of only how many rows there are.
    struct NestedModel;

    const NESTED: &[(usize, &str, bool, bool)] = &[
        // (depth, label, has_children, expanded)
        (0, "root", true, true),
        (1, "child_a", false, true),
        (1, "child_b", true, true),
        (2, "grandchild", false, true),
        (0, "other", false, true),
    ];

    impl TreeModel for NestedModel {
        fn node_count(&self) -> usize {
            NESTED.len()
        }
        fn node_path(&self, index: usize) -> Option<String> {
            NESTED.get(index).map(|(_, label, _, _)| (*label).to_string())
        }
        fn node(&self, index: usize) -> Option<NodeView> {
            NESTED.get(index).map(|(depth, label, has_children, expanded)| NodeView {
                label: (*label).to_string(),
                depth: *depth,
                has_children: *has_children,
                expanded: *expanded,
            })
        }
    }

    /// A flat model must keep rendering exactly as it did before the view understood depth.
    ///
    /// This is the compatibility half of the hierarchy change: `TreeModel::node`'s default gives
    /// every node depth 0 and no children, so the visible projection is the whole model and the one
    /// level of indent is the same 15 px the control has always drawn. Without this invariant the
    /// change would silently re-indent every existing flat tree (and its committed snapshot).
    #[test]
    fn a_flat_model_shows_every_node_at_the_original_indent() {
        let mut view = TreeView::new(Rect::new(0, 0, 120, 100));
        view.set_model(Arc::new(StaticTreeModel));

        assert_eq!(view.visible_indices(), vec![0, 1], "no branch exists, so nothing is hidden");
        assert_eq!(view.node_count(), 2);
        assert_eq!(view.node_path(0).as_deref(), Some("root"));
        assert_eq!(
            view.disclosure_rect(0),
            None,
            "a flat model has no branches, so no disclosure glyph is drawn"
        );
        // `(depth + 1) * TREE_INDENT` with depth 0 is the historical single indent.
        assert_eq!(TREE_INDENT, 15);
    }

    /// Collapsing a branch hides its descendants, and the visible projection says which model
    /// nodes the remaining rows name.
    #[test]
    fn collapsing_a_branch_hides_descendants_and_the_mapping_is_reported() {
        let mut view = TreeView::new(Rect::new(0, 0, 200, 200));
        view.set_model(Arc::new(NestedModel));

        assert_eq!(view.visible_indices(), vec![0, 1, 2, 3, 4]);

        // Visible index 2 is `child_b` (model index 2) and it has one child.
        assert!(view.collapse(2), "collapsing a branch reports that it happened");
        assert_eq!(view.visible_indices(), vec![0, 1, 2, 4]);
        assert_eq!(view.node_count(), 4);
        assert_eq!(view.model_index(3), Some(4), "the row after child_b is `other` now");
        assert_eq!(
            view.node_path(3).as_deref(),
            Some("other"),
            "the visible projection is the only index space the public accessors use"
        );
        assert!(!view.is_expanded(2));

        // Collapsing the root hides everything below it.
        assert!(view.collapse(0));
        assert_eq!(view.visible_indices(), vec![0, 4]);
        assert_eq!(view.model_index(0), Some(0));
        assert_eq!(view.model_index(1), Some(4));
    }

    /// A leaf cannot be expanded or collapsed, and saying so is the point.
    #[test]
    fn a_leaf_refuses_expansion() {
        let mut view = TreeView::new(Rect::new(0, 0, 200, 200));
        view.set_model(Arc::new(NestedModel));

        assert!(!view.expand(1), "`child_a` has no children, so there is nothing to expand");
        assert!(!view.collapse(1));
        assert_eq!(view.visible_indices(), vec![0, 1, 2, 3, 4], "a refused toggle changes nothing");
        // An index past the last row is refused too.
        assert!(!view.expand(99));
    }

    /// Selection is a node identity, not a row number: collapsing the branch around it must not
    /// silently re-point it at whatever row slid into that position.
    #[test]
    fn selection_follows_the_node_across_a_collapse() {
        let mut view = TreeView::new(Rect::new(0, 0, 200, 200));
        view.set_model(Arc::new(NestedModel));

        // Select `grandchild` (visible 3, model 3).
        assert!(view.select_node(3));
        assert_eq!(view.selected_node(), Some(3));

        // Collapse `child_b`, which hides the grandchild: there is no row for it now.
        assert!(view.collapse(2));
        assert_eq!(view.selected_node(), None, "a hidden node has no visible index to report");

        // Re-open the branch: the same node is selected again, not a neighbour.
        assert!(view.expand(2));
        assert_eq!(view.selected_node(), Some(3), "the selection was the node, not the row");
    }

    /// The disclosure glyph exists only for branches, and sits in the indent gutter of its own row.
    #[test]
    fn only_branches_have_a_disclosure_glyph_in_their_own_gutter() {
        let mut view = TreeView::new(Rect::new(0, 0, 200, 200));
        view.set_model(Arc::new(NestedModel));

        let root_glyph = view.disclosure_rect(0).expect("root is a branch");
        let child_glyph = view.disclosure_rect(2).expect("child_b is a branch");
        assert_eq!(view.disclosure_rect(1), None, "child_a is a leaf");
        assert_eq!(view.disclosure_rect(3), None, "grandchild is a leaf");

        let row0 = view.node_row_rect(0).unwrap();
        let row2 = view.node_row_rect(2).unwrap();
        assert_eq!(root_glyph.y, row0.y + (TREE_ROW_HEIGHT - TREE_DISCLOSURE_SIZE) / 2);
        assert!(
            child_glyph.x > root_glyph.x,
            "a deeper row's glyph must sit deeper: {} vs {}",
            child_glyph.x,
            root_glyph.x
        );
        assert_eq!(child_glyph.y, row2.y + (TREE_ROW_HEIGHT - TREE_DISCLOSURE_SIZE) / 2);
    }

    /// A press on the glyph toggles the branch; a press on the label only selects.
    #[test]
    fn a_press_on_the_glyph_toggles_and_a_press_on_the_label_selects() {
        use crate::event::{Event, EventHandler};

        let mut view = TreeView::new(Rect::new(0, 0, 200, 200));
        view.set_model(Arc::new(NestedModel));

        let glyph = view.disclosure_rect(2).expect("child_b is a branch");
        let on_glyph = crate::core::Point::new(glyph.x + 1, glyph.y + 1);
        view.handle_event(&Event::MousePress { pos: on_glyph, button: 1, modifiers: 0 });
        assert_eq!(view.visible_indices(), vec![0, 1, 2, 4], "the glyph closed child_b");
        assert_eq!(view.selected_node(), Some(2), "and the press still selected the row");

        // The label band, well to the right of the glyph, must not toggle anything.
        let label = crate::core::Point::new(glyph.x + TREE_INDENT + 4, glyph.y + 1);
        view.handle_event(&Event::MousePress { pos: label, button: 1, modifiers: 0 });
        assert_eq!(view.visible_indices(), vec![0, 1, 2, 4], "a label press does not toggle");
        assert_eq!(view.selected_node(), Some(2));
    }

    /// A delegate that stands in for a host's editor factory.
    struct RecordingDelegate;

    impl ItemDelegate for RecordingDelegate {
        fn create_editor(
            &self,
            _parent: &mut BaseWidget,
            _row: usize,
            _column: usize,
        ) -> Option<Box<dyn Widget>> {
            Some(Box::new(crate::widget::base_widgets::label::Label::new(
                String::new(),
                Rect::new(0, 0, 10, 10),
            )))
        }
        fn set_editor_data(&self, _editor: &mut dyn Widget, _row: usize, _column: usize) {}
        fn get_editor_data(
            &self,
            _editor: &dyn Widget,
            row: usize,
            column: usize,
        ) -> Option<String> {
            // The delegate is addressed with the node's **model** index; echoing it back makes the
            // addressing visible to the assertion below.
            Some(format!("{row}:{column}"))
        }
    }

    /// The in-place edit seam mirrors the table's: begin, commit, cancel.
    #[test]
    fn in_place_edit_runs_through_the_delegate_and_is_addressed_by_model_index() {
        let mut view = TreeView::new(Rect::new(0, 0, 200, 200));
        view.set_model(Arc::new(NestedModel));
        assert!(!view.has_delegate());
        assert!(view.begin_edit(0).is_none(), "with no delegate there is no editor to build");
        assert_eq!(
            view.set("has_delegate", CapabilityValue::Bool(true)),
            Err(crate::widget::capability::types::CapabilityAccessError::ReadOnlyProperty)
        );

        view.set_delegate(Arc::new(RecordingDelegate));
        assert!(view.has_delegate());
        assert!(view.delegate_ref().is_some());

        let editor = view.begin_edit(3).expect("grandchild is a row");
        assert_eq!(view.editing_node(), Some(3), "the edit is keyed by the node's model index");
        assert_eq!(view.commit_edit(editor.as_ref()).as_deref(), Some("3:0"));

        // Cancel drops an open edit without producing a value.
        let editor = view.begin_edit(1).expect("child_a is a row");
        assert_eq!(view.editing_node(), Some(1));
        view.cancel_edit();
        assert_eq!(view.editing_node(), None);
        drop(editor);

        // An index that is not a row cannot start an edit.
        assert!(view.begin_edit(99).is_none());
    }

    /// The capability table's expand/collapse commands drive the focused node.
    #[test]
    fn the_published_expand_commands_act_on_the_focused_node() {
        use crate::widget::capability::types::CapabilityAccessError;

        let mut view = TreeView::new(Rect::new(0, 0, 200, 200));
        view.set_model(Arc::new(NestedModel));

        // Nothing is focused yet, so the command cannot complete — which is `OutOfRange`, not an
        // unknown name.
        assert_eq!(view.command("expand"), Err(CapabilityAccessError::OutOfRange));

        assert!(view.set_focused_node(2));
        assert_eq!(view.focused_node_expanded(), Some(true));
        assert_eq!(view.command("collapse"), Ok(()));
        assert_eq!(view.focused_node_expanded(), Some(false));
        assert_eq!(view.command("toggle"), Ok(()));
        assert_eq!(view.focused_node_expanded(), Some(true));

        // A focused leaf has no expansion state to report or change.
        assert!(view.set_focused_node(1));
        assert_eq!(view.focused_node_expanded(), None);
        assert_eq!(view.command("expand"), Err(CapabilityAccessError::OutOfRange));
    }

    /// N-V-08: an index that cannot be a row offset is refused, not narrowed and multiplied.
    ///
    /// `TREE_ROW_HEIGHT * index as i32` turned a `usize` above `i32::MAX` into a negative offset and
    /// then wrapped the product, so an absurd index could map onto a visible row. It must answer
    /// `None` instead.
    #[test]
    fn an_out_of_range_row_index_is_refused() {
        let view = TreeView::new(Rect::new(0, 0, 200, 200));
        // The first row exists when there are nodes; an index far beyond `i32::MAX` never can.
        assert_eq!(view.node_row_rect(usize::MAX), None);
        assert_eq!(view.node_row_rect(i32::MAX as usize + 1), None);
        // A small, in-view index still resolves.
        assert!(view.node_row_rect(0).is_some());
    }

    /// Expansion signals, scrolling, keyboard navigation and lazy fetch are wired.
    #[test]
    fn expansion_signals_scrolling_and_keyboard_are_wired() {
        use crate::event::EventHandler;
        let mut view = TreeView::new(Rect::new(0, 0, 220, 100));
        let mut nodes: Vec<String> = (0..30).map(|index| std::format!("row {index}")).collect();
        nodes.insert(0, "root".to_string());
        view.set_model(Arc::new(VecTreeModel::new(nodes)));
        view.handle_event(&Event::Wheel { delta: crate::core::Point::new(0, -1), modifiers: 0 });
        assert_eq!(view.scroll_row(), 1);

        view.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Tab });
        view.handle_event(&Event::KeyPress { key: crate::event::key_codes::END, modifiers: 0 });
        assert!(view.focused_node().is_some());
        assert!(view.scroll_row() > 0, "the focused row is scrolled into view");

        let mut branch = NodeView::leaf("branch");
        branch.has_children = true;
        branch.expanded = false;
        let mut child = NodeView::leaf("child");
        child.depth = 1;
        let mut branched = TreeView::new(Rect::new(0, 0, 220, 200));
        branched.set_model(Arc::new(VecTreeModel::from_views(vec![branch, child])));
        let expanded_index = std::sync::Arc::new(std::sync::Mutex::new(None));
        let sink = std::sync::Arc::clone(&expanded_index);
        branched.expanded.connect(move |index| {
            if let Ok(mut slot) = sink.lock() {
                *slot = Some(*index);
            }
        });
        assert!(branched.expand(0));
        assert!(branched.is_expanded(0));
        assert_eq!(expanded_index.lock().map(|slot| *slot).unwrap_or(None), Some(0));
    }

    /// The opt-in expansion fade advances with `tick` and settles at full opacity.
    #[test]
    fn the_expansion_fade_advances_and_settles() {
        let mut branch = NodeView::leaf("branch");
        branch.has_children = true;
        branch.expanded = false;
        let mut child = NodeView::leaf("child");
        child.depth = 1;
        let mut view = TreeView::new(Rect::new(0, 0, 220, 200));
        view.set_model(Arc::new(VecTreeModel::from_views(vec![branch, child])));
        assert_eq!(view.expansion_progress(), 1.0, "a settled tree is fully opaque");

        view.set_animated(true);
        assert!(view.animated());
        view.set_show_scrollbar(true);
        assert!(view.show_scrollbar());
        view.set_source_list(true);
        assert!(view.source_list());
        assert!(view.expand(0));
        assert!(view.is_animating(), "the fade started");
        assert!(view.expansion_progress() < 1.0);
        assert!(view.tick(16));
        let _ = crate::widget::svg::render_to_svg(&mut view);
        assert!(!view.tick(10_000), "a long step settles the fade");
        assert_eq!(view.expansion_progress(), 1.0);
    }

    /// A flat lazy model: index 0 starts with no children (`has_children == false`) and can be
    /// fetched. Its storage lives behind a `Mutex`, the shape the `&self` fetch hook requires.
    struct LazyTreeModel {
        loaded: std::sync::Mutex<bool>,
    }

    impl TreeModel for LazyTreeModel {
        fn node_count(&self) -> usize {
            if *self.loaded.lock().expect("lazy lock") {
                3
            } else {
                1
            }
        }

        fn node_path(&self, index: usize) -> Option<String> {
            match index {
                0 => Some("root".to_string()),
                1 => Some("root/child-a".to_string()),
                2 => Some("root/child-b".to_string()),
                _ => None,
            }
        }

        fn node(&self, index: usize) -> Option<NodeView> {
            let mut node = self.node_path(index).map(NodeView::leaf)?;
            if index == 0 {
                // A lazy branch starts collapsed; `NodeView::leaf` defaults to expanded.
                node.expanded = false;
            } else {
                node.depth = 1;
            }
            Some(node)
        }

        fn can_fetch_more(&self, index: usize) -> bool {
            index == 0 && !*self.loaded.lock().expect("lazy lock")
        }

        fn fetch_more(&self, index: usize) -> bool {
            if index != 0 {
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

        let mut view = TreeView::new(Rect::new(0, 0, 220, 200));
        view.set_model(Arc::new(LazyTreeModel { loaded: std::sync::Mutex::new(false) }));

        let started = std::sync::Arc::new(std::sync::Mutex::new(Vec::<Vec<usize>>::new()));
        let finished = std::sync::Arc::new(std::sync::Mutex::new(Vec::<(Vec<usize>, bool)>::new()));
        let started_sink = std::sync::Arc::clone(&started);
        let finished_sink = std::sync::Arc::clone(&finished);
        view.fetch_started.connect(move |path| {
            if let Ok(mut seen) = started_sink.lock() {
                seen.push(path.as_ref().clone());
            }
        });
        view.fetch_finished.connect(move |payload| {
            if let Ok(mut seen) = finished_sink.lock() {
                seen.push((*payload).clone());
            }
        });

        assert!(!view.can_fetch_more(&[0]), "lazy loading is off by default");
        view.set("lazy_loading", CapabilityValue::Bool(true)).expect("writable");
        assert!(view.lazy_loading());
        assert!(view.can_fetch_more(&[0]));

        assert_eq!(view.node_count(), 1);
        assert!(view.expand(0), "a fetchable branch is expandable before its children exist");
        assert_eq!(view.node_count(), 3, "the fetch revealed the two children");
        assert_eq!(*started.lock().expect("started lock"), vec![vec![0]]);
        assert_eq!(*finished.lock().expect("finished lock"), vec![(vec![0], true)]);
        assert!(!view.loading_indicator_visible());
        assert!(view.loading_paths().is_empty());

        assert_eq!(view.fetch_timeout_ms(), 5000);
        view.set("fetch_timeout_ms", CapabilityValue::UInt(2500)).expect("writable");
        assert_eq!(view.fetch_timeout_ms(), 2500);
    }

    /// F-P10: the inverse projection maps every node to its row without a scan.
    #[test]
    fn projection_positions_are_maintained_with_the_projection() {
        let mut view = TreeView::new(Rect::new(0, 0, 200, 200));
        view.set_model(Arc::new(VecTreeModel::new(
            (0..120).map(|n| format!("node {n}")).collect(),
        )));
        assert_eq!(view.projection_positions.len(), 120);
        assert_eq!(view.visible_row_of(50), Some(50));
        assert_eq!(view.visible_position_of(119), Some(119));

        // A row that exists but is hidden has no position.
        assert_eq!(view.visible_row_of(500), None);
    }

    /// V-10: extended selection walks the visible range and ctrl toggles one node.
    #[test]
    fn multi_select_ranges_and_toggles_nodes() {
        use crate::shortcut::Modifiers;

        let mut view = TreeView::new(Rect::new(0, 0, 200, 200));
        view.set_model(Arc::new(VecTreeModel::new((0..6).map(|n| format!("node {n}")).collect())));
        view.set_selection_mode(SelectionMode::Extended);
        assert_eq!(view.selection_mode(), SelectionMode::Extended);

        assert!(view.select_node(1));
        assert!(view.select_node_with_modifiers(3, Modifiers::SHIFT));
        assert_eq!(view.selected_nodes(), vec![1, 2, 3]);

        assert!(view.select_node_with_modifiers(2, Modifiers::CTRL));
        assert_eq!(view.selected_nodes(), vec![1, 3]);
    }

    /// V-11: `scroll_to_node` and `ensure_visible` move the viewport to a node.
    #[test]
    fn scroll_to_node_and_ensure_visible_locate_rows() {
        let mut view = TreeView::new(Rect::new(0, 0, 200, 200));
        view.set_model(Arc::new(VecTreeModel::new(
            (0..100).map(|n| format!("node {n}")).collect(),
        )));

        assert!(view.scroll_to_node(60));
        assert_eq!(view.scroll_row(), 60);

        assert!(view.ensure_visible(0));
        assert_eq!(view.scroll_row(), 0, "scrolling up to the node");

        let page = (view.base.geometry().height / TREE_ROW_HEIGHT as u32) as usize;
        assert!(view.ensure_visible(page + 5));
        assert_eq!(view.scroll_row(), 6, "moved by exactly the overflow");
        assert!(!view.scroll_to_node(10_000), "there is no such node");

        assert!(view.auto_scroll_to_expanded());
        view.set_auto_scroll_to_expanded(false);
        assert!(!view.auto_scroll_to_expanded());
    }

    /// V-010: the paint cursor starts at the first visible row instead of materialising a
    /// prefix and skipping it.
    #[test]
    fn the_visible_slice_starts_at_the_scroll_position() {
        let mut view = TreeView::new(Rect::new(0, 0, 200, 200));
        view.set_model(Arc::new(VecTreeModel::new(
            (0..1_000).map(|n| format!("node {n}")).collect(),
        )));

        let rows = view.visible_indices_from(900, 4);
        assert_eq!(rows, vec![900, 901, 902, 903], "only the viewport rows are produced");
        assert_eq!(view.node_path(900), Some("node 900".to_string()));
        assert_eq!(view.visible_indices_from(998, 10), vec![998, 999], "the tail is clamped");
    }

    /// V-016: indent guide lines are opt-in and change the painted output.
    #[test]
    fn indent_lines_are_off_by_default_and_paint_when_enabled() {
        struct Nested;
        impl TreeModel for Nested {
            fn node_count(&self) -> usize {
                2
            }
            fn node_path(&self, index: usize) -> Option<String> {
                Some(std::format!("n{index}"))
            }
            fn node(&self, index: usize) -> Option<NodeView> {
                match index {
                    0 => Some(NodeView {
                        label: "root".to_string(),
                        depth: 0,
                        has_children: true,
                        expanded: true,
                    }),
                    1 => Some(NodeView {
                        label: "child".to_string(),
                        depth: 1,
                        has_children: false,
                        expanded: false,
                    }),
                    _ => None,
                }
            }
        }

        let mut view = TreeView::new(Rect::new(0, 0, 200, 100));
        view.set_model(Arc::new(Nested));
        assert!(!view.indent_lines());
        let plain = crate::widget::svg::render_to_svg(&mut view);
        view.set_indent_lines(true);
        assert!(view.indent_lines());
        let guided = crate::widget::svg::render_to_svg(&mut view);
        assert_ne!(plain, guided, "the guides are actually painted");
    }

    /// V-016: a column descriptor array paints the model's extra columns.
    #[test]
    fn column_descriptors_paint_extra_cells() {
        struct MultiColumn;
        impl TreeModel for MultiColumn {
            fn node_count(&self) -> usize {
                2
            }
            fn node_path(&self, index: usize) -> Option<String> {
                Some(std::format!("label{index}"))
            }
            fn node_column_text(&self, index: usize, column: usize) -> Option<String> {
                match column {
                    0 => self.node_path(index),
                    _ => Some(std::format!("extra{index}")),
                }
            }
        }

        let mut view = TreeView::new(Rect::new(0, 0, 300, 100));
        view.set_model(Arc::new(MultiColumn));
        let single = crate::widget::svg::render_to_svg(&mut view);
        let single_boxes = crate::widget::svg::text_ink_boxes(&single).len();

        view.set_columns(vec![TreeColumn::new("Name", 120), TreeColumn::new("Extra", 120)]);
        assert_eq!(view.columns().len(), 2);
        let multi = crate::widget::svg::render_to_svg(&mut view);
        let multi_boxes = crate::widget::svg::text_ink_boxes(&multi).len();
        assert_eq!(multi_boxes, single_boxes * 2, "one extra text run per node");

        view.set_columns(Vec::new());
        assert!(view.columns().is_empty());
    }

    /// V-016: a same-control drag reports a move request through the ghost/drop path.
    #[test]
    fn a_dropped_row_reports_a_move_request() {
        let mut view = TreeView::new(Rect::new(0, 0, 200, 100));
        view.set_model(Arc::new(VecTreeModel::new(vec![
            "a".to_string(),
            "b".to_string(),
            "c".to_string(),
        ])));

        let source = view.node_row_rect(0).expect("row 0");
        let payload = view
            .drag_payload(Point::new(source.x + 5, source.y + 5))
            .expect("a payload from row 0")
            .with_source(view.base().id());
        assert!(view.can_accept(&payload));

        let requested = Arc::new(std::sync::Mutex::new(Vec::<(usize, usize)>::new()));
        let sink = Arc::clone(&requested);
        view.row_move_requested.connect(move |pair| {
            sink.lock().expect("request sink").push(*pair);
        });

        let target = view.node_row_rect(2).expect("row 2");
        assert_eq!(
            view.on_drop(&payload, Point::new(target.x + 5, target.y + 5)),
            DropEffect::Move
        );
        assert_eq!(*requested.lock().expect("request sink"), vec![(0, 2)]);
    }
}
