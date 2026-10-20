// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `Platform` implementation for Linux (GTK when `gtk-native` is on, otherwise a
//! state-only host).
//!
//! Two facts drive most of what is here:
//!
//! * **GTK binds a process to one main thread.** Building a widget from any other
//!   thread aborts the process, so the surface methods check
//!   `gtk::is_initialized_main_thread()` and refuse rather than crash. That refusal is
//!   reported as `false`, which callers must read as "cannot display here".
//! * **OpenHarmony also reports `unix_desktop`.** Every arm that selects this
//!   backend therefore excludes `target_env = "ohos"`, or an OpenHarmony build would
//!   pick a GTK host that cannot exist there.

use super::types::{LinuxHandleKind, LinuxPlatform};
// `Vec` is imported from the alloc bridge rather than the prelude: `mini` is `no_std` and has
// no standard prelude, while this module still compiles there (feedback-20261019 §4).
use crate::compat::atomic::Ordering;
use crate::compat::OnceLock;
use crate::compat::String;
use crate::compat::Vec;
#[cfg(all(unix_desktop, feature = "gtk-native"))]
use crate::core::MutexExt;
use crate::core::PlatformFamily;
#[cfg(unix_desktop)]
use crate::platform::accessibility::linux::LinuxAccessibilityBridge;
#[cfg(unix_desktop)]
use crate::platform::accessibility::AccessibilityBridge;
use crate::platform::Platform;
use crate::platform::PlatformCapabilities;
// Only the `gtk-native` display overrides name these; importing them unconditionally would make
// them unused on every other build of this backend (principle #10 — no warnings).
#[cfg(all(unix_desktop, feature = "gtk-native"))]
use crate::platform::{Display, DisplayId};
use crate::platform::{WidgetTriggerEvent, WidgetTriggerKind};
#[cfg(not(all(unix_desktop, feature = "gtk-native")))]
use core::time::Duration;
#[cfg(all(unix_desktop, feature = "gtk-native"))]
use gtk::prelude::*;
// `glib` is a `gtk` re-export rather than a declared dependency of this crate, so it is
// named through `gtk` instead of as a bare crate (the same reason `canvas.rs` aliases it).
// Gated on `gtk-native`: without GTK there is nothing to re-export it from, and this
// module compiles then too.
#[cfg(all(unix_desktop, feature = "gtk-native"))]
use gtk::glib;
#[cfg(not(all(unix_desktop, feature = "gtk-native")))]
use std::thread;

/// The frame interval this backend's event loop runs at, in milliseconds.
///
/// One constant rather than a `16` written at each loop and at each `drive_frame`
/// call, because the two must agree: a loop that wakes on a 16 ms timer but advances
/// the animation bus by a different delta makes every transition run at a speed that
/// is a ratio of the two, with nothing to notice the mismatch. Named here so a backend
/// that later runs at the display's own rate changes exactly one value.
const FRAME_INTERVAL_MS: u64 = 16;

/// Resolves a window id to the GTK toplevel this backend owns for it.
///
/// A host holds the widget-registry id while [`LinuxNativeState::windows`] is keyed by the
/// platform id `create_window` returned; `host_window_for` bridges the two. GTK widgets are
/// main-thread-only, so any other thread answers `None` rather than aborting the process.
#[cfg(all(unix_desktop, feature = "gtk-native"))]
fn native_window_for(
    platform: &LinuxPlatform,
    window_id: crate::core::ObjectId,
) -> Option<gtk::Window> {
    if !gtk::is_initialized_main_thread() {
        return None;
    }
    let host = {
        #[cfg(not(alloc_frugal))]
        {
            crate::widget::runtime::host_window_for(window_id).unwrap_or(window_id)
        }
        #[cfg(alloc_frugal)]
        {
            window_id
        }
    };
    let native = platform.native.lock_guard();
    native.windows.get(&host).cloned()
}

/// GDK's integer scale override (`GDK_SCALE`), or `1.0` when unset or unparsable.
///
/// Split out because it is read on the path where no GTK display is available
/// ([`Platform::dpi_scale_factor`]): GDK honours this variable before a display exists, so a
/// host that set it and read `1.0` back would be told its own setting does not apply. A value
/// that is not a positive integer is ignored rather than treated as `0` — that would report a
/// zero scale factor, which nothing downstream can interpret.
///
/// Gated on `gtk-native` because that is the only configuration whose
/// `dpi_scale_factor` can answer at all: without GTK this backend opens no window, reports
/// `1.0`, and [`Platform::capabilities`] says `dpi_scaling: false`.
#[cfg(all(unix_desktop, feature = "gtk-native"))]
fn gdk_scale_override() -> f32 {
    match std::env::var("GDK_SCALE").ok().and_then(|value| value.trim().parse::<u32>().ok()) {
        Some(scale) if scale > 0 => scale as f32,
        _ => 1.0,
    }
}

/// GDK's fractional DPI override (`GDK_DPI_SCALE`), or `1.0` when unset or unparsable.
///
/// The companion of [`gdk_scale_override`]: GDK multiplies the integer scale factor by this,
/// which is how a host asks for 1.5x. A non-positive or unparsable value is ignored for the
/// same reason as above, and it carries the same gate.
#[cfg(all(unix_desktop, feature = "gtk-native"))]
fn gdk_dpi_scale_override() -> f32 {
    match std::env::var("GDK_DPI_SCALE").ok().and_then(|value| value.trim().parse::<f32>().ok()) {
        Some(scale) if scale > 0.0 => scale,
        _ => 1.0,
    }
}

impl Platform for LinuxPlatform {
    fn as_any(&self) -> &dyn crate::compat::Any {
        self
    }
    fn backend_name(&self) -> &'static str {
        #[cfg(all(unix_desktop, feature = "gtk-native"))]
        {
            "gtk"
        }
        #[cfg(not(all(unix_desktop, feature = "gtk-native")))]
        {
            // Honest name: without the `gtk-native` feature this backend keeps
            // widget state in-process and never opens native GTK windows.
            "linux-state-backend"
        }
    }
    fn family(&self) -> PlatformFamily {
        PlatformFamily::Desktop
    }

    /// The Linux state backend keeps its widget tree in-process.
    ///
    /// Stated explicitly rather than inherited: the trait default is now an honest all-`false`
    /// (see [`Platform::capabilities`]), and this backend's own name says it "keeps widget state
    /// in-process and never opens native GTK windows", so it must not claim a host integration it
    /// does not wire up. `dpi_scaling`, `ime` and `accessibility` are answered by the in-process
    /// toolkit layer; there is no host menu protocol, so `native_menu` is `false` — the same
    /// reasoning [`crate::platform::wayland`] records.
    ///
    /// # Why `dpi_scaling` follows `dpi_scale_factor`
    ///
    /// It used to be unconditionally `true` while `dpi_scale_factor()` inherited the trait's
    /// constant `1.0` — a claim with no implementation behind it (principle #37), and a
    /// fabricated value on any HiDPI display. Both are now derived from the one query this
    /// backend actually performs: [`Self::dpi_scale_factor`] asks GDK's monitor when
    /// `gtk-native` is on, and this flag reports whether it did.
    fn capabilities(&self) -> PlatformCapabilities {
        PlatformCapabilities {
            dpi_scaling: self.dpi_scale_factor() != 1.0
                || cfg!(all(unix_desktop, feature = "gtk-native")),
            ime: true,
            accessibility: true,
            native_menu: false,
            typed_widget_trigger: true,
            // `create_file_dialog` is not overridden, so it falls through to the trait default
            // (which answers "no control was created") and there is no system chooser.
            //
            // The clipboard is the GDK display clipboard (see `super::gdk_clipboard`), and GDK
            // is a *build* fact while a display is a *session* fact: a `gtk-native` build in a
            // headless shell has the interface and nothing behind it, so both are asked.
            //
            // # Why this is a `cfg` block and not `cfg!(…) && …`
            //
            // `cfg!` is a runtime `bool`, so its right-hand operand is still *type-checked* in
            // every build. `gdk_clipboard` is compiled only with `gtk-native`, and this module is
            // compiled for **OpenHarmony** targets too (`unix_desktop`, no `gtk-native`),
            // so `cfg!(gtk-native) && super::gdk_clipboard::available()` failed to resolve there
            // — which is how `check_harmony_cross.sh` caught it.
            clipboard: {
                #[cfg(all(unix_desktop, feature = "gtk-native"))]
                {
                    super::gdk_clipboard::available()
                }
                #[cfg(not(all(unix_desktop, feature = "gtk-native")))]
                {
                    false
                }
            },
            // `open_external` spawns `xdg-open`.
            open_external: true,
            // The GTK file chooser, in the builds that compile GTK in.
            //
            // This is a **build** fact and not the `is_initialized_main_thread()` the method
            // itself checks, which the probe (`examples/probe_file_picker.rs`) is what settled: a
            // host asks `capabilities()` before it drives a loop — that is the whole point of
            // asking — and a predicate that is false until `init()` has run told such a host there
            // is no picker on a machine that has one. So the flag states what the backend has
            // ("there is a GTK chooser here"), and *availability* is the method's own answer:
            // with no display session the call reports `PathPickerError::Unavailable`, which is
            // exactly the case that variant is documented for.
            file_dialog: cfg!(all(unix_desktop, feature = "gtk-native")),
            // `fc-list` is the fontconfig enumerator on X11/BSD sessions; a host without it
            // answers an empty list (B28.39).
            system_fonts: true,
        }
    }

    /// Presents the GTK file chooser and answers with what the user picked.
    ///
    /// # Why the answer type is not `Option<PathBuf>`
    ///
    /// See [`crate::platform::PathPickerError`]: "the user cancelled" and "there is no picker
    /// here" are different facts with different host fallbacks, so they are different values. The
    /// three outcomes this maps are: `ResponseType::Accept` with a filename → `Ok(Some(path))`;
    /// any other response → `Ok(None)`; a thread or session without a usable toolkit →
    /// `Err(Unavailable)`.
    ///
    /// # Why the dialog is destroyed rather than merely dropped
    ///
    /// `gtk::Dialog::run` runs a nested main loop and leaves the dialog hidden on return. GTK's
    /// own documentation says a dialog created this way must be destroyed explicitly, or it stays
    /// on the (invisible) widget stack for the rest of the session, and a host that picks files
    /// repeatedly would accumulate one per pick.
    ///
    /// # What is *not* claimed
    ///
    /// The returned path is not checked for existence or writability. A save dialog legitimately
    /// names a file that does not exist yet, and a toolkit filter cannot promise that what the
    /// user typed is something the process may open — the caller's own `open`/`create` reports
    /// that, with the error the OS actually produced.
    #[cfg(all(unix_desktop, feature = "gtk-native"))]
    fn choose_path(
        &self,
        purpose: crate::platform::PathPurpose,
        title: &str,
    ) -> Result<Option<std::path::PathBuf>, crate::platform::PathPickerError> {
        use crate::platform::PathPickerError;
        if !gtk::is_initialized_main_thread() {
            // The same rule as `create_window`: a GTK widget built off the main thread aborts the
            // process, and a dialog is a widget. Reported rather than skipped, so the caller knows
            // to ask from the loop's thread instead.
            return Err(PathPickerError::Unavailable);
        }
        let action = match purpose {
            crate::platform::PathPurpose::OpenFile => gtk::FileChooserAction::Open,
            crate::platform::PathPurpose::SaveFile => gtk::FileChooserAction::Save,
            crate::platform::PathPurpose::SelectDirectory => gtk::FileChooserAction::SelectFolder,
        };
        let dialog = gtk::FileChooserDialog::new(Some(title), None::<&gtk::Window>, action);
        dialog.add_button("Cancel", gtk::ResponseType::Cancel);
        dialog.add_button("Select", gtk::ResponseType::Accept);
        let chosen = if dialog.run() == gtk::ResponseType::Accept {
            dialog.filename().map(|path| path.to_path_buf())
        } else {
            None
        };
        // SAFETY: `dialog` is a live GTK widget on the main thread, and it is not used again
        // after this call.
        unsafe {
            dialog.destroy();
        }
        Ok(chosen)
    }

    /// Hands a URL or path to the desktop's default handler by spawning `xdg-open`.
    ///
    /// See [`Platform::open_external`] for the hand-off (not completion) contract, and
    /// [`crate::platform::external`] for why an option-shaped target is refused rather than
    /// escaped: `xdg-open` parses its own arguments and rejects both `-` and `-- /path`.
    fn open_external(&self, target: &str) -> Result<(), crate::platform::types::OpenError> {
        crate::platform::external::spawn_handler("xdg-open", &[], target)
    }

    /// Whether `xdg-open` is on this host's `PATH`.
    ///
    /// The companion of [`Self::open_external`]: that method hands a target to `xdg-open`, so "is
    /// the handler installed" is exactly "is `xdg-open` findable on `PATH`". See
    /// [`Platform::has_external_handler`] for why this is a separate, side-effect-free query rather
    /// than the capability flag (which reports only that the backend has the method).
    fn has_external_handler(&self) -> bool {
        crate::platform::external::program_is_on_path("xdg-open")
    }

    /// The displays GDK reports; see [`super::gdk_displays`].
    ///
    /// Gated with this backend's GTK code: without `gtk-native` there is no window and no display
    /// to enumerate, so the trait default (an empty list) is the honest answer and stays in place.
    #[cfg(all(unix_desktop, feature = "gtk-native"))]
    fn displays(&self) -> Vec<Display> {
        super::gdk_displays::displays()
    }

    /// The display the window currently occupies, from the window's own position.
    ///
    /// `gtk::Window::position()` gives the toplevel's screen-space origin, which is the same point
    /// GDK uses to decide where a window at that location lands, and
    /// [`super::gdk_displays::display_index_at`] resolves it to the index [`Self::displays`] hands
    /// out. An id that names no GTK window (a control, or an unregistered window) answers `None`.
    ///
    /// # Why the id is translated first
    ///
    /// The id a host holds for a window is the registry id, while `native.windows` is keyed by the
    /// platform's own window id; `host_window_for` is the association recorded at creation (the
    /// same translation `set_widget_visible` and the paint path use). A caller that passes the
    /// platform id directly is honoured too, because `contains_key` accepts it.
    #[cfg(all(unix_desktop, feature = "gtk-native"))]
    fn display_of_window(&self, window_id: crate::core::ObjectId) -> Option<DisplayId> {
        if !gtk::is_initialized_main_thread() {
            // GTK is main-thread-only; asking from anywhere else would abort the process.
            return None;
        }
        let (x, y) = {
            let native = self.native.lock_guard();
            let host = if native.windows.contains_key(&window_id) {
                Some(window_id)
            } else {
                crate::widget::runtime::host_window_for(window_id)
            };
            host.and_then(|host| native.windows.get(&host))?.position()
        };
        super::gdk_displays::display_index_at(x, y).map(DisplayId::new)
    }

    /// A revision of GDK's current display set; see [`super::gdk_displays::generation`].
    #[cfg(all(unix_desktop, feature = "gtk-native"))]
    fn display_generation(&self) -> u64 {
        super::gdk_displays::generation()
    }

    /// Metadata for one GDK monitor; see [`super::gdk_displays::info`].
    #[cfg(all(unix_desktop, feature = "gtk-native"))]
    fn display_info(&self, display: DisplayId) -> Option<crate::platform::types::DisplayInfo> {
        super::gdk_displays::info(display)
    }

    /// Whether the GTK theme looks like a high-contrast theme.
    ///
    /// GTK does not expose an accessibility high-contrast flag the way macOS and Windows do. The
    /// observable convention is the theme name: the Adwaita high-contrast themes are called
    /// `HighContrast`/`HighContrastInverse`, and distributions ship names containing `Contrast`.
    /// This is a **heuristic**, documented as such on the trait; `None` when GTK is not available
    /// or no theme name is set. Not overridden without `gtk-native`: a state-only build has no
    /// toolkit to ask.
    #[cfg(all(unix_desktop, feature = "gtk-native"))]
    fn high_contrast(&self) -> Option<bool> {
        if !gtk::is_initialized_main_thread() {
            return None;
        }
        let settings = gtk::Settings::default()?;
        let name = settings.gtk_theme_name()?;
        Some(name.contains("Contrast"))
    }

    #[cfg(not(all(unix_desktop, feature = "gtk-native")))]
    fn high_contrast(&self) -> Option<bool> {
        None
    }

    /// Whether animations are disabled in GTK's settings.
    ///
    /// `gtk-enable-animations` is the switch GTK itself honours; when it is off the user asked for
    /// no transitions, which is exactly the reduced-motion fact. `None` without `gtk-native`.
    #[cfg(all(unix_desktop, feature = "gtk-native"))]
    fn reduce_motion(&self) -> Option<bool> {
        if !gtk::is_initialized_main_thread() {
            return None;
        }
        let settings = gtk::Settings::default()?;
        Some(!settings.is_gtk_enable_animations())
    }

    #[cfg(not(all(unix_desktop, feature = "gtk-native")))]
    fn reduce_motion(&self) -> Option<bool> {
        None
    }

    /// The locale named by the process environment.
    ///
    /// GTK has no synchronous "current locale" API that does not duplicate the C library's
    /// answer, so this reads the POSIX variables through
    /// [`crate::platform::locale::from_environment`]. The value is interned for the style layer.
    fn system_locale(&self) -> Option<&'static str> {
        crate::platform::locale::intern(crate::platform::locale::from_environment())
    }

    /// A revision of the environment facts, derived from their current values.
    fn environment_generation(&self) -> u64 {
        crate::platform::locale::generation_hash(
            self.system_theme(),
            self.high_contrast(),
            self.reduce_motion(),
            self.system_locale(),
        )
    }

    /// The installed families from fontconfig (`fc-list`); empty when it is absent.
    #[cfg(all(unix_desktop, not(alloc_frugal)))]
    fn system_fonts(&self) -> Vec<crate::platform::types::SystemFont> {
        crate::platform::fonts_unix::system_fonts()
    }

    #[cfg(all(unix_desktop, alloc_frugal))]
    fn system_fonts(&self) -> Vec<crate::platform::types::SystemFont> {
        Vec::new()
    }

    /// X11 and Wayland do not lay system UI over a toplevel's client area.
    ///
    /// Panels, docks and the desktop's own chrome live outside the client area, so all four
    /// distances are zero — a measurement, not a fallback. `Some(zeros)` keeps the widget layer
    /// from substituting its portrait-phone default on a desktop window, and matches winit's X11
    /// and Wayland backends (`PhysicalInsets::new(0, 0, 0, 0)`).
    fn safe_area_insets(
        &self,
        _window_id: crate::core::ObjectId,
    ) -> Option<crate::platform::types::SafeAreaInsets> {
        Some(crate::platform::types::SafeAreaInsets { top: 0, bottom: 0, left: 0, right: 0 })
    }

    /// Reads `MemTotal` from `/proc/meminfo` via [`crate::platform::os_probes`].
    fn total_memory_mb(&self) -> Option<u64> {
        crate::platform::os_probes::total_memory_mb()
    }

    /// The display's scale factor, asked of GTK (or GDK's environment override).
    ///
    /// # Why this is not inherited
    ///
    /// The trait default is the constant `1.0`, which is the *unknown* answer, not a
    /// measurement. Reporting it while [`Self::capabilities`] claimed `dpi_scaling: true` was a
    /// fabricated value (principle #37), and on a HiDPI display it meant every control the
    /// library paints was laid out at half or a third of its intended physical size.
    ///
    /// # The three sources, in order
    ///
    /// 1. **`gtk-native` with a live display**: the monitor's own `scale_factor()`. This is the
    ///    authority — it is what GTK itself lays widgets out with.
    /// 2. **`gtk-native` without one** (off the main thread, or no display): GDK's documented
    ///    environment overrides, `GDK_SCALE` (an integer factor) and `GDK_DPI_SCALE` (a
    ///    fractional one layered on top). GTK honours both, so a host that set one and read
    ///    `1.0` back would be told its setting does not apply.
    /// 3. **Neither**: `1.0`. This backend really is unscaled then — it opens no window at all
    ///    — and [`Self::capabilities`] reports `dpi_scaling: false` to match.
    fn dpi_scale_factor(&self) -> f32 {
        #[cfg(all(unix_desktop, feature = "gtk-native"))]
        {
            // GTK is main-thread-only; asking from anywhere else would abort the process.
            if gtk::is_initialized_main_thread() {
                if let Some(display) = gtk::gdk::Display::default() {
                    // The monitor the pointer is on, which is the one a new window would open
                    // on — asking the primary monitor instead would report the wrong scale on
                    // a mixed-DPI setup.
                    if let Some(monitor) = display.monitor(0) {
                        let scale = monitor.scale_factor();
                        if scale > 0 {
                            // GDK's fractional component layers on the integer factor; applying
                            // both is what matches how GTK sized the widgets.
                            return (scale as f32) * gdk_dpi_scale_override();
                        }
                    }
                }
            }
            gdk_scale_override() * gdk_dpi_scale_override()
        }
        #[cfg(not(all(unix_desktop, feature = "gtk-native")))]
        {
            1.0
        }
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

    /// Submits the job to the unix print spooler via [`crate::platform::os_probes`].
    fn spawn_print_job(&self, job_file: &std::path::Path) -> Result<(), String> {
        crate::platform::os_probes::spawn_print_job(job_file)
    }

    /// `lp` or `lpr` must be present for the system print backend to work.
    fn has_print_support(&self) -> bool {
        crate::platform::types::unix_print_clients_available()
    }

    /// A self-drawn widget gets a `gtk::DrawingArea` inside the window's content
    /// container; its `draw` signal blits a frame from `widget::runtime`.
    /// See `linux/canvas.rs`.
    ///
    /// Gated on exactly the same conditions as `canvas.rs` itself: `mini`/
    /// `embedded` have no widget registry, and a build without `gtk-native` has
    /// no GTK toplevel to put a `DrawingArea` in. In both cases the trait
    /// defaults apply and `supports_surfaces()` honestly reports `false`.
    #[cfg(all(unix_desktop, feature = "gtk-native", widgets_unstripped))]
    fn mount_surface(
        &self,
        parent: crate::core::ObjectId,
        id: crate::core::ObjectId,
        rect: crate::core::Rect,
    ) -> bool {
        super::canvas::mount_canvas(self, parent, id, rect)
    }

    #[cfg(all(unix_desktop, feature = "gtk-native", widgets_unstripped))]
    fn resize_surface(&self, id: crate::core::ObjectId, rect: crate::core::Rect) -> bool {
        super::canvas::resize_canvas(self, id, rect)
    }

    #[cfg(all(unix_desktop, feature = "gtk-native", widgets_unstripped))]
    fn unmount_surface(&self, id: crate::core::ObjectId) -> bool {
        super::canvas::unmount_canvas(self, id)
    }

    /// `true` only when the widget surface exists for this profile.
    #[cfg(all(unix_desktop, feature = "gtk-native", widgets_unstripped))]
    fn supports_surfaces(&self) -> bool {
        true
    }

    /// Shows or hides a GTK toplevel.
    ///
    /// # Why this has to exist
    ///
    /// Nothing else showed a window. The only `show_all()` in the backend sat inside
    /// `mount_canvas`, so a window became visible **as a side effect of mounting a surface
    /// onto it**: a demo that mounted something appeared, and a demo that mounted nothing
    /// never appeared at all — while its log still said the window was shown. Showing a
    /// window is its own operation, so it is implemented here rather than left to fall out
    /// of an unrelated call.
    ///
    /// Both ids are accepted: the **host** window id the platform built (what a
    /// `WindowHandle` carries) and the **widget** id of the window itself. A caller reaches
    /// here with one or the other depending on which layer it sits in, and resolving both
    /// keeps the caller from having to know which it holds.
    ///
    /// The trait's signature returns `()`, so an id that names no GTK window is ignored
    /// rather than reported. That is the common case, not an error: most ids arriving here
    /// are ordinary controls, for which the model flag is the whole story.
    #[cfg(all(unix_desktop, feature = "gtk-native"))]
    fn set_widget_visible(&self, widget_id: crate::core::ObjectId, visible: bool) {
        // GTK widgets are main-thread-only; driving them from anywhere else would abort.
        if !gtk::is_initialized_main_thread() {
            return;
        }
        let native = self.native.lock_guard();
        let host = if native.windows.contains_key(&widget_id) {
            Some(widget_id)
        } else {
            crate::widget::runtime::host_window_for(widget_id)
        };
        let Some(window) = host.and_then(|host| native.windows.get(&host)) else {
            return;
        };
        if visible {
            // `show_all` rather than `show`: a child created before its parent was realized
            // is not shown by `show` alone, and a window's controls are created exactly
            // that way.
            window.show_all();
        } else {
            window.hide();
        }
    }

    /// Queue a redraw on the canvas's `DrawingArea`.
    #[cfg(all(unix_desktop, feature = "gtk-native", widgets_unstripped))]
    fn invalidate_surface(&self, id: crate::core::ObjectId) -> bool {
        super::canvas::repaint_canvas(self, id)
    }
    /// Queue a redraw of one rectangle of the canvas's `DrawingArea`.
    ///
    /// Gated exactly like [`Self::invalidate_surface`]: without `gtk-native` there are
    /// no canvases to invalidate, so the trait default (`false`) is the honest answer
    /// and the caller falls back to a whole-surface repaint.
    #[cfg(all(unix_desktop, feature = "gtk-native", widgets_unstripped))]
    fn invalidate_surface_rect(&self, id: crate::core::ObjectId, rect: crate::core::Rect) -> bool {
        super::canvas::repaint_canvas_rect(self, id, rect)
    }
    fn init(&self) {
        self.runtime.initialized.store(true, Ordering::SeqCst);
        #[cfg(all(unix_desktop, feature = "gtk-native"))]
        {
            // GTK 3 permits `gtk::init()` on exactly one thread per process and
            // **aborts the process** if another thread calls it ("Attempted to
            // initialize GTK from two different threads"). The check-then-init
            // sequence must therefore be atomic: testing `is_initialized()` first
            // and initializing second lets two threads both observe "not
            // initialized" and then both call `gtk::init()`, which is a data race
            // inside GTK (observed as intermittent panics and, under load, a
            // SIGSEGV). Holding a process-wide mutex across both steps makes the
            // first caller the GTK main thread and every later caller a no-op, so
            // `init()` is safe from any thread — which is what a test runner (one
            // worker thread per `#[test]`) and a worker thread both require.
            use std::sync::{Mutex, OnceLock};
            static GTK_INIT_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
            let lock = GTK_INIT_LOCK.get_or_init(|| Mutex::new(()));
            let _guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

            if gtk::is_initialized_main_thread() {
                // Already owned by this thread; nothing to do.
            } else if gtk::is_initialized() {
                log::debug!(
                    "[linux] init: GTK is already initialized on another thread; \
                     not re-initializing (GTK permits a single main thread)"
                );
            } else if let Err(e) = gtk::init() {
                log::error!("[linux] gtk::init() failed: {:?}", e);
            }
        }
        #[cfg(not(all(unix_desktop, feature = "gtk-native")))]
        {}
    }
    fn run(&self) {
        #[cfg(all(unix_desktop, feature = "gtk-native"))]
        {
            // One frame, on every tick of the GTK loop.
            //
            // `gtk::main()` runs the toolkit's loop, and the toolkit's callbacks are what
            // *fill* the trigger queue: a `connect_size_allocate` handler calls
            // `queue_resize_trigger`, which pushes a `Resized` event. `gtk::main()` never
            // reads that queue back, so the event sat there and the layout was never
            // re-run -- the backend reported a resize correctly and the library never acted
            // on it. See `crate::drain_triggers` for the contract.
            //
            // `crate::drive_frame` is what replaced the bare `drain_triggers()` call here.
            // Draining alone was half a frame: the queue emptied, and no control was ever
            // advanced, so every animation in the library was unreachable from a GTK window
            // (BLUE24 §0A.1 measurement 1 -- `tick_animations` had no caller). A frame now
            // drains, advances the animation bus once, and answers whether another frame is
            // owed; see `drive_frame` for why that order is the only correct one.
            //
            // `glib::timeout_add_local` runs its closure on the GTK main thread, which is
            // the thread GTK requires for widget work, so dispatching from here is
            // main-thread work rather than a cross-thread call.
            //
            // The timer keeps running regardless of the answer: `ControlFlow::Break` here
            // would stop the *drain* as well, so a still window would stop receiving the
            // events that could start it moving again. `drive_frame` already makes a still
            // frame nearly free -- two thread-local lookups and one `is_animating()` sweep
            // -- which is the property that makes an unconditional heartbeat affordable.
            glib::timeout_add_local(core::time::Duration::from_millis(FRAME_INTERVAL_MS), || {
                crate::drive_frame(FRAME_INTERVAL_MS as u32);
                glib::ControlFlow::Continue
            });
            gtk::main();
        }
        #[cfg(not(all(unix_desktop, feature = "gtk-native")))]
        {
            if !self.runtime.initialized.load(Ordering::SeqCst) {
                self.init();
            }
            self.runtime.running.store(true, Ordering::SeqCst);
            // The library-painted path's frame loop.
            //
            // This used to be `while running { sleep(16) }` -- a loop that woke every
            // frame and did **nothing at all**, which is why the animation bus was inert
            // on a GTK-less Linux window: the sleep was the whole body. It now advances
            // the frame, and *that* is what makes a hover transition on this path reach
            // its end (BLUE24 §1 criterion 5 -- the end-to-end "it really moves" check).
            //
            // The sleep length is still fixed at the frame interval. A host that wants an
            // event-driven loop would block until the next event when
            // `needs_another_frame` is `false`; this backend has no such primitive --
            // `Platform::run` is entered once and owns the thread until `quit` -- so it
            // polls. What matters for correctness is that the *library* is asked per frame
            // and that a still frame costs nothing but the sweep.
            while self.runtime.running.load(Ordering::SeqCst) {
                crate::drive_frame(FRAME_INTERVAL_MS as u32);
                thread::sleep(Duration::from_millis(FRAME_INTERVAL_MS));
            }
        }
    }
    fn quit(&self) -> bool {
        // The exit condition of this backend's own frame loop is `runtime.running`, an atomic
        // any thread may store, so a request from anywhere ends the *state* loop.
        self.runtime.running.store(false, Ordering::SeqCst);
        #[cfg(all(unix_desktop, feature = "gtk-native"))]
        {
            // GTK's stop is main-thread-only: `gtk_main_quit` walks the global main-loop stack
            // that `gtk::main()` installs, and GTK permits only the thread that owns the toolkit
            // to touch it -- the same rule `create_window` above obeys. So a caller on any other
            // thread records the request instead, and the loop's own frame tick performs the stop
            // there (`crate::platform::stop`). That keeps the answer below honest: the request
            // does have a reader, because `run`'s `glib::timeout_add_local` closure calls
            // `drive_frame` on the GTK main thread every frame.
            //
            // Before this, the off-main case called `gtk_main_quit` directly, which is an
            // unsynchronised touch of GTK's main-loop stack from a thread GTK does not allow --
            // a data race whose symptom (a loop that stops, or one that does not) would depend on
            // timing. The tick path is the same stop, performed where it is legal.
            if gtk::is_initialized_main_thread() {
                gtk::main_quit();
            } else {
                crate::platform::stop::request();
            }
        }
        true
    }

    /// Release every registry entry the backend holds for `widget_id`.
    ///
    /// Beyond the authoritative `BackendState` record, the Linux backend keeps
    /// per-widget entries only in the native GTK registries in `native` (under
    /// `gtk-native`). They must be purged, otherwise a UI rebuilt in a
    /// create/destroy loop would leak one entry per discarded widget.
    ///
    /// Only the library's own bookkeeping is released here: no GTK call is made,
    /// and the native objects are dropped when their registry entries are removed
    /// (GTK keeps its own reference for objects still attached to a parent).
    /// Pops the next typed widget-trigger event from this backend's queue.
    ///
    /// # Why Linux must delegate, and why `typed_widget_trigger: true` was a claim until it did
    ///
    /// [`PlatformCapabilities::typed_widget_trigger`] says this backend can produce and deliver a
    /// typed trigger. The queue exists — Linux holds a `BackendState` like every other backend, and
    /// the trait default for both methods is `None`/`false` — but neither method was overridden, so
    /// the flag was a promise the backend did not keep.
    ///
    /// It is reachable: `NativeControlBackend::poll_widget_trigger_event` forwards straight to
    /// `get_platform().poll_widget_trigger_event()` (`control_backend/native.rs`), so on a GTK or
    /// state-backed Linux host an injected trigger was accepted by the API and then silently
    /// dropped — the "reported success for something that did not happen" shape.
    ///
    /// Delegating to `BackendState` is the same two lines `harmony`, `android`, `ios`, `wayland`,
    /// `windows` and `mobile` already write, so there is one queue implementation and not a
    /// seventh.
    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        self.state.pop_widget_trigger_event()
    }

    /// Pushes a typed widget-trigger event, refusing ids this backend never made.
    fn inject_widget_trigger_event(&self, widget_id: u64, kind: WidgetTriggerKind) -> bool {
        self.state.inject_widget_trigger_event(widget_id, kind)
    }

    /// Pops the next pending trigger as a bare id, over the same queue as the typed view.
    fn poll_widget_triggered(&self) -> Option<u64> {
        self.poll_widget_trigger_event().map(|event| event.widget_id)
    }

    fn destroy_widget(&self, widget_id: u64) -> bool {
        #[cfg(all(unix_desktop, feature = "gtk-native"))]
        {
            // A `gtk::Window` dropped by Rust is **not** a destroyed toplevel: GTK and the window
            // manager keep the X window alive until `gtk_widget_destroy` runs. Removing the map
            // entry alone therefore left the window on screen (measured with `wmctrl`: the id was
            // still listed after `destroy_widget`). Destroy it explicitly, but only where GTK
            // allows the call — the toolkit is main-thread-only, and `create_window` is the only
            // path that can have built a native toplevel at all.
            let on_gtk_thread = gtk::is_initialized_main_thread();
            let mut native = self.native.lock_guard();
            let window = native.windows.remove(&widget_id);
            // Resolve and drop the input-method context *before* the toolkit tears the widget
            // down. See the `ime_contexts` field note: dropping it from inside signal-handler
            // destruction runs IM-module code against an already-invalid object.
            let ime = native.ime_contexts.remove(&widget_id);
            native.root_boxes.remove(&widget_id);
            native.content_overlay.remove(&widget_id);
            native.window_painters.remove(&widget_id);
            native.widgets.remove(&widget_id);
            drop(native);
            if let Some(ime) = ime {
                // `try_borrow_mut` rather than `borrow_mut`: a (re-entrant) call that destroys
                // the window from inside `filter_keypress` would still be holding the read
                // borrow. Refusing here leaks the context into the closure's `Weak` (which then
                // fails to upgrade) instead of panicking inside a GTK callback.
                if let Ok(mut slot) = ime.try_borrow_mut() {
                    if let Some(context) = slot.take() {
                        // End the IME session while the widget/context are both valid; then drop
                        // the context before `gtk_widget_destroy` runs.
                        context.release();
                        drop(context);
                    }
                }
            }
            if on_gtk_thread {
                if let Some(window) = window {
                    // SAFETY: `window` is a toplevel this backend created through
                    // `gtk::Window::new` on this thread, and the call runs only when
                    // `gtk::is_initialized_main_thread()` says this is that thread. Nothing else
                    // holds a Rust reference to it: it was just removed from `native.windows`.
                    unsafe {
                        window.destroy();
                    }
                    // `gtk_widget_destroy` queues the X `DestroyWindow`; with no main loop
                    // running (the session was just stopped) nothing else would flush it, and
                    // the WM would keep listing the window. Flushing here makes the destruction
                    // observable to the server immediately.
                    if let Some(display) = gtk::gdk::Display::default() {
                        display.flush();
                    }
                }
            }
        }

        // The state record is the authority on whether the widget existed.
        self.state.destroy_widget(widget_id)
    }

    /// Creates a top-level window, with a real GTK toplevel when this is the GTK
    /// main thread.
    ///
    /// # Off-main callers get a state-only window
    ///
    /// GTK 3 binds every widget to one main thread: `gtk::Window::new` (and the
    /// rest of the toolkit) calls `assert_initialized_main_thread!()`, which
    /// **aborts the process** from any other thread. Off-main is a real case — the
    /// C ABI may be driven from a worker thread, and a test harness runs every
    /// `#[test]` on its own thread. Skipping the native construction there and
    /// returning a state-only handle keeps the call honouring its contract (a
    /// valid, text/geometry-consistent id) instead of taking down the process.
    ///
    /// This mirrors `CocoaPlatform::create_window`, which refuses to construct an
    /// `NSWindow` off the AppKit main thread for exactly the same reason.
    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        let id = self.insert_widget(LinuxHandleKind::Window, title, x, y, width, height);
        // A fresh GTK toplevel is restored, windowed, resizable and decorated.
        self.state.init_window_state(id, crate::platform::state::WindowStateRecord::new_window());
        #[cfg(all(unix_desktop, feature = "gtk-native"))]
        {
            // Only the thread that owns GTK may build widgets on it.
            if !gtk::is_initialized_main_thread() {
                log::debug!(
                    "[linux] create_window: off the GTK main thread; registered a \
                     state-only window (id={id}). GTK widgets are main-thread-only."
                );
                return id;
            }
            let window = gtk::Window::new(gtk::WindowType::Toplevel);
            window.set_title(title);
            window.set_default_size(width as i32, height as i32);
            window.move_(x, y);
            let root = gtk::Box::new(gtk::Orientation::Vertical, 0);

            // The window-content painter, with the surface container above it.
            //
            // The backend creates no native controls, so a window's ordinary children —
            // buttons, check boxes, labels — have no GTK widget of their own and nothing
            // painted them: the window showed its chrome over an empty client area. This
            // `DrawingArea` closes that gap by painting the window's whole widget tree
            // itself (see `crate::widget::runtime::render_frame_tree`).
            //
            // The stacking order matters: the painter goes in first so it is the *bottom*,
            // and the overlay holding mounted surfaces sits above it, so a self-drawn
            // control mounted as a surface paints over the tree. That is what lets one
            // window mix both kinds.
            let paint_area = gtk::DrawingArea::new();
            // Expand rather than claim an absolute rectangle: the overlay resizes it with
            // the window, so a repaint after a resize covers the new extent.
            paint_area.set_hexpand(true);
            paint_area.set_vexpand(true);
            paint_area.set_has_tooltip(false);
            // The area takes the keyboard for the whole window's tree. This used to be set only
            // inside the press and key handlers, which was too late for GTK: a widget that is not
            // focusable at map time cannot be given focus by the window manager, and `grab_focus`
            // therefore did nothing. Setting it at construction is what makes the grab below
            // reachable.
            paint_area.set_can_focus(true);
            // Once the area is on screen it must own the keyboard, or key events go to the
            // toplevel and never reach the library's focused control. `map` is the first moment
            // `grab_focus` can succeed, because GTK only grants focus to a viewable widget.
            paint_area.connect_map(|area| {
                area.grab_focus();
            });
            // The input method must see keys before the widget layer does; see `ImeContext` for
            // why the library owns it instead of the host.
            //
            // The closure holds a `Weak`: `destroy_widget` takes the strong reference out of
            // `native.ime_contexts` and drops the context *before* GTK destroys the widget. A
            // strong capture would unref the `IMMulticontext` from inside GTK's own
            // signal-handler teardown, which warns (and runs IM-module code at a point where
            // the object is already invalid).
            let ime = super::canvas::ImeContext::new(&paint_area);
            let ime_for_keys = std::rc::Rc::downgrade(&ime);

            // Key input is owned by the **toplevel**, not by the area.
            //
            // # Why one dispatcher and not two
            //
            // GTK's toplevel has its own input context and can filter a key around the focused
            // widget's handler. A second key path in the area could not know whether that context
            // had already consumed the key: measured, a field that opted out of IME received
            // nothing because the toplevel context consumed the keys before the area saw them.
            // Handling keys here — before GTK's class closure runs — leaves exactly one decision:
            // either our context consumes the key (a composition step or a commit) or the key is
            // delivered to the focused control. Returning `Stop` every time is what keeps GTK's
            // second context out of the path, so a composition cannot be delivered twice.
            //
            // The area still has its own handler for hosts that mount a surface into a window
            // this backend did not create; for windows created here, this handler runs first and
            // stops the event.
            let key_area = paint_area.clone();
            window.connect_key_press_event(move |_, event| {
                let Some(window_widget) = crate::widget::runtime::widget_id_for_host_window(id)
                else {
                    return glib::Propagation::Stop;
                };
                // Tab is a library action, not content: it moves focus before the input method
                // sees it. Shift reverses the direction, matching every other toolkit.
                if super::canvas::gdk_keyval_is_tab(*event.keyval()) {
                    const WIDGET_SHIFT: u32 = 1;
                    let forward = super::canvas::modifier_bits(event) & WIDGET_SHIFT == 0;
                    crate::widget::runtime::focus_next(forward);
                    key_area.queue_draw();
                    crate::platform::linux::canvas::note_canvas_redraw(window_widget);
                    return glib::Propagation::Stop;
                }
                if ime_for_keys.upgrade().is_some_and(|ime| {
                    ime.borrow().as_ref().is_some_and(|ime| ime.filter_keypress(event))
                }) {
                    key_area.queue_draw();
                    crate::platform::linux::canvas::note_canvas_redraw(window_widget);
                    return glib::Propagation::Stop;
                }
                // A printable character is committed text; everything else keeps its key code
                // so shortcuts and navigation reach the control unchanged. `printable_text`
                // refuses a control chord, so Ctrl+C is a copy rather than a typed `c`.
                let translated = if let Some(text) = super::canvas::printable_text(event) {
                    crate::event::Event::TextInput { text }
                } else {
                    crate::event::Event::KeyPress {
                        key: *event.keyval(),
                        modifiers: super::canvas::modifier_bits(event),
                    }
                };
                let target = crate::widget::runtime::focused_widget().unwrap_or(window_widget);
                if crate::widget::runtime::dispatch_event(target, &translated) {
                    key_area.queue_draw();
                    crate::platform::linux::canvas::note_canvas_redraw(target);
                }
                glib::Propagation::Stop
            });

            // The release half. `Event::KeyRelease` is published and consumed (`base.rs` turns it
            // into `key_up`), so a control tracking a held key learns it came up. It is routed to
            // the same owner as the press and stops the toplevel's context for the same reason.
            let release_area = paint_area.clone();
            window.connect_key_release_event(move |_, event| {
                let Some(window_widget) = crate::widget::runtime::widget_id_for_host_window(id)
                else {
                    return glib::Propagation::Stop;
                };
                let key_event = crate::event::Event::KeyRelease {
                    key: *event.keyval(),
                    modifiers: super::canvas::modifier_bits(event),
                };
                let target = crate::widget::runtime::focused_widget().unwrap_or(window_widget);
                if crate::widget::runtime::dispatch_event(target, &key_event) {
                    release_area.queue_draw();
                    crate::platform::linux::canvas::note_canvas_redraw(target);
                }
                glib::Propagation::Stop
            });

            // The container the window layout positions into; see `content_overlay`.
            let overlay = gtk::Overlay::new();
            overlay.add(&paint_area);
            root.pack_start(&overlay, true, true, 0);
            window.add(&root);

            // Paint the window's widget tree on every expose.
            //
            // The id GTK reports here is the **platform's**, and the tree lives under the
            // widget-registry id, so the association recorded at window creation is what
            // translates between them. Painting the platform id would look up an id that
            // addresses no widget and draw nothing — a silently empty window, which is the
            // failure this path exists to remove.
            paint_area.connect_draw(move |widget, context| {
                let area_width = widget.allocated_width().max(1) as u32;
                let area_height = widget.allocated_height().max(1) as u32;
                let Some(window_widget) = crate::widget::runtime::widget_id_for_host_window(id)
                else {
                    // No widget owns this host window yet. Not an error, and nothing to
                    // paint: the clear colour already fills the area.
                    return glib::Propagation::Proceed;
                };
                match crate::widget::runtime::render_frame_tree(
                    window_widget,
                    crate::core::Size::new(area_width, area_height),
                    crate::core::Color::rgb(240, 240, 240),
                ) {
                    Some(frame) => {
                        super::canvas::blit_rgba(context, area_width, area_height, &frame);
                    }
                    None => {
                        log::debug!(
                            "[linux] window {id} produced no tree frame \
                             (no drawable children, or the window is not mounted)"
                        );
                    }
                }
                glib::Propagation::Proceed
            });

            // ── Input ─────────────────────────────────────────────────────────
            //
            // The window's tree painter also takes pointer input.
            //
            // # Why the painter is the input surface
            //
            // Painting a window's controls into one area means the area is where the
            // pointer lands, so it is the only thing that can receive a click. Without
            // this, a demo whose controls have no mounted surface of their own received
            // **no events at all**: the controls were visible and their geometry was
            // correct, so nothing looked wrong — the callbacks simply never ran.
            //
            // The router walks the window's child tree and hit-tests each control (see
            // `widget::runtime::dispatch_pointer_event`), so a click on a button reaches
            // that button. It needs the child links, which every created control records
            // (`control_backend::custom::mount_named_widget`).
            //
            // Coordinates are already in the window's space: the paint area starts at the
            // window's content origin, and controls are positioned in that same space.
            paint_area.add_events(
                gtk::gdk::EventMask::BUTTON_PRESS_MASK
                    | gtk::gdk::EventMask::BUTTON_RELEASE_MASK
                    | gtk::gdk::EventMask::POINTER_MOTION_MASK
                    | gtk::gdk::EventMask::SCROLL_MASK
                    | gtk::gdk::EventMask::KEY_PRESS_MASK
                    | gtk::gdk::EventMask::KEY_RELEASE_MASK
                    | gtk::gdk::EventMask::LEAVE_NOTIFY_MASK,
            );
            let click_area = paint_area.clone();
            paint_area.connect_button_press_event(move |widget, event| {
                let (x, y) = event.position();
                let point = crate::core::Point::new(x as i32, y as i32);
                let Some(window_widget) = crate::widget::runtime::widget_id_for_host_window(id)
                else {
                    return glib::Propagation::Proceed;
                };
                // The area is focusable so a clicked control can take the keyboard
                // afterwards. The runtime router focuses the control the press landed on
                // (`dispatch_pointer_event`); the grab below gives GTK the same answer, so keys
                // are delivered to this area instead of the toplevel. Both halves are needed:
                // the router decides *which* control, GTK decides *whether* the area sees keys.
                widget.set_can_focus(true);
                let _ = super::canvas::focus_area_if_enabled(widget);
                if crate::widget::runtime::dispatch_pointer_event(
                    window_widget,
                    &crate::event::Event::mouse_press_with(
                        point.x,
                        point.y,
                        1,
                        crate::platform::linux::canvas::modifier_state_bits(event.state()),
                    ),
                    point,
                ) {
                    click_area.queue_draw();
                    crate::platform::linux::canvas::note_canvas_redraw(window_widget);
                }
                glib::Propagation::Proceed
            });

            let release_area = paint_area.clone();
            paint_area.connect_button_release_event(move |widget, event| {
                let (x, y) = event.position();
                let point = crate::core::Point::new(x as i32, y as i32);
                let Some(window_widget) = crate::widget::runtime::widget_id_for_host_window(id)
                else {
                    return glib::Propagation::Proceed;
                };
                if crate::widget::runtime::dispatch_pointer_event(
                    window_widget,
                    &crate::event::Event::MouseRelease { pos: point, button: 1 },
                    point,
                ) {
                    widget.queue_draw();
                    crate::platform::linux::canvas::note_canvas_redraw(window_widget);
                }
                // A released click is also what a click callback keys off, and the draw
                // above covers the visual half of it.
                release_area.queue_draw();
                glib::Propagation::Proceed
            });

            // Hover, and the wheel.
            //
            // # Why the masks above were not enough
            //
            // `POINTER_MOTION_MASK` and `SCROLL_MASK` were already requested, so GTK was
            // delivering both signals to a widget that had never connected them — the
            // request made the work happen and the result was discarded. Two capabilities
            // were therefore unreachable on a GTK **window** while working on a GTK
            // **canvas** (which connects both): no control ever showed a hover highlight,
            // and a scroll area never scrolled. That is the same window/canvas asymmetry
            // this block's own doc records for the click and key paths, one message later.
            paint_area.connect_motion_notify_event(move |widget, event| {
                let (x, y) = event.position();
                let point = crate::core::Point::new(x as i32, y as i32);
                let Some(window_widget) = crate::widget::runtime::widget_id_for_host_window(id)
                else {
                    return glib::Propagation::Proceed;
                };
                // Routed through the same path a click uses, which is what lets the router
                // own the enter/leave pair: the widget layer synthesises `MouseEnter` and
                // `MouseLeave` from a move, so a plain `MouseMove` carries the hover.
                if crate::widget::runtime::dispatch_pointer_event(
                    window_widget,
                    &crate::event::Event::MouseMove { pos: point },
                    point,
                ) {
                    widget.queue_draw();
                    crate::platform::linux::canvas::note_canvas_redraw(window_widget);
                }
                glib::Propagation::Proceed
            });

            let wheel_area = paint_area.clone();
            paint_area.connect_scroll_event(move |_widget, event| {
                let Some(window_widget) = crate::widget::runtime::widget_id_for_host_window(id)
                else {
                    return glib::Propagation::Proceed;
                };
                // The canvas decodes the wheel the same way; see `linux::canvas` for why
                // `direction()` wins over `delta()` when it is set.
                let (_, delta_y) = event.delta();
                let dy = match event.direction() {
                    gtk::gdk::ScrollDirection::Up => -1.0,
                    gtk::gdk::ScrollDirection::Down => 1.0,
                    _ => delta_y,
                };
                let wheel = crate::event::Event::Wheel {
                    delta: crate::core::Point::new(0, dy.round() as i32),
                    modifiers: 0,
                };
                let target = crate::widget::runtime::hovered_widget().unwrap_or(window_widget);
                if crate::widget::runtime::dispatch_event(target, &wheel) {
                    wheel_area.queue_draw();
                    crate::platform::linux::canvas::note_canvas_redraw(target);
                }
                glib::Propagation::Proceed
            });

            // The pointer left the window entirely, which the coordinate-based hover
            // transition cannot observe: there is no control under the pointer any more, so
            // whatever was highlighted would stay highlighted forever. The canvas connects
            // the same signal for the same reason.
            let leave_area = paint_area.clone();
            paint_area.connect_leave_notify_event(move |widget, _| {
                crate::widget::runtime::clear_hover(crate::core::Point::new(0, 0));
                widget.queue_draw();
                if let Some(window_widget) = crate::widget::runtime::widget_id_for_host_window(id) {
                    crate::platform::linux::canvas::note_canvas_redraw(window_widget);
                }
                leave_area.queue_draw();
                glib::Propagation::Proceed
            });

            // Touch, and drag-and-drop.
            //
            // # Why the canvas had gestures and the window did not
            //
            // `linux::canvas` requests `TOUCH_MASK` and connects `connect_touch_event`, so a
            // mounted surface feeds the eleven gesture recognisers. The window requested no
            // such mask, so on a touch-capable desktop (or a `tablet` build on a touchscreen)
            // a pinch or a swipe over a **window-hosted** control produced nothing at all,
            // while the same gesture over a mounted surface worked. Same window, same user,
            // two answers — the asymmetry this block's own doc warns about.
            //
            // Gated on `touch` exactly as the canvas is: `Event::Touch*` does not exist
            // without the capability (see `crate::event::types`).
            #[cfg(feature = "touch")]
            {
                paint_area.add_events(gtk::gdk::EventMask::TOUCH_MASK);
                paint_area.connect_touch_event(move |widget, event| {
                    // Narrowing to `EventTouch` first: `position()` is declared on the concrete
                    // type, not on the generic `gdk::Event` the handler receives.
                    let Some(touch) = event.downcast_ref::<gtk::gdk::EventTouch>() else {
                        return glib::Propagation::Proceed;
                    };
                    let Some(window_widget) = crate::widget::runtime::widget_id_for_host_window(id)
                    else {
                        return glib::Propagation::Proceed;
                    };
                    let (x, y) = touch.position();
                    let point = crate::core::Point::new(x as i32, y as i32);
                    // `GdkEventSequence` identifies the contact for its whole lifetime, which is
                    // what the recognisers follow. Its pointer is used as the id and only ever
                    // compared — the canvas makes the same choice.
                    let touch_id = touch
                        .event_sequence()
                        .map(|sequence| sequence.as_ptr() as u64)
                        .unwrap_or(0);
                    let translated = match touch.event_type() {
                        gtk::gdk::EventType::TouchBegin => {
                            crate::event::Event::TouchBegin { pos: point, touch_id }
                        }
                        gtk::gdk::EventType::TouchUpdate => {
                            crate::event::Event::TouchMove { pos: point, touch_id }
                        }
                        // A cancel withdraws the contact: it terminates tracking without
                        // committing a completed gesture (D09-EVT-02). Emitting `TouchEnd`
                        // here would let a cancelled tap activate a control, so the internal
                        // cancel event is produced instead; the translator and touch-aware
                        // controls consume it by clearing state without a release.
                        gtk::gdk::EventType::TouchCancel => {
                            crate::event::translator::touch_cancel(point, touch_id)
                        }
                        gtk::gdk::EventType::TouchEnd => {
                            crate::event::Event::TouchEnd { pos: point, touch_id }
                        }
                        _ => return glib::Propagation::Proceed,
                    };
                    if crate::widget::runtime::dispatch_pointer_event(
                        window_widget,
                        &translated,
                        point,
                    ) {
                        widget.queue_draw();
                        crate::platform::linux::canvas::note_canvas_redraw(window_widget);
                    }
                    glib::Propagation::Proceed
                });
            }

            // Key input is handled by the toplevel handler above. The area stays focusable so GTK
            // gives it the keyboard (and therefore emits key events to this window at all); it
            // installs no key handler of its own, which keeps one dispatcher for one window.

            // Report every re-allocation of the toplevel as a `Resized` trigger.
            //
            // Without this the library only learns a new window size when someone calls
            // `WindowHandle::set_geometry`, so a user dragging the window edge left every
            // child control at the geometry it had for the previous size. The signal fires
            // for programmatic resizes too, which is harmless: the caller is expected to
            // re-run its layout, and re-running it twice is idempotent.
            //
            // # Why the id is translated before reporting
            //
            // GTK hands this closure the **platform's** window id (a small number such as
            // `1`), while everything that consumes a resize — `queue_resize_trigger`,
            // `window_client_size`, `apply_window_layout` — is keyed by the
            // **widget-registry** id, which is a different and much larger number.
            // `queue_resize_trigger` validates its argument with `is_mounted` and refuses an
            // id that addresses no widget, so reporting the platform id made every resize a
            // silent no-op: measured, a window resized to 944x600 logged
            // `platform_id=1 size=944x600 widget_owner=<large> accepted=false`, and the
            // window layout never ran again.
            //
            // `widget_id_for_host_window` is the association recorded at window creation.
            // A window with no owner yet — built before any widget was associated with it —
            // reports nothing, which is correct: there is no layout that could react.
            window.connect_size_allocate(move |_, allocation| {
                let (width, height) =
                    (allocation.width().max(0) as u32, allocation.height().max(0) as u32);
                if let Some(widget_id) = crate::widget::runtime::widget_id_for_host_window(id) {
                    crate::queue_resize_trigger(widget_id, width, height);
                }
            });

            // BLUE28 B28.32: a configure event that moves the toplevel is a `Moved` trigger, and
            // a delete event is `Closed`. Neither existed on GTK, so a host listening for
            // `on_close` or re-anchoring a layout to the screen never heard anything.
            //
            // The first configure reports the initial placement; treating that as a move would
            // fire a spurious event on every window creation. Only a change after the first one
            // is a move the user or the window manager made.
            let last_position = std::rc::Rc::new(core::cell::Cell::new(None::<(i32, i32)>));
            let moved_window = window.clone();
            moved_window.connect_configure_event(move |_, event| {
                // GDK3's `EventConfigure::position()` is already integer root coordinates; there is
                // no fractional part to round (the `f64`-shaped API is GDK4's).
                let position = event.position();
                if last_position.get() != Some(position) {
                    let first = last_position.get().is_none();
                    last_position.set(Some(position));
                    if !first {
                        if let Some(widget_id) =
                            crate::widget::runtime::widget_id_for_host_window(id)
                        {
                            #[cfg(not(alloc_frugal))]
                            crate::inject_widget_trigger_event(
                                widget_id,
                                crate::platform::WidgetTriggerKind::Moved,
                            );
                        }
                    }
                }
                // gtk-rs 0.18 types `configure-event` as `-> bool` (`true` stops the default
                // handler); the `glib::Propagation` shape is for the handlers that adopted it.
                false
            });
            window.connect_delete_event(move |_, _| {
                if let Some(widget_id) = crate::widget::runtime::widget_id_for_host_window(id) {
                    #[cfg(not(alloc_frugal))]
                    crate::inject_widget_trigger_event(
                        widget_id,
                        crate::platform::WidgetTriggerKind::Closed,
                    );
                }
                glib::Propagation::Proceed
            });

            let mut native = self.native.lock_guard();
            native.windows.insert(id, window.clone());
            native.ime_contexts.insert(id, ime);
            native.root_boxes.insert(id, root);
            native.content_overlay.insert(id, overlay.clone());
            native.window_painters.insert(id, paint_area);
            native.widgets.insert(id, window.clone().upcast::<gtk::Widget>());
        }
        id
    }

    #[cfg(unix_desktop)]
    fn ime_bridge(&self) -> Option<&dyn crate::platform::ime::ImeBridge> {
        Some(&self.ime_bridge)
    }

    /// Reports that this backend cannot switch the input method off for one control.
    ///
    /// GTK owns a per-toplevel `GtkIMContext` and filters a key in the window's event handler
    /// *before* any signal the library can connect. A custom-painted field therefore cannot opt
    /// out of an active input method on this backend; claiming otherwise would make the call look
    /// effective while the window context still composed. Text-entry controls join the input
    /// method and other controls stay out of it (see `is_widget_ime_enabled`), which is the
    /// behaviour the backend can actually honour. Winit can offer `set_ime_allowed` because its
    /// X11/Wayland backends own the input context; a GTK host does not.
    fn set_widget_ime_enabled(&self, _widget_id: crate::core::ObjectId, _enabled: bool) -> bool {
        false
    }

    /// The control's effective IME policy: its kind decides.
    ///
    /// A text-entry control joins the input method and a button does not, so a host can ask this
    /// question and get an answer that matches the GTK input path.
    fn is_widget_ime_enabled(&self, widget_id: crate::core::ObjectId) -> bool {
        #[cfg(not(alloc_frugal))]
        {
            crate::widget::runtime::accepts_ime_input(widget_id)
        }
        #[cfg(alloc_frugal)]
        {
            // The allocation-frugal profile has no widget registry to classify, so there is no
            // input method to report either.
            let _ = widget_id;
            false
        }
    }

    /// Applies a window state through GTK, or records it when no toplevel exists.
    ///
    /// BLUE28 B28.19: this used to inherit the trait default (`false`), so
    /// `WindowHandle::set_maximized` reported "not applied" on the desktop backend the `desktop`
    /// profile selects on Linux. The native calls are GTK's own window operations; a state-only
    /// build (no `gtk-native`) still records the value, which is the honest fallback.
    fn set_window_state(
        &self,
        widget_id: crate::core::ObjectId,
        flag: crate::platform::WindowStateFlag,
        on: bool,
    ) -> bool {
        #[cfg(all(unix_desktop, feature = "gtk-native"))]
        if let Some(window) = native_window_for(self, widget_id) {
            use crate::platform::WindowStateFlag;
            match flag {
                WindowStateFlag::Maximized => {
                    if on {
                        window.maximize();
                    } else {
                        window.unmaximize();
                    }
                }
                WindowStateFlag::Minimized => {
                    if on {
                        window.iconify();
                    } else {
                        window.deiconify();
                    }
                }
                WindowStateFlag::Fullscreen => {
                    if on {
                        window.fullscreen();
                    } else {
                        window.unfullscreen();
                    }
                }
                WindowStateFlag::Resizable => window.set_resizable(on),
                WindowStateFlag::Decorated => window.set_decorated(on),
            }
            self.state.set_window_state(widget_id, flag, on);
            return true;
        }
        self.state.set_window_state(widget_id, flag, on)
    }

    /// Reads a window state from GTK, falling back to the recorded value.
    ///
    /// GTK 3 has no getter for "minimised" or "fullscreen" on `gtk::Window`; those two answer
    /// from the record rather than guessing from an unrelated property.
    fn is_window_in_state(
        &self,
        widget_id: crate::core::ObjectId,
        flag: crate::platform::WindowStateFlag,
    ) -> Option<bool> {
        #[cfg(all(unix_desktop, feature = "gtk-native"))]
        if let Some(window) = native_window_for(self, widget_id) {
            use crate::platform::WindowStateFlag;
            match flag {
                WindowStateFlag::Maximized => return Some(window.is_maximized()),
                WindowStateFlag::Resizable => return Some(window.is_resizable()),
                WindowStateFlag::Decorated => return Some(window.is_decorated()),
                WindowStateFlag::Minimized | WindowStateFlag::Fullscreen => {}
            }
        }
        self.state.window_state(widget_id, flag)
    }

    /// Applies a minimum size through `gtk::Window::set_geometry_hints`.
    fn set_window_min_size(
        &self,
        widget_id: crate::core::ObjectId,
        width: u32,
        height: u32,
    ) -> bool {
        #[cfg(all(unix_desktop, feature = "gtk-native"))]
        if let Some(window) = native_window_for(self, widget_id) {
            let geometry = gtk::gdk::Geometry::new(
                width as i32,
                height as i32,
                0,
                0,
                0,
                0,
                0,
                0,
                0.0,
                0.0,
                gtk::gdk::Gravity::NorthWest,
            );
            window.set_geometry_hints(
                None::<&gtk::Widget>,
                Some(&geometry),
                gtk::gdk::WindowHints::MIN_SIZE,
            );
            self.state.set_window_min_size(widget_id, width, height);
            return true;
        }
        self.state.set_window_min_size(widget_id, width, height)
    }

    /// The minimum size recorded for a window; GTK exposes no getter for its geometry hints.
    fn window_min_size(&self, widget_id: crate::core::ObjectId) -> Option<(u32, u32)> {
        self.state.window_min_size(widget_id)
    }

    /// Loads a window icon from `path` through `gtk::Window::set_icon_from_file`.
    fn set_window_icon(&self, widget_id: crate::core::ObjectId, path: &str) -> bool {
        #[cfg(all(unix_desktop, feature = "gtk-native"))]
        if let Some(window) = native_window_for(self, widget_id) {
            match window.set_icon_from_file(path) {
                Ok(()) => {
                    self.state.set_window_icon(widget_id, path);
                    return true;
                }
                Err(error) => {
                    log::warn!("[linux] set_icon_from_file({path}) failed: {error}");
                    return false;
                }
            }
        }
        self.state.set_window_icon(widget_id, path)
    }

    /// The icon path recorded by [`Self::set_window_icon`].
    fn window_icon(&self, widget_id: crate::core::ObjectId) -> Option<String> {
        self.state.window_icon(widget_id)
    }

    /// Applies a cursor shape through GDK.
    ///
    /// The runtime calls this on hover changes; `gdk::Window::set_cursor` is the toolkit's own
    /// per-window cursor, so it survives motion until another control changes the intent.
    #[cfg(all(unix_desktop, feature = "gtk-native"))]
    fn set_cursor(
        &self,
        window_id: crate::core::ObjectId,
        icon: crate::platform::CursorIcon,
    ) -> bool {
        use crate::platform::CursorIcon;
        let Some(window) = native_window_for(self, window_id) else {
            return false;
        };
        let Some(gdk_window) = window.window() else {
            return false;
        };
        let Some(display) = gtk::gdk::Display::default() else {
            return false;
        };
        let name = match icon {
            CursorIcon::Default => "default",
            CursorIcon::Pointer => "pointer",
            CursorIcon::Text => "text",
            CursorIcon::Crosshair => "crosshair",
            CursorIcon::Move => "move",
            CursorIcon::NotAllowed => "not-allowed",
            CursorIcon::ResizeHorizontal => "ew-resize",
            CursorIcon::ResizeVertical => "ns-resize",
            CursorIcon::ResizeNwSe => "nwse-resize",
            CursorIcon::ResizeNeSw => "nesw-resize",
            CursorIcon::Grab => "grab",
            CursorIcon::Grabbing => "grabbing",
            CursorIcon::Wait => "wait",
            CursorIcon::Progress => "progress",
            CursorIcon::Help => "help",
            CursorIcon::ZoomIn => "zoom-in",
            CursorIcon::ZoomOut => "zoom-out",
        };
        let cursor = gtk::gdk::Cursor::from_name(&display, name)
            .or_else(|| gtk::gdk::Cursor::from_name(&display, "default"));
        let Some(cursor) = cursor else {
            return false;
        };
        gdk_window.set_cursor(Some(&cursor));
        true
    }

    /// Hides the pointer by clearing GDK's cursor; showing restores the default shape.
    #[cfg(all(unix_desktop, feature = "gtk-native"))]
    fn set_cursor_visible(&self, window_id: crate::core::ObjectId, visible: bool) -> bool {
        let Some(window) = native_window_for(self, window_id) else {
            return false;
        };
        let Some(gdk_window) = window.window() else {
            return false;
        };
        if !visible {
            gdk_window.set_cursor(None);
            return true;
        }
        let Some(display) = gtk::gdk::Display::default() else {
            return false;
        };
        if let Some(cursor) = gtk::gdk::Cursor::from_name(&display, "default") {
            gdk_window.set_cursor(Some(&cursor));
            true
        } else {
            false
        }
    }

    /// The window's current client size.
    ///
    /// With `gtk-native` and a real `gtk::Window` this asks GTK, which is the authority
    /// once the user has resized the window. Otherwise it falls back to the size recorded
    /// when the last resize was reported, and finally to the created geometry — each step
    /// answerable, so a caller always gets a real number rather than a guess.
    #[cfg(unix_desktop)]
    fn window_client_size(&self, window_id: crate::core::ObjectId) -> Option<(u32, u32)> {
        #[cfg(all(unix_desktop, feature = "gtk-native"))]
        {
            // GTK widgets are main-thread-only; asking from anywhere else would abort.
            if gtk::is_initialized_main_thread() {
                let native = self.native.lock_guard();
                if let Some(window) = native.windows.get(&window_id) {
                    let (width, height) = window.size();
                    if width > 0 && height > 0 {
                        return Some((width as u32, height as u32));
                    }
                }
            }
        }
        // Ask the control backend, which owns the window and is therefore the only
        // store that knows the size a resize reported.
        crate::window_client_size(window_id).or_else(|| self.state.window_size(window_id))
    }

    /// Reports a container's new client size and queues a `Resized` trigger.
    fn queue_resize_trigger(
        &self,
        window_id: crate::core::ObjectId,
        width: u32,
        height: u32,
    ) -> bool {
        // Forward to the control backend, which owns the window and the queue the app
        // polls. Writing to the platform's own state would land in a store the host
        // never reads, because `create_window` goes through the control backend.
        crate::queue_resize_trigger(window_id, width, height)
    }

    /// Publishes `text` on the display clipboard, or stores it in the backend's record when this
    /// build has no GTK path.
    ///
    /// Without `gtk-native` there is no GDK clipboard to hand the text to, but the record is
    /// still the honest answer for the running process — and it is what every other backend's
    /// `state` delegation does. Inheriting the trait default here made a copy/paste inside the
    /// library a silent no-op while the identical call on every sibling backend worked.
    ///
    /// With `gtk-native` the write goes through [`super::gdk_clipboard`], which owns the
    /// main-thread rule and the read-back confirmation. This method used to publish to GDK
    /// inline and then report **`state`'s** answer, so a failed display write was indistinguishable
    /// from a successful one — and `get_clipboard_text` below read only the record, so the two
    /// halves of one clipboard never met.
    fn set_clipboard_text(&self, text: &str) -> bool {
        #[cfg(all(unix_desktop, feature = "gtk-native"))]
        if super::gdk_clipboard::available() {
            // The record is updated *as well*: a worker thread that reads back through
            // `get_clipboard_text` off the main thread has no GDK to ask, and the process-local
            // answer keeps copy/paste inside the library working there.
            self.state.set_clipboard_text(text);
            return super::gdk_clipboard::set_text(text);
        }
        self.state.set_clipboard_text(text)
    }

    /// The clipboard's text: the display's when a display clipboard is reachable, the in-process
    /// record otherwise.
    ///
    /// Reading only the record (as this used to) meant the text another application had put on
    /// the clipboard was invisible here, and the text this process published to GDK could not be
    /// read back — the write and the read answered from two different places.
    fn get_clipboard_text(&self) -> String {
        #[cfg(all(unix_desktop, feature = "gtk-native"))]
        if let Some(text) = super::gdk_clipboard::text() {
            return text;
        }
        self.state.clipboard_text()
    }

    /// The clipboard's text, or `None` when there is none.
    ///
    /// Overridden so that "the display clipboard holds no text" is distinguishable from "this
    /// process wrote nothing": the trait default would collapse both into an empty `String` and
    /// then report `None`, which is right for the record but wrong for a display clipboard that
    /// another application can fill.
    fn clipboard_text(&self) -> Option<String> {
        #[cfg(all(unix_desktop, feature = "gtk-native"))]
        if super::gdk_clipboard::available() {
            return super::gdk_clipboard::text();
        }
        let text = self.state.clipboard_text();
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }

    /// The host clipboard, or `None` when this build has no display clipboard.
    fn clipboard_backend(&self) -> Option<&dyn crate::platform::clipboard::RichClipboardBackend> {
        #[cfg(all(unix_desktop, feature = "gtk-native"))]
        {
            if super::gdk_clipboard::available() {
                // A unit struct, so no `OnceLock` is needed: the backend holds no GTK object (it
                // looks the clipboard up per call, which is also what keeps it `Send + Sync`).
                static CLIPBOARD: super::gdk_clipboard::GdkClipboard =
                    super::gdk_clipboard::GdkClipboard;
                Some(&CLIPBOARD)
            } else {
                None
            }
        }
        #[cfg(not(all(unix_desktop, feature = "gtk-native")))]
        {
            None
        }
    }

    #[cfg(unix_desktop)]
    fn accessibility_bridge(&self) -> Option<&dyn AccessibilityBridge> {
        static BRIDGE: OnceLock<LinuxAccessibilityBridge> = OnceLock::new();
        Some(BRIDGE.get_or_init(LinuxAccessibilityBridge::new))
    }
}
