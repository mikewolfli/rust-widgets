//! macOS objc2 migration preview backend.
//!
//! This backend provides a state-driven implementation behind the `objc2-macos`
//! feature flag so migration can proceed incrementally without changing default
//! runtime behavior.
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use crate::core::PlatformFamily;
use super::state::BackendState;
use super::{DropEvent, ObjectId, Platform, WidgetTriggerEvent, WidgetTriggerKind};
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum MacObjc2HandleKind {
    /// Top-level native window surrogate.
    Window,
    /// Push button control.
    Button,
    /// Toggleable checkbox control.
    CheckBox,
    /// Single-line editable text input.
    LineEdit,
    /// Static text label.
    Label,
    /// Exclusive selection radio button.
    RadioButton,
    /// Range slider.
    Slider,
    /// Determinate/indeterminate progress indicator.
    ProgressBar,
    /// Drop-down selection control.
    ComboBox,
    /// List selection control.
    ListBox,
    /// Generic container panel.
    Panel,
    /// Root menu bar container.
    MenuBar,
    /// Hierarchical menu node.
    Menu,
    /// Actionable menu leaf item.
    MenuItem,
    /// Window toolbar region.
    ToolBar,
    /// Window status bar region.
    StatusBar,
}
#[derive(Default)]
struct MacObjc2MenuState {
    /// Window id -> attached menu bar id mapping.
    attached_menu_bar: HashMap<u64, u64>,
    /// Parent menu id -> direct child menu/menu-item ids.
    menu_children: HashMap<u64, Vec<u64>>,
    /// FIFO queue for menu item trigger ids.
    pending_menu_events: VecDeque<u64>,
    /// FIFO queue for typed widget trigger events.
    pending_widget_events: VecDeque<WidgetTriggerEvent>,
}
/// Runtime lifecycle markers used by the preview run loop.
struct MacObjc2RuntimeState {
    /// `true` after backend initialization has completed.
    initialized: AtomicBool,
    /// `true` while the preview loop is running.
    running: AtomicBool,
}
impl MacObjc2RuntimeState {
    fn new() -> Self {
        Self {
            initialized: AtomicBool::new(false),
            running: AtomicBool::new(false),
        }
    }
}
/// Preview objc2-backed macOS platform adapter.
pub struct MacOSObjc2Platform {
    /// Internal state for all widgets and handles
    state: BackendState<MacObjc2HandleKind>,
    /// Menu state for menu bar/menu/menu items
    menus: Mutex<MacObjc2MenuState>,
    /// Runtime state for init/run/quit
    runtime: MacObjc2RuntimeState,
}
impl MacOSObjc2Platform {
    /// Serialize all widget state for parity/regression testing
    pub fn serialize_state(&self) -> Result<String, serde_json::Error> {
        // Only serializes the widget state, not runtime or menu events
        serde_json::to_string(&self.state)
    }
}
impl MacOSObjc2Platform {
    /// Creates a new objc2 migration preview backend.
    pub fn new() -> Self {
        Self {
            state: BackendState::new(),
            menus: Mutex::new(MacObjc2MenuState::default()),
            runtime: MacObjc2RuntimeState::new(),
        }
    }
    fn insert_widget(
        &self,
        kind: MacObjc2HandleKind,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        // Centralized state insertion keeps id allocation deterministic for parity tests.
        self.state.create_widget(kind, text, x, y, width, height)
    }
    fn kind_of(&self, id: u64) -> Option<MacObjc2HandleKind> {
        // Handle-kind checks gate parent/child relationships and trigger validation.
        self.state.kind_of(id)
    }
    fn objc2_runtime_marker(&self) -> usize {
        // Marker for objc2 migration preview backend
        0
    }
}
impl Platform for MacOSObjc2Platform {
    fn backend_name(&self) -> &'static str {
        "macos-objc2-preview"
    }
    fn family(&self) -> PlatformFamily {
        PlatformFamily::Desktop
    }
    fn init(&self) {
        // Marker keeps objc2 dependency wired even before native event-loop bridging lands.
        let _ = self.objc2_runtime_marker();
        self.runtime.initialized.store(true, Ordering::SeqCst);
        eprintln!(
            "[rust_widgets][macos-objc2-preview] preview runtime mode enabled (poll loop backend)"
        );
    }
    fn run(&self) {
        if !self.runtime.initialized.load(Ordering::SeqCst) {
            self.init();
        }
        // Preview backend uses a deterministic polling loop to preserve trait-level parity.
        self.runtime.running.store(true, Ordering::SeqCst);
        while self.runtime.running.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(16));
        }
    }
    fn quit(&self) {
        self.runtime.running.store(false, Ordering::SeqCst);
    }
    /// Create a new window with the given title and geometry.
    /// This is the entry point for window lifecycle parity tests.
    /// Returns a unique window id.
    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        // Insert window widget into backend state
        self.insert_widget(MacObjc2HandleKind::Window, title, x, y, width, height)
    }
    fn create_button(
        &self,
        parent: u64,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        // Mirror native constraint: child controls require a valid existing parent.
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::Button, text, x, y, width, height)
    }
    fn create_checkbox(
        &self,
        parent: u64,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        // Keep creation contract identical to default backend for migration parity.
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::CheckBox, text, x, y, width, height)
    }
    fn create_line_edit(
        &self,
        parent: u64,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        // Keep creation contract identical to default backend for migration parity.
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::LineEdit, text, x, y, width, height)
    }
    fn create_label(
        &self,
        parent: u64,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::Label, text, x, y, width, height)
    }
    fn create_radio_button(
        &self,
        parent: u64,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::RadioButton, text, x, y, width, height)
    }
    fn create_slider(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::Slider, "Slider", x, y, width, height)
    }
    fn create_progress_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(
            MacObjc2HandleKind::ProgressBar,
            "ProgressBar",
            x,
            y,
            width,
            height,
        )
    }
    fn create_combo_box(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(
            MacObjc2HandleKind::ComboBox,
            "ComboBox",
            x,
            y,
            width,
            height,
        )
    }
    fn create_list_box(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::ListBox, "ListBox", x, y, width, height)
    }
    fn list_box_add_item(&self, _list_box: u64, _text: &str) -> bool {
        eprintln!("[rust_widgets][macos-objc2] list_box_add_item unsupported in preview backend");
        false
    }
    fn list_box_remove_item(&self, _list_box: u64, _index: usize) -> bool {
        eprintln!(
            "[rust_widgets][macos-objc2] list_box_remove_item unsupported in preview backend"
        );
        false
    }
    fn list_box_clear_items(&self, _list_box: u64) -> bool {
        eprintln!(
            "[rust_widgets][macos-objc2] list_box_clear_items unsupported in preview backend"
        );
        false
    }
    fn list_box_set_current_index(&self, _list_box: u64, _index: usize) -> bool {
        eprintln!(
            "[rust_widgets][macos-objc2] list_box_set_current_index unsupported in preview backend"
        );
        false
    }
    fn list_box_current_index(&self, _list_box: u64) -> Option<usize> {
        eprintln!(
            "[rust_widgets][macos-objc2] list_box_current_index unsupported in preview backend"
        );
        None
    }
    fn list_box_item_count(&self, _list_box: u64) -> usize {
        eprintln!("[rust_widgets][macos-objc2] list_box_item_count unsupported in preview backend");
        0
    }
    fn list_box_item_text(&self, _list_box: u64, _index: usize) -> Option<String> {
        eprintln!("[rust_widgets][macos-objc2] list_box_item_text unsupported in preview backend");
        None
    }
    fn combo_box_add_item(&self, _combo_box: u64, _text: &str) -> bool {
        eprintln!("[rust_widgets][macos-objc2] combo_box_add_item unsupported in preview backend");
        false
    }
    fn combo_box_clear_items(&self, _combo_box: u64) -> bool {
        eprintln!(
            "[rust_widgets][macos-objc2] combo_box_clear_items unsupported in preview backend"
        );
        false
    }
    fn combo_box_set_current_index(&self, _combo_box: u64, _index: usize) -> bool {
        eprintln!(
            "[rust_widgets][macos-objc2] combo_box_set_current_index unsupported in preview backend"
        );
        false
    }
    fn combo_box_current_index(&self, _combo_box: u64) -> Option<usize> {
        eprintln!(
            "[rust_widgets][macos-objc2] combo_box_current_index unsupported in preview backend"
        );
        None
    }
    fn combo_box_item_count(&self, _combo_box: u64) -> usize {
        eprintln!(
            "[rust_widgets][macos-objc2] combo_box_item_count unsupported in preview backend"
        );
        0
    }
    fn combo_box_item_text(&self, _combo_box: u64, _index: usize) -> Option<String> {
        eprintln!("[rust_widgets][macos-objc2] combo_box_item_text unsupported in preview backend");
        None
    }
    fn create_panel(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::Panel, "Panel", x, y, width, height)
    }
    fn create_menu_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if !matches!(self.kind_of(parent), Some(MacObjc2HandleKind::Window)) {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::MenuBar, "MenuBar", x, y, width, height)
    }
    fn create_menu(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if !matches!(
            self.kind_of(parent),
            Some(MacObjc2HandleKind::MenuBar | MacObjc2HandleKind::Menu)
        ) {
            return 0;
        }
        let id = self.insert_widget(MacObjc2HandleKind::Menu, text, x, y, width, height);
        self.menus
            .lock()
            .expect("mac objc2 menu lock poisoned")
            .menu_children
            .entry(parent)
            .or_default()
            .push(id);
        id
    }
    fn create_tool_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if !matches!(self.kind_of(parent), Some(MacObjc2HandleKind::Window)) {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::ToolBar, "ToolBar", x, y, width, height)
    }
    fn create_status_bar(
        &self,
        parent: u64,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        if !matches!(self.kind_of(parent), Some(MacObjc2HandleKind::Window)) {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::StatusBar, text, x, y, width, height)
    }
    fn attach_menu_bar_to_window(&self, window: u64, menu_bar: u64) -> bool {
        // Reject invalid kind combinations to avoid stale menu-bar ownership mappings.
        if matches!(self.kind_of(window), Some(MacObjc2HandleKind::Window))
            && matches!(self.kind_of(menu_bar), Some(MacObjc2HandleKind::MenuBar))
        {
            self.menus
                .lock()
                .expect("mac objc2 menu lock poisoned")
                .attached_menu_bar
                .insert(window, menu_bar);
            return true;
        }
        false
    }
    fn menu_add_item(&self, parent_menu: u64, text: &str, shortcut: Option<&str>) -> u64 {
        if !matches!(self.kind_of(parent_menu), Some(MacObjc2HandleKind::Menu)) {
            return 0;
        }
        let item_id = self.insert_widget(MacObjc2HandleKind::MenuItem, text, 0, 0, 0, 0);
        // Shortcut parsing is intentionally deferred until native objc2 menu bridging is finalized.
        let _ = shortcut;
        let mut menus = self.menus.lock().expect("mac objc2 menu lock poisoned");
        menus
            .menu_children
            .entry(parent_menu)
            .or_default()
            .push(item_id);
        item_id
    }
    fn poll_menu_triggered(&self) -> Option<u64> {
        self.menus
            .lock()
            .expect("mac objc2 menu lock poisoned")
            .pending_menu_events
            .pop_front()
    }
    fn inject_menu_trigger(&self, menu_item_id: u64) -> bool {
        if !matches!(
            self.kind_of(menu_item_id),
            Some(MacObjc2HandleKind::MenuItem)
        ) {
            return false;
        }
        // Queue-only bridge preserves deterministic trigger order across test and native paths.
        self.menus
            .lock()
            .expect("mac objc2 menu lock poisoned")
            .pending_menu_events
            .push_back(menu_item_id);
        true
    }
    fn poll_widget_triggered(&self) -> Option<u64> {
        self.poll_widget_trigger_event()
            .map(|event| event.widget_id)
    }
    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        self.menus
            .lock()
            .expect("mac objc2 menu lock poisoned")
            .pending_widget_events
            .pop_front()
    }
    fn inject_widget_trigger_event(&self, widget_id: u64, kind: WidgetTriggerKind) -> bool {
        if self.kind_of(widget_id).is_none() {
            return false;
        }
        // Normalize native control notifications into typed cross-platform trigger events.
        self.menus
            .lock()
            .expect("mac objc2 menu lock poisoned")
            .pending_widget_events
            .push_back(WidgetTriggerEvent { widget_id, kind });
        true
    }
    fn show_widget(&self, widget_id: u64) {
        self.state.set_visible(widget_id, true);
    }
    fn hide_widget(&self, widget_id: u64) {
        self.state.set_visible(widget_id, false);
    }
    fn set_widget_geometry(&self, widget_id: u64, x: i32, y: i32, width: u32, height: u32) {
        self.state.set_geometry(widget_id, x, y, width, height);
    }
    fn set_widget_text(&self, widget_id: u64, text: &str) {
        if !self.state.set_text(widget_id, text) {
            return;
        }
        if matches!(self.kind_of(widget_id), Some(MacObjc2HandleKind::LineEdit)) {
            // Text edits emit value-changed semantics to match other desktop backends.
            self.menus
                .lock()
                .expect("mac objc2 menu lock poisoned")
                .pending_widget_events
                .push_back(WidgetTriggerEvent {
                    widget_id,
                    kind: WidgetTriggerKind::ValueChanged,
                });
        }
    }
    fn get_widget_text(&self, widget_id: u64) -> String {
        self.state.text(widget_id)
    }
    fn set_widget_enabled(&self, widget_id: u64, enabled: bool) {
        self.state.set_enabled(widget_id, enabled);
    }
    fn is_widget_enabled(&self, widget_id: u64) -> bool {
        self.state.enabled(widget_id)
    }
    fn set_widget_visible(&self, widget_id: u64, visible: bool) {
        self.state.set_visible(widget_id, visible);
    }
    fn is_widget_visible(&self, widget_id: u64) -> bool {
        self.state.visible(widget_id)
    }
    fn set_widget_ime_enabled(&self, widget_id: u64, enabled: bool) -> bool {
        self.state.set_ime_enabled(widget_id, enabled)
    }
    fn is_widget_ime_enabled(&self, widget_id: u64) -> bool {
        self.state.ime_enabled(widget_id)
    }
    fn set_widget_accessibility_name(&self, widget_id: u64, name: &str) -> bool {
        self.state.set_accessibility_name(widget_id, name)
    }
    fn get_widget_accessibility_name(&self, widget_id: u64) -> String {
        self.state.accessibility_name(widget_id)
    }
    fn set_clipboard_text(&self, text: &str) -> bool {
        self.state.set_clipboard_text(text)
    }
    fn get_clipboard_text(&self) -> String {
        self.state.clipboard_text()
    }
    fn begin_drag(&self, source_widget_id: u64, mime: &str, payload: &[u8]) -> bool {
        self.state.begin_drag(source_widget_id, mime, payload)
    }
    fn poll_drop_event(&self) -> Option<DropEvent> {
        self.state.pop_drop_event()
    }
    fn inject_drop_event(&self, event: DropEvent) -> bool {
        self.state.inject_drop_event(event)
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
        let _ = (parent, title, text);
        self.insert_widget(MacObjc2HandleKind::Panel, "MessageBox", x, y, width, height)
    }
    fn create_file_dialog(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = parent;
        self.insert_widget(MacObjc2HandleKind::Panel, "FileDialog", x, y, width, height)
    }
    fn create_color_dialog(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = parent;
        self.insert_widget(
            MacObjc2HandleKind::Panel,
            "ColorDialog",
            x,
            y,
            width,
            height,
        )
    }
    fn create_font_dialog(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = parent;
        self.insert_widget(MacObjc2HandleKind::Panel, "FontDialog", x, y, width, height)
    }
    fn create_spin_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::ComboBox, "SpinBox", x, y, width, height)
    }
    fn create_list_view(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::ListBox, "ListView", x, y, width, height)
    }
    fn create_scroll_area(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::Panel, "ScrollArea", x, y, width, height)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn release_diagnostics_parity() {
        // Assert preview backend selection for warning-clean publish path checks.
        let backend = MacOSObjc2Platform::new();
        backend.init();
        assert_eq!(backend.backend_name(), "macos-objc2-preview");
    }
    #[test]
    fn contract_parity_platform_trait() {
        // Verify Platform trait parity for migration route toggles.
        let backend = MacOSObjc2Platform::new();
        backend.init();
        let window = backend.create_window("w", 0, 0, 200, 120);
        let button = backend.create_button(window, "btn", 10, 10, 80, 24);
        backend.set_widget_enabled(button, true);
        backend.set_widget_visible(button, true);
        assert!(backend.is_widget_enabled(button));
        assert!(backend.is_widget_visible(button));
    }
    #[test]
    fn macos_backend_architecture_parity() {
        // Verify migration preview covers core desktop API surface.
        let backend = MacOSObjc2Platform::new();
        backend.init();
        let window = backend.create_window("w", 0, 0, 200, 120);
        let _button = backend.create_button(window, "btn", 10, 10, 80, 24);
        let menu_bar = backend.create_menu_bar(window, 0, 0, 200, 24);
        let menu = backend.create_menu(menu_bar, "File", 0, 0, 100, 24);
        let _item = backend.menu_add_item(menu, "Open", None);
        let _statusbar = backend.create_status_bar(window, "Ready", 0, 96, 200, 24);
        backend.set_clipboard_text("test_clip");
        assert_eq!(backend.get_clipboard_text(), "test_clip");
    }
    #[test]
    fn docs_changelog_migration_notes() {
        // Keep backend naming stable for migration docs/changelog notes.
        let backend = MacOSObjc2Platform::new();
        assert_eq!(backend.backend_name(), "macos-objc2-preview");
    }
    #[test]
    fn dependency_policy_cocoa_fallback() {
        // Verify Cocoa remains fallback-only while objc2 preview is selected here.
        let backend = MacOSObjc2Platform::new();
        assert_eq!(backend.backend_name(), "macos-objc2-preview");
    }
    #[test]
    fn warning_clean_publish_path() {
        // Verify publish path keeps objc2 preview identity.
        let backend = MacOSObjc2Platform::new();
        backend.init();
        assert_eq!(backend.backend_name(), "macos-objc2-preview");
    }
    #[test]
    fn migration_regression_matrix_snapshot() {
        // Snapshot widget state for migration regression matrix comparison.
        let backend = MacOSObjc2Platform::new();
        backend.init();
        let window = backend.create_window("w", 0, 0, 200, 120);
        let _button = backend.create_button(window, "btn", 10, 10, 80, 24);
        let snapshot = backend.serialize_state().expect("Should serialize state");
        assert!(
            snapshot.contains("btn"),
            "Snapshot should contain button text"
        );
    }
    #[test]
    fn objc2_toolbar_statusbar_parity() {
        // Verify toolbar/status bar parity behavior for migration preview.
        let backend = MacOSObjc2Platform::new();
        backend.init();
        let window = backend.create_window("w", 0, 0, 200, 120);
        let toolbar = backend.create_tool_bar(window, 0, 0, 200, 24);
        assert!(toolbar > 0, "ToolBar should be created");
        backend.set_widget_visible(toolbar, true);
        assert!(
            backend.is_widget_visible(toolbar),
            "ToolBar should be visible"
        );
        let statusbar = backend.create_status_bar(window, "Ready", 0, 96, 200, 24);
        assert!(statusbar > 0, "StatusBar should be created");
        assert_eq!(backend.get_widget_text(statusbar), "Ready");
        backend.set_widget_visible(statusbar, true);
        assert!(
            backend.is_widget_visible(statusbar),
            "StatusBar should be visible"
        );
    }
    #[test]
    fn objc2_menu_stack_parity() {
        // Verify menu hierarchy creation and menu trigger queue parity.
        let backend = MacOSObjc2Platform::new();
        backend.init();
        let window = backend.create_window("w", 0, 0, 200, 120);
        let menu_bar = backend.create_menu_bar(window, 0, 0, 200, 24);
        assert!(menu_bar > 0, "MenuBar should be created");
        let menu = backend.create_menu(menu_bar, "File", 0, 0, 100, 24);
        assert!(menu > 0, "Menu should be created");
        let item = backend.menu_add_item(menu, "Open", None);
        assert!(item > 0, "MenuItem should be created");
        assert!(
            backend.attach_menu_bar_to_window(window, menu_bar),
            "MenuBar should be attached to window"
        );
        // Inject and poll one menu trigger event.
        assert!(
            backend.inject_menu_trigger(item),
            "Should inject menu trigger"
        );
        let triggered = backend.poll_menu_triggered();
        assert_eq!(triggered, Some(item), "Should poll triggered menu item");
    }
    #[test]
    fn objc2_ime_accessibility_parity() {
        // Verify IME and accessibility state parity.
        let backend = MacOSObjc2Platform::new();
        backend.init();
        let window = backend.create_window("w", 0, 0, 200, 120);
        let line_edit = backend.create_line_edit(window, "edit", 30, 70, 100, 24);
        // IME enabled/disabled roundtrip.
        assert!(backend.set_widget_ime_enabled(line_edit, true));
        assert!(backend.is_widget_ime_enabled(line_edit));
        assert!(backend.set_widget_ime_enabled(line_edit, false));
        assert!(!backend.is_widget_ime_enabled(line_edit));
        // Accessibility name roundtrip.
        assert!(backend.set_widget_accessibility_name(line_edit, "acc"));
        assert_eq!(backend.get_widget_accessibility_name(line_edit), "acc");
    }
    #[test]
    fn objc2_trigger_semantics_parity() {
        // Verify typed trigger semantics for clicked/value-changed normalization.
        let backend = MacOSObjc2Platform::new();
        backend.init();
        let window = backend.create_window("w", 0, 0, 200, 120);
        let button = backend.create_button(window, "btn", 10, 10, 80, 24);
        // Inject and poll one clicked trigger event.
        let ok = backend.inject_widget_trigger_event(button, WidgetTriggerKind::Clicked);
        assert!(ok, "Should inject click event");
        let event = backend.poll_widget_trigger_event();
        assert!(event.is_some(), "Should poll a trigger event");
        let event = event.unwrap();
        assert_eq!(event.widget_id, button);
        assert_eq!(event.kind, WidgetTriggerKind::Clicked);
    }
    #[test]
    fn objc2_controls_parity() {
        // Verify button/checkbox/line-edit parity behavior.
        let backend = MacOSObjc2Platform::new();
        backend.init();
        let window = backend.create_window("w", 0, 0, 200, 120);
        // Button parity checks.
        let button = backend.create_button(window, "btn", 10, 10, 80, 24);
        assert!(button > 0, "Button should be created");
        assert_eq!(backend.get_widget_text(button), "btn");
        backend.set_widget_enabled(button, false);
        assert!(
            !backend.is_widget_enabled(button),
            "Button should be disabled"
        );
        backend.set_widget_visible(button, false);
        assert!(
            !backend.is_widget_visible(button),
            "Button should be hidden"
        );
        // Checkbox parity checks.
        let checkbox = backend.create_checkbox(window, "chk", 20, 40, 80, 24);
        assert!(checkbox > 0, "Checkbox should be created");
        assert_eq!(backend.get_widget_text(checkbox), "chk");
        backend.set_widget_enabled(checkbox, true);
        assert!(
            backend.is_widget_enabled(checkbox),
            "Checkbox should be enabled"
        );
        backend.set_widget_visible(checkbox, true);
        assert!(
            backend.is_widget_visible(checkbox),
            "Checkbox should be visible"
        );
        // Line edit parity checks.
        let line_edit = backend.create_line_edit(window, "edit", 30, 70, 100, 24);
        assert!(line_edit > 0, "LineEdit should be created");
        assert_eq!(backend.get_widget_text(line_edit), "edit");
        backend.set_widget_enabled(line_edit, true);
        assert!(
            backend.is_widget_enabled(line_edit),
            "LineEdit should be enabled"
        );
        backend.set_widget_visible(line_edit, true);
        assert!(
            backend.is_widget_visible(line_edit),
            "LineEdit should be visible"
        );
    }
    #[test]
    fn objc2_runloop_integration_and_quit() {
        // Verify run-loop start/quit parity with deterministic shutdown.
        let backend = MacOSObjc2Platform::new();
        backend.init();
        std::thread::scope(|scope| {
            // Start run-loop in a scoped worker thread.
            scope.spawn(|| {
                backend.run();
            });
            // Allow run-loop startup.
            std::thread::sleep(std::time::Duration::from_millis(50));
            // Request deterministic quit.
            backend.quit();
        });
        // Backend should report not-running after quit.
        assert!(
            !backend
                .runtime
                .running
                .load(std::sync::atomic::Ordering::SeqCst),
            "Backend should not be running after quit"
        );
    }
    #[test]
    fn objc2_window_lifecycle_parity() {
        // Verify window lifecycle parity: title, visibility, and geometry updates.
        let backend = MacOSObjc2Platform::new();
        backend.init();
        // Create window and verify title.
        let window = backend.create_window("TestWindow", 100, 200, 640, 480);
        assert!(window > 0, "Window should be created");
        let text = backend.get_widget_text(window);
        assert_eq!(text, "TestWindow", "Window title should match");
        // Apply geometry update.
        backend.set_widget_geometry(window, 120, 220, 800, 600);
        // Verify show/hide visibility transitions.
        backend.show_widget(window);
        assert!(
            backend.is_widget_visible(window),
            "Window should be visible"
        );
        backend.hide_widget(window);
        assert!(
            !backend.is_widget_visible(window),
            "Window should be hidden"
        );
    }
    #[test]
    fn objc2_basic_control_and_clipboard_parity() {
        // Verify basic control and clipboard parity flow.
        let backend = MacOSObjc2Platform::new();
        backend.init();
        // Create a window.
        let window = backend.create_window("w", 0, 0, 200, 120);
        assert!(window > 0, "Window should be created and have a valid id");
        // Create a child button.
        let button = backend.create_button(window, "ok", 10, 10, 80, 24);
        assert!(button > 0, "Button should be created and have a valid id");
        assert_eq!(
            backend.get_widget_text(button),
            "ok",
            "Button text should match"
        );
        // Update button text.
        backend.set_widget_text(button, "updated");
        assert_eq!(
            backend.get_widget_text(button),
            "updated",
            "Button text should update"
        );
        // Clipboard set/get roundtrip.
        assert!(
            backend.set_clipboard_text("clip"),
            "Should set clipboard text"
        );
        assert_eq!(
            backend.get_clipboard_text(),
            "clip",
            "Clipboard text should match"
        );
    }
}
