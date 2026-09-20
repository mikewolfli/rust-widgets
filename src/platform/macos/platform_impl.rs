// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `impl Platform for MacOSPlatform` — the main trait implementation.

#![allow(deprecated)] // Cocoa 0.24 fallback; remove when objc2 backend fully replaces cocoa

use crate::compat::String;
use crate::core::{ObjectId, PlatformFamily};
use crate::platform::accessibility::AccessibilityBridge;
use crate::platform::clipboard::RichClipboardBackend;
use crate::platform::ime::ImeBridge;
use crate::platform::macos::types::*;
use crate::platform::Platform;
use cocoa::appkit::{
    NSApp, NSApplication, NSApplicationActivationOptions, NSApplicationActivationPolicyRegular,
    NSBackingStoreBuffered, NSRunningApplication, NSView, NSWindow,
};
use cocoa::base::{id, nil, BOOL, NO};
use cocoa::foundation::{NSAutoreleasePool, NSPoint, NSRect, NSString};
use objc::{class, msg_send, sel, sel_impl};
use std::ffi::CStr;
use std::os::raw::c_char;

impl Platform for MacOSPlatform {
    fn as_any(&self) -> &dyn crate::compat::Any {
        self
    }
    fn backend_name(&self) -> &'static str {
        "cocoa"
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
    fn invalidate_surface_rect(&self, id: ObjectId, rect: crate::core::Rect) -> bool {
        self.invalidate_surface_rect_impl(id, rect)
    }
    fn family(&self) -> PlatformFamily {
        PlatformFamily::Desktop
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
        // SAFETY: NSApp() returns the shared application instance initialized in init().
        // run() must be called on the main thread, which is guaranteed by the platform
        // contract (init is called before run on the same thread).
        unsafe {
            NSApp().run();
        }
    }
    fn quit(&self) {
        // Stopping the shared application is also main-thread-only.
        if !super::types::is_main_thread() {
            log::debug!("[macos] quit skipped: not on the AppKit main thread");
            return;
        }
        // SAFETY: NSApp().stop_(nil) is safe to call on the main thread after the
        // application has been initialized. The nil argument tells the app to stop
        // without a specific sender.
        unsafe {
            NSApp().stop_(nil);
        }
    }
    fn destroy_widget(&self, widget_id: ObjectId) -> bool {
        // Teardown is safe on any thread: nothing here messages AppKit. The
        // retained native objects (NSWindow/NSView instances) stay referenced by
        // the AppKit view hierarchy, which releases them when the window closes.
        // Off-main the backend never constructed a native object at all (it
        // registered a state-only handle), so the state record and side tables
        // are the only per-widget resources in either case.
        //
        // Each lock guard is released at the end of its own statement so that no
        // two of the backend's mutexes are ever held at the same time.
        self.handles.lock().expect("macos handle lock poisoned").remove(&widget_id);
        // Drop the per-widget accessibility registration that `register_handle` added.
        self.a11y_bridge.unregister_handle(widget_id);
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
            let raw_content = NSView::alloc(nil);
            let content_view = if raw_content == nil {
                log::error!("[macos] create_window: NSView::alloc returned nil (out of memory?)");
                pool.drain();
                return 0;
            } else {
                NSView::initWithFrame_(raw_content, Self::make_rect(0, 0, width, height))
            };

            // Check for nil after NSString::alloc for the title
            let raw_title = NSString::alloc(nil);
            if raw_title == nil {
                log::error!("[macos] create_window: NSString::alloc returned nil (out of memory?)");
                pool.drain();
                return 0;
            }

            let _: () = msg_send![window, setContentView: content_view];
            window.cascadeTopLeftFromPoint_(NSPoint::new(20.0, 20.0));
            NSWindow::setTitle_(window, raw_title.init_str(title));
            window.makeKeyAndOrderFront_(nil);
            let _: () = msg_send![window, display];
            let id = self.register_handle(
                HandleKind::Window,
                title,
                x,
                y,
                width,
                height,
                window as usize,
            );
            // The id has to exist before it can be associated with the delegate, so this
            // comes after `register_handle`. A window resized by the user then re-runs the
            // host's layout instead of keeping stale child geometry.
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
        let shortcuts = self.menu_item_shortcuts.lock().ok()?;
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
    fn poll_menu_triggered(&self) -> Option<u64> {
        let mut events = menu_events().lock().expect("menu event lock poisoned");
        if events.is_empty() {
            None
        } else {
            Some(events.remove(0))
        }
    }

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
        let result = std::panic::catch_unwind(|| unsafe {
            let pb: id = msg_send![class!(NSPasteboard), generalPasteboard];
            if pb == nil {
                return false;
            }
            let _: () = msg_send![pb, clearContents];
            let ns_str = NSString::alloc(nil).init_str(text);
            let type_str = NSString::alloc(nil).init_str("public.utf8-plain-text");
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
        let result = std::panic::catch_unwind(|| unsafe {
            let pb: id = msg_send![class!(NSPasteboard), generalPasteboard];
            if pb == nil {
                return None;
            }
            let type_str = NSString::alloc(nil).init_str("public.utf8-plain-text");
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

    fn set_widget_text(&self, widget_id: ObjectId, text: &str) {
        self.state.set_text(widget_id, text);
        // Keep the live AppKit title in step when there is a native window. A
        // state-only handle has `ptr == 0`, so the message is skipped entirely.
        if let Some(handle) = self.get_handle(widget_id) {
            if handle.ptr != 0 && handle.kind == HandleKind::Window {
                // SAFETY: the handle was created in this process for an NSWindow
                // and `setTitle:` is declared by NSWindow.
                unsafe {
                    let window = Self::as_id(handle);
                    let title = NSString::alloc(nil).init_str(text);
                    NSWindow::setTitle_(window, title);
                }
            }
        }
    }

    fn set_widget_enabled(&self, widget_id: ObjectId, enabled: bool) {
        self.state.set_enabled(widget_id, enabled);
    }

    fn is_widget_enabled(&self, widget_id: ObjectId) -> bool {
        self.state.enabled(widget_id)
    }

    fn set_widget_visible(&self, widget_id: ObjectId, visible: bool) {
        self.state.set_visible(widget_id, visible);
    }

    fn is_widget_visible(&self, widget_id: ObjectId) -> bool {
        self.state.visible(widget_id)
    }

    fn set_widget_geometry(&self, widget_id: ObjectId, x: i32, y: i32, width: u32, height: u32) {
        self.state.set_geometry(widget_id, x, y, width, height);
    }

    /// Window state (maximised/minimised/fullscreen/resizable) round-trips
    /// through the record, so it works for the off-main state-only path too.
    fn set_window_state(
        &self,
        widget_id: ObjectId,
        flag: crate::platform::WindowStateFlag,
        on: bool,
    ) -> bool {
        self.state.set_window_state(widget_id, flag, on)
    }

    fn is_window_in_state(
        &self,
        widget_id: ObjectId,
        flag: crate::platform::WindowStateFlag,
    ) -> Option<bool> {
        self.state.window_state(widget_id, flag)
    }

    fn set_window_min_size(&self, widget_id: ObjectId, width: u32, height: u32) -> bool {
        self.state.set_window_min_size(widget_id, width, height)
    }

    fn window_min_size(&self, widget_id: ObjectId) -> Option<(u32, u32)> {
        self.state.window_min_size(widget_id)
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

    fn accessibility_bridge(&self) -> Option<&dyn AccessibilityBridge> {
        Some(&self.a11y_bridge)
    }
}
