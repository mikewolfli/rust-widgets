// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Cross-control wiring tests for the media-playback group.
//!
//! These used to live in the parent crate's `signal::event_bridge` tests; they pin behaviour that
//! only exists between two migrated controls (a shared `WidgetKind` and its per-capability
//! wiring table), so they moved with the controls.

use rust_widgets::core::Rect;
use rust_widgets::signal::EventSignalBinder;
use rust_widgets::widget::capability::WidgetFactory;

/// A control sharing a `WidgetKind` with a larger control must not inherit its shortfall.
///
/// # The defect this pins
///
/// `WidgetKind` is not one-to-one with capability: `WidgetKind::VideoPlayer` backs both
/// `video_player` (18 published events) and `chapter_list_panel` (1). The wiring-outcome table
/// was keyed by kind alone and folded the two counts with `max` **independently**, so a
/// `ChapterListPanel` wired from an entry that already held `video_player`'s larger
/// `published` reported 18 unwired events that do not exist — precisely the false shortfall
/// this query exists to make impossible.
///
/// The two controls must share **one binder** for the defect to appear: the table is
/// per-binder, so a fresh binder for each control would hide the cross-contamination
/// entirely. That is also what a real host looks like — one binder wires every control in a
/// window. The **larger** control goes first, which is the order that exposes it.
#[test]
fn a_control_on_a_shared_kind_reports_its_own_shortfall() {
    // A suite control lives in the suite's own factory, which the shared table does not
    // include. The factory-aware binder forms are what let this test (and a host that uses
    // `new_with_extensions`) resolve the capability row deterministically.
    let mut factory = WidgetFactory::new_with_defaults();
    crate::install(&mut factory).expect("the suite installs into a fresh factory");
    let r = Rect::new(0, 0, 100, 40);
    let hub = std::sync::Arc::new(rust_widgets::signal::CustomSignalHub::new());
    let mut binder = EventSignalBinder::new(hub);

    let player = crate::media_playback::video_player::VideoPlayer::new(r);
    assert_eq!(
        binder.forward_all_with(&factory, &player),
        18,
        "`video_player` publishes eighteen events after `on_chapter_entered` (MW-002)"
    );

    let panel = crate::media_playback::chapter_list_panel::ChapterListPanel::new(r);
    assert_eq!(
        binder.unwired_events_for_with(&factory, &panel),
        Some(1),
        "the shortfall is this control's own single event, not `video_player`'s eighteen \
             folded in through a shared `WidgetKind`"
    );
    // The larger sibling is still fully wired; neither answer may be read off the other.
    assert_eq!(binder.unwired_events_for_with(&factory, &player), Some(0));
}
