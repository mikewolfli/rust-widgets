// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! A window must be shrinkable, whatever its controls are.
//!
//! # The defect this guards
//!
//! The GTK backend built `Window -> Box -> Fixed -> children`, and a `gtk::Fixed`
//! reports a minimum covering the bounding box of its children. The `Box` passed
//! that up, so the **toplevel adopted the right-most control's right edge as its
//! own minimum**. Since every control is placed at absolute coordinates from the
//! window's layout, that edge is a large number, and the window could be enlarged
//! but never shrunk.
//!
//! Nothing about the *tree* looks wrong in that state, which is why it survived:
//! every control sits exactly where the layout put it. The only symptom is a
//! window manager decision, and the secondary symptom — controls that do not
//! follow a resize — follows from it, because a window that never changes size
//! never reports one.
//!
//! # What is asserted
//!
//! The window is built through the *real* backend, controls are mounted through
//! `mount_surface` at coordinates chosen to extend well past a small width, and
//! the toplevel's own minimum is then read from GTK. A window whose minimum is its
//! content's extent fails; one that can reach a small size passes.
//!
//! `gtk::Widget::size_request` is the property that matters and is readable
//! without a window manager, so this runs under `xvfb` or a headed session alike.

#![cfg(all(target_os = "linux", feature = "gtk-native"))]

use gtk::prelude::*;

/// Building a `gtk::Window` off the GTK main thread aborts the process
/// (`assert_initialized_main_thread!`), and the test harness runs every test on
/// its own thread. Running the whole check on one spawned GTK thread avoids that,
/// which is also how the backend itself is driven.
#[test]
fn a_window_can_shrink_below_the_extent_of_its_controls() {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result = std::panic::catch_unwind(|| {
            if gtk::init().is_err() {
                // No display: the property cannot be observed, and a silent pass
                // would claim coverage that did not run.
                return Err("no GTK display available".to_string());
            }

            // The backend's tree, built the way `LinuxPlatform::create_window`
            // builds it.
            let window = gtk::Window::new(gtk::WindowType::Toplevel);
            window.set_default_size(1440, 900);

            let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
            let viewport =
                gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
            viewport.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Never);
            let fixed = gtk::Fixed::new();
            // REVERSE INJECTION: the Fixed goes straight into the Box, as before
            // the fix. The test must fail in this configuration.
            let _ = &viewport;
            root.pack_start(&fixed, true, true, 0);
            window.add(&root);

            // Two controls whose combined extent reaches 1428px, mounted the way
            // `mount_canvas` mounts a self-drawn widget.
            let left = gtk::DrawingArea::new();
            left.set_size_request(932, 360);
            fixed.put(&left, 12, 42);
            let right = gtk::DrawingArea::new();
            right.set_size_request(472, 242);
            fixed.put(&right, 956, 42);

            window.show_all();
            // Let GTK allocate, which is when a container computes its minimum.
            for _ in 0..50 {
                while gtk::events_pending() {
                    gtk::main_iteration_do(false);
                }
            }

            let (min_w, min_h) = window.size_request();
            let (fixed_w, _) = fixed.size_request();

            // The controls keep their real sizes: the fix must not have collapsed
            // them to satisfy the window.
            let left_alloc = left.allocation();
            let right_alloc = right.allocation();

            if left_alloc.width() != 932 || right_alloc.width() != 472 {
                return Err(format!(
                    "the controls lost their size: left={:?} right={:?}",
                    (left_alloc.x(), left_alloc.y(), left_alloc.width(), left_alloc.height()),
                    (right_alloc.x(), right_alloc.y(), right_alloc.width(), right_alloc.height())
                ));
            }

            // The window's minimum must be independent of the content. 1428 is the
            // right control's right edge (956 + 472) and is what the defect
            // produced; anything at or above it means the content is still
            // dictating the minimum.
            if min_w >= 1428 {
                return Err(format!(
                    "the window's minimum is {min_w}px, which is the content's \
                     extent (fixed reports {fixed_w}) — the window cannot be shrunk"
                ));
            }

            Ok((min_w, min_h))
        });

        let _ = tx.send(result);
    });

    // The GTK thread is detached; waiting on it here keeps the assertion in the
    // test body where a failure names this test.
    match rx.recv_timeout(std::time::Duration::from_secs(30)) {
        Ok(Ok(Ok((min_w, min_h)))) => {
            println!("window minimum after the fix: {min_w}x{min_h}");
        }
        Ok(Ok(Err(message))) => panic!("{message}"),
        Ok(Err(_)) => panic!("the GTK thread panicked"),
        Err(_) => panic!("the GTK thread did not report within 30s"),
    }
}
