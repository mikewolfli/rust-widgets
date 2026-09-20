// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Native surface for **self-drawn** widgets on macOS.
//!
//! Registers an `NSView` subclass (`RustWidgetsCanvasView`) whose `drawRect:`
//! pulls one RGBA frame out of `crate::widget::runtime` and blits it into the
//! view's CoreGraphics context. That is the entire contract: the widget owns its
//! pixels, this view owns the window region they land in.
//!
//! See `docs/plans/custom-paint_mounting.md` for why this is one capability rather
//! than a `create_*` method per self-drawn widget kind.
//!
//! # Feature gate
//!
//! This module needs `crate::widget::runtime` to look widgets up by id and to
//! render frames, and `widget::runtime` does not exist in `mini`/`embedded`
//! builds (see `src/widget/mod.rs`). It is therefore gated on the same pair
//! conditions — a profile that has no widget registry has no way to host a
//! self-drawn surface, and `supports_surfaces()` reports `false` accordingly.
//!
//! Without this gate the `mini` profile fails to build with seven errors about a
//! missing `widget::runtime`, which is a worse outcome than simply not offering
//! the capability.

#![cfg(all(target_os = "macos", feature = "cocoa-legacy", widgets_unstripped))]

use super::cg;
use super::types::{self as macos_types, CocoaHandle, HandleKind, MacOSPlatform};
use crate::core::{Color, ObjectId, Point, Rect, Size};
use crate::event::Event;
use crate::platform::types::MousePhase;
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
        let mut decl = ClassDecl::new("RustWidgetsCanvasView", superclass).expect(
            "the Objective-C runtime refused to declare RustWidgetsCanvasView (a class \
                 with that name is already registered, so the surface cannot be hosted)",
        );
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
            // Hover: AppKit calls these through the tracking area installed in
            // `updateTrackingAreas`, which is how a control learns the pointer left.
            decl.add_method(sel!(mouseExited:), mouse_exited as extern "C" fn(&Object, Sel, id));
            decl.add_method(
                sel!(updateTrackingAreas),
                update_tracking_areas as extern "C" fn(&Object, Sel),
            );
            decl.add_method(sel!(keyDown:), key_down as extern "C" fn(&Object, Sel, id));
            // Touch: AppKit delivers finger contacts through these responder methods
            // rather than the mouse path. Without them the gesture engine never saw a
            // `TouchBegin`, so all eleven recognisers were reachable only from tests.
            //
            // Registered only with the `touch` capability: `Event::Touch*` does not
            // exist without it (see `crate::event::types`), so the handlers below are
            // gated the same way and a build without `touch` has nothing to install.
            #[cfg(feature = "touch")]
            {
                decl.add_method(
                    sel!(touchesBeganWithEvent:),
                    touches_began as extern "C" fn(&Object, Sel, id),
                );
                decl.add_method(
                    sel!(touchesMovedWithEvent:),
                    touches_moved as extern "C" fn(&Object, Sel, id),
                );
                decl.add_method(
                    sel!(touchesEndedWithEvent:),
                    touches_ended as extern "C" fn(&Object, Sel, id),
                );
                decl.add_method(
                    sel!(touchesCancelledWithEvent:),
                    touches_cancelled as extern "C" fn(&Object, Sel, id),
                );
            }
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

/// `-mouseExited:` clears the hover target.
///
/// AppKit delivers this through a tracking area (installed in `updateTrackingAreas`),
/// and it is the one case the coordinate-based hover transition cannot observe: the
/// pointer is outside the view, so a previously hovered control would stay
/// highlighted. Repainting is requested so the cleared state becomes visible.
extern "C" fn mouse_exited(this: &Object, _cmd: Sel, _event: id) {
    let outcome = std::panic::catch_unwind(|| {
        // SAFETY: `this` is the live canvas view.
        unsafe {
            let view = this as *const Object as id;
            crate::widget::runtime::clear_hover(Point::new(0, 0));
            let _: () = msg_send![view, setNeedsDisplay: YES];
        }
    });
    if outcome.is_err() {
        log::error!("[macos] canvas: panic while handling mouseExited:");
    }
}

/// Installs the tracking area that makes `-mouseExited:` fire.
///
/// AppKit rebuilds tracking areas whenever the view's geometry changes, so the areas
/// are replaced rather than appended — appending on every call would leak an area per
/// resize. `NSTrackingActiveInKeyWindow` is used so hover follows the same rule as
/// clicks (an inactive window does not highlight controls under the pointer).
unsafe fn install_tracking_area(view: id) {
    let existing: id = msg_send![view, trackingAreas];
    if existing != nil {
        let count: usize = msg_send![existing, count];
        for _ in 0..count {
            let area: id = msg_send![existing, objectAtIndex: 0usize];
            if area != nil {
                let _: () = msg_send![view, removeTrackingArea: area];
            }
        }
    }
    let bounds: NSRect = msg_send![view, bounds];
    let area: id = msg_send![class!(NSTrackingArea), alloc];
    let area: id = msg_send![area,
        initWithRect: bounds
        options: TRACKING_ACTIVE_IN_KEY_WINDOW | TRACKING_MOUSE_ENTERED | TRACKING_MOUSE_EXITED | TRACKING_IN_VISIBLE_RECT
        owner: view
        userInfo: std::ptr::null::<Object>()];
    if area != nil {
        let _: () = msg_send![view, addTrackingArea: area];
        let _: () = msg_send![area, release];
    }
}

/// `NSTrackingArea` option: report enter/exit while the window is key.
const TRACKING_ACTIVE_IN_KEY_WINDOW: usize = 0x0040;
/// `NSTrackingArea` option: send `-mouseEntered:`.
const TRACKING_MOUSE_ENTERED: usize = 0x0001;
/// `NSTrackingArea` option: send `-mouseExited:`.
const TRACKING_MOUSE_EXITED: usize = 0x0002;
/// `NSTrackingArea` option: track within the view's visible rect.
const TRACKING_IN_VISIBLE_RECT: usize = 0x0080;

/// `-updateTrackingAreas` reinstalls the tracking area after a geometry change.
extern "C" fn update_tracking_areas(this: &Object, _cmd: Sel) {
    let outcome = std::panic::catch_unwind(|| {
        // SAFETY: `this` is the live canvas view.
        unsafe {
            let view = this as *const Object as id;
            install_tracking_area(view);
        }
    });
    if outcome.is_err() {
        log::error!("[macos] canvas: panic while updating tracking areas");
    }
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

/// Returns the canvas view's origin within its parent.
///
/// AppKit reports pointer positions in view-local space, but widget geometry in this
/// library is absolute, so the origin is added back before hit-testing. `frame` is
/// read live rather than cached because AppKit is free to move the view (auto layout,
/// a resize) without telling this module.
unsafe fn view_origin(view: id) -> Point {
    let frame: NSRect = msg_send![view, frame];
    Point::new(frame.origin.x as i32, frame.origin.y as i32)
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

/// Which touch responder method AppKit called.
#[cfg(feature = "touch")]
#[derive(Clone, Copy)]
enum TouchPhase {
    Began,
    Moved,
    Ended,
}

/// `-touchesBeganWithEvent:` — one or more fingers landed on the canvas.
#[cfg(feature = "touch")]
extern "C" fn touches_began(this: &Object, _cmd: Sel, event: id) {
    forward_touches(this, event, TouchPhase::Began);
}

/// `-touchesMovedWithEvent:` — a tracked finger moved.
#[cfg(feature = "touch")]
extern "C" fn touches_moved(this: &Object, _cmd: Sel, event: id) {
    forward_touches(this, event, TouchPhase::Moved);
}

/// `-touchesEndedWithEvent:` — a tracked finger lifted.
#[cfg(feature = "touch")]
extern "C" fn touches_ended(this: &Object, _cmd: Sel, event: id) {
    forward_touches(this, event, TouchPhase::Ended);
}

/// `-touchesCancelledWithEvent:` — AppKit withdrew the contact.
///
/// Reported as an end, because the recognisers need a terminator for every begin: a
/// `TouchBegin` with no matching `TouchEnd` leaves `PinchGesture` holding a phantom
/// finger forever, and the next real pinch then measures against it.
#[cfg(feature = "touch")]
extern "C" fn touches_cancelled(this: &Object, _cmd: Sel, event: id) {
    forward_touches(this, event, TouchPhase::Ended);
}

/// Translates AppKit touches into widget touch events and delivers them.
///
/// # Why the gesture engine needs this
///
/// `is_touch()` accepts only `Touch*` and gesture variants, so with no backend emitting
/// `TouchBegin` the engine's `process` was never reached by real input and all eleven
/// recognisers were exercised only by unit tests. AppKit reports finger contacts here
/// rather than through `mouseDown:`.
///
/// # Coordinates
///
/// `NSTouch` reports a *normalized* position in `0.0..=1.0` relative to the view, which
/// is resolution-independent and unaffected by the view's backing scale. It is scaled by
/// the view's bounds and then offset by the canvas origin, so a touch and a click at the
/// same place resolve to the same absolute point.
///
/// # Identity
///
/// `NSTouch.identity` is a stable per-contact object, which is exactly what `TouchId`
/// must be for the recognisers to follow a finger across move and end. Its pointer is
/// used as the id: AppKit guarantees it identifies the contact for its whole lifetime,
/// and the address is only ever compared for equality.
///
/// # Feature gate
///
/// Compiled only with `touch`, together with the four responder methods that call it:
/// `Event::TouchBegin`/`TouchMove`/`TouchEnd` are themselves gated in
/// `crate::event::types`, so there is nothing to translate without the capability.
#[cfg(feature = "touch")]
fn forward_touches(this: &Object, event: id, phase: TouchPhase) {
    let outcome = std::panic::catch_unwind(|| {
        // SAFETY: `this` is the live canvas view; `event` is the NSEvent AppKit passed
        // to the selector, valid for the duration of this call.
        unsafe {
            let view = this as *const Object as id;
            if event == nil {
                return;
            }
            let Some(widget_id) = widget_id_of(view) else { return };
            let origin = view_origin(view);
            let bounds: NSRect = msg_send![view, bounds];

            // `touchesBeganWithEvent:` carries the new contacts in the event's touch
            // set; the moved/ended variants report the same contacts, so all three read
            // the event's set rather than a per-phase selector.
            let all_touches: id = msg_send![event, touchesMatchingPhase: 0u64 inView: view];
            if all_touches == nil {
                return;
            }
            let count: u64 = msg_send![all_touches, count];
            let mut delivered = false;
            for index in 0..count {
                let touch: id = msg_send![all_touches, objectAtIndex: index];
                if touch == nil {
                    continue;
                }
                let normalized: NSPoint = msg_send![touch, normalizedPosition];
                let local_x = (normalized.x * bounds.size.width).round() as i32;
                let local_y = (normalized.y * bounds.size.height).round() as i32;
                // The touch id is the contact's identity pointer. `TouchId` is a u64,
                // and the recognisers only compare it, never dereference it.
                let identity: id = msg_send![touch, identity];
                let touch_id = identity as u64;
                let position = Point::new(origin.x + local_x, origin.y + local_y);
                let translated = match phase {
                    TouchPhase::Began => Event::TouchBegin { pos: position, touch_id },
                    TouchPhase::Moved => Event::TouchMove { pos: position, touch_id },
                    TouchPhase::Ended => Event::TouchEnd { pos: position, touch_id },
                };
                if crate::platform::platform_facts().route_pointer_event(
                    widget_id,
                    &translated,
                    position,
                ) {
                    delivered = true;
                }
            }
            if delivered {
                let _: () = msg_send![view, setNeedsDisplay: YES];
            }
        }
    });
    if outcome.is_err() {
        log::error!("[macos] canvas: panic while forwarding a touch event");
    }
}
/// Translates an AppKit mouse event into a widget [`Event`] and delivers it.
///
/// Routing goes through the platform's hit test so a click on a widget nested inside
/// the mounted one reaches that widget, rather than always reaching the surface owner.
fn forward_mouse(this: &Object, event: id, phase: MousePhase) {
    let outcome = std::panic::catch_unwind(|| {
        // SAFETY: `this` is the live canvas view; `event` is an NSEvent AppKit
        // passed to the selector, valid for the duration of this call.
        unsafe {
            let view = this as *const Object as id;
            let Some(widget_id) = widget_id_of(view) else { return };
            let Some(local) = event_local_point(view, event) else { return };
            let origin = view_origin(view);
            let position = Point::new(origin.x + local.x, origin.y + local.y);
            let translated = match phase {
                MousePhase::Press => Event::MousePress { pos: position, button: 1 },
                MousePhase::Release => Event::MouseRelease { pos: position, button: 1 },
                MousePhase::Drag => Event::MouseMove { pos: position },
            };
            let delivered = crate::platform::platform_facts().route_pointer_event(
                widget_id,
                &translated,
                position,
            );
            if !delivered {
                log::debug!("[macos] canvas: mouse event dropped, id={widget_id} is not mounted");
                return;
            }
            if matches!(phase, MousePhase::Press) {
                // A click can move focus to a nested control; take the keyboard so
                // subsequent keys are delivered to this view.
                let window: id = msg_send![view, window];
                if window != nil {
                    let _: () = msg_send![window, makeFirstResponder: view];
                }
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
///
/// Tab is consumed here to move focus: it is not a printable character, so forwarding
/// it to the widget would be a no-op and the user could never leave the first control.
extern "C" fn key_down(this: &Object, _cmd: Sel, event: id) {
    let outcome = std::panic::catch_unwind(|| {
        // SAFETY: `this` is the live canvas view; `event` is the NSEvent AppKit
        // delivered to this selector.
        unsafe {
            let view = this as *const Object as id;
            let Some(widget_id) = widget_id_of(view) else { return };
            let (key, modifiers) = super::types::translate_key_event(event);
            // AppKit reports Tab as keycode 48 with no printable characters. The
            // widget-layer modifier bit for Shift is 1 (see `Modifiers::from_event_bits`).
            const WIDGET_SHIFT: u32 = 1;
            if key == KEY_TAB {
                crate::widget::runtime::focus_next(modifiers & WIDGET_SHIFT == 0);
                let _: () = msg_send![view, setNeedsDisplay: YES];
                return;
            }
            let translated = if let Some(text) = printable_characters(event) {
                Event::TextInput { text }
            } else {
                Event::KeyPress { key, modifiers }
            };
            // Keys follow focus: with nothing focused, the surface owner keeps them.
            let target = crate::widget::runtime::focused_widget().unwrap_or(widget_id);
            if !crate::widget::runtime::dispatch_event(target, &translated) {
                log::debug!("[macos] canvas: key event dropped, id={target} is not mounted");
                return;
            }
            let _: () = msg_send![view, setNeedsDisplay: YES];
        }
    });
    if outcome.is_err() {
        log::error!("[macos] canvas: panic while forwarding a key event");
    }
}

/// AppKit virtual key code for Tab (not a character code).
const KEY_TAB: u32 = 48;

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
///
/// The `_rect` parameter is the dirty rectangle AppKit passes, and the body
/// deliberately ignores it in favour of `bounds` (see the note below); its name
/// carries the underscore so the intent is visible in the signature rather than
/// left as a compiler warning.
extern "C" fn draw_rect(this: &Object, _cmd: Sel, _rect: NSRect) {
    let outcome = std::panic::catch_unwind(|| {
        // SAFETY: `this` is the live canvas view AppKit is asking to draw.
        unsafe {
            let view = this as *const Object as id;
            let Some(widget_id) = widget_id_of(view) else {
                log::error!("[macos] canvas: drawRect: on a view with no associated widget id");
                return;
            };
            // The size comes from `bounds`, not from `rect`: AppKit passes the *dirty*
            // rectangle to `drawRect:`, which for a partial invalidation is smaller
            // than the view. Sizing the frame from it would render the view into a
            // buffer the size of the damaged band, and the image would then be drawn
            // stretched into the wrong place.
            //
            // `render_frame_cached` rather than `render_frame`: it carries the previous
            // frame forward and repaints only the damage, so a widget in
            // `RepaintMode::Dirty` does not re-rasterise the whole view for a small
            // `setNeedsDisplayInRect:`. The returned frame is complete because
            // `CGContextDrawImage` presents a whole image.
            let bounds: NSRect = msg_send![view, bounds];
            let width = bounds.size.width.round().max(1.0) as u32;
            let height = bounds.size.height.round().max(1.0) as u32;
            let Some(frame) = crate::widget::runtime::render_frame_cached(
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
    let table = crate::compat::lock(mounted_views());
    table.get(&id).map(|ptr| *ptr as id)
}

impl MacOSPlatform {
    /// Creates the canvas view and adds it to the parent window's content view.
    pub(crate) fn mount_surface_impl(&self, parent: ObjectId, id: ObjectId, rect: Rect) -> bool {
        if !macos_types::is_main_thread() {
            log::error!(
                "[macos] mount_surface: refused off the AppKit main thread (parent={parent}, id={id})"
            );
            return false;
        }
        if !crate::widget::runtime::is_mounted(id) {
            log::error!(
                "[macos] mount_surface: id={id} is not in widget::runtime; \
                 call runtime::register before mounting"
            );
            return false;
        }
        let Some(parent_handle) = self.get_handle(parent) else {
            log::error!("[macos] mount_surface: unknown parent window {parent}");
            return false;
        };
        if !matches!(parent_handle.kind, HandleKind::Window) {
            log::error!(
                "[macos] mount_surface: parent {parent} is {:?}, expected a Window",
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
                log::error!("[macos] mount_surface: canvas view allocation failed");
                pool.drain();
                return false;
            }
            set_widget_id(view, id);
            let content_view = NSWindow::contentView(MacOSPlatform::as_id(parent_handle));
            if content_view == nil {
                log::error!("[macos] mount_surface: window {parent} has no content view");
                pool.drain();
                return false;
            }
            let _: () = msg_send![content_view, addSubview: view];
            let _: () = msg_send![view, setNeedsDisplay: YES];
            pool.drain();

            crate::compat::lock(mounted_views()).insert(id, view as usize);
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
            "[macos] mount_surface: id={id} mounted at ({}, {}, {}, {})",
            rect.x,
            rect.y,
            rect.width,
            rect.height
        );
        true
    }

    /// Resizes a mounted canvas view.
    pub(crate) fn resize_surface_impl(&self, id: ObjectId, rect: Rect) -> bool {
        if !macos_types::is_main_thread() {
            log::error!("[macos] resize_surface: refused off the AppKit main thread");
            return false;
        }
        let Some(view) = view_for(id) else {
            log::error!("[macos] resize_surface: id={id} is not mounted");
            return false;
        };
        // SAFETY: `view` was produced by mount_surface_impl and is only
        // removed by unmount_surface_impl.
        unsafe {
            // `setFrame:display:` rather than a bare `setFrame:`. A view has to be
            // told to redraw: AppKit does not schedule a display pass for a frame
            // change on its own, so a bare `setFrame:` leaves the old pixels on
            // screen until some unrelated invalidation happens to arrive. That
            // asymmetry is BLUE14 F-5, and it is why this gate checks the two-
            // argument selector by name.
            let _: () = msg_send![
                view,
                setFrame: NSRect::new(
                    NSPoint::new(rect.x as f64, rect.y as f64),
                    NSSize::new(rect.width as f64, rect.height as f64),
                )
                display: YES
            ];
        }
        crate::widget::runtime::set_geometry(id, rect);
        true
    }

    /// Marks the canvas view as needing display.
    ///
    /// Used when a widget is mutated from outside the event loop (a menu action
    /// or tool-bar button), so the change becomes visible without waiting for an
    /// unrelated invalidation.
    pub(crate) fn invalidate_surface_impl(&self, id: ObjectId) -> bool {
        let Some(view) = view_for(id) else {
            return false;
        };
        if !macos_types::is_main_thread() {
            log::error!("[macos] invalidate_surface: refused off the AppKit main thread");
            return false;
        }
        // SAFETY: `view` came from the side table populated by mount_surface_impl;
        // setNeedsDisplay: is a valid NSView selector.
        unsafe {
            let _: () = msg_send![view, setNeedsDisplay: YES];
        }
        true
    }

    /// Marks one rectangle of the canvas view as needing display.
    ///
    /// # Why AppKit can narrow this
    ///
    /// `setNeedsDisplayInRect:` exists on `NSView` and takes a rectangle in the view's
    /// own coordinate space, which for a non-flipped view has its origin at the
    /// bottom-left. The caller's rectangle is in the library's top-down space, so the
    /// y coordinate is mirrored here — passing it through unchanged would invalidate a
    /// band the same distance from the *other* edge.
    ///
    /// Returns `false` off the main thread, for an unknown id, or for an empty
    /// rectangle, so the caller falls back to invalidating the whole view.
    pub(crate) fn invalidate_surface_rect_impl(&self, id: ObjectId, rect: Rect) -> bool {
        let Some(view) = view_for(id) else {
            return false;
        };
        if !macos_types::is_main_thread() {
            log::error!("[macos] invalidate_surface_rect: refused off the AppKit main thread");
            return false;
        }
        if rect.width == 0 || rect.height == 0 {
            return false;
        }

        // SAFETY: `view` came from the side table populated by mount_surface_impl.
        // `bounds` and `isFlipped` are valid NSView getters; the former returns an
        // `NSRect`, which cocoa's binding represents as the same four CGFloats AppKit
        // uses.
        let (bounds_height, view_is_flipped) = unsafe {
            let bounds: NSRect = msg_send![view, bounds];
            let flipped: cocoa::base::BOOL = msg_send![view, isFlipped];
            (bounds.size.height, flipped != cocoa::base::NO)
        };

        let y = if view_is_flipped {
            rect.y as f64
        } else {
            bounds_height - (rect.y as f64 + rect.height as f64)
        };

        let target = NSRect::new(
            NSPoint::new(rect.x as f64, y),
            NSSize::new(rect.width as f64, rect.height as f64),
        );
        // SAFETY: `target` is a valid NSRect for the duration of the call, and
        // setNeedsDisplayInRect: is a valid NSView selector taking one NSRect by value.
        unsafe {
            let _: () = msg_send![view, setNeedsDisplayInRect: target];
        }
        true
    }

    /// Removes a mounted canvas view from its window.
    pub(crate) fn unmount_surface_impl(&self, id: ObjectId) -> bool {
        if !macos_types::is_main_thread() {
            log::error!("[macos] unmount_surface: refused off the AppKit main thread");
            return false;
        }
        let view = crate::compat::lock(mounted_views()).remove(&id);
        let Some(view) = view else {
            log::error!("[macos] unmount_surface: id={id} is not mounted");
            return false;
        };
        // SAFETY: the side-table entry was removed just above, so the view has
        // no other owner in this module and removing it from its superview is
        // the correct teardown.
        unsafe {
            let _: () = msg_send![view as id, removeFromSuperview];
        }
        crate::compat::lock(&self.handles).remove(&id);
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

// ═══════════════════════════════════════════════════════════════════════════════
// Window resize reporting
// ═══════════════════════════════════════════════════════════════════════════════

/// Association key holding the logical widget id on a window.
fn window_association_key() -> id {
    static KEY: OnceLock<usize> = OnceLock::new();
    let ptr = *KEY.get_or_init(|| {
        // SAFETY: standard NSString construction; the value lives for the process.
        unsafe { NSString::alloc(nil).init_str("RustWidgetsWindowWidgetId") as id as usize }
    });
    ptr as id
}

/// The `NSWindowDelegate` subclass that reports resizes.
///
/// # Why a delegate rather than a notification observer
///
/// A window resized by the user must re-run the host's layout. AppKit reports this
/// through `windowDidResize:`, and the delegate is the documented receiver. The window
/// id travels as an associated object because `ClassDecl` needs its ivars finalised
/// before `register()` — the same constraint the canvas view class documents.
fn window_delegate_class() -> *const Class {
    static CLASS: OnceLock<usize> = OnceLock::new();
    (*CLASS.get_or_init(|| {
        let superclass = class!(NSObject);
        let mut decl = ClassDecl::new("RustWidgetsWindowDelegate", superclass)
            .expect("the Objective-C runtime refused to declare RustWidgetsWindowDelegate");
        // SAFETY: `windowDidResize:` is `NSWindowDelegate` API and takes the notifying
        // `NSWindow *`; the function pointer reproduces that ABI. The registered
        // implementation must have the safe `extern "C"` ABI `objc::declare` accepts,
        // so the `unsafe fn` body is wrapped by `window_did_resize_impl`.
        unsafe {
            decl.add_method(
                sel!(windowDidResize:),
                window_did_resize_impl as extern "C" fn(&Object, Sel, id),
            );
        }
        (decl.register() as *const Class) as usize
    })) as *const Class
}

/// Safe-ABI trampoline registered as the `windowDidResize:` implementation.
///
/// `objc::declare::ClassDecl::add_method` only accepts the **safe**
/// `extern "C" fn(&Object, Sel, ..)` form, and Rust forbids casting an
/// `unsafe extern "C" fn` item to a safe one (E0605). The wrapper therefore
/// exists purely to give the runtime a safe pointer; every unsafe operation
/// stays inside `window_did_resize`.
extern "C" fn window_did_resize_impl(this: &Object, cmd: Sel, notification: id) {
    // SAFETY: AppKit invokes this only through the `windowDidResize:` selector,
    // which guarantees `this` is the delegate object and `notification` is the
    // `NSNotification` for the window that resized.
    unsafe { window_did_resize(this, cmd, notification) }
}

/// `windowDidResize:` — records the new client size and queues a `Resized` trigger.
unsafe fn window_did_resize(this: &Object, _cmd: Sel, notification: id) {
    let widget_id = associated_widget_id(this, window_association_key());
    if widget_id == 0 {
        return;
    }
    // `NSNotification.object` is the window that resized.
    let window: id = msg_send![notification, object];
    if window == nil {
        return;
    }
    // The content rect excludes the title bar, which is the area the layout may use.
    // `contentView` reports it directly and is simpler than converting a frame rect.
    let content: id = msg_send![window, contentView];
    if content == nil {
        return;
    }
    let bounds: NSRect = msg_send![content, bounds];
    let width = bounds.size.width.max(0.0) as u32;
    let height = bounds.size.height.max(0.0) as u32;
    if width == 0 || height == 0 {
        return;
    }
    // The trigger is queued on the process-wide resize queue the host polls;
    // routing it through the platform singleton added nothing but a downcast
    // whose result was discarded.
    crate::queue_resize_trigger(widget_id, width, height);
}

/// Installs the resize delegate on `window` and tags it with `widget_id`.
///
/// # Safety
///
/// `window` must be a live `NSWindow` on the AppKit main thread.
pub(crate) unsafe fn install_resize_delegate(window: id, widget_id: ObjectId) {
    // The delegate is retained by the window, so a process-lifetime allocation here is
    // correct: AppKit releases it with the window.
    let delegate: id = msg_send![window_delegate_class(), new];
    objc_setAssociatedObject(
        delegate as *mut c_void,
        window_association_key() as *const c_void,
        widget_id as *mut c_void,
        OBJC_ASSOCIATION_ASSIGN,
    );
    let _: () = msg_send![window, setDelegate: delegate];
}

/// Reads the logical widget id associated with `object`, or 0 when absent.
///
/// # Safety
///
/// `object` must be a live Objective-C object.
unsafe fn associated_widget_id(object: &Object, key: id) -> ObjectId {
    let value =
        objc_getAssociatedObject(object as *const Object as *mut c_void, key as *const c_void);
    value as ObjectId
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

// probe
