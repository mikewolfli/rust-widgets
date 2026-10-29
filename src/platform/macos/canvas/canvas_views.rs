// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Canvas-view bookkeeping for the cocoa (legacy) backend.
//!
//! Two concerns live here, both keyed by an `NSView` identity rather than by widget state:
//!
//! * the **window-canvas association** — the flag that marks a content view as painting the
//!   whole widget tree, and the `host id -> view` side table a repaint request resolves
//!   through;
//! * the **reference ownership rules** for a canvas view. `mount_surface_impl` hands the
//!   `alloc` reference to the superview and takes the side table's own reference;
//!   `unmount_surface_impl` detaches and releases that reference. Keeping both halves of
//!   that pair on one helper is what stops them drifting (PF-X03).
//!
//! Split out of `canvas.rs` so that file stays within the production-line limit
//! (principle #9); the behaviour is unchanged.

use super::{objc_getAssociatedObject, objc_setAssociatedObject, OBJC_ASSOCIATION_ASSIGN};
use crate::core::ObjectId;
use cocoa::base::{id, nil};
use cocoa::foundation::NSString;
use objc::{msg_send, sel, sel_impl};
use std::collections::HashMap;
use std::os::raw::c_void;
use std::sync::{Mutex, OnceLock};

/// The association key marking a canvas as a **window** canvas.
///
/// Its presence means "this view is a window's content view, so `drawRect:` paints the whole
/// widget tree" rather than one mounted box. Carried as an associated object for the same reason
/// the widget id is: a plain `NSView` subclass is not KVC-compliant and `setValue:forKey:` would
/// raise.
pub(super) fn window_flag_key() -> id {
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
pub(super) unsafe fn set_window_canvas(view: id) {
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
pub(super) unsafe fn is_window_canvas(view: id) -> bool {
    let value = objc_getAssociatedObject(view as *mut c_void, window_flag_key() as *const c_void);
    !value.is_null()
}

/// Side table from a **host** window id to its canvas content view.
///
/// Kept separate from [`mounted_views`]: a window canvas is the window's own content view, not a
/// mounted surface, so `resize_surface`/`unmount_surface` (which are keyed by surface id) must not
/// reach it. Only the invalidation path looks here.
pub(super) fn window_canvases() -> &'static Mutex<HashMap<ObjectId, usize>> {
    static VIEWS: OnceLock<Mutex<HashMap<ObjectId, usize>>> = OnceLock::new();
    VIEWS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The window canvas that should repaint `id`.
///
/// `id` may be a window's widget-registry id (what `request_repaint_subtree` names for the window)
/// or a child control's id (which has no native view of its own). Both resolve, through the
/// recorded host association, to the window whose canvas paints the whole tree.
pub(super) fn window_canvas_for(id: ObjectId) -> Option<id> {
    let host = crate::widget::runtime::host_window_for(id).or_else(|| {
        crate::widget::runtime::top_level_window_of(id)
            .and_then(crate::widget::runtime::host_window_for)
    })?;
    let table = crate::compat::lock(window_canvases());
    table.get(&host).map(|ptr| *ptr as id)
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
pub(super) unsafe fn retain_canvas_view(view: id) {
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
