// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Platform abstraction types and capability contracts.

use crate::compat::{format, Box, String, Vec};
use crate::core::{ObjectId, Orientation, PlatformFamily};
#[cfg(all(feature = "serde", widgets_unstripped))]
use serde::{Deserialize, Serialize};

/// Whether the CUPS print clients (`lp` or `lpr`) are installed.
///
/// Shared by the Unix-oriented backends (macOS, Linux, Wayland) so the detection
/// rule lives in exactly one place. iOS/Android/Harmony do not use it — they print
/// through platform frameworks rather than a spooler command — and the WASM
/// backend has no process to spawn at all.
///
/// The test is whether the binary can be **spawned**, not whether it exits zero:
/// CUPS `lp` rejects `--version` with exit status 1 while still printing its
/// usage text, so an exit-code check would wrongly report "no spooler" on a
/// machine that has one. Only a spawn failure (`ErrorKind::NotFound`) means the
/// binary is absent.
///
/// `allow(dead_code)` rather than a `cfg` gate: the callers live in
/// `platform/{macos,macos_objc2,linux,wayland}`, which are individually
/// `cfg`-gated. Enumerating those same conditions here would duplicate four
/// separate gate expressions — exactly the drift that caused the `full_widgets`
/// bug — so the helper stays available to all targets and simply goes unused on
/// the rest.
#[allow(dead_code)]
pub(crate) fn unix_print_clients_available() -> bool {
    ["lp", "lpr"].iter().any(|cmd| {
        // Any `ExitStatus` at all proves the executable exists and ran.
        std::process::Command::new(cmd).arg("--help").output().is_ok()
    })
}

/// The shortcut notation matching the OS this crate is compiled for.
///
/// Apple platforms use AppKit glyphs (`⌘⇧Z`); everything else uses the spelled-out
/// `Ctrl+Shift+Z` convention. This is the one place the compile target decides the
/// notation, keeping the `cfg!` out of `src/shortcut/` (principle #36). Backends
/// may override it through `Platform::shortcut_style`.
pub const fn compile_target_shortcut_style() -> crate::shortcut::PlatformShortcutStyle {
    if cfg!(any(target_os = "macos", target_os = "ios")) {
        crate::shortcut::PlatformShortcutStyle::Mac
    } else {
        crate::shortcut::PlatformShortcutStyle::Desktop
    }
}

/// A backend-owned native web engine view.
///
/// Widgets in `src/web/` drive a real browser engine through this trait without
/// naming any platform library. The concrete type (a `webkit2gtk::WebView` on the
/// Linux GTK backend) is constructed by the backend and never appears in the
/// widget layer — see principle #36.
///
/// All methods report failure through enums or `Result` rather than panicking:
/// a headless CI host has no display, and that must surface as "no engine" so the
/// caller can fall back to the simulated path.
pub trait NativeWebEngine: Send {
    /// Begins loading `url`.
    fn load_url(&mut self, url: &str) -> Result<(), String>;
    /// Begins loading `html`, optionally resolving relative references against
    /// `base_url`.
    fn load_html(&mut self, html: &str, base_url: Option<&str>) -> Result<(), String>;
    /// Navigates back in the session history.
    fn go_back(&mut self);
    /// Navigates forward in the session history.
    fn go_forward(&mut self);
    /// Reloads the current document.
    fn reload(&mut self);
    /// Cancels an in-flight page load.
    fn stop_loading(&mut self);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(all(feature = "serde", widgets_unstripped), derive(Serialize, Deserialize))]
/// Normalized classification of a widget trigger.
///
/// The platform reports the same semantic answer on every host, so consuming
/// code does not branch on OS. The explicit discriminants are part of the
/// published representation: `Unknown` is `0`, so an unset or zero-initialised
/// value degrades to "unknown" rather than to a real trigger kind.
pub enum WidgetTriggerKind {
    /// No concrete trigger semantic is known.
    Unknown = 0,
    /// Primary activation action (button click, checkbox toggle, etc.).
    Clicked = 1,
    /// Stateful value changed (line edit text, slider value, etc.).
    ValueChanged = 2,
    /// Selection changed (combo/list/tree/table current selection updates).
    SelectionChanged = 3,
    /// Widget/window closed lifecycle trigger.
    Closed = 4,
    /// A container's own size changed.
    ///
    /// Reported when the host resizes a window (the user dragging its edge, or the
    /// window manager tiling it). It is distinct from the widget-level triggers above:
    /// nothing the user *touched* changed, but every child geometry is now stale.
    ///
    /// A host that offers layout managers must re-run them when this arrives, or the
    /// controls keep the geometry they were given for the previous size.
    Resized = 5,
}
/// Typed widget trigger event with source widget id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(all(feature = "serde", widgets_unstripped), derive(Serialize, Deserialize))]
pub struct WidgetTriggerEvent {
    /// Logical widget id generated by the platform abstraction.
    pub widget_id: ObjectId,
    /// Normalized trigger kind for cross-platform consumption.
    pub kind: WidgetTriggerKind,
}
/// Drag-and-drop payload event.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(all(feature = "serde", widgets_unstripped), derive(Serialize, Deserialize))]
pub struct DropEvent {
    /// Logical id of drag source widget.
    pub source_widget_id: ObjectId,
    /// Logical id of drop target widget.
    pub target_widget_id: ObjectId,
    /// MIME type describing payload bytes.
    pub mime: String,
    /// Opaque drag payload bytes.
    pub payload: Vec<u8>,
}

/// Controls how text is displayed in a text-entry control.
///
/// Defined here rather than in `app` because a backend must name it when it
/// implements [`Platform::set_widget_echo_mode`], and `platform` must not depend
/// on `app` (that would invert the layering, principle #3). `app` re-exports it
/// so callers keep their existing import path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(all(feature = "serde", widgets_unstripped), derive(Serialize, Deserialize))]
pub enum EchoMode {
    /// Display characters as-is.
    Normal,
    /// Mask every character (e.g. for passwords).
    Password,
    /// Do not echo characters at all.
    NoEcho,
}

/// A togglable window state that a native backend can apply and read back.
///
/// These are modelled as an enum rather than a handful of `set_window_*`
/// booleans so a caller can express "toggle the state the user asked for" in one
/// call, and so a backend dispatches with a `match` instead of a chain of
/// `if`/`else` — the Rust-idiomatic form of C's `void*` + type tag (principle
/// #31).
///
/// Not every state is meaningful on every toolkit, which is expected: the
/// backend reports `false` from the setter when it cannot honour one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(all(feature = "serde", widgets_unstripped), derive(Serialize, Deserialize))]
pub enum WindowStateFlag {
    /// Maximised (zoomed) rather than restored.
    Maximized,
    /// Minimised (iconified).
    Minimized,
    /// Full-screen rather than windowed.
    Fullscreen,
    /// User can resize the window by dragging its edges.
    Resizable,
    /// Window has a title bar / borders drawn by the OS.
    Decorated,
}
/// Supported desktop backend families.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopBackend {
    /// Windows Win32 backend.
    Win32,
    /// Apple Cocoa backend.
    Cocoa,
    /// Linux GTK backend.
    Gtk,
    /// Harmony desktop backend.
    HarmonyDesktop,
}
/// Supported mobile backend families.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MobileBackend {
    /// Android mobile backend.
    Android,
    /// iOS mobile backend.
    Ios,
    /// Harmony mobile backend.
    HarmonyMobile,
}
/// Cross-platform runtime capability flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformCapabilities {
    /// High-DPI scale-aware geometry and text support.
    pub dpi_scaling: bool,
    /// Input method editor integration capability.
    pub ime: bool,
    /// Native accessibility bridge capability.
    pub accessibility: bool,
    /// Native menu creation and trigger support.
    pub native_menu: bool,
    /// Typed widget trigger event support.
    pub typed_widget_trigger: bool,
}
/// Native-runtime capability contract used by desktop-oriented negotiation.
///
/// This is the same five flags as [`PlatformCapabilities`], and deliberately so:
/// the negotiation result and the backend's own report must not be able to
/// disagree. It was previously a separate struct with a field-for-field
/// `from_platform_caps` copy, which is exactly how the two drift — a new
/// capability would be added to one and silently dropped by the other.
///
/// The name is kept because it is the vocabulary of the negotiation API
/// (`CapabilityContract::Native(..)`, `Platform::native_capability_contract`),
/// and renaming it would be a needless break for callers. Because it is an alias,
/// the two types are interchangeable and no conversion is possible to get wrong.
pub type NativeCapabilityContract = PlatformCapabilities;
/// Embedded-runtime capability contract used by constrained-profile negotiation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmbeddedCapabilityContract {
    /// Whether the runtime assumes a fixed DPI scale factor.
    pub fixed_dpi: bool,
    /// Whether low-memory behavior is expected by default.
    pub low_memory_mode: bool,
    /// Whether typed widget triggers are supported.
    pub typed_widget_trigger: bool,
}
/// Runtime capability negotiation result split by profile contract.
///
/// A backend answers with exactly one of these, matching the family it was
/// built for: see [`Platform::native_capability_contract`] and
/// [`Platform::embedded_capability_contract`]. The variant tells the caller
/// which half of the capability model applies, so it must not have to guess
/// from the widget set.
pub enum CapabilityContract {
    /// Desktop/hosted backend, described by the full capability set.
    Native(NativeCapabilityContract),
    /// Constrained backend, described by the reduced embedded set.
    Embedded(EmbeddedCapabilityContract),
}
/// The capabilities a backend with the given `family` reports when it does **not**
/// override [`Platform::capabilities`].
///
/// Exposed so the documented OS capability matrix can be checked against source
/// rather than trusted. A backend cannot be instantiated off its own host, but the
/// *trait default* is a pure function of the family, and that is the half of the
/// answer a reader of the matrix is least able to verify.
///
/// Pair it with the backend's own `fn capabilities` when one exists.
pub fn default_capabilities_for(family: PlatformFamily) -> PlatformCapabilities {
    let desktop = matches!(family, PlatformFamily::Desktop);
    PlatformCapabilities {
        dpi_scaling: desktop,
        ime: desktop,
        accessibility: desktop,
        native_menu: desktop,
        typed_widget_trigger: true,
    }
}

/// Platform backend contract used by widget/runtime layers.
///
/// Core lifecycle methods are required. Optional capabilities (IME,
/// accessibility, drag-and-drop) have default no-op implementations
/// on the Platform trait itself — backends override them when supported.
pub trait Platform: Send + Sync {
    /// Returns self as a `&dyn Any` for downcasting.
    fn as_any(&self) -> &dyn core::any::Any {
        // Safe fallback — unsized coercion on a unit type avoids panicking.
        // Concrete backends should override with a self-referencing impl.
        static UNIT: () = ();
        &UNIT as &dyn core::any::Any
    }
    /// Returns the mobile backend extension when this platform drives a mobile
    /// native view.
    ///
    /// Mobile runtimes (Android/iOS) override this so the `mobile-*` runtime
    /// helpers configure the *same* platform instance that `get_platform()`
    /// returns, instead of a separate preview singleton. Desktop/stub backends
    /// keep the default `None`.
    #[cfg(feature = "mobile-api")]
    fn mobile_extension(&self) -> Option<&dyn MobilePlatformExtension> {
        None
    }
    /// Returns backend identifier string.
    fn backend_name(&self) -> &'static str;
    /// Returns platform family classification.
    fn family(&self) -> PlatformFamily;
    /// Runtime capabilities exposed by the current backend.
    ///
    /// # What the default answers, and why the family decides it
    ///
    /// A backend that does not override this reports `true` for `dpi_scaling`, `ime`,
    /// `accessibility` and `native_menu` **iff it reports the `Desktop` family**.
    /// Those four are host integrations, and a backend that classifies itself as a
    /// desktop is asserting the host has them. `typed_widget_trigger` is always
    /// `true` because it is implemented by the library, not the host.
    ///
    /// The practical consequence: **overriding matters for the desktop family.**
    /// Wayland, HarmonyOS and Android-style backends classify as `Desktop` (or a
    /// mobile analogue) yet do not honour a native menu, so each must override and
    /// say so. A backend that forgets inherits `true`, which is a silent
    /// over-claim — see [`default_capabilities_for`], which spells out what an
    /// untouched backend would report, and the capability-matrix gate, which
    /// compares that against what each backend actually claims.
    fn capabilities(&self) -> PlatformCapabilities {
        let desktop = matches!(self.family(), PlatformFamily::Desktop);
        PlatformCapabilities {
            dpi_scaling: desktop,
            ime: desktop,
            accessibility: desktop,
            native_menu: desktop,
            typed_widget_trigger: true,
        }
    }
    /// Native capability contract published by desktop-capable runtimes.
    fn native_capability_contract(&self) -> Option<NativeCapabilityContract> {
        if matches!(self.family(), PlatformFamily::Desktop) {
            // The two types are one alias, so the backend's own report *is* the
            // contract — there is nothing to convert and nothing to keep in sync.
            Some(self.capabilities())
        } else {
            None
        }
    }
    /// Embedded capability contract published by constrained runtimes.
    fn embedded_capability_contract(&self) -> Option<EmbeddedCapabilityContract> {
        if matches!(self.family(), PlatformFamily::Embedded) {
            Some(EmbeddedCapabilityContract {
                fixed_dpi: self.dpi_scale_factor() == 1.0,
                low_memory_mode: true,
                typed_widget_trigger: self.capabilities().typed_widget_trigger,
            })
        } else {
            None
        }
    }
    /// Logical DPI scale factor for coordinate transforms.
    fn dpi_scale_factor(&self) -> f32 {
        1.0
    }
    /// Initialises the backend, acquiring whatever host resources it needs.
    ///
    /// Called once before any widget is created. A backend that fails here has
    /// no way to report it through this signature, so implementations should
    /// make failures observable (log, or expose a queryable status) rather than
    /// silently degrading.
    fn init(&self);
    /// Runs the backend's event loop until [`Platform::quit`] or the host closes
    /// the last window.
    ///
    /// This blocks; it is the top of the stack for a host that lets the platform
    /// own the main loop. Backends whose loop is externally driven may treat it
    /// as returning immediately.
    fn run(&self);
    /// Requests that the event loop started by [`Platform::run`] terminate.
    ///
    /// A request, not a synchronous stop: the loop exits at its next
    /// opportunity, so state may still be delivered after this returns.
    fn quit(&self);

    /// Destroy a widget and release the resources associated with it.
    ///
    /// Returns `true` when the widget existed and was torn down.
    ///
    /// Implementations must release **everything** they registered for the
    /// widget: the backend state record, any native-object registry entry (and
    /// where the platform requires it, the underlying OS object itself), and any
    /// per-widget side tables (list/combo box items, selections, menu ownership).
    ///
    /// # Long-running applications
    ///
    /// Before this method existed, a backend's registries could only grow: a UI
    /// that is rebuilt (create/discard cycles) leaked one state record plus one
    /// native handle per discarded widget, without bound. A 8000-widget churn
    /// loop measured +49 MB RSS on the macOS objc2 backend.
    ///
    /// The default implementation returns `false` so that third-party backends
    /// written against an earlier version keep compiling; every in-tree backend
    /// overrides it.
    fn destroy_widget(&self, _widget_id: ObjectId) -> bool {
        false
    }

    /// Mounts a widget onto a surface supplied by this backend.
    ///
    /// # Why this is one method and not one per widget kind
    ///
    /// Every widget is painted by the library (BLUE15 rule #55), so there is no
    /// per-kind OS control for a `create_*` method to map onto. What the backend
    /// owes a widget is exactly one thing: a surface that calls back into the
    /// process-wide widget registry whenever the host wants a repaint.
    /// `mount_surface` is that surface.
    ///
    /// The caller registers the widget first
    /// (`widget::runtime::register`) and passes the resulting id here
    /// together with the parent window and the desired rectangle. The backend
    /// must not take ownership of the widget: it lives in the registry for as
    /// long as the mount exists.
    ///
    /// # Callers never name the mechanism
    ///
    /// The surface is whatever this backend uses — a child window, a drawing
    /// area, a view — and that is an implementation detail of `src/platform/`.
    /// Upper layers ask only "can you display this widget?"
    /// ([`Platform::supports_surfaces`]) and otherwise stay free of
    /// per-OS knowledge.
    ///
    /// # Return value
    ///
    /// `true` when a surface was created and will be repainted from the
    /// registry. `false` when this backend cannot display library-painted content —
    /// the default below. Callers must treat `false` as "cannot display here"
    /// and say so, rather than showing an empty window.
    fn mount_surface(&self, _parent: ObjectId, _id: ObjectId, _rect: crate::core::Rect) -> bool {
        false
    }

    /// Updates the rectangle of a previously mounted surface.
    ///
    /// Returns `false` when `id` is not mounted on this backend.
    fn resize_surface(&self, _id: ObjectId, _rect: crate::core::Rect) -> bool {
        false
    }

    /// Unmounts a surface and releases it.
    ///
    /// The widget stays in the process-wide registry; the caller decides when to
    /// drop it via `widget::runtime::unregister`.
    ///
    /// Returns `false` when `id` is not mounted on this backend.
    fn unmount_surface(&self, _id: ObjectId) -> bool {
        false
    }

    /// Reads back the current size of a window's usable client area.
    ///
    /// # Why this exists
    ///
    /// When the host resizes a window, the window's own geometry in this library's
    /// mirror is not updated — only the OS knows the new size. A caller that keeps a
    /// layout (`app::WindowHandle::set_layout`) therefore needs a way to ask "how big
    /// is this window now?" without knowing which toolkit is underneath.
    ///
    /// # Return value
    ///
    /// `Some((width, height))` in logical pixels when the backend knows the window and
    /// can report its client area; `None` when it does not — either the id addresses
    /// nothing, or this backend has no window to ask. The default is `None`, which is
    /// the honest answer rather than a fabricated size: a caller that receives it keeps
    /// the geometry it already had instead of laying out against a guess.
    fn window_client_size(&self, _window_id: ObjectId) -> Option<(u32, u32)> {
        None
    }

    /// Reports that a container's client area became `width` by `height`.
    ///
    /// # Who calls this
    ///
    /// Backends call it from their own resize callback (`configure-event` on GTK,
    /// `WM_SIZE` on Win32, `windowDidResize:` on AppKit), and a host that owns the event
    /// loop may call it for a resize it learns about by other means.
    ///
    /// The call has two effects: the size becomes answerable through
    /// [`Self::window_client_size`], and a [`WidgetTriggerKind::Resized`] event is queued
    /// so the app layer re-runs the window's layout. "Something resized" and "to what"
    /// travel separately on purpose — the event is a queue entry that gets consumed, the
    /// size is state that survives it.
    ///
    /// Returns `false` for an id this backend does not know, in which case neither is
    /// recorded and nothing is queued.
    ///
    /// [`WidgetTriggerKind::Resized`]: crate::platform::WidgetTriggerKind::Resized
    fn queue_resize_trigger(&self, _window_id: ObjectId, _width: u32, _height: u32) -> bool {
        false
    }

    /// Marks a mounted surface as needing a repaint.
    ///
    /// Returns `false` when `id` is not mounted on this backend. Backends that
    /// do not implement it keep the default so unmounted ids stay a no-op.
    fn invalidate_surface(&self, _id: ObjectId) -> bool {
        false
    }

    /// Marks a *rectangle* of a mounted surface as needing a repaint.
    ///
    /// # Why this is a separate method rather than a parameter
    ///
    /// `invalidate_surface` is implemented by every backend and is the one path every
    /// host goes through; changing its signature would touch ten backends for a
    /// capability most of them cannot express. A window toolkit that only offers
    /// "this control is dirty" (no sub-rectangle) is not broken by this method
    /// existing — it simply does not override it.
    ///
    /// # Return value
    ///
    /// `true` when the backend narrowed the repaint to `rect`.
    ///
    /// `false` — **including the default** — means "I could not narrow it", and the
    /// caller must then fall back to [`Self::invalidate_surface`], which repaints the
    /// whole control. Returning `false` rather than repainting inside this method is
    /// deliberate: a caller that ignored the answer would still get a correct frame,
    /// because the fallback is the call it already had. A backend that silently did
    /// nothing here would produce a stale surface instead, which is a much worse bug
    /// than an unused optimisation.
    fn invalidate_surface_rect(&self, _id: ObjectId, _rect: crate::core::Rect) -> bool {
        false
    }

    /// Returns `true` when this backend can host library-painted widgets.
    ///
    /// Backends report `true` only once [`Platform::mount_surface`] is
    /// actually implemented, so hosts can ask before building a UI that they
    /// would not be able to display.
    fn supports_surfaces(&self) -> bool {
        false
    }

    /// Routes a pointer event that arrived at the surface `root` to the widget
    /// actually under `point`.
    ///
    /// # Why the backend owns this
    ///
    /// A mounted surface corresponds to one widget, but the user may click any
    /// widget nested inside it. Only the backend knows where its surface sits in the
    /// window, so only the backend can turn a surface-local position into the
    /// coordinate space the widget tree uses (which, in this library, is absolute —
    /// see `widget::runtime::widget_at`).
    ///
    /// The default implementation resolves the point against the widget tree rooted
    /// at `root` and delivers there, which is correct for any backend whose surface
    /// hosts a tree of widgets. A backend that already routes input itself (because
    /// its toolkit delivers per-child events) overrides this.
    ///
    /// # Naming
    ///
    /// The method describes the *intent* ("route this pointer event"), not the
    /// mechanism, so callers stay free of per-OS knowledge (BLUE15 rules #35/#52).
    ///
    /// Returns whether a widget accepted the event.
    ///
    /// Under the alloc-frugal `mini` profile there is no widget registry (see
    /// `src/widget/mod.rs`), so no widget can be routed to and the honest answer is
    /// `false` — the same "this host cannot do it" reply every other optional method
    /// in this trait gives, rather than a silent no-op that looks like success.
    fn route_pointer_event(
        &self,
        root: ObjectId,
        event: &crate::event::Event,
        point: crate::core::Point,
    ) -> bool {
        #[cfg(alloc_frugal)]
        {
            let _ = (root, event, point);
            false
        }
        #[cfg(not(alloc_frugal))]
        {
            crate::widget::runtime::dispatch_pointer_event(root, event, point)
        }
    }

    /// Renders a shortcut in the notation this operating system uses in menus.
    ///
    /// macOS returns `⌘⇧Z`; Windows and Linux return `Ctrl+Shift+Z`. Callers use
    /// this for menu labels and tooltips so a single shortcut table reads
    /// natively everywhere, with no `cfg` in application code.
    ///
    /// This covers *display* only. Whether the accelerator is actually wired up
    /// to fire is [`Platform::menu_add_item`]'s responsibility.
    fn format_shortcut(&self, shortcut: &crate::shortcut::Shortcut) -> String {
        crate::shortcut::format_shortcut_for_platform(shortcut, self.shortcut_style())
    }

    /// How this backend's operating system spells shortcut glyphs.
    ///
    /// Lets the notation follow the *backend* rather than a compile-time `cfg` in
    /// the shortcut layer: the macOS backends answer `Mac` (`⌘⇧Z`) and every other
    /// backend answers `Desktop` (`Ctrl+Shift+Z`). The default derives from the
    /// build target so backends with no opinion — including out-of-tree ones — stay
    /// correct.
    ///
    /// See principle #35: menu notation is presented through a runtime query, not a
    /// `cfg!(target_os)` check in a middle layer.
    fn shortcut_style(&self) -> crate::shortcut::PlatformShortcutStyle {
        crate::shortcut::PlatformShortcutStyle::current()
    }

    /// Total physical memory installed on this machine, in mebibytes.
    ///
    /// Upper layers (menu/GPU adaptivity) need the *machine* memory to size
    /// their caches, and that figure is only obtainable through the host OS:
    /// `/proc/meminfo` on Linux, `sysctl hw.memsize` on macOS,
    /// `GlobalMemoryStatusEx` on Windows. Probing it in a middle layer would
    /// violate the platform-isolation rule (principle #36), so every backend
    /// answers here instead.
    ///
    /// Returns `None` when this backend cannot determine the value. Callers
    /// must treat `None` as "unknown" and degrade honestly — never substitute a
    /// made-up constant such as `4096`.
    fn total_memory_mb(&self) -> Option<u64> {
        None
    }

    /// Whether the machine is currently drawing power from its battery.
    ///
    /// Desktop towers and servers report `false`. Backends on hardware without
    /// a battery also report `false` — the question is "is a battery draining",
    /// not "does a battery exist". Platforms that cannot tell return `false`,
    /// which selects the *non*-throttled defaults; that is the safe direction,
    /// because a wrong "on battery" would silently strip animations.
    fn is_on_battery(&self) -> bool {
        false
    }

    /// This process's resident memory as a fraction of its reserved address
    /// space, clamped to `[0.0, 1.0]`.
    ///
    /// Used by the adaptive-quality monitor to decide when to shed caches. The
    /// figure comes from OS process accounting — `/proc/self/status`
    /// (`VmRSS`/`VmSize`) on Linux, `proc_pidinfo`/`ps` on macOS,
    /// `GetProcessMemoryInfo` on Windows — so it is answered by the backend and
    /// never probed from a middle layer (principle #36).
    ///
    /// Returns `None` when this backend has no process-memory source; callers
    /// treat that as "unknown" rather than zero.
    fn process_memory_utilization(&self) -> Option<f32> {
        None
    }

    /// This process's CPU utilization as a fraction `[0.0, 1.0]`.
    ///
    /// Like [`Platform::process_memory_utilization`], this is obtained from OS
    /// process accounting and therefore lives behind the backend. Returns `None`
    /// when the backend has no reliable source; callers must not interpret that
    /// as an idle system.
    fn process_cpu_utilization(&self) -> Option<f32> {
        None
    }

    /// Hands a rendered print job to the OS print subsystem.
    ///
    /// The job is passed as a file already serialized in the platform's own
    /// job format. Each backend invokes whatever mechanism its OS provides:
    /// `lpr`/`lp` on macOS and Linux, the shell `Print` verb on Windows. The
    /// file's lifetime is owned by the caller, which deletes it after this
    /// returns; the implementation must not retain the path.
    ///
    /// Returns `Err` with a human-readable reason when no print mechanism is
    /// available or all of them failed, so the caller can report it instead of
    /// pretending the job printed.
    fn spawn_print_job(&self, job_file: &std::path::Path) -> Result<(), String> {
        Err(format!(
            "no system print backend is available for job file '{}': this platform does not \
             expose a spooler this build can submit to",
            job_file.display()
        ))
    }

    /// Whether the OS exposes a print spooler this backend can submit to.
    ///
    /// `PrintDialog::show` asks this before claiming a job can be printed, so a
    /// host without a spooler gets a truthful `false` instead of a dialog that
    /// silently discards the document. Probing for `lp`/`lpr`/`print` is an OS
    /// concern and therefore lives in the backend (principle #36).
    ///
    /// The default is `false`; backends with a spooler override it.
    fn has_print_support(&self) -> bool {
        false
    }

    /// Native widget kinds this backend can construct as real OS controls.
    ///
    /// Control routing asks this instead of testing `cfg(target_os)`: a backend
    /// reports the primitives it actually implements (`SysListView32` on Windows,
    /// for instance), and everything not listed falls back to the custom-painted
    /// backend. This keeps the routing table free of per-OS branches while still
    /// letting a platform promote kinds as its native coverage grows
    /// (principle #36).
    ///
    /// The default is empty: a backend that publishes nothing routes every kind
    /// through the global policy table.
    fn native_widget_kinds(&self) -> &'static [crate::widget::WidgetKind] {
        &[]
    }

    /// Creates a native web engine view, when this backend can host one.
    ///
    /// Returns `None` on backends with no embeddable engine (or no display), which
    /// tells `src/web/` to use its simulated navigation path. The concrete engine
    /// type is a backend-private implementation detail; callers only ever see
    /// [`NativeWebEngine`], so no platform crate is named above this layer
    /// (principle #36).
    ///
    /// The default is `None`; backends with a real engine override it.
    fn create_web_engine(&self) -> Option<Box<dyn NativeWebEngine>> {
        None
    }

    /// Translates a shortcut into the backend's own accelerator representation.
    ///
    /// Returns `None` when this backend has no accelerator support, or when the
    /// shortcut uses a key the backend cannot express. Backends override this
    /// only when registering a menu item needs a representation other than the
    /// displayed text (for example a Win32 `ACCEL` table entry).
    /// Converts a shortcut into the backend's own native representation, when
    /// one is needed for registration.
    ///
    /// Returns `None` when the backend has no representation for shortcuts —
    /// which is the norm and the default, since most hosts register a shortcut
    /// by displayed text alone. `None` therefore means "register it by its text",
    /// not "this shortcut is unsupported"; the latter must be reported by the
    /// registration call itself.
    fn parse_shortcut(&self, _shortcut: &crate::shortcut::Shortcut) -> Option<String> {
        None
    }

    /// Creates a top-level window and returns its id, placing it at `(x, y)` in
    /// logical pixels with the given size.
    ///
    /// The returned id addresses the window in the same space as widget ids, so
    /// later calls ([`Platform::destroy_widget`], geometry updates) accept it.
    /// A backend unable to create a window has no failure channel through this
    /// signature; it should log rather than return a fabricated id.
    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> ObjectId;

    // ── Control construction ────────────────────────────────────────────────
    //
    // Every method below has a default that reports "this host creates no such
    // control". That is the truthful answer now that the library paints every
    // `WidgetKind`: the host supplies a window and a drawing surface, not controls
    // (BLUE15 #55/#56). The declarations are kept rather than deleted so a backend
    // that genuinely owns a native primitive can still opt in — deliberately, in
    // one place — without an API change.
    //
    // A backend implementing one of these must make it return a real, usable id;
    // returning `0` while claiming support is the dishonesty this default removes.

    /// Creates a push button control.
    ///
    /// Default: no control is created, because the library paints buttons itself.
    fn create_button(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = (parent, text, x, y, width, height);
        0
    }
    /// Creates a check box control. See [`Platform::create_button`] for the default.
    fn create_checkbox(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = (parent, text, x, y, width, height);
        0
    }
    /// Creates a single-line text field. See [`Platform::create_button`].
    fn create_line_edit(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = (parent, text, x, y, width, height);
        0
    }
    /// Creates a static label. See [`Platform::create_button`].
    fn create_label(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = (parent, text, x, y, width, height);
        0
    }
    /// Creates a radio button. See [`Platform::create_button`].
    fn create_radio_button(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = (parent, text, x, y, width, height);
        0
    }
    /// Creates a slider. See [`Platform::create_button`].
    fn create_slider(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        let _ = (parent, x, y, width, height);
        0
    }
    /// Creates a progress bar. See [`Platform::create_button`].
    fn create_progress_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = (parent, x, y, width, height);
        0
    }
    /// Creates a combo box. See [`Platform::create_button`].
    fn create_combo_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = (parent, x, y, width, height);
        0
    }
    /// Appends an item to a combo box. Default: no storage, so nothing is added.
    fn combo_box_add_item(&self, combo_box: ObjectId, _text: &str) -> bool {
        let _ = combo_box;
        false
    }
    /// Clears a combo box's items. Default: no storage, so nothing changes.
    fn combo_box_clear_items(&self, combo_box: ObjectId) -> bool {
        let _ = combo_box;
        false
    }
    /// Selects a combo-box item. Default: no storage.
    fn combo_box_set_current_index(&self, combo_box: ObjectId, index: usize) -> bool {
        let _ = (combo_box, index);
        false
    }
    /// Returns the selected combo-box index. Default: none is tracked.
    fn combo_box_current_index(&self, combo_box: ObjectId) -> Option<usize> {
        let _ = combo_box;
        None
    }
    /// Returns how many items a combo box holds. Default: none are tracked.
    fn combo_box_item_count(&self, combo_box: ObjectId) -> usize {
        let _ = combo_box;
        0
    }
    /// Returns a combo-box item's text. Default: none are tracked.
    fn combo_box_item_text(&self, combo_box: ObjectId, index: usize) -> Option<String> {
        let _ = (combo_box, index);
        None
    }
    /// Creates a list box. See [`Platform::create_button`].
    fn create_list_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = (parent, x, y, width, height);
        0
    }
    /// Appends an item to a list box. Default: no storage.
    fn list_box_add_item(&self, list_box: ObjectId, text: &str) -> bool {
        let _ = (list_box, text);
        false
    }
    /// Removes a list-box item. Default: no storage.
    fn list_box_remove_item(&self, list_box: ObjectId, index: usize) -> bool {
        let _ = (list_box, index);
        false
    }
    /// Clears a list box. Default: no storage.
    fn list_box_clear_items(&self, list_box: ObjectId) -> bool {
        let _ = list_box;
        false
    }
    /// Selects a list-box item. Default: no storage.
    fn list_box_set_current_index(&self, list_box: ObjectId, index: usize) -> bool {
        let _ = (list_box, index);
        false
    }
    /// Returns the selected list-box index. Default: none is tracked.
    fn list_box_current_index(&self, list_box: ObjectId) -> Option<usize> {
        let _ = list_box;
        None
    }
    /// Returns how many items a list box holds. Default: none are tracked.
    fn list_box_item_count(&self, list_box: ObjectId) -> usize {
        let _ = list_box;
        0
    }
    /// Returns a list-box item's text. Default: none are tracked.
    fn list_box_item_text(&self, list_box: ObjectId, index: usize) -> Option<String> {
        let _ = (list_box, index);
        None
    }
    /// Creates a container panel. See [`Platform::create_button`].
    fn create_panel(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        let _ = (parent, x, y, width, height);
        0
    }
    /// Creates a menu bar. See [`Platform::create_button`].
    fn create_menu_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = (parent, x, y, width, height);
        0
    }
    /// Creates a menu. See [`Platform::create_button`].
    fn create_menu(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = (parent, text, x, y, width, height);
        0
    }
    /// Attaches a menu bar to a window. Default: no native menus are managed.
    fn attach_menu_bar_to_window(&self, window: ObjectId, menu_bar: ObjectId) -> bool {
        let _ = (window, menu_bar);
        false
    }
    /// Adds an item to a menu.
    ///
    /// # `shortcut`
    ///
    /// This parameter is the shortcut **as it should be displayed**:
    /// `"⌘⇧Z"` on macOS, `"Ctrl+Shift+Z"` on Windows and Linux. Passing `None`
    /// (or `""`) adds a plain item with no accelerator.
    ///
    /// Note the asymmetry with the rest of this crate, where an application
    /// action is declared once with a typed [`crate::shortcut::Shortcut`] and the
    /// platform decides its notation. A display string is the right contract for
    /// *this* method because the text is bound to a concrete menu item on one
    /// concrete OS: building it here keeps the platform-specific spelling out of
    /// the caller, and lets platforms that register accelerators separately (for
    /// example a Win32 `ACCEL` table) parse the text instead of re-deriving it.
    ///
    /// Default: no native menu exists, so no item is added.
    fn menu_add_item(&self, parent_menu: ObjectId, text: &str, shortcut: Option<&str>) -> ObjectId {
        let _ = (parent_menu, text, shortcut);
        0
    }
    /// Returns the accelerator display text bound to a menu item.
    ///
    /// `None` when the id is not a menu item or it has no accelerator. Useful to
    /// assert that a shortcut was actually registered, rather than only rendered
    /// into a label.
    fn menu_item_shortcut(&self, _menu_item: ObjectId) -> Option<String> {
        None
    }
    /// Returns the backend's native handle for a widget, if it has one.
    ///
    /// The value is opaque and backend-specific: it is the platform's own object
    /// pointer or handle (`NSView*` on macOS, `HWND` on Windows, ...), not a
    /// cross-platform type. Backends that keep only logical state return `None`,
    /// which is also the answer for widgets created off the UI thread.
    ///
    /// This is the escape hatch for host code and integration tests that need to
    /// reach the underlying control; it must not be used for routine widget work.
    fn get_native_handle(&self, _widget: ObjectId) -> Option<usize> {
        None
    }
    /// Returns the next pending menu activation. Default: none are produced.
    fn poll_menu_triggered(&self) -> Option<ObjectId> {
        None
    }
    /// Queues a menu activation as if the user had chosen it. Default: no queue.
    fn inject_menu_trigger(&self, menu_item_id: ObjectId) -> bool {
        let _ = menu_item_id;
        false
    }

    /// Activates a menu item through the host's own dispatch, as a click or a matched
    /// key equivalent would.
    ///
    /// Returns `true` when the backend performed the activation. The default answers
    /// `false`, which is the honest answer for a backend that keeps no native menu
    /// object to dispatch to — a caller must not read this as "the item did nothing",
    /// only as "this host has no native dispatch for it".
    ///
    /// This is what lets a runtime probe exercise the real menu route (AppKit's
    /// `performActionForItemAtIndex:`, for example) without the caller importing a
    /// platform toolkit or branching on `cfg(target_os)` itself.
    fn activate_menu_item(&self, _menu_item: ObjectId) -> bool {
        false
    }
    /// Returns the next pending widget activation. Default: none are produced.
    fn poll_widget_triggered(&self) -> Option<ObjectId> {
        None
    }
    /// Returns the next pending typed widget activation. Default: none are produced.
    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        None
    }
    /// Queues a typed widget activation. Default: no queue.
    fn inject_widget_trigger_event(&self, widget_id: ObjectId, kind: WidgetTriggerKind) -> bool {
        let _ = (widget_id, kind);
        false
    }
    /// Creates a tool bar. See [`Platform::create_button`].
    fn create_tool_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = (parent, x, y, width, height);
        0
    }
    /// Creates a status bar. See [`Platform::create_button`].
    fn create_status_bar(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = (parent, text, x, y, width, height);
        0
    }
    #[allow(clippy::too_many_arguments)]
    /// Creates a message box. See [`Platform::create_button`].
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
        let _ = (parent, title, text, x, y, width, height);
        0
    }
    /// Creates a file chooser. See [`Platform::create_button`].
    fn create_file_dialog(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = (parent, x, y, width, height);
        0
    }
    /// Creates a colour chooser. See [`Platform::create_button`].
    fn create_color_dialog(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = (parent, x, y, width, height);
        0
    }
    /// Creates a font chooser. See [`Platform::create_button`].
    fn create_font_dialog(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = (parent, x, y, width, height);
        0
    }
    /// Creates a spin box. See [`Platform::create_button`].
    fn create_spin_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = (parent, x, y, width, height);
        0
    }
    /// Creates a list view. See [`Platform::create_button`].
    fn create_list_view(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = (parent, x, y, width, height);
        0
    }
    /// Creates a scrollable area. See [`Platform::create_button`].
    fn create_scroll_area(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = (parent, x, y, width, height);
        0
    }

    /// Creates a labelled group frame.
    ///
    /// The hybrid route sends `WidgetKind::GroupBox` to the library, so this only
    /// matters to a host that owns a dedicated group primitive. Default: none.
    fn create_group_box(
        &self,
        parent: ObjectId,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = (parent, title, x, y, width, height);
        0
    }

    /// Creates a plain bordered frame container. See [`Platform::create_button`].
    fn create_frame(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        let _ = (parent, x, y, width, height);
        0
    }

    /// Creates a tabbed container. See [`Platform::create_button`].
    fn create_tab_widget(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = (parent, x, y, width, height);
        0
    }

    /// Creates a draggable splitter. See [`Platform::create_button`].
    fn create_splitter(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = (parent, x, y, width, height);
        0
    }

    /// Creates a toggle button. See [`Platform::create_button`].
    fn create_toggle_button(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = (parent, text, x, y, width, height);
        0
    }

    /// Creates a calendar view. See [`Platform::create_button`].
    fn create_calendar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = (parent, x, y, width, height);
        0
    }

    /// Creates a scroll bar. See [`Platform::create_button`].
    fn create_scroll_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = (parent, x, y, width, height);
        0
    }

    /// Creates a fractional spin box. See [`Platform::create_button`].
    fn create_double_spin_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = (parent, x, y, width, height);
        0
    }

    /// Creates a font-family combo box. See [`Platform::create_button`].
    fn create_font_combo_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = (parent, x, y, width, height);
        0
    }

    /// Creates a context (on-demand) menu. See [`Platform::create_button`].
    fn create_context_menu(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = (parent, x, y, width, height);
        0
    }

    /// Creates a transient popup window. See [`Platform::create_button`].
    fn create_popup_window(
        &self,
        parent: ObjectId,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = (parent, title, x, y, width, height);
        0
    }

    /// Creates a generic dialog. See [`Platform::create_button`].
    fn create_dialog(
        &self,
        parent: ObjectId,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = (parent, title, x, y, width, height);
        0
    }

    /// Creates an input dialog. See [`Platform::create_button`].
    fn create_input_dialog(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = (parent, x, y, width, height);
        0
    }

    /// Creates a progress dialog. See [`Platform::create_button`].
    fn create_progress_dialog(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = (parent, x, y, width, height);
        0
    }

    /// Creates a directory chooser. See [`Platform::create_button`].
    fn create_directory_dialog(
        &self,
        parent: ObjectId,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = (parent, title, x, y, width, height);
        0
    }

    /// Creates a date picker. See [`Platform::create_button`].
    fn create_date_picker(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = (parent, x, y, width, height);
        0
    }

    /// Creates a time picker. See [`Platform::create_button`].
    fn create_time_picker(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = (parent, x, y, width, height);
        0
    }

    /// Creates a combined date+time picker. See [`Platform::create_button`].
    fn create_date_time_picker(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = (parent, x, y, width, height);
        0
    }

    /// Creates a busy/activity indicator. See [`Platform::create_button`].
    fn create_activity_indicator(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = (parent, x, y, width, height);
        0
    }

    /// Shows a control. Default: the host holds no controls to show.
    fn show_widget(&self, widget_id: ObjectId) {
        let _ = widget_id;
    }
    /// Hides a control. Default: the host holds no controls to hide.
    fn hide_widget(&self, widget_id: ObjectId) {
        let _ = widget_id;
    }
    /// Moves and resizes a control. Default: the host holds no controls.
    fn set_widget_geometry(&self, widget_id: ObjectId, x: i32, y: i32, width: u32, height: u32) {
        let _ = (widget_id, x, y, width, height);
    }
    /// Updates a control's label. Default: the host holds no controls.
    fn set_widget_text(&self, widget_id: ObjectId, text: &str) {
        let _ = (widget_id, text);
    }
    /// Reads a control's label. Default: the host holds no controls.
    fn get_widget_text(&self, widget_id: ObjectId) -> String {
        let _ = widget_id;
        String::new()
    }
    /// Enables or disables a control. Default: the host holds no controls.
    fn set_widget_enabled(&self, widget_id: ObjectId, enabled: bool) {
        let _ = (widget_id, enabled);
    }
    /// Reports whether a control is enabled. Default: the host holds no controls.
    fn is_widget_enabled(&self, widget_id: ObjectId) -> bool {
        let _ = widget_id;
        false
    }
    /// Shows or hides a control. Default: the host holds no controls.
    fn set_widget_visible(&self, widget_id: ObjectId, visible: bool) {
        let _ = (widget_id, visible);
    }
    /// Reports whether a control is visible. Default: the host holds no controls.
    fn is_widget_visible(&self, widget_id: ObjectId) -> bool {
        let _ = widget_id;
        false
    }

    // ─────────────────────────────────────────────────────────────
    // Uniform native-control properties (one API, every OS)
    // ─────────────────────────────────────────────────────────────
    //
    // Native controls carry more state than text: a slider has a value and a
    // range, a combo box has a selected index, a checkbox has a checked state.
    // Before these methods existed the only OS-independent way to reach that
    // state was [`Platform::set_widget_text`], which (on backends that support
    // it) parses the string into a number — lossy for any control whose payload
    // is not its display text, and unavailable for selection indices.
    //
    // These methods unify the *shape* of the call, not the capabilities of the
    // controls. Each backend maps them onto whatever its own native control
    // actually exposes and reports honestly when it cannot (see below). The
    // point is that a caller writes **one** expression — `set_widget_value(id,
    // v)` — and it compiles and runs on every OS without `cfg(target_os)` or a
    // per-OS `if`/`else`, which is the whole purpose of this trait (principle
    // #35/#36).
    //
    // # Per-OS capability differences are allowed
    //
    // A control may be value-carrying on one OS and not on another; that is a
    // genuine platform difference, not a defect. The defaults return `false` /
    // `None` — never a made-up value — so a backend that has no such property
    // reports it as *absent* (principle #37) and the caller branches on the
    // result at runtime if it cares. What is forbidden is a backend pretending
    // a write took effect when nothing changed.

    /// Set a control's primary numeric value (slider, progress bar, spin box,
    /// scroll bar, dial, ... wherever the OS control has one).
    ///
    /// Returns `true` when the backend wrote the value to a real native control
    /// (or an authoritative state model). Returns `false` when this backend's
    /// control has no numeric value, or the widget id is unknown; the caller
    /// must treat `false` as "the value did not change".
    fn set_widget_value(&self, _widget_id: ObjectId, _value: f64) -> bool {
        false
    }

    /// Read a control's primary numeric value.
    ///
    /// `None` when this backend's control has no numeric value (unknown id, or
    /// a control without one). When it returns `Some`, the value must reflect
    /// what the native control actually holds, not a cached default.
    fn widget_value(&self, _widget_id: ObjectId) -> Option<f64> {
        None
    }

    /// Set a control's `(min, max)` range (slider, spin box, scroll bar).
    ///
    /// Returns `false` when this backend's control has no range or the widget id
    /// is unknown. Clamping the current value against the new range is the
    /// backend's job, exactly as the native control does it.
    fn set_widget_range(&self, _widget_id: ObjectId, _min: f64, _max: f64) -> bool {
        false
    }

    /// Read a control's `(min, max)` range.
    fn widget_range(&self, _widget_id: ObjectId) -> Option<(f64, f64)> {
        None
    }

    /// Set a control's selection index (combo box, list box, tab widget).
    ///
    /// `index == None` clears the selection where this OS control supports it.
    /// Returns `false` when this backend's control has no selection model, the
    /// id is unknown, or the index is out of bounds.
    fn set_widget_selected_index(&self, _widget_id: ObjectId, _index: Option<usize>) -> bool {
        false
    }

    /// Read a control's current selection index.
    fn widget_selected_index(&self, _widget_id: ObjectId) -> Option<usize> {
        None
    }

    /// Set a checkable control's checked state (check box, radio button, toggle
    /// button).
    ///
    /// Returns `false` when this backend's control is not checkable or the id is
    /// unknown.
    fn set_widget_checked(&self, _widget_id: ObjectId, _checked: bool) -> bool {
        false
    }

    /// Read a checkable control's checked state.
    fn is_widget_checked(&self, _widget_id: ObjectId) -> Option<bool> {
        None
    }

    /// Set a control's increment step (slider, spin box, scroll bar).
    ///
    /// The step is how far a single keyboard/arrow interaction moves the value.
    /// Returns `false` when this backend's control has no settable step (a
    /// progress bar, for example) or the id is unknown.
    fn set_widget_step(&self, _widget_id: ObjectId, _step: f64) -> bool {
        false
    }

    /// Read a control's increment step.
    fn widget_step(&self, _widget_id: ObjectId) -> Option<f64> {
        None
    }

    /// Set an indeterminate (busy) state on a progress-style control.
    ///
    /// The three desktop toolkits all express this natively — AppKit
    /// `setIndeterminate:`, Win32 `PBS_MARQUEE`, GTK `pulse()` — so the control
    /// animates instead of showing a fixed fraction. Returns `false` when this
    /// backend's control has no indeterminate mode or the id is unknown.
    fn set_widget_indeterminate(&self, _widget_id: ObjectId, _indeterminate: bool) -> bool {
        false
    }

    /// Read a progress-style control's indeterminate state.
    fn is_widget_indeterminate(&self, _widget_id: ObjectId) -> Option<bool> {
        None
    }

    /// Set a text-entry control's read-only state.
    ///
    /// AppKit uses `setEditable:`, Win32 `EM_SETREADONLY`, GTK
    /// `set_editable(false)`. Returns `false` when this backend's control is not
    /// a text entry or the id is unknown.
    fn set_widget_read_only(&self, _widget_id: ObjectId, _read_only: bool) -> bool {
        false
    }

    /// Read a text-entry control's read-only state.
    fn is_widget_read_only(&self, _widget_id: ObjectId) -> Option<bool> {
        None
    }

    /// Set a text-entry control's maximum accepted length in characters.
    ///
    /// Win32 uses `EM_SETLIMITTEXT` and GTK `set_max_length`; AppKit's
    /// `NSTextField` has no direct equivalent (it is enforced through a delegate),
    /// so the macOS backend honestly returns `false` here. Callers that need the
    /// limit on every OS must enforce it themselves.
    fn set_widget_max_length(&self, _widget_id: ObjectId, _max_length: u32) -> bool {
        false
    }

    /// Read a text-entry control's maximum accepted length.
    fn widget_max_length(&self, _widget_id: ObjectId) -> Option<u32> {
        None
    }

    /// Apply or clear a window state (maximised, minimised, full-screen, ...).
    ///
    /// Returns `true` when the backend changed the real OS window. Returns
    /// `false` when the id is not a window, the state is not meaningful on this
    /// toolkit, or the window is state-only (created off the UI thread). The
    /// caller must treat `false` as "the window state did not change".
    fn set_window_state(&self, _widget_id: ObjectId, _flag: WindowStateFlag, _on: bool) -> bool {
        false
    }

    /// Read a window state.
    ///
    /// `None` when the id is not a window or the state is not meaningful on this
    /// toolkit (so it cannot be reported honestly).
    fn is_window_in_state(&self, _widget_id: ObjectId, _flag: WindowStateFlag) -> Option<bool> {
        None
    }

    /// Set a window's minimum content size.
    ///
    /// Implemented natively by AppKit (`setContentMinSize:`), Win32 (the
    /// `WM_GETMINMAXINFO` handler) and GTK (`set_geometry_hints` with
    /// `GDK_HINT_MIN_SIZE`). Returns `false` when the id is not a window, the
    /// window is state-only, or the backend has no native constraint mechanism.
    fn set_window_min_size(&self, _widget_id: ObjectId, _width: u32, _height: u32) -> bool {
        false
    }

    /// Read a window's minimum content size.
    ///
    /// `None` when the id is not a window. A window without an explicit minimum
    /// reports the platform default rather than a made-up zero, because `(0, 0)`
    /// is what the OS treats as "no constraint" and would be indistinguishable
    /// from a real request.
    fn window_min_size(&self, _widget_id: ObjectId) -> Option<(u32, u32)> {
        None
    }

    /// Set a window's icon from a file path.
    ///
    /// AppKit loads an `NSImage` and assigns it with `setRepresentation:`, Win32
    /// loads an `HICON` with `LoadImageW` and sends `WM_SETICON`, GTK uses
    /// `set_icon_from_file`. Returns `false` when the id is not a window, the
    /// path cannot be loaded, or the backend has no icon concept.
    fn set_window_icon(&self, _widget_id: ObjectId, _path: &str) -> bool {
        false
    }

    /// Read a window's icon path, if one was set.
    ///
    /// This reports the *path the caller supplied*, not decoded pixels — no
    /// desktop toolkit hands an icon back as a path, so this is the state model's
    /// answer and is documented as such.
    fn window_icon(&self, _widget_id: ObjectId) -> Option<String> {
        None
    }

    /// Set a text entry's selection range as `(start, end)` character offsets.
    ///
    /// AppKit uses `setSelectedRange:` on the `NSTextView`, Win32 `EM_SETSEL`, GTK
    /// `select_region`. Returns `false` when the id is not a text entry, the range
    /// is inverted/out of bounds for this backend, or the window/control has no
    /// native selectable text.
    fn set_widget_selection(&self, _widget_id: ObjectId, _start: u32, _end: u32) -> bool {
        false
    }

    /// Read a text entry's selection range.
    ///
    /// `None` when the id is not a text entry or nothing is selected, so a caller
    /// can tell "no selection" from "selected the empty range at 0".
    fn widget_selection(&self, _widget_id: ObjectId) -> Option<(u32, u32)> {
        None
    }

    /// Set a text entry's placeholder (cue) text.
    ///
    /// Win32 uses `EM_SETCUEBANNER` and GTK `set_placeholder_text`. AppKit's
    /// `NSTextView` — which is what this crate's macOS line edit is built on — has
    /// **no** placeholder concept, so the macOS backend honestly returns `false`
    /// rather than pretending (principle #37).
    fn set_widget_placeholder(&self, _widget_id: ObjectId, _text: &str) -> bool {
        false
    }

    /// Read a text entry's placeholder text.
    fn widget_placeholder(&self, _widget_id: ObjectId) -> Option<String> {
        None
    }

    /// Set a text entry's echo mode.
    ///
    /// Win32 uses `EM_SETPASSWORDCHAR` and GTK `set_visibility`. AppKit expresses
    /// this by class choice (`NSSecureTextField` vs `NSTextField`), and this
    /// crate's macOS line edit is an `NSTextView`, so the macOS backend reports
    /// `false` — switching class would make windows re-parent an existing view,
    /// which is out of scope for an attribute write (principle #37).
    fn set_widget_echo_mode(&self, _widget_id: ObjectId, _mode: EchoMode) -> bool {
        false
    }

    /// Read a text entry's echo mode.
    fn widget_echo_mode(&self, _widget_id: ObjectId) -> Option<EchoMode> {
        None
    }

    /// Apply a slider's orientation.
    ///
    /// This is called **once, right after creation**, not as a general setter:
    /// Win32 fixes orientation with the `TBS_VERT` window style and has no
    /// `TBM_*` message to change it later, and AppKit configures the track
    /// direction when the slider is built. Only GTK can flip it live, so exposing
    /// this as a creation-time step is what makes one call site work on all three.
    ///
    /// Returns `false` when the id is not a slider on this backend or the change
    /// could not be applied.
    fn set_slider_orientation(&self, _widget_id: ObjectId, _orientation: Orientation) -> bool {
        false
    }

    /// Read a slider's orientation, when the backend tracks one.
    fn slider_orientation(&self, _widget_id: ObjectId) -> Option<Orientation> {
        None
    }

    /// Set a checkable control's tri-state mode.
    ///
    /// When enabled, the control accepts an indeterminate/partial state in
    /// addition to on/off. Win32 uses `BS_3STATE`/`BS_AUTO3STATE`, GTK
    /// `set_inconsistent`, and AppKit `setAllowsMixedState:` together with
    /// `NSControlStateValueMixed` — so all three desktops support it.
    ///
    /// Returns `false` when the id is not a checkable control on this backend.
    fn set_widget_tristate(&self, _widget_id: ObjectId, _enabled: bool) -> bool {
        false
    }

    /// Read a checkable control's tri-state mode.
    fn is_widget_tristate(&self, _widget_id: ObjectId) -> Option<bool> {
        None
    }

    /// Put a radio button into a named mutually-exclusive group.
    ///
    /// Selecting one member clears the others. GTK models this natively with
    /// `RadioButton::join_group`, Win32 with the `WS_GROUP` style that makes
    /// consecutive siblings mutually exclusive, and AppKit by making adjacent
    /// same-class buttons in one superview auto-exclusive. Returns `false` when
    /// the id is not a radio button on this backend.
    fn set_widget_group(&self, _widget_id: ObjectId, _group: &str) -> bool {
        false
    }

    /// Read a radio button's group name.
    fn widget_group(&self, _widget_id: ObjectId) -> Option<String> {
        None
    }

    /// Set a scrollable container's scroll offset in virtual pixels.
    ///
    /// Win32 uses `SetScrollPos` on the scroll styles of the container window,
    /// GTK drives the `hadjustment`/`vadjustment` pair, and AppKit scrolls the
    /// `NSClipView`. Returns `false` when the id is not a scroll area on this
    /// backend.
    fn set_widget_scroll_position(&self, _widget_id: ObjectId, _x: i32, _y: i32) -> bool {
        false
    }

    /// Read a scrollable container's scroll offset.
    fn widget_scroll_position(&self, _widget_id: ObjectId) -> Option<(i32, i32)> {
        None
    }

    /// Enable or disable IME input handling for a widget.
    fn set_widget_ime_enabled(&self, _widget_id: ObjectId, _enabled: bool) -> bool {
        false
    }
    /// Query IME enabled state for a widget.
    fn is_widget_ime_enabled(&self, _widget_id: ObjectId) -> bool {
        false
    }
    /// Returns a reference to the platform's IME bridge, if available.
    fn ime_bridge(&self) -> Option<&dyn crate::platform::ime::ImeBridge> {
        None
    }
    /// Set accessibility name/label for a widget.
    fn set_widget_accessibility_name(&self, _widget_id: ObjectId, _name: &str) -> bool {
        false
    }
    /// Read accessibility name/label for a widget.
    fn get_widget_accessibility_name(&self, _widget_id: ObjectId) -> String {
        String::new()
    }
    /// Returns a reference to the platform's accessibility bridge, if available.
    fn accessibility_bridge(
        &self,
    ) -> Option<&dyn crate::platform::accessibility::AccessibilityBridge> {
        None
    }

    /// Returns a reference to the platform's rich clipboard backend, if available.
    fn clipboard_backend(&self) -> Option<&dyn crate::platform::clipboard::RichClipboardBackend> {
        None
    }
    /// Set plain text clipboard content.
    fn set_clipboard_text(&self, _text: &str) -> bool {
        false
    }
    /// Read plain text clipboard content.
    fn get_clipboard_text(&self) -> String {
        String::new()
    }
    /// Start a drag operation from source widget.
    fn begin_drag(&self, _source_widget_id: ObjectId, _mime: &str, _payload: &[u8]) -> bool {
        false
    }
    /// Poll next drop event if available.
    fn poll_drop_event(&self) -> Option<DropEvent> {
        None
    }
    /// Inject drop event into backend queue.
    fn inject_drop_event(&self, _event: DropEvent) -> bool {
        false
    }
}
/// Optional mobile-specific extension contract.
pub trait MobilePlatformExtension: Send + Sync {
    /// Returns the active mobile backend family.
    fn mobile_backend(&self) -> MobileBackend;
    /// Attaches runtime to an externally provided native view handle.
    fn attach_to_native_view(&self, _native_handle: usize) -> bool;
}
