// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Native AppKit FFI wrappers for the macOS objc2 backend (BLUE11 R1.5/R2.3 100%).
//!
//! This module is gated behind `#[cfg(target_os = "macos")]` and the
//! `objc2-macos` feature flag.
//!
//! # The presentation path
//!
//! `macos_objc2` is a self-drawn backend: every `WidgetKind` is painted by
//! `src/widget/`, so the only native object it needs is a **surface** that pulls
//! one RGBA frame out of [`crate::widget::runtime`] and blits it. That surface is
//! [`RustWidgetsObjc2CanvasView`], an `NSView` subclass whose `drawRect:` mirrors
//! `macos/canvas.rs::draw_rect` for the sibling cocoa backend.
//!
//! Without it `MacOSObjc2Platform::supports_surfaces()` promised a presentation
//! path that did not exist: `mount_surface` recorded bookkeeping and no pixels
//! ever reached the screen.

#![cfg(target_os = "macos")]
#![cfg(feature = "macos")]
// objc2 init methods may require unsafe blocks depending on platform
#![allow(unused_unsafe)]

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::sel;
use objc2::{define_class, msg_send, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSBackingStoreType, NSGraphicsContext, NSView, NSWindow, NSWindowStyleMask,
};
use objc2_core_graphics::CGContext;
use objc2_foundation::{NSRect, NSSize, NSString};

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

/// Side table from a mounted surface id to its canvas view pointer.
///
/// Distinct from [`NATIVE_VIEWS`], which holds the window (or control) a widget id
/// was created as. A *surface* is the drawing view mounted **onto** that window, so
/// `resize_surface` / `invalidate_surface` / `unmount_surface` need to find it by
/// the id passed to [`mount_surface_native`] independently of the window registry.
static SURFACE_VIEWS: LazyLock<Mutex<HashMap<u64, NativePtr>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Returns whether the calling thread is the AppKit main thread.
///
/// Every native entry point in this module re-checks this before messaging
/// AppKit. It is exported so `platform_impl` can choose the documented state-only
/// fallback off the main thread rather than mistaking "AppKit is unreachable" for
/// "the surface failed to mount".
pub(crate) fn on_main_thread() -> bool {
    MainThreadMarker::new().is_some()
}

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

/// Returns whether `object` is (or descends from) `NSWindow`.
///
/// Used by [`destroy_native_handle`] to decide between closing a window and
/// detaching a plain view: `removeFromSuperview` on an `NSWindow` does nothing
/// useful and never takes it off screen, so a logically destroyed window could
/// stay visible.
unsafe fn object_is_window(object: *mut AnyObject) -> bool {
    let window_class = <NSWindow as objc2::ClassType>::class();
    let is_window: bool = msg_send![object, isKindOfClass: window_class];
    is_window
}

/// Destroys a native handle created by this backend.
///
/// A **window** is closed with `close` (which hides and releases the window's
/// AppKit resources); any other view is detached with `removeFromSuperview`.
/// Both run only on the main thread — off it, AppKit must not be messaged, so the
/// call logs and returns `false` and the caller keeps the bookkeeping it had.
pub(crate) fn destroy_native_handle(widget_id: u64) -> bool {
    if MainThreadMarker::new().is_none() {
        log::error!(
            "[macos-objc2] destroy_handle: refused off the AppKit main thread (id={widget_id})"
        );
        return false;
    }
    let removed = NATIVE_VIEWS.lock().unwrap().remove(&widget_id);
    let Some(ptr) = removed else {
        // No native object was ever created for this id (a state-only widget), so
        // there is nothing to tear down and the caller's bookkeeping stands.
        return false;
    };
    unsafe {
        let object = ptr.0 as *mut AnyObject;
        if object_is_window(object) {
            // `close`, not `orderOut:`: `close` is the symmetric counterpart of
            // `makeKeyAndOrderFront:`, releasing the window and its views. An
            // `orderOut:` alone would hide it but leave the object retained.
            let _: () = msg_send![object, close];
        } else {
            let _: () = msg_send![object, removeFromSuperview];
        }
        let _: () = msg_send![object, release];
    }
    true
}

fn make_rect(x: i32, y: i32, width: u32, height: u32) -> NSRect {
    NSRect::new(
        objc2_foundation::NSPoint::new(x as f64, y as f64),
        NSSize::new(width.max(1) as f64, height.max(1) as f64),
    )
}

// ---------------------------------------------------------------------------
// Canvas view: the presentation path
// ---------------------------------------------------------------------------

/// Instance variables of [`RustWidgetsObjc2CanvasView`].
///
/// The widget id is stored in an ivar rather than an associated object because
/// `define_class!` finalises the ivar layout at class-registration time, which is
/// exactly the point of the macro — the id travels with the view for the view's
/// whole lifetime, including the `drawRect:` calls AppKit makes from its own
/// display machinery.
pub(crate) struct CanvasIvars {
    /// Registry id of the widget this view presents.
    widget_id: std::cell::Cell<u64>,
    /// `true` when this canvas is a window's content view, so `drawRect:` paints
    /// the window **and its descendants** rather than one mounted box.
    is_window: std::cell::Cell<bool>,
}

define_class!(
    // SAFETY:
    // - `NSView` has no subclassing requirements beyond using the designated
    //   initializer, which `init_with_frame` performs.
    // - `RustWidgetsObjc2CanvasView` does not implement `Drop`; the ivar is a
    //   `Cell<u64>`, which needs no release.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "RustWidgetsObjc2CanvasView"]
    #[ivars = CanvasIvars]
    pub(crate) struct RustWidgetsObjc2CanvasView;

    impl RustWidgetsObjc2CanvasView {
        /// `drawRect:` — pull one RGBA frame from the widget runtime and blit it.
        ///
        /// Mirrors `macos/canvas.rs::draw_rect`: the size comes from `bounds`, not
        /// from the dirty rectangle AppKit passes, because a partial invalidation
        /// hands a rect smaller than the view and sizing the frame from it would
        /// stretch the image into the wrong place.
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty_rect: NSRect) {
            // A panic must not unwind through AppKit: a foreign exception cannot
            // cross the Objective-C frame, so it is caught and logged here.
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.blit_frame();
            }));
            if outcome.is_err() {
                log::error!("[macos-objc2] canvas: panic while drawing a self-drawn widget");
            }
        }

        /// The view uses AppKit's native coordinate system, so a frame buffer's
        /// top-left origin lands at the view's top-left after the blit flips rows.
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            false
        }
    }
);

impl RustWidgetsObjc2CanvasView {
    /// Renders one frame for the carried widget id and draws it into the view's
    /// current CoreGraphics context.
    fn blit_frame(&self) {
        let widget_id = self.ivars().widget_id.get();
        if widget_id == 0 {
            log::error!("[macos-objc2] canvas: drawRect: on a view with no widget id");
            return;
        }
        // The size comes from `bounds`, not the dirty rect (see `draw_rect`).
        let bounds = self.bounds();
        let width = bounds.size.width.round().max(1.0) as u32;
        let height = bounds.size.height.round().max(1.0) as u32;
        let clear = crate::core::Color::WHITE;
        let size = crate::core::Size::new(width, height);

        #[cfg(widgets_unstripped)]
        let frame = if self.ivars().is_window.get() {
            // A window's content view paints the window's own chrome and every child
            // that implements `Draw`; a plain `render_frame_cached` would show the
            // window over an empty client area.
            crate::widget::runtime::render_frame_tree(widget_id, size, clear)
        } else {
            // A mounted surface is one self-drawn widget in its own box, which is
            // what the cocoa backend's `draw_rect` blits.
            crate::widget::runtime::render_frame_cached(widget_id, size, clear)
        };
        #[cfg(widgets_unstripped)]
        let Some(frame) = frame
        else {
            // Not mounted (dropped while the view survived) or it refused to paint.
            log::error!(
                "[macos-objc2] canvas: drawRect: widget id={widget_id} produced no frame \
                 (unmounted, or it does not implement Draw)"
            );
            return;
        };
        #[cfg(not(widgets_unstripped))]
        {
            // A stripped profile has no `widget::runtime`, so there is no frame to
            // pull. Log the reason rather than shipping a silent blank view.
            log::error!(
                "[macos-objc2] canvas: drawRect: widget id={widget_id} cannot render in a \
                 profile without an unstripped widget set"
            );
            return;
        }

        let Some(context) = NSGraphicsContext::currentContext() else {
            log::error!(
                "[macos-objc2] canvas: drawRect: +[NSGraphicsContext currentContext] was nil"
            );
            return;
        };
        let cg_context = context.CGContext();
        if !blit_rgba(&cg_context, width, height, &frame) {
            log::error!("[macos-objc2] canvas: drawRect: blit failed for {width}x{height}");
        }
    }

    /// Allocates a canvas view for `widget_id` sized to `frame`, on the main thread.
    ///
    /// `is_window` selects tree rendering (a window paints its children) over the
    /// single-box rendering a mounted surface uses.
    ///
    /// The ivars are installed before `initWithFrame:`, which is the two-phase
    /// initialisation `define_class!` expects.
    pub(crate) fn new(
        mtm: MainThreadMarker,
        widget_id: u64,
        is_window: bool,
        frame: NSRect,
    ) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(CanvasIvars {
            widget_id: std::cell::Cell::new(widget_id),
            is_window: std::cell::Cell::new(is_window),
        });
        // SAFETY: `this` is an allocated instance with its ivars initialised, and
        // `initWithFrame:` is the designated initializer of `NSView`.
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

/// Blits a top-down, straight-alpha RGBA frame into a CoreGraphics context.
///
/// The two details the caller should not have to think about:
///
/// 1. **Premultiplication** — the frame is straight alpha; `PremultipliedLast` is
///    not, so RGB is scaled by alpha.
/// 2. **Ownership** — the colour space, bitmap context and image are all released
///    here, including on the early-return paths.
///
/// The image is drawn at its own pixel size (a 1:1 blit). Like
/// `macos/cg.rs::blit_rgba`, no CTM flip is applied: the canvas reports
/// `isFlipped == NO`, so the frame's row order matches the context's.
///
/// Uses `objc2-core-graphics` (already a dependency of the `macos` feature)
/// rather than re-declaring the FFI the cocoa backend keeps in `macos/cg.rs`.
///
/// Returns `false` when a CoreGraphics object cannot be created or the frame is
/// too small for the requested size.
fn blit_rgba(context: &CGContext, width: u32, height: u32, frame: &[u8]) -> bool {
    use objc2_core_graphics::{
        CGBitmapContextCreate, CGBitmapContextCreateImage, CGBitmapContextGetBytesPerRow,
        CGBitmapContextGetData, CGColorSpace, CGContext, CGImage, CGImageAlphaInfo,
        CGImageByteOrderInfo, CGInterpolationQuality,
    };

    if width == 0 || height == 0 {
        return false;
    }
    let expected = width as usize * height as usize * 4;
    if frame.len() < expected {
        log::error!(
            "[macos-objc2] canvas: frame is {} bytes, need {} for {width}x{height}",
            frame.len(),
            expected
        );
        return false;
    }

    let Some(color_space) = CGColorSpace::new_device_rgb() else {
        log::error!("[macos-objc2] canvas: CGColorSpaceCreateDeviceRGB returned null");
        return false;
    };

    // `kCGBitmapByteOrder32Little | kCGImageAlphaPremultipliedLast` gives an
    // in-memory R, G, B, A byte order — the order the software renderer produces.
    let bitmap_info = CGImageAlphaInfo::PremultipliedLast.0 | CGImageByteOrderInfo::Order32Little.0;
    // SAFETY: every pointer argument is either null (let CG allocate) or a value
    // derived from the borrowed `frame` and the live colour space.
    let Some(bitmap) = (unsafe {
        CGBitmapContextCreate(
            std::ptr::null_mut(),
            width as usize,
            height as usize,
            8,
            width as usize * 4,
            Some(&color_space),
            bitmap_info,
        )
    }) else {
        log::error!(
            "[macos-objc2] canvas: CGBitmapContextCreate returned null for {width}x{height}"
        );
        return false;
    };

    // Copy in with premultiplied alpha, row by row using the bitmap's **actual**
    // stride: CoreGraphics may pad rows once the width crosses an alignment
    // boundary, and assuming `width * 4` shifts every row after the first.
    let destination = unsafe { CGBitmapContextGetData(Some(&bitmap)) } as *mut u8;
    if !destination.is_null() {
        let stride = unsafe { CGBitmapContextGetBytesPerRow(Some(&bitmap)) };
        for y in 0..height as usize {
            for x in 0..width as usize {
                let source = (y * width as usize + x) * 4;
                let target = y * stride + x * 4;
                let alpha = frame[source + 3] as u32;
                // SAFETY: `target + 3 < height * stride` because the bitmap was
                // created at `height` rows; each row of `destination` is `stride`
                // bytes wide.
                unsafe {
                    *destination.add(target) = (frame[source] as u32 * alpha / 255) as u8;
                    *destination.add(target + 1) = (frame[source + 1] as u32 * alpha / 255) as u8;
                    *destination.add(target + 2) = (frame[source + 2] as u32 * alpha / 255) as u8;
                    *destination.add(target + 3) = alpha as u8;
                }
            }
        }
    }

    let Some(image) = (unsafe { CGBitmapContextCreateImage(Some(&bitmap)) }) else {
        log::error!("[macos-objc2] canvas: CGBitmapContextCreateImage returned null");
        return false;
    };

    // SAFETY: `context` is valid for the duration of the draw (AppKit owns it for
    // the current `drawRect:`), and `image` is live. `NSRect` is an alias of
    // `CGRect`, so the rect built from the image's own pixel size can be passed
    // straight to `CGContextDrawImage`.
    unsafe {
        CGContext::set_interpolation_quality(Some(context), CGInterpolationQuality::None);
        let image_width = CGImage::width(Some(&image)) as f64;
        let image_height = CGImage::height(Some(&image)) as f64;
        let rect = NSRect::new(
            objc2_foundation::NSPoint::new(0.0, 0.0),
            NSSize::new(image_width, image_height),
        );
        CGContext::draw_image(Some(context), rect, Some(&image));
    }
    true
}

// ---------------------------------------------------------------------------
// Window creation
// ---------------------------------------------------------------------------

pub(crate) fn create_ns_window(
    mtm: MainThreadMarker,
    widget_id: u64,
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

    // Install a canvas as the window's content view **before** the window is put
    // on screen, so the very first frame the compositor shows is already the
    // library-painted one. Making the window key/ordered front with a plain
    // content view would flash an empty white client area first.
    //
    // The canvas carries `widget_id` and the window flag, so `invalidate_surface`
    // for this window id can mark the content view and `drawRect:` paints the
    // window's own chrome plus every child that implements `Draw`.
    let canvas =
        RustWidgetsObjc2CanvasView::new(mtm, widget_id, true, make_rect(0, 0, width, height));
    window.setContentView(Some(&canvas));
    window.makeKeyAndOrderFront(None);
    window
}

// ---------------------------------------------------------------------------
// Surface lifecycle
// ---------------------------------------------------------------------------

/// Mounts a canvas view for `id` on the window registered under `parent`.
///
/// Creates [`RustWidgetsObjc2CanvasView`] sized to `rect`, adds it to the parent
/// window's `contentView`, records it in [`SURFACE_VIEWS`] so later
/// resize/invalidate/unmount calls can reach it, and marks it for display.
///
/// # Return value
///
/// `false` when there is no parent window under `parent`, it has no content view,
/// the view cannot be allocated, or the call runs off the main thread.
pub(crate) fn mount_surface_native(
    parent: u64,
    id: u64,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> bool {
    let Some(mtm) = MainThreadMarker::new() else {
        log::error!(
            "[macos-objc2] mount_surface: refused off the AppKit main thread (parent={parent}, id={id})"
        );
        return false;
    };
    // Resolve the parent window through the registry `create_ns_window` populated.
    //
    // The registry is typed as an opaque `*mut c_void`, and it also holds non-window views (surfaces
    // store their canvas there too). The runtime class check keeps a parent id that names a plain
    // view — rather than a window — from being reinterpreted as an `NSWindow`, which would be a type
    // confusion the compiler cannot catch on an erased pointer.
    let parent_ptr = match NATIVE_VIEWS.lock().unwrap().get(&parent).copied() {
        Some(ptr) if unsafe { object_is_window(ptr.0 as *mut AnyObject) } => ptr.0 as *mut NSWindow,
        Some(_) => {
            log::error!(
                "[macos-objc2] mount_surface: parent {parent} is not a window; a surface is mounted on\
                 a window's content view"
            );
            return false;
        }
        None => {
            log::error!("[macos-objc2] mount_surface: unknown parent window {parent}");
            return false;
        }
    };
    // SAFETY: the registry only ever stores objects `create_ns_window` made, and
    // the main-thread guard above satisfies AppKit's threading requirement.
    let parent_window: &NSWindow = unsafe { &*parent_ptr };
    let Some(content_view) = parent_window.contentView() else {
        log::error!("[macos-objc2] mount_surface: window {parent} has no content view");
        return false;
    };

    let frame = make_rect(x, y, width, height);
    let canvas = RustWidgetsObjc2CanvasView::new(mtm, id, false, frame);
    content_view.addSubview(&canvas);
    canvas.setNeedsDisplay(true);
    // The view is being revealed, which is the platform repainting a surface the
    // library just mounted; announce it like every other platform redraw.
    crate::notify_native_redraw(id, None);

    SURFACE_VIEWS
        .lock()
        .unwrap()
        .insert(id, NativePtr(Retained::as_ptr(&canvas) as *mut std::ffi::c_void));
    // The content view retains the canvas as a subview, so dropping the local
    // `Retained` here leaves the AppKit-owned reference intact.
    drop(canvas);
    log::debug!("[macos-objc2] mount_surface: id={id} mounted at ({x}, {y}, {width}, {height})");
    true
}

/// Resizes the mounted surface `id` and asks AppKit to redraw it.
///
/// Returns `false` when `id` has no mounted surface or the call is off-main.
pub(crate) fn resize_surface_native(id: u64, x: i32, y: i32, width: u32, height: u32) -> bool {
    if MainThreadMarker::new().is_none() {
        log::error!("[macos-objc2] resize_surface: refused off the AppKit main thread");
        return false;
    }
    let Some(ptr) = surface_view(id) else {
        log::error!("[macos-objc2] resize_surface: id={id} is not mounted");
        return false;
    };
    // SAFETY: `ptr` was produced by `mount_surface_native` and is only removed by
    // `unmount_surface_native`; `setFrame:` and `setNeedsDisplay:` are NSView API.
    unsafe {
        let view: &NSView = &*(ptr as *const NSView);
        // `setNeedsDisplay:` after `setFrame:`: AppKit does not schedule a display
        // pass for a frame change on its own, so a bare `setFrame:` leaves the old
        // pixels on screen until an unrelated invalidation arrives.
        view.setFrame(make_rect(x, y, width, height));
        view.setNeedsDisplay(true);
    }
    true
}

/// Marks the mounted surface `id` as needing display.
///
/// Returns `false` when `id` has no mounted surface or the call is off-main.
pub(crate) fn invalidate_surface_native(id: u64) -> bool {
    if MainThreadMarker::new().is_none() {
        log::error!("[macos-objc2] invalidate_surface: refused off the AppKit main thread");
        return false;
    }
    let Some(ptr) = surface_view(id) else {
        return false;
    };
    // SAFETY: `ptr` came from the side table populated by `mount_surface_native`;
    // `setNeedsDisplay:` is a valid NSView selector.
    unsafe {
        let view: &NSView = &*(ptr as *const NSView);
        view.setNeedsDisplay(true);
    }
    true
}

/// Marks a window's content view as needing display.
///
/// A repaint request for a **window** id reaches the content view, which is what
/// draws that window's children. Returns `false` for an id with no native window.
pub(crate) fn invalidate_window_native(id: u64) -> bool {
    if MainThreadMarker::new().is_none() {
        log::error!("[macos-objc2] invalidate_window: refused off the AppKit main thread");
        return false;
    }
    let window_ptr = match NATIVE_VIEWS.lock().unwrap().get(&id).copied() {
        Some(ptr) => ptr.0 as *mut NSWindow,
        None => return false,
    };
    // SAFETY: the registry only stores objects `create_ns_window` made; the
    // main-thread guard above satisfies AppKit's threading requirement.
    unsafe {
        let window: &NSWindow = &*window_ptr;
        if let Some(content_view) = window.contentView() {
            content_view.setNeedsDisplay(true);
        }
    }
    true
}

/// Removes the mounted surface `id` from its window.
///
/// Returns `false` when `id` has no mounted surface or the call is off-main.
pub(crate) fn unmount_surface_native(id: u64) -> bool {
    if MainThreadMarker::new().is_none() {
        log::error!("[macos-objc2] unmount_surface: refused off the AppKit main thread");
        return false;
    }
    let removed = SURFACE_VIEWS.lock().unwrap().remove(&id);
    let Some(ptr) = removed else {
        log::error!("[macos-objc2] unmount_surface: id={id} is not mounted");
        return false;
    };
    // SAFETY: the side-table entry was removed just above, so the view has no
    // other owner in this module and detaching it from its superview is the
    // correct teardown.
    unsafe {
        let view: &NSView = &*(ptr.0 as *const NSView);
        view.removeFromSuperview();
    }
    true
}

/// Looks up the canvas view pointer for a mounted surface.
fn surface_view(id: u64) -> Option<*mut NSView> {
    SURFACE_VIEWS.lock().unwrap().get(&id).map(|ptr| ptr.0 as *mut NSView)
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
