// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Mobile phase-1 platform slice (Android baseline).
use super::state::BackendState;
use super::{
    MobileBackend, MobilePlatformExtension, Platform, WidgetTriggerEvent, WidgetTriggerKind,
};
use crate::core::{ObjectId, PlatformFamily};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

/// Logical handle kinds that survive the self-drawn widget strategy.
///
/// # BLUE15: the host no longer builds controls
///
/// Widget creation used to allocate one state row per logical control kind
/// (`Button`, `Label`, `CheckBox`, `ListBox`, `ComboBox`, ...). Under the
/// self-drawn strategy the host owes the widget layer a window and a drawing
/// surface, and the library paints every `WidgetKind`, so a per-kind
/// `create_*` has no host object to map onto (BLUE15 #56). Only the handles the
/// host itself still owns are modelled here: the window, plus the menu tree that
/// the Activity materialises through its own menu callbacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum MobileHandleKind {
    /// Top-level window handed to the library as a drawing surface.
    Window,
    /// Root menu bar bound to the host Activity's own menu.
    MenuBar,
    /// Hierarchical menu node.
    Menu,
    /// Actionable menu leaf item.
    MenuItem,
}

/// Mobile platform menu state.
#[derive(Default)]
struct MobileMenuState {
    /// Window id -> attached menu bar id mapping.
    attached_menu_bar: HashMap<ObjectId, ObjectId>,
    /// Parent menu id -> direct child menu/menu-item ids.
    menu_children: HashMap<ObjectId, Vec<ObjectId>>,
}

/// Baseline Android mobile platform adapter.
pub struct AndroidMobilePlatform {
    state: BackendState<MobileHandleKind>,
    attached_native_view: AtomicUsize,
    menus: Mutex<MobileMenuState>,
}
impl AndroidMobilePlatform {
    /// Creates a new Android mobile platform adapter.
    pub fn new() -> Self {
        Self {
            state: BackendState::new(),
            attached_native_view: AtomicUsize::new(0),
            menus: Mutex::new(MobileMenuState::default()),
        }
    }
}
crate::impl_default_via_new!(AndroidMobilePlatform);
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

    /// Reads `MemTotal` from `/proc/meminfo` via [`crate::platform::os_probes`].
    fn total_memory_mb(&self) -> Option<u64> {
        crate::platform::os_probes::total_memory_mb()
    }

    /// Reports whether any battery in `/sys/class/power_supply` is discharging.
    fn is_on_battery(&self) -> bool {
        crate::platform::os_probes::is_on_battery()
    }

    /// Samples RSS over VmSize for this process from `/proc/self/status`.
    fn process_memory_utilization(&self) -> Option<f32> {
        crate::platform::os_probes::process_memory_utilization()
    }

    /// Estimates CPU load as thread count over twice the available cores.
    fn process_cpu_utilization(&self) -> Option<f32> {
        crate::platform::os_probes::process_cpu_utilization()
    }

    /// Android printing goes through the platform print framework via JNI, which
    /// this preview backend does not bind.
    fn spawn_print_job(&self, _job_file: &std::path::Path) -> Result<(), String> {
        Err("Android printing requires the platform print framework (not bound)".to_string())
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
    /// Release every registry entry the backend holds for `widget_id`.
    ///
    /// Besides the authoritative `BackendState` record the mobile backend keeps one
    /// per-widget side table: the menu bookkeeping (`menus`). It must be purged,
    /// otherwise a UI rebuilt in a create/destroy loop would leak one entry per
    /// discarded widget.
    fn destroy_widget(&self, widget_id: ObjectId) -> bool {
        {
            let mut menus = self.menus.lock().expect("mobile menu lock poisoned");
            menus.attached_menu_bar.remove(&widget_id);
            // The widget may be a container in the menu tree: drop both the
            // children it owned and the child entry under its own parent.
            menus.menu_children.remove(&widget_id);
            for children in menus.menu_children.values_mut() {
                children.retain(|child| *child != widget_id);
            }
        }

        // The state record is the authority on whether the widget existed.
        self.state.destroy_widget(widget_id)
    }
    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        self.insert_widget(MobileHandleKind::Window, title, x, y, width, height)
    }

    // ─── Menu model ──────────────────────────────────────────────────────
    //
    // These are NOT control construction. Android has no standalone menu-bar or
    // menu *View*: the host Activity owns the menu and materialises it through
    // `onCreateOptionsMenu` / `onOptionsItemSelected`. What lives here is the
    // in-process model that maps a Rust-side menu tree onto that callback surface,
    // plus an injectable trigger queue so the library's own menu widget can report
    // activations without a UI toolkit of its own.
    //
    // They therefore survive the self-drawing change, exactly as on Android/iOS:
    // the library paints the menu *appearance*, while the host still owns the menu
    // *identity* the OS asks about. `capabilities().native_menu` stays `false`,
    // because no OS menu object is created here.

    fn create_menu_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        // A menu bar is owned by a window.
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
        // A menu hangs off a menu bar or another menu.
        if !matches!(self.kind_of(parent), Some(MobileHandleKind::MenuBar | MobileHandleKind::Menu))
        {
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
        // A menu item must hang off a menu.
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
        // Only a menu item may produce a menu trigger.
        if !matches!(self.kind_of(menu_item_id), Some(MobileHandleKind::MenuItem)) {
            return false;
        }
        self.state.push_menu_event(menu_item_id);
        true
    }
    fn poll_widget_triggered(&self) -> Option<ObjectId> {
        self.poll_widget_trigger_event().map(|event| event.widget_id)
    }
    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        self.state.pop_widget_event()
    }
    fn inject_widget_trigger_event(&self, widget_id: ObjectId, kind: WidgetTriggerKind) -> bool {
        if !self.state.contains_widget(widget_id) {
            return false;
        }
        self.state.push_widget_event(WidgetTriggerEvent { widget_id, kind });
        true
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
        self.attached_native_view.store(native_handle, Ordering::SeqCst);
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

    /// The window is the one primitive the host still owns, so it must keep working.
    #[test]
    fn mobile_backend_creates_window() {
        let platform = AndroidMobilePlatform::new();
        let window = platform.create_window("mobile", 0, 0, 320, 480);
        assert_ne!(window, 0);
        assert_eq!(platform.kind_of(window), Some(MobileHandleKind::Window));
    }

    /// The library paints every control, so the host no longer builds any of them.
    ///
    /// Each `create_*` now falls through to the `Platform` trait default, which
    /// returns `0` even when handed a valid window id. Returning a non-zero handle
    /// here would claim that a host control exists when none does (BLUE15 #56).
    #[test]
    fn mobile_backend_builds_no_controls() {
        let platform = AndroidMobilePlatform::new();
        let window = platform.create_window("mobile", 0, 0, 320, 480);
        assert_ne!(window, 0);

        assert_eq!(platform.create_button(window, "b", 0, 0, 80, 24), 0);
        assert_eq!(platform.create_checkbox(window, "c", 0, 0, 80, 24), 0);
        assert_eq!(platform.create_line_edit(window, "", 0, 0, 80, 24), 0);
        assert_eq!(platform.create_label(window, "l", 0, 0, 80, 24), 0);
        assert_eq!(platform.create_radio_button(window, "r", 0, 0, 80, 24), 0);
        assert_eq!(platform.create_slider(window, 0, 0, 80, 24), 0);
        assert_eq!(platform.create_progress_bar(window, 0, 0, 80, 24), 0);
        assert_eq!(platform.create_combo_box(window, 0, 0, 80, 24), 0);
        assert_eq!(platform.create_list_box(window, 0, 0, 80, 24), 0);
        assert_eq!(platform.create_panel(window, 0, 0, 80, 24), 0);
        assert_eq!(platform.create_tool_bar(window, 0, 0, 80, 24), 0);
        assert_eq!(platform.create_status_bar(window, "s", 0, 0, 80, 24), 0);
        assert_eq!(platform.create_toggle_button(window, "t", 0, 0, 80, 24), 0);
        assert_eq!(platform.create_spin_box(window, 0, 0, 80, 24), 0);
        assert_eq!(platform.create_list_view(window, 0, 0, 80, 24), 0);
        assert_eq!(platform.create_scroll_area(window, 0, 0, 80, 24), 0);
        assert_eq!(platform.create_group_box(window, "g", 0, 0, 80, 24), 0);
        assert_eq!(platform.create_frame(window, 0, 0, 80, 24), 0);
        assert_eq!(platform.create_tab_widget(window, 0, 0, 80, 24), 0);
        assert_eq!(platform.create_splitter(window, 0, 0, 80, 24), 0);
        assert_eq!(platform.create_calendar(window, 0, 0, 80, 24), 0);
        assert_eq!(platform.create_scroll_bar(window, 0, 0, 80, 24), 0);
        assert_eq!(platform.create_double_spin_box(window, 0, 0, 80, 24), 0);
        assert_eq!(platform.create_font_combo_box(window, 0, 0, 80, 24), 0);
        assert_eq!(platform.create_context_menu(window, 0, 0, 80, 24), 0);
        assert_eq!(platform.create_popup_window(window, "p", 0, 0, 80, 24), 0);
        assert_eq!(platform.create_dialog(window, "d", 0, 0, 80, 24), 0);
        assert_eq!(platform.create_input_dialog(window, 0, 0, 80, 24), 0);
        assert_eq!(platform.create_progress_dialog(window, 0, 0, 80, 24), 0);
        assert_eq!(platform.create_directory_dialog(window, "x", 0, 0, 80, 24), 0);
        assert_eq!(platform.create_date_picker(window, 0, 0, 80, 24), 0);
        assert_eq!(platform.create_time_picker(window, 0, 0, 80, 24), 0);
        assert_eq!(platform.create_date_time_picker(window, 0, 0, 80, 24), 0);
        assert_eq!(platform.create_activity_indicator(window, 0, 0, 80, 24), 0);
        assert_eq!(platform.create_message_box(window, "m", "body", 0, 0, 80, 24), 0);
        assert_eq!(platform.create_file_dialog(window, 0, 0, 80, 24), 0);
        assert_eq!(platform.create_color_dialog(window, 0, 0, 80, 24), 0);
        assert_eq!(platform.create_font_dialog(window, 0, 0, 80, 24), 0);
    }

    /// Item storage went away with the controls that owned it.
    ///
    /// A combo box and a list box can no longer be created, so the item operations
    /// have no widget to attach to and must report that nothing was stored.
    #[test]
    fn mobile_backend_tracks_no_control_items() {
        let platform = AndroidMobilePlatform::new();
        let window = platform.create_window("mobile", 0, 0, 320, 480);

        assert!(!platform.combo_box_add_item(window, "One"));
        assert_eq!(platform.combo_box_item_count(window), 0);
        assert_eq!(platform.combo_box_current_index(window), None);
        assert!(!platform.combo_box_set_current_index(window, 0));
        assert!(!platform.combo_box_clear_items(window));

        assert!(!platform.list_box_add_item(window, "A"));
        assert_eq!(platform.list_box_item_count(window), 0);
        assert_eq!(platform.list_box_current_index(window), None);
        assert!(!platform.list_box_set_current_index(window, 0));
        assert!(!platform.list_box_remove_item(window, 0));
        assert!(!platform.list_box_clear_items(window));
    }

    /// Typed trigger events remain a library-side queue, independent of controls.
    ///
    /// The window is a real handle, so it can carry an injected event; an id that
    /// was never created cannot.
    #[test]
    fn mobile_backend_routes_widget_trigger_events() {
        let platform = AndroidMobilePlatform::new();
        let window = platform.create_window("mobile", 0, 0, 320, 480);

        assert!(platform.inject_widget_trigger_event(window, WidgetTriggerKind::ValueChanged));
        let event = platform.poll_widget_trigger_event().expect("event should exist");
        assert_eq!(event.widget_id, window);
        assert_eq!(event.kind, WidgetTriggerKind::ValueChanged);

        assert!(!platform.inject_widget_trigger_event(0, WidgetTriggerKind::Clicked));
        assert!(platform.poll_widget_trigger_event().is_none());
    }

    /// The menu tree is an in-process model the Activity materialises itself.
    #[test]
    fn mobile_backend_models_menu_tree_and_validates_triggers() {
        let platform = AndroidMobilePlatform::new();
        let window = platform.create_window("mobile", 0, 0, 320, 480);
        let menu_bar = platform.create_menu_bar(window, 0, 0, 320, 24);
        let menu = platform.create_menu(menu_bar, "File", 0, 0, 100, 24);
        let menu_item = platform.menu_add_item(menu, "Open", None);

        assert_ne!(menu_bar, 0);
        assert_ne!(menu, 0);
        assert_ne!(menu_item, 0);
        assert_eq!(platform.kind_of(menu_bar), Some(MobileHandleKind::MenuBar));
        assert_eq!(platform.kind_of(menu), Some(MobileHandleKind::Menu));
        assert_eq!(platform.kind_of(menu_item), Some(MobileHandleKind::MenuItem));

        // A menu bar requires a window; a menu requires a menu bar or menu.
        assert_eq!(platform.create_menu_bar(menu, 0, 0, 320, 24), 0);
        assert_eq!(platform.create_menu(window, "Bad", 0, 0, 100, 24), 0);
        assert_eq!(platform.menu_add_item(window, "Bad", None), 0);

        assert!(platform.attach_menu_bar_to_window(window, menu_bar));
        assert!(!platform.attach_menu_bar_to_window(window, menu_item));

        // Only a menu item may be injected as a menu trigger.
        assert!(platform.inject_menu_trigger(menu_item));
        assert!(!platform.inject_menu_trigger(window));
        assert!(!platform.inject_menu_trigger(menu_bar));
        assert_eq!(platform.poll_menu_triggered(), Some(menu_item));
    }

    /// The menu claim is honest: the host owns the menu, the library paints it.
    #[test]
    fn mobile_backend_does_not_claim_a_native_menu() {
        let platform = AndroidMobilePlatform::new();
        assert!(!platform.capabilities().native_menu);
    }

    /// Attaching a native view is the host capability that survived.
    #[test]
    fn mobile_backend_attaches_native_view() {
        let platform = AndroidMobilePlatform::new();
        assert_eq!(platform.attached_native_view(), None);
        assert!(!platform.attach_to_native_view(0));
        assert!(platform.attach_to_native_view(0x1234));
        assert_eq!(platform.attached_native_view(), Some(0x1234));
    }
}
