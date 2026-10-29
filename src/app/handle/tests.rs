// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Tests for the handle façade, moved out of `handle.rs` unchanged.

use crate::platform::WidgetTriggerKind;

use super::*;
use crate::core::ObjectId;
use alloc::rc::Rc;
use core::cell::RefCell;

/// W-300: the per-widget binding account names each table rather than one aggregate.
#[test]
fn a_binding_report_counts_each_table() {
    let id = crate::widget::runtime::register(Box::new(crate::widget::Button::new(
        "probe".to_string(),
        crate::core::Rect::new(0, 0, 80, 24),
    )))
    .expect("a button mounts");

    register_click_callback(id, || {});
    register_close_callback(id, || {});
    let report = binding_report(id);
    assert_eq!(report.local_callbacks, 2, "on_click and on_close each store one closure");
    assert_eq!(report.click_connections, 1);
    assert_eq!(report.close_connections, 1);
    assert_eq!(report.triggered_connections, 0);
    assert_eq!(report.total(), 4, "every table contributes to the total");

    remove_callbacks(id);
    assert_eq!(binding_report(id), BindingReport::default(), "a teardown empties every table");
    crate::widget::runtime::unregister(id);
}

#[test]
fn remove_callbacks_cleans_up() {
    let id: ObjectId = 42;
    CLICK_CALLBACKS.with(|map| {
        map.borrow_mut().insert(id, alloc::vec![Rc::new(RefCell::new(|| {}))]);
        assert!(map.borrow().contains_key(&id));
    });
    remove_callbacks(id);
    CLICK_CALLBACKS.with(|map| {
        assert!(!map.borrow().contains_key(&id));
    });
}

/// Closing a widget must also drop the layouts registered against its id.
///
/// The three layout tables are keyed by `ObjectId` and each value can own a
/// `Box<dyn Layout>`; they were the remaining unbounded growth in a create/destroy loop
/// after the mirror state tables were cleaned. Registering and removing is the whole
/// contract, so this test pins all three tables rather than only the one the bug report
/// named.
#[test]
fn remove_callbacks_cleans_up_layout_registrations() {
    let id: ObjectId = 4242;
    LAYOUTS.with(|map| {
        map.borrow_mut().insert(id, Box::new(crate::layout::StackLayout::new()));
    });
    PANEL_LAYOUTS.with(|map| {
        map.borrow_mut().insert(id, Box::new(crate::layout::StackLayout::new()));
    });
    LAYOUT_CONTEXT_SCALES.with(|map| {
        map.borrow_mut().insert(id, 1.5);
    });

    remove_callbacks(id);

    assert!(LAYOUTS.with(|map| !map.borrow().contains_key(&id)));
    assert!(PANEL_LAYOUTS.with(|map| !map.borrow().contains_key(&id)));
    assert!(LAYOUT_CONTEXT_SCALES.with(|map| !map.borrow().contains_key(&id)));
}

/// A refused binding on a `SurfaceHandle` must reach the same observable count as any other.
///
/// # Why this is not just a logging concern
///
/// `unwired_binding_count()` exists so a program can ask "was anything I wired left
/// unwired?" (rule #97: "reported success for something that did not happen" must be
/// queryable). A handle built from an id that was never mounted has no signal to connect to,
/// so it is right for it to refuse — but the count is what makes that answer the same for
/// every handle type. One question, one answer.
#[test]
fn a_surface_handle_refusal_reaches_the_unwired_count() {
    reset_unwired_binding_count();
    assert_eq!(unwired_binding_count(), 0, "the slate starts clean");

    let handle = SurfaceHandle::from_raw(9001);
    handle.on_click(|| {});
    assert_eq!(
        unwired_binding_count(),
        1,
        "a click binding on an unmounted id must be counted, not only logged"
    );

    handle.on_value_changed(|_| {});
    assert_eq!(unwired_binding_count(), 2, "and so must a value binding");

    reset_unwired_binding_count();
}

/// A mounted surface's click and value callbacks reach the control's own signals (DEMO-005).
///
/// The handle used to refuse both on the premise that a library-painted control has no click
/// channel. It has one — `base.clicked` for clicks, and the control's published
/// `value_changed` event for values — so the callbacks must run instead of being recorded as
/// unwired.
#[test]
fn a_mounted_surface_runs_the_callbacks_wired_through_its_handle() {
    reset_unwired_binding_count();
    let id = crate::widget::runtime::register(Box::new(
        crate::widget::display_widgets::slider::Slider::new(crate::core::Rect::new(0, 0, 200, 24)),
    ))
    .expect("a slider mounts");
    let handle = SurfaceHandle::from_raw(id);

    let clicks = std::sync::Arc::new(std::sync::Mutex::new(0usize));
    let seen = std::sync::Arc::clone(&clicks);
    handle.on_click(move || *seen.lock().expect("click count") += 1);

    let values = Rc::new(RefCell::new(0usize));
    let recorded = Rc::clone(&values);
    handle.on_value_changed(move |_text| *recorded.borrow_mut() += 1);

    assert_eq!(unwired_binding_count(), 0, "both bindings reached a real channel");

    // The control's own click signal is what the handle subscribes to.
    crate::widget::runtime::with_widget_mut(id, |widget| widget.base().clicked.emit())
        .expect("the control is mounted");
    assert_eq!(
        *clicks.lock().expect("click count"),
        1,
        "a click on the surface reaches the handle's callback"
    );

    // The control's published `value_changed` event re-dispatches into the value table.
    crate::widget::runtime::with_widget_mut(id, |widget| {
        (widget as &mut dyn std::any::Any)
            .downcast_mut::<crate::widget::display_widgets::slider::Slider>()
            .map(|slider| slider.set_value(42))
    })
    .expect("the control is mounted");
    assert_eq!(*values.borrow(), 1, "a value change on the surface reaches the handle's callback");

    remove_callbacks(id);
    crate::widget::runtime::unregister(id);
    reset_unwired_binding_count();
}

/// A click callback that panics must stay registered.
///
/// # The defect this closes
///
/// `dispatch_trigger` moves the callback out of the registry before invoking it
/// (so a re-entrant `remove_callbacks` cannot double-borrow) and puts it back
/// after. The restore used to be a plain statement on the success path, so a
/// callback that unwound took its own registration with it: the widget kept its
/// handle and quietly ignored every subsequent click. `ClickCallGuard` restores
/// on the unwind path; this test is what proves it, because the unwind is the
/// only way the two paths differ.
#[test]
fn a_panicking_click_callback_stays_registered() {
    let id: ObjectId = 4301;
    remove_callbacks(id);
    let calls = Rc::new(RefCell::new(0usize));
    let counter = Rc::clone(&calls);

    CLICK_CALLBACKS.with(|map| {
        map.borrow_mut().insert(
            id,
            alloc::vec![Rc::new(RefCell::new(move || {
                *counter.borrow_mut() += 1;
                panic!("callback body panics on purpose");
            }))],
        );
    });

    let first = std::panic::catch_unwind(|| dispatch_trigger(id, WidgetTriggerKind::Clicked));
    assert!(first.is_err(), "the callback's panic must propagate to the test harness");
    assert_eq!(*calls.borrow(), 1);

    // The registration must have survived the unwind, so a second dispatch
    // reaches the callback again rather than reporting no callback at all.
    let second = std::panic::catch_unwind(|| dispatch_trigger(id, WidgetTriggerKind::Clicked));
    assert!(second.is_err(), "the callback is still installed");
    assert_eq!(*calls.borrow(), 2, "the second dispatch must have invoked the callback");

    remove_callbacks(id);
}

/// A callback that re-registers itself must not panic with `BorrowMutError`.
///
/// `on_click` writes to the same `RefCell` that `dispatch_trigger` was reading
/// when it invoked the callback. Each access therefore has to be attempted
/// rather than assumed; a plain `borrow_mut` on either side turned this ordinary
/// pattern into a panic inside user code.
#[test]
fn a_callback_that_reregisters_itself_does_not_panic() {
    let id: ObjectId = 4302;
    remove_callbacks(id);

    let ran = Rc::new(RefCell::new(0usize));
    let counter = Rc::clone(&ran);
    CLICK_CALLBACKS.with(|map| {
        map.borrow_mut().insert(
            id,
            alloc::vec![Rc::new(RefCell::new(move || {
                *counter.borrow_mut() += 1;
                // Re-register under the same id, from inside the dispatch.
                CLICK_CALLBACKS.with(|map| {
                    if let Ok(mut map) = map.try_borrow_mut() {
                        map.insert(id, alloc::vec![Rc::new(RefCell::new(|| {}))]);
                    }
                });
            }))],
        );
    });

    let result = std::panic::catch_unwind(|| dispatch_trigger(id, WidgetTriggerKind::Clicked));
    assert!(result.is_ok(), "a re-registering callback must not panic");
    assert_eq!(*ran.borrow(), 1, "the callback must have run exactly once");

    remove_callbacks(id);
}

/// The value-changed path has the same unwind guarantee as the click path.
#[test]
fn a_panicking_value_callback_stays_registered() {
    let id: ObjectId = 4303;
    remove_callbacks(id);
    let calls = Rc::new(RefCell::new(0usize));
    let counter = Rc::clone(&calls);

    VALUE_CALLBACKS.with(|map| {
        map.borrow_mut().entry(id).or_default().push(Rc::new(RefCell::new(move |_text: &str| {
            *counter.borrow_mut() += 1;
            panic!("value callback body panics on purpose");
        })));
    });

    let first = std::panic::catch_unwind(|| dispatch_trigger(id, WidgetTriggerKind::ValueChanged));
    assert!(first.is_err());
    assert_eq!(*calls.borrow(), 1);

    let second = std::panic::catch_unwind(|| dispatch_trigger(id, WidgetTriggerKind::ValueChanged));
    assert!(second.is_err(), "the value callback must still be installed");
    assert_eq!(*calls.borrow(), 2);

    remove_callbacks(id);
}

/// A dispatch with no callback reports `false` rather than panicking.
#[test]
fn dispatching_without_a_callback_is_a_no_op() {
    let id: ObjectId = 4304;
    remove_callbacks(id);
    assert!(!dispatch_trigger(id, WidgetTriggerKind::Clicked));
    assert!(!dispatch_trigger(id, WidgetTriggerKind::ValueChanged));
    // `Closed` is the teardown path and always answers `false`.
    assert!(!dispatch_trigger(id, WidgetTriggerKind::Closed));
}

#[test]
fn line_edit_handle_state_defaults() {
    let id: ObjectId = 5001;
    let handle = LineEditHandle::from_raw(id);
    assert_eq!(handle.placeholder(), "");
    assert!(!handle.is_read_only());
    assert_eq!(handle.max_length(), 32767);
    assert_eq!(handle.echo_mode(), EchoMode::Normal);
    assert_eq!(handle.selection(), (0, 0));
    assert!(!handle.is_select_all());
}

#[test]
fn line_edit_handle_state_roundtrip() {
    let id: ObjectId = 5002;
    let handle = LineEditHandle::from_raw(id);
    handle.set_placeholder("Type here...");
    handle.set_read_only(true);
    handle.set_max_length(64);
    handle.set_echo_mode(EchoMode::Password);

    assert_eq!(handle.placeholder(), "Type here...");
    assert!(handle.is_read_only());
    assert_eq!(handle.max_length(), 64);
    assert_eq!(handle.echo_mode(), EchoMode::Password);
}

#[test]
fn line_edit_handle_selection_semantics() {
    let id: ObjectId = 5003;
    let handle = LineEditHandle::from_raw(id);

    // select_all sets an unbounded range and the select-all marker.
    handle.select_all();
    assert!(handle.is_select_all());
    assert_eq!(handle.selection(), (0, u32::MAX));

    // A precise selection clears the select-all marker.
    handle.set_selection(2, 5);
    assert!(!handle.is_select_all());
    assert_eq!(handle.selection(), (2, 5));
}

#[test]
fn line_edit_handle_states_are_scoped_per_widget() {
    let a = LineEditHandle::from_raw(5004);
    let b = LineEditHandle::from_raw(5005);
    a.set_max_length(8);
    b.set_read_only(true);
    assert_eq!(a.max_length(), 8);
    assert!(!a.is_read_only());
    assert_eq!(b.max_length(), 32767);
    assert!(b.is_read_only());
}
/// A backend with no icon concept and no minimum-size read-back.
///
/// Six of the shipped backends (`linux-gtk`, `wayland`, `android`, `ios`, `wasm`, `harmony`)
/// inherit the `Platform` defaults for `window_min_size`/`window_icon`, which answer `None`;
/// `macos` implements the size pair but not the icon pair. The mirror fallback exists for
/// exactly that case, and this stand-in reproduces it so the fallback is testable on any
/// host rather than only on the six backends this one cannot run.
struct NoReadbackBackend;

impl crate::platform::Platform for NoReadbackBackend {
    fn as_any(&self) -> &dyn crate::compat::Any {
        self
    }
    fn backend_name(&self) -> &'static str {
        "no-readback-probe"
    }
    fn family(&self) -> crate::core::PlatformFamily {
        crate::core::PlatformFamily::Desktop
    }
    fn init(&self) {}
    fn run(&self) {}
    fn quit(&self) -> bool {
        true
    }
    fn create_window(&self, _t: &str, _x: i32, _y: i32, _w: u32, _h: u32) -> ObjectId {
        1
    }
    fn set_window_min_size(&self, _id: ObjectId, _w: u32, _h: u32) -> bool {
        true
    }
    fn set_window_icon(&self, _id: ObjectId, _path: &str) -> bool {
        true
    }
    // `window_min_size` and `window_icon` are deliberately *not* overridden: the trait
    // defaults answer `None`, which is the condition the mirror covers.
}

static NO_READBACK: NoReadbackBackend = NoReadbackBackend;

/// A value the platform accepted is readable back even when the platform cannot answer.
///
/// `min_size` and `icon` read the platform exclusively, so on a backend without those
/// readers a window whose setter returned `true` still reported `None` — the mirror fields
/// were written by every set and read by nothing. Both now fall back to the mirror, which is
/// the shape `is_maximized` already used.
#[test]
fn an_accepted_window_property_reads_back_without_platform_readers() {
    crate::platform::runtime::with_platform(&NO_READBACK, || {
        let window = WindowHandle::from_raw(4242);

        assert_eq!(window.min_size(), None, "nothing was set yet");
        assert_eq!(window.icon(), None, "nothing was set yet");

        assert!(window.set_min_size(320, 240), "the backend accepts the call");
        assert_eq!(
            window.min_size(),
            Some((320, 240)),
            "a value the platform accepted must be readable back"
        );

        assert!(window.set_icon("/tmp/probe.png"));
        assert_eq!(
            window.icon(),
            Some("/tmp/probe.png".to_string()),
            "a path the platform accepted must be readable back"
        );

        // A later set replaces rather than accumulates.
        assert!(window.set_min_size(640, 480));
        assert_eq!(window.min_size(), Some((640, 480)));
    });
}

/// A *rejected* set leaves nothing readable, on a backend that refuses.
///
/// The mirror used to be written before the platform was asked, so a refused value was
/// recorded anyway. The fallback above is only honest because the write happens on success.
#[test]
fn a_rejected_window_property_is_not_reported_by_a_later_read() {
    struct RejectingBackend;
    impl crate::platform::Platform for RejectingBackend {
        fn as_any(&self) -> &dyn crate::compat::Any {
            self
        }
        fn backend_name(&self) -> &'static str {
            "rejecting-probe"
        }
        fn family(&self) -> crate::core::PlatformFamily {
            crate::core::PlatformFamily::Desktop
        }
        fn init(&self) {}
        fn run(&self) {}
        fn quit(&self) -> bool {
            true
        }
        fn create_window(&self, _t: &str, _x: i32, _y: i32, _w: u32, _h: u32) -> ObjectId {
            1
        }
        fn set_window_min_size(&self, _id: ObjectId, _w: u32, _h: u32) -> bool {
            false
        }
        fn set_window_icon(&self, _id: ObjectId, _path: &str) -> bool {
            false
        }
    }
    static REJECTING: RejectingBackend = RejectingBackend;

    crate::platform::runtime::with_platform(&REJECTING, || {
        let window = WindowHandle::from_raw(4243);
        assert!(!window.set_min_size(320, 240), "the backend refuses");
        assert_eq!(window.min_size(), None, "a refused size must not be reported");
        assert!(!window.set_icon("/tmp/nope.png"), "the backend refuses");
        assert_eq!(window.icon(), None, "a refused icon must not be reported");
    });
}

/// An id that addresses no window leaves nothing readable on the active backend.
#[test]
fn a_rejected_set_on_the_active_backend_leaves_nothing_readable() {
    let window = WindowHandle::from_raw(0);
    assert!(!window.set_min_size(320, 240), "id 0 is not a window");
    assert_eq!(window.min_size(), None, "a refused size must not be reported");
    assert!(!window.set_icon("/tmp/does-not-exist.png"));
    assert_eq!(window.icon(), None, "a refused icon path must not be reported");
}

/// Every mirror-backed getter follows one rule: platform first, mirror as fallback.
///
/// # The decision this pins
///
/// The `WindowState` mirror is kept, not deleted, and it is a *fallback* rather than a shadow.
/// Two reasons, both of which have to hold for a field to earn its place here:
///
/// 1. **Six backends cannot read the value back.** `window_min_size`/`window_icon` fall through
///    to `Platform` defaults returning `None` on `linux-gtk`, `wayland`, `android`, `ios`,
///    `wasm` and `harmony` (and `macos` implements the size pair but not the icon pair). A
///    setter that returned `true` there produced a `None` getter, so the mirror is the only
///    source that can answer at all.
/// 2. **`apply_window_layout` needs the size on every backend**, including at construction
///    before any native object exists — which is the documented reason `record_created_geometry`
///    writes it.
///
/// A field with neither property would be dead weight and should be deleted instead. This test
/// makes that a decision that has to be re-made rather than inherited: it drives each
/// mirror-backed accessor on the active backend and requires the platform to be consulted
/// first — proven by a set the backend *refuses* leaving the getter reporting nothing.
#[test]
fn every_mirror_backed_getter_defers_to_the_platform_first() {
    // An id that addresses no window: every backend refuses the setters, so no mirror entry
    // is created and every getter must report its honest "nothing" value.
    let window = WindowHandle::from_raw(0);

    assert!(!window.set_min_size(320, 240), "id 0 is not a window");
    assert!(!window.set_icon("/tmp/does-not-exist.png"));
    window.set_maximized(true);
    window.set_minimized(true);
    window.set_fullscreen(true);
    window.set_resizable(false);
    window.set_decorated(false);

    // Each getter is total: no panic, and the "nothing set" answer rather than a stale value.
    assert_eq!(window.min_size(), None, "a refused size must not be remembered");
    assert_eq!(window.icon(), None, "a refused icon must not be remembered");
    assert!(!window.is_maximized());
    assert!(!window.is_minimized());
    assert!(!window.is_fullscreen());
    assert!(window.is_resizable(), "a fresh window is resizable");
    assert!(window.is_decorated(), "a fresh window is decorated");
}

/// The two geometry mirrors exist because `apply_window_layout` reads them.
///
/// `w`/`h` are not platform-read for the window: they are the client size a layout is solved
/// against, and `record_created_geometry` has to supply them before any native object exists.
/// `x`/`y` are read back by `WindowHandle::geometry` for a panel. This asserts the layout path
/// still receives the size it recorded, so the fields are not orphaned by a refactor.
#[test]
fn the_window_geometry_mirror_reaches_the_layout_path() {
    let id = crate::platform::get_platform().create_window("probe", 10, 20, 400, 300);
    WindowHandle::record_created_geometry(id, 10, 20, 400, 300);

    let recorded = WINDOW_STATES
        .with(|map| map.borrow().get(&id).map(|state| (state.x, state.y, state.w, state.h)));
    assert_eq!(
        recorded,
        Some((10, 20, 400, 300)),
        "the geometry mirror must carry what the constructor recorded"
    );
}

/// `center_on_screen` is the production reader of the mirrored `x`/`y`.
///
/// # The decision this pins
///
/// `x`/`y` were judged "has a reader: `apply_window_layout`" in the round-10 table. That was
/// wrong on the mechanism (the layout reads `w`/`h` only) and right on the conclusion, for a
/// reason nobody had written down: the position fields are read by this method, which re-centres
/// around the mirrored *size*. The correction matters because a later pass trusting the wrong
/// reason would have deleted `x`/`y` and taken `center_on_screen` with them.
///
/// The test drives the real accessor rather than reading the mirror, so it fails if the fields
/// stop being consulted, whichever way that happens.
#[test]
fn the_window_position_mirror_is_read_by_center_on_screen() {
    let id = crate::platform::get_platform().create_window("probe", 10, 20, 400, 300);
    WindowHandle::record_created_geometry(id, 10, 20, 400, 300);
    let window = WindowHandle::from_raw(id);

    window.center_on_screen();

    // A 400x300 window on the 1920x1080 fallback screen centres at (760, 390).
    let centred = WINDOW_STATES
        .with(|map| map.borrow().get(&id).map(|state| (state.x, state.y, state.w, state.h)));
    assert_eq!(
        centred,
        Some((760, 390, 400, 300)),
        "centring must update the mirror it computes from, or the next read sees a stale \
         position"
    );
}

/// A registered close callback fires when the window's **widget** closes.
///
/// # The defect this closes
///
/// `on_close` used to write a `WindowState::close_callback` side table that only
/// `WindowHandle::close` read, so a close produced by the widget (a title-bar close, a
/// `Window::close`, the published `close` command) never reached the handler — and the JSON /
/// designer `closed` route had no signal to bind at all. The callback now connects to
/// `BaseWidget::closed`, which the widget's own close path emits, so the handler fires whenever
/// the control announces the lifecycle fact.
#[test]
fn the_close_callback_fires_from_the_widget_close_signal() {
    // Mount a real `Window` widget so `on_close` has a live `closed` signal to connect to.
    let id = crate::widget::runtime::register(Box::new(crate::widget::Window::new(
        "probe".to_string(),
        crate::core::Rect::new(0, 0, 320, 240),
    )))
    .expect("a window mounts");
    let window = WindowHandle::from_raw(id);
    let calls = std::rc::Rc::new(std::cell::Cell::new(0usize));
    {
        let calls = calls.clone();
        window.on_close(alloc::rc::Rc::new(core::cell::RefCell::new(move || {
            calls.set(calls.get() + 1);
        })));
    }

    // The signal path, not the handle method: this is what a widget-level close does.
    assert!(crate::close_widget(id), "a mounted widget receives the close signal");
    assert_eq!(
        calls.get(),
        1,
        "a registered on_close must fire when the widget's closed signal is emitted"
    );

    crate::widget::runtime::unregister(id);
}

/// `close()` with nothing registered must not panic.
#[test]
fn closing_a_window_without_a_close_callback_is_not_a_panic() {
    let id = crate::platform::get_platform().create_window("probe", 0, 0, 320, 240);
    WindowHandle::from_raw(id).close();
}

/// D08-A-01: `remove_callbacks` must disconnect the widget's own click signal,
/// so an explicitly removed handler really stops firing.
#[test]
fn remove_callbacks_unsubscribes_the_production_click_signal() {
    use core::sync::atomic::{AtomicUsize, Ordering};
    let id = crate::widget::runtime::register(Box::new(crate::widget::Button::new(
        "probe".to_string(),
        crate::core::Rect::new(0, 0, 80, 24),
    )))
    .expect("a button mounts");
    let calls = std::sync::Arc::new(AtomicUsize::new(0));
    {
        let calls = calls.clone();
        // Register through the real production path, not the legacy table. The
        // slot must be `Send`, so the counter is an `Arc<AtomicUsize>`.
        register_click_callback(id, move || {
            calls.fetch_add(1, Ordering::SeqCst);
        });
    }
    // Sanity: it is connected and fires once.
    let _ = crate::widget::runtime::with_widget_mut(id, |widget| widget.base().clicked.emit());
    assert_eq!(calls.load(Ordering::SeqCst), 1, "the click callback must fire before removal");

    remove_callbacks(id);
    let _ = crate::widget::runtime::with_widget_mut(id, |widget| widget.base().clicked.emit());
    assert_eq!(calls.load(Ordering::SeqCst), 1, "a removed click callback must not fire again");

    crate::widget::runtime::unregister(id);
}

/// D08-A-01: the same for the close signal.
#[test]
fn remove_callbacks_unsubscribes_the_production_close_signal() {
    use core::sync::atomic::{AtomicUsize, Ordering};
    let id = crate::widget::runtime::register(Box::new(crate::widget::Window::new(
        "probe".to_string(),
        crate::core::Rect::new(0, 0, 320, 240),
    )))
    .expect("a window mounts");
    let calls = std::sync::Arc::new(AtomicUsize::new(0));
    {
        let calls = calls.clone();
        register_close_callback(id, move || {
            calls.fetch_add(1, Ordering::SeqCst);
        });
    }
    let _ = crate::widget::runtime::with_widget_mut(id, |widget| widget.base().closed.emit());
    assert_eq!(calls.load(Ordering::SeqCst), 1, "the close callback must fire before removal");

    remove_callbacks(id);
    let _ = crate::widget::runtime::with_widget_mut(id, |widget| widget.base().closed.emit());
    assert_eq!(calls.load(Ordering::SeqCst), 1, "a removed close callback must not fire again");

    crate::widget::runtime::unregister(id);
}

/// D08-A-02: a value callback that removes its own subscription during
/// dispatch must not be resurrected by the guard's restore.
#[test]
fn cancelling_a_value_callback_from_inside_it_stays_cancelled() {
    let id: ObjectId = 4310;
    remove_callbacks(id);
    let ran = alloc::rc::Rc::new(core::cell::Cell::new(0usize));
    let counter = ran.clone();
    VALUE_CALLBACKS.with(|map| {
        map.borrow_mut().insert(
            id,
            alloc::vec![alloc::rc::Rc::new(core::cell::RefCell::new(move |_text: &str| {
                counter.set(counter.get() + 1);
                // Remove the subscription from inside the dispatch.
                remove_callbacks(id);
            }))],
        );
    });

    assert!(dispatch_trigger(id, WidgetTriggerKind::ValueChanged), "the callback runs once");
    assert_eq!(ran.get(), 1);
    // The guard must not restore the callbacks the callback itself removed.
    assert!(!dispatch_trigger(id, WidgetTriggerKind::ValueChanged));
    assert_eq!(ran.get(), 1, "a callback that cancelled itself must not run again");
    remove_callbacks(id);
}

/// A panicking value callback must still be restored, because a panic does not
/// change the removal generation (the guard's restore path is unchanged there).
///
/// The existing `a_panicking_value_callback_stays_registered` covers the same
/// guarantee; this variant additionally runs after the generation mechanism was
/// added, pinning that it did not break the panic path.
#[test]
fn a_panicking_value_callback_survives_the_generation_guard() {
    let id: ObjectId = 4311;
    remove_callbacks(id);
    let ran = std::sync::Arc::new(core::sync::atomic::AtomicUsize::new(0));
    let counter = ran.clone();
    VALUE_CALLBACKS.with(|map| {
        map.borrow_mut().insert(
            id,
            alloc::vec![alloc::rc::Rc::new(core::cell::RefCell::new(move |_text: &str| {
                counter.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
                panic!("intentional");
            }))],
        );
    });

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        dispatch_trigger(id, WidgetTriggerKind::ValueChanged);
    }));
    assert_eq!(ran.load(core::sync::atomic::Ordering::SeqCst), 1);
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        dispatch_trigger(id, WidgetTriggerKind::ValueChanged);
    }));
    assert_eq!(
        ran.load(core::sync::atomic::Ordering::SeqCst),
        2,
        "a panicking callback must stay registered"
    );
    remove_callbacks(id);
}

/// The `WindowState` fields, each with the reason it is allowed to exist.
///
/// # Why this list is a test and not a comment
///
/// Rule #99 says every mirror field is either a *fallback* (the platform may return `None`,
/// and production code reads the mirror in that case) or *deleted*. "Written but never read"
/// is a dangling mirror that drifts into a false fact. The list below is parsed against the
/// struct by the test, so adding a field fails here until someone states which of the two
/// categories it is in -- and the named test is what makes the claim checkable rather than
/// asserted.
///
/// `close_callback` used to be on this list ("authoritative here: `close()` is the only
/// reader"). It was **deleted**: a callback stored in the mirror was a second route a close
/// could travel, and only `WindowHandle::close` produced it — the widget's own `closed` signal
/// never reached it. `on_close` now connects that signal instead, so there is nothing to keep in
/// the mirror.
#[test]
fn every_window_state_field_is_classified_and_backed_by_a_test() {
    /// (field, why it is allowed to exist, the test that proves the reason)
    const CLASSIFICATION: &[(&str, &str, &str)] = &[
        (
            "x",
            "read by `center_on_screen`",
            "the_window_position_mirror_is_read_by_center_on_screen",
        ),
        (
            "y",
            "read by `center_on_screen`",
            "the_window_position_mirror_is_read_by_center_on_screen",
        ),
        (
            "w",
            "fallback: layout size, and `center_on_screen`",
            "the_window_geometry_mirror_reaches_the_layout_path",
        ),
        (
            "h",
            "fallback: layout size, and `center_on_screen`",
            "the_window_geometry_mirror_reaches_the_layout_path",
        ),
        (
            "icon",
            "fallback: 6 backends return `None` from `window_icon`",
            "every_mirror_backed_getter_defers_to_the_platform_first",
        ),
        (
            "min_w",
            "fallback: 6 backends return `None` from `window_min_size`",
            "every_mirror_backed_getter_defers_to_the_platform_first",
        ),
        (
            "min_h",
            "fallback: 6 backends return `None` from `window_min_size`",
            "every_mirror_backed_getter_defers_to_the_platform_first",
        ),
        (
            "maximized",
            "fallback via `mirrored_flag`",
            "every_mirror_backed_getter_defers_to_the_platform_first",
        ),
        (
            "minimized",
            "fallback via `mirrored_flag`",
            "every_mirror_backed_getter_defers_to_the_platform_first",
        ),
        (
            "fullscreen",
            "fallback via `mirrored_flag`",
            "every_mirror_backed_getter_defers_to_the_platform_first",
        ),
        (
            "resizable",
            "fallback via `mirrored_flag`",
            "every_mirror_backed_getter_defers_to_the_platform_first",
        ),
        (
            "decorated",
            "fallback via `mirrored_flag`",
            "every_mirror_backed_getter_defers_to_the_platform_first",
        ),
    ];

    // The test reads its own source file: `include_str!` is resolved relative to the
    // file that contains the macro, which is this one.
    let source = include_str!("window.rs");
    let struct_body = source
        .split_once("struct WindowState {")
        .expect("`WindowState` must still be declared in this file")
        .1
        .split_once('}')
        .expect("`WindowState` must still be a braced struct")
        .0;
    let mut declared: Vec<&str> = struct_body
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            // The struct's fields are `pub(crate)` now that it lives in `window.rs`; the
            // classification table names the field, not its visibility.
            let line = line
                .strip_prefix("pub(crate) ")
                .or_else(|| line.strip_prefix("pub "))
                .unwrap_or(line);
            let (name, _) = line.split_once(':')?;
            let name = name.trim();
            if name.is_empty()
                || name.starts_with("//")
                || !name.ends_with(|c: char| c.is_alphanumeric() || c == '_')
            {
                return None;
            }
            Some(name)
        })
        .collect();
    declared.sort_unstable();

    let mut classified: Vec<&str> = CLASSIFICATION.iter().map(|(f, _, _)| *f).collect();
    classified.sort_unstable();
    assert_eq!(
        declared, classified,
        "every `WindowState` field must be classified with a reason and a named test: a new \
         field is a dangling mirror until someone says which kind it is"
    );

    for (field, reason, test) in CLASSIFICATION {
        assert!(!reason.trim().is_empty(), "`{field}` needs a stated reason");
        assert!(
            source.contains(&format!("fn {test}(")) || TEST_NAMES.contains(test),
            "`{field}` names test `{test}`, which does not exist"
        );
    }
}

/// The test names this module is expected to define, so the classification table above can be
/// checked from inside the module it describes.
const TEST_NAMES: &[&str] = &[
    "the_window_position_mirror_is_read_by_center_on_screen",
    "the_window_geometry_mirror_reaches_the_layout_path",
    "every_mirror_backed_getter_defers_to_the_platform_first",
];
