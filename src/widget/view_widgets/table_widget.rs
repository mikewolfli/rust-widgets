//! Table widget.
use crate::core::Rect;
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use std::collections::HashMap;
use std::sync::Arc;
/// Table model abstraction for table-like views.
pub trait TableModel: Send + Sync {
    /// Number of rows exposed by model.
    fn row_count(&self) -> usize;
    /// Number of columns exposed by model.
    fn column_count(&self) -> usize;
    /// Data for row and column index, if present.
    fn data(&self, row: usize, column: usize) -> Option<String>;
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
    /// Explicit column width overrides in logical pixels.
    column_widths: HashMap<usize, u32>,
    /// Explicit row height overrides in logical pixels.
    row_heights: HashMap<usize, u32>,
    /// Optional display/editor delegate.
    delegate: Option<Arc<dyn ItemDelegate>>,
    /// Emitted when selected row changes.
    pub selection_changed: Signal1<usize>,
    /// Emitted when focused row changes.
    pub focused_row_changed: Signal1<Option<usize>>,
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
            column_widths: HashMap::new(),
            row_heights: HashMap::new(),
            delegate: None,
            selection_changed: Signal1::new(),
            focused_row_changed: Signal1::new(),
        }
    }
    /// Binds an external table model.
    pub fn set_model(&mut self, model: Arc<dyn TableModel>) {
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
    /// Returns the bound table model, if present.
    pub fn model_ref(&self) -> Option<&Arc<dyn TableModel>> {
        self.model.as_ref()
    }
    /// Returns visible row count.
    pub fn row_count(&self) -> usize {
        self.model.as_ref().map(|m| m.row_count()).unwrap_or(0)
    }
    /// Returns visible column count.
    pub fn column_count(&self) -> usize {
        self.model.as_ref().map(|m| m.column_count()).unwrap_or(0)
    }
    /// Returns item text by row and column index.
    pub fn item(&self, row: usize, column: usize) -> Option<String> {
        self.model.as_ref().and_then(|m| m.data(row, column))
    }
    /// Select one row in the current view projection.
    pub fn select_row(&mut self, row: usize) -> bool {
        if row < self.row_count() {
            self.selection.select_row(row);
            self.selection_changed.emit(row);
            self.set_focused_row(row);
            true
        } else {
            false
        }
    }
    /// Clear current row selection.
    pub fn clear_selection(&mut self) {
        self.selection.clear();
    }
    /// Sets focused row in current projection.
    pub fn set_focused_row(&mut self, row: usize) -> bool {
        if row >= self.row_count() {
            return false;
        }
        if self.focused_row == Some(row) {
            return true;
        }
        self.focused_row = Some(row);
        self.focused_row_changed.emit(self.focused_row);
        true
    }
    /// Clears focused row.
    pub fn clear_focused_row(&mut self) {
        if self.focused_row.is_none() {
            return;
        }
        self.focused_row = None;
        self.focused_row_changed.emit(None);
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
    /// Returns whether a delegate is currently bound.
    pub fn has_delegate(&self) -> bool {
        self.delegate.is_some()
    }
    /// Returns the bound item delegate, if present.
    pub fn delegate_ref(&self) -> Option<&Arc<dyn ItemDelegate>> {
        self.delegate.as_ref()
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
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl Draw for TableWidget {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        use crate::core::Color;
        // Draw background
        context.fill_rect(rect, Color::from_rgb(255, 255, 255));
        // Draw border
        context.draw_rect(rect, Color::from_rgb(200, 200, 200));
        // Draw grid from model
        if let Some(ref model) = self.model {
            let row_h = 20;
            let col_w = if model.column_count() > 0 {
                (rect.width / model.column_count() as u32).max(40)
            } else {
                rect.width
            };
            let row_count = model.row_count();
            let col_count = model.column_count();
            let current_row = self.focused_row;
            for r in 0..row_count {
                let y = rect.y + row_h * r as i32;
                if y + row_h > rect.y + rect.height as i32 {
                    break;
                }
                if Some(r) == current_row {
                    context.fill_rect(
                        crate::core::Rect::new(rect.x, y, rect.width, row_h as u32),
                        Color::from_rgb(200, 220, 255),
                    );
                }
                for c in 0..col_count {
                    let x = rect.x + (col_w as i32) * c as i32;
                    if let Some(text) = model.data(r, c) {
                        context.draw_text(
                            crate::core::Point::new(x + 2, y + row_h / 2),
                            &text,
                            &crate::core::Font::default(),
                            Color::from_rgb(0, 0, 0),
                        );
                    }
                    // Draw column separator
                    if c < col_count - 1 {
                        context.draw_line(
                            crate::core::Point::new(x + col_w as i32, y),
                            crate::core::Point::new(x + col_w as i32, y + row_h),
                            Color::from_rgb(220, 220, 220),
                        );
                    }
                }
                // Draw row separator
                if r < row_count - 1 {
                    context.draw_line(
                        crate::core::Point::new(rect.x, y + row_h),
                        crate::core::Point::new(rect.x + rect.width as i32, y + row_h),
                        Color::from_rgb(220, 220, 220),
                    );
                }
            }
        }
    }
}
impl crate::event::EventHandler for TableWidget {
    fn handle_event(&mut self, event: &crate::event::Event) {
        if !self.base.is_enabled() {
            return;
        }
        if let crate::event::Event::MousePress { pos, button } = event {
            if *button == 1 {
                let rect = self.base.geometry();
                let row_h = 20;
                if pos.y >= rect.y {
                    let index = ((pos.y - rect.y) / row_h) as usize;
                    if index < self.row_count() {
                        self.select_row(index);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;
    use crate::event::types::EventHandler;
    use std::sync::Arc;
    use std::sync::Mutex;

    struct StaticTableModel;

    impl TableModel for StaticTableModel {
        fn row_count(&self) -> usize {
            2
        }

        fn column_count(&self) -> usize {
            2
        }

        fn data(&self, row: usize, column: usize) -> Option<String> {
            match (row, column) {
                (0, 0) => Some("r0c0".to_string()),
                (0, 1) => Some("r0c1".to_string()),
                (1, 0) => Some("r1c0".to_string()),
                (1, 1) => Some("r1c1".to_string()),
                _ => None,
            }
        }
    }

    struct NoopDelegate;

    impl ItemDelegate for NoopDelegate {
        fn create_editor(
            &self,
            _parent: &mut BaseWidget,
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

    #[test]
    fn table_widget_model_delegate_and_size_accessors() {
        let mut table = TableWidget::new(Rect::new(0, 0, 200, 120));

        assert!(!table.has_model());
        assert!(table.model_ref().is_none());
        assert!(!table.has_delegate());
        assert!(table.delegate_ref().is_none());

        table.set_model(Arc::new(StaticTableModel));
        assert!(table.has_model());
        assert!(table.model_ref().is_some());
        assert_eq!(table.item(99, 0), None);

        table.set_column_width(2, 140);
        table.set_row_height(1, 28);
        assert_eq!(table.column_width(2), Some(140));
        assert_eq!(table.row_height(1), Some(28));
        assert_eq!(table.column_width(999), None);
        assert_eq!(table.row_height(999), None);

        table.set_delegate(Arc::new(NoopDelegate));
        assert!(table.has_delegate());
        assert!(table.delegate_ref().is_some());
    }

    // ── Tests moved from deprecated `TableView` alias ────────────────

    struct TestTableModel {
        rows: usize,
        cols: usize,
        data_accessed: std::sync::atomic::AtomicBool,
    }

    impl TestTableModel {
        fn new(rows: usize, cols: usize) -> Self {
            Self { rows, cols, data_accessed: std::sync::atomic::AtomicBool::new(false) }
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
            self.data_accessed.store(true, std::sync::atomic::Ordering::SeqCst);
            Some(format!("r{}c{}", row, column))
        }
    }

    #[test]
    fn test_default_creation() {
        let tv = TableWidget::new(Rect::new(0, 0, 400, 300));

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

    #[test]
    fn test_set_columns() {
        let model = Arc::new(TestTableModel::new(3, 5));
        let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));

        tv.set_model(model);

        assert!(tv.has_model());
        assert_eq!(tv.column_count(), 5);

        tv.set_column_width(0, 120);
        assert_eq!(tv.column_width(0), Some(120));
        assert_eq!(tv.column_width(99), None);
    }

    #[test]
    fn test_rows_via_model() {
        let model = Arc::new(TestTableModel::new(4, 3));
        let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));

        assert_eq!(tv.row_count(), 0);

        tv.set_model(model);
        assert_eq!(tv.row_count(), 4);
    }

    #[test]
    fn test_cell_values() {
        let model = Arc::new(TestTableModel::new(2, 2));
        let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));

        tv.set_model(model);

        assert_eq!(tv.item(0, 0), Some("r0c0".to_string()));
        assert_eq!(tv.item(0, 1), Some("r0c1".to_string()));
        assert_eq!(tv.item(1, 0), Some("r1c0".to_string()));
        assert_eq!(tv.item(1, 1), Some("r1c1".to_string()));
    }

    #[test]
    fn test_cell_values_out_of_bounds() {
        let model = Arc::new(TestTableModel::new(2, 3));
        let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));

        tv.set_model(model);

        assert_eq!(tv.item(5, 0), Some("r5c0".to_string()));
        assert_eq!(tv.item(0, 10), Some("r0c10".to_string()));
        assert_eq!(tv.item(0, 0), Some("r0c0".to_string()));

        assert_eq!(TableWidget::new(Rect::new(0, 0, 100, 100)).item(0, 0), None);
    }

    #[test]
    fn test_column_and_row_counts() {
        let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));

        assert_eq!(tv.row_count(), 0);
        assert_eq!(tv.column_count(), 0);

        let model = Arc::new(TestTableModel::new(10, 4));
        tv.set_model(model);

        assert_eq!(tv.row_count(), 10);
        assert_eq!(tv.column_count(), 4);
    }

    #[test]
    fn test_selection_mode() {
        let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));

        assert_eq!(
            tv.selection_mode(),
            crate::widget::view_widgets::list_view::SelectionMode::Single
        );

        tv.set_selection_mode(crate::widget::view_widgets::list_view::SelectionMode::Multi);
        assert_eq!(
            tv.selection_mode(),
            crate::widget::view_widgets::list_view::SelectionMode::Multi
        );

        tv.set_selection_mode(crate::widget::view_widgets::list_view::SelectionMode::Extended);
        assert_eq!(
            tv.selection_mode(),
            crate::widget::view_widgets::list_view::SelectionMode::Extended
        );

        tv.set_selection_mode(crate::widget::view_widgets::list_view::SelectionMode::Single);
        assert_eq!(
            tv.selection_mode(),
            crate::widget::view_widgets::list_view::SelectionMode::Single
        );
    }

    #[test]
    fn test_select_and_clear_row() {
        let model = Arc::new(TestTableModel::new(5, 3));
        let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));
        tv.set_model(model);

        assert!(tv.selected_row().is_none());
        assert!(tv.selected_rows().is_empty());
        assert!(tv.focused_row().is_none());

        assert!(tv.select_row(2));
        assert_eq!(tv.selected_row(), Some(2));
        assert_eq!(tv.focused_row(), Some(2));
        assert_eq!(tv.selected_rows(), vec![2]);

        assert!(!tv.select_row(99));
        assert_eq!(tv.selected_row(), Some(2));

        tv.clear_selection();
        assert!(tv.selected_row().is_none());
        assert!(tv.selected_rows().is_empty());
    }

    #[test]
    fn test_focused_row() {
        let model = Arc::new(TestTableModel::new(5, 3));
        let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));
        tv.set_model(model);

        assert!(tv.focused_row().is_none());

        assert!(tv.set_focused_row(3));
        assert_eq!(tv.focused_row(), Some(3));

        assert!(tv.set_focused_row(3));
        assert_eq!(tv.focused_row(), Some(3));

        assert!(!tv.set_focused_row(99));
        assert_eq!(tv.focused_row(), Some(3));

        tv.clear_focused_row();
        assert!(tv.focused_row().is_none());

        tv.clear_focused_row();
        assert!(tv.focused_row().is_none());
    }

    #[test]
    fn test_selection_changed_signal() {
        let model = Arc::new(TestTableModel::new(5, 3));
        let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));
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
        let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));
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

    #[test]
    fn test_geometry_delegation() {
        let mut tv = TableWidget::new(Rect::new(10, 20, 400, 300));

        assert_eq!(tv.geometry(), Rect::new(10, 20, 400, 300));

        tv.set_geometry(Rect::new(0, 0, 500, 400));
        assert_eq!(tv.geometry(), Rect::new(0, 0, 500, 400));
        assert_eq!(tv.rect(), Rect::new(0, 0, 500, 400));
        assert_eq!(tv.position(), Point::new(0, 0));
        assert_eq!(tv.size(), crate::core::Size::new(500, 400));
    }

    #[test]
    fn test_widget_id_and_kind() {
        let tv = TableWidget::new(Rect::new(0, 0, 400, 300));

        assert_eq!(tv.kind(), WidgetKind::Table);
        assert_ne!(tv.id(), 0);

        let tv2 = TableWidget::new(Rect::new(0, 0, 200, 100));
        assert_ne!(tv.id(), tv2.id());
    }

    #[test]
    fn test_svg_output() {
        let mut tv = TableWidget::new(Rect::new(0, 0, 200, 100));

        // Without model
        let svg = crate::widget::svg::render_to_svg(&mut tv);
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(svg.contains("width=\"200\""));
        assert!(svg.contains("height=\"100\""));

        // With model
        let model = Arc::new(TestTableModel::new(2, 3));
        let mut tv2 = TableWidget::new(Rect::new(0, 0, 300, 150));
        tv2.set_model(model);
        let svg2 = crate::widget::svg::render_to_svg(&mut tv2);
        assert!(svg2.starts_with("<svg"));
    }

    #[test]
    fn test_disabled_state_blocks_events() {
        let model = Arc::new(TestTableModel::new(5, 3));
        let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));
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
        tv.handle_event(&crate::event::Event::MousePress {
            pos: crate::core::Point::new(10, 15),
            button: 1,
        });
        assert!(captured.lock().unwrap().is_none());

        // Re-enable and verify it works
        tv.set_enabled(true);
        tv.handle_event(&crate::event::Event::MousePress {
            pos: crate::core::Point::new(10, 15),
            button: 1,
        });
        assert_eq!(*captured.lock().unwrap(), Some(0));
    }

    #[test]
    fn test_row_height_overrides() {
        let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));

        assert_eq!(tv.row_height(0), None);
        assert_eq!(tv.row_height(99), None);

        tv.set_row_height(1, 28);
        assert_eq!(tv.row_height(1), Some(28));

        tv.set_row_height(5, 32);
        assert_eq!(tv.row_height(5), Some(32));
        assert_eq!(tv.row_height(1), Some(28));
    }

    #[test]
    fn test_item_delegate() {
        struct TestDelegate;

        impl ItemDelegate for TestDelegate {
            fn create_editor(
                &self,
                _parent: &mut BaseWidget,
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

        let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));
        assert!(!tv.has_delegate());
        assert!(tv.delegate_ref().is_none());

        tv.set_delegate(Arc::new(TestDelegate));
        assert!(tv.has_delegate());
        assert!(tv.delegate_ref().is_some());
    }
}
