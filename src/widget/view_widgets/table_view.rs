//! Table view widget.
use crate::core::Rect;
use crate::render::RenderContext;
use crate::widget::base::{BaseWidget, Widget, WidgetKind};
use crate::widget::view_widgets::table_widget::{ItemDelegate, TableModel, TableWidget};
use std::sync::Arc;
/// Dedicated table-view widget contract with table model projection parity.
///
/// TableView provides a simplified interface to TableWidget, focusing on model-view functionality
/// with a clean API for common table operations.
pub struct TableView {
    table: TableWidget,
}
impl TableView {
    /// Creates an empty table view with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            table: TableWidget::new(geometry),
        }
    }
    /// Binds an external table model to the view.
    pub fn set_model(&mut self, model: Arc<dyn TableModel>) {
        self.table.set_model(model);
    }
    /// Returns the number of visible rows in the table.
    pub fn row_count(&self) -> usize {
        self.table.row_count()
    }
    /// Returns the number of visible columns in the table.
    pub fn column_count(&self) -> usize {
        self.table.column_count()
    }
    /// Reads the table header text for the specified column.
    pub fn header(&self, col: usize) -> Option<String> {
        // Default implementation - can be overridden by models
        Some(format!("Column {}", col + 1))
    }
    /// Reads the table cell value at the specified row and column.
    pub fn cell(&self, row: usize, col: usize) -> Option<String> {
        self.table.item(row, col)
    }
    /// Reads the formatted display value for the specified cell, taking into account any item delegate.
    pub fn display_cell(&self, row: usize, col: usize) -> Option<String> {
        self.table.item(row, col)
    }
    /// Sets an item delegate for display and editor conversion.
    pub fn set_delegate(&mut self, delegate: Arc<dyn ItemDelegate>) {
        self.table.set_delegate(delegate);
    }
    /// Clears any custom item delegate, reverting to default behavior.
    pub fn clear_delegate(&mut self) {
        self.table.set_delegate(Arc::new(DefaultDelegate));
    }
    /// Selects a row in the table.
    pub fn select_row(&mut self, row: usize) -> bool {
        self.table.select_row(row)
    }
    /// Clears the current row selection.
    pub fn clear_selection(&mut self) {
        self.table.clear_selection();
    }
    /// Sets the focused row in the table.
    pub fn set_focused_row(&mut self, row: usize) -> bool {
        self.table.set_focused_row(row)
    }
    /// Clears the focused row.
    pub fn clear_focused_row(&mut self) {
        self.table.clear_focused_row();
    }
    /// Returns the currently focused row, if any.
    pub fn focused_row(&self) -> Option<usize> {
        self.table.focused_row()
    }
    /// Returns the currently selected row, if any.
    pub fn selected_row(&self) -> Option<usize> {
        self.table.selected_row()
    }
    /// Returns all selected rows in stable order.
    pub fn selected_rows(&self) -> Vec<usize> {
        self.table.selected_rows()
    }
    /// Sets the row selection mode.
    pub fn set_selection_mode(
        &mut self,
        mode: crate::widget::view_widgets::list_view::SelectionMode,
    ) {
        self.table.set_selection_mode(mode);
    }
    /// Returns the current selection mode.
    pub fn selection_mode(&self) -> crate::widget::view_widgets::list_view::SelectionMode {
        self.table.selection_mode()
    }
    /// Sets a custom column width.
    pub fn set_column_width(&mut self, column: usize, width: u32) {
        self.table.set_column_width(column, width);
    }
    /// Sets a custom row height.
    pub fn set_row_height(&mut self, row: usize, height: u32) {
        self.table.set_row_height(row, height);
    }
}
/// Default item delegate implementation.
struct DefaultDelegate;
impl ItemDelegate for DefaultDelegate {
    fn create_editor(
        &self,
        _parent: &mut BaseWidget,
        _row: usize,
        _column: usize,
    ) -> Option<Box<dyn Widget>> {
        None
    }
    fn set_editor_data(&self, _editor: &mut dyn Widget, _row: usize, _column: usize) {
        // No-op
    }
    fn get_editor_data(&self, _editor: &dyn Widget, _row: usize, _column: usize) -> Option<String> {
        None
    }
}
impl Widget for TableView {
    fn base(&self) -> &BaseWidget {
        self.table.base()
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        self.table.base_mut()
    }
}
impl crate::widget::base::Draw for TableView {
    fn draw(&mut self, context: &mut RenderContext) {
        self.table.draw(context);
    }
}
impl crate::event::EventHandler for TableView {
    fn handle_event(&mut self, event: &crate::event::Event) {
        self.table.handle_event(event);
    }
}
