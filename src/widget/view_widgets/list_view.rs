// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! List view widget.
use crate::core::HorizontalAlignment;
use crate::core::Rect;
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::widget::capability::access::{selection_mode_to_str, view_mode_to_str};
use crate::widget::capability::coercion::{expect_selection_mode, expect_usize, expect_view_mode};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
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
        Self { items, data_changed: GenericSignal::new() }
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
        Self { mode: SelectionMode::Single, selected_rows: Vec::new(), current_row: None }
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
    ///
    /// In [`SelectionMode::None`] the view accepts no selection, so this is a
    /// no-op — the mode is a property of the view, not a transient state, and
    /// silently selecting anyway would contradict what the caller asked for.
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
            SelectionMode::None => {}
        }
    }
    /// Clears selection.
    pub fn clear(&mut self) {
        self.selected_rows.clear();
        self.current_row = None;
    }
    /// Returns whether the view accepts selection at all.
    pub fn is_selectable(&self) -> bool {
        self.mode != SelectionMode::None
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
/// Selection mode for list, tree, table and list-box views.
///
/// Re-exported from [`crate::widget::input_widgets::listbox::SelectionMode`] — the
/// input layer owns the canonical definition because `ListBox` needs it in every
/// profile, while this module is `full_widgets`-gated. One definition for every
/// selection surface (principle #54).
///
/// `None` means the view accepts no selection at all, which is distinct from an
/// empty selection in `Single`/`Multi` mode: it is a property of the view, not a
/// state of the data.
pub use crate::widget::input_widgets::listbox::SelectionMode;
/// How a list view presents its items.
///
/// # Not the same as [`crate::widget::container_widgets::mdiarea::ViewMode`]
///
/// Both types are called `ViewMode`, but they describe different axes and are
/// **not** interchangeable:
///
/// * this one — how one `ListView` presents its *own items* (list, icons,
///   details, thumbnails);
/// * [`mdiarea::ViewMode`][mdi] — how an `MdiArea` lays its *child windows* out
///   (free-floating sub-windows vs. tabs).
///
/// They are deliberately left as two enums: merging them would create one type
/// whose variants are meaningless in the other's context, so a `match` could not
/// be exhaustive in either widget. See principle #49.
///
/// [mdi]: crate::widget::container_widgets::mdiarea::ViewMode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    /// Flat list of items.
    #[default]
    List,
    /// Large icons in a grid.
    Icon,
    /// Rows with extra columns of detail.
    Details,
    /// Thumbnail grid.
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
        self.selection.current_row().filter(|row| *row < self.row_count())
    }
    /// All selected rows in stable order.
    pub fn selected_rows(&self) -> Vec<usize> {
        self.selection.rows().into_iter().filter(|row| *row < self.row_count()).collect()
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

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(200, 200)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `ListView`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_view.in.rs` / `access_write_view.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before.
impl WidgetProperties for ListView {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "has_model" => Ok(CapabilityValue::Bool(self.has_model())),
            "row_count" => Ok(CapabilityValue::UInt(self.row_count() as u64)),
            "focused_row" => match self.focused_row() {
                Some(row) => Ok(CapabilityValue::UInt(row as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "selection_mode" => Ok(CapabilityValue::String(
                selection_mode_to_str(self.selection_mode()).to_string(),
            )),
            "view_mode" => {
                Ok(CapabilityValue::String(view_mode_to_str(self.view_mode()).to_string()))
            }
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "focused_row" => match value {
                CapabilityValue::Null => {
                    self.clear_focused_row();
                    Ok(())
                }
                other => {
                    let row = expect_usize(other)?;
                    if self.set_focused_row(row) {
                        Ok(())
                    } else {
                        Err(CapabilityAccessError::UnsupportedOnWidget)
                    }
                }
            },
            "selection_mode" => {
                self.set_selection_mode(expect_selection_mode(value)?);
                Ok(())
            }
            "view_mode" => {
                self.set_view_mode(expect_view_mode(value)?);
                Ok(())
            }
            "has_model" => Err(CapabilityAccessError::ReadOnlyProperty),
            "row_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "has_model",
            "row_count",
            "focused_row",
            "selection_mode",
            "view_mode",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for ListView {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        use crate::core::Color;
        // Draw background
        context.fill_rect(rect, Color::rgb(255, 255, 255));
        // Draw border
        context.draw_rect(rect, Color::rgb(200, 200, 200));
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
                        Color::rgb(200, 220, 255),
                    );
                }
                if let Some(text) = model.data(i) {
                    context.draw_text(
                        crate::core::Point::new(rect.x + 2, y + item_height / 2),
                        &text,
                        &crate::core::Font::default(),
                        Color::rgb(0, 0, 0),
                        HorizontalAlignment::Left,
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
            _ => { /* Other events are not relevant */ }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    struct StaticListModel;

    impl ListModel for StaticListModel {
        fn row_count(&self) -> usize {
            2
        }

        fn data(&self, row: usize) -> Option<String> {
            match row {
                0 => Some("A".to_string()),
                1 => Some("B".to_string()),
                _ => None,
            }
        }
    }

    #[test]
    fn list_view_model_binding_roundtrip() {
        let mut view = ListView::new(Rect::new(0, 0, 100, 80));
        assert!(!view.has_model());
        assert!(view.model_ref().is_none());

        view.set_model(Arc::new(StaticListModel));

        assert!(view.has_model());
        assert!(view.model_ref().is_some());
        assert_eq!(view.row_count(), 2);
        assert_eq!(view.item(99), None);
    }
}
