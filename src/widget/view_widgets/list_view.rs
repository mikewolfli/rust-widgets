//! List view widget.
use crate::core::Rect;
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
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
    /// Returns a reference to the data changed signal.
    pub fn data_changed_signal(&self) -> &GenericSignal {
        &self.data_changed
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
impl Default for SelectionModel {
    fn default() -> Self {
        Self::new()
    }
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
        if self.mode == SelectionMode::Single && self.selected_rows.len() > 1 {
            if let Some(&last) = self.selected_rows.last() {
                self.selected_rows = vec![last];
            } else {
                self.selected_rows.clear();
            }
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
/// View mode for list views.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    #[default]
    List,
    Icon,
    Details,
    Thumbnails,
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
    /// View mode for rendering items.
    view_mode: ViewMode,
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
            view_mode: ViewMode::default(),
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
    /// Returns whether a model is currently bound.
    pub fn has_model(&self) -> bool {
        self.model.is_some()
    }
    /// Returns the bound list model, if present.
    pub fn model_ref(&self) -> Option<&Arc<dyn ListModel>> {
        self.model.as_ref()
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

    /// Returns the current view mode.
    pub fn view_mode(&self) -> ViewMode {
        self.view_mode
    }

    /// Sets the view mode.
    pub fn set_view_mode(&mut self, mode: ViewMode) {
        self.view_mode = mode;
        self.base.request_redraw();
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

impl Draw for ListView {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        use crate::core::Color;
        // Draw background
        context.fill_rect(rect, Color::from_rgb(255, 255, 255));
        // Draw border
        context.draw_rect(rect, Color::from_rgb(200, 200, 200));
        // Draw items from model
        if let Some(ref model) = self.model {
            let item_height = 20;
            let row_count = model.row_count();
            let current_row = self.focused_row;
            for i in 0..row_count {
                let y = rect.y + item_height * i as i32;
                if y + item_height > rect.y + rect.height as i32 {
                    break;
                }
                if Some(i) == current_row {
                    context.fill_rect(
                        crate::core::Rect::new(rect.x, y, rect.width, item_height as u32),
                        Color::from_rgb(200, 220, 255),
                    );
                }
                if let Some(text) = model.data(i) {
                    context.draw_text(
                        crate::core::Point::new(rect.x + 2, y + item_height / 2),
                        &text,
                        &crate::core::Font::default(),
                        Color::from_rgb(0, 0, 0),
                    );
                }
            }
        }
    }
}
impl crate::event::EventHandler for ListView {
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
                    let row_count = self.row_count();
                    if index < row_count {
                        self.focused_row = Some(index);
                        self.selection.select_row(index);
                        if let Some(row) = self.focused_row {
                            self.selection_changed.emit(row);
                            self.focused_row_changed.emit(Some(row));
                        }
                    }
                }
            }
            #[cfg(feature = "touch")]
            crate::event::Event::Tap { pos } => {
                let rect = self.base.geometry();
                let item_height = 20;
                if pos.y >= rect.y {
                    let index = ((pos.y - rect.y) / item_height) as usize;
                    let row_count = self.row_count();
                    if index < row_count {
                        self.focused_row = Some(index);
                        self.selection.select_row(index);
                        if let Some(row) = self.focused_row {
                            self.selection_changed.emit(row);
                            self.focused_row_changed.emit(Some(row));
                        }
                    }
                }
            }
            _ => {}
        }
    }
}
