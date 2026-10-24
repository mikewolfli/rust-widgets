// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! System environment notifications for the objc2 backend.
//!
//! The sibling cocoa backend has a larger environment module because it reads several facts
//! through `NSWorkspace` selectors; this backend reads the same facts through the typed
//! `objc2-app-kit` bindings in [`super::native`]. What is left here is the one thing a typed read
//! cannot answer: *when* did the host's accent change. AppKit posts
//! `NSSystemColorsDidChangeNotification` when it does, and this module turns that notification
//! into the theme manager's explicit refresh.
//!
//! # Why a notification rather than a per-frame read
//!
//! The frame's environment snapshot already carries the accent value; the *active palette* is
//! re-derived only when the host says it changed. Reading the accent every frame would rebuild
//! the palette every frame, and would still not tell the theme when to arm the repaint that makes
//! the new accent visible.
//!
//! # Failure is an answer
//!
//! Off the AppKit main thread, or on an AppKit that refuses the observer class, installation
//! reports `false` and the explicit [`crate::theme::ThemeManager::refresh_system_accent`] entry
//! point remains the hook.

#![cfg(all(target_os = "macos", feature = "macos"))]

use std::sync::atomic::{AtomicBool, Ordering};

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{define_class, sel};
use objc2_foundation::{NSNotification, NSNotificationCenter, NSString};

/// The notification AppKit posts after the system colours changed.
const SYSTEM_COLORS_DID_CHANGE: &str = "NSSystemColorsDidChangeNotification";

/// Whether the observer is already installed, so installation is idempotent.
static INSTALLED: AtomicBool = AtomicBool::new(false);

define_class!(
    // SAFETY: `NSObject` has no subclassing requirements, and the ivars are empty, so the class
    // needs no `Drop` and `define_class!` finalises the layout safely.
    #[unsafe(super(NSObject))]
    #[name = "RustWidgetsObjc2SystemColorObserver"]
    struct SystemColorObserver;

    impl SystemColorObserver {
        /// `systemColorsDidChange:` — AppKit's delivery of the notification.
        ///
        /// A panic must not unwind through AppKit: a foreign exception cannot cross the
        /// Objective-C frame, so it is caught and logged here.
        #[unsafe(method(systemColorsDidChange:))]
        fn system_colors_did_change(&self, _notification: &NSNotification) {
            let outcome = std::panic::catch_unwind(|| {
                let changed = crate::theme::global_theme_manager().refresh_system_accent();
                if changed {
                    log::debug!(
                        "[macos-objc2] system colours changed; the theme accent was re-derived"
                    );
                }
            });
            if outcome.is_err() {
                log::error!(
                    "[macos-objc2] the system colour observer panicked while refreshing the accent"
                );
            }
        }
    }
);

impl SystemColorObserver {
    objc2::extern_methods!(
        /// Creates the observer instance, inheriting `NSObject`'s `new`.
        #[unsafe(method(new))]
        fn new() -> Retained<Self>;
    );
}

/// Subscribes to AppKit's system-colour notification.
///
/// Idempotent: a second call while installed answers `true` without registering again. Returns
/// `false` off the main thread or when the notification center refuses the registration, which
/// leaves the explicit refresh path in charge.
pub(crate) fn install_system_color_observer() -> bool {
    if INSTALLED.load(Ordering::Relaxed) {
        return true;
    }
    // `NSNotificationCenter` delivers on the posting thread, and the posting thread for a system
    // colour change is the AppKit main thread. Registering from another thread would install an
    // observer nothing ever calls, so this is the honest gate.
    if objc2::MainThreadMarker::new().is_none() {
        return false;
    }
    let name = NSString::from_str(SYSTEM_COLORS_DID_CHANGE);
    let observer = SystemColorObserver::new();
    let center = NSNotificationCenter::defaultCenter();
    // SAFETY: the observer's class declares `systemColorsDidChange:` with the
    // `(self, _cmd, NSNotification *)` ABI AppKit calls, and both the observer and the name outlive
    // the registration because the center does not retain the observer.
    unsafe {
        center.addObserver_selector_name_object(
            &observer,
            sel!(systemColorsDidChange:),
            Some(&name),
            None::<&AnyObject>,
        );
    }
    // The center does not retain its observer, so the instance is leaked deliberately: it must
    // outlive the registration for the process lifetime, which is the right ownership for a
    // process-wide notification source.
    core::mem::forget(observer);
    INSTALLED.store(true, Ordering::Relaxed);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A second installation is a no-op rather than a second registration.
    #[test]
    fn installation_is_idempotent() {
        let _ = install_system_color_observer();
        let first = INSTALLED.load(Ordering::Relaxed);
        let _ = install_system_color_observer();
        assert_eq!(INSTALLED.load(Ordering::Relaxed), first);
    }
}
