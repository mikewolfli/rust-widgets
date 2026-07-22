//! Tree view widget.
use crate::core::{HorizontalAlignment};
use crate::core::Rect;
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use std::sync::Arc;
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
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(200, 200)
    }
}

impl Draw for TreeView {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        use crate::core::Color;
        // Draw background
        context.fill_rect(rect, Color::rgb(255, 255, 255));
        // Draw border
        context.draw_rect(rect, Color::rgb(200, 200, 200));
        // Draw nodes from model
        if let Some(ref model) = self.model {
            let item_height = 20;
            let indent = 15;
            let node_count = model.node_count();
            for i in 0..node_count {
                let y = rect.y + item_height * i as i32;
                if y + item_height > rect.y + rect.height as i32 {
                    break;
                }
                if Some(i) == self.focused_node {
                    context.fill_rect(
                        crate::core::Rect::new(rect.x, y, rect.width, item_height as u32),
                        Color::rgb(200, 220, 255),
                    );
                }
                if let Some(path) = model.node_path(i) {
                    context.draw_text(
                        crate::core::Point::new(rect.x + indent, y + item_height / 2),
                        &path,
                        &crate::core::Font::default(),
                        Color::rgb(0, 0, 0),
                        HorizontalAlignment::Left,
                    );
                }
            }
        }
    }
}
impl crate::event::EventHandler for TreeView {
    fn handle_event(&mut self, event: &crate::event::Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            crate::event::Event::MousePress { pos, button } if *button == 1 => {
                let rect = self.base.geometry();
                let item_height = 20;
                if pos.y >= rect.y {
                    let index = ((pos.y - rect.y) / item_height) as usize;
                    if index < self.node_count() {
                        self.select_node(index);
                    }
                }
            }
            #[cfg(feature = "touch")]
            crate::event::Event::Tap { pos } => {
                let rect = self.base.geometry();
                let item_height = 20;
                if pos.y >= rect.y {
                    let index = ((pos.y - rect.y) / item_height) as usize;
                    if index < self.node_count() {
                        self.select_node(index);
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
}
