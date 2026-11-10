// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Widget teardown (`Platform::destroy_widget`) regression tests.
//!
//! These assert on the library's **own** registries, not on process RSS: RSS is
//! dominated by AppKit's internal caches (a pure-AppKit control churn loop
//! reproduces the same curve without any of this crate's code), so it is not a
//! valid measure of this crate's bookkeeping. What matters here is that a
//! destroyed id leaves no entry behind and behaves as "gone".
//!
//! # What changed and why
//!
//! These tests used to churn `create_button`/`create_checkbox` and assert the
//! backend freed a real control handle. The library paints controls now, so there
//! are no host handles to churn — the lifecycle that remains is the **window**,
//! which every host still creates and destroys. Asserting the control case would
//! either leave the tests red or force a shadow control registry back into the
//! backend, which is the duplication BLUE15 removed.
//!
//! The window lifecycle covers the same property the original tests were after:
//! `destroy` reports existence truthfully, is idempotent, leaks no registration
//! across churn, and does not disturb ids that were never destroyed.
//!
//! # Why each test gets its own host
//!
//! [`host`] builds a private instance of the backend this build resolves to. The process-global one
//! is a `OnceLock` shared by the whole test binary, and a host whose model is one window
//! (`harmony`, `android`, `wasm`) answers `0` to a second live `create_window` (BLUE28 B28.43), so a
//! window another test had already opened made these fail with "must create a window" — a false
//! finding about a correctly behaving backend. See `platform::contract_tests` for the measurement.

use crate::platform::Platform;

/// A private instance of the backend this host resolves to, initialised.
fn host() -> Box<dyn Platform> {
    let platform = crate::platform::runtime::fresh_native_platform();
    platform.init();
    platform
}

/// `destroy_widget` reports existence correctly and is idempotent.
#[test]
fn destroy_widget_is_authoritative_and_idempotent() {
    let host = host();
    let platform = host.as_ref();

    let window = Platform::create_window(platform, "teardown", 0, 0, 400, 300);
    assert_ne!(window, 0, "{} must create a window", platform.backend_name());

    assert!(Platform::destroy_widget(platform, window), "first destroy must report true");
    assert!(
        !Platform::destroy_widget(platform, window),
        "destroying an already-destroyed id must report false"
    );
    assert!(!Platform::destroy_widget(platform, 999_999), "an unknown id must report false");
}

/// Ids that were never handed out must never be reported as existing.
///
/// A backend that answered `true` here would make `destroy_widget` useless as an
/// existence probe, which is the contract callers rely on to detect a dropped
/// handle.
#[test]
fn unsupported_and_unknown_ids_report_absence() {
    let host = host();
    let platform = host.as_ref();

    let window = Platform::create_window(platform, "w", 0, 0, 400, 300);
    assert_ne!(window, 0);

    // A control this host does not provide yields no id; there is then nothing for
    // `destroy_widget` to find, and it must say so rather than inventing an entry.
    let control = Platform::create_button(platform, window, "b", 10, 10, 80, 30);
    assert!(
        !Platform::destroy_widget(platform, control),
        "destroying an id that was never issued must report false ({})",
        platform.backend_name()
    );
}

/// Churn must not grow the backend's own registrations.
///
/// This is the registry-level guarantee `destroy_widget` exists to provide: after
/// creating and destroying the same number of windows repeatedly, every destroyed
/// id must be gone and each fresh `create_window` must still succeed.
///
/// # Why the batch follows `supports_multiple_windows`
///
/// The property is about *churn*, not concurrency: create and destroy repeatedly, and every fresh
/// `create_window` must still succeed. A host whose model is one window (`harmony`, `android`,
/// `wasm`) refuses a second **live** window by design (BLUE28 B28.43), so asking it for forty at
/// once would test the documented refusal rather than the leak this test exists for, and would fail
/// on a backend that is behaving correctly. The batch is therefore what the host says it can hold at
/// once — forty for a window-table backend, one otherwise — and the loop still proves no
/// registration survives its destroy.
#[test]
fn destroy_widget_keeps_widget_set_stable_across_churn() {
    let host = host();
    let platform = host.as_ref();

    let batch = if platform.supports_multiple_windows() { 40 } else { 1 };
    for round in 0..25 {
        let mut ids = Vec::with_capacity(batch);
        for _ in 0..batch {
            ids.push(Platform::create_window(platform, "churn", 0, 0, 200, 150));
        }
        for id in ids {
            assert_ne!(id, 0, "round {round}: window creation must keep succeeding under churn");
            assert!(Platform::destroy_widget(platform, id), "round {round}: destroy must succeed");
            assert!(
                !Platform::destroy_widget(platform, id),
                "round {round}: a destroyed id must stay gone"
            );
        }
    }
}

/// Destroying one id must not disturb another that is still live.
#[test]
fn destroy_widget_does_not_disturb_siblings() {
    let host = host();
    let platform = host.as_ref();

    // A host that owns one window cannot hold the three this test is about. Its case is answered
    // with the property that *is* checkable on it: a second live window is refused (BLUE28 B28.43),
    // and destroying the first frees the slot again — the same "destroy one id, another id's fate is
    // unchanged" claim, expressed with the one sibling such a host has.
    if !platform.supports_multiple_windows() {
        let first = Platform::create_window(platform, "a", 0, 0, 300, 200);
        assert_ne!(first, 0, "{} must create its one window", platform.backend_name());
        assert_eq!(
            Platform::create_window(platform, "b", 0, 0, 300, 200),
            0,
            "a single-window host must refuse a second live window, not alias the first"
        );
        assert!(Platform::destroy_widget(platform, first), "the first window must still exist");
        let third = Platform::create_window(platform, "c", 0, 0, 300, 200);
        assert_ne!(third, 0, "destroying the only window must free the slot for the next one");
        assert_ne!(third, first, "the freed slot must not hand the destroyed id back");
        assert!(Platform::destroy_widget(platform, third));
        return;
    }

    let a = Platform::create_window(platform, "a", 0, 0, 300, 200);
    let b = Platform::create_window(platform, "b", 30, 30, 300, 200);
    let c = Platform::create_window(platform, "c", 60, 60, 300, 200);
    assert!(a != 0 && b != 0 && c != 0);

    assert!(Platform::destroy_widget(platform, b));
    assert!(!Platform::destroy_widget(platform, b), "the destroyed id stays gone");

    // The survivors must still exist: destroying one window must not have taken the
    // others with it. `destroy_widget` is the existence probe every host supports,
    // so it is the honest way to assert this across backends.
    assert!(
        Platform::destroy_widget(platform, a),
        "sibling 'a' must still exist after 'b' was destroyed"
    );
    assert!(
        Platform::destroy_widget(platform, c),
        "sibling 'c' must still exist after 'b' was destroyed"
    );
    assert!(!Platform::destroy_widget(platform, b), "the destroyed window 'b' must stay gone");
}
