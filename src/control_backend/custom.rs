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
        log::warn!("shallow implementation: DataView is an alias for TableWidget");
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
