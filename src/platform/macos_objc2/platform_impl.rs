use super::types::{parse_shortcut, submenu_id, MacOSObjc2Platform, MacObjc2HandleKind};
use crate::core::ObjectId;
use crate::core::PlatformFamily;
use crate::platform::{DropEvent, Platform, WidgetTriggerEvent, WidgetTriggerKind};
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

impl Platform for MacOSObjc2Platform {
    // ---- Lifecycle & identity ----
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn backend_name(&self) -> &'static str {
        "macos-objc2-preview"
    }
    fn family(&self) -> PlatformFamily {
        PlatformFamily::Desktop
    }

    /// Reads installed physical memory through `sysctl hw.memsize`, the
    /// documented macOS interface for the machine's total RAM.
    fn total_memory_mb(&self) -> Option<u64> {
        let output =
            std::process::Command::new("sysctl").args(["-n", "hw.memsize"]).output().ok()?;
        if !output.status.success() {
            return None;
        }
        let bytes = String::from_utf8_lossy(&output.stdout).trim().parse::<u64>().ok()?;
        Some(bytes / (1024 * 1024))
    }

    /// Reports `true` when the machine is discharging its battery.
    ///
    /// `pmset -g batt` prints `Now drawing from 'Battery Power'` while on
    /// battery and `'AC Power'` while plugged in; desktops always report AC.
    fn is_on_battery(&self) -> bool {
        let Ok(output) = std::process::Command::new("pmset").args(["-g", "batt"]).output() else {
            return false;
        };
        output.status.success() && String::from_utf8_lossy(&output.stdout).contains("Battery Power")
    }

    /// Samples this process's resident memory against total installed RAM.
    fn process_memory_utilization(&self) -> Option<f32> {
        let pid = std::process::id().to_string();
        let output =
            std::process::Command::new("ps").args(["-o", "rss=", "-p", &pid]).output().ok()?;
        if !output.status.success() {
            return None;
        }
        let rss_kb = String::from_utf8_lossy(&output.stdout).trim().parse::<f64>().ok()?;
        // Use the real machine total rather than assuming a "typical" 8 GB, so
        // the ratio stays meaningful on 16/32/64 GB systems.
        let total_kb = self.total_memory_mb()? as f64 * 1024.0;
        if total_kb <= 0.0 {
            return None;
        }
        Some(((rss_kb / total_kb) as f32).clamp(0.0, 1.0))
    }

    /// CPU load is not sampled through a stable public interface here, so this
    /// backend reports `None` and the monitor keeps its default.
    fn process_cpu_utilization(&self) -> Option<f32> {
        None
    }

    /// Submits through the CUPS `lpr` client, falling back to `lp`.
    fn spawn_print_job(&self, job_file: &std::path::Path) -> Result<(), String> {
        if let Ok(status) = std::process::Command::new("lpr").arg(job_file).status() {
            if status.success() {
                return Ok(());
            }
        }
        if let Ok(status) = std::process::Command::new("lp").arg(job_file).status() {
            if status.success() {
                return Ok(());
            }
        }
        Err("no available system print command succeeded (tried: lpr, lp)".to_string())
    }

    /// macOS always ships CUPS, so the `lp`/`lpr` clients are present.
    fn has_print_support(&self) -> bool {
        crate::platform::types::unix_print_clients_available()
    }

    /// Renders menu accelerators with AppKit symbols (`⌘⇧Z`).
    fn shortcut_style(&self) -> crate::shortcut::PlatformShortcutStyle {
        crate::shortcut::PlatformShortcutStyle::Mac
    }
    fn init(&self) {
        // Marker keeps objc2 dependency wired even before native event-loop bridging lands.
        let _ = self.objc2_runtime_marker();
        // Bootstrap the shared NSApplication so native windows/menus the backend
        // creates are tracked by AppKit (finishLaunching installs the app).
        #[cfg(all(target_os = "macos", feature = "macos"))]
        let bootstrapped = super::native::bootstrap_ns_application();
        #[cfg(not(all(target_os = "macos", feature = "macos")))]
        let bootstrapped = false;
        if bootstrapped {
            log::debug!("[macos-objc2] NSApplication bootstrapped on the main thread");
        } else {
            log::debug!(
                "[macos-objc2] NSApplication not bootstrapped (off-main or non-macOS host)"
            );
        }
        self.runtime.initialized.store(true, Ordering::SeqCst);
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
    fn destroy_widget(&self, widget_id: ObjectId) -> bool {
        // Capture the kind before the state record is removed so the kind-specific
        // native registries can be released as well.
        let kind = self.state.kind_of(widget_id);
        let existed = self.state.destroy_widget(widget_id);

        // Release the retained AppKit objects. `remove_native_view` releases the
        // object stored under the widget id and drops its parent-map entry; a
        // `Menu` widget additionally owns an `NSMenu` submenu stored under a
        // derived id that is not released anywhere else.
        #[cfg(all(target_os = "macos", feature = "macos"))]
        {
            if matches!(kind, Some(MacObjc2HandleKind::Menu)) {
                super::native::remove_native_view(super::types::submenu_id(widget_id));
            }
            super::native::remove_native_view(widget_id);
        }

        // Drop the per-widget ComboBox/ListBox list storage. The guard is
        // released at the end of this statement, before the menu lock below.
        self.list_data.lock().expect("mac objc2 list data lock poisoned").remove(&widget_id);

        // Drop every menu bookkeeping entry that names this widget: ownership as
        // an attached menu bar of any window, membership in a parent menu's
        // child list, its shortcut table entry, and any queued trigger events
        // that would otherwise fire for a widget that no longer exists.
        let mut menus = self.menus.lock().expect("mac objc2 menu lock poisoned");
        menus.attached_menu_bar.retain(|_window, menu_bar| *menu_bar != widget_id);
        let destroyed_children = menus.menu_children.remove(&widget_id);
        menus.menu_children.retain(|_parent, children| {
            children.retain(|child| *child != widget_id);
            !children.is_empty()
        });
        menus.menu_item_shortcuts.remove(&widget_id);
        menus.pending_menu_events.retain(|queued| *queued != widget_id);
        menus.pending_widget_events.retain(|event| event.widget_id != widget_id);
        drop(menus);

        // A destroyed `Menu` owns child menu items that were only reachable
        // through it; cascade the teardown so those items are not orphaned.
        for child in destroyed_children.into_iter().flatten() {
            self.destroy_widget(child);
        }

        existed
    }

    // ---- Window creation ----
    /// Create a new window with the given title and geometry.
    /// This is the entry point for window lifecycle parity tests.
    /// Returns a unique window id.
    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        // Insert window widget into backend state
        let id = self.insert_widget(MacObjc2HandleKind::Window, title, x, y, width, height);

        #[cfg(all(target_os = "macos", feature = "macos"))]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let window = super::native::create_ns_window(mtm, title, x, y, width, height);
            super::native::store_native_view(id, &*window as *const _ as *mut std::ffi::c_void);
        }

        id
    }

    // ---- Clipboard & Drag-Drop ----
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

    // ---- Widget state ----
    fn show_widget(&self, widget_id: u64) {
        self.state.set_visible(widget_id, true);
        #[cfg(all(target_os = "macos", feature = "macos"))]
        super::native::set_native_hidden(widget_id, false);
    }
    fn hide_widget(&self, widget_id: u64) {
        self.state.set_visible(widget_id, false);
        #[cfg(all(target_os = "macos", feature = "macos"))]
        super::native::set_native_hidden(widget_id, true);
    }
    fn set_widget_geometry(&self, widget_id: u64, x: i32, y: i32, width: u32, height: u32) {
        self.state.set_geometry(widget_id, x, y, width, height);
        #[cfg(all(target_os = "macos", feature = "macos"))]
        super::native::set_native_frame(widget_id, x, y, width, height);
    }
    fn set_widget_text(&self, widget_id: u64, text: &str) {
        if !self.state.set_text(widget_id, text) {
            return;
        }
        #[cfg(all(target_os = "macos", feature = "macos"))]
        super::native::set_native_text(widget_id, text);
        if matches!(self.kind_of(widget_id), Some(MacObjc2HandleKind::LineEdit)) {
            // Text edits emit value-changed semantics to match other desktop backends.
            self.menus
                .lock()
                .expect("mac objc2 menu lock poisoned")
                .pending_widget_events
                .push_back(WidgetTriggerEvent { widget_id, kind: WidgetTriggerKind::ValueChanged });
        }
    }
    fn get_widget_text(&self, widget_id: u64) -> String {
        self.state.text(widget_id)
    }
    fn set_widget_enabled(&self, widget_id: u64, enabled: bool) {
        self.state.set_enabled(widget_id, enabled);
        #[cfg(all(target_os = "macos", feature = "macos"))]
        super::native::set_native_enabled(widget_id, enabled);
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

    // ---- Menu creation & events ----
    fn create_menu_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if !matches!(self.kind_of(parent), Some(MacObjc2HandleKind::Window)) {
            return 0;
        }
        let id = self.insert_widget(MacObjc2HandleKind::MenuBar, "MenuBar", x, y, width, height);
        // Build a real NSMenu as the menu bar so `attach_menu_bar_to_window` can
        // install an actual main menu. Off-main we keep the state-only handle.
        #[cfg(all(target_os = "macos", feature = "macos"))]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let menu = super::native::create_ns_menu(mtm, "MainMenu");
            super::native::store_native_view(id, &*menu as *const _ as *mut std::ffi::c_void);
        }
        id
    }
    fn create_menu(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if !matches!(
            self.kind_of(parent),
            Some(MacObjc2HandleKind::MenuBar | MacObjc2HandleKind::Menu)
        ) {
            return 0;
        }
        let id = self.insert_widget(MacObjc2HandleKind::Menu, text, x, y, width, height);
        // Each submenu is an NSMenuItem carrying an NSMenu submenu, exactly like
        // the cocoa-legacy backend builds it.
        #[cfg(all(target_os = "macos", feature = "macos"))]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let item = super::native::create_ns_menu_item(mtm, text, "");
            let submenu = super::native::create_ns_menu(mtm, text);
            let item_ptr = &*item as *const _ as *mut std::ffi::c_void;
            let submenu_ptr = &*submenu as *const _ as *mut std::ffi::c_void;
            super::native::menu_set_submenu_on_item(item_ptr, submenu_ptr);
            super::native::store_native_view(id, item_ptr);
            // Also keep the submenu alive under a derived id so `menu_add_item`
            // can append to it later.
            super::native::store_native_view(submenu_id(id), submenu_ptr);
            // A MenuBar parent hosts items directly; a Menu parent hosts them in
            // its own NSMenu submenu (its native view is the NSMenuItem).
            let container = match self.kind_of(parent) {
                Some(MacObjc2HandleKind::Menu) => {
                    super::native::get_native_view(submenu_id(parent))
                }
                _ => super::native::get_native_view(parent),
            };
            if let Some(container_ptr) = container {
                super::native::menu_add_child_to_menu(container_ptr, item_ptr);
            }
        }
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
            // Real AppKit install: set the NSMenu as NSApplication.mainMenu.
            #[cfg(all(target_os = "macos", feature = "macos"))]
            if let Some(menu_ptr) = super::native::get_native_view(menu_bar) {
                let installed = super::native::install_main_menu(menu_ptr);
                log::debug!("[macos-objc2] attach_menu_bar_to_window installed_native={installed}");
            }
            return true;
        }
        false
    }
    fn menu_add_item(&self, parent_menu: u64, text: &str, shortcut: Option<&str>) -> u64 {
        if !matches!(self.kind_of(parent_menu), Some(MacObjc2HandleKind::Menu)) {
            return 0;
        }
        let item_id = self.insert_widget(MacObjc2HandleKind::MenuItem, text, 0, 0, 0, 0);
        let (key, modifier_mask) = parse_shortcut(shortcut);
        #[cfg(all(target_os = "macos", feature = "macos"))]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let item = super::native::create_ns_menu_item(mtm, text, &key);
            let item_ptr = &*item as *const _ as *mut std::ffi::c_void;
            super::native::store_native_view(item_id, item_ptr);
            super::native::set_native_menu_shortcut(item_id, &key, modifier_mask);
            // Append the item to the parent menu's NSMenu submenu.
            if let Some(target_ptr) = super::native::get_native_view(submenu_id(parent_menu)) {
                super::native::menu_add_child_to_menu(target_ptr, item_ptr);
            }
        }
        let mut menus = self.menus.lock().expect("mac objc2 menu lock poisoned");
        menus.menu_children.entry(parent_menu).or_default().push(item_id);
        menus.menu_item_shortcuts.insert(item_id, (key, modifier_mask));
        item_id
    }
    fn poll_menu_triggered(&self) -> Option<u64> {
        self.menus.lock().expect("mac objc2 menu lock poisoned").pending_menu_events.pop_front()
    }
    fn inject_menu_trigger(&self, menu_item_id: u64) -> bool {
        if !matches!(self.kind_of(menu_item_id), Some(MacObjc2HandleKind::MenuItem)) {
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
        self.poll_widget_trigger_event().map(|event| event.widget_id)
    }
    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        self.menus.lock().expect("mac objc2 menu lock poisoned").pending_widget_events.pop_front()
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

    // ---- Widget Creation ----
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
        let id = self.insert_widget(MacObjc2HandleKind::Button, text, x, y, width, height);

        #[cfg(all(target_os = "macos", feature = "macos"))]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let button = super::native::create_ns_button(mtm, text, x, y, width, height);
            super::native::store_native_view(id, &*button as *const _ as *mut std::ffi::c_void);
            super::native::add_as_subview(id, parent);
        }

        id
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
        let id = self.insert_widget(MacObjc2HandleKind::CheckBox, text, x, y, width, height);

        #[cfg(all(target_os = "macos", feature = "macos"))]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let checkbox = super::native::create_ns_checkbox(mtm, text, x, y, width, height);
            super::native::store_native_view(id, &*checkbox as *const _ as *mut std::ffi::c_void);
            super::native::add_as_subview(id, parent);
        }

        id
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
        let id = self.insert_widget(MacObjc2HandleKind::LineEdit, text, x, y, width, height);

        #[cfg(all(target_os = "macos", feature = "macos"))]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let field = super::native::create_ns_textfield(mtm, text, x, y, width, height);
            super::native::store_native_view(id, &*field as *const _ as *mut std::ffi::c_void);
            super::native::add_as_subview(id, parent);
        }

        id
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
        let id = self.insert_widget(MacObjc2HandleKind::Label, text, x, y, width, height);

        #[cfg(all(target_os = "macos", feature = "macos"))]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let label = super::native::create_ns_label(mtm, text, x, y, width, height);
            super::native::store_native_view(id, &*label as *const _ as *mut std::ffi::c_void);
            super::native::add_as_subview(id, parent);
        }

        id
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
        let id = self.insert_widget(MacObjc2HandleKind::RadioButton, text, x, y, width, height);

        #[cfg(all(target_os = "macos", feature = "macos"))]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let radio = super::native::create_ns_radio(mtm, text, x, y, width, height);
            super::native::store_native_view(id, &*radio as *const _ as *mut std::ffi::c_void);
            super::native::add_as_subview(id, parent);
        }

        id
    }
    fn create_slider(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(MacObjc2HandleKind::Slider, "Slider", x, y, width, height);

        #[cfg(all(target_os = "macos", feature = "macos"))]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let slider = super::native::create_ns_slider(mtm, x, y, width, height);
            super::native::store_native_view(id, &*slider as *const _ as *mut std::ffi::c_void);
            super::native::add_as_subview(id, parent);
        }

        id
    }
    fn create_progress_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id =
            self.insert_widget(MacObjc2HandleKind::ProgressBar, "ProgressBar", x, y, width, height);

        #[cfg(all(target_os = "macos", feature = "macos"))]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let progress = super::native::create_ns_progress(mtm, x, y, width, height);
            super::native::store_native_view(id, &*progress as *const _ as *mut std::ffi::c_void);
            super::native::add_as_subview(id, parent);
        }

        id
    }
    fn create_combo_box(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(MacObjc2HandleKind::ComboBox, "ComboBox", x, y, width, height);

        #[cfg(all(target_os = "macos", feature = "macos"))]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let combo = super::native::create_ns_combo_box(mtm, "", x, y, width, height);
            super::native::store_native_view(id, &*combo as *const _ as *mut std::ffi::c_void);
            super::native::add_as_subview(id, parent);
        }

        id
    }
    fn create_list_box(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(MacObjc2HandleKind::ListBox, "ListBox", x, y, width, height);

        #[cfg(all(target_os = "macos", feature = "macos"))]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let scroll = super::native::create_ns_list_box(mtm, x, y, width, height);
            super::native::store_native_view(id, &*scroll as *const _ as *mut std::ffi::c_void);
            super::native::add_as_subview(id, parent);
        }

        id
    }
    fn list_box_add_item(&self, list_box: u64, text: &str) -> bool {
        // Validate that widget exists and is a ListBox.
        if !matches!(self.kind_of(list_box), Some(MacObjc2HandleKind::ListBox)) {
            return false;
        }
        let mut data = self.list_data.lock().expect("mac objc2 list data lock poisoned");
        let entry = data.entry(list_box).or_default();
        entry.items.push(text.to_string());
        true
    }
    fn list_box_remove_item(&self, list_box: u64, index: usize) -> bool {
        if !matches!(self.kind_of(list_box), Some(MacObjc2HandleKind::ListBox)) {
            return false;
        }
        let mut data = self.list_data.lock().expect("mac objc2 list data lock poisoned");
        let entry = match data.get_mut(&list_box) {
            Some(e) => e,
            None => return false,
        };
        if index >= entry.items.len() {
            return false;
        }
        entry.items.remove(index);
        // Adjust current_index if the removed item was at or before it.
        if let Some(cur) = entry.current_index {
            if cur == index {
                // Item at the selected index was removed — clear selection.
                entry.current_index = None;
            } else if cur > index {
                // Selection shifted down by one.
                entry.current_index = Some(cur - 1);
            }
        }
        true
    }
    fn list_box_clear_items(&self, list_box: u64) -> bool {
        if !matches!(self.kind_of(list_box), Some(MacObjc2HandleKind::ListBox)) {
            return false;
        }
        let mut data = self.list_data.lock().expect("mac objc2 list data lock poisoned");
        if let Some(entry) = data.get_mut(&list_box) {
            entry.items.clear();
            entry.current_index = None;
        }
        true
    }
    fn list_box_set_current_index(&self, list_box: u64, index: usize) -> bool {
        if !matches!(self.kind_of(list_box), Some(MacObjc2HandleKind::ListBox)) {
            return false;
        }
        let mut data = self.list_data.lock().expect("mac objc2 list data lock poisoned");
        let entry = match data.get_mut(&list_box) {
            Some(e) => e,
            None => return false,
        };
        if index >= entry.items.len() {
            return false;
        }
        entry.current_index = Some(index);
        true
    }
    fn list_box_current_index(&self, list_box: u64) -> Option<usize> {
        if !matches!(self.kind_of(list_box), Some(MacObjc2HandleKind::ListBox)) {
            return None;
        }
        let data = self.list_data.lock().expect("mac objc2 list data lock poisoned");
        data.get(&list_box).and_then(|entry| entry.current_index)
    }
    fn list_box_item_count(&self, list_box: u64) -> usize {
        if !matches!(self.kind_of(list_box), Some(MacObjc2HandleKind::ListBox)) {
            return 0;
        }
        let data = self.list_data.lock().expect("mac objc2 list data lock poisoned");
        data.get(&list_box).map_or(0, |entry| entry.items.len())
    }
    fn list_box_item_text(&self, list_box: u64, index: usize) -> Option<String> {
        if !matches!(self.kind_of(list_box), Some(MacObjc2HandleKind::ListBox)) {
            return None;
        }
        let data = self.list_data.lock().expect("mac objc2 list data lock poisoned");
        data.get(&list_box).and_then(|entry| entry.items.get(index)).cloned()
    }
    fn combo_box_add_item(&self, combo_box: u64, text: &str) -> bool {
        if !matches!(self.kind_of(combo_box), Some(MacObjc2HandleKind::ComboBox)) {
            return false;
        }
        let mut data = self.list_data.lock().expect("mac objc2 list data lock poisoned");
        let entry = data.entry(combo_box).or_default();
        entry.items.push(text.to_string());
        true
    }
    fn combo_box_clear_items(&self, combo_box: u64) -> bool {
        if !matches!(self.kind_of(combo_box), Some(MacObjc2HandleKind::ComboBox)) {
            return false;
        }
        let mut data = self.list_data.lock().expect("mac objc2 list data lock poisoned");
        if let Some(entry) = data.get_mut(&combo_box) {
            entry.items.clear();
            entry.current_index = None;
        }
        true
    }
    fn combo_box_set_current_index(&self, combo_box: u64, index: usize) -> bool {
        if !matches!(self.kind_of(combo_box), Some(MacObjc2HandleKind::ComboBox)) {
            return false;
        }
        let mut data = self.list_data.lock().expect("mac objc2 list data lock poisoned");
        let entry = match data.get_mut(&combo_box) {
            Some(e) => e,
            None => return false,
        };
        if index >= entry.items.len() {
            return false;
        }
        entry.current_index = Some(index);
        true
    }
    fn combo_box_current_index(&self, combo_box: u64) -> Option<usize> {
        if !matches!(self.kind_of(combo_box), Some(MacObjc2HandleKind::ComboBox)) {
            return None;
        }
        let data = self.list_data.lock().expect("mac objc2 list data lock poisoned");
        data.get(&combo_box).and_then(|entry| entry.current_index)
    }
    fn combo_box_item_count(&self, combo_box: u64) -> usize {
        if !matches!(self.kind_of(combo_box), Some(MacObjc2HandleKind::ComboBox)) {
            return 0;
        }
        let data = self.list_data.lock().expect("mac objc2 list data lock poisoned");
        data.get(&combo_box).map_or(0, |entry| entry.items.len())
    }
    fn combo_box_item_text(&self, combo_box: u64, index: usize) -> Option<String> {
        if !matches!(self.kind_of(combo_box), Some(MacObjc2HandleKind::ComboBox)) {
            return None;
        }
        let data = self.list_data.lock().expect("mac objc2 list data lock poisoned");
        data.get(&combo_box).and_then(|entry| entry.items.get(index)).cloned()
    }
    fn create_panel(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(MacObjc2HandleKind::Panel, "Panel", x, y, width, height);

        #[cfg(all(target_os = "macos", feature = "macos"))]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let panel = super::native::create_ns_panel(mtm, x, y, width, height);
            super::native::store_native_view(id, &*panel as *const _ as *mut std::ffi::c_void);
            super::native::add_as_subview(id, parent);
        }

        id
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
        let id = self.insert_widget(MacObjc2HandleKind::SpinBox, "SpinBox", x, y, width, height);

        #[cfg(all(target_os = "macos", feature = "macos"))]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let stepper = super::native::create_ns_stepper(mtm, x, y, width, height);
            super::native::store_native_view(id, &*stepper as *const _ as *mut std::ffi::c_void);
            super::native::add_as_subview(id, parent);
        }

        id
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
        let id = self.insert_widget(MacObjc2HandleKind::ListView, "ListView", x, y, width, height);

        #[cfg(all(target_os = "macos", feature = "macos"))]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let list = super::native::create_ns_list_box(mtm, x, y, width, height);
            super::native::store_native_view(id, &*list as *const _ as *mut std::ffi::c_void);
            super::native::add_as_subview(id, parent);
        }

        id
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
        let id =
            self.insert_widget(MacObjc2HandleKind::ScrollArea, "ScrollArea", x, y, width, height);

        #[cfg(all(target_os = "macos", feature = "macos"))]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let scroll = super::native::create_ns_scroll_view(mtm, x, y, width, height);
            super::native::store_native_view(id, &*scroll as *const _ as *mut std::ffi::c_void);
            super::native::add_as_subview(id, parent);
        }

        id
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
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::GroupBox, title, x, y, width, height)
    }
    fn create_frame(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::Frame, "Frame", x, y, width, height)
    }
    fn create_tab_widget(
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
        self.insert_widget(MacObjc2HandleKind::TabWidget, "TabWidget", x, y, width, height)
    }
    fn create_splitter(
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
        self.insert_widget(MacObjc2HandleKind::Splitter, "Splitter", x, y, width, height)
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
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(MacObjc2HandleKind::ToggleButton, text, x, y, width, height)
    }
    fn create_calendar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(MacObjc2HandleKind::Calendar, "Calendar", x, y, width, height)
    }
    fn create_scroll_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(MacObjc2HandleKind::ScrollBar, "ScrollBar", x, y, width, height)
    }
    fn create_double_spin_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(
            MacObjc2HandleKind::DoubleSpinBox,
            "DoubleSpinBox",
            x,
            y,
            width,
            height,
        )
    }
    fn create_font_combo_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(
            MacObjc2HandleKind::FontComboBox,
            "FontComboBox",
            x,
            y,
            width,
            height,
        )
    }
    fn create_context_menu(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(
            MacObjc2HandleKind::ContextMenu,
            "ContextMenu",
            x,
            y,
            width,
            height,
        )
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
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(MacObjc2HandleKind::PopupWindow, title, x, y, width, height)
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
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(MacObjc2HandleKind::Dialog, title, x, y, width, height)
    }
    fn create_input_dialog(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(MacObjc2HandleKind::InputDialog, "Input", x, y, width, height)
    }
    fn create_progress_dialog(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(
            MacObjc2HandleKind::ProgressDialog,
            "Progress",
            x,
            y,
            width,
            height,
        )
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
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(MacObjc2HandleKind::DirectoryDialog, title, x, y, width, height)
    }
    fn create_date_picker(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(MacObjc2HandleKind::DatePicker, "DatePicker", x, y, width, height)
    }
    fn create_time_picker(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(MacObjc2HandleKind::TimePicker, "TimePicker", x, y, width, height)
    }
    fn create_date_time_picker(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(
            MacObjc2HandleKind::DateTimePicker,
            "DateTimePicker",
            x,
            y,
            width,
            height,
        )
    }
    fn create_activity_indicator(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(
            MacObjc2HandleKind::ActivityIndicator,
            "ActivityIndicator",
            x,
            y,
            width,
            height,
        )
    }

    // ---- Dialog creation ----
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
        let _ = parent;
        #[cfg(not(all(target_os = "macos", feature = "macos")))]
        let _ = title;
        let id = self.insert_widget(MacObjc2HandleKind::MessageBox, text, x, y, width, height);
        #[cfg(all(target_os = "macos", feature = "macos"))]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let alert = super::native::create_ns_alert(mtm, title, text);
            super::native::store_native_view(id, &*alert as *const _ as *mut std::ffi::c_void);
        }
        id
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
        let id =
            self.insert_widget(MacObjc2HandleKind::FileDialog, "FileDialog", x, y, width, height);
        #[cfg(all(target_os = "macos", feature = "macos"))]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let panel = super::native::create_ns_open_panel(mtm);
            super::native::store_native_view(id, &*panel as *const _ as *mut std::ffi::c_void);
        }
        id
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
        let id =
            self.insert_widget(MacObjc2HandleKind::ColorDialog, "ColorDialog", x, y, width, height);
        #[cfg(all(target_os = "macos", feature = "macos"))]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let panel = super::native::create_ns_color_panel(mtm);
            super::native::store_native_view(id, &*panel as *const _ as *mut std::ffi::c_void);
        }
        id
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
        let id =
            self.insert_widget(MacObjc2HandleKind::FontDialog, "FontDialog", x, y, width, height);
        #[cfg(all(target_os = "macos", feature = "macos"))]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let panel = super::native::create_ns_font_panel(mtm);
            super::native::store_native_view(id, &*panel as *const _ as *mut std::ffi::c_void);
        }
        id
    }
}
