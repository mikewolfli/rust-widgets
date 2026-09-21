// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Absolute child placement for the Linux/GTK backend, on a `gtk::Overlay`.
//!
//! # Why this module exists
//!
//! The backend positions every control itself: the window layout computes a `Rect` per
//! widget and the backend hands that rectangle to GTK. The obvious GTK container for
//! absolute placement is `gtk::Fixed`, and that is what the backend used.
//!
//! It cannot be used. A `Fixed` reports the **bounding box of its children** as its own
//! minimum size, and the `Box` above it passes that up, so the toplevel adopted the
//! right-most control's offset as its minimum width: a control placed at x=956 pinned the
//! window at 1428px, so it could be enlarged but never shrunk. Measured on the exact
//! backend tree:
//!
//! | placement | smallest window width reached |
//! |---|---|
//! | `Fixed` + child `set_size_request` | 1428 |
//! | `Fixed` + child sized by `size_allocate` | 957 |
//! | **`Overlay` + start margins** | **400** |
//!
//! Sizing the children by `size_allocate` only lowers the floor to the right child's `put`
//! origin, which still refuses a smaller window. An `Overlay` does not aggregate its
//! children's bounds, and the children keep their exact sizes and positions: verified with
//! `translate_coordinates`, which reported them at (12, 42) and (956, 42) — the same
//! placement a `Fixed` produced.
//!
//! See `tools/fixed_alloc/src/main.rs` for the measurement.
//!
//! # Why one module rather than a call at each site
//!
//! Window creation, mounting and resizing all place children, and all three must agree: a
//! control placed one way at mount and another after the first resize would jump. Keeping
//! the rule in one place is what makes that impossible.

#![cfg(all(target_os = "linux", feature = "gtk-native"))]

use gtk::prelude::*;

/// Adds `child` to `overlay` at `(x, y)` with size `(width, height)`.
///
/// The position is a pair of start margins rather than a container offset, because an
/// `Overlay` positions by container properties — there is no `put`. Sizing is
/// `set_size_request`, which is a minimum: safe here precisely because an overlay does not
/// pass a child's minimum up to the window, which is the defect this module exists to
/// avoid.
///
/// Each dimension is floored at 1. GTK allocates nothing for a zero-sized child, so a
/// control asked to occupy a degenerate rectangle stays a one-pixel control rather than
/// disappearing — a state that can be diagnosed, unlike an absent one.
pub(crate) fn place(
    overlay: &gtk::Overlay,
    child: &impl IsA<gtk::Widget>,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) {
    let widget = child.as_ref();
    // Align to the start so the margins are the whole offset: the default (`Fill`) lets
    // the overlay stretch the child to the overlay's size and ignores the margins.
    widget.set_halign(gtk::Align::Start);
    widget.set_valign(gtk::Align::Start);
    widget.set_margin_start(x);
    widget.set_margin_top(y);
    widget.set_size_request(width.max(1) as i32, height.max(1) as i32);
    overlay.add_overlay(widget);
}

/// Moves and resizes a child already placed by [`place`].
///
/// The overlay keeps the child, so only the position and size change.
/// `set_margin_start`/`set_margin_top` are the position and `set_size_request` the size,
/// matching [`place`] exactly — the two must agree or a control would sit at one rectangle
/// when mounted and another after the first resize.
pub(crate) fn reposition(child: &impl IsA<gtk::Widget>, x: i32, y: i32, width: u32, height: u32) {
    let widget = child.as_ref();
    widget.set_margin_start(x);
    widget.set_margin_top(y);
    widget.set_size_request(width.max(1) as i32, height.max(1) as i32);
    widget.queue_resize();
}
