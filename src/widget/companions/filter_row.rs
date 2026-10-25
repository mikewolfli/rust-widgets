//! `FilterRow` — one editor per column over the shared filter model.

use crate::compat::{format, String, ToString, Vec};
use crate::core::{Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::impl_draw_bridge;
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{EventSignalRef, GenericSignal, Signal1};
use crate::widget::capability::coercion::{
    expect_bool, expect_string, expect_string_list, expect_usize,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::view_widgets::filter_expr::{FilterCondition, FilterOperator};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

use super::helpers::{accent_color, surface_ink};

// ---------------------------------------------------------------------------
// COMP-05 FilterRow
// ---------------------------------------------------------------------------

/// Inset around a filter row's editors.
const FILTER_INSET: u32 = 4;

/// A row of column editors that emits the shared filter model.
///
/// `conditions` always refer to the grid's source columns by index; the visible `columns` list
/// only supplies the editor labels. `live` controls whether every keystroke publishes or only
/// Enter does.
pub struct FilterRow {
    base: BaseWidget,
    columns: Vec<String>,
    conditions: Vec<FilterCondition>,
    operator: FilterOperator,
    live: bool,
    /// One edit buffer per column, aligned with `columns`.
    editors: Vec<String>,
    active_column: Option<usize>,
    /// Emitted with the complete condition list after any change.
    pub filter_changed: Signal1<Vec<FilterCondition>>,
    /// Emitted after [`Self::clear_filters`] removed the conditions.
    pub filter_cleared: GenericSignal,
}

impl FilterRow {
    /// Creates an empty filter row.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::QueryBuilder, geometry, "FilterRow"),
            columns: Vec::new(),
            conditions: Vec::new(),
            operator: FilterOperator::Contains,
            live: true,
            editors: Vec::new(),
            active_column: None,
            filter_changed: Signal1::new(),
            filter_cleared: GenericSignal::new(),
        }
    }

    /// Replaces the column labels, preserving the conditions that still address an existing
    /// column.
    pub fn set_columns(&mut self, columns: Vec<String>) {
        self.editors.resize(columns.len(), String::new());
        self.columns = columns;
        self.sync_editors();
        if self.active_column.is_some_and(|index| index >= self.columns.len()) {
            self.active_column = None;
        }
        self.base.request_redraw();
    }

    /// Returns the editor labels.
    pub fn columns(&self) -> &[String] {
        &self.columns
    }

    /// Returns the current conditions.
    pub fn conditions(&self) -> &[FilterCondition] {
        &self.conditions
    }

    /// Sets one column's condition, replacing any condition already on that column.
    ///
    /// Returns `false` when `column` is not one of the row's columns.
    pub fn set_condition(&mut self, column: usize, condition: FilterCondition) -> bool {
        if column >= self.columns.len() {
            return false;
        }
        self.replace_condition(column, condition);
        self.publish_changed();
        true
    }

    /// Removes every condition and publishes both the empty list and the clear notice.
    pub fn clear_filters(&mut self) {
        self.conditions.clear();
        for editor in &mut self.editors {
            editor.clear();
        }
        self.base.request_redraw();
        self.publish_changed();
        self.filter_cleared.emit();
    }

    /// Sets the operator used by newly committed conditions.
    pub fn set_operator(&mut self, operator: FilterOperator) {
        if self.operator != operator {
            self.operator = operator;
            self.base.request_redraw();
        }
    }

    /// Returns the operator used by new conditions.
    pub fn operator(&self) -> FilterOperator {
        self.operator
    }

    /// Enables per-keystroke publishing.
    pub fn set_live(&mut self, live: bool) {
        self.live = live;
    }

    /// Returns whether publishing is per-keystroke.
    pub fn live(&self) -> bool {
        self.live
    }

    /// Replaces the whole condition list, re-reading the editor buffers from the new entries.
    fn replace_conditions(&mut self, conditions: Vec<FilterCondition>) {
        self.conditions = conditions;
        for editor in &mut self.editors {
            editor.clear();
        }
        self.sync_editors();
        self.base.request_redraw();
        self.publish_changed();
    }

    /// Copies each condition's operand into its column's editor. Called after any bulk change;
    /// per-keystroke commits must not call this or the caret content would be rewritten.
    fn sync_editors(&mut self) {
        for condition in &self.conditions {
            if let Some(editor) = self.editors.get_mut(condition.column) {
                editor.clear();
                editor.push_str(&condition.operand);
            }
        }
    }

    /// Replaces or inserts one condition and its editor text.
    fn replace_condition(&mut self, column: usize, mut condition: FilterCondition) {
        condition.column = column;
        if let Some(existing) = self.conditions.iter_mut().find(|item| item.column == column) {
            *existing = condition.clone();
        } else {
            self.conditions.push(condition.clone());
        }
        self.conditions.sort_by_key(|item| item.column);
        if let Some(editor) = self.editors.get_mut(column) {
            editor.clear();
            editor.push_str(&condition.operand);
        }
        self.base.request_redraw();
    }

    /// Commits the active column's editor text: empty removes the condition, otherwise the text
    /// becomes the operand under the row's operator.
    fn commit_column(&mut self, column: usize) {
        let Some(text) = self.editors.get(column).map(|text| text.trim().to_string()) else {
            return;
        };
        if text.is_empty() {
            let before = self.conditions.len();
            self.conditions.retain(|condition| condition.column != column);
            if self.conditions.len() != before {
                self.publish_changed();
            }
            return;
        }
        let condition = FilterCondition::new(column, self.operator, text);
        self.replace_condition(column, condition);
        self.publish_changed();
    }

    /// Publishes the current condition list.
    fn publish_changed(&mut self) {
        self.base.request_redraw();
        self.filter_changed.emit(self.conditions.clone());
    }

    /// The column whose editor contains `pos`.
    fn column_at(&self, pos: Point) -> Option<usize> {
        if self.columns.is_empty() || !self.geometry().contains(pos) {
            return None;
        }
        let width = (self.geometry().width / self.columns.len() as u32).max(1);
        let offset = (pos.x - self.geometry().x).max(0) as u32;
        let index = (offset / width) as usize;
        (index < self.columns.len()).then_some(index)
    }
}

impl EventHandler for FilterRow {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button: 1, .. } => {
                self.active_column = self.column_at(*pos);
                self.base.request_redraw();
            }
            Event::TextInput { text } => {
                let Some(column) = self.active_column else {
                    return;
                };
                if let Some(editor) = self.editors.get_mut(column) {
                    editor.push_str(text);
                }
                if self.live {
                    self.commit_column(column);
                } else {
                    self.base.request_redraw();
                }
            }
            Event::KeyPress { key, .. } => {
                let Some(column) = self.active_column else {
                    return;
                };
                match *key {
                    8 => {
                        if let Some(editor) = self.editors.get_mut(column) {
                            editor.pop();
                        }
                        if self.live {
                            self.commit_column(column);
                        } else {
                            self.base.request_redraw();
                        }
                    }
                    13 => self.commit_column(column),
                    27 => {
                        if let Some(editor) = self.editors.get_mut(column) {
                            editor.clear();
                        }
                        let before = self.conditions.len();
                        self.conditions.retain(|condition| condition.column != column);
                        self.active_column = None;
                        if self.conditions.len() != before {
                            self.publish_changed();
                        } else {
                            self.base.request_redraw();
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

impl Draw for FilterRow {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let style = self.style();
        let (surface, ink) = surface_ink(style);
        let accent = accent_color();
        let font = effective_font(style);
        context.fill_rect(rect, surface);
        context.draw_rect(rect, surface.blend(&ink, 0.18));

        if self.columns.is_empty() {
            let placeholder =
                Rect::new(rect.x + 8, rect.y, rect.width.saturating_sub(16), rect.height);
            context.draw_text_fitted(
                placeholder,
                "No filter columns",
                font,
                surface.blend(&ink, 0.55),
                crate::core::HorizontalAlignment::Left,
            );
            return;
        }

        let width = (rect.width / self.columns.len() as u32).max(1);
        for (index, label) in self.columns.iter().enumerate() {
            let x = rect.x + (index as u32 * width) as i32;
            let cell = Rect::new(
                x + FILTER_INSET as i32,
                rect.y + FILTER_INSET as i32,
                width.saturating_sub(FILTER_INSET * 2),
                rect.height.saturating_sub(FILTER_INSET * 2),
            );
            context.fill_rect(cell, surface.blend(&ink, 0.05));
            let border =
                if self.active_column == Some(index) { accent } else { surface.blend(&ink, 0.22) };
            context.draw_rect(cell, border);
            let editor = self.editors.get(index).map(String::as_str).unwrap_or("");
            let (text, color) = if editor.is_empty() {
                (label.as_str(), surface.blend(&ink, 0.50))
            } else {
                (editor, ink)
            };
            context.draw_text_fitted(
                cell,
                text,
                font,
                color,
                crate::core::HorizontalAlignment::Left,
            );
        }
    }
}

impl Widget for FilterRow {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn size_hint(&self) -> Size {
        Size::new((self.columns.len() as u32 * 120).max(120), 32)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    fn event_signal_dyn(&self, name: &str) -> Option<EventSignalRef> {
        match name {
            "filter_changed" => {
                Some(EventSignalRef::mapped("filter_changed", &self.filter_changed, |conditions| {
                    CapabilityValue::String(format!("{conditions:?}"))
                }))
            }
            "filter_cleared" => Some(EventSignalRef::unit("filter_cleared", &self.filter_cleared)),
            _ => None,
        }
    }
}

/// Encodes one condition as `[column, operator, operand]`.
fn condition_value(condition: &FilterCondition) -> CapabilityValue {
    CapabilityValue::Tuple(vec![
        CapabilityValue::UInt(condition.column as u64),
        CapabilityValue::String(condition.operator.as_str().to_string()),
        CapabilityValue::String(condition.operand.clone()),
    ])
}

/// Decodes the condition-list property.
fn expect_conditions(
    value: CapabilityValue,
) -> Result<Vec<FilterCondition>, CapabilityAccessError> {
    let CapabilityValue::Tuple(items) = value else {
        return Err(CapabilityAccessError::TypeMismatch);
    };
    let mut conditions = Vec::with_capacity(items.len());
    for item in items {
        let CapabilityValue::Tuple(parts) = item else {
            return Err(CapabilityAccessError::TypeMismatch);
        };
        if parts.len() != 3 {
            return Err(CapabilityAccessError::TypeMismatch);
        }
        let mut parts = parts.into_iter();
        let column = expect_usize(parts.next().ok_or(CapabilityAccessError::TypeMismatch)?)?;
        let token = expect_string(parts.next().ok_or(CapabilityAccessError::TypeMismatch)?)?;
        let operator = FilterOperator::from_name(&token.to_ascii_lowercase())
            .ok_or(CapabilityAccessError::TypeMismatch)?;
        let operand = expect_string(parts.next().ok_or(CapabilityAccessError::TypeMismatch)?)?;
        conditions.push(FilterCondition::new(column, operator, operand));
    }
    Ok(conditions)
}

impl WidgetProperties for FilterRow {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "columns" => Ok(CapabilityValue::Tuple(
                self.columns.iter().map(|column| CapabilityValue::String(column.clone())).collect(),
            )),
            "conditions" => {
                Ok(CapabilityValue::Tuple(self.conditions.iter().map(condition_value).collect()))
            }
            "operator" => Ok(CapabilityValue::String(self.operator.as_str().to_string())),
            "live" => Ok(CapabilityValue::Bool(self.live)),
            _ => base_property_get(self, name),
        }
    }
    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "columns" => {
                self.set_columns(expect_string_list(value)?);
                Ok(())
            }
            "conditions" => {
                self.replace_conditions(expect_conditions(value)?);
                Ok(())
            }
            "operator" => {
                let token = expect_string(value)?.to_ascii_lowercase();
                self.set_operator(
                    FilterOperator::from_name(&token).ok_or(CapabilityAccessError::TypeMismatch)?,
                );
                Ok(())
            }
            "live" => {
                self.set_live(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }
    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["columns", "conditions", "operator", "live", BASE_PROPERTY_NAMES]
    }
}
