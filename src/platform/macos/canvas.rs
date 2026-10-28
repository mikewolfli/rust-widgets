// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Native surface for **self-drawn** widgets on macOS.
//!
//! Registers an `NSView` subclass (`RustWidgetsCanvasView`) whose `drawRect:`
//! pulls one RGBA frame out of `crate::widget::runtime` and blits it into the
//! view's CoreGraphics context. That is the entire contract: the widget owns its
//! pixels, this view owns the window region they land in.
//!
//! See `docs/plans/custom_widget_mounting.md` for why this is one capability rather
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
use cocoa::base::{id, nil, NO, YES};
use cocoa::foundation::{
    NSArray, NSAutoreleasePool, NSNotFound, NSPoint, NSRange, NSRect, NSSize, NSString, NSUInteger,
};
use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Protocol, Sel};
use objc::{class, msg_send, sel, sel_impl};
use std::collections::HashMap;
use std::os::raw::c_void;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// `OBJC_ASSOCIATION_ASSIGN` (0): borrow the value rather than retaining it.
///
/// The associated value here is a plain integer id, not an object, so no
/// ownership policy applies. Declared locally because the `objc` crate exposes
/// the constant only through the `objc-sys` surface.
const OBJC_ASSOCIATION_ASSIGN: usize = 0;

/// `OBJC_ASSOCIATION_RETAIN_NONATOMIC` (1): retain the associated object, without atomicity.
///
/// Used to give the per-window resize delegate an owner that lives as long as its window
/// (ENG-C5); AppKit only touches it on the main thread, so nonatomic is sufficient.
const OBJC_ASSOCIATION_RETAIN_NONATOMIC: usize = 1;

/// `NSRange` in the spelling `ClassDecl::add_method` accepts.
///
/// `cocoa`'s `NSRange` does not implement `objc::Encode`, and both the type (another crate) and
/// the trait (another crate) are foreign, so the impl cannot live on either. This newtype is
/// layout-identical to `NSRange` (`#[repr(transparent)]`), which is what AppKit passes to every
/// `NSTextInputClient` range parameter, and its `Encode` supplies the runtime's own
/// `{_NSRange=QQ}` encoding.
#[repr(transparent)]
#[derive(Clone, Copy)]
struct EncodedRange(NSRange);

// SAFETY: `EncodedRange` is `#[repr(transparent)]` over `NSRange`, whose two `NSUInteger`
// fields are the only members; the encoding describes exactly that struct.
unsafe impl objc::Encode for EncodedRange {
    fn encode() -> objc::Encoding {
        let unit = <NSUInteger as objc::Encode>::encode();
        let encoding = format!("{{_NSRange={}{}}}", unit.as_str(), unit.as_str());
        // SAFETY: the string is a well-formed Objective-C type encoding owned by this call.
        unsafe { objc::Encoding::from_str(&encoding) }
    }
}

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

thread_local! {
    /// True while `keyDown:` is inside `interpretKeyEvents:`.
    ///
    /// AppKit calls back into the `NSTextInputClient` methods synchronously from that call. The
    /// flag is the same one the objc2 canvas keeps, and it exists for the same reason: a
    /// callback that consumed the key must stop `key_down` from also forwarding it as text.
    static IME_INTERPRETING: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
    /// Set by an `NSTextInputClient` callback that consumed the key.
    static IME_HANDLED: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
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
            // `NSTextInputClient` is how AppKit hands composition and committed text to a view
            // that is not an `NSTextView`. Declaring the protocol is what makes the view an
            // input-client candidate; the methods below are the callbacks themselves. The
            // protocol is registered by AppKit, so a runtime without it (a stripped test host)
            // simply skips the declaration rather than failing class registration.
            if let Some(protocol) = Protocol::get("NSTextInputClient") {
                decl.add_protocol(protocol);
            }
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
            // Plain movement, which is what drives hover. AppKit sends this only because the
            // tracking area above asks for `NSTrackingMouseMoved`; see that constant for why a
            // surface without it has no hover at all. Its absence was a real gap, not a style
            // choice: the other two backends both deliver a button-less move.
            decl.add_method(sel!(mouseMoved:), mouse_moved as extern "C" fn(&Object, Sel, id));
            // Secondary button. AppKit routes the right button through its own responder
            // methods, so without these a context menu (or any right-drag) never reached a
            // widget — the sibling `windows::canvas` has had the equivalent
            // `WM_RBUTTONDOWN`/`WM_RBUTTONUP` arms all along, and `MousePhase` carried the
            // `Secondary*` variants for exactly this purpose (rule #54: one enum, two
            // backends, and only one of them consuming it is a half-wired feature).
            decl.add_method(
                sel!(rightMouseDown:),
                right_mouse_down as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(sel!(rightMouseUp:), right_mouse_up as extern "C" fn(&Object, Sel, id));
            // Hover: AppKit calls both through the tracking area installed in
            // `updateTrackingAreas`, which is how a control learns the pointer entered or
            // left. **Both** selectors have to be registered: the area requests
            // `NSTrackingMouseEntered` as well, and an unimplemented responder method
            // raises rather than being ignored (see `mouse_entered`).
            decl.add_method(sel!(mouseEntered:), mouse_entered as extern "C" fn(&Object, Sel, id));
            decl.add_method(sel!(mouseExited:), mouse_exited as extern "C" fn(&Object, Sel, id));
            decl.add_method(
                sel!(updateTrackingAreas),
                update_tracking_areas as extern "C" fn(&Object, Sel),
            );
            decl.add_method(sel!(keyDown:), key_down as extern "C" fn(&Object, Sel, id));
            decl.add_method(sel!(keyUp:), key_up as extern "C" fn(&Object, Sel, id));
            // IME: the `NSTextInputClient` callbacks. AppKit's input method calls these from
            // `interpretKeyEvents:` (which `key_down` invokes first) and when the system input
            // window changes. Without them the backend declared `ime: true` in
            // `platform_impl.rs` while no composition could ever reach a widget (P-202).
            decl.add_method(
                sel!(hasMarkedText),
                has_marked_text as extern "C" fn(&Object, Sel) -> cocoa::base::BOOL,
            );
            decl.add_method(
                sel!(markedRange),
                marked_range as extern "C" fn(&Object, Sel) -> EncodedRange,
            );
            decl.add_method(
                sel!(selectedRange),
                selected_range as extern "C" fn(&Object, Sel) -> EncodedRange,
            );
            decl.add_method(
                sel!(validAttributesForMarkedText),
                valid_attributes_for_marked_text as extern "C" fn(&Object, Sel) -> id,
            );
            decl.add_method(
                sel!(attributedSubstringForProposedRange:actualRange:),
                attributed_substring
                    as extern "C" fn(&Object, Sel, EncodedRange, *mut c_void) -> id,
            );
            decl.add_method(
                sel!(setMarkedText:selectedRange:replacementRange:),
                set_marked_text as extern "C" fn(&Object, Sel, id, EncodedRange, EncodedRange),
            );
            decl.add_method(
                sel!(insertText:replacementRange:),
                insert_text as extern "C" fn(&Object, Sel, id, EncodedRange),
            );
            decl.add_method(sel!(unmarkText), unmark_text as extern "C" fn(&Object, Sel));
            decl.add_method(
                sel!(doCommandBySelector:),
                do_command_by_selector as extern "C" fn(&Object, Sel, Sel),
            );
            decl.add_method(
                sel!(firstRectForCharacterRange:actualRange:),
                first_rect_for_character_range
                    as extern "C" fn(&Object, Sel, EncodedRange, *mut c_void) -> NSRect,
            );
            decl.add_method(
                sel!(characterIndexForPoint:),
                character_index_for_point as extern "C" fn(&Object, Sel, NSPoint) -> NSUInteger,
            );
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

/// Tells the library that AppKit is about to redraw one of its own views for `id`.
///
/// # Why the platform's own repaint is announced
///
/// A canvas view owns its pixels, so asking AppKit (`setNeedsDisplay:`) is the platform's
/// job and not something to route back through the library (BLUE24 §7.1). But the library
/// still has to **know**, or three things stop being true at once: the frame's damage account
/// would describe repaints the platform made without it, the still-frame guarantee would be
/// unprovable, and a mixed window would submit one interaction twice (BLUE24 §7.2).
///
/// This is the macOS spelling of the same announcement `platform::linux::canvas`
/// makes; both reach the one ledger, so a frame's account is the same whichever backend
/// produced it.
fn note_native_redraw(id: ObjectId) {
    crate::notify_native_redraw(id, None);
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
            // The hover that just cleared belonged to this view's widget, so the redraw is
            // this surface's; announcing it keeps the frame's account complete (§7.2).
            if let Some(widget_id) = widget_id_of(view) {
                note_native_redraw(widget_id);
            }
        }
    });
    if outcome.is_err() {
        log::error!("[macos] canvas: panic while handling mouseExited:");
    }
}

/// `-mouseEntered:` re-establishes the hover target.
///
/// # Why an enter handler is required, not merely symmetric
///
/// The tracking area installed by [`install_tracking_area`] requests both
/// `NSTrackingMouseEntered` **and** `NSTrackingMouseExited`. AppKit therefore sends
/// `-mouseEntered:` to the owner whenever the pointer crosses in, and a view that does
/// not implement the selector does not merely miss the event: AppKit's default
/// `NSView` implementation of the tracking-area protocol calls
/// `doesNotRecognizeSelector:`, which raises `NSInvalidArgumentException`. A foreign
/// exception cannot unwind through Rust, so the process **aborts**.
///
/// It is also a real capability gap: `clear_hover` runs on the way out and nothing ever
/// re-enters, so a control hovered by moving *into* the surface (rather than by moving
/// within it) stayed unhighlighted. `-mouseEntered:` carries a coordinate in window
/// space, which is converted the same way a click is and handed to the router, so the
/// control under the pointer is entered exactly as a move would enter it.
extern "C" fn mouse_entered(this: &Object, _cmd: Sel, event: id) {
    let outcome = std::panic::catch_unwind(|| {
        // SAFETY: `this` is the live canvas view; `event` is the NSEvent AppKit passed to
        // the tracking-area message, valid for the duration of this call.
        unsafe {
            let view = this as *const Object as id;
            let Some(widget_id) = widget_id_of(view) else { return };
            let Some(local) = event_local_point(view, event) else { return };
            let origin = view_origin(view);
            let position = Point::new(origin.x + local.x, origin.y + local.y);
            // Routed rather than applied directly: the router is what owns the enter/leave
            // pair and updates the previous-target cell, so an enter delivered here and a
            // move delivered beside it cannot produce two enters for one control.
            crate::platform::platform_facts().route_pointer_event(
                widget_id,
                &Event::MouseMove { pos: position },
                position,
            );
            let _: () = msg_send![view, setNeedsDisplay: YES];
            note_native_redraw(widget_id);
        }
    });
    if outcome.is_err() {
        log::error!("[macos] canvas: panic while handling mouseEntered:");
    }
}

/// Installs the tracking area that makes `-mouseExited:` fire.
///
/// AppKit rebuilds tracking areas whenever the view's geometry changes, so the areas
/// are replaced rather than appended — appending on every call would leak an area per
/// resize. `NSTrackingActiveInKeyWindow` is used so hover follows the same rule as
/// clicks (an inactive window does not highlight controls under the pointer).
///
/// # Safety
/// `view` must be a live `NSView` and the call must run on the main thread.
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
        options: TRACKING_ACTIVE_IN_KEY_WINDOW | TRACKING_MOUSE_ENTERED | TRACKING_MOUSE_EXITED | TRACKING_MOUSE_MOVED | TRACKING_IN_VISIBLE_RECT
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
/// `NSTrackingArea` option: send `-mouseMoved:` while the pointer is inside.
///
/// # Why hover needs this one specifically
///
/// AppKit delivers `-mouseMoved:` **only** to a view that asked for it. `-mouseDragged:`
/// (button held) and the tracking-area enter/exit pair are separate messages, so a
/// surface that registers only those three learns where the pointer *is* when a button
/// goes down and when it crosses the boundary — but never while it travels between two
/// controls inside the view.
///
/// That is the whole of hover: `widget::runtime::dispatch_pointer_event` synthesises the
/// `MouseEnter`/`MouseLeave` pair from a plain move (see `dispatch_hover_transition`), and
/// Windows (`WM_MOUSEMOVE`) and GTK (`connect_motion_notify_event`) both deliver one with
/// no button held. Without this option the macOS canvas was the one backend where no
/// control ever highlighted under the pointer.
const TRACKING_MOUSE_MOVED: usize = 0x0004;
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
///
/// # Safety
/// `view` and `event` must be live AppKit objects; `event` may be null, which reads as `None`.
unsafe fn event_local_point(view: id, event: id) -> Option<Point> {
    if event == nil {
        return None;
    }
    let window_point: NSPoint = msg_send![event, locationInWindow];
    Some(local_point(view, window_point))
}

/// Reads an AppKit event's modifier flags, or `0` for a null event.
///
/// `NSEvent`'s `modifierFlags` is the same source [`super::accelerator::map_modifiers`] reads
/// for keys, so a pointer press and a key press report the same bitfield for the same held keys.
///
/// # Safety
/// `event` must be a live `NSEvent` or null; a null event reads as zero modifiers.
unsafe fn ns_event_modifier_flags(event: id) -> u64 {
    if event == nil {
        return 0;
    }
    let flags: u64 = msg_send![event, modifierFlags];
    flags
}

/// Reads `NSEvent.clickCount`, the number of presses in the current click run.
///
/// Returns `0` for a null event, which callers compare with `>= 2`, so a missing
/// event degrades to a single click rather than panicking. `clickCount` is an
/// `NSInteger`, i.e. `isize` in the C ABI the runtime uses here.
///
/// # Safety
/// `event` must be a live `NSEvent` or null; a null event reads as zero clicks.
unsafe fn click_count(event: id) -> isize {
    if event == nil {
        return 0;
    }
    msg_send![event, clickCount]
}

/// Returns the canvas view's origin within its parent.
///
/// AppKit reports pointer positions in view-local space, but widget geometry in this
/// library is absolute, so the origin is added back before hit-testing. `frame` is
/// read live rather than cached because AppKit is free to move the view (auto layout,
/// a resize) without telling this module.
///
/// # Safety
/// `view` must be a live `NSView` on the main thread.
unsafe fn view_origin(view: id) -> Point {
    let frame: NSRect = msg_send![view, frame];
    Point::new(frame.origin.x as i32, frame.origin.y as i32)
}

/// Reads the modifier bitfield out of an AppKit event using the shared mapping.
extern "C" fn mouse_down(this: &Object, _cmd: Sel, event: id) {
    // A second primary press inside the system's double-click interval is its own
    // event ([`Event::MouseDoubleClick`]), not a press with a higher count: widgets
    // that act on a double click (an in-place editor, a list row) match on the
    // variant. AppKit reports the run length in `clickCount`, which is where the
    // interval's own definition lives, so this module never re-derives one.
    // SAFETY: `event` is the NSEvent AppKit passed to this selector; `clickCount` is
    // read-only NSView API and the call is bounded by the `catch_unwind` in
    // `forward_mouse` for the dispatch half. A null event is handled inside
    // `click_count`, which returns 0.
    if unsafe { click_count(event) } >= 2 {
        forward_mouse(this, event, MousePhase::DoubleClick);
        return;
    }
    forward_mouse(this, event, MousePhase::Press);
}

extern "C" fn mouse_up(this: &Object, _cmd: Sel, event: id) {
    forward_mouse(this, event, MousePhase::Release);
}

extern "C" fn mouse_dragged(this: &Object, _cmd: Sel, event: id) {
    forward_mouse(this, event, MousePhase::Drag);
}

/// `-mouseMoved:` — the pointer travelled with **no** button held.
///
/// A drag and a move both reach the widget as [`Event::MouseMove`], but they are not the
/// same repaint obligation: a drag must redraw on every sample, a hover move only when
/// the hover target changes. [`MousePhase::Move`] carries that distinction into
/// [`forward_mouse`].
extern "C" fn mouse_moved(this: &Object, _cmd: Sel, event: id) {
    forward_mouse(this, event, MousePhase::Move);
}

/// `-rightMouseDown:` — the secondary button went down.
///
/// A distinct responder method from `-mouseDown:`, so the phase (and therefore the
/// button the widget sees) cannot be confused with the primary one.
extern "C" fn right_mouse_down(this: &Object, _cmd: Sel, event: id) {
    forward_mouse(this, event, MousePhase::SecondaryPress);
}

/// `-rightMouseUp:` — the secondary button came up.
extern "C" fn right_mouse_up(this: &Object, _cmd: Sel, event: id) {
    forward_mouse(this, event, MousePhase::SecondaryRelease);
}

/// Which touch responder method AppKit called.
#[cfg(feature = "touch")]
#[derive(Clone, Copy)]
enum TouchPhase {
    Began,
    Moved,
    Ended,
    /// AppKit withdrew the contact: abandon the gesture without committing it.
    Cancelled,
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
/// Carried as its own `Cancelled` phase rather than an end (D09-EVT-02): a cancelled
/// contact must still terminate the recogniser's tracking (a `TouchBegin` with no
/// terminator leaves `PinchGesture` holding a phantom finger), but it must **not** be
/// treated as a successful lift. The internal cancel event lets controls reset their
/// latches without firing `clicked`/`toggled`/a stroke commit.
#[cfg(feature = "touch")]
extern "C" fn touches_cancelled(this: &Object, _cmd: Sel, event: id) {
    forward_touches(this, event, TouchPhase::Cancelled);
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
                    // The internal touch-cancel event: the translator and touch-aware
                    // controls clear state without synthesising a completed release
                    // (D09-EVT-02).
                    TouchPhase::Cancelled => {
                        crate::event::translator::touch_cancel(position, touch_id)
                    }
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
                note_native_redraw(widget_id);
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
                MousePhase::Press => Event::mouse_press_with(
                    position.x,
                    position.y,
                    1,
                    super::accelerator::map_modifiers(ns_event_modifier_flags(event)),
                ),
                MousePhase::Release => Event::MouseRelease { pos: position, button: 1 },
                MousePhase::Drag | MousePhase::Move => Event::MouseMove { pos: position },
                MousePhase::SecondaryPress => Event::mouse_press_with(
                    position.x,
                    position.y,
                    crate::event::mouse_button::SECONDARY,
                    super::accelerator::map_modifiers(ns_event_modifier_flags(event)),
                ),
                MousePhase::SecondaryRelease => Event::MouseRelease {
                    pos: position,
                    button: crate::event::mouse_button::SECONDARY,
                },
                MousePhase::DoubleClick => Event::mouse_double_click(position.x, position.y, 1),
            };
            // A hover move that stayed inside the same control changes nothing on screen.
            // It used to repaint the whole window anyway, which made pointer movement over a
            // large window cost a full software frame per sample. Reading the hover target
            // before and after is the exact question the move can change; every other phase
            // (press, release, drag, double click) owes a repaint unconditionally.
            let hover_before = if matches!(phase, MousePhase::Move) {
                crate::widget::runtime::hovered_widget()
            } else {
                None
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
            if matches!(
                phase,
                MousePhase::Press | MousePhase::SecondaryPress | MousePhase::DoubleClick
            ) {
                // A click can move focus to a nested control; take the keyboard so
                // subsequent keys are delivered to this view.
                let window: id = msg_send![view, window];
                if window != nil {
                    let _: () = msg_send![window, makeFirstResponder: view];
                }
            }
            let hover_changed = !matches!(phase, MousePhase::Move)
                || hover_before != crate::widget::runtime::hovered_widget();
            if !hover_changed {
                return;
            }
            // The widget's state changed, so ask AppKit to repaint it. Telling the library
            // too is what keeps a frame's account complete and stops the native and
            // self-painted worlds from counting one interaction twice (BLUE24 §7.2).
            let _: () = msg_send![view, setNeedsDisplay: YES];
            note_native_redraw(widget_id);
        }
    });
    if outcome.is_err() {
        log::error!("[macos] canvas: panic while forwarding a mouse event");
    }
}

/// Translates a key event into a widget [`Event`] and delivers it.
///
/// AppKit's input method gets first refusal through `interpretKeyEvents:`. When the input
/// method consumes the key (a marked-text update, a commit, or a plain character it turned
/// into `insertText:`) the callback sets `IME_HANDLED` and this path stops. Everything else —
/// arrows, Tab, chords — falls through to the translation below, so no key is swallowed.
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
            // Give the input method first refusal. `interpretKeyEvents:` calls back into the
            // `NSTextInputClient` methods registered on this class; the callbacks set
            // `IME_HANDLED` when they consumed the event. The flag is reset afterwards so a
            // callback invoked later (an asynchronous composition update) cannot affect the
            // next key.
            IME_INTERPRETING.set(true);
            IME_HANDLED.set(false);
            let events = NSArray::arrayWithObjects(nil, &[event]);
            let _: () = msg_send![view, interpretKeyEvents: events];
            let handled = IME_HANDLED.get();
            IME_INTERPRETING.set(false);
            if handled {
                return;
            }
            let translated_key = super::types::translate_key_event(event);
            let modifiers = translated_key.modifiers;
            // AppKit reports Tab as keycode 48 with no printable characters. The
            // widget-layer modifier bit for Shift is 1 (see `Modifiers::from_event_bits`).
            const WIDGET_SHIFT: u32 = 1;
            // A control chord is a command, not content: `⌘C`/`⌃C` must reach the
            // control's key handler (67 with the primary bit) instead of being typed as
            // the letter `c`. Option is deliberately *not* a chord here: on macOS it is
            // how a text field types an accented character (`⌥e` → `´`).
            const WIDGET_CONTROL: u32 = 2;
            const WIDGET_META: u32 = 8;
            if u32::from(translated_key.appkit_key_code) == KEY_TAB {
                crate::widget::runtime::focus_next(modifiers & WIDGET_SHIFT == 0);
                let _: () = msg_send![view, setNeedsDisplay: YES];
                note_native_redraw(widget_id);
                return;
            }
            let chord = modifiers & (WIDGET_CONTROL | WIDGET_META) != 0;
            let translated = if chord {
                // `charactersIgnoringModifiers` is the key's text with ⌘/⌃ stripped, so
                // `⌘A` answers `"a"`. The framework key code for that is `65` (upper
                // case), which is what `LineEdit` matches for `Primary+A`.
                Event::KeyPress {
                    key: translated_key.logical_key_with_ignored_modifiers,
                    modifiers,
                }
            } else if let Some(text) = translated_key.text {
                Event::TextInput { text }
            } else {
                Event::KeyPress { key: translated_key.logical_key, modifiers }
            };
            // Keys follow focus: with nothing focused, the surface owner keeps them.
            let target = crate::widget::runtime::focused_widget().unwrap_or(widget_id);
            if !crate::widget::runtime::dispatch_event(target, &translated) {
                log::debug!("[macos] canvas: key event dropped, id={target} is not mounted");
                return;
            }
            let _: () = msg_send![view, setNeedsDisplay: YES];
            note_native_redraw(target);
        }
    });
    if outcome.is_err() {
        log::error!("[macos] canvas: panic while forwarding a key event");
    }
}

/// AppKit's key-up selector.
///
/// # Why the release half has to be forwarded
///
/// `Event::KeyRelease` is published and consumed (`base.rs` turns it into `key_up`), but no
/// backend produced it before this arm: a control that tracks a held key could never learn it came
/// up. AppKit sends `keyUp:` for every key it sent `keyDown:` for, so registering the method is
/// what makes the variant reachable. `windows::types` has the same fix on Win32 and
/// `linux::canvas` on GTK, so the three desktop backends agree.
///
/// No printable-character branch: a release produces no text, so `KeyRelease` is the only event
/// this can be. Tab is not special-cased either — focus traversal happened on the press, and
/// repeating it here would move focus twice for one Tab.
extern "C" fn key_up(this: &Object, _cmd: Sel, event: id) {
    let outcome = std::panic::catch_unwind(|| {
        // SAFETY: `this` is the live canvas view; `event` is the NSEvent AppKit
        // delivered to this selector.
        unsafe {
            let view = this as *const Object as id;
            let Some(widget_id) = widget_id_of(view) else { return };
            let translated_key = super::types::translate_key_event(event);
            let released = Event::KeyRelease {
                key: translated_key.logical_key_with_ignored_modifiers,
                modifiers: translated_key.modifiers,
            };
            // Keys follow focus, the same rule `key_down` applies, so a release reaches the
            // control its press did.
            let target = crate::widget::runtime::focused_widget().unwrap_or(widget_id);
            if !crate::widget::runtime::dispatch_event(target, &released) {
                log::debug!("[macos] canvas: key release dropped, id={target} is not mounted");
                return;
            }
            let _: () = msg_send![view, setNeedsDisplay: YES];
            note_native_redraw(target);
        }
    });
    if outcome.is_err() {
        log::error!("[macos] canvas: panic while forwarding a key release event");
    }
}

/// AppKit virtual key code for Tab (not a character code).
const KEY_TAB: u32 = 48;

// ── NSTextInputClient ──
//
// AppKit calls these on the canvas view. Each one mirrors the objc2 canvas
// (`macos_objc2/native.rs`, BLUE28 B28.12) so both macOS backends drive the same
// `MacOsImeBridge` state machine identically. The bridge is shared with
// `Platform::ime_bridge()` (see `macos::types::ime_bridge`), which is what lets a composition
// reach the control the widget runtime has focused.

/// `-hasMarkedText`: whether a composition is in progress.
extern "C" fn has_marked_text(_this: &Object, _cmd: Sel) -> cocoa::base::BOOL {
    if super::types::ime_bridge().has_marked_text() {
        YES
    } else {
        NO
    }
}

/// `-markedRange`: the composed range in UTF-16 units, or `NSNotFound` when nothing is marked.
extern "C" fn marked_range(_this: &Object, _cmd: Sel) -> EncodedRange {
    match super::types::ime_bridge().get_marked_text() {
        Some(text) => EncodedRange(NSRange::new(0, text.encode_utf16().count() as NSUInteger)),
        None => EncodedRange(NSRange::new(NSNotFound as NSUInteger, 0)),
    }
}

/// `-selectedRange`: the caret sits at the end of the composition, matching objc2.
extern "C" fn selected_range(_this: &Object, _cmd: Sel) -> EncodedRange {
    let length =
        super::types::ime_bridge().get_marked_text().map_or(0, |text| text.encode_utf16().count());
    EncodedRange(NSRange::new(length as NSUInteger, 0))
}

/// `-validAttributesForMarkedText`: plain text needs no attributes, so the set is empty.
extern "C" fn valid_attributes_for_marked_text(_this: &Object, _cmd: Sel) -> id {
    // SAFETY: `+[NSArray array]` returns an autoreleased empty array owned by the call's pool.
    unsafe { NSArray::array(nil) }
}

/// `-attributedSubstringForProposedRange:actualRange:`: the widget layer owns the text, so the
/// surface has none to hand back.
extern "C" fn attributed_substring(
    _this: &Object,
    _cmd: Sel,
    _range: EncodedRange,
    actual_range: *mut c_void,
) -> id {
    if !actual_range.is_null() {
        // SAFETY: AppKit passes a writable `NSRange` out-parameter or null; the null case is
        // checked, and `NSRange` is the type AppKit named in the selector.
        unsafe { *(actual_range as *mut NSRange) = NSRange::new(0, 0) };
    }
    nil
}

/// `-setMarkedText:selectedRange:replacementRange:`: records the composition and delivers it as
/// `Event::ImePreedit` to the focused control.
extern "C" fn set_marked_text(
    this: &Object,
    _cmd: Sel,
    string: id,
    selected_range: EncodedRange,
    _replacement_range: EncodedRange,
) {
    let outcome = std::panic::catch_unwind(|| {
        // SAFETY: `this` is the live canvas view; `string` is the object AppKit passed.
        unsafe {
            let text = if string == nil {
                // AppKit spells "remove the marked text" as a nil argument. Treating it as an
                // empty composition is what actually clears the state (and matches what the GTK
                // context does on `preedit-end`); returning early would leave the old preedit
                // painted.
                String::new()
            } else {
                let Some(text) = text_of(string) else { return };
                text
            };
            if IME_INTERPRETING.get() {
                IME_HANDLED.set(true);
            }
            let bridge = super::types::ime_bridge();
            bridge.set_marked_text(
                &text,
                selected_range.0.location as i32,
                selected_range.0.location.saturating_add(selected_range.0.length) as i32,
            );
            // `set_marked_text` delivers the preedit to the widget the bridge has focused. Two
            // cases are left for this view to cover: nothing has focused the bridge yet (deliver
            // to this canvas's own widget), and the composition was withdrawn (an empty marked
            // string produces no `ImePreedit`, so send the empty composition that clears it).
            let delivered_by_bridge =
                bridge.focused_widget_id().is_some() && bridge.has_marked_text();
            if !delivered_by_bridge {
                if let Some(target) = bridge.focused_widget_id().or_else(|| ime_target(this)) {
                    let composition = bridge.composition().unwrap_or_default();
                    let _ = crate::platform::ime::deliver_composition_owned(target, composition);
                }
            }
            redraw_canvas(this);
        }
    });
    if outcome.is_err() {
        log::error!("[macos] canvas: panic while updating IME composition");
    }
}

/// `-insertText:replacementRange:`: a commit at the end of a composition, or a plain key AppKit
/// translated into text.
extern "C" fn insert_text(this: &Object, _cmd: Sel, string: id, _replacement_range: EncodedRange) {
    let outcome = std::panic::catch_unwind(|| {
        // SAFETY: `this` is the live canvas view; `string` is the object AppKit passed.
        unsafe {
            let Some(text) = text_of(string) else { return };
            if IME_INTERPRETING.get() {
                IME_HANDLED.set(true);
            }
            let bridge = super::types::ime_bridge();
            if bridge.has_marked_text() {
                // A commit at the end of a composition. `commit_text` clears the marked range and
                // delivers `Event::ImeCommit` to the bridge's focused widget.
                if bridge.focused_widget_id().is_some() {
                    bridge.commit_text(&text);
                } else if let Some(target) = ime_target(this) {
                    // No runtime focus reached the bridge, but this canvas still knows which
                    // control it presents: clear the preview, then commit there.
                    bridge.discard_marked_text();
                    let _ = crate::platform::ime::deliver_composition_owned(
                        target,
                        crate::platform::ime::ImeComposition::default(),
                    );
                    let _ = crate::platform::ime::deliver_commit(target, &text);
                }
            } else if let Some(target) = ime_target(this) {
                // A plain key that AppKit translated into text. It travels as `TextInput` so a
                // control's text path runs exactly as it did before IME was wired.
                if !crate::widget::runtime::dispatch_event(target, &Event::TextInput { text }) {
                    log::debug!("[macos] canvas: IME text dropped, id={target} is not mounted");
                }
            }
            redraw_canvas(this);
        }
    });
    if outcome.is_err() {
        log::error!("[macos] canvas: panic while forwarding IME text");
    }
}

/// `-unmarkText`: the input method withdrew its composition without committing.
extern "C" fn unmark_text(this: &Object, _cmd: Sel) {
    clear_composition(this);
}

/// `-doCommandBySelector:`: `interpretKeyEvents:` reached a command key it did not turn into
/// text. A composition can no longer become text, so it is discarded; the key itself is **not**
/// marked handled, which is what lets `key_down` forward it as a `KeyPress`.
extern "C" fn do_command_by_selector(this: &Object, _cmd: Sel, _selector: Sel) {
    clear_composition(this);
}

/// `-firstRectForCharacterRange:actualRange:`: the candidate window is anchored at the surface
/// origin, in screen coordinates.
extern "C" fn first_rect_for_character_range(
    this: &Object,
    _cmd: Sel,
    range: EncodedRange,
    actual_range: *mut c_void,
) -> NSRect {
    if !actual_range.is_null() {
        // SAFETY: see `attributed_substring`; the caller owns the out-parameter.
        unsafe { *(actual_range as *mut NSRange) = range.0 };
    }
    // SAFETY: `this` is the live canvas view AppKit is asking about.
    unsafe {
        let view = this as *const Object as id;
        let rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1.0, 1.0));
        let window: id = msg_send![view, window];
        if window == nil {
            rect
        } else {
            NSWindow::convertRectToScreen_(window, rect)
        }
    }
}

/// `-characterIndexForPoint:`: the surface has no indexable text (the control owns it).
extern "C" fn character_index_for_point(_this: &Object, _cmd: Sel, _point: NSPoint) -> NSUInteger {
    0
}

/// Discards the bridge's composition and clears the widget's preview.
///
/// AppKit calls `unmarkText` and `doCommandBySelector:` even when nothing is marked, so the work
/// is conditional: an arrow key must not emit an empty `ImePreedit` per keystroke.
fn clear_composition(this: &Object) {
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let bridge = super::types::ime_bridge();
        if !bridge.has_marked_text() {
            return;
        }
        bridge.discard_marked_text();
        if let Some(target) = bridge.focused_widget_id().or_else(|| ime_target(this)) {
            // An empty composition is how a control is told to stop painting its preedit; the
            // GTK backend sends the same event on `preedit-end`.
            let _ = crate::platform::ime::deliver_composition_owned(
                target,
                crate::platform::ime::ImeComposition::default(),
            );
        }
        redraw_canvas(this);
    }));
    if outcome.is_err() {
        log::error!("[macos] canvas: panic while clearing IME composition");
    }
}

/// Asks AppKit to repaint this canvas and records the native redraw for the frame ledger.
fn redraw_canvas(this: &Object) {
    // SAFETY: `this` is the live canvas view AppKit called; the call runs on the main thread.
    unsafe {
        let view = this as *const Object as id;
        let _: () = msg_send![view, setNeedsDisplay: YES];
        if let Some(target) = ime_target(this) {
            note_native_redraw(target);
        }
    }
}

/// The control an IME callback should deliver to: the widget runtime's focus, or this view's
/// widget when no control has taken focus.
///
/// The fallback keeps a composition usable on a surface whose control never received a click (a
/// canvas created with focus already inside it), and it is the same rule the key path applies
/// (`focused_widget().unwrap_or(widget_id)`).
fn ime_target(this: &Object) -> Option<ObjectId> {
    // SAFETY: `this` is the live canvas view AppKit is calling back for.
    unsafe {
        crate::widget::runtime::focused_widget()
            .or_else(|| widget_id_of(this as *const Object as id))
    }
}

/// Reads the text AppKit passes to `insertText:`/`setMarkedText:`.
///
/// The argument is documented as an `NSString` or an `NSAttributedString`; both are accepted and
/// anything else is ignored rather than reinterpreted.
///
/// # Safety
/// `object` must be the argument AppKit supplied to one of those selectors.
unsafe fn text_of(object: id) -> Option<String> {
    if object == nil {
        return None;
    }
    let is_string: cocoa::base::BOOL = msg_send![object, isKindOfClass: class!(NSString)];
    let string_object: id = if is_string == YES {
        object
    } else {
        let is_attributed: cocoa::base::BOOL =
            msg_send![object, isKindOfClass: class!(NSAttributedString)];
        if is_attributed != YES {
            return None;
        }
        msg_send![object, string]
    };
    let utf8: *const std::os::raw::c_char = msg_send![string_object, UTF8String];
    if utf8.is_null() {
        return None;
    }
    Some(std::ffi::CStr::from_ptr(utf8).to_string_lossy().into_owned())
}

/// Translates AppKit's dirty rectangle into a half-open band of frame rows.
///
/// The canvas reports `isFlipped == NO`, so AppKit's `drawRect:` rectangle has its origin at the
/// view's bottom-left while a frame buffer starts at its top row. The intersection is clamped to
/// the pixel bounds; `None` means "present the whole frame", which is also the answer for the
/// null/empty rectangle AppKit may pass.
fn dirty_row_range(dirty: NSRect, width: u32, height: u32) -> Option<(usize, usize)> {
    if width == 0 || height == 0 {
        return None;
    }
    if !dirty.origin.x.is_finite()
        || !dirty.origin.y.is_finite()
        || !dirty.size.width.is_finite()
        || !dirty.size.height.is_finite()
        || dirty.size.width <= 0.0
        || dirty.size.height <= 0.0
    {
        return None;
    }
    let width = width as f64;
    let height = height as f64;
    let min_x = dirty.origin.x.max(0.0);
    let max_x = (dirty.origin.x + dirty.size.width).min(width);
    let min_y = dirty.origin.y.max(0.0);
    let max_y = (dirty.origin.y + dirty.size.height).min(height);
    if max_x <= min_x || max_y <= min_y {
        return None;
    }
    if min_x <= 0.0 && max_x >= width && min_y <= 0.0 && max_y >= height {
        return None;
    }
    let first_row = (height - max_y).floor().max(0.0) as usize;
    let last_row = (height - min_y).ceil().min(height) as usize;
    (first_row < last_row).then_some((first_row, last_row))
}

/// `-drawRect:` renders the mounted widget and blits the frame.
///
/// Wrapped in `catch_unwind` because AppKit calls this from its own run loop; a
/// panic unwinding through an Objective-C frame is undefined behaviour.
///
/// The size still comes from `bounds`, never from `rect` (see the note below): a partial
/// invalidation hands a smaller rectangle, and sizing the frame from it would stretch the picture.
/// The rectangle is not ignored, though — it becomes the row band the blit converts and clips to,
/// so a cursor blink or a progress-bar step no longer pays a full-window conversion.
extern "C" fn draw_rect(this: &Object, _cmd: Sel, rect: NSRect) {
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
            let size = Size::new(width, height);
            // The cached frame is a full frame, so limiting the conversion to the damaged rows is
            // safe: rows outside the band are neither converted nor composited (the blit clips).
            let dirty_rows = dirty_row_range(rect, width, height);

            // A window's content view paints the window's own chrome **and every child**
            // (`render_frame_tree_cached`), whereas a mounted surface is one widget in its own box
            // (`render_frame_cached`). Without this branch the client area of every cocoa window
            // stayed blank: `create_window` installed a plain `NSView` and ordinary controls are
            // never mounted surfaces, so nothing ever drew them. This is the cocoa spelling of
            // the window painter Linux, Windows and the objc2 backend each already have
            // (`render_frame_tree`); the cached form reuses the previous frame for an exposure
            // that follows no content change, rather than re-rasterising the whole tree.
            // See `docs/log/log-20261016-3.md` §11.
            let frame = if is_window_canvas(view) {
                // The view stores the **host** id `create_window` returned, while
                // `render_frame_tree_cached` looks the widget up by its **registry** id; translate here
                // (the same boundary the objc2 backend translates at). No association yet means
                // the window has not been adopted by a widget — a normal transient just after
                // `create_window`, so it is a debug note, not an error.
                let Some(widget) = crate::widget::runtime::widget_id_for_host_window(widget_id)
                else {
                    log::debug!(
                        "[macos] canvas: drawRect: host id={widget_id} has no owning widget yet; \
                         nothing to paint"
                    );
                    return;
                };
                crate::widget::runtime::render_frame_tree_cached_shared(widget, size, Color::WHITE)
            } else {
                crate::widget::runtime::render_frame_cached_shared(widget_id, size, Color::WHITE)
            };
            let Some(frame) = frame else {
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
            // `[NSGraphicsContext currentContext]` is a **class** method. Asking
            let context: id = msg_send![class!(NSGraphicsContext), currentContext];
            if context == nil {
                log::error!(
                    "[macos] canvas: drawRect: +[NSGraphicsContext currentContext] was nil"
                );
                return;
            }
            let native: cg::CGContextRef = msg_send![context, CGContext];
            if !cg::blit_rgba(native, width, height, &frame, dirty_rows) {
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
///
/// # Safety
/// `view` must be a live `NSView` on the main thread.
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
///
/// # Safety
/// `view` must be a live `NSView` and the call must run on the main thread.
unsafe fn set_widget_id(view: id, widget_id: ObjectId) {
    objc_setAssociatedObject(
        view as *mut c_void,
        association_key() as *const c_void,
        widget_id as *mut c_void,
        OBJC_ASSOCIATION_ASSIGN,
    );
}

/// The association key marking a canvas as a **window** canvas.
///
/// Its presence means "this view is a window's content view, so `drawRect:` paints the whole
/// widget tree" rather than one mounted box. Carried as an associated object for the same reason
/// the widget id is: a plain `NSView` subclass is not KVC-compliant and `setValue:forKey:` would
/// raise.
fn window_flag_key() -> id {
    static KEY: OnceLock<usize> = OnceLock::new();
    let ptr = *KEY.get_or_init(|| {
        // SAFETY: standard `NSString` construction; the value lives for the process.
        unsafe { NSString::alloc(nil).init_str("RustWidgetsCanvasIsWindow") as id as usize }
    });
    ptr as id
}

/// Marks `view` as a window canvas.
///
/// # Safety
///
/// `view` must be a live `NSView` on the AppKit main thread.
unsafe fn set_window_canvas(view: id) {
    // Any non-null value marks the view; the pointer is never dereferenced.
    objc_setAssociatedObject(
        view as *mut c_void,
        window_flag_key() as *const c_void,
        std::ptr::null_mut::<c_void>().wrapping_add(1),
        OBJC_ASSOCIATION_ASSIGN,
    );
}

/// Whether `view` is a window canvas (see [`window_flag_key`]).
///
/// # Safety
///
/// `view` must be a live `NSView`.
unsafe fn is_window_canvas(view: id) -> bool {
    let value = objc_getAssociatedObject(view as *mut c_void, window_flag_key() as *const c_void);
    !value.is_null()
}

/// Side table from a **host** window id to its canvas content view.
///
/// Kept separate from [`mounted_views`]: a window canvas is the window's own content view, not a
/// mounted surface, so `resize_surface`/`unmount_surface` (which are keyed by surface id) must not
/// reach it. Only the invalidation path looks here.
fn window_canvases() -> &'static Mutex<HashMap<ObjectId, usize>> {
    static VIEWS: OnceLock<Mutex<HashMap<ObjectId, usize>>> = OnceLock::new();
    VIEWS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The window canvas that should repaint `id`.
///
/// `id` may be a window's widget-registry id (what `request_repaint_subtree` names for the window)
/// or a child control's id (which has no native view of its own). Both resolve, through the
/// recorded host association, to the window whose canvas paints the whole tree.
fn window_canvas_for(id: ObjectId) -> Option<id> {
    let host = crate::widget::runtime::host_window_for(id).or_else(|| {
        crate::widget::runtime::top_level_window_of(id)
            .and_then(crate::widget::runtime::host_window_for)
    })?;
    let table = crate::compat::lock(window_canvases());
    table.get(&host).map(|ptr| *ptr as id)
}

/// Creates a canvas content view for the window registered under `host_id`.
///
/// The view is sized to the window's content area, tagged with `host_id`, marked as a window
/// canvas, and recorded in [`window_canvases`] so a repaint request can reach it. Returns `None`
/// when the view could not be allocated.
///
/// # Safety
///
/// Must be called on the AppKit main thread with `host_id` already registered.
pub(crate) unsafe fn make_window_canvas(host_id: ObjectId, width: u32, height: u32) -> Option<id> {
    let class = canvas_view_class();
    let allocated: id = msg_send![class, alloc];
    let view: id = msg_send![
        allocated,
        initWithFrame: NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(width as f64, height as f64),
        )
    ];
    if view == nil {
        log::error!("[macos] create_window: canvas view allocation failed");
        return None;
    }
    set_widget_id(view, host_id);
    set_window_canvas(view);
    crate::compat::lock(window_canvases()).insert(host_id, view as usize);
    Some(view)
}

/// Drops the [`window_canvases`] entry for a host window that is being destroyed.
///
/// The entry holds a raw pointer to the window's content view. Once the window is closed that
/// pointer is stale, and leaving it in the table let a repaint request route to a view that no
/// longer belongs to any window. Clearing it here is the destroy-side half of the balance that
/// `make_window_canvas` establishes.
pub(crate) fn forget_window_canvas(host_id: ObjectId) {
    let _ = crate::compat::lock(window_canvases()).remove(&host_id);
}

/// Takes a reference that this module's side table owns for a canvas view.
///
/// The discipline mirrors `macos_objc2::native::store_native_view`: the view is retained for the
/// side table, so the table's pointer stays valid independently of the superview, and the matching
/// release happens once at teardown.
///
/// # Safety
///
/// `view` must be a live canvas view on the AppKit main thread.
unsafe fn retain_canvas_view(view: id) {
    let _: *mut c_void = msg_send![view, retain];
}

/// Releases one reference this module owns to a canvas view.
///
/// `mount_surface_impl` calls it for the `alloc` reference once `addSubview:` has retained the
/// view; `unmount_surface_impl` calls it for the side table's reference after
/// `removeFromSuperview`. Keeping both halves on one helper is what stops the pair from drifting.
///
/// # Safety
///
/// `view` must be a live canvas view and the caller must own a reference to it.
pub(crate) unsafe fn release_canvas_view(view: id) {
    let _: () = msg_send![view, release];
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
                // Nothing took a reference to the view, so its `alloc` reference is the only one.
                release_canvas_view(view);
                pool.drain();
                return false;
            }
            let _: () = msg_send![content_view, addSubview: view];
            // `addSubview:` retained the view. The side table takes its own reference (the same
            // retain/release pair `macos_objc2::native::store_native_view` keeps), and the `alloc`
            // reference is then dropped: without this the view left behind one owned reference per
            // mount/unmount cycle and every cycle leaked an `NSView`.
            retain_canvas_view(view);
            release_canvas_view(view);
            let _: () = msg_send![view, setNeedsDisplay: YES];
            // The view is being revealed, which is the platform repainting a surface the
            // library just mounted. Announcing it keeps the frame that reveals it accounted
            // for like every other platform redraw (§7.2).
            note_native_redraw(id);
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
            // Two messages, not `setFrame:display:`.
            //
            // `setFrame:` alone is not enough: AppKit does not schedule a display pass for
            // a frame change on its own, so a moved or resized surface keeps its old pixels
            // until some unrelated invalidation happens to arrive (BLUE14 F-5). The
            // historical spelling of that fix was the two-argument `setFrame:display:`,
            // but **`NSView` on macOS does not respond to it** — neither the SDK's own
            // `NSView` nor this subclass does (`swift -e` probe:
            // `NSView().responds(to: Selector(("setFrame:display:"))) == false`). Sending it
            // raised `unrecognized selector sent to instance`, and because a foreign
            // exception cannot unwind through Rust, the process aborted — the code-editor
            // demo died on its first layout pass. `setFrame:` plus an explicit
            // `setNeedsDisplay:` expresses the same intent with messages AppKit does
            // implement, and coalesces the redraw into the next display cycle.
            let _: () = msg_send![
                view,
                setFrame: NSRect::new(
                    NSPoint::new(rect.x as f64, rect.y as f64),
                    NSSize::new(rect.width as f64, rect.height as f64),
                )
            ];
            let _: () = msg_send![view, setNeedsDisplay: YES];
        }
        // The platform is now going to repaint this surface at its new geometry; announce it
        // exactly as `mount_surface_impl` does, so the frame account stays complete.
        note_native_redraw(id);
        crate::widget::runtime::set_geometry(id, rect);
        true
    }

    /// Marks the canvas view as needing display.
    ///
    /// Used when a widget is mutated from outside the event loop (a menu action
    /// or tool-bar button), so the change becomes visible without waiting for an
    /// unrelated invalidation.
    ///
    /// `id` may name a mounted surface, or a window / a control inside one. The latter two have no
    /// canvas of their own — the window's content view paints the whole tree — so they resolve
    /// through [`window_canvas_for`]. Without that resolution the repaint request the library makes
    /// for a window (and for every control, which the same walk requests) reached nothing, and the
    /// window stayed blank.
    pub(crate) fn invalidate_surface_impl(&self, id: ObjectId) -> bool {
        let Some(view) = view_for(id).or_else(|| window_canvas_for(id)) else {
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
        // SAFETY: the side-table entry was removed just above, so the module's reference is the
        // one being released. `removeFromSuperview` first drops the superview's reference (zero
        // when the table's is the only other one), then the table's own reference is released —
        // the same order `macos_objc2::native::remove_native_view` uses.
        unsafe {
            let _: () = msg_send![view as id, removeFromSuperview];
            release_canvas_view(view as id);
        }
        crate::compat::lock(&self.handles).remove(&id);
        self.state.destroy_widget(id);
        true
    }
}

/// Converts a point in a non-flipped AppKit view into **widget** coordinates (top-down, local).
///
/// Split out of [`local_point`] because it is pure arithmetic and therefore testable without a
/// live `NSView`: the whole point of the function is the subtraction, and a missing subtraction
/// is invisible on a machine with no display (it made every click land on the mirrored control).
pub(crate) fn widget_point(view_height: f64, local: NSPoint) -> Point {
    Point::new(local.x.round() as i32, (view_height - local.y).round() as i32)
}

/// Converts a window-space AppKit point into canvas-local coordinates.
///
/// NSWindow reports bottom-up window coordinates and the canvas is **not** flipped
/// ([`is_flipped`] returns `NO`, see its note on why), so `convertPoint:fromView:` answers a
/// `y` measured from the view's **bottom** edge. Widget geometry counts from the top, so
/// [`widget_point`] subtracts it from the view's height — without that every click was mirrored
/// vertically and landed on whichever control sat at `height - y`, which made a press on the top
/// row arrive as a press on the bottom row (or on empty space, so nothing happened at all).
///
/// `bounds`, not `frame`: same size, but `bounds` is expressed in the view's own coordinate
/// space and carries no superview offset.
///
/// # Safety
///
/// `view` must be a live canvas view.
pub(crate) unsafe fn local_point(view: id, window_point: NSPoint) -> Point {
    let local: NSPoint = msg_send![view, convertPoint: window_point fromView: nil];
    let bounds: NSRect = msg_send![view, bounds];
    widget_point(bounds.size.height, local)
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

/// The class AppKit's frame heartbeat targets.
///
/// # Why the heartbeat is its own class rather than the window delegate
///
/// The heartbeat must exist for the whole session, including before any window is created and for a
/// session that never creates one. It used to be a repeating `NSTimer` installed *per window* in
/// [`install_resize_delegate`], so a session with no window had no frame tick at all: a stop request
/// (or any queued main-thread work) had no reader, and the session could not end itself — the
/// designer feedback `docs/issues/feedback-20261015.md` §十.2. Moving the timer to its own class, and
/// installing it once in `run()`, makes the tick a property of the loop rather than of a window.
///
/// # Why this is the *only* frame timer
///
/// `install_resize_delegate` no longer starts a timer, so there is exactly one heartbeat per process
/// (principle #101: one implementation path). Two timers would call `crate::drive_frame` twice per
/// interval and advance every animation at double speed.
fn frame_tick_class() -> *const Class {
    static CLASS: OnceLock<usize> = OnceLock::new();
    (*CLASS.get_or_init(|| {
        let superclass = class!(NSObject);
        let mut decl = ClassDecl::new("RustWidgetsFrameTimer", superclass)
            .expect("the Objective-C runtime refused to declare RustWidgetsFrameTimer");
        // SAFETY: `tick:` takes no argument beyond the selector's implicit ones, so the ABI is
        // `(self, _cmd)` plus the timer the selector carries. The registered implementation is the
        // safe `extern "C"` form `add_method` accepts.
        unsafe {
            decl.add_method(sel!(tick:), drain_triggers_impl as extern "C" fn(&Object, Sel, id));
        }
        (decl.register() as *const Class) as usize
    })) as *const Class
}

/// Installs the process-wide frame heartbeat on the current run loop.
///
/// Called once from `MacOSPlatform::run()` before `-[NSApplication run]`, so the tick exists for the
/// whole session regardless of how many windows are created (including none). The timer repeats
/// unconditionally: an `NSTimer` cannot be told to stop without invalidating it, and invalidating it
/// would also stop the drain that reads resize triggers — see [`drain_triggers_impl`].
///
/// # Safety
///
/// Must be called on the AppKit main thread with an active run loop (i.e. from `run()`), because
/// `scheduledTimerWithTimeInterval:…` adds the timer to the current thread's run loop.
pub(crate) unsafe fn install_frame_heartbeat() {
    // Start the display link before the timer so the first tick already has a presented-time
    // deadline. A host where CoreVideo is unavailable answers `false`; the timer below then keeps
    // its fixed interval and `supports_vsync_frame` stays `false`.
    let _ = super::display_link::start();
    let target: id = msg_send![frame_tick_class(), new];
    let _timer: id = msg_send![
        class!(NSTimer),
        scheduledTimerWithTimeInterval: (FRAME_INTERVAL_MS as f64) / 1000.0f64
        target: target
        selector: sel!(tick:)
        userInfo: nil
        repeats: YES
    ];
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
///
/// # Safety
/// `this` must be the delegate object and `notification` a live resize notification, as AppKit guarantees.
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

/// Safe-ABI trampoline for the frame timer implementation.
///
/// Same reason as [`window_did_resize_impl`]: `add_method` accepts only the safe
/// `extern "C"` form.
///
/// # Why it drives a frame rather than draining the queue
///
/// The selector is still `tick:` and the timer is unchanged, but the body used to be
/// `crate::drain_triggers()` alone -- half a frame. Emptying the trigger queue re-runs
/// layout after a resize and does nothing else, so on macOS every hover fade, caret
/// blink and toggle transition was unreachable: nothing advanced the animation bus
/// (BLUE24 §0A.1 measurement 1). `crate::drive_frame` is that missing half, and it
/// drains as well, so this is strictly the same queue handling plus the animation step.
///
/// The timer is left repeating unconditionally. A `ControlFlow`-style stop is not
/// available to `NSTimer` without invalidating it, and doing so would stop the drain
/// too -- so a still window would stop hearing the resizes that could start it moving.
/// A still frame is only a lookup plus an `is_animating()` sweep, which is what makes
/// the unconditional heartbeat affordable; see [`crate::drive_frame`].
extern "C" fn drain_triggers_impl(_this: &Object, _cmd: Sel, timer: id) {
    // The display link publishes the instant the next frame is presented. When it has one, the
    // animation clock advances by the measured presented-time delta instead of the fixed
    // assumption; without one the helper answers the nominal interval and resets its phase.
    let deadline = super::display_link::next_deadline();
    let delta = crate::platform::frame_wait::presented_delta_ms(deadline);
    let outcome = crate::drive_frame(delta);
    if outcome.needs_another_frame {
        if let Some(deadline) = deadline {
            retarget_heartbeat(timer, deadline);
        }
    } else {
        // Nothing is animating and nothing is queued (G-503): a still window does not need 60
        // wakeups a second. The tick is moved out to the idle backstop, which still drains the
        // resize triggers and notices a host that changed state without a wake, and the next
        // animated frame moves it back to the display deadline.
        retarget_heartbeat_in(
            timer,
            Duration::from_millis(crate::platform::types::IDLE_FRAME_INTERVAL_MS),
        );
    }
}

/// Moves the repeating heartbeat's next fire to `deadline` so the loop follows the display.
///
/// The timer stays repeating and is never invalidated: invalidating it would also stop the
/// resize drain that shares this tick. `setFireDate:` only moves the next fire; the follow-up
/// iterations are retargeted the same way, so the cadence tracks the display rather than drifting
/// against it. A deadline that is already at hand is left alone so a missed presentation cannot
/// turn the timer into a busy loop.
fn retarget_heartbeat(timer: id, deadline: Instant) {
    let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
        return;
    };
    retarget_heartbeat_in(timer, remaining);
}

/// Moves the repeating heartbeat's next fire to `delay` from now.
///
/// Shared by the presented-deadline path and the idle backstop (G-503). A delay below one
/// millisecond is left alone so a missed presentation cannot turn the timer into a busy loop.
fn retarget_heartbeat_in(timer: id, delay: Duration) {
    if timer == nil {
        return;
    }
    if delay < Duration::from_millis(1) {
        return;
    }
    // SAFETY: `timer` is the live `NSTimer` AppKit passed to the tick selector, and `NSDate` is
    // the class `setFireDate:` takes. Both messages run on the AppKit main thread, which is where
    // the timer fires.
    unsafe {
        let date: id = msg_send![class!(NSDate), dateWithTimeIntervalSinceNow: delay.as_secs_f64()];
        if date != nil {
            let _: () = msg_send![timer, setFireDate: date];
        }
    }
}

/// The frame interval this backend's timer runs at, in milliseconds.
///
/// The timer's interval and the delta handed to [`crate::drive_frame`] must be the same
/// number, or every transition runs at the ratio between them; naming it once keeps
/// them from drifting apart (see `platform/linux/platform_impl.rs`'s twin).
const FRAME_INTERVAL_MS: u64 = crate::platform::types::FRAME_INTERVAL_MS;

/// Association key holding the resize delegate on its window.
///
/// `NSWindow.delegate` is a **weak** (`assign`) property, so the window does not own its delegate.
/// The delegate is attached under this key with a retaining policy instead, which gives it
/// exactly the lifetime the old comment claimed the window provided (ENG-C5).
fn resize_delegate_key() -> id {
    static KEY: OnceLock<usize> = OnceLock::new();
    let ptr = *KEY.get_or_init(|| {
        // SAFETY: standard NSString construction; the value lives for the process.
        unsafe { NSString::alloc(nil).init_str("RustWidgetsResizeDelegate") as id as usize }
    });
    ptr as id
}

/// Installs the resize delegate on `window` and tags it with `widget_id`.
///
/// The frame heartbeat is **not** started here; it lives on the process-wide timer
/// [`install_frame_heartbeat`] installs from `run()`, so a session ticks whether or not a window
/// exists (feedback §十.2). This function only wires resize reporting.
///
/// # Safety
///
/// `window` must be a live `NSWindow` on the AppKit main thread.
pub(crate) unsafe fn install_resize_delegate(window: id, widget_id: ObjectId) {
    // `NSWindow.delegate` does not retain its delegate (it is a weak property), so the `+1`
    // reference `new` hands back had no owner: every real window leaked one delegate (ENG-C5).
    // Attaching the delegate to the window as a retained associated object makes its lifetime
    // match the window, which is what the previous comment assumed AppKit provided.
    let delegate: id = msg_send![window_delegate_class(), new];
    objc_setAssociatedObject(
        delegate as *mut c_void,
        window_association_key() as *const c_void,
        widget_id as *mut c_void,
        OBJC_ASSOCIATION_ASSIGN,
    );
    // Order matters: associate (retains), balance `new`, then hand the weak slot the object that
    // the association now owns.
    objc_setAssociatedObject(
        window as *mut c_void,
        resize_delegate_key() as *const c_void,
        delegate as *mut c_void,
        OBJC_ASSOCIATION_RETAIN_NONATOMIC,
    );
    let _: () = msg_send![delegate, release];
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

    /// The canvas is **not** flipped, so AppKit's `y` counts up from the bottom while widget
    /// geometry counts down from the top. Dropping the subtraction turned every click into its
    /// own mirror image: a press on the top row arrived as a press on the bottom row, which is
    /// how "the demo's controls do not react" looked from the outside.
    #[test]
    fn a_bottom_up_view_point_maps_to_top_down_widget_coordinates() {
        // 760-tall view: the top edge is widget y = 0, the bottom edge widget y = 760.
        assert_eq!(widget_point(760.0, NSPoint::new(20.0, 760.0)), Point::new(20, 0));
        assert_eq!(widget_point(760.0, NSPoint::new(20.0, 0.0)), Point::new(20, 760));
        // A press 20 px below the top edge is 740 px up from the bottom.
        assert_eq!(widget_point(760.0, NSPoint::new(95.0, 740.0)), Point::new(95, 20));
        // `x` is untouched: only the vertical axis is inverted.
        assert_eq!(widget_point(760.0, NSPoint::new(1180.0, 380.0)), Point::new(1180, 380));
    }
}

// probe
