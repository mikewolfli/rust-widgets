//! JSON-node → widget instantiation and typed handle access.
//!
//! This module provides [`BoundJsonLayout`], the binding between a JSON
//! declarative layout (defined via BLUE4.md spec) and the live widget tree.
//!
//! After [`JsonLoader::load`] parses a JSON string and instantiates all
//! widgets, it returns a `BoundJsonLayout`. Callers retrieve typed widget
//! handles by the JSON `"id"` attribute, e.g.:
//!
//! ```ignore
//! let layout = JsonLoader::load(json_str)?;
//! let btn = layout.widget_by_name::<ButtonHandle>("ok_btn")?;
//! btn.set_text("Confirm");
//! ```

use std::collections::HashMap;

use crate::app::{
    ButtonHandle, CheckBoxHandle, ComboBoxHandle, FrameHandle, GridWidgetHandle, LabelHandle,
    LineEditHandle, ListBoxHandle, ListViewHandle, PanelHandle, ProgressBarHandle,
    RadioButtonHandle, ScrollAreaHandle, ScrollBarHandle, SliderHandle, SpinBoxHandle,
    TabWidgetHandle, TextEditHandle, WidgetHandle, WindowHandle,
};
use crate::core::ObjectId;

/// A named widget layout, mapping JSON `"id"` attributes to `ObjectId` values.
///
/// Created by [`JsonLoader::load`] after instantiating a JSON layout.
/// Provides typed widget access via [`widget_by_name`](BoundJsonLayout::widget_by_name).
pub struct BoundJsonLayout {
    name_map: HashMap<String, ObjectId>,
}

impl BoundJsonLayout {
    /// Create a new empty layout binding.
    pub fn new() -> Self {
        Self {
            name_map: HashMap::new(),
        }
    }

    /// Register a name-to-id mapping (called during instantiation).
    pub fn register(&mut self, name: impl Into<String>, id: ObjectId) {
        self.name_map.insert(name.into(), id);
    }

    /// Look up an [`ObjectId`] by JSON `"id"` attribute.
    pub fn id(&self, name: &str) -> Option<ObjectId> {
        self.name_map.get(name).copied()
    }

    /// Number of registered widgets.
    pub fn len(&self) -> usize {
        self.name_map.len()
    }

    /// Whether no widgets are registered.
    pub fn is_empty(&self) -> bool {
        self.name_map.is_empty()
    }

    // ── Typed widget access ────────────────────────────────

    /// Retrieve a typed handle for a widget by its JSON `id`.
    ///
    /// Returns `Err` if the widget name is not found.
    ///
    /// ```ignore
    /// let btn = layout.widget_by_name::<ButtonHandle>("ok_btn")?;
    /// btn.set_text("Confirm");
    /// ```
    pub fn widget_by_name<T: WidgetHandle>(&self, name: &str) -> Result<T, String> {
        let raw_id = self
            .id(name)
            .ok_or_else(|| format!("widget '{}' not found in layout", name))?;
        Ok(T::from_raw(raw_id))
    }

    /// Convenience: get a button handle by JSON id.
    pub fn button(&self, name: &str) -> Result<ButtonHandle, String> {
        self.widget_by_name(name)
    }

    /// Convenience: get a label handle by JSON id.
    pub fn label(&self, name: &str) -> Result<LabelHandle, String> {
        self.widget_by_name(name)
    }

    /// Convenience: get a line-edit handle by JSON id.
    pub fn line_edit(&self, name: &str) -> Result<LineEditHandle, String> {
        self.widget_by_name(name)
    }

    /// Convenience: get a checkbox handle by JSON id.
    pub fn checkbox(&self, name: &str) -> Result<CheckBoxHandle, String> {
        self.widget_by_name(name)
    }

    /// Convenience: get a radio button handle by JSON id.
    pub fn radio_button(&self, name: &str) -> Result<RadioButtonHandle, String> {
        self.widget_by_name(name)
    }

    /// Convenience: get a combo box handle by JSON id.
    pub fn combo_box(&self, name: &str) -> Result<ComboBoxHandle, String> {
        self.widget_by_name(name)
    }

    /// Convenience: get a list box handle by JSON id.
    pub fn list_box(&self, name: &str) -> Result<ListBoxHandle, String> {
        self.widget_by_name(name)
    }

    /// Convenience: get a slider handle by JSON id.
    pub fn slider(&self, name: &str) -> Result<SliderHandle, String> {
        self.widget_by_name(name)
    }

    /// Convenience: get a progress bar handle by JSON id.
    pub fn progress_bar(&self, name: &str) -> Result<ProgressBarHandle, String> {
        self.widget_by_name(name)
    }

    /// Convenience: get a panel handle by JSON id.
    pub fn panel(&self, name: &str) -> Result<PanelHandle, String> {
        self.widget_by_name(name)
    }

    /// Convenience: get a spin box handle by JSON id.
    pub fn spin_box(&self, name: &str) -> Result<SpinBoxHandle, String> {
        self.widget_by_name(name)
    }

    /// Convenience: get a list view handle by JSON id.
    pub fn list_view(&self, name: &str) -> Result<ListViewHandle, String> {
        self.widget_by_name(name)
    }

    /// Convenience: get a scroll area handle by JSON id.
    pub fn scroll_area(&self, name: &str) -> Result<ScrollAreaHandle, String> {
        self.widget_by_name(name)
    }

    /// Convenience: get a text edit handle by JSON id.
    pub fn text_edit(&self, name: &str) -> Result<TextEditHandle, String> {
        self.widget_by_name(name)
    }

    /// Convenience: get a scroll bar handle by JSON id.
    pub fn scroll_bar(&self, name: &str) -> Result<ScrollBarHandle, String> {
        self.widget_by_name(name)
    }

    /// Convenience: get a tab widget handle by JSON id.
    pub fn tab_widget(&self, name: &str) -> Result<TabWidgetHandle, String> {
        self.widget_by_name(name)
    }

    /// Convenience: get a grid widget handle by JSON id.
    pub fn grid_widget(&self, name: &str) -> Result<GridWidgetHandle, String> {
        self.widget_by_name(name)
    }

    /// Convenience: get a frame handle by JSON id.
    pub fn frame(&self, name: &str) -> Result<FrameHandle, String> {
        self.widget_by_name(name)
    }

    /// Convenience: get a window handle by JSON id.
    pub fn window(&self, name: &str) -> Result<WindowHandle, String> {
        self.widget_by_name(name)
    }
}

impl Default for BoundJsonLayout {
    fn default() -> Self {
        Self::new()
    }
}
