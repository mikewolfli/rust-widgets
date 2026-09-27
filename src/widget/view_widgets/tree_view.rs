// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Tree view widget.
use crate::core::Color;
use crate::core::HorizontalAlignment;
use crate::core::Rect;
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::widget::capability::coercion::expect_usize;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::ControlMetrics;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
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

/// A child node's indent, in pixels: 15.
const TREE_INDENT: i32 = 15;

/// Tree model abstraction for tree-like views.
pub trait TreeModel: Send + Sync {
    /// Number of nodes exposed by model.
    fn node_count(&self) -> usize;
    /// Node path by visible index, if present.
    fn node_path(&self, index: usize) -> Option<String>;
    /// Optional signal emitted when model data projection changes.
    fn data_changed_signal(&self) -> Option<&GenericSignal> {
        None
    }
}
/// In-memory tree model backed by a vector of strings.
pub struct VecTreeModel {
    nodes: Vec<String>,
    data_changed: GenericSignal,
}
impl VecTreeModel {
    /// Creates a new vector tree model.
    pub fn new(nodes: Vec<String>) -> Self {
        Self { nodes, data_changed: GenericSignal::new() }
    }
    /// Returns a reference to the data changed signal.
    pub fn data_changed_signal(&self) -> &GenericSignal {
        &self.data_changed
    }
    /// Appends a node to the model.
    pub fn append(&mut self, node: String) {
        self.nodes.push(node);
        self.data_changed.emit();
    }
    /// Removes a node at given index.
    pub fn remove(&mut self, index: usize) -> Option<String> {
        if index < self.nodes.len() {
            let node = self.nodes.remove(index);
            self.data_changed.emit();
            Some(node)
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
        self.nodes.get(index).cloned()
    }
    fn data_changed_signal(&self) -> Option<&GenericSignal> {
        Some(&self.data_changed)
    }
}
/// Tree view widget with optional external model binding.
pub struct TreeView {
    base: BaseWidget,
    /// Optional bound tree model.
    model: Option<Arc<dyn TreeModel>>,
    /// Scoped model-to-view signal subscriptions.
    model_connection_scope: ConnectionScope,
    /// View-side selected node index.
    selected_node: Option<usize>,
    /// View-side focused node index.
    focused_node: Option<usize>,
    /// The row the pointer is currently over, or `None` when it is between/outside the rows.
    ///
    /// Filled by `MouseMove` through the same [`Self::node_at`] the click uses, so the highlighted
    /// row is the row a click would affect.
    hovered_node: Option<usize>,
    /// Emitted when selected node changes.
    pub selection_changed: Signal1<usize>,
    /// Emitted when focused node changes.
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
            selection_changed: Signal1::new(),
            focused_node_changed: Signal1::new(),
        }
    }
    /// Binds an external tree model.
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
    /// Returns current visible node count.
    pub fn node_count(&self) -> usize {
        self.model.as_ref().map(|model| model.node_count()).unwrap_or(0)
    }
    /// Returns node path by visible index.
    pub fn node_path(&self, index: usize) -> Option<String> {
        self.model.as_ref().and_then(|model| model.node_path(index))
    }
    /// Selects a node by visible index.
    pub fn select_node(&mut self, index: usize) -> bool {
        if index < self.node_count() {
            self.selected_node = Some(index);
            self.selection_changed.emit(index);
            self.set_focused_node(index);
            true
        } else {
            false
        }
    }
    /// Clears node selection.
    pub fn clear_selection(&mut self) {
        self.selected_node = None;
    }
    /// Sets focused node by visible index.
    pub fn set_focused_node(&mut self, index: usize) -> bool {
        if index >= self.node_count() {
            return false;
        }
        if self.focused_node == Some(index) {
            return true;
        }
        self.focused_node = Some(index);
        self.focused_node_changed.emit(self.focused_node);
        true
    }
    /// Clears node focus.
    pub fn clear_focused_node(&mut self) {
        if self.focused_node.is_none() {
            return;
        }
        self.focused_node = None;
        self.focused_node_changed.emit(None);
    }
    /// Returns focused node index when present.
    pub fn focused_node(&self) -> Option<usize> {
        self.focused_node.filter(|index| *index < self.node_count())
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
        let y = content.y + TREE_ROW_HEIGHT * index as i32;
        // A row that would extend past the content box is not shown, matching the draw loop's own
        // bound so the row the pointer can hit is exactly the row that was painted.
        if y + TREE_ROW_HEIGHT > content.y + content.height as i32 {
            return None;
        }
        Some(Rect::new(content.x, y, content.width, TREE_ROW_HEIGHT as u32))
    }

    /// The node at a screen point, or `None` when the point is not over a painted row.
    pub fn node_at(&self, pos: crate::core::Point) -> Option<usize> {
        let content = ControlMetrics::band_inset(self.base.geometry(), TREE_INSET);
        if pos.y < content.y || pos.y >= content.y + content.height as i32 {
            return None;
        }
        let index = ((pos.y - content.y) / TREE_ROW_HEIGHT).max(0) as usize;
        (index < self.node_count()).then_some(index)
    }
    /// Returns selected node index if present.
    pub fn selected_node(&self) -> Option<usize> {
        self.selected_node.filter(|index| *index < self.node_count())
    }
    fn normalize_projection_state(&mut self) {
        let node_count = self.node_count();
        self.selected_node = self.selected_node.filter(|index| *index < node_count);
        self.focused_node = self.focused_node.filter(|index| *index < node_count);
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
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "has_model",
            "node_count",
            "focused_node",
            "selected_node",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `tree_view` publishes. Both are payload-free.
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
        let style = self.base.style().clone();
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
        context.face(
            rect,
            surface,
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
            let indent = TREE_INDENT;
            let node_count = model.node_count();
            let font = crate::core::Font::default();
            for i in 0..node_count {
                // The row's own rectangle comes from the same derivation the hit test uses, so a
                // click cannot land on a row other than the one that was painted.
                let Some(row) = self.node_row_rect(i) else {
                    break;
                };
                if Some(i) == self.focused_node {
                    context.fill_rect(row, focused_bg);
                } else if Some(i) == self.hovered_node {
                    // A weaker wash than the focus, from the same accent: a row the pointer is over
                    // is a *preview* of the row a click would focus.
                    context.fill_rect(row, surface.blend(&accent, 0.12));
                }
                if let Some(path) = model.node_path(i) {
                    if !path.is_empty() {
                        let cell = crate::core::Rect::new(
                            row.x + indent,
                            row.y,
                            row.width.saturating_sub(indent as u32),
                            row.height,
                        );
                        context.draw_text_fitted(
                            context.text_line(cell, &font),
                            &path,
                            &font,
                            ink,
                            HorizontalAlignment::Left,
                        );
                    }
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
            crate::event::Event::MousePress { pos, button } if *button == 1 => {
                // The same index derivation the draw uses, so the row a click selects is the row
                // the user pointed at.
                if let Some(index) = self.node_at(*pos) {
                    self.select_node(index);
                }
            }
            crate::event::Event::MouseMove { pos } => {
                let hovered = self.node_at(*pos);
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
        view.handle_event(&Event::MousePress { pos: centre, button: 1 });
        assert_eq!(
            view.selected_node(),
            Some(1),
            "a click inside node 1's own painted band must select node 1"
        );

        // And a click in the frame's own inset margin selects nothing rather than clamping to the
        // first node — the assertion that actually carries the defect (see the doc comment).
        let mut fresh = TreeView::new(Rect::new(0, 0, 120, 100));
        fresh.set_model(Arc::new(StaticTreeModel));
        fresh.handle_event(&Event::MousePress { pos: crate::core::Point::new(4, 0), button: 1 });
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
}
