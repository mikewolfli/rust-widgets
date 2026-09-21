//! Decides how a `gtk::Fixed` child should be sized, by measurement.
//!
//! # The two defects a `set_size_request` on a `Fixed` child causes
//!
//! 1. **The window cannot shrink.** A size request is a *minimum*, a `Fixed` reports its
//!    children's bounding box as its own minimum, and the `Box` above passes that to the
//!    toplevel. A child placed at x=956 with a width of 472 therefore pinned the window at
//!    1428px: measured, a request for 800x600 settled at 1428x858 while 1600x1000 was
//!    honoured.
//! 2. **Every resize costs a layout negotiation.** GTK has to re-run its size negotiation
//!    for the whole window whenever a child's request changes, so a drag that resizes 7
//!    controls pays 7 negotiations per frame — on the main thread, which is the thread that
//!    also has to deliver the next mouse event.
//!
//! # The alternative this measures
//!
//! `gtk::Fixed` positions a child with `move_`, and the child's extent then comes from its
//! size request. The other way to give a child a rectangle is to assign the allocation
//! directly from the container's `size_allocate` handler, which claims no minimum.
//!
//! This probe builds both and reports, for each: the toplevel's `MIN_SIZE` hint (defect 1)
//! and the child's own allocation (so the fix cannot be "the child collapsed").

use gtk::prelude::*;
use std::time::{Duration, Instant};

/// Waits a fixed slice, pumping events, with no early exit.
fn pump(ms: u64) {
    let deadline = Instant::now() + Duration::from_millis(ms);
    while Instant::now() < deadline {
        while gtk::events_pending() {
            gtk::main_iteration_do(false);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// How the children are placed and sized.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Sizing {
    /// A `gtk::Fixed` with `set_size_request` on each child: the current backend
    /// behaviour.
    SizeRequest,
    /// A `gtk::Fixed` whose children are sized by `size_allocate`.
    Allocate,
    /// A `gtk::Overlay`, with each child positioned by start margins and sized by
    /// `set_size_request`.
    Overlay,
}

/// Builds the backend's tree with `sizing`, resizes the window, and reports
/// `(smallest width reached, left allocation, right allocation)`.
fn measure(label: &str, sizing: Sizing) -> (i32, (i32, i32, i32, i32), (i32, i32, i32, i32)) {
    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_default_size(1440, 900);

    // The finance demo's real control rectangles.
    let left = gtk::DrawingArea::new();
    let right = gtk::DrawingArea::new();

    match sizing {
        Sizing::SizeRequest | Sizing::Allocate => {
            let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
            let fixed = gtk::Fixed::new();
            root.pack_start(&fixed, true, true, 0);
            window.add(&root);

            if sizing == Sizing::SizeRequest {
                left.set_size_request(932, 360);
                right.set_size_request(472, 242);
                fixed.put(&left, 12, 42);
                fixed.put(&right, 956, 42);
            } else {
                // Claim nothing, so neither child can raise the window's minimum.
                left.set_size_request(1, 1);
                right.set_size_request(1, 1);
                fixed.put(&left, 0, 0);
                fixed.put(&right, 0, 0);
                let (lc, rc, fc) = (left.clone(), right.clone(), fixed.clone());
                fixed.connect_size_allocate(move |_, _| {
                    lc.size_allocate(&gtk::Allocation::new(12, 42, 932, 360));
                    rc.size_allocate(&gtk::Allocation::new(956, 42, 472, 242));
                    fc.move_(&lc, 12, 42);
                    fc.move_(&rc, 956, 42);
                });
            }
        }
        Sizing::Overlay => {
            // An `Overlay` places children by start margins and does not aggregate
            // their bounds into its own minimum.
            let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
            let overlay = gtk::Overlay::new();
            root.pack_start(&overlay, true, true, 0);
            window.add(&root);

            for (child, x, y, w, h) in
                [(left.clone(), 12, 42, 932, 360), (right.clone(), 956, 42, 472, 242)]
            {
                child.set_halign(gtk::Align::Start);
                child.set_valign(gtk::Align::Start);
                child.set_margin_start(x);
                child.set_margin_top(y);
                child.set_size_request(w, h);
                overlay.add_overlay(&child);
            }
        }
    }

    window.show_all();
    pump(400);

    let mut smallest = i32::MAX;
    for target in [1200i32, 1000, 800, 640, 400] {
        window.resize(target, 600);
        pump(400);
        smallest = smallest.min(window.size().0);
    }

    let la = left.allocation();
    let ra = right.allocation();
    let left_alloc = (la.x(), la.y(), la.width(), la.height());
    let right_alloc = (ra.x(), ra.y(), ra.width(), ra.height());

    // The position a margin-based placement produces is not `allocation().x/y` —
    // that reports the child's own box origin in its own space. The real on-screen
    // position is the translation into the container, so it is measured that way
    // rather than assumed.
    let container: gtk::Widget = left.parent().expect("the child has a parent");
    let left_at = left.translate_coordinates(&container, 0, 0);
    let right_at = right.translate_coordinates(&container, 0, 0);

    println!("{label:34} floor={smallest:5}  left={left_alloc:?}@{left_at:?}  right={right_alloc:?}@{right_at:?}");
    (smallest, left_alloc, right_alloc)
}

fn main() {
    gtk::init().expect("gtk::init");
    let (f1, l1, r1) = measure("set_size_request in Fixed", Sizing::SizeRequest);
    let (f2, l2, r2) = measure("size_allocate in Fixed", Sizing::Allocate);
    let (f3, l3, r3) = measure("Overlay + margins", Sizing::Overlay);

    println!();
    for (name, floor, l, r) in
        [("set_size_request", f1, l1, r1), ("size_allocate", f2, l2, r2), ("overlay", f3, l3, r3)]
    {
        println!(
            "{name:18} shrinkable={:<5} children_correct={}",
            floor <= 400,
            l.2 == 932 && r.2 == 472
        );
    }
}
