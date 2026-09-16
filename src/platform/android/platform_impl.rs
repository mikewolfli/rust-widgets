// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Android platform trait implementation.
//!
//! Implements the `Platform` contract for Android mobile devices.
//! This is a state-driven backend that can be progressively enhanced
//! with native Android views via JNI bindings.
//!
//! ## JNI Integration Path (android-jni feature)
//!
//! All widget creation methods (`create_window`, `create_button`, etc.)
//! currently delegate to the state backend (`AndroidPlatform::insert_widget`)
//! which returns a monotonically increasing handle ID.
//!
//! To wire real Android Views:
//!
//! 1. Check [`AndroidPlatform::jni_available()`] — returns `true` when
//!    the `android-jni` feature is enabled and `JAVA_VM` is initialized.
//! 2. When JNI is wired, each creation method should additionally call the
//!    corresponding JNI native method to create a real Android View and
//!    register it in the view registry.
//! 3. State operations (`set_widget_text`, `set_widget_geometry`, etc.) should
//!    first perform the Rust-side mutation, then forward the call to JNI.
//! 4. All real JNI code should be feature-gated (`#[cfg(feature = "android-jni")]`)
//!    so the state-only backend remains the default for testing and CI.

use super::types::{AndroidHandleKind, AndroidPlatform};
use crate::core::PlatformFamily;
use crate::platform::{DropEvent, Platform, WidgetTriggerEvent, WidgetTriggerKind};
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

impl AndroidPlatform {}

impl Platform for AndroidPlatform {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn backend_name(&self) -> &'static str {
        "android-state-backend"
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

    /// Android has no desktop print spooler; printing goes through the platform
    /// print framework via JNI, which this state backend does not bind.
    fn spawn_print_job(&self, job_file: &std::path::Path) -> Result<(), String> {
        Err(format!(
            "Android printing requires the platform print framework, which is not bound in \
             this build; job file '{}' was not printed",
            job_file.display()
        ))
    }

    #[cfg(feature = "mobile-api")]
    fn mobile_extension(&self) -> Option<&dyn crate::platform::contract::MobilePlatformExtension> {
        Some(self)
    }

    fn init(&self) {
        let _ = self.android_runtime_marker();
        self.runtime.initialized.store(true, Ordering::SeqCst);
    }

    fn run(&self) {
        if !self.runtime.initialized.load(Ordering::SeqCst) {
            self.init();
        }
        // Android state backend uses polling loop for deterministic behavior.
        self.runtime.running.store(true, Ordering::SeqCst);
        while self.runtime.running.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(16));
        }
    }

    fn quit(&self) {
        self.runtime.running.store(false, Ordering::SeqCst);
    }

    /// Release every registry entry the backend holds for `widget_id`.
    ///
    /// Android keeps one per-widget side table beyond the authoritative
    /// `BackendState` record: the menu bookkeeping (`menus`). It must be purged,
    /// otherwise a UI rebuilt in a create/destroy loop would leak one entry per
    /// discarded widget. The lock is scoped to its own statement so no two guards
    /// are held at the same time.
    fn destroy_widget(&self, widget_id: u64) -> bool {
        {
            let mut menus = self.menus.lock().expect("android menus lock poisoned");
            menus.attached_menu_bar.remove(&widget_id);
            // The widget may be a container in the menu tree: drop both the
            // children it owned and the child entry under its own parent.
            menus.menu_children.remove(&widget_id);
            for children in menus.menu_children.values_mut() {
                children.retain(|child| *child != widget_id);
            }
            // Drop queued triggers that reference a widget that no longer exists.
            menus.pending_menu_events.retain(|queued| *queued != widget_id);
        }

        // The state record is the authority on whether the widget existed.
        self.state.destroy_widget(widget_id)
    }

    // ─── Widget creation ─────────────────────────────────────────────────

    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        self.insert_widget(AndroidHandleKind::Window, title, x, y, width, height)
    }

    // ─── Menu model ──────────────────────────────────────────────────────
    //
    // These are NOT control construction. Android has no standalone menu-bar or
    // menu *View*: the host Activity owns the menu and materialises it through the
    // platform's own `onCreateOptionsMenu` / `onOptionsItemSelected` callbacks. What
    // lives here is the in-process model that maps a Rust-side menu tree onto that
    // callback surface, plus an injectable trigger queue so the library's own menu
    // widget can report activations without a UI toolkit of its own.
    //
    // They therefore survive the self-drawing change, exactly as on iOS: the
    // library paints the menu *appearance*, while the host still owns the menu
    // *identity* the OS asks about. `capabilities().native_menu` stays `false`,
    // because no OS menu object is created here.

    fn create_menu_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if !matches!(self.kind_of(parent), Some(AndroidHandleKind::Window)) {
            return 0;
        }
        self.insert_widget(AndroidHandleKind::MenuBar, "MenuBar", x, y, width, height)
    }

    fn create_menu(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        // A menu hangs off a menu bar or another menu.
        if !matches!(
            self.kind_of(parent),
            Some(AndroidHandleKind::MenuBar) | Some(AndroidHandleKind::Menu)
        ) {
            return 0;
        }
        self.insert_widget(AndroidHandleKind::Menu, text, x, y, width, height)
    }

    /// Binds `menu_bar` to `window` as that window's menu.
    ///
    /// Both ids and their kinds are validated, so a caller cannot attach a menu bar
    /// to something that is not a window (or attach a non-menu-bar), which would make
    /// the host ask a widget for a menu it does not have.
    fn attach_menu_bar_to_window(&self, window: u64, menu_bar: u64) -> bool {
        if !matches!(self.kind_of(window), Some(AndroidHandleKind::Window)) {
            return false;
        }
        if !matches!(self.kind_of(menu_bar), Some(AndroidHandleKind::MenuBar)) {
            return false;
        }
        let mut menus = self.menus.lock().expect("android menus lock poisoned");
        menus.attached_menu_bar.insert(window, menu_bar);
        true
    }

    fn menu_add_item(&self, parent_menu: u64, text: &str, shortcut: Option<&str>) -> u64 {
        // A menu item must hang off a menu.
        if !matches!(self.kind_of(parent_menu), Some(AndroidHandleKind::Menu)) {
            return 0;
        }
        let id = self.insert_widget(AndroidHandleKind::MenuItem, text, 0, 0, 0, 0);

        let display_text = match shortcut {
            Some(shortcut) => format!("{} ({})", text, shortcut),
            None => text.to_string(),
        };
        self.state.set_text(id, &display_text);

        let mut menus = self.menus.lock().expect("android menus lock poisoned");
        menus.menu_children.entry(parent_menu).or_default().push(id);

        // Native menu items are represented through the Activity's own menu
        // resource; no standalone Android View exists for a menu entry.
        id
    }

    fn poll_menu_triggered(&self) -> Option<u64> {
        let mut menus = self.menus.lock().expect("android menus lock poisoned");
        menus.pending_menu_events.pop_front()
    }

    fn inject_menu_trigger(&self, menu_item_id: u64) -> bool {
        // Only a menu item may produce a menu trigger.
        if !matches!(self.kind_of(menu_item_id), Some(AndroidHandleKind::MenuItem)) {
            return false;
        }
        let mut menus = self.menus.lock().expect("android menus lock poisoned");
        menus.pending_menu_events.push_back(menu_item_id);
        true
    }

    fn poll_widget_triggered(&self) -> Option<u64> {
        self.state.pop_widget_trigger()
    }

    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        self.state.pop_widget_trigger_event()
    }

    fn inject_widget_trigger_event(&self, widget_id: u64, kind: WidgetTriggerKind) -> bool {
        self.state.inject_widget_trigger_event(widget_id, kind)
    }

    /// Mounts a library-painted widget onto a surface this host will present.
    ///
    /// No native view is created per control (every `WidgetKind` is painted by
    /// `src/widget/`), so the surface is a record plus a repaint queue: the Activity
    /// owns the pixels and pulls them with the render API.
    fn mount_surface(&self, _parent: u64, id: u64, rect: crate::core::Rect) -> bool {
        self.state.mount_surface_record(id, rect)
    }

    /// Updates the rect of a mounted surface. `false` when `id` is not mounted.
    fn resize_surface(&self, id: u64, rect: crate::core::Rect) -> bool {
        self.state.resize_surface_record(id, rect)
    }

    /// Releases a mounted surface.
    fn unmount_surface(&self, id: u64) -> bool {
        self.state.unmount_surface_record(id)
    }

    /// Queues a repaint for the host to pick up. `false` when `id` is not mounted.
    fn invalidate_surface(&self, id: u64) -> bool {
        self.state.invalidate_surface_record(id)
    }

    /// The backend displays library-painted widgets by handing the host their frames.
    fn supports_surfaces(&self) -> bool {
        true
    }

    // ─── Widget manipulation ─────────────────────────────────────────────

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
        self.state.set_text(widget_id, text);
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

    // ─── Clipboard ───────────────────────────────────────────────────────

    fn set_clipboard_text(&self, text: &str) -> bool {
        self.state.set_clipboard_text(text)
    }

    fn get_clipboard_text(&self) -> String {
        self.state.clipboard_text()
    }

    // ─── Drag and drop ───────────────────────────────────────────────────

    fn begin_drag(&self, source_widget_id: u64, mime: &str, payload: &[u8]) -> bool {
        self.state.begin_drag(source_widget_id, mime, payload)
    }

    fn poll_drop_event(&self) -> Option<DropEvent> {
        self.state.pop_drop_event()
    }

    fn inject_drop_event(&self, event: DropEvent) -> bool {
        self.state.inject_drop_event(event)
    }

    // ─── IME ─────────────────────────────────────────────────────────────

    fn set_widget_ime_enabled(&self, widget_id: u64, enabled: bool) -> bool {
        self.state.set_ime_enabled(widget_id, enabled)
    }

    fn is_widget_ime_enabled(&self, widget_id: u64) -> bool {
        self.state.ime_enabled(widget_id)
    }

    // ─── Accessibility ───────────────────────────────────────────────────

    fn set_widget_accessibility_name(&self, widget_id: u64, name: &str) -> bool {
        self.state.set_accessibility_name(widget_id, name)
    }

    fn get_widget_accessibility_name(&self, widget_id: u64) -> String {
        self.state.accessibility_name(widget_id)
    }
}

// ─── MobilePlatformExtension ─────────────────────────────────────────────

impl crate::platform::contract::MobilePlatformExtension for AndroidPlatform {
    fn mobile_backend(&self) -> crate::platform::MobileBackend {
        crate::platform::MobileBackend::Android
    }

    fn attach_to_native_view(&self, native_handle: usize) -> bool {
        // The handle is the host Activity's Java `Context` reference. Store it as
        // a GlobalRef so a later platform request (a document picker, say) can
        // resolve an `Activity` from any thread; without it this host cannot
        // serve such requests at all.
        #[cfg(feature = "android-jni")]
        {
            if native_handle == 0 || !crate::platform::android_jni::is_initialized() {
                return false;
            }
            // `JObject` does not own the reference; the GlobalRef created by
            // `set_activity_context` is what keeps it alive.
            let context_obj: jni::objects::JObject<'_> =
                unsafe { jni::objects::JObject::from_raw(native_handle as jni::sys::jobject) };
            crate::platform::android_jni::with_jni_env(|env| {
                crate::platform::android_jni::set_activity_context(env, &context_obj)
            })
            .unwrap_or(false)
        }
        #[cfg(not(feature = "android-jni"))]
        {
            let _ = native_handle;
            false
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn android_platform_window_creation() {
        let platform = AndroidPlatform::new();
        platform.init();

        let window_id = platform.create_window("Test Window", 0, 0, 320, 568);
        assert_ne!(window_id, 0);

        assert_eq!(platform.backend_name(), "android-state-backend");
        assert_eq!(platform.family(), PlatformFamily::Mobile);
    }

    /// A control id must never address something the host did not create.
    ///
    /// This replaces a test that asserted the opposite — that a `create_button` under
    /// a valid window produced a live id — because the host no longer builds
    /// controls. Asserting `0` for both a bad *and* a good parent is what makes the
    /// answer meaningful: the id is absent, not merely parent-sensitive.
    #[test]
    fn control_members_report_absence_for_every_parent() {
        let platform = AndroidPlatform::new();
        platform.init();

        assert_eq!(platform.create_button(999, "Button", 0, 0, 80, 44), 0);

        let window_id = platform.create_window("Window", 0, 0, 320, 568);
        assert_ne!(window_id, 0);
        assert_eq!(platform.create_button(window_id, "Button", 0, 0, 80, 44), 0);
    }

    /// The per-control list storage went with the controls that backed it, so these
    /// members must report absence rather than accept writes into a model nothing
    /// can read.
    #[test]
    fn combo_box_data_members_report_absence() {
        let platform = AndroidPlatform::new();
        platform.init();

        let window_id = platform.create_window("Window", 0, 0, 320, 568);
        assert_eq!(platform.create_combo_box(window_id, 0, 0, 200, 40), 0);
        assert!(!platform.combo_box_add_item(window_id, "Item 1"));
        assert_eq!(platform.combo_box_item_count(window_id), 0);
        assert!(!platform.combo_box_clear_items(window_id));
    }

    #[test]
    fn android_menu_requires_correct_parent_kind() {
        let platform = AndroidPlatform::new();
        platform.init();
        let window = platform.create_window("Window", 0, 0, 320, 568);
        let button = platform.create_button(window, "Button", 0, 0, 80, 44);

        // MenuBar requires a window.
        assert_eq!(platform.create_menu_bar(button, 0, 0, 320, 24), 0);
        let menu_bar = platform.create_menu_bar(window, 0, 0, 320, 24);
        assert_ne!(menu_bar, 0);

        // A menu requires a menu bar or another menu.
        assert_eq!(platform.create_menu(window, "Bad", 0, 0, 80, 24), 0);
        assert_eq!(platform.create_menu(button, "Bad", 0, 0, 80, 24), 0);
        let menu = platform.create_menu(menu_bar, "File", 0, 0, 80, 24);
        assert_ne!(menu, 0);
        let submenu = platform.create_menu(menu, "Recent", 0, 0, 80, 24);
        assert_ne!(submenu, 0, "a menu may nest under another menu");

        // A menu item requires a menu.
        assert_eq!(platform.menu_add_item(window, "Bad", None), 0);
        assert_eq!(platform.menu_add_item(menu_bar, "Bad", None), 0);
        let item = platform.menu_add_item(menu, "Open", Some("Ctrl+O"));
        assert_ne!(item, 0);

        // Only a menu item may be injected as a menu trigger.
        assert!(!platform.inject_menu_trigger(window));
        assert!(!platform.inject_menu_trigger(menu_bar));
        assert!(platform.inject_menu_trigger(item));
        assert_eq!(platform.poll_menu_triggered(), Some(item));

        // attach_menu_bar_to_window validates both ids and their kinds.
        assert!(!platform.attach_menu_bar_to_window(9999, menu_bar));
        assert!(!platform.attach_menu_bar_to_window(window, item));
        assert!(platform.attach_menu_bar_to_window(window, menu_bar));
    }

    #[test]
    fn android_menu_item_retains_shortcut() {
        let platform = AndroidPlatform::new();
        platform.init();
        let window = platform.create_window("Window", 0, 0, 320, 568);
        let menu_bar = platform.create_menu_bar(window, 0, 0, 320, 24);
        let menu = platform.create_menu(menu_bar, "File", 0, 0, 80, 24);

        let plain = platform.menu_add_item(menu, "Open", None);
        assert_eq!(platform.get_widget_text(plain), "Open");

        let with_shortcut = platform.menu_add_item(menu, "Save", Some("Ctrl+S"));
        assert_eq!(platform.get_widget_text(with_shortcut), "Save (Ctrl+S)");
    }

    /// The backend must host library-painted widgets, and each step must work:
    /// a bare `true` from `supports_surfaces()` would be a claim, not a capability.
    #[test]
    fn android_hosts_widget_surfaces_and_queues_repaints() {
        let platform = AndroidPlatform::new();
        assert!(platform.supports_surfaces());

        let window = platform.create_window("Window", 0, 0, 412, 915);
        let rect = crate::core::Rect::new(0, 0, 120, 44);
        assert!(platform.mount_surface(window, window, rect));
        assert_eq!(platform.state.surface_rect(window), Some(rect));

        assert!(platform.invalidate_surface(window));
        assert_eq!(platform.state.take_pending_repaint(), Some(window));
        assert_eq!(platform.state.pending_repaint_count(), 0);

        let moved = crate::core::Rect::new(8, 8, 200, 80);
        assert!(platform.resize_surface(window, moved));
        assert_eq!(platform.state.surface_rect(window), Some(moved));

        assert!(platform.unmount_surface(window));
        assert_eq!(platform.state.surface_rect(window), None);
    }

    /// A surface for a widget this backend never made must be refused.
    #[test]
    fn android_refuses_a_surface_for_an_unknown_widget() {
        let platform = AndroidPlatform::new();
        assert!(!platform.mount_surface(1, 9_999, crate::core::Rect::new(0, 0, 10, 10)));
        assert!(!platform.invalidate_surface(9_999));
        assert_eq!(platform.state.mounted_surface_count(), 0);
    }
}
