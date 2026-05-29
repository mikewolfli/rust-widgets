use crate::control_backend::trait_def::ControlBackend;
use crate::control_backend::types::{
    ControlBackendKind, CustomControlState, CustomWidgetProperties,
};
use crate::core::ObjectId;
use crate::platform::{WidgetTriggerEvent, WidgetTriggerKind};
use crate::widget::WidgetKind;
use std::sync::Mutex;
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
                widget_kind: WidgetKind::MenuItem,
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
    fn create_toggle_button(
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
                widget_kind: WidgetKind::ToggleButton,
            },
        );
        widget_id
    }
    fn create_check_list_box(
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
            .insert(widget_id, "CheckListBox".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::CheckListBox,
            },
        );
        widget_id
    }
    fn create_double_spin_box(
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
        state.texts.insert(widget_id, "0.0".to_string());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, true);
        state
            .accessibility_names
            .insert(widget_id, "DoubleSpinBox".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::DoubleSpinBox,
            },
        );
        widget_id
    }
    fn create_dial(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
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
            .insert(widget_id, "Dial".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::Dial,
            },
        );
        widget_id
    }
    fn create_wizard(
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
                widget_kind: WidgetKind::Wizard,
            },
        );
        widget_id
    }
    fn create_date_picker(
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
            .insert(widget_id, "DatePicker".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::DatePicker,
            },
        );
        widget_id
    }
    fn create_time_picker(
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
            .insert(widget_id, "TimePicker".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::TimePicker,
            },
        );
        widget_id
    }
    fn create_date_time_picker(
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
            .insert(widget_id, "DateTimePicker".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::DateTimePicker,
            },
        );
        widget_id
    }
    fn create_directory_dialog(
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
                widget_kind: WidgetKind::DirectoryDialog,
            },
        );
        widget_id
    }
    fn create_data_view(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        log::warn!("shallow implementation: DataView maps to virtualized data-view host");
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
            .insert(widget_id, "DataView".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::DataView,
            },
        );
        widget_id
    }
    fn create_property_grid(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        log::warn!("shallow implementation: PropertyGrid is an alias for TreeView");
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
            .insert(widget_id, "PropertyGrid".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::PropertyGrid,
            },
        );
        widget_id
    }
    fn create_toolbox(
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
            .insert(widget_id, "Toolbox".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::Toolbox,
            },
        );
        widget_id
    }
    fn create_collapsible_pane(
        &self,
        parent: ObjectId,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        log::warn!("shallow implementation: CollapsiblePane is an alias for Panel");
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
                widget_kind: WidgetKind::CollapsiblePane,
            },
        );
        widget_id
    }
    fn create_dock_widget(
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
                widget_kind: WidgetKind::DockWidget,
            },
        );
        widget_id
    }
    fn create_web_view(
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
            .insert(widget_id, "WebView".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::WebView,
            },
        );
        widget_id
    }
    fn create_activity_indicator(
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
            .insert(widget_id, "ActivityIndicator".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::ActivityIndicator,
            },
        );
        widget_id
    }
    fn create_calendar(
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
            .insert(widget_id, "Calendar".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::Calendar,
            },
        );
        widget_id
    }
    fn create_column_view(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        log::warn!("shallow implementation: ColumnView is an alias for TreeView");
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
            .insert(widget_id, "ColumnView".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::ColumnView,
            },
        );
        widget_id
    }
    fn create_undo_view(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        log::warn!("shallow implementation: UndoView is an alias for ListView");
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
            .insert(widget_id, "UndoView".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::UndoView,
            },
        );
        widget_id
    }
    fn create_command_link(
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
                widget_kind: WidgetKind::CommandLink,
            },
        );
        widget_id
    }
    fn create_lcd_number(
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
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "LCDNumber".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::LCDNumber,
            },
        );
        widget_id
    }
    fn create_font_combo_box(
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
            .insert(widget_id, "FontComboBox".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::FontComboBox,
            },
        );
        widget_id
    }
    fn create_web_engine_view(
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
            .insert(widget_id, "WebEngineView".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::WebEngineView,
            },
        );
        widget_id
    }
    fn create_web_engine_page(
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
            .insert(widget_id, "WebEnginePage".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::WebEnginePage,
            },
        );
        widget_id
    }
    fn create_web_engine_settings(
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
            .insert(widget_id, "WebEngineSettings".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::WebEngineSettings,
            },
        );
        widget_id
    }
    fn create_web_engine_download_item(
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
            .insert(widget_id, "WebEngineDownloadItem".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::WebEngineDownloadItem,
            },
        );
        widget_id
    }
    fn create_web_engine_cookie_store(
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
            .insert(widget_id, "WebEngineCookieStore".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::WebEngineCookieStore,
            },
        );
        widget_id
    }
    fn create_web_engine_web_channel(
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
            .insert(widget_id, "WebEngineWebChannel".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::WebEngineWebChannel,
            },
        );
        widget_id
    }
    fn create_web_engine_find_text_result(
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
            .insert(widget_id, "WebEngineFindTextResult".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::WebEngineFindTextResult,
            },
        );
        widget_id
    }
    fn create_web_engine_notification(
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
            .insert(widget_id, "WebEngineNotification".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::WebEngineNotification,
            },
        );
        widget_id
    }
    fn create_web_engine_script_dialog(
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
            .insert(widget_id, "WebEngineScriptDialog".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::WebEngineScriptDialog,
            },
        );
        widget_id
    }
    fn create_web_engine_context_menu_request(
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
            .insert(widget_id, "WebEngineContextMenuRequest".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::WebEngineContextMenuRequest,
            },
        );
        widget_id
    }
    fn create_action(
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
                widget_kind: WidgetKind::Action,
            },
        );
        widget_id
    }
    fn create_tool_button(
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
                widget_kind: WidgetKind::ToolButton,
            },
        );
        widget_id
    }
    fn create_tool_box(
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
            .insert(widget_id, "ToolBox".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::ToolBox,
            },
        );
        widget_id
    }
    fn create_context_menu(
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
                widget_kind: WidgetKind::ContextMenu,
            },
        );
        widget_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control_backend::types::ControlBackendKind;

    #[test]
    fn custom_paint_control_backend_new_creates_valid_instance() {
        let backend = CustomPaintControlBackend::new();
        assert_eq!(backend.backend_name(), "custom-paint-control-backend");
        assert_eq!(backend.kind(), ControlBackendKind::Custom);
    }

    #[test]
    fn custom_paint_control_backend_default() {
        let backend = CustomPaintControlBackend::default();
        assert_eq!(backend.backend_name(), "custom-paint-control-backend");
        assert_eq!(backend.kind(), ControlBackendKind::Custom);
    }

    #[test]
    fn create_window_allocates_valid_id() {
        let backend = CustomPaintControlBackend::new();
        let id = backend.create_window("Test Window", 0, 0, 800, 600);
        assert_ne!(id, 0, "custom backend must allocate non-zero widget IDs");
    }

    #[test]
    fn create_window_sets_text() {
        let backend = CustomPaintControlBackend::new();
        let id = backend.create_window("Hello Window", 10, 20, 640, 480);
        let text = backend.get_widget_text(id);
        assert_eq!(text, "Hello Window");
    }

    #[test]
    fn create_window_is_enabled_and_visible() {
        let backend = CustomPaintControlBackend::new();
        let id = backend.create_window("Test", 0, 0, 100, 100);
        assert!(backend.is_widget_enabled(id));
        assert!(backend.is_widget_visible(id));
    }

    #[test]
    fn create_button_allocates_valid_id() {
        let backend = CustomPaintControlBackend::new();
        let id = backend.create_button(0, "Click", 10, 20, 100, 30);
        assert_ne!(id, 0);
    }

    #[test]
    fn create_button_sets_text() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 400, 300);
        let id = backend.create_button(parent, "Submit", 50, 50, 80, 25);
        let text = backend.get_widget_text(id);
        assert_eq!(text, "Submit");
    }

    #[test]
    fn create_button_is_enabled_and_visible() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 400, 300);
        let id = backend.create_button(parent, "OK", 0, 0, 80, 25);
        assert!(backend.is_widget_enabled(id));
        assert!(backend.is_widget_visible(id));
    }

    #[test]
    fn create_label_allocates_valid_id() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 400, 300);
        let id = backend.create_label(parent, "Hello", 10, 10, 200, 20);
        assert_ne!(id, 0);
    }

    #[test]
    fn create_label_sets_text() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 400, 300);
        let id = backend.create_label(parent, "Hello World", 10, 10, 200, 20);
        assert_eq!(backend.get_widget_text(id), "Hello World");
    }

    #[test]
    fn create_checkbox_sets_text() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 400, 300);
        let id = backend.create_checkbox(parent, "Enable feature", 10, 10, 150, 25);
        assert_ne!(id, 0);
        assert_eq!(backend.get_widget_text(id), "Enable feature");
    }

    #[test]
    fn create_radio_button_sets_text() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 400, 300);
        let id = backend.create_radio_button(parent, "Option A", 10, 10, 150, 25);
        assert_ne!(id, 0);
        assert_eq!(backend.get_widget_text(id), "Option A");
    }

    #[test]
    fn widget_ids_are_incremental() {
        let backend = CustomPaintControlBackend::new();
        let id1 = backend.create_window("A", 0, 0, 100, 100);
        let id2 = backend.create_window("B", 0, 0, 100, 100);
        let id3 = backend.create_button(0, "C", 0, 0, 50, 20);
        assert!(
            id1 < id2,
            "first alloc id ({}) must be < second ({})",
            id1,
            id2
        );
        assert!(
            id2 < id3,
            "second alloc id ({}) must be < third ({})",
            id2,
            id3
        );
    }

    #[test]
    fn create_multiple_widgets_independent_state() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 500, 400);
        let btn1 = backend.create_button(parent, "Btn1", 0, 0, 50, 20);
        let btn2 = backend.create_button(parent, "Btn2", 60, 0, 50, 20);
        assert_eq!(backend.get_widget_text(btn1), "Btn1");
        assert_eq!(backend.get_widget_text(btn2), "Btn2");
        backend.set_widget_text(btn1, "Updated");
        assert_eq!(backend.get_widget_text(btn1), "Updated");
        assert_eq!(backend.get_widget_text(btn2), "Btn2");
    }

    #[test]
    fn set_and_get_widget_text() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 400, 300);
        let id = backend.create_button(parent, "Original", 0, 0, 100, 30);
        assert_eq!(backend.get_widget_text(id), "Original");
        backend.set_widget_text(id, "Modified");
        assert_eq!(backend.get_widget_text(id), "Modified");
    }

    #[test]
    fn set_and_get_widget_enabled() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 400, 300);
        let id = backend.create_button(parent, "Test", 0, 0, 100, 30);
        assert!(backend.is_widget_enabled(id));
        backend.set_widget_enabled(id, false);
        assert!(!backend.is_widget_enabled(id));
        backend.set_widget_enabled(id, true);
        assert!(backend.is_widget_enabled(id));
    }

    #[test]
    fn set_and_get_widget_visible() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 400, 300);
        let id = backend.create_button(parent, "Test", 0, 0, 100, 30);
        assert!(backend.is_widget_visible(id));
        backend.set_widget_visible(id, false);
        assert!(!backend.is_widget_visible(id));
        backend.set_widget_visible(id, true);
        assert!(backend.is_widget_visible(id));
    }

    #[test]
    fn show_and_hide_widget() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 400, 300);
        let id = backend.create_button(parent, "Test", 0, 0, 100, 30);
        backend.hide_widget(id);
        assert!(!backend.is_widget_visible(id));
        backend.show_widget(id);
        assert!(backend.is_widget_visible(id));
    }

    #[test]
    fn set_and_get_widget_ime_enabled() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 400, 300);
        let id = backend.create_button(parent, "Test", 0, 0, 100, 30);
        assert!(!backend.is_widget_ime_enabled(id));
        assert!(backend.set_widget_ime_enabled(id, true));
        assert!(backend.is_widget_ime_enabled(id));
    }

    #[test]
    fn set_and_get_widget_accessibility_name() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 400, 300);
        let id = backend.create_button(parent, "Test", 0, 0, 100, 30);
        let default_name = backend.get_widget_accessibility_name(id);
        assert_eq!(default_name, "Test");
        assert!(backend.set_widget_accessibility_name(id, "Custom Label"));
        assert_eq!(backend.get_widget_accessibility_name(id), "Custom Label");
    }

    #[test]
    fn create_slider_allocates_valid_id() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 400, 300);
        let id = backend.create_slider(parent, 10, 10, 200, 30);
        assert_ne!(id, 0);
    }

    #[test]
    fn create_progress_bar_allocates_valid_id() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 400, 300);
        let id = backend.create_progress_bar(parent, 10, 10, 300, 20);
        assert_ne!(id, 0);
    }

    #[test]
    fn create_line_edit_sets_text() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 400, 300);
        let id = backend.create_line_edit(parent, "default text", 10, 10, 200, 25);
        assert_ne!(id, 0);
        assert_eq!(backend.get_widget_text(id), "default text");
    }

    #[test]
    fn create_combo_box_allocates_valid_id() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 400, 300);
        let id = backend.create_combo_box(parent, 10, 10, 150, 25);
        assert_ne!(id, 0);
    }

    #[test]
    fn create_list_box_allocates_valid_id() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 400, 300);
        let id = backend.create_list_box(parent, 10, 10, 150, 100);
        assert_ne!(id, 0);
    }

    #[test]
    fn create_panel_allocates_valid_id() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 400, 300);
        let id = backend.create_panel(parent, 10, 10, 380, 280);
        assert_ne!(id, 0);
    }

    #[test]
    fn create_menu_bar_and_menu() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 800, 600);
        let menu_bar = backend.create_menu_bar(parent, 0, 0, 800, 30);
        assert_ne!(menu_bar, 0);
        let menu = backend.create_menu(parent, "File", 0, 0, 50, 30);
        assert_ne!(menu, 0);
        let item = backend.menu_add_item(menu, "Open", Some("Ctrl+O"));
        assert_ne!(item, 0);
    }

    #[test]
    fn create_tool_bar_and_status_bar() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 800, 600);
        let tool_bar = backend.create_tool_bar(parent, 0, 0, 800, 40);
        assert_ne!(tool_bar, 0);
        let status_bar = backend.create_status_bar(parent, "Ready", 0, 560, 800, 40);
        assert_ne!(status_bar, 0);
        assert_eq!(backend.get_widget_text(status_bar), "Ready");
    }

    #[test]
    fn create_dialog_allocates_valid_id() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 800, 600);
        let id = backend.create_dialog(parent, "Settings", 100, 100, 400, 300);
        assert_ne!(id, 0);
        assert_eq!(backend.get_widget_text(id), "Settings");
    }

    #[test]
    fn create_message_box_allocates_valid_id() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 800, 600);
        let id = backend.create_message_box(parent, "Info", "Hello!", 200, 200, 300, 150);
        assert_ne!(id, 0);
        assert_eq!(backend.get_widget_text(id), "Hello!");
    }

    #[test]
    fn create_file_dialog_allocates_valid_id() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 800, 600);
        let id = backend.create_file_dialog(parent, "Open File", 100, 100, 500, 400);
        assert_ne!(id, 0);
    }

    #[test]
    fn create_color_dialog_allocates_valid_id() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 800, 600);
        let id = backend.create_color_dialog(parent, "Pick Color", 100, 100, 400, 300);
        assert_ne!(id, 0);
    }

    #[test]
    fn attach_menu_bar_to_window_returns_true() {
        let backend = CustomPaintControlBackend::new();
        let window = backend.create_window("Win", 0, 0, 800, 600);
        let menu_bar = backend.create_menu_bar(window, 0, 0, 800, 30);
        let result = backend.attach_menu_bar_to_window(window, menu_bar);
        assert!(result);
    }

    #[test]
    fn poll_and_inject_menu_trigger() {
        let backend = CustomPaintControlBackend::new();
        assert!(backend.poll_menu_triggered().is_none());
        let result = backend.inject_menu_trigger(42);
        assert!(result);
        let triggered = backend.poll_menu_triggered();
        assert_eq!(triggered, Some(42));
        assert!(backend.poll_menu_triggered().is_none());
    }

    #[test]
    fn poll_and_inject_widget_trigger_event() {
        let backend = CustomPaintControlBackend::new();
        assert!(backend.poll_widget_trigger_event().is_none());
        assert!(backend.poll_widget_triggered().is_none());
        let injected = backend.inject_widget_trigger_event(99, WidgetTriggerKind::Clicked);
        assert!(injected);
        let event = backend.poll_widget_trigger_event();
        assert!(event.is_some());
        let event = event.unwrap();
        assert_eq!(event.widget_id, 99);
        let triggered = backend.poll_widget_triggered();
        assert!(triggered.is_none());
    }

    #[test]
    fn set_widget_geometry_updates_properties() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 800, 600);
        let id = backend.create_button(parent, "Btn", 10, 20, 100, 30);
        backend.set_widget_geometry(id, 50, 60, 200, 40);
    }

    #[test]
    fn create_canvas_allocates_valid_id() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 800, 600);
        let id = backend.create_canvas(parent, 0, 0, 400, 300);
        assert_ne!(id, 0);
    }

    #[test]
    fn create_table_allocates_valid_id() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 800, 600);
        let id = backend.create_table(parent, 0, 0, 400, 300);
        assert_ne!(id, 0);
    }

    #[test]
    fn create_grid_allocates_valid_id() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 800, 600);
        let id = backend.create_grid(parent, 0, 0, 400, 300);
        assert_ne!(id, 0);
    }

    #[test]
    fn create_chart_allocates_valid_id() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 800, 600);
        let id = backend.create_chart(parent, 0, 0, 400, 300);
        assert_ne!(id, 0);
    }

    #[test]
    fn create_toggle_button_sets_text() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 800, 600);
        let id = backend.create_toggle_button(parent, "Toggle", 0, 0, 100, 30);
        assert_ne!(id, 0);
        assert_eq!(backend.get_widget_text(id), "Toggle");
    }

    #[test]
    fn create_check_list_box_allocates_valid_id() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 800, 600);
        let id = backend.create_check_list_box(parent, 0, 0, 200, 150);
        assert_ne!(id, 0);
    }

    #[test]
    fn create_double_spin_box_allocates_valid_id() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 800, 600);
        let id = backend.create_double_spin_box(parent, 0, 0, 100, 25);
        assert_ne!(id, 0);
    }

    #[test]
    fn create_dial_allocates_valid_id() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 800, 600);
        let id = backend.create_dial(parent, 0, 0, 100, 100);
        assert_ne!(id, 0);
    }

    #[test]
    fn create_wizard_allocates_valid_id() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 800, 600);
        let id = backend.create_wizard(parent, "Setup Wizard", 100, 100, 500, 400);
        assert_ne!(id, 0);
    }

    #[test]
    fn create_date_picker_allocates_valid_id() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 800, 600);
        let id = backend.create_date_picker(parent, 0, 0, 200, 30);
        assert_ne!(id, 0);
    }

    #[test]
    fn create_time_picker_allocates_valid_id() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 800, 600);
        let id = backend.create_time_picker(parent, 0, 0, 200, 30);
        assert_ne!(id, 0);
    }

    #[test]
    fn create_date_time_picker_allocates_valid_id() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 800, 600);
        let id = backend.create_date_time_picker(parent, 0, 0, 200, 30);
        assert_ne!(id, 0);
    }

    #[test]
    fn create_directory_dialog_allocates_valid_id() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 800, 600);
        let id = backend.create_directory_dialog(parent, "Open Folder", 100, 100, 500, 400);
        assert_ne!(id, 0);
    }

    #[test]
    fn create_data_view_allocates_valid_id() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 800, 600);
        let id = backend.create_data_view(parent, 0, 0, 400, 300);
        assert_ne!(id, 0);
    }

    #[test]
    fn create_property_grid_allocates_valid_id() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 800, 600);
        let id = backend.create_property_grid(parent, 0, 0, 300, 400);
        assert_ne!(id, 0);
    }

    #[test]
    fn create_toolbox_allocates_valid_id() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 800, 600);
        let id = backend.create_toolbox(parent, 0, 0, 200, 400);
        assert_ne!(id, 0);
    }

    #[test]
    fn create_collapsible_pane_sets_text() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 800, 600);
        let id = backend.create_collapsible_pane(parent, "Details", 0, 0, 300, 200);
        assert_ne!(id, 0);
        assert_eq!(backend.get_widget_text(id), "Details");
    }

    #[test]
    fn create_dock_widget_sets_text() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 800, 600);
        let id = backend.create_dock_widget(parent, "Dock Panel", 0, 0, 200, 400);
        assert_ne!(id, 0);
        assert_eq!(backend.get_widget_text(id), "Dock Panel");
    }

    #[test]
    fn create_web_view_allocates_valid_id() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 800, 600);
        let id = backend.create_web_view(parent, 0, 0, 800, 600);
        assert_ne!(id, 0);
    }

    #[test]
    fn create_activity_indicator_allocates_valid_id() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 800, 600);
        let id = backend.create_activity_indicator(parent, 0, 0, 40, 40);
        assert_ne!(id, 0);
    }

    #[test]
    fn create_calendar_allocates_valid_id() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 800, 600);
        let id = backend.create_calendar(parent, 0, 0, 300, 250);
        assert_ne!(id, 0);
    }

    #[test]
    fn create_column_view_allocates_valid_id() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 800, 600);
        let id = backend.create_column_view(parent, 0, 0, 300, 400);
        assert_ne!(id, 0);
    }

    #[test]
    fn create_undo_view_allocates_valid_id() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 800, 600);
        let id = backend.create_undo_view(parent, 0, 0, 200, 300);
        assert_ne!(id, 0);
    }

    #[test]
    fn create_command_link_sets_text() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 800, 600);
        let id = backend.create_command_link(parent, "Open Folder", 0, 0, 300, 40);
        assert_ne!(id, 0);
        assert_eq!(backend.get_widget_text(id), "Open Folder");
    }

    #[test]
    fn create_lcd_number_allocates_valid_id() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 800, 600);
        let id = backend.create_lcd_number(parent, 0, 0, 100, 40);
        assert_ne!(id, 0);
    }

    #[test]
    fn create_font_combo_box_allocates_valid_id() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 800, 600);
        let id = backend.create_font_combo_box(parent, 0, 0, 200, 25);
        assert_ne!(id, 0);
    }

    #[test]
    fn create_action_sets_text() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 800, 600);
        let id = backend.create_action(parent, "Save", 0, 0, 50, 25);
        assert_ne!(id, 0);
        assert_eq!(backend.get_widget_text(id), "Save");
    }

    #[test]
    fn create_tool_button_sets_text() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 800, 600);
        let id = backend.create_tool_button(parent, "Save", 0, 0, 40, 40);
        assert_ne!(id, 0);
        assert_eq!(backend.get_widget_text(id), "Save");
    }

    #[test]
    fn create_tool_box_allocates_valid_id() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 800, 600);
        let id = backend.create_tool_box(parent, 0, 0, 200, 400);
        assert_ne!(id, 0);
    }

    #[test]
    fn create_context_menu_sets_text() {
        let backend = CustomPaintControlBackend::new();
        let parent = backend.create_window("Parent", 0, 0, 800, 600);
        let id = backend.create_context_menu(parent, "Edit", 0, 0, 100, 200);
        assert_ne!(id, 0);
        assert_eq!(backend.get_widget_text(id), "Edit");
    }

    #[test]
    fn send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<CustomPaintControlBackend>();
    }
}
