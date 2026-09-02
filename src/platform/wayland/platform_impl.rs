//! Wayland backend platform implementation.
//!
//! This module implements the `Platform` trait for the Wayland backend.
//! All widget operations are backed by `BackendState<WaylandHandleKind>`.
//! Native Wayland protocol integration (via wayland-client / wayland-protocols)
//! is wired when the `wayland-native` feature is active.
//!
//! Architecture follows the same pattern as LinuxPlatform / HarmonyPlatform:
//! - State-only operations for all widget creation and lifecycle
//! - Thread-safe interior mutability
//! - Deterministic ID allocation via `insert_widget()`

use crate::core::ObjectId;
use crate::core::PlatformFamily;
use crate::event::EventLoop;
use crate::platform::types::{
    DropEvent, Platform, PlatformCapabilities, WidgetTriggerEvent, WidgetTriggerKind,
};
use crate::platform::wayland::types::{ListData, WaylandHandleKind, WaylandPlatform};

#[cfg(all(feature = "wayland-native", target_os = "linux"))]
use wayland_client as wl_client;

#[cfg(all(feature = "wayland-native", target_os = "linux"))]
use wayland_protocols as wl_protocols;

// ---------------------------------------------------------------------------
// Platform trait implementation
// ---------------------------------------------------------------------------

impl Platform for WaylandPlatform {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn backend_name(&self) -> &'static str {
        "wayland"
    }

    fn family(&self) -> PlatformFamily {
        PlatformFamily::Desktop
    }

    fn capabilities(&self) -> PlatformCapabilities {
        PlatformCapabilities {
            dpi_scaling: true,
            ime: true,
            accessibility: true,
            native_menu: true,
            typed_widget_trigger: true,
        }
    }

    fn dpi_scale_factor(&self) -> f32 {
        // Attempt to detect DPI from environment when wayland-client is not wired.
        // Check common environment variables set by Wayland compositors.
        if let Ok(scale_str) = std::env::var("GDK_SCALE") {
            if let Ok(scale) = scale_str.parse::<f32>() {
                if scale > 0.0 {
                    return scale;
                }
            }
        }
        if let Ok(scale_str) = std::env::var("QT_SCALE_FACTOR") {
            if let Ok(scale) = scale_str.parse::<f32>() {
                if scale > 0.0 {
                    return scale;
                }
            }
        }
        if let Ok(scale_str) = std::env::var("RUST_WIDGETS_DPI_SCALE") {
            if let Ok(scale) = scale_str.parse::<f32>() {
                if scale > 0.0 {
                    return scale;
                }
            }
        }
        // Fallback: assume 96 DPI (1.0 scale factor).
        // When wayland-native feature is active, check the native session
        // for wl_output scale factor obtained from the compositor.
        #[cfg(all(feature = "wayland-native", target_os = "linux"))]
        {
            if let Ok(guard) = self.native_session.lock() {
                if let Some(ref session) = *guard {
                    let scale = session.state.dpi_scale;
                    if scale > 0.0 {
                        return scale;
                    }
                }
            }
        }
        1.0
    }

    // -----------------------------------------------------------------------
    // Initialization / lifecycle
    // -----------------------------------------------------------------------

    fn init(&self) {
        self.runtime.initialized.store(true, std::sync::atomic::Ordering::SeqCst);
        log::info!("[wayland] Platform initialized (state-only backend).");
    }

    fn run(&self) {
        self.runtime.running.store(true, std::sync::atomic::Ordering::SeqCst);
        // Check environment to provide diagnostics about Wayland session state.
        let wayland_display =
            std::env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| String::from("wayland-0"));
        let xdg_session =
            std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| String::from("unknown"));
        log::info!(
            "[wayland] Platform running: XDG_SESSION_TYPE={}, WAYLAND_DISPLAY={}",
            xdg_session,
            wayland_display
        );

        // Start EventLoop with native pump for Wayland dispatch.
        // The EventLoop runs in a background thread and calls the pump
        // on each iteration to dispatch pending native Wayland events.
        // Meanwhile, this thread blocks waiting for the quit signal.
        let mut event_loop = EventLoop::new();
        #[cfg(all(feature = "wayland-native", not(feature = "mini"), target_os = "linux"))]
        if let Some(pump) = create_event_loop_pump() {
            event_loop.set_native_pump(pump);
        }
        event_loop.start();

        // ── Polling loop ──
        // Current state: busy-wait with 16 ms sleep (~60 Hz polling rate) checking
        // the `running` flag. This is sufficient for the state-only backend but
        // is NOT a proper Wayland event loop.
        //
        // Path to a proper Wayland event loop:
        // 1. Use `wl_client::EventQueue::dispatch()` or `dispatch_pending()` on
        //    the Wayland connection in each iteration instead of sleeping.
        // 2. Integrate with an epoll/fd-based pump so the thread blocks on the
        //    Wayland socket fd rather than busy-waiting.
        // 3. Replace `EventLoop` with `calloop` or an `os_pipe`-based wakeup
        //    mechanism to support proper event sources.
        // 4. Wire `wl_display_roundtrip()` during init to ensure all globals are
        //    received before entering the main loop.
        while self.runtime.running.load(std::sync::atomic::Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(16));
        }
    }

    fn quit(&self) {
        self.runtime.running.store(false, std::sync::atomic::Ordering::SeqCst);
        log::info!("[wayland] Platform quit.");
    }

    // -----------------------------------------------------------------------
    // Widget creation
    // -----------------------------------------------------------------------

    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        // Try native Wayland surface creation when the wayland-native feature is active
        #[cfg(all(feature = "wayland-native", target_os = "linux"))]
        if let Some(id) = self.try_create_native_window(title, x, y, width, height) {
            return id;
        }
        // Fallback: state-only window
        self.insert_widget(WaylandHandleKind::Window, title, x, y, width, height)
    }

    fn create_button(
        &self,
        _parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.insert_widget(WaylandHandleKind::Button, text, x, y, width, height)
    }

    fn create_checkbox(
        &self,
        _parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.insert_widget(WaylandHandleKind::CheckBox, text, x, y, width, height)
    }

    fn create_line_edit(
        &self,
        _parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.insert_widget(WaylandHandleKind::LineEdit, text, x, y, width, height)
    }

    fn create_label(
        &self,
        _parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.insert_widget(WaylandHandleKind::Label, text, x, y, width, height)
    }

    fn create_radio_button(
        &self,
        _parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.insert_widget(WaylandHandleKind::RadioButton, text, x, y, width, height)
    }

    fn create_slider(
        &self,
        _parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.insert_widget(WaylandHandleKind::Slider, "Slider", x, y, width, height)
    }

    fn create_progress_bar(
        &self,
        _parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.insert_widget(WaylandHandleKind::ProgressBar, "ProgressBar", x, y, width, height)
    }

    fn create_combo_box(
        &self,
        _parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let id = self.insert_widget(WaylandHandleKind::ComboBox, "ComboBox", x, y, width, height);
        // Initialize empty list data for this combo box.
        if let Ok(mut data) = self.list_data.lock() {
            data.entry(id).or_insert_with(ListData::default);
        }
        id
    }

    fn create_list_box(
        &self,
        _parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let id = self.insert_widget(WaylandHandleKind::ListBox, "ListBox", x, y, width, height);
        // Initialize empty list data for this list box.
        if let Ok(mut data) = self.list_data.lock() {
            data.entry(id).or_insert_with(ListData::default);
        }
        id
    }

    fn create_panel(&self, _parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        self.insert_widget(WaylandHandleKind::Panel, "Panel", x, y, width, height)
    }

    fn create_menu_bar(
        &self,
        _parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.insert_widget(WaylandHandleKind::MenuBar, "MenuBar", x, y, width, height)
    }

    fn create_menu(
        &self,
        _parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.insert_widget(WaylandHandleKind::Menu, text, x, y, width, height)
    }

    fn create_tool_bar(
        &self,
        _parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.insert_widget(WaylandHandleKind::ToolBar, "ToolBar", x, y, width, height)
    }

    fn create_status_bar(
        &self,
        _parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.insert_widget(WaylandHandleKind::StatusBar, text, x, y, width, height)
    }

    // -----------------------------------------------------------------------
    // Dialogs and extended controls
    // -----------------------------------------------------------------------

    fn create_message_box(
        &self,
        _parent: ObjectId,
        _title: &str,
        _text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.insert_widget(WaylandHandleKind::MessageBox, _text, x, y, width, height)
    }

    fn create_file_dialog(
        &self,
        _parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.insert_widget(WaylandHandleKind::FileDialog, "FileDialog", x, y, width, height)
    }

    fn create_color_dialog(
        &self,
        _parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.insert_widget(WaylandHandleKind::ColorDialog, "ColorDialog", x, y, width, height)
    }

    fn create_font_dialog(
        &self,
        _parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.insert_widget(WaylandHandleKind::FontDialog, "FontDialog", x, y, width, height)
    }

    fn create_spin_box(
        &self,
        _parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.insert_widget(WaylandHandleKind::SpinBox, "SpinBox", x, y, width, height)
    }

    fn create_list_view(
        &self,
        _parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.insert_widget(WaylandHandleKind::ListView, "ListView", x, y, width, height)
    }

    fn create_scroll_area(
        &self,
        _parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.insert_widget(WaylandHandleKind::ScrollArea, "ScrollArea", x, y, width, height)
    }

    // -----------------------------------------------------------------------
    // Menu system
    // -----------------------------------------------------------------------

    fn attach_menu_bar_to_window(&self, window: ObjectId, menu_bar: ObjectId) -> bool {
        if let Ok(mut menus) = self.menus.lock() {
            menus.attached_menu_bar.insert(window, menu_bar);
            true
        } else {
            log::error!("[wayland] attach_menu_bar_to_window: mutex poisoned");
            false
        }
    }

    fn menu_add_item(&self, parent_menu: ObjectId, text: &str, shortcut: Option<&str>) -> ObjectId {
        let display = if let Some(shortcut) = shortcut {
            format!("{}\t{}", text, shortcut)
        } else {
            text.to_string()
        };
        let id = self.insert_widget(WaylandHandleKind::MenuItem, &display, 0, 0, 0, 0);
        if let Ok(mut menus) = self.menus.lock() {
            menus.menu_children.entry(parent_menu).or_default().push(id);
        }
        id
    }

    fn poll_menu_triggered(&self) -> Option<ObjectId> {
        if let Ok(mut menus) = self.menus.lock() {
            menus.pending_menu_events.pop_front()
        } else {
            log::error!("[wayland] poll_menu_triggered: mutex poisoned");
            None
        }
    }

    fn inject_menu_trigger(&self, menu_item_id: ObjectId) -> bool {
        if !self.state.contains_widget(menu_item_id) {
            return false;
        }
        if let Ok(mut menus) = self.menus.lock() {
            menus.pending_menu_events.push_back(menu_item_id);
            true
        } else {
            log::error!("[wayland] inject_menu_trigger: mutex poisoned");
            false
        }
    }

    // -----------------------------------------------------------------------
    // Widget trigger events
    // -----------------------------------------------------------------------

    fn poll_widget_triggered(&self) -> Option<ObjectId> {
        if let Ok(mut menus) = self.menus.lock() {
            menus.pending_widget_events.pop_front().map(|e| e.widget_id)
        } else {
            log::error!("[wayland] poll_widget_triggered: mutex poisoned");
            None
        }
    }

    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        if let Ok(mut menus) = self.menus.lock() {
            menus.pending_widget_events.pop_front()
        } else {
            log::error!("[wayland] poll_widget_trigger_event: mutex poisoned");
            None
        }
    }

    fn inject_widget_trigger_event(&self, widget_id: ObjectId, kind: WidgetTriggerKind) -> bool {
        if !self.state.contains_widget(widget_id) {
            return false;
        }
        if let Ok(mut menus) = self.menus.lock() {
            menus.pending_widget_events.push_back(WidgetTriggerEvent { widget_id, kind });
            true
        } else {
            log::error!("[wayland] inject_widget_trigger_event: mutex poisoned");
            false
        }
    }

    // -----------------------------------------------------------------------
    // Widget lifecycle operations
    // -----------------------------------------------------------------------

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
        self.state.set_text(widget_id, text);
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

    // -----------------------------------------------------------------------
    // IME / Accessibility
    // -----------------------------------------------------------------------

    fn set_widget_ime_enabled(&self, widget_id: ObjectId, enabled: bool) -> bool {
        self.state.set_ime_enabled(widget_id, enabled)
    }

    fn is_widget_ime_enabled(&self, widget_id: ObjectId) -> bool {
        self.state.ime_enabled(widget_id)
    }

    fn set_widget_accessibility_name(&self, widget_id: ObjectId, name: &str) -> bool {
        self.state.set_accessibility_name(widget_id, name)
    }

    fn get_widget_accessibility_name(&self, widget_id: ObjectId) -> String {
        self.state.accessibility_name(widget_id)
    }

    // -----------------------------------------------------------------------
    // Clipboard
    // -----------------------------------------------------------------------

    fn set_clipboard_text(&self, text: &str) -> bool {
        self.state.set_clipboard_text(text)
    }

    fn get_clipboard_text(&self) -> String {
        self.state.clipboard_text()
    }

    // -----------------------------------------------------------------------
    // Drag and drop
    // -----------------------------------------------------------------------

    fn begin_drag(&self, source_widget_id: ObjectId, mime: &str, payload: &[u8]) -> bool {
        self.state.begin_drag(source_widget_id, mime, payload)
    }

    fn poll_drop_event(&self) -> Option<DropEvent> {
        self.state.pop_drop_event()
    }

    fn inject_drop_event(&self, event: DropEvent) -> bool {
        self.state.inject_drop_event(event)
    }

    // -----------------------------------------------------------------------
    // ComboBox data methods
    // -----------------------------------------------------------------------

    fn combo_box_add_item(&self, combo_box: ObjectId, text: &str) -> bool {
        match self.list_data.lock() {
            Ok(mut data) => match data.get_mut(&combo_box) {
                Some(list) => {
                    list.items.push(text.to_string());
                    true
                }
                None => false,
            },
            Err(_) => {
                log::error!("[wayland] combo_box_add_item: mutex poisoned");
                false
            }
        }
    }

    fn combo_box_clear_items(&self, combo_box: ObjectId) -> bool {
        match self.list_data.lock() {
            Ok(mut data) => match data.get_mut(&combo_box) {
                Some(list) => {
                    list.items.clear();
                    list.current_index = None;
                    true
                }
                None => false,
            },
            Err(_) => {
                log::error!("[wayland] combo_box_clear_items: mutex poisoned");
                false
            }
        }
    }

    fn combo_box_set_current_index(&self, combo_box: ObjectId, index: usize) -> bool {
        match self.list_data.lock() {
            Ok(mut data) => match data.get_mut(&combo_box) {
                Some(list) if index < list.items.len() => {
                    let previous = list.current_index;
                    list.current_index = Some(index);
                    // Fire selection changed trigger event.
                    if previous != Some(index) {
                        if let Ok(mut menus) = self.menus.lock() {
                            menus.pending_widget_events.push_back(WidgetTriggerEvent {
                                widget_id: combo_box,
                                kind: WidgetTriggerKind::SelectionChanged,
                            });
                        } else {
                            log::error!(
                                "[wayland] combo_box_set_current_index: menus mutex poisoned"
                            );
                        }
                    }
                    true
                }
                _ => false,
            },
            Err(_) => {
                log::error!("[wayland] combo_box_set_current_index: list_data mutex poisoned");
                false
            }
        }
    }

    fn combo_box_current_index(&self, combo_box: ObjectId) -> Option<usize> {
        match self.list_data.lock() {
            Ok(data) => data.get(&combo_box).and_then(|list| list.current_index),
            Err(_) => {
                log::error!("[wayland] combo_box_current_index: mutex poisoned");
                None
            }
        }
    }

    fn combo_box_item_count(&self, combo_box: ObjectId) -> usize {
        match self.list_data.lock() {
            Ok(data) => data.get(&combo_box).map(|list| list.items.len()).unwrap_or(0),
            Err(_) => {
                log::error!("[wayland] combo_box_item_count: mutex poisoned");
                0
            }
        }
    }

    fn combo_box_item_text(&self, combo_box: ObjectId, index: usize) -> Option<String> {
        match self.list_data.lock() {
            Ok(data) => data.get(&combo_box).and_then(|list| list.items.get(index).cloned()),
            Err(_) => {
                log::error!("[wayland] combo_box_item_text: mutex poisoned");
                None
            }
        }
    }

    // -----------------------------------------------------------------------
    // ListBox data methods
    // -----------------------------------------------------------------------

    fn list_box_add_item(&self, list_box: ObjectId, text: &str) -> bool {
        match self.list_data.lock() {
            Ok(mut data) => match data.get_mut(&list_box) {
                Some(list) => {
                    list.items.push(text.to_string());
                    true
                }
                None => false,
            },
            Err(_) => {
                log::error!("[wayland] list_box_add_item: mutex poisoned");
                false
            }
        }
    }

    fn list_box_remove_item(&self, list_box: ObjectId, index: usize) -> bool {
        match self.list_data.lock() {
            Ok(mut data) => match data.get_mut(&list_box) {
                Some(list) if index < list.items.len() => {
                    list.items.remove(index);
                    // Adjust current index if needed.
                    if let Some(cur) = list.current_index {
                        if cur == index {
                            if list.items.is_empty() {
                                list.current_index = None;
                            } else if cur >= list.items.len() {
                                list.current_index = Some(list.items.len() - 1);
                            }
                        } else if cur > index {
                            list.current_index = Some(cur - 1);
                        }
                    }
                    true
                }
                _ => false,
            },
            Err(_) => {
                log::error!("[wayland] list_box_remove_item: mutex poisoned");
                false
            }
        }
    }

    fn list_box_clear_items(&self, list_box: ObjectId) -> bool {
        match self.list_data.lock() {
            Ok(mut data) => match data.get_mut(&list_box) {
                Some(list) => {
                    list.items.clear();
                    list.current_index = None;
                    true
                }
                None => false,
            },
            Err(_) => {
                log::error!("[wayland] list_box_clear_items: mutex poisoned");
                false
            }
        }
    }

    fn list_box_set_current_index(&self, list_box: ObjectId, index: usize) -> bool {
        match self.list_data.lock() {
            Ok(mut data) => match data.get_mut(&list_box) {
                Some(list) if index < list.items.len() => {
                    let previous = list.current_index;
                    list.current_index = Some(index);
                    // Fire selection changed trigger event.
                    if previous != Some(index) {
                        if let Ok(mut menus) = self.menus.lock() {
                            menus.pending_widget_events.push_back(WidgetTriggerEvent {
                                widget_id: list_box,
                                kind: WidgetTriggerKind::SelectionChanged,
                            });
                        } else {
                            log::error!(
                                "[wayland] list_box_set_current_index: menus mutex poisoned"
                            );
                        }
                    }
                    true
                }
                _ => false,
            },
            Err(_) => {
                log::error!("[wayland] list_box_set_current_index: list_data mutex poisoned");
                false
            }
        }
    }

    fn list_box_current_index(&self, list_box: ObjectId) -> Option<usize> {
        match self.list_data.lock() {
            Ok(data) => data.get(&list_box).and_then(|list| list.current_index),
            Err(_) => {
                log::error!("[wayland] list_box_current_index: mutex poisoned");
                None
            }
        }
    }

    fn list_box_item_count(&self, list_box: ObjectId) -> usize {
        match self.list_data.lock() {
            Ok(data) => data.get(&list_box).map(|list| list.items.len()).unwrap_or(0),
            Err(_) => {
                log::error!("[wayland] list_box_item_count: mutex poisoned");
                0
            }
        }
    }

    fn list_box_item_text(&self, list_box: ObjectId, index: usize) -> Option<String> {
        match self.list_data.lock() {
            Ok(data) => data.get(&list_box).and_then(|list| list.items.get(index).cloned()),
            Err(_) => {
                log::error!("[wayland] list_box_item_text: mutex poisoned");
                None
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Native Wayland window creation (gated by "wayland-native" feature)
// ---------------------------------------------------------------------------

#[cfg(all(feature = "wayland-native", target_os = "linux"))]
impl WaylandPlatform {
    /// Dispatch pending native Wayland events.
    ///
    /// Intended to be called from an `EventLoop` native pump callback
    /// on each iteration so that Wayland protocol events are dispatched
    /// without a separate blocking `run()` loop.
    #[cfg(not(feature = "mini"))]
    pub(crate) fn dispatch_native_events(&self) {
        let mut guard = self.native_session.lock().unwrap();
        if let Some(ref mut session) = *guard {
            let _ = session.event_queue.dispatch_pending(&mut session.state);
        }
    }

    /// Attempt to create a native Wayland xdg_toplevel for this window.
    /// Returns `Some(id)` on success, or `None` to fall back to state-only.
    fn try_create_native_window(
        &self,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> Option<ObjectId> {
        // Helper: connect to Wayland display and discover globals.
        // Returns `Some(WaylandSession)` on success or `None` on failure.
        fn connect_wayland() -> Option<WaylandSession> {
            let conn = wl_client::Connection::connect_to_env()
                .inspect_err(|e| {
                    log::error!("[wayland] Failed to connect to Wayland display: {}", e)
                })
                .ok()?;
            let display = conn.display();
            let mut event_queue = conn.new_event_queue();
            let qh = event_queue.handle();
            let _registry = display.get_registry(&qh, ());

            let mut state = WaylandSessionState {
                compositor: None,
                xdg_wm_base: None,
                wl_output: None,
                dpi_scale: 1.0,
            };
            event_queue
                .roundtrip(&mut state)
                .inspect_err(|e| log::error!("[wayland] Registry roundtrip failed: {}", e))
                .ok()?;

            if state.compositor.is_none() {
                log::error!("[wayland] wl_compositor global not available");
                return None;
            }
            if state.xdg_wm_base.is_none() {
                log::error!("[wayland] xdg_wm_base global not available");
                return None;
            }

            log::info!("[wayland] Connected to display; compositor and xdg_wm_base bound.");
            Some(WaylandSession { conn, event_queue, state })
        }

        // Obtain or create the persistent Wayland session.
        let mut guard = self.native_session.lock().unwrap();
        if guard.is_none() {
            *guard = connect_wayland();
        }
        let session = guard.as_mut()?;

        // 5. Create wl_surface, xdg_surface, and xdg_toplevel
        let compositor = session.state.compositor.as_ref()?;
        let xdg_wm_base = session.state.xdg_wm_base.as_ref()?;
        let qh = session.event_queue.handle();

        let surface = compositor.create_surface(&qh, ());
        let xdg_surface = xdg_wm_base.get_xdg_surface(&surface, &qh, ());
        let toplevel = xdg_surface.get_toplevel(&qh, ());

        // 6. Set window properties
        toplevel.set_title(title.to_string());
        toplevel.set_app_id("rust_widgets".to_string());
        if width > 0 && height > 0 {
            toplevel.set_min_size(width as i32, height as i32);
        }

        // 7. Commit the surface to make the compositor aware of it
        surface.commit();

        // 8. Roundtrip to process the xdg_surface.configure event
        session.event_queue.roundtrip(&mut session.state).ok()?;

        log::info!("[wayland] Created xdg_toplevel '{}' ({}x{})", title, width, height);

        let id = self.insert_widget(WaylandHandleKind::Window, title, x, y, width, height);
        log::info!("[wayland] Window {} registered with state backend", id);

        Some(id)
    }
}

// ---------------------------------------------------------------------------
// Wayland session state & dispatch implementations
// ---------------------------------------------------------------------------

/// Dispatch state for the Wayland session.
/// Holds the global proxies obtained from the registry.
#[cfg(all(feature = "wayland-native", target_os = "linux"))]
pub(crate) struct WaylandSessionState {
    pub(crate) compositor: Option<wl_client::protocol::wl_compositor::WlCompositor>,
    pub(crate) xdg_wm_base: Option<wl_protocols::xdg::shell::client::xdg_wm_base::XdgWmBase>,
    /// wl_output proxy for DPI scale detection.
    pub(crate) wl_output: Option<wl_client::protocol::wl_output::WlOutput>,
    /// DPI scale factor obtained from wl_output::scale event.
    /// Defaults to 1.0 until the compositor sends a scale event.
    pub(crate) dpi_scale: f32,
}

/// A persistent Wayland session containing the connection, event queue,
/// and proxy state. Stored inside `WaylandPlatform.native_session`.
#[cfg(all(feature = "wayland-native", target_os = "linux"))]
pub(crate) struct WaylandSession {
    /// The Wayland connection. Must stay alive while `event_queue` is in use.
    #[allow(dead_code)]
    pub(crate) conn: wl_client::Connection,
    pub(crate) event_queue: wl_client::EventQueue<WaylandSessionState>,
    pub(crate) state: WaylandSessionState,
}

// ---------------------------------------------------------------------------
// EventLoop native pump integration
// ---------------------------------------------------------------------------

/// Create a native platform event pump for the Wayland backend.
///
/// Returns a closure suitable for `EventLoop::set_native_pump()`.
/// When called, it looks up the global `WaylandPlatform` singleton and
/// dispatches pending Wayland events through `dispatch_pending()`.
///
/// Returns `None` if the active platform is not Wayland.
#[cfg(all(feature = "wayland-native", not(feature = "mini"), target_os = "linux"))]
pub fn create_event_loop_pump() -> Option<Box<dyn Fn() + Send + Sync>> {
    let platform = crate::platform::runtime::get_platform();
    let wayland = platform.as_any().downcast_ref::<WaylandPlatform>()?;
    Some(Box::new(move || {
        wayland.dispatch_native_events();
    }))
}

// --- Registry global discovery ---

#[cfg(all(feature = "wayland-native", target_os = "linux"))]
impl wl_client::Dispatch<wl_client::protocol::wl_registry::WlRegistry, ()> for WaylandSessionState {
    fn event(
        state: &mut Self,
        registry: &wl_client::protocol::wl_registry::WlRegistry,
        event: wl_client::protocol::wl_registry::Event,
        _data: &(),
        _conn: &wl_client::Connection,
        _qh: &wl_client::QueueHandle<WaylandSessionState>,
    ) {
        use wl_client::protocol::wl_registry::Event;
        if let Event::Global { name, interface, version } = event {
            log::debug!(
                "[wayland] Registry global: interface='{}', version={}",
                interface,
                version
            );
            match interface.as_str() {
                "wl_compositor" => {
                    let comp = registry
                        .bind::<wl_client::protocol::wl_compositor::WlCompositor, _, _>(
                            name,
                            version.min(4),
                            _qh,
                            (),
                        );
                    state.compositor = Some(comp);
                    log::info!("[wayland] Bound wl_compositor (v{})", version);
                }
                "xdg_wm_base" => {
                    let xdg = registry
                        .bind::<wl_protocols::xdg::shell::client::xdg_wm_base::XdgWmBase, _, _>(
                            name,
                            version.min(6),
                            _qh,
                            (),
                        );
                    state.xdg_wm_base = Some(xdg);
                    log::info!("[wayland] Bound xdg_wm_base (v{})", version);
                }
                "wl_output" => {
                    let output = registry.bind::<wl_client::protocol::wl_output::WlOutput, _, _>(
                        name,
                        version.min(4),
                        _qh,
                        (),
                    );
                    state.wl_output = Some(output);
                    log::info!("[wayland] Bound wl_output (v{})", version);
                }
                _ => { /* Other globals need no special handling */ }
            }
        }
    }
}

// --- wl_compositor (no events) ---

#[cfg(all(feature = "wayland-native", target_os = "linux"))]
impl wl_client::Dispatch<wl_client::protocol::wl_compositor::WlCompositor, ()>
    for WaylandSessionState
{
    fn event(
        _state: &mut Self,
        _proxy: &wl_client::protocol::wl_compositor::WlCompositor,
        _event: wl_client::protocol::wl_compositor::Event,
        _data: &(),
        _conn: &wl_client::Connection,
        _qh: &wl_client::QueueHandle<WaylandSessionState>,
    ) {
        // wl_compositor has no server-to-client events.
    }
}

// --- wl_output (handle scale events for DPI detection) ---

#[cfg(all(feature = "wayland-native", target_os = "linux"))]
impl wl_client::Dispatch<wl_client::protocol::wl_output::WlOutput, ()> for WaylandSessionState {
    fn event(
        state: &mut Self,
        _proxy: &wl_client::protocol::wl_output::WlOutput,
        event: wl_client::protocol::wl_output::Event,
        _data: &(),
        _conn: &wl_client::Connection,
        _qh: &wl_client::QueueHandle<WaylandSessionState>,
    ) {
        use wl_client::protocol::wl_output::Event;
        match event {
            Event::Scale { factor } => {
                state.dpi_scale = factor as f32;
                log::info!("[wayland] wl_output scale factor: {} (DPI: {})", factor, factor * 96);
            }
            Event::Done => {
                log::trace!("[wayland] wl_output done");
            }
            Event::Geometry { .. }
            | Event::Mode { .. }
            | Event::Name { .. }
            | Event::Description { .. } => {
                log::trace!("[wayland] wl_output event: {:?}", event);
            }
            _ => {
                log::trace!("[wayland] wl_output unhandled event: {:?}", event);
            }
        }
    }
}

// --- wl_surface (surface enter/leave events) ---

#[cfg(all(feature = "wayland-native", target_os = "linux"))]
impl wl_client::Dispatch<wl_client::protocol::wl_surface::WlSurface, ()> for WaylandSessionState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_client::protocol::wl_surface::WlSurface,
        _event: wl_client::protocol::wl_surface::Event,
        _data: &(),
        _conn: &wl_client::Connection,
        _qh: &wl_client::QueueHandle<WaylandSessionState>,
    ) {
        // wl_surface events (enter/leave/frame/etc.) are logged but not acted upon yet.
        log::trace!("[wayland] wl_surface event: {:?}", _event);
    }
}

// --- xdg_wm_base (handle ping events) ---

#[cfg(all(feature = "wayland-native", target_os = "linux"))]
impl wl_client::Dispatch<wl_protocols::xdg::shell::client::xdg_wm_base::XdgWmBase, ()>
    for WaylandSessionState
{
    fn event(
        _state: &mut Self,
        proxy: &wl_protocols::xdg::shell::client::xdg_wm_base::XdgWmBase,
        event: <wl_protocols::xdg::shell::client::xdg_wm_base::XdgWmBase as wl_client::Proxy>::Event,
        _data: &(),
        _conn: &wl_client::Connection,
        _qh: &wl_client::QueueHandle<WaylandSessionState>,
    ) {
        use wl_protocols::xdg::shell::client::xdg_wm_base::Event;
        if let Event::Ping { serial } = event {
            proxy.pong(serial);
            log::trace!("[wayland] xdg_wm_base ping/pong (serial={})", serial);
        }
    }
}

// --- xdg_surface (acknowledge configure events) ---

#[cfg(all(feature = "wayland-native", target_os = "linux"))]
impl wl_client::Dispatch<wl_protocols::xdg::shell::client::xdg_surface::XdgSurface, ()>
    for WaylandSessionState
{
    fn event(
        _state: &mut Self,
        proxy: &wl_protocols::xdg::shell::client::xdg_surface::XdgSurface,
        event: <wl_protocols::xdg::shell::client::xdg_surface::XdgSurface as wl_client::Proxy>::Event,
        _data: &(),
        _conn: &wl_client::Connection,
        _qh: &wl_client::QueueHandle<WaylandSessionState>,
    ) {
        use wl_protocols::xdg::shell::client::xdg_surface::Event;
        if let Event::Configure { serial } = event {
            proxy.ack_configure(serial);
            log::trace!("[wayland] xdg_surface configure ack (serial={})", serial);
        }
    }
}

// --- xdg_toplevel (handle configure / close) ---

#[cfg(all(feature = "wayland-native", target_os = "linux"))]
impl wl_client::Dispatch<wl_protocols::xdg::shell::client::xdg_toplevel::XdgToplevel, ()>
    for WaylandSessionState
{
    fn event(
        _state: &mut Self,
        _proxy: &wl_protocols::xdg::shell::client::xdg_toplevel::XdgToplevel,
        event: <wl_protocols::xdg::shell::client::xdg_toplevel::XdgToplevel as wl_client::Proxy>::Event,
        _data: &(),
        _conn: &wl_client::Connection,
        _qh: &wl_client::QueueHandle<WaylandSessionState>,
    ) {
        use wl_protocols::xdg::shell::client::xdg_toplevel::Event;
        match event {
            Event::Configure { width, height, states } => {
                log::trace!(
                    "[wayland] xdg_toplevel configure: {}x{}, states={:?}",
                    width,
                    height,
                    states
                );
            }
            Event::Close => {
                log::info!("[wayland] xdg_toplevel close requested");
            }
            Event::ConfigureBounds { .. } | Event::WmCapabilities { .. } => {
                log::trace!("[wayland] xdg_toplevel event: {:?}", event);
            }
            _ => {
                log::trace!("[wayland] xdg_toplevel unhandled event: {:?}", event);
            }
        }
    }
}
