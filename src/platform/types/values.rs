// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The value and capability types the platform layer speaks.

// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use crate::compat::{OnceLock, String, Vec};
use crate::core::{ObjectId, Orientation, PlatformFamily, Rect};

/// The frame period every platform backend schedules against, in milliseconds.
///
/// One definition for all hosts: the number handed to [`crate::drive_frame`] and the interval
/// between frame deadlines must be the same, or every transition runs at the ratio between them.
/// The backends used to declare `16` in nine separate files (with two different integer types),
/// which is exactly the drift shape principle #54 forbids. `frame_wait` derives each deadline from
/// the previous target plus this value, so frame work that fits the interval does not stretch the
/// cadence. A backend that later runs at the display's own rate changes this one value.
pub(crate) const FRAME_INTERVAL_MS: u64 = 16;

/// How long an idle backend loop waits before looking again, in milliseconds.
///
/// When `drive_frame` reports that no frame is owed, an idle loop has nothing to do. Waiting a
/// full second would delay a stop request or a state change with no wake channel, so the idle
/// wait is bounded; at 100 ms it cuts idle wakeups 6× versus the frame tick while keeping those
/// unseen transitions responsive. A posted event, a stop request, or scheduled cross-thread work
/// wakes the loop immediately -- this value is only the backstop.
pub(crate) const IDLE_FRAME_INTERVAL_MS: u64 = 100;
#[cfg(all(feature = "serde", widgets_unstripped))]
use serde::{Deserialize, Serialize};

/// Which mouse event a canvas callback should synthesise.
///
/// Shared by the macOS and Windows canvas backends, which both turn a native
/// mouse callback into the library's own `Event`. Each backend previously defined
/// its own byte-identical copy (same variants, same doc, same derive), which is
/// exactly the duplication rule #54 forbids: adding a variant would have had to be
/// done twice, and nothing would have failed if only one was updated.
///
/// Gated to exactly the builds where a consumer exists. Every consumer is itself
/// OS- and feature-gated — `platform/macos/canvas.rs` needs `cocoa-legacy`,
/// `platform/macos_objc2/native.rs` needs `macos`, and `platform/windows/canvas.rs`
/// needs Windows — so naming those same conditions here is what keeps this
/// definition from becoming dead code on a build that merely *could* have a canvas.
/// The condition mirrors the consumers rather than re-deriving a conjunction at each
/// site.
#[cfg(all(
    widgets_unstripped,
    any(
        all(target_os = "macos", any(feature = "cocoa-legacy", feature = "macos")),
        target_os = "windows"
    )
))]
#[derive(Clone, Copy)]
pub(crate) enum MousePhase {
    /// Button went down.
    Press,
    /// Button came up.
    Release,
    /// Pointer moved while the button was held.
    Drag,
    /// Pointer moved with **no** button held.
    ///
    /// Distinct from [`Self::Drag`] because the repaint rule is different: a drag owes a
    /// repaint for every sample (a selection or slider follows it), while a hover move
    /// owes one only when the pointer crossed into or out of a different control. Treating
    /// both as `Drag` made every mouse movement over a 1280×800 window repaint the whole
    /// frame, which is the "laggy while nothing changed" report.
    Move,
    /// The secondary button went down (a context menu, a right-drag).
    ///
    /// A separate variant rather than a `button` parameter on [`Self::Press`]: the
    /// button is what selects the *event*, and a backend that encodes it in the phase
    /// cannot accidentally dispatch a primary press for a secondary one.
    SecondaryPress,
    /// The secondary button came up.
    SecondaryRelease,
    /// A second primary press arrived inside the system's double-click interval.
    ///
    /// This is a distinct event in the library ([`crate::event::Event::MouseDoubleClick`]),
    /// not a press with a higher count, so it cannot share [`Self::Press`].
    DoubleClick,
}

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
    // The answer is fixed for the life of the process (either a client is on `PATH` or it is not),
    // and each check spawns up to two child processes (G-603). The probe therefore runs on its own
    // thread and publishes the answer here; a UI thread that asks never waits for a spooler.
    static AVAILABLE: OnceLock<PrintProbe> = OnceLock::new();
    let probe = AVAILABLE.get_or_init(PrintProbe::default);
    probe.start(|| {
        ["lp", "lpr"].iter().any(|cmd| {
            // Any `ExitStatus` at all proves the executable exists and ran.
            std::process::Command::new(cmd).arg("--help").output().is_ok()
        })
    });
    probe.available()
}

/// A capability probe whose answer arrives after the question.
///
/// # Why the answer is published through an atomic instead of returned from the thread
///
/// [`Platform::has_print_support`](crate::platform::Platform::has_print_support) is a synchronous
/// query, so it cannot wait for a child process. The probe is started once, on a worker thread,
/// and the boolean is published here. A caller that asks before the worker finishes gets `false`
/// — the honest "nothing has proved the capability" answer — and the next query sees the result,
/// because the answer cannot change again for the life of the process.
///
/// `started` is an `AtomicBool` rather than a `OnceLock` body so a second caller never blocks on
/// the first: it either starts the worker or reads the published value, never a lock held across
/// a child process.
#[derive(Default)]
struct PrintProbe {
    started: core::sync::atomic::AtomicBool,
    available: core::sync::atomic::AtomicBool,
}

impl PrintProbe {
    /// Starts `probe` on a worker thread unless it already ran.
    ///
    /// `&'static self` because the worker outlives the call and reads the published flag through
    /// this reference; every caller holds the value in a process-lifetime `OnceLock`.
    fn start(&'static self, probe: impl FnOnce() -> bool + Send + 'static) {
        use core::sync::atomic::Ordering;
        if self.started.swap(true, Ordering::AcqRel) {
            return;
        }
        let available = &self.available;
        std::thread::spawn(move || {
            available.store(probe(), Ordering::Release);
        });
    }

    /// The published answer, or `false` while the probe is still running.
    fn available(&self) -> bool {
        self.available.load(core::sync::atomic::Ordering::Acquire)
    }
}

/// Starts the print probes at startup, so a dialog opened later sees a settled answer.
///
/// [`crate::platform::runtime::init`] calls this once. Asking lazily from a print dialog would
/// work, but the first ask would then still be answered from "not proved yet" — and a host that
/// offers a print action at startup would hide it for that first moment.
#[cfg(not(alloc_frugal))]
pub(crate) fn warm_print_probes() {
    if cfg!(target_os = "windows") {
        let _ = windows_shell_spawnable();
    } else if cfg!(unix_desktop) {
        let _ = unix_print_clients_available();
    }
}

/// Whether the Windows print submission path can be reached from this process.
///
/// [`Platform::spawn_print_job`](crate::platform::Platform::spawn_print_job) on
/// Windows submits through `Start-Process -Verb Print`, which only `powershell`
/// provides. The capability question is therefore "can this process spawn
/// `powershell`", and the test is **spawnability** — principle #38 — not exit
/// status: `powershell -NoProfile -Command "exit 0"` exiting zero proves nothing
/// extra, while a `powershell` that starts and fails is still a `powershell` that
/// exists.
///
/// Like [`unix_print_clients_available`], this stays available on every target and
/// goes unused on the rest, so the gate expression is not duplicated here.
#[allow(dead_code)]
pub(crate) fn windows_shell_spawnable() -> bool {
    // `is_ok()` means the process was created. That is the whole claim. Run on a worker thread
    // for the same reason as the unix probe: the capability cannot change while the process runs,
    // and each check starts `powershell`, which must not be on the UI thread (G-603).
    static AVAILABLE: OnceLock<PrintProbe> = OnceLock::new();
    let probe = AVAILABLE.get_or_init(PrintProbe::default);
    probe.start(|| {
        std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", "exit 0"])
            .output()
            .is_ok()
    });
    probe.available()
}

// ── Kernel-adaptive probes ──
//
// The generic mobile backend is compiled for every host (its state machine is
// platform-independent and unit-tested off-device), so it cannot name one kernel's
// probe module directly. These three helpers pick the source at compile time from
// the *host kernel* — a fact about where the code is running, not about which OS the
// backend models — and never invent a figure:
//
//   * `target_vendor = "apple"` -> `darwin_probes` (`sysconf`, `pmset`, `ps`)
//   * everything else           -> `os_probes`    (`/proc`, `/sys`)
//
// A host with neither source returns the trait's honest "unknown" answer, which is
// what the caller sees as `None` / `false`.

/// Installed physical memory from whichever kernel probe this host has.
#[allow(dead_code)]
pub(crate) fn host_memory_probe() -> Option<u64> {
    #[cfg(target_vendor = "apple")]
    {
        crate::platform::darwin_probes::total_memory_mb()
    }
    #[cfg(not(target_vendor = "apple"))]
    {
        crate::platform::os_probes::total_memory_mb()
    }
}

/// Battery-discharge state from whichever kernel probe this host has.
#[allow(dead_code)]
pub(crate) fn host_battery_probe() -> bool {
    #[cfg(target_vendor = "apple")]
    {
        crate::platform::darwin_probes::is_on_battery()
    }
    #[cfg(not(target_vendor = "apple"))]
    {
        crate::platform::os_probes::is_on_battery()
    }
}

/// This process's RSS-over-reserved-space ratio from whichever kernel probe this
/// host has.
#[allow(dead_code)]
pub(crate) fn host_process_memory_probe() -> Option<f32> {
    #[cfg(target_vendor = "apple")]
    {
        crate::platform::darwin_probes::process_memory_utilization()
    }
    #[cfg(not(target_vendor = "apple"))]
    {
        crate::platform::os_probes::process_memory_utilization()
    }
}

/// The shortcut notation matching the OS this crate is compiled for.
///
/// Apple platforms use AppKit glyphs (`⌘⇧Z`); everything else uses the spelled-out
/// `Ctrl+Shift+Z` convention. This is the one place the compile target decides the
/// notation, keeping the `cfg!` out of `src/shortcut/` (principle #36). Backends
/// may override it through `Platform::shortcut_style`.
pub const fn compile_target_shortcut_style() -> crate::shortcut::PlatformShortcutStyle {
    if cfg!(any(target_os = "macos", apple_uikit)) {
        crate::shortcut::PlatformShortcutStyle::Mac
    } else {
        crate::shortcut::PlatformShortcutStyle::Desktop
    }
}

/// A backend-owned native web engine view.
///
/// **Removed in BLUE20 layer 5 (ruling W1, 2026-09-21).** This trait declared six
/// navigation methods and had exactly one implementation — 76 lines of one-line
/// forwards to `webkit2gtk` on Linux — which was never added to a GTK container, so
/// it never displayed a page. A trait with an implementation that produces no
/// observable behaviour, and a default `None` on every other backend, made the crate
/// look like it rendered the web while it did not.
///
/// What replaced it:
///
/// * `Platform::supports_web_engine` — the capability question, answered honestly
///   (`false` everywhere today) instead of a `Option<Box<dyn ..>>` whose `None` a
///   caller could not distinguish from "not attempted".
/// * `src/web/`'s simulated loader, which is the `web_engine_view` control's real
///   behaviour and is unaffected.
/// * `evaluate_javascript`, which runs on the pure-Rust `boa` engine and never went
///   through this trait at all (see `src/web/web_engine.rs`).
///
/// The deletion is recorded in `docs/log/` and gated by
/// `tools/check_web_engine_honest.sh`, which fails if any of the removed names returns.

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
    /// The window moved without changing size (BLUE28 B28.32).
    ///
    /// The event carries no coordinates: the trigger queue is a fixed `(id, kind)` pair, and a
    /// host that needs the new position reads [`Platform::display_of_window`] and the window's own
    /// geometry. The point of the signal is that a layout anchored to the screen must re-run, not
    /// that the library mirrors the compositor's coordinates.
    ///
    /// [`Platform::display_of_window`]: crate::platform::Platform::display_of_window
    Moved = 6,
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
/// What a drag operation does to the dropped item.
///
/// BLUE28 B28.23: the byte-only `DropEvent` could not express a file drop or the difference
/// between copying and moving. This enum is the platform-neutral action a source proposes and a
/// target accepts; it is `Copy` because it carries no payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(all(feature = "serde", widgets_unstripped), derive(Serialize, Deserialize))]
pub enum DropAction {
    /// Copy the item; the source keeps it.
    Copy,
    /// Move the item; the source relinquishes it.
    Move,
    /// Create a link to the item.
    Link,
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
    /// Files carried by the drag, when the source dropped files rather than bytes.
    ///
    /// `mime` is `text/uri-list` for such a drop; the paths are the decoded entries. Empty for a
    /// byte-only drag, which is why it does not change the meaning of `payload`.
    #[cfg(not(alloc_frugal))]
    pub files: Vec<std::path::PathBuf>,
    /// The action the source proposed and the target accepted.
    pub action: DropAction,
    /// Drop position in the target's logical coordinates, when the platform reports one.
    pub position: Option<(i32, i32)>,
}

/// Controls how text is displayed in a text-entry control.
///
/// Defined here rather than in `app` because a backend must name it when it
/// implements `Platform::set_widget_echo_mode`, and `platform` must not depend
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
/// Supported mobile backend families.
///
/// # Why there is no `DesktopBackend` beside this
///
/// There used to be one, listing `Win32`/`Cocoa`/`Gtk`/`HarmonyDesktop`. It was the symmetric-looking
/// twin of this enum and it was **dead in every direction**: `grep -rn "DesktopBackend::" src/
/// bindings/` found no construction, no `match`, and no trait method returning it, so the only
/// references in the tree were its own definition and the `pub use` that re-exported it. It was not
/// a contract — it was a type that looked like one.
///
/// It was also **wrong**, which is the part that makes deleting it the honest fix rather than a
/// tidy-up: its four variants covered neither Wayland nor `macos-objc2`, both of which are desktop
/// backends this crate ships. A caller who had trusted it to enumerate the desktop backends would
/// have silently mishandled two of them — the same "plausible value with nothing behind it" shape
/// the capability flags kept showing up in.
///
/// What actually answers "which desktop backend is this" is `Platform::backend_name`, which every
/// backend implements and which is already the key the capability matrix and `runtime_gui_mode_for`
/// use. An enum duplicating a name that is authoritative elsewhere is a second source for one fact
/// (principle #54), and this one had already diverged.
///
/// `MobileBackend` stays because it is genuinely used: it is the return of
/// `MobilePlatformExtension::mobile_backend`, and `runtime.rs` matches on it to select the
/// `android-mobile` / `ios-mobile` / `harmony-mobile` name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MobileBackend {
    /// Android mobile backend.
    Android,
    /// iOS mobile backend.
    Ios,
    /// Harmony mobile backend.
    HarmonyMobile,
}

/// The device's physical screen orientation (BLUE28 B28.24).
///
/// Distinct from [`Orientation`], which describes a layout axis. A phone can be landscape-left or
/// landscape-right; a layout that only knows "horizontal" cannot tell them apart, and an app that
/// aligns a control with a camera notch needs the difference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScreenOrientation {
    /// Home indicator at the bottom.
    Portrait,
    /// Home indicator at the top.
    PortraitUpsideDown,
    /// Home indicator on the right.
    LandscapeLeft,
    /// Home indicator on the left.
    LandscapeRight,
}

/// The lifecycle phase a mobile host reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MobileLifecycleState {
    /// Foreground and receiving frames.
    Active,
    /// Visible but not receiving input (a call, a sheet, an app switcher preview).
    Inactive,
    /// Backgrounded; animations must stop and state must be retained.
    Background,
}
/// Cross-platform runtime capability flags.
///
/// # Why the host-service flags are here and not inferred from a failed call
///
/// [`clipboard`](Self::clipboard) and [`open_external`](Self::open_external) describe host
/// *services*, which a backend may or may not have. They used to be unanswerable: a caller
/// learned that it has no clipboard only by writing one and watching the result, and could not
/// ask about external opening at all because no such method existed. That is a capability
/// difference expressed by *trial and error* rather than by a runtime query, which principle #37
/// rules out. `capabilities()` is already the one channel for exactly this kind of fact, so the
/// two flags belong beside the five that were already here.
///
/// # Why there is no `file_dialog` flag beside them
///
/// A file chooser *is* a control, and this crate paints every control itself: the library ships
/// [`FileDialog`](crate::widget::dialog::file_dialog::FileDialog), and the host drives it (it owns
/// the list and writes the chosen row back through the writable `selected_file`). "Is the chooser
/// the host's or ours" is therefore a **mechanism** question, and principle #4 says a caller
/// expresses only *what it wants* — "a file chooser" — never which mechanism produces it. A flag
/// spelling out "the backend can present the host's own file chooser" put that mechanism choice on
/// the public API (principle #52), and it would have been `false` on every backend, so it carried
/// no fact a caller could act on either. `native_menu` is not a counter-example: a menu bar is a
/// host integration this crate has no self-drawn equivalent of, whereas the file chooser does.
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
    /// The backend has a host clipboard for text (not merely an in-process buffer).
    pub clipboard: bool,
    /// The backend can hand a URL or path to the host's default handler
    /// (see `Platform::open_external`).
    pub open_external: bool,
    /// The backend can present the host's own path picker — "open file", "save file" or "choose a
    /// directory" — and answer with the path the user chose or with "cancelled"
    /// (see `Platform::choose_path`).
    ///
    /// # What this flag is not
    ///
    /// It is not "the library can ask a user for a path". That is
    /// [`crate::widget::dialog::file_dialog`]: a library-painted dialog the *host* fills — it has
    /// no directory listing of its own, so a host that has no list can browse nothing with it.
    /// `true` here means the request can be handed to the operating system's picker, which
    /// browses the filesystem itself and is what a host that has no list needs.
    ///
    /// A `false` is therefore not a defect but a boundary a host plans around: the designer who
    /// reported the original gap ends up on this flag, because the alternative — trying the call
    /// and reading the error — is the "learn it when the action fails" shape principle #37
    /// forbids.
    pub file_dialog: bool,
    /// The backend can enumerate the host's installed font families
    /// (see `Platform::system_fonts`).
    ///
    /// winit does not provide this; it is this crate's own gap, recorded as BLUE28 B28.39. A
    /// `true` means `Platform::system_fonts` answers a real list; an empty list under a `true`
    /// is possible on a minimal system and is not a promise violation.
    pub system_fonts: bool,
}

/// What a host wants a path for.
///
/// # Why one enum rather than three trait methods
///
/// "Open a file", "save a file" and "choose a directory" are three *purposes* of one platform
/// operation: each toolkit spells them as one dialog with a mode flag (`FOS_PICKFOLDERS` /
/// `OFN_PICKFOLDERS` on Windows, `FileChooserAction` on GTK), and the work a backend does —
/// present it, wait, translate the answer — is identical. Three trait methods would triplicate
/// that work in every backend and would let one of the three drift (principles #54/#101).
pub enum PathPurpose {
    /// Pick an existing file to read.
    OpenFile,
    /// Pick a path to write; most toolkits warn about overwriting an existing file.
    SaveFile,
    /// Pick a directory.
    SelectDirectory,
}

/// One entry in a system path picker's filter list.
///
/// # Why the platform layer repeats a shape the widget layer also has
///
/// [`crate::widget::dialog::FileFilter`] is the **library-painted** dialog's type, and the platform
/// layer must not depend on the widget layer (the dependency runs the other way). The two are
/// deliberately the same shape — a user-facing description plus extension suffixes written without
/// the leading dot, with the single entry `*` meaning "every file" — so a host translates one into
/// the other field by field instead of re-deriving what a filter means (DLG-009).
///
/// # How a backend uses it
///
/// [`Platform::choose_path_with_filters`](crate::platform::Platform::choose_path_with_filters)
/// hands these to the host picker. A toolkit whose picker has no per-filter dropdown (AppKit's
/// `allowedFileTypes` is one allowed set, not a list of named filters) can only express the union;
/// such a backend applies the union and the description becomes informational. A backend that cannot
/// express filters at all ignores the list and answers through
/// [`Platform::choose_path`](crate::platform::Platform::choose_path) — the default, and a stated
/// limit rather than a silently dropped request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathFilter {
    /// The label the picker's filter list shows, for example `"Images"`.
    pub description: String,
    /// The accepted extension suffixes, written **without** the leading dot (`"png"`, not
    /// `".png"`). The single entry `"*"` means "every file"; an empty list accepts nothing, which
    /// is a filter that names no file rather than one that names every file.
    pub extensions: Vec<String>,
}

/// Why the host's own path picker did not answer.
///
/// # Why "cancelled" is not one of these
///
/// A cancelled dialog is a *successful* interaction with a defined outcome, so it is
/// `Ok(None)`; only "there is no picker" and "there is one but it cannot be shown here" are
/// errors. Collapsing those two into `Ok(None)` is the merge the clipboard read was fixed for
/// (a host could not tell "the user pressed Cancel" from "this backend cannot show a dialog") —
/// and this time the merge would be worse, because the host's fallback differs: with no picker it
/// should draw its own list, whereas a picker that failed to open is worth reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathPickerError {
    /// This backend has no picker of its own (pairs with a `false`
    /// [`PlatformCapabilities::file_dialog`]).
    Unsupported,
    /// This backend has a picker, but it cannot be presented from here: no display session, or a
    /// thread the toolkit does not allow a modal window on. Asking later from the right thread or
    /// session can succeed, which is why this is not folded into [`Self::Unsupported`].
    Unavailable,
}

/// The answer to an asynchronous path request (BLUE28 B28.08 decision A).
///
/// # Why a separate contract from `choose_path`
///
/// `choose_path` is synchronous: it blocks until the modal dialog closes. A mobile or Web host
/// presents its picker as an Activity result / promise, so the library cannot block there. The
/// asynchronous pair `Platform::request_path` + `Platform::take_path_choice` keeps one
/// contract for those hosts; `choose_path` stays for the desktops that can block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathChoice {
    /// The user chose this path.
    Chosen(std::path::PathBuf),
    /// The user dismissed the picker.
    Cancelled,
    /// The picker could not be shown or its result could not be read.
    Failed,
}

/// Why handing a target to the host's default handler failed.
///
/// The three cases are kept apart because they call for different responses from a caller:
/// `Unsupported` is permanent on this backend, `InvalidTarget` is the caller's argument, and
/// `NoHandler` is a host configuration fact that the caller may want to report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenError {
    /// This backend has no way to hand a target to the host (see
    /// [`PlatformCapabilities::open_external`]).
    Unsupported,
    /// `target` is not something the host's handler can be given: empty, or shaped like a
    /// command-line option.
    ///
    /// # Why option-shaped targets are refused here rather than escaped
    ///
    /// On Linux the handler is `xdg-open`, and it parses its own arguments: `xdg-open -` and
    /// `xdg-open -- /path` both fail with "unexpected option" and exit 1, so inserting a `--`
    /// separator is not a fix. Every caller would otherwise have to re-derive the same
    /// rejection, and the platform knowledge would leak out of `src/platform/` — the
    /// duplication principle #36 forbids. Refusing once, here, is what keeps that check in one
    /// place.
    InvalidTarget,
    /// The host has no default handler for this target on `PATH`.
    NoHandler,
}
/// Capability contract a **hosted** (desktop-oriented) backend negotiates.
///
/// This is the same flag set as [`PlatformCapabilities`], and deliberately so:
/// the negotiation result and the backend's own report must not be able to
/// disagree. It was previously a separate struct with a field-for-field
/// `from_platform_caps` copy, which is exactly how the two drift — a new
/// capability would be added to one and silently dropped by the other.
///
/// The name describes what the contract *is* (the hosted half of the capability split)
/// rather than a rendering mechanism; see [`CapabilityContract::Hosted`].
pub type HostedCapabilityContract = PlatformCapabilities;

/// Legacy mechanism-vocabulary spelling of [`HostedCapabilityContract`].
#[deprecated(
    note = "use `HostedCapabilityContract`; the contract splits hosted from embedded, not native from other mechanisms"
)]
pub type NativeCapabilityContract = HostedCapabilityContract;
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
/// built for: see `Platform::hosted_capability_contract` and
/// `Platform::embedded_capability_contract`. The variant tells the caller
/// which half of the capability model applies, so it must not have to guess
/// from the widget set.
pub enum CapabilityContract {
    /// Desktop/hosted backend, described by the full capability set.
    Hosted(HostedCapabilityContract),
    /// Constrained backend, described by the reduced embedded set.
    Embedded(EmbeddedCapabilityContract),
}
/// The capabilities a backend with the given `family` reports when it does **not**
/// override `Platform::capabilities`.
///
/// Exposed so the documented OS capability matrix can be checked against source
/// rather than trusted. A backend cannot be instantiated off its own host, but the
/// *trait default* is a pure function of the family, and that is the half of the
/// answer a reader of the matrix is least able to verify.
///
/// # Why this is no longer derived from the family
///
/// It used to answer `true` for the four host integrations on `Desktop` — an
/// **inference from a classification to a capability** (see `Platform::capabilities`,
/// which now defaults to an honest all-`false`). The function still takes a family so
/// the matrix test can name the case it is describing, but the answer no longer varies
/// with it: an undeclared capability is absent, whichever class of machine the backend
/// runs on. Pair it with the backend's own `fn capabilities` when one exists.
pub fn default_capabilities_for(_family: PlatformFamily) -> PlatformCapabilities {
    PlatformCapabilities {
        dpi_scaling: false,
        ime: false,
        accessibility: false,
        native_menu: false,
        typed_widget_trigger: true,
        clipboard: false,
        open_external: false,
        // An undeclared capability, like the four above it: no backend gets a picker by
        // inference from its family (see this function's docs).
        file_dialog: false,
        system_fonts: false,
    }
}

/// Identifies one display in a backend's `Platform::displays` list.
///
/// A newtype rather than a bare `u64` so a display id cannot be confused with a widget id or a
/// window id at a call site: they are all integers, and the mistake is silent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DisplayId(u64);

impl DisplayId {
    /// Wraps a backend-defined raw identifier.
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// The backend-defined raw identifier.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// One display (monitor) the host can place windows on.
///
/// # Why this exists
///
/// A host that keeps a window visible across a monitor change — "the panel was on the display
/// that just got unplugged; put it back on a display that remains" — needs two readings it could
/// not get from this crate before: the current set of displays, and which one a window is on.
/// `Platform::dpi_scale_factor` answers a single global scale for "the" display and
/// [`PlatformCapabilities::dpi_scaling`] says whether that answer is measured, but neither can
/// enumerate screens, so a host could neither compare a window's screen against the remaining
/// ones nor read a per-display `scale` after a DPI change. See
/// `docs/issues/feedback-20261015.md` §十二.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Display {
    /// Identifies this display within the backend's list; stable while it stays attached.
    pub id: DisplayId,
    /// The display's full bounds in the desktop's logical pixel space.
    pub bounds: Rect,
    /// The part of [`Self::bounds`] a window may occupy — excludes taskbars, docks and panels.
    pub work_area: Rect,
    /// The scale factor for this display (`1.0` means unscaled).
    pub scale: f32,
    /// Whether this is the host's primary/reference display.
    pub is_primary: bool,
}

/// One installed font family the host can render with.
///
/// # Why `source` is optional
///
/// A host that wants the widget layer to paint with a system face must hand back its **bytes**
/// through `crate::render::text::register_face`, and that needs a readable file. CoreText
/// (`kCTFontURLAttribute`), DirectWrite (`IDWriteLocalFontFileLoader`) and fontconfig all name the
/// file behind a family, so `source` is populated there. A backend that knows only names — Harmony's
/// empty list, for example — reports `None`. A caller that requires bytes must skip entries whose
/// `source` is `None` rather than guessing a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemFont {
    /// The family name, e.g. `"Noto Sans"`.
    pub family: String,
    /// The style name when the backend reports one, e.g. `"Regular"`; empty when it does not.
    pub style: String,
    /// A readable font file for this family, when the backend knows one.
    pub source: Option<std::path::PathBuf>,
}

/// A window state to apply at creation time, and the attributes that could not be applied.
///
/// BLUE28 B28.41: `create_window` takes only a title and geometry, so a host that needs a
/// borderless/transparent/non-resizable window had no way to ask. Rather than add parameters to
/// the one historical entry point, the attributes are a value type and the creation result says
/// which ones the backend could not honour — silently dropping them is the failure mode this
/// type exists to prevent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WindowAttributes {
    /// Window title. Empty keeps the backend's default (also empty).
    pub title: String,
    /// Top-left position; `None` lets the backend choose (the historical `create_window` uses 0,0).
    pub position: Option<(i32, i32)>,
    /// Client size in logical pixels. `None` uses 800×600, matching the documented default.
    pub size: Option<(u32, u32)>,
    /// Minimum client size, when the backend can constrain it.
    pub min_size: Option<(u32, u32)>,
    /// Maximum client size, when the backend can constrain it.
    pub max_size: Option<(u32, u32)>,
    /// Whether the user may resize the window.
    pub resizable: Option<bool>,
    /// Whether the OS draws a title bar and border.
    pub decorations: Option<bool>,
    /// Whether the window content is transparent. Backend support is platform-specific; see
    /// [`Platform::set_window_transparent`](crate::platform::Platform::set_window_transparent).
    pub transparent: Option<bool>,
    /// Whether the window is shown at creation. `None` keeps the historical hidden-until-`show`
    /// behaviour documented on [`crate::create_window`].
    pub visible: Option<bool>,
    /// Whether the window stays above other windows. Backend support is platform-specific; see
    /// [`Platform::set_window_always_on_top`](crate::platform::Platform::set_window_always_on_top).
    pub always_on_top: Option<bool>,
    /// Start fullscreen.
    pub fullscreen: Option<bool>,
    /// Start maximised.
    pub maximized: Option<bool>,
    /// Preferred light/dark appearance (`Some(true)` = dark), when the backend supports a
    /// per-window override; see
    /// [`Platform::set_window_preferred_theme`](crate::platform::Platform::set_window_preferred_theme).
    pub preferred_theme: Option<bool>,
    /// Whether the OS may include the window in screenshots/recordings; see
    /// [`Platform::set_window_content_protected`](crate::platform::Platform::set_window_content_protected).
    pub content_protected: Option<bool>,
    /// Icon file path, when the backend has a per-window icon concept.
    pub window_icon: Option<String>,
}

/// The result of [`Platform::create_window_with`](crate::platform::Platform::create_window_with).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowCreation {
    /// The window id, or `0` when the backend could not create one.
    pub window: ObjectId,
    /// Attributes this backend could not apply, by field name. Empty means every requested
    /// attribute was applied.
    pub unsupported: Vec<&'static str>,
}

/// The colour space a display advertises, when the backend can read one.
///
/// # Why this is not the image layer's `ColorSpace`
///
/// `crate::image::format::ColorSpace` describes an **image file's** encoding. This type describes
/// a **display transport's** colour space, which is the fact a renderer would need to decide how
/// to encode its output. They overlap today only in `Srgb`; keeping them separate avoids making
/// the platform layer depend on the image codecs module, and neither is an alias of the other
/// because their remaining variants differ. BLUE28 B28.21 marks colour management as P3: the
/// field is published so a backend can fill it, but no renderer consumes it yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayColorSpace {
    /// Standard sRGB.
    Srgb,
    /// Display P3.
    DisplayP3,
    /// BT.2020 (HDR transports).
    Bt2020,
}

/// Optional metadata about one display, beyond the geometry [`Display`] carries.
///
/// BLUE28 B28.21: `Display` keeps its existing fields (a non-breaking extension), and this type
/// carries the facts that were missing. Every field is optional because a backend that cannot read
/// one answers `None` rather than inventing a refresh rate or a colour space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayInfo {
    /// Refresh rate in millihertz (`60_000` = 60 Hz).
    pub refresh_rate_mhz: Option<u32>,
    /// The display's physical orientation, as the core layer's axis vocabulary
    /// (`Horizontal` = landscape, `Vertical` = portrait).
    pub orientation: Option<Orientation>,
    /// The transport's colour space, when the backend can read it.
    pub color_space: Option<DisplayColorSpace>,
    /// Whether the display reports HDR capability.
    pub hdr: Option<bool>,
}

/// The cursor shape a control asks the pointer to take while it is hovered.
///
/// BLUE28 B28.09 separates the **widget intent** from the platform's cursor object: a control
/// answers [`Widget::cursor_icon`](crate::widget::Widget::cursor_icon), the runtime dispatches it
/// once per hover change through `Platform::set_cursor`, and each backend maps it to its own
/// cursor API. The variant set mirrors winit's `CursorIcon` minus the platform-only variants this
/// library has no consumer for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(all(feature = "serde", widgets_unstripped), derive(Serialize, Deserialize))]
pub enum CursorIcon {
    /// The platform's default arrow.
    Default,
    /// A hand, for links and clickable non-buttons.
    Pointer,
    /// An I-beam, for text entry and selectable text.
    Text,
    /// A crosshair, for drawing surfaces.
    Crosshair,
    /// A four-way move cursor.
    Move,
    /// A "not allowed" cursor.
    NotAllowed,
    /// Horizontal resize.
    ResizeHorizontal,
    /// Vertical resize.
    ResizeVertical,
    /// Diagonal resize (top-left / bottom-right).
    ResizeNwSe,
    /// Diagonal resize (top-right / bottom-left).
    ResizeNeSw,
    /// An open hand, for a draggable surface that is not being dragged.
    Grab,
    /// A closed hand, while dragging.
    Grabbing,
    /// A busy/wait cursor.
    Wait,
    /// A progress cursor (busy but interactive).
    Progress,
    /// A help cursor.
    Help,
    /// Zoom in.
    ZoomIn,
    /// Zoom out.
    ZoomOut,
}

/// The parts of a window's edges that system UI occupies, in logical pixels.
///
/// # What the four numbers mean
///
/// They are **edge distances**, not a rectangle: `top` is how much of the window's top edge is
/// covered by system UI (a notch, a status bar, a camera housing), and content must start below
/// it. `left`/`right` cover side obstructions (rounded corners, a landscape notch), and `bottom`
/// covers the home indicator or a gesture bar.
///
/// # Why the default is not all zeros
///
/// It describes a portrait phone with a notch and a home indicator. It is the value the
/// [`SafeArea`](crate::widget::SafeArea) widget paints with when the host supplies none, because
/// on such a device zero insets place content under the status bar and the home indicator. It is
/// **not** a platform answer: a backend that cannot read the real insets reports `None` from
/// `Platform::safe_area_insets`, which is a different fact from "the insets are zero".
///
/// # Why this type lives in the platform layer
///
/// `Platform::safe_area_insets` returns it, and the widget layer re-exports it. Defining the
/// struct in the widget layer and translating in the platform layer would put two descriptions of
/// one fact in the tree — the duplication principle #54 forbids.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SafeAreaInsets {
    /// Space to reserve along the top edge, in logical pixels.
    pub top: u32,
    /// Space to reserve along the bottom edge, in logical pixels.
    pub bottom: u32,
    /// Space to reserve along the left edge, in logical pixels.
    pub left: u32,
    /// Space to reserve along the right edge, in logical pixels.
    pub right: u32,
}
