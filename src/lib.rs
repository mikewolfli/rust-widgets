// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! rust_widgets - cross-platform native GUI architecture in pure Rust.

// BLUE11 R9.6: Unsafe code audit — unsafe is required for platform FFI
// Note: Removed `#![allow(unsafe_code)]` — default is allow, no-op.
// `missing_docs` is denied rather than allowed: every public item in every profile
// is documented, and this lint is what keeps it that way. It is enforced on the
// `desktop`, `embedded`, `mini` and `--all-features` builds alike, and CI's clippy
// job runs with `-D warnings`, so a new undocumented public item fails the build.
#![deny(missing_docs)]
// `mini` is the no-std-oriented profile: it compiles against `core` + `alloc` only.
// Every shared type is imported through `compat`, so this attribute is the switch
// that turns the abstraction into an enforced contract.
#![cfg_attr(feature = "mini", no_std)]
// BLUE11: Clippy lints enabled for quality enforcement.
// Individual allows are placed next to their specific violations.
#![cfg_attr(test, allow(clippy::needless_pass_by_value, clippy::unwrap_used))]
// Required unconditionally — `alloc` is available in both std (re-exported)
// and no_std contexts. `core` is always available via `extern crate std` under
// std, but we need direct `alloc::` paths in `compat` for no_std builds.
extern crate alloc;

// `#![no_std]` removes the *standard* prelude, and with it the `thread_local!`
// macro. `thread_local!` is not part of the `core` prelude: it is declared in
// `std` and reaches an ordinary crate only through the `#[prelude_import]`
// `std::prelude::rust_20xx` glob that the compiler injects when `std` is
// implicitly linked. Under `mini` the compiler stops injecting that glob (the
// crate is `no_std`), so every `thread_local!` call site in the crate fails to
// resolve — a *macro*-only regression, which is why it breaks modules such as
// `layout` that never touch `std` directly.
//
// The attribute below makes the compiler re-inject the std prelude even though
// `#![no_std]` is in effect, which is exactly what this crate wants: it is
// no-std-*oriented* and still links `std` on every target it builds for, and the
// glob is purely lexical (no `std` item is brought into scope — `Option`,
// `String`, shadows and the rest stay `core`/`compat`-sourced as before).
//
// Two alternatives were measured and rejected:
// * spelling each call site `std::thread_local!` — the macro is not reachable as
//   a path under `no_std` (verified: `cannot find `thread_local` in `std``), so
//   this does not compile at all;
// * `use std::thread_local;` — a macro-only import of a macro that is *not*
//   exported from the `std` crate root under `no_std`, same failure.
#[cfg(alloc_frugal)]
#[macro_use]
extern crate std;

// `#![no_std]` also removes the *alloc* prelude's trait imports, and unlike the
// macros above they cannot be brought back wholesale.
//
// `ToString` is not in the `alloc` prelude at all: it is re-exported by
// `std::prelude::v1` (`library/std/src/prelude/v1.rs`), which a crate receives
// only through the compiler-injected prelude glob that `#![no_std]` suppresses.
// The consequence is that every `s.to_string()` in the crate — on `&str`,
// `char`, `&&str`, `Shortcut`, `io::Error` — stops resolving under `mini`.
//
// This cannot be repaired from here either. Name resolution in Rust is *lexical*:
// a `use` is visible to items lexically nested in the block or module containing
// it, never to a module reached by path. A crate-root `use std::string::ToString;`
// would fix this file and nothing else, and the ninety-odd call sites live in
// child modules. Verified against a `#![no_std]` crate whose root imported the
// trait while a child module called `.to_string()`: the child still fails E0599.
//
// `#[macro_use]` is the mechanism that *does* cross module boundaries, but it
// only carries the macro namespace — and `ToOwned`, the sibling that supplies
// `.to_owned()`, is a trait, not a macro.
//
// So each module that needs these methods imports the trait itself, from
// `compat`, which re-exports it as [`compat::MiniToString`]. The alias keeps the
// import list readable next to `String` and `ToString` on desktop builds, where
// both names resolve.
//
// **BLUE13 Phase 3: Alloc bridge — unified imports for std and no_std ──**
// All crate files import from `compat` instead of directly from std.
pub mod compat;

/// Action/command system.
pub mod action;
/// Generic asset file watcher.
///
/// Gated on the capability it actually needs (`desktop-runtime` supplies
/// `notify` + `crossbeam-channel`), not on the `desktop` profile: `tablet` and
/// `mobile` enable the same capability, so gating on the profile removed a
/// documented public path from builds that can support it (principle #41).
#[cfg(all(feature = "desktop-runtime", not(alloc_frugal)))]
pub mod asset;
/// Audio module — format detection, decoding, encoding, sample processing, and normalization.
#[cfg(feature = "audio")]
pub mod audio;
/// C ABI and language bindings (C / Java-JNI / etc.).
///
/// Available on the `desktop` profile and on any profile that exposes an
/// FFI consumer (`jni` for Java, `mobile-api` for the mobile runtime). The
/// `mini` profile excludes it because it is `alloc`-free / no-std oriented.
#[cfg(all(any(feature = "desktop", feature = "jni", feature = "mobile-api"), not(alloc_frugal)))]
pub mod bindings;
/// Clipboard helpers.
pub mod clipboard;
/// Control backend abstraction for native/custom control implementations.
pub mod control_backend;
/// Core types and shared contracts.
pub mod core;
/// Reactive data binding system — Model → View automatic synchronization.
pub mod data_binding;
/// Designer support: mode 2, generating Rust source from a project (BLUE19 T-23, D7-b-3).
///
/// # Why this has its own gate rather than riding on `full_widgets`
///
/// `full_widgets` answers "does this build have the widget tree and the capability table?", which
/// every device application needs. This module answers a different question — "is this build **also**
/// a design tool?" — and a shipping application does not.
///
/// The generator and its artifact writer are development-time facilities. Leaving them on for every
/// profile would link a code generator plus `std::fs::write` into delivery artifacts, which is the
/// weight mode 2 exists to remove (BLUE19 §5.1.2).
///
/// `desktop` enables the `designer` feature by default — it is the profile a designer **host** runs
/// on. `tablet`/`mobile`/`mini`/`embedded` do not, because they are the **targets** of a generation,
/// not its host (BLUE19 §5.3.4); a caller who wants the generator there asks for it:
/// `--features tablet,designer`.
///
/// The condition is expressed as the `designer_tooling` alias from `build.rs` rather than a
/// hand-written conjunction, per rule #47.
#[cfg(designer_tooling)]
pub mod designer;
/// Embedded system optimizations and support.
/// Unified error system (ErrorId, RwError, c_try!).
pub mod error;
/// Event types and dispatch helpers.
pub mod event;
/// Gesture recognizer system (gated behind `touch` feature).
#[cfg(feature = "touch")]
pub mod gesture;
/// Hardware-adaptive GPU management.
pub mod gpu;
/// Internationalization module.
#[cfg(feature = "i18n")]
pub mod i18n;
/// Image module — format detection, decoding, encoding, transform, and color conversion.
///
/// **Decoding** is real for: PNG (all bit depths, scanline filters, palette), JPEG,
/// BMP, QOI, Farbfeld, PNM (P5/P6). GIF/WebP/TIFF/AVIF/ICO/SVG decode returns an
/// explicit `Err` rather than fabricated pixels until a codec lands.
#[cfg(feature = "image")]
pub mod image;
/// Declarative JSON window engine (QML-like).
///
/// Compiled for a full device build — `desktop`/`tablet`/`mobile` with an unstripped
/// widget set. Stated as the `full_widgets` alias rather than the conjunction, so it
/// cannot drift from `crate::app` and `crate::view`, which need exactly the same thing
/// (BLUE15 rule #57).
#[cfg(full_widgets)]
pub mod json;
/// Layout managers.
pub mod layout;
/// Memory management utilities.
pub mod memory;
/// Advanced widgets (gated behind `advanced-widgets` feature).
#[cfg(feature = "advanced-widgets")]
pub mod menu_config;
/// Object tree and object utilities.
pub mod object;
/// Performance monitoring and optimization.
pub mod performance;
/// Platform abstraction and backend adapters.
pub mod platform;
/// Quality management for adaptive rendering.
pub mod quality;
/// Rendering traits and primitives.
pub mod render;
/// Runtime render-engine abstraction.
pub mod render_engine;
/// Global shortcut system for keyboard shortcuts.
pub mod shortcut;
/// Signal-slot utilities.
pub mod signal;
/// Style system primitives.
pub mod style;
/// Test infrastructure and utilities.
pub mod test;
/// Theme management — the palette, the appearance selector and the role table controls
/// resolve against, plus the process-wide manager that selects among them.
///
/// # Gating (BLUE20 layer 4, 2026-09-21)
///
/// Gated on `device_profile` (`desktop`/`tablet`/`mobile`), which is a *conjunction*: the
/// theme model needs a colour system (`not(alloc_frugal)`) **and** `serde`, and only the
/// three device profiles enable both. `mini` has no palette; the mixed
/// `windows + controls-custom` build has no `serde`.
///
/// # Why the widget layer no longer breaks when this is absent
///
/// ~120 control files read the theme while drawing, and they also compile in
/// `--no-default-features --features "windows desktop-runtime controls-native
/// controls-custom"` — a full widget set with no device profile. Referencing
/// `crate::theme` from those files was 78 compile errors at HEAD. They now go through
/// `crate::style`, which exposes the same lookups (and the same type *shape* where the real
/// types are absent) and answers "no theme" there, so one path compiles in every profile
/// and the widgets fall back to their own literals where there is no palette to read
/// (principles #37/#47: a missing capability is reported, and the condition lives in one
/// place rather than at each call site).
#[cfg(device_profile)]
pub mod theme;
/// Undo/Redo framework for undoable commands and cross-widget undo/redo.
pub mod undo;
/// Video module — container format detection, frame extraction, metadata, and playback.
#[cfg(feature = "video")]
pub mod video;
/// Generic utility modules (asset watcher, helpers, etc.).
/// Declarative-retained view layer: `state → Node → diff → Patch → retained tree`.
///
/// The declarative half of the hybrid architecture. Additive: it consumes the existing
/// widget factory and property contract rather than changing them.
///
/// Compiled for a device profile (`desktop`/`tablet`/`mobile`) unless the caller opts
/// out with the `no-declarative-view` feature; never compiled for `mini`/`embedded`.
/// The whole condition is the single alias `declarative_view`, so the opt-out and the
/// stripped-profile rule cannot drift apart (BLUE15 rule #57).
#[cfg(declarative_view)]
pub mod view;
/// Web view and engine components.
#[cfg(widgets_unstripped)]
pub mod web;
/// Optional WGPU GPU acceleration backend (gated behind `gpu-wgpu` feature).
#[cfg(feature = "gpu-wgpu")]
pub mod wgpu_backend;
/// Widget definitions and widget helpers.
pub mod widget;
// Re-export all widget types for convenience
pub use widget::*;

/// Translates a message key, or returns it verbatim when i18n is not compiled in.
///
/// # Why a function next to the `tr!` macro
///
/// `tr!` needs a **literal** key, so it cannot serve a caller that holds a key at
/// runtime (a control's `set_translated_tooltip`, a data-driven label). Without this
/// function such a caller has to write its own `cfg` pair, which is exactly how
/// `set_translated_tooltip` ended up gating i18n on `desktop` and silently losing
/// translations on `tablet`/`mobile` (principle #41).
///
/// Having one feature-independent entry point means a call site never needs to know
/// whether the catalogue exists.
///
/// # The no-`i18n` behaviour
///
/// Returns the key itself and warns, matching [`tr!`]: a build without the catalogue
/// must not look localized, and an empty string would be worse than the raw key(which
/// at least names what is missing).
#[cfg(feature = "i18n")]
pub fn translate_key(key: &str) -> crate::compat::String {
    crate::i18n::translate(key)
}

/// Translates a message key, or returns it verbatim when i18n is not compiled in.
///
/// See the `i18n` variant's documentation for why this function exists alongside the
/// [`tr!`] macro.
#[cfg(not(feature = "i18n"))]
pub fn translate_key(key: &str) -> crate::compat::String {
    log::warn!("i18n translate_key called but the i18n feature is disabled, key={key}");
    crate::compat::String::from(key)
}
// NOTE: there is no top-level `chart` module. The chart *engine* (layout, axes,
// ticks, SVG context, adapter) and the chart *widgets* live together under
// `crate::widget::chart_widgets`, because they are two layers of one feature.
// The engine is reachable as `rust_widgets::widget::chart_widgets::charts`
// (and `::types`/`::layout`/`::svg`/`::adapter`).
/// Translates a message key — the no-`i18n` fallback spelling.
///
/// This macro exists so that code which calls `tr!` still compiles when the `i18n`
/// feature is off. It performs **no translation**: it logs a warning naming the key
/// and returns the key itself. That makes a missing translation loud during
/// development instead of quietly rendering an empty string, but it also means the
/// returned text is a message *key*, not user-facing copy — a build without `i18n`
/// must not be shipped as a localized one.
///
/// Accepts the same three arities as the real macro (`$key`, `$key, $count`, and
/// `$key, $context, $count`) so call sites need no `cfg` of their own; the plural
/// and context arguments are ignored here.
#[cfg(not(feature = "i18n"))]
#[macro_export]
macro_rules! tr {
    ($key:expr) => {{
        log::warn!("i18n tr! called but the i18n feature is disabled, key={}", $key);
        $key.to_string()
    }};
    ($key:expr, $count:expr) => {{
        log::warn!("i18n tr! called but the i18n feature is disabled, key={}", $key);
        $key.to_string()
    }};
    ($key:expr, $context:expr, $count:expr) => {{
        log::warn!("i18n tr! called but the i18n feature is disabled, key={}", $key);
        $key.to_string()
    }};
}
/// Application lifecycle wrapper and type-safe widget handles (not available in mini mode).
///
/// Same condition as `crate::json`: `full_widgets`.
#[cfg(full_widgets)]
pub mod app;
/// Index-based widget registry for runtime lookup.
pub mod index;
#[cfg(feature = "pdf")]
/// PDF rendering/export support.
pub mod pdf;
#[cfg(feature = "print")]
/// Print and preview support.
pub mod print;
/// Initialize global platform and i18n subsystems.
///
/// One function for every profile (BLUE15 rule #58): the branch that used to be
/// a `cfg`-gated pair now asks [`platform::profile`], so adding a profile does
/// not mean adding a copy of this function.
pub fn init() {
    trace_runtime_route("init");
    platform::profile::runtime_init();
    platform::profile::init_optional_subsystems();
}
/// Run platform main event loop.
pub fn run() {
    trace_runtime_route("run");
    platform::profile::runtime_run();
}
/// Request platform event loop shutdown.
pub fn quit() {
    trace_runtime_route("quit");
    platform::profile::runtime_quit();
}
/// Logs the resolved profile/backend/route when `RUST_WIDGETS_TRACE_RUNTIME=1`.
///
/// # Why this writes to stderr as well as the log
///
/// `log::info!` goes to the `log` facade, and this crate installs no logger on
/// desktop builds (`src/platform/android_jni.rs` does, via logcat, and that is the
/// only one). A program that has not installed one therefore saw **nothing** when it
/// asked for the trace, so the runtime audit BLUE15 #55 requires could not actually
/// be performed: `RUST_WIDGETS_TRACE_RUNTIME=1` printed an empty line and the route
/// stayed unverified. Writing the same record to stderr when the variable is set
/// makes the audit work out of the box, while the `log` record keeps the event
/// available to a host that does install a logger. The stderr write is opt-in — the
/// variable has to be exactly `1` — so a normal run prints nothing.
pub(crate) fn trace_runtime_route(stage: &str) {
    if std::env::var("RUST_WIDGETS_TRACE_RUNTIME").ok().as_deref() != Some("1") {
        return;
    }

    let line = format!(
        "[rust_widgets.runtime] stage={} profile={} backend={} route={} host={}",
        stage,
        platform::profile::profile_name(),
        platform::platform_facts().backend_name(),
        platform::profile::route_name(),
        platform::profile::host_name()
    );
    log::info!("{line}");
    eprintln!("{line}");
}
// Convenient wrapper functions for platform operations
// Users can call these directly without manually getting a platform instance

/// Resolve the backend that should create a widget of this kind.
///
/// # Why this exists
///
/// A widget kind may map onto a real platform primitive (a `Button` on Win32/
/// AppKit/GTK) or have no primitive at all (`Chart`, `CodeEditor`, …), in which
/// case the platform layer supplies the surface. **Which of the two happens is a
/// backend decision and must not leak to callers**: this function is the single
/// place the creation path asks for it, so the choice cannot drift between call
/// sites (see `control_backend::dispatcher`).
///
/// On a profile without an OS runtime (`mini`, `embedded`) the custom state
/// backend answers instead, so the same call works everywhere.
#[cfg(not(alloc_frugal))]
fn backend_for_kind(kind: widget::WidgetKind) -> &'static dyn control_backend::ControlBackend {
    control_backend::get_control_backend_for_widget(kind)
}

// ── Absent kinds do not need a stand-in ──
//
// `mini`/`embedded` compile out `MenuBar`, `Menu`, `ToolBar`, `StatusBar`,
// `ListView`, the dialogs and their `MessageBox`. The `create_*` functions that
// route on them must still **exist** in every profile so the public surface does
// not vary (principle #53), and the way that is expressed here is a single
// `#[cfg(widgets_unstripped)]` gate on the body plus an honest failure otherwise.
//
// # The dead layer this replaced
//
// There used to be nine `KIND_*` const pairs, each naming the variant under
// `widgets_unstripped` and substituting `WidgetKind::Panel` otherwise, all fed
// into `backend_for_kind(...)`. **That value was never read.**
// `get_control_backend_for_widget` discards its argument (there is one creation
// mechanism, so there is nothing to resolve), and every backend `create_*`
// hardcodes the kind it mounts (`create_menu_bar` mounts `WidgetKind::MenuBar`
// inside `control_backend/custom`). So the stand-in never reached a widget, and
// repainting its gate — to `widgets_unstripped`, to `full_widgets`, to anything —
// changed nothing observable. A prior round recorded a `KIND_LIST_VIEW` gate fix
// as a behaviour change; it was not one.
//
// The reachable defect was the opposite of what the alias pair implied: on `mini`
// and `embedded` the body still called `create_menu_bar`, which the custom backend
// used to answer with a valid id addressing **nothing**, so a caller on those
// profiles held an id that failed every later call. The body is now gated, so those
// profiles get `0` — the documented "no such control here" answer, matching what
// `tests/kind_alias_gate_test.rs` pins for the profiles that do ship the control.

/// The id returned when the running profile does not ship the requested control.
///
/// Not `Panel`'s id, and not any id that would address a different control: a
/// caller that receives `0` can test for it, whereas a valid id addressing the
/// wrong thing cannot be detected at all. `widget::runtime` documents `0` as "no
/// widget", and the C ABI already reports it the same way.
///
/// Gated `not(widgets_unstripped)` because that is exactly where it is consumed:
/// on a profile that ships the whole widget set every `create_*` reaches its real
/// constructor and the fallback branch is compiled out.
#[cfg(all(not(alloc_frugal), not(widgets_unstripped)))]
const NO_SUCH_CONTROL: crate::core::ObjectId = 0;

/// Create a top-level window with specified title and geometry.
///
/// # Example
/// ```
/// let window_id = rust_widgets::create_window("My App", 100, 100, 800, 600);
/// ```
#[cfg(not(alloc_frugal))]
pub fn create_window(
    title: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::Window).create_window(title, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Create a button control as a child of specified parent.
///
/// The backend decides whether this becomes a platform button or is painted by
/// the platform's custom surface; callers get a handle either way.
pub fn create_button(
    parent: crate::core::ObjectId,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::Button).create_button(parent, text, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a check-box. The backend decides whether it is a platform control
/// or is painted by the platform's custom surface.
pub fn create_checkbox(
    parent: crate::core::ObjectId,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::CheckBox)
        .create_checkbox(parent, text, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a single-line text editor. The backend chooses how it is hosted.
pub fn create_line_edit(
    parent: crate::core::ObjectId,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::LineEdit)
        .create_line_edit(parent, text, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a read-only text label. The backend chooses how it is hosted.
pub fn create_label(
    parent: crate::core::ObjectId,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::Label).create_label(parent, text, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a radio button. The backend chooses how it is hosted.
pub fn create_radio_button(
    parent: crate::core::ObjectId,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::RadioButton)
        .create_radio_button(parent, text, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a slider. The backend chooses how it is hosted.
pub fn create_slider(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::Slider).create_slider(parent, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a progress bar. The backend chooses how it is hosted.
pub fn create_progress_bar(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::ProgressBar)
        .create_progress_bar(parent, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a combo box. The backend chooses how it is hosted.
pub fn create_combo_box(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::ComboBox).create_combo_box(parent, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a list box. The backend chooses how it is hosted.
pub fn create_list_box(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::ListBox).create_list_box(parent, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a panel (a container surface). The backend chooses how it is hosted.
pub fn create_panel(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    // `WidgetKind::Panel` is the closest always-available kind; on profiles where
    // a dedicated panel kind exists the routing table answers for it.
    backend_for_kind(widget::WidgetKind::Panel).create_panel(parent, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Create a message box dialog as a child of specified parent.
///
/// Creates a message box. The backend chooses how it is hosted.
///
/// Returns `0` on a profile that does not ship the message box (`mini`,
/// `embedded`). Callers must treat `0` as "no control was created" rather than
/// using it as a parent: see the note above [`NO_SUCH_CONTROL`].
pub fn create_message_box(
    parent: crate::core::ObjectId,
    title: &str,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    #[cfg(widgets_unstripped)]
    {
        backend_for_kind(widget::WidgetKind::MessageBox)
            .create_message_box(parent, title, text, x, y, width, height)
    }
    #[cfg(not(widgets_unstripped))]
    {
        let _ = (parent, title, text, x, y, width, height);
        NO_SUCH_CONTROL
    }
}
#[cfg(not(alloc_frugal))]
/// Create a file dialog as a child of specified parent.
///
/// Creates a file dialog. The backend chooses how it is hosted.
///
/// Returns `0` on a profile that does not ship the file dialog.
pub fn create_file_dialog(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    #[cfg(widgets_unstripped)]
    {
        backend_for_kind(widget::WidgetKind::FileDialog)
            .create_file_dialog(parent, "", x, y, width, height)
    }
    #[cfg(not(widgets_unstripped))]
    {
        let _ = (parent, x, y, width, height);
        NO_SUCH_CONTROL
    }
}
#[cfg(not(alloc_frugal))]
/// Create a color dialog as a child of specified parent.
///
/// Returns `0` on a profile that does not ship the color dialog.
pub fn create_color_dialog(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    #[cfg(widgets_unstripped)]
    {
        backend_for_kind(widget::WidgetKind::ColorDialog)
            .create_color_dialog(parent, "", x, y, width, height)
    }
    #[cfg(not(widgets_unstripped))]
    {
        let _ = (parent, x, y, width, height);
        NO_SUCH_CONTROL
    }
}
#[cfg(not(alloc_frugal))]
/// Create a font dialog as a child of specified parent.
///
/// Returns `0` on a profile that does not ship the font dialog.
pub fn create_font_dialog(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    #[cfg(widgets_unstripped)]
    {
        backend_for_kind(widget::WidgetKind::FontDialog)
            .create_font_dialog(parent, "", x, y, width, height)
    }
    #[cfg(not(widgets_unstripped))]
    {
        let _ = (parent, x, y, width, height);
        NO_SUCH_CONTROL
    }
}
#[cfg(not(alloc_frugal))]
/// Creates a spin box. The backend chooses how it is hosted.
pub fn create_spin_box(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::SpinBox).create_spin_box(parent, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a list view. The backend chooses how it is hosted.
///
/// Returns `0` on a profile that does not ship the item view.
pub fn create_list_view(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    #[cfg(widgets_unstripped)]
    {
        backend_for_kind(widget::WidgetKind::ListView).create_list_view(parent, x, y, width, height)
    }
    #[cfg(not(widgets_unstripped))]
    {
        let _ = (parent, x, y, width, height);
        NO_SUCH_CONTROL
    }
}
/// Creates a scroll area. The backend chooses how it is hosted.
#[cfg(not(alloc_frugal))]
pub fn create_scroll_area(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::ScrollArea).create_scroll_area(parent, x, y, width, height)
}
/// Mounts a widget onto a surface supplied by the host.
///
/// The host platform supplies a window and a drawing surface; the library paints
/// the widget through [`widget::Draw`]. Which surface hosts it — a child window,
/// a drawing area, a view — is decided inside `src/platform/` and never named
/// here; callers only need to know whether the backend can host widgets, which
/// [`supports_surfaces`] answers.
///
/// `id` must already be registered in `widget::runtime`. Prefer the
/// higher-level `app::WindowHandle::mount_surface`, which performs the
/// registration for you and reports failures as a `Result`.
///
/// Returns `false` when the backend has no surface to offer, or when it refuses
/// this particular mount. Backends that cannot host widgets log why.
#[cfg(not(alloc_frugal))]
pub fn mount_surface(
    parent: crate::core::ObjectId,
    id: crate::core::ObjectId,
    rect: crate::core::Rect,
) -> bool {
    platform::get_platform().mount_surface(parent, id, rect)
}

/// Moves and resizes a mounted surface.
#[cfg(not(alloc_frugal))]
pub fn resize_surface(id: crate::core::ObjectId, rect: crate::core::Rect) -> bool {
    platform::get_platform().resize_surface(id, rect)
}

/// Unmounts a surface from its window.
#[cfg(not(alloc_frugal))]
pub fn unmount_surface(id: crate::core::ObjectId) -> bool {
    platform::get_platform().unmount_surface(id)
}

/// Marks a mounted surface as needing a repaint.
///
/// Returns `false` when the id is not a surface mounted on the active backend.
#[cfg(not(alloc_frugal))]
pub fn invalidate_surface(id: crate::core::ObjectId) -> bool {
    platform::get_platform().invalidate_surface(id)
}

/// Asks the platform to repaint one rectangle of a mounted widget.
///
/// Returns `true` when the backend narrowed the repaint to `rect`. `false` — including
/// from a backend that never implemented this — means the caller should fall back to
/// [`invalidate_surface`], which repaints the whole widget. Backends are not required
/// to implement it, because several window toolkits offer only whole-widget
/// invalidation.
#[cfg(not(alloc_frugal))]
pub fn invalidate_surface_rect(id: crate::core::ObjectId, rect: crate::core::Rect) -> bool {
    platform::get_platform().invalidate_surface_rect(id, rect)
}

/// Asks the platform to repaint one rectangle, or reports that it cannot.
///
/// The alloc-frugal profile has no platform trait to forward to, so this reports
/// `false` and the caller invalidates the whole surface instead — which is what that
/// profile would have done anyway.
#[cfg(alloc_frugal)]
pub fn invalidate_surface_rect(_id: crate::core::ObjectId, _rect: crate::core::Rect) -> bool {
    false
}

/// Returns `true` when the active backend can host library-painted widgets.
#[cfg(not(alloc_frugal))]
pub fn supports_surfaces() -> bool {
    platform::get_platform().supports_surfaces()
}

/// Mounts a widget object on the platform's surface, or reports why it cannot.
///
/// This is the **single implementation** of the register → mount → roll back on
/// failure sequence, used by both [`create_widget_of_kind`] and
/// [`app::WindowHandle::mount_surface`]. Keeping one
/// copy is what stops the two paths from disagreeing about ownership: on any
/// failure the widget is unregistered, so the registry never holds a widget the
/// backend is not showing.
///
/// # Resolving the parent
///
/// `parent` may be either a **window widget** (what [`create_window`] returns and
/// what `App::new_window` hands out) or a **host window** (what
/// `Platform::create_window` returns). `mount_surface` is a `Platform` method, so
/// it can only resolve the latter — the platform knows nothing of the widget
/// registry. Translating here, at the one place that already owns both the widget
/// registry and the platform handle, is what lets a caller mount controls on the
/// window it created. Without it the library's own windows could not carry the
/// library's own controls, because the two id spaces never met.
///
/// Applies the active theme to a widget that is about to be registered.
///
/// # Why this shim exists at the crate root
///
/// Both creation funnels (`mount_widget_object` here and
/// `CustomPaintControlBackend::mount_widget_of_kind`) must apply the theme, and
/// neither may carry its own `#[cfg]` — a call-site gate is exactly where the
/// condition drifts away from the module's own gate. That drift broke the
/// `embedded` build twice: once for the JSON loader, once here.
///
/// So the call is unconditional and the *body* is gated, which means the condition
/// is written once, next to the module it describes.
#[cfg(not(alloc_frugal))]
fn apply_active_theme(widget: &mut Box<dyn widget::Widget>) {
    #[cfg(device_profile)]
    {
        crate::theme::apply_active_theme(&mut **widget);
    }
    #[cfg(not(device_profile))]
    {
        // No theme module in this profile, so there is nothing to apply. Naming the
        // parameter keeps the signature identical in every build.
        let _ = widget;
    }
}

/// Applies the active theme to every widget that already exists.
///
/// # Why a sweep is needed
///
/// A theme switch is a global change: applying it only inside the creation funnels
/// would mean the controls on screen keep the previous palette until they are
/// rebuilt. This walks the live registry and re-runs the same merge a newly created
/// control gets, so an explicit per-control style still wins (see `theme::apply`).
///
/// A profile without a theme module has nothing to apply and does nothing.
#[cfg(not(alloc_frugal))]
pub fn reapply_active_theme() {
    #[cfg(device_profile)]
    {
        widget::runtime::for_each_mounted_widget(|_id, widget| {
            crate::theme::apply_active_theme(widget);
        });
    }
}

/// Returns `Ok(id)` with the widget live in the registry, or `Err(reason)`.
#[cfg(not(alloc_frugal))]
fn mount_widget_object(
    parent: crate::core::ObjectId,
    mut widget: Box<dyn widget::Widget>,
    rect: crate::core::Rect,
) -> Result<crate::core::ObjectId, widget::runtime::SurfaceMountError> {
    use widget::runtime::SurfaceMountError;

    // Apply the active theme before the widget becomes visible. This is one of the
    // two funnels every created control passes through (the other is
    // `CustomPaintControlBackend::mount_widget_of_kind`), which is what makes a
    // theme switch affect controls created through the C ABI and not only those the
    // JSON loader builds. An explicit style the caller or the constructor already
    // set is preserved; see `theme::apply`.
    apply_active_theme(&mut widget);

    // Register first: the backend looks the widget up by id on every repaint.
    let id = widget::runtime::register(widget).ok_or(SurfaceMountError::NoRegistryOnThread)?;
    widget::runtime::set_geometry(id, rect);

    let host_parent = widget::runtime::host_window_for(parent).unwrap_or(parent);

    if !platform::get_platform().mount_surface(host_parent, id, rect) {
        // Do not leave a widget stranded when the backend refused to show it.
        widget::runtime::unregister(id);
        if !supports_surfaces() {
            return Err(SurfaceMountError::UnsupportedByBackend(backend_name()));
        }
        return Err(SurfaceMountError::RejectedByBackend(backend_name()));
    }
    Ok(id)
}

/// Creates a widget of any kind — **the single, mechanism-free creation entry**.
///
/// # How this differs from the per-kind `create_*` functions
///
/// The `create_*` functions are the typo-safe spelling for the common widgets
/// (`create_button`, `create_slider`, …). This function is the general form: hand
/// Creates a widget of `kind`, painted by the library.
///
/// # What this does now
///
/// Every kind is painted by the library (BLUE15 rule #55), so there is a single
/// path: build (or accept) the `Box<dyn Widget>` and hand it to the host surface
/// through `widget::runtime`. The `Platform` layer supplies a surface to paint on,
/// not a control to map onto, so no branch here asks which mechanism to use.
///
/// # The `widget` argument
///
/// A caller may supply a pre-built object so that a widget with constructor
/// arguments the factory cannot express (`CodeEditor` with initial text, a chart
/// with series) reaches this path without a bespoke branch. When it is `None` the
/// widget factory is asked; that factory is the single place that knows every
/// kind's constructor (rule #65).
///
/// Returns `0` — a truthful "not created" — when the kind has no constructor and
/// the caller supplied no object. Callers already handle `0` for the per-kind
/// functions, so this needs no separate error channel.
#[cfg(not(alloc_frugal))]
pub fn create_widget_of_kind(
    kind: widget::WidgetKind,
    parent: crate::core::ObjectId,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    widget: Option<Box<dyn widget::Widget>>,
) -> crate::core::ObjectId {
    let rect = crate::core::Rect::new(x, y, width, height);

    // Prefer the caller's object; otherwise build from the factory, which is the
    // only component that knows every kind's constructor.
    //
    // The gate is `full_widgets`, not `device_profile`: the factory and
    // `kind_name` both require the registry, which exists only when a device
    // profile is present *and* the widgets were not stripped. Spelling it
    // `device_profile` made the callee narrower than its caller, so a build that
    // overlapped two axis-1 features (`--features "tablet,embedded"`) failed with
    // `cannot find function kind_name` / `cannot find WidgetFactory`. `full_widgets`
    // is exactly the conjunction the body needs, so there is no narrower gate left
    // for this call site to drift away from (principle #47).
    let widget = widget.or_else(|| {
        #[cfg(all(device_profile, full_widgets))]
        {
            widget::WidgetFactory::new_with_defaults().create(&kind_name(kind), rect, text)
        }
        #[cfg(not(all(device_profile, full_widgets)))]
        {
            let _ = (text, rect);
            None
        }
    });

    let Some(widget) = widget else {
        log::warn!(
            "create_widget_of_kind: no constructor for {kind:?} and no widget object was \
             supplied; returning 0"
        );
        return 0;
    };

    match mount_widget_object(parent, widget, rect) {
        Ok(id) => id,
        Err(error) => {
            log::warn!("create_widget_of_kind: cannot host {kind:?}: {error}");
            0
        }
    }
}

/// The canonical factory name for a [`WidgetKind`].
///
/// Derived from the widget capability registry (the same table the factory
/// dispatches on) rather than from `Debug` output, so the two can never disagree.
/// Only the registry-backed arm exists where the registry does; every other build
/// derives the name from the variant, which is the same convention the registry
/// uses.
#[cfg(all(not(alloc_frugal), full_widgets))]
fn kind_name(kind: widget::WidgetKind) -> alloc::string::String {
    use alloc::string::ToString;
    if let Some(capability) = widget::WidgetFactory::new_with_defaults().capability_by_kind(kind) {
        return capability.canonical_name.to_string();
    }
    widget::capability::factory_name_for_kind(kind).to_string()
}

// A second `kind_name` used to live here, gated `not(full_widgets)` and marked
// `#[allow(dead_code)]`. Its only caller is gated `device_profile`, and `build.rs`
// derives `full_widgets` and `device_profile` from the same `has_profile`
// predicate, so the two gates could never both hold: the ~20-line `Debug`-derived
// snake_case converter was unreachable in *every* configuration, and the allow was
// hiding that rather than expressing a `cfg`-gated keep-alive. Deleted, because a
// registry-backed arm that cannot be called is not a fallback (principle #4).

/// Stub for mini mode (no platform runtime, no windows).
#[cfg(alloc_frugal)]
pub fn mount_surface(
    _parent: crate::core::ObjectId,
    _id: crate::core::ObjectId,
    _rect: crate::core::Rect,
) -> bool {
    false
}

/// Stub for mini mode.
#[cfg(alloc_frugal)]
pub fn resize_surface(_id: crate::core::ObjectId, _rect: crate::core::Rect) -> bool {
    false
}

/// Stub for mini mode.
#[cfg(alloc_frugal)]
pub fn unmount_surface(_id: crate::core::ObjectId) -> bool {
    false
}

/// Stub for mini mode.
#[cfg(alloc_frugal)]
pub fn invalidate_surface(_id: crate::core::ObjectId) -> bool {
    false
}

/// Stub for mini mode.
#[cfg(alloc_frugal)]
pub fn supports_surfaces() -> bool {
    false
}

/// Show a widget by its object id.
///
/// Routed through the control backend, which is the one mechanism that owns the
/// control (BLUE15 #55): the platform supplies the surface, not the control.
#[cfg(not(alloc_frugal))]
pub fn show_widget(widget_id: crate::core::ObjectId) {
    control_backend::get_control_backend().set_widget_visible(widget_id, true);
}
/// Hide a widget by its object id.
#[cfg(not(alloc_frugal))]
pub fn hide_widget(widget_id: crate::core::ObjectId) {
    control_backend::get_control_backend().set_widget_visible(widget_id, false);
}
/// Set geometry of a widget.
#[cfg(not(alloc_frugal))]
pub fn set_widget_geometry(
    widget_id: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) {
    control_backend::get_control_backend().set_widget_geometry(widget_id, x, y, width, height);
}
/// Read a widget's rectangle, or `None` when the id addresses nothing.
#[cfg(not(alloc_frugal))]
pub fn widget_geometry(widget_id: crate::core::ObjectId) -> Option<(i32, i32, u32, u32)> {
    control_backend::get_control_backend().get_widget_geometry(widget_id)
}
/// Set text of a widget.
#[cfg(not(alloc_frugal))]
pub fn set_widget_text(widget_id: crate::core::ObjectId, text: &str) {
    control_backend::get_control_backend().set_widget_text(widget_id, text);
}
/// Get text of a widget.
#[cfg(not(alloc_frugal))]
pub fn get_widget_text(widget_id: crate::core::ObjectId) -> String {
    control_backend::get_control_backend().get_widget_text(widget_id)
}
/// Set enabled state of a widget.
#[cfg(not(alloc_frugal))]
pub fn set_widget_enabled(widget_id: crate::core::ObjectId, enabled: bool) {
    control_backend::get_control_backend().set_widget_enabled(widget_id, enabled);
}
/// Check if a widget is enabled.
#[cfg(not(alloc_frugal))]
pub fn is_widget_enabled(widget_id: crate::core::ObjectId) -> bool {
    control_backend::get_control_backend().is_widget_enabled(widget_id)
}
/// Set visibility of a widget.
#[cfg(not(alloc_frugal))]
pub fn set_widget_visible(widget_id: crate::core::ObjectId, visible: bool) {
    control_backend::get_control_backend().set_widget_visible(widget_id, visible);
}
/// Check if a widget is visible.
#[cfg(not(alloc_frugal))]
pub fn is_widget_visible(widget_id: crate::core::ObjectId) -> bool {
    control_backend::get_control_backend().is_widget_visible(widget_id)
}

/// Set a widget's primary numeric value (slider, progress bar, spin box, ...).
///
/// Returns `false` when the widget exposes no numeric value, so callers never
/// mistake "unsupported" for "set to 0".
#[cfg(not(alloc_frugal))]
pub fn set_widget_value(widget_id: crate::core::ObjectId, value: f64) -> bool {
    widget::capability::widget_access::write_number(widget_id, &["value"], value)
}

/// Read a widget's primary numeric value.
#[cfg(not(alloc_frugal))]
pub fn widget_value(widget_id: crate::core::ObjectId) -> Option<f64> {
    widget::capability::widget_access::read_number(widget_id, &["value"])
}

/// Set a widget's `(min, max)` range.
#[cfg(not(alloc_frugal))]
pub fn set_widget_range(widget_id: crate::core::ObjectId, min: f64, max: f64) -> bool {
    let names: &[&str] = &["minimum", "min", "min_value"];
    let max_names: &[&str] = &["maximum", "max", "max_value"];
    widget::capability::widget_access::write_number(widget_id, names, min)
        && widget::capability::widget_access::write_number(widget_id, max_names, max)
}

/// Read a widget's `(min, max)` range.
#[cfg(not(alloc_frugal))]
pub fn widget_range(widget_id: crate::core::ObjectId) -> Option<(f64, f64)> {
    let min = widget::capability::widget_access::read_number(
        widget_id,
        &["minimum", "min", "min_value"],
    )?;
    let max = widget::capability::widget_access::read_number(
        widget_id,
        &["maximum", "max", "max_value"],
    )?;
    Some((min, max))
}

/// Set a widget's selection index (combo box, list box, tab widget).
///
/// `None` clears the selection, which is what these controls store as `Null` —
/// distinct from selecting index 0.
#[cfg(not(alloc_frugal))]
pub fn set_widget_selected_index(widget_id: crate::core::ObjectId, index: Option<usize>) -> bool {
    let names: &[&str] = &["selected_index", "current_index", "current_row", "active_index"];
    let value = match index {
        Some(index) => crate::widget::capability::CapabilityValue::UInt(index as u64),
        None => crate::widget::capability::CapabilityValue::Null,
    };
    widget::capability::widget_access::write_first(widget_id, names, &value)
}

/// Read a widget's selection index.
#[cfg(not(alloc_frugal))]
pub fn widget_selected_index(widget_id: crate::core::ObjectId) -> Option<usize> {
    widget::capability::widget_access::read_index(
        widget_id,
        &["selected_index", "current_index", "current_row", "active_index"],
    )
}

/// Set a widget's checked state (check box, radio button, toggle button).
#[cfg(not(alloc_frugal))]
pub fn set_widget_checked(widget_id: crate::core::ObjectId, checked: bool) -> bool {
    widget::capability::widget_access::write_first(
        widget_id,
        &["checked"],
        &crate::widget::capability::CapabilityValue::Bool(checked),
    )
}

/// Read a widget's checked state, or `None` when it is not checkable.
#[cfg(not(alloc_frugal))]
pub fn is_widget_checked(widget_id: crate::core::ObjectId) -> Option<bool> {
    widget::capability::widget_access::read_flag(widget_id, &["checked"])
}

/// Set a widget's increment step (slider, spin box, scroll bar).
#[cfg(not(alloc_frugal))]
pub fn set_widget_step(widget_id: crate::core::ObjectId, step: f64) -> bool {
    widget::capability::widget_access::write_number(widget_id, &["single_step", "step"], step)
}

/// Read a widget's increment step.
#[cfg(not(alloc_frugal))]
pub fn widget_step(widget_id: crate::core::ObjectId) -> Option<f64> {
    widget::capability::widget_access::read_number(widget_id, &["single_step", "step"])
}

/// Set a progress-style widget's indeterminate (busy) state.
#[cfg(not(alloc_frugal))]
pub fn set_widget_indeterminate(widget_id: crate::core::ObjectId, indeterminate: bool) -> bool {
    widget::capability::widget_access::write_first(
        widget_id,
        &["indeterminate"],
        &crate::widget::capability::CapabilityValue::Bool(indeterminate),
    )
}

/// Read a progress-style widget's indeterminate state.
#[cfg(not(alloc_frugal))]
pub fn is_widget_indeterminate(widget_id: crate::core::ObjectId) -> Option<bool> {
    widget::capability::widget_access::read_flag(widget_id, &["indeterminate"])
}

/// Set a text-entry widget's read-only state.
#[cfg(not(alloc_frugal))]
pub fn set_widget_read_only(widget_id: crate::core::ObjectId, read_only: bool) -> bool {
    widget::capability::widget_access::write_first(
        widget_id,
        &["read_only"],
        &crate::widget::capability::CapabilityValue::Bool(read_only),
    )
}

/// Read a text-entry widget's read-only state.
#[cfg(not(alloc_frugal))]
pub fn is_widget_read_only(widget_id: crate::core::ObjectId) -> Option<bool> {
    widget::capability::widget_access::read_flag(widget_id, &["read_only"])
}

/// Set a text-entry widget's maximum accepted length.
#[cfg(not(alloc_frugal))]
pub fn set_widget_max_length(widget_id: crate::core::ObjectId, max_length: u32) -> bool {
    widget::capability::widget_access::write_first(
        widget_id,
        &["max_length"],
        &crate::widget::capability::CapabilityValue::UInt(max_length as u64),
    )
}

/// Read a text-entry widget's maximum accepted length.
#[cfg(not(alloc_frugal))]
pub fn widget_max_length(widget_id: crate::core::ObjectId) -> Option<u32> {
    widget::capability::widget_access::read_index(widget_id, &["max_length"])
        .and_then(|v| u32::try_from(v).ok())
}

/// Apply or clear a window state (maximised, minimised, full-screen, ...).
///
/// Returns `false` when the id is not a window or the backend cannot honour the
/// state on its toolkit.
#[cfg(not(alloc_frugal))]
pub fn set_window_state(
    widget_id: crate::core::ObjectId,
    flag: platform::WindowStateFlag,
    on: bool,
) -> bool {
    platform::get_platform().set_window_state(widget_id, flag, on)
}

/// Read a window state, or `None` when the id is not a window.
#[cfg(not(alloc_frugal))]
pub fn is_window_in_state(
    widget_id: crate::core::ObjectId,
    flag: platform::WindowStateFlag,
) -> Option<bool> {
    platform::get_platform().is_window_in_state(widget_id, flag)
}

/// Set a window's minimum content size.
#[cfg(not(alloc_frugal))]
pub fn set_window_min_size(widget_id: crate::core::ObjectId, width: u32, height: u32) -> bool {
    platform::get_platform().set_window_min_size(widget_id, width, height)
}

/// Read a window's minimum content size.
#[cfg(not(alloc_frugal))]
pub fn window_min_size(widget_id: crate::core::ObjectId) -> Option<(u32, u32)> {
    platform::get_platform().window_min_size(widget_id)
}

/// Set a window's icon from a file path.
#[cfg(not(alloc_frugal))]
pub fn set_window_icon(widget_id: crate::core::ObjectId, path: &str) -> bool {
    platform::get_platform().set_window_icon(widget_id, path)
}

/// Read a window's icon path, if one was set.
#[cfg(not(alloc_frugal))]
pub fn window_icon(widget_id: crate::core::ObjectId) -> Option<String> {
    platform::get_platform().window_icon(widget_id)
}

/// Set a text entry's selection range as `(start, end)` character offsets.
///
/// # Current support
///
/// No control publishes a selection range as a property (the text controls track
/// it internally and expose `cursor_position`), so this reports `false` rather
/// than pretending the range was applied. Routing it through the property
/// contract means publishing the property on the text controls is the only change
/// needed to make it work.
#[cfg(not(alloc_frugal))]
pub fn set_widget_selection(widget_id: crate::core::ObjectId, start: u32, end: u32) -> bool {
    let names: &[&str] = &["selection"];
    // The pair travels as the historical "start,end" spelling used by the text
    // controls' own setter, so a control that publishes `selection` parses one
    // string rather than needing two properties kept in step.
    widget::capability::widget_access::write_first(
        widget_id,
        names,
        &crate::widget::capability::CapabilityValue::String(format!("{start},{end}")),
    )
}

/// Read a text entry's selection range, or `None` when nothing is selected.
#[cfg(not(alloc_frugal))]
pub fn widget_selection(widget_id: crate::core::ObjectId) -> Option<(u32, u32)> {
    let raw = widget::capability::widget_access::read_text(widget_id, &["selection"])?;
    let (start, end) = raw.split_once(',')?;
    Some((start.trim().parse().ok()?, end.trim().parse().ok()?))
}

/// Set a text entry's placeholder (cue) text.
#[cfg(not(alloc_frugal))]
pub fn set_widget_placeholder(widget_id: crate::core::ObjectId, text: &str) -> bool {
    widget::capability::widget_access::write_first(
        widget_id,
        &["placeholder_text", "placeholder"],
        &crate::widget::capability::CapabilityValue::String(text.to_string()),
    )
}

/// Read a text entry's placeholder text.
#[cfg(not(alloc_frugal))]
pub fn widget_placeholder(widget_id: crate::core::ObjectId) -> Option<String> {
    widget::capability::widget_access::read_text(widget_id, &["placeholder_text", "placeholder"])
}

/// Set a text entry's echo mode.
///
/// # Current support
///
/// No control publishes an echo property yet, so this reports `false` on every
/// widget rather than pretending the mode was applied. It is routed through the
/// property contract — the place a control would express it — so that adding the
/// property to the text controls is the only change needed to make it work.
#[cfg(not(alloc_frugal))]
pub fn set_widget_echo_mode(widget_id: crate::core::ObjectId, mode: platform::EchoMode) -> bool {
    let token = match mode {
        platform::EchoMode::Normal => "normal",
        platform::EchoMode::Password => "password",
        platform::EchoMode::NoEcho => "no_echo",
    };
    widget::capability::widget_access::write_first(
        widget_id,
        &["echo_mode"],
        &crate::widget::capability::CapabilityValue::String(token.to_string()),
    )
}

/// Read a text entry's echo mode.
#[cfg(not(alloc_frugal))]
pub fn widget_echo_mode(widget_id: crate::core::ObjectId) -> Option<platform::EchoMode> {
    match widget::capability::widget_access::read_text(widget_id, &["echo_mode"])?.as_str() {
        "normal" => Some(platform::EchoMode::Normal),
        "password" => Some(platform::EchoMode::Password),
        "no_echo" => Some(platform::EchoMode::NoEcho),
        _ => None,
    }
}

/// Apply a slider's creation-time orientation.
#[cfg(not(alloc_frugal))]
pub fn set_slider_orientation(
    widget_id: crate::core::ObjectId,
    orientation: crate::core::Orientation,
) -> bool {
    widget::capability::widget_access::write_first(
        widget_id,
        &["orientation"],
        &crate::widget::capability::CapabilityValue::String(
            widget::capability::orientation_to_str(orientation).to_string(),
        ),
    )
}

/// Read a slider's orientation.
#[cfg(not(alloc_frugal))]
pub fn slider_orientation(widget_id: crate::core::ObjectId) -> Option<crate::core::Orientation> {
    let token = widget::capability::widget_access::read_text(widget_id, &["orientation"])?;
    widget::capability::expect_orientation(crate::widget::capability::CapabilityValue::String(
        token,
    ))
    .ok()
}

/// Set a checkable control's tri-state mode.
#[cfg(not(alloc_frugal))]
pub fn set_widget_tristate(widget_id: crate::core::ObjectId, enabled: bool) -> bool {
    widget::capability::widget_access::write_first(
        widget_id,
        &["tristate_enabled"],
        &crate::widget::capability::CapabilityValue::Bool(enabled),
    )
}

/// Read a checkable control's tri-state mode.
#[cfg(not(alloc_frugal))]
pub fn is_widget_tristate(widget_id: crate::core::ObjectId) -> Option<bool> {
    widget::capability::widget_access::read_flag(widget_id, &["tristate_enabled"])
}

/// Put a radio button into a named mutually-exclusive group.
///
/// An empty group clears the membership, which these controls store as `Null`.
#[cfg(not(alloc_frugal))]
pub fn set_widget_group(widget_id: crate::core::ObjectId, group: &str) -> bool {
    let value = if group.is_empty() {
        crate::widget::capability::CapabilityValue::Null
    } else {
        crate::widget::capability::CapabilityValue::String(group.to_string())
    };
    widget::capability::widget_access::write_first(widget_id, &["group_id", "group"], &value)
}

/// Read a radio button's group name.
#[cfg(not(alloc_frugal))]
pub fn widget_group(widget_id: crate::core::ObjectId) -> Option<String> {
    widget::capability::widget_access::read_text(widget_id, &["group_id", "group"])
}

/// Set a scrollable container's scroll offset.
#[cfg(not(alloc_frugal))]
pub fn set_widget_scroll_position(widget_id: crate::core::ObjectId, x: i32, y: i32) -> bool {
    let x_ok = widget::capability::widget_access::write_number(
        widget_id,
        &["scroll_position_x"],
        x as f64,
    );
    let y_ok = widget::capability::widget_access::write_number(
        widget_id,
        &["scroll_position_y"],
        y as f64,
    );
    x_ok && y_ok
}

/// Read a scrollable container's scroll offset.
#[cfg(not(alloc_frugal))]
pub fn widget_scroll_position(widget_id: crate::core::ObjectId) -> Option<(i32, i32)> {
    let x = widget::capability::widget_access::read_number(widget_id, &["scroll_position_x"])?;
    let y = widget::capability::widget_access::read_number(widget_id, &["scroll_position_y"])?;
    Some((x as i32, y as i32))
}
// ComboBox operations
/// Appends an item with the given text to a combo box.
///
/// Returns `false` when the backend refuses — which includes being handed an id
/// that is not a combo box.
#[cfg(not(alloc_frugal))]
pub fn combo_box_add_item(combo_box: crate::core::ObjectId, text: &str) -> bool {
    control_backend::get_control_backend().combo_box_add_item(combo_box, text)
}
/// Removes every item from a combo box, leaving it empty and with nothing
/// selected. Returns `false` when the backend refuses.
#[cfg(not(alloc_frugal))]
pub fn combo_box_clear_items(combo_box: crate::core::ObjectId) -> bool {
    control_backend::get_control_backend().combo_box_clear_items(combo_box)
}
/// Selects the combo box item at `index`.
///
/// Routed through the property contract as the `current_index` property, so this
/// reports `false` when the widget does not publish that property. An
/// out-of-range `index` is **not** reliably reported as a failure — read
/// [`combo_box_current_index`] back to confirm what took effect.
#[cfg(not(alloc_frugal))]
pub fn combo_box_set_current_index(combo_box: crate::core::ObjectId, index: usize) -> bool {
    widget::capability::widget_access::write_first(
        combo_box,
        &["current_index"],
        &crate::widget::capability::CapabilityValue::UInt(index as u64),
    )
}
/// The combo box's selected index, or `None` when nothing is selected or the id
/// is unknown.
#[cfg(not(alloc_frugal))]
pub fn combo_box_current_index(combo_box: crate::core::ObjectId) -> Option<usize> {
    widget::capability::widget_access::read_index(combo_box, &["current_index"])
}
/// How many items a combo box holds.
///
/// Reports `0` for an unknown id as well as for a genuinely empty list, so it
/// cannot be used to tell "empty" from "not a combo box".
#[cfg(not(alloc_frugal))]
pub fn combo_box_item_count(combo_box: crate::core::ObjectId) -> usize {
    widget::capability::widget_access::read_index(combo_box, &["item_count"]).unwrap_or(0)
}
/// The text of the combo box item at `index`, or `None` when the index is out of
/// range or the id is unknown.
#[cfg(not(alloc_frugal))]
pub fn combo_box_item_text(combo_box: crate::core::ObjectId, index: usize) -> Option<String> {
    control_backend::get_control_backend().combo_box_item_text(combo_box, index)
}
// ListBox operations
/// Appends an item with the given text to a list box. Returns `false` when the
/// backend refuses.
#[cfg(not(alloc_frugal))]
pub fn list_box_add_item(list_box: crate::core::ObjectId, text: &str) -> bool {
    control_backend::get_control_backend().list_box_add_item(list_box, text)
}
/// Removes the list box item at `index`, shifting later items down.
///
/// Returns `false` when the backend refuses. As with the combo box, an
/// out-of-range index is not reliably distinguished from a refusal — re-read
/// [`list_box_item_count`] to check.
#[cfg(not(alloc_frugal))]
pub fn list_box_remove_item(list_box: crate::core::ObjectId, index: usize) -> bool {
    control_backend::get_control_backend().list_box_remove_item(list_box, index)
}
/// Removes every item from a list box. Returns `false` when the backend refuses.
#[cfg(not(alloc_frugal))]
pub fn list_box_clear_items(list_box: crate::core::ObjectId) -> bool {
    control_backend::get_control_backend().list_box_clear_items(list_box)
}
/// Selects the list box item at `index`.
///
/// Tries the `current_row` property first and falls back to `current_index`,
/// because both spellings are published by list controls in this library.
/// Returns `false` when neither is published; an out-of-range `index` is not
/// reliably reported, so read [`list_box_current_index`] back to confirm.
#[cfg(not(alloc_frugal))]
pub fn list_box_set_current_index(list_box: crate::core::ObjectId, index: usize) -> bool {
    widget::capability::widget_access::write_first(
        list_box,
        &["current_row", "current_index"],
        &crate::widget::capability::CapabilityValue::UInt(index as u64),
    )
}
/// The list box's selected index, or `None` when nothing is selected or the id
/// is unknown.
#[cfg(not(alloc_frugal))]
pub fn list_box_current_index(list_box: crate::core::ObjectId) -> Option<usize> {
    widget::capability::widget_access::read_index(list_box, &["current_row", "current_index"])
}
/// How many items a list box holds. Reports `0` for an unknown id as well as for
/// an empty list.
#[cfg(not(alloc_frugal))]
pub fn list_box_item_count(list_box: crate::core::ObjectId) -> usize {
    widget::capability::widget_access::read_index(list_box, &["item_count"]).unwrap_or(0)
}
/// The text of the list box item at `index`, or `None` when the index is out of
/// range or the id is unknown.
#[cfg(not(alloc_frugal))]
pub fn list_box_item_text(list_box: crate::core::ObjectId, index: usize) -> Option<String> {
    control_backend::get_control_backend().list_box_item_text(list_box, index)
}
// Event polling
/// Takes the id of the next widget that has been activated, or `None` when no
/// activation is queued.
///
/// This is a queue, not a state: each successful call removes one event, so a
/// loop calling it until `None` drains the pending activations. Use
/// [`poll_widget_trigger_event`] when the kind of activation matters.
#[cfg(not(alloc_frugal))]
pub fn poll_widget_triggered() -> Option<crate::core::ObjectId> {
    control_backend::get_control_backend().poll_widget_triggered()
}
/// Takes the next queued widget activation together with what kind it was, or
/// `None` when the queue is empty. Drains the same queue as
/// [`poll_widget_triggered`].
#[cfg(not(alloc_frugal))]
pub fn poll_widget_trigger_event() -> Option<WidgetTriggerEvent> {
    control_backend::get_control_backend().poll_widget_trigger_event()
}

/// Delivers every queued widget trigger, in order, until the queue is empty.
///
/// Returns the number of events consumed.
///
/// # Why this lives at the crate root rather than in `crate::app`
///
/// Every platform backend's event loop needs to call it — GTK's timeout, the Win32
/// message pass, Wayland's idle step, Android's polling loop — and `crate::app` is gated
/// `full_widgets` while the trigger queue is available to every profile. A backend that
/// called `crate::app::drain_triggers` therefore compiled on `desktop` and failed on
/// `mini` with `cannot find 'app' in 'crate'`, which is the same class of defect as a call
/// into a gated `create_*`: one unpicked profile hides it completely
/// (`tools/check_profiles.sh` runs the profile matrix; a `desktop` build never sees it).
///
/// The trigger queue is not a widget-set feature — it is how a backend reports "the user
/// resized the window" — so the drain sits beside the queue it drains, at the widest gate
/// its dependency has.
///
/// # Why it is defined in every profile, not gated
///
/// The queue exists in every profile except `alloc_frugal` (`mini`). Gating the function
/// would mean gating each of the **eleven** backend call sites, and a gate that has to be
/// repeated eleven times is a gate that will be forgotten in one of them — which is
/// exactly how this function came to be missing from the crate root in the first place.
/// Defining it unconditionally moves the condition to one place, and `alloc_frugal`
/// answers `0` because there is genuinely nothing to drain there.
///
/// # Termination
///
/// Drains until the queue reports empty, so a tick's cost is bounded by the number of
/// events that arrived since the last tick. A backend that produced events faster than it
/// consumed them would starve its own loop, which is why the queue is drained to empty
/// rather than to a fixed count: "drained" is the only state from which the next tick's
/// backlog is knowable.
pub fn drain_triggers() -> usize {
    #[cfg(alloc_frugal)]
    {
        // This profile has no trigger queue, so there is nothing queued and nothing to
        // deliver. `0` is the truthful count, not a failure.
        0
    }
    #[cfg(not(alloc_frugal))]
    {
        let mut dispatched = 0usize;
        while let Some(event) = poll_widget_trigger_event() {
            #[cfg(full_widgets)]
            {
                app::dispatch_trigger(event.widget_id, event.kind);
            }
            #[cfg(not(full_widgets))]
            {
                // No router in this profile. Reading the event above is what removes it
                // from the queue; naming it keeps the binding exercised in every build.
                let _ = (event.widget_id, event.kind);
            }
            dispatched += 1;
        }
        dispatched
    }
}
/// Queues an activation of `widget_id` as if the user had performed it, for
/// tests and for driving the UI from outside the event loop.
///
/// Returns `false` when the backend will not accept the injected event, in
/// which case nothing is queued and no later poll will report it.
#[cfg(not(alloc_frugal))]
pub fn inject_widget_trigger_event(
    widget_id: crate::core::ObjectId,
    kind: WidgetTriggerKind,
) -> bool {
    control_backend::get_control_backend().inject_widget_trigger_event(widget_id, kind)
}
/// Reports that a container's client area became `width` by `height`, and queues a
/// `Resized` trigger so a host re-runs its layout.
///
/// # Who calls this
///
/// The platform backends call it from their window-resize callback (GTK
/// `size-allocate`, Win32 `WM_SIZE`, AppKit `windowDidResize:`), and a host that owns
/// its event loop may call it for a resize it learns about by other means. It is the
/// one entry point for "the user resized this window", which no `set_widget_geometry`
/// call covers — the program was not the one changing the size.
///
/// The reported size is retained (see [`window_client_size`]) and a
/// [`WidgetTriggerKind::Resized`] event is queued. The two are deliberately separate:
/// the event is consumed by polling, while the size keeps answering afterwards.
///
/// Returns `false` when `window_id` addresses nothing, so a stale id cannot inject a
/// phantom resize.
#[cfg(not(alloc_frugal))]
pub fn queue_resize_trigger(window_id: crate::core::ObjectId, width: u32, height: u32) -> bool {
    control_backend::get_control_backend().queue_resize_trigger(window_id, width, height)
}
/// Reports a container resize in an allocation-frugal (`mini`) build.
///
/// The signature is identical to the full build's (rule #21: a profile change must
/// not change the API). `mini` runs without a platform singleton, so there is no
/// window whose resize could be observed and nothing to queue — answering `false`
/// reports that honestly instead of accepting a resize that could never be delivered.
#[cfg(alloc_frugal)]
pub fn queue_resize_trigger(window_id: crate::core::ObjectId, width: u32, height: u32) -> bool {
    let _ = (window_id, width, height);
    false
}
/// The client size last reported for `window_id`, or `None` when none was.
///
/// The readback half of [`queue_resize_trigger`]: a host that receives a `Resized`
/// event learns *which* window changed, and asks here *how big* it now is. Falls back
/// to the window's current geometry, which is what it still has if the user has never
/// resized it.
#[cfg(not(alloc_frugal))]
pub fn window_client_size(window_id: crate::core::ObjectId) -> Option<(u32, u32)> {
    control_backend::get_control_backend().window_client_size(window_id)
}
/// The last reported client size in an allocation-frugal (`mini`) build.
///
/// Always `None`: the same signature as the full build, but a `mini` build keeps no
/// window records, so no size was ever reported to answer with.
#[cfg(alloc_frugal)]
pub fn window_client_size(window_id: crate::core::ObjectId) -> Option<(u32, u32)> {
    let _ = window_id;
    None
}
// Clipboard
/// Replaces the platform clipboard's text contents.
///
/// Returns `false` when the clipboard could not be written — including when
/// another application holds it, which is a normal, transient condition. This
/// function takes only text; see [`platform_clipboard`] for the rich backend.
#[cfg(not(alloc_frugal))]
pub fn set_clipboard_text(text: &str) -> bool {
    platform::get_platform().set_clipboard_text(text)
}
/// The platform clipboard's text contents.
///
/// Returns an empty string when the clipboard holds no text, holds a non-text
/// format, or cannot be read — the three cases are **not** distinguished here,
/// so an empty result does not mean the copy succeeded with empty text.
#[cfg(not(alloc_frugal))]
pub fn get_clipboard_text() -> String {
    platform::get_platform().get_clipboard_text()
}
/// Returns the platform's rich clipboard backend, if available.
#[cfg(not(alloc_frugal))]
pub fn platform_clipboard() -> Option<&'static dyn crate::platform::clipboard::RichClipboardBackend>
{
    platform::get_platform().clipboard_backend()
}
// Menu operations
/// Creates a native menu bar as a child of `parent`.
///
/// On macOS the bar only becomes the application's main menu once it is attached
/// with [`attach_menu_bar_to_window`]. Add menus with [`create_menu`] and items
/// with [`menu_add_item`].
#[cfg(not(alloc_frugal))]
pub fn create_menu_bar(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    #[cfg(widgets_unstripped)]
    {
        backend_for_kind(widget::WidgetKind::MenuBar).create_menu_bar(parent, x, y, width, height)
    }
    #[cfg(not(widgets_unstripped))]
    {
        let _ = (parent, x, y, width, height);
        NO_SUCH_CONTROL
    }
}
/// Adds a top-level menu to a menu bar.
///
/// `parent` must be the **menu bar**, not the window: a menu's parent is
/// structurally its bar and several backends silently do nothing when given a
/// window instead.
///
/// Returns `0` on a profile that does not ship menus.
#[cfg(not(alloc_frugal))]
pub fn create_menu(
    parent: crate::core::ObjectId,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    #[cfg(widgets_unstripped)]
    {
        backend_for_kind(widget::WidgetKind::Menu).create_menu(parent, text, x, y, width, height)
    }
    #[cfg(not(widgets_unstripped))]
    {
        let _ = (parent, text, x, y, width, height);
        NO_SUCH_CONTROL
    }
}
/// Attaches a menu bar to a window.
///
/// Returns `false` when the backend cannot attach it.
#[cfg(not(alloc_frugal))]
pub fn attach_menu_bar_to_window(
    window: crate::core::ObjectId,
    menu_bar: crate::core::ObjectId,
) -> bool {
    control_backend::get_control_backend().attach_menu_bar_to_window(window, menu_bar)
}
/// Adds an item to a menu, returning the item's id.
///
/// `shortcut` is **display text** and is neither parsed nor registered as an
/// accelerator — use [`format_shortcut`] to render a platform-correct string
/// from a typed shortcut declaration. The returned id is what
/// [`poll_menu_triggered`] reports once the item is chosen.
#[cfg(not(alloc_frugal))]
pub fn menu_add_item(
    parent_menu: crate::core::ObjectId,
    text: &str,
    shortcut: Option<&str>,
) -> crate::core::ObjectId {
    control_backend::get_control_backend().menu_add_item(parent_menu, text, shortcut)
}
/// Renders a shortcut the way the current operating system writes it.
///
/// macOS returns `⌘⇧Z`-style glyphs; Windows and Linux return
/// `Ctrl+Shift+Z`. Use this for any label the user reads (menu text, tooltips,
/// the shortcut column of a command palette) so one shortcut declaration reads
/// natively on every platform.
///
/// For the host-independent notation (useful in serialized keymaps and tests),
/// use [`crate::shortcut::Shortcut::format_shortcut`] together with
/// [`crate::shortcut::format_shortcut_for_platform`] instead.
///
/// ```
/// use rust_widgets::shortcut::{Key, Shortcut};
///
/// let shown = rust_widgets::format_shortcut(&Shortcut::primary(Key::Z));
/// #[cfg(target_os = "macos")]
/// assert_eq!(shown, "⌘Z");
/// #[cfg(not(target_os = "macos"))]
/// assert_eq!(shown, "Ctrl+Z");
/// ```
#[cfg(not(alloc_frugal))]
pub fn format_shortcut(shortcut: &crate::shortcut::Shortcut) -> String {
    platform::get_platform().format_shortcut(shortcut)
}
/// Takes the id of the next menu item the user activated, or `None` when none is
/// queued.
///
/// Like the widget version this drains a queue, so a loop calling it until
/// `None` collects every pending activation. The id returned is the one
/// [`menu_add_item`] handed back.
#[cfg(not(alloc_frugal))]
pub fn poll_menu_triggered() -> Option<crate::core::ObjectId> {
    control_backend::get_control_backend().poll_menu_triggered()
}
/// Returns the accelerator text bound to a menu item, if any.
///
/// Read from the menu item's own `shortcut` property, so the answer describes the
/// item the library drew rather than a host-side copy that could drift.
#[cfg(not(alloc_frugal))]
pub fn menu_item_shortcut(menu_item: crate::core::ObjectId) -> Option<String> {
    control_backend::get_control_backend().menu_item_shortcut(menu_item)
}
/// Returns the backend's opaque handle for a widget, when it has one.
///
/// Named for the **intent** ("the backend's own handle for this control"), not for
/// the mechanism. The value is opaque: it is whatever object the backend uses to
/// represent the control, and its meaning is entirely backend-specific. It exists so
/// host code and integration tests can reach the underlying object for things the
/// cross-platform API does not model.
///
/// This was `native_handle`, which named a mechanism in the public API — the same
/// vocabulary the surface API deliberately avoids (`src/lib.rs`'s mount/resize
/// helpers, and principle #52). The old name remains as a deprecated alias so an
/// existing call site gets a warning rather than a build break.
///
/// Returns `None` when the widget is unknown, or when the backend created it in
/// state-only mode (for example off the UI thread) and therefore holds no object to
/// return.
#[cfg(not(alloc_frugal))]
pub fn backend_handle(widget: crate::core::ObjectId) -> Option<usize> {
    platform::get_platform().get_native_handle(widget)
}
/// Queues activation of a menu item as if the user had chosen it, for tests.
///
/// Returns `false` when the backend will not accept the injected event.
#[cfg(not(alloc_frugal))]
pub fn inject_menu_trigger(menu_item_id: crate::core::ObjectId) -> bool {
    control_backend::get_control_backend().inject_menu_trigger(menu_item_id)
}
// ToolBar and StatusBar
/// Creates a tool bar strip as a child of `parent`.
///
/// The returned id can be wrapped in an `app::ToolBarHandle` for the
/// typed operations (adding actions, changing orientation).
///
/// Returns `0` on a profile that does not ship the tool bar.
#[cfg(not(alloc_frugal))]
pub fn create_tool_bar(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    #[cfg(widgets_unstripped)]
    {
        backend_for_kind(widget::WidgetKind::ToolBar).create_tool_bar(parent, x, y, width, height)
    }
    #[cfg(not(widgets_unstripped))]
    {
        let _ = (parent, x, y, width, height);
        NO_SUCH_CONTROL
    }
}
/// Creates a status bar as a child of `parent`, with `text` as its message.
///
/// Returns `0` on a profile that does not ship the status bar.
#[cfg(not(alloc_frugal))]
pub fn create_status_bar(
    parent: crate::core::ObjectId,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    #[cfg(widgets_unstripped)]
    {
        backend_for_kind(widget::WidgetKind::StatusBar)
            .create_status_bar(parent, text, x, y, width, height)
    }
    #[cfg(not(widgets_unstripped))]
    {
        let _ = (parent, text, x, y, width, height);
        NO_SUCH_CONTROL
    }
}
// Drag and Drop
/// Starts a drag of `payload` out of `source_widget_id`, advertising it as
/// `mime`.
///
/// `payload` is copied into the platform's drag object and is not retained by
/// this library, so the caller may free it once this returns. Returns `false`
/// when the backend cannot begin a drag — for instance when a drag is already in
/// progress — in which case no drop event will follow.
#[cfg(not(alloc_frugal))]
pub fn begin_drag(source_widget_id: crate::core::ObjectId, mime: &str, payload: &[u8]) -> bool {
    platform::get_platform().begin_drag(source_widget_id, mime, payload)
}
/// Takes the next completed drop, or `None` when none is pending.
///
/// Polling a queue: each successful call removes one event. A drop is only
/// reported after the user has released, so a drag in progress yields `None`.
#[cfg(not(alloc_frugal))]
pub fn poll_drop_event() -> Option<DropEvent> {
    platform::get_platform().poll_drop_event()
}
/// Queues a drop event as if the user had performed it, for tests.
///
/// Returns `false` when the backend will not accept the injected event, in which
/// case a later poll will not report it.
#[cfg(not(alloc_frugal))]
pub fn inject_drop_event(event: DropEvent) -> bool {
    platform::get_platform().inject_drop_event(event)
}
// IME and Accessibility
/// Enables or disables input-method (IME) input for a text-entry widget.
///
/// Returns `false` when the backend refuses — typically because the id is not a
/// text-entry control, or because the platform has no IME support to switch.
#[cfg(not(alloc_frugal))]
pub fn set_widget_ime_enabled(widget_id: crate::core::ObjectId, enabled: bool) -> bool {
    control_backend::get_control_backend().set_widget_ime_enabled(widget_id, enabled)
}
/// Whether input-method input is enabled for a widget.
///
/// Reports `false` both for "enabled is off" and for "this id is unknown or not
/// a text control", so it cannot distinguish the two.
#[cfg(not(alloc_frugal))]
pub fn is_widget_ime_enabled(widget_id: crate::core::ObjectId) -> bool {
    control_backend::get_control_backend().is_widget_ime_enabled(widget_id)
}
/// Returns the platform's IME bridge, if available.
#[cfg(not(alloc_frugal))]
pub fn platform_ime_bridge() -> Option<&'static dyn crate::platform::ime::ImeBridge> {
    platform::get_platform().ime_bridge()
}
/// Sets the accessible name of a widget, as reported to assistive technology.
///
/// This is the label a screen reader announces — distinct from the visible text,
/// which is often too terse or ambiguous to read aloud. Returns `false` when the
/// backend refuses.
#[cfg(not(alloc_frugal))]
pub fn set_widget_accessibility_name(widget_id: crate::core::ObjectId, name: &str) -> bool {
    control_backend::get_control_backend().set_widget_accessibility_name(widget_id, name)
}
/// The accessible name set by [`set_widget_accessibility_name`].
///
/// Returns an empty string when no name was set, when the id is unknown, or when
/// the backend refused — the cases are not distinguished, so an empty result
/// does not prove the widget is nameless to assistive technology.
#[cfg(not(alloc_frugal))]
pub fn get_widget_accessibility_name(widget_id: crate::core::ObjectId) -> String {
    control_backend::get_control_backend().get_widget_accessibility_name(widget_id)
}
// Re-exports from platform module for convenience
#[cfg(not(alloc_frugal))]
pub use platform::{
    backend_name, capabilities, dpi_scale_factor, get_platform, init as platform_init,
    quit as platform_quit, run as platform_run, runtime_gui_mode, runtime_gui_mode_for,
};
pub use platform::{
    CapabilityContract, DesktopBackend, DropEvent, EmbeddedCapabilityContract, MobileBackend,
    NativeCapabilityContract, PlatformCapabilities, RuntimeGuiMode, WidgetTriggerEvent,
    WidgetTriggerKind, WindowStateFlag,
};

// ═══════════════════════════════════════════════════════
// Deprecated aliases kept for API compatibility (principle #21)
// ═══════════════════════════════════════════════════════

/// Deprecated spellings of the surface API.
///
/// These names said "custom", which described a rendering mechanism — "painted
/// by the library rather than mapped onto an OS control". That distinction no
/// longer exists: the host platform supplies only a window and a drawing surface
/// and the library paints every widget, so the mechanism vocabulary has no
/// referent. Use the `*_surface` names instead.
///
/// The forwards are re-exported at the crate root below, so an existing
/// `rust_widgets::mount_custom_widget(...)` call keeps compiling and only gains a
/// deprecation warning — the caller is told the name changed rather than being
/// broken by it (principle #21).
#[allow(deprecated)]
/// # Reachability
///
/// **State:** Reserved: the compatibility shim for APIs replaced in earlier
/// rounds; it exists so old call sites keep compiling. Removal condition: on the
/// next major version.
///
/// Deprecated aliases for APIs renamed in earlier rounds.
///
/// Every function here forwards to its replacement and carries a `#[deprecated]`
/// note naming that replacement, so a caller gets a compiler warning that says
/// what to use instead.
pub mod deprecated {
    /// Deprecated alias of [`crate::mount_surface`].
    #[deprecated(
        note = "renamed to `mount_surface`; 'custom' named a mechanism that no longer exists"
    )]
    pub fn mount_custom_widget(
        parent: crate::core::ObjectId,
        id: crate::core::ObjectId,
        rect: crate::core::Rect,
    ) -> bool {
        crate::mount_surface(parent, id, rect)
    }

    /// Deprecated alias of [`crate::resize_surface`].
    #[deprecated(
        note = "renamed to `resize_surface`; 'custom' named a mechanism that no longer exists"
    )]
    pub fn resize_custom_widget(id: crate::core::ObjectId, rect: crate::core::Rect) -> bool {
        crate::resize_surface(id, rect)
    }

    /// Deprecated alias of [`crate::unmount_surface`].
    #[deprecated(
        note = "renamed to `unmount_surface`; 'custom' named a mechanism that no longer exists"
    )]
    pub fn unmount_custom_widget(id: crate::core::ObjectId) -> bool {
        crate::unmount_surface(id)
    }

    /// Deprecated alias of [`crate::invalidate_surface`].
    #[deprecated(
        note = "renamed to `invalidate_surface`; 'custom' named a mechanism that no longer exists"
    )]
    pub fn request_custom_repaint(id: crate::core::ObjectId) -> bool {
        crate::invalidate_surface(id)
    }

    /// Deprecated alias of [`crate::supports_surfaces`].
    #[deprecated(
        note = "renamed to `supports_surfaces`; 'custom' named a mechanism that no longer exists"
    )]
    pub fn supports_custom_widgets() -> bool {
        crate::supports_surfaces()
    }

    /// Deprecated alias of [`crate::backend_handle`].
    ///
    /// Gated with its replacement: the `mini` profile has no platform singleton to ask,
    /// so `backend_handle` does not exist there and an ungated alias would name a
    /// missing function.
    #[cfg(not(alloc_frugal))]
    #[deprecated(
        note = "renamed to `backend_handle`; 'native' named a mechanism, and which object a \
                backend holds is its own business (principle #52)"
    )]
    pub fn native_handle(widget: crate::core::ObjectId) -> Option<usize> {
        crate::backend_handle(widget)
    }
}

// Old crate-root paths keep resolving, so a rename is a warning rather than a
// break. Exported here rather than relying on callers reaching into `deprecated`.
#[allow(deprecated)]
pub use deprecated::{
    mount_custom_widget, request_custom_repaint, resize_custom_widget, supports_custom_widgets,
    unmount_custom_widget,
};
// Gated with `backend_handle`, which the alloc-frugal profile does not compile (it has
// no platform singleton to ask), so the alias must not be re-exported there.
#[cfg(not(alloc_frugal))]
#[allow(deprecated)]
pub use deprecated::native_handle;

#[cfg(test)]
mod docs_paths_tests;

#[cfg(test)]
mod compat_path_tests {
    /// The old crate-root spellings must keep resolving after the rename.
    ///
    /// The rename is a *deprecation*, not a break (principle #21): existing callers
    /// must still compile. Referring to the functions here fails the build if a
    /// re-export is ever dropped, which is otherwise invisible because nothing else
    /// in the crate uses the old names.
    #[test]
    #[allow(deprecated)]
    fn old_surface_names_still_resolve_at_the_crate_root() {
        let _: fn(crate::core::ObjectId, crate::core::ObjectId, crate::core::Rect) -> bool =
            crate::mount_custom_widget;
        let _: fn(crate::core::ObjectId, crate::core::Rect) -> bool = crate::resize_custom_widget;
        let _: fn(crate::core::ObjectId) -> bool = crate::unmount_custom_widget;
        let _: fn(crate::core::ObjectId) -> bool = crate::request_custom_repaint;
        let _: fn() -> bool = crate::supports_custom_widgets;
    }

    /// The deprecated forwards must reach the same function, not a stale copy.
    #[test]
    #[allow(deprecated)]
    fn deprecated_aliases_delegate_to_the_surface_api() {
        assert_eq!(crate::supports_custom_widgets(), crate::supports_surfaces());
    }
}
