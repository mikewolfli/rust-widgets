//! Mobile phase-1 platform slice (Android baseline).
use super::state::BackendState;
use super::{
    MobileBackend, MobilePlatformExtension, Platform, WidgetTriggerEvent, WidgetTriggerKind,
};
use crate::core::{ObjectId, PlatformFamily};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
/// Logical handle kinds used by mobile baseline state model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum MobileHandleKind {
    Window,
    Button,
    LineEdit,
    Label,
    CheckBox,
    RadioButton,
    Slider,
    ProgressBar,
    ComboBox,
    ListBox,
    Panel,
    MenuBar,
    Menu,
    MenuItem,
    ToolBar,
    StatusBar,
}
#[derive(Default)]
struct MobileMenuState {
    attached_menu_bar: HashMap<ObjectId, ObjectId>,
    menu_children: HashMap<ObjectId, Vec<ObjectId>>,
}
/// Baseline Android mobile platform adapter.
pub struct AndroidMobilePlatform {
    state: BackendState<MobileHandleKind>,
    attached_native_view: AtomicUsize,
    menus: Mutex<MobileMenuState>,
    combo_items: Mutex<HashMap<ObjectId, Vec<String>>>,
    combo_current_index: Mutex<HashMap<ObjectId, Option<usize>>>,
    list_items: Mutex<HashMap<ObjectId, Vec<String>>>,
    list_current_index: Mutex<HashMap<ObjectId, Option<usize>>>,
}
impl AndroidMobilePlatform {
    /// Creates a new Android mobile platform adapter.
    pub fn new() -> Self {
        Self {
            state: BackendState::new(),
            attached_native_view: AtomicUsize::new(0),
            menus: Mutex::new(MobileMenuState::default()),
            combo_items: Mutex::new(HashMap::new()),
            combo_current_index: Mutex::new(HashMap::new()),
            list_items: Mutex::new(HashMap::new()),
            list_current_index: Mutex::new(HashMap::new()),
        }
    }
}
impl Default for AndroidMobilePlatform {
    fn default() -> Self {
        Self::new()
    }
}
impl AndroidMobilePlatform {
    /// Insert one widget into the mobile state table.
    fn insert_widget(
        &self,
        kind: MobileHandleKind,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.state.create_widget(kind, text, x, y, width, height)
    }
    #[allow(clippy::too_many_arguments)]
    fn create_child_widget(
        &self,
        parent: ObjectId,
        kind: MobileHandleKind,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if !self.state.contains_widget(parent) {
            return 0;
        }
        self.insert_widget(kind, text, x, y, width, height)
    }
    /// Returns currently attached native view handle when present.
    pub fn attached_native_view(&self) -> Option<usize> {
        let handle = self.attached_native_view.load(Ordering::SeqCst);
        if handle == 0 {
            None
        } else {
            Some(handle)
        }
    }
    fn kind_of(&self, id: ObjectId) -> Option<MobileHandleKind> {
        self.state.kind_of(id)
    }
}
impl Platform for AndroidMobilePlatform {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn backend_name(&self) -> &'static str {
        "android-mobile"
    }
    fn family(&self) -> PlatformFamily {
        PlatformFamily::Mobile
    }
    fn init(&self) {
        log::info!("[mobile] AndroidMobilePlatform init (state-only preview backend)");
    }
    fn run(&self) {
        log::info!("[mobile] AndroidMobilePlatform run (state-only preview backend)");
    }
    fn quit(&self) {
        log::info!("[mobile] AndroidMobilePlatform quit");
    }
    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        self.insert_widget(MobileHandleKind::Window, title, x, y, width, height)
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
        self.create_child_widget(parent, MobileHandleKind::Button, text, x, y, width, height)
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
        self.create_child_widget(
            parent,
            MobileHandleKind::LineEdit,
            text,
            x,
            y,
            width,
            height,
        )
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
        self.create_child_widget(parent, MobileHandleKind::Label, text, x, y, width, height)
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
        self.create_child_widget(
            parent,
            MobileHandleKind::CheckBox,
            text,
            x,
            y,
            width,
            height,
        )
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
        self.create_child_widget(
            parent,
            MobileHandleKind::RadioButton,
            text,
            x,
            y,
            width,
            height,
        )
    }
    fn create_slider(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        self.create_child_widget(
            parent,
            MobileHandleKind::Slider,
            "Slider",
            x,
            y,
            width,
            height,
        )
    }
    fn create_progress_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.create_child_widget(
            parent,
            MobileHandleKind::ProgressBar,
            "ProgressBar",
            x,
            y,
            width,
            height,
        )
    }
    fn create_combo_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let id = self.create_child_widget(
            parent,
            MobileHandleKind::ComboBox,
            "ComboBox",
            x,
            y,
            width,
            height,
        );
        if id != 0 {
            self.combo_items
                .lock()
                .expect("mobile combo lock poisoned")
                .entry(id)
                .or_default();
            self.combo_current_index
                .lock()
                .expect("mobile combo index lock poisoned")
                .entry(id)
                .or_insert(None);
        }
        id
    }
    fn create_list_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let id = self.create_child_widget(
            parent,
            MobileHandleKind::ListBox,
            "ListBox",
            x,
            y,
            width,
            height,
        );
        if id != 0 {
            self.list_items
                .lock()
                .expect("mobile list lock poisoned")
                .entry(id)
                .or_default();
            self.list_current_index
                .lock()
                .expect("mobile list index lock poisoned")
                .entry(id)
                .or_insert(None);
        }
        id
    }
    fn list_box_add_item(&self, list_box: ObjectId, text: &str) -> bool {
        if !matches!(self.kind_of(list_box), Some(MobileHandleKind::ListBox)) {
            return false;
        }
        self.list_items
            .lock()
            .expect("mobile list lock poisoned")
            .entry(list_box)
            .or_default()
            .push(text.to_string());
        true
    }
    fn list_box_remove_item(&self, list_box: ObjectId, index: usize) -> bool {
        if !matches!(self.kind_of(list_box), Some(MobileHandleKind::ListBox)) {
            return false;
        }
        let mut items = self.list_items.lock().expect("mobile list lock poisoned");
        let Some(vec) = items.get_mut(&list_box) else {
            return false;
        };
        if index >= vec.len() {
            return false;
        }
        vec.remove(index);
        drop(items);
        let mut current = self
            .list_current_index
            .lock()
            .expect("mobile list index lock poisoned");
        if let Some(sel) = current.get_mut(&list_box) {
            *sel = match *sel {
                Some(i) if i == index => None,
                Some(i) if i > index => Some(i - 1),
                other => other,
            };
        }
        true
    }
    fn list_box_clear_items(&self, list_box: ObjectId) -> bool {
        if !matches!(self.kind_of(list_box), Some(MobileHandleKind::ListBox)) {
            return false;
        }
        self.list_items
            .lock()
            .expect("mobile list lock poisoned")
            .entry(list_box)
            .or_default()
            .clear();
        self.list_current_index
            .lock()
            .expect("mobile list index lock poisoned")
            .insert(list_box, None);
        true
    }
    fn list_box_set_current_index(&self, list_box: ObjectId, index: usize) -> bool {
        if !matches!(self.kind_of(list_box), Some(MobileHandleKind::ListBox)) {
            return false;
        }
        let count = self.list_box_item_count(list_box);
        if index >= count {
            return false;
        }
        self.list_current_index
            .lock()
            .expect("mobile list index lock poisoned")
            .insert(list_box, Some(index));
        true
    }
    fn list_box_current_index(&self, list_box: ObjectId) -> Option<usize> {
        if !matches!(self.kind_of(list_box), Some(MobileHandleKind::ListBox)) {
            return None;
        }
        self.list_current_index
            .lock()
            .expect("mobile list index lock poisoned")
            .get(&list_box)
            .copied()
            .flatten()
    }
    fn list_box_item_count(&self, list_box: ObjectId) -> usize {
        if !matches!(self.kind_of(list_box), Some(MobileHandleKind::ListBox)) {
            return 0;
        }
        self.list_items
            .lock()
            .expect("mobile list lock poisoned")
            .get(&list_box)
            .map_or(0, Vec::len)
    }
    fn list_box_item_text(&self, list_box: ObjectId, index: usize) -> Option<String> {
        if !matches!(self.kind_of(list_box), Some(MobileHandleKind::ListBox)) {
            return None;
        }
        self.list_items
            .lock()
            .expect("mobile list lock poisoned")
            .get(&list_box)
            .and_then(|items| items.get(index).cloned())
    }
    fn combo_box_add_item(&self, combo_box: ObjectId, text: &str) -> bool {
        if !matches!(self.kind_of(combo_box), Some(MobileHandleKind::ComboBox)) {
            return false;
        }
        self.combo_items
            .lock()
            .expect("mobile combo lock poisoned")
            .entry(combo_box)
            .or_default()
            .push(text.to_string());
        true
    }
    fn combo_box_clear_items(&self, combo_box: ObjectId) -> bool {
        if !matches!(self.kind_of(combo_box), Some(MobileHandleKind::ComboBox)) {
            return false;
        }
        self.combo_items
            .lock()
            .expect("mobile combo lock poisoned")
            .entry(combo_box)
            .or_default()
            .clear();
        self.combo_current_index
            .lock()
            .expect("mobile combo index lock poisoned")
            .insert(combo_box, None);
        true
    }
    fn combo_box_set_current_index(&self, combo_box: ObjectId, index: usize) -> bool {
        if !matches!(self.kind_of(combo_box), Some(MobileHandleKind::ComboBox)) {
            return false;
        }
        let count = self.combo_box_item_count(combo_box);
        if index >= count {
            return false;
        }
        self.combo_current_index
            .lock()
            .expect("mobile combo index lock poisoned")
            .insert(combo_box, Some(index));
        true
    }
    fn combo_box_current_index(&self, combo_box: ObjectId) -> Option<usize> {
        if !matches!(self.kind_of(combo_box), Some(MobileHandleKind::ComboBox)) {
            return None;
        }
        self.combo_current_index
            .lock()
            .expect("mobile combo index lock poisoned")
            .get(&combo_box)
            .copied()
            .flatten()
    }
    fn combo_box_item_count(&self, combo_box: ObjectId) -> usize {
        if !matches!(self.kind_of(combo_box), Some(MobileHandleKind::ComboBox)) {
            return 0;
        }
        self.combo_items
            .lock()
            .expect("mobile combo lock poisoned")
            .get(&combo_box)
            .map_or(0, Vec::len)
    }
    fn combo_box_item_text(&self, combo_box: ObjectId, index: usize) -> Option<String> {
        if !matches!(self.kind_of(combo_box), Some(MobileHandleKind::ComboBox)) {
            return None;
        }
        self.combo_items
            .lock()
            .expect("mobile combo lock poisoned")
            .get(&combo_box)
            .and_then(|items| items.get(index).cloned())
    }
    fn create_panel(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        self.create_child_widget(
            parent,
            MobileHandleKind::Panel,
            "Panel",
            x,
            y,
            width,
            height,
        )
    }
    fn create_menu_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if !matches!(self.kind_of(parent), Some(MobileHandleKind::Window)) {
            return 0;
        }
        self.insert_widget(MobileHandleKind::MenuBar, "MenuBar", x, y, width, height)
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
        if !matches!(
            self.kind_of(parent),
            Some(MobileHandleKind::MenuBar | MobileHandleKind::Menu)
        ) {
            return 0;
        }
        let id = self.insert_widget(MobileHandleKind::Menu, text, x, y, width, height);
        self.menus
            .lock()
            .expect("mobile menu lock poisoned")
            .menu_children
            .entry(parent)
            .or_default()
            .push(id);
        id
    }
    fn attach_menu_bar_to_window(&self, window: ObjectId, menu_bar: ObjectId) -> bool {
        if matches!(self.kind_of(window), Some(MobileHandleKind::Window))
            && matches!(self.kind_of(menu_bar), Some(MobileHandleKind::MenuBar))
        {
            self.menus
                .lock()
                .expect("mobile menu lock poisoned")
                .attached_menu_bar
                .insert(window, menu_bar);
            return true;
        }
        false
    }
    fn menu_add_item(
        &self,
        parent_menu: ObjectId,
        text: &str,
        _shortcut: Option<&str>,
    ) -> ObjectId {
        if !matches!(self.kind_of(parent_menu), Some(MobileHandleKind::Menu)) {
            return 0;
        }
        let item_id = self.insert_widget(MobileHandleKind::MenuItem, text, 0, 0, 0, 0);
        self.menus
            .lock()
            .expect("mobile menu lock poisoned")
            .menu_children
            .entry(parent_menu)
            .or_default()
            .push(item_id);
        item_id
    }
    fn create_tool_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if !matches!(self.kind_of(parent), Some(MobileHandleKind::Window)) {
            return 0;
        }
        self.insert_widget(MobileHandleKind::ToolBar, "ToolBar", x, y, width, height)
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
        if !matches!(self.kind_of(parent), Some(MobileHandleKind::Window)) {
            return 0;
        }
        self.insert_widget(MobileHandleKind::StatusBar, text, x, y, width, height)
    }
    fn show_widget(&self, widget_id: ObjectId) {
        self.state.set_visible(widget_id, true);
    }
    fn hide_widget(&self, widget_id: ObjectId) {
        self.state.set_visible(widget_id, false);
    }
    fn set_widget_geometry(&self, widget_id: ObjectId, x: i32, y: i32, width: u32, height: u32) {
        self.state.set_geometry(widget_id, x, y, width, height);
    }
    fn set_widget_text(&self, widget_id: ObjectId, text: &str) {
        let _ = self.state.set_text(widget_id, text);
    }
    fn get_widget_text(&self, widget_id: ObjectId) -> String {
        self.state.text(widget_id)
    }
    fn set_widget_enabled(&self, widget_id: ObjectId, enabled: bool) {
        self.state.set_enabled(widget_id, enabled);
    }
    fn is_widget_enabled(&self, widget_id: ObjectId) -> bool {
        self.state.enabled(widget_id)
    }
    fn set_widget_visible(&self, widget_id: ObjectId, visible: bool) {
        self.state.set_visible(widget_id, visible);
    }
    fn is_widget_visible(&self, widget_id: ObjectId) -> bool {
        self.state.visible(widget_id)
    }
    fn poll_menu_triggered(&self) -> Option<ObjectId> {
        self.state.pop_menu_event()
    }
    fn inject_menu_trigger(&self, menu_item_id: ObjectId) -> bool {
        if !matches!(self.kind_of(menu_item_id), Some(MobileHandleKind::MenuItem)) {
            return false;
        }
        self.state.push_menu_event(menu_item_id);
        true
    }
    fn poll_widget_triggered(&self) -> Option<ObjectId> {
        self.poll_widget_trigger_event()
            .map(|event| event.widget_id)
    }
    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        self.state.pop_widget_event()
    }
    fn inject_widget_trigger_event(&self, widget_id: ObjectId, kind: WidgetTriggerKind) -> bool {
        if !self.state.contains_widget(widget_id) {
            return false;
        }
        self.state
            .push_widget_event(WidgetTriggerEvent { widget_id, kind });
        true
    }
    fn create_message_box(
        &self,
        _parent: ObjectId,
        title: &str,
        _text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.insert_widget(MobileHandleKind::Window, title, x, y, width, height)
    }
    fn create_file_dialog(
        &self,
        _parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.insert_widget(MobileHandleKind::Window, "file_dialog", x, y, width, height)
    }
    fn create_color_dialog(
        &self,
        _parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.insert_widget(
            MobileHandleKind::Window,
            "color_dialog",
            x,
            y,
            width,
            height,
        )
    }
    fn create_font_dialog(
        &self,
        _parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.insert_widget(MobileHandleKind::Window, "font_dialog", x, y, width, height)
    }
    fn create_spin_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.create_child_widget(
            parent,
            MobileHandleKind::LineEdit,
            "spin_box",
            x,
            y,
            width,
            height,
        )
    }
    fn create_list_view(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.create_child_widget(
            parent,
            MobileHandleKind::ListBox,
            "list_view",
            x,
            y,
            width,
            height,
        )
    }
    fn create_scroll_area(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.create_child_widget(
            parent,
            MobileHandleKind::Panel,
            "scroll_area",
            x,
            y,
            width,
            height,
        )
    }
}
impl MobilePlatformExtension for AndroidMobilePlatform {
    fn mobile_backend(&self) -> MobileBackend {
        MobileBackend::Android
    }
    fn attach_to_native_view(&self, native_handle: usize) -> bool {
        if native_handle == 0 {
            return false;
        }
        self.attached_native_view
            .store(native_handle, Ordering::SeqCst);
        true
    }
}
static MOBILE_PLATFORM: OnceLock<AndroidMobilePlatform> = OnceLock::new();
/// Returns process-global mobile platform singleton.
pub fn get_mobile_platform() -> &'static AndroidMobilePlatform {
    MOBILE_PLATFORM.get_or_init(AndroidMobilePlatform::new)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mobile_backend_creates_extended_controls() {
        let platform = AndroidMobilePlatform::new();
        let window = platform.create_window("mobile", 0, 0, 320, 480);
        assert_ne!(window, 0);
        let line_edit = platform.create_line_edit(window, "name", 10, 10, 120, 24);
        let label = platform.create_label(window, "label", 10, 40, 120, 24);
        let checkbox = platform.create_checkbox(window, "check", 10, 70, 120, 24);
        let slider = platform.create_slider(window, 10, 100, 160, 24);
        assert_ne!(line_edit, 0);
        assert_ne!(label, 0);
        assert_ne!(checkbox, 0);
        assert_ne!(slider, 0);
        assert_eq!(
            platform.state.kind_of(line_edit),
            Some(MobileHandleKind::LineEdit)
        );
        assert_eq!(platform.state.kind_of(label), Some(MobileHandleKind::Label));
        assert_eq!(
            platform.state.kind_of(checkbox),
            Some(MobileHandleKind::CheckBox)
        );
        assert_eq!(
            platform.state.kind_of(slider),
            Some(MobileHandleKind::Slider)
        );
    }
    #[test]
    fn mobile_backend_routes_trigger_events_for_extended_controls() {
        let platform = AndroidMobilePlatform::new();
        let window = platform.create_window("mobile", 0, 0, 320, 480);
        let line_edit = platform.create_line_edit(window, "", 10, 10, 120, 24);
        let checkbox = platform.create_checkbox(window, "", 10, 40, 120, 24);
        assert!(platform.inject_widget_trigger_event(line_edit, WidgetTriggerKind::ValueChanged));
        assert!(platform.inject_widget_trigger_event(checkbox, WidgetTriggerKind::Clicked));
        let first = platform
            .poll_widget_trigger_event()
            .expect("first event should exist");
        let second = platform
            .poll_widget_trigger_event()
            .expect("second event should exist");
        assert_eq!(first.widget_id, line_edit);
        assert_eq!(first.kind, WidgetTriggerKind::ValueChanged);
        assert_eq!(second.widget_id, checkbox);
        assert_eq!(second.kind, WidgetTriggerKind::Clicked);
    }
    #[test]
    fn mobile_backend_creates_menu_host_controls_and_validates_triggers() {
        let platform = AndroidMobilePlatform::new();
        let window = platform.create_window("mobile", 0, 0, 320, 480);
        let menu_bar = platform.create_menu_bar(window, 0, 0, 320, 24);
        let menu = platform.create_menu(menu_bar, "File", 0, 0, 100, 24);
        let menu_item = platform.menu_add_item(menu, "Open", None);
        let tool_bar = platform.create_tool_bar(window, 0, 24, 320, 24);
        let status_bar = platform.create_status_bar(window, "Ready", 0, 456, 320, 24);
        assert_ne!(menu_bar, 0);
        assert_ne!(menu, 0);
        assert_ne!(menu_item, 0);
        assert_ne!(tool_bar, 0);
        assert_ne!(status_bar, 0);
        assert!(platform.attach_menu_bar_to_window(window, menu_bar));
        assert!(platform.inject_menu_trigger(menu_item));
        assert_eq!(platform.poll_menu_triggered(), Some(menu_item));
        assert!(!platform.inject_menu_trigger(tool_bar));
    }

    #[test]
    fn mobile_combo_box_item_operations_are_state_backed() {
        let platform = AndroidMobilePlatform::new();
        let window = platform.create_window("mobile", 0, 0, 320, 480);
        let combo = platform.create_combo_box(window, 10, 10, 120, 24);
        assert_ne!(combo, 0);

        assert!(platform.combo_box_add_item(combo, "One"));
        assert!(platform.combo_box_add_item(combo, "Two"));
        assert_eq!(platform.combo_box_item_count(combo), 2);
        assert_eq!(
            platform.combo_box_item_text(combo, 0).as_deref(),
            Some("One")
        );
        assert_eq!(
            platform.combo_box_item_text(combo, 1).as_deref(),
            Some("Two")
        );

        assert!(platform.combo_box_set_current_index(combo, 1));
        assert_eq!(platform.combo_box_current_index(combo), Some(1));

        assert!(platform.combo_box_clear_items(combo));
        assert_eq!(platform.combo_box_item_count(combo), 0);
        assert_eq!(platform.combo_box_current_index(combo), None);
    }

    #[test]
    fn mobile_list_box_item_operations_are_state_backed() {
        let platform = AndroidMobilePlatform::new();
        let window = platform.create_window("mobile", 0, 0, 320, 480);
        let list = platform.create_list_box(window, 10, 10, 120, 120);
        assert_ne!(list, 0);

        assert!(platform.list_box_add_item(list, "A"));
        assert!(platform.list_box_add_item(list, "B"));
        assert!(platform.list_box_add_item(list, "C"));
        assert_eq!(platform.list_box_item_count(list), 3);
        assert_eq!(platform.list_box_item_text(list, 2).as_deref(), Some("C"));

        assert!(platform.list_box_set_current_index(list, 2));
        assert_eq!(platform.list_box_current_index(list), Some(2));

        assert!(platform.list_box_remove_item(list, 1));
        assert_eq!(platform.list_box_item_count(list), 2);
        assert_eq!(platform.list_box_item_text(list, 1).as_deref(), Some("C"));
        assert_eq!(platform.list_box_current_index(list), Some(1));

        assert!(platform.list_box_clear_items(list));
        assert_eq!(platform.list_box_item_count(list), 0);
        assert_eq!(platform.list_box_current_index(list), None);
    }
}
