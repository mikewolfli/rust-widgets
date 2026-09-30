// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Mobile phase-1 platform slice (Android baseline).
use super::state::BackendState;
use super::{
    MobileBackend, MobilePlatformExtension, Platform, WidgetTriggerEvent, WidgetTriggerKind,
};
use crate::compat::atomic::{AtomicUsize, Ordering};
use crate::compat::{HashMap, Mutex, OnceLock};
use crate::core::{ObjectId, PlatformFamily};
use crate::platform::types::{host_battery_probe, host_memory_probe, host_process_memory_probe};
use std::thread;
use std::time::Duration;

/// How often the run loop ticks, in milliseconds — one 60 Hz frame.
///
/// The same value as the Android, iOS and HarmonyOS backends. It is a *frame* interval,
/// not a poll interval for a network-like resource: this loop is the single place a
/// backend turns "what the host queued" into "what the widgets did", so the tick rate is
/// the upper bound on how quickly a resize or an animation reaches the tree.
const FRAME_INTERVAL_MS: u64 = 16;

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
    /// Whether `init` has run, and whether the run loop is still going.
    ///
    /// Held here rather than in a shared `RuntimeState` because this backend has only
    /// these two facts to keep: `run` must not depend on `init` having been called
    /// explicitly (a host that goes straight to `run` still needs the drain), and `quit`
    /// must be able to stop a loop that is already running.
    initialized: std::sync::atomic::AtomicBool,
    running: std::sync::atomic::AtomicBool,
}
impl AndroidMobilePlatform {
    /// Creates a new Android mobile platform adapter.
    pub fn new() -> Self {
        Self {
            state: BackendState::new(),
            attached_native_view: AtomicUsize::new(0),
            menus: Mutex::new(MobileMenuState::default()),
            initialized: std::sync::atomic::AtomicBool::new(false),
            running: std::sync::atomic::AtomicBool::new(false),
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
    // The uniform widget-property methods are answered once, over `self.state`, by the
    // shared expansion in `platform::state_impl` rather than re-written per backend.
    crate::impl_platform_state_properties!();

    fn as_any(&self) -> &dyn crate::compat::Any {
        self
    }
    fn backend_name(&self) -> &'static str {
        "android-mobile"
    }
    fn family(&self) -> PlatformFamily {
        PlatformFamily::Mobile
    }

    /// Reads installed physical memory from whichever kernel probe this host has.
    ///
    /// This backend is the **Android** adapter, and Android sits on a Linux kernel,
    /// so `/proc/meminfo` is the correct source and [`os_probes`](crate::platform::os_probes)
    /// answers it. On an Apple host — where this type is compiled as the state-machine
    /// stand-in rather than a real backend — the Darwin probes answer instead. Neither
    /// is a fabricated figure, which is what principle #37 requires.
    fn total_memory_mb(&self) -> Option<u64> {
        host_memory_probe()
    }

    /// Reports whether the machine is drawing from its battery.
    ///
    /// See [`Self::total_memory_mb`] for why the source follows the host kernel.
    fn is_on_battery(&self) -> bool {
        host_battery_probe()
    }

    /// Samples this process's resident memory against its reserved address space.
    ///
    /// See [`Self::total_memory_mb`] for why the source follows the host kernel.
    fn process_memory_utilization(&self) -> Option<f32> {
        host_process_memory_probe()
    }

    /// CPU tick accounting has no lock-free source here, so this reports `None`
    /// rather than a fabricated figure.
    fn process_cpu_utilization(&self) -> Option<f32> {
        None
    }

    /// Android printing goes through the platform print framework via JNI, which
    /// this preview backend does not bind.
    fn spawn_print_job(&self, job_file: &std::path::Path) -> Result<(), String> {
        Err(format!(
            "Android printing requires the platform print framework, which is not bound in \
             this build; job file '{}' was not printed",
            job_file.display()
        ))
    }
    fn init(&self) {
        log::info!("[mobile] AndroidMobilePlatform init (state-only preview backend)");
        self.initialized.store(true, Ordering::SeqCst);
    }

    /// The run loop: this backend's drain tick.
    ///
    /// # Why a state-only backend needs a loop at all
    ///
    /// `AndroidMobilePlatform` is the *preview* backend — what the mobile API falls back
    /// to when the active platform has no mobile extension (`platform::runtime`), which is
    /// the ordinary case when running the mobile API on a desktop host. It creates no
    /// OS window, but it is still the backend a `create_window` call lands on, and
    /// [`Platform::queue_resize_trigger`](super::Platform::queue_resize_trigger) still
    /// forwards into the control backend, which records the size and queues a
    /// `Resized` event.
    ///
    /// This method used to be a single `log::info!`. The queue was therefore filled and
    /// never drained: the resize was accepted, the size was recorded, and the window's
    /// layout never re-ran — the exact failure the HarmonyOS backend documents having
    /// fixed (`harmony/platform_impl.rs`: "a `Resized` event sat in the queue and no window
    /// layout re-ran"). Android, iOS, HarmonyOS and wasm all tick; this one did not, and
    /// nothing reported it because the event is popped-if-something-pops-it.
    ///
    /// `crate::drive_frame` both drains the trigger queue and advances the animation bus,
    /// in the order they must happen. See the HarmonyOS loop for the same reasoning.
    fn run(&self) {
        if !self.initialized.load(Ordering::SeqCst) {
            self.init();
        }
        self.running.store(true, Ordering::SeqCst);
        log::info!("[mobile] AndroidMobilePlatform run (state-only preview backend)");
        while self.running.load(Ordering::SeqCst) {
            crate::drive_frame(FRAME_INTERVAL_MS as u32);
            thread::sleep(Duration::from_millis(FRAME_INTERVAL_MS));
        }
    }

    fn quit(&self) {
        self.running.store(false, Ordering::SeqCst);
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
            let mut menus = crate::compat::lock(&self.menus);
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
    /// See [`crate::drain_widget_triggers_for`] for why the targeted pop exists.
    fn pop_widget_trigger_event_for(&self, widget_id: ObjectId) -> Option<WidgetTriggerEvent> {
        self.state.pop_widget_event_for(widget_id)
    }
    fn inject_widget_trigger_event(&self, widget_id: ObjectId, kind: WidgetTriggerKind) -> bool {
        if !self.state.contains_widget(widget_id) {
            return false;
        }
        self.state.push_widget_event(WidgetTriggerEvent { widget_id, kind });
        true
    }

    /// Mounts a library-painted widget onto a surface this host will present.
    ///
    /// The host owns the pixels (see [`Self::attach_to_native_view`]); this records
    /// which widgets are being displayed and queues repaints for the host to drain.
    /// Before this the mobile backend had no surface path at all, so a host that asked
    /// could not display anything.
    fn mount_surface(&self, _parent: ObjectId, id: ObjectId, rect: crate::core::Rect) -> bool {
        self.state.mount_surface_record(id, rect)
    }

    /// Updates the rect of a mounted surface. `false` when `id` is not mounted.
    fn resize_surface(&self, id: ObjectId, rect: crate::core::Rect) -> bool {
        self.state.resize_surface_record(id, rect)
    }

    /// Releases a mounted surface.
    fn unmount_surface(&self, id: ObjectId) -> bool {
        self.state.unmount_surface_record(id)
    }

    /// The window's current client size, as last reported by the host.
    ///
    /// Falls back to the size the window was created with. `None` for an id this backend
    /// does not know, so a caller can tell "no such window" from "a size I can use".
    fn window_client_size(&self, window_id: ObjectId) -> Option<(u32, u32)> {
        // Ask the control backend, which owns the window and is therefore the only
        // store that knows the size a resize reported.
        crate::window_client_size(window_id).or_else(|| self.state.window_size(window_id))
    }

    /// Reports a container's new client size and queues a `Resized` trigger.
    fn queue_resize_trigger(&self, window_id: ObjectId, width: u32, height: u32) -> bool {
        // Forward to the control backend, which owns the window and the queue the app
        // polls. Writing to the platform's own state would land in a store the host
        // never reads, because `create_window` goes through the control backend.
        crate::queue_resize_trigger(window_id, width, height)
    }

    /// Queues a repaint for the host to pick up. `false` when `id` is not mounted.
    ///
    /// # Why this is overridden rather than inherited
    ///
    /// The trait's default for a surface record would answer `false` for a window
    /// — it asks "is `id` a mounted surface?" and a window is not one. But the
    /// library asks for a **window** to be repainted whenever a child control
    /// changes (see `widget::runtime::request_repaint_subtree`), because a window's
    /// frame is what draws its ordinary children. Inheriting the surface-only
    /// default therefore made every such request a silent no-op on this backend,
    /// so a control could answer its event and never appear to change.
    ///
    /// [`BackendState::record_repaint_request`] is the honest question here — "do
    /// I know this widget?" — and it queues for a window exactly as it does for a
    /// mounted surface. A host draining [`crate::platform::state::BackendState::take_pending_repaint`]
    /// resolves the id it receives with `crate::app::window_handle_for` (a widget
    /// id) or `surface_rect` (a directly mounted one); see that method's docs.
    fn invalidate_surface(&self, id: ObjectId) -> bool {
        self.state.record_repaint_request(id)
    }

    /// The backend displays library-painted widgets by handing the host their frames.
    ///
    /// See [`crate::platform::Platform::supports_surfaces`] and
    /// [`crate::platform::Platform::invalidate_surface`] for what this does and does
    /// not promise: it advertises [`crate::platform::Platform::mount_surface`], not
    /// "the container's frame shows its children".
    fn supports_surfaces(&self) -> bool {
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

    /// The mobile backend must host library-painted widgets: the attached native view
    /// is the surface, and the host drains repaints from this queue.
    #[test]
    fn mobile_backend_hosts_widget_surfaces() {
        let platform = AndroidMobilePlatform::new();
        assert!(platform.supports_surfaces());

        let window = platform.create_window("Window", 0, 0, 360, 780);
        let rect = crate::core::Rect::new(0, 0, 100, 40);
        assert!(platform.mount_surface(window, window, rect));
        assert_eq!(platform.state.surface_rect(window), Some(rect));
        assert_eq!(platform.state.mounted_surface_count(), 1);

        // Coalesced: two invalidations in one frame produce one repaint.
        assert!(platform.invalidate_surface(window));
        assert!(platform.invalidate_surface(window));
        assert_eq!(platform.state.pending_repaint_count(), 1);
        assert_eq!(platform.state.take_pending_repaint(), Some(window));
        assert_eq!(platform.state.pending_repaint_count(), 0);

        // A gone widget cannot stay queued for a repaint nobody can produce.
        assert!(platform.invalidate_surface(window));
        assert!(platform.unmount_surface(window));
        assert_eq!(platform.state.surface_rect(window), None);
        assert_eq!(platform.state.pending_repaint_count(), 0);
    }

    /// A surface for a widget this backend never made must be refused.
    #[test]
    fn mobile_backend_refuses_a_surface_for_an_unknown_widget() {
        let platform = AndroidMobilePlatform::new();
        assert!(!platform.mount_surface(1, 9_999, crate::core::Rect::new(0, 0, 10, 10)));
        assert!(!platform.invalidate_surface(9_999));
        assert!(!platform.resize_surface(9_999, crate::core::Rect::new(0, 0, 10, 10)));
        assert!(!platform.unmount_surface(9_999));
    }
}
