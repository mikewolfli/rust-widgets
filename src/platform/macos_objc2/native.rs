// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Native AppKit FFI wrappers for the macOS objc2 backend (BLUE11 R1.5/R2.3 100%).
//!
//! This module is gated behind `#[cfg(target_os = "macos")]` and the `macos`
//! feature flag (`objc2-macos` is only an alias that enables `macos`).
//!
//! # The presentation path
//!
//! `macos_objc2` is a self-drawn backend: every `WidgetKind` is painted by
//! `src/widget/`, so the only native object it needs is a **surface** that pulls
//! one RGBA frame out of [`crate::widget::runtime`] and blits it. That surface is
//! [`RustWidgetsObjc2CanvasView`], an `NSView` subclass whose `drawRect:` mirrors
//! `macos/canvas.rs::draw_rect` for the sibling cocoa backend.
//!
//! Without it `MacOSObjc2Platform::supports_surfaces()` promised a presentation
//! path that did not exist: `mount_surface` recorded bookkeeping and no pixels
//! ever reached the screen.

#![cfg(target_os = "macos")]
#![cfg(feature = "macos")]
// objc2 init methods may require unsafe blocks depending on platform
#![allow(unused_unsafe)]

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::sel;
use objc2::{define_class, msg_send, AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSAccessibilityButtonRole, NSAccessibilityCheckBoxRole, NSAccessibilityComboBoxRole,
    NSAccessibilityGroupRole, NSAccessibilityImageRole, NSAccessibilityIncrementorRole,
    NSAccessibilityLinkRole, NSAccessibilityListRole, NSAccessibilityMenuItemRole,
    NSAccessibilityMenuRole, NSAccessibilityOutlineRole, NSAccessibilityProgressIndicatorRole,
    NSAccessibilityRadioButtonRole, NSAccessibilityRole, NSAccessibilitySliderRole,
    NSAccessibilityStaticTextRole, NSAccessibilityTabGroupRole, NSAccessibilityTableRole,
    NSAccessibilityTextFieldRole, NSAccessibilityUnknownRole, NSAccessibilityWindowRole,
    NSAppearanceNameDarkAqua, NSApplication, NSApplicationActivationPolicy, NSBackingStoreType,
    NSColor, NSColorSpace, NSCursor, NSEventMask, NSScreen, NSTrackingArea, NSTrackingAreaOptions,
    NSView, NSWindow, NSWindowStyleMask, NSWorkspace,
};
// Named only by the input handlers and the blit, both of which are `widgets_unstripped` code.
#[cfg(widgets_unstripped)]
use objc2_app_kit::{NSEvent, NSGraphicsContext};
// The blit is the only CGContext consumer, and it is `widgets_unstripped` code.
#[cfg(widgets_unstripped)]
use objc2_core_graphics::CGContext;
use objc2_foundation::{
    NSArray, NSAttributedString, NSAttributedStringKey, NSDate, NSDefaultRunLoopMode, NSEdgeInsets,
    NSLocale, NSNotification, NSRange, NSRect, NSSize, NSString,
};

// `Vec` is not in the `core` prelude and this file is compiled on profiles that have no `std`
// prelude either; the crate's own shim is the one place that decides where it comes from.
use crate::compat::{MiniToString, Vec};

/// The process-wide IME bridge shared by the canvas view and `Platform::ime_bridge` (B28.12).
///
/// One `NSTextInputContext` exists per application, and every canvas view reports into the same
/// focused-widget state, so a single bridge is the accurate model. Keeping it here (rather than
/// on `MacOSObjc2Platform`) is what lets the Objective-C callbacks reach it without carrying a
/// pointer to a Rust struct through an ivar.
static IME_BRIDGE: std::sync::OnceLock<crate::platform::ime_macos::MacOsImeBridge> =
    std::sync::OnceLock::new();

/// The bridge `Platform::ime_bridge` hands out and the view's callbacks drive.
pub(crate) fn ime_bridge() -> &'static crate::platform::ime_macos::MacOsImeBridge {
    IME_BRIDGE.get_or_init(crate::platform::ime_macos::MacOsImeBridge::new)
}

thread_local! {
    /// True while `keyDown:` is inside `interpretKeyEvents:`.
    static IME_INTERPRETING: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
    /// Set by an `NSTextInputClient` callback that consumed the event.
    static IME_HANDLED: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
}
// The tree-routing half of this file — the routes below and the mouse/key handlers that call them —
// is gated on `widgets_unstripped`, so these three are gated with it: `mini`/`embedded` compile the
// file but have no `widget::runtime` to route into, and an unused import is a build failure for a
// downstream `-D warnings` consumer.
#[cfg(widgets_unstripped)]
use crate::core::{ObjectId, Point};
#[cfg(widgets_unstripped)]
use crate::event::Event;
#[cfg(widgets_unstripped)]
use crate::platform::types::MousePhase;
// Display enumeration and safe-area reads are `Platform` facts, not paint-path facts, so these are
// used on every profile that compiles this backend.
use crate::core::Rect;
use crate::platform::types::{Display, DisplayId, SafeAreaInsets};

use std::collections::HashMap;
use std::sync::LazyLock;
use std::sync::Mutex;

// ─── System appearance ───

/// Whether the app's effective appearance is dark.
///
/// `NSApplication.effectiveAppearance` is the appearance AppKit resolved for the app (including a
/// user's explicit per-app override), which is why it is read instead of the `AppleInterfaceStyle`
/// default. It is main-thread-only, so this answers `None` off the main thread. Any name other
/// than `NSAppearanceNameDarkAqua` is light: AppKit's only two base appearances are Aqua and Dark
/// Aqua, and the high-contrast variants inherit from them.
pub(crate) fn system_theme() -> Option<bool> {
    let mtm = MainThreadMarker::new()?;
    let app = NSApplication::sharedApplication(mtm);
    let appearance = app.effectiveAppearance();
    let name = appearance.name();
    // SAFETY: `NSAppearanceNameDarkAqua` is a well-known, immutable AppKit constant.
    let dark_name = unsafe { NSAppearanceNameDarkAqua };
    Some(&*name == dark_name)
}

// ─── Cursor (B28.09) ───

/// Applies a cursor shape through `NSCursor.set`.
///
/// `None` off the main thread. Shapes without an exact AppKit cursor fall back to the closest
/// one; the mapping is here so every caller gets the same fallback.
#[allow(deprecated)] // Newer AppKit asks for a position/direction; the axis cursors are the portable pair.
pub(crate) fn set_cursor(icon: crate::platform::CursorIcon) -> bool {
    use crate::platform::CursorIcon;
    if MainThreadMarker::new().is_none() {
        return false;
    }
    let cursor = match icon {
        CursorIcon::Default => NSCursor::arrowCursor(),
        CursorIcon::Pointer | CursorIcon::Help => NSCursor::pointingHandCursor(),
        CursorIcon::Text => NSCursor::IBeamCursor(),
        CursorIcon::Crosshair | CursorIcon::ZoomIn | CursorIcon::ZoomOut => {
            NSCursor::crosshairCursor()
        }
        CursorIcon::Move | CursorIcon::Grab => NSCursor::openHandCursor(),
        CursorIcon::Grabbing => NSCursor::closedHandCursor(),
        CursorIcon::NotAllowed => NSCursor::operationNotAllowedCursor(),
        CursorIcon::ResizeHorizontal | CursorIcon::ResizeNwSe | CursorIcon::ResizeNeSw => {
            NSCursor::resizeLeftRightCursor()
        }
        CursorIcon::ResizeVertical => NSCursor::resizeUpDownCursor(),
        CursorIcon::Wait | CursorIcon::Progress => NSCursor::arrowCursor(),
    };
    cursor.set();
    true
}

/// Hides or shows the pointer through the AppKit cursor singleton.
pub(crate) fn set_cursor_visible(visible: bool) -> bool {
    if MainThreadMarker::new().is_none() {
        return false;
    }
    if visible {
        NSCursor::arrowCursor().set();
    } else {
        NSCursor::hide();
    }
    true
}

// ─── System fonts (B28.39) ───

/// The installed families, their style names and backing files, through CoreText.
///
/// `NSFontManager.availableFontFamilies` reports names only; CoreText's `kCTFontURLAttribute` is
/// what supplies the file the renderer can register. The enumeration is not main-thread-only.
pub(crate) fn system_fonts() -> Vec<crate::platform::types::SystemFont> {
    crate::platform::fonts_apple::system_fonts()
}

// ─── Environment facts (B28.06) ───

/// How long a cached `NSWorkspace`/`NSLocale` fact may be reused.
///
/// `Platform::environment_generation` reads each of these facts on every style-environment poll
/// (250 ms, `src/style/environment.rs`). One second is the same TTL the Windows backend uses for the
/// same facts: it is far below the rate at which a user flips an accessibility switch, and it
/// removes AppKit round trips from the frame cycle.
const ENVIRONMENT_TTL: std::time::Duration = std::time::Duration::from_secs(1);

/// The slot shape shared by every cached fact below.
type EnvironmentFactCache<T> = std::sync::OnceLock<Mutex<Option<(std::time::Instant, T)>>>;

/// Re-reads one AppKit environment fact at most once per [`ENVIRONMENT_TTL`].
///
/// # Cached facts and their invalidation trigger
///
/// | fact | slot | source |
/// |---|---|---|
/// | `high_contrast` | [`HIGH_CONTRAST_CACHE`] | `NSWorkspace.accessibilityDisplayShouldIncreaseContrast` |
/// | `reduce_motion` | [`REDUCE_MOTION_CACHE`] | `NSWorkspace.accessibilityDisplayShouldReduceMotion` |
/// | `system_locale` | [`SYSTEM_LOCALE_CACHE`] | `NSLocale.currentLocale`, then `LC_ALL`/`LANG` |
///
/// **Invalidation trigger: TTL expiry only.** The system-colour observer in [`super::environment`]
/// refreshes the accent palette but does not clear these slots (the missing push source is tracked
/// as issue P-201), so a changed switch becomes visible within one second and no later. The read
/// runs with no lock held, so concurrent misses sample the host rather than queueing.
fn cached_environment_fact<T: Copy>(
    cache: &'static EnvironmentFactCache<T>,
    read: impl FnOnce() -> T,
) -> T {
    // `get_or_init`, not `get`: an uninitialised `OnceLock` answers `None` forever, so `get` here
    // would read the slot, find it absent, run the read and drop the value — a cache that never
    // caches anything.
    let slot = cache.get_or_init(|| Mutex::new(None));
    if let Ok(guard) = slot.lock() {
        if let Some((sampled_at, value)) = *guard {
            if sampled_at.elapsed() < ENVIRONMENT_TTL {
                return value;
            }
        }
    }
    let value = read();
    if let Ok(mut guard) = slot.lock() {
        *guard = Some((std::time::Instant::now(), value));
    }
    value
}

static HIGH_CONTRAST_CACHE: EnvironmentFactCache<bool> = std::sync::OnceLock::new();
static REDUCE_MOTION_CACHE: EnvironmentFactCache<bool> = std::sync::OnceLock::new();
static SYSTEM_LOCALE_CACHE: EnvironmentFactCache<Option<&'static str>> = std::sync::OnceLock::new();

/// Whether the user asked for increased contrast.
///
/// `NSWorkspace.accessibilityDisplayShouldIncreaseContrast` is the AppKit answer to the system
/// "Increase contrast" setting. `NSWorkspace` is thread-safe as a shared singleton, so this does
/// not require a main-thread marker. Cached for [`ENVIRONMENT_TTL`]; see
/// [`cached_environment_fact`] for the invalidation trigger.
pub(crate) fn high_contrast() -> bool {
    cached_environment_fact(&HIGH_CONTRAST_CACHE, || {
        NSWorkspace::sharedWorkspace().accessibilityDisplayShouldIncreaseContrast()
    })
}

/// Whether the user asked for reduced motion.
///
/// `NSWorkspace.accessibilityDisplayShouldReduceMotion`, the macOS "Reduce motion" setting. Cached
/// for [`ENVIRONMENT_TTL`]; see [`cached_environment_fact`] for the invalidation trigger.
pub(crate) fn reduce_motion() -> bool {
    cached_environment_fact(&REDUCE_MOTION_CACHE, || {
        NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion()
    })
}

/// Whether the user asked for translucent materials to be reduced.
///
/// `NSWorkspace.accessibilityDisplayShouldReduceTransparency`, the macOS "Reduce transparency"
/// setting. The workspace is thread-safe, so unlike the appearance read above this answers from
/// any thread.
pub(crate) fn reduce_transparency() -> bool {
    NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceTransparency()
}

/// The host's current accent colour.
///
/// `NSColor.controlAccentColor` is dynamic: the system updates it when the user changes the accent
/// in System Settings. The colour is converted to sRGB before its components are read because the
/// system does not promise which colour space the dynamic colour uses.
pub(crate) fn accent_color() -> Option<crate::core::Color> {
    let color = NSColor::controlAccentColor();
    let space = NSColorSpace::sRGBColorSpace();
    let converted = color.colorUsingColorSpace(&space)?;
    Some(crate::core::Color::from_f32(
        converted.redComponent() as f32,
        converted.greenComponent() as f32,
        converted.blueComponent() as f32,
        converted.alphaComponent() as f32,
    ))
}

/// The family name the system UI face is registered under.
///
/// This is a **label the caller names**, not a claim that a static family of that name is
/// installed. On a stock macOS the system UI face is the variable font at
/// `/System/Library/Fonts/SFNS.ttf`; the OS does not install static text/display weight files, so
/// one registered face serves every text token: the renderer instantiates its `wght` axis at each
/// [`crate::core::Font::weight`] (`crate::render::text::variable`), and a host face with no such
/// axis keeps its default instance.
#[cfg(feature = "runtime-fonts")]
pub(crate) const SYSTEM_UI_FAMILY: &str = "SF Pro";

/// Registers the system UI face in the runtime face registry.
///
/// Returns the family name on success. The bytes are the host's own system UI font, read once per
/// process and leaked because [`crate::render::text::register_face`] takes `&'static [u8]`; a
/// registered face lives for the rest of the process by contract, so the leak is paid once.
///
/// `None` when the font file is absent or unreadable, or when the registry refuses the bytes. The
/// caller must fall back in that case rather than claim the face is in use.
#[cfg(feature = "runtime-fonts")]
pub(crate) fn register_system_ui_face() -> Option<&'static str> {
    static BYTES: std::sync::OnceLock<Option<&'static [u8]>> = std::sync::OnceLock::new();
    let bytes = *BYTES.get_or_init(read_system_ui_face_bytes);
    let bytes = bytes?;
    crate::render::text::register_face(SYSTEM_UI_FAMILY, bytes).then_some(SYSTEM_UI_FAMILY)
}

/// Reads the host's system UI font file, once.
///
/// The display variant is checked as a second candidate so a system that splits text and display
/// faces still registers one of them; both are variable faces when present.
#[cfg(feature = "runtime-fonts")]
fn read_system_ui_face_bytes() -> Option<&'static [u8]> {
    const CANDIDATES: [&str; 2] =
        ["/System/Library/Fonts/SFNS.ttf", "/System/Library/Fonts/SFNSDisplay.ttf"];
    for path in CANDIDATES {
        match std::fs::read(path) {
            Ok(bytes) if !bytes.is_empty() => {
                return Some(&*crate::compat::Box::leak(bytes.into_boxed_slice()))
            }
            Ok(_) => log::debug!("[fonts] {path} is empty; not registering a system UI face"),
            Err(error) => {
                log::debug!("[fonts] could not read {path}: {error}")
            }
        }
    }
    None
}

/// The user's locale, from `NSLocale.currentLocale.localeIdentifier`.
///
/// AppKit spells the identifier with an underscore (`en_US`); this normalizes it to the BCP-47
/// hyphenated form every other backend reports, then interns it for the style layer. The hyphen
/// replacement is skipped entirely when the identifier has no territory (`en`), and the interned
/// value is held in the one-second environment cache so the interner's lock is not taken on every
/// style-environment poll. See [`cached_environment_fact`] for the invalidation trigger.
pub(crate) fn system_locale() -> Option<&'static str> {
    cached_environment_fact(&SYSTEM_LOCALE_CACHE, || {
        let identifier = NSLocale::currentLocale().localeIdentifier().to_string();
        let tag = if identifier.contains('_') { identifier.replace('_', "-") } else { identifier };
        crate::platform::locale::intern(Some(tag)).or_else(|| {
            crate::platform::locale::intern(crate::platform::locale::from_environment())
        })
    })
}

// ─── Displays and window geometry ───

/// Converts an AppKit rectangle into this crate's [`Rect`].
///
/// `NSRect` holds `CGFloat` (`f64`) values, so each is rounded rather than truncated. A negative
/// extent cannot occur for a real screen, but it is clamped so a `u32` width is never produced from
/// one. The values are reported **unconverted** (AppKit's bottom-left origin), the same coordinate
/// space `create_window` passes to `initWithContentRect:`, so a host comparing the two reads one
/// space.
fn rect_of(rect: NSRect) -> Rect {
    Rect::new(
        rect.origin.x.round() as i32,
        rect.origin.y.round() as i32,
        rect.size.width.max(0.0).round() as u32,
        rect.size.height.max(0.0).round() as u32,
    )
}

/// The `CGDirectDisplayID` AppKit carries for a screen, when it reports one.
fn display_number(screen: &NSScreen) -> Option<u64> {
    let description = screen.deviceDescription();
    let key = NSString::from_str("NSScreenNumber");
    let value = description.objectForKey(&key)?;
    // SAFETY: AppKit documents `NSScreenNumber` as an `NSNumber`; `unsignedIntValue` is its
    // integer accessor.
    let number: u32 = unsafe { msg_send![&*value, unsignedIntValue] };
    Some(u64::from(number))
}

/// The id [`displays`] gives a screen: its device number, or its enumeration index when AppKit
/// reports no device description (a documented fallback for a key that should always be present).
fn display_id_at(index: usize, screen: &NSScreen) -> u64 {
    display_number(screen).unwrap_or(index as u64)
}

/// The displays AppKit reports, in `+[NSScreen screens]` order.
///
/// This is the objc2 counterpart of `src/platform/macos/displays.rs`, which the cocoa backend
/// uses. An empty list is returned off the AppKit main thread: messaging `NSScreen` from another
/// thread raises, and the trait's honest absence is what the caller can act on.
pub(crate) fn displays() -> Vec<Display> {
    let Some(mtm) = MainThreadMarker::new() else {
        return Vec::new();
    };
    let screens = NSScreen::screens(mtm);
    let main_screen = NSScreen::mainScreen(mtm);
    let mut out = Vec::new();
    for (index, screen) in screens.iter().enumerate() {
        let frame = screen.frame();
        let visible = screen.visibleFrame();
        let scale = screen.backingScaleFactor();
        let is_primary = main_screen.as_ref().is_some_and(|main| {
            core::ptr::eq(objc2::rc::Retained::as_ptr(main), objc2::rc::Retained::as_ptr(&screen))
        });
        out.push(Display {
            id: DisplayId::new(display_id_at(index, &screen)),
            bounds: rect_of(frame),
            work_area: rect_of(visible),
            // A non-positive scale would divide by zero downstream; AppKit reports >= 1.0, and the
            // clamp keeps a bad value from becoming a divide-by-zero.
            scale: if scale > 0.0 { scale as f32 } else { 1.0 },
            is_primary,
        });
    }
    out
}

/// The scale factor of AppKit's main screen.
///
/// The per-screen `backingScaleFactor` [`displays`] reports, read once for the trait's global
/// [`Platform::dpi_scale_factor`](crate::platform::Platform::dpi_scale_factor). `None` off the
/// main thread, where messaging `NSScreen` would raise.
pub(crate) fn main_scale() -> Option<f32> {
    let mtm = MainThreadMarker::new()?;
    let screen = NSScreen::mainScreen(mtm)?;
    let scale = screen.backingScaleFactor();
    // A non-positive scale would divide by zero downstream; AppKit reports >= 1.0.
    if scale > 0.0 {
        Some(scale as f32)
    } else {
        None
    }
}

/// The registry pointer for `window_id`, when it names a live native object.
fn native_ptr(window_id: u64) -> Option<*mut AnyObject> {
    NATIVE_VIEWS.lock().ok()?.get(&window_id).map(|ptr| ptr.0 as *mut AnyObject)
}

/// Whether `object` is an `NSWindow` rather than a plain view.
///
/// # Safety
///
/// `object` must be a live AppKit object on the main thread, as every registry entry is.
unsafe fn is_window(object: *mut AnyObject) -> bool {
    let window_class = <NSWindow as objc2::ClassType>::class();
    let result: bool = msg_send![object, isKindOfClass: window_class];
    result
}

/// The screen a window currently occupies, as the id [`displays`] hands out.
///
/// The lookup accepts both id spaces: the platform id `create_window` returned and the widget id a
/// host may hold. An unknown id, a non-window handle, or a call off the main thread answers `None`
/// rather than messaging an object that is not an `NSWindow`.
pub(crate) fn screen_id_of_window(window_id: u64) -> Option<u64> {
    let _mtm = MainThreadMarker::new()?;
    let object = native_ptr(window_id)?;
    // SAFETY: the registry only holds retained AppKit objects it created, and the main thread is
    // established by the marker above.
    if !unsafe { is_window(object) } {
        return None;
    }
    let window: &NSWindow = unsafe { &*(object as *const NSWindow) };
    let screen = window.screen()?;
    let index = NSScreen::screens(_mtm).iter().position(|candidate| {
        core::ptr::eq(objc2::rc::Retained::as_ptr(&candidate), objc2::rc::Retained::as_ptr(&screen))
    })?;
    Some(display_id_at(index, &screen))
}

/// The live `NSWindow` a window id names, when it is one.
fn window_ref(window_id: u64) -> Option<Retained<NSWindow>> {
    let _mtm = MainThreadMarker::new()?;
    let object = native_ptr(window_id)?;
    // SAFETY: the registry holds retained AppKit objects created on the main thread, and the
    // marker above proves this is that thread. `retain` takes its own +1 reference, so the
    // returned object outlives any registry removal that happens during the call.
    unsafe {
        if !is_window(object) {
            return None;
        }
        Retained::retain(object as *mut NSWindow)
    }
}

/// Applies a window state through AppKit.
///
/// Returns `false` when the id is not a live `NSWindow` or the call is off the main thread; the
/// caller then keeps the recorded state as its fallback.
pub(crate) fn set_window_state(
    window_id: u64,
    flag: crate::platform::WindowStateFlag,
    on: bool,
) -> bool {
    use crate::platform::WindowStateFlag;
    let Some(window) = window_ref(window_id) else {
        return false;
    };
    match flag {
        WindowStateFlag::Maximized => {
            if on != window.isZoomed() {
                window.zoom(None);
            }
        }
        WindowStateFlag::Minimized => {
            if on && !window.isMiniaturized() {
                window.performMiniaturize(None);
            } else if !on && window.isMiniaturized() {
                window.deminiaturize(None);
            }
        }
        WindowStateFlag::Fullscreen => {
            let fullscreen = window.styleMask().contains(NSWindowStyleMask::FullScreen);
            if on != fullscreen {
                window.toggleFullScreen(None);
            }
        }
        WindowStateFlag::Resizable => {
            let mask = window.styleMask();
            let mut updated = mask;
            updated.set(NSWindowStyleMask::Resizable, on);
            if updated != mask {
                window.setStyleMask(updated);
            }
        }
        WindowStateFlag::Decorated => {
            let mask = window.styleMask();
            let mut updated = mask;
            let chrome = NSWindowStyleMask::Titled
                | NSWindowStyleMask::Closable
                | NSWindowStyleMask::Miniaturizable
                | NSWindowStyleMask::Resizable;
            if on {
                updated.insert(chrome);
            } else {
                updated.remove(chrome);
            }
            if updated != mask {
                window.setStyleMask(updated);
            }
        }
    }
    true
}

/// Reads a window state from AppKit.
///
/// `None` when the id is not a live `NSWindow` or the call is off the main thread; the caller
/// falls back to the recorded state.
pub(crate) fn window_state(window_id: u64, flag: crate::platform::WindowStateFlag) -> Option<bool> {
    use crate::platform::WindowStateFlag;
    let window = window_ref(window_id)?;
    let mask = window.styleMask();
    Some(match flag {
        WindowStateFlag::Maximized => window.isZoomed(),
        WindowStateFlag::Minimized => window.isMiniaturized(),
        WindowStateFlag::Fullscreen => mask.contains(NSWindowStyleMask::FullScreen),
        WindowStateFlag::Resizable => mask.contains(NSWindowStyleMask::Resizable),
        WindowStateFlag::Decorated => mask.contains(NSWindowStyleMask::Titled),
    })
}

/// Applies a minimum content size through AppKit.
pub(crate) fn set_window_min_size(window_id: u64, width: u32, height: u32) -> bool {
    let Some(window) = window_ref(window_id) else {
        return false;
    };
    window.setContentMinSize(NSSize::new(width as f64, height as f64));
    true
}

/// Reads a window's minimum content size from AppKit.
pub(crate) fn window_min_size(window_id: u64) -> Option<(u32, u32)> {
    let window = window_ref(window_id)?;
    let size = window.contentMinSize();
    if size.width > 0.0 && size.height > 0.0 {
        Some((size.width.round() as u32, size.height.round() as u32))
    } else {
        None
    }
}

/// The window's content size in logical pixels, read from its content view.
///
/// `NSWindow.contentLayoutRect` is AppKit's own notion of the area available to content, but it
/// excludes the title bar only when the window uses a full-size content view. The content view's
/// frame is the rectangle the library paints into, which is the size a layout must use.
pub(crate) fn window_client_size(window_id: u64) -> Option<(u32, u32)> {
    let window = window_ref(window_id)?;
    let view = window.contentView()?;
    let frame = view.frame();
    let width = frame.size.width.max(0.0).round() as u32;
    let height = frame.size.height.max(0.0).round() as u32;
    (width > 0 && height > 0).then_some((width, height))
}

/// The raw pointer of a native object this backend owns, for `Platform::get_native_handle`.
pub(crate) fn native_handle(window_id: u64) -> Option<usize> {
    native_ptr(window_id).map(|ptr| ptr as usize)
}

/// The raw `NSView` pointer a window's GPU surface is created from.
///
/// rwh's AppKit variant names an `NSView`; the window's content view is what the library paints
/// into. The pointer stays valid while the window is in the registry, which is the lifetime the
/// caller is documented to use it within.
#[cfg(feature = "raw-window-handle")]
pub(crate) fn raw_view_handle(window_id: u64) -> Option<*mut core::ffi::c_void> {
    let window = window_ref(window_id)?;
    let view = window.contentView()?;
    Some(objc2::rc::Retained::as_ptr(&view) as *mut core::ffi::c_void)
}

/// The safe-area insets of a window (or of a view this backend owns), in logical pixels.
///
/// `NSView.safeAreaInsets` exists on macOS 11+; older systems answer `None` rather than zero, which
/// is the same "cannot tell" rule the trait documents. For a window the insets are read from its
/// content view, because that is the view the library paints into and the one AppKit lays out under
/// the title bar / notch.
pub(crate) fn safe_area_insets(window_id: u64) -> Option<SafeAreaInsets> {
    let _mtm = MainThreadMarker::new()?;
    let object = native_ptr(window_id)?;
    // SAFETY: registry entries are retained AppKit objects and this is the main thread.
    let view = unsafe {
        if is_window(object) {
            let window: &NSWindow = &*(object as *const NSWindow);
            let content = window.contentView()?;
            objc2::rc::Retained::as_ptr(&content) as *mut AnyObject
        } else {
            object
        }
    };
    // SAFETY: `view` is a live `NSView` on the main thread; the selector check keeps a pre-11 AppKit
    // from receiving a message it does not implement.
    let responds: bool = unsafe { msg_send![view, respondsToSelector: sel!(safeAreaInsets)] };
    if !responds {
        return None;
    }
    let insets: NSEdgeInsets = unsafe { msg_send![view, safeAreaInsets] };
    Some(SafeAreaInsets {
        top: logical_pixels(insets.top),
        bottom: logical_pixels(insets.bottom),
        left: logical_pixels(insets.left),
        right: logical_pixels(insets.right),
    })
}

/// Rounds a `CGFloat` edge distance to logical pixels, clamping a negative value to zero rather
/// than wrapping it into a huge `u32`.
fn logical_pixels(value: f64) -> u32 {
    value.max(0.0).round() as u32
}

/// Wrapper around `*mut c_void` that implements `Send`.
///
/// `Send` is required because the pointer is stored in a process-global
/// `Mutex<HashMap<..>>` (the registry lives in a `LazyLock` static).
///
/// `Sync` is deliberately NOT implemented: AppKit objects are owned by the
/// main thread, and an `unsafe impl Sync` would additionally permit `&NativePtr`
/// to be shared across threads, removing the compiler's last guard against
/// off-main AppKit access. Every native entry point still re-checks the thread
/// before messaging AppKit (see `check_apple_thread_safety.sh`), and keeping the
/// bound narrow means a future helper that forgets that check has a smaller
/// unsafe surface to slip through.
#[derive(Clone, Copy)]
struct NativePtr(*mut std::ffi::c_void);
// SAFETY: the pointer names an AppKit object owned by the main thread; `Send` only allows the pointer to move into a process-global map, and every native entry point re-checks the thread before messaging.
unsafe impl Send for NativePtr {}

/// Thread-local storage for native widget handles.
static NATIVE_VIEWS: LazyLock<Mutex<HashMap<u64, NativePtr>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

static PENDING_NATIVE_RELEASES: LazyLock<Mutex<Vec<NativePtr>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

/// Side table from a mounted surface id to its canvas view pointer.
///
/// Distinct from [`NATIVE_VIEWS`], which holds the window (or control) a widget id
/// was created as. A *surface* is the drawing view mounted **onto** that window, so
/// `resize_surface` / `invalidate_surface` / `unmount_surface` need to find it by
/// the id passed to [`mount_surface_native`] independently of the window registry.
static SURFACE_VIEWS: LazyLock<Mutex<HashMap<u64, NativePtr>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Returns whether the calling thread is the AppKit main thread.
///
/// Every native entry point in this module re-checks this before messaging
/// AppKit. It is exported so `platform_impl` can choose the documented state-only
/// fallback off the main thread rather than mistaking "AppKit is unreachable" for
/// "the surface failed to mount".
pub(crate) fn on_main_thread() -> bool {
    MainThreadMarker::new().is_some()
}

pub(crate) fn store_native_view(widget_id: u64, view: *mut std::ffi::c_void) {
    if view.is_null() {
        return;
    }
    remove_native_view(widget_id);
    // SAFETY: `view` is the live NSView created for this widget; AppKit keeps it alive for the call.
    unsafe {
        let object = view as *mut AnyObject;
        let _: *mut AnyObject = msg_send![object, retain];
    }
    NATIVE_VIEWS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(widget_id, NativePtr(view));
}

pub(crate) fn remove_native_view(widget_id: u64) {
    let removed =
        NATIVE_VIEWS.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).remove(&widget_id);
    if let Some(ptr) = removed {
        // SAFETY: `ptr` came from `NATIVE_VIEWS`, so it names a live AppKit object owned by this module.
        unsafe {
            let object = ptr.0 as *mut AnyObject;
            // Detach from the superview first. A parent view retains whatever was
            // added as its subview, and that reference is NOT balanced by the
            // registry's `retain`/`release` pair — so releasing only the registry
            // reference left the object alive and owned by the parent forever
            // (a create/destroy UI churn grew RSS by ~6 KB per widget, without
            // bound). `removeFromSuperview` drops the parent's reference, which
            // is what actually lets the object deallocate.
            let has_superview: bool =
                msg_send![object, respondsToSelector: sel!(removeFromSuperview)];
            if has_superview {
                let _: () = msg_send![object, removeFromSuperview];
            }
            let _: () = msg_send![object, release];
        }
    }
}

/// Returns whether `object` is (or descends from) `NSWindow`.
///
/// Used by [`destroy_native_handle`] to decide between closing a window and
/// detaching a plain view: `removeFromSuperview` on an `NSWindow` does nothing
/// useful and never takes it off screen, so a logically destroyed window could
/// stay visible.
///
/// # Safety
/// `object` must be a live AppKit object on the main thread.
unsafe fn object_is_window(object: *mut AnyObject) -> bool {
    let window_class = <NSWindow as objc2::ClassType>::class();
    let is_window: bool = msg_send![object, isKindOfClass: window_class];
    is_window
}

/// What [`destroy_native_handle`] did (or could not do).
///
/// The caller must be able to tell "there was no native object" (its bookkeeping
/// is the only thing to drop) from "there was one but AppKit is main-thread-only
/// and we are not on it" (the bookkeeping must be **kept** so a later main-thread
/// call can finish). A plain `bool` collapsed the two and let an off-main destroy
/// report success while the window stayed on screen (D08-P-02).
pub(crate) enum NativeDestroyOutcome {
    /// A native object was found and torn down.
    Destroyed,
    /// No native object was ever created for this id (a state-only widget).
    NoNativeObject,
    /// AppKit refused: not the main thread. The registry entry is **kept** so the
    /// teardown can be retried on the main thread.
    RefusedOffMainThread,
}

/// Destroys a native handle created by this backend.
///
/// A **window** is closed with `close` (which hides and releases the window's
/// AppKit resources); any other view is detached with `removeFromSuperview`.
/// Both run only on the main thread — off it, AppKit must not be messaged, so the
/// call logs and reports [`NativeDestroyOutcome::RefusedOffMainThread`] and the
/// caller keeps the bookkeeping it had.
pub(crate) fn destroy_native_handle(widget_id: u64) -> NativeDestroyOutcome {
    // The lookup runs **before** the thread check. A state-only widget (no native
    // object) has nothing for AppKit to refuse, so its teardown must succeed on any
    // thread; doing the thread check first made an off-main destroy of a widget that
    // was never natively created report `RefusedOffMainThread`, keeping bookkeeping it
    // never needed and breaking the shared teardown contract (a host that creates and
    // destroys off the main loop could not finish a destroy it was entitled to). Only a
    // real native handle is main-thread-bound, so only it is refused.
    if !NATIVE_VIEWS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .contains_key(&widget_id)
    {
        return NativeDestroyOutcome::NoNativeObject;
    }
    if MainThreadMarker::new().is_none() {
        log::error!(
            "[macos-objc2] destroy_handle: refused off the AppKit main thread (id={widget_id})"
        );
        return NativeDestroyOutcome::RefusedOffMainThread;
    }
    let removed =
        NATIVE_VIEWS.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).remove(&widget_id);
    let Some(ptr) = removed else {
        // No native object was ever created for this id (a state-only widget), so
        // there is nothing to tear down and the caller's bookkeeping stands.
        return NativeDestroyOutcome::NoNativeObject;
    };
    // SAFETY: `ptr` is the live native object being torn down on the main thread.
    unsafe {
        let object = ptr.0 as *mut AnyObject;
        if object_is_window(object) {
            // `close`, not `orderOut:`: `close` is the symmetric counterpart of
            // `makeKeyAndOrderFront:`, releasing the window and its views. An
            // `orderOut:` alone would hide it but leave the object retained.
            let _: () = msg_send![object, close];
        } else {
            let _: () = msg_send![object, removeFromSuperview];
        }
        let _: () = msg_send![object, release];
    }
    NativeDestroyOutcome::Destroyed
}

fn release_closed_window(host_id: u64) {
    if MainThreadMarker::new().is_none() {
        log::error!(
            "[macos-objc2] close_window: refused off the AppKit main thread (id={host_id})"
        );
        return;
    }
    let removed =
        NATIVE_VIEWS.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).remove(&host_id);
    if let Some(ptr) = removed {
        PENDING_NATIVE_RELEASES.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).push(ptr);
    }
}

fn drain_pending_native_releases() {
    let pending = std::mem::take(
        &mut *PENDING_NATIVE_RELEASES.lock().unwrap_or_else(|poisoned| poisoned.into_inner()),
    );
    for ptr in pending {
        // SAFETY: `ptr` was queued after its last use; release runs once on the main thread.
        unsafe {
            let object = ptr.0 as *mut AnyObject;
            let _: () = msg_send![object, release];
        }
    }
}

fn make_rect(x: i32, y: i32, width: u32, height: u32) -> NSRect {
    NSRect::new(
        objc2_foundation::NSPoint::new(x as f64, y as f64),
        NSSize::new(width.max(1) as f64, height.max(1) as f64),
    )
}

// ---------------------------------------------------------------------------
// Canvas view: the presentation path
// ---------------------------------------------------------------------------

/// Instance variables of [`RustWidgetsObjc2CanvasView`].
///
/// The widget id is stored in an ivar rather than an associated object because
/// `define_class!` finalises the ivar layout at class-registration time, which is
/// exactly the point of the macro — the id travels with the view for the view's
/// whole lifetime, including the `drawRect:` calls AppKit makes from its own
/// display machinery.
pub(crate) struct CanvasIvars {
    /// Registry id of the widget this view presents.
    widget_id: std::cell::Cell<u64>,
    /// `true` when this canvas is a window's content view, so `drawRect:` paints
    /// the window **and its descendants** rather than one mounted box.
    ///
    /// Written by the constructor on every profile — that is what decides the field's value — and
    /// read only by the painter, which a stripped profile does not compile. The attribute states
    /// that, rather than hiding a field that is genuinely used.
    #[cfg_attr(not(widgets_unstripped), allow(dead_code))]
    is_window: std::cell::Cell<bool>,
}

define_class!(
    // SAFETY:
    // - `NSView` has no subclassing requirements beyond using the designated
    //   initializer, which `init_with_frame` performs.
    // - `RustWidgetsObjc2CanvasView` does not implement `Drop`; the ivar is a
    //   `Cell<u64>`, which needs no release.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "RustWidgetsObjc2CanvasView"]
    #[ivars = CanvasIvars]
    pub(crate) struct RustWidgetsObjc2CanvasView;

    impl RustWidgetsObjc2CanvasView {
        /// `drawRect:` — pull one RGBA frame from the widget runtime and blit it.
        ///
        /// Mirrors `macos/canvas.rs::draw_rect`: the size comes from `bounds`, not
        /// from the dirty rectangle AppKit passes, because a partial invalidation
        /// hands a rect smaller than the view and sizing the frame from it would
        /// stretch the image into the wrong place.
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty_rect: NSRect) {
            // A panic must not unwind through AppKit: a foreign exception cannot
            // cross the Objective-C frame, so it is caught and logged here.
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.blit_frame();
            }));
            if outcome.is_err() {
                log::error!("[macos-objc2] canvas: panic while drawing a self-drawn widget");
            }
        }

        /// The view uses AppKit's native coordinate system, so a frame buffer's
        /// top-left origin lands at the view's top-left after the blit flips rows.
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            false
        }

        /// The canvas owns the keyboard while it hosts the focused widget, so it must
        /// accept first-responder status — otherwise AppKit never delivers `keyDown:`.
        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self) -> bool {
            true
        }

        // ── NSAccessibility (BLUE28 B28.13) ──
        //
        // The view is the accessibility element for the widget it presents. VoiceOver reads the
        // role and label below; the bridge in `platform/accessibility/macos_objc2.rs` posts the
        // change notifications against this same view pointer.

        #[unsafe(method(isAccessibilityElement))]
        fn is_accessibility_element(&self) -> bool {
            self.ivars().widget_id.get() != 0
        }

        #[unsafe(method(accessibilityRole))]
        fn accessibility_role(&self) -> Option<&'static NSAccessibilityRole> {
            // The role is read from the widget registry, which does not exist under
            // `alloc_frugal`; `isAccessibilityElement` is false there (no widget id is ever
            // mounted), so the honest answer is "no role" rather than a fabricated one.
            #[cfg(widgets_unstripped)]
            {
                Some(ns_accessibility_role_for(self.accessibility_role_of_widget()))
            }
            #[cfg(not(widgets_unstripped))]
            {
                None
            }
        }

        #[unsafe(method(accessibilityLabel))]
        fn accessibility_label(&self) -> *mut NSString {
            let label = self.accessibility_label_of_widget();
            objc2::rc::Retained::autorelease_return(NSString::from_str(&label))
        }

        #[unsafe(method(accessibilityFrame))]
        fn accessibility_frame(&self) -> NSRect {
            let bounds = self.bounds();
            self.window().map(|window| window.convertRectToScreen(bounds)).unwrap_or(bounds)
        }

        // ── NSTextInputClient (BLUE28 B28.12) ──
        //
        // AppKit routes a key press through `interpretKeyEvents:` in `keyDown:` below. The
        // input method calls these methods with marked text while composing and with
        // `insertText:` when it commits; the same `MacOsImeBridge` state machine the cocoa
        // backend uses tracks the ranges in UTF-16 units.

        #[unsafe(method(hasMarkedText))]
        fn has_marked_text(&self) -> bool {
            ime_bridge().has_marked_text()
        }

        #[unsafe(method(markedRange))]
        fn marked_range(&self) -> NSRange {
            match ime_bridge().get_marked_text() {
                Some(text) => NSRange::new(0, text.encode_utf16().count()),
                None => NSRange::new(usize::MAX, 0),
            }
        }

        #[unsafe(method(selectedRange))]
        fn selected_range(&self) -> NSRange {
            let length =
                ime_bridge().get_marked_text().map_or(0, |text| text.encode_utf16().count());
            NSRange::new(length, 0)
        }

        #[unsafe(method(validAttributesForMarkedText))]
        fn valid_attributes_for_marked_text(&self) -> *mut NSArray<NSAttributedStringKey> {
            // An empty set is correct for plain-text composition: no attributes are required.
            objc2::rc::Retained::autorelease_return(NSArray::from_slice(&[]))
        }

        #[unsafe(method(attributedSubstringForProposedRange:actualRange:))]
        fn attributed_substring(
            &self,
            _range: NSRange,
            actual_range: *mut NSRange,
        ) -> *mut NSAttributedString {
            if !actual_range.is_null() {
                // SAFETY: AppKit passes a writable out-parameter or null; the null case is
                // checked, and on the non-null path the pointer names its own stack slot for
                // the duration of this call.
                unsafe { *actual_range = NSRange::new(0, 0) };
            }
            // The surface holds no attributed text of its own; the widget layer owns it.
            core::ptr::null_mut()
        }

        #[unsafe(method(firstRectForCharacterRange:actualRange:))]
        fn first_rect_for_character_range(
            &self,
            range: NSRange,
            actual_range: *mut NSRange,
        ) -> NSRect {
            if !actual_range.is_null() {
                // SAFETY: see `attributed_substring`; the caller owns the out-parameter.
                unsafe { *actual_range = range };
            }
            // The candidate window is placed at the surface origin. AppKit wants screen
            // coordinates, so the view rect is converted through the window.
            let rect = NSRect::new(
                objc2_foundation::NSPoint::new(0.0, 0.0),
                NSSize::new(1.0, 1.0),
            );
            self.window().map(|window| window.convertRectToScreen(rect)).unwrap_or(rect)
        }

        #[unsafe(method(characterIndexForPoint:))]
        fn character_index_for_point(&self, _point: objc2_foundation::NSPoint) -> usize {
            0
        }

        #[unsafe(method(unmarkText))]
        fn unmark_text(&self) {
            ime_bridge().discard_marked_text();
            self.setNeedsDisplay(true);
        }

        #[unsafe(method(setMarkedText:selectedRange:replacementRange:))]
        fn set_marked_text(
            &self,
            string: &AnyObject,
            selected_range: NSRange,
            _replacement_range: NSRange,
        ) {
            let Some(text) = text_of(string) else { return };
            if IME_INTERPRETING.get() {
                IME_HANDLED.set(true);
            }
            ime_bridge().set_marked_text(
                &text,
                selected_range.location as i32,
                selected_range.location.saturating_add(selected_range.length) as i32,
            );
            self.setNeedsDisplay(true);
        }

        #[unsafe(method(insertText:replacementRange:))]
        fn insert_text(&self, string: &AnyObject, _replacement_range: NSRange) {
            let Some(text) = text_of(string) else { return };
            if IME_INTERPRETING.get() {
                IME_HANDLED.set(true);
            }
            let bridge = ime_bridge();
            if bridge.has_marked_text() {
                // A commit at the end of a composition.
                bridge.commit_text(&text);
            } else {
                // A plain key that AppKit translated into text. It travels as `TextInput` so a
                // control's text path runs exactly as it does for `forward_key`.
                #[cfg(widgets_unstripped)]
                {
                    let _ = self.forward_ime_text(&text);
                }
            }
            self.setNeedsDisplay(true);
        }

        #[cfg(widgets_unstripped)]
        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            // A second primary press inside the system's double-click interval is its
            // own event (`Event::MouseDoubleClick`), not a press with a higher count —
            // the same rule `macos::canvas` applies, and the `DoubleClick` arm of
            // `forward_mouse` below exists for exactly this. AppKit reports the run
            // length in `clickCount`, so the interval's definition is never
            // re-derived here.
            if event.clickCount() >= 2 {
                self.forward_mouse(event, MousePhase::DoubleClick);
                return;
            }
            self.forward_mouse(event, MousePhase::Press);
        }

        #[cfg(widgets_unstripped)]
        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, event: &NSEvent) {
            self.forward_mouse(event, MousePhase::Release);
        }

        #[cfg(widgets_unstripped)]
        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, event: &NSEvent) {
            self.forward_mouse(event, MousePhase::Drag);
        }

        #[cfg(widgets_unstripped)]
        #[unsafe(method(rightMouseDown:))]
        fn right_mouse_down(&self, event: &NSEvent) {
            self.forward_mouse(event, MousePhase::SecondaryPress);
        }

        /// `-mouseMoved:` — the pointer travelled with **no** button held.
        ///
        /// This is the hover path. It is a distinct phase from `-mouseDragged:` because the
        /// repaint rule differs: a drag owes a frame per sample (a selection follows it), while a
        /// hover owes one only when the pointer crosses into another control. AppKit delivers this
        /// message only to a view with a tracking area that asked for it, which is what
        /// `install_hover_tracking_area` installs. Without the pair this backend never highlighted
        /// a control under the pointer (and `MousePhase::Move` had no constructor).
        #[cfg(widgets_unstripped)]
        #[unsafe(method(mouseMoved:))]
        fn mouse_moved(&self, event: &NSEvent) {
            self.forward_mouse(event, MousePhase::Move);
        }

        #[cfg(widgets_unstripped)]
        #[unsafe(method(rightMouseUp:))]
        fn right_mouse_up(&self, event: &NSEvent) {
            self.forward_mouse(event, MousePhase::SecondaryRelease);
        }

        #[cfg(widgets_unstripped)]
        #[unsafe(method(scrollWheel:))]
        fn scroll_wheel(&self, event: &NSEvent) {
            self.forward_wheel(event);
        }

        #[cfg(widgets_unstripped)]
        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            // Give an active input method first refusal. `interpretKeyEvents:` calls back into
            // the `NSTextInputClient` methods above; a method that consumed the event sets
            // `IME_HANDLED`, and an unhandled key (an arrow, Tab, a shortcut) still reaches the
            // ordinary `forward_key` path below, so nothing is swallowed.
            IME_INTERPRETING.set(true);
            IME_HANDLED.set(false);
            let events = NSArray::from_slice(&[event]);
            self.interpretKeyEvents(&events);
            let handled = IME_HANDLED.get();
            IME_INTERPRETING.set(false);
            if !handled {
                self.forward_key(event, false);
            }
        }

        #[cfg(widgets_unstripped)]
        #[unsafe(method(keyUp:))]
        fn key_up(&self, event: &NSEvent) {
            self.forward_key(event, true);
        }

        /// AppKit calls this on the window's content view when the window is resized.
        ///
        /// The superclass call performs the resize; the library is then told the new
        /// client size so layout re-runs — otherwise a user drag of the window edge
        /// changed the pixels but never the widget geometry.
        #[unsafe(method(setFrameSize:))]
        fn set_frame_size(&self, new_size: NSSize) {
            // SAFETY: `super` is the documented objc2 spelling for messaging the
            // superclass implementation; `setFrameSize:` is the NSView setter.
            let _: () = unsafe { msg_send![super(self), setFrameSize: new_size] };
            #[cfg(widgets_unstripped)]
            if let Some(widget_id) = self.widget_target() {
                crate::queue_resize_trigger(
                    widget_id,
                    new_size.width.round().max(1.0) as u32,
                    new_size.height.round().max(1.0) as u32,
                );
            }
        }

        /// The window delegate hook for the close button: emit the widget's `closed`
        /// signal (so `on_close` handlers run), then allow AppKit to close the window.
        #[unsafe(method(windowShouldClose:))]
        fn window_should_close(&self, _window: &NSWindow) -> bool {
            #[cfg(widgets_unstripped)]
            if let Some(widget_id) = self.widget_target() {
                let _ = crate::close_widget(widget_id);
            }
            true
        }

        #[unsafe(method(windowWillClose:))]
        fn window_will_close(&self, _notification: &NSNotification) {
            let host_id = self.ivars().widget_id.get();
            if host_id != 0 {
                release_closed_window(host_id);
            }
        }
    }
);

impl RustWidgetsObjc2CanvasView {
    /// Renders one frame for the carried widget id and draws it into the view's
    /// current CoreGraphics context.
    fn blit_frame(&self) {
        let widget_id = self.ivars().widget_id.get();
        if widget_id == 0 {
            log::error!("[macos-objc2] canvas: drawRect: on a view with no widget id");
            return;
        }
        // The size comes from `bounds`, not the dirty rect (see `draw_rect`).
        //
        // Everything below produces and blits a frame, and only `widget::runtime` can produce one,
        // so it is one `widgets_unstripped` block rather than an attribute per binding: on a
        // stripped profile the `#[cfg(not(..))]` arm below has already reported why there is
        // nothing to draw, and none of these bindings exist to be unused.
        #[cfg(widgets_unstripped)]
        {
            let bounds = self.bounds();
            let width = bounds.size.width.round().max(1.0) as u32;
            let height = bounds.size.height.round().max(1.0) as u32;
            let clear = crate::core::Color::WHITE;
            let size = crate::core::Size::new(width, height);

            let frame = if self.ivars().is_window.get() {
                // The window canvas stores the **host** id `create_window` returned (the
                // `BackendState` counter starts at 1), while `render_frame_tree` looks the
                // widget up by its **widget-registry** id (allocated from a far higher
                // base). Translate at this boundary so the frame targets the correct
                // widget instead of missing it and painting nothing.
                let Some(widget) = crate::widget::runtime::widget_id_for_host_window(widget_id)
                else {
                    log::error!(
                        "[macos-objc2] canvas: drawRect: host id={widget_id} has no owning                          widget; nothing to paint"
                    );
                    return;
                };
                // A window's content view paints the window's own chrome and every child
                // that implements `Draw`; a plain `render_frame_cached` would show the
                // window over an empty client area.
                crate::widget::runtime::render_frame_tree_cached_shared(widget, size, clear)
            } else {
                // A mounted surface is one self-drawn widget in its own box, which is
                // what the cocoa backend's `draw_rect` blits.
                crate::widget::runtime::render_frame_cached_shared(widget_id, size, clear)
            };
            let Some(frame) = frame else {
                // Not mounted (dropped while the view survived) or it refused to paint.
                log::error!(
                    "[macos-objc2] canvas: drawRect: widget id={widget_id} produced no frame                      (unmounted, or it does not implement Draw)"
                );
                return;
            };

            let Some(context) = NSGraphicsContext::currentContext() else {
                log::error!(
                    "[macos-objc2] canvas: drawRect: +[NSGraphicsContext currentContext] was nil"
                );
                return;
            };
            let cg_context = context.CGContext();
            if !blit_rgba(&cg_context, width, height, &frame) {
                log::error!("[macos-objc2] canvas: drawRect: blit failed for {width}x{height}");
            }
        }
        #[cfg(not(widgets_unstripped))]
        {
            // A stripped profile has no `widget::runtime`, so there is no frame to pull. Log the
            // reason rather than shipping a silent blank view.
            log::error!(
                "[macos-objc2] canvas: drawRect: widget id={widget_id} cannot render in a \
                 profile without an unstripped widget set"
            );
        }
    }

    /// Allocates a canvas view for `widget_id` sized to `frame`, on the main thread.
    ///
    /// `is_window` selects tree rendering (a window paints its children) over the
    /// single-box rendering a mounted surface uses.
    ///
    /// The ivars are installed before `initWithFrame:`, which is the two-phase
    /// initialisation `define_class!` expects.
    pub(crate) fn new(
        mtm: MainThreadMarker,
        widget_id: u64,
        is_window: bool,
        frame: NSRect,
    ) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(CanvasIvars {
            widget_id: std::cell::Cell::new(widget_id),
            is_window: std::cell::Cell::new(is_window),
        });
        // SAFETY: `this` is an allocated instance with its ivars initialised, and
        // `initWithFrame:` is the designated initializer of `NSView`.
        let view: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        view.install_hover_tracking_area();
        view
    }

    /// Installs the tracking area that makes `-mouseMoved:` fire.
    ///
    /// `NSTrackingInVisibleRect` keeps the area sized to the view as it grows and shrinks, so it is
    /// installed once and never re-added on a geometry change (which would leak an area per
    /// resize). `NSTrackingActiveInKeyWindow` matches the click rule: an inactive window does not
    /// highlight controls under the pointer.
    fn install_hover_tracking_area(&self) {
        let options = NSTrackingAreaOptions::MouseEnteredAndExited
            | NSTrackingAreaOptions::MouseMoved
            | NSTrackingAreaOptions::ActiveInKeyWindow
            | NSTrackingAreaOptions::InVisibleRect;
        for area in self.trackingAreas().iter() {
            self.removeTrackingArea(&area);
        }
        // SAFETY: every Objective-C object is an `AnyObject`, and `self` is a live view.
        let owner: &AnyObject = unsafe { &*(self as *const Self).cast() };
        // SAFETY: `bounds` is the view's own rect; `self` is the area's owner and outlives it,
        // and the tracking area does not retain its owner. `None` is the documented empty user info.
        let area = unsafe {
            NSTrackingArea::initWithRect_options_owner_userInfo(
                NSTrackingArea::alloc(),
                self.bounds(),
                options,
                Some(owner),
                None,
            )
        };
        self.addTrackingArea(&area);
    }

    /// The widget-registry id this canvas presents.
    ///
    /// A window canvas stores the **host** id `create_window` returned; a mounted
    /// surface stores the widget id directly. The event router and the frame lookup
    /// both speak the widget-registry id, so the window case translates here.
    #[cfg(widgets_unstripped)]
    fn widget_target(&self) -> Option<ObjectId> {
        let id = self.ivars().widget_id.get();
        if id == 0 {
            return None;
        }
        if self.ivars().is_window.get() {
            crate::widget::runtime::widget_id_for_host_window(id)
        } else {
            Some(id)
        }
    }

    /// The pointer's position in **widget** coordinates (top-down, absolute).
    ///
    /// `isFlipped` is `false` on this class — the view keeps AppKit's bottom-up native system so
    /// the blit's row order matches the context — so `convertPoint:fromView:` answers a `y`
    /// counted up from the view's bottom edge. Widget geometry counts down from the top, so the
    /// view's own height is subtracted. Without that, every pointer event was mirrored
    /// vertically: a press on the top row hit whichever control lives at `height - y`.
    #[cfg(widgets_unstripped)]
    fn pointer_position(&self, event: &NSEvent) -> Point {
        let local = self.convertPoint_fromView(event.locationInWindow(), None);
        let origin = self.frame().origin;
        let height = self.bounds().size.height;
        Point::new(
            (origin.x + local.x).round() as i32,
            (origin.y + height - local.y).round() as i32,
        )
    }

    /// Translates an AppKit mouse event into a widget event and routes it.
    #[cfg(widgets_unstripped)]
    fn forward_mouse(&self, event: &NSEvent, phase: MousePhase) {
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let Some(widget_id) = self.widget_target() else {
                return;
            };
            let position = self.pointer_position(event);
            let modifiers = crate::platform::macos::accelerator::map_modifiers(
                event.modifierFlags().bits() as u64,
            );
            let translated = match phase {
                MousePhase::Press => Event::mouse_press_with(
                    position.x,
                    position.y,
                    crate::event::mouse_button::PRIMARY,
                    modifiers,
                ),
                MousePhase::Release => Event::MouseRelease {
                    pos: position,
                    button: crate::event::mouse_button::PRIMARY,
                },
                MousePhase::Drag | MousePhase::Move => Event::MouseMove { pos: position },
                MousePhase::SecondaryPress => Event::mouse_press_with(
                    position.x,
                    position.y,
                    crate::event::mouse_button::SECONDARY,
                    modifiers,
                ),
                MousePhase::SecondaryRelease => Event::MouseRelease {
                    pos: position,
                    button: crate::event::mouse_button::SECONDARY,
                },
                MousePhase::DoubleClick => Event::mouse_double_click(
                    position.x,
                    position.y,
                    crate::event::mouse_button::PRIMARY,
                ),
            };
            let delivered = crate::platform::platform_facts().route_pointer_event(
                widget_id,
                &translated,
                position,
            );
            if !delivered {
                return;
            }
            if matches!(
                phase,
                MousePhase::Press | MousePhase::SecondaryPress | MousePhase::DoubleClick
            ) {
                // A click can move focus to a nested control; take the keyboard so
                // subsequent keys are delivered to this view.
                if let Some(window) = self.window() {
                    window.makeFirstResponder(Some(self));
                }
            }
            self.setNeedsDisplay(true);
        }));
        if outcome.is_err() {
            log::error!("[macos-objc2] canvas: panic while forwarding a mouse event");
        }
    }

    /// Translates an AppKit scroll-wheel event into a widget wheel event.
    #[cfg(widgets_unstripped)]
    fn forward_wheel(&self, event: &NSEvent) {
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let Some(widget_id) = self.widget_target() else {
                return;
            };
            let position = self.pointer_position(event);
            let modifiers = crate::platform::macos::accelerator::map_modifiers(
                event.modifierFlags().bits() as u64,
            );
            // AppKit reports a positive `scrollingDeltaY` for scrolling up; the widget
            // layer's wheel `delta.y` is positive for scrolling down, so the sign is
            // inverted (matching `macos::canvas` and `windows::canvas`).
            let translated = Event::wheel(
                event.scrollingDeltaX().round() as i32,
                -(event.scrollingDeltaY().round() as i32),
                modifiers,
            );
            if crate::platform::platform_facts().route_pointer_event(
                widget_id,
                &translated,
                position,
            ) {
                self.setNeedsDisplay(true);
            }
        }));
        if outcome.is_err() {
            log::error!("[macos-objc2] canvas: panic while forwarding a wheel event");
        }
    }

    /// Translates an AppKit key event into a widget key/text event and routes it.
    #[cfg(widgets_unstripped)]
    /// The accessibility role of the widget this canvas presents.
    fn accessibility_role_of_widget(&self) -> crate::platform::accessibility::A11yRole {
        #[cfg(widgets_unstripped)]
        if let Some(id) = self.widget_target() {
            if let Some(role) = crate::widget::runtime::with_widget(id, |widget| {
                crate::platform::accessibility::A11yRole::from(widget.kind())
            }) {
                return role;
            }
        }
        crate::platform::accessibility::A11yRole::Unknown
    }

    /// The label a screen reader announces for this canvas.
    fn accessibility_label_of_widget(&self) -> crate::compat::String {
        #[cfg(widgets_unstripped)]
        if let Some(id) = self.widget_target() {
            if let Some(name) =
                crate::widget::runtime::with_widget(id, |widget| widget.accessible_name())
            {
                return name;
            }
        }
        crate::compat::String::new()
    }

    /// Delivers text AppKit produced from a plain key press to the focused control.
    ///
    /// This is the non-composition half of `NSTextInputClient`: AppKit calls
    /// `insertText:replacementRange:` for printable keys as well as for a commit, and a plain key
    /// must reach the widget as `Event::TextInput` rather than as an IME commit.
    #[cfg(widgets_unstripped)]
    fn forward_ime_text(&self, text: &str) -> bool {
        let Some(widget_id) = self.widget_target() else { return false };
        let target = crate::widget::runtime::focused_widget().unwrap_or(widget_id);
        if crate::widget::runtime::dispatch_event(
            target,
            &Event::TextInput { text: text.to_string() },
        ) {
            true
        } else {
            log::debug!("[macos-objc2] IME text dropped, id={target} is not mounted");
            false
        }
    }

    /// Translates an AppKit key event into a widget key/text event and routes it.
    ///
    /// Gated with its callers (`keyDown:`/`keyUp:`): the widget runtime and the event model
    /// do not exist under `alloc_frugal`, and an ungated method referred to types that were
    /// cfg'd out (this is a `mini × macos` build failure, not a behavioural choice).
    #[cfg(widgets_unstripped)]
    fn forward_key(&self, event: &NSEvent, release: bool) {
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let Some(widget_id) = self.widget_target() else {
                return;
            };
            let key = event.keyCode() as u32;
            let modifiers = crate::platform::macos::accelerator::map_modifiers(
                event.modifierFlags().bits() as u64,
            );
            if release {
                // A release produces no text, so `KeyRelease` is the only event it can be.
                let target = crate::widget::runtime::focused_widget().unwrap_or(widget_id);
                if crate::widget::runtime::dispatch_event(
                    target,
                    &Event::KeyRelease { key, modifiers },
                ) {
                    self.setNeedsDisplay(true);
                }
                return;
            }
            // Printable characters travel as `TextInput` so the widget's text path runs;
            // non-printing keys travel as `KeyPress` with their hardware key code.
            let translated = if let Some(text) = printable_characters(event) {
                Event::TextInput { text }
            } else {
                Event::KeyPress { key, modifiers }
            };
            // Keys follow focus; with nothing focused, the surface owner keeps them.
            let target = crate::widget::runtime::focused_widget().unwrap_or(widget_id);
            if crate::widget::runtime::dispatch_event(target, &translated) {
                self.setNeedsDisplay(true);
            }
        }));
        if outcome.is_err() {
            log::error!("[macos-objc2] canvas: panic while forwarding a key event");
        }
    }
}

/// Maps a widget role to the AppKit role constant a screen reader understands.
///
/// The constants are AppKit's own `NSAccessibilityRole` values, not display strings: VoiceOver
/// resolves the role through them, so no localized copy passes through this crate.
///
/// Gated with its only caller: the role is derived from a mounted widget, which does not exist
/// under `alloc_frugal`.
#[cfg(widgets_unstripped)]
fn ns_accessibility_role_for(
    role: crate::platform::accessibility::A11yRole,
) -> &'static NSAccessibilityRole {
    use crate::platform::accessibility::A11yRole;
    // SAFETY: these are AppKit's immutable role constants; reading the extern statics is the
    // documented way to obtain them and the values live for the process lifetime.
    unsafe {
        match role {
            A11yRole::Button | A11yRole::Switch => NSAccessibilityButtonRole,
            A11yRole::Label | A11yRole::Heading | A11yRole::Paragraph => {
                NSAccessibilityStaticTextRole
            }
            A11yRole::TextField => NSAccessibilityTextFieldRole,
            A11yRole::CheckBox => NSAccessibilityCheckBoxRole,
            A11yRole::RadioButton => NSAccessibilityRadioButtonRole,
            A11yRole::Slider => NSAccessibilitySliderRole,
            A11yRole::ProgressBar => NSAccessibilityProgressIndicatorRole,
            A11yRole::List => NSAccessibilityListRole,
            A11yRole::Table => NSAccessibilityTableRole,
            A11yRole::Image => NSAccessibilityImageRole,
            A11yRole::Link => NSAccessibilityLinkRole,
            A11yRole::Group => NSAccessibilityGroupRole,
            A11yRole::Window => NSAccessibilityWindowRole,
            A11yRole::Dialog | A11yRole::Alert => NSAccessibilityWindowRole,
            A11yRole::Menu => NSAccessibilityMenuRole,
            A11yRole::MenuItem => NSAccessibilityMenuItemRole,
            A11yRole::Tab => NSAccessibilityTabGroupRole,
            A11yRole::ComboBox => NSAccessibilityComboBoxRole,
            A11yRole::SpinButton => NSAccessibilityIncrementorRole,
            A11yRole::StatusBar => NSAccessibilityGroupRole,
            A11yRole::ToolTip => NSAccessibilityGroupRole,
            A11yRole::Tree => NSAccessibilityOutlineRole,
            A11yRole::Unknown => NSAccessibilityUnknownRole,
        }
    }
}

/// Reads text from the `id` AppKit passes to `insertText:`/`setMarkedText:`.
///
/// The argument is documented as an `NSString` or an `NSAttributedString`; both are accepted,
/// and anything else is ignored rather than transmuted.
fn text_of(object: &AnyObject) -> Option<crate::compat::String> {
    if let Some(text) = object.downcast_ref::<NSString>() {
        return Some(text.to_string());
    }
    if let Some(text) = object.downcast_ref::<NSAttributedString>() {
        return Some(text.string().to_string());
    }
    None
}

/// Reads the printable characters of a key event, or `None` when it has none.
#[cfg(widgets_unstripped)]
fn printable_characters(event: &NSEvent) -> Option<crate::compat::String> {
    let characters = event.characters()?;
    let text = characters.to_string();
    let printable: crate::compat::String = text.chars().filter(|ch| !ch.is_control()).collect();
    if printable.is_empty() {
        None
    } else {
        Some(printable)
    }
}

/// Blits a top-down, straight-alpha RGBA frame into a CoreGraphics context.
///
/// The two details the caller should not have to think about:
///
/// 1. **Premultiplication** — the frame is straight alpha; `PremultipliedLast` is
///    not, so RGB is scaled by alpha.
/// 2. **Ownership** — the colour space, bitmap context and image are all released
///    here, including on the early-return paths.
///
/// The image is drawn at its own pixel size (a 1:1 blit). Like
/// `macos/cg.rs::blit_rgba`, no CTM flip is applied: the canvas reports
/// `isFlipped == NO`, so the frame's row order matches the context's.
///
/// Uses `objc2-core-graphics` (already a dependency of the `macos` feature)
/// rather than re-declaring the FFI the cocoa backend keeps in `macos/cg.rs`.
///
/// Returns `false` when a CoreGraphics object cannot be created or the frame is
/// too small for the requested size.
#[cfg(widgets_unstripped)]
fn blit_rgba(context: &CGContext, width: u32, height: u32, frame: &[u8]) -> bool {
    use objc2_core_graphics::{
        CGBitmapContextCreate, CGBitmapContextCreateImage, CGBitmapContextGetBytesPerRow,
        CGBitmapContextGetData, CGColorSpace, CGContext, CGImage, CGImageAlphaInfo,
        CGImageByteOrderInfo, CGInterpolationQuality,
    };

    if width == 0 || height == 0 {
        return false;
    }
    let expected = width as usize * height as usize * 4;
    if frame.len() < expected {
        log::error!(
            "[macos-objc2] canvas: frame is {} bytes, need {} for {width}x{height}",
            frame.len(),
            expected
        );
        return false;
    }

    let Some(color_space) = CGColorSpace::new_device_rgb() else {
        log::error!("[macos-objc2] canvas: CGColorSpaceCreateDeviceRGB returned null");
        return false;
    };

    // `kCGImageAlphaPremultipliedLast | kCGBitmapByteOrder32Big` gives an in-memory R, G, B, A
    // byte order — the order the software renderer produces. The sibling raw-objc backend
    // (`platform::macos::cg`) shipped this pairing with `Order32Little`, which normalises to
    // A, B, G, R and put the alpha byte in the red channel: every window came out tinted with
    // the theme's error red while the frame handed to the blit was correct.
    let bitmap_info = CGImageAlphaInfo::PremultipliedLast.0 | CGImageByteOrderInfo::Order32Big.0;
    // SAFETY: every pointer argument is either null (let CG allocate) or a value
    // derived from the borrowed `frame` and the live colour space.
    let Some(bitmap) = (unsafe {
        CGBitmapContextCreate(
            std::ptr::null_mut(),
            width as usize,
            height as usize,
            8,
            width as usize * 4,
            Some(&color_space),
            bitmap_info,
        )
    }) else {
        log::error!(
            "[macos-objc2] canvas: CGBitmapContextCreate returned null for {width}x{height}"
        );
        return false;
    };

    // Copy in with premultiplied alpha, row by row using the bitmap's **actual**
    // stride: CoreGraphics may pad rows once the width crosses an alignment
    // boundary, and assuming `width * 4` shifts every row after the first.
    let destination = unsafe { CGBitmapContextGetData(Some(&bitmap)) } as *mut u8;
    if !destination.is_null() {
        let stride = unsafe { CGBitmapContextGetBytesPerRow(Some(&bitmap)) };
        for y in 0..height as usize {
            for x in 0..width as usize {
                let source = (y * width as usize + x) * 4;
                let target = y * stride + x * 4;
                let alpha = frame[source + 3] as u32;
                // SAFETY: `target + 3 < height * stride` because the bitmap was
                // created at `height` rows; each row of `destination` is `stride`
                // bytes wide.
                unsafe {
                    *destination.add(target) = (frame[source] as u32 * alpha / 255) as u8;
                    *destination.add(target + 1) = (frame[source + 1] as u32 * alpha / 255) as u8;
                    *destination.add(target + 2) = (frame[source + 2] as u32 * alpha / 255) as u8;
                    *destination.add(target + 3) = alpha as u8;
                }
            }
        }
    }

    let Some(image) = (unsafe { CGBitmapContextCreateImage(Some(&bitmap)) }) else {
        log::error!("[macos-objc2] canvas: CGBitmapContextCreateImage returned null");
        return false;
    };

    // SAFETY: `context` is valid for the duration of the draw (AppKit owns it for
    // the current `drawRect:`), and `image` is live. `NSRect` is an alias of
    // `CGRect`, so the rect built from the image's own pixel size can be passed
    // straight to `CGContextDrawImage`.
    unsafe {
        CGContext::set_interpolation_quality(Some(context), CGInterpolationQuality::None);
        let image_width = CGImage::width(Some(&image)) as f64;
        let image_height = CGImage::height(Some(&image)) as f64;
        let rect = NSRect::new(
            objc2_foundation::NSPoint::new(0.0, 0.0),
            NSSize::new(image_width, image_height),
        );
        CGContext::draw_image(Some(context), rect, Some(&image));
    }
    true
}

// ---------------------------------------------------------------------------
// Window creation
// ---------------------------------------------------------------------------

pub(crate) fn create_ns_window(
    mtm: MainThreadMarker,
    widget_id: u64,
    title: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Retained<NSWindow> {
    let style_mask = NSWindowStyleMask::Titled
        | NSWindowStyleMask::Closable
        | NSWindowStyleMask::Miniaturizable
        | NSWindowStyleMask::Resizable;
    let rect = make_rect(x, y, width, height);
    // SAFETY: NSWindow::initWithContentRect_styleMask_backing_defer is called with
    // a valid MainThreadMarker, ensuring this runs on the main thread. The alloc
    // is obtained from mtm which guarantees the correct memory allocation context.
    // The returned Retained<NSWindow> is always non-null (objc2 init methods return
    // a valid retained object on success, or panic on allocation failure).
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            mtm.alloc(),
            rect,
            style_mask,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    window.setTitle(&NSString::from_str(title));

    // Install a canvas as the window's content view **before** the window is put
    // on screen, so the very first frame the compositor shows is already the
    // library-painted one. Making the window key/ordered front with a plain
    // content view would flash an empty white client area first.
    //
    // The canvas carries `widget_id` and the window flag, so `invalidate_surface`
    // for this window id can mark the content view and `drawRect:` paints the
    // window's own chrome plus every child that implements `Draw`.
    let canvas =
        RustWidgetsObjc2CanvasView::new(mtm, widget_id, true, make_rect(0, 0, width, height));
    // The canvas is the accessibility element for this widget (B28.13). The pointer stays valid
    // for the window's lifetime because `NSWindow` retains its content view.
    crate::platform::accessibility::macos_objc2::bridge()
        .register_handle(widget_id, Retained::as_ptr(&canvas) as *mut AnyObject as usize);
    window.setContentView(Some(&canvas));
    // The canvas doubles as the window's delegate so `windowShouldClose:` forwards the
    // close to the widget layer. NSWindow holds its delegate as a weak (assign)
    // reference, but the canvas is retained as the content view for the window's whole
    // lifetime, so the pointer stays valid. `msg_send!` is used rather than the typed
    // `setDelegate` because that one needs a `ProtocolObject<dyn NSWindowDelegate>`.
    let delegate_ptr = Retained::as_ptr(&canvas) as *mut AnyObject;
    // SAFETY: `window` is a live NSWindow and `delegate_ptr` is a retained canvas for its lifetime.
    let _: () = unsafe { msg_send![&*window, setDelegate: delegate_ptr] };
    window.makeKeyAndOrderFront(None);
    window
}

// ---------------------------------------------------------------------------
// Surface lifecycle
// ---------------------------------------------------------------------------

/// Mounts a canvas view for `id` on the window registered under `parent`.
///
/// Creates [`RustWidgetsObjc2CanvasView`] sized to `rect`, adds it to the parent
/// window's `contentView`, records it in [`SURFACE_VIEWS`] so later
/// resize/invalidate/unmount calls can reach it, and marks it for display.
///
/// # Return value
///
/// `false` when there is no parent window under `parent`, it has no content view,
/// the view cannot be allocated, or the call runs off the main thread.
pub(crate) fn mount_surface_native(
    parent: u64,
    id: u64,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> bool {
    let Some(mtm) = MainThreadMarker::new() else {
        log::error!(
            "[macos-objc2] mount_surface: refused off the AppKit main thread (parent={parent}, id={id})"
        );
        return false;
    };
    // Resolve the parent window through the registry `create_ns_window` populated.
    //
    // The registry is typed as an opaque `*mut c_void`, and it also holds non-window views (surfaces
    // store their canvas there too). The runtime class check keeps a parent id that names a plain
    // view — rather than a window — from being reinterpreted as an `NSWindow`, which would be a type
    // confusion the compiler cannot catch on an erased pointer.
    let parent_ptr = match NATIVE_VIEWS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(&parent)
        .copied()
    {
        Some(ptr) if unsafe { object_is_window(ptr.0 as *mut AnyObject) } => ptr.0 as *mut NSWindow,
        Some(_) => {
            log::error!(
                "[macos-objc2] mount_surface: parent {parent} is not a window; a surface is mounted on\
                 a window's content view"
            );
            return false;
        }
        None => {
            log::error!("[macos-objc2] mount_surface: unknown parent window {parent}");
            return false;
        }
    };
    // SAFETY: the registry only ever stores objects `create_ns_window` made, and
    // the main-thread guard above satisfies AppKit's threading requirement.
    let parent_window: &NSWindow = unsafe { &*parent_ptr };
    let Some(content_view) = parent_window.contentView() else {
        log::error!("[macos-objc2] mount_surface: window {parent} has no content view");
        return false;
    };

    let frame = make_rect(x, y, width, height);
    let canvas = RustWidgetsObjc2CanvasView::new(mtm, id, false, frame);
    crate::platform::accessibility::macos_objc2::bridge()
        .register_handle(id, Retained::as_ptr(&canvas) as *mut AnyObject as usize);
    content_view.addSubview(&canvas);
    canvas.setNeedsDisplay(true);
    // The view is being revealed, which is the platform repainting a surface the
    // library just mounted; announce it like every other platform redraw. `notify_native_redraw`
    // is a full-widget-set item (it drives the repaint queue), so a stripped profile skips it.
    #[cfg(widgets_unstripped)]
    crate::notify_native_redraw(id, None);

    SURFACE_VIEWS
        .lock()
        .unwrap()
        .insert(id, NativePtr(Retained::as_ptr(&canvas) as *mut std::ffi::c_void));
    // The content view retains the canvas as a subview, so dropping the local
    // `Retained` here leaves the AppKit-owned reference intact.
    drop(canvas);
    log::debug!("[macos-objc2] mount_surface: id={id} mounted at ({x}, {y}, {width}, {height})");
    true
}

/// Resizes the mounted surface `id` and asks AppKit to redraw it.
///
/// Returns `false` when `id` has no mounted surface or the call is off-main.
pub(crate) fn resize_surface_native(id: u64, x: i32, y: i32, width: u32, height: u32) -> bool {
    if MainThreadMarker::new().is_none() {
        log::error!("[macos-objc2] resize_surface: refused off the AppKit main thread");
        return false;
    }
    let Some(ptr) = surface_view(id) else {
        log::error!("[macos-objc2] resize_surface: id={id} is not mounted");
        return false;
    };
    // SAFETY: `ptr` was produced by `mount_surface_native` and is only removed by
    // `unmount_surface_native`; `setFrame:` and `setNeedsDisplay:` are NSView API.
    unsafe {
        let view: &NSView = &*(ptr as *const NSView);
        // `setNeedsDisplay:` after `setFrame:`: AppKit does not schedule a display
        // pass for a frame change on its own, so a bare `setFrame:` leaves the old
        // pixels on screen until an unrelated invalidation arrives.
        view.setFrame(make_rect(x, y, width, height));
        view.setNeedsDisplay(true);
    }
    true
}

/// Marks the mounted surface `id` as needing display.
///
/// Returns `false` when `id` has no mounted surface or the call is off-main.
pub(crate) fn invalidate_surface_native(id: u64) -> bool {
    if MainThreadMarker::new().is_none() {
        log::error!("[macos-objc2] invalidate_surface: refused off the AppKit main thread");
        return false;
    }
    let Some(ptr) = surface_view(id) else {
        return false;
    };
    // SAFETY: `ptr` came from the side table populated by `mount_surface_native`;
    // `setNeedsDisplay:` is a valid NSView selector.
    unsafe {
        let view: &NSView = &*(ptr as *const NSView);
        view.setNeedsDisplay(true);
    }
    true
}

/// Marks a window's content view as needing display.
///
/// A repaint request for a **window** id reaches the content view, which is what
/// draws that window's children. Returns `false` for an id with no native window.
pub(crate) fn invalidate_window_native(id: u64) -> bool {
    if MainThreadMarker::new().is_none() {
        log::error!("[macos-objc2] invalidate_window: refused off the AppKit main thread");
        return false;
    }
    // The library asks for a repaint by **widget** id, but `NATIVE_VIEWS` is keyed by
    // the **host** id `create_window` returned (the two id spaces never coincide).
    // Translate at the boundary; an id with no host association is tried as-is, the
    // same fallback `windows::window_hwnd_for_widget_id` uses for its own registry.
    #[cfg(widgets_unstripped)]
    let host_id = crate::widget::runtime::host_window_for(id).unwrap_or(id);
    #[cfg(not(widgets_unstripped))]
    let host_id = id;
    let window_ptr = match NATIVE_VIEWS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(&host_id)
        .copied()
    {
        Some(ptr) => ptr.0 as *mut NSWindow,
        None => return false,
    };
    // SAFETY: the registry only stores objects `create_ns_window` made; the
    // main-thread guard above satisfies AppKit's threading requirement.
    unsafe {
        let window: &NSWindow = &*window_ptr;
        if let Some(content_view) = window.contentView() {
            content_view.setNeedsDisplay(true);
        }
    }
    true
}

/// Removes the mounted surface `id` from its window.
///
/// Returns `false` when `id` has no mounted surface or the call is off-main.
pub(crate) fn unmount_surface_native(id: u64) -> bool {
    if MainThreadMarker::new().is_none() {
        log::error!("[macos-objc2] unmount_surface: refused off the AppKit main thread");
        return false;
    }
    let removed = SURFACE_VIEWS.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).remove(&id);
    let Some(ptr) = removed else {
        log::error!("[macos-objc2] unmount_surface: id={id} is not mounted");
        return false;
    };
    // SAFETY: the side-table entry was removed just above, so the view has no
    // other owner in this module and detaching it from its superview is the
    // correct teardown.
    unsafe {
        let view: &NSView = &*(ptr.0 as *const NSView);
        view.removeFromSuperview();
    }
    true
}

/// Looks up the canvas view pointer for a mounted surface.
fn surface_view(id: u64) -> Option<*mut NSView> {
    SURFACE_VIEWS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(&id)
        .map(|ptr| ptr.0 as *mut NSView)
}

/// Bootstrap the shared `NSApplication` (create + `finishLaunching`).
///
/// Mirrors the cocoa-legacy backend's `init()` so the objc2 backend is a real
/// AppKit application rather than only a state machine. Returns `false` when
/// called off the main thread (nothing is touched in that case).
pub(crate) fn bootstrap_ns_application() -> bool {
    let Some(mtm) = MainThreadMarker::new() else {
        return false;
    };
    let app = NSApplication::sharedApplication(mtm);
    // A GUI host must be a regular app (Dock icon + activation) so its windows can
    // become key and receive input; without this the app is a background accessory.
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    app.finishLaunching();
    app.activate();
    true
}

/// Pumps one AppKit event, waiting at most `timeout` for one to arrive.
///
/// `-[NSApplication nextEventMatchingMask:untilDate:inMode:dequeue:]` blocks until an event is
/// due, so the backend loop waits in the run loop instead of polling AppKit every 16 ms: input is
/// handled the moment it arrives and an idle window does not wake until the timeout backstop.
///
/// The non-blocking variant this replaced (`untilDate: NSDate.distantPast`) is gone with the
/// polling loop; every caller wants the wait, and an unused non-blocking pump is dead code.
pub(crate) fn pump_native_event_until(timeout: core::time::Duration) -> bool {
    dispatch_one_native_event(&NSDate::dateWithTimeIntervalSinceNow(timeout.as_secs_f64()))
}

/// Dispatches at most one event no later than `until`.
fn dispatch_one_native_event(until: &NSDate) -> bool {
    let Some(mtm) = MainThreadMarker::new() else {
        return false;
    };
    let app = NSApplication::sharedApplication(mtm);
    // SAFETY: `NSDefaultRunLoopMode` is a well-known, immutable AppKit mode constant.
    let mode = unsafe { NSDefaultRunLoopMode };
    let event = app.nextEventMatchingMask_untilDate_inMode_dequeue(
        NSEventMask::Any,
        Some(until),
        mode,
        true,
    );
    let dispatched = if let Some(event) = event {
        app.sendEvent(&event);
        true
    } else {
        false
    };
    // A window-close delegate queues its registry retain for release after AppKit's
    // close callback has unwound; releasing the last retain inside that callback
    // could deallocate the NSWindow before `close` returns.
    drain_pending_native_releases();
    dispatched
}
