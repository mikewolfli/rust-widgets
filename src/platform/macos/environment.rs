// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! AppKit environment facts for the cocoa (legacy) backend.
//!
//! BLUE28 B28.06: the style layer already had `high_contrast`, `motion_preference` and `locale`,
//! but the platform side forwarded none of them. This module is the AppKit half of the answer:
//! `NSWorkspace`'s accessibility display flags and `NSLocale.currentLocale`.
//!
//! # Caching, and what invalidates it
//!
//! The style layer polls `Platform::environment_generation` every 250 ms (`src/style/environment.rs`),
//! and that derivation reads each of these facts. Reading them fresh on every poll cost 4+ AppKit
//! `msg_send` round trips four times a second for settings a user changes on the order of days, so
//! the three facts named in [`cached_environment_fact`] — `high_contrast`, `reduce_motion` and
//! `system_locale` (which caches both the `NSLocale` read and the environment-variable fallback) —
//! are held in a one-second TTL slot. **The wrapper is the shared one**
//! ([`crate::platform::environment_cache`]), so a notification that reaches
//! `Platform::notify_environment_changed` drops these readings immediately and the TTL remains only
//! the backstop for a change no notification announced (P-302). The system colour notification
//! below refreshes the accent palette but does not itself touch these slots or the environment
//! revision (the missing push source for this backend is tracked as issue P-201), so a changed
//! accessibility switch becomes visible within one second and no later. `system_theme` and
//! `accent_color` are not cached here: the appearance read lives in `platform_impl`, and the accent
//! is delivered by the notification instead of by polling.
//!
//! The accent has a second half: [`install_system_color_observer`] subscribes to AppKit's
//! system-colour notification and calls the theme manager's explicit refresh. The frame's
//! snapshot already carries the accent value, but the active palette is only re-derived when
//! the host says the accent changed, so the observer is what turns a system event into a new
//! palette without rebuilding it every frame. The observer is the only notification
//! subscription in this module; the other facts stay on-demand reads.
//!
//! `NSWorkspace`'s accessibility reads are main-thread-only in practice; their entry points answer
//! `None` off the main thread rather than messaging AppKit from a worker and raising. That check
//! stays **outside** the cache: an off-main answer of `None` describes this thread, not the host, so
//! caching it would hand the UI thread a fabricated "cannot tell" for up to a second. `NSLocale` is
//! documented thread-safe, so the locale read is not gated that way.

#[cfg(feature = "runtime-fonts")]
use crate::compat::Box;
use crate::compat::String;
use crate::compat::Vec;
use cocoa::base::{id, nil, BOOL, NO};
#[cfg(device_profile)]
use cocoa::foundation::NSString;
#[cfg(device_profile)]
use objc::declare::ClassDecl;
#[cfg(device_profile)]
use objc::runtime::{Object, Sel};
use objc::{class, msg_send, sel, sel_impl};
use std::ffi::{c_char, CStr};
// P-302: the TTL slot and its invalidation rule live once, in the shared wrapper, so the macOS,
// Windows and Android backends cannot drift and a notification invalidates every one of them.
#[cfg(not(alloc_frugal))]
use crate::platform::environment_cache::{cached_environment_fact, EnvironmentFactCache};

/// The `mini` profile compiles no environment cache (see `platform::environment_cache`), so the
/// same call has to stay available without one: the reading goes straight to AppKit on every
/// question. That is the honest shape for a sized profile — it trades the repeated read for the
/// slot — and it keeps the call sites below identical in every build.
#[cfg(alloc_frugal)]
fn cached_environment_fact<T: Copy>(
    _cache: &'static EnvironmentFactCache<T>,
    read: impl FnOnce() -> T,
) -> T {
    read()
}

/// The slot shape, declared only where [`cached_environment_fact`] really caches.
#[cfg(alloc_frugal)]
type EnvironmentFactCache<T> =
    std::sync::OnceLock<std::sync::Mutex<Option<(std::time::Instant, T)>>>;

static HIGH_CONTRAST_CACHE: EnvironmentFactCache<Option<bool>> = std::sync::OnceLock::new();
static REDUCE_MOTION_CACHE: EnvironmentFactCache<Option<bool>> = std::sync::OnceLock::new();
static SYSTEM_LOCALE_CACHE: EnvironmentFactCache<Option<&'static str>> = std::sync::OnceLock::new();

/// Whether `+[NSWorkspace sharedWorkspace]` implements `selector`.
///
/// # Safety
///
/// Called only from the AppKit main thread, and `selector` is one of the two literal selectors
/// below. The workspace is an AppKit singleton.
unsafe fn workspace_implements(selector: objc::runtime::Sel) -> Option<id> {
    let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
    if workspace == nil {
        return None;
    }
    let responds: BOOL = msg_send![workspace, respondsToSelector: selector];
    (responds != NO).then_some(workspace)
}

/// Whether the user asked for increased contrast.
///
/// `NSWorkspace.accessibilityDisplayShouldIncreaseContrast` is the same source winit's AppKit
/// backend reads. `None` off the main thread or on an AppKit that does not implement the selector.
/// Wrapped in the one-second environment cache described by [`cached_environment_fact`].
pub(crate) fn high_contrast() -> Option<bool> {
    if !super::types::is_main_thread() {
        return None;
    }
    cached_environment_fact(&HIGH_CONTRAST_CACHE, || {
        // SAFETY: main thread established; the workspace and the selector are immutable AppKit
        // facts.
        unsafe {
            let workspace = workspace_implements(sel!(accessibilityDisplayShouldIncreaseContrast))?;
            let answer: BOOL = msg_send![workspace, accessibilityDisplayShouldIncreaseContrast];
            Some(answer != NO)
        }
    })
}

/// Whether the user asked for reduced motion.
///
/// The macOS "Reduce motion" accessibility setting. `None` off the main thread or when the
/// selector is unavailable. Wrapped in the one-second environment cache described by
/// [`cached_environment_fact`].
pub(crate) fn reduce_motion() -> Option<bool> {
    if !super::types::is_main_thread() {
        return None;
    }
    cached_environment_fact(&REDUCE_MOTION_CACHE, || {
        // SAFETY: as above.
        unsafe {
            let workspace = workspace_implements(sel!(accessibilityDisplayShouldReduceMotion))?;
            let answer: BOOL = msg_send![workspace, accessibilityDisplayShouldReduceMotion];
            Some(answer != NO)
        }
    })
}

/// Whether the user asked for translucent materials to be reduced.
///
/// `NSWorkspace.accessibilityDisplayShouldReduceTransparency` is the same source the contrast and
/// motion reads above use. `None` off the main thread or when the selector is unavailable, which
/// is distinct from `Some(false)`: the latter was observed.
pub(crate) fn reduce_transparency() -> Option<bool> {
    if !super::types::is_main_thread() {
        return None;
    }
    // SAFETY: as above; the workspace and the selector are immutable AppKit facts.
    unsafe {
        let workspace = workspace_implements(sel!(accessibilityDisplayShouldReduceTransparency))?;
        let answer: BOOL = msg_send![workspace, accessibilityDisplayShouldReduceTransparency];
        Some(answer != NO)
    }
}

/// The host's current accent colour.
///
/// `NSColor.controlAccentColor` is a dynamic colour: the system updates it when the user changes
/// the accent in System Settings, so reading on demand is what makes a running application see the
/// new choice. The colour is converted to sRGB before its components are read because the system
/// does not promise which colour space the dynamic colour uses.
///
/// `None` off the main thread, on a system whose `NSColor` does not implement the selector, or
/// when AppKit returns no colour.
pub(crate) fn accent_color() -> Option<crate::core::Color> {
    if !super::types::is_main_thread() {
        return None;
    }
    // SAFETY: the main thread is established; `NSColor`/`NSColorSpace` are AppKit singletons and
    // the component reads happen before this function returns.
    unsafe {
        let responds: BOOL =
            msg_send![class!(NSColor), respondsToSelector: sel!(controlAccentColor)];
        if responds == NO {
            return None;
        }
        let color: id = msg_send![class!(NSColor), controlAccentColor];
        if color == nil {
            return None;
        }
        let space: id = msg_send![class!(NSColorSpace), sRGBColorSpace];
        if space == nil {
            return None;
        }
        let converted: id = msg_send![color, colorUsingColorSpace: space];
        if converted == nil {
            return None;
        }
        let to_unit = |component: f64| component.clamp(0.0, 1.0) as f32;
        let red: f64 = msg_send![converted, redComponent];
        let green: f64 = msg_send![converted, greenComponent];
        let blue: f64 = msg_send![converted, blueComponent];
        let alpha: f64 = msg_send![converted, alphaComponent];
        Some(crate::core::Color::from_f32(
            to_unit(red),
            to_unit(green),
            to_unit(blue),
            to_unit(alpha),
        ))
    }
}

/// Whether the system-colour observer has already been installed in this process.
///
/// A `false` is retried on the next main-thread call rather than cached: the theme manager may
/// initialise on a worker thread first, and that attempt is allowed to fail without disabling
/// the observer for the process.
#[cfg(device_profile)]
static SYSTEM_COLOR_OBSERVER_INSTALLED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Installs the AppKit observer that follows host accent changes.
///
/// `NSSystemColorsDidChangeNotification` is the notification AppKit posts after the system
/// colours change; the accent is one of those system colours, so this is the notification the
/// platform offers for the fact. The observer calls
/// [`ThemeManager::refresh_system_accent`](crate::theme::ThemeManager::refresh_system_accent),
/// which re-reads the accent and re-derives an accent-derived palette; that method arms the
/// repaint, so no frame-loop polling is needed. If a particular macOS version does not post the
/// notification for an accent-only change, the explicit refresh remains the hook — this
/// function never claims the observer fired.
///
/// Returns whether the observer is installed after the call. `false` is the honest answer off
/// the main thread (AppKit notifications and `NSColor` are main-thread facts), when the
/// Objective-C runtime refuses the observer class, or when AppKit returns no notification
/// centre. A caller that receives `false` keeps the explicit refresh entry point as its hook.
///
/// # Threading and lock order
///
/// The callback runs on the main thread. It takes the theme mutex, so it must never be
/// delivered **synchronously** from a scope that already holds that mutex: the mutex is not
/// reentrant. AppKit posts this notification from its run loop, which is outside the frame
/// step, so the delivery point is correct today; a caller that forwards the notification by
/// hand must do so with no theme guard held. The lock order is theme -> frame_wait (see
/// [`crate::theme::global_theme_manager`]), and this observer does not invert it.
///
/// The observer and its `NSString` notification name are deliberately kept for the process
/// lifetime: the notification centre does not retain a selector-based observer, and the name is
/// read by the centre after this function returns. Both are one object each, allocated once.
#[cfg(device_profile)]
pub(crate) fn install_system_color_observer() -> bool {
    use std::sync::atomic::Ordering;

    if SYSTEM_COLOR_OBSERVER_INSTALLED.load(Ordering::Relaxed) {
        return true;
    }
    if !super::types::is_main_thread() {
        return false;
    }
    let installed = install_system_color_observer_once();
    if installed {
        SYSTEM_COLOR_OBSERVER_INSTALLED.store(true, Ordering::Relaxed);
    }
    installed
}

/// The one-shot half of [`install_system_color_observer`], separated so the flag above is the
/// only cached state.
#[cfg(device_profile)]
fn install_system_color_observer_once() -> bool {
    // SAFETY: this runs on the AppKit main thread (checked by the caller). The declared class has
    // one method whose ABI is the `extern "C" fn(&Object, Sel, id)` selector signature, the
    // observer is created with `new` and kept alive for the process, and the notification name
    // string is allocated once and kept alive for the notification centre to read.
    unsafe {
        let Some(mut declaration) =
            ClassDecl::new("RustWidgetsSystemColorObserver", class!(NSObject))
        else {
            log::debug!(
                "[macos] the Objective-C runtime refused RustWidgetsSystemColorObserver; the \
                 accent reverts to the explicit refresh path"
            );
            return false;
        };
        declaration.add_method(
            sel!(systemColorsDidChange:),
            system_colors_did_change as extern "C" fn(&Object, Sel, id),
        );
        let observer_class = declaration.register();
        let observer: id = msg_send![observer_class, new];
        if observer == nil {
            return false;
        }
        let center: id = msg_send![class!(NSNotificationCenter), defaultCenter];
        if center == nil {
            return false;
        }
        let name: id = NSString::alloc(nil).init_str("NSSystemColorsDidChangeNotification");
        if name == nil {
            return false;
        }
        let _: () = msg_send![
            center,
            addObserver: observer
            selector: sel!(systemColorsDidChange:)
            name: name
            object: nil
        ];
        true
    }
}

/// The notification trampoline: AppKit calls this on the main thread after system colours change.
///
/// The environment provider is thread-local, so an off-main delivery cannot answer for the UI
/// thread's provider and is ignored rather than refreshing the wrong snapshot.
#[cfg(device_profile)]
extern "C" fn system_colors_did_change(_this: &Object, _cmd: Sel, _notification: id) {
    if !super::types::is_main_thread() {
        return;
    }
    let outcome = std::panic::catch_unwind(|| {
        let changed = crate::theme::global_theme_manager().refresh_system_accent();
        if changed {
            log::debug!("[macos] system colours changed; the theme accent was re-derived");
        }
    });
    if outcome.is_err() {
        log::error!("[macos] the system colour observer panicked while refreshing the accent");
    }
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
            Ok(bytes) if !bytes.is_empty() => return Some(&*Box::leak(bytes.into_boxed_slice())),
            Ok(_) => log::debug!("[fonts] {path} is empty; not registering a system UI face"),
            Err(error) => {
                log::debug!("[fonts] could not read {path}: {error}")
            }
        }
    }
    None
}

/// The installed families, their style names and backing files, through CoreText.
///
/// `NSFontManager.availableFontFamilies` reports names only; CoreText's `kCTFontURLAttribute` is
/// what supplies the file the renderer can register. The enumeration is not main-thread-only.
pub(crate) fn system_fonts() -> Vec<crate::platform::types::SystemFont> {
    crate::platform::fonts_apple::system_fonts()
}

/// The user's locale, normalized by `NSLocale.localeIdentifier` (for example `en_US`).
///
/// AppKit returns an autoreleased `NSString`; the UTF-8 bytes are copied before the call returns,
/// so no object outlives this function.
///
/// # Cost
///
/// Two allocations and a global lock used to be paid on every style-environment poll: the C string
/// was copied into an owned `String`, `replace` allocated a second `String` for the same text, and
/// the interner took its lock. Now the identifier is read with `to_string_lossy` — which **borrows**
/// when the bytes are valid UTF-8, as a locale identifier always is — and the hyphen form is built in
/// a single allocation with one pass that also decides whether any dash is needed at all. The final
/// value is interned, so repeated reads of the same tag keep returning the same pointer, and it is
/// held in the one-second environment cache (see [`cached_environment_fact`]) so the interner lock is
/// not taken once per poll either. The returned string is byte-identical to the previous spelling:
/// `en_US` → `en-US`.
pub(crate) fn system_locale() -> Option<&'static str> {
    cached_environment_fact(&SYSTEM_LOCALE_CACHE, system_locale_uncached)
}

/// The uncached half of [`system_locale`], kept separate so the cache wrapper stays the only reader.
fn system_locale_uncached() -> Option<&'static str> {
    // SAFETY: every object messaged is an AppKit singleton or a child of one, and the UTF-8 buffer
    // is read before the autorelease pool that owns it can drain.
    unsafe {
        let locale: id = msg_send![class!(NSLocale), currentLocale];
        if locale == nil {
            return None;
        }
        let identifier: id = msg_send![locale, localeIdentifier];
        if identifier == nil {
            return None;
        }
        let utf8: *const c_char = msg_send![identifier, UTF8String];
        if utf8.is_null() {
            return None;
        }
        let raw = CStr::from_ptr(utf8).to_string_lossy();
        // One allocation: `to_string_lossy` borrows when the identifier is valid UTF-8, and the
        // hyphenated form is written straight out instead of building a copy and then replacing it
        // into a second string. Identifiers without a territory (`en`) take the borrow, which then
        // allocates nothing more than the interned copy `locale::intern` makes anyway.
        let tag = if raw.contains('_') {
            let mut tag = String::with_capacity(raw.len());
            tag.extend(raw.chars().map(|ch| if ch == '_' { '-' } else { ch }));
            tag
        } else {
            raw.into_owned()
        };
        crate::platform::locale::intern(Some(tag)).or_else(|| {
            crate::platform::locale::intern(crate::platform::locale::from_environment())
        })
    }
}
