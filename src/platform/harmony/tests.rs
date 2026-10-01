// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Integration tests for the Harmony desktop backend.
//!
//! Every `WidgetKind` is painted by `src/widget/`, so the backend is a pure state
//! model: windows carry geometry/text, trigger queues carry events, and there is
//! no native-control surface left to exercise.

use crate::platform::harmony::HarmonyPlatform;
use crate::platform::Platform;
use crate::WidgetTriggerKind;
// `ToString` is not in the prelude of a `mini` (`no_std` + `alloc_frugal`) build, and this
// test file is compiled under every profile the cross-target gate runs — including that one.
// Importing it explicitly is what keeps `.to_string()` available everywhere rather than only
// in the profiles that happen to use the `std` prelude.
use alloc::string::ToString;

/// The `#[repr(C)]` input structs must keep the byte layout the SDK declares.
///
/// # Why this is a test and not a comment
///
/// These three structs are hand-written mirrors of `native_interface_xcomponent.h` (the module
/// explains why the binding is not generated). A field added, removed or reordered in the wrong
/// place does not fail to compile — it silently shifts every field after it, so the dispatcher
/// reads a timestamp where it expected a coordinate. `#[repr(C)]` guarantees the *rule*; it
/// cannot check that the rule was applied to the right *declaration*.
///
/// Gated on `feature = "xcomponent"` because that is what compiles the module the structs live
/// in — the plain harmony cross-target build does not enable it (it links `libace_ndk`, which
/// only an OpenHarmony link step can satisfy), so an ungated reference fails to resolve there.
///
/// The expected offsets below are measured from the SDK header, not derived:
///
/// ```text
/// $ cc -o probe probe.c && ./probe        # with the SDK 20 header declarations
/// sizeof=56
/// id=0 screenX=4 screenY=8 x=12 y=16 type=20 size=24 force=32 ts=40 pressed=48
/// ```
///
/// `TouchPoint` deliberately omits `type`; see its own documentation for why that lands on the
/// same offsets anyway. This test is what would catch it **ceasing** to be true.
#[cfg(feature = "xcomponent")]
#[test]
fn the_sdk_structs_keep_their_declared_layout() {
    use core::mem::{align_of, size_of};

    // Offsets without `offset_of!` (unstable on the MSRV), by measuring a zeroed value.
    macro_rules! off {
        ($ty:ty, $field:ident) => {{
            let value = core::mem::MaybeUninit::<$ty>::zeroed();
            let base = value.as_ptr() as usize;
            #[allow(unused_unsafe)]
            let field = unsafe { &(*value.as_ptr()).$field } as *const _ as usize;
            field - base
        }};
    }

    assert_eq!(size_of::<crate::platform::harmony::xcomponent::TouchPoint>(), 56);
    assert_eq!(align_of::<crate::platform::harmony::xcomponent::TouchPoint>(), 8);
    assert_eq!(off!(crate::platform::harmony::xcomponent::TouchPoint, id), 0);
    assert_eq!(off!(crate::platform::harmony::xcomponent::TouchPoint, screen_x), 4);
    assert_eq!(off!(crate::platform::harmony::xcomponent::TouchPoint, screen_y), 8);
    assert_eq!(off!(crate::platform::harmony::xcomponent::TouchPoint, x), 12);
    assert_eq!(off!(crate::platform::harmony::xcomponent::TouchPoint, y), 16);
    assert_eq!(off!(crate::platform::harmony::xcomponent::TouchPoint, size), 24);
    assert_eq!(off!(crate::platform::harmony::xcomponent::TouchPoint, force), 32);
    assert_eq!(off!(crate::platform::harmony::xcomponent::TouchPoint, time_stamp), 40);
    assert_eq!(off!(crate::platform::harmony::xcomponent::TouchPoint, is_pressed), 48);

    assert_eq!(off!(crate::platform::harmony::xcomponent::MouseEvent, x), 0);
    assert_eq!(off!(crate::platform::harmony::xcomponent::MouseEvent, y), 4);
    assert_eq!(off!(crate::platform::harmony::xcomponent::MouseEvent, screen_x), 8);
    assert_eq!(off!(crate::platform::harmony::xcomponent::MouseEvent, screen_y), 12);
    assert_eq!(off!(crate::platform::harmony::xcomponent::MouseEvent, time_stamp), 16);
    assert_eq!(off!(crate::platform::harmony::xcomponent::MouseEvent, action), 24);
    assert_eq!(off!(crate::platform::harmony::xcomponent::MouseEvent, button), 28);

    // The event struct's own prefix, up to and including `device_id`.
    assert_eq!(off!(crate::platform::harmony::xcomponent::TouchEvent, id), 0);
    assert_eq!(off!(crate::platform::harmony::xcomponent::TouchEvent, event_type), 20);
    assert_eq!(off!(crate::platform::harmony::xcomponent::TouchEvent, size), 24);
    assert_eq!(off!(crate::platform::harmony::xcomponent::TouchEvent, force), 32);
    assert_eq!(off!(crate::platform::harmony::xcomponent::TouchEvent, device_id), 40);
    assert_eq!(off!(crate::platform::harmony::xcomponent::TouchEvent, time_stamp), 48);
}

#[test]
fn platform_creates_and_runs() {
    let backend = HarmonyPlatform::new();
    backend.init();
    assert_eq!(backend.backend_name(), "harmony-state-backend");

    let window = backend.create_window("TestWindow", 100, 200, 640, 480);
    assert!(window > 0, "Window should be created");
    assert_eq!(backend.get_widget_text(window), "TestWindow");
}

#[test]
fn widget_lifecycle() {
    let backend = HarmonyPlatform::new();
    backend.init();
    let window = backend.create_window("w", 0, 0, 200, 120);
    assert!(window > 0, "Window should be created");

    // Test show/hide.
    backend.show_widget(window);
    assert!(backend.is_widget_visible(window), "Widget should be visible after show");

    backend.hide_widget(window);
    assert!(!backend.is_widget_visible(window), "Widget should be hidden after hide");

    backend.show_widget(window);
    assert!(backend.is_widget_visible(window), "Widget should be visible after second show");

    // Test enable/disable.
    backend.set_widget_enabled(window, false);
    assert!(!backend.is_widget_enabled(window), "Widget should be disabled");

    backend.set_widget_enabled(window, true);
    assert!(backend.is_widget_enabled(window), "Widget should be enabled");

    // Test text set/get roundtrip.
    backend.set_widget_text(window, "updated");
    assert_eq!(backend.get_widget_text(window), "updated", "Widget text should update");

    // Test geometry update.
    backend.set_widget_geometry(window, 20, 20, 100, 30);

    // Test IME (defaults to true in BackendState::WidgetRecord).
    assert!(backend.is_widget_ime_enabled(window), "IME should be enabled by default");
    assert!(backend.set_widget_ime_enabled(window, false), "set_widget_ime_enabled should succeed");
    assert!(
        !backend.is_widget_ime_enabled(window),
        "IME should be disabled after set_widget_ime_enabled(false)"
    );
}

#[test]
fn clipboard_roundtrip() {
    let backend = HarmonyPlatform::new();
    backend.init();

    // Clipboard should start empty.
    assert_eq!(backend.get_clipboard_text(), "", "Clipboard should be empty initially");

    // Set and get roundtrip.
    assert!(backend.set_clipboard_text("hello harmony"), "Should set clipboard text");
    assert_eq!(backend.get_clipboard_text(), "hello harmony", "Clipboard text should match");

    // Overwrite with new value.
    assert!(backend.set_clipboard_text("updated clipboard"), "Should overwrite clipboard text");
    assert_eq!(
        backend.get_clipboard_text(),
        "updated clipboard",
        "Clipboard text should reflect update"
    );

    // Set empty string.
    assert!(backend.set_clipboard_text(""), "Should set empty clipboard text");
    assert_eq!(
        backend.get_clipboard_text(),
        "",
        "Clipboard should be empty after setting empty string"
    );
}

#[test]
fn widget_trigger_events() {
    let backend = HarmonyPlatform::new();
    backend.init();
    let window = backend.create_window("w", 0, 0, 200, 120);

    // Queue should be empty initially.
    assert!(backend.poll_widget_trigger_event().is_none());

    // Inject a clicked event.
    assert!(backend.inject_widget_trigger_event(window, WidgetTriggerKind::Clicked));

    // Poll the injected event.
    let event = backend.poll_widget_trigger_event();
    assert!(event.is_some(), "Should poll a trigger event");
    let event = event.unwrap();
    assert_eq!(event.widget_id, window);
    assert_eq!(event.kind, WidgetTriggerKind::Clicked);

    // No more events.
    assert!(backend.poll_widget_trigger_event().is_none());

    // Inject a value-changed event.
    assert!(backend.inject_widget_trigger_event(window, WidgetTriggerKind::ValueChanged));
    let event = backend.poll_widget_trigger_event();
    assert!(event.is_some(), "Should poll value-changed event");
    let event = event.unwrap();
    assert_eq!(event.widget_id, window);
    assert_eq!(event.kind, WidgetTriggerKind::ValueChanged);
}

/// The two FIFO views over one queue must agree, through either door.
///
/// # The defect this pins
///
/// [`Platform::poll_widget_triggered`] and [`Platform::poll_widget_trigger_event`]
/// are documented as the same event stream seen twice — the first as a bare id, the
/// second carrying the [`WidgetTriggerKind`] — and
/// `platform::tests::consistency_compat_poll_widget_triggered_is_single_delivery_shim`
/// asserts exactly that about `StubPlatform`. Harmony overrode only the typed view, so
/// an injected trigger was observable through `poll_widget_trigger_event` and
/// simultaneously absent through `poll_widget_triggered`.
///
/// The assertions are deliberately paired and *interleaved* rather than each view
/// being tested in isolation, because the property is not "both return something" — a
/// backend with two independent queues would satisfy that — it is "they are one queue".
/// Draining through one view must consume the event the other would have returned.
#[test]
fn the_two_trigger_views_share_one_queue() {
    let backend = HarmonyPlatform::new();
    backend.init();
    let window = backend.create_window("w", 0, 0, 200, 120);

    assert!(backend.poll_widget_triggered().is_none(), "the queue starts empty");

    assert!(backend.inject_widget_trigger_event(window, WidgetTriggerKind::Clicked));
    assert_eq!(
        backend.poll_widget_triggered(),
        Some(window),
        "the id view must see what was injected"
    );
    assert!(
        backend.poll_widget_trigger_event().is_none(),
        "and the typed view must agree it is gone: one queue, not two"
    );

    // Now the other order, so the test cannot pass by the typed view being the only
    // one that drains.
    assert!(backend.inject_widget_trigger_event(window, WidgetTriggerKind::ValueChanged));
    let event = backend.poll_widget_trigger_event().expect("the typed view sees it");
    assert_eq!(event.kind, WidgetTriggerKind::ValueChanged);
    assert_eq!(event.widget_id, window);
    assert!(backend.poll_widget_triggered().is_none(), "and the id view agrees it is gone");
}

#[test]
fn drag_and_drop() {
    let backend = HarmonyPlatform::new();
    backend.init();
    let window = backend.create_window("w", 0, 0, 200, 120);

    // Drag and drop: begin_drag succeeds because the window is a valid widget.
    assert!(
        backend.begin_drag(window, "text/plain", b"payload"),
        "begin_drag should succeed for a valid widget"
    );
    assert!(
        backend.poll_drop_event().is_some(),
        "poll_drop_event should return Some after begin_drag"
    );
    assert!(
        backend.inject_drop_event(crate::platform::DropEvent {
            source_widget_id: window,
            target_widget_id: window,
            mime: "text/plain".into(),
            payload: vec![]
        }),
        "inject_drop_event should succeed for valid target widget"
    );
}

/// Teardown must report whether the widget existed, and a second call must not
/// claim success for an id that is already gone.
#[test]
fn destroy_widget_reports_existence() {
    let backend = HarmonyPlatform::new();
    backend.init();
    let window = backend.create_window("w", 0, 0, 200, 120);
    assert!(backend.destroy_widget(window));
    assert!(!backend.destroy_widget(window));
    assert!(!backend.destroy_widget(4242));
}

/// The Harmony backend is state-only, so it must not advertise a native menu or
/// inherit desktop defaults that would overstate its capabilities.
///
/// # Why each flag is asserted as it is
///
/// A `PlatformCapabilities` flag means "the named backing method is implemented **by this
/// backend** and will answer for this host": `dpi_scale_factor()`, `ime_bridge()` and
/// `accessibility_bridge()`. So each expectation below is about whether a backing method
/// exists, not about what ArkUI could theoretically do.
///
/// * `dpi_scaling` — always `false`: no `dpi_scale_factor()` override exists, and ArkUI
///   reports the component's size in pixels, not a density.
/// * `ime` — always `false`: no `ime_bridge()` override exists. The XComponent bridge does
///   deliver keys (with modifier state) and does ask ArkUI for the soft keyboard, but that
///   is direct key input, not an input-method client.
/// * `accessibility` — **tracks the `xcomponent` feature**, because that is exactly when
///   `accessibility_bridge()` is implemented. Asserting `false` unconditionally would be
///   wrong in one of the two configurations, so the expectation is written as the same
///   `cfg!` the backend uses; the two must agree by construction, and this pins that.
///
/// This test previously asserted all three `true` and had gone stale: it encoded a contract
/// the backend deliberately moved away from when it stopped creating ArkUI controls. The
/// rule is to **under**-claim rather than over-claim, which is the direction a default must
/// err.
#[test]
fn capabilities_are_explicit_and_honest() {
    let backend = HarmonyPlatform::new();
    let caps = backend.capabilities();

    assert_eq!(backend.family(), crate::core::PlatformFamily::Desktop);
    assert!(
        !caps.dpi_scaling,
        "no `dpi_scale_factor` override exists, so a `true` would promise a method that \
         cannot answer"
    );
    assert!(!caps.ime, "no `ime_bridge` override exists; key input is not an IME client");
    assert_eq!(
        caps.accessibility,
        cfg!(all(feature = "xcomponent", not(alloc_frugal))),
        "`accessibility` must track whether `accessibility_bridge()` is compiled in, which is \
         exactly when the XComponent bridge is"
    );
    assert!(!caps.native_menu, "the Harmony menu is an in-process tree, not an OS menu");
    assert!(caps.typed_widget_trigger, "typed trigger events are supported");
}

/// The accessibility flag and its method must agree — the capability contract, locally.
///
/// `src/platform/tests.rs` states this relation for every backend the host can construct,
/// but only in the one direction (`flag ⇒ method`). This asserts the pair for Harmony in
/// **both** directions, because here the flag is not a constant: it is derived from the same
/// `cfg` as the method, and a future edit that changed one and not the other would silently
/// produce either an over-claim (a `true` promising `None`) or an under-claim that hides a
/// working bridge.
#[test]
fn the_accessibility_flag_matches_its_bridge() {
    let backend = HarmonyPlatform::new();
    let claimed = backend.capabilities().accessibility;
    let answers = backend.accessibility_bridge().is_some();
    assert_eq!(
        claimed, answers,
        "`accessibility: {claimed}` but `accessibility_bridge()` answers \
         {answers}; the two are one statement about one fact and must not drift"
    );
    if answers {
        // A bridge that answers must be usable for the half that needs no provider.
        let bridge = backend.accessibility_bridge().expect("just asserted");
        bridge.set_accessibility_name(42, "Play");
        assert_eq!(bridge.accessibility_name(42), Some("Play".to_string()));
    }
}

/// Mounting a surface must also tell the XComponent bridge, or input is dead.
///
/// # The defect this pins
///
/// `mount_surface` used to write the `BackendState` record and nothing else. Recording
/// the mount makes a widget *visible* (the repaint queue is how the ArkTS side learns a
/// frame is ready) but not *interactive*: every input callback in `harmony::xcomponent`
/// opens with `mounted_widget() else { return; }`, and that is the bridge's own record,
/// written only by `xcomponent::set_mounted_widget`. With no caller, every touch, mouse,
/// hover, key, focus and blur callback returned on its first line — so the bridge bound
/// successfully, reported success, and delivered nothing.
///
/// # What is asserted, and what is not
///
/// Without a bound XComponent, `set_mounted_widget` deliberately **refuses** (a mount that
/// no surface can present must not be recorded as interactive). So the assertion here is
/// the observable one: mounting still succeeds for presentation, and the refusal is
/// reported rather than silent. The positive half — that a bound component accepts the
/// mount and then routes input — needs a real ArkUI host, and is asserted in
/// `xcomponent.rs`'s own tests plus the OHOS cross-build.
#[test]
fn mounting_a_surface_also_informs_the_xcomponent_bridge() {
    let backend = HarmonyPlatform::new();
    let window = backend.create_window("w", 0, 0, 640, 480);
    let rect = crate::core::Rect::new(0, 0, 100, 40);

    assert!(
        backend.mount_surface(window, window, rect),
        "the presentation half must record the mount even with no XComponent bound"
    );
    assert_eq!(backend.state.surface_rect(window), Some(rect));

    #[cfg(all(feature = "xcomponent", not(alloc_frugal)))]
    {
        // No ArkTS host ran `rw_harmony_bind_xcomponent`, so the bridge must have refused
        // the mount rather than claiming a widget it cannot route input to.
        assert_eq!(
            crate::platform::harmony::xcomponent::mounted_widget(),
            None,
            "an unbound bridge must not record a mounted widget"
        );
    }
}

/// The surface contract: the backend hosts library-painted widgets and reports it.
///
/// This test used to assert the *opposite* (`supports_surfaces()` was `false` because
/// `mount_surface` was unimplemented). The surface is now a record plus a repaint
/// queue the ArkTS host drains, so the contract is the one below — and each step is
/// asserted, so a regression to "claims support but does nothing" is caught rather
/// than passing on the strength of a boolean.
#[test]
fn widget_surfaces_are_advertised_and_round_trip() {
    let backend = HarmonyPlatform::new();
    assert!(
        backend.supports_surfaces(),
        "the backend records surfaces and queues repaints, so it can host widgets"
    );

    let window = backend.create_window("w", 0, 0, 640, 480);
    let rect = crate::core::Rect::new(0, 0, 100, 40);
    assert!(backend.mount_surface(window, window, rect));
    assert_eq!(backend.state.surface_rect(window), Some(rect));
    assert_eq!(backend.state.mounted_surface_count(), 1);

    // Invalidating queues exactly one repaint, which the host then drains.
    assert!(backend.invalidate_surface(window));
    assert!(backend.invalidate_surface(window), "a second invalidate still reports the mount");
    assert_eq!(backend.state.pending_repaint_count(), 1, "repaints are coalesced");
    assert_eq!(backend.state.take_pending_repaint(), Some(window));
    assert_eq!(backend.state.pending_repaint_count(), 0);

    // Resizing moves the recorded rect.
    let moved = crate::core::Rect::new(10, 10, 200, 80);
    assert!(backend.resize_surface(window, moved));
    assert_eq!(backend.state.surface_rect(window), Some(moved));

    // Unmounting forgets it, and a stale repaint request cannot survive the unmount.
    assert!(backend.invalidate_surface(window));
    assert!(backend.unmount_surface(window));
    assert_eq!(backend.state.surface_rect(window), None);
    assert_eq!(backend.state.pending_repaint_count(), 0, "a gone widget must not be repainted");
    assert!(!backend.unmount_surface(window), "unmounting twice must report no-op");
}

/// A surface for a widget the backend never made must be refused, not silently
/// recorded: a frame nobody can produce is not a display.
#[test]
fn mounting_a_surface_for_an_unknown_widget_is_refused() {
    let backend = HarmonyPlatform::new();
    assert!(!backend.mount_surface(1, 9_999, crate::core::Rect::new(0, 0, 10, 10)));
    assert!(!backend.invalidate_surface(9_999));
    assert!(!backend.resize_surface(9_999, crate::core::Rect::new(0, 0, 10, 10)));
    assert_eq!(backend.state.mounted_surface_count(), 0);
}
