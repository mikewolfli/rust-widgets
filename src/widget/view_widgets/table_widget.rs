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
        self.selection
            .current_row()
            .filter(|row| *row < self.row_count())
    }
    /// All selected rows in stable order.
    pub fn selected_rows(&self) -> Vec<usize> {
        self.selection
            .rows()
            .into_iter()
            .filter(|row| *row < self.row_count())
            .collect()
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
