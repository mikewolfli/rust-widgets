// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Display-link pacing for the cocoa backend: the shared CoreVideo link plus its AppKit observers.
//!
//! # What is shared and what is not
//!
//! The CoreVideo side — the `CVTimeStamp` layout, the mach timebase, the callback that publishes
//! the presented time — is pure C ABI and lives in [`crate::platform::display_link`] for both
//! macOS backends. What cannot be shared is the AppKit half: this backend holds `cocoa`/`objc`
//! bindings and can register the two notifications that keep the link healthy.
//!
//! * `NSApplicationDidChangeScreenParametersNotification` restarts the link so CoreVideo re-reads
//!   the active display set;
//! * `NSApplicationWillTerminateNotification` stops it so the callback thread is gone before the
//!   process tears down AppKit.
//!
//! A host where the observer cannot be installed still gets the shared module's stall recovery,
//! which restarts a link whose callback has gone quiet.
//!
//! # Failure is an answer
//!
//! A host with no window server (CI, a headless session, a sandbox that denies CoreVideo) gets
//! `false` from [`start`] and `None` from [`next_deadline`]; nothing panics and the fixed frame
//! grid keeps running.

#![cfg(all(target_os = "macos", feature = "cocoa-legacy", widgets_unstripped))]

use std::sync::OnceLock;

use cocoa::base::{id, nil};
use cocoa::foundation::NSString;
use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Sel};
use objc::{class, msg_send, sel, sel_impl};

use crate::platform::display_link as shared;

/// Creates the process-wide link and starts it, then installs the AppKit observers once.
///
/// Returns `true` only when CoreVideo accepted every step. A second call while running is a no-op
/// that answers `true`; a call after [`stop`] creates a fresh link. A failure leaves the fixed
/// frame grid in charge and does not poison later attempts.
pub(crate) fn start() -> bool {
    let started = shared::start();
    if started && !shared::observers_installed() {
        // A refusal costs only the notification refresh: the link keeps running and the shared
        // module's stall recovery still restarts it after a display change.
        if install_lifecycle_observers() {
            shared::mark_observers_installed();
        }
    }
    started
}

/// Stops and releases the process-wide display link.
pub(crate) fn stop() {
    shared::stop();
}

/// Whether the display link is currently started.
pub(crate) fn is_running() -> bool {
    shared::is_running()
}

/// The next presented-time instant, when a callback has supplied one.
pub(crate) fn next_deadline() -> Option<std::time::Instant> {
    shared::next_deadline()
}

/// Restarts the link so CoreVideo re-reads the display configuration.
fn restart() {
    shared::restart();
}

/// The Objective-C class that owns the AppKit notifications, or `None` when the runtime refuses
/// to declare it.
fn observer_class() -> Option<*const Class> {
    static CLASS: OnceLock<usize> = OnceLock::new();
    let pointer = *CLASS.get_or_init(|| {
        let superclass = class!(NSObject);
        let Some(mut decl) = ClassDecl::new("RustWidgetsDisplayLinkObserver", superclass) else {
            return 0;
        };
        // SAFETY: both selectors are `NSNotificationCenter` observer API: each implementation has
        // the `(self, _cmd, NSNotification *)` ABI AppKit calls it with.
        unsafe {
            decl.add_method(
                sel!(screenParametersChanged:),
                screen_parameters_changed as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(applicationWillTerminate:),
                application_will_terminate as extern "C" fn(&Object, Sel, id),
            );
        }
        (decl.register() as *const Class) as usize
    });
    if pointer == 0 {
        None
    } else {
        Some(pointer as *const Class)
    }
}

/// `NSApplicationDidChangeScreenParametersNotification` handler.
extern "C" fn screen_parameters_changed(_this: &Object, _cmd: Sel, _notification: id) {
    restart();
}

/// `NSApplicationWillTerminateNotification` handler: stop before AppKit goes away.
extern "C" fn application_will_terminate(_this: &Object, _cmd: Sel, _notification: id) {
    stop();
}

/// Registers the two lifecycle observers on the default notification center.
///
/// Installed once per process, on the first successful [`start`] from the AppKit main thread. The
/// observer object and the notification names live for the process lifetime, which is the right
/// ownership for a process-wide callback source. Returns `false` when the observer cannot be
/// installed; the display link is still usable in that case.
fn install_lifecycle_observers() -> bool {
    if !super::types::is_main_thread() {
        return false;
    }
    let Some(class) = observer_class() else {
        return false;
    };
    // SAFETY: `NSNotificationCenter` is thread-safe and the two selector implementations have the
    // ABI AppKit uses. The observer and the name strings are intentionally process-lifetime
    // values (the center does not retain the observer, so releasing it would unregister it).
    unsafe {
        let center: id = msg_send![class!(NSNotificationCenter), defaultCenter];
        if center == nil {
            return false;
        }
        let observer: id = msg_send![class, new];
        if observer == nil {
            return false;
        }
        let screen_name: id =
            NSString::alloc(nil).init_str("NSApplicationDidChangeScreenParametersNotification");
        if screen_name == nil {
            return false;
        }
        let _: () = msg_send![
            center,
            addObserver: observer
            selector: sel!(screenParametersChanged:)
            name: screen_name
            object: nil
        ];
        let terminate_name: id =
            NSString::alloc(nil).init_str("NSApplicationWillTerminateNotification");
        if terminate_name == nil {
            return false;
        }
        let _: () = msg_send![
            center,
            addObserver: observer
            selector: sel!(applicationWillTerminate:)
            name: terminate_name
            object: nil
        ];
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serialises the two tests that touch the process-wide link. Test threads run in parallel,
    /// and a start or stop in one would otherwise change the other's answer.
    static LINK_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn no_deadline_is_reported_while_stopped() {
        let _guard = LINK_TEST_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        stop();
        assert!(!is_running());
        assert_eq!(next_deadline(), None);
    }

    #[test]
    fn display_link_start_reports_availability_without_panicking() {
        let _guard = LINK_TEST_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        // A host with no window server gets `false` from CoreVideo. That answer is a normal
        // result here: the test only requires that construction neither panics nor reports a
        // running link it did not start.
        let started = start();
        assert_eq!(is_running(), started);
        stop();
        assert!(!is_running());
        assert_eq!(next_deadline(), None);
    }
}
