//! Table view widget — deprecated type alias for `TableWidget`.
///
/// `TableView` was a thin wrapper around `TableWidget` that added no new behavior.
/// All usages should be replaced with `TableWidget` directly.
/// This file will be removed in a future version.
/// Deprecated — use `TableWidget` directly.
pub type TableView = crate::widget::view_widgets::table_widget::TableWidget;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Point, Rect};
    use crate::event::{Event, EventHandler};
    use crate::widget::svg::render_to_svg;
    use crate::widget::view_widgets::list_view::SelectionMode;
    use crate::widget::view_widgets::table_widget::{ItemDelegate, TableModel, TableWidget};
    use crate::widget::Widget;
    use crate::WidgetKind;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    /// A concrete TableModel with specified rows/cols and cell data.
    struct TestTableModel {
        rows: usize,
        cols: usize,
        /// Callback to check whether data was accessed.
        data_accessed: AtomicBool,
    }

    impl TestTableModel {
        fn new(rows: usize, cols: usize) -> Self {
            Self { rows, cols, data_accessed: AtomicBool::new(false) }
        }
    }

    impl TableModel for TestTableModel {
        fn row_count(&self) -> usize {
            self.rows
        }

        fn column_count(&self) -> usize {
            self.cols
        }

        fn data(&self, row: usize, column: usize) -> Option<String> {
            self.data_accessed.store(true, Ordering::SeqCst);
            Some(format!("r{}c{}", row, column))
        }
    }

    // ── 1. Creating default table view ──────────────────────────────

    #[test]
    fn test_default_creation() {
        let tv = TableView::new(Rect::new(0, 0, 400, 300));

        assert_eq!(tv.kind(), WidgetKind::Table);
        assert_eq!(tv.geometry(), Rect::new(0, 0, 400, 300));
        assert!(!tv.has_model());
        assert!(tv.model_ref().is_none());
        assert_eq!(tv.row_count(), 0);
        assert_eq!(tv.column_count(), 0);
        assert!(tv.selected_row().is_none());
        assert!(tv.selected_rows().is_empty());
        assert!(tv.focused_row().is_none());
        assert!(!tv.has_delegate());
        assert!(tv.delegate_ref().is_none());
        assert!(tv.is_visible());
        assert!(tv.is_enabled());
    }

    // ── 2. Setting columns ──────────────────────────────────────────

    #[test]
    fn test_set_columns() {
        let model = Arc::new(TestTableModel::new(3, 5));
        let mut tv = TableView::new(Rect::new(0, 0, 400, 300));

        tv.set_model(model);

        assert!(tv.has_model());
        assert_eq!(tv.column_count(), 5);

        // Column width overrides
        tv.set_column_width(0, 120);
        assert_eq!(tv.column_width(0), Some(120));
        assert_eq!(tv.column_width(99), None);
    }

    // ── 3. Adding rows (via model) ───────────────────────────────────

    #[test]
    fn test_rows_via_model() {
        let model = Arc::new(TestTableModel::new(4, 3));
        let mut tv = TableView::new(Rect::new(0, 0, 400, 300));

        assert_eq!(tv.row_count(), 0);

        tv.set_model(model);
        assert_eq!(tv.row_count(), 4);
    }

    // ── 4. Setting cell values ──────────────────────────────────────

    #[test]
    fn test_cell_values() {
        let model = Arc::new(TestTableModel::new(2, 2));
        let mut tv = TableView::new(Rect::new(0, 0, 400, 300));

        tv.set_model(model);

        assert_eq!(tv.item(0, 0), Some("r0c0".to_string()));
        assert_eq!(tv.item(0, 1), Some("r0c1".to_string()));
        assert_eq!(tv.item(1, 0), Some("r1c0".to_string()));
        assert_eq!(tv.item(1, 1), Some("r1c1".to_string()));
    }

    // ── 5. Getting cell values (out-of-bounds) ───────────────────────

    #[test]
    fn test_cell_values_out_of_bounds() {
        let model = Arc::new(TestTableModel::new(2, 3));
        let mut tv = TableView::new(Rect::new(0, 0, 400, 300));

        tv.set_model(model);

        // item() delegates to model.data() without row/col bounds checking
        assert_eq!(tv.item(5, 0), Some("r5c0".to_string()));
        assert_eq!(tv.item(0, 10), Some("r0c10".to_string()));
        assert_eq!(tv.item(0, 0), Some("r0c0".to_string()));

        // Without model, item returns None
        assert_eq!(TableView::new(Rect::new(0, 0, 100, 100)).item(0, 0), None);
    }

    // ── 6. Column count and row count ───────────────────────────────

    #[test]
    fn test_column_and_row_counts() {
        let mut tv = TableView::new(Rect::new(0, 0, 400, 300));

        assert_eq!(tv.row_count(), 0);
        assert_eq!(tv.column_count(), 0);

        let model = Arc::new(TestTableModel::new(10, 4));
        tv.set_model(model);

        assert_eq!(tv.row_count(), 10);
        assert_eq!(tv.column_count(), 4);
    }

    // ── 7. Selection mode ───────────────────────────────────────────

    #[test]
    fn test_selection_mode() {
        let mut tv = TableView::new(Rect::new(0, 0, 400, 300));

        // Default is Single
        assert_eq!(tv.selection_mode(), SelectionMode::Single);

        tv.set_selection_mode(SelectionMode::Multi);
        assert_eq!(tv.selection_mode(), SelectionMode::Multi);

        tv.set_selection_mode(SelectionMode::Extended);
        assert_eq!(tv.selection_mode(), SelectionMode::Extended);

        tv.set_selection_mode(SelectionMode::Single);
        assert_eq!(tv.selection_mode(), SelectionMode::Single);
    }

    // ── 8. Current/selected row ─────────────────────────────────────

    #[test]
    fn test_select_and_clear_row() {
        let model = Arc::new(TestTableModel::new(5, 3));
        let mut tv = TableView::new(Rect::new(0, 0, 400, 300));
        tv.set_model(model);

        assert!(tv.selected_row().is_none());
        assert!(tv.selected_rows().is_empty());
        assert!(tv.focused_row().is_none());

        // Select row 2
        assert!(tv.select_row(2));
        assert_eq!(tv.selected_row(), Some(2));
        assert_eq!(tv.focused_row(), Some(2));
        assert_eq!(tv.selected_rows(), vec![2]);

        // Select out of bounds
        assert!(!tv.select_row(99));
        assert_eq!(tv.selected_row(), Some(2)); // Unchanged

        // Clear selection
        tv.clear_selection();
        assert!(tv.selected_row().is_none());
        assert!(tv.selected_rows().is_empty());
    }

    // ── 9. Focused row management ───────────────────────────────────

    #[test]
    fn test_focused_row() {
        let model = Arc::new(TestTableModel::new(5, 3));
        let mut tv = TableView::new(Rect::new(0, 0, 400, 300));
        tv.set_model(model);

        assert!(tv.focused_row().is_none());

        assert!(tv.set_focused_row(3));
        assert_eq!(tv.focused_row(), Some(3));

        // Set again to same row
        assert!(tv.set_focused_row(3));
        assert_eq!(tv.focused_row(), Some(3));

        // Out of bounds
        assert!(!tv.set_focused_row(99));
        assert_eq!(tv.focused_row(), Some(3)); // Unchanged

        tv.clear_focused_row();
        assert!(tv.focused_row().is_none());

        // Double clear is safe
        tv.clear_focused_row();
        assert!(tv.focused_row().is_none());
    }

    // ── 10. Signal accessors ────────────────────────────────────────

    #[test]
    fn test_selection_changed_signal() {
        let model = Arc::new(TestTableModel::new(5, 3));
        let mut tv = TableView::new(Rect::new(0, 0, 400, 300));
        tv.set_model(model);

        let captured = Arc::new(Mutex::new(None::<usize>));
        tv.selection_changed.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<usize>| {
                *captured.lock().unwrap() = Some(*val);
            }
        });

        tv.select_row(1);
        assert_eq!(*captured.lock().unwrap(), Some(1));

        tv.select_row(3);
        assert_eq!(*captured.lock().unwrap(), Some(3));
    }

    #[test]
    fn test_focused_row_changed_signal() {
        let model = Arc::new(TestTableModel::new(5, 3));
        let mut tv = TableView::new(Rect::new(0, 0, 400, 300));
        tv.set_model(model);

        let captured = Arc::new(Mutex::new(None));
        tv.focused_row_changed.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<Option<usize>>| {
                *captured.lock().unwrap() = *val;
            }
        });

        tv.set_focused_row(2);
        assert_eq!(*captured.lock().unwrap(), Some(2));

        tv.clear_focused_row();
        assert_eq!(*captured.lock().unwrap(), None);
    }

    // ── 11. Geometry delegation ─────────────────────────────────────

    #[test]
    fn test_geometry_delegation() {
        let mut tv = TableView::new(Rect::new(10, 20, 400, 300));

        assert_eq!(tv.geometry(), Rect::new(10, 20, 400, 300));

        tv.set_geometry(Rect::new(0, 0, 500, 400));
        assert_eq!(tv.geometry(), Rect::new(0, 0, 500, 400));
        assert_eq!(tv.rect(), Rect::new(0, 0, 500, 400));
        assert_eq!(tv.position(), Point::new(0, 0));
        assert_eq!(tv.size(), crate::core::Size::new(500, 400));
    }

    // ── 12. Widget ID and kind ──────────────────────────────────────

    #[test]
    fn test_widget_id_and_kind() {
        let tv = TableView::new(Rect::new(0, 0, 400, 300));

        assert_eq!(tv.kind(), WidgetKind::Table);
        assert_ne!(tv.id(), 0);

        let tv2 = TableView::new(Rect::new(0, 0, 200, 100));
        assert_ne!(tv.id(), tv2.id());
    }

    // ── 13. SVG output ──────────────────────────────────────────────

    #[test]
    fn test_svg_output() {
        let mut tv = TableView::new(Rect::new(0, 0, 200, 100));

        // Without model
        let svg = render_to_svg(&mut tv);
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(svg.contains("width=\"200\""));
        assert!(svg.contains("height=\"100\""));

        // With model
        let model = Arc::new(TestTableModel::new(2, 3));
        let mut tv2 = TableView::new(Rect::new(0, 0, 300, 150));
        tv2.set_model(model);
        let svg2 = render_to_svg(&mut tv2);
        assert!(svg2.starts_with("<svg"));
    }

    // ── 14. Disabled state blocking ─────────────────────────────────

    #[test]
    fn test_disabled_state_blocks_events() {
        let model = Arc::new(TestTableModel::new(5, 3));
        let mut tv = TableView::new(Rect::new(0, 0, 400, 300));
        tv.set_model(model);

        tv.set_enabled(false);
        assert!(!tv.is_enabled());

        let captured = Arc::new(Mutex::new(None::<usize>));
        tv.selection_changed.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<usize>| {
                *captured.lock().unwrap() = Some(*val);
            }
        });

        // Mouse press inside widget should not trigger selection when disabled
        tv.handle_event(&Event::MousePress { pos: Point::new(10, 15), button: 1 });
        assert!(captured.lock().unwrap().is_none());

        // Re-enable and verify it works
        tv.set_enabled(true);
        tv.handle_event(&Event::MousePress { pos: Point::new(10, 15), button: 1 });
        assert_eq!(*captured.lock().unwrap(), Some(0));
    }

    // ── 15. Row height overrides ───────────────────────────────────

    #[test]
    fn test_row_height_overrides() {
        let mut tv = TableView::new(Rect::new(0, 0, 400, 300));

        assert_eq!(tv.row_height(0), None);
        assert_eq!(tv.row_height(99), None);

        tv.set_row_height(1, 28);
        assert_eq!(tv.row_height(1), Some(28));

        tv.set_row_height(5, 32);
        assert_eq!(tv.row_height(5), Some(32));
        assert_eq!(tv.row_height(1), Some(28));
    }

    // ── 16. Item delegate ───────────────────────────────────────────

    #[test]
    fn test_item_delegate() {
        struct TestDelegate;

        impl ItemDelegate for TestDelegate {
            fn create_editor(
                &self,
                _parent: &mut crate::widget::BaseWidget,
                _row: usize,
                _column: usize,
            ) -> Option<Box<dyn Widget>> {
                None
            }

            fn set_editor_data(&self, _editor: &mut dyn Widget, _row: usize, _column: usize) {}

            fn get_editor_data(
                &self,
                _editor: &dyn Widget,
                _row: usize,
                _column: usize,
            ) -> Option<String> {
                None
            }
        }

        let mut tv = TableView::new(Rect::new(0, 0, 400, 300));
        assert!(!tv.has_delegate());
        assert!(tv.delegate_ref().is_none());

        tv.set_delegate(Arc::new(TestDelegate));
        assert!(tv.has_delegate());
        assert!(tv.delegate_ref().is_some());
    }
}
