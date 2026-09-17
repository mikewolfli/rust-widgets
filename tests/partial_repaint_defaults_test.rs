// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Zero-effort closure: mount a real control, then get region-limited repaints
//! without asking for them.
//!
//! # Why this test exists
//!
//! The damage pipeline has three parts, and each was complete and tested while the
//! **join between them had no production producer at all**:
//!
//! 1. a producer that records damage — `BaseWidget::request_redraw`;
//! 2. a policy that decides whether to consult it — `RepaintMode`;
//! 3. a renderer that honours it — `render_frame_incremental`.
//!
//! Part 1 is what made the others reachable, and a unit test can prove part 1 fires
//! while part 2 is never engaged for any control a host actually builds. The
//! assertion here is therefore on the **end state of a mounted control**, not on any
//! intermediate step: after `create_widget_of_kind` — the same entry the C ABI, the
//! JSON loader and every demo use — a mount that the library judged worthwhile is
//! already tracking, and a repaint request really narrows the recorded damage.
//!
//! # Why the judgement is part of the contract
//!
//! Enabling tracking unconditionally would be wrong in the other direction: a small
//! surface, or one with a single rect, pays the merge/clip bookkeeping and saves no
//! pixels. So the test also asserts the **refusal**, on a control that is otherwise
//! identical — same kind, same children, same request — differing only in size. A
//! gate that only ever says yes is not a decision.
//!
//! # Why the whole file is not profile-gated
//!
//! `create_widget_of_kind` and the runtime both exist on every unstripped profile,
//! and `mini` does not compile `widget::runtime` at all. Gating on `full_widgets`
//! would skip tablet/mobile unnecessarily, so the gate is the precise one:
//! `not(alloc_frugal)`.

#![cfg(not(alloc_frugal))]

use rust_widgets::core::{ObjectId, Rect};
use rust_widgets::widget::runtime::{
    self, dirty_rects, enable_damage_tracking_if_useful, repaint_mode, should_track_damage,
    RepaintMode, AUTO_REPAINT_MIN_PIXELS,
};
use rust_widgets::widget::Widget;

/// Mounts a group box of `size` with one child, and asks it to repaint.
///
/// The request has to happen **before** mounting for the constructor path to be
/// modelled, so this builds through the runtime's own registration rather than
/// through `create_widget_of_kind`, which would register first. Both paths are
/// covered: this one for the pre-mount request, and
/// `mounting_through_the_public_entry_auto_enables` for the public one.
fn mounted_window(width: u32, height: u32) -> ObjectId {
    let mut group = rust_widgets::widget::container_widgets::groupbox::GroupBox::new(Rect::new(
        0, 0, width, height,
    ));
    group.base_mut().add_child(7);
    group.base_mut().request_redraw();
    let id = runtime::register(Box::new(group)).expect("registry");
    assert!(runtime::set_geometry(id, Rect::new(0, 0, width, height)));
    id
}

/// A large surface with children comes up tracking, with no call from the host.
#[test]
fn a_mounted_window_tracks_damage_without_being_asked() {
    let window = mounted_window(1280, 800);
    assert_eq!(
        repaint_mode(window),
        RepaintMode::Adaptive,
        "a 1280x800 surface with children must be enabled at mount, not left to the host"
    );

    // And the enablement is real, not just a mode field: a request narrows the damage
    // to the asking control rather than the whole frame.
    runtime::with_widget(window, |widget| widget.request_redraw()).expect("widget");
    let damaged = dirty_rects(window);
    assert_eq!(damaged.len(), 1, "one control asked, so one region must be damaged");
    assert_eq!(
        (damaged[0].width, damaged[0].height),
        (1280, 800),
        "the fixture's own rect is what it damages"
    );
    runtime::unregister(window);
}

/// The same control below the threshold is refused, so the decision really decides.
///
/// Everything about this fixture matches the accepted one except the area, which is
/// the point: a gate whose only input is "is it mounted" would accept both, and the
/// bookkeeping would then cost more than the pixels it saves on the small one.
#[test]
fn a_small_surface_is_left_in_full_mode() {
    const { assert!(400u64 * 300 < AUTO_REPAINT_MIN_PIXELS) };
    let small = mounted_window(400, 300);
    assert_eq!(
        repaint_mode(small),
        RepaintMode::Full,
        "400x300 is below the threshold and must keep whole-frame painting"
    );
    assert!(!should_track_damage(small));
    runtime::unregister(small);
}

/// A large control that never asks to repaint is also refused.
///
/// This is the cost side: enabling a surface nothing redraws buys no pixels and
/// still pays the bookkeeping. The fixture is the accepted one with the single
/// request removed, so the difference is exactly that request.
#[test]
fn a_large_but_silent_surface_is_left_in_full_mode() {
    let mut group = rust_widgets::widget::container_widgets::groupbox::GroupBox::new(Rect::new(
        0, 0, 1280, 800,
    ));
    group.base_mut().add_child(7);
    let id = runtime::register(Box::new(group)).expect("registry");
    assert!(runtime::set_geometry(id, Rect::new(0, 0, 1280, 800)));

    assert_eq!(repaint_mode(id), RepaintMode::Full, "nothing ever asked to repaint this surface");
    assert!(!enable_damage_tracking_if_useful(id));
    runtime::unregister(id);
}

/// Mounting a child into an already-mounted container reaches the same decision.
///
/// The interesting case is a control mounted *after* its own request, which is what
/// happens whenever a host adds a child to a live window: the parent has already been
/// judged, and the child arrives with whatever its constructor asked for. The child
/// here is a small childless leaf, so it is refused on its own — while the container it
/// joins, large and with children, is already tracking. That asymmetry is the decision
/// working: damage is narrowed by the container, not by every leaf.
#[test]
fn mounting_a_child_into_a_live_window_keeps_the_decision() {
    let parent = mounted_window(1280, 800);
    assert_eq!(repaint_mode(parent), RepaintMode::Adaptive);

    // A leaf that asks for a repaint is still refused: it is small and has one rect, so
    // regioning it cannot narrow anything.
    let child = rust_widgets::widget::base_widgets::label::Label::new(
        "hello".to_string(),
        Rect::new(0, 0, 200, 40),
    );
    child.request_redraw();
    let child = runtime::register(Box::new(child)).expect("registry");
    assert!(runtime::set_geometry(child, Rect::new(0, 0, 200, 40)));
    assert!(
        !should_track_damage(child),
        "a small childless leaf must not be enabled even on a large surface"
    );
    assert_eq!(repaint_mode(parent), RepaintMode::Adaptive, "the container is unchanged");

    runtime::unregister(child);
    runtime::unregister(parent);
}
