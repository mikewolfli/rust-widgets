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

use std::fmt;

use crate::app::{
    ButtonHandle, CheckBoxHandle, ComboBoxHandle, FrameHandle, GridWidgetHandle, LabelHandle,
    LineEditHandle, ListBoxHandle, ListViewHandle, PanelHandle, ProgressBarHandle,
    RadioButtonHandle, ScrollAreaHandle, ScrollBarHandle, SliderHandle, SpinBoxHandle,
    TabWidgetHandle, TextEditHandle, WidgetHandle, WindowHandle,
};
use crate::core::ObjectId;

/// A named widget layout, mapping JSON `"id"` attributes to `ObjectId` values.
///
/// Created by `JsonLoader::load` after instantiating a JSON layout.
/// Provides typed widget access via [`widget_by_name`](BoundJsonLayout::widget_by_name).
pub struct BoundJsonLayout {
    name_map: HashMap<String, ObjectId>,
}

impl fmt::Debug for BoundJsonLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BoundJsonLayout").field("len", &self.len()).finish()
    }
}

impl BoundJsonLayout {
    /// Create a new empty layout binding.
    pub fn new() -> Self {
        Self { name_map: HashMap::new() }
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
        let raw_id =
            self.id(name).ok_or_else(|| format!("widget '{}' not found in layout", name))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_empty_layout() {
        let layout = BoundJsonLayout::new();
        assert_eq!(layout.len(), 0);
        assert!(layout.is_empty());
    }

    #[test]
    fn register_and_retrieve_id() {
        let mut layout = BoundJsonLayout::new();
        let id = 42;
        layout.register("my_widget", id);
        assert_eq!(layout.len(), 1);
        assert!(!layout.is_empty());
        assert_eq!(layout.id("my_widget"), Some(id));
    }

    #[test]
    fn id_returns_none_for_unknown() {
        let layout = BoundJsonLayout::new();
        assert_eq!(layout.id("nonexistent"), None);
    }

    #[test]
    fn register_multiple_widgets() {
        let mut layout = BoundJsonLayout::new();
        layout.register("btn1", 1);
        layout.register("btn2", 2);
        layout.register("label1", 3);
        assert_eq!(layout.len(), 3);
    }

    #[test]
    fn duplicate_name_overwrites() {
        let mut layout = BoundJsonLayout::new();
        let id1 = 10;
        let id2 = 20;
        layout.register("dup", id1);
        layout.register("dup", id2);
        assert_eq!(layout.len(), 1);
        assert_eq!(layout.id("dup"), Some(id2));
    }

    #[test]
    fn widget_by_name_not_found_error() {
        let layout = BoundJsonLayout::new();
        let result = layout.widget_by_name::<LabelHandle>("missing");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn convenience_handle_methods() {
        let mut layout = BoundJsonLayout::new();
        let id = 1;
        layout.register("my_btn", id);
        layout.register("my_label", id);
        layout.register("my_edit", id);
        // These should succeed since the widget exists in the name map
        assert!(layout.button("my_btn").is_ok());
        assert!(layout.label("my_label").is_ok());
        assert!(layout.line_edit("my_edit").is_ok());
    }

    #[test]
    fn convenience_handles_return_err_for_missing() {
        let layout = BoundJsonLayout::new();
        assert!(layout.button("no_such").is_err());
        assert!(layout.checkbox("no_such").is_err());
        assert!(layout.combo_box("no_such").is_err());
        assert!(layout.slider("no_such").is_err());
        assert!(layout.progress_bar("no_such").is_err());
        assert!(layout.panel("no_such").is_err());
        assert!(layout.spin_box("no_such").is_err());
        assert!(layout.scroll_area("no_such").is_err());
        assert!(layout.tab_widget("no_such").is_err());
        assert!(layout.grid_widget("no_such").is_err());
        assert!(layout.frame("no_such").is_err());
        assert!(layout.window("no_such").is_err());
    }

    #[test]
    fn default_is_empty() {
        let layout = BoundJsonLayout::default();
        assert!(layout.is_empty());
        assert_eq!(layout.len(), 0);
    }

    #[test]
    fn register_string_and_str() {
        let mut layout = BoundJsonLayout::new();
        let id = 99;
        layout.register("from_str".to_string(), id);
        layout.register("from_ref", id);
        assert_eq!(layout.id("from_str"), Some(id));
        assert_eq!(layout.id("from_ref"), Some(id));
    }
}
