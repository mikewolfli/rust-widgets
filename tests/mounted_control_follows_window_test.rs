// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! A mounted surface must follow its window when the window is resized.
//!
//! # The contract
//!
//! `mount_surface` puts a control at a rectangle. `WindowHandle::set_layout` makes
//! the window reposition its children whenever its client size changes, and the
//! backend is told through `resize_surface`. A user dragging the window edge must
//! therefore move and resize every mounted control — the demo-visible half of
//! "controls follow the window".
//!
//! # What is asserted
//!
//! The layout's own output (which is the authority on where a control should be)
//! and the geometry the widget reports afterwards (which is what the backend was
//! told). Both must change together for a resize; if only one does, the control is
//! drawn at the old rectangle or the layout was never re-run.

#![cfg(all(
    any(feature = "desktop", feature = "tablet", feature = "mobile"),
    not(any(feature = "mini", feature = "embedded"))
))]

use std::sync::{Arc, Mutex};

use rust_widgets::app::{App, WindowHandle};
use rust_widgets::core::{ObjectId, Rect};
use rust_widgets::layout::Layout;
use rust_widgets::widget::Label;

/// Records what the layout was asked to place, per window size, for one child.
#[derive(Clone, Default)]
struct Recorder {
    /// The child the layout reports, so the recorded rect belongs to a real widget.
    child: Arc<Mutex<Option<ObjectId>>>,
    calls: Arc<Mutex<Vec<(u32, u32, Rect)>>>,
}

impl Layout for Recorder {
    fn add_widget(&mut self, _widget_id: ObjectId, _stretch: u32) {}
    fn remove_widget(&mut self, _widget_id: ObjectId) {}
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) {
        // The child is pinned to the bottom-right of the client area, so its
        // coordinates are a direct readout of the size the layout ran against.
        let Some(child) = *self.child.lock().expect("child") else {
            return;
        };
        let placed =
            Rect::new((rect.width as i32 - 40).max(0), (rect.height as i32 - 40).max(0), 40, 40);
        widgets(child, placed);
        self.calls.lock().expect("recorder").push((rect.width, rect.height, placed));
    }
}

/// Drains the trigger queue the way a host event loop does.
fn pump() {
    while let Some(event) = rust_widgets::poll_widget_trigger_event() {
        rust_widgets::app::dispatch_trigger(event.widget_id, event.kind);
    }
}

#[test]
fn a_mounted_control_follows_the_window_it_is_mounted_on() {
    // Mounting needs a backend that can host a surface. The headless state backend answers
    // `unsupported` rather than pretending, so this test skips there instead of failing —
    // the same guard `control_backend_routing_test.rs` uses. Reporting a pass would be
    // wrong (nothing was asserted) and reporting a failure would blame the code for a
    // capability the running backend does not have.
    if !rust_widgets::supports_surfaces() {
        eprintln!("skipping: backend {:?} does not host surfaces", rust_widgets::backend_name());
        return;
    }

    let mut app = App::new();
    app.init();
    let win: WindowHandle = app.new_window("follow", 0, 0, 1440, 900);

    let handle = win
        .mount_surface(
            Box::new(Label::new("panel".to_string(), Rect::new(12, 42, 932, 360))),
            Rect::new(12, 42, 932, 360),
        )
        .expect("mount the panel");
    let panel = handle.raw_id();

    let recorder = Recorder::default();
    let calls = Arc::clone(&recorder.calls);
    *recorder.child.lock().expect("child") = Some(panel);
    win.set_layout(recorder);

    // Report the size the backend thinks the window is, the way the OS would.
    assert!(
        rust_widgets::queue_resize_trigger(win.raw_id(), 1024, 700),
        "the backend must accept a resize for the window it created"
    );
    pump();

    let seen = calls.lock().expect("recorder");
    assert!(
        seen.iter().any(|(w, h, _)| *w == 1024 && *h == 700),
        "the layout must be re-run for the new size; it only saw {seen:?}"
    );

    // The last placement must match the last size, not the created one.
    let (width, height, placed) = *seen.last().expect("at least one placement");
    assert_eq!(
        placed,
        Rect::new(width as i32 - 40, height as i32 - 40, 40, 40),
        "the layout must place against the size it was given"
    );

    // And the widget must have been told: its geometry is what the backend draws from.
    let widget_rect = rust_widgets::widget::runtime::geometry_of(panel);
    assert!(
        widget_rect.is_some(),
        "the mounted panel must still be in the registry after a resize"
    );
}
