// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The suite-wide event contract: every published name resolves on a real control.
//!
//! The parent crate's `tests/event_wiring_test.rs` audits the controls the parent registers.
//! When a group moves here its capability row moves with it, so that gate can no longer see it:
//! the row is no longer in the parent's registry. This test closes the same loop from the side
//! that owns the row — it builds every group's controls through the factory and asks each one for
//! every event its own capability publishes.
//!
//! A name that resolves nowhere is the defect principle #97 names: `connect_event` would answer
//! `Ok` for a name nobody can subscribe to, and the wire would be silently inert.

use rust_widgets::core::Rect;
use rust_widgets::widget::capability::WidgetFactory;

/// Builds a factory with every enabled suite group installed.
fn suite_factory() -> WidgetFactory {
    let mut factory = WidgetFactory::new_with_defaults();
    crate::install(&mut factory).expect("the suite installs into a fresh factory");
    factory
}

/// Asserts every published event of every row is reachable from a constructed control.
fn assert_group(
    factory: &mut WidgetFactory,
    group: &str,
    rows: Vec<(&'static str, Vec<&'static str>)>,
) {
    for (control, events) in rows {
        let Some(widget) = factory.create(control, Rect::new(0, 0, 240, 120), "") else {
            panic!("{group}: `{control}` is published but the factory cannot build it")
        };
        assert_eq!(
            factory.capability_for_kind_instance(widget.as_ref()).map(|row| row.canonical_name),
            Some(control),
            "{group}: the factory must resolve `{control}` to its own capability row, not a \
             sibling that shares its kind"
        );
        for event in events {
            assert!(
                widget.event_signal_dyn(event).is_some(),
                "{group}: `{control}` publishes `{event}` but `event_signal_dyn` does not resolve \
                 it, so a subscription would succeed and never fire"
            );
        }
    }
}

#[test]
fn every_published_suite_event_resolves_on_its_control() {
    let mut factory = suite_factory();

    let (form_events, form_set_events) = crate::form_widgets::published_event_names();
    assert_group(&mut factory, "form", vec![("form", form_events), ("form_set", form_set_events)]);

    #[cfg(feature = "webview")]
    assert_group(&mut factory, "webview", crate::webview::published_event_names());

    #[cfg(feature = "finance")]
    assert_group(&mut factory, "finance", crate::finance::published_event_names());

    #[cfg(feature = "maps")]
    assert_group(&mut factory, "maps", crate::maps::published_event_names());

    #[cfg(feature = "project")]
    assert_group(&mut factory, "project", crate::project::published_event_names());

    #[cfg(feature = "devtools")]
    assert_group(&mut factory, "devtools", crate::devtools::published_event_names());

    #[cfg(feature = "media-playback")]
    assert_group(&mut factory, "media-playback", crate::media_playback::published_event_names());
}
