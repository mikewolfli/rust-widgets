// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `impl Platform for MacOSPlatform` — the main trait implementation.

#![allow(deprecated)] // Cocoa 0.24 fallback; remove when objc2 backend fully replaces cocoa

use crate::compat::String;
use crate::compat::Vec;
use crate::core::{ObjectId, PlatformFamily};
use crate::platform::accessibility::AccessibilityBridge;
use crate::platform::clipboard::RichClipboardBackend;
use crate::platform::ime::ImeBridge;
use crate::platform::macos::types::*;
use crate::platform::types::{Display, DisplayId, SafeAreaInsets};
use crate::platform::Platform;
use crate::platform::PlatformCapabilities;
use cocoa::appkit::{
    NSApp, NSApplication, NSApplicationActivationOptions, NSApplicationActivationPolicyRegular,
    NSBackingStoreBuffered, NSRunningApplication, NSWindow, NSWindowStyleMask,
};
// A plain content view is the `mini`/`embedded` fallback only; the canvas profile installs its own
// drawing view, so the import is compiled out where it would otherwise be unused.
#[cfg(not(widgets_unstripped))]
use cocoa::appkit::NSView;
use cocoa::base::{id, nil, BOOL, NO, YES};
use cocoa::foundation::{NSAutoreleasePool, NSPoint, NSRect, NSSize, NSString};
use objc::{class, msg_send, sel, sel_impl};
use std::ffi::CStr;
use std::os::raw::c_char;

/// An autoreleased `NSString` for a Rust `&str`.
///
/// `NSString::init_str` returns a +1-retained object, so an unmanaged result leaks on every call —
/// the clipboard reads/writes and the window-title setter each leaked one per invocation.
/// `autorelease` hands ownership to the enclosing pool, which is the correct lifetime for a string
/// that is only read during the following Objective-C call.
///
/// # SAFETY
/// Must be called on a thread with an active autorelease pool (AppKit's main thread has one).
///
/// # Safety
/// Must be called on the main thread with an active autorelease pool.
unsafe fn autoreleased_nsstring(s: &str) -> id {
    NSString::alloc(nil).init_str(s).autorelease()
}

/// `NSEventTypeApplicationDefined` (`15`).
///
/// The type of event `MacOSPlatform::quit` posts to wake `-[NSApplication run]` after `stop:`
/// raises its flag. It is a literal rather than a `cocoa` constant because the crate exposes the
/// obsolete `NSApplicationDefined` spelling, while the ABI value is what the message needs.
const NS_EVENT_TYPE_APPLICATION_DEFINED: u64 = 15;

/// Resolves a window id a host holds to the platform window id this backend's registry is keyed by.
///
/// A window has two ids: the **widget-registry** id the host gets back from `create_window` (the
/// window *as a widget*), and the **platform** id `MacOSPlatform::create_window` returned and
/// registered. `host_window_for` is the association recorded at creation — the same translation
/// `set_widget_visible` and the paint path use — so an id that names a widget window resolves to the
/// handle this backend knows. A profile with no widget runtime (alloc-frugal, i.e. `mini`) has
/// neither a registry id nor that helper, and resolves the platform id alone; the caller tries the
/// registry directly first, so a platform id still resolves there.
#[cfg(not(alloc_frugal))]
fn translate_widget_id(window_id: ObjectId) -> Option<ObjectId> {
    crate::widget::runtime::host_window_for(window_id)
}

/// The `alloc_frugal` arm: there is no widget runtime, so only platform ids exist.
#[cfg(alloc_frugal)]
fn translate_widget_id(_window_id: ObjectId) -> Option<ObjectId> {
    None
}

/// `NSEdgeInsets` in its Objective-C ABI form (four `CGFloat`s).
///
/// The cocoa backend is the legacy one and does not depend on `objc2`, so this small struct (and
/// its `Encode` impl) is what lets `msg_send!` receive the 32-byte struct return from
/// `-[NSView safeAreaInsets]`. The encoding string is the compiler's name for the type.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct NsEdgeInsets {
    top: f64,
    left: f64,
    bottom: f64,
    // SAFETY: the encoding is the exact layout clang emits for `NSEdgeInsets` (four `CGFloat`s).
    right: f64,
}

unsafe impl objc::Encode for NsEdgeInsets {
    fn encode() -> objc::Encoding {
        // SAFETY: the literal is exactly the encoding clang emits for `NSEdgeInsets` (four
        // `CGFloat`s, which are `double` on every 64-bit Apple target).
        unsafe { objc::Encoding::from_str("{NSEdgeInsets=dddd}") }
    }
}

/// Rounds a `CGFloat` edge distance to logical pixels, clamping a negative value to zero rather
/// than wrapping it into a huge `u32`.
fn logical_pixels(value: f64) -> u32 {
    value.max(0.0).round() as u32
}

/// Resolves a window id to the AppKit window it names, in either id space.
///
/// The id a host holds is the widget-registry id; this backend's handle table is keyed by the
/// platform id `create_window` returned, and `translate_widget_id` bridges them. `None` for an
/// unknown id, a non-window handle, or an uninitialised pointer.
fn resolve_window_ptr(platform: &MacOSPlatform, window_id: ObjectId) -> Option<id> {
    let host = {
        let handles = crate::compat::lock(&platform.handles);
        if handles.contains_key(&window_id) {
            Some(window_id)
        } else {
            translate_widget_id(window_id)
        }
    }?;
    let handle = crate::compat::lock(&platform.handles).get(&host).copied()?;
    if handle.kind != HandleKind::Window || handle.ptr == 0 {
        return None;
    }
    Some(handle.ptr as id)
}

impl Platform for MacOSPlatform {
    fn as_any(&self) -> &dyn crate::compat::Any {
        self
    }
    fn backend_name(&self) -> &'static str {
        "cocoa"
    }

    /// Answers from the CoreVideo display link that `run()` starts.
    ///
    /// A profile without the canvas surface has no frame loop to pace, and a host where CoreVideo
    /// could not start answers `false`; both cases keep the fixed frame grid.
    fn supports_vsync_frame(&self) -> bool {
        #[cfg(widgets_unstripped)]
        {
            super::display_link::is_running()
        }
        #[cfg(not(widgets_unstripped))]
        {
            false
        }
    }

    /// The next presented-time instant published by the display-link callback.
    fn next_vsync_deadline(&self) -> Option<std::time::Instant> {
        #[cfg(widgets_unstripped)]
        {
            super::display_link::next_deadline()
        }
        #[cfg(not(widgets_unstripped))]
        {
            None
        }
    }

    /// A library-painted widget gets an `NSView` subclass whose `drawRect:` blits a
    /// frame out of `widget::runtime`. See `macos/canvas.rs`.
    ///
    /// Gated on the same profile conditions as `canvas.rs`: `widget::runtime` is
    /// absent from `mini`/`embedded`, so the fallback defaults below apply there
    /// and `supports_surfaces()` honestly reports `false`.
    #[cfg(widgets_unstripped)]
    fn mount_surface(&self, parent: ObjectId, id: ObjectId, rect: crate::core::Rect) -> bool {
        self.mount_surface_impl(parent, id, rect)
    }

    #[cfg(widgets_unstripped)]
    fn resize_surface(&self, id: ObjectId, rect: crate::core::Rect) -> bool {
        self.resize_surface_impl(id, rect)
    }

    #[cfg(widgets_unstripped)]
    fn unmount_surface(&self, id: ObjectId) -> bool {
        self.unmount_surface_impl(id)
    }

    /// `true` only when the surface actually exists for this profile.
    ///
    /// Reporting `true` in a build where `canvas.rs` is compiled out would be a
    /// lie: a host would mount a widget and get an empty window with no error.
    #[cfg(widgets_unstripped)]
    fn supports_surfaces(&self) -> bool {
        true
    }

    /// Mark the canvas view as needing display, which schedules `drawRect:`.
    #[cfg(widgets_unstripped)]
    fn invalidate_surface(&self, id: ObjectId) -> bool {
        self.invalidate_surface_impl(id)
    }

    /// Mark one rectangle of the canvas view as needing display.
    ///
    /// Gated with `invalidate_surface` above: both are `canvas.rs` facilities, and
    /// `canvas.rs` is compiled out on the stripped profiles. Without the gate this
    /// method referenced a type that does not exist there, so `mini,cocoa-legacy`
    /// did not compile — the same drift the sibling gate above already prevents.
    #[cfg(widgets_unstripped)]
    fn invalidate_surface_rect(&self, id: ObjectId, rect: crate::core::Rect) -> bool {
        self.invalidate_surface_rect_impl(id, rect)
    }
    fn family(&self) -> PlatformFamily {
        PlatformFamily::Desktop
    }

    /// The host integrations this backend actually provides.
    ///
    /// # Why `native_menu` is `false` and `dpi_scaling` is `true`
    ///
    /// The comment above used to claim "AppKit supplies the screen scale, the input-method client,
    /// the accessibility tree and the application menu, so all four are `true`". The premise is
    /// true about AppKit and false about this type: `grep` over `src/platform/macos/` finds **no**
    /// `create_menu_bar`, `menu_add_item` or `attach_menu_bar_to_window`, so `native_menu` promised
    /// methods that are not there. It stays `false`.
    ///
    /// `dpi_scaling` used to be `false` for the same reason — there was no `dpi_scale_factor`
    /// override. BLUE28 B28.03 added one, read from `NSScreen.backingScaleFactor`, so the flag is
    /// now the same measured fact [`Self::dpi_scale_factor`] answers.
    ///
    /// `ime` and `accessibility` stay `true`: both have real implementations reachable
    /// from this backend (`ime_bridge()` → `ImeMacosBridge`, `accessibility_bridge()` →
    /// the AppKit bridge), and `shortcut_style()` really is `Mac`.
    ///
    /// The two host-service flags follow the same rule: both are `true` because both
    /// methods have real bodies (`NSPasteboard`, and the `open` handler spawned by
    /// [`Self::open_external`]).
    fn capabilities(&self) -> PlatformCapabilities {
        PlatformCapabilities {
            dpi_scaling: true,
            ime: true,
            accessibility: true,
            native_menu: false,
            typed_widget_trigger: true,
            // `set/get_clipboard_text` talk to `NSPasteboard`, so a copy is visible outside
            // this process.
            clipboard: true,
            // `open_external` spawns `open`.
            open_external: true,
            // `choose_path` presents `NSOpenPanel`/`NSSavePanel`, so the flag and the method are
            // one implementation.
            file_dialog: true,
            // CoreText's font manager and `kCTFontURLAttribute` list families and name their
            // backing files (B28.39).
            system_fonts: true,
        }
    }

    /// Presents AppKit's own file picker — see `super::file_picker` for the panel choice, the
    /// main-thread rule and what the answer does and does not claim.
    fn choose_path(
        &self,
        purpose: crate::platform::PathPurpose,
        title: &str,
    ) -> Result<Option<std::path::PathBuf>, crate::platform::PathPickerError> {
        super::file_picker::choose_path(purpose, title)
    }

    /// [`Self::choose_path`] with the caller's filters applied to the panel (P-203).
    ///
    /// AppKit's panel exposes one allowed-type set rather than a named filter list, so the union
    /// of the requested extensions is what it can enforce; `file_picker::apply_filters` records
    /// the rest of the limit.
    fn choose_path_with_filters(
        &self,
        purpose: crate::platform::PathPurpose,
        title: &str,
        filters: &[crate::platform::PathFilter],
    ) -> Result<Option<std::path::PathBuf>, crate::platform::PathPickerError> {
        super::file_picker::choose_path_with_filters(purpose, title, filters)
    }

    /// Hands a URL or path to the system's default handler by spawning `open`.
    ///
    /// See [`Platform::open_external`] for why this returns on hand-off rather than on the
    /// handler finishing, and `crate::platform::external` for the shared validation and
    /// spawn.
    fn open_external(&self, target: &str) -> Result<(), crate::platform::types::OpenError> {
        crate::platform::external::spawn_handler("open", &[], target)
    }

    /// Whether `open` is on this host's `PATH`.
    ///
    /// The companion of [`Self::open_external`]: that method hands a target to `open`, so "is the
    /// handler installed" is exactly "is `open` findable on `PATH`". See
    /// [`Platform::has_external_handler`] for why this is a separate, side-effect-free query rather
    /// than the capability flag (which reports only that the backend has the method).
    fn has_external_handler(&self) -> bool {
        crate::platform::external::program_is_on_path("open")
    }

    /// The displays AppKit reports; see `super::displays`.
    fn displays(&self) -> Vec<Display> {
        super::displays::displays()
    }

    /// The main screen's scale, read from `NSScreen.backingScaleFactor`.
    ///
    /// A Retina Mac answers `2.0`; an external 1x display answers `1.0`. `1.0` is also the answer
    /// off the main thread, where AppKit cannot be asked — the honest fallback this trait's other
    /// defaults use, and the one [`Self::capabilities`] pairs with `dpi_scaling: true` only because
    /// the measured path exists on the main thread.
    fn dpi_scale_factor(&self) -> f32 {
        super::displays::main_scale().unwrap_or(1.0)
    }

    /// The screen the window is on, asked of AppKit's `-[NSWindow screen]`.
    ///
    /// # Both id spaces
    ///
    /// The id a host holds for a window is the widget-registry id, while this backend's handle
    /// table is keyed by the platform id `create_window` returned; the association recorded at
    /// creation (`translate_widget_id`, the same translation the paint path uses) bridges them. A
    /// caller that passes the platform id directly is honoured too, because the registry lookup
    /// accepts it. An id that names no window, or a `Canvas` handle rather than a `Window`, answers
    /// `None` instead of messaging an object that is not an `NSWindow`.
    fn display_of_window(&self, window_id: ObjectId) -> Option<DisplayId> {
        if !super::types::is_main_thread() {
            // AppKit is main-thread-only; messaging it from anywhere else would raise.
            return None;
        }
        // SAFETY: the registry only holds pointers this backend created, a `Window` handle is an
        // `NSWindow`, and the main thread is established above.
        unsafe { super::displays::window_display(resolve_window_ptr(self, window_id)?) }
    }

    /// A revision of AppKit's current display set; see `super::displays::generation`.
    fn display_generation(&self) -> u64 {
        super::displays::generation()
    }

    /// Metadata for one AppKit display.
    ///
    /// AppKit exposes the frame (which gives the orientation) but no synchronous refresh-rate or
    /// colour-space accessor through `NSScreen`; those answer `None` rather than a guess. The
    /// orientation is derived from the bounds: a wider-than-tall screen is landscape.
    fn display_info(&self, id: DisplayId) -> Option<crate::platform::types::DisplayInfo> {
        let display = self.displays().into_iter().find(|display| display.id == id)?;
        Some(crate::platform::types::DisplayInfo {
            refresh_rate_mhz: None,
            orientation: Some(if display.bounds.width >= display.bounds.height {
                crate::core::Orientation::Horizontal
            } else {
                crate::core::Orientation::Vertical
            }),
            color_space: None,
            hdr: None,
        })
    }

    /// The window's safe-area insets, read from its content view's `safeAreaInsets`.
    ///
    /// AppKit introduced the selector in macOS 11; a pre-11 system answers `None` rather than
    /// zero, which is the trait's "cannot tell" rule. `None` also covers an unknown id, a
    /// non-window handle, and a call off the main thread. The cocoa backend is what the `desktop`
    /// profile selects on macOS, so without this the widget-layer default (44/34) would be the
    /// only answer a default build could get.
    fn safe_area_insets(&self, window_id: ObjectId) -> Option<SafeAreaInsets> {
        if !super::types::is_main_thread() {
            return None;
        }
        let window = resolve_window_ptr(self, window_id)?;
        // SAFETY: `window` is an `NSWindow` this backend created on the AppKit main thread. The
        // selector check keeps a pre-11 AppKit from receiving a message it does not implement.
        unsafe {
            let content: id = msg_send![window, contentView];
            if content == nil {
                return None;
            }
            let responds: BOOL = msg_send![content, respondsToSelector: sel!(safeAreaInsets)];
            if responds == NO {
                return None;
            }
            let insets: NsEdgeInsets = msg_send![content, safeAreaInsets];
            Some(SafeAreaInsets {
                top: logical_pixels(insets.top),
                bottom: logical_pixels(insets.bottom),
                left: logical_pixels(insets.left),
                right: logical_pixels(insets.right),
            })
        }
    }

    /// Whether the user asked for increased contrast.
    ///
    /// `NSWorkspace.accessibilityDisplayShouldIncreaseContrast`; see `super::environment`.
    fn high_contrast(&self) -> Option<bool> {
        super::environment::high_contrast()
    }

    /// Whether the user asked for reduced motion.
    ///
    /// `NSWorkspace.accessibilityDisplayShouldReduceMotion`; see `super::environment`.
    fn reduce_motion(&self) -> Option<bool> {
        super::environment::reduce_motion()
    }

    /// Whether the user asked for translucent materials to be reduced.
    ///
    /// `NSWorkspace.accessibilityDisplayShouldReduceTransparency`; see `super::environment`.
    fn reduce_transparency(&self) -> Option<bool> {
        super::environment::reduce_transparency()
    }

    /// The host's current accent colour, from `NSColor.controlAccentColor`.
    ///
    /// See `super::environment::accent_color` for the colour-space conversion and why the answer
    /// is `None` rather than a default blue when AppKit cannot be asked.
    fn accent_color(&self) -> Option<crate::core::Color> {
        super::environment::accent_color()
    }

    /// The user's locale, from `NSLocale.currentLocale`.
    fn system_locale(&self) -> Option<&'static str> {
        super::environment::system_locale()
    }

    /// A revision of the environment facts, derived from their current values.
    ///
    /// macOS posts notifications for each of these; this backend is polled, so the revision is a
    /// hash of the values a caller would read. `0` when AppKit cannot be asked (off-main).
    ///
    /// `crate::platform::locale::environment_revision` is the shared accessor: hashing the four
    /// facts by hand here is what kept an accent or reduced-transparency change invisible to the
    /// style snapshot (P-03), and what made adding a fact an eight-file edit (P-06).
    fn environment_generation(&self) -> u64 {
        crate::platform::locale::environment_revision(self)
    }

    /// The font families AppKit reports; see `super::environment::system_fonts`.
    fn system_fonts(&self) -> Vec<crate::platform::types::SystemFont> {
        super::environment::system_fonts()
    }

    /// The pasteboard's change counter, from `NSPasteboard.changeCount`.
    ///
    /// This is the polling signal a clipboard-manager host reads: every write by any process
    /// increments it. `0` off the main thread, where AppKit cannot be asked.
    fn clipboard_generation(&self) -> u64 {
        if !super::types::is_main_thread() {
            return 0;
        }
        // SAFETY: AppKit singleton on the main thread; `changeCount` is a scalar accessor.
        unsafe {
            let pasteboard: id = msg_send![class!(NSPasteboard), generalPasteboard];
            if pasteboard == nil {
                return 0;
            }
            let count: i64 = msg_send![pasteboard, changeCount];
            count.max(0) as u64
        }
    }

    /// Whether the app's effective appearance is dark.
    ///
    /// `-[NSApplication effectiveAppearance]` resolves the appearance AppKit applies to the app,
    /// including a user's per-app override. The name is compared to `NSAppearanceNameDarkAqua`
    /// through its string value (`"NSAppearanceNameDarkAqua"`), which avoids declaring the
    /// Objective-C constant as an `extern static` in this legacy backend. Main-thread-only, so
    /// `None` off it.
    fn system_theme(&self) -> Option<bool> {
        if !super::types::is_main_thread() {
            return None;
        }
        // SAFETY: every object messaged is a live AppKit singleton or a retained child, and the
        // main thread is established above.
        unsafe {
            let app: id = msg_send![class!(NSApplication), sharedApplication];
            if app == nil {
                return None;
            }
            let appearance: id = msg_send![app, effectiveAppearance];
            if appearance == nil {
                return None;
            }
            let name: id = msg_send![appearance, name];
            if name == nil {
                return None;
            }
            let utf8: *const c_char = msg_send![name, UTF8String];
            if utf8.is_null() {
                return None;
            }
            // `name` is an autoreleased string owned by AppKit for the duration of the call; the
            // `CStr` is read before returning, so the caller's pool safety is not affected.
            let name = CStr::from_ptr(utf8).to_string_lossy();
            Some(name.contains("Dark"))
        }
    }

    /// Reads installed physical memory via `sysconf` (`_SC_PHYS_PAGES` x `_SC_PAGESIZE`).
    ///
    /// BSD spells this `sysctl hw.memsize`; `sysconf` answers the same question on the
    /// same kernel and needs no FFI dependency the crate does not already link.
    fn total_memory_mb(&self) -> Option<u64> {
        crate::platform::darwin_probes::total_memory_mb()
    }

    /// macOS ships `pmset`; `-g batt` prints a `'AC Power'`/`'Battery Power'` line.
    fn is_on_battery(&self) -> bool {
        crate::platform::darwin_probes::is_on_battery()
    }

    /// Samples RSS over virtual size via `ps -o rss=,vsz=`.
    fn process_memory_utilization(&self) -> Option<f32> {
        crate::platform::darwin_probes::process_memory_utilization()
    }

    /// CPU tick accounting has no lock-free macOS source here, so this reports
    /// `None` rather than a fabricated figure.
    fn process_cpu_utilization(&self) -> Option<f32> {
        None
    }

    /// Submits the job to the unix print spooler via [`crate::platform::os_probes`].
    fn spawn_print_job(&self, job_file: &std::path::Path) -> Result<(), String> {
        crate::platform::os_probes::spawn_print_job(job_file)
    }

    /// macOS ships CUPS, so `lp`/`lpr` are present on every normal install.
    fn has_print_support(&self) -> bool {
        crate::platform::types::unix_print_clients_available()
    }

    /// Renders menu accelerators with AppKit symbols (`⌘⇧Z`).
    fn shortcut_style(&self) -> crate::shortcut::PlatformShortcutStyle {
        crate::shortcut::PlatformShortcutStyle::Mac
    }
    fn init(&self) {
        // AppKit's `NSApplication` singleton may only be created/activated on the
        // main thread. Off-main we skip the native bootstrap entirely and leave
        // the backend in state mode (mirrors the `macos_objc2` preview backend).
        if !super::types::is_main_thread() {
            log::debug!("[macos] init skipped: not on the AppKit main thread (state-only mode)");
            return;
        }
        // SAFETY: NSAutoreleasePool::new(nil) is safe per Apple's documentation
        // (nil argument is allowed). NSApplication sharedApplication and messaging
        // are called on the main thread, which is required by Cocoa. All Objective-C
        // message sends use valid selectors from the cocoa/objc crates.
        unsafe {
            let pool = NSAutoreleasePool::new(nil);
            let app = NSApplication::sharedApplication(nil);
            app.setActivationPolicy_(NSApplicationActivationPolicyRegular);
            let _: () = msg_send![app, finishLaunching];
            let current_app = NSRunningApplication::currentApplication(nil);
            current_app.activateWithOptions_(
                NSApplicationActivationOptions::NSApplicationActivateIgnoringOtherApps,
            );
            pool.drain();
        }
    }
    fn run(&self) {
        // `-[NSApplication run]` must only be entered from the main thread; from
        // any other thread it would raise a foreign exception. Off-main callers
        // get a deterministic state-mode polling loop instead.
        if !super::types::is_main_thread() {
            log::debug!("[macos] run skipped: not on the AppKit main thread (state-only loop)");
            return;
        }
        // The frame heartbeat is installed here, once, **before** entering the run loop, so a session
        // ticks whether or not any window is created. It used to be a per-window `NSTimer` installed
        // by `create_window`, which meant a session with no window never ticked: a stop request or
        // any queued main-thread work had no reader, and the session could not end itself (designer
        // feedback `docs/issues/feedback-20261015.md` §十.2). The tick has to be scheduled on the
        // run loop before it runs; `-[NSApplication run]` does not return until the app stops, so
        // there is no point *after* it to install anything.
        #[cfg(widgets_unstripped)]
        // SAFETY: the main thread is established above, and the timer is scheduled on its run loop.
        unsafe {
            super::canvas::install_frame_heartbeat();
        }
        // The `-[NSApplication run]` call that follows must be on the main thread too.
        //
        // SAFETY: `NSApp()` returns the shared application instance initialized in
        // `init()`. `run()` must be called on the main thread, which the guard above
        // has established.
        unsafe {
            NSApp().run();
        }
    }
    /// Requests that the AppKit run loop stop.
    ///
    /// # Why the off-main case returns `false` instead of a quiet success
    ///
    /// `-[NSApplication stop:]` is main-thread-only, so this backend cannot honour a request
    /// that arrives on any other thread — and there is no safe way for it to hand the request
    /// to the main thread from here: the obvious selector (`performSelectorOnMainThread:…`) is
    /// banned by the `check_apple_thread_safety.sh` gate, and this crate links no dispatch
    /// queue to use instead. The refusal is therefore **reported** rather than logged, which is
    /// what lets a host that is not driving the loop itself (a smoke test, CI, a script)
    /// learn that its stop request will not take effect and arrange its own ending. It used to
    /// log and return `()`, so the same caller could not tell "asked" from "will stop".
    fn quit(&self) -> bool {
        // Wake the frame wait before the thread check: `wake()` is safe from any thread and
        // stops a loop parked on the idle interval from waiting out its timeout.
        crate::platform::frame_wait::wake();
        if !super::types::is_main_thread() {
            log::debug!("[macos] quit refused: NSApplication stop is main-thread-only");
            return false;
        }
        // SAFETY: NSApp().stop_(nil) is safe to call on the main thread after the
        // application has been initialized. The nil argument tells the app to stop
        // without a specific sender.
        unsafe {
            NSApp().stop_(nil);
            // `stop:` only raises a flag; `-[NSApplication run]` observes it when it next returns
            // from `nextEventMatchingMask:`. When the loop is servicing timers rather than waiting
            // for events — which is exactly what this backend's frame heartbeat makes it do — that
            // return can be indefinitely delayed, so an application-defined event is posted to wake
            // the loop. This is the documented companion to `stop:` for that case, and it is what
            // makes a bounded session end promptly instead of "still running after 20 s"
            // (`docs/issues/feedback-20261015.md` §十.2).
            let event: id = msg_send![
                class!(NSEvent),
                otherEventWithType: NS_EVENT_TYPE_APPLICATION_DEFINED
                location: NSPoint::new(0.0, 0.0)
                modifierFlags: 0u64
                timestamp: 0.0f64
                windowNumber: 0i64
                context: nil
                subtype: 0i16
                data1: 0i64
                data2: 0i64
            ];
            if event != nil {
                let _: () = msg_send![NSApp(), postEvent: event atStart: YES];
            }
        }
        true
    }
    /// Pops the next typed widget-trigger event from this backend's queue.
    ///
    /// # Why this backend must delegate
    ///
    /// [`PlatformCapabilities::typed_widget_trigger`] claims this backend produces and delivers
    /// typed triggers, and it holds a `BackendState` (the shared queue) exactly like `harmony`,
    /// `android`, `ios`, `wayland`, `windows` and `mobile` — every one of which writes these two
    /// lines. This backend did not, so both methods fell through to the trait defaults (`None` /
    /// `false`) while the flag said `true`.
    ///
    /// The gap is reachable rather than theoretical: `NativeControlBackend` forwards both straight
    /// to `get_platform()`, so an injected trigger was accepted and then dropped on the floor.
    fn poll_widget_trigger_event(&self) -> Option<crate::platform::WidgetTriggerEvent> {
        self.state.pop_widget_trigger_event()
    }

    /// Pushes a typed widget-trigger event, refusing ids this backend never made.
    fn inject_widget_trigger_event(
        &self,
        widget_id: ObjectId,
        kind: crate::platform::WidgetTriggerKind,
    ) -> bool {
        self.state.inject_widget_trigger_event(widget_id, kind)
    }

    /// Pops the next pending trigger as a bare id, over the same queue as the typed view.
    fn poll_widget_triggered(&self) -> Option<ObjectId> {
        self.poll_widget_trigger_event().map(|event| event.widget_id)
    }

    fn destroy_widget(&self, widget_id: ObjectId) -> bool {
        // Take the native handle out under its own lock, then release the guard
        // before messaging AppKit (the two mutexes are never held together).
        let handle = crate::compat::lock(&self.handles).remove(&widget_id);
        // Drop the per-widget accessibility registration that `register_handle` added.
        self.a11y_bridge.unregister_handle(widget_id);
        // Native teardown for a window this backend really created: close it (so it
        // leaves the screen) and release the `alloc`-owned reference `create_window`
        // took. Without this, `destroy_widget` only deleted a `Copy` `usize` bookkeeping
        // entry and the OS window stayed visible with its reference never balanced
        // (D08-P-01). The close/release are main-thread-only, so they are skipped
        // off-main, where `create_window` also never constructed a native object.
        if let Some(handle) = handle {
            if handle.ptr != 0
                && matches!(handle.kind, HandleKind::Window)
                && super::types::is_main_thread()
            {
                // SAFETY: `handle.ptr` is the `NSWindow*` this backend obtained from
                // `NSWindow::alloc(..).initWithContentRect_..(..)` in `create_window`, so
                // it is a valid owned AppKit object. `close`/`release` are the balanced
                // teardown for that `alloc`, and both are main-thread-only (checked above).
                unsafe {
                    let window = handle.ptr as id;
                    let _: () = msg_send![window, close];
                    let _: () = msg_send![window, release];
                }
            }
        }
        // The state record is the authority for whether the widget existed.
        self.state.destroy_widget(widget_id)
    }
    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        // Off-main (e.g. the C ABI called from a worker thread or unit tests),
        // never construct `NSWindow`: AppKit raises a foreign exception that
        // aborts the process. Register a state-only handle instead so every
        // caller still receives a valid, text/geometry-consistent widget id.
        if !super::types::is_main_thread() {
            let id =
                self.register_state_only_handle(HandleKind::Window, title, x, y, width, height);
            // `window_style()` is titled + closable + resizable + miniaturizable,
            // so a fresh macOS window starts resizable and decorated.
            self.state
                .init_window_state(id, crate::platform::state::WindowStateRecord::new_window());
            return id;
        }
        // SAFETY: Cocoa APIs require the main thread, guaranteed by the platform contract.
        // NSAutoreleasePool::new(nil) is safe with nil argument. All Objective-C messages
        // use valid selectors from the cocoa crate. Self::register_handle() stores the
        // raw pointer cast as usize without aliasing issues. Nil returns from alloc are
        // checked and logged before proceeding.
        unsafe {
            let pool = NSAutoreleasePool::new(nil);

            // Check for nil after NSWindow::alloc — Cocoa returns nil on allocation failure.
            let raw_window = NSWindow::alloc(nil);
            if raw_window == nil {
                log::error!("[macos] create_window: NSWindow::alloc returned nil (out of memory?)");
                pool.drain();
                return 0;
            }
            let window = raw_window.initWithContentRect_styleMask_backing_defer_(
                Self::make_rect(x, y, width, height),
                Self::window_style(),
                NSBackingStoreBuffered,
                NO,
            );

            // Check for nil after NSView::alloc
            //
            // A profile without the library canvas (`mini`/`embedded`) gets a plain `NSView` as its
            // content area. The canvas profile installs its own drawing view below, so this is
            // compiled out there rather than allocated and thrown away.
            #[cfg(not(widgets_unstripped))]
            {
                let raw_content = NSView::alloc(nil);
                let content_view = if raw_content == nil {
                    log::error!(
                        "[macos] create_window: NSView::alloc returned nil (out of memory?)"
                    );
                    pool.drain();
                    return 0;
                } else {
                    NSView::initWithFrame_(raw_content, Self::make_rect(0, 0, width, height))
                };
                let _: () = msg_send![window, setContentView: content_view];
            }

            // Check for nil after NSString::alloc for the title
            let raw_title = NSString::alloc(nil);
            if raw_title == nil {
                log::error!("[macos] create_window: NSString::alloc returned nil (out of memory?)");
                pool.drain();
                return 0;
            }

            window.cascadeTopLeftFromPoint_(NSPoint::new(20.0, 20.0));
            NSWindow::setTitle_(window, raw_title.init_str(title));
            // The id has to exist before the canvas can be tagged with it, so `register_handle`
            // runs before the content view is installed. The canvas stores the **host** id and
            // paints the window's whole widget tree (`render_frame_tree`); a plain `NSView` here is
            // why the client area of every cocoa window used to stay blank — nothing ever drew the
            // children, which are not mounted surfaces.
            let id = self.register_handle(
                HandleKind::Window,
                title,
                x,
                y,
                width,
                height,
                window as usize,
            );
            #[cfg(widgets_unstripped)]
            if let Some(canvas) = super::canvas::make_window_canvas(id, width, height) {
                let _: () = msg_send![window, setContentView: canvas];
            }
            window.makeKeyAndOrderFront_(nil);
            let _: () = msg_send![window, display];
            // A window resized by the user then re-runs the host's layout instead of keeping stale
            // child geometry.
            //
            // Gated on `widgets_unstripped` because `macos::canvas` is: a
            // `macos-legacy + mini|embedded` build has no `NSView` canvas to install a
            // resize delegate on, and referencing the module ungated failed to compile
            // with `cannot find canvas in super`. The state-only handle above is the
            // whole window contract for those profiles, so there is nothing to
            // delegate a resize to.
            #[cfg(widgets_unstripped)]
            super::canvas::install_resize_delegate(window as id, id);
            self.state
                .init_window_state(id, crate::platform::state::WindowStateRecord::new_window());
            pool.drain();
            id
        }
    }

    /// The window's current client size, asked of AppKit.
    ///
    /// `contentView.bounds` is the authority once the user has dragged the window edge;
    /// the recorded size is only a fallback for an id with no live `NSWindow`.
    #[cfg(target_os = "macos")]
    fn window_client_size(&self, window_id: ObjectId) -> Option<(u32, u32)> {
        // SAFETY: `view_for`/`get_native_handle` return pointers this backend created,
        // and the messages sent are read-only AppKit accessors. Off-main is excluded by
        // the thread check, because AppKit must only be driven from the main thread.
        unsafe {
            if super::types::is_main_thread() {
                if let Some(handle) = self.get_native_handle(window_id) {
                    let window = handle as id;
                    let content: id = msg_send![window, contentView];
                    if content != nil {
                        let bounds: NSRect = msg_send![content, bounds];
                        let width = bounds.size.width.max(0.0) as u32;
                        let height = bounds.size.height.max(0.0) as u32;
                        if width > 0 && height > 0 {
                            return Some((width, height));
                        }
                    }
                }
            }
        }
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

    fn menu_item_shortcut(&self, menu_item: ObjectId) -> Option<String> {
        let shortcuts = crate::compat::try_lock(&self.menu_item_shortcuts)?;
        shortcuts.get(&menu_item).cloned().filter(|text| !text.is_empty())
    }
    fn get_native_handle(&self, widget: ObjectId) -> Option<usize> {
        // A handle with a null pointer means the widget exists only as logical
        // state, so there is no native object to hand out.
        let handle = self.get_handle(widget)?;
        if handle.ptr == 0 {
            return None;
        }
        Some(handle.ptr)
    }

    /// Applies a cursor shape through `NSCursor`.
    ///
    /// AppKit's per-view cursor rects would keep the shape across motion, but they need a view
    /// subclass hook; `-[NSCursor set]` is the immediate call AppKit documents for a cursor a
    /// control sets while it is hovered. Shapes with no exact AppKit cursor fall back to the
    /// closest one (diagonal resize → horizontal resize, zoom → crosshair).
    fn set_cursor(&self, _window: ObjectId, icon: crate::platform::CursorIcon) -> bool {
        use crate::platform::CursorIcon;
        if !super::types::is_main_thread() {
            return false;
        }
        // SAFETY: every cursor is an AppKit singleton; `set` is a main-thread UI call.
        unsafe {
            let cursor: id = match icon {
                CursorIcon::Default => msg_send![class!(NSCursor), arrowCursor],
                CursorIcon::Pointer => msg_send![class!(NSCursor), pointingHandCursor],
                CursorIcon::Text => msg_send![class!(NSCursor), IBeamCursor],
                CursorIcon::Crosshair | CursorIcon::ZoomIn | CursorIcon::ZoomOut => {
                    msg_send![class!(NSCursor), crosshairCursor]
                }
                CursorIcon::Move => msg_send![class!(NSCursor), openHandCursor],
                CursorIcon::NotAllowed => msg_send![class!(NSCursor), operationNotAllowedCursor],
                CursorIcon::ResizeHorizontal | CursorIcon::ResizeNwSe | CursorIcon::ResizeNeSw => {
                    msg_send![class!(NSCursor), resizeLeftRightCursor]
                }
                CursorIcon::ResizeVertical => msg_send![class!(NSCursor), resizeUpDownCursor],
                CursorIcon::Grab => msg_send![class!(NSCursor), openHandCursor],
                CursorIcon::Grabbing => msg_send![class!(NSCursor), closedHandCursor],
                CursorIcon::Wait | CursorIcon::Progress => {
                    msg_send![class!(NSCursor), arrowCursor]
                }
                CursorIcon::Help => msg_send![class!(NSCursor), pointingHandCursor],
            };
            if cursor == nil {
                return false;
            }
            let _: () = msg_send![cursor, set];
        }
        true
    }

    /// Hides or shows the pointer through `NSCursor`.
    fn set_cursor_visible(&self, _window: ObjectId, visible: bool) -> bool {
        if !super::types::is_main_thread() {
            return false;
        }
        // SAFETY: AppKit cursor class methods on the main thread.
        unsafe {
            if visible {
                let cursor: id = msg_send![class!(NSCursor), arrowCursor];
                let _: () = msg_send![cursor, set];
            } else {
                let _: () = msg_send![class!(NSCursor), hide];
            }
        }
        true
    }

    /// The window's `NSView` as a `raw-window-handle` 0.6 AppKit handle.
    ///
    /// rwh names an `NSView`, not an `NSWindow`, because that is what a GPU surface is created
    /// for. A window handle is therefore translated to its `contentView`; a canvas handle is
    /// already the view. Off the main thread AppKit cannot be messaged, so the answer is `None`.
    #[cfg(all(feature = "raw-window-handle", target_os = "macos"))]
    fn raw_window_handle(&self, window: ObjectId) -> Option<raw_window_handle::RawWindowHandle> {
        if !super::types::is_main_thread() {
            return None;
        }
        let handle = self.get_handle(window)?;
        if handle.ptr == 0 {
            return None;
        }
        // SAFETY: the handle points at an AppKit object this backend created on the main thread.
        let view = unsafe {
            match handle.kind {
                HandleKind::Window => {
                    let content: id = msg_send![handle.ptr as id, contentView];
                    if content == nil {
                        return None;
                    }
                    content as *mut core::ffi::c_void
                }
                #[cfg(widgets_unstripped)]
                HandleKind::Canvas => handle.ptr as *mut core::ffi::c_void,
            }
        };
        let ns_view = core::ptr::NonNull::new(view)?;
        Some(raw_window_handle::RawWindowHandle::AppKit(
            raw_window_handle::AppKitWindowHandle::new(ns_view),
        ))
    }

    /// AppKit needs no display object for a surface; the empty variant is the documented answer.
    #[cfg(all(feature = "raw-window-handle", target_os = "macos"))]
    fn raw_display_handle(&self) -> Option<raw_window_handle::RawDisplayHandle> {
        Some(raw_window_handle::RawDisplayHandle::AppKit(
            raw_window_handle::AppKitDisplayHandle::new(),
        ))
    }
    // `poll_menu_triggered` / `inject_menu_trigger` are **not** overridden: this backend owns no
    // native menu (the `NSMenu` bar was removed — see `status.md`), so nothing can ever queue an
    // activation and the trait defaults (`None` / `false`) are the honest answers. There used to be
    // a `MENU_EVENTS` static and a `Vec::remove(0)` poll behind these methods; nothing in the crate
    // pushed to it, so every poll paid a mutex lock plus a memmove to read an always-empty queue.

    /// Drives a menu item through AppKit's own action dispatch.
    ///
    /// `NSMenuItem -performActionForItemAtIndex:` is the single path AppKit takes for
    /// both a mouse click and a matched key equivalent, so a probe that reaches it is
    /// exercising the real menu route rather than a synthetic event.
    ///
    /// Off the main thread, or with no live `NSMenuItem`, this returns `false`: AppKit
    /// may only be driven from the main thread, and a state-only item has no action to
    /// send.
    fn activate_menu_item(&self, menu_item: ObjectId) -> bool {
        let Some(handle) = self.get_native_handle(menu_item) else {
            return false;
        };
        if !super::types::is_main_thread() {
            return false;
        }
        // SAFETY: `get_native_handle` returns a pointer this backend created for a live
        // `NSMenuItem`; the messages sent are the read-only `action`/`target` accessors
        // and the item's own action dispatch. Main-thread-only is enforced above.
        unsafe {
            let item = handle as id;
            if item == nil {
                return false;
            }
            let action: objc::runtime::Sel = msg_send![item, action];
            // A menu item with no action carries a null selector; comparing against
            // `Sel::from_ptr(null)` is how objc 0.2 exposes that check.
            if action == objc::runtime::Sel::from_ptr(std::ptr::null()) {
                return false;
            }
            let target: id = msg_send![item, target];
            if target == nil {
                return false;
            }
            // Route through `NSApplication -sendAction:to:from:`, the documented
            // dispatch entry point, rather than `performSelector:withObject:`.
            //
            // Two reasons. First, `performSelector:` is the API the project's
            // `check_apple_thread_safety.sh` gate bans in Apple native code (rule C),
            // because it bypasses the action machinery and its selector is unchecked at
            // runtime. Second, `sendAction:to:from:` is what a real menu click goes
            // through: the sender is passed as the `from:` argument, so a handler that
            // reads its sender (a common `validateMenuItem:`/action pattern) sees the
            // item rather than `nil`. `performSelector:withObject:` passes the item as a
            // plain argument, which an `NSMenuItem` action target does not expect.
            let app: id = msg_send![objc::class!(NSApplication), sharedApplication];
            if app == nil {
                return false;
            }
            let _: bool = msg_send![app, sendAction: action to: target from: item];
        }
        true
    }
    fn set_clipboard_text(&self, text: &str) -> bool {
        // `NSPasteboard` is a window-server singleton and may only be touched on
        // the AppKit main thread; off-main we go straight to state.
        if !super::types::is_main_thread() {
            return self.state.set_clipboard_text(text);
        }
        // Try real NSPasteboard integration first
        // SAFETY: runs on the main thread with an autorelease pool; `catch_unwind` contains any panic.
        let result = std::panic::catch_unwind(|| unsafe {
            let pb: id = msg_send![class!(NSPasteboard), generalPasteboard];
            if pb == nil {
                return false;
            }
            let _: () = msg_send![pb, clearContents];
            let ns_str = autoreleased_nsstring(text);
            let type_str = autoreleased_nsstring("public.utf8-plain-text");
            let success: BOOL = msg_send![pb, setString:ns_str forType:type_str];
            success != NO
        });
        // Fall back to state on ObjC failure (including panics)
        result.unwrap_or_else(|_| self.state.set_clipboard_text(text))
    }
    fn get_clipboard_text(&self) -> String {
        // `NSPasteboard` is main-thread-only; state is the off-main source.
        if !super::types::is_main_thread() {
            return self.state.clipboard_text();
        }
        // Try real NSPasteboard integration first
        // SAFETY: runs on the main thread with an autorelease pool; `catch_unwind` contains any panic.
        let result = std::panic::catch_unwind(|| unsafe {
            let pb: id = msg_send![class!(NSPasteboard), generalPasteboard];
            if pb == nil {
                return None;
            }
            let type_str = autoreleased_nsstring("public.utf8-plain-text");
            let text_obj: id = msg_send![pb, stringForType:type_str];
            if text_obj == nil {
                return None;
            }
            let c_str: *const c_char = msg_send![text_obj, UTF8String];
            if c_str.is_null() {
                return None;
            }
            Some(CStr::from_ptr(c_str).to_string_lossy().into_owned())
        });
        // Fall back to state on ObjC failure
        result.unwrap_or(None).unwrap_or_else(|| self.state.clipboard_text())
    }
    fn ime_bridge(&self) -> Option<&dyn ImeBridge> {
        Some(&self.ime_bridge)
    }

    /// Reads the record the backend keeps for every id it allocated.
    ///
    /// Both allocation paths — a real `NSWindow` on the main thread and a
    /// state-only handle off it — insert a [`crate::platform::state::WidgetRecord`],
    /// so text/geometry/enabled/visible are answered from that one record. Without
    /// these overrides the trait defaults would report empty text for a window the
    /// backend demonstrably created.
    fn get_widget_text(&self, widget_id: ObjectId) -> String {
        self.state.text(widget_id)
    }

    /// Updates a control's label and, for a native window, its AppKit title.
    ///
    /// The answer is whether the id names a widget this backend recorded — the same answer
    /// `BackendState::set_text` gives (P-203). The AppKit title write is best-effort on top of it
    /// and cannot fail in a way that would change the answer: a handle that is not a window is
    /// simply not a title to update.
    fn set_widget_text(&self, widget_id: ObjectId, text: &str) -> bool {
        let stored = self.state.set_text(widget_id, text);
        // Keep the live AppKit title in step when there is a native window. A
        // state-only handle has `ptr == 0`, so the message is skipped entirely.
        if let Some(handle) = self.get_handle(widget_id) {
            if handle.ptr != 0 && handle.kind == HandleKind::Window {
                // SAFETY: the handle was created in this process for an NSWindow
                // and `setTitle:` is declared by NSWindow.
                unsafe {
                    let window = Self::as_id(handle);
                    let title = autoreleased_nsstring(text);
                    NSWindow::setTitle_(window, title);
                }
            }
        }
        stored
    }

    fn set_widget_enabled(&self, widget_id: ObjectId, enabled: bool) {
        self.state.set_enabled(widget_id, enabled);
    }

    fn is_widget_enabled(&self, widget_id: ObjectId) -> bool {
        self.state.enabled(widget_id)
    }

    /// Answers whether the id names a widget this backend recorded (P-203).
    fn set_widget_visible(&self, widget_id: ObjectId, visible: bool) -> bool {
        self.state.set_visible(widget_id, visible)
    }

    fn is_widget_visible(&self, widget_id: ObjectId) -> bool {
        self.state.visible(widget_id)
    }

    /// Answers whether the id names a widget this backend recorded (P-203).
    fn set_widget_geometry(
        &self,
        widget_id: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> bool {
        self.state.set_geometry(widget_id, x, y, width, height)
    }

    /// Window state (maximised/minimised/fullscreen/resizable) round-trips
    /// through the record, so it works for the off-main state-only path too.
    fn set_window_state(
        &self,
        widget_id: ObjectId,
        flag: crate::platform::WindowStateFlag,
        on: bool,
    ) -> bool {
        use crate::platform::WindowStateFlag;
        // Record first: the state is the fallback when no AppKit window exists (off-main, or a
        // state-only handle), and it is what `is_window_in_state` reads in that case.
        let recorded = self.state.set_window_state(widget_id, flag, on);
        if !super::types::is_main_thread() {
            return recorded;
        }
        let Some(window) = resolve_window_ptr(self, widget_id) else {
            return recorded;
        };
        // SAFETY: `window` is an `NSWindow` this backend created on the AppKit main thread, and
        // every message below is a documented `NSWindow` operation whose result is checked.
        unsafe {
            match flag {
                WindowStateFlag::Maximized => {
                    let zoomed: BOOL = msg_send![window, isZoomed];
                    if on != (zoomed != NO) {
                        NSWindow::zoom_(window, nil);
                    }
                }
                WindowStateFlag::Minimized => {
                    let minimized: BOOL = msg_send![window, isMiniaturized];
                    if on && minimized == NO {
                        NSWindow::performMiniaturize_(window, nil);
                    } else if !on && minimized != NO {
                        NSWindow::deminiaturize_(window, nil);
                    }
                }
                WindowStateFlag::Fullscreen => {
                    let mask: NSWindowStyleMask = msg_send![window, styleMask];
                    let fullscreen = mask.contains(NSWindowStyleMask::NSFullScreenWindowMask);
                    if on != fullscreen {
                        NSWindow::toggleFullScreen_(window, nil);
                    }
                }
                WindowStateFlag::Resizable => {
                    let mask: NSWindowStyleMask = msg_send![window, styleMask];
                    let mut updated = mask;
                    updated.set(NSWindowStyleMask::NSResizableWindowMask, on);
                    if updated != mask {
                        window.setStyleMask_(updated);
                    }
                }
                WindowStateFlag::Decorated => {
                    let mask: NSWindowStyleMask = msg_send![window, styleMask];
                    let mut updated = mask;
                    let chrome = NSWindowStyleMask::NSTitledWindowMask
                        | NSWindowStyleMask::NSClosableWindowMask
                        | NSWindowStyleMask::NSMiniaturizableWindowMask
                        | NSWindowStyleMask::NSResizableWindowMask;
                    if on {
                        updated.insert(chrome);
                    } else {
                        updated.remove(chrome);
                    }
                    if updated != mask {
                        window.setStyleMask_(updated);
                    }
                }
            }
        }
        true
    }

    fn is_window_in_state(
        &self,
        widget_id: ObjectId,
        flag: crate::platform::WindowStateFlag,
    ) -> Option<bool> {
        use crate::platform::WindowStateFlag;
        if super::types::is_main_thread() {
            if let Some(window) = resolve_window_ptr(self, widget_id) {
                // SAFETY: a live `NSWindow` on the AppKit main thread; every accessor is read-only.
                unsafe {
                    let mask: NSWindowStyleMask = msg_send![window, styleMask];
                    return match flag {
                        WindowStateFlag::Maximized => {
                            let zoomed: BOOL = msg_send![window, isZoomed];
                            Some(zoomed != NO)
                        }
                        WindowStateFlag::Minimized => {
                            let minimized: BOOL = msg_send![window, isMiniaturized];
                            Some(minimized != NO)
                        }
                        WindowStateFlag::Fullscreen => {
                            Some(mask.contains(NSWindowStyleMask::NSFullScreenWindowMask))
                        }
                        WindowStateFlag::Resizable => {
                            Some(mask.contains(NSWindowStyleMask::NSResizableWindowMask))
                        }
                        WindowStateFlag::Decorated => {
                            Some(mask.contains(NSWindowStyleMask::NSTitledWindowMask))
                        }
                    };
                }
            }
        }
        // No native window (or not on the main thread): the recorded state is the fallback.
        self.state.window_state(widget_id, flag)
    }

    fn set_window_min_size(&self, widget_id: ObjectId, width: u32, height: u32) -> bool {
        let recorded = self.state.set_window_min_size(widget_id, width, height);
        if !super::types::is_main_thread() {
            return recorded;
        }
        let Some(window) = resolve_window_ptr(self, widget_id) else {
            return recorded;
        };
        // SAFETY: `window` is an `NSWindow` on the AppKit main thread; `setContentMinSize:`
        // constrains the content view, which is what the library paints into.
        unsafe {
            let size = NSSize::new(width as f64, height as f64);
            window.setContentMinSize_(size);
        }
        true
    }

    fn window_min_size(&self, widget_id: ObjectId) -> Option<(u32, u32)> {
        if super::types::is_main_thread() {
            if let Some(window) = resolve_window_ptr(self, widget_id) {
                // SAFETY: a live `NSWindow` on the AppKit main thread; `contentMinSize` is a
                // scalar struct accessor.
                unsafe {
                    let size: NSSize = msg_send![window, contentMinSize];
                    if size.width > 0.0 && size.height > 0.0 {
                        return Some((size.width.round() as u32, size.height.round() as u32));
                    }
                }
            }
        }
        self.state.window_min_size(widget_id)
    }

    /// AppKit window icons are a property of the application bundle, not of one window.
    ///
    /// `-[NSWindow setRepresentation:]` exists for document proxies but does not set the icon the
    /// window manager displays; the Dock and window-switcher icon comes from `CFBundleIconFile`.
    /// Reporting `false` is the honest answer (BLUE28 B28.19) and stops `WindowHandle::icon` from
    /// mirroring a path the OS never accepted.
    fn set_window_icon(&self, _widget_id: ObjectId, _path: &str) -> bool {
        false
    }

    fn set_widget_ime_enabled(&self, widget_id: ObjectId, enabled: bool) -> bool {
        self.state.set_ime_enabled(widget_id, enabled)
    }

    fn is_widget_ime_enabled(&self, widget_id: ObjectId) -> bool {
        self.state.ime_enabled(widget_id)
    }

    fn set_widget_accessibility_name(&self, widget_id: ObjectId, name: &str) -> bool {
        self.state.set_accessibility_name(widget_id, name)
    }

    fn get_widget_accessibility_name(&self, widget_id: ObjectId) -> String {
        self.state.accessibility_name(widget_id)
    }

    fn clipboard_backend(&self) -> Option<&dyn RichClipboardBackend> {
        Some(&self.clipboard)
    }

    // ── Drag and drop ───────────────────────────────────────────────────────
    //
    // # The gap this closes
    //
    // This backend implemented none of the three drag-and-drop methods, so it inherited the
    // trait defaults (`false`/`None`) and a drag started here did nothing at all — while the
    // same call worked on Windows, on every mobile backend and on the portable host. There was
    // no error to notice: `begin_drag` returns `bool` and the callers that ignore it, ignore it.
    //
    // # What is implemented, and what is not
    //
    // These are the **state-backed** drag and drop the Windows backend also uses: a payload and
    // a MIME type are recorded against the source widget, and a drop event is queued for the
    // host to poll. That is a complete, usable in-process contract — a host that owns its own
    // drag session (an `NSDraggingSession`, an ArkUI drag, a browser `dragstart`) reports the
    // result through `inject_drop_event`, and a library-internal drag works end to end without
    // any AppKit involvement.
    //
    // What is deliberately **not** here is a native `NSDraggingSession`: starting and driving
    // one requires an `NSPasteboard`, an `IDraggingSource` implementation and a live event loop,
    // none of which can be verified on a Linux host. Claiming it by writing the code and never
    // running it is the failure mode the project's own standards forbid; the state contract
    // above is what can be delivered honestly today, and the Windows backend records the same
    // choice in the same words.

    /// Records a drag originating at `source_widget_id`, with its payload.
    ///
    /// Returns `false` for an id this backend did not create, so a drag cannot be started from
    /// a widget that is not there.
    fn begin_drag(&self, source_widget_id: ObjectId, mime: &str, payload: &[u8]) -> bool {
        self.state.begin_drag(source_widget_id, mime, payload)
    }

    /// Pops the next drop event, if one is queued.
    fn poll_drop_event(&self) -> Option<crate::platform::DropEvent> {
        self.state.pop_drop_event()
    }

    /// Queues a drop event, as a host-reported drop would.
    ///
    /// # Why the refusal is checked here and not left to the queue
    ///
    /// A drop naming a widget that does not exist could only be dispatched against a recycled
    /// id, so an unknown target is refused rather than queued.
    fn inject_drop_event(&self, event: crate::platform::DropEvent) -> bool {
        if !self.state.contains_widget(event.target_widget_id) {
            return false;
        }
        self.state.inject_drop_event(event)
    }

    fn accessibility_bridge(&self) -> Option<&dyn AccessibilityBridge> {
        Some(&self.a11y_bridge)
    }
}
