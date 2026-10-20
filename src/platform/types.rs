// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Platform abstraction types and capability contracts.

use crate::compat::{format, String, Vec};
use crate::core::{ObjectId, Orientation, PlatformFamily, Rect};
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
    ["lp", "lpr"].iter().any(|cmd| {
        // Any `ExitStatus` at all proves the executable exists and ran.
        std::process::Command::new(cmd).arg("--help").output().is_ok()
    })
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
    // `is_ok()` means the process was created. That is the whole claim.
    std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", "exit 0"])
        .output()
        .is_ok()
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
/// * [`Platform::supports_web_engine`] — the capability question, answered honestly
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
/// What actually answers "which desktop backend is this" is [`Platform::backend_name`], which every
/// backend implements and which is already the key the capability matrix and `runtime_gui_mode_for`
/// use. An enum duplicating a name that is authoritative elsewhere is a second source for one fact
/// (principle #54), and this one had already diverged.
///
/// `MobileBackend` stays because it is genuinely used: it is the return of
/// [`MobilePlatformExtension::mobile_backend`], and `runtime.rs` matches on it to select the
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
    /// (see [`Platform::open_external`]).
    pub open_external: bool,
    /// The backend can present the host's own path picker — "open file", "save file" or "choose a
    /// directory" — and answer with the path the user chose or with "cancelled"
    /// (see [`Platform::choose_path`]).
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
    /// (see [`Platform::system_fonts`]).
    ///
    /// winit does not provide this; it is this crate's own gap, recorded as BLUE28 B28.39. A
    /// `true` means [`Platform::system_fonts`] answers a real list; an empty list under a `true`
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
/// asynchronous pair [`Platform::request_path`] + [`Platform::take_path_choice`] keeps one
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
/// Native-runtime capability contract used by desktop-oriented negotiation.
///
/// This is the same flag set as [`PlatformCapabilities`], and deliberately so:
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
/// # Why this is no longer derived from the family
///
/// It used to answer `true` for the four host integrations on `Desktop` — an
/// **inference from a classification to a capability** (see [`Platform::capabilities`],
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

/// Identifies one display in a backend's [`Platform::displays`] list.
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
/// [`Platform::dpi_scale_factor`] answers a single global scale for "the" display and
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
/// through [`crate::render::text::register_face`], and that needs a readable file. CoreText
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
    /// Maximum client size. No `Platform` method exists yet, so this is always reported unsupported.
    pub max_size: Option<(u32, u32)>,
    /// Whether the user may resize the window.
    pub resizable: Option<bool>,
    /// Whether the OS draws a title bar and border.
    pub decorations: Option<bool>,
    /// Whether the window content is transparent. Backend support is platform-specific.
    pub transparent: Option<bool>,
    /// Whether the window is shown at creation. `None` keeps the historical hidden-until-`show`
    /// behaviour documented on [`crate::create_window`].
    pub visible: Option<bool>,
    /// Whether the window stays above other windows. Backend support is platform-specific.
    pub always_on_top: Option<bool>,
    /// Start fullscreen.
    pub fullscreen: Option<bool>,
    /// Start maximised.
    pub maximized: Option<bool>,
    /// Preferred light/dark appearance (`Some(true)` = dark), when the backend supports a
    /// per-window override.
    pub preferred_theme: Option<bool>,
    /// Whether the OS may include the window in screenshots/recordings.
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
/// once per hover change through [`Platform::set_cursor`], and each backend maps it to its own
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
/// [`Platform::safe_area_insets`], which is a different fact from "the insets are zero".
///
/// # Why this type lives in the platform layer
///
/// [`Platform::safe_area_insets`] returns it, and the widget layer re-exports it. Defining the
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
    /// # What the default answers, and why it is not inferred
    ///
    /// The honest default is **no host integration at all**: a backend that does not override
    /// this reports `false` for `dpi_scaling`, `ime`, `accessibility` and `native_menu`, and
    /// `true` only for `typed_widget_trigger` — which is implemented by the library, not the
    /// host, so it cannot be absent.
    ///
    /// # Why the family no longer decides it
    ///
    /// It used to report the four host integrations as `true` iff `self.family()` was
    /// `PlatformFamily::Desktop`. That is an **inference from a classification to a capability**:
    /// a family says what kind of machine this is, not which integrations the build actually
    /// wired up. A backend that forgot to override inherited `true` and silently claimed a
    /// native menu it does not honour — the exact "plausible fake value" rule #37 forbids. Each
    /// backend already states its own answer where it is true; a default of `false` makes a
    /// forgotten override **under-claim** (a missing feature) instead of **over-claiming** (a
    /// feature that fails at runtime), which is the direction a default must err.
    ///
    /// The capability-matrix gate compares what each backend claims against this default, so a
    /// backend that reports `false` here on purpose is visible rather than merely quiet.
    fn capabilities(&self) -> PlatformCapabilities {
        // One definition for the honest default. This used to spell the literal out again
        // beside `default_capabilities_for`, which is the same flags written twice — the
        // drift shape principle #54 forbids, and the one the capability-matrix gate reads.
        default_capabilities_for(self.family())
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
    ///
    /// `low_memory_mode` is `false` by default — the same rule [`Self::capabilities`] states: the
    /// default must not claim a mode this backend has not declared. An embedded backend that
    /// really runs under a memory budget overrides and says so.
    fn embedded_capability_contract(&self) -> Option<EmbeddedCapabilityContract> {
        if matches!(self.family(), PlatformFamily::Embedded) {
            Some(EmbeddedCapabilityContract {
                fixed_dpi: self.dpi_scale_factor() == 1.0,
                low_memory_mode: false,
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
    /// The displays this host can place windows on, in the backend's own order.
    ///
    /// # What an empty list means
    ///
    /// `Vec::new()` is the default, and it says "this backend reports no displays" — a state-only
    /// backend genuinely has none, and a backend that does not enumerate them reports the same
    /// honest absence rather than a made-up screen (principle #37; the same direction every default
    /// on this trait errs). A host that needs to know the difference before it relies on the list
    /// pairs it with [`PlatformCapabilities::dpi_scaling`], which is `false` on exactly the
    /// backends whose display facts are not measured here.
    ///
    /// # What a backend that implements it must be able to answer
    ///
    /// At least one entry on a real desktop session, the entry's `scale` changing when the
    /// display's DPI changes, and the list shrinking when a display is unplugged. The Linux/GTK
    /// backend reads these from GDK's monitor list; see its override for the mechanics.
    fn displays(&self) -> Vec<Display> {
        Vec::new()
    }
    /// Identifies the display a window currently occupies, or `None` when that cannot be answered.
    ///
    /// `None` is the default for the same reason [`Self::displays`] is empty by default: a backend
    /// that does not track window placement reports an honest absence. A backend that implements it
    /// answers from the window's own position against [`Self::displays`].
    fn display_of_window(&self, _window_id: ObjectId) -> Option<DisplayId> {
        None
    }
    /// A value that changes when the set of displays changes (a display attached or removed, the
    /// primary display changed, or a display's DPI changed).
    ///
    /// # Why a re-readable counter and not a callback
    ///
    /// The library cannot deliver a callback on a thread the host is not running, and a host that
    /// does not own the loop cannot be *pushed* an event. What it can be given is a value it
    /// compares against the one it saw last — the same shape [`crate::asset::AssetWatcher`] uses for
    /// its change signal. `0` means "this backend does not report display changes", the honest
    /// default; a host that reads a non-zero value must treat equality as "nothing changed" only
    /// within one backend, so the two are compared with each other, never to `0`.
    ///
    /// A backend may derive this from the current configuration (so it is a *revision* rather than a
    /// pushed notification) or bump it from a native signal; either way the contract is only that a
    /// different value means ["re-read `displays()`"].
    ///
    /// ["re-read `displays()`"]: Platform::displays
    fn display_generation(&self) -> u64 {
        0
    }

    /// Metadata about one display, beyond [`Display`]'s geometry.
    ///
    /// `None` when the id does not name a display this backend enumerated, or when the backend has
    /// no metadata source at all. A `Some` with every field `None` means the display exists but no
    /// optional fact is readable; that distinction is why the outer `Option` is kept.
    fn display_info(&self, _display: DisplayId) -> Option<DisplayInfo> {
        None
    }

    /// Called immediately before the host presents a frame, so a compositor-backed backend can
    /// align its frame callback with the presentation (BLUE28 B28.42).
    ///
    /// The default is a no-op: a backend whose presentation is synchronous (Win32, AppKit, GTK's
    /// drawing area) already has nothing to wait for. The library never starts a frame loop of its
    /// own from here; the host drives the loop and calls this at the point it is about to blit.
    fn pre_present_notify(&self, _window: ObjectId) {}
    /// The safe-area insets of a window, in logical pixels.
    ///
    /// # What the three answers mean
    ///
    /// * `Some(insets)` — the host reported the window's actual insets. Zero values are a real
    ///   measurement (a desktop window has no system UI over it).
    /// * `None` — this backend cannot read them: no window exists, the call is made from a thread
    ///   the toolkit does not allow, or the OS API is absent. The caller must **not** substitute
    ///   [`SafeAreaInsets::default`] without saying so, because that value fabricates a notch.
    ///
    /// # Why this is on the platform trait
    ///
    /// The insets are OS knowledge (iOS `UIView.safeAreaInsets`, Android `WindowInsets`). A middle
    /// layer that read them would have to know each platform's spelling, which is the layering
    /// rule #36 keeps inside the backends. winit exposes the same fact as `Window::safe_area()`;
    /// the widget-layer [`SafeArea`](crate::widget::SafeArea) is this crate's consumer.
    ///
    /// # Implemented so far
    ///
    /// The UIKit backend (iOS/tvOS/visionOS) reads the recorded `UIWindow`'s `safeAreaInsets` on the
    /// main thread; both macOS backends read `NSView.safeAreaInsets` from the window's content view;
    /// Android reads `WindowManager.getCurrentWindowMetrics().getWindowInsets()` (API 30+).
    /// Windows, GTK/X11, Wayland and Orbital report `Some(zeros)` — their client area has no system
    /// UI over it, which is a measurement rather than an absence. HarmonyOS and the state-only/Web
    /// backends inherit `None`, the honest answer: they cannot name a window whose insets to read.
    fn safe_area_insets(&self, _window_id: ObjectId) -> Option<SafeAreaInsets> {
        None
    }
    /// The host's text-size preference, as a multiplier on the nominal font size.
    ///
    /// A backend would answer this from its own platform's setting — Android's `fontScale`, iOS's
    /// `UIContentSizeCategory`, Windows' text-scaling percentage. The paragraph below is what the
    /// method is *for*; see "Implemented so far" for the backends that currently bind it. (This
    /// doc used to list the three platform settings as though they were implemented, which is the
    /// "documentation describes behaviour the code does not have" shape.)
    ///
    /// # Behaviour of the default
    ///
    /// The default of `1.0` means "this backend does not report a preference", which is honest
    /// rather than fabricated: a layout that assumed a scaling factor nobody supplied would place
    /// text in a space it does not occupy. [`crate::platform::profile::text_scale`] clamps whatever
    /// arrives into `1.0..=3.0` and is the accessor the layout and theme layers actually call.
    ///
    /// # Why it is on this trait rather than read from the environment
    ///
    /// The value is OS knowledge. A middle layer that read it would have to know each platform's
    /// spelling of the setting, which is exactly the layering rule #36 keeps inside the backends.
    ///
    /// # What implementing it requires
    ///
    /// A backend that binds the real setting: on Windows the text-scaling percentage is a
    /// registered accessibility value, on iOS `UIContentSizeCategory` is on the trait's own
    /// `UIApplication` object, and on Android `fontScale` is on the `Configuration` the Activity
    /// holds. Each is a per-backend query, which is why the method is here and not in a shared
    /// helper.
    ///
    /// # Implemented so far
    ///
    /// `WindowsPlatform` reads
    /// `HKCU\Software\Microsoft\Accessibility\TextScaleFactor` and reports the user's chosen
    /// percentage; `AndroidPlatform` reads `Configuration.fontScale` through the JNI bridge. The
    /// remaining backends report `1.0`, which says "no preference reported" rather than "the user
    /// chose 100%". Those two facts are different and only the first is a safe default — see the
    /// Windows override for why the DPI override is *not* the same setting, and this method's
    /// sibling [`Self::dpi_scale_factor`] for the layout scale. iOS's `UIContentSizeCategory` is
    /// deliberately not read: it is a *category* name rather than a multiplier, and mapping it
    /// needs the app's preferred content size, which this backend cannot observe.
    fn text_scale(&self) -> f32 {
        1.0
    }

    /// Whether the host asks for increased contrast.
    ///
    /// `None` is the honest answer for a backend with no such system setting, and is distinct from
    /// `Some(false)`: the latter says the user has seen the setting and left it off. A widget that
    /// prefers an outline over a shadow when this is `Some(true)` should keep its existing
    /// appearance for `None` rather than assuming the user chose normal contrast.
    ///
    /// Sources: Windows `SPI_GETHIGHCONTRAST`, macOS
    /// `NSWorkspace.accessibilityDisplayShouldIncreaseContrast`, iOS
    /// `UIAccessibility.isDarkerSystemColorsEnabled`, Android
    /// `AccessibilityManager.isHighTextContrastEnabled`, GTK's theme-name heuristic (documented
    /// as a heuristic, not a measurement).
    fn high_contrast(&self) -> Option<bool> {
        None
    }

    /// Whether the host asks for reduced motion.
    ///
    /// The style layer collapses animation durations to zero when this is `Some(true)`; `None`
    /// keeps the authored motion. Sources: Windows `SPI_GETCLIENTAREAANIMATION`, macOS
    /// `NSWorkspace.accessibilityDisplayShouldReduceMotion`, iOS
    /// `UIAccessibility.isReduceMotionEnabled`, Android `animator_duration_scale`, GTK's
    /// `gtk-enable-animations`.
    fn reduce_motion(&self) -> Option<bool> {
        None
    }

    /// The interface locale as a BCP-47 tag, or `None` when this backend cannot read one.
    ///
    /// The value is interned once per distinct locale (see `platform::locale`), so the caller can
    /// hold it as a `&'static str` in the style layer's environment snapshot without allocating on
    /// every frame. A backend whose locale can change mid-session returns the new tag; the
    /// interner replaces its single cached value rather than growing.
    fn system_locale(&self) -> Option<&'static str> {
        None
    }

    /// A value that changes when the environment facts change (appearance, contrast, motion,
    /// locale).
    ///
    /// Same polling contract as [`Self::display_generation`]: `0` means this backend does not
    /// report changes. A host that reads a non-zero value compares consecutive readings; a
    /// different value means "re-read the facts". Backends without a native change signal derive
    /// it from the current values, which is a revision rather than a pushed notification.
    fn environment_generation(&self) -> u64 {
        0
    }

    /// Applies a cursor shape to a window.
    ///
    /// The runtime calls this once when the hovered control changes, not per motion event. A
    /// backend that has no cursor API answers `false`, which is the same honest-absence rule as
    /// the other optional methods. A window id that is not a live window also answers `false`.
    fn set_cursor(&self, _window: ObjectId, _icon: CursorIcon) -> bool {
        false
    }

    /// Shows or hides the pointer over a window, when the platform can.
    ///
    /// Kept separate from [`Self::set_cursor`] because hiding is not a shape: a host that hides
    /// the pointer for a video surface still expects the shape to be restored when it returns.
    fn set_cursor_visible(&self, _window: ObjectId, _visible: bool) -> bool {
        false
    }

    /// The installed font families this host can render with.
    ///
    /// An empty list is the honest default: a backend that has not bound an enumeration API reports
    /// "none known" rather than a made-up family. Pair it with
    /// [`PlatformCapabilities::system_fonts`] to distinguish "no enumerator" from "an empty
    /// system". A caller that needs bytes filters on [`SystemFont::source`].
    fn system_fonts(&self) -> Vec<SystemFont> {
        Vec::new()
    }
    /// Whether the host's system theme is currently dark.
    ///
    /// # Why `Option<bool>` rather than a theme enum
    ///
    /// The answer is one bit of OS state. [`ThemeMode`](crate::style::ThemeMode) is a *policy*
    /// (`Light`, `Dark`, `Auto`) that lives in the style layer, and a platform that returned it
    /// would be deciding policy from OS knowledge — the inversion rule #36 forbids. The style
    /// layer's [`environment`](crate::style::environment) converts `Some(true)` to `Dark` and
    /// `Some(false)` to `Light`.
    ///
    /// # What each answer means
    ///
    /// * `Some(true)` — the OS's app appearance is dark.
    /// * `Some(false)` — the OS's app appearance is light.
    /// * `None` — this backend cannot ask, or the call is made from a thread the toolkit does not
    ///   allow. The caller keeps its neutral default; it must not treat `None` as "light".
    ///
    /// # Implemented so far
    ///
    /// Windows reads `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\
    /// AppsUseLightTheme`, macOS reads `NSApplication.effectiveAppearance` on the main thread (both
    /// the cocoa and objc2 backends), the UIKit backend reads
    /// `UITraitCollection.currentTraitCollection().userInterfaceStyle`, and Android reads
    /// `Configuration.uiMode`. GTK/X11, Wayland, Redox, HarmonyOS and the Web backend answer `None`.
    fn system_theme(&self) -> Option<bool> {
        None
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
    ///
    /// # Why this returns `bool`
    ///
    /// Some toolkits accept a stop request only from the thread that runs the loop. AppKit is
    /// the case in point: `-[NSApplication stop:]` is main-thread-only, so a backend reached
    /// from another thread cannot honour the request. That backend used to log the refusal and
    /// return `()`, which made "I asked it to stop" and "it will stop" indistinguishable to a
    /// caller — the quiet no-op principle #53 rules out. The return value is the fact the
    /// caller needs: `true` means the request was delivered to the loop's own thread and the
    /// loop will stop; `false` means this backend cannot deliver it from here, and a caller
    /// that needs a hard stop must arrange one itself (for example by ending the process).
    ///
    /// # Stopping from another thread
    ///
    /// This method is the *perform it now* half, so a backend that the toolkit restricts answers
    /// `false` when the call reaches it from a thread the toolkit would refuse. A caller that is
    /// not the loop's driver — a smoke test, CI, a scripted demo — should call
    /// [`crate::platform::quit`] instead: it records a request that the loop reads at its own
    /// next frame tick, where every toolkit permits the stop, so the session can end without a
    /// cross-thread toolkit call and without `std::process::exit`. See
    /// [`crate::platform::stop`] for that seam.
    fn quit(&self) -> bool;

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

    /// Marks a mounted surface — or **a widget this backend draws as part of a
    /// container's frame** — as needing a repaint.
    ///
    /// # Why a window id is a legitimate argument
    ///
    /// Since 2.0 the library paints every `WidgetKind` itself, so an ordinary
    /// control has no surface of its own: it is drawn into the frame of the
    /// **window** it lives in. `widget::runtime::request_repaint_subtree` is the
    /// function that acts on that fact, and it invalidates the top-level window
    /// first and the control second — precisely because on a
    /// [`mount_surface`](Platform::mount_surface)-style backend the second call
    /// reaches nothing.
    ///
    /// A backend whose container really paints its children (the Windows window
    /// procedure's `WM_PAINT`, GTK's `connect_draw`, AppKit's `drawRect:`) must
    /// therefore accept a **window** id here and return `true`. A backend that
    /// only ever repaints what [`mount_surface`](Platform::mount_surface)
    /// registered is the honest exception, and answers `false` for a window.
    ///
    /// # Return value
    ///
    /// `true` when the backend accepted the request and will repaint. `false`
    /// when `id` names nothing this backend can repaint — a widget it does not
    /// know, or (for a surface-only host) a container it does not draw.
    ///
    /// # Why the default is the shared surface record
    ///
    /// The default used to be a flat `false`. Because *every* hand-written loop
    /// in this file assigns into `Platform::invalidate_surface`, that made a
    /// library-driven repaint a silent no-op on every host that mounts surfaces
    /// only — `mobile`, `harmony`, `macos_objc2`, `portable` — while the same
    /// call worked on the three desktop backends. Nothing reported the failure:
    /// the control answered its event, the frame ledger counted the submission,
    /// and the screen never changed.
    ///
    /// The shared [`BackendState`](crate::platform::state::BackendState) already
    /// records which widgets this backend mounted and queues a repaint the host
    /// drains ([`take_pending_repaint`](crate::platform::state::BackendState::take_pending_repaint)),
    /// so a backend with no painter of its own inherits one honest definition of
    /// "this surface is stale" instead of a stub that discards it. It stays an
    /// *optional* method (principle #53), and it answers `false` for an id this
    /// backend never mounted rather than claiming a repaint it cannot make.
    ///
    /// The three desktop backends override it with their real invalidation and
    /// are unaffected; a backend that overrides it for another reason keeps its
    /// own behaviour untouched.
    fn invalidate_surface(&self, id: ObjectId) -> bool {
        let _ = id;
        false
    }

    /// Records a repaint against whatever this backend uses as its surface store.
    ///
    /// Provided for the backends that keep their surfaces in
    /// [`BackendState`](crate::platform::state::BackendState) and used by the trait's
    /// own default for [`Self::invalidate_surface`]. See that method for why a window
    /// id — not only a mounted surface — is a legitimate argument.
    ///
    /// # Why this is a doc comment on the *next* method and not a declaration of its own
    ///
    /// It describes a *helper* (`BackendState::mark_needs_repaint`) rather than a trait method, so
    /// there is no item for it to attach to. Left as written it was a doc comment followed by a
    /// blank line and then another doc comment — which is what `clippy::empty_line_after_doc_comments`
    /// flags, and, more importantly, what makes the next method's own documentation read as if it
    /// belonged to this paragraph. The trait-method documentation is below; the helper's rationale
    /// lives beside the helper.
    ///
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

    /// Removes and returns the next widget whose surface is awaiting a repaint.
    ///
    /// # Who drains this
    ///
    /// A host that owns its own event loop and drawing API — OpenHarmony (ArkUI),
    /// Android, iOS, the objc2 preview — cannot be called into by a toolkit's own
    /// "this window is dirty" notification, so the backend records the stale surfaces
    /// and the host pulls them through this method. On the desktop backends the
    /// library drives the loop instead, and this is not on the path: they invalidate
    /// the real window directly (see [`Self::invalidate_surface`]).
    ///
    /// # Return value
    ///
    /// `None` when nothing is pending. A widget already awaiting a repaint is not
    /// queued twice, so a burst of invalidations in one frame costs one entry.
    ///
    /// # Why the default is `None` and not a stored queue
    ///
    /// The shared [`BackendState`](crate::platform::state::BackendState) does keep such
    /// a queue, and the backends that mount surfaces override this to drain it. A
    /// backend that mounts none has no queue to answer from, and `None` is the truthful
    /// reply rather than a fabricated "nothing pending" over a store that does not
    /// exist.
    fn take_pending_repaint(&self) -> Option<ObjectId> {
        None
    }

    /// Returns `true` when this backend can host library-painted widgets.
    ///
    /// # What `true` promises
    ///
    /// The method this flag advertises is
    /// [`mount_surface`](Platform::mount_surface): `true` means "asking me to mount
    /// a widget will produce a surface that repaints". It says **nothing** about
    /// how a surface is presented, and it must not be read as "the container's
    /// frame shows its children" — a host answers that question with
    /// [`invalidate_surface`](Platform::invalidate_surface), which on some
    /// backends ([`portable`](crate::platform::portable), `harmony`, `mobile`,
    /// `macos_objc2`) repaints only what was mounted while a desktop backend also
    /// repaints a window's whole child list.
    ///
    /// Hosts ask before building a UI they could not display, so the answer must
    /// be the truth about this build: a backend that has no surface path returns
    /// the `false` default rather than promising one.
    fn supports_surfaces(&self) -> bool {
        false
    }

    /// Returns `true` when this backend can deliver pointer/stylus events carrying pen
    /// pressure and tilt (D09-POINTER-01).
    ///
    /// # What `true` promises
    ///
    /// That the backend has a path from a real pen device to
    /// [`Event::PointerPress`](crate::event::Event::PointerPress)/
    /// [`PointerMove`](crate::event::Event::PointerMove)/
    /// [`PointerRelease`](crate::event::Event::PointerRelease), so a control that consumes
    /// them (a `SignaturePad`, a `Button`) receives real pen data rather than nothing.
    ///
    /// # Why the built-in backends answer `false`
    ///
    /// None of this crate's built-in backends can read a pen's pressure axis without
    /// reaching past the safe bindings they depend on — for example `gdk` 0.18 leaves
    /// `gdk_device_get_axis_value` unwrapped, so GTK's Linux backend has no *safe* way to
    /// turn a pen contact into a normalized pressure and must not fabricate one from the
    /// mouse it does receive (principle #37). The honest answer is therefore `false`, and
    /// the pen path is reached through the same host-injected [`Platform::route_pointer_event`]
    /// route a test uses: a host that *does* own a pen device (an Android `MotionEvent`, an
    /// iOS `UITouch`, an out-of-tree backend with a raw device handle) constructs the
    /// `PointerPress`/`PointerMove`/`PointerRelease` events itself and routes them here.
    ///
    /// A host should ask this before building a pen-only feature, so "this build cannot
    /// show you pen pressure" is answerable rather than silently degrading to a mouse line.
    fn supports_pen_input(&self) -> bool {
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
    /// # Stylus events
    ///
    /// The `PointerPress`/`PointerMove`/`PointerRelease` family routes through this same
    /// method (D09-POINTER-01), so a host that owns a pen device delivers real pen data by
    /// constructing those events and calling here. A control that consumes them (a
    /// `SignaturePad`'s ink, a `Button`'s activation) therefore works end to end through the
    /// public route whether the data came from a device or a test. [`Platform::supports_pen_input`]
    /// reports whether *this backend* has such a device path of its own.
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

    // A `native_widget_kinds()` capability method used to sit here, asking which
    // `WidgetKind`s a backend could build as real OS controls. It was deleted for the
    // reason `control_backend/routing.rs` records in its own doc: there is one painting
    // mechanism now, so "which mechanism paints this kind?" no longer has two answers.
    // The method had zero production callers — `route_preference_for_widget_kind`
    // returns `CustomRequired` unconditionally, no backend overrode it, and its only
    // reference was the test asserting its default was empty (also removed). A runtime
    // capability question whose answer no caller can act on is a claim, not a
    // capability (principle #41/#53), and one that names a mechanism in the public
    // `Platform` trait is exactly what principle #52 forbids.

    /// Whether this backend can host a **real** web rendering engine.
    ///
    /// # Why this replaced `create_web_engine` (BLUE20 layer 5, ruling W1)
    ///
    /// The previous shape returned `Option<Box<dyn NativeWebEngine>>`, and its `None`
    /// conflated two different facts: "this backend has no engine" and "this backend's
    /// engine could not be constructed right now" (no display, GTK not initialised). A
    /// caller that needed to tell the user which happened could not — the doc said so in
    /// as many words. Reporting a *capability* answers the question a caller can act on,
    /// and it is the same shape as [`Platform::supports_surfaces`] next to it
    /// (principle #53: a capability difference is a runtime fact, not an API fork).
    ///
    /// The honest answer is `false` on **every** current backend. The one implementation
    /// that ever returned `true` was Linux behind the `webkit-engine` feature, and it was
    /// removed because it never displayed a page (see the note where `NativeWebEngine`
    /// was). Backends that gain one must override this *and* say so here.
    ///
    /// Callers use it to decide whether to present "rendered page" or "simulated
    /// navigation" affordances, which is the distinction that was previously
    /// unqueryable.
    fn supports_web_engine(&self) -> bool {
        false
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

    /// Whether this backend can own more than one top-level window at a time.
    ///
    /// The default is `true`: the desktop backends, iOS and Redox keep a window table. A backend
    /// whose host model is one Activity/one canvas (Android, HarmonyOS, Web) overrides this to
    /// `false` and makes a second [`Self::create_window`] return `0`, so a caller learns the limit
    /// at creation instead of silently receiving a second handle that paints into the first
    /// window's surface (BLUE28 B28.43).
    fn supports_multiple_windows(&self) -> bool {
        true
    }

    /// Creates a window with a set of attributes and reports what could not be applied.
    ///
    /// The default forwards to [`Self::create_window`] with the attributes' title/position/size,
    /// then applies each attribute through the existing trait methods. That means a backend gets
    /// the behaviour it already implements (min size, state, icon, visibility) for free, and every
    /// attribute with no trait method behind it is named in [`WindowCreation::unsupported`] rather
    /// than silently dropped. A backend that can apply transparent/always-on-top/theme at creation
    /// time overrides this and calls the same `apply_window_attributes` for the rest.
    fn create_window_with(&self, attributes: &WindowAttributes) -> WindowCreation {
        let (x, y) = attributes.position.unwrap_or((0, 0));
        let (width, height) = attributes.size.unwrap_or((800, 600));
        let window = self.create_window(&attributes.title, x, y, width, height);
        if window == 0 {
            let mut unsupported = Vec::new();
            if attributes.min_size.is_some() {
                unsupported.push("min_size");
            }
            if attributes.max_size.is_some() {
                unsupported.push("max_size");
            }
            for (value, name) in [
                (attributes.resizable, "resizable"),
                (attributes.decorations, "decorations"),
                (attributes.transparent, "transparent"),
                (attributes.visible, "visible"),
                (attributes.always_on_top, "always_on_top"),
                (attributes.fullscreen, "fullscreen"),
                (attributes.maximized, "maximized"),
                (attributes.preferred_theme, "preferred_theme"),
                (attributes.content_protected, "content_protected"),
            ] {
                if value.is_some() {
                    unsupported.push(name);
                }
            }
            if attributes.window_icon.is_some() {
                unsupported.push("window_icon");
            }
            return WindowCreation { window, unsupported };
        }
        let unsupported = self.apply_window_attributes(window, attributes);
        WindowCreation { window, unsupported }
    }

    /// Applies the attributes to an existing window, returning the ones this backend refused.
    ///
    /// Every attribute goes through the trait method that already owns that fact, so there is one
    /// implementation per attribute rather than a second one here. Attributes with no method —
    /// maximum size, transparency, always-on-top, per-window theme, content protection — are named
    /// as unsupported. That list is the honest answer until a backend adds a real method.
    fn apply_window_attributes(
        &self,
        window: ObjectId,
        attributes: &WindowAttributes,
    ) -> Vec<&'static str> {
        let mut unsupported = Vec::new();
        if let Some((width, height)) = attributes.min_size {
            if !self.set_window_min_size(window, width, height) {
                unsupported.push("min_size");
            }
        }
        if attributes.max_size.is_some() {
            unsupported.push("max_size");
        }
        for (value, flag, name) in [
            (attributes.resizable, WindowStateFlag::Resizable, "resizable"),
            (attributes.decorations, WindowStateFlag::Decorated, "decorations"),
            (attributes.fullscreen, WindowStateFlag::Fullscreen, "fullscreen"),
            (attributes.maximized, WindowStateFlag::Maximized, "maximized"),
        ] {
            if let Some(on) = value {
                if !self.set_window_state(window, flag, on) {
                    unsupported.push(name);
                }
            }
        }
        if attributes.transparent.is_some() {
            unsupported.push("transparent");
        }
        if attributes.always_on_top.is_some() {
            unsupported.push("always_on_top");
        }
        if attributes.preferred_theme.is_some() {
            unsupported.push("preferred_theme");
        }
        if attributes.content_protected.is_some() {
            unsupported.push("content_protected");
        }
        if let Some(path) = attributes.window_icon.as_deref() {
            if !self.set_window_icon(window, path) {
                unsupported.push("window_icon");
            }
        }
        if let Some(visible) = attributes.visible {
            // `set_widget_visible` has no result channel; a failed call is visible to the caller
            // by reading `is_widget_visible` afterwards.
            self.set_widget_visible(window, visible);
        }
        unsupported
    }

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
    /// Whether this platform can present an **icon** on a native menu item.
    ///
    /// # Why this is askable rather than implicit
    ///
    /// [`Platform::menu_add_item`] takes text and a shortcut and nothing else, so a caller holding an
    /// [`IconSource`](crate::widget::IconSource) has no way to pass it. Without this query the icon is
    /// simply *absent*, which is indistinguishable from "the icon resolved and painted nothing" —
    /// the conflation blue27 §19.13 case I15 exists to catch and §19.11 names as its rule: a native
    /// surface that cannot show a glyph must report **unsupported presentation**, and the caller keeps
    /// its text, rather than the framework pretending everything succeeded.
    ///
    /// # Why the default is `false` and no platform overrides it yet
    ///
    /// §19.11 forbids declaring support before the update/delete, DPI and thread behaviour of the OS
    /// image handle has been verified. No desktop backend in this crate has that path yet (macOS
    /// would need an `NSImage` built at the item's scale and released with the menu; Win32 a
    /// `HBITMAP`/`HICON` with an owner-draw item; GTK a `GtkImage`), so claiming `true` anywhere
    /// would be a claim with nothing behind it. Answering `false` is the accurate statement, and a
    /// caller that reads it can skip building the icon entirely.
    ///
    /// Window and installer icons are **not** this path: they keep the file-path contract of
    /// [`Platform::set_window_icon`] (§13), because an OS application icon is not an arbitrary glyph.
    fn supports_menu_item_icons(&self) -> bool {
        false
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

    /// The standard `raw-window-handle` 0.6 window handle for a window, when this backend owns a
    /// native object it can name.
    ///
    /// # What this is and is not
    ///
    /// This is the **read-only** interop adapter winit's `window_handle()` provides: a host that
    /// wants to create its own GPU surface with `wgpu` can pass it to `Surface::create`. It does
    /// not transfer ownership, it does not change [`Self::mount_surface`], and acquiring a handle
    /// does **not** entitle the host to bypass the library's painting — the same pixels are still
    /// produced by the widget tree. `None` is the honest answer for a backend with no such object
    /// (state-only, Redox, embedded).
    ///
    /// Gated on the `raw-window-handle` feature so the dependency stays opt-in (B28.40).
    #[cfg(feature = "raw-window-handle")]
    fn raw_window_handle(&self, _window: ObjectId) -> Option<raw_window_handle::RawWindowHandle> {
        None
    }

    /// The standard `raw-window-handle` 0.6 display handle, paired with
    /// [`Self::raw_window_handle`].
    ///
    /// `None` when the backend has no display object, or when the platform has no rwh variant
    /// (a host-only backend). The handle borrows nothing, so it is `Copy` and needs no lifetime.
    #[cfg(feature = "raw-window-handle")]
    fn raw_display_handle(&self) -> Option<raw_window_handle::RawDisplayHandle> {
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
    /// Removes the oldest queued activation belonging to `widget_id`, leaving other widgets' queued.
    ///
    /// Default: none, which is truthful for a platform that keeps no addressable queue -- the same
    /// reason [`Self::poll_widget_trigger_event`] defaults to none. See
    /// [`crate::drain_widget_triggers_for`] for why the targeted pop exists beside the FIFO one.
    fn pop_widget_trigger_event_for(&self, widget_id: ObjectId) -> Option<WidgetTriggerEvent> {
        let _ = widget_id;
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
    ///
    /// Implemented natively by Win32 (`ShowWindow`, style masks), both macOS backends
    /// (`zoom:`/`performMiniaturize:`/`toggleFullScreen:`/`setStyleMask:`) and GTK
    /// (`maximize`/`iconify`/`fullscreen`/`set_resizable`/`set_decorated`). Wayland, Android,
    /// iOS, HarmonyOS, Redox and Web go through the shared state macro: the flag is recorded and
    /// the call reports whether the id names a window, but no OS window changes. A caller that
    /// needs to know whether the OS applied it reads [`Self::is_window_in_state`], which those
    /// backends answer from the record.
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
    /// Win32 loads an `HICON` with `LoadImageW` and sends `WM_SETICON`; GTK uses
    /// `set_icon_from_file`. The macOS backends answer `false`: an `NSWindow` has no icon, and the
    /// Dock/window-switcher image is the application bundle's `CFBundleIconFile`. Returns `false`
    /// when the id is not a window, the path cannot be loaded, or the backend has no icon concept.
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

    /// A value that changes when the system clipboard changes, or `0` when this backend cannot
    /// report it.
    ///
    /// Same polling contract as [`Self::display_generation`]: a host compares consecutive values
    /// and re-reads the clipboard when they differ. `0` is the honest "not reported" answer for a
    /// backend whose clipboard is an in-process record.
    fn clipboard_generation(&self) -> u64 {
        0
    }
    /// Set plain text clipboard content.
    fn set_clipboard_text(&self, _text: &str) -> bool {
        false
    }
    /// Read plain text clipboard content.
    ///
    /// Returns `""` for three different situations — the clipboard holds no text, it holds a
    /// non-text format, or the read failed — so a caller that must tell them apart should use
    /// [`Platform::clipboard_text`] instead.
    fn get_clipboard_text(&self) -> String {
        String::new()
    }
    /// The clipboard's text, or `None` when there is no text to read.
    ///
    /// # Why this exists beside `get_clipboard_text`
    ///
    /// A host that wants to tell the user "the clipboard holds an image, not text" needs the
    /// distinction the `String` accessor deliberately collapses. `None` here means "there is no
    /// text" — the clipboard is empty, holds a non-text format, or this backend has no clipboard —
    /// which is exactly the fact a paste handler branches on.
    ///
    /// The default derives it from [`Platform::get_clipboard_text`] and therefore treats an empty
    /// string as "no text". That is the honest reading for a backend whose only source is the
    /// merging accessor; a backend with a richer clipboard overrides this to answer precisely.
    fn clipboard_text(&self) -> Option<String> {
        let text = self.get_clipboard_text();
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }
    /// Hands `target` — a URL or a filesystem path — to the host's default handler.
    ///
    /// # What this is for, and why it is on the trait
    ///
    /// Opening a file or a link in the user's own application is a host service, and each OS
    /// spells it differently (`open` on macOS, `xdg-open` on Linux, `cmd /C start` on Windows).
    /// Without this method every host re-derives the same `cfg(target_os)` ladder and re-handles
    /// the same argument-injection question, which spreads platform knowledge outside
    /// `src/platform/` (principles #35/#36) and gives each host its own subtly different bugs.
    ///
    /// Returns as soon as the request has been handed off, **not** when the target has been
    /// opened: the handler is a separate process with its own lifetime. A caller that needs to
    /// know the target was actually opened must not read this `Ok(())` as proof that it was.
    ///
    /// The default reports [`OpenError::Unsupported`], which pairs with a `false`
    /// [`PlatformCapabilities::open_external`].
    fn open_external(&self, _target: &str) -> Result<(), OpenError> {
        Err(OpenError::Unsupported)
    }
    /// Whether the host has a program installed that [`Platform::open_external`] can hand a target
    /// to.
    ///
    /// # Why this exists beside the capability flag
    ///
    /// [`PlatformCapabilities::open_external`] answers "does this backend *implement*
    /// [`Platform::open_external`]" — a fact about the backend, `true` on every desktop backend
    /// whether or not the machine actually has the handler program (`xdg-open` / `open` / `cmd`)
    /// installed. A caller that wants to attribute a failed "open this document" action — "this
    /// machine has no handler" versus "this target is malformed" — needs the **host-configuration**
    /// fact, and until this method existed the only way to learn it was [`OpenError::NoHandler`],
    /// which comes back only after a target is really handed over (that is, really opened). This
    /// query asks the same question without touching a target.
    ///
    /// # Why the default is `false`
    ///
    /// A backend that does not override this has no handler program to report, which is the honest
    /// absence (principle #37) — the same direction every other default on this trait errs. It is
    /// not a claim that the host has no handler in general; it is the backend saying "I do not know
    /// of one from here", which pairs with [`PlatformCapabilities::open_external`] being `false` on
    /// exactly the backends that reach this default.
    fn has_external_handler(&self) -> bool {
        false
    }
    /// Asks the host for a path, using the picker the user's own desktop provides.
    ///
    /// # Why a host needs this beside the library's own dialog
    ///
    /// [`crate::widget::dialog::file_dialog`] is painted by this library and **filled by the
    /// host**: it has no directory listing, so a host that has no list of its own can browse
    /// nothing with it. This request goes to the operating system's picker instead, which browses
    /// the filesystem itself — the difference between "here is a file selector you must feed" and
    /// "let the user pick a file" that the designer reported as missing.
    ///
    /// # Return value
    ///
    /// * `Ok(Some(path))` — the user chose that path.
    /// * `Ok(None)` — the user cancelled. A complete interaction, not a failure; see
    ///   [`PathPickerError`] for why it is not merged with the errors.
    /// * `Err(PathPickerError::Unavailable)` — this backend has a picker but cannot present it
    ///   here (no display session, or a thread the toolkit forbids a modal window on).
    /// * `Err(PathPickerError::Unsupported)` — this backend has none. That is the default below,
    ///   and [`PlatformCapabilities::file_dialog`] is the same fact queryable **before** the call,
    ///   so a host can choose its fallback without provoking the failure.
    ///
    /// # This call blocks
    ///
    /// A picker is modal on every desktop toolkit: it owns the user's input until it is answered.
    /// Call it from the thread that owns the event loop; the loop keeps running while the dialog
    /// is up, which is what makes the call safe from inside a callback as well.
    fn choose_path(
        &self,
        _purpose: PathPurpose,
        _title: &str,
    ) -> Result<Option<std::path::PathBuf>, PathPickerError> {
        Err(PathPickerError::Unsupported)
    }

    /// Starts an asynchronous path request, returning `false` when this backend has no picker.
    ///
    /// This is the non-blocking half of the BLUE28 B28.08 contract. A mobile/Web host presents
    /// its picker from its own event loop; the answer is delivered later through the host's result
    /// callback (Android `onActivityResult`, iOS document-picker delegate, Harmony ArkTS result,
    /// Web promise) and retrieved through [`Self::take_path_choice`].
    ///
    /// `true` means the request was handed to the host, not that the user will choose a path. A
    /// backend that can only present the synchronous [`Self::choose_path`] answers `false` here.
    fn request_path(&self, _purpose: PathPurpose, _title: &str) -> bool {
        false
    }

    /// Takes the next completed asynchronous path request, if one has arrived.
    ///
    /// `None` means "no answer yet" (or no request was made); [`PathChoice::Cancelled`] and
    /// [`PathChoice::Failed`] are real answers and must not be collapsed into `None`.
    fn take_path_choice(&self) -> Option<PathChoice> {
        None
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
    /// The device's current screen orientation, or `None` when the host has not reported one.
    fn orientation(&self) -> Option<ScreenOrientation> {
        None
    }
    /// A value that changes when the orientation changes.
    fn orientation_generation(&self) -> u64 {
        0
    }
    /// The host-reported lifecycle phase.
    fn lifecycle_state(&self) -> Option<MobileLifecycleState> {
        None
    }
    /// Tells the backend the app is entering the background.
    ///
    /// A backend that can stop its own frame loop does so and answers `true`; the default is the
    /// honest `false` for a host that owns the loop itself.
    fn suspend(&self) -> bool {
        false
    }
    /// Tells the backend the app is active again.
    fn resume(&self) -> bool {
        false
    }
}
