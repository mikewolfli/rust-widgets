use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use crate::control_backend::types::{ControlBackendKind, CustomControlState, CustomWidgetProperties};
use crate::control_backend::trait_def::ControlBackend;
use crate::core::ObjectId;
use crate::platform::{WidgetTriggerEvent, WidgetTriggerKind};
use crate::widget::WidgetKind;
/// Custom-painted control backend scaffold.
pub struct CustomPaintControlBackend {
    state: Mutex<CustomControlState>,
}
impl CustomPaintControlBackend {
    /// Create custom-painted control backend.
    pub fn new() -> Self {
        Self {
            state: Mutex::new(CustomControlState {
                next_widget_id: 1,
                ..CustomControlState::default()
            }),
        }
    }
    fn alloc_widget_id(&self) -> ObjectId {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let widget_id = state.next_widget_id;
        state.next_widget_id = state.next_widget_id.saturating_add(1);
        widget_id
    }
}
impl Default for CustomPaintControlBackend {
    fn default() -> Self {
        Self::new()
    }
}
impl ControlBackend for CustomPaintControlBackend {
    fn backend_name(&self) -> &'static str {
        "custom-paint-control-backend"
    }
    fn kind(&self) -> ControlBackendKind {
        ControlBackendKind::Custom
    }
    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.texts.insert(widget_id, title.to_string());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, title.to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: None,
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::Window,
            },
        );
        widget_id
    }
    fn create_button(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.texts.insert(widget_id, text.to_string());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, text.to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::Button,
            },
        );
        widget_id
    }
    fn create_checkbox(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.texts.insert(widget_id, text.to_string());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, text.to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::CheckBox,
            },
        );
        widget_id
    }
    fn create_line_edit(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.texts.insert(widget_id, text.to_string());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, true); // LineEdit enables IME by default
        state
            .accessibility_names
            .insert(widget_id, text.to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::LineEdit,
            },
        );
        widget_id
    }
    fn create_label(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.texts.insert(widget_id, text.to_string());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, text.to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::Label,
            },
        );
        widget_id
    }
    fn create_radio_button(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.texts.insert(widget_id, text.to_string());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, text.to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::RadioButton,
            },
        );
        widget_id
    }
    fn create_slider(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "Slider".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::Slider,
            },
        );
        widget_id
    }
    fn create_progress_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "ProgressBar".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::ProgressBar,
            },
        );
        widget_id
    }
    fn create_combo_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "ComboBox".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::ComboBox,
            },
        );
        widget_id
    }
    fn create_list_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "ListBox".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::ListBox,
            },
        );
        widget_id
    }
    fn create_panel(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "Panel".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::Panel,
            },
        );
        widget_id
    }
    fn create_menu_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "MenuBar".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::MenuBar,
            },
        );
        widget_id
    }
    fn create_menu(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.texts.insert(widget_id, text.to_string());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, text.to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::Menu,
            },
        );
        widget_id
    }
    fn attach_menu_bar_to_window(&self, _window: ObjectId, _menu_bar: ObjectId) -> bool {
        true
    }
    fn menu_add_item(
        &self,
        parent_menu: ObjectId,
        text: &str,
        _shortcut: Option<&str>,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.texts.insert(widget_id, text.to_string());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, text.to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent_menu),
                x: 0,
                y: 0,
                width: 0,
                height: 0,
                widget_kind: WidgetKind::Menu,
            },
        );
        widget_id
    }
    fn create_tool_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "ToolBar".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::ToolBar,
            },
        );
        widget_id
    }
    fn create_status_bar(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.texts.insert(widget_id, text.to_string());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, text.to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::StatusBar,
            },
        );
        widget_id
    }
    fn poll_menu_triggered(&self) -> Option<ObjectId> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .menu_trigger_queue
            .pop_front()
    }
    fn inject_menu_trigger(&self, menu_item_id: ObjectId) -> bool {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.menu_trigger_queue.push_back(menu_item_id);
        true
    }
    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .widget_trigger_queue
            .pop_front()
    }
    fn inject_widget_trigger_event(&self, widget_id: ObjectId, kind: WidgetTriggerKind) -> bool {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state
            .widget_trigger_queue
            .push_back(WidgetTriggerEvent { widget_id, kind });
        true
    }
    fn set_widget_text(&self, widget_id: ObjectId, text: &str) {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .texts
            .insert(widget_id, text.to_string());
    }
    fn get_widget_text(&self, widget_id: ObjectId) -> String {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .texts
            .get(&widget_id)
            .cloned()
            .unwrap_or_default()
    }
    fn set_widget_enabled(&self, widget_id: ObjectId, enabled: bool) {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .enabled
            .insert(widget_id, enabled);
    }
    fn is_widget_enabled(&self, widget_id: ObjectId) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .enabled
            .get(&widget_id)
            .copied()
            .unwrap_or(false)
    }
    fn set_widget_visible(&self, widget_id: ObjectId, visible: bool) {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .visible
            .insert(widget_id, visible);
    }
    fn is_widget_visible(&self, widget_id: ObjectId) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .visible
            .get(&widget_id)
            .copied()
            .unwrap_or(false)
    }
    fn set_widget_geometry(&self, widget_id: ObjectId, x: i32, y: i32, width: u32, height: u32) {
        if let Some(props) = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .widget_properties
            .get_mut(&widget_id)
        {
            props.x = x;
            props.y = y;
            props.width = width;
            props.height = height;
        }
    }
    fn set_widget_ime_enabled(&self, widget_id: ObjectId, enabled: bool) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .ime_enabled
            .insert(widget_id, enabled);
        true
    }
    fn is_widget_ime_enabled(&self, widget_id: ObjectId) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .ime_enabled
            .get(&widget_id)
            .copied()
            .unwrap_or(false)
    }
    fn set_widget_accessibility_name(&self, widget_id: ObjectId, name: &str) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .accessibility_names
            .insert(widget_id, name.to_string());
        true
    }
    fn get_widget_accessibility_name(&self, widget_id: ObjectId) -> String {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .accessibility_names
            .get(&widget_id)
            .cloned()
            .unwrap_or_default()
    }
    fn create_dialog(
        &self,
        parent: ObjectId,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.texts.insert(widget_id, title.to_string());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, title.to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::Dialog,
            },
        );
        widget_id
    }
    fn create_message_box(
        &self,
        parent: ObjectId,
        title: &str,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.texts.insert(widget_id, text.to_string());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, title.to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::MessageBox,
            },
        );
        widget_id
    }
    fn create_file_dialog(
        &self,
        parent: ObjectId,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.texts.insert(widget_id, title.to_string());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, title.to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::FileDialog,
            },
        );
        widget_id
    }
    fn create_color_dialog(
        &self,
        parent: ObjectId,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.texts.insert(widget_id, title.to_string());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, title.to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::ColorDialog,
            },
        );
        widget_id
    }
    fn create_font_dialog(
        &self,
        parent: ObjectId,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.texts.insert(widget_id, title.to_string());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, title.to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::FontDialog,
            },
        );
        widget_id
    }
    fn create_popup_window(
        &self,
        parent: ObjectId,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.texts.insert(widget_id, title.to_string());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, title.to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::PopupWindow,
            },
        );
        widget_id
    }
    fn create_text_edit(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.texts.insert(widget_id, text.to_string());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, true); // TextEdit enables IME by default
        state
            .accessibility_names
            .insert(widget_id, text.to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::TextEdit,
            },
        );
        widget_id
    }
    fn create_rich_edit(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.texts.insert(widget_id, text.to_string());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, true); // RichEdit enables IME by default
        state
            .accessibility_names
            .insert(widget_id, text.to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::RichEdit,
            },
        );
        widget_id
    }
    fn create_spin_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.texts.insert(widget_id, "0".to_string());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, true); // SpinBox enables IME by default
        state
            .accessibility_names
            .insert(widget_id, "SpinBox".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::SpinBox,
            },
        );
        widget_id
    }
    fn create_list_view(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "ListView".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::ListView,
            },
        );
        widget_id
    }
    fn create_tree_view(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "TreeView".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::TreeView,
            },
        );
        widget_id
    }
    fn create_scroll_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "ScrollBar".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::ScrollBar,
            },
        );
        widget_id
    }
    fn create_scroll_area(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "ScrollArea".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::ScrollArea,
            },
        );
        widget_id
    }
    fn create_dock_panel(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "DockPanel".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::DockPanel,
            },
        );
        widget_id
    }
    fn create_group_box(
        &self,
        parent: ObjectId,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.texts.insert(widget_id, title.to_string());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, title.to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::GroupBox,
            },
        );
        widget_id
    }
    fn create_tab_widget(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "TabWidget".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::TabWidget,
            },
        );
        widget_id
    }
    fn create_splitter(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "Splitter".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::Splitter,
            },
        );
        widget_id
    }
    fn create_stack_widget(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "StackWidget".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::StackedWidget,
            },
        );
        widget_id
    }
    fn create_mdi_area(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "MdiArea".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::MdiArea,
            },
        );
        widget_id
    }
    fn create_canvas(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "Canvas".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::Canvas,
            },
        );
        widget_id
    }
    fn create_table(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "Table".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::Table,
            },
        );
        widget_id
    }
    fn create_grid(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "Grid".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::Grid,
            },
        );
        widget_id
    }
    fn create_chart(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "Chart".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::Chart,
            },
        );
        widget_id
    }
}
