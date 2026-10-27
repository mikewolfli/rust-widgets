// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `Platform` implementation for the HarmonyOS / OpenHarmony backend.
//!
//! The backend is state-driven: widget creation, geometry, text, visibility, menus,
//! clipboard, drag/drop and IME metadata all live in
//! [`crate::platform::state::BackendState`], and library-painted widgets are
//! displayed through the surface methods (see `super::status.md`).
//!
//! The one fact worth knowing before editing anything here: OpenHarmony targets
//! report `target_env = "ohos"` and `target_os = "linux"`, so backend selection keys
//! off `target_env` — see [`crate::platform::profile::is_openharmony_target`].

use super::super::Platform;
use super::types::*;
use crate::compat::atomic::Ordering;
use crate::compat::{format, String, Vec};
use crate::core::{ObjectId, PlatformFamily};
use crate::platform::types::{Display, DisplayId};
use crate::{WidgetTriggerEvent, WidgetTriggerKind};

/// The frame interval this backend's loop runs at, in milliseconds.
///
/// The same value as every other backend's twin constant, and named here for the same
/// reason: the sleep between iterations and the delta handed to [`crate::drive_frame`]
/// must be the same number, or every transition runs at the ratio between them with
/// nothing to notice the mismatch. A backend that later runs at the display's own rate
/// changes exactly this value.
const FRAME_INTERVAL_MS: u64 = crate::platform::types::FRAME_INTERVAL_MS;

impl Platform for HarmonyPlatform {
    // The uniform widget-property methods are answered once, over `self.state`, by the
    // shared expansion in `platform::state_impl` rather than re-written per backend.
    crate::impl_platform_state_properties!();

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    /// The backend identifier, in the open-ended `family-name` form every other
    /// backend uses: `gtk`, `cocoa`, `wayland`, `wasm-state-backend`,
    /// `android-state-backend`, `ios-state-backend`, `macos-objc2-preview`,
    /// `portable`.
    ///
    /// # Why this is no longer `"harmony-desktop"`
    ///
    /// The name carried a *desktop* classification the backend does not act on. The
    /// [`Self::family`] answer is [`PlatformFamily::Desktop`], so a backend that
    /// models a phone still asks for desktop capability defaults, and the name
    /// advertised that as if it were a decision. HarmonyOS is the mobile OS this
    /// backend exists for; the only reason the desktop instance is the one that gets
    /// constructed today is that no HarmonyOS mobile lane exists yet (the
    /// `mobile-*` surface is Android/iOS, see [`MobilePlatformExtension`]).
    ///
    /// Renaming to the state-backed spelling makes the name describe **what the
    /// backend is** — an in-process state backend, exactly like its Android and iOS
    /// siblings — instead of which host happened to select it. `cocoa` is the only
    /// remaining backend whose name is short, and that is the historical one.
    ///
    /// [`MobilePlatformExtension`]: crate::platform::types::MobilePlatformExtension
    fn backend_name(&self) -> &'static str {
        "harmony-state-backend"
    }
    fn family(&self) -> PlatformFamily {
        PlatformFamily::Desktop
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

    /// CPU utilization has no reliable one-shot source here, so this reports `None` (see
    /// `os_probes::process_cpu_utilization`); the thread budget is a separate fact.
    fn process_cpu_utilization(&self) -> Option<f32> {
        crate::platform::os_probes::process_cpu_utilization()
    }

    /// HarmonyOS printing is served by the ArkUI print service, which this state
    /// backend does not bind; it reports the gap instead of faking success.
    fn spawn_print_job(&self, job_file: &std::path::Path) -> Result<(), String> {
        Err(format!(
            "HarmonyOS printing requires the ArkUI print service, which is not bound in this \
             build; job file '{}' was not printed",
            job_file.display()
        ))
    }

    /// Whether ArkUI exposes a print spooler to this backend.
    ///
    /// `PrintDialog` asks this before offering the action, so a host without the
    /// service gets a truthful `false` instead of a dialog that discards the
    /// document.
    fn has_print_support(&self) -> bool {
        false
    }

    /// Capabilities published by the Harmony backend.
    ///
    /// # The rule these flags follow
    ///
    /// A flag on this struct means "the method behind it is implemented **by this backend**
    /// and will answer for this host". Each has a named backing method —
    /// `dpi_scale_factor()`, `ime_bridge()` and `accessibility_bridge()` — so a `true` that
    /// has no backing method promises a method that cannot answer, which is the one
    /// direction a default must never err.
    ///
    /// # `accessibility` is now `true`, and why
    ///
    /// This backend now implements `accessibility_bridge()`
    /// ([`super::accessibility::HarmonyAccessibilityBridge`]), so the flag has a real
    /// method behind it and the pair is consistent. The bridge is gated on `xcomponent`
    /// like the XComponent bind it depends on, because the `ArkUI_AccessibilityProvider`
    /// is only reachable through a bound `OH_NativeXComponent`.
    ///
    /// # `dpi_scaling` follows the device build; `ime` is a real bridge
    ///
    /// * `dpi_scaling` — no `dpi_scale_factor()` override exists. ArkUI reports the
    ///   component's size in pixels, not a density, so this backend genuinely cannot answer.
    /// * `ime` — `ime_bridge()` is implemented over the shared marked-text state machine, and the
    ///   ArkTS host forwards composition/commit through `rw_harmony_ime_marked_text` /
    ///   `rw_harmony_ime_commit` (B28.12).
    ///
    /// `typed_widget_trigger` is `true` and stays `true`: unlike the others it is implemented
    /// by this backend (`inject_widget_trigger_event`, `poll_widget_trigger_event`) over the
    /// shared queue rather than by the host, so it cannot be absent.
    /// The capabilities this backend can honour.
    fn capabilities(&self) -> crate::platform::types::PlatformCapabilities {
        crate::platform::types::PlatformCapabilities {
            // The display manager answers `densityPixels` on a device target; on a host build
            // the stand-in reports no displays, so the flag follows the build fact (the same
            // rule the pasteboard flag below uses).
            dpi_scaling: cfg!(target_env = "ohos"),
            // ArkTS owns the soft keyboard and forwards composition/commit through
            // `rw_harmony_ime_marked_text`/`rw_harmony_ime_commit` (B28.12).
            ime: true,
            accessibility: cfg!(all(feature = "xcomponent", not(alloc_frugal))),
            native_menu: false,
            typed_widget_trigger: true,
            // `OH_Pasteboard` over UDMF, linked from the SDK's `libpasteboard`/`libudmf`. The
            // flag and the method bodies are produced from the **same** `cfg`
            // ([`super::pasteboard::PASTEBOARD_AVAILABLE`]), so a build that compiles the host
            // stand-in reports `false` while a build that links the real service reports `true`.
            clipboard: super::pasteboard::PASTEBOARD_AVAILABLE,
            // No handler spawn: OpenHarmony opens a URL through an `AbilityContext`, which this
            // backend has no handle on.
            open_external: false,
            // No picker: the ArkUI file dialog needs the same `AbilityContext` the handler above
            // lacks.
            file_dialog: false,
            // No `OH_Drawing` font-manager binding in this backend yet; an empty list is the
            // honest answer (B28.39 records the source to use when one is added).
            system_fonts: false,
        }
    }

    /// The ArkTS host's marked-text/commit bridge (B28.12).
    ///
    /// The platform-independent state machine is shared with the iOS/macOS bridges; the ArkTS
    /// host calls `rw_harmony_ime_marked_text` and `rw_harmony_ime_commit` from its input-method
    /// callbacks. `focus_in`/`focus_out` come from the widget runtime.
    fn ime_bridge(&self) -> Option<&dyn crate::platform::ime::ImeBridge> {
        Some(super::host_facts::ime_bridge())
    }

    /// The system colour mode, when the ArkTS host has reported one.
    ///
    /// The SDK exposes it as a callback (`OH_ArkUI_RegisterSystemColorModeChangeEvent`), not a
    /// synchronous query, so the host pushes it through `rw_harmony_set_theme`; `None` before that
    /// is the honest "not told yet" answer.
    fn system_theme(&self) -> Option<bool> {
        super::host_facts::theme()
    }

    /// The ArkTS `Configuration.fontSizeScale`, when the host has reported it.
    fn text_scale(&self) -> f32 {
        super::host_facts::font_scale()
    }

    /// The merged avoid-area insets, when the host has reported them.
    fn safe_area_insets(
        &self,
        _window_id: crate::core::ObjectId,
    ) -> Option<crate::platform::types::SafeAreaInsets> {
        super::host_facts::safe_area()
    }

    /// The environment revision, including the host-reported facts.
    fn environment_generation(&self) -> u64 {
        super::host_facts::generation()
    }

    /// The locale named by the process environment, when the host exposes one.
    ///
    /// OpenHarmony has no synchronous `NSLocale`-style API in this bridge; the POSIX variables
    /// are the only source available, and an unset environment answers `None`.
    fn system_locale(&self) -> Option<&'static str> {
        crate::platform::locale::interned_from_environment()
    }

    /// The displays the OpenHarmony display manager reports.
    ///
    /// A host build (the `harmony` preview feature on a desktop target) has no display manager,
    /// so the module's stand-in answers an empty list. On a device the list is the real one from
    /// `OH_NativeDisplayManager_CreateAllDisplays`; see [`super::displays`].
    fn displays(&self) -> Vec<Display> {
        super::displays::displays()
    }

    /// The display's logical scale, from `densityPixels`.
    ///
    /// `1.0` when no display is readable (a host build, or the service refused the query).
    /// [`Self::capabilities`] reports `dpi_scaling: false` on exactly the host build where the
    /// stand-in produces no reading, so the constant is not a fabricated claim.
    fn dpi_scale_factor(&self) -> f32 {
        let displays = self.displays();
        displays
            .iter()
            .find(|display| display.is_primary)
            .or_else(|| displays.first())
            .map(|display| display.scale)
            .unwrap_or(1.0)
    }

    /// The display a window this backend owns is placed on.
    ///
    /// This backend's windows are state records: ArkUI owns the XComponent and places it, so
    /// there is no per-window origin to compare against the display list. A window record is
    /// answered with the primary display — the one the host application is shown on — and any
    /// other id is answered `None`, so a control id cannot masquerade as a placed window.
    fn display_of_window(&self, window_id: ObjectId) -> Option<DisplayId> {
        // The id a host holds is the widget-registry id, while `state` is keyed by the
        // platform's own handle id; `host_window_for` is the association recorded at creation.
        // A caller that passes the platform id directly is honoured too.
        let host = if self.state.kind_of(window_id).is_some() {
            window_id
        } else {
            #[cfg(not(alloc_frugal))]
            {
                crate::widget::runtime::host_window_for(window_id).unwrap_or(window_id)
            }
            #[cfg(alloc_frugal)]
            {
                window_id
            }
        };
        if !matches!(self.state.kind_of(host), Some(HarmonyHandleKind::Window)) {
            return None;
        }
        let displays = self.displays();
        displays
            .iter()
            .find(|display| display.is_primary)
            .or_else(|| displays.first())
            .map(|display| display.id)
    }

    /// A revision of the display configuration; see [`super::displays::generation`].
    fn display_generation(&self) -> u64 {
        super::displays::generation()
    }

    /// The ArkUI accessibility bridge, when the XComponent bridge is compiled in.
    ///
    /// Returns the process-wide bridge, which posts notifications through the
    /// `ArkUI_AccessibilityProvider` of whatever component was last bound. It deliberately
    /// answers `Some` even before a component is bound: the bridge exists and can hold
    /// names, and its `notify_*` methods report — rather than hide — that they had no
    /// provider to post through. Answering `None` would make the accessibility flag and
    /// this method disagree, which is the inconsistency the capability contract forbids.
    #[cfg(all(feature = "xcomponent", not(alloc_frugal)))]
    fn accessibility_bridge(
        &self,
    ) -> Option<&'static dyn crate::platform::accessibility::AccessibilityBridge> {
        Some(super::accessibility::bridge())
    }
    fn init(&self) {
        self.runtime.initialized.store(true, Ordering::SeqCst);
    }
    fn run(&self) {
        if !self.runtime.initialized.load(Ordering::SeqCst) {
            self.init();
        }
        self.runtime.running.store(true, Ordering::SeqCst);
        // No native message pump: the host owns the window and reports changes through
        // `queue_resize_trigger`, so this loop is what drains them. Without the drain a
        // `Resized` event sat in the queue and no window layout re-ran. See
        // `crate::drain_triggers`.
        //
        // `crate::drive_frame` also advances the animation bus, which is the half this loop
        // was missing: draining alone re-ran layout and left every hover fade, caret blink
        // and toggle transition unreachable (BLUE24 §0A.1 measurement 1). One call per tick
        // gives both, in the order they must happen.
        while self.runtime.running.load(Ordering::SeqCst) {
            let outcome = crate::drive_frame(FRAME_INTERVAL_MS as u32);
            // Block when no frame is owed instead of waking every 16 ms. A posted event, a stop
            // request, or scheduled cross-thread work wakes the loop immediately.
            crate::platform::frame_wait::pause_between_frames(outcome.needs_another_frame);
        }
    }
    fn quit(&self) -> bool {
        // The loop this backend runs reads `self.runtime.running` every tick, so a stop
        // request from any thread lands and is honoured; `true` reports that fact rather
        // than merely that the call was made.
        self.runtime.running.store(false, Ordering::SeqCst);
        // The loop may be parked in `pause_between_frames`; wake it so the stop is immediate.
        crate::platform::frame_wait::wake();
        // Release the process-lifetime system handles this backend caches (G-605). A later use
        // recreates them, so a restarted loop is not left without a clipboard.
        super::pasteboard::release();
        true
    }

    /// Release the state record for `widget_id`.
    ///
    /// Harmony keeps no per-widget side table now that the native control
    /// constructors are gone: the shared list storage and the menu bookkeeping
    /// that used to need purging here no longer exist, so dropping the
    /// authoritative `BackendState` record is the whole teardown.
    fn destroy_widget(&self, widget_id: u64) -> bool {
        self.state.destroy_widget(widget_id)
    }
    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.state.count_kind(HarmonyHandleKind::Window) > 0 {
            log::warn!(
                "[harmony] create_window: this backend hosts one XComponent window; the second
                 request returns 0 (BLUE28 B28.43)"
            );
            return 0;
        }
        if let Some(host_id) = self.host_window_id() {
            log::debug!("[harmony] create_window: host-bound ArkTS window id {host_id}");
        }
        self.insert_widget(HarmonyHandleKind::Window, title, x, y, width, height)
    }

    /// OpenHarmony's host model is one XComponent with one window.
    fn supports_multiple_windows(&self) -> bool {
        false
    }
    // ─── Clipboard ───
    //
    // The system pasteboard when this build links it (OpenHarmony targets), the in-process
    // `BackendState` record otherwise. The `Option` bindings below are the mechanism: the host
    // path answers `None` exactly when it is not compiled for this target, so a host build keeps
    // the record's behaviour instead of silently reporting failed writes.

    fn set_clipboard_text(&self, text: &str) -> bool {
        match super::pasteboard::set_text(text) {
            Some(written) => written,
            None => self.state.set_clipboard_text(text),
        }
    }

    fn get_clipboard_text(&self) -> String {
        super::pasteboard::text().unwrap_or_else(|| self.state.clipboard_text())
    }

    /// The pasteboard's text, or `None` when it holds none.
    ///
    /// Overridden because the trait default derives it from [`Self::get_clipboard_text`], and on
    /// the host path an empty string means "the clipboard holds an empty text" — a real answer
    /// that the default would report as "no text".
    fn clipboard_text(&self) -> Option<String> {
        match super::pasteboard::text() {
            Some(text) => Some(text),
            None => {
                if super::pasteboard::PASTEBOARD_AVAILABLE {
                    // The pasteboard answered, and it holds no text: that is the answer, and it
                    // is not the same fact as the record below being empty.
                    return None;
                }
                let text = self.state.clipboard_text();
                if text.is_empty() {
                    None
                } else {
                    Some(text)
                }
            }
        }
    }

    /// The host clipboard, or `None` when this build has no OpenHarmony pasteboard.
    fn clipboard_backend(&self) -> Option<&dyn crate::platform::clipboard::RichClipboardBackend> {
        if !super::pasteboard::PASTEBOARD_AVAILABLE {
            return None;
        }
        static CLIPBOARD: std::sync::OnceLock<super::pasteboard::HarmonyClipboard> =
            std::sync::OnceLock::new();
        Some(CLIPBOARD.get_or_init(super::pasteboard::HarmonyClipboard::new))
    }

    /// Pops the next typed widget-trigger event injected into this backend.
    ///
    /// The queue lives in `BackendState` and is shared by every state-only
    /// backend (iOS/Android/mobile/stub). Harmony delegates to it like the rest;
    /// leaving this to the trait default made the pair below silently
    /// asymmetric — `inject_widget_trigger_event` returned `false` and this
    /// always returned `None`, so no injected trigger could ever be observed.
    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        self.state.pop_widget_trigger_event()
    }

    /// Pops the next pending trigger as a bare [`ObjectId`].
    ///
    /// # Why this had to be added, and why the comment above was only half true
    ///
    /// The doc on [`Self::poll_widget_trigger_event`] says the trait default made
    /// "the pair below" asymmetric, and then restored only one half. There are
    /// **two** FIFO views over the same queue — the `ObjectId` one here and the
    /// typed one above — and the trait documents them as the same event stream seen
    /// twice:
    ///
    /// > Returns the next pending widget activation.
    /// > Returns the next pending typed widget activation. Default: none are produced.
    ///
    /// Every sibling state backend overrides both (`AndroidPlatform`,
    /// `IosMobilePlatform`, `AndroidMobilePlatform`, `StubPlatform`), and
    /// `platform::tests::consistency_compat_poll_widget_triggered_is_single_delivery_shim`
    /// pins the two views as agreeing. Harmony overrode only the typed one, so
    /// `poll_widget_triggered()` returned `None` while
    /// `poll_widget_trigger_event()` returned an event — the *same* widget, observably
    /// absent through one door and present through the other.
    ///
    /// It is derived from the typed event rather than draining the queue again, so
    /// the two remain one stream: a caller that alternates between the views advances
    /// one FIFO, which is what the consistency test asserts about the stub.
    fn poll_widget_triggered(&self) -> Option<ObjectId> {
        self.poll_widget_trigger_event().map(|event| event.widget_id)
    }

    /// Injects a typed widget-trigger event, refusing ids the backend never made.
    fn inject_widget_trigger_event(&self, widget_id: u64, kind: WidgetTriggerKind) -> bool {
        self.state.inject_widget_trigger_event(widget_id, kind)
    }

    /// Mounts a library-painted widget onto a surface this host will present.
    ///
    /// The backend keeps no native object per control (every `WidgetKind` is painted
    /// by `src/widget/`), so the surface is a record plus a repaint queue: the ArkTS
    /// side owns the pixels and pulls them with the render API, and this tells it
    /// which widgets exist and when they went stale.
    ///
    /// Before this the backend inherited the trait default and reported `false`, so a
    /// host that asked could not display anything even though nothing was missing but
    /// the wiring.
    ///
    /// # Why the XComponent bridge is told as well
    ///
    /// When `feature = "xcomponent"` is on there are **two** halves to this call, and only
    /// one of them used to happen. Recording the mount makes the widget *visible* — the
    /// repaint queue is how the ArkTS side learns a frame is ready. It does not make the
    /// widget *interactive*: every input callback in `harmony::xcomponent` (touch, mouse,
    /// hover, key, focus, blur) begins with `mounted_widget()`, and that is the bridge's
    /// own record, written only by `xcomponent::set_mounted_widget`.
    ///
    /// Nothing called it. The bridge therefore bound its callbacks successfully, reported
    /// success, and then dropped every event on the first line — a widget that painted and
    /// never reacted, with no error at any log level. This is the mount that closes it.
    ///
    /// The bridge is best-effort here rather than a hard failure: a host that mounts
    /// before the ArkTS `onLoad` has run has no XComponent yet, and
    /// `set_mounted_widget` answers `false` for exactly that state. The surface record is
    /// still written, because the widget really is displayed and the host can rebind later
    /// — refusing the whole mount would break the presentation path over an input path the
    /// host may not have wanted yet.
    fn mount_surface(&self, _parent: u64, id: u64, rect: crate::core::Rect) -> bool {
        let recorded = self.state.mount_surface_record(id, rect);
        #[cfg(all(feature = "xcomponent", not(alloc_frugal)))]
        {
            if !super::xcomponent::set_mounted_widget(id, rect) {
                log::debug!(
                    "[harmony] mount_surface: widget {id} is displayed but no XComponent is \
                     bound yet, so input will not reach it until the ArkTS onLoad calls \
                     rw_harmony_bind_xcomponent"
                );
            }
        }
        recorded
    }

    /// Updates the rect of a mounted surface. `false` when `id` is not mounted.
    fn resize_surface(&self, id: u64, rect: crate::core::Rect) -> bool {
        let recorded = self.state.resize_surface_record(id, rect);
        #[cfg(all(feature = "xcomponent", not(alloc_frugal)))]
        {
            // The bridge's rect is the input hit-test space, so a surface that moved
            // without telling it would route touches to the old geometry.
            if recorded {
                super::xcomponent::set_mounted_widget(id, rect);
            }
        }
        recorded
    }

    /// Releases a mounted surface.
    fn unmount_surface(&self, id: u64) -> bool {
        let recorded = self.state.unmount_surface_record(id);
        #[cfg(all(feature = "xcomponent", not(alloc_frugal)))]
        {
            // Only clear when the bridge is actually displaying *this* widget: unmounting
            // some other surface must not silently stop input for the one on screen.
            if recorded && super::xcomponent::mounted_widget() == Some(id) {
                super::xcomponent::clear_mounted_widget();
            }
        }
        recorded
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
    /// # Who calls this on Harmony
    ///
    /// The **host ArkTS page**, not this backend. OpenHarmony delivers a size change to
    /// the host's `onAreaChange` callback on the component it created the surface in; the
    /// library holds no ArkUI component handle to subscribe with (this backend is a state
    /// model — see `create_window`), so there is no callback for it to attach.
    ///
    /// A host reports the new size here (or through [`crate::queue_resize_trigger`], the
    /// same call). One that does not keeps the created size, which is what
    /// `window_client_size` falls back to.
    fn queue_resize_trigger(&self, window_id: u64, width: u32, height: u32) -> bool {
        // Forward to the control backend, which owns the window and the queue the app
        // polls. Writing to the platform's own state would land in a store the host
        // never reads, because `create_window` goes through the control backend.
        crate::queue_resize_trigger(window_id, width, height)
    }

    /// Queues a repaint for the host to pick up. `false` when `id` is unknown.
    ///
    /// # Why a window id is accepted
    ///
    /// The library asks for the **window** to be repainted whenever one of its
    /// ordinary children changes (`widget::runtime::request_repaint_subtree`),
    /// because a window is what draws those children. A surface-only record
    /// answered `false` for such a request, so an event could be handled and the
    /// screen still never change — silently. This backend knows every widget it
    /// created, so the honest test is membership, not "is it a mounted surface".
    fn invalidate_surface(&self, id: u64) -> bool {
        self.state.record_repaint_request(id)
    }

    /// Removes and returns the next widget awaiting a repaint.
    ///
    /// The drain half of [`Self::invalidate_surface`]: without it the queue the host
    /// fills through `rw_take_pending_repaint` would always answer `None`/`0`, so a
    /// stale widget could never be asked to redraw.
    fn take_pending_repaint(&self) -> Option<ObjectId> {
        self.state.take_pending_repaint()
    }

    /// The backend displays library-painted widgets by handing the host their frames.
    ///
    /// # What this promises, and what it does not
    ///
    /// `true` advertises [`mount_surface`](crate::platform::Platform::mount_surface):
    /// a mounted widget gets a surface the host presents. It is **not** a claim
    /// that an arbitrary window's frame draws its children — a host that needs
    /// that asks
    /// [`invalidate_surface`](crate::platform::Platform::invalidate_surface),
    /// which on this backend queues a repaint for any widget it knows, window
    /// included.
    fn supports_surfaces(&self) -> bool {
        true
    }

    /// This backend is a mobile target, so the mobile runtime helpers configure this instance
    /// rather than a separate preview singleton (BLUE28 B28.24).
    #[cfg(feature = "mobile-api")]
    fn mobile_extension(&self) -> Option<&dyn crate::platform::MobilePlatformExtension> {
        Some(self)
    }
}

/// The ArkTS window id the host bound through `rw_harmony_set_window_id`, when any.
///
/// An **inherent** method, not a trait item: `Platform` does not declare it, and a
/// `pub(crate) fn` inside the trait `impl` block is rejected by the compiler (`E0449`). The
/// FFI setter and this reader are the two halves of one contract, and the reader is what
/// makes the written id observable to the backend's diagnostics instead of write-only state.
impl HarmonyPlatform {
    /// The ArkTS window id the host bound through `rw_harmony_set_window_id`, when any.
    pub(crate) fn host_window_id(&self) -> Option<i32> {
        super::host_facts::window_id()
    }
}

/// HarmonyOS' lifecycle/orientation contract (BLUE28 B28.24).
///
/// The orientation comes from `rw_harmony_set_orientation`; the ArkTS host reads
/// `window.getWindowProperties().orientation` in its `onConfigurationUpdate` callback. Until it
/// reports one, [`orientation`](crate::platform::MobilePlatformExtension::orientation) answers
/// `None` rather than a guessed portrait.
impl crate::platform::MobilePlatformExtension for HarmonyPlatform {
    fn mobile_backend(&self) -> crate::platform::MobileBackend {
        crate::platform::MobileBackend::HarmonyMobile
    }

    fn attach_to_native_view(&self, native_handle: usize) -> bool {
        // This backend is a state model: it holds no ArkUI component, so accepting a handle
        // would promise an attachment that does not exist. The `xcomponent` bridge owns the real
        // surface and exposes its own attach path.
        let _ = native_handle;
        false
    }

    fn orientation(&self) -> Option<crate::platform::ScreenOrientation> {
        crate::platform::lifecycle::orientation()
    }

    fn orientation_generation(&self) -> u64 {
        crate::platform::lifecycle::rotation_generation()
    }

    fn lifecycle_state(&self) -> Option<crate::platform::MobileLifecycleState> {
        crate::platform::lifecycle::lifecycle_state()
    }

    fn suspend(&self) -> bool {
        crate::platform::lifecycle::set_state(crate::platform::MobileLifecycleState::Background)
    }

    fn resume(&self) -> bool {
        crate::platform::lifecycle::set_state(crate::platform::MobileLifecycleState::Active)
    }
}
