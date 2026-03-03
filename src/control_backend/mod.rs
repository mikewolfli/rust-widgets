//! Control backend abstraction for native and custom-painted control paths.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

#[cfg(feature = "controls-custom")]
use std::sync::OnceLock;

use crate::core::ObjectId;
use crate::platform::{WidgetTriggerEvent, WidgetTriggerKind, get_platform};
use crate::widget::WidgetKind;

/// Control backend family used by runtime routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlBackendKind {
    /// Native platform control implementation.
    Native,
    /// Custom-painted control implementation.
    Custom,
}

/// Compile-time control route preference for a widget kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlRoutePreference {
    /// Prefer native backend when available.
    NativePreferred,
    /// Require custom-painted backend route.
    CustomRequired,
}

/// Returns the policy preference for one widget kind.
pub fn route_preference_for_widget_kind(kind: WidgetKind) -> ControlRoutePreference {
    match kind {
        WidgetKind::Window
        | WidgetKind::Dialog
        | WidgetKind::MessageBox
        | WidgetKind::FileDialog
        | WidgetKind::ColorDialog
        | WidgetKind::FontDialog
        | WidgetKind::PopupWindow
        | WidgetKind::Button
        | WidgetKind::CheckBox
        | WidgetKind::RadioButton
        | WidgetKind::Label
        | WidgetKind::LineEdit
        | WidgetKind::ComboBox
        | WidgetKind::SpinBox
        | WidgetKind::ListBox
        | WidgetKind::ProgressBar
        | WidgetKind::Slider
        | WidgetKind::ScrollBar
        | WidgetKind::ScrollArea
        | WidgetKind::Panel
        | WidgetKind::GroupBox
        | WidgetKind::TabWidget
        | WidgetKind::Splitter
        | WidgetKind::StackWidget
        | WidgetKind::MenuBar
        | WidgetKind::Menu
        | WidgetKind::ToolBar
        | WidgetKind::StatusBar => ControlRoutePreference::NativePreferred,
        WidgetKind::TextEdit
        | WidgetKind::RichEdit
        | WidgetKind::ListView
        | WidgetKind::TreeView
        | WidgetKind::DockPanel
        | WidgetKind::MdiArea
        | WidgetKind::Canvas
        | WidgetKind::Table
        | WidgetKind::Grid
        | WidgetKind::Chart => ControlRoutePreference::CustomRequired,
    }
}

/// Unified control backend contract.
pub trait ControlBackend: Send + Sync {
    /// Backend display name.
    fn backend_name(&self) -> &'static str;
    /// Backend family kind.
    fn kind(&self) -> ControlBackendKind;

    /// Create top-level window.
    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> ObjectId;
    /// Create button control.
    fn create_button(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create checkbox control.
    fn create_checkbox(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create line edit control.
    fn create_line_edit(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create label control.
    fn create_label(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create radio button control.
    fn create_radio_button(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create slider control.
    fn create_slider(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId;
    /// Create progress bar control.
    fn create_progress_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create combo box control.
    fn create_combo_box(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId;
    /// Create list box control.
    fn create_list_box(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId;
    /// Create panel control.
    fn create_panel(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId;
    /// Create menu bar host control.
    fn create_menu_bar(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId;
    /// Create menu host control.
    fn create_menu(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Attach menu bar to top-level window.
    fn attach_menu_bar_to_window(&self, window: ObjectId, menu_bar: ObjectId) -> bool;
    /// Add menu item to menu host control.
    fn menu_add_item(&self, parent_menu: ObjectId, text: &str, shortcut: Option<&str>) -> ObjectId;
    /// Create tool bar host control.
    fn create_tool_bar(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId;
    /// Create status bar host control.
    fn create_status_bar(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;

    /// Poll next menu trigger id.
    fn poll_menu_triggered(&self) -> Option<ObjectId>;
    /// Inject a menu trigger id.
    fn inject_menu_trigger(&self, menu_item_id: ObjectId) -> bool;
    /// Poll next widget id trigger if available.
    fn poll_widget_triggered(&self) -> Option<ObjectId> {
        self.poll_widget_trigger_event().map(|event| event.widget_id)
    }
    /// Poll next typed widget trigger event.
    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent>;
    /// Inject a typed widget trigger event.
    fn inject_widget_trigger_event(&self, widget_id: ObjectId, kind: WidgetTriggerKind) -> bool;

    /// Set widget text.
    fn set_widget_text(&self, widget_id: ObjectId, text: &str);
    /// Get widget text.
    fn get_widget_text(&self, widget_id: ObjectId) -> String;
    /// Show widget.
    fn show_widget(&self, widget_id: ObjectId) {
        self.set_widget_visible(widget_id, true);
    }
    /// Hide widget.
    fn hide_widget(&self, widget_id: ObjectId) {
        self.set_widget_visible(widget_id, false);
    }
    /// Set widget enabled state.
    fn set_widget_enabled(&self, widget_id: ObjectId, enabled: bool);
    /// Read widget enabled state.
    fn is_widget_enabled(&self, widget_id: ObjectId) -> bool;
    /// Set widget visibility.
    fn set_widget_visible(&self, widget_id: ObjectId, visible: bool);
    /// Read widget visibility state.
    fn is_widget_visible(&self, widget_id: ObjectId) -> bool;
}

/// Native control backend that forwards to platform backend.
pub struct NativeControlBackend;

impl NativeControlBackend {
    /// Create native control backend.
    pub const fn new() -> Self {
        Self
    }
}

impl ControlBackend for NativeControlBackend {
    fn backend_name(&self) -> &'static str {
        "native-control-backend"
    }

    fn kind(&self) -> ControlBackendKind {
        ControlBackendKind::Native
    }

    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        get_platform().create_window(title, x, y, width, height)
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
        get_platform().create_button(parent, text, x, y, width, height)
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
        get_platform().create_checkbox(parent, text, x, y, width, height)
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
        get_platform().create_line_edit(parent, text, x, y, width, height)
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
        get_platform().create_label(parent, text, x, y, width, height)
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
        get_platform().create_radio_button(parent, text, x, y, width, height)
    }

    fn create_slider(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_slider(parent, x, y, width, height)
    }

    fn create_progress_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_progress_bar(parent, x, y, width, height)
    }

    fn create_combo_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_combo_box(parent, x, y, width, height)
    }

    fn create_list_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_list_box(parent, x, y, width, height)
    }

    fn create_panel(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_panel(parent, x, y, width, height)
    }

    fn create_menu_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_menu_bar(parent, x, y, width, height)
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
        get_platform().create_menu(parent, text, x, y, width, height)
    }

    fn attach_menu_bar_to_window(&self, window: ObjectId, menu_bar: ObjectId) -> bool {
        get_platform().attach_menu_bar_to_window(window, menu_bar)
    }

    fn menu_add_item(&self, parent_menu: ObjectId, text: &str, shortcut: Option<&str>) -> ObjectId {
        get_platform().menu_add_item(parent_menu, text, shortcut)
    }

    fn create_tool_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_tool_bar(parent, x, y, width, height)
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
        get_platform().create_status_bar(parent, text, x, y, width, height)
    }

    fn poll_menu_triggered(&self) -> Option<ObjectId> {
        get_platform().poll_menu_triggered()
    }

    fn inject_menu_trigger(&self, menu_item_id: ObjectId) -> bool {
        get_platform().inject_menu_trigger(menu_item_id)
    }

    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        get_platform().poll_widget_trigger_event()
    }

    fn inject_widget_trigger_event(&self, widget_id: ObjectId, kind: WidgetTriggerKind) -> bool {
        get_platform().inject_widget_trigger_event(widget_id, kind)
    }

    fn set_widget_text(&self, widget_id: ObjectId, text: &str) {
        get_platform().set_widget_text(widget_id, text);
    }

    fn get_widget_text(&self, widget_id: ObjectId) -> String {
        get_platform().get_widget_text(widget_id)
    }

    fn set_widget_enabled(&self, widget_id: ObjectId, enabled: bool) {
        get_platform().set_widget_enabled(widget_id, enabled);
    }

    fn is_widget_enabled(&self, widget_id: ObjectId) -> bool {
        get_platform().is_widget_enabled(widget_id)
    }

    fn set_widget_visible(&self, widget_id: ObjectId, visible: bool) {
        get_platform().set_widget_visible(widget_id, visible);
    }

    fn show_widget(&self, widget_id: ObjectId) {
        get_platform().show_widget(widget_id);
    }

    fn hide_widget(&self, widget_id: ObjectId) {
        get_platform().hide_widget(widget_id);
    }

    fn is_widget_visible(&self, widget_id: ObjectId) -> bool {
        get_platform().is_widget_visible(widget_id)
    }
}

#[derive(Default)]
struct CustomControlState {
    next_widget_id: ObjectId,
    texts: HashMap<ObjectId, String>,
    enabled: HashMap<ObjectId, bool>,
    visible: HashMap<ObjectId, bool>,
    menu_trigger_queue: VecDeque<ObjectId>,
    widget_trigger_queue: VecDeque<WidgetTriggerEvent>,
}

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

    fn create_window(&self, _title: &str, _x: i32, _y: i32, _width: u32, _height: u32) -> ObjectId {
        self.alloc_widget_id()
    }

    fn create_button(
        &self,
        _parent: ObjectId,
        _text: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        self.alloc_widget_id()
    }

    fn create_checkbox(
        &self,
        _parent: ObjectId,
        _text: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        self.alloc_widget_id()
    }

    fn create_line_edit(
        &self,
        _parent: ObjectId,
        _text: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        self.alloc_widget_id()
    }

    fn create_label(
        &self,
        _parent: ObjectId,
        _text: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        self.alloc_widget_id()
    }

    fn create_radio_button(
        &self,
        _parent: ObjectId,
        _text: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        self.alloc_widget_id()
    }

    fn create_slider(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        self.alloc_widget_id()
    }

    fn create_progress_bar(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        self.alloc_widget_id()
    }

    fn create_combo_box(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        self.alloc_widget_id()
    }

    fn create_list_box(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        self.alloc_widget_id()
    }

    fn create_panel(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        self.alloc_widget_id()
    }

    fn create_menu_bar(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        self.alloc_widget_id()
    }

    fn create_menu(
        &self,
        _parent: ObjectId,
        _text: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        self.alloc_widget_id()
    }

    fn attach_menu_bar_to_window(&self, _window: ObjectId, _menu_bar: ObjectId) -> bool {
        true
    }

    fn menu_add_item(&self, _parent_menu: ObjectId, _text: &str, _shortcut: Option<&str>) -> ObjectId {
        self.alloc_widget_id()
    }

    fn create_tool_bar(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        self.alloc_widget_id()
    }

    fn create_status_bar(
        &self,
        _parent: ObjectId,
        _text: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        self.alloc_widget_id()
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
}

fn native_control_backend() -> &'static NativeControlBackend {
    static BACKEND: NativeControlBackend = NativeControlBackend::new();
    &BACKEND
}

#[cfg(feature = "controls-custom")]
fn custom_control_backend() -> &'static CustomPaintControlBackend {
    static BACKEND: OnceLock<CustomPaintControlBackend> = OnceLock::new();
    BACKEND.get_or_init(CustomPaintControlBackend::new)
}

/// Return active control backend selected by compile-time features.
#[cfg(all(feature = "controls-native", feature = "controls-custom"))]
pub fn get_control_backend() -> &'static dyn ControlBackend {
    native_control_backend()
}

/// Return active control backend selected by compile-time features.
#[cfg(all(not(feature = "controls-native"), feature = "controls-custom"))]
pub fn get_control_backend() -> &'static dyn ControlBackend {
    custom_control_backend()
}

/// Return active control backend selected by compile-time features.
#[cfg(all(feature = "controls-native", not(feature = "controls-custom")))]
pub fn get_control_backend() -> &'static dyn ControlBackend {
    native_control_backend()
}

/// Return active control backend selected by compile-time features.
#[cfg(all(not(feature = "controls-native"), not(feature = "controls-custom")))]
pub fn get_control_backend() -> &'static dyn ControlBackend {
    native_control_backend()
}

/// Returns control backend resolved by compile-time policy for one widget kind.
#[cfg(all(feature = "controls-native", feature = "controls-custom"))]
pub fn get_control_backend_for_widget(kind: WidgetKind) -> &'static dyn ControlBackend {
    match route_preference_for_widget_kind(kind) {
        ControlRoutePreference::NativePreferred => native_control_backend(),
        ControlRoutePreference::CustomRequired => custom_control_backend(),
    }
}

/// Returns control backend resolved by compile-time policy for one widget kind.
#[cfg(all(not(feature = "controls-native"), feature = "controls-custom"))]
pub fn get_control_backend_for_widget(_kind: WidgetKind) -> &'static dyn ControlBackend {
    custom_control_backend()
}

/// Returns control backend resolved by compile-time policy for one widget kind.
#[cfg(all(feature = "controls-native", not(feature = "controls-custom")))]
pub fn get_control_backend_for_widget(_kind: WidgetKind) -> &'static dyn ControlBackend {
    native_control_backend()
}

/// Returns control backend resolved by compile-time policy for one widget kind.
#[cfg(all(not(feature = "controls-native"), not(feature = "controls-custom")))]
pub fn get_control_backend_for_widget(_kind: WidgetKind) -> &'static dyn ControlBackend {
    native_control_backend()
}

/// Return compile-time control policy label used by diagnostics and docs.
#[cfg(all(feature = "controls-native", feature = "controls-custom"))]
pub fn active_control_policy() -> &'static str {
    "hybrid-native-first"
}

/// Return compile-time control policy label used by diagnostics and docs.
#[cfg(all(not(feature = "controls-native"), feature = "controls-custom"))]
pub fn active_control_policy() -> &'static str {
    "custom-full"
}

/// Return compile-time control policy label used by diagnostics and docs.
#[cfg(all(feature = "controls-native", not(feature = "controls-custom")))]
pub fn active_control_policy() -> &'static str {
    "native-strict"
}

/// Return compile-time control policy label used by diagnostics and docs.
#[cfg(all(not(feature = "controls-native"), not(feature = "controls-custom")))]
pub fn active_control_policy() -> &'static str {
    "native-strict"
}
