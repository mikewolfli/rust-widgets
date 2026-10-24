// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! AppKit environment facts for the cocoa (legacy) backend.
//!
//! BLUE28 B28.06: the style layer already had `high_contrast`, `motion_preference` and `locale`,
//! but the platform side forwarded none of them. This module is the AppKit half of the answer:
//! `NSWorkspace`'s accessibility display flags and `NSLocale.currentLocale`. The values are read
//! on demand (the style layer takes a snapshot once per frame), never cached here — macOS posts
//! notifications when these change, and a caller that polls sees the new value.
//!
//! The accent has a second half: [`install_system_color_observer`] subscribes to AppKit's
//! system-colour notification and calls the theme manager's explicit refresh. The frame's
//! snapshot already carries the accent value, but the active palette is only re-derived when
//! the host says the accent changed, so the observer is what turns a system event into a new
//! palette without rebuilding it every frame. The observer is the only notification
//! subscription in this module; the other facts stay on-demand reads.
//!
//! `NSWorkspace` and `NSLocale` are main-thread-only in practice; every entry point answers `None`
//! off the main thread rather than messaging AppKit from a worker and raising.

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
pub(crate) fn high_contrast() -> Option<bool> {
    if !super::types::is_main_thread() {
        return None;
    }
    // SAFETY: main thread established; the workspace and the selector are immutable AppKit facts.
    unsafe {
        let workspace = workspace_implements(sel!(accessibilityDisplayShouldIncreaseContrast))?;
        let answer: BOOL = msg_send![workspace, accessibilityDisplayShouldIncreaseContrast];
        Some(answer != NO)
    }
}

/// Whether the user asked for reduced motion.
///
/// The macOS "Reduce motion" accessibility setting. `None` off the main thread or when the
/// selector is unavailable.
pub(crate) fn reduce_motion() -> Option<bool> {
    if !super::types::is_main_thread() {
        return None;
    }
    // SAFETY: as above.
    unsafe {
        let workspace = workspace_implements(sel!(accessibilityDisplayShouldReduceMotion))?;
        let answer: BOOL = msg_send![workspace, accessibilityDisplayShouldReduceMotion];
        Some(answer != NO)
    }
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
pub(crate) fn system_locale() -> Option<&'static str> {
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
        let tag: String = CStr::from_ptr(utf8).to_string_lossy().into_owned();
        crate::platform::locale::intern(Some(tag.replace('_', "-"))).or_else(|| {
            crate::platform::locale::intern(crate::platform::locale::from_environment())
        })
    }
}
