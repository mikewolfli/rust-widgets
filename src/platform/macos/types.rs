// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! macOS platform types, structs, enums, and helper functions.

#![allow(deprecated)] // Cocoa 0.24 fallback; remove when objc2 backend fully replaces cocoa

use crate::compat::{HashMap, Mutex, String};
use crate::core::ObjectId;
use crate::platform::accessibility::macos::MacOSAccessibilityBridge;
use crate::platform::state::BackendState;
use cocoa::appkit::NSWindowStyleMask;
use cocoa::base::id;
// `nil` is referenced only by `translate_key_event` below, which is gated
// `widgets_unstripped`; importing it unconditionally warns on `mini`.
#[cfg(widgets_unstripped)]
use cocoa::base::nil;
use cocoa::foundation::{NSPoint, NSRect, NSSize};
use objc::{class, msg_send, sel, sel_impl};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum HandleKind {
    /// Top-level NSWindow.
    Window,
    /// A self-drawn widget mounted into a window (see `macos/canvas.rs`).
    ///
    /// Absent from `mini`/`embedded` because that is where `canvas.rs` — its only
    /// producer — is compiled out.
    #[cfg(widgets_unstripped)]
    Canvas,
}
#[derive(Clone, Copy)]
pub(crate) struct CocoaHandle {
    /// Opaque native pointer cast to usize.
    pub(crate) ptr: usize,
    /// Runtime handle kind used for dispatch.
    pub(crate) kind: HandleKind,
}

/// macOS desktop platform adapter.
pub struct MacOSPlatform {
    /// Shared logical widget state split from native handles.
    pub(crate) state: BackendState<HandleKind>,
    /// Logical id -> native handle mapping.
    pub(crate) handles: Mutex<HashMap<ObjectId, CocoaHandle>>,
    /// Platform IME bridge for text input method integration.
    pub(crate) ime_bridge: crate::platform::ime_macos::MacOsImeBridge,
    /// Platform rich clipboard backend.
    pub(crate) clipboard: crate::platform::clipboard_stubs::macos::MacOsClipboard,
    /// Platform accessibility bridge for NSAccessibility notifications.
    pub(crate) a11y_bridge: MacOSAccessibilityBridge,
    /// Accelerator display text per menu item, so a host can confirm which chord
    /// was actually registered (see `Platform::menu_item_shortcut`).
    pub(crate) menu_item_shortcuts: Mutex<HashMap<ObjectId, String>>,
}

/// Translates an AppKit `NSEvent` key event into `(key_code, widget_modifiers)`.
///
/// # Safety
///
/// `event` must be a live `NSEvent` instance, or `nil` (yielding `(0, 0)`).
///
/// Gated with `canvas.rs`: it is the only caller, and `mini`/`embedded` have no
/// self-drawn canvas for a key event to arrive through.
#[cfg(all(target_os = "macos", widgets_unstripped))]
pub(crate) unsafe fn translate_key_event(event: id) -> (u32, u32) {
    if event == nil {
        return (0, 0);
    }
    let key_code: u16 = msg_send![event, keyCode];
    let flags: u64 = msg_send![event, modifierFlags];
    super::accelerator::map_key_modifiers(flags, key_code)
}

/// Returns `true` when the caller is running on the AppKit main thread.
///
/// AppKit objects (`NSWindow`, `NSView`, `NSApplication`, panels, pasteboard
/// singletons, ...) may only be created or mutated from the main thread; doing
/// otherwise raises an Objective-C exception that Rust cannot catch, which
/// aborts the whole process with
/// `fatal runtime error: Rust cannot catch foreign exceptions`.
///
/// Every native entry point in this legacy backend therefore consults this
/// guard first and falls back to a state-only handle when it returns `false`.
/// This mirrors the `objc2::MainThreadMarker::new()` gating already used by the
/// `macos_objc2` backend and by `create_native_dialog` here.
pub(crate) fn is_main_thread() -> bool {
    // SAFETY: `+[NSThread isMainThread]` is a thread-safe class method that is
    // safe to call from any thread and never raises.
    unsafe { msg_send![class!(NSThread), isMainThread] }
}

impl MacOSPlatform {
    /// Creates a new macOS platform adapter.
    pub fn new() -> Self {
        Self {
            state: BackendState::new(),
            handles: Mutex::new(HashMap::new()),
            ime_bridge: crate::platform::ime_macos::MacOsImeBridge::new(),
            clipboard: crate::platform::clipboard_stubs::macos::MacOsClipboard,
            a11y_bridge: MacOSAccessibilityBridge::new(),
            menu_item_shortcuts: Mutex::new(HashMap::new()),
        }
    }
}

crate::impl_default_via_new!(MacOSPlatform);

impl MacOSPlatform {
    pub(crate) fn make_rect(x: i32, y: i32, width: u32, height: u32) -> NSRect {
        NSRect::new(NSPoint::new(x as f64, y as f64), NSSize::new(width as f64, height as f64))
    }
    pub(crate) fn window_style() -> NSWindowStyleMask {
        NSWindowStyleMask::NSTitledWindowMask
            | NSWindowStyleMask::NSClosableWindowMask
            | NSWindowStyleMask::NSResizableWindowMask
            | NSWindowStyleMask::NSMiniaturizableWindowMask
    }

    pub(crate) fn get_handle(&self, widget_id: ObjectId) -> Option<CocoaHandle> {
        let guard = crate::compat::lock(&self.handles);
        let handle = guard.get(&widget_id).copied();
        if handle.is_none() {
            log::error!(
                "[macos] get_handle: widget_id={} not found in handle registry ({} total handles)",
                widget_id,
                guard.len()
            );
        }
        handle
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn register_handle(
        &self,
        kind: HandleKind,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        ptr: usize,
    ) -> ObjectId {
        let id = self.state.create_widget(kind, text, x, y, width, height);
        {
            let mut guard = crate::compat::lock(&self.handles);
            if guard.contains_key(&id) {
                log::warn!(
                    "[macos] register_handle: overwriting existing handle for widget_id={}",
                    id
                );
            }
            guard.insert(id, CocoaHandle { ptr, kind });
        }
        // Register the native handle with the accessibility bridge.
        self.a11y_bridge.register_handle(id, ptr);
        log::trace!("[macos] register_handle: id={}, kind={:?}, ptr=0x{:x}", id, kind, ptr);
        id
    }
    pub(crate) fn as_id(handle: CocoaHandle) -> id {
        handle.ptr as id
    }
    /// Registers a state-only handle (`ptr == 0`) for the off-main-thread
    /// fallback path, keeping widget ids, text and geometry semantics intact
    /// without ever touching AppKit.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn register_state_only_handle(
        &self,
        kind: HandleKind,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.register_handle(kind, text, x, y, width, height, 0)
    }
}
