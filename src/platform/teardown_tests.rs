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

use crate::platform::{get_platform, Platform};

/// `destroy_widget` reports existence correctly and is idempotent.
#[test]
fn destroy_widget_is_authoritative_and_idempotent() {
    let platform = get_platform();
    platform.init();
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
    let platform = get_platform();
    platform.init();
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
#[test]
fn destroy_widget_keeps_widget_set_stable_across_churn() {
    let platform = get_platform();
    platform.init();

    for round in 0..25 {
        let mut ids = Vec::with_capacity(40);
        for _ in 0..40 {
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
    let platform = get_platform();
    platform.init();

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
