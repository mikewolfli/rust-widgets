// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Tree view widget.
use crate::core::Color;
use crate::core::HorizontalAlignment;
use crate::core::Point;
use crate::core::Rect;
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::widget::capability::coercion::expect_usize;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{effective_font, ControlMetrics};
use crate::widget::view_widgets::table_widget::ItemDelegate;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::collections::HashMap;
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
}
impl TreeView {
    /// Creates an empty tree view.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::TreeView, geometry, "TreeView"),
            model: None,
            model_connection_scope: ConnectionScope::new(),
            selected_node: None,
            focused_node: None,
            hovered_node: None,
            expanded_override: HashMap::new(),
            delegate: None,
            editing: None,
            selection_changed: Signal1::new(),
            focused_node_changed: Signal1::new(),
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
            data_changed.connect_scoped(&self.model_connection_scope, move || {
                redraw.emit();
                layout.emit();
            });
        }
        self.model = Some(model);
        self.expanded_override.clear();
        self.editing = None;
        self.normalize_projection_state();
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
        let Some(model) = self.model.as_ref() else {
            return Vec::new();
        };
        let count = model.node_count();
        let mut rows = Vec::with_capacity(count);
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
            }
            if node.has_children {
                ancestors.push((node.depth, self.is_expanded(index)));
            }
        }
        rows
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
    /// cannot disagree about what "toggle" means.
    pub fn toggle_expanded(&mut self, index: usize) -> bool {
        let Some(model_index) = self.visible_indices().get(index).copied() else {
            return false;
        };
        self.set_expanded(index, !self.is_expanded(model_index))
    }

    fn set_expanded(&mut self, visible_index: usize, expanded: bool) -> bool {
        let Some(model_index) = self.visible_indices().get(visible_index).copied() else {
            return false;
        };
        let has_children = self
            .model
            .as_ref()
            .and_then(|model| model.node(model_index))
            .map(|node| node.has_children)
            .unwrap_or(false);
        if !has_children {
            return false;
        }
        if self.is_expanded(model_index) == expanded {
            return true;
        }
        self.expanded_override.insert(model_index, expanded);
        // A collapse can hide the selected or focused node; the projection state has to follow, the
        // same way it follows a model swap.
        self.normalize_projection_state();
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    /// Returns the current visible node count.
    pub fn node_count(&self) -> usize {
        self.visible_row_count()
    }

    /// Returns the label of the visible row `index`.
    pub fn node_path(&self, index: usize) -> Option<String> {
        let model_index = *self.visible_indices().get(index)?;
        self.model.as_ref()?.node(model_index).map(|node| node.label)
    }

    /// Returns the shape of the visible row `index`.
    ///
    /// The row's own view, so a host can ask the same questions the renderer does (is this row a
    /// branch, how deep is it) without recomputing the visible projection itself.
    pub fn node_view(&self, index: usize) -> Option<NodeView> {
        let model_index = *self.visible_indices().get(index)?;
        self.model.as_ref()?.node(model_index)
    }

    /// The **model** index a visible row names, if the row exists.
    pub fn model_index(&self, index: usize) -> Option<usize> {
        self.visible_indices().get(index).copied()
    }

    /// Selects a node by **visible** index.
    ///
    /// Repaints: the selected row's highlight is painted, so without the request the
    /// selection would only appear once some unrelated event repainted the control.
    pub fn select_node(&mut self, index: usize) -> bool {
        let Some(model_index) = self.visible_indices().get(index).copied() else {
            return false;
        };
        self.selected_node = Some(model_index);
        self.selection_changed.emit(index);
        self.set_focused_node(index);
        self.base.request_redraw();
        true
    }
    /// Clears node selection.
    ///
    /// Repaints: the highlight for the previously selected row is painted from this
    /// field, so clearing it has to mark damage.
    pub fn clear_selection(&mut self) {
        if self.selected_node.is_none() {
            return;
        }
        self.selected_node = None;
        self.base.request_redraw();
    }
    /// Sets focused node by **visible** index.
    ///
    /// Repaints: the focus ring is painted from this field.
    pub fn set_focused_node(&mut self, index: usize) -> bool {
        let Some(model_index) = self.visible_indices().get(index).copied() else {
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
        self.visible_indices().iter().position(|index| *index == focused)
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
        // `index as i32` narrowed a `usize` above `i32::MAX` to a negative row offset, and the
        // multiplication then wrapped — a row that scrolls nowhere near the control could map onto a
        // visible position. Reject an index that cannot be represented before multiplying; such an
        // index is far past any renderable row, so `None` is the honest answer.
        let row_offset = i32::try_from(index).ok()?;
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
        let index = ((pos.y - content.y) / TREE_ROW_HEIGHT).max(0) as usize;
        // `visible_row_count`, not `visible_indices().len()`: this runs on every pointer move, and
        // counting the projection does not need to materialise it.
        (index < self.visible_row_count()).then_some(index)
    }

    /// How many rows the visible projection has, without building the list.
    ///
    /// The same walk as [`Self::visible_indices`] with nothing pushed, for the readers that only
    /// need the bound: the hit test runs per pointer move, and a `Vec` per move would make a large
    /// tree's hover cost follow the document rather than the viewport.
    pub fn visible_row_count(&self) -> usize {
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
        self.visible_indices().iter().position(|index| *index == selected)
    }

    fn normalize_projection_state(&mut self) {
        let model_count = self.model.as_ref().map(|model| model.node_count()).unwrap_or(0);
        // Selection, focus and the open edit are **model** indices, so they are filtered against
        // the model's size — not against the visible row count, which a collapse can shrink without
        // the node having gone away.
        self.selected_node = self.selected_node.filter(|index| *index < model_count);
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
            "focused_node_changed" => Some(EventSignalRef::mapped(
                "focused_node_changed",
                &self.focused_node_changed,
                |v| match v {
                    Some(value) => CapabilityValue::UInt(*value as u64),
                    None => CapabilityValue::Null,
                },
            )),
            _ => None,
        }
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
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
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
            "expanded_focused_node",
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
        let theme = crate::style::resolved_theme_style("tree_view");
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

        // Draw background
        context.face_with_gradient(
            rect,
            surface,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );
        // Draw border
        context.draw_rect(rect, border);
        // Draw nodes from model. Each node is a band of the content box, never of the
        // control, and the label's line box is derived from the row rather than from
        // `y + item_height / 2` — which put the glyph box's top edge on the row's middle
        // line and left the first node pinned to y=0.
        if let Some(ref model) = self.model {
            // The **visible projection**, computed once for the whole frame: a row index here is
            // the same index `node_row_rect`, `node_at` and the selection speak, so the row that is
            // painted is the row a click hits.
            let rows = self.visible_indices();
            let font = effective_font(style);
            for (visible, &model_index) in rows.iter().enumerate() {
                // The row's own rectangle comes from the same derivation the hit test uses, so a
                // click cannot land on a row other than the one that was painted.
                let Some(row) = self.node_row_rect(visible) else {
                    break;
                };
                // Highlighting compares **model** indices: the selection is stable across an
                // expansion change, so the wash follows the node and not the row number.
                if Some(model_index) == self.focused_node {
                    context.fill_rect(row, focused_bg);
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
                        context.draw_line(a, b, ink);
                        context.draw_line(b, c, ink);
                        context.draw_line(c, a, ink);
                    }
                }
                if !node.label.is_empty() {
                    let cell = crate::core::Rect::new(
                        row.x + indent,
                        row.y,
                        row.width.saturating_sub(indent.max(0) as u32),
                        row.height,
                    );
                    context.draw_text_fitted(
                        context.text_line(cell, font),
                        &node.label,
                        font,
                        ink,
                        HorizontalAlignment::Left,
                    );
                }
            }
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
            crate::event::Event::MousePress { pos, button, .. } if *button == 1 => {
                // The same index derivation the draw uses, so the row a click selects is the row
                // the user pointed at.
                if let Some(index) = self.node_at(*pos) {
                    // A press on the disclosure glyph opens or closes the branch instead of just
                    // selecting it: that is the one gesture the glyph exists for, and testing it
                    // here (rather than in the label band) keeps the two hit areas from overlapping.
                    let on_glyph =
                        self.disclosure_rect(index).is_some_and(|glyph| point_in_rect(*pos, glyph));
                    self.select_node(index);
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
                    _ => false,
                };
                if changed {
                    self.base.request_redraw();
                }
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
}
