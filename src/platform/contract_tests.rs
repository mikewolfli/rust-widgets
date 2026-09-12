//! Cross-backend contract-consistency tests.
//!
//! The platform-consistency gates in `tools/` are static: they class impost and
//! count implementations. What they cannot show is whether the *same call*
//! behaves the same way on every backend. These tests pin down the behavioural
//! contract that every `Platform` implementation must honour, so a divergence
//! fails a test instead of being discovered by an embedder.
//!
//! The contract points covered:
//!   1. Creating a child with an unknown/nonexistent parent returns 0.
//!   2. Creating a child with parent 0 (no parent) returns 0.
//!   3. A failed creation does not allocate an id (a later successful creation
//!      is not "polluted" by the failure).
//!   4. `destroy_widget` on an unknown id returns false and does not panic.
//!   5. Text of an unknown widget is an empty string, never a panic.
//!   6. Geometry/visibility setters on an unknown widget are safe no-ops.

use crate::platform::{get_platform, Platform};

/// (1) An unknown parent must be rejected with `0`.
#[test]
fn contract_unknown_parent_is_rejected() {
    let platform = get_platform();
    platform.init();

    // There is no public "contains" query; the observable proof that the id is
    // nonexistent is that its text is empty and destroying it reports false.
    let bogus_parent = 987_654_321u64;
    assert!(Platform::get_widget_text(platform, bogus_parent).is_empty());
    assert!(!Platform::destroy_widget(platform, bogus_parent));

    let created = Platform::create_button(platform, bogus_parent, "x", 0, 0, 40, 20);
    assert_eq!(
        created,
        0,
        "creating a child under an unknown parent must return 0 ({} backend)",
        platform.backend_name()
    );
}

/// (2) Parent `0` is not a valid parent either.
#[test]
fn contract_zero_parent_is_rejected() {
    let platform = get_platform();
    platform.init();
    let created = Platform::create_label(platform, 0, "label", 0, 0, 40, 20);
    assert_eq!(
        created,
        0,
        "parent 0 must be rejected with 0 ({} backend)",
        platform.backend_name()
    );
}

/// (3) A rejected creation must not hand out a usable handle.
#[test]
fn contract_rejected_creation_yields_no_usable_handle() {
    let platform = get_platform();
    platform.init();

    let rejected = Platform::create_checkbox(platform, 4_242_424, "x", 0, 0, 10, 10);
    assert_eq!(rejected, 0, "rejected creation must return 0");

    // The zero handle must not behave like a live widget.
    assert!(
        !Platform::is_widget_visible(platform, rejected),
        "the failure handle 0 must not refer to a visible widget"
    );
    assert_eq!(
        Platform::get_widget_text(platform, rejected),
        "",
        "reading text from the failure handle must be empty, not a dangling read"
    );
}

/// (4) `destroy_widget` must tolerate unknown ids.
#[test]
fn contract_destroy_unknown_id_is_false_not_panic() {
    let platform = get_platform();
    platform.init();
    assert!(
        !Platform::destroy_widget(platform, 5_555_555),
        "destroying an unknown id must report false ({} backend)",
        platform.backend_name()
    );
    assert!(!Platform::destroy_widget(platform, 0), "destroying handle 0 must report false");
}

/// (5) Reading text from an unknown widget is empty, never a panic.
#[test]
fn contract_unknown_widget_text_is_empty() {
    let platform = get_platform();
    platform.init();
    let text = Platform::get_widget_text(platform, 6_666_666);
    assert!(text.is_empty(), "unknown widget text must be empty, got {text:?}");
}

/// (6) State setters on unknown widgets must be safe no-ops.
#[test]
fn contract_setters_on_unknown_widget_are_safe() {
    let platform = get_platform();
    platform.init();

    let unknown = 7_777_777u64;
    Platform::set_widget_text(platform, unknown, "x");
    Platform::set_widget_geometry(platform, unknown, 1, 2, 3, 4);
    Platform::set_widget_enabled(platform, unknown, false);
    Platform::hide_widget(platform, unknown);
    Platform::show_widget(platform, unknown);

    // None of the above may have conjured a widget into existence: a created
    // widget defaults to visible, so a still-invisible id was never created.
    assert!(
        !Platform::is_widget_visible(platform, unknown),
        "setters on an unknown id must not create it ({} backend)",
        platform.backend_name()
    );
    assert_eq!(
        Platform::get_widget_text(platform, unknown),
        "",
        "text must still be empty after setters on an unknown id"
    );
}

/// A destroy/recreate cycle must produce a *distinct* id, so a stale handle from
/// a destroyed widget can never alias a newly created one.
#[test]
fn contract_recreate_does_not_reuse_destroyed_id() {
    let platform = get_platform();
    platform.init();
    let window = Platform::create_window(platform, "ids", 0, 0, 300, 200);

    let first = Platform::create_button(platform, window, "a", 0, 0, 20, 20);
    assert_ne!(first, 0);
    assert!(Platform::destroy_widget(platform, first));

    let second = Platform::create_button(platform, window, "b", 0, 0, 20, 20);
    assert_ne!(second, 0);
    assert_ne!(
        first, second,
        "ids must not be recycled: a stale handle would alias the new widget"
    );

    // The stale handle must stay dead.
    assert!(!Platform::destroy_widget(platform, first), "the stale id must still be gone");
    assert!(Platform::destroy_widget(platform, second), "the new widget must be live");
}

// ---------------------------------------------------------------------------
// Run the same contract against every backend constructible on this host.
//
// A contract that only holds for the currently-selected backend is not a
// contract. `StubPlatform` and the other feature-gated state backends can all
// be instantiated directly, so the behaviour is asserted for each of them.
// ---------------------------------------------------------------------------

/// Asserts the whole contract suite against one `Platform` instance.
fn assert_contract(platform: &dyn Platform) {
    let backend = platform.backend_name();
    platform.init();

    // (1) unknown parent is rejected
    let bogus = 987_654_321u64;
    assert_eq!(
        Platform::create_button(platform, bogus, "x", 0, 0, 40, 20),
        0,
        "{backend}: unknown parent must be rejected"
    );

    // (2) parent 0 is rejected
    assert_eq!(
        Platform::create_label(platform, 0, "x", 0, 0, 40, 20),
        0,
        "{backend}: parent 0 must be rejected"
    );

    // (3) unknown-id operations are safe no-ops
    let unknown = 7_777_777u64;
    Platform::set_widget_text(platform, unknown, "x");
    Platform::set_widget_geometry(platform, unknown, 1, 2, 3, 4);
    Platform::hide_widget(platform, unknown);
    Platform::show_widget(platform, unknown);
    assert!(
        !Platform::is_widget_visible(platform, unknown),
        "{backend}: setters must not materialise an unknown id"
    );
    assert_eq!(Platform::get_widget_text(platform, unknown), "", "{backend}: unknown text");

    // (4) destroy tolerates unknown ids
    assert!(!Platform::destroy_widget(platform, unknown), "{backend}: destroy unknown -> false");
    assert!(!Platform::destroy_widget(platform, 0), "{backend}: destroy 0 -> false");

    // (5) a live round-trip, then teardown
    let window = Platform::create_window(platform, "w", 0, 0, 200, 150);
    assert_ne!(window, 0, "{backend}: window creation");
    let child = Platform::create_button(platform, window, "b", 0, 0, 20, 20);
    assert_ne!(child, 0, "{backend}: child creation under a valid parent");
    Platform::set_widget_text(platform, child, "hello");
    assert_eq!(Platform::get_widget_text(platform, child), "hello", "{backend}: text round-trip");
    assert!(Platform::destroy_widget(platform, child), "{backend}: destroy live child");
    assert_eq!(
        Platform::get_widget_text(platform, child),
        "",
        "{backend}: destroyed widget must not retain text"
    );
    assert!(!Platform::destroy_widget(platform, child), "{backend}: destroy is idempotent");
}

#[test]
fn contract_holds_for_stub_platform() {
    let platform =
        crate::platform::StubPlatform::new("contract-stub", crate::core::PlatformFamily::Desktop);
    assert_contract(&platform);
}

#[test]
fn contract_holds_for_selected_backend() {
    assert_contract(get_platform());
}

#[cfg(feature = "harmony")]
#[test]
fn contract_holds_for_harmony_platform() {
    use crate::platform::harmony::HarmonyPlatform;
    let platform = HarmonyPlatform::new();
    assert_contract(&platform);
}

#[cfg(feature = "wasm")]
#[test]
fn contract_holds_for_wasm_platform() {
    use crate::platform::wasm::WasmPlatform;
    let platform = WasmPlatform::default();
    assert_contract(&platform);
}

/// Child-control creation must reject an unknown parent on every backend.
///
/// This covers the control kinds that are easy to forget when a backend adds a
/// new `create_*` method: the state-backed macOS backend previously ignored the
/// parent for SpinBox / ListView / ScrollArea while validating it for every
/// other control, so a caller could get a parentless widget back.
#[test]
fn contract_child_controls_reject_unknown_parent_stub() {
    let platform =
        crate::platform::StubPlatform::new("contract-stub", crate::core::PlatformFamily::Desktop);
    assert_child_controls_reject_unknown_parent(&platform);
}

#[test]
fn contract_child_controls_reject_unknown_parent_selected() {
    assert_child_controls_reject_unknown_parent(get_platform());
}

/// Every child-control creator must return `0` for a nonexistent parent.
fn assert_child_controls_reject_unknown_parent(platform: &dyn Platform) {
    let backend = platform.backend_name();
    platform.init();
    let bogus = 424_242u64;

    macro_rules! rejects {
        ($label:literal, $call:expr) => {{
            assert_eq!($call, 0, "{backend}: {} must reject an unknown parent", $label);
        }};
    }

    rejects!("button", Platform::create_button(platform, bogus, "b", 0, 0, 10, 10));
    rejects!("checkbox", Platform::create_checkbox(platform, bogus, "c", 0, 0, 10, 10));
    rejects!("label", Platform::create_label(platform, bogus, "l", 0, 0, 10, 10));
    rejects!("line_edit", Platform::create_line_edit(platform, bogus, "e", 0, 0, 10, 10));
    rejects!("radio_button", Platform::create_radio_button(platform, bogus, "r", 0, 0, 10, 10));
    rejects!("slider", Platform::create_slider(platform, bogus, 0, 0, 10, 10));
    rejects!("progress_bar", Platform::create_progress_bar(platform, bogus, 0, 0, 10, 10));
    rejects!("combo_box", Platform::create_combo_box(platform, bogus, 0, 0, 10, 10));
    rejects!("list_box", Platform::create_list_box(platform, bogus, 0, 0, 10, 10));
    rejects!("panel", Platform::create_panel(platform, bogus, 0, 0, 10, 10));
    rejects!("spin_box", Platform::create_spin_box(platform, bogus, 0, 0, 10, 10));
    rejects!("list_view", Platform::create_list_view(platform, bogus, 0, 0, 10, 10));
    rejects!("scroll_area", Platform::create_scroll_area(platform, bogus, 0, 0, 10, 10));
    rejects!("group_box", Platform::create_group_box(platform, bogus, "g", 0, 0, 10, 10));
    rejects!("frame", Platform::create_frame(platform, bogus, 0, 0, 10, 10));
    rejects!("tab_widget", Platform::create_tab_widget(platform, bogus, 0, 0, 10, 10));
    rejects!("splitter", Platform::create_splitter(platform, bogus, 0, 0, 10, 10));
}

#[cfg(feature = "macos")]
#[test]
fn contract_holds_for_macos_objc2_platform() {
    use crate::platform::macos_objc2::MacOSObjc2Platform;
    let platform = MacOSObjc2Platform::new();
    assert_contract(&platform);
}

#[cfg(feature = "mobile-api")]
#[test]
fn contract_holds_for_mobile_platform() {
    use crate::platform::mobile::AndroidMobilePlatform;
    let platform = AndroidMobilePlatform::new();
    assert_contract(&platform);
}
