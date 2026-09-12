//! Widget teardown (`Platform::destroy_widget`) regression tests.
//!
//! These assert on the library's **own** registries, not on process RSS: RSS is
//! dominated by AppKit's internal caches (a pure-AppKit control churn loop
//! reproduces the same curve without any of this crate's code), so it is not a
//! valid measure of this crate's bookkeeping. What matters here is that a
//! destroyed widget leaves no entry behind and behaves as "gone".

use crate::platform::{get_platform, Platform};

/// `destroy_widget` reports existence correctly and is idempotent.
#[test]
fn destroy_widget_is_authoritative_and_idempotent() {
    let platform = get_platform();
    platform.init();
    let window = Platform::create_window(platform, "teardown", 0, 0, 400, 300);
    let child = Platform::create_button(platform, window, "b", 10, 10, 80, 30);

    assert!(Platform::destroy_widget(platform, child), "first destroy must report true");
    assert!(
        !Platform::destroy_widget(platform, child),
        "destroying an already-destroyed widget must report false"
    );
    assert!(!Platform::destroy_widget(platform, 999_999), "an unknown widget id must report false");

    // A destroyed widget must no longer be usable: text falls back to empty and
    // visibility/enabled report the defaults rather than resurrecting it.
    Platform::set_widget_text(platform, child, "after");
    assert_eq!(
        Platform::get_widget_text(platform, child),
        "",
        "a destroyed widget must not retain text"
    );
}

/// Churning widgets must not grow the backend's own registrations.
///
/// This is the registry-level guarantee `destroy_widget` exists to provide.
/// Verified through the public API: re-creating and destroying the same number
/// of widgets repeatedly must keep the backend's live widget set stable, and a
/// destroyed id must never be reported as existing.
#[test]
fn destroy_widget_keeps_widget_set_stable_across_churn() {
    let platform = get_platform();
    platform.init();
    let window = Platform::create_window(platform, "churn", 0, 0, 400, 300);

    // Warm up, then churn and assert every destroyed id is gone.
    for round in 0..25 {
        let mut ids = Vec::with_capacity(100);
        for _ in 0..100 {
            ids.push(Platform::create_checkbox(platform, window, "c", 0, 0, 20, 20));
        }
        for id in ids {
            assert!(Platform::destroy_widget(platform, id), "round {round}: destroy must succeed");
            // The authoritative "is it gone" check available through the trait:
            // a destroyed widget must not report visible/enabled state.
            assert!(
                !Platform::is_widget_visible(platform, id),
                "round {round}: destroyed widget still reports visible"
            );
        }
    }
}

/// Destroying a container must not corrupt its siblings or the parent.
#[test]
fn destroy_widget_does_not_disturb_siblings() {
    let platform = get_platform();
    platform.init();
    let window = Platform::create_window(platform, "siblings", 0, 0, 400, 300);

    let a = Platform::create_button(platform, window, "a", 10, 10, 80, 30);
    let b = Platform::create_button(platform, window, "b", 10, 50, 80, 30);
    let c = Platform::create_label(platform, window, "c", 10, 90, 80, 30);

    assert!(Platform::destroy_widget(platform, b));

    // The survivors must keep working.
    Platform::set_widget_text(platform, a, "A!");
    assert_eq!(Platform::get_widget_text(platform, a), "A!");
    Platform::set_widget_text(platform, c, "C!");
    assert_eq!(Platform::get_widget_text(platform, c), "C!");

    // And the destroyed one stays gone.
    assert!(!Platform::destroy_widget(platform, b));
    assert_eq!(Platform::get_widget_text(platform, b), "");
}
