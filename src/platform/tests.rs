// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Platform integration tests — capability negotiation, widget creation round-trips,
//! event injection, clipboard, IME, and drag-drop flows.
//!
//! All tests use `StubPlatform` (in-memory state backend) so they run on any host
//! without native platform dependencies.

use crate::core::{ObjectId, PlatformFamily};
use crate::platform::{Platform, StubPlatform, WidgetTriggerEvent, WidgetTriggerKind};

/// The process-global backend must be the Harmony backend when the `harmony`
/// preview feature is enabled, rather than falling through to the generic
/// desktop stub. Regression guard for the missing `create_native_platform` arm.
#[cfg(feature = "harmony")]
#[test]
fn runtime_selects_harmony_backend_when_feature_enabled() {
    let platform = crate::platform::get_platform();
    assert_eq!(platform.backend_name(), "harmony-state-backend");
    assert_eq!(platform.family(), PlatformFamily::Desktop);
}

/// Every build must resolve a *real* backend, and only one.
///
/// `create_native_platform` is defined eleven times, once per target/feature
/// combination, and they must stay mutually exclusive. Overlap is a compile error
/// (`E0428`), but the opposite failure — a combination that matches **no** arm —
/// would fall through to `unknown-runtime-stub`, which looks like a working build
/// until the first control does nothing. This test names that failure.
///
/// Measured behaviour (so the assertion below is not an overclaim): a build with a
/// device profile resolves to that platform's real backend, and a build with **no**
/// device profile still resolves to a real one on a supported target
/// (`linux-state-backend` on Linux, for example) because the per-target arm does not
/// depend on the profile. The unknown stub is therefore reached only on a target this
/// crate has no backend for, which the CI target matrix does not cover. The assertion
/// is scoped to device builds because that is where a missing arm would be a silent
/// regression rather than an expected fallback.
#[test]
fn the_selected_backend_is_real_and_not_the_unknown_stub() {
    let name = crate::platform::backend_name();
    assert!(!name.is_empty(), "a backend must name itself");

    let has_device_profile = cfg!(device_profile);
    if has_device_profile {
        assert_ne!(
            name, "unknown-runtime-stub",
            "a device build resolved no backend at all: one of the \
             `create_native_platform` arms is missing its condition"
        );
    }
}

/// The generic stub must never be selected merely because the host OS is
/// unrecognised; a feature-selected backend takes precedence.
#[cfg(feature = "harmony")]
#[test]
fn runtime_does_not_fall_back_to_unknown_stub_with_harmony() {
    let name = crate::platform::backend_name();
    assert_ne!(name, "unknown-runtime-stub");
}
#[test]
fn consistency_menu_trigger_roundtrip() {
    let platform = StubPlatform::new("test-desktop", PlatformFamily::Desktop);
    let window = platform.create_window("w", 0, 0, 100, 100);
    assert!(platform.inject_menu_trigger(window));
    assert_eq!(platform.poll_menu_triggered(), Some(window));
}
#[test]
fn consistency_typed_widget_trigger_roundtrip() {
    let platform = StubPlatform::new("test-desktop", PlatformFamily::Desktop);
    let window = platform.create_window("w", 0, 0, 100, 100);
    platform.register_widget(window, "btn", 0, 0, 80, 30);
    assert!(platform.inject_widget_trigger_event(window, WidgetTriggerKind::Clicked));
    assert_eq!(
        platform.poll_widget_trigger_event(),
        Some(WidgetTriggerEvent { widget_id: window, kind: WidgetTriggerKind::Clicked })
    );
}
#[test]
fn consistency_compat_poll_widget_triggered_is_single_delivery_shim() {
    let platform = StubPlatform::new("test-desktop", PlatformFamily::Desktop);
    let window = platform.create_window("w", 0, 0, 100, 100);
    assert!(platform.inject_widget_trigger_event(window, WidgetTriggerKind::Clicked));
    assert_eq!(platform.poll_widget_triggered(), Some(window));
    assert_eq!(platform.poll_widget_triggered(), None);
    assert_eq!(platform.poll_widget_trigger_event(), None);
}
#[test]
fn consistency_list_box_data_path_roundtrip() {
    let platform = StubPlatform::new("test-desktop", PlatformFamily::Desktop);
    let list_box = platform.create_window("list", 0, 0, 120, 80);
    assert!(platform.set_widget_selected_index(list_box, Some(1)));
    assert_eq!(platform.widget_selected_index(list_box), Some(1));
    assert!(platform.set_widget_selected_index(list_box, None));
    assert_eq!(platform.widget_selected_index(list_box), None);
}
#[test]
fn consistency_combo_box_data_and_event_path_roundtrip() {
    let platform = StubPlatform::new("test-desktop", PlatformFamily::Desktop);
    let combo = platform.create_window("combo", 0, 0, 120, 24);
    assert!(platform.set_widget_selected_index(combo, Some(1)));
    assert_eq!(platform.widget_selected_index(combo), Some(1));
    assert!(platform.inject_widget_trigger_event(combo, WidgetTriggerKind::SelectionChanged));
    assert_eq!(
        platform.poll_widget_trigger_event(),
        Some(WidgetTriggerEvent { widget_id: combo, kind: WidgetTriggerKind::SelectionChanged })
    );
}
#[test]
fn consistency_capability_contract_by_profile() {
    let desktop = StubPlatform::new("test-desktop", PlatformFamily::Desktop);
    let embedded = StubPlatform::new("test-embedded", PlatformFamily::Embedded);
    assert!(desktop.native_capability_contract().is_some());
    assert!(desktop.embedded_capability_contract().is_none());
    assert!(embedded.native_capability_contract().is_none());
    assert!(embedded.embedded_capability_contract().is_some());
}
/// A host owns exactly one primitive: the window. Every control is painted by
/// the library, so it registers through the drawing bridge instead — and the
/// registered id must be readable through the same accessors a host-allocated
/// window uses, otherwise the two creation paths would have different semantics.
#[test]
fn embedded_profile_self_drawn_controls_are_registered_with_the_host() {
    let platform = StubPlatform::new("test-embedded", PlatformFamily::Embedded);
    platform.register_widget(0x1001, "b", 0, 0, 80, 24);
    platform.register_widget(0x1002, "c", 0, 0, 80, 24);
    assert_eq!(platform.get_widget_text(0x1001), "b");
    assert!(platform.is_widget_visible(0x1002));
    assert!(platform.is_widget_enabled(0x1002));
    assert_eq!(platform.widget_count(), 2);
    assert!(platform.destroy_widget(0x1001));
    assert_eq!(platform.widget_count(), 1);
}
/// Menus, tool bars and status bars have no OS object on a host that paints
/// everything itself, so the backend must report the gap rather than invent an
/// id (principle #37). These assertions previously pinned the *embedded* profile
/// as the special case; after BLUE15 the refusal is uniform across all profiles.
#[test]
fn host_controls_are_explicitly_unsupported_on_every_profile() {
    for family in [PlatformFamily::Desktop, PlatformFamily::Embedded] {
        let platform = StubPlatform::new("test", family);
        let window = platform.create_window("w", 0, 0, 200, 120);
        assert_eq!(platform.create_menu_bar(window, 0, 0, 200, 24), 0);
        assert_eq!(platform.create_menu(window, "File", 0, 0, 80, 24), 0);
        assert_eq!(platform.menu_add_item(window, "Open", None), 0);
        assert_eq!(platform.create_tool_bar(window, 0, 24, 200, 24), 0);
        assert_eq!(platform.create_status_bar(window, "ready", 0, 96, 200, 24), 0);
        assert_eq!(platform.create_button(window, "b", 0, 0, 80, 24), 0);
        assert!(!platform.attach_menu_bar_to_window(window, window));
        assert!(!platform.inject_menu_trigger(ObjectId::MAX));
    }
}
#[test]
fn embedded_profile_selection_state_roundtrip() {
    let platform = StubPlatform::new("test-embedded", PlatformFamily::Embedded);
    let combo = platform.create_window("combo", 0, 0, 120, 24);
    assert_ne!(combo, 0);
    assert!(platform.set_widget_selected_index(combo, Some(1)));
    assert_eq!(platform.widget_selected_index(combo), Some(1));
    assert!(platform.inject_widget_trigger_event(combo, WidgetTriggerKind::SelectionChanged));
    assert_eq!(
        platform.poll_widget_trigger_event(),
        Some(WidgetTriggerEvent { widget_id: combo, kind: WidgetTriggerKind::SelectionChanged })
    );
    let list = platform.create_window("list", 0, 30, 120, 80);
    assert_ne!(list, 0);
    assert!(platform.set_widget_selected_index(list, Some(0)));
    assert_eq!(platform.widget_selected_index(list), Some(0));
    assert!(platform.inject_widget_trigger_event(list, WidgetTriggerKind::SelectionChanged));
    assert_eq!(
        platform.poll_widget_trigger_event(),
        Some(WidgetTriggerEvent { widget_id: list, kind: WidgetTriggerKind::SelectionChanged })
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Platform isolation contract (principle #35–#37)
//
// OS facts must be answered by the backend through semantic `Platform` methods,
// never sniffed by a middle layer. These tests pin the default behavior so a
// backend cannot silently start fabricating values.
// ═══════════════════════════════════════════════════════════════════════════════

/// A backend with no OS to interrogate must say "unknown", not invent a number.
/// A fabricated figure would make the adaptive layers size caches against a lie.
#[test]
fn stub_reports_unknown_platform_facts_honestly() {
    let platform = StubPlatform::new("test-desktop", PlatformFamily::Desktop);
    assert_eq!(platform.total_memory_mb(), None);
    assert_eq!(platform.process_memory_utilization(), None);
    assert_eq!(platform.process_cpu_utilization(), None);
}

/// `is_on_battery` defaults to `false` on backends that cannot tell. That is the
/// safe direction: a wrong `true` would strip animations from a plugged-in host.
#[test]
fn battery_unknown_defaults_to_mains_power() {
    let platform = StubPlatform::new("test-desktop", PlatformFamily::Desktop);
    assert!(!platform.is_on_battery());
}

/// A backend with no spooler must refuse the job rather than report success, so
/// the caller can surface the failure instead of pretending it printed.
#[test]
fn stub_without_spooler_refuses_print_job() {
    let platform = StubPlatform::new("test-desktop", PlatformFamily::Desktop);
    assert!(!platform.has_print_support());
    let result = platform.spawn_print_job(std::path::Path::new("/tmp/does-not-matter.txt"));
    assert!(result.is_err(), "a backend with no spooler must report the gap, not fake success");
}

/// Control routing resolves every kind through the global policy table, with no
/// per-backend membership question.
///
/// This replaces a test that asserted `Platform::native_widget_kinds()` defaulted to
/// empty. The method is gone — `control_backend/routing.rs` records that there is one
/// painting mechanism now, so the question it answered no longer has two answers, and
/// it had zero production callers. What remains worth pinning is the property that
/// makes routing deterministic: every kind, including the kinds Win32 would once have
/// claimed as native primitives, resolves the same way.
#[test]
fn routing_resolves_every_kind_through_the_single_policy() {
    use crate::control_backend::route_preference_for_widget_kind;
    for kind in [
        crate::widget::WidgetKind::ListBox,
        crate::widget::WidgetKind::ScrollArea,
        crate::widget::WidgetKind::SpinBox,
        crate::widget::WidgetKind::Button,
    ] {
        assert_eq!(
            route_preference_for_widget_kind(kind),
            crate::control_backend::ControlRoutePreference::CustomRequired,
            "{kind:?} must route through the single painting backend"
        );
    }
}

/// Guards the spooler probe against an exit-code false negative.
///
/// CUPS `lp` rejects `--version` with status 1 while still printing usage, so a
/// check keyed on `status.success()` reported "no spooler" on machines that had
/// one. Presence is proven by the command being spawnable, and this test pins
/// that `lp` (which ships on every macOS and most Linux installs) is detected
/// when it exists on `PATH`.
#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn unix_print_probe_detects_cups_client_despite_nonzero_version_exit() {
    let lp_exists = std::process::Command::new("lp").arg("--help").output().is_ok();
    let lpr_exists = std::process::Command::new("lpr").arg("--help").output().is_ok();
    if lp_exists || lpr_exists {
        assert!(
            crate::platform::types::unix_print_clients_available(),
            "a spooler client exists on PATH but was reported missing"
        );
    }
}

/// A backend with no engine must say so, so a caller can present the simulated
/// navigation path for what it is instead of implying a rendered page.
///
/// This replaced an assertion that the old `create_web_engine()` returned `None`. The
/// trait method was removed by BLUE20 layer 5 (ruling W1) because its `None` conflated
/// "no engine exists" with "the engine could not be constructed", and a capability
/// question is what a caller can actually act on. The default is the honest answer, so
/// a backend that declares nothing declares no engine.
#[test]
fn stub_reports_no_web_engine_capability() {
    let platform = StubPlatform::new("test-desktop", PlatformFamily::Desktop);
    assert!(
        !platform.supports_web_engine(),
        "a backend that implements nothing must not claim a rendering engine"
    );
}

/// Shortcut notation is asked of the backend rather than decided by `cfg!` in the
/// shortcut layer. The stub inherits the build-target default, and on an Apple
/// host that must be the AppKit symbol style.
#[test]
fn shortcut_style_follows_the_backend_not_a_middle_layer_cfg() {
    use crate::shortcut::PlatformShortcutStyle;
    let platform = StubPlatform::new("test-desktop", PlatformFamily::Desktop);
    let expected = if cfg!(any(target_os = "macos", target_os = "ios")) {
        PlatformShortcutStyle::Mac
    } else {
        PlatformShortcutStyle::Desktop
    };
    assert_eq!(platform.shortcut_style(), expected);
}

/// The format used for menu labels must agree with the backend's declared style,
/// so `Shortcut::primary` renders `⌘Z` or `Ctrl+Z` as the host expects.
#[test]
fn format_shortcut_uses_the_backend_style() {
    use crate::shortcut::{format_shortcut_for_platform, Key, Shortcut};
    let platform = StubPlatform::new("test-desktop", PlatformFamily::Desktop);
    let shortcut = Shortcut::primary(Key::Z);
    assert_eq!(
        platform.format_shortcut(&shortcut),
        format_shortcut_for_platform(&shortcut, platform.shortcut_style()),
    );
}

/// Every interactive backend is reported interactive, and every state-only backend is not.
///
/// # What this pins, and how the first version of it failed
///
/// [`crate::platform::runtime_gui_mode_for`] decides interactivity from a backend's name. That
/// name is a string the backend chooses for itself, so the mapping has two failure modes and the
/// first fix for it hit **both**:
///
/// 1. **A dead arm.** `"harmony-desktop"` stayed listed after the backend renamed itself, and it
///    was masked because the same arm *also* carried the new name.
/// 2. **A new false answer.** The fix required `capabilities().dpi_scaling` as evidence — and the
///    same round had correctly set macOS's `dpi_scaling` to `false` (it never overrides
///    `dpi_scale_factor`). So the real `cocoa` backend, which *does* open AppKit windows and *is*
///    what the shipped `desktop` profile selects on macOS, answered `PreviewOrStub`. A `desktop`
///    build would have told every macOS user "no window will appear".
///
/// The lesson is in the table below: correctness has to be checked at the **predicate**, where two
/// individually-right changes can disagree, not only in the change itself. The original test only
/// covered a synthetic `cocoa`-named struct, so the real backend — `#[cfg]`-gated to a host the
/// suite does not run on — was never exercised.
///
/// # Why the rule is a compile-time fact now
///
/// `dpi_scaling` answers "can this backend ask the display its scale factor", which is not the
/// question. "Is a native window path compiled in" is a fact about the build, so it is asked with
/// `cfg!` where it is conditional (`gtk`, `wayland`, whose name is the same in a toolkit-less
/// build) and is implied by the type existing where it is not.
#[test]
fn every_backend_is_reported_at_the_mode_it_actually_has() {
    use crate::platform::{runtime_gui_mode_for, RuntimeGuiMode};

    /// A backend with `name` and no behaviour at all. Only `backend_name` matters here, because
    /// that is the whole input the predicate reads.
    struct Named(&'static str);

    impl Platform for Named {
        fn as_any(&self) -> &dyn core::any::Any {
            self
        }
        fn backend_name(&self) -> &'static str {
            self.0
        }
        fn family(&self) -> PlatformFamily {
            PlatformFamily::Desktop
        }
        fn init(&self) {}
        fn run(&self) {}
        fn quit(&self) {}
        fn create_window(&self, _t: &str, _x: i32, _y: i32, _w: u32, _h: u32) -> ObjectId {
            0
        }
    }

    // The name-side rule, independent of which host this test runs on. `cocoa` and
    // `WindowsPlatform` need no `cfg!` because their modules only exist on their own target with
    // their own feature, so an instance existing *is* the compile-time proof — which is exactly
    // what the previous fix got wrong by consulting a runtime capability instead.
    for name in ["cocoa", "WindowsPlatform"] {
        assert_eq!(
            runtime_gui_mode_for(&Named(name)),
            RuntimeGuiMode::NativeInteractive,
            "{name} opens native windows, so it must be reported interactive regardless of what \
             its capabilities() say about DPI — those are separate questions"
        );
    }

    // `gtk` and `wayland` are conditional on the build, so their expectation is too.
    let expected_gtk = if cfg!(all(target_os = "linux", feature = "gtk-native")) {
        RuntimeGuiMode::NativeInteractive
    } else {
        RuntimeGuiMode::PreviewOrStub
    };
    assert_eq!(runtime_gui_mode_for(&Named("gtk")), expected_gtk);
    let expected_wayland = if cfg!(all(target_os = "linux", feature = "wayland-native")) {
        RuntimeGuiMode::NativeInteractive
    } else {
        RuntimeGuiMode::PreviewOrStub
    };
    assert_eq!(runtime_gui_mode_for(&Named("wayland")), expected_wayland);

    // Every state-only backend, including the two macOS fallbacks the old code never listed and
    // the harmony name the old arm had gone stale on. A state backend creates no window, so a
    // `NativeInteractive` here is a false promise.
    for name in [
        "macos-objc2-preview",
        "macos-fallback-stub",
        "unknown-runtime-stub",
        "harmony-state-backend",
        "android-state-backend",
        "android-mobile",
        "ios-state-backend",
        "wasm-state-backend",
        "portable",
        "recording-test-backend",
        // The renamed-away spelling must not be treated as interactive on the strength of the
        // word "desktop" in it — it is not a backend at all any more.
        "harmony-desktop",
    ] {
        assert_eq!(
            runtime_gui_mode_for(&Named(name)),
            RuntimeGuiMode::PreviewOrStub,
            "{name} creates no native window, so claiming it will open one is the silent \
             failure this function exists to prevent"
        );
    }

    // And the real constructible backend on this host must agree with its own row: the strongest
    // available check that the predicate has not drifted from what the crate actually ships.
    let stub = StubPlatform::new("portable", PlatformFamily::Embedded);
    assert_eq!(runtime_gui_mode_for(&stub), RuntimeGuiMode::PreviewOrStub);
}

/// Every backend that **declares** `typed_widget_trigger` must actually deliver a trigger.
///
/// # The defect this pins
///
/// `PlatformCapabilities::typed_widget_trigger` was `true` on every backend — it is the one flag
/// the trait default already sets, on the reasoning that the library implements it rather than the
/// host. The reasoning is right about *why* the capability can exist and wrong about whether a
/// given backend **wired it up**: the implementation is two one-line delegations to the shared
/// `BackendState` queue, and six backends wrote them while `linux`, `macos`, `macos_objc2` and
/// `wasm` did not. Each of those four answered the trait defaults (`None` / `false`) while
/// claiming the flag.
///
/// It is host-visible, not theoretical: `NativeControlBackend` forwards both methods straight to
/// `get_platform()` (`control_backend/native.rs`), so on a GTK/cocoa/objc2 desktop build an
/// injected trigger was accepted by the API and then silently dropped.
///
/// # Why this is asserted over the *real* backends and not just the stub
///
/// `consistency_typed_widget_trigger_roundtrip` already covers `StubPlatform`, and it passed
/// throughout — because the stub is one of the six that delegate. A test that only exercises the
/// backend which happens to be correct is exactly how four others kept the flag without the
/// methods, so this one builds each backend this host can construct and asks **them**.
///
/// # What it asserts, in order
///
/// The flag, then the round-trip, then that the two FIFO views share one queue. The middle step is
/// the one that would have failed: `inject` returning `false` is a refusal, so a backend missing
/// the delegation fails on `assert!` rather than silently passing on a `None` comparison.
#[test]
fn a_declared_typed_trigger_capability_is_backed_by_the_queue_that_delivers_it() {
    use crate::platform::PlatformCapabilities;

    /// `name` is only for the failure message; the backend is the subject.
    fn assert_delivers(name: &str, platform: &dyn Platform) {
        let caps: PlatformCapabilities = platform.capabilities();
        if !caps.typed_widget_trigger {
            // Under-claiming is honest and out of scope: the backend is saying it has no queue.
            return;
        }
        let window = platform.create_window("trigger-probe", 0, 0, 100, 100);
        assert!(
            platform.inject_widget_trigger_event(window, WidgetTriggerKind::Clicked),
            "{name}: declares `typed_widget_trigger: true` but `inject_widget_trigger_event` \
             refused a window it just created, so the flag promises a queue this backend does \
             not delegate to"
        );
        assert_eq!(
            platform.poll_widget_trigger_event(),
            Some(WidgetTriggerEvent { widget_id: window, kind: WidgetTriggerKind::Clicked }),
            "{name}: the injected event must come back out of the typed view"
        );
        assert_eq!(
            platform.poll_widget_triggered(),
            None,
            "{name}: and both views must be one queue, so the typed poll already consumed it"
        );
    }

    // The always-constructible one, so the assertions themselves are never vacuous.
    assert_delivers("stub", &StubPlatform::new("trigger-stub", PlatformFamily::Desktop));

    // The real backend this host compiles in, if any. Each arm repeats the module's own gate,
    // because a backend that is not compiled in cannot be constructed and must not be pretended.
    #[cfg(target_os = "linux")]
    assert_delivers("linux", &crate::platform::linux::LinuxPlatform::new());
    #[cfg(all(target_os = "linux", feature = "wayland-native"))]
    assert_delivers("wayland", &crate::platform::wayland::WaylandPlatform::new());
    #[cfg(all(target_os = "macos", feature = "cocoa-legacy"))]
    assert_delivers("cocoa", &crate::platform::macos::MacOSPlatform::default());
    #[cfg(all(target_os = "macos", any(feature = "macos", feature = "cocoa-legacy")))]
    assert_delivers("macos-objc2", &crate::platform::macos_objc2::MacOSObjc2Platform::default());
    #[cfg(target_os = "windows")]
    assert_delivers("windows", &crate::platform::windows::WindowsPlatform::new());
    #[cfg(feature = "wasm")]
    assert_delivers("wasm", &crate::platform::wasm::WasmPlatform::default());
    #[cfg(feature = "harmony")]
    assert_delivers("harmony", &crate::platform::harmony::HarmonyPlatform::new());
}

/// A backend's capability flags must agree with the methods those flags promise.
///
/// # Why the earlier flag fixes needed this and did not have it
///
/// Six backends were corrected by hand across two rounds, each time by a reader noticing that a
/// flag's method was missing. Nothing prevented the next one, and the two closest calls are
/// instructive because both were *individually correct changes*:
///
/// * `macos`'s `dpi_scaling` was corrected to `false` (right — it never overrides
///   `dpi_scale_factor`), and `runtime_gui_mode_for` was reading that flag as evidence of "can open
///   a window" — so the real, window-opening cocoa backend started answering `PreviewOrStub`.
/// * `wayland` kept `ime: true` and `accessibility: true` for longer than any other backend, masked
///   by the fact that it *did* override `dpi_scale_factor` — one honest flag beside two dishonest
///   ones reads as a backend that checked.
///
/// Both are the same failure: a flag and its method are two statements about one fact, and only
/// the method is checked by the compiler. This test states the relation for every backend this host
/// can construct, so a flag cannot drift from its method without a failure naming the pair.
///
/// # Why the direction is one-way
///
/// The assertions are `flag implies method`. They deliberately do **not** assert the converse: a
/// backend that implements a method and reports `false` is *under*-claiming, which the trait's own
/// documentation calls the direction a default must err. Asserting both would make the honest
/// default idiom unrepresentable and would fail a backend for being cautious.
#[test]
fn capability_flags_agree_with_the_methods_they_promise() {
    use crate::platform::PlatformCapabilities;

    fn assert_agrees(name: &str, platform: &dyn Platform) {
        let caps: PlatformCapabilities = platform.capabilities();
        if caps.ime {
            assert!(
                platform.ime_bridge().is_some(),
                "{name}: claims `ime: true` but `ime_bridge()` answers None, so the flag \
                 promises an input-method client that does not exist"
            );
        }
        if caps.accessibility {
            assert!(
                platform.accessibility_bridge().is_some(),
                "{name}: claims `accessibility: true` but `accessibility_bridge()` answers None"
            );
        }
        if caps.dpi_scaling {
            // Not asserted for `dpi_scaling: true` — plenty of honest backends report the flag
            // while `dpi_scale_factor()` happens to be `1.0` on this host (an unscaled display is
            // still a scaled-capable backend). What is *not* honest is `true` with no override at
            // all, which is a compile-time fact the source gate checks
            // (`check_capability_flags_match_their_methods.sh`); here the runtime value is only
            // required to be a usable number rather than a NaN or a negative scale.
            let scale = platform.dpi_scale_factor();
            assert!(
                scale.is_finite() && scale > 0.0,
                "{name}: claims `dpi_scaling: true` but reports a scale of {scale}, which no \
                 layout can use"
            );
        }
    }

    assert_agrees("stub", &StubPlatform::new("flags-stub", PlatformFamily::Desktop));

    #[cfg(target_os = "linux")]
    assert_agrees("linux", &crate::platform::linux::LinuxPlatform::new());
    #[cfg(all(target_os = "linux", feature = "wayland-native"))]
    assert_agrees("wayland", &crate::platform::wayland::WaylandPlatform::new());
    #[cfg(all(target_os = "macos", feature = "cocoa-legacy"))]
    assert_agrees("cocoa", &crate::platform::macos::MacOSPlatform::default());
    #[cfg(all(target_os = "macos", any(feature = "macos", feature = "cocoa-legacy")))]
    assert_agrees("macos-objc2", &crate::platform::macos_objc2::MacOSObjc2Platform::default());
    #[cfg(target_os = "windows")]
    assert_agrees("windows", &crate::platform::windows::WindowsPlatform::new());
    #[cfg(feature = "wasm")]
    assert_agrees("wasm", &crate::platform::wasm::WasmPlatform::default());
    #[cfg(feature = "harmony")]
    assert_agrees("harmony", &crate::platform::harmony::HarmonyPlatform::new());
    #[cfg(target_os = "android")]
    assert_agrees("android", &crate::platform::android::AndroidPlatform::new());
    #[cfg(target_os = "ios")]
    assert_agrees("ios", &crate::platform::ios::IosMobilePlatform::new());
}
