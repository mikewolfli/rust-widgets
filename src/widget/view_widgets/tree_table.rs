// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! TreeTable widget combining hierarchical rows with table columns.

use std::collections::HashSet;
use std::sync::Arc;

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::Event;
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::widget::capability::coercion::expect_usize;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
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
    /// Optional signal emitted when model projection changes.
    fn data_changed_signal(&self) -> Option<&GenericSignal> {
        None
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
    /// Emits visible row count changes.
    pub projection_changed: Signal1<usize>,
    /// Emits selected visible row index changes.
    pub selection_changed: Signal1<Option<usize>>,
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
            projection_changed: Signal1::new(),
            selection_changed: Signal1::new(),
        }
    }

    /// Binds model and rebuilds visible projection.
    pub fn set_model(&mut self, model: Arc<dyn TreeTableModel>) {
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
        self.expanded_paths.clear();
        self.selected_row = None;
        self.rebuild_projection();
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
        self.row_height = row_height.max(1);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns row height.
    pub fn row_height(&self) -> u32 {
        self.row_height
    }

    /// Sets fixed column width.
    pub fn set_column_width(&mut self, column_width: u32) {
        self.column_width = column_width.max(1);
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

    /// Expands row when it has children.
    pub fn expand_row(&mut self, row: usize) -> bool {
        let Some(path) = self.row_path(row).map(ToOwned::to_owned) else {
            return false;
        };
        if !self.path_has_children(&path) {
            return false;
        }
        if self.expanded_paths.insert(path) {
            self.rebuild_projection();
            self.base.request_layout();
            self.base.request_redraw();
        }
        true
    }

    /// Collapses row.
    pub fn collapse_row(&mut self, row: usize) -> bool {
        let Some(path) = self.row_path(row).map(ToOwned::to_owned) else {
            return false;
        };
        if self.expanded_paths.remove(&path) {
            self.expanded_paths.retain(|expanded| !Self::is_descendant_of(expanded, &path));
            self.rebuild_projection();
            self.base.request_layout();
            self.base.request_redraw();
        }
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
        let mut next = Vec::new();
        let Some(model) = self.model.as_ref() else {
            self.visible_rows = next;
            self.selected_row = None;
            self.projection_changed.emit(0);
            return;
        };

        for root in 0..model.root_count() {
            let mut path = vec![root];
            self.flatten_path(model.as_ref(), &mut next, &mut path, 0);
        }

        self.visible_rows = next;
        self.selected_row = self.selected_row.filter(|row| *row < self.visible_rows.len());
        self.projection_changed.emit(self.visible_rows.len());
    }

    fn flatten_path(
        &self,
        model: &dyn TreeTableModel,
        acc: &mut Vec<VisibleRow>,
        path: &mut Vec<usize>,
        depth: usize,
    ) {
        acc.push(VisibleRow { path: path.clone(), depth });

        if !self.expanded_paths.contains(path) {
            return;
        }

        let child_count = model.child_count(path);
        for child_index in 0..child_count {
            path.push(child_index);
            self.flatten_path(model, acc, path, depth + 1);
            let _ = path.pop();
        }
    }

    fn row_at(&self, y: i32) -> Option<usize> {
        let rect = self.base.geometry();
        if y < rect.y || y >= rect.y + rect.height as i32 {
            return None;
        }
        let local = (y - rect.y).max(0) as usize;
        let index = local / self.row_height as usize;
        (index < self.visible_rows.len()).then_some(index)
    }

    fn is_descendant_of(path: &[usize], ancestor: &[usize]) -> bool {
        path.len() > ancestor.len() && path[..ancestor.len()] == *ancestor
    }
}

impl Widget for TreeTable {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(400, 300)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
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
        context.fill_rect(rect, Color::rgb(255, 255, 255));
        context.draw_rect(rect, Color::rgb(200, 200, 200));

        if self.visible_rows.is_empty() {
            return;
        }

        let columns = self.column_count();
        if columns == 0 {
            return;
        }

        let row_h = self.row_height as i32;
        let col_w = self.column_width as i32;

        for row in 0..self.visible_rows.len() {
            let y = rect.y + row as i32 * row_h;
            if y >= rect.y + rect.height as i32 {
                break;
            }

            if self.selected_row == Some(row) {
                context.fill_rect(
                    Rect::new(rect.x, y, rect.width, self.row_height),
                    Color::rgb(210, 230, 255),
                );
            }

            for col in 0..columns {
                let x = rect.x + col as i32 * col_w;
                if x >= rect.x + rect.width as i32 {
                    break;
                }

                context.draw_rect(
                    Rect::new(x, y, self.column_width, self.row_height),
                    Color::rgb(230, 230, 230),
                );

                if let Some(text) = self.item(row, col) {
                    let indent =
                        if col == 0 { self.row_depth(row).unwrap_or(0) as i32 * 14 } else { 0 };
                    context.draw_text(
                        Point::new(x + 4 + indent, y + row_h / 2),
                        &text,
                        &Font::default(),
                        Color::rgb(0, 0, 0),
                        HorizontalAlignment::Left,
                    );
                }
            }
        }
    }
}

impl crate::event::EventHandler for TreeTable {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button } if *button == 1 => {
                if let Some(row) = self.row_at(pos.y) {
                    let _ = self.select_row(row);
                }
            }
            Event::MousePress { pos, button } if *button == 2 => {
                if let Some(row) = self.row_at(pos.y) {
                    let _ = self.toggle_row_expanded(row);
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
}
