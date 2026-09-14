// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Native AppKit FFI wrappers for the macOS objc2 backend (BLUE11 R1.5/R2.3 100%).
//!
//! This module is gated behind `#[cfg(target_os = "macos")]` and the
//! `objc2-macos` feature flag.

#![cfg(target_os = "macos")]
#![cfg(feature = "macos")]
// objc2 init methods may require unsafe blocks depending on platform
#![allow(unused_unsafe)]

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::MainThreadMarker;
use objc2::{msg_send, sel};
use objc2_app_kit::{NSApplication, NSBackingStoreType, NSWindow, NSWindowStyleMask};
use objc2_foundation::{NSRect, NSString};

use std::collections::HashMap;
use std::sync::LazyLock;
use std::sync::Mutex;

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
unsafe impl Send for NativePtr {}

/// Thread-local storage for native widget handles.
static NATIVE_VIEWS: LazyLock<Mutex<HashMap<u64, NativePtr>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(crate) fn store_native_view(widget_id: u64, view: *mut std::ffi::c_void) {
    if view.is_null() {
        return;
    }
    remove_native_view(widget_id);
    unsafe {
        let object = view as *mut AnyObject;
        let _: *mut AnyObject = msg_send![object, retain];
    }
    NATIVE_VIEWS.lock().unwrap().insert(widget_id, NativePtr(view));
}

pub(crate) fn remove_native_view(widget_id: u64) {
    let removed = NATIVE_VIEWS.lock().unwrap().remove(&widget_id);
    if let Some(ptr) = removed {
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

fn make_rect(x: i32, y: i32, width: u32, height: u32) -> NSRect {
    NSRect::new(
        objc2_foundation::NSPoint::new(x as f64, y as f64),
        objc2_foundation::NSSize::new(width.max(1) as f64, height.max(1) as f64),
    )
}

pub(crate) fn create_ns_window(
    mtm: MainThreadMarker,
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
    window.makeKeyAndOrderFront(None);
    window
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
    app.finishLaunching();
    true
}
