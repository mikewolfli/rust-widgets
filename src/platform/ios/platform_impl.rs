// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! iOS platform trait implementation.
//!
//! Implements the `Platform` contract for iOS mobile devices. This is a
//! state-driven backend: every widget is recorded in `BackendState<IosHandleKind>`
//! and the trait defaults cover anything the host cannot supply.
//!
//! # BLUE15: the host no longer builds UIKit controls
//!
//! This backend used to instantiate a real `UIView` per logical widget —
//! `UIButton`, `UILabel`, `UISwitch`, `UITextField`, `UITableView`, … — and then
//! forward every state mutation to the live object. Under the self-drawn strategy
//! that is exactly the duplication BLUE15 removes: the library paints every
//! `WidgetKind`, and the host owes the widget layer a **window** and a **drawing
//! surface** (rules #55/#56). A per-kind `create_*` here had no OS object to map
//! onto, so the control creators and the view registry that existed only to serve
//! them are gone (rule #59: delete means delete).
//!
//! What survives is the platform-facing part the library cannot replace: the
//! window lifecycle, the runtime loop, `UIPrintInteractionController`-backed facts
//! reported honestly (see `spawn_print_job`), the clipboard, IME flags,
//! accessibility names, and the injectable menu / widget-trigger queues.
//!
//! ## Menu / status-bar semantics (iOS)
//!
//! iOS has no desktop `MenuBar`/`StatusBar` chrome. These handles are modelled
//! as **in-process data**: kind-constrained parents, textual payload, and
//! injectable trigger events. They are intentionally *not* advertised as native
//! menu capability (`capabilities().native_menu == false`) and never attached to
//! a UIKit view. Apps that need UIKit contextual menus should build them from the
//! same data instead of treating these handles as native menus.

use super::types::{IosHandleKind, IosMobilePlatform};
use crate::compat::atomic::Ordering;
use crate::compat::{format, lock, String};
use crate::core::PlatformFamily;
use crate::platform::{
    DropEvent, Platform, PlatformCapabilities, WidgetTriggerEvent, WidgetTriggerKind,
};
use core::time::Duration;
use std::thread;

impl Platform for IosMobilePlatform {
    fn as_any(&self) -> &dyn crate::compat::Any {
        self
    }

    fn backend_name(&self) -> &'static str {
        "ios-state-backend"
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

    /// iOS printing goes through `UIPrintInteractionController`, not a spooler
    /// command, so this state backend cannot submit a job file.
    fn spawn_print_job(&self, job_file: &std::path::Path) -> Result<(), String> {
        Err(format!(
            "iOS printing requires UIPrintInteractionController, which is not bound in this \
             build; job file '{}' was not printed",
            job_file.display()
        ))
    }

    /// iOS uses the same AppKit-style accelerator symbols as macOS (`⌘⇧Z`).
    fn shortcut_style(&self) -> crate::shortcut::PlatformShortcutStyle {
        crate::shortcut::PlatformShortcutStyle::Mac
    }

    #[cfg(feature = "mobile-api")]
    fn mobile_extension(&self) -> Option<&dyn crate::platform::types::MobilePlatformExtension> {
        Some(self)
    }

    fn capabilities(&self) -> PlatformCapabilities {
        PlatformCapabilities {
            dpi_scaling: true,
            ime: true,
            accessibility: true,
            native_menu: false,
            typed_widget_trigger: true,
        }
    }

    fn init(&self) {
        let _ = self.ios_runtime_marker();
        self.runtime.initialized.store(true, Ordering::SeqCst);
    }

    fn run(&self) {
        if !self.runtime.initialized.load(Ordering::SeqCst) {
            self.init();
        }
        // iOS state backend uses polling loop for deterministic behavior.
        self.runtime.running.store(true, Ordering::SeqCst);
        while self.runtime.running.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(16));
        }
    }

    fn quit(&self) {
        self.runtime.running.store(false, Ordering::SeqCst);
    }

    fn destroy_widget(&self, widget_id: u64) -> bool {
        // The state record is the authority on whether the widget existed.
        let existed = self.state.destroy_widget(widget_id);

        // Drop the per-widget list storage for both list-backed widgets. Each lock
        // is released at the end of its statement so no two guards are held at once.

        // Drop the menu bookkeeping that names this widget: attached menu-bar
        // ownership, membership in a parent menu's child list, and any queued
        // trigger that would otherwise fire for a widget that no longer exists.
        let mut menus = lock(&self.menus);
        menus.attached_menu_bar.retain(|_window, menu_bar| *menu_bar != widget_id);
        let destroyed_children = menus.menu_children.remove(&widget_id);
        menus.menu_children.retain(|_parent, children| {
            children.retain(|child| *child != widget_id);
            !children.is_empty()
        });
        menus.pending_menu_events.retain(|queued| *queued != widget_id);
        drop(menus);

        // A destroyed `Menu` owns child menu items that were only reachable
        // through it; cascade the teardown so those items are not orphaned.
        for child in destroyed_children.into_iter().flatten() {
            self.destroy_widget(child);
        }

        existed
    }

    // ─── Window ───

    /// Creates the host window and records it.
    ///
    /// The window is the one OS object this backend creates. Under self-drawing the
    /// *controls* are the library's job (BLUE15 #55/#56), but a window is not: iOS
    /// requires a real `UIWindow` to have a place to draw into at all, which is why
    /// `native::create_ui_window` exists.
    ///
    /// Calling it is what makes the FFI helper live rather than orphaned. It is
    /// gated on the real iOS target, because the helper is compiled only there; the
    /// backend's state machine still runs on every host so its tests stay
    /// executable, and that is why the call is inside a `cfg` rather than around it.
    /// The UIKit handle is deliberately **not** stored: no control mutator needs it,
    /// and the library paints through the surface attached via
    /// `MobilePlatformExtension::attach_to_native_view`.
    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        let id = self.insert_widget(IosHandleKind::Window, title, x, y, width, height);

        #[cfg(all(target_os = "ios", feature = "ios-uikit-ffi"))]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            // The window must exist for the surface to have a superview. Dropping the
            // `Retained<UIWindow>` is correct: UIKit owns it from
            // `makeKeyAndVisible()`, and leaking the Rust handle would keep a
            // reference past the app's lifetime.
            let _window = super::native::create_ui_window(mtm, title, x, y, width, height);
        }

        id
    }

    fn set_widget_text(&self, widget_id: u64, text: &str) {
        let _ = self.state.set_text(widget_id, text);
    }

    fn set_widget_geometry(&self, widget_id: u64, x: i32, y: i32, width: u32, height: u32) {
        self.state.set_geometry(widget_id, x, y, width, height);
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

    // ─── Menu Bar / Menu / Menu Item ───
    //
    // iOS has no desktop menu chrome, and no native menu protocol either. These
    // handles are an in-process data model: MenuBar is owned by a Window, Menu
    // belongs to a MenuBar/Menu, and MenuItem belongs to a Menu. Triggers are
    // delivered through the injectable `pending_menu_events` queue, so the library's
    // own menu widget can report activations; `capabilities().native_menu` stays
    // false, because no OS menu object is created.
    //
    // They survive the self-drawing change for the same reason the window does: the
    // host owns the menu *identity* an OS menu surface would need, while the library
    // paints the menu *appearance*.

    fn create_menu_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if !matches!(self.kind_of(parent), Some(IosHandleKind::Window)) {
            return 0;
        }
        self.insert_widget(IosHandleKind::MenuBar, "MenuBar", x, y, width, height)
    }

    fn create_menu(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        // A menu hangs off a menu bar or another menu.
        if !matches!(self.kind_of(parent), Some(IosHandleKind::MenuBar) | Some(IosHandleKind::Menu))
        {
            return 0;
        }
        self.insert_widget(IosHandleKind::Menu, text, x, y, width, height)
    }

    fn attach_menu_bar_to_window(&self, window: u64, menu_bar: u64) -> bool {
        if !matches!(self.kind_of(window), Some(IosHandleKind::Window)) {
            return false;
        }
        if !matches!(self.kind_of(menu_bar), Some(IosHandleKind::MenuBar)) {
            return false;
        }
        let mut menus = lock(&self.menus);
        menus.attached_menu_bar.insert(window, menu_bar);
        true
    }

    fn menu_add_item(&self, parent_menu: u64, text: &str, _shortcut: Option<&str>) -> u64 {
        if !matches!(self.kind_of(parent_menu), Some(IosHandleKind::Menu)) {
            return 0;
        }
        let id = self.insert_widget(IosHandleKind::MenuItem, text, 0, 0, 0, 0);

        let mut menus = lock(&self.menus);
        menus.menu_children.entry(parent_menu).or_default().push(id);

        id
    }

    fn poll_menu_triggered(&self) -> Option<u64> {
        lock(&self.menus).pending_menu_events.pop_front()
    }

    fn inject_menu_trigger(&self, menu_item_id: u64) -> bool {
        if !matches!(self.kind_of(menu_item_id), Some(IosHandleKind::MenuItem)) {
            return false;
        }
        lock(&self.menus).pending_menu_events.push_back(menu_item_id);
        true
    }

    // ─── Widget Trigger Events ───

    fn poll_widget_triggered(&self) -> Option<u64> {
        self.poll_widget_trigger_event().map(|event| event.widget_id)
    }

    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        self.state.pop_widget_event()
    }

    fn inject_widget_trigger_event(&self, widget_id: u64, kind: WidgetTriggerKind) -> bool {
        if self.kind_of(widget_id).is_none() {
            return false;
        }
        self.state.push_widget_event(WidgetTriggerEvent { widget_id, kind });
        true
    }

    /// Mounts a library-painted widget onto a surface this host will present.
    ///
    /// The backend keeps no native object per control (every `WidgetKind` is painted
    /// by `src/widget/`), so the surface is a record plus a repaint queue: the UIKit
    /// side owns the pixels and pulls them with the render API, and this tells it
    /// which widgets exist and when they went stale.
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

    /// The window's current client size, as last reported by the host.
    ///
    /// Falls back to the size the window was created with. `None` for an id this backend
    /// does not know, so a caller can tell "no such window" from "a size I can use".
    fn window_client_size(&self, window_id: u64) -> Option<(u32, u32)> {
        // Ask the control backend, which owns the window and is therefore the only
        // store that knows the size a resize reported.
        crate::window_client_size(window_id).or_else(|| self.state.window_size(window_id))
    }

    /// Reports a container's new client size and queues a `Resized` trigger.
    ///
    /// # Who calls this on iOS
    ///
    /// The **host**, not this backend. This platform has no resize callback of its own:
    /// a `UIWindow` fills its scene and the size change arrives at the host's view
    /// controller (`viewDidLayoutSubviews`) or at its
    /// `UIDevice.orientationDidChangeNotification` observer, neither of which the library
    /// sees — it has no window delegate and deliberately stores no UIKit handle (see
    /// `create_window`). Fabricating a subscription here would mean holding a UIKit object
    /// the module documents as not holding.
    ///
    /// So an iOS host that wants its layout to follow the screen reports the new size here
    /// (or through the crate-level [`crate::queue_resize_trigger`], which is the same
    /// call). A host that does not is not broken: its window keeps the size it was created
    /// with, which is what `window_client_size` falls back to.
    fn queue_resize_trigger(&self, window_id: u64, width: u32, height: u32) -> bool {
        // Forward to the control backend, which owns the window and the queue the app
        // polls. Writing to the platform's own state would land in a store the host
        // never reads, because `create_window` goes through the control backend.
        crate::queue_resize_trigger(window_id, width, height)
    }

    /// Queues a repaint for the host to pick up. `false` when `id` is not mounted.
    fn invalidate_surface(&self, id: u64) -> bool {
        self.state.invalidate_surface_record(id)
    }

    /// The backend displays library-painted widgets by handing the host their frames.
    fn supports_surfaces(&self) -> bool {
        true
    }

    // ─── Tool Bar / Status Bar ───

    // ─── Message Box ───

    // ─── Dialogs (state-only on iOS) ───

    // ─── Spin Box ───

    // ─── List View ───

    // ─── Show / Hide ───

    fn show_widget(&self, widget_id: u64) {
        self.state.set_visible(widget_id, true);
    }

    fn hide_widget(&self, widget_id: u64) {
        self.state.set_visible(widget_id, false);
    }

    // ─── Enabled / Visible ───

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
}

#[cfg(all(test, not(alloc_frugal)))]
mod tests {
    use super::*;

    #[test]
    fn ios_platform_window_creation() {
        let platform = IosMobilePlatform::new();
        platform.init();

        let window_id = platform.create_window("Test Window", 0, 0, 320, 568);
        assert_ne!(window_id, 0);

        assert_eq!(platform.backend_name(), "ios-state-backend");
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
        let platform = IosMobilePlatform::new();
        platform.init();

        assert_eq!(platform.create_button(999, "Button", 0, 0, 80, 44), 0);

        let window_id = platform.create_window("Window", 0, 0, 320, 568);
        assert_ne!(window_id, 0);
        assert_eq!(platform.create_button(window_id, "Button", 0, 0, 80, 44), 0);
    }

    /// The list/combo data members are gone with the controls that backed them, so
    /// their trait defaults must report failure rather than accept writes into a
    /// model nothing can read.
    #[test]
    fn list_and_combo_data_members_report_failure() {
        let platform = IosMobilePlatform::new();
        platform.init();

        let window_id = platform.create_window("Window", 0, 0, 320, 568);
        assert!(!platform.list_box_add_item(window_id, "Item 1"));
        assert!(!platform.list_box_clear_items(window_id));
        assert!(!platform.combo_box_add_item(window_id, "Item 1"));
        assert_eq!(platform.combo_box_item_count(window_id), 0);
    }

    #[cfg(all(feature = "serde_json", feature = "serde", widgets_unstripped))]
    #[test]
    fn ios_platform_state_serialization() {
        let platform = IosMobilePlatform::new();
        platform.init();

        let _window_id = platform.create_window("Window", 0, 0, 320, 568);
        let result = platform.serialize_state();
        assert!(result.is_ok());
    }

    #[test]
    fn ios_platform_reports_explicit_mobile_capabilities() {
        let platform = IosMobilePlatform::new();
        let caps = platform.capabilities();

        assert_eq!(platform.family(), PlatformFamily::Mobile);
        assert!(caps.dpi_scaling);
        assert!(caps.ime);
        assert!(caps.accessibility);
        assert!(!caps.native_menu);
        assert!(caps.typed_widget_trigger);
    }

    /// The host must create **no** control: the library paints every `WidgetKind`,
    /// so a `create_*` that answered with a live id would announce a capability the
    /// backend does not have (BLUE15 #55/#56). The trait defaults report `0`, and
    /// this pins that — a regression here would mean a control was reintroduced.
    #[test]
    fn host_creates_no_controls_only_a_window() {
        let platform = IosMobilePlatform::new();
        platform.init();

        let window = platform.create_window("Window", 0, 0, 320, 568);
        assert_ne!(window, 0, "the window is the one primitive the host owns");
        assert_eq!(platform.kind_of(window), Some(IosHandleKind::Window));

        // Every control member now inherits the trait default, which reports that
        // this host provides no such primitive rather than inventing an id.
        assert_eq!(platform.create_button(window, "OK", 0, 0, 80, 44), 0);
        assert_eq!(platform.create_label(window, "hi", 0, 0, 80, 44), 0);
        assert_eq!(platform.create_list_box(window, 0, 0, 320, 120), 0);
        assert_eq!(platform.create_combo_box(window, 0, 0, 160, 44), 0);
    }

    #[test]
    fn ios_platform_capabilities_describe_a_self_drawn_host() {
        let platform = IosMobilePlatform::new();
        let caps = platform.capabilities();

        // iOS ships no native menu bar; the menu model is in-process bookkeeping,
        // which is why the capability is advertised as `false`.
        assert!(!caps.native_menu);
        assert!(caps.typed_widget_trigger);
    }

    /// The backend must host library-painted widgets, and each step must work:
    /// a bare `true` from `supports_surfaces()` would be a claim, not a capability.
    #[test]
    fn ios_hosts_widget_surfaces_and_queues_repaints() {
        use crate::platform::Platform as _;

        let platform = IosMobilePlatform::new();
        assert!(platform.supports_surfaces());

        let window = platform.create_window("w", 0, 0, 390, 844);
        let rect = crate::core::Rect::new(0, 0, 120, 44);
        assert!(platform.mount_surface(window, window, rect));
        assert_eq!(platform.state.surface_rect(window), Some(rect));

        assert!(platform.invalidate_surface(window));
        assert_eq!(platform.state.take_pending_repaint(), Some(window));
        assert_eq!(platform.state.pending_repaint_count(), 0);

        assert!(platform.unmount_surface(window));
        assert_eq!(platform.state.surface_rect(window), None);
    }

    /// A surface for a widget this backend never made must be refused.
    #[test]
    fn ios_refuses_a_surface_for_an_unknown_widget() {
        use crate::platform::Platform as _;

        let platform = IosMobilePlatform::new();
        assert!(!platform.mount_surface(1, 9_999, crate::core::Rect::new(0, 0, 10, 10)));
        assert!(!platform.invalidate_surface(9_999));
        assert_eq!(platform.state.mounted_surface_count(), 0);
    }
}
