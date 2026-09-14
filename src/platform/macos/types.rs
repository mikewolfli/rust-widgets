// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! macOS platform types, structs, enums, and helper functions.

#![allow(deprecated)] // Cocoa 0.24 fallback; remove when objc2 backend fully replaces cocoa

use crate::core::ObjectId;
use crate::platform::accessibility::macos::MacOSAccessibilityBridge;
use crate::platform::state::BackendState;
use cocoa::appkit::NSWindowStyleMask;
use cocoa::base::{id, nil};
use cocoa::foundation::{NSPoint, NSRect, NSSize};
use objc::{class, msg_send, sel, sel_impl};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

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

static MENU_EVENTS: OnceLock<Mutex<Vec<u64>>> = OnceLock::new();
pub(crate) fn menu_events() -> &'static Mutex<Vec<u64>> {
    // Shared menu-trigger queue used by Cocoa selector bridge.
    MENU_EVENTS.get_or_init(|| Mutex::new(Vec::new()))
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
    map_key_modifiers(flags, key_code)
}

/// Translates an AppKit `NSEvent` key event into `(key_code, widget_modifiers)`.
///
/// Testable core of [`translate_key_event`]: takes the raw AppKit values so the
/// mapping can be asserted without constructing an `NSEvent`.
///
/// # Modifier mapping
///
/// The widget layer uses a small fixed bitfield: shift = 1, control = 2,
/// alt = 4, meta/command = 8 (see `Modifiers::from_event_bits`). This converts
/// AppKit's `NSEventModifierFlag` bits into that layout, so a `KeyPress`
/// arriving through a self-drawn canvas carries the same bits as one arriving
/// through the regular event loop.
///
/// Command **must** be forwarded as bit 3: it is the macOS primary accelerator
/// (`⌘Z`, `⌘S`, ...). Dropping it made every Command chord reach the widget as a
/// bare key press, which is why application shortcuts never fired on macOS.
///
/// # Key code
///
/// AppKit's `keyCode` is a hardware layout code and is returned unchanged; the
/// widget layer's key handling is written against exactly those codes for the
/// non-printing keys (arrows, Enter, Escape, Backspace, Delete, Page Up/Down,
/// Home/End). Printable characters travel as `Event::TextInput` and therefore
/// never depend on this number.
#[cfg(widgets_unstripped)]
pub(crate) fn map_key_modifiers(appkit_flags: u64, key_code: u16) -> (u32, u32) {
    // AppKit modifier flags are shared with the accelerator parser, which owns
    // their definitions so the two macOS backends cannot disagree on them.
    use super::accelerator::{MOD_COMMAND, MOD_CONTROL, MOD_OPTION, MOD_SHIFT};
    const WIDGET_SHIFT: u32 = 1;
    const WIDGET_CONTROL: u32 = 2;
    const WIDGET_ALT: u32 = 4;
    const WIDGET_META: u32 = 8;
    let mut modifiers = 0u32;
    if appkit_flags & MOD_SHIFT != 0 {
        modifiers |= WIDGET_SHIFT;
    }
    if appkit_flags & MOD_CONTROL != 0 {
        modifiers |= WIDGET_CONTROL;
    }
    if appkit_flags & MOD_OPTION != 0 {
        modifiers |= WIDGET_ALT;
    }
    if appkit_flags & MOD_COMMAND != 0 {
        modifiers |= WIDGET_META;
    }
    (key_code as u32, modifiers)
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

impl Default for MacOSPlatform {
    fn default() -> Self {
        Self::new()
    }
}

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
        let guard = self.handles.lock().expect("macos handle lock poisoned");
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
            let mut guard = self.handles.lock().expect("macos handle lock poisoned");
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
