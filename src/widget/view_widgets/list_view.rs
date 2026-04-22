//! List view widget.

use crate::core::Rect;
use crate::object::Object;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::widget::base::{BaseWidget, Widget, WidgetKind};
use crate::widget::base_widgets::frame::Frame;
use std::sync::Arc;

/// List model abstraction for list-like views.
pub trait ListModel: Send + Sync {
    /// Number of rows exposed by model.
    fn row_count(&self) -> usize;
    /// Data for row index, if present.
    fn data(&self, row: usize) -> Option<String>;

    /// Optional signal emitted when model data projection changes.
    fn data_changed_signal(&self) -> Option<&GenericSignal> {
        None
    }
}

/// In-memory list model backed by a vector of strings.
pub struct VecListModel {
    items: Vec<String>,
    data_changed: GenericSignal,
}

impl VecListModel {
    /// Creates a new vector list model.
    pub fn new(items: Vec<String>) -> Self {
        Self {
            items,
            data_changed: GenericSignal::new(),
        }
    }

    /// Appends an item to the model.
    pub fn append(&mut self, item: String) {
        self.items.push(item);
        self.data_changed.emit();
    }

    /// Removes an item at given index.
    pub fn remove(&mut self, index: usize) -> Option<String> {
        if index < self.items.len() {
            let item = self.items.remove(index);
            self.data_changed.emit();
            Some(item)
        } else {
            None
        }
    }

    /// Clears all items.
    pub fn clear(&mut self) {
        self.items.clear();
        self.data_changed.emit();
    }
}

impl ListModel for VecListModel {
    fn row_count(&self) -> usize {
        self.items.len()
    }

    fn data(&self, row: usize) -> Option<String> {
        self.items.get(row).cloned()
    }

    fn data_changed_signal(&self) -> Option<&GenericSignal> {
        Some(&self.data_changed)
    }
}

/// Selection state for list/tree/table views.
pub struct SelectionModel {
    /// Current selection mode.
    mode: SelectionMode,
    /// Currently selected rows.
    selected_rows: Vec<usize>,
    /// Currently focused row.
    current_row: Option<usize>,
}

impl SelectionModel {
    /// Creates a new selection model.
    pub fn new() -> Self {
        Self {
            mode: SelectionMode::Single,
            selected_rows: Vec::new(),
            current_row: None,
        }
    }

    /// Sets selection mode.
    pub fn set_mode(&mut self, mode: SelectionMode) {
        self.mode = mode;
        self.normalize();
    }

    /// Returns current selection mode.
    pub fn mode(&self) -> SelectionMode {
        self.mode
    }

    /// Selects a row.
    pub fn select_row(&mut self, row: usize) {
        match self.mode {
            SelectionMode::Single => {
                self.selected_rows.clear();
                self.selected_rows.push(row);
                self.current_row = Some(row);
            }
            SelectionMode::Multi => {
                if !self.selected_rows.contains(&row) {
                    self.selected_rows.push(row);
                }
                self.current_row = Some(row);
            }
            SelectionMode::Extended => {
                // Extended selection logic
                self.selected_rows.push(row);
                self.current_row = Some(row);
            }
        }
    }

    /// Clears selection.
    pub fn clear(&mut self) {
        self.selected_rows.clear();
        self.current_row = None;
    }

    /// Returns current row.
    pub fn current_row(&self) -> Option<usize> {
        self.current_row
    }

    /// Returns all selected rows.
    pub fn rows(&self) -> Vec<usize> {
        self.selected_rows.clone()
    }

    fn normalize(&mut self) {
        match self.mode {
            SelectionMode::Single => {
                if self.selected_rows.len() > 1 {
                    if let Some(&last) = self.selected_rows.last() {
                        self.selected_rows = vec![last];
                    } else {
                        self.selected_rows.clear();
                    }
                }
            }
            _ => {}
        }
    }
}

/// Selection mode for list/tree/table views.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionMode {
    /// Single selection.
    Single,
    /// Multiple selection.
    Multi,
    /// Extended selection (Ctrl+Click, Shift+Click).
    Extended,
}

/// List view widget.
pub struct ListView {
    base: BaseWidget,
    /// Optional bound list model.
    model: Option<Arc<dyn ListModel>>,
    /// Scoped model-to-view signal subscriptions.
    model_connection_scope: ConnectionScope,
    /// View-side selection state.
    selection: SelectionModel,
    /// View-side focused row.
    focused_row: Option<usize>,
    /// Emitted when selected row changes.
    pub selection_changed: Signal1<usize>,
    /// Emitted when focused row changes.
    pub focused_row_changed: Signal1<Option<usize>>,
}

impl ListView {
    /// Creates an empty list view.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ListView, geometry, "ListView"),
            model: None,
            model_connection_scope: ConnectionScope::new(),
            selection: SelectionModel::new(),
            focused_row: None,
            selection_changed: Signal1::new(),
            focused_row_changed: Signal1::new(),
        }
    }

    /// Binds an external list model.
    pub fn set_model(&mut self, model: Arc<dyn ListModel>) {
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

    /// Returns visible row count.
    pub fn row_count(&self) -> usize {
        self.model.as_ref().map(|m| m.row_count()).unwrap_or(0)
    }

    /// Returns item text by row index.
    pub fn item(&self, row: usize) -> Option<String> {
        self.model.as_ref().and_then(|m| m.data(row))
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
    pub fn set_selection_mode(&mut self, mode: SelectionMode) {
        self.selection.set_mode(mode);
    }

    /// Returns current selection mode.
    pub fn selection_mode(&self) -> SelectionMode {
        self.selection.mode()
    }

    fn normalize_projection_state(&mut self) {
        let row_count = self.row_count();
        self.selection.selected_rows.retain(|row| *row < row_count);
        self.selection.current_row = self.selection.current_row.filter(|row| *row < row_count);
        self.focused_row = self.focused_row.filter(|row| *row < row_count);
    }
}

impl Widget for ListView {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl crate::widget::base::Draw for ListView {
    fn draw(&self, canvas: &mut dyn crate::render::Canvas) {
        // Default drawing implementation
        Frame::draw_frame(canvas, self.base().geometry());
    }
}

impl crate::event::EventHandler for ListView {
    fn handle_event(&mut self, event: &crate::event::Event) -> bool {
        // Default event handling
        false
    }
}
