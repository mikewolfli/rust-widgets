//! Native surface for **self-drawn** widgets on macOS.
//!
//! Registers an `NSView` subclass (`RustWidgetsCanvasView`) whose `drawRect:`
//! pulls one RGBA frame out of [`crate::widget::runtime`] and blits it into the
//! view's CoreGraphics context. That is the entire contract: the widget owns its
//! pixels, this view owns the window region they land in.
//!
//! See `docs/plans/self_drawn_mounting.md` for why this is one capability rather
//! than a `create_*` method per self-drawn widget kind.
//!
//! # Feature gate
//!
//! This module needs `crate::widget::runtime` to look widgets up by id and to
//! render frames, and `widget::runtime` does not exist in `mini`/`embedded`
//! builds (see `src/widget/mod.rs`). It is therefore gated on the same pair
//! conditions — a profile that has no widget registry has no way to host a
//! self-drawn surface, and `supports_self_drawn()` reports `false` accordingly.
//!
//! Without this gate the `mini` profile fails to build with seven errors about a
//! missing `widget::runtime`, which is a worse outcome than simply not offering
//! the capability.

#![cfg(all(
    target_os = "macos",
    feature = "cocoa-legacy",
    not(any(feature = "mini", feature = "embedded"))
))]

use super::cg;
use super::types::{self as macos_types, CocoaHandle, HandleKind, MacOSPlatform};
use crate::core::{Color, ObjectId, Point, Rect, Size};
use crate::event::Event;
use cocoa::appkit::NSWindow;
use cocoa::base::{id, nil, YES};
use cocoa::foundation::{NSAutoreleasePool, NSPoint, NSRect, NSSize, NSString};
use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Sel};
use objc::{class, msg_send, sel, sel_impl};
use std::collections::HashMap;
use std::os::raw::c_void;
use std::sync::{Mutex, OnceLock};

/// `OBJC_ASSOCIATION_ASSIGN` (0): borrow the value rather than retaining it.
///
/// The associated value here is a plain integer id, not an object, so no
/// ownership policy applies. Declared locally because the `objc` crate exposes
/// the constant only through the `objc-sys` surface.
const OBJC_ASSOCIATION_ASSIGN: usize = 0;

#[link(name = "objc")]
unsafe extern "C" {
    /// Returns the associated value for `key` on `object`, or null.
    fn objc_getAssociatedObject(object: *mut c_void, key: *const c_void) -> *mut c_void;
    /// Sets the associated value for `key` on `object`.
    fn objc_setAssociatedObject(
        object: *mut c_void,
        key: *const c_void,
        value: *mut c_void,
        policy: usize,
    );
}

/// Returns the `NSString` key used for the widget-id association.
///
/// A canvas view stores its widget id as an **associated object** rather than an
/// ivar: `ClassDecl` needs the ivar layout finalised before `register()`, which
/// does not compose with the `OnceLock` class cache every sibling module uses.
/// AppKit owns the associated object and releases it with the view.
fn association_key() -> id {
    static KEY: OnceLock<usize> = OnceLock::new();
    let ptr = *KEY.get_or_init(|| {
        // SAFETY: `alloc`/`init_str` are the standard NSString constructors and
        // the value is retained by the process for its whole lifetime.
        unsafe { NSString::alloc(nil).init_str("RustWidgetsCanvasWidgetId") as id as usize }
    });
    ptr as id
}

/// The `NSView` subclass backing every self-drawn mount.
fn canvas_view_class() -> *const Class {
    static CLASS: OnceLock<usize> = OnceLock::new();
    (*CLASS.get_or_init(|| {
        let superclass = class!(NSView);
        let mut decl = ClassDecl::new("RustWidgetsCanvasView", superclass)
            .expect("failed to declare RustWidgetsCanvasView");
        // SAFETY: both selectors are NSView API; the function pointers have the
        // ABI AppKit calls them with (`drawRect:` takes an NSRect by value, which
        // `objc`'s `extern "C" fn(&Object, Sel, NSRect)` reproduces).
        unsafe {
            decl.add_method(sel!(drawRect:), draw_rect as extern "C" fn(&Object, Sel, NSRect));
            decl.add_method(
                sel!(isFlipped),
                is_flipped as extern "C" fn(&Object, Sel) -> cocoa::base::BOOL,
            );
            // Input forwarding: the widget owns its own interaction model, so the
            // view's only job is to translate AppKit events and hand them over.
            decl.add_method(sel!(mouseDown:), mouse_down as extern "C" fn(&Object, Sel, id));
            decl.add_method(sel!(mouseUp:), mouse_up as extern "C" fn(&Object, Sel, id));
            decl.add_method(sel!(mouseDragged:), mouse_dragged as extern "C" fn(&Object, Sel, id));
            decl.add_method(sel!(keyDown:), key_down as extern "C" fn(&Object, Sel, id));
            decl.add_method(
                sel!(acceptsFirstResponder),
                accepts_first_responder as extern "C" fn(&Object, Sel) -> cocoa::base::BOOL,
            );
        }
        (decl.register() as *const Class) as usize
    })) as *const Class
}

/// The view uses AppKit's **native** coordinate system (`isFlipped == NO`).
///
/// Reporting `YES` made AppKit place the view's origin at the top of the
/// window's content area, so a canvas at `(0, 0)` received `drawRect:` with a
/// rect of `(0, -28, 900, 648)` — the title-bar offset — and rendered entirely
/// above the visible region. Keeping the default keeps the drawn rect aligned
/// with the view's own bounds; the blit itself is orientation-preserving (see
/// `cg::blit_rgba`), so no flip is needed here either.
extern "C" fn is_flipped(_this: &Object, _cmd: Sel) -> cocoa::base::BOOL {
    cocoa::base::NO
}

/// `-acceptsFirstResponder` returns YES so the canvas can receive key events.
extern "C" fn accepts_first_responder(_this: &Object, _cmd: Sel) -> cocoa::base::BOOL {
    YES
}

/// Converts an AppKit event's window location into canvas-local coordinates.
///
/// `locationInWindow` is bottom-up; the canvas is flipped, so the conversion has
/// to go through `convertPoint:fromView:` rather than subtracting.
unsafe fn event_local_point(view: id, event: id) -> Option<Point> {
    if event == nil {
        return None;
    }
    let window_point: NSPoint = msg_send![event, locationInWindow];
    Some(local_point(view, window_point))
}

/// Reads the modifier bitfield out of an AppKit event using the shared mapping.
extern "C" fn mouse_down(this: &Object, _cmd: Sel, event: id) {
    forward_mouse(this, event, MousePhase::Press);
}

extern "C" fn mouse_up(this: &Object, _cmd: Sel, event: id) {
    forward_mouse(this, event, MousePhase::Release);
}

extern "C" fn mouse_dragged(this: &Object, _cmd: Sel, event: id) {
    forward_mouse(this, event, MousePhase::Drag);
}

/// Which mouse event to synthesise.
#[derive(Clone, Copy)]
enum MousePhase {
    Press,
    Release,
    Drag,
}

/// Translates an AppKit mouse event into a widget [`Event`] and delivers it.
fn forward_mouse(this: &Object, event: id, phase: MousePhase) {
    let outcome = std::panic::catch_unwind(|| {
        // SAFETY: `this` is the live canvas view; `event` is an NSEvent AppKit
        // passed to the selector, valid for the duration of this call.
        unsafe {
            let view = this as *const Object as id;
            let Some(widget_id) = widget_id_of(view) else { return };
            let Some(position) = event_local_point(view, event) else { return };
            let translated = match phase {
                MousePhase::Press => Event::MousePress { pos: position, button: 1 },
                MousePhase::Release => Event::MouseRelease { pos: position, button: 1 },
                MousePhase::Drag => Event::MouseMove { pos: position },
            };
            if !crate::widget::runtime::dispatch_event(widget_id, &translated) {
                log::debug!("[macos] canvas: mouse event dropped, id={widget_id} is not mounted");
                return;
            }
            // The widget's state changed, so ask AppKit to repaint it.
            let _: () = msg_send![view, setNeedsDisplay: YES];
        }
    });
    if outcome.is_err() {
        log::error!("[macos] canvas: panic while forwarding a mouse event");
    }
}

/// Translates a key event into a widget [`Event`] and delivers it.
///
/// Non-printing keys go through as [`Event::KeyPress`]; printable characters are
/// delivered as [`Event::TextInput`] so the widget's text path (including its
/// auto-pairing and IME handling) is the one that runs.
extern "C" fn key_down(this: &Object, _cmd: Sel, event: id) {
    let outcome = std::panic::catch_unwind(|| {
        // SAFETY: `this` is the live canvas view; `event` is the NSEvent AppKit
        // delivered to this selector.
        unsafe {
            let view = this as *const Object as id;
            let Some(widget_id) = widget_id_of(view) else { return };
            let (key, modifiers) = super::types::translate_key_event(event);
            let translated = if let Some(text) = printable_characters(event) {
                Event::TextInput { text }
            } else {
                Event::KeyPress { key, modifiers }
            };
            if !crate::widget::runtime::dispatch_event(widget_id, &translated) {
                log::debug!("[macos] canvas: key event dropped, id={widget_id} is not mounted");
                return;
            }
            let _: () = msg_send![view, setNeedsDisplay: YES];
        }
    });
    if outcome.is_err() {
        log::error!("[macos] canvas: panic while forwarding a key event");
    }
}

/// Returns the printable characters of a key event, when it produced any.
///
/// Control characters are excluded: Enter, Tab and Escape are meaningful as
/// [`Event::KeyPress`] and the widget maps them to editing commands there.
unsafe fn printable_characters(event: id) -> Option<String> {
    let characters: id = msg_send![event, characters];
    if characters == nil {
        return None;
    }
    let utf8: *const std::os::raw::c_char = msg_send![characters, UTF8String];
    if utf8.is_null() {
        return None;
    }
    let c_str = std::ffi::CStr::from_ptr(utf8);
    let text = c_str.to_string_lossy().into_owned();
    let printable: String = text.chars().filter(|ch| !ch.is_control()).collect();
    if printable.is_empty() {
        None
    } else {
        Some(printable)
    }
}

/// `-drawRect:` renders the mounted widget and blits the frame.
///
/// Wrapped in `catch_unwind` because AppKit calls this from its own run loop; a
/// panic unwinding through an Objective-C frame is undefined behaviour.
extern "C" fn draw_rect(this: &Object, _cmd: Sel, rect: NSRect) {
    let outcome = std::panic::catch_unwind(|| {
        // SAFETY: `this` is the live canvas view AppKit is asking to draw.
        unsafe {
            let view = this as *const Object as id;
            let Some(widget_id) = widget_id_of(view) else {
                log::error!("[macos] canvas: drawRect: on a view with no associated widget id");
                return;
            };
            let width = rect.size.width.round().max(1.0) as u32;
            let height = rect.size.height.round().max(1.0) as u32;
            let Some(frame) = crate::widget::runtime::render_frame(
                widget_id,
                Size::new(width, height),
                Color::WHITE,
            ) else {
                // The widget is not mounted (dropped while the view survived) or
                // refused to paint. Say so instead of leaving an unexplained blank.
                log::error!(
                    "[macos] canvas: drawRect: widget id={widget_id} produced no frame \
                     (unmounted, or it does not implement Draw)"
                );
                return;
            };
            // `[NSGraphicsContext currentContext]` is a **class** method. Asking
            // the view for `currentContext` sends an unknown selector and AppKit
            // raises `NSInvalidArgumentException`, which aborts the process
            // because a foreign exception cannot unwind through Rust.
            let context: id = msg_send![class!(NSGraphicsContext), currentContext];
            if context == nil {
                log::error!(
                    "[macos] canvas: drawRect: +[NSGraphicsContext currentContext] was nil"
                );
                return;
            }
            let native: cg::CGContextRef = msg_send![context, CGContext];
            if !cg::blit_rgba(native, width, height, &frame) {
                log::error!("[macos] canvas: drawRect: blit failed for {width}x{height}");
            }
        }
    });
    if outcome.is_err() {
        log::error!("[macos] canvas: panic while drawing a self-drawn widget");
    }
}

/// Reads the widget id associated with a canvas view.
///
/// Uses `objc_getAssociatedObject`, not KVC: `setValue:forKey:` requires the
/// class to be KVC-compliant for that key, and a plain `NSView` subclass is not
/// — it raises `NSUnknownKeyException`, which aborts the process because a
/// foreign exception cannot unwind through Rust.
unsafe fn widget_id_of(view: id) -> Option<ObjectId> {
    let value = objc_getAssociatedObject(view as *mut c_void, association_key() as *const c_void);
    if value.is_null() {
        return None;
    }
    let id = value as ObjectId;
    (id != 0).then_some(id)
}

/// Associates `widget_id` with `view`.
///
/// The id is stored directly as a pointer-sized value (no boxing, no retention),
/// which is safe here because it is a plain integer and the association is
/// cleaned up by AppKit when the view is released.
unsafe fn set_widget_id(view: id, widget_id: ObjectId) {
    objc_setAssociatedObject(
        view as *mut c_void,
        association_key() as *const c_void,
        widget_id as *mut c_void,
        OBJC_ASSOCIATION_ASSIGN,
    );
}

/// Side table from mounted `ObjectId` to its canvas view pointer.
///
/// Needed so resize/unmount can reach the view again; the widget id itself lives
/// on the view as an associated object.
fn mounted_views() -> &'static Mutex<HashMap<ObjectId, usize>> {
    static VIEWS: OnceLock<Mutex<HashMap<ObjectId, usize>>> = OnceLock::new();
    VIEWS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Returns the canvas view for a mounted id, if any.
pub(crate) fn view_for(id: ObjectId) -> Option<id> {
    let table = mounted_views().lock().expect("canvas view lock poisoned");
    table.get(&id).map(|ptr| *ptr as id)
}

impl MacOSPlatform {
    /// Creates the canvas view and adds it to the parent window's content view.
    pub(crate) fn mount_self_drawn_impl(&self, parent: ObjectId, id: ObjectId, rect: Rect) -> bool {
        if !macos_types::is_main_thread() {
            log::error!(
                "[macos] mount_self_drawn: refused off the AppKit main thread (parent={parent}, id={id})"
            );
            return false;
        }
        if !crate::widget::runtime::is_mounted(id) {
            log::error!(
                "[macos] mount_self_drawn: id={id} is not in widget::runtime; \
                 call runtime::register before mounting"
            );
            return false;
        }
        let Some(parent_handle) = self.get_handle(parent) else {
            log::error!("[macos] mount_self_drawn: unknown parent window {parent}");
            return false;
        };
        if !matches!(parent_handle.kind, HandleKind::Window) {
            log::error!(
                "[macos] mount_self_drawn: parent {parent} is {:?}, expected a Window",
                parent_handle.kind
            );
            return false;
        }

        // SAFETY: the main-thread guard above satisfies AppKit's requirement.
        // Every message send targets a selector declared on the canvas class or
        // on NSView/NSWindow. The widget id is carried on the view itself, so it
        // survives independently of the Rust-side side table.
        let view: id = unsafe {
            let pool = NSAutoreleasePool::new(nil);
            let class = canvas_view_class();
            let allocated: id = msg_send![class, alloc];
            let view: id = msg_send![
                allocated,
                initWithFrame: NSRect::new(
                    NSPoint::new(rect.x as f64, rect.y as f64),
                    NSSize::new(rect.width as f64, rect.height as f64),
                )
            ];
            if view == nil {
                log::error!("[macos] mount_self_drawn: canvas view allocation failed");
                pool.drain();
                return false;
            }
            set_widget_id(view, id);
            let content_view = NSWindow::contentView(MacOSPlatform::as_id(parent_handle));
            if content_view == nil {
                log::error!("[macos] mount_self_drawn: window {parent} has no content view");
                pool.drain();
                return false;
            }
            let _: () = msg_send![content_view, addSubview: view];
            let _: () = msg_send![view, setNeedsDisplay: YES];
            pool.drain();

            mounted_views().lock().expect("canvas view lock poisoned").insert(id, view as usize);
            view
        };

        crate::widget::runtime::set_geometry(id, rect);
        self.state.register_widget_with_id(
            id,
            HandleKind::Canvas,
            "CodeEditor",
            rect.x,
            rect.y,
            rect.width,
            rect.height,
        );
        self.handles
            .lock()
            .expect("macos handle lock poisoned")
            .insert(id, CocoaHandle { ptr: view as usize, kind: HandleKind::Canvas });
        log::debug!(
            "[macos] mount_self_drawn: id={id} mounted at ({}, {}, {}, {})",
            rect.x,
            rect.y,
            rect.width,
            rect.height
        );
        true
    }

    /// Resizes a mounted canvas view.
    pub(crate) fn resize_self_drawn_impl(&self, id: ObjectId, rect: Rect) -> bool {
        if !macos_types::is_main_thread() {
            log::error!("[macos] resize_self_drawn: refused off the AppKit main thread");
            return false;
        }
        let Some(view) = view_for(id) else {
            log::error!("[macos] resize_self_drawn: id={id} is not mounted");
            return false;
        };
        // SAFETY: `view` was produced by mount_self_drawn_impl and is only
        // removed by unmount_self_drawn_impl.
        unsafe {
            let _: () = msg_send![
                view,
                setFrame: NSRect::new(
                    NSPoint::new(rect.x as f64, rect.y as f64),
                    NSSize::new(rect.width as f64, rect.height as f64),
                )
            ];
            let _: () = msg_send![view, setNeedsDisplay: YES];
        }
        crate::widget::runtime::set_geometry(id, rect);
        true
    }

    /// Marks the canvas view as needing display.
    ///
    /// Used when a widget is mutated from outside the event loop (a menu action
    /// or tool-bar button), so the change becomes visible without waiting for an
    /// unrelated invalidation.
    pub(crate) fn repaint_self_drawn_impl(&self, id: ObjectId) -> bool {
        let Some(view) = view_for(id) else {
            return false;
        };
        if !macos_types::is_main_thread() {
            log::error!("[macos] repaint_self_drawn: refused off the AppKit main thread");
            return false;
        }
        // SAFETY: `view` came from the side table populated by mount_self_drawn_impl;
        // setNeedsDisplay: is a valid NSView selector.
        unsafe {
            let _: () = msg_send![view, setNeedsDisplay: YES];
        }
        true
    }

    /// Removes a mounted canvas view from its window.
    pub(crate) fn unmount_self_drawn_impl(&self, id: ObjectId) -> bool {
        if !macos_types::is_main_thread() {
            log::error!("[macos] unmount_self_drawn: refused off the AppKit main thread");
            return false;
        }
        let view = mounted_views().lock().expect("canvas view lock poisoned").remove(&id);
        let Some(view) = view else {
            log::error!("[macos] unmount_self_drawn: id={id} is not mounted");
            return false;
        };
        // SAFETY: the side-table entry was removed just above, so the view has
        // no other owner in this module and removing it from its superview is
        // the correct teardown.
        unsafe {
            let _: () = msg_send![view as id, removeFromSuperview];
        }
        self.handles.lock().expect("macos handle lock poisoned").remove(&id);
        self.state.destroy_widget(id);
        true
    }
}

/// Converts a window-space AppKit point into canvas-local coordinates.
///
/// NSWindow reports bottom-up window coordinates; the canvas is flipped, so the
/// conversion has to go through `convertPoint:fromView:` rather than subtracting.
///
/// # Safety
///
/// `view` must be a live canvas view.
pub(crate) unsafe fn local_point(view: id, window_point: NSPoint) -> Point {
    let local: NSPoint = msg_send![view, convertPoint: window_point fromView: nil];
    Point::new(local.x.round() as i32, local.y.round() as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_class_is_declared_once() {
        let first = canvas_view_class();
        let second = canvas_view_class();
        assert_eq!(first, second, "the OnceLock must return a stable class pointer");
        assert!(!first.is_null());
    }

    #[test]
    fn association_key_is_stable() {
        let first = association_key();
        let second = association_key();
        assert_eq!(first, second);
        assert_ne!(first, nil);
    }

    #[test]
    fn view_for_unknown_id_is_none() {
        assert!(view_for(0xABCD).is_none());
    }
}
