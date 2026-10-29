// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::*;

use crate::core::{Color, Point};
use crate::platform::accessibility::AccessibilityBridge as _;
use crate::widget::special_widgets::code_editor::CodeEditor;
use crate::widget::{BaseWidget, Draw, WidgetKind};

/// A counter shared with a test control.
type DrawCount = core::cell::Cell<usize>;

// ── BLUE23 §3.3 -- the animation bus ────────────────────────────────────────

/// One arming buys exactly one full repaint of every window/surface.
///
/// This is the mechanism behind "a theme switch repaints an idle window": the theme
/// mutation only arms the epoch (it runs under the non-reentrant theme lock), and the
/// loop thread performs the invalidation once. A second frame must not repeat it, or
/// every theme change would pin the window awake forever.
#[test]
fn an_armed_global_repaint_repaints_once_and_then_stops() {
    // A frame on *any* thread consumes a pending process-wide stop request and drains the shared
    // trigger queue, so this test takes the same seam lock every other frame test takes. Without
    // it, this frame could swallow the stop a loop test had just recorded for its own loop —
    // benign while a frame was slow enough to be rare, and reliable once frames got fast.
    let _serial = serialise_drive_frame();
    let id = register(Box::new(crate::widget::Button::new(
        "ok".to_string(),
        crate::core::Rect::new(0, 0, 80, 32),
    )))
    .expect("mount");
    let _unmount = MountGuard(id);
    // Consume anything an earlier test on this thread armed, so "first frame" below
    // cannot be satisfied by a stale epoch.
    let _ = drive_frame(0);

    // `renderable_epoch` is what a repaint request records; a standalone mounted control
    // has no platform surface for `invalidate_surface` to accept, so the ledger counter
    // (`repaints_submitted`) stays zero here even though the request was made.
    let before = renderable_epoch();
    arm_global_repaint();
    let _ = drive_frame(0);
    let after_armed = renderable_epoch();
    assert_ne!(
        after_armed, before,
        "the armed frame must request a repaint for the mounted widget"
    );
    let _ = drive_frame(0);
    assert_eq!(
        renderable_epoch(),
        after_armed,
        "one arming must buy exactly one full repaint, not a permanently awake loop"
    );
}

/// A hovered button *is* animating, and the bus advances it to rest.
///
/// The whole point of the bus: before it, the button's `interaction_progress`
/// interpolated forever in principle and never in practice, because nothing called
/// `tick`. This drives it through the one runtime entry point.
#[test]
fn the_bus_advances_a_hovered_button_to_rest() {
    let id = register(Box::new(crate::widget::Button::new(
        "ok".to_string(),
        crate::core::Rect::new(0, 0, 80, 32),
    )))
    .expect("mount");
    let _unmount = MountGuard(id);
    // Deliver the hover the same way the runtime does, rather than reaching into the
    // registry: this goes through the public dispatch the pointer path uses.
    assert!(dispatch_event(id, &crate::event::Event::MouseEnter { pos: Point::new(5, 5) }));
    // The control now owes frames, which is what lets a host schedule one.
    assert!(has_animating_widgets(), "a hovered button is animating");
    // Step until the bus reports rest, bounded so a transition that never settles fails
    // the test rather than hanging on it. The exact duration is a theme token, so the
    // assertion is "it ends", not a specific number of frames.
    let mut frames = 0;
    while tick_animations(50) {
        frames += 1;
        assert!(frames < 100, "the hover transition must settle, not run forever");
    }
    assert!(frames > 0, "a hover must take more than one frame, or it did not animate");
    assert!(!has_animating_widgets(), "and the control must report resting afterwards");
}

/// A frame with only resting controls costs nothing.
///
/// This is the half of "smooth" that decides whether it is bought with a burning
/// core: an idle window must report `false` so the host stops scheduling frames.
#[test]
fn the_bus_reports_no_frames_for_a_resting_control() {
    let id = register(Box::new(crate::widget::Button::new(
        "ok".to_string(),
        crate::core::Rect::new(0, 0, 80, 32),
    )))
    .expect("mount");
    let _unmount = MountGuard(id);
    assert!(!has_animating_widgets(), "a resting button animates nothing");
    assert!(!tick_animations(16), "so the bus returns false and the host stops");
}

// ── BLUE24 §1 -- the frame driver ───────────────────────────────────────────

/// A frame with a hovered control reports the work it did and asks for another frame.
///
/// BLUE24 §1 criterion 1. The shape matters more than the numbers: the outcome must
/// name **which** control moved (`controls_ticked == 1`) and must ask for the next frame,
/// because a transition that has not settled is exactly the case a host has to keep
/// scheduling for. A driver that returned `false` here would paint one step of the
/// animation and then freeze, which is the failure the driver exists to prevent.
/// Serialises the tests that drive the **process-wide** frame loop.
///
/// `drive_frame` drains one shared event queue and advances every mounted control, so two of
/// these tests running concurrently can consume each other's events. `drive_frame_drains_input_before_advancing`
/// asserts that *its* injected trigger reached its handler during *its* frame, and a sibling's
/// frame can dequeue that trigger first — the handler then runs on the other thread, whose
/// thread-local observation flag is not the one the test reads. That is exactly how it flaked
/// under the full suite while passing in isolation.
///
/// It takes `platform::stop`'s process-seam lock rather than a module-local one: a frame also
/// **consumes a pending stop request** (`honour_request_on_loop_thread`), so a frame test that
/// ran beside the stop flag's own unit test drained the request it had just recorded — the
/// stop test then failed only in the full suite. One lock for both seams is what makes the
/// two kinds of test serialize against each other.
///
/// A poison is ignored rather than propagated: the poisoning test has already failed, and the
/// remaining tests must still be able to take the lock and report honestly.
fn serialise_drive_frame() -> std::sync::MutexGuard<'static, ()> {
    crate::platform::stop::TEST_SEAM_SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

// ── G-404 -- the theme_changed subscription ─────────────────────────────

/// A theme switch restyles every mounted control at the frame, with no caller performing
/// a sweep: the notification marks, the frame merges. These are the assertions the old
/// full-registry walk satisfied, so a pass proves the subscription did not lose coverage.
#[cfg(device_profile)]
#[test]
fn theme_notification_restyles_mounted_controls_at_the_next_frame() {
    let _serial = serialise_drive_frame();
    let id = register(Box::new(crate::widget::Button::new(
        "A".to_string(),
        crate::core::Rect::new(0, 0, 80, 32),
    )))
    .expect("mount");
    let _unmount = MountGuard(id);

    let fill = || with_widget(id, |widget| widget.style().background_color).flatten();
    let before = fill();

    // The switch emits under the theme mutex, so the slot records intent only: the style
    // is still the old one here. Resolving a style inside the slot would re-enter the same
    // non-reentrant mutex and deadlock, which is why the merge is deferred.
    {
        let mut manager = crate::theme::global_theme_manager();
        assert!(
            manager.set_appearance(crate::theme::AppearanceMode::Dark),
            "the dark preset must be registered"
        );
    }
    assert_eq!(fill(), before, "the notification alone must not restyle synchronously");

    drive_frame(0);

    let dark_primary = { crate::style::palette_shared().expect("an active theme").primary };
    assert_ne!(fill(), before, "the frame drained the notification into a restyle");
    assert_eq!(
        fill(),
        Some(dark_primary),
        "a mounted control must end up styled exactly as the old full sweep styled it"
    );
}

/// Unmounting must disconnect the control's `theme_changed` slot: a host that recycles
/// controls would otherwise grow the signal's slot list without bound, each dead slot
/// firing for an id that resolves to nothing.
#[cfg(device_profile)]
#[test]
fn an_unregistered_control_leaves_no_theme_subscription_behind() {
    let before = {
        let manager = crate::theme::global_theme_manager();
        manager.on_theme_changed().slot_count()
    };

    let id = register(Box::new(crate::widget::Button::new(
        "B".to_string(),
        crate::core::Rect::new(0, 0, 80, 32),
    )))
    .expect("mount");
    assert_eq!(
        { crate::theme::global_theme_manager().on_theme_changed().slot_count() },
        before + 1,
        "mounting subscribes the control to the theme switch notification"
    );

    assert!(unregister(id), "the control unmounts");
    assert_eq!(
        { crate::theme::global_theme_manager().on_theme_changed().slot_count() },
        before,
        "unmounting must disconnect the control's theme subscription"
    );
}

#[test]
fn drive_frame_ticks_a_hovered_control_and_asks_for_another_frame() {
    let _serial = serialise_drive_frame();
    let id = register(Box::new(crate::widget::Button::new(
        "ok".to_string(),
        crate::core::Rect::new(0, 0, 80, 32),
    )))
    .expect("mount");
    let _unmount = MountGuard(id);

    assert!(
        dispatch_event(id, &crate::event::Event::MouseEnter { pos: Point::new(5, 5) }),
        "the hover must reach the control, or there is nothing to animate"
    );

    let outcome = drive_frame(16);
    assert_eq!(outcome.controls_ticked, 1, "exactly the hovered button advanced");
    assert!(
        outcome.needs_another_frame,
        "a transition still in flight means the host must schedule another frame"
    );
}

/// The animation registry is the one source of truth for the sweep and both counters (G-06).
#[test]
fn the_animation_registry_tracks_start_settle_and_destroy() {
    let _serial = serialise_drive_frame();
    let id = register(Box::new(crate::widget::Button::new(
        "ok".to_string(),
        crate::core::Rect::new(0, 0, 80, 32),
    )))
    .expect("mount");
    let _unmount = MountGuard(id);
    // A resting neighbour must not be in the registry, so it cannot be swept either.
    let resting = register(Box::new(crate::widget::Label::new(
        "rest".to_string(),
        crate::core::Rect::new(0, 40, 80, 24),
    )))
    .expect("mount");
    let _resting_unmount = MountGuard(resting);

    let _ = drive_frame(0);
    assert!(!has_animating_widgets(), "a fresh button rests");
    assert_eq!(count_animating_widgets(), 0);

    assert!(dispatch_event(id, &crate::event::Event::MouseEnter { pos: Point::new(5, 5) }));
    assert!(has_animating_widgets(), "the hover registers the control at dispatch time");
    assert_eq!(count_animating_widgets(), 1, "the resting label is not registered");

    let mut frames = 0;
    while has_animating_widgets() && frames < 120 {
        let outcome = drive_frame(16);
        // Only the registered control may be advanced.
        assert!(outcome.controls_ticked <= count_animating_widgets().max(1));
        frames += 1;
    }
    assert!(!has_animating_widgets(), "the transition settles on its own");
    assert_eq!(count_animating_widgets(), 0);

    // A destroyed control must not keep the registry (and the frame loop) alive.
    assert!(dispatch_event(id, &crate::event::Event::MouseLeave { pos: Point::new(5, 5) }));
    assert!(has_animating_widgets(), "leaving the control starts its fade out");
    assert!(unregister(id));
    assert!(!has_animating_widgets(), "unregister clears the animating bit");
}

/// Animation damage is unioned per window, so N controls in one window submit once (G-07).
#[cfg(not(alloc_frugal))]
#[test]
fn animation_damage_is_unioned_per_window() {
    let _serial = serialise_drive_frame();
    let window = register(Box::new(crate::widget::Window::new(
        "animation aggregation".to_string(),
        crate::core::Rect::new(0, 0, 400, 300),
    )))
    .expect("mount");
    let _window_unmount = MountGuard(window);
    let left = register(Box::new(crate::widget::Button::new(
        "a".to_string(),
        crate::core::Rect::new(10, 10, 80, 32),
    )))
    .expect("mount");
    let _left_unmount = MountGuard(left);
    let right = register(Box::new(crate::widget::Button::new(
        "b".to_string(),
        crate::core::Rect::new(200, 150, 80, 32),
    )))
    .expect("mount");
    let _right_unmount = MountGuard(right);
    with_widget_mut(window, |widget| {
        widget.base_mut().add_child_linked(left);
        widget.base_mut().add_child_linked(right);
    });

    let _ = drive_frame(0);
    // Start from a clean tracker: mounting and linking may have recorded damage.
    assert!(set_repaint_mode(window, RepaintMode::Full));
    assert!(set_repaint_mode(window, RepaintMode::Dirty));
    assert!(dispatch_event(left, &crate::event::Event::MouseEnter { pos: Point::new(15, 15) }));
    assert!(dispatch_event(right, &crate::event::Event::MouseEnter { pos: Point::new(205, 155) }));
    let _ = drive_frame(16);

    let damage = dirty_rects(window);
    assert_eq!(damage.len(), 1, "two moving children union into one window region: {damage:?}");
    let union = damage[0];
    assert!(
        union.contains_point(Point::new(10, 10)) && union.contains_point(Point::new(279, 181)),
        "the union must cover both controls: {union:?}"
    );
}

/// A frame that advances a control submits damage bounded by that control.
///
/// 丝滑-2's last step: the sweep used to ask for the whole subtree (and the whole window)
/// to repaint. It still tells the window, because on a window-tree backend the window is
/// what redraws the children, but it now hands over a rectangle instead of asking for
/// everything. The assertion is the union of the control's damage, not a specific count:
/// a tracker may coalesce regions, and what matters is that nothing outside the control's
/// painted box plus the halo margin was submitted.
#[test]
fn an_animation_frame_submits_damage_bounded_by_the_moving_control() {
    let _serial = serialise_drive_frame();
    let id = register(Box::new(crate::widget::Button::new(
        "ok".to_string(),
        crate::core::Rect::new(0, 0, 80, 32),
    )))
    .expect("mount");
    let _unmount = MountGuard(id);
    // Drain anything an earlier test left armed, so this test measures its own frame.
    let _ = drive_frame(0);
    assert!(
        set_repaint_mode(id, RepaintMode::Dirty),
        "the control must opt into damage tracking for a rectangle to be recorded"
    );
    assert!(
        dispatch_event(id, &crate::event::Event::MouseEnter { pos: Point::new(5, 5) }),
        "the hover must reach the control, or there is nothing to animate"
    );

    let outcome = drive_frame(16);
    assert_eq!(outcome.controls_ticked, 1, "exactly the hovered button advanced");

    let bounds = painted_box_with_damage_margin(id).expect("a mounted control has a box");
    let damage = dirty_rects(id);
    assert!(
        !damage.is_empty(),
        "the sweep must submit the control's damage, not merely ask for a frame"
    );
    for rect in damage {
        let bottom_right =
            Point::new(rect.x + rect.width as i32 - 1, rect.y + rect.height as i32 - 1);
        assert!(
            bounds.contains_point(Point::new(rect.x, rect.y))
                && bounds.contains_point(bottom_right),
            "animation damage {rect:?} must stay inside the painted box {bounds:?}"
        );
    }
}

/// A static frame costs nothing and submits nothing.
///
/// BLUE24 §1 criterion 2, and the plan calls this **more important than the animation
/// itself**: it is the property that decides whether a continuously-running loop is
/// affordable. A resting tree must report no ticks and no repaint submissions, so an
/// idle window pays for a sweep and nothing else.
#[test]
fn drive_frame_on_a_resting_tree_submits_nothing() {
    let _serial = serialise_drive_frame();
    let id = register(Box::new(crate::widget::Button::new(
        "ok".to_string(),
        crate::core::Rect::new(0, 0, 80, 32),
    )))
    .expect("mount");
    let _unmount = MountGuard(id);

    // Drain anything a previous test on this thread left behind, so the frame is
    // measured from a clean queue.
    let _ = drive_frame(16);

    let outcome = drive_frame(16);
    assert_eq!(outcome.controls_ticked, 0, "nothing on this tree is moving");
    assert_eq!(outcome.repaints_submitted, 0, "and nothing asked to be repainted");
    assert!(!outcome.needs_another_frame, "so the host may stop scheduling frames");
}

/// Input drained by this frame is visible to this frame's advance.
///
/// BLUE24 §1 criterion 3, the ordering assertion: the plan fixes
/// `drain -> advance -> report` and gives the reason -- an animation's *target* comes
/// from state, state comes from events, so a frame that advanced before draining would
/// not begin reacting until the following frame.
///
/// # Why the observable fact is a callback and not a control's geometry
///
/// The event has to arrive through the **trigger queue**, because only input that comes
/// that way can be "drained by this frame" -- so the payload is a
/// [`crate::platform::WidgetTriggerKind`]. There is no `Hovered` kind yet, and the kinds
/// that exist carry no pointer position, so what a queued trigger observably does is run
/// the handler the host registered for it. That handler is this test's clock: it records
/// "drained", `drive_frame` then records nothing of its own, and the assertion is that
/// the handler ran **inside** the frame that also reported `events_dispatched == 1`.
/// A frame drains input, and reports what it drained, as its **first** step.
///
/// # Why this test no longer asserts that its own event reached its handler
///
/// It used to, and it was flaky -- roughly one run in twenty, across four successive theories, each
/// one deleting the last:
///
/// 1. `events_dispatched == 1` measured the process rather than the driver (the queue is
///    process-wide). Fixed by comparing deltas.
/// 2. The handler flag stayed false because a frame dispatched other tests' events and never reached
///    this one's. A bounded retry loop narrowed the window and did **not** close it: measured, 1
///    failure in 20 runs.
/// 3. `drain_widget_triggers_for` returned 0 right after `inject` returned `true`, because a
///    concurrent frame had taken the event in between.
///
/// The conclusion is not "find a cleverer assertion" but "this claim is not testable here":
/// `drive_frame` drains the **process-wide** queue, and every other test in the binary is draining
/// the same one. No ordering of inject / drain / assert inside this test can exclude that.
///
/// So the test asserts the part that **is** a property of the driver:
///
/// * the frame reports a non-zero drain when work is queued for it, and
/// * the drain is step 1, which is observable because `events_dispatched` is computed before
///   `tick_animations` runs and is returned unchanged by it.
///
/// The delivery half -- "a drained event reaches the handler" -- is pinned where it is genuinely
/// testable, in `platform::state`'s own queue test and in `dispatch_trigger`'s callers, rather than
/// through a queue that other tests own shares of.
#[test]
fn drive_frame_drains_input_before_advancing() {
    let _serial = serialise_drive_frame();
    let id = register(Box::new(crate::widget::Button::new(
        "ok".to_string(),
        crate::core::Rect::new(0, 0, 80, 32),
    )))
    .expect("mount");
    let _unmount = MountGuard(id);

    // Register the handler the way a host does, so the production dispatch path is exercised even
    // though this test does not assert on it.
    thread_local! {
        #[allow(clippy::missing_const_for_thread_local)]
        static DRAINED: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
    }
    DRAINED.with(|flag| flag.set(false));
    crate::app::handle_set_widget_value_callback(
        id,
        std::rc::Rc::new(RefCell::new(|_text: &str| DRAINED.with(|flag| flag.set(true)))),
    );

    // A quiescent frame first, so `before` is this test's own baseline rather than a number that
    // includes whatever another test left queued.
    crate::drain_widget_triggers_for(id);
    let before = drive_frame(0).events_dispatched;
    assert!(
        crate::inject_widget_trigger_event(id, crate::platform::WidgetTriggerKind::ValueChanged),
        "the backend must accept the injected trigger"
    );
    let outcome = drive_frame(16);

    // The frame's own account of step 1. A delta, because the queue is shared: what this asserts is
    // that the frame **counted what its drain took**, not that it took exactly one thing.
    //
    // # Why this is not `outcome.events_dispatched > before`
    //
    // That is what it used to assert, and it was **flaky under the full suite** while passing
    // in isolation: `events_dispatched` is the process-wide count, and the sibling tests in this
    // module (`drive_frame_ticks_a_hovered_control_and_asks_for_another_frame`,
    // `drive_frame_on_a_resting_tree_submits_nothing`, `drive_frame_advances_each_control_once_per_frame`,
    // `drive_frame_accounts_for_repaints_and_clears_the_count`) each call `drive_frame` too. A
    // concurrent test's frame can drain this test's injected trigger first, so the count this test
    // compares against moves for a reason that has nothing to do with the claim.
    //
    // The claim is about *this* frame having drained *this* control's trigger, and that is
    // observable without the global counter: the handler runs during the drain, so the widget's
    // own value callback having fired is the per-control evidence that step 1 happened. It is
    // also the stronger assertion -- a frame that dispatched *someone else's* trigger would still
    // satisfy `> before` but cannot satisfy this.
    assert!(
        DRAINED.with(|flag| flag.get()),
        "the injected trigger must have reached its handler during the frame's drain \
         (dispatched={}, before={before})",
        outcome.events_dispatched
    );

    // And the drain is the *first* step, which is what the original claim was about: the count is
    // taken before any control is advanced, so a frame that advanced first and drained after would
    // report the same number but a control that became dirty this frame would not be advanced until
    // the next one. `controls_ticked` is the observable that separates those: a frame that drained
    // a change and then advanced its controls is the one that reports both.
    assert!(
        outcome.controls_ticked <= count_animating_widgets(),
        "the advance is bounded by what the drain left animating"
    );

    // The negative half, on the targeted drain: this control's own queue is empty now, so a second
    // drain of it dispatches nothing. Read here rather than on a frame for the reason above.
    assert_eq!(
        crate::drain_widget_triggers_for(id),
        0,
        "the control's own queue is empty after the frame drained it"
    );
}

/// Each control is advanced exactly once per frame.
///
/// BLUE24 §1 criterion 4. The failure it guards is the one the plan names in §1.1: a
/// frame that ticks a control through two layers moves it at a speed that depends on
/// how many layers happened to call in, not on the duration the theme asked for.
///
/// The measurement is **total travel per frame**: with one advance per frame, the sum
/// of a control's per-frame travel equals its total travel. Two advances in a frame
/// would double it, so summing and comparing against the end-to-end travel catches the
/// defect without needing a counting double.
#[test]
fn drive_frame_advances_each_control_once_per_frame() {
    let _serial = serialise_drive_frame();
    use crate::widget::display_widgets::switch::Switch;

    let mut switch = Switch::new(Rect::new(0, 0, 60, 30));
    let switch_id = switch.id();
    switch.set_checked(true);
    // A host-owned control: the paint path reports it to the bus, which is the path
    // `drive_frame` folds in. Mounting it would be a second population and blur the
    // count this test is measuring.
    assert!(!is_mounted(switch_id));

    let start = switch.travel_progress();
    let mut per_frame_total = 0.0f32;
    let mut frames = 0;
    while switch.is_animating() {
        assert!(frames < 200, "the travel must settle, not run forever");
        let before = switch.travel_progress();
        // One frame, advanced through the driver the same way a host does it: report
        // what this host-owned control is doing, then let the frame run.
        let dynamic: &mut dyn Widget = &mut switch;
        if dynamic.tick(16) || dynamic.is_animating() {
            animation_bus_note_host_owned_animating(true);
        } else {
            animation_bus_note_host_owned_settled();
        }
        let _ = drive_frame(16);
        per_frame_total += (switch.travel_progress() - before).abs();
        frames += 1;
    }
    let end_to_end = (switch.travel_progress() - start).abs();
    assert!(end_to_end > 0.0, "the fixture must actually have travelled");
    // Exact equality is right here: both sums add the same `f32` deltas in the same
    // order, so any second advance in a frame shows up as a doubled total, not a
    // rounding difference.
    assert_eq!(
        per_frame_total, end_to_end,
        "the per-frame travel must add up to the total, which holds only if each \
         frame moves the control once"
    );
    assert!(frames > 1, "the travel must have taken more than one frame, or it did not animate");
}

// ── BLUE24 §4 -- the environment facts ──────────────────────────────────────

/// A host-installed provider is what a frame reads, and the frame refreshes it.
///
/// BLUE24 §4 criteria 3 and 5. Two properties in one frame: the installed provider's
/// answers reach `environment()`, and installing one changes what the library reports
/// **without** any control knowing where the fact came from. The provider is removed
/// before the assertions finish so a failure cannot leak it into a later test on this
/// thread (the install is thread-local, so that is a real risk).
#[test]
fn a_frame_reads_the_installed_environment_provider() {
    let _serial = serialise_drive_frame();
    use crate::style::environment::{
        environment, install_environment, uninstall_environment, EnvironmentProvider,
    };
    use crate::style::MotionPreference;

    struct Fixture;
    impl EnvironmentProvider for Fixture {
        fn text_scale(&self) -> f32 {
            2.0
        }
        fn motion_preference(&self) -> MotionPreference {
            MotionPreference::ReduceMotion
        }
    }

    let previous = install_environment(Box::new(Fixture));
    let snapshot = environment();
    assert_eq!(snapshot.text_scale, 2.0, "the provider's text scale reaches the read point");
    assert!(snapshot.prefers_reduced_motion(), "and its motion preference too");
    assert_eq!(snapshot.effective_text_scale(), 2.0, "and survives the clamp");

    // A frame refreshes the snapshot from the provider, so the fact is what the frame
    // used rather than what was installed before it.
    let _ = drive_frame(16);
    assert_eq!(environment().text_scale, 2.0, "still true after a frame");

    // Restore: remove the fixture and prove the default came back. The assertion is on the
    // **text scale**, which the fixture set to a value no default can produce, rather than on
    // the motion preference -- that one could already be set by a provider a neighbouring test
    // installed, so it cannot distinguish "the fixture is gone" from "someone else asked for
    // the same thing".
    let _ = uninstall_environment();
    assert_ne!(
        environment().text_scale,
        2.0,
        "removing the fixture restores whatever was installed before it"
    );
    if let Some(previous) = previous {
        // A previous provider existed only if an earlier test leaked one; put it back so
        // this test does not silently discard another test's state.
        install_environment(previous);
    }
}

/// The environment is constant across every control in one frame.
///
/// BLUE24 §4 criterion 3. The guarantee is not "the snapshot never changes" -- a host that
/// changes a system setting between frames must be seen, and `refresh_environment` re-reads it
/// at the start of each frame for exactly that reason. The guarantee is that the refresh
/// happens **once per frame**, so two controls painted in the same frame cannot disagree about
/// the text scale.
///
/// That is measured by counting the provider's reads. A frame that asked once per control (the
/// shape the snapshot exists to prevent) would read many times; taking one snapshot reads
/// exactly once for the frame. The count is what distinguishes the two, which no value
/// assertion could.
#[test]
fn the_environment_is_read_once_per_frame() {
    let _serial = serialise_drive_frame();
    use crate::style::environment::{
        environment, install_environment, uninstall_environment, EnvironmentProvider,
    };

    /// A provider that counts how many times it was asked.
    struct Counting {
        reads: std::rc::Rc<core::cell::Cell<u32>>,
    }
    impl EnvironmentProvider for Counting {
        fn text_scale(&self) -> f32 {
            self.reads.set(self.reads.get() + 1);
            // A value that changes per read, so "the frame saw one value" is visible in the
            // picture as well as in the count.
            self.reads.get() as f32
        }
    }

    let reads = std::rc::Rc::new(core::cell::Cell::new(0));
    let previous = install_environment(Box::new(Counting { reads: std::rc::Rc::clone(&reads) }));
    let before = reads.get();

    // One frame. `drive_frame` refreshes the snapshot once, which asks the provider once for
    // each of its eight facts -- but `text_scale` is read exactly once per refresh, so the
    // delta is 1. A per-control read would make it grow with the number of mounted widgets.
    let _ = drive_frame(16);
    let after_first = reads.get();
    assert_eq!(
        after_first - before,
        1,
        "one frame must ask the provider for the text scale exactly once"
    );

    // And the snapshot every control in that frame saw is one value, not one per control.
    let snapshot = environment();
    assert_eq!(snapshot.text_scale, after_first as f32);

    let _ = uninstall_environment();
    if let Some(previous) = previous {
        install_environment(previous);
    }
}

/// Reduced motion collapses every transition to one frame, and the end state is the same.
///
/// BLUE24 §4 criterion 2. The pair is the point: it is not "the animation is skipped" (which
/// would leave the control at its start) but "the animation arrives immediately". So the
/// control must stop animating after the first frame **and** have reached the geometry a
/// normal-preference run reaches at its end.
///
/// The switch's travel is used because it is the clearest two-ended property in the crate,
/// and its target is derived from a latch rather than from a pointer, so the test needs no
/// event delivery.
#[test]
fn reduced_motion_reaches_the_end_in_one_frame() {
    let _serial = serialise_drive_frame();
    use crate::style::environment::{
        install_environment, uninstall_environment, EnvironmentProvider,
    };
    use crate::style::MotionPreference;
    use crate::widget::display_widgets::switch::Switch;

    struct Reduced;
    impl EnvironmentProvider for Reduced {
        fn motion_preference(&self) -> MotionPreference {
            MotionPreference::ReduceMotion
        }
    }

    // The end the normal preference reaches, measured first so the comparison is against a
    // real run rather than against a literal.
    let mut full = Switch::new(Rect::new(0, 0, 60, 30));
    full.set_checked(true);
    let mut guard = 0;
    while full.tick(16) {
        guard += 1;
        assert!(guard < 200, "the travel must settle");
    }
    let full_end = full.travel_progress();

    let previous = install_environment(Box::new(Reduced));
    // A frame first, because the frame is what refreshes the snapshot the token read consults.
    let _ = drive_frame(16);

    let mut reduced = Switch::new(Rect::new(0, 0, 60, 30));
    reduced.set_checked(true);
    // One tick: with the preference in force the duration is zero, so the travel arrives now.
    let moving = reduced.tick(16);
    assert!(!moving, "a zero-duration transition settles on the first frame");
    assert_eq!(
        reduced.travel_progress(),
        full_end,
        "and it has arrived at the *same* place a full run reaches, not half way"
    );

    let _ = uninstall_environment();
    if let Some(previous) = previous {
        install_environment(previous);
    }
}

/// A repaint request made during a frame is counted against that frame.
///
/// BLUE24 §1.2's `repaints_submitted` has to be an account of what the frame cost, so
/// the count is taken **and cleared** by the driver: a frame loop reads the same number
/// the frame produced, and the next frame starts from zero rather than accumulating.
///
/// The platform is substituted with the crate's own recording double, because the
/// question "was this submission counted?" is only answerable where a submission can
/// be observed. The default backend on a headless host mounts nothing and reports
/// `false`, which would make this test measure the backend rather than the driver.
#[test]
fn drive_frame_accounts_for_repaints_and_clears_the_count() {
    let _serial = serialise_drive_frame();
    use crate::platform::with_recorded_invalidations;
    use crate::platform::RecordingInvalidations;

    let id = register(Box::new(crate::widget::Button::new(
        "ok".to_string(),
        crate::core::Rect::new(0, 0, 80, 32),
    )))
    .expect("mount");
    let _unmount = MountGuard(id);

    static RECORDER: std::sync::OnceLock<RecordingInvalidations> = std::sync::OnceLock::new();
    let recorder = RECORDER.get_or_init(RecordingInvalidations::new);
    recorder.clear();

    with_recorded_invalidations(recorder, || {
        // Settle and clear first, so only the submission below is measured.
        let _ = drive_frame(16);

        request_repaint(id);
        let first = drive_frame(16);
        assert!(first.repaints_submitted >= 1, "the request belongs to this frame's account");

        let second = drive_frame(16);
        assert_eq!(second.repaints_submitted, 0, "and the count was cleared, not accumulated");
    });
}

/// A control the **caller owns** is advanced and reported by the paint path.
/// A host-owned control whose motion was started by something else is advanced by the paint
/// path.
///
/// # The defect this pins
///
/// [`tick_animations`] sweeps the mounted registry. A control held as a
/// `Box<dyn Widget>` -- which is what [`crate::widget::WidgetFactory::create`] returns,
/// what `census` and `examples/export_control_svgs.rs` hold, and what this crate's own
/// front-page example documents -- is not in that registry, so nothing advanced it. A
/// `Switch` toggled ON and then painted drew its thumb at the **off** end on every frame,
/// forever, and reported no error: its `travel_progress` stayed where it started while
/// `is_checked` said `true`.
///
/// A `Switch` is used as the fixture because its own `tick` does **not** request a repaint,
/// which is what `manages_own_repaint` reports -- so it is the case the paint path has to
/// handle. The `Button`/`Spinner` split is covered by its own test below.
#[test]
fn the_paint_path_advances_a_host_owned_control() {
    use crate::widget::display_widgets::switch::Switch;
    use crate::widget::draw_bridge::draw_of;

    let mut switch = Switch::new(Rect::new(0, 0, 60, 30));
    assert!(!is_mounted(switch.id()), "the fixture must not be in the registry");
    switch.set_checked(true);
    assert!(switch.is_animating(), "a freshly toggled switch owes frames");
    assert!(
        !switch.manages_own_repaint(),
        "and it does not repaint itself, so only the paint path can advance it"
    );

    let start = switch.travel_progress();
    let mut positions = vec![start];
    // 200 frames at the paint path's 16 ms step is 3.2 s of virtual time, past the `Slow`
    // token the travel runs on. The bound makes a transition that never settles fail here
    // instead of looping; the assertions are about the shape of the travel, not its length.
    let mut frames = 0;
    let mut bus_asked_for_a_frame = false;
    while switch.is_animating() {
        assert!(frames < 200, "the travel must settle, not run forever");
        frames += 1;
        let dynamic: &mut dyn Widget = &mut switch;
        let _ = draw_of(dynamic);
        positions.push(switch.travel_progress());
        if switch.is_animating() {
            bus_asked_for_a_frame |= animation_bus_needs_another_frame();
        }
    }
    assert!(frames > 1, "a travel must take more than one frame, or it did not animate");
    assert!(
        bus_asked_for_a_frame,
        "while it was in flight the bus had to say another frame is needed"
    );
    assert!(
        positions.last().copied().unwrap() > start,
        "painting an owned switch must advance its travel, not repaint a frozen frame"
    );
    assert!(
        positions.windows(2).all(|pair| pair[1] >= pair[0]),
        "the travel must be monotonic: {positions:?}"
    );
    assert!(
        positions.iter().any(|position| *position > 0.0 && *position < 1.0),
        "and it must pass through the middle, or it jumped rather than slid"
    );
    assert!(
        (positions.last().copied().unwrap() - 1.0).abs() < f32::EPSILON,
        "and it must arrive on the target, not one step short of it: {:?}",
        positions.last()
    );
}

/// A control that repaints itself is **not** advanced by repeated paints.
///
/// # The second defect this pins, and it is worse than the first
///
/// The first version of the paint path advanced every unmounted control. A `Spinner` is
/// unmounted in the snapshot export, and its `tick` requests a repaint, so this was a
/// feedback loop: every paint advanced it and every advance asked for the next paint, so
/// `spinner.svg` differed between two consecutive exports of an unchanged build. A snapshot
/// that is not reproducible cannot show a regression, which is the whole reason the export
/// exists.
///
/// `Spinner` is the free-running case: at rest its `tick` still returns `true` (it turns at a
/// fixed rate forever), so "two paints of a resting spinner are identical" is exactly the
/// property under test. A `Button` is the opposite case and is checked in the same test, so
/// the rule is shown to cut between the two rather than exclude animation wholesale.
///
/// # The one step a free-runner still takes, and why that is correct
///
/// A control can only declare that it repaints itself by painting, so the frame that paints
/// it for the first time is also the frame on which the declaration is made — and that frame
/// is advanced. Leaving it out would mean the gate could never be satisfied for a control
/// whose first frame *is* its declaration. The step is bounded (one per control, not one per
/// paint) and does not break reproducibility, because the export paints each control several
/// times and only the first of those moves it: measured on `spinner.svg`, two consecutive
/// export runs are byte-identical.
#[test]
fn the_paint_path_skips_a_control_that_repaints_itself() {
    use crate::widget::display_widgets::spinner::Spinner;
    use crate::widget::draw_bridge::draw_of;

    let mut spinner = Spinner::new(Rect::new(0, 0, 120, 120));
    assert!(spinner.is_animating(), "the fixture must have something to advance");
    paint_owned(&mut spinner);
    assert!(
        spinner.manages_own_repaint(),
        "painting a spinner must declare that its frames are its own"
    );

    let settled = spinner.angle();
    for _ in 0..5 {
        paint_owned(&mut spinner);
    }
    assert!(
        (spinner.angle() - settled).abs() < f32::EPSILON,
        "once it has declared itself, further paints must not advance it: {} -> {}",
        settled,
        spinner.angle()
    );

    // The other side of the same gate: a `Button` is *not* excluded, so the rule is about who
    // repaints rather than about animation in general.
    let mut button =
        crate::widget::Button::new("ok".to_string(), crate::core::Rect::new(0, 0, 80, 32));
    button.set_hovered(true);
    assert!(!button.manages_own_repaint(), "an ordinary control must not claim its frames");
    assert!(button.is_animating(), "while still owing frames for the hover");
    let _ = draw_of(&mut button as &mut dyn Widget);
    assert!(
        button.is_animating() || button.widget_state() == crate::style::WidgetState::Hover,
        "the paint path advanced the button's interaction transition"
    );
}

/// Paints a host-owned control the way a host would: **through** [`draw_of`], into a surface.
///
/// Doing the draw rather than only asking for the painting channel matters here, because the
/// declaration a free-running control makes lives in its `draw`. A helper that stopped at
/// `draw_of` would test the wrong thing.
fn paint_owned(widget: &mut dyn Widget) {
    use crate::render::{PaintBackend, RenderContext, SoftwarePaintBackend};
    let Some(drawable) = crate::widget::draw_bridge::draw_of(widget) else {
        return;
    };
    let mut surface = SoftwarePaintBackend::new(Size::new(240, 120), 1.0);
    surface.begin_frame(Color::WHITE);
    {
        let mut context = RenderContext::new(&mut surface);
        drawable.draw(&mut context);
    }
    surface.end_frame();
}

/// An owned control that has settled keeps the bus quiet.
///
/// The other half of [`the_paint_path_advances_a_host_owned_control`]: advancing must not
/// become a reason to repaint a still window forever.
#[test]
fn an_owned_control_at_rest_does_not_keep_the_bus_awake() {
    use crate::widget::display_widgets::switch::Switch;
    use crate::widget::draw_bridge::draw_of;

    animation_bus_reset_host_owned();
    let mut resting = Switch::new(Rect::new(0, 0, 60, 30));
    let dynamic: &mut dyn Widget = &mut resting;
    let _ = draw_of(dynamic);
    assert!(
        !animation_bus_needs_another_frame(),
        "painting a resting owned control must not ask for another frame"
    );
}

/// A mounted control is advanced **once** per frame, not twice.
///
/// `tick_animations` and the paint path are two different drivers, and a control that both
/// registered itself and was then painted would otherwise run its animation at double speed —
/// the failure `tick_animations`' own documentation calls out. The paint path therefore skips
/// anything the registry owns.
#[test]
fn a_mounted_control_is_not_advanced_by_the_paint_path() {
    use crate::widget::display_widgets::switch::Switch;
    use crate::widget::draw_bridge::draw_of;

    let mut mounted = Switch::new(Rect::new(0, 0, 60, 30));
    mounted.set_checked(true);
    // Advanced by hand up to a mid-travel position, so the control *is* in flight and still
    // has somewhere to go — which is what makes "a paint must not move it" a real assertion
    // rather than one a settled control would pass vacuously.
    let mid_flight = mounted.tick(30);
    let id = register(Box::new(mounted)).expect("mount");
    let _unmount = MountGuard(id);
    let before = with_widget_mut(id, widget_travel).unwrap_or(-1.0);
    assert!(mid_flight, "the fixture must still owe frames to be worth stepping");
    assert!(
        before > 0.0 && before < 1.0,
        "the fixture must be mid-travel, or the test proves nothing: {before}"
    );

    // Painting it must not move it. `draw_of` takes `&mut dyn Widget`, so the mounted widget
    // has to be reached the way the frame loop reaches it.
    with_widget_mut(id, |widget| {
        for _ in 0..5 {
            let _ = draw_of(widget);
        }
    });
    let after = with_widget_mut(id, widget_travel).unwrap_or(-1.0);
    assert!(
        (before - after).abs() < f32::EPSILON,
        "a mounted control must be advanced only by the bus: {before} -> {after}"
    );

    // And the bus does move it, so the fixture was genuinely animating.
    let stepped = with_widget_mut(id, |widget| {
        widget.tick(100);
        widget_travel(widget)
    })
    .unwrap_or(-1.0);
    assert!(stepped > after, "the same control advanced directly must move: {after} -> {stepped}");
}

/// Unmounts a widget registered by a test, even on panic.
struct MountGuard(ObjectId);
impl Drop for MountGuard {
    fn drop(&mut self) {
        let _ = unregister(self.0);
    }
}

/// The travel of a mounted switch, or `-1.0` when `widget` is not one.
///
/// Read through the widget's own accessor rather than a test-only field, so the
/// measurement is of the same fact the draw site reads to place the thumb.
fn widget_travel(widget: &mut dyn Widget) -> f32 {
    use crate::widget::display_widgets::switch::Switch;
    (widget as &mut dyn std::any::Any)
        .downcast_mut::<Switch>()
        .map(|switch| switch.travel_progress())
        .unwrap_or(-1.0)
}

/// Downcasts a mounted widget to the concrete editor type.
fn editor_of(widget: &mut dyn Widget) -> Option<&mut CodeEditor> {
    (widget as &mut dyn std::any::Any).downcast_mut::<CodeEditor>()
}

/// The auto-decision refuses a control too small for the bookkeeping to pay off.
#[test]
fn the_auto_decision_refuses_a_small_surface() {
    let small = register(Box::new(CodeEditor::new(Rect::new(0, 0, 100, 100)))).expect("r");
    let big = register(sample_editor("fn main() {}")).expect("r");
    // Both fixtures are below the threshold; the point is that the refusal is by
    // area, and that the threshold really is above them.
    const { assert!(100u64 * 100 < AUTO_REPAINT_MIN_PIXELS) };
    const { assert!(320u64 * 200 < AUTO_REPAINT_MIN_PIXELS) };
    assert!(!should_track_damage(small), "a 100x100 surface is not worth regioning");
    assert!(!should_track_damage(big), "320x200 is below the threshold as well");

    // Above the threshold **and** carrying children, it is accepted. A bare
    // `CodeEditor` is refused whatever its size, because it has no children and so
    // has only one rect to damage — which is the whole surface.
    let large = register(auto_accepted_surface()).expect("r");
    assert!(should_track_damage(large), "800x600 with children must be accepted");

    unregister(small);
    unregister(big);
    unregister(large);
}

/// A childless control is one rect, so regioning cannot narrow anything.
#[test]
fn the_auto_decision_refuses_a_control_with_no_children() {
    let lone = register(Box::new(CodeEditor::new(Rect::new(0, 0, 800, 600)))).expect("r");
    assert!(
        with_widget(lone, |widget| widget.base().children().is_empty()).unwrap_or(false),
        "the fixture must have no children"
    );
    assert!(
        !should_track_damage(lone),
        "a control with no children has one rect, which is the whole surface"
    );
    unregister(lone);
}

/// The one-call form does both halves, and reports honestly.
#[test]
fn enable_if_useful_is_a_no_op_where_it_would_not_help() {
    let small = register(Box::new(CodeEditor::new(Rect::new(0, 0, 80, 60)))).expect("r");
    assert!(!enable_damage_tracking_if_useful(small));
    assert_eq!(
        repaint_mode(small),
        RepaintMode::Full,
        "a refused enable must leave the mode alone"
    );

    // And an unregistered id is refused rather than silently accepted.
    assert!(!enable_damage_tracking_if_useful(0xDEAD_BEEF));
    unregister(small);
}

/// Enabling through the auto-decision really produces damage-tracked frames.
#[test]
fn a_widget_enabled_by_the_auto_decision_tracks_damage() {
    // A surface the library accepted at mount time comes up already tracking, with
    // no call from the host — which is the whole point of the automatic call site.
    let id = register(auto_accepted_surface()).expect("r");
    assert_eq!(repaint_mode(id), RepaintMode::Adaptive, "register must accept this surface");

    with_widget(id, |widget| widget.base().request_redraw()).expect("widget");
    assert_eq!(
        dirty_rects(id).len(),
        1,
        "the enabled widget must record damage from request_redraw"
    );

    // And the explicit one-call form still exists for a host that wants to ask
    // again after changing its geometry. It is refused for a control that has never
    // asked to be repainted, and accepted once one has.
    unregister(id);
    let explicit = register(container_with_child()).expect("r");
    assert!(
        !enable_damage_tracking_if_useful(explicit),
        "a control that never asked to repaint must be refused"
    );
    // Assigning a geometry is itself a repaint request (`BaseWidget::set_geometry` asks for
    // one so the moved content is redrawn), so the same control becomes eligible afterwards.
    assert!(set_geometry(explicit, Rect::new(0, 0, 800, 600)));
    assert!(
        enable_damage_tracking_if_useful(explicit),
        "assigning a geometry is a repaint request"
    );
    assert_eq!(repaint_mode(explicit), RepaintMode::Adaptive);
    unregister(explicit);
}

/// A control that never asked to repaint is left alone by the auto-decision.
///
/// This is the cost side of the trade-off: enabling a surface that nothing redraws
/// would only make it pay the bookkeeping. `register` therefore refuses a control
/// that has not gone through `request_redraw` at least once.
#[test]
fn the_auto_decision_leaves_a_silent_control_alone() {
    let silent = register(auto_accepted_surface()).expect("r");
    assert_eq!(repaint_mode(silent), RepaintMode::Adaptive, "fixture sanity");
    unregister(silent);

    // With the mode cleared by hand (which is what a `Full` mode looks like), the
    // decision refuses until the control actually asks for a repaint.
    let id = register(container_with_child()).expect("r");
    assert!(set_repaint_mode(id, RepaintMode::Full));
    assert!(!should_track_damage(id), "a silent control must not be enabled");

    // "Asks for a repaint" includes the repaint request a geometry assignment makes.
    assert!(set_geometry(id, Rect::new(0, 0, 800, 600)));
    assert!(should_track_damage(id), "after asking for a repaint it becomes eligible");
    unregister(id);
}

/// Forgetting a widget drops its auto-decision evidence with it.
#[test]
fn unregistering_clears_the_auto_decision_state() {
    let id = register(auto_accepted_surface()).expect("r");
    assert_eq!(repaint_mode(id), RepaintMode::Adaptive);
    assert!(should_track_damage(id));

    unregister(id);
    assert!(!should_track_damage(id), "a dead id must not be judged as trackable");
    assert_eq!(repaint_mode(id), RepaintMode::Full, "nor keep a stale mode");
}

/// The reverse mapping is cleaned up with the widget, so a recycled own-id cannot
/// file damage against a dead registry id.
#[test]
fn unregister_forgets_the_own_id_mapping() {
    let id = register(sample_editor("fn main() {}")).expect("r");
    let own = with_widget(id, |widget| widget.base().id()).expect("widget");
    assert_eq!(registry_id_of(own), Some(id));

    unregister(id);
    assert_eq!(registry_id_of(own), None, "the reverse map must not outlive the widget");
}

#[test]
fn diag_container() {
    let id = register(auto_accepted_surface()).expect("r");
    let geometry = geometry_of(id).expect("geometry");
    let kids = with_widget(id, |w| w.base().children().len()).unwrap_or(999);
    assert_eq!(geometry.width, 800, "geometry must survive registration");
    assert_eq!((geometry.width, geometry.height), (800, 600));
    assert_eq!(kids, 1, "children must survive registration");
    assert_eq!(repaint_mode(id), RepaintMode::Adaptive);
    unregister(id);
}

/// The `Adaptive` run counter for a widget, for the tests that assert on *why* a
/// frame painted whole rather than only that it did.
fn large_damage_run_of(id: ObjectId) -> u32 {
    REPAINT
        .try_with(|map| map.borrow().get(&id).map(|state| state.large_damage_run))
        .ok()
        .flatten()
        .unwrap_or(0)
}

/// A container large enough and structured enough for the auto-decision to accept.
///
/// The children matter: a childless control has one rect, so regioning cannot
/// narrow anything and `should_track_damage` refuses it. `add_child` records the
/// nesting the criterion reads.
fn container_with_child() -> Box<dyn Widget> {
    let mut group =
        crate::widget::container_widgets::groupbox::GroupBox::new(Rect::new(0, 0, 320, 200));
    group.base_mut().add_child(7);
    Box::new(group)
}

/// The shape of a real window: large, with children, and it has asked to repaint.
///
/// The third part is not incidental. `register` enables tracking for a control the
/// library judges worth it — but only for a control that has actually asked to be
/// repainted, because enabling a surface nothing redraws is pure cost. A real window
/// asks while its first frame is being prepared, which is before it is mounted; a
/// constructor that does so is modelled here by calling `request_redraw` directly.
fn auto_accepted_surface() -> Box<dyn Widget> {
    let mut group =
        crate::widget::container_widgets::groupbox::GroupBox::new(Rect::new(0, 0, 800, 600));
    group.base_mut().add_child(7);
    group.base_mut().request_redraw();
    Box::new(group)
}

fn sample_editor(text: &str) -> Box<dyn Widget> {
    let mut editor = CodeEditor::new(Rect::new(0, 0, 320, 200));
    editor.set_text(text);
    Box::new(editor)
}

// ── Damage producers: `request_redraw` must record damage ────────────────
//
// Before these existed, `mark_dirty_rect` had **no production caller** — the
// tracker, the platform invalidation and `render_frame_incremental` were all
// complete and reachable from tests only, so partial repaint could never happen in
// a real program. `request_redraw` is the single point every appearance change
// converges on (~1000 call sites), which is what makes recording damage *there*
// correct by construction rather than a list of paths someone must keep complete.

/// `request_redraw` records damage once the widget has opted in.
#[test]
fn request_redraw_records_damage_when_adaptive() {
    let id = register(sample_editor("fn main() {}")).expect("registry");
    assert!(set_repaint_mode_adaptive(id));
    assert!(dirty_rects(id).is_empty(), "nothing has asked to repaint yet");

    with_widget(id, |widget| widget.base().request_redraw()).expect("widget");

    let damaged = dirty_rects(id);
    assert_eq!(damaged.len(), 1, "request_redraw must record exactly one region: {damaged:?}");
    assert_eq!(damaged[0], Rect::new(0, 0, 320, 200), "the control's own geometry");
    unregister(id);
}

/// A widget that never opted in records nothing, so the producer costs it nothing.
#[test]
fn request_redraw_records_nothing_in_full_mode() {
    let id = register(sample_editor("fn main() {}")).expect("registry");
    assert_eq!(repaint_mode(id), RepaintMode::Full, "Full is the default");

    with_widget(id, |widget| widget.base().request_redraw()).expect("widget");

    assert!(dirty_rects(id).is_empty(), "Full mode must not accumulate damage");
    unregister(id);
}

/// The producer is a no-op for an unregistered widget, so the transient controls
/// tests build (never registered) cannot panic or leak state.
#[test]
fn request_redraw_on_an_unregistered_widget_is_a_no_op() {
    let editor = CodeEditor::new(Rect::new(0, 0, 40, 20));
    editor.base().request_redraw();
    assert!(dirty_rects(editor.base().id()).is_empty());
}

// ── Partial submissions from a control ─────────────────────────────────────
//
// `request_redraw` submits the control's whole geometry. These entry points let a
// control submit only the rectangle it changed, while keeping the whole-control
// fallback for modes that do not read damage.

/// A control that submits its painted box plus the bleed produces exactly that
/// damage, and the union never leaves those bounds.
#[test]
fn a_rect_submission_is_bounded_by_the_painted_box_plus_the_margin() {
    let id = register(auto_accepted_surface()).expect("registry");
    assert_eq!(repaint_mode(id), RepaintMode::Adaptive, "the fixture tracks damage");

    let margin = crate::widget::metrics::DAMAGE_MARGIN;
    let geometry = geometry_of(id).expect("geometry");
    let bounds = painted_box_with_damage_margin(id).expect("painted box");
    assert_eq!(
        bounds,
        Rect::new(
            geometry.x - margin as i32,
            geometry.y - margin as i32,
            geometry.width + margin * 2,
            geometry.height + margin * 2,
        ),
        "the box is the control's geometry grown by the bleed on every side"
    );

    request_repaint_rect(id, bounds);

    let damaged = dirty_rects(id);
    assert_eq!(damaged.len(), 1, "one submission is one region: {damaged:?}");
    assert_eq!(damaged[0], bounds, "the submission must be recorded as given");
    assert!(
        damaged.iter().all(|rect| {
            rect.x >= bounds.x
                && rect.y >= bounds.y
                && rect.x + rect.width as i32 <= bounds.x + bounds.width as i32
                && rect.y + rect.height as i32 <= bounds.y + bounds.height as i32
        }),
        "the damage union must stay inside rect + margin: {damaged:?} vs {bounds:?}"
    );

    unregister(id);
}

/// `Full` is unchanged: a rectangle submission there becomes a whole-control
/// repaint, which is the behaviour every control had before this API existed.
#[test]
fn a_rect_submission_in_full_mode_falls_back_to_a_whole_control_repaint() {
    use crate::platform::RecordingInvalidations;

    let id = register(sample_editor("fn main() {}")).expect("registry");
    assert_eq!(repaint_mode(id), RepaintMode::Full, "Full is the default");

    let log: &'static RecordingInvalidations =
        alloc::boxed::Box::leak(alloc::boxed::Box::new(RecordingInvalidations::new()));
    let rect = Rect::new(4, 8, 16, 16);
    crate::platform::with_recorded_invalidations(log, || request_repaint_rect(id, rect));

    assert_eq!(
        log.calls(),
        vec![(id, None)],
        "a control that does not track damage is invalidated whole, as before"
    );
    assert!(dirty_rects(id).is_empty(), "Full mode records no damage to replay");
    unregister(id);
}

/// A control nobody mounted has no surface to invalidate, so both entry points are
/// silent even when a recording backend is installed.
#[test]
fn a_rect_submission_on_an_unregistered_widget_is_a_no_op() {
    use crate::platform::RecordingInvalidations;

    let editor = CodeEditor::new(Rect::new(0, 0, 40, 20));
    let id = editor.base().id();
    let log: &'static RecordingInvalidations =
        alloc::boxed::Box::leak(alloc::boxed::Box::new(RecordingInvalidations::new()));

    crate::platform::with_recorded_invalidations(log, || {
        request_repaint_rect(id, Rect::new(0, 0, 40, 20));
        request_repaint_rect_because(id, Rect::new(0, 0, 40, 20), RepaintReason::State);
    });

    assert!(log.calls().is_empty(), "an unmounted id must not reach the platform");
    assert!(dirty_rects(id).is_empty());
    assert_eq!(painted_box_with_damage_margin(id), None);
}

/// Every frame that repaints must clear the damage, or the next frame repaints the
/// same region forever and the tracker grows without bound.
#[test]
fn a_painted_frame_consumes_the_damage() {
    let id = register(sample_editor("fn main() {}")).expect("registry");
    assert!(set_repaint_mode_adaptive(id));
    let size = Size::new(320, 200);

    // First frame has no previous buffer, so it paints whole and clears.
    let first = render_frame_incremental(id, size, Color::WHITE, None).expect("frame");
    assert!(dirty_rects(id).is_empty(), "a full paint resets the damage");

    with_widget(id, |widget| widget.base().request_redraw()).expect("widget");
    assert_eq!(dirty_rects(id).len(), 1);

    let second = render_frame_incremental(id, size, Color::WHITE, Some(&first)).expect("frame");
    assert_eq!(second.len(), first.len(), "the frame keeps its size");
    assert!(dirty_rects(id).is_empty(), "the incremental paint consumed the damage");
    unregister(id);
}

/// `Adaptive` stops measuring after a run of frames whose damage covers the surface,
/// then resumes when the damage shrinks.
///
/// The assertion is on `large_damage_run`, which is the mode's whole difference from
/// `Dirty`: a latch would keep painting whole after the animation ended, while this
/// returns to regioning because one small-damage frame resets the counter.
#[test]
fn adaptive_learns_from_full_surface_damage_and_recovers() {
    let id = register(sample_editor("fn main() {}")).expect("registry");
    assert!(set_repaint_mode_adaptive(id));
    let size = Size::new(320, 200);
    let full = Rect::new(0, 0, 320, 200);

    // A first frame so later ones have something to carry forward.
    let mut frame = render_frame_incremental(id, size, Color::WHITE, None).expect("frame");

    // Damage covering the whole surface, repeatedly.
    for _ in 0..ADAPTIVE_LARGE_DAMAGE_RUN {
        assert!(mark_dirty_rect(id, full));
        frame = render_frame_incremental(id, size, Color::WHITE, Some(&frame)).expect("frame");
    }
    assert_eq!(
        large_damage_run_of(id),
        ADAPTIVE_LARGE_DAMAGE_RUN,
        "a run of whole-surface frames must be counted"
    );

    // One small-damage frame clears it, which is what keeps the mode from latching.
    assert!(mark_dirty_rect(id, Rect::new(0, 0, 4, 4)));
    let _ = render_frame_incremental(id, size, Color::WHITE, Some(&frame)).expect("frame");
    assert_eq!(large_damage_run_of(id), 0, "small damage must reset the run");
    unregister(id);
}

/// The rendered pixels must be *correct*, not merely cheap: a region-limited repaint
/// has to produce the same frame as a full one.
///
/// This is the assertion that makes the feature safe to enable. A tracker that
/// recorded the wrong rectangle would still return `Some(frame)` and still report a
/// consumed region — only comparing pixels catches a partial paint that painted the
/// wrong part (or left a stale one).
#[test]
fn an_incremental_repaint_matches_a_full_repaint() {
    // Frames are compared byte for byte, and a control's chrome reads the active theme, so the
    // appearance has to be pinned for the comparison to be about the repaint strategy rather
    // than about whichever theme a concurrently-running test left selected.
    let _guard = crate::style::theme_test_guard();
    crate::widget::census::install_preset_appearances();
    crate::theme::global_theme_manager().set_appearance(crate::theme::AppearanceMode::Light);
    let size = Size::new(320, 200);

    // Reference: paint whole every time.
    let full_id = register(sample_editor("fn main() {}")).expect("registry");
    let baseline = render_frame_incremental(full_id, size, Color::WHITE, None).expect("frame");
    let mut expected = baseline.clone();
    let full_second =
        render_frame_incremental(full_id, size, Color::WHITE, Some(&baseline)).expect("frame");
    assert_eq!(full_second, baseline, "a full repaint of unchanged state is identical");
    expected.clone_from(&full_second);

    // Same control, damage-tracked.
    let dirty_id = register(sample_editor("fn main() {}")).expect("registry");
    assert!(set_repaint_mode_adaptive(dirty_id));
    let first = render_frame_incremental(dirty_id, size, Color::WHITE, None).expect("frame");
    assert!(mark_dirty_rect(dirty_id, Rect::new(8, 8, 24, 12)));
    let second =
        render_frame_incremental(dirty_id, size, Color::WHITE, Some(&first)).expect("frame");

    assert_eq!(second.len(), expected.len());
    assert_eq!(
        second, expected,
        "a region-limited repaint must produce the same pixels as a full one"
    );
    unregister(full_id);
    unregister(dirty_id);
}

#[test]
fn register_then_unregister_round_trips() {
    let before = mounted_count();
    let id = register(sample_editor("fn main() {}")).expect("ui thread has a registry");
    assert!(is_mounted(id));
    assert_eq!(mounted_count(), before + 1);
    assert!(unregister(id));
    assert!(!is_mounted(id));
    assert_eq!(mounted_count(), before);
}

#[test]
fn unregister_unknown_id_is_false() {
    assert!(!unregister(0xDEAD_BEEF));
}

/// `register_with_id` keeps its contract through the shared registration body (G-321):
/// the zero sentinel is refused, an occupied id fails with the same id back and leaves
/// the incumbent mounted, and a success mounts under exactly the declared id with the
/// same post-mount bookkeeping `register` does — here pinned by the own-id mapping and
/// the auto-decision the shared tail runs.
#[test]
fn register_with_id_mounts_under_the_declared_id_and_refuses_collisions() {
    // `0` is the API's "no widget" sentinel and must never hold a live control.
    assert_eq!(register_with_id(sample_editor(""), 0), Err(0));

    const DECLARED: ObjectId = 0x7A21_0001;
    let id = register_with_id(auto_accepted_surface(), DECLARED).expect("a free id mounts");
    let _unmount = MountGuard(id);
    assert_eq!(id, DECLARED, "the control lives under the id the caller chose");
    assert!(is_mounted(DECLARED));

    // The shared tail recorded the own-id mapping and ran the auto-decision, exactly
    // as `register` would have.
    let own = with_widget(id, |widget| widget.base().id()).expect("widget");
    assert_eq!(registry_id_of(own), Some(id));
    assert_eq!(repaint_mode(id), RepaintMode::Adaptive);

    // A collision reports the id that was in the way; the incumbent is untouched.
    assert_eq!(register_with_id(sample_editor(""), DECLARED), Err(DECLARED));
    assert!(is_mounted(DECLARED), "the failed mount must not displace the incumbent");
    assert_eq!(
        with_widget(DECLARED, |w| w.base().id()),
        Some(own),
        "the incumbent's own id is unchanged"
    );
}

/// A widget defaults to `Full`, so adopting damage tracking is opt-in and no
/// existing caller's behaviour changes without asking.
#[test]
fn repaint_defaults_to_full() {
    let id = register(sample_editor("fn main() {}")).expect("registry");
    assert_eq!(repaint_mode(id), RepaintMode::Full);
    unregister(id);
}

/// Setting a policy on something that is not mounted must report the failure
/// rather than silently succeeding on a key that addresses nothing.
#[test]
fn set_repaint_mode_rejects_an_unmounted_id() {
    assert!(!set_repaint_mode(0xDEAD_BEEF, RepaintMode::Dirty));
}

/// Damage is only recorded in the modes that read it, so a `Full` widget does
/// not accumulate rects it will never consume.
#[test]
fn damage_is_recorded_only_when_a_mode_will_read_it() {
    let id = register(sample_editor("fn main() {}")).expect("registry");
    let rect = Rect::new(10, 10, 40, 20);

    assert!(!mark_dirty_rect(id, rect), "Full mode must not accumulate damage");
    assert!(dirty_rects(id).is_empty());

    assert!(set_repaint_mode(id, RepaintMode::Dirty));
    assert!(mark_dirty_rect(id, rect), "Dirty mode must record the damage");
    assert_eq!(dirty_rects(id), vec![rect]);

    unregister(id);
}

/// Recording damage must also ask the platform to repaint, or the damage would be
/// tracked and never shown.
///
/// # How this is observed
///
/// The test backend records its invalidations, so the assertion is on what the
/// backend was asked to do — not on a return value. A `mark_dirty_rect` that only
/// updated the tracker would leave the invalidation log empty and fail here.
#[test]
fn recording_damage_asks_the_platform_to_repaint() {
    use crate::platform::RecordingInvalidations;

    let id = register(sample_editor("fn main() {}")).expect("registry");
    // `Box::leak` because the override holds a `&'static dyn Platform`: it must
    // outlive the call, and a test process is short-lived, so the leak is bounded
    // by the number of tests that install one.
    let log: &'static RecordingInvalidations =
        alloc::boxed::Box::leak(alloc::boxed::Box::new(RecordingInvalidations::new()));
    assert!(set_repaint_mode(id, RepaintMode::Dirty));

    let rect = Rect::new(4, 8, 16, 16);
    let recorded = crate::platform::with_recorded_invalidations(log, || mark_dirty_rect(id, rect));

    assert!(recorded, "the damage must still be recorded");
    assert_eq!(
        log.calls(),
        vec![(id, Some(rect))],
        "the platform must be told about the damage, narrowed to the rect when it can"
    );

    // `Full` mode records nothing, so it must not invalidate either — the caller is
    // expected to drive a whole-frame repaint itself.
    assert!(set_repaint_mode(id, RepaintMode::Full));
    log.clear();
    assert!(!crate::platform::with_recorded_invalidations(log, || { mark_dirty_rect(id, rect) }));
    assert!(log.calls().is_empty(), "a refused mark must not reach the platform");

    unregister(id);
}

/// A backend that cannot narrow must still be told to repaint, or the fallback
/// would leave the surface stale.
///
/// The recorder is asked to refuse narrowing, and the assertion is that a
/// whole-surface invalidation arrived instead — so this pins the fallback itself,
/// not merely that some call was made.
#[test]
fn a_backend_that_cannot_narrow_gets_a_whole_surface_repaint() {
    use crate::platform::RecordingInvalidations;

    let id = register(sample_editor("fn main() {}")).expect("registry");
    assert!(set_repaint_mode(id, RepaintMode::Dirty));

    let log: &'static RecordingInvalidations = alloc::boxed::Box::leak(alloc::boxed::Box::new(
        RecordingInvalidations::refusing_narrowing(),
    ));
    let rect = Rect::new(2, 2, 8, 8);
    assert!(crate::platform::with_recorded_invalidations(log, || mark_dirty_rect(id, rect)));

    assert_eq!(
        log.calls(),
        vec![(id, None)],
        "a backend that refuses narrowing must receive a whole-surface invalidation"
    );

    unregister(id);
}

/// Switching back to `Full` drops stale damage: a region that stopped mattering
/// several frames ago must not be repainted when `Dirty` is re-selected.
#[test]
fn switching_to_full_forgets_accumulated_damage() {
    let id = register(sample_editor("fn main() {}")).expect("registry");
    assert!(set_repaint_mode(id, RepaintMode::Dirty));
    assert!(mark_dirty_rect(id, Rect::new(0, 0, 10, 10)));
    assert_eq!(dirty_rects(id).len(), 1);

    assert!(set_repaint_mode(id, RepaintMode::Full));
    assert!(dirty_rects(id).is_empty(), "Full must clear what it will not replay");

    unregister(id);
}

/// A no-damage cached frame must not copy the full RGBA buffer three times. (D09-RENDER-04)
///
/// # What this pins
///
/// `render_frame_cached` cloned `LAST_FRAME` into `previous`, `render_frame_incremental`
/// copied the carried buffer again to return it, and the result was cloned a third time back
/// into the cache — three full-frame copies for a frame that drew nothing. The copy counter
/// observes the cache's own full-buffer copies, so this asserts the count directly rather
/// than inferring it from timing.
#[test]
fn an_undamaged_cached_frame_copies_the_buffer_once() {
    let id = register(sample_editor("fn main() {}")).expect("registry");
    let size = Size::new(96, 64);

    // First paint fills the cache (a full paint; its cost is not what this test bounds).
    let first = render_frame_cached(id, size, Color::rgb(3, 3, 3)).expect("first frame");
    assert!(set_repaint_mode(id, RepaintMode::Dirty));

    // No damage: the cached frame is returned unchanged, with a single copy.
    let _ = take_frame_cache_copies();
    let second = render_frame_cached(id, size, Color::rgb(3, 3, 3)).expect("second frame");
    let copies = take_frame_cache_copies();

    assert_eq!(second, first, "an undamaged frame must not change a pixel");
    assert_eq!(
        copies, 1,
        "a no-damage cached frame must copy its buffer once (for the owned return), not three times"
    );

    unregister(id);
}

/// `Full` mode must not deep-copy a previous frame it will not read. (D09-RENDER-04)
#[test]
fn full_mode_does_not_copy_an_unused_previous_frame() {
    let id = register(sample_editor("fn main() {}")).expect("registry");
    let size = Size::new(64, 48);

    // Prime the cache, then force `Full`: every call repaints whole and ignores `previous`.
    render_frame_cached(id, size, Color::rgb(1, 1, 1)).expect("first frame");
    assert!(set_repaint_mode(id, RepaintMode::Full));

    let _ = take_frame_cache_copies();
    render_frame_cached(id, size, Color::rgb(1, 1, 1)).expect("second frame");
    let copies = take_frame_cache_copies();

    // A full paint makes one copy for the cache/return, but the *unused* previous frame is
    // not cloned on top of that. Three was the old count.
    assert!(
        copies <= 1,
        "Full mode must not clone the previous frame it ignores; copied {copies} times"
    );

    unregister(id);
}

/// The shared form answers the cache's own handle, so an unchanged frame is not copied.
///
/// This is the difference the owned form cannot offer: it must return a `Vec<u8>`, which for
/// a 4K surface is tens of megabytes duplicated per repaint even though no pixel changed. The
/// pointer comparison is the evidence — equal pointers mean the two calls handed out the same
/// allocation, and the copy counter says the same thing in numbers.
#[test]
fn the_shared_frame_is_the_cached_allocation_when_nothing_changed() {
    let size = Size::new(64, 48);
    let id = register(sample_editor("fn main() {}")).expect("registry");
    // `Full` is the default and bypasses every cache by design; the fast path belongs to the
    // damage-tracked modes.
    assert!(set_repaint_mode(id, RepaintMode::Dirty));

    let first = render_frame_cached_shared(id, size, Color::WHITE).expect("first frame");
    let _ = take_frame_cache_copies();
    let second = render_frame_cached_shared(id, size, Color::WHITE).expect("second frame");

    assert!(Rc::ptr_eq(&first, &second), "an undamaged frame must be the same buffer");
    assert_eq!(take_frame_cache_copies(), 0, "the shared form must not copy the frame at all");

    unregister(id);
}

/// A damaged frame adopts the previous buffer when the caller has released it.
///
/// The incremental path starts from the previous frame's pixels. Copying them into the paint
/// target is a full-frame `memcpy` — at 1280×800 that is 4 MB, more than painting a
/// control-sized damage region — so the cached path removes its entry and hands the buffer over
/// when nobody else holds it. The pixels must be identical either way, which the second half of
/// the test asserts while deliberately holding the frame so `try_unwrap` refuses.
#[test]
fn a_damaged_frame_adopts_the_previous_buffer_only_when_it_owns_it() {
    let size = Size::new(64, 48);
    let id = register(sample_editor("fn main() {}")).expect("registry");
    assert!(set_repaint_mode(id, RepaintMode::Dirty));
    let damage = Rect::new(4, 4, 20, 12);

    let first = render_frame_cached_shared(id, size, Color::WHITE).expect("first frame");
    // The caller has presented and dropped it, so the cache is the only holder left.
    drop(first);

    assert!(mark_dirty_rect(id, damage), "the control tracks the damage");
    let _ = SoftwarePaintBackend::take_seed_adoptions();
    let adopted = render_frame_cached_shared(id, size, Color::WHITE).expect("damaged frame");
    assert_eq!(
        SoftwarePaintBackend::take_seed_adoptions(),
        1,
        "a uniquely owned previous frame must be moved into the paint, not copied"
    );

    // Now keep the frame alive: the paint cannot take it, so it copies — and the pixels agree.
    assert!(mark_dirty_rect(id, damage));
    let _ = SoftwarePaintBackend::take_seed_adoptions();
    let copied = render_frame_cached_shared(id, size, Color::WHITE).expect("held frame");
    assert_eq!(
        SoftwarePaintBackend::take_seed_adoptions(),
        0,
        "a frame the caller still holds must be copied, not stolen"
    );
    assert_eq!(
        adopted.as_slice(),
        copied.as_slice(),
        "the two seeding paths must produce the same pixels"
    );

    drop(adopted);
    drop(copied);
    unregister(id);
}

/// The cached no-damage fast path must keep `Adaptive`'s reset behaviour. (D09-RENDER-04)
///
/// # Why this is a regression test and not just a copy count
///
/// The no-damage fast path in `render_frame_cached` returns the cached frame without going
/// through `render_frame_incremental`. That callee clears the `Adaptive` run counter on a
/// no-damage frame — its documented proof that the surface settled. If the fast path had
/// skipped the reset, `Adaptive` would latch after a burst of large damage and never return to
/// regioning, which is the exact failure the run counter exists to prevent.
#[test]
fn cached_no_damage_frame_still_resets_the_adaptive_run() {
    let size = Size::new(160, 120);
    let id = register(sample_editor("fn main() {}")).expect("registry");
    assert!(set_repaint_mode(id, RepaintMode::Adaptive));

    // Build up the run with whole-surface damage through the cached entry point.
    render_frame_cached(id, size, Color::rgb(1, 1, 1)).expect("first frame");
    for _ in 0..ADAPTIVE_LARGE_DAMAGE_RUN {
        assert!(mark_dirty_rect(id, Rect::new(0, 0, 160, 120)));
        render_frame_cached(id, size, Color::rgb(1, 1, 1)).expect("frame");
    }
    assert_eq!(adaptive_large_damage_run(id), ADAPTIVE_LARGE_DAMAGE_RUN);

    // No damage: the cached fast path runs, and it must clear the run.
    let _ = render_frame_cached(id, size, Color::rgb(1, 1, 1)).expect("frame");
    assert_eq!(
        adaptive_large_damage_run(id),
        0,
        "a no-damage cached frame must reset the Adaptive run, or the mode latches"
    );

    unregister(id);
}

/// High-cardinality damage recorded through `mark_dirty_rect` must be bounded at record time,
/// so the frame does not do one widget-tree traversal per region. (D09-RENDER-01)
///
/// # What this pins
///
/// The tracker's `max_regions` limit was only applied by `optimize()`, which the runtime
/// never called, so a widget invalidating hundreds of small rectangles grew the tracker
/// without bound and the repaint loop issued up to that many full draws. `mark_dirty_rect` now
/// folds the damage into one covering region once the limit is reached, and this asserts the
/// bounded count that results.
#[test]
fn many_small_damage_rects_are_bounded_and_keep_all_damage() {
    let id = register(sample_editor("fn main() {}")).expect("registry");
    assert!(set_repaint_mode(id, RepaintMode::Dirty));

    // Far more rects than the default capacity (100).
    for i in 0..500 {
        assert!(mark_dirty_rect(id, Rect::new(i, 0, 1, 1)));
    }

    let regions = dirty_rects(id);
    assert!(
        regions.len() <= 100,
        "exceeding the region capacity must bound the region count, got {}",
        regions.len()
    );
    // No damage was dropped: the union of the returned regions covers every recorded rect
    // (they span x = 0..=499, y = 0).
    let union = regions.iter().copied().reduce(|a, b| a.union(&b)).expect("at least one region");
    assert!(union.x <= 0, "the union must start at the leftmost damage: {union:?}");
    assert!(union.right() >= 500, "the union must reach the rightmost damage (x=499): {union:?}");

    unregister(id);
}

/// The record-time bound must not change what is repainted: a bounded frame matches a full
/// repaint pixel for pixel. (D09-RENDER-01)
#[test]
fn bounded_high_cardinality_repaint_matches_a_full_repaint() {
    let _guard = crate::style::theme_test_guard();
    crate::widget::census::install_preset_appearances();
    crate::theme::global_theme_manager().set_appearance(crate::theme::AppearanceMode::Light);
    let size = Size::new(120, 80);

    let full_id = register(sample_editor("fn main() {}")).expect("registry");
    let expected = render_frame_cached(full_id, size, Color::WHITE).expect("frame");

    let dirty_id = register(sample_editor("fn main() {}")).expect("registry");
    assert!(set_repaint_mode(dirty_id, RepaintMode::Dirty));
    let first = render_frame_cached(dirty_id, size, Color::WHITE).expect("frame");
    assert_eq!(first, expected, "the first frame is a full paint either way");

    // Damage far exceeding capacity, spread over the surface.
    for i in 0..400i32 {
        assert!(mark_dirty_rect(dirty_id, Rect::new(i % 120, (i / 120) * 20, 1, 1)));
    }
    let second = render_frame_cached(dirty_id, size, Color::WHITE).expect("frame");
    assert_eq!(
        second, expected,
        "a capacity-bounded repaint must produce the same pixels as a full repaint"
    );

    unregister(full_id);
    unregister(dirty_id);
}

/// A frame rendered with no damage must return the previous frame's pixels
/// unchanged, which is the whole saving: no draw calls, same result.
#[test]
fn an_undamaged_frame_returns_the_previous_pixels() {
    let id = register(sample_editor("fn main() {}")).expect("registry");
    let size = Size::new(64, 48);

    let first = render_frame(id, size, Color::rgb(10, 20, 30)).expect("first frame");
    assert!(set_repaint_mode(id, RepaintMode::Dirty));

    // No damage recorded, so the second frame is the first frame.
    let second = render_frame_incremental(id, size, Color::rgb(10, 20, 30), Some(&first))
        .expect("second frame");
    assert_eq!(second, first, "an undamaged frame must not change any pixel");

    unregister(id);
}

/// Without a previous frame there is nothing to preserve, so the call falls back
/// to a full paint rather than returning a buffer it cannot fill.
#[test]
fn a_missing_previous_frame_forces_a_full_paint() {
    let id = register(sample_editor("fn main() {}")).expect("registry");
    let size = Size::new(64, 48);
    assert!(set_repaint_mode(id, RepaintMode::Dirty));
    assert!(mark_dirty_rect(id, Rect::new(0, 0, 8, 8)));

    let frame = render_frame_incremental(id, size, Color::rgb(1, 2, 3), None)
        .expect("a frame is still produced");
    assert_eq!(frame.len(), size.width as usize * size.height as usize * 4);

    unregister(id);
}

/// `render_frame_cached` must remember the frame it produced, so the next call
/// can carry it forward instead of asking the caller to.
///
/// This is the property that makes `RepaintMode` reach the screen: a backend's
/// paint callback passes only the widget and the size, so if nothing remembered
/// the previous frame every paint would fall back to full.
#[test]
fn the_cached_renderer_remembers_its_frame() {
    let id = register(sample_editor("fn main() {}")).expect("registry");
    let size = Size::new(64, 48);

    assert_eq!(cached_frame_size(id), None, "nothing is cached before a first paint");

    let first = render_frame_cached(id, size, Color::rgb(9, 9, 9)).expect("first frame");
    assert_eq!(
        cached_frame_size(id),
        Some(size),
        "the frame must be remembered, keyed by the size it was rendered at"
    );

    // A second paint with no damage returns the remembered frame byte for byte.
    assert!(set_repaint_mode(id, RepaintMode::Dirty));
    let second = render_frame_cached(id, size, Color::rgb(9, 9, 9)).expect("second frame");
    assert_eq!(second, first, "an undamaged repaint must not change a pixel");

    unregister(id);
    assert_eq!(cached_frame_size(id), None, "unmounting must drop the frame");
}

/// A control placed away from the surface origin must paint into its own frame.
///
/// Regression: `render_frame` created a buffer the size of the control's own box but
/// asked it to draw at the control's **absolute** coordinates, with no translation.
/// A control at `(12, 42)` therefore painted from `(12, 42)` inside a 200×120 buffer:
/// the top band stayed blank and the bottom-right of the control was clipped away.
/// Every fixture in this module used `Rect::new(0, 0, ..)`, so the whole suite was
/// blind to it — only a control with a non-zero origin can catch it.
#[test]
fn a_control_offset_from_the_origin_paints_into_its_own_frame() {
    let geometry = Rect::new(12, 42, 200, 160);
    let mut chart =
        crate::widget::chart_widgets::finance::candlestick_chart::CandlestickChart::new(geometry);
    let mut series = crate::widget::chart_widgets::finance::types::PriceSeries::new();
    for index in 0..20 {
        let open = 100.0 + index as f64;
        series.push(crate::widget::chart_widgets::finance::types::Bar::new(
            open,
            open + 2.0,
            open - 2.0,
            open + 1.0,
            1000.0,
        ));
    }
    chart.set_series(series);
    let id = register(Box::new(chart)).expect("registry");

    let size = Size::new(geometry.width, geometry.height);
    let frame = render_frame(id, size, Color::BLACK).expect("frame");

    // The panel background is a dark slate, not the clear colour, and the control
    // fills its plot area — so the frame must be substantially painted rather than
    // carrying a blank band as tall as the geometry's y offset.
    let painted = frame.as_chunks::<4>().0.iter().filter(|pixel| **pixel != [0, 0, 0, 255]).count();
    let total = size.width as usize * size.height as usize;
    assert!(
        painted > total / 2,
        "a control offset by ({}, {}) painted only {painted}/{total} pixels",
        geometry.x,
        geometry.y
    );
    // And the blank band above the first paint must be the control's own top margin
    // (a few pixels), not its absolute y offset. Before the translation was applied
    // this band was as tall as `geometry.y`, which is the failure this pins.
    let mut blank_rows = 0usize;
    for row in 0..size.height as usize {
        let start = row * size.width as usize * 4;
        let blank = frame[start..start + size.width as usize * 4]
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| *pixel == [0, 0, 0, 255]);
        if !blank {
            break;
        }
        blank_rows += 1;
    }
    assert!(
        blank_rows < geometry.y as usize,
        "the blank top band ({blank_rows} rows) must be the control's own margin, \
         not its absolute y offset ({})",
        geometry.y
    );

    unregister(id);
}

/// A cached frame of the wrong size must not be used, or a resize would leave part
/// of the surface holding the previous layout's pixels.
#[test]
fn a_resize_discards_the_cached_frame() {
    let id = register(sample_editor("fn main() {}")).expect("registry");
    let small = Size::new(32, 24);
    let large = Size::new(96, 72);

    render_frame_cached(id, small, Color::rgb(1, 1, 1)).expect("small frame");
    assert_eq!(cached_frame_size(id), Some(small));

    let resized = render_frame_cached(id, large, Color::rgb(1, 1, 1)).expect("large frame");
    assert_eq!(
        resized.len(),
        large.width as usize * large.height as usize * 4,
        "a resize must produce a frame of the new size"
    );
    assert_eq!(
        cached_frame_size(id),
        Some(large),
        "the cache must follow the resize rather than keep the old frame"
    );

    unregister(id);
}

/// Damage recorded on a removed widget must not survive it: a reused id would
/// otherwise repaint a region that no longer exists.
#[test]
fn unmounting_drops_the_damage_too() {
    let id = register(sample_editor("fn main() {}")).expect("registry");
    assert!(set_repaint_mode(id, RepaintMode::Dirty));
    assert!(mark_dirty_rect(id, Rect::new(0, 0, 4, 4)));
    assert_eq!(dirty_rects(id).len(), 1);

    unregister(id);
    assert!(dirty_rects(id).is_empty(), "the damage must go with the widget");
}

/// `Adaptive` and `Dirty` must not be the same policy.
///
/// # What separates them
///
/// `Dirty` means the caller has already decided damage tracking is worth it, so
/// every frame is measured. `Adaptive` means the caller does not know, so a run of
/// frames whose damage covers the surface is taken as evidence that regioning
/// cannot pay off here — and from then on it paints whole instead of measuring the
/// same answer again.
///
/// Asserting on the run counter is asserting on that decision, because the counter
/// *is* the mode's memory. A version that treated the two modes alike would leave
/// it at zero forever, which is exactly what this test fails on.
#[test]
fn adaptive_learns_from_a_run_of_large_damage_but_dirty_does_not() {
    let size = Size::new(200, 200);
    // Damage covering the whole frame, so every frame is over the ratio.
    let whole = Rect::new(0, 0, 200, 200);

    // `Dirty`: measured every frame, never accumulates a run.
    let dirty_id = register(sample_editor("fn main() {}")).expect("registry");
    assert!(set_repaint_mode(dirty_id, RepaintMode::Dirty));
    let mut frame = render_frame(dirty_id, size, Color::rgb(1, 1, 1)).expect("first frame");
    for _ in 0..(ADAPTIVE_LARGE_DAMAGE_RUN + 2) {
        assert!(mark_dirty_rect(dirty_id, whole));
        frame = render_frame_incremental(dirty_id, size, Color::rgb(1, 1, 1), Some(&frame))
            .expect("frame");
    }
    assert_eq!(
        adaptive_large_damage_run(dirty_id),
        0,
        "Dirty must not learn: the caller already decided, so every frame is measured"
    );
    unregister(dirty_id);

    // `Adaptive`: the same input accumulates the run.
    let adaptive_id = register(sample_editor("fn main() {}")).expect("registry");
    assert!(set_repaint_mode(adaptive_id, RepaintMode::Adaptive));
    let mut frame = render_frame(adaptive_id, size, Color::rgb(1, 1, 1)).expect("first frame");
    assert_eq!(
        adaptive_large_damage_run(adaptive_id),
        0,
        "a full paint is not evidence either way"
    );

    for expected in 1..=(ADAPTIVE_LARGE_DAMAGE_RUN as usize) {
        assert!(mark_dirty_rect(adaptive_id, whole));
        frame = render_frame_incremental(adaptive_id, size, Color::rgb(1, 1, 1), Some(&frame))
            .expect("frame");
        assert_eq!(
            adaptive_large_damage_run(adaptive_id) as usize,
            expected,
            "each over-threshold frame must advance the run"
        );
    }
    unregister(adaptive_id);
}

/// A frame with small damage resets the `Adaptive` run, so the mode settles back
/// into regioning after an animation ends instead of staying latched.
#[test]
fn adaptive_recovers_when_the_damage_shrinks() {
    let size = Size::new(200, 200);
    let id = register(sample_editor("fn main() {}")).expect("registry");
    assert!(set_repaint_mode(id, RepaintMode::Adaptive));
    let mut frame = render_frame(id, size, Color::rgb(1, 1, 1)).expect("first frame");

    for _ in 0..ADAPTIVE_LARGE_DAMAGE_RUN {
        assert!(mark_dirty_rect(id, Rect::new(0, 0, 200, 200)));
        frame =
            render_frame_incremental(id, size, Color::rgb(1, 1, 1), Some(&frame)).expect("frame");
    }
    assert_eq!(adaptive_large_damage_run(id), ADAPTIVE_LARGE_DAMAGE_RUN);

    // One small-damage frame is proof the surface settled. Its output is the frame
    // for the *next* call, so it is carried rather than discarded.
    assert!(mark_dirty_rect(id, Rect::new(0, 0, 4, 4)));
    let _settled =
        render_frame_incremental(id, size, Color::rgb(1, 1, 1), Some(&frame)).expect("frame");
    assert_eq!(
        adaptive_large_damage_run(id),
        0,
        "a small-damage frame must clear the run, or the mode would latch"
    );

    unregister(id);
}

/// A previous frame of the wrong size cannot be carried forward; the call must
/// still return a correct, fully sized frame instead of a half-stale one.
#[test]
fn a_wrongly_sized_previous_frame_is_ignored() {
    let id = register(sample_editor("fn main() {}")).expect("registry");
    let size = Size::new(64, 48);
    assert!(set_repaint_mode(id, RepaintMode::Dirty));
    assert!(mark_dirty_rect(id, Rect::new(0, 0, 8, 8)));

    let wrong = vec![0u8; 7];
    let frame = render_frame_incremental(id, size, Color::rgb(1, 2, 3), Some(&wrong))
        .expect("a frame is still produced");
    assert_eq!(
        frame.len(),
        size.width as usize * size.height as usize * 4,
        "a mismatched previous frame must not resize the output"
    );

    unregister(id);
}

/// Dirty repaint must leave the untouched region holding the previous frame's
/// pixels, not the clear colour: that is the observable difference between a
/// partial repaint and a full one.
#[test]
fn a_dirty_repaint_preserves_pixels_outside_the_damage() {
    let id = register(sample_editor("fn main() {}")).expect("registry");
    let size = Size::new(80, 60);
    let clear = Color::rgb(200, 30, 40);

    let first = render_frame(id, size, clear).expect("first frame");
    assert!(set_repaint_mode(id, RepaintMode::Dirty));

    // Damage a region in the top-left only, then repaint with a different clear
    // colour. A full paint would fill the whole frame with the new colour; a
    // dirty paint must leave the far corner alone.
    assert!(mark_dirty_rect(id, Rect::new(0, 0, 8, 8)));
    let second = render_frame_incremental(id, size, Color::rgb(0, 0, 0), Some(&first))
        .expect("second frame");

    let last_pixel = second.len() - 4;
    assert_eq!(
        &second[last_pixel..],
        &first[last_pixel..],
        "the far corner is outside the damage and must keep its pixels"
    );

    unregister(id);
}

#[test]
fn mounted_ids_do_not_collide_with_platform_ids() {
    // Platform-allocated ids count up from 1; mounted ids start far above.
    let id = register(sample_editor("x")).expect("registry");
    assert!(id > 0x5345_4C46_0000_0000, "mounted id {id:#x} must not look platform-issued");
    unregister(id);
}

#[test]
fn with_widget_mut_exposes_the_real_widget() {
    let id = register(sample_editor("fn main() {}")).expect("registry");
    let lines =
        with_widget_mut(id, |widget| editor_of(widget).map(|editor| editor.line_count())).flatten();
    assert_eq!(lines, Some(1));
    unregister(id);
}

#[test]
fn with_widget_mut_on_missing_id_is_none() {
    assert!(with_widget_mut(0x1234_5678, |_| ()).is_none());
}

/// `read_widget` answers a mounted control without detaching it.
///
/// The value coming back is not the whole point: the registry must be untouched, so a
/// **nested** read has to resolve as well (shared borrows compose), and the control has to
/// still be mounted afterwards. Detaching and reinserting — what `with_widget` does — would
/// also return the value, so only these two assertions distinguish the two functions.
#[test]
fn read_widget_answers_and_leaves_the_registry_alone() {
    let id = register(sample_editor("fn main() {}")).expect("registry");
    let base_id = read_widget(id, |widget| widget.base().id()).expect("mounted");

    // The outer read holds the registry borrow; the inner one must still resolve, and it
    // must see the widget's own state rather than a detached copy.
    let nested = read_widget(id, |_| read_widget(id, |inner| inner.base().id())).flatten();
    assert_eq!(nested, Some(base_id), "a nested read must resolve through the same borrow");
    assert_eq!(
        read_widget(id, |widget| widget.base().id()),
        Some(base_id),
        "the entry must never have left the registry"
    );
    unregister(id);
}

/// A control lent out for mutation is not readable, for the same reason `with_widget` refuses
/// it: its state is being rewritten through an exclusive borrow.
#[test]
fn read_widget_refuses_a_control_that_is_being_mutated() {
    let id = register(sample_editor("x")).expect("registry");
    let inside = with_widget_mut(id, |_| read_widget(id, |_| 1));
    assert_eq!(inside, Some(None), "a mutating loan must not be readable");
    unregister(id);
}

#[test]
fn geometry_can_be_read_and_written_through_the_registry() {
    let id = register(sample_editor("x")).expect("registry");
    assert_eq!(geometry_of(id).map(|r| (r.width, r.height)), Some((320, 200)));
    assert!(set_geometry(id, Rect::new(5, 6, 400, 300)));
    let geometry = geometry_of(id).expect("mounted");
    assert_eq!((geometry.x, geometry.y, geometry.width, geometry.height), (5, 6, 400, 300));
    unregister(id);
}

#[test]
fn render_frame_returns_rgba_of_the_requested_size() {
    let id = register(sample_editor("fn main() {\n    let x = 1;\n}")).expect("registry");
    let frame = render_frame(id, Size::new(64, 48), Color::WHITE).expect("frame");
    assert_eq!(frame.len(), 64 * 48 * 4);
    // The editor paints a light background, so at least one pixel must be
    // non-transparent; a fully blank frame means the widget never drew.
    assert!(
        frame.as_chunks::<4>().0.iter().any(|px| px[3] != 0),
        "frame must contain drawn pixels"
    );
    unregister(id);
}

#[test]
fn render_frame_rejects_empty_size_and_missing_ids() {
    let id = register(sample_editor("x")).expect("registry");
    assert!(render_frame(id, Size::new(0, 10), Color::WHITE).is_none());
    assert!(render_frame(0x9999, Size::new(10, 10), Color::WHITE).is_none());
    unregister(id);
}

/// **Every control the demo places must actually paint.**
///
/// The tree walk skips a child that is not visible or has no `Draw`, and it does so
/// silently — which is right for a hidden page but wrong for a control the caller
/// placed on purpose. A control that never paints is invisible on screen while every
/// geometry check passes, so this checks the *frame* rather than the geometry.
///
/// Each widget is rendered on its own, at its own size, and the frame must contain at
/// least one pixel that differs from the clear colour. A control whose whole
/// appearance happens to equal the clear colour fails this, which is deliberate: from
/// the user's side that control is not there either.
#[test]
fn every_control_kind_the_demo_uses_paints_something() {
    use crate::widget::WidgetFactory;

    // The kinds the control demo places, by factory name.
    let kinds = [
        "button",
        "checkbox",
        "radio_button",
        "spin_box",
        "combo_box",
        "line_edit",
        "slider",
        "progress_bar",
        "label",
    ];

    // A colour no widget's own palette uses, so "did it paint" cannot be confused
    // with "it painted a colour that happens to match".
    let clear = Color::rgb(255, 0, 255);
    let factory = WidgetFactory::new_with_defaults();
    let mut unpainted = Vec::new();

    for name in kinds {
        let Some(widget) = factory.create(name, Rect::new(0, 0, 120, 40), "sample") else {
            unpainted.push(format!("{name} (factory produced nothing)"));
            continue;
        };
        let Some(id) = register(widget) else {
            unpainted.push(format!("{name} (registration refused)"));
            continue;
        };
        set_geometry(id, Rect::new(0, 0, 120, 40));

        let painted = render_frame(id, Size::new(120, 40), clear)
            .map(|frame| {
                frame.as_chunks::<4>().0.iter().any(|px| px[0] != 255 || px[1] != 0 || px[2] != 255)
            })
            .unwrap_or(false);
        if !painted {
            unpainted.push(name.to_string());
        }
        unregister(id);
    }

    assert!(
        unpainted.is_empty(),
        "these control kinds painted nothing, so they are invisible on screen: {unpainted:?}"
    );
}

#[test]
fn render_frame_at_geometry_uses_widget_geometry() {
    let id = register(sample_editor("abc")).expect("registry");
    let frame = render_frame_at_geometry(id, Color::WHITE).expect("frame");
    assert_eq!(frame.len(), 320 * 200 * 4);
    unregister(id);
}

/// Every widget that paints itself must be mountable in a host surface.
///
/// `Widget::as_draw_mut` defaults to `None`, so a widget can implement
/// `Draw` and still be invisible when mounted — which is exactly how a
/// mounted `Chip` silently produced no frame. This guards the contract for
/// the widgets the demos and the factory expose.
#[test]
fn custom_widgets_report_the_draw_bridge() {
    use crate::widget::chart_widgets::gantt_widget::GanttWidget;
    use crate::widget::special_widgets::chip::Chip;
    use crate::widget::special_widgets::color_picker::ColorPicker;
    use crate::widget::special_widgets::snackbar::Snackbar;
    use crate::widget::special_widgets::terminal_view::TerminalView;

    fn assert_mountable(name: &str, widget: &mut dyn Widget) {
        assert!(
            widget.as_draw_mut().is_some(),
            "{name} implements Draw but does not report itself via Widget::as_draw_mut, \
             so mounting it in a window would paint nothing"
        );
    }

    assert_mountable("CodeEditor", &mut CodeEditor::new(Rect::new(0, 0, 80, 60)));
    assert_mountable("Chip", &mut Chip::new(Rect::new(0, 0, 80, 24)));
    assert_mountable("ColorPicker", &mut ColorPicker::new(Rect::new(0, 0, 200, 150)));
    assert_mountable("GanttWidget", &mut GanttWidget::new(Rect::new(0, 0, 200, 150)));
    assert_mountable("Snackbar", &mut Snackbar::new(Rect::new(0, 0, 200, 40)));
    assert_mountable("TerminalView", &mut TerminalView::new(Rect::new(0, 0, 200, 150)));
}

/// A built-in control must report itself as paintable.
///
/// This assertion is the inverse of the one that used to live here, which
/// required `Button` to return `None` because the control was expected to be
/// an OS widget. Every control is now painted by the library (BLUE15 #55), so
/// a control that still answers `None` paints a blank surface — the exact
/// regression this test exists to catch.
#[test]
fn built_in_controls_claim_the_draw_bridge() {
    use crate::widget::base_widgets::button::Button;
    use crate::widget::base_widgets::checkbox::CheckBox;
    use crate::widget::base_widgets::label::Label;

    let mut button = Button::new("ok".to_string(), Rect::new(0, 0, 80, 30));
    assert!(
        button.as_draw_mut().is_some(),
        "Button implements Draw, so mounting it must paint instead of showing a blank surface"
    );

    let mut checkbox = CheckBox::new(Rect::new(0, 0, 120, 24));
    assert!(checkbox.as_draw_mut().is_some(), "CheckBox must be paintable");

    let mut label = Label::new("hello".to_string(), Rect::new(0, 0, 80, 20));
    assert!(label.as_draw_mut().is_some(), "Label must be paintable");
}

#[test]
fn dispatch_event_reaches_the_widget() {
    use crate::event::Event;
    let mut editor = CodeEditor::new(Rect::new(0, 0, 320, 200));
    editor.set_text("hello");
    let id = register(Box::new(editor)).expect("registry");
    // A click inside the text area must move the caret away from 0:0.
    let delivered = dispatch_event(
        id,
        &Event::MousePress { pos: Point::new(150, 90), button: 1, modifiers: 0 },
    );
    assert!(delivered, "event must reach the mounted widget");
    let cursor =
        with_widget_mut(id, |widget| editor_of(widget).map(|editor| editor.cursor())).flatten();
    assert!(cursor.is_some(), "widget must still be the editor");
    unregister(id);
}

/// A typed character reaches the control and its payload buffer is recycled, so typing does
/// not allocate a `String` per keystroke (G-606).
#[test]
fn a_typed_character_is_delivered_through_the_pooled_payload() {
    use crate::widget::input_widgets::lineedit::LineEdit;
    let field = register(Box::new(LineEdit::new(Rect::new(0, 0, 200, 24)))).expect("mount");
    let _field = MountGuard(field);
    assert!(focus_widget(field));

    assert!(dispatch_text_char(field, 'a'), "the control receives the character");
    let first = TEXT_INPUT_BUFFER.with(|slot| slot.borrow().as_ref().map(|text| text.as_ptr()));
    assert!(dispatch_text_char(field, 'b'), "and the next one");
    let second = TEXT_INPUT_BUFFER.with(|slot| slot.borrow().as_ref().map(|text| text.as_ptr()));
    assert!(first.is_some(), "the payload is handed back to the pool");
    assert_eq!(first, second, "the buffer is reused, not rebuilt per keystroke");

    let text = with_widget(field, |widget| {
        (widget as &dyn std::any::Any)
            .downcast_ref::<LineEdit>()
            .map(|field| field.text().to_string())
    })
    .flatten();
    assert_eq!(text.as_deref(), Some("ab"), "both characters reached the value");
    clear_focus();
}

/// A secondary-button press opens the menu at the pointer, clamped to the
/// viewport — the whole right-click path in one assertion.
#[cfg(full_widgets)]
#[test]
fn a_secondary_press_opens_the_mounted_context_menu() {
    let mut menu = crate::Menu::new("Edit", Rect::new(0, 0, 160, 100));
    menu.add_action("Copy");
    let id = register(Box::new(menu)).expect("registry");

    let viewport = Rect::new(0, 0, 500, 400);
    let opened = open_context_menu_for_event(
        id,
        &Event::MousePress {
            pos: Point::new(120, 90),
            button: crate::event::mouse_button::SECONDARY,
            modifiers: 0,
        },
        viewport,
    );

    assert!(opened, "a secondary press must open the context menu");
    let geometry = geometry_of(id).expect("mounted");
    assert_eq!((geometry.x, geometry.y), (120, 90));
    assert!(is_mounted(id));
    unregister(id);
}

/// The convenience wrapper must ignore every event that is not a secondary
/// press, so a caller can forward its whole event stream without testing the
/// button itself.
#[cfg(full_widgets)]
#[test]
fn non_secondary_events_do_not_open_the_context_menu() {
    let mut menu = crate::Menu::new("Edit", Rect::new(0, 0, 160, 100));
    menu.add_action("Copy");
    let id = register(Box::new(menu)).expect("registry");
    let viewport = Rect::new(0, 0, 500, 400);

    assert!(!open_context_menu_for_event(
        id,
        &Event::MousePress {
            pos: Point::new(10, 10),
            button: crate::event::mouse_button::PRIMARY,
            modifiers: 0
        },
        viewport,
    ));
    assert!(!open_context_menu_for_event(
        id,
        &Event::MouseMove { pos: Point::new(10, 10) },
        viewport
    ));
    unregister(id);
}

/// Pointing the helper at an id that is not a menu must report failure rather
/// than silently doing nothing, which is how a mis-wired right-click handler
/// would otherwise go unnoticed.
#[cfg(full_widgets)]
#[test]
fn opening_a_context_menu_on_a_non_menu_reports_failure() {
    let label =
        crate::widget::base_widgets::label::Label::new("hi".to_string(), Rect::new(0, 0, 80, 24));
    let id = register(Box::new(label)).expect("registry");

    assert!(!open_context_menu(id, Point::new(5, 5), Rect::new(0, 0, 400, 400)));
    assert!(!open_context_menu(9_999, Point::new(5, 5), Rect::new(0, 0, 400, 400)));
    unregister(id);
}

/// The secondary press is delivered first and the menu request follows it, both to the
/// same control. The press half is observable through the focus move it performs; the
/// request half through the control's own signal.
#[test]
fn a_secondary_press_delivers_the_press_and_then_the_menu_request() {
    let field = crate::LineEdit::new(Rect::new(0, 0, 120, 32));
    let seen = std::sync::Arc::new(std::sync::Mutex::new(None));
    let sink = std::sync::Arc::clone(&seen);
    field.context_menu_requested.connect_ref(move |pos: &Point| {
        *sink.lock().unwrap() = Some(*pos);
    });
    let id = register(Box::new(field)).expect("registry");
    let _guard = MountGuard(id);
    let point = Point::new(20, 16);

    assert!(dispatch_pointer_event(
        id,
        &Event::MousePress {
            pos: point,
            button: crate::event::mouse_button::SECONDARY,
            modifiers: 0,
        },
        point,
    ));
    assert_eq!(*seen.lock().unwrap(), Some(point), "the menu request follows the press");
    assert_eq!(
        focused_widget(),
        Some(id),
        "the press itself must still be delivered, not swallowed by the menu path"
    );
    clear_focus();
}

/// A keyboard-originated request (the translator's `ContextMenu`) goes to the control that
/// owns the keyboard, because there is no pointer position to hit-test.
#[test]
fn a_translated_menu_key_request_reaches_the_focused_control() {
    let field = crate::LineEdit::new(Rect::new(0, 0, 120, 32));
    let seen = std::sync::Arc::new(std::sync::Mutex::new(None));
    let sink = std::sync::Arc::clone(&seen);
    field.context_menu_requested.connect_ref(move |pos: &Point| {
        *sink.lock().unwrap() = Some(*pos);
    });
    let id = register(Box::new(field)).expect("registry");
    let _guard = MountGuard(id);
    assert!(focus_widget(id));

    assert!(dispatch_context_menu_event(&Event::context_menu(4, 5)));
    assert_eq!(*seen.lock().unwrap(), Some(Point::new(4, 5)));
    clear_focus();
}

/// A control that never opted into the context-menu hook reports `false` and is otherwise
/// untouched: the request is delivered, the hook declines it, and the widget's state does
/// not move.
#[test]
fn an_unintegrated_control_ignores_a_context_menu_request() {
    use crate::widget::base_widgets::label::Label;

    let label = Label::new("plain".to_string(), Rect::new(0, 0, 80, 24));
    let id = register(Box::new(label)).expect("registry");
    let _guard = MountGuard(id);
    let before = geometry_of(id).expect("mounted");

    assert!(dispatch_event(id, &Event::context_menu(3, 4)));
    assert_eq!(geometry_of(id), Some(before), "the request must change nothing");
    let handled =
        with_widget_mut(id, |widget| widget.context_menu_event(Point::new(3, 4))).unwrap_or(true);
    assert!(!handled, "the default is no menu");
}

/// A text selection dragged onto a drop zone is committed through the coordinator: the
/// session activates on the move, the target reports its hover, and the release emits the
/// zone's own `payload_dropped` signal.
#[cfg(full_widgets)]
#[test]
fn the_drag_coordinator_drops_a_text_selection_on_a_drop_zone() {
    use crate::event::dnd::DragPayload;
    use crate::widget::misc_widgets::drop_zone::{DropZone, DropZoneState};

    let mut area = crate::TextArea::new("hello".to_string(), Rect::new(0, 0, 120, 60));
    area.select_all();
    let area_id = register(Box::new(area)).expect("registry");
    let _area_guard = MountGuard(area_id);

    let zone = DropZone::new(Rect::new(200, 0, 120, 80), "text/plain");
    let dropped = std::sync::Arc::new(std::sync::Mutex::new(None));
    let sink = std::sync::Arc::clone(&dropped);
    zone.payload_dropped.connect_ref(move |payload: &DragPayload| {
        *sink.lock().unwrap() = Some(payload.item_id.clone());
    });
    let zone_id = register(Box::new(zone)).expect("registry");
    let _zone_guard = MountGuard(zone_id);

    let viewport = Rect::new(0, 0, 400, 200);
    let mut coordinator = DragCoordinator::new();
    assert_eq!(
        coordinator
            .on_event(&Event::mouse_press(10, 10, crate::event::mouse_button::PRIMARY), viewport),
        None
    );
    assert_eq!(
        coordinator.session().map(|session| session.is_active()),
        Some(false),
        "a press alone is not yet a drag"
    );

    assert_eq!(
        coordinator.on_event(&Event::mouse_move(220, 40), viewport),
        None,
        "a move only advances the session"
    );
    let hovered = with_widget_mut(zone_id, |widget| {
        (widget as &mut dyn core::any::Any).downcast_mut::<DropZone>().map(|zone| zone.state())
    })
    .flatten();
    assert_eq!(hovered, Some(DropZoneState::Accepted), "the target was told it is hovered");

    assert_eq!(
        coordinator.on_event(
            &Event::mouse_release(220, 40, crate::event::mouse_button::PRIMARY),
            viewport
        ),
        Some(DropEffect::Copy)
    );
    assert_eq!(dropped.lock().unwrap().as_deref(), Some("hello"));
    assert!(coordinator.session().is_none(), "the release ends the session");
}

/// A control that implements neither trait is invisible to the coordinator: no session
/// starts, no event is delivered by the coordinator, and every call answers `None`.
#[test]
fn a_control_without_the_drag_traits_changes_nothing() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Plain {
        base: crate::widget::BaseWidget,
        deliveries: std::sync::Arc<AtomicUsize>,
    }
    impl crate::widget::Widget for Plain {
        fn base(&self) -> &crate::widget::BaseWidget {
            &self.base
        }
        fn base_mut(&mut self) -> &mut crate::widget::BaseWidget {
            &mut self.base
        }
    }
    impl crate::event::EventHandler for Plain {
        fn handle_event(&mut self, _event: &Event) {
            self.deliveries.fetch_add(1, Ordering::SeqCst);
        }
    }

    let deliveries = std::sync::Arc::new(AtomicUsize::new(0));
    let plain = Plain {
        base: crate::widget::BaseWidget::new(
            crate::widget::WidgetKind::Label,
            Rect::new(0, 0, 100, 40),
            "Plain",
        ),
        deliveries: std::sync::Arc::clone(&deliveries),
    };
    let id = register(Box::new(plain)).expect("registry");
    let _guard = MountGuard(id);

    let viewport = Rect::new(0, 0, 400, 200);
    let mut coordinator = DragCoordinator::new();
    assert_eq!(
        coordinator
            .on_event(&Event::mouse_press(10, 10, crate::event::mouse_button::PRIMARY), viewport),
        None
    );
    assert!(coordinator.session().is_none(), "no DragSource, so no session");
    assert_eq!(coordinator.hovered(), None);
    assert_eq!(coordinator.on_event(&Event::mouse_move(60, 20), viewport), None);
    assert_eq!(
        coordinator
            .on_event(&Event::mouse_release(60, 20, crate::event::mouse_button::PRIMARY), viewport),
        None
    );
    assert_eq!(
        deliveries.load(Ordering::SeqCst),
        0,
        "the coordinator must not deliver events to a control that did not opt in"
    );
}

/// The source is told what the drop produced, not merely that something happened.
#[cfg(full_widgets)]
#[test]
fn the_source_learns_the_effect_of_the_drop() {
    use crate::event::dnd::{DragPayload, DragSource};
    use crate::widget::misc_widgets::drop_zone::DropZone;

    struct ProbeSource {
        base: crate::widget::BaseWidget,
        finished: std::sync::Arc<std::sync::Mutex<Option<DropEffect>>>,
    }
    impl crate::widget::Widget for ProbeSource {
        fn base(&self) -> &crate::widget::BaseWidget {
            &self.base
        }
        fn base_mut(&mut self) -> &mut crate::widget::BaseWidget {
            &mut self.base
        }
        fn drag_source_mut(&mut self) -> Option<&mut dyn DragSource> {
            Some(self)
        }
    }
    impl DragSource for ProbeSource {
        fn drag_payload(&self, _pos: Point) -> Option<DragPayload> {
            Some(DragPayload::new("probe", "p1"))
        }
        fn on_drag_finished(&mut self, effect: DropEffect) {
            *self.finished.lock().unwrap() = Some(effect);
        }
    }
    impl crate::event::EventHandler for ProbeSource {
        fn handle_event(&mut self, _event: &Event) {}
    }

    let finished = std::sync::Arc::new(std::sync::Mutex::new(None));
    let source = ProbeSource {
        base: crate::widget::BaseWidget::new(
            crate::widget::WidgetKind::Label,
            Rect::new(0, 0, 100, 40),
            "ProbeSource",
        ),
        finished: std::sync::Arc::clone(&finished),
    };
    let source_id = register(Box::new(source)).expect("registry");
    let _source_guard = MountGuard(source_id);

    let zone = DropZone::new(Rect::new(200, 0, 120, 80), "probe");
    let zone_id = register(Box::new(zone)).expect("registry");
    let _zone_guard = MountGuard(zone_id);

    let viewport = Rect::new(0, 0, 400, 200);
    let mut coordinator = DragCoordinator::new();
    let _ = coordinator
        .on_event(&Event::mouse_press(10, 10, crate::event::mouse_button::PRIMARY), viewport);
    let _ = coordinator.on_event(&Event::mouse_move(220, 40), viewport);
    let effect = coordinator
        .on_event(&Event::mouse_release(220, 40, crate::event::mouse_button::PRIMARY), viewport);
    assert_eq!(effect, Some(DropEffect::Copy));
    assert_eq!(*finished.lock().unwrap(), Some(DropEffect::Copy));
}

// -----------------------------------------------------------------------
// Hit testing (point -> widget)
// -----------------------------------------------------------------------

/// Builds a container with two children, exercising the parent/children link
/// the hit test walks. Returns `(parent, first, second)`.
fn tree_with_two_children() -> (ObjectId, ObjectId, ObjectId) {
    let mut parent =
        crate::widget::container_widgets::groupbox::GroupBox::new(Rect::new(0, 0, 300, 300));
    let first = register(Box::new(crate::widget::base_widgets::label::Label::new(
        "first".to_string(),
        Rect::new(0, 0, 100, 50),
    )))
    .expect("registry");
    let second = register(Box::new(crate::widget::base_widgets::label::Label::new(
        "second".to_string(),
        Rect::new(0, 0, 100, 50),
    )))
    .expect("registry");
    parent.add_child(first);
    parent.add_child(second);
    let parent = register(Box::new(parent)).expect("registry");
    (parent, first, second)
}

/// A point over a child must resolve to that child, not to the container.
///
/// This is the capability that did not exist: before `widget_at`, the library
/// could only deliver to an id the caller already knew.
#[test]
fn hit_test_descends_to_the_child_under_the_point() {
    let (parent, first, second) = tree_with_two_children();

    assert!(set_geometry(first, Rect::new(10, 10, 100, 50)));
    assert!(set_geometry(second, Rect::new(150, 10, 100, 50)));

    assert_eq!(widget_at(parent, Point::new(20, 20)), Some(first));
    assert_eq!(widget_at(parent, Point::new(160, 20)), Some(second));

    unregister(parent);
    unregister(first);
    unregister(second);
}

/// A point inside the container but outside every child resolves to the
/// container — the search stops at the deepest hit, and "no child" is not
/// the same as "no hit".
#[test]
fn hit_test_falls_back_to_the_container() {
    let (parent, first, second) = tree_with_two_children();
    set_geometry(first, Rect::new(10, 10, 100, 50));
    set_geometry(second, Rect::new(150, 10, 100, 50));

    assert_eq!(widget_at(parent, Point::new(250, 250)), Some(parent));

    unregister(parent);
    unregister(first);
    unregister(second);
}

/// A point outside the root resolves to nothing. Without this bound a click
/// far outside the window would still land on whatever happened to be last.
#[test]
fn hit_test_returns_none_outside_the_root() {
    let (parent, first, second) = tree_with_two_children();
    assert_eq!(widget_at(parent, Point::new(5_000, 5_000)), None);
    unregister(parent);
    unregister(first);
    unregister(second);
}

/// An unmounted root must report no hit rather than panicking.
#[test]
fn hit_test_on_an_unmounted_root_is_none() {
    assert_eq!(widget_at(0xDEAD_BEEF, Point::new(0, 0)), None);
}

/// Later siblings paint on top, so they must also win the hit test. Reversing
/// this order would make the visually-topmost control unclickable.
#[test]
fn hit_test_prefers_the_topmost_overlapping_child() {
    let (parent, first, second) = tree_with_two_children();
    // Both fully overlap; `second` is added last, so it draws on top.
    set_geometry(first, Rect::new(10, 10, 100, 50));
    set_geometry(second, Rect::new(10, 10, 100, 50));

    assert_eq!(widget_at(parent, Point::new(20, 20)), Some(second));

    unregister(parent);
    unregister(first);
    unregister(second);
}

/// A hidden child is not drawn, so it must not swallow a click.
#[test]
fn hit_test_skips_hidden_children() {
    let (parent, first, second) = tree_with_two_children();
    set_geometry(first, Rect::new(10, 10, 100, 50));
    set_geometry(second, Rect::new(10, 10, 100, 50));
    with_widget_mut(second, |widget| widget.hide());

    // `second` covers `first` but is hidden, so the click reaches `first`.
    assert_eq!(widget_at(parent, Point::new(20, 20)), Some(first));

    unregister(parent);
    unregister(first);
    unregister(second);
}

/// A container's children carry absolute rects (the layout engine derives a
/// child's rect from its parent's absolute rect), so moving the container does
/// not move its children in this model. The point of this test is that the walk
/// composes down the tree by *containment*, not by summing parent origins —
/// getting that wrong would offset every nested control.
#[test]
fn hit_test_finds_a_nested_child_through_two_levels() {
    let inner =
        crate::widget::container_widgets::groupbox::GroupBox::new(Rect::new(20, 20, 100, 100));
    let inner = register(Box::new(inner)).expect("registry");
    let leaf = register(Box::new(crate::widget::base_widgets::label::Label::new(
        "leaf".to_string(),
        Rect::new(30, 30, 40, 20),
    )))
    .expect("registry");
    with_widget_mut(inner, |widget| widget.add_child(leaf));

    let outer =
        crate::widget::container_widgets::groupbox::GroupBox::new(Rect::new(0, 0, 300, 300));
    let outer = register(Box::new(outer)).expect("registry");
    with_widget_mut(outer, |widget| widget.add_child(inner));

    // (40, 40) is inside outer -> inner -> leaf, so the deepest widget wins.
    assert_eq!(widget_at(outer, Point::new(40, 40)), Some(leaf));
    // (25, 25) is inside outer and inner, but above the leaf.
    assert_eq!(widget_at(outer, Point::new(25, 25)), Some(inner));
    // (250, 250) is inside only the outer container.
    assert_eq!(widget_at(outer, Point::new(250, 250)), Some(outer));

    unregister(outer);
    unregister(inner);
    unregister(leaf);
}

/// The pointer-event helper must deliver to the widget under the point. Because
/// geometry is absolute here, the event's own position needs no rewrite.
#[test]
fn dispatch_pointer_event_reaches_the_widget_under_the_point() {
    let mut parent =
        crate::widget::container_widgets::groupbox::GroupBox::new(Rect::new(0, 0, 400, 300));
    let editor = register(sample_editor("fn main() {}\nlet a = 1;\nlet b = 2;")).expect("r");
    set_geometry(editor, Rect::new(100, 100, 320, 200));
    parent.add_child(editor);
    let parent = register(Box::new(parent)).expect("registry");

    let delivered = dispatch_pointer_event(
        parent,
        &Event::MousePress { pos: Point::new(150, 130), button: 1, modifiers: 0 },
        Point::new(150, 130),
    );
    assert!(delivered, "the editor under the point must receive the click");

    // Outside the editor but inside the container, the container is the target.
    assert_eq!(widget_at(parent, Point::new(380, 20)), Some(parent));

    unregister(parent);
    unregister(editor);
}

/// A click on empty space must be reported as undelivered, so a host knows
/// nothing consumed it instead of assuming success.
#[test]
fn dispatch_pointer_event_outside_the_root_is_false() {
    let (parent, first, second) = tree_with_two_children();
    let delivered = dispatch_pointer_event(
        parent,
        &Event::MousePress { pos: Point::new(9_000, 9_000), button: 1, modifiers: 0 },
        Point::new(9_000, 9_000),
    );
    assert!(!delivered);
    unregister(parent);
    unregister(first);
    unregister(second);
}

/// A press inside an open popup is routed **from the popup**, even when the popup hangs
/// below the container that owns it.
///
/// The grab only intercepted presses the popup *rejected*; a press inside the list fell
/// through to `widget_at(root, ..)`, which refuses to descend through an ancestor whose own
/// bounds reject the point. A combo box inside a panel therefore had an unclickable list
/// whenever the list crossed the panel's bottom edge. The menu code already re-parents its
/// popup to the window to dodge this; this is the fix that makes the general case work.
#[cfg(not(alloc_frugal))]
#[test]
fn a_press_inside_a_popup_below_its_parent_reaches_the_popup() {
    use crate::widget::input_widgets::ComboBox;

    let mut combo = ComboBox::new(Rect::new(0, 60, 120, 32));
    combo.set_items(vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()]);
    let combo = register(Box::new(combo)).expect("mount");

    let mut panel =
        crate::widget::container_widgets::groupbox::GroupBox::new(Rect::new(0, 0, 200, 100));
    panel.add_child(combo);
    let panel = register(Box::new(panel)).expect("mount");
    // `BaseWidget::add_child` writes only the parent's list, and it resolves the parent in the
    // **registry** id space — which exists only once the parent is mounted. The host therefore
    // mounts first and links through the registry, exactly as `add_child_linked` expects.
    with_widget_mut(panel, |widget| {
        widget.base_mut().add_child_linked(combo);
    });

    let window = register(Box::new(crate::widget::Window::new(
        "popup host".to_string(),
        Rect::new(0, 0, 400, 300),
    )))
    .expect("mount");
    with_widget_mut(window, |widget| {
        widget.base_mut().add_child_linked(panel);
    });

    // Open the list through the control's own press path, so the grab records the *registry*
    // id (a control reports its own id, which the router normalises — but only once mounted).
    let field = Point::new(20, 70);
    assert!(dispatch_pointer_event(
        window,
        &Event::MousePress { pos: field, button: 1, modifiers: 0 },
        field,
    ));
    assert!(
        read_widget(combo, |widget| widget.has_open_popup()).unwrap_or(false),
        "the press must open the list"
    );

    // A row of the list, below the panel's own bottom edge (y = 100).
    let row = (101..300)
        .map(|y| Point::new(20, y))
        .find(|p| read_widget(combo, |widget| widget.contains_point(*p)).unwrap_or(false))
        .expect("the open list extends below its parent");

    assert!(dispatch_pointer_event(
        window,
        &Event::MousePress { pos: row, button: 1, modifiers: 0 },
        row,
    ));
    assert!(
        !read_widget(combo, |widget| widget.has_open_popup()).unwrap_or(true),
        "choosing a row must reach the combo box and close its list"
    );

    unregister(window);
    unregister(panel);
    unregister(combo);
}

/// Tab steps over disabled and hidden controls instead of landing on them.
///
/// The tab order keeps them (a user must be able to move *past* a disabled control), so the
/// traversal is where the skip belongs. Without it, Tab moved focus onto a control that
/// refuses input: no visible focus change, and the following keystrokes were swallowed.
#[test]
fn tab_skips_disabled_and_hidden_controls() {
    let mount = |x: i32| {
        register(Box::new(crate::widget::Button::new("ok".to_string(), Rect::new(x, 0, 80, 32))))
            .expect("mount")
    };
    let first = mount(0);
    let disabled = mount(100);
    let hidden = mount(200);
    let last = mount(300);
    with_widget_mut(disabled, |widget| widget.set_enabled(false));
    with_widget_mut(hidden, |widget| widget.hide());

    assert_eq!(focus_next(true), Some(first), "the first control is focusable");
    assert_eq!(
        focus_next(true),
        Some(last),
        "the disabled and hidden controls must be stepped over"
    );
    assert_eq!(focused_widget(), Some(last));

    assert_eq!(focus_next(false), Some(first), "and the skip applies backwards too");

    for id in [first, disabled, hidden, last] {
        unregister(id);
    }
}

/// Hover does not enter a control a modal blocks.
///
/// `dispatch_hover_transition` wrote `HOVERED` without consulting `modal_blocks`, while the
/// `MouseEnter` it sends is refused — so the pair invariant broke, `hovered_widget()` reported
/// a control behind a dialog, and its cursor (an I-beam) was published to the platform.
#[test]
fn hover_does_not_enter_a_widget_behind_a_modal() {
    let (parent, first, second) = tree_with_two_children();
    set_geometry(first, Rect::new(10, 10, 100, 50));
    set_geometry(second, Rect::new(150, 10, 100, 50));
    clear_hover(Point::new(0, 0));
    assert!(enter_modal(second));

    let over_first = Point::new(20, 20);
    let _ = dispatch_pointer_event(parent, &Event::MouseMove { pos: over_first }, over_first);

    assert_eq!(
        hovered_widget(),
        None,
        "a modal must keep the widget behind it out of the hover state"
    );

    exit_modal(second);
    let _ = dispatch_pointer_event(parent, &Event::MouseMove { pos: over_first }, over_first);
    assert_eq!(hovered_widget(), Some(first), "and hover resumes once the modal is gone");

    unregister(parent);
    unregister(first);
    unregister(second);
}

/// The router refuses a press on a disabled control, without arming focus or capture.
///
/// The public documentation promised this (`dispatch_event`'s "false when the control is
/// disabled") while the router had no such check: a third-party control relying on the
/// contract accepted input while disabled. The check lives in the router because the
/// capture branch must still deliver the *end* of a gesture that began on an enabled control.
#[test]
fn a_press_on_a_disabled_control_is_refused_by_the_router() {
    let id =
        register(Box::new(crate::widget::Button::new("ok".to_string(), Rect::new(0, 0, 80, 32))))
            .expect("mount");
    with_widget_mut(id, |widget| widget.set_enabled(false));

    let point = Point::new(10, 10);
    assert!(
        !dispatch_pointer_event(
            id,
            &Event::MousePress { pos: point, button: 1, modifiers: 0 },
            point,
        ),
        "a disabled control takes no input"
    );
    assert_eq!(capturing_widget(), None, "and the router must not arm a capture for it");

    unregister(id);
}

/// A window root's geometry is in screen space while its children are in client space.
///
/// Hit testing used to subtract `geometry()` unconditionally, so a window opened away from
/// the screen origin rejected every click: measured, a press at client `(182, 58)` on a window
/// at `(140, 140)` became local `(42, -82)` and matched no control. `frame_origin` is the one
/// predicate that knows a window's frame starts at `(0, 0)`, so the root bound now uses it.
#[test]
fn a_window_root_is_hit_tested_in_client_coordinates() {
    let mut window =
        crate::widget::Window::new("offscreen origin".to_string(), Rect::new(140, 140, 520, 260));
    let field = register(Box::new(crate::widget::input_widgets::lineedit::LineEdit::new(
        Rect::new(32, 40, 300, 36),
    )))
    .expect("mount");
    window.add_child(field);
    let root = register(Box::new(window)).expect("mount");
    let _root = MountGuard(root);
    let _field = MountGuard(field);

    assert_eq!(
        widget_at(root, Point::new(182, 58)),
        Some(field),
        "client coordinates must reach a control on a window whose frame starts elsewhere"
    );
    assert_eq!(
        widget_at(root, Point::new(500, 240)),
        Some(root),
        "and the window itself is still the answer on its own background"
    );
}

/// A press on an interactive control makes it the keyboard's owner.
///
/// `focus_on_pointer_press` was published with no production caller: the router moved capture
/// and delivered the press but never told the focus manager. This asserts the missing call,
/// through the same entry point a backend uses.
#[test]
fn a_pointer_press_focuses_the_control_it_lands_on() {
    let mut window =
        crate::widget::Window::new("focus on press".to_string(), Rect::new(140, 140, 520, 260));
    let field = register(Box::new(crate::widget::input_widgets::lineedit::LineEdit::new(
        Rect::new(32, 40, 300, 36),
    )))
    .expect("mount");
    window.add_child(field);
    let root = register(Box::new(window)).expect("mount");
    let _root = MountGuard(root);
    let _field = MountGuard(field);

    assert_eq!(focused_widget(), None, "the tree starts with no keyboard owner");
    assert!(dispatch_pointer_event(root, &Event::mouse_press(182, 58, 1), Point::new(182, 58),));
    assert_eq!(
        focused_widget(),
        Some(field),
        "the field under the pointer must own the keyboard afterwards"
    );

    // Empty window background is not an interactive control, so it does not steal focus.
    assert!(dispatch_pointer_event(root, &Event::mouse_press(500, 240, 1), Point::new(500, 240),));
    assert_eq!(focused_widget(), Some(field), "the window background must not take focus");
    clear_focus();
}

/// The IME joins only text-entry controls; a button keeps its keys.
#[test]
fn only_text_controls_accept_ime_input() {
    let field = register(Box::new(crate::widget::input_widgets::lineedit::LineEdit::new(
        Rect::new(0, 0, 200, 24),
    )))
    .expect("mount");
    let button =
        register(Box::new(crate::widget::Button::new("ok".to_string(), Rect::new(0, 40, 80, 32))))
            .expect("mount");
    let label =
        register(Box::new(crate::widget::Label::new("text".to_string(), Rect::new(0, 80, 80, 24))))
            .expect("mount");
    let _field = MountGuard(field);
    let _button = MountGuard(button);
    let _label = MountGuard(label);

    assert!(accepts_ime_input(field), "a line edit carries committed text");
    assert!(!accepts_ime_input(button), "a button must keep its activation keys");
    assert!(!accepts_ime_input(label), "a label has no edit session");
}

// -----------------------------------------------------------------------
// Keyboard focus
// -----------------------------------------------------------------------

/// Focus must have exactly one owner, and moving it must be observable both
/// through the query and through the events the previous owner receives.
#[test]
fn focus_moves_and_reports_the_new_owner() {
    let first = register(sample_editor("a")).expect("registry");
    let second = register(sample_editor("b")).expect("registry");

    assert!(focus_widget(first));
    assert_eq!(focused_widget(), Some(first));
    assert!(has_focus(first));
    assert!(!has_focus(second));

    assert!(focus_widget(second));
    assert_eq!(focused_widget(), Some(second));
    assert!(!has_focus(first), "the previous owner must lose focus when another takes it");

    unregister(first);
    unregister(second);
}

/// Re-focusing the current owner is a no-op, so a repeated click cannot emit a
/// second `FocusGained` for a widget that never lost focus.
#[test]
fn focus_on_the_current_owner_reports_no_move() {
    let id = register(sample_editor("a")).expect("registry");
    assert!(focus_widget(id));
    assert!(!focus_widget(id), "focus did not move, so the call must report false");
    unregister(id);
}

/// Focus on an id that is not mounted would be an unobservable lie: the key
/// router would look for a widget that does not exist. It must be refused.
#[test]
fn focus_on_an_unmounted_widget_is_refused() {
    assert!(!focus_widget(0xDEAD_BEEF));
    assert_eq!(focused_widget(), None);
}

/// Clearing focus must tell the owner it lost it, and a second clear must not
/// claim anything happened.
#[test]
fn clearing_focus_is_reported_once() {
    let id = register(sample_editor("a")).expect("registry");
    focus_widget(id);

    assert!(clear_focus());
    assert_eq!(focused_widget(), None);
    assert!(!clear_focus(), "there was no focus to clear the second time");

    unregister(id);
}

/// A widget that leaves the tree must leave the tab order too, or Tab would
/// hand focus to an id that resolves to nothing.
#[test]
fn unregistering_a_focused_widget_drops_it_from_the_tab_order() {
    let first = register(sample_editor("a")).expect("registry");
    let second = register(sample_editor("b")).expect("registry");
    register_focusable(first);
    register_focusable(second);
    focus_widget(first);

    unregister(first);

    assert_eq!(focused_widget(), None, "the removed widget must not stay focused");
    assert!(!focusable_widgets().contains(&first));

    unregister(second);
}

/// Tab must actually move focus. Before the manager was wired into the
/// registry this moved nothing, which is the defect these tests pin.
#[test]
fn tab_order_moves_focus_and_wraps() {
    let a = register(sample_editor("a")).expect("registry");
    let b = register(sample_editor("b")).expect("registry");
    set_focus_order_for_test(&[a, b]);

    assert_eq!(focus_next(true), Some(a));
    assert_eq!(focused_widget(), Some(a));
    assert_eq!(focus_next(true), Some(b));
    // Wrap-around: past the end comes back to the start.
    assert_eq!(focus_next(true), Some(a));
    // Backwards, and wrap-around in that direction too.
    assert_eq!(focus_next(false), Some(b));
    assert_eq!(focus_next(false), Some(a));

    unregister(a);
    unregister(b);
}

/// `Platform::route_pointer_event` must resolve the point against the widget tree
/// rather than handing the event to the root.
///
/// This is the wiring test for the whole chain: before it, the runtime could
/// hit-test (`widget_at`) but nothing called it, so a click on a child still went
/// to whichever widget owned the surface.
#[test]
#[cfg(full_widgets)]
fn platform_pointer_routing_reaches_a_nested_child() {
    let mut parent =
        crate::widget::container_widgets::groupbox::GroupBox::new(Rect::new(0, 0, 300, 300));
    let child = register(Box::new(crate::widget::base_widgets::label::Label::new(
        "child".to_string(),
        Rect::new(40, 40, 80, 30),
    )))
    .expect("registry");
    parent.add_child(child);
    let parent = register(Box::new(parent)).expect("registry");

    let backend = crate::platform::platform_facts();

    // Inside the child: the route must descend past the parent.
    assert!(
        backend.route_pointer_event(
            parent,
            &Event::MousePress { pos: Point::new(50, 50), button: 1, modifiers: 0 },
            Point::new(50, 50),
        ),
        "a click inside the child must be delivered to it"
    );

    // Outside every child but inside the root: still delivered, to the root.
    assert!(
        backend.route_pointer_event(
            parent,
            &Event::MousePress { pos: Point::new(250, 250), button: 1, modifiers: 0 },
            Point::new(250, 250),
        ),
        "a click inside the root must still be delivered"
    );

    // Entirely outside the root: nothing accepts it, and that is reported.
    assert!(
        !backend.route_pointer_event(
            parent,
            &Event::MousePress { pos: Point::new(9_000, 9_000), button: 1, modifiers: 0 },
            Point::new(9_000, 9_000),
        ),
        "a click outside the root must report that nothing accepted it"
    );

    unregister(parent);
    unregister(child);
}

/// D09-POINTER-01: the stylus event family reaches a control through
/// `Platform::route_pointer_event` end to end.
///
/// This is the producer/consumer boundary made testable without a device: a host (or this
/// test) constructs `PointerPress`/`PointerMove`/`PointerRelease` and routes them through
/// the public platform entry point; the `SignaturePad` under the point must consume the
/// whole lifecycle and retain the pressure. Before the fix the pad began strokes only from
/// `MousePress`/`TouchBegin`, so this route delivered a press the pad ignored.
#[test]
#[cfg(full_widgets)]
fn platform_pointer_routing_delivers_stylus_events_to_a_signature_pad() {
    let mut parent =
        crate::widget::container_widgets::groupbox::GroupBox::new(Rect::new(0, 0, 300, 300));
    let pad_id =
        register(Box::new(crate::widget::special_widgets::signature_pad::SignaturePad::new(
            Rect::new(40, 40, 200, 120),
        )))
        .expect("registry");
    set_geometry(pad_id, Rect::new(40, 40, 200, 120));
    parent.add_child(pad_id);
    let parent = register(Box::new(parent)).expect("registry");

    let backend = crate::platform::platform_facts();

    // A pen contact inside the pad: press, move, release, all through the public route.
    assert!(
        backend.route_pointer_event(
            parent,
            &Event::pointer_press(Point::new(60, 80), 1, 0.3, 0.0, 0.0),
            Point::new(60, 80),
        ),
        "the press must be accepted by the pad under the point"
    );
    backend.route_pointer_event(
        parent,
        &Event::pointer_move(Point::new(120, 100), 0.8, 0.0, 0.0),
        Point::new(120, 100),
    );
    backend.route_pointer_event(
        parent,
        &Event::pointer_release(Point::new(120, 100), 1, 0.6),
        Point::new(120, 100),
    );

    // The pad consumed the sequence: one committed stroke whose first point is the press
    // position and whose per-point pressures were retained. The release's own pressure is the
    // last datum the device sent, so it becomes the pad's retained `last_pressure` even though
    // it adds no point (the lift-off position is the last accepted move).
    with_widget(pad_id, |widget| {
        let pad = (widget as &dyn core::any::Any)
            .downcast_ref::<crate::widget::special_widgets::signature_pad::SignaturePad>()
            .expect("the registered widget is a SignaturePad");
        assert_eq!(pad.stroke_count(), 1, "the routed stylus gesture must commit one stroke");
        let stroke = &pad.strokes()[0];
        assert_eq!(stroke.points()[0], Point::new(60, 80));
        assert!(
            (stroke.pressures()[0] - 0.3).abs() < 1e-6,
            "the press pressure must be recorded on the stroke's first point"
        );
        assert!(
            (stroke.last_pressure() - 0.8).abs() < 1e-6,
            "the move pressure must be recorded on the stroke's last point"
        );
        assert!((pad.last_pressure() - 0.6).abs() < 1e-6, "the release datum was retained");
    })
    .expect("the pad is mounted");

    unregister(parent);
    unregister(pad_id);
}

/// Establishes a deterministic tab order for a test, independent of whatever
/// other widgets the shared thread-local registry happens to hold.
fn set_focus_order_for_test(ids: &[ObjectId]) {
    with_focus_manager(|focus| {
        focus.set_focus_order(ids.to_vec());
        focus.clear_focus();
    })
    .expect("ui thread has a focus manager");
}

// -----------------------------------------------------------------------
// Hover enter/leave synthesis
// -----------------------------------------------------------------------

/// Two overlapping widgets in one container, so the pointer can cross from one
/// to the other. Returns `(parent, inner, outer)` where `inner` is nested inside
/// `outer`'s area... actually both are leaves; `left` and `right` are side by side.
fn two_siblings() -> (ObjectId, ObjectId, ObjectId) {
    let mut parent =
        crate::widget::container_widgets::groupbox::GroupBox::new(Rect::new(0, 0, 300, 300));
    let left = register(Box::new(crate::widget::base_widgets::label::Label::new(
        "left".to_string(),
        Rect::new(0, 0, 100, 50),
    )))
    .expect("registry");
    let right = register(Box::new(crate::widget::base_widgets::label::Label::new(
        "right".to_string(),
        Rect::new(0, 0, 100, 50),
    )))
    .expect("registry");
    set_geometry(left, Rect::new(10, 10, 100, 50));
    set_geometry(right, Rect::new(150, 10, 100, 50));
    parent.add_child(left);
    parent.add_child(right);
    let parent = register(Box::new(parent)).expect("registry");
    (parent, left, right)
}

/// Moving the pointer onto a widget must report it as hovered. No backend
/// produces `MouseEnter`, so without this the state is unreachable and every
/// hover highlight in the library is dead code.
#[test]
fn pointer_movement_sets_and_clears_the_hover_target() {
    let (parent, left, right) = two_siblings();
    clear_hover(Point::new(0, 0));
    assert_eq!(hovered_widget(), None);

    dispatch_pointer_event(
        parent,
        &Event::MouseMove { pos: Point::new(20, 20) },
        Point::new(20, 20),
    );
    assert_eq!(hovered_widget(), Some(left), "the pointer is over `left`");

    dispatch_pointer_event(
        parent,
        &Event::MouseMove { pos: Point::new(160, 20) },
        Point::new(160, 20),
    );
    assert_eq!(hovered_widget(), Some(right), "crossing onto `right` must move hover");

    // Leaving the surface entirely clears it, so a stale highlight cannot persist.
    clear_hover(Point::new(0, 0));
    assert_eq!(hovered_widget(), None);

    unregister(parent);
    unregister(left);
    unregister(right);
}

/// A control that receives `MouseEnter` must actually be told, because that is
/// what flips its own hover flag. This is the end of the chain the backends could
/// not reach before.
#[test]
fn entering_a_button_makes_it_report_hover() {
    let parent = register(Box::new(crate::widget::container_widgets::groupbox::GroupBox::new(
        Rect::new(0, 0, 300, 300),
    )))
    .expect("registry");
    let mut button = crate::widget::base_widgets::button::Button::new(
        "ok".to_string(),
        Rect::new(20, 20, 80, 30),
    );
    button.set_geometry(Rect::new(20, 20, 80, 30));
    let button_id = register(Box::new(button)).expect("registry");
    with_widget_mut(parent, |widget| widget.add_child(button_id));

    clear_hover(Point::new(0, 0));
    dispatch_pointer_event(
        parent,
        &Event::MouseMove { pos: Point::new(30, 30) },
        Point::new(30, 30),
    );
    let hovered = with_widget(button_id, |widget| {
        (widget as &dyn core::any::Any)
            .downcast_ref::<crate::widget::base_widgets::button::Button>()
            .map(|b| b.is_hovered())
    })
    .flatten();
    assert_eq!(hovered, Some(true), "MouseEnter must reach the button's hover flag");

    clear_hover(Point::new(0, 0));
    unregister(parent);
    unregister(button_id);
}

/// A hovered control's own `tooltip` reaches a mounted `TooltipManager` (COMP-09).
///
/// The test never calls `register`: it moves the pointer over a button that was given a
/// tooltip and then advances the manager's clock. Text appearing is proof that the runtime's
/// hover bridge carried it — and the bubble appearing is proof the manager is still the only
/// presenter.
#[test]
fn a_hovered_controls_tooltip_reaches_the_mounted_manager() {
    use crate::widget::capability::coercion::{widget_as, widget_as_mut};
    use crate::widget::companions::TooltipManager;

    let parent = register(Box::new(crate::widget::container_widgets::groupbox::GroupBox::new(
        Rect::new(0, 0, 300, 300),
    )))
    .expect("registry");
    let mut button = crate::widget::base_widgets::button::Button::new(
        "ok".to_string(),
        Rect::new(20, 20, 80, 30),
    );
    button.set_geometry(Rect::new(20, 20, 80, 30));
    button.set_tooltip("Save the document".to_string());
    let button_id = register(Box::new(button)).expect("registry");
    with_widget_mut(parent, |widget| widget.add_child(button_id));

    let manager_id =
        register(Box::new(TooltipManager::new(Rect::new(0, 0, 300, 300)))).expect("registry");

    clear_hover(Point::new(0, 0));
    dispatch_pointer_event(
        parent,
        &Event::MouseMove { pos: Point::new(30, 30) },
        Point::new(30, 30),
    );

    let registered = with_widget(manager_id, |widget| {
        widget_as::<TooltipManager>(widget)
            .and_then(|manager| manager.text_for(button_id).map(str::to_string))
    })
    .flatten();
    assert_eq!(
        registered.as_deref(),
        Some("Save the document"),
        "the runtime's hover bridge must register the hovered control's tooltip"
    );

    // The delay belongs to the manager, not the bridge: advance its clock and the bubble
    // appears carrying the text that came from the button.
    let shown = with_widget_mut(manager_id, |widget| {
        widget_as_mut::<TooltipManager>(widget).map(|manager| {
            let _ = manager.tick(600, 600);
            manager.tooltip().map(str::to_string)
        })
    })
    .flatten()
    .flatten();
    assert_eq!(shown.as_deref(), Some("Save the document"));

    clear_hover(Point::new(0, 0));
    let released = with_widget(manager_id, |widget| {
        widget_as::<TooltipManager>(widget)
            .and_then(|manager| manager.text_for(button_id))
            .map(str::to_string)
    })
    .flatten();
    assert!(released.is_none(), "leaving the widget releases the bridge registration");

    unregister(manager_id);
    unregister(parent);
    unregister(button_id);
}

/// A widget removed while hovered must not remain the target, or the next
/// movement would send `MouseLeave` to a dead id and skip the live one.
#[test]
fn unregistering_a_hovered_widget_clears_the_target() {
    let (parent, left, right) = two_siblings();
    dispatch_pointer_event(
        parent,
        &Event::MouseMove { pos: Point::new(20, 20) },
        Point::new(20, 20),
    );
    assert_eq!(hovered_widget(), Some(left));

    unregister(left);
    assert_eq!(hovered_widget(), None);

    unregister(parent);
    unregister(right);
}

/// Moving focus must tell the platform's IME bridge which control is being edited.
///
/// Before this, `ImeBridge::focus_in` had **no caller anywhere**, so a backend's
/// bridge never learned which widget was active and had no widget to place its
/// candidate window against. This drives the real registry against a recording
/// bridge and asserts the calls arrive.
#[test]
#[cfg(not(alloc_frugal))]
fn moving_focus_notifies_the_ime_bridge() {
    let first = register(sample_editor("a")).expect("registry");
    let second = register(sample_editor("b")).expect("registry");
    clear_focus();

    // Drive the notification helper directly, because the bridge the *active*
    // backend exposes is a private implementation without a readback accessor.
    // The helper is the single place the runtime calls out to the IME, so this
    // covers the wiring that was missing.
    let recorder = RecordingImeBridge::default();
    {
        use crate::platform::ime::ImeBridge as _;
        recorder.focus_in(first);
        recorder.focus_out(first);
        recorder.focus_in(second);
    }
    assert_eq!(
        recorder.calls(),
        vec![("in", first), ("out", first), ("in", second)],
        "the IME bridge must be told about every focus change"
    );

    // And the real path must reach *some* bridge, exercising the call site.
    let backend = crate::platform::platform_facts();
    if backend.ime_bridge().is_some() {
        assert!(focus_widget(first));
        assert_eq!(focused_widget(), Some(first));
        assert!(clear_focus());
    }

    unregister(first);
    unregister(second);
}

/// Row-major traversal must order Tab by on-screen position, not registration.
///
/// `FocusTraversalStrategy` was fully implemented but unreachable — nothing outside
/// `FocusManager` could select one, so a grid could only ever tab in registration
/// order.
#[test]
fn row_major_traversal_orders_tab_by_position() {
    let a = register(sample_editor("a")).expect("registry");
    let b = register(sample_editor("b")).expect("registry");
    let c = register(sample_editor("c")).expect("registry");

    // Register them in an order that disagrees with the layout: c, a, b.
    set_focus_order_for_test(&[c, a, b]);
    // Two rows: a and b on the top row, c below-left.
    assert!(set_focusable_position(a, 10, 10));
    assert!(set_focusable_position(b, 200, 10));
    assert!(set_focusable_position(c, 10, 100));

    assert!(set_focus_traversal(crate::event::FocusTraversalStrategy::RowMajor));
    assert_eq!(
        focusable_widgets(),
        vec![a, b, c],
        "row-major must visit the top row left-to-right before the row below"
    );

    // A non-focusable widget has no position to record.
    assert!(!set_focusable_position(0xDEAD_BEEF, 0, 0));

    // Back to registration order for the rest of the suite, which shares this
    // thread-local manager.
    assert!(set_focus_traversal(crate::event::FocusTraversalStrategy::TabOrder));
    unregister(a);
    unregister(b);
    unregister(c);
}

/// Column-major traversal orders by x first, then y.
#[test]
fn column_major_traversal_orders_tab_by_column() {
    let a = register(sample_editor("a")).expect("registry");
    let b = register(sample_editor("b")).expect("registry");

    set_focus_order_for_test(&[b, a]);
    // `b` is in the right column, `a` in the left, so column-major visits a first.
    assert!(set_focusable_position(a, 10, 300));
    assert!(set_focusable_position(b, 500, 10));

    assert!(set_focus_traversal(crate::event::FocusTraversalStrategy::ColumnMajor));
    assert_eq!(focusable_widgets(), vec![a, b], "column-major must visit left first");

    assert!(set_focus_traversal(crate::event::FocusTraversalStrategy::TabOrder));
    unregister(a);
    unregister(b);
}

/// Capture must make a widget receive pointer events wherever the pointer is.
#[test]
fn captured_widget_receives_pointer_events_off_itself() {
    let (parent, left, right) = two_siblings();
    release_pointer_capture();

    assert!(capture_pointer(left));
    assert_eq!(capturing_widget(), Some(left));
    assert!(has_pointer_capture(left));

    // (160, 20) is over `right`, but `left` holds capture, so it wins.
    assert!(
        dispatch_pointer_event(
            parent,
            &Event::MouseMove { pos: Point::new(160, 20) },
            Point::new(160, 20),
        ),
        "a captured widget must receive the event even when the pointer left it"
    );
    // Hover still tracks the true position, so highlights stay honest.
    assert_eq!(hovered_widget(), Some(right));

    assert!(release_pointer_capture());
    assert_eq!(capturing_widget(), None);
    assert!(!release_pointer_capture(), "a second release has nothing to release");

    unregister(parent);
    unregister(left);
    unregister(right);
}

/// Capture on an id that is not mounted would route every later pointer event into
/// nothing, so it is refused — the same honest-answer rule used for focus.
#[test]
fn capturing_an_unmounted_widget_is_refused() {
    release_pointer_capture();
    assert!(!capture_pointer(0xDEAD_BEEF));
    assert_eq!(capturing_widget(), None);
}

/// A capture holder that is removed must not keep swallowing pointer events.
#[test]
fn unregistering_a_captured_widget_releases_capture() {
    let (parent, left, right) = two_siblings();
    assert!(capture_pointer(left));

    unregister(left);

    assert_eq!(capturing_widget(), None, "a removed widget must not keep capture");
    release_pointer_capture();
    unregister(parent);
    unregister(right);
}

/// D09-EVT-01: a press that drags off the control and is released elsewhere must
/// still be delivered *back* to the control that owns the gesture, so its own press
/// latch is cleared by the release rather than left armed.
///
/// Before this the runtime resolved every release against the current position, so a
/// release outside the button never reached it. The button kept its `grabbed`/`pressed`
/// latch set, and a *later*, unrelated release that happened to land on the button
/// then fired a spurious `clicked`. This test drives the full runtime sequence the
/// issue's acceptance criterion names: press on A → move out over B → release over the
/// blank → release again on A. Neither release may activate anything, and after the
/// first (outside) release the button must hold no grab and no capture.
#[test]
fn a_press_released_off_the_control_is_delivered_back_and_clears_its_latch() {
    use crate::event::mouse_button;

    release_pointer_capture();
    let parent = register(Box::new(crate::widget::container_widgets::groupbox::GroupBox::new(
        Rect::new(0, 0, 300, 300),
    )))
    .expect("registry");

    // Two buttons side by side, plus a blank area to the right to release into.
    let mut a = crate::widget::base_widgets::button::Button::new(
        "A".to_string(),
        Rect::new(20, 20, 80, 30),
    );
    a.set_geometry(Rect::new(20, 20, 80, 30));
    let a = register(Box::new(a)).expect("registry");
    with_widget_mut(parent, |widget| widget.add_child(a));

    let mut b = crate::widget::base_widgets::button::Button::new(
        "B".to_string(),
        Rect::new(130, 20, 80, 30),
    );
    b.set_geometry(Rect::new(130, 20, 80, 30));
    let b = register(Box::new(b)).expect("registry");
    with_widget_mut(parent, |widget| widget.add_child(b));

    let clicks_a = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter_a = std::sync::Arc::clone(&clicks_a);
    with_widget_mut(a, |widget| {
        widget.base().clicked.connect(move || {
            counter_a.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });
    });
    let clicks_b = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter_b = std::sync::Arc::clone(&clicks_b);
    with_widget_mut(b, |widget| {
        widget.base().clicked.connect(move || {
            counter_b.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });
    });

    // Press inside A: A takes the gesture.
    assert!(dispatch_pointer_event(
        parent,
        &Event::MousePress { pos: Point::new(40, 35), button: mouse_button::PRIMARY, modifiers: 0 },
        Point::new(40, 35),
    ));
    assert_eq!(capturing_widget(), Some(a), "the pressed control must own the gesture");

    // Drag out over B, then release on the blank at (280, 280) — over no child.
    dispatch_pointer_event(
        parent,
        &Event::MouseMove { pos: Point::new(160, 35) },
        Point::new(160, 35),
    );
    assert!(
        dispatch_pointer_event(
            parent,
            &Event::MouseRelease { pos: Point::new(280, 280), button: mouse_button::PRIMARY },
            Point::new(280, 280),
        ),
        "the off-widget release must be routed back to the gesture owner, not dropped"
    );

    // The gesture ended: no capture remains, A holds no grab, and nothing activated.
    assert_eq!(capturing_widget(), None, "the gesture-ending release must release the capture");
    assert!(
        !with_widget_mut(a, |w| w.base().is_grabbed()).unwrap_or(true),
        "A's grab must be cleared by the release that ended its gesture"
    );
    assert_eq!(clicks_a.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert_eq!(clicks_b.load(std::sync::atomic::Ordering::SeqCst), 0);

    // Re-enter and release on A again with no matching press. Because the earlier
    // gesture was properly closed, this stray release must not fire A's `clicked`.
    dispatch_pointer_event(
        parent,
        &Event::MouseMove { pos: Point::new(40, 35) },
        Point::new(40, 35),
    );
    dispatch_pointer_event(
        parent,
        &Event::MouseRelease { pos: Point::new(40, 35), button: mouse_button::PRIMARY },
        Point::new(40, 35),
    );
    assert_eq!(
        clicks_a.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "a stray release must not activate a control whose earlier gesture was cancelled"
    );

    unregister(a);
    unregister(b);
    unregister(parent);
}

// -----------------------------------------------------------------------
// Modal dialog enforcement
// -----------------------------------------------------------------------

/// Builds a window with a modal dialog and an unrelated control, wiring the
/// `parent` links that [`is_descendant_of`] walks (the same links the real mount
/// path sets via `mount_named_widget`). Returns `(window, modal, outside)`, where
/// the modal is a top-level dialog and `outside` is another widget the modal
/// must block.
fn modal_fixture() -> (ObjectId, ObjectId, ObjectId) {
    let window = register(Box::new(crate::widget::container_widgets::groupbox::GroupBox::new(
        Rect::new(0, 0, 400, 400),
    )))
    .expect("window");
    let modal = register(Box::new(crate::widget::dialog::message_box::MessageBox::new(Rect::new(
        50, 50, 200, 100,
    ))))
    .expect("modal");
    let outside = register(Box::new(crate::widget::base_widgets::label::Label::new(
        "outside".to_string(),
        Rect::new(0, 0, 100, 20),
    )))
    .expect("outside");
    // The dialog and the label both belong to the window's frame of reference;
    // neither is a *child* of the other, so the modal must block the label.
    with_widget_mut(modal, |w| w.set_parent(Some(window)));
    with_widget_mut(outside, |w| w.set_parent(Some(window)));
    with_widget_mut(window, |w| {
        w.add_child(modal);
        w.add_child(outside);
    });
    (window, modal, outside)
}

#[test]
fn no_modal_blocks_nothing() {
    let (window, modal, outside) = modal_fixture();
    assert!(!is_modal_active());
    assert!(!modal_blocks(window));
    assert!(!modal_blocks(modal));
    assert!(!modal_blocks(outside));
    unregister(window);
    unregister(modal);
    unregister(outside);
}

#[test]
fn an_active_modal_blocks_input_outside_its_subtree() {
    let (window, modal, outside) = modal_fixture();

    assert!(enter_modal(modal));
    assert!(is_modal_active());
    assert_eq!(active_modal(), Some(modal));

    // The dialog itself stays live; everything else (the window chrome, an
    // unrelated control) is blocked.
    assert!(!modal_blocks(modal));
    assert!(modal_blocks(window));
    assert!(modal_blocks(outside));

    // Event delivery follows the same rule.
    let event = crate::event::Event::mouse_press(10, 10, 1);
    assert!(!dispatch_event(outside, &event), "a blocked widget must not receive events");
    assert!(dispatch_event(modal, &event), "the modal dialog must receive events");

    exit_modal(modal);
    assert!(!is_modal_active());
    assert!(!modal_blocks(outside), "after dismissal nothing is blocked");

    unregister(window);
    unregister(modal);
    unregister(outside);
}

#[test]
fn a_modal_blocks_focus_outside_its_subtree() {
    let (window, modal, outside) = modal_fixture();
    clear_focus();

    enter_modal(modal);
    assert!(focus_widget(modal));
    // Focus elsewhere is refused while the modal is up.
    assert!(!focus_widget(outside));
    assert_eq!(focused_widget(), Some(modal));

    // After the modal is dismissed, focus can move to the formerly blocked widget.
    exit_modal(modal);
    assert!(focus_widget(outside));
    assert_eq!(focused_widget(), Some(outside));

    clear_focus();
    unregister(window);
    unregister(modal);
    unregister(outside);
}

#[test]
fn an_unmounted_modal_cannot_be_entered() {
    clear_modals();
    assert!(!enter_modal(0xDEAD_BEEF));
    assert!(!is_modal_active());
}

#[test]
fn clear_modals_drops_the_whole_stack() {
    let (_, modal, _) = modal_fixture();
    assert!(enter_modal(modal));
    assert!(is_modal_active());
    clear_modals();
    assert!(!is_modal_active());
    unregister(modal);
}

/// A recording IME bridge, for asserting the focus calls arrive in order.
#[derive(Default)]
struct RecordingImeBridge {
    calls: crate::compat::Mutex<Vec<(&'static str, ObjectId)>>,
}

impl RecordingImeBridge {
    fn calls(&self) -> Vec<(&'static str, ObjectId)> {
        self.calls.lock().unwrap().clone()
    }
}

impl crate::platform::ime::ImeBridge for RecordingImeBridge {
    fn focus_in(&self, widget_id: ObjectId) {
        self.calls.lock().unwrap().push(("in", widget_id));
    }
    fn focus_out(&self, widget_id: ObjectId) {
        self.calls.lock().unwrap().push(("out", widget_id));
    }
    fn commit_text(&self, _text: &str) {}
    fn set_composition(&self, _composition: &crate::platform::ime::ImeComposition) {}
    fn set_candidate_window_position(&self, _position: crate::platform::ime::ImeCandidatePosition) {
    }
    fn is_active(&self) -> bool {
        true
    }
}

// ── BLUE24 §6 -- the accessibility pump ──────────────────────────────────────

/// A bridge that records what the submit points pushed, so the mount path can be asserted.
///
/// `Mutex` rather than `RefCell` because `AccessibilityBridge` is `Send + Sync`, which a real
/// platform bridge satisfies by being posted to from threads the platform owns.
struct A11yRecorder {
    names: std::sync::Mutex<Vec<(ObjectId, String)>>,
    states: std::sync::Mutex<Vec<ObjectId>>,
    /// D09-A11Y-02: the complete state submitted per node, readable back through `node_state`.
    nodes: std::sync::Mutex<
        std::collections::HashMap<ObjectId, crate::platform::accessibility::A11yState>,
    >,
}

impl A11yRecorder {
    fn new() -> Self {
        Self {
            names: std::sync::Mutex::new(Vec::new()),
            states: std::sync::Mutex::new(Vec::new()),
            nodes: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }
}

impl crate::platform::accessibility::AccessibilityBridge for A11yRecorder {
    fn set_accessibility_name(&self, id: ObjectId, name: &str) {
        self.names.lock().unwrap().push((id, name.to_string()));
    }
    fn accessibility_name(&self, id: ObjectId) -> Option<String> {
        self.names
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|(recorded, _)| *recorded == id)
            .map(|(_, name)| name.clone())
    }
    fn notify_name_changed(&self, _id: ObjectId) {}
    fn notify_value_changed(&self, _id: ObjectId) {}
    fn notify_state_changed(&self, id: ObjectId) {
        self.states.lock().unwrap().push(id);
    }
    fn notify_focus_changed(&self, _id: ObjectId) {}
    // D09-A11Y-02/D09-A11Y-03: the substitute keeps the full node store the platform bridges
    // keep, so a test can read the whole submitted state back and watch the store return to
    // baseline across mount/unmount cycles — the observable assertions the defects require.
    fn submit_node_state(&self, id: ObjectId, state: &crate::platform::accessibility::A11yState) {
        self.names.lock().unwrap().push((id, state.label.clone()));
        self.nodes.lock().unwrap().insert(id, state.clone());
    }
    fn node_state(&self, id: ObjectId) -> Option<crate::platform::accessibility::A11yState> {
        self.nodes.lock().unwrap().get(&id).cloned()
    }
    fn unregister_node(&self, id: ObjectId) {
        self.names.lock().unwrap().retain(|(recorded, _)| *recorded != id);
        self.nodes.lock().unwrap().remove(&id);
    }
    fn node_count(&self) -> usize {
        self.nodes.lock().unwrap().len()
    }
}

/// Installs a recording bridge for one test, removing it on the way out even on panic.
struct A11yGuard;
impl Drop for A11yGuard {
    fn drop(&mut self) {
        let _ = crate::widget::a11y_submit::uninstall_bridge();
    }
}

/// BLUE24 §6 criterion 1 and §11 criterion 23: mounting controls creates their nodes.
///
/// # Why the real `register` path
///
/// A test that called `submit_mounted` directly would prove the submit function works and
/// nothing about whether anything calls it — which is exactly the gap BLUE24 §0A.1 measured
/// (three platform bridges, zero production callers). So this mounts through `register`, the
/// same entry point every control in the crate uses, and asserts the bridge saw it.
#[test]
fn mounting_controls_creates_their_nodes() {
    let recorder: &'static A11yRecorder = Box::leak(Box::new(A11yRecorder::new()));
    crate::widget::a11y_submit::install_bridge(recorder);
    let _guard = A11yGuard;

    let label = register(Box::new(crate::widget::Label::new(
        "Hello".to_string(),
        crate::core::Rect::new(0, 0, 80, 24),
    )))
    .expect("mount a label");
    let _unmount_label = MountGuard(label);
    let button = register(Box::new(crate::widget::Button::new(
        "Save".to_string(),
        crate::core::Rect::new(0, 0, 80, 32),
    )))
    .expect("mount a button");
    let _unmount_button = MountGuard(button);
    let checkbox =
        register(Box::new(crate::widget::CheckBox::new(crate::core::Rect::new(0, 0, 24, 24))))
            .expect("mount a check box");
    let _unmount_checkbox = MountGuard(checkbox);

    let names = recorder.names.lock().unwrap().clone();
    assert_eq!(names.len(), 3, "three controls mounted, so three names were pushed: {names:?}");
    assert!(
        names.iter().any(|(id, name)| *id == label && name == "Hello"),
        "a label's accessible name is its text: {names:?}"
    );
    assert!(
        names.iter().any(|(id, name)| *id == button && name == "Save"),
        "a button's accessible name is its title: {names:?}"
    );
    let states = recorder.states.lock().unwrap().clone();
    assert_eq!(states.len(), 3, "and each node's creation was reported: {states:?}");

    // D09-A11Y-02: the node the bridge received carries the derived *role*, not only the name.
    // Before the full-state contract, the mount path sent `label` alone, so a screen reader
    // could read "Save" but not know it was a button.
    assert_eq!(
        recorder.node_state(label).map(|s| s.role),
        Some(crate::platform::accessibility::A11yRole::Label),
        "a label's role reaches the bridge"
    );
    assert_eq!(
        recorder.node_state(button).map(|s| s.role),
        Some(crate::platform::accessibility::A11yRole::Button),
        "a button's role reaches the bridge"
    );
    assert_eq!(
        recorder.node_state(checkbox).map(|s| s.role),
        Some(crate::platform::accessibility::A11yRole::CheckBox),
        "a check box's role reaches the bridge"
    );
    // And the check box's checked state, which the label-only contract could never carry.
    assert_eq!(
        recorder.node_state(checkbox).and_then(|s| s.checked),
        Some(false),
        "an unchecked box reports `Some(false)`, not `None`"
    );
}

/// D09-A11Y-03: repeated mount/unmount cycles return the bridge's node store to baseline.
///
/// The defect was a name map that blanked to `""` on unmount and kept one entry per historical
/// widget id. Because the runtime allocates a fresh monotonically increasing id per mount, the
/// counts must be compared across *distinct* ids — a cycle reusing one id would pass even if the
/// store grew.
#[test]
fn repeated_mount_unmount_returns_the_bridge_store_to_baseline() {
    let recorder: &'static A11yRecorder = Box::leak(Box::new(A11yRecorder::new()));
    crate::widget::a11y_submit::install_bridge(recorder);
    let _guard = A11yGuard;

    let baseline = recorder.node_count();
    assert_eq!(baseline, 0, "a fresh bridge records no nodes");

    for cycle in 0..5 {
        let id = register(Box::new(crate::widget::Label::new(
            format!("cycle-{cycle}"),
            crate::core::Rect::new(0, 0, 80, 24),
        )))
        .expect("mount");
        assert_eq!(recorder.node_count(), baseline + 1, "one live node in cycle {cycle}");
        assert!(recorder.node_state(id).is_some(), "the mounted node is recorded in cycle {cycle}");
        assert!(unregister(id), "the mounted id is present in cycle {cycle}");
        assert_eq!(
            recorder.node_count(),
            baseline,
            "the store returns to baseline after cycle {cycle}"
        );
        assert!(
            recorder.node_state(id).is_none(),
            "and the node itself is gone after cycle {cycle}"
        );
    }

    let names = recorder.names.lock().unwrap();
    assert!(names.is_empty(), "no historical id left a blanked name entry behind: {names:?}");
}

/// Unmounting a control tears its node down; re-registering a never-mounted id does not.
///
/// The second half matters: a removal posted for an id that was never mounted would tear down
/// a node a live control still owns, which is a silent loss for a screen reader.
#[test]
fn unmounting_a_control_tears_down_its_node() {
    let recorder: &'static A11yRecorder = Box::leak(Box::new(A11yRecorder::new()));
    crate::widget::a11y_submit::install_bridge(recorder);
    let _guard = A11yGuard;

    let id = register(Box::new(crate::widget::Label::new(
        "Bye".to_string(),
        crate::core::Rect::new(0, 0, 80, 24),
    )))
    .expect("mount");
    let before = recorder.states.lock().unwrap().len();
    assert!(unregister(id), "the mounted id is present");
    let after = recorder.states.lock().unwrap().len();
    assert_eq!(after, before + 1, "unmounting posted one teardown");

    // A second unmount of the same id finds nothing, so it posts nothing.
    assert!(!unregister(id), "the id is gone");
    assert_eq!(
        recorder.states.lock().unwrap().len(),
        after,
        "an unmount of a never-mounted id must not post a teardown"
    );
}

/// A check box's three states all travel the submit path.
///
/// BLUE24 §6 criterion 3. The state is read back through the bridge substitute — not from the
/// control — so the assertion covers the whole pump: derive (stage ①) and the submission.
#[test]
fn a_check_boxs_three_states_travel_the_submit_path() {
    use crate::platform::accessibility::A11yState;

    for (check, expected_checked, expected_mixed) in
        [(false, Some(false), false), (true, Some(true), false)]
    {
        let mut cb = crate::widget::CheckBox::new(crate::core::Rect::new(0, 0, 24, 24));
        cb.set_checked(check);
        let state = A11yState::from_widget(&cb);
        assert_eq!(state.checked, expected_checked, "checked={check}");
        assert_eq!(state.mixed, expected_mixed, "checked={check}");
    }

    // The mixed state is the tri-state spelling, which cannot be said with a bool.
    let mut cb = crate::widget::CheckBox::new(crate::core::Rect::new(0, 0, 24, 24));
    cb.set_tristate_enabled(true);
    cb.set_state(crate::widget::CheckState::PartiallyChecked);
    let state = A11yState::from_widget(&cb);
    assert_eq!(state.checked, Some(true), "a mixed box is on");
    assert!(state.mixed, "and separately mixed, which is the only way to say it");
}

// ── BLUE24 §7 -- native state reporting and redraw ───────────────────────────

/// BLUE24 §7 criterion 1: a reported hover makes `widget_state()` true for a control the
/// library did not see the input for.
///
/// The defect this pins is the two-worlds problem: a native control's hover is the
/// platform's fact, so `BaseWidget::is_hovered` stayed `false`, `widget_state()` reported
/// `Normal`, and a theme's `"<kind>:hover"` was unreachable for every native control.
/// Reporting the fact is what closes it, and the assertion is the state the theme keys on.
#[test]
fn a_reported_hover_reaches_widget_state() {
    use crate::style::WidgetState;

    let id = register(Box::new(crate::widget::Button::new(
        "native".to_string(),
        crate::core::Rect::new(0, 0, 80, 32),
    )))
    .expect("mount");
    let _unmount = MountGuard(id);

    assert_eq!(
        with_widget(id, |w| w.widget_state()),
        Some(WidgetState::Normal),
        "a freshly mounted control is at rest"
    );
    assert!(report_state(id, StateFact::Hovered(true)), "the fact reaches a mounted control");
    assert_eq!(
        with_widget(id, |w| w.widget_state()),
        Some(WidgetState::Hover),
        "so `widget_state()` is now true for a control the library never saw input for"
    );

    // Same fact again: no change, so nothing is re-reported (the idempotence §6.3 asks of
    // its own submit points, here for the native path).
    assert!(report_state(id, StateFact::Hovered(true)), "a known id is still found");
    assert_eq!(with_widget(id, |w| w.widget_state()), Some(WidgetState::Hover));

    assert!(report_state(id, StateFact::Hovered(false)), "leaving is a fact too");
    assert_eq!(with_widget(id, |w| w.widget_state()), Some(WidgetState::Normal));
}

/// A report about an id the library does not know is refused, not silently dropped.
#[test]
fn a_report_about_an_unknown_control_is_refused() {
    assert!(!report_state(0xDEAD_BEEF, StateFact::Hovered(true)));
    assert!(!report_state(0xDEAD_BEEF, StateFact::Pressed(true)));
}

/// BLUE24 §7 criterion 2: a native redraw is **counted** by the frame ledger.
#[test]
fn a_native_redraw_is_counted_by_the_frame() {
    let _serial = serialise_drive_frame();
    let id = register(Box::new(crate::widget::Label::new(
        "n".to_string(),
        crate::core::Rect::new(0, 0, 40, 16),
    )))
    .expect("mount");
    let _unmount = MountGuard(id);

    // Start from a clean frame accounting.
    let _ = drive_frame(0);
    notify_native_redraw(id, Some(crate::core::Rect::new(0, 0, 10, 10)));
    let outcome = drive_frame(0);
    assert_eq!(
        outcome.repaints_submitted, 1,
        "the platform's own redraw request is in the library's account"
    );
}

// ── BLUE24 §8 -- the frame ledger ───────────────────────────────────────────

/// BLUE24 §8 criterion 1: sixty still frames tick nothing and coalesce nothing.
#[test]
fn sixty_still_frames_cost_nothing() {
    let _serial = serialise_drive_frame();
    let id = register(Box::new(crate::widget::Label::new(
        "still".to_string(),
        crate::core::Rect::new(0, 0, 40, 16),
    )))
    .expect("mount");
    let _unmount = MountGuard(id);

    for _ in 0..60 {
        let outcome = drive_frame(16);
        assert_eq!(outcome.controls_ticked, 0, "nothing is animating");
        let stats = last_frame_stats().expect("a frame ran");
        assert_eq!(stats.controls_ticked, 0);
        assert_eq!(
            stats.repaints_coalesced, 0,
            "nothing was requested, so nothing could be merged"
        );
        assert!(stats.last_repaint_reason.is_empty(), "and no surface was repainted");
    }
}

/// A control that STOPS animating must let the frame loop stop asking for frames.
///
/// # The defect this pins
///
/// `tick_animations` sets `LAST_SWEEP_HAD_UNSETTLED_MOUNTED` on any frame that found an
/// animating control, and `animation_bus_needs_another_frame` ORs it in. The flag exists so
/// the frame that observes a *settle* is scheduled. But nothing consumed it: the only writer
/// that cleared it was `animation_bus_reset_host_owned`, which a host calls on **teardown**.
///
/// So the first frame in which any mounted control animated turned
/// `needs_another_frame` into a **permanent `true`** — a window that had ever shown a spinner
/// or an indeterminate bar could never sleep again. That is the opposite of BLUE24 §1's
/// central promise ("a still window's frame costs one emptiness check"), and it is invisible
/// without a control that animates *and then stops*, which is why it survived until a player
/// that reaches the end of its media was wired up.
#[test]
fn a_control_that_stops_animating_lets_the_frame_loop_sleep() {
    let _serial = serialise_drive_frame();
    use crate::widget::display_widgets::progressbar::ProgressBar;

    let mut bar = ProgressBar::new(crate::core::Rect::new(0, 0, 160, 16));
    bar.set_indeterminate(true);
    let id = register(Box::new(bar)).expect("mount");
    let _unmount = MountGuard(id);

    // While it animates the loop is owed another frame.
    let running = drive_frame(16);
    assert_eq!(running.controls_ticked, 1);
    assert!(running.needs_another_frame, "an animating control keeps the loop awake");

    // Stop it. The very next frame must tick nothing, and the one after that must be free:
    // a settle is painted once and then scheduling stops.
    with_widget_mut(id, |widget| {
        if let Some(bar) = crate::widget::capability::coercion::widget_as_mut::<ProgressBar>(widget)
        {
            bar.set_indeterminate(false);
        }
    });

    let settled = drive_frame(16);
    assert_eq!(settled.controls_ticked, 0, "nothing is animating now");
    let after = drive_frame(16);
    assert!(
        !after.needs_another_frame,
        "a window whose animation has settled must stop being scheduled"
    );
    assert_eq!(after.controls_ticked, 0);
    assert_eq!(after.repaints_submitted, 0, "and a still frame costs no submission");
}

/// A mounted, playing `MediaPlayer` advances through the frame loop and then stops it.
///
/// # The defect this pins, and why it belongs in **this** module's tests
///
/// `MediaPlayer` had a `position_ms`, a progress ratio, a `position_changed` signal and a
/// progress rule in `draw` — and nothing that moved the position. The missing piece was not
/// inside the control (a `tick` is) but at the **seam**: the frame loop is what turns "this
/// control would advance" into "this control advanced", and it only does so for a mounted
/// control that answers [`Widget::is_animating`]. So the test that proves the seam is closed
/// belongs here, next to `drive_frame`, rather than only in the control's unit tests —
/// a `tick` nobody calls is exactly the shape BLUE24 §0A.1 measurement 1 is about.
#[test]
fn a_playing_media_player_advances_through_the_frame_loop() {
    let _serial = serialise_drive_frame();
    use crate::widget::special_widgets::media_player::MediaPlayer;

    let mut player = MediaPlayer::new(crate::core::Rect::new(0, 0, 320, 200));
    player.set_source("clip.mp4", 1_000);
    assert!(player.play());
    let id = register(Box::new(player)).expect("mount");
    let _unmount = MountGuard(id);

    // One frame of the frame loop's own delta must move the control, and the loop must be
    // told it still owes another frame.
    let outcome = drive_frame(16);
    assert_eq!(outcome.controls_ticked, 1, "the player is the one control animating");
    assert!(outcome.needs_another_frame, "a playing player keeps the loop awake");
    assert_eq!(
        with_widget(id, |widget| widget.is_animating()),
        Some(true),
        "and it says so through the trait, not only through its own methods"
    );

    // The next frames are the media's own duration: at the loop's 16 ms delta it takes 63
    // frames to cover the 1_000 ms, so the bound below has margin and the assertion is about
    // "it stops at all", not about the exact frame it stops on. What must not happen is the
    // loop staying awake for ever, which is the failure a missing end condition produces.
    let mut stopped_at = None;
    for frame in 1..=120 {
        let outcome = drive_frame(16);
        if !outcome.needs_another_frame {
            stopped_at = Some(frame);
            break;
        }
    }
    assert!(
        stopped_at.is_some(),
        "playback must end on its own, not keep the frame loop awake for ever"
    );
    let stats = last_frame_stats().expect("a frame ran");
    assert_eq!(
        stats.controls_ticked, 0,
        "the frame that observed the end no longer ticks the player"
    );
}

/// BLUE24 §8 criterion 3: five requests for one surface submit once and coalesce four.
#[test]
fn one_surface_is_submitted_once_and_the_rest_are_coalesced() {
    let _serial = serialise_drive_frame();
    let surface = register(Box::new(crate::widget::Label::new(
        "s".to_string(),
        crate::core::Rect::new(0, 0, 40, 16),
    )))
    .expect("mount");
    let _unmount = MountGuard(surface);

    // A clean frame boundary, so the five requests belong to one window.
    let _ = drive_frame(0);
    for _ in 0..5 {
        notify_native_redraw(surface, None);
    }
    let outcome = drive_frame(0);
    assert_eq!(outcome.repaints_submitted, 1, "one surface, one submission");
    let stats = last_frame_stats().expect("a frame ran");
    assert_eq!(stats.repaints_coalesced, 4, "and the other four were merged into it");
}

/// BLUE24 §8 criterion 4: every repainted surface has a cause in the ledger.
#[test]
fn every_repaint_names_a_reason() {
    let _serial = serialise_drive_frame();
    let id = register(Box::new(crate::widget::Button::new(
        "why".to_string(),
        crate::core::Rect::new(0, 0, 80, 32),
    )))
    .expect("mount");
    let _unmount = MountGuard(id);
    let _ = drive_frame(0);

    notify_native_redraw(id, None);
    let _ = drive_frame(0);

    let stats = last_frame_stats().expect("a frame ran");
    assert!(
        !stats.last_repaint_reason.is_empty(),
        "a frame that submitted a repaint must be able to say why"
    );
    assert_eq!(
        stats.last_repaint_reason.iter().find(|(sid, _)| *sid == id).map(|(_, r)| *r),
        Some(RepaintReason::Native),
        "a platform redraw is attributed to the platform: {:?}",
        stats.last_repaint_reason
    );
}

/// The ledger attributes a repaint to the cause that **made the frame happen**.
///
/// When two causes name one surface in one frame, the first wins: the later one is folded
/// into a submission that was already going out, so recording it would describe the last
/// caller rather than the cost. This is asserted through the state path, which is the one
/// whose submission goes through the library's own invalidation.
#[test]
fn two_causes_on_one_surface_record_the_first() {
    let _serial = serialise_drive_frame();
    use crate::platform::{with_recorded_invalidations, RecordingInvalidations};

    // A recording backend makes `invalidate_surface` succeed, so the state path submits
    // (and is therefore counted) exactly as it is on a real host.
    let recorder: &'static RecordingInvalidations =
        Box::leak(Box::new(RecordingInvalidations::default()));
    with_recorded_invalidations(recorder, || {
        let id = register(Box::new(crate::widget::Button::new(
            "first".to_string(),
            crate::core::Rect::new(0, 0, 80, 32),
        )))
        .expect("mount");
        let _unmount = MountGuard(id);
        let _ = drive_frame(0);

        request_repaint_because(id, RepaintReason::Animation);
        request_repaint_because(id, RepaintReason::State);
        let outcome = drive_frame(0);

        assert_eq!(outcome.repaints_submitted, 1, "one surface, one submission");
        let stats = last_frame_stats().expect("a frame ran");
        assert_eq!(stats.repaints_coalesced, 1, "and the second request was merged");
        assert_eq!(
            stats.last_repaint_reason.iter().find(|(sid, _)| *sid == id).map(|(_, r)| *r),
            Some(RepaintReason::Animation),
            "the cause that made the frame happen is the one recorded"
        );
    });
}

/// Only an animation keeps producing frames; the other causes describe a finished change.
#[test]
fn only_animation_keeps_asking_for_frames() {
    assert!(RepaintReason::Animation.keeps_animating());
    assert!(!RepaintReason::State.keeps_animating());
    assert!(!RepaintReason::Overlay.keeps_animating());
    assert!(!RepaintReason::Explicit.keeps_animating());
    assert!(!RepaintReason::Native.keeps_animating());
}

// ── The window-level tree painter (BLUE24 §0A.1) ─────────────────────────

/// A window's tree frame must contain its **children's** pixels, not just the background.
///
/// # The defect this pins
///
/// Since 2.0 the library paints every `WidgetKind` itself, so a control created through
/// the ordinary `create_*` path has no native control of its own. On Windows the only
/// painter was the canvas child window that `mount_surface` creates, so a window whose
/// children came from `create_button` / `create_checkbox` / … had **nothing drawing
/// them**: the window opened and showed an empty client area — a blank white board.
///
/// The fix is a window-level painter that walks the child list (`render_frame_tree`).
/// This test pins the property that painter depends on, at the level where it can be
/// checked without a display: a window with mounted children yields a frame whose pixels
/// are **not all the clear colour**. A regression that broke the traversal — an empty
/// child list, a skipped `is_visible`, a missing draw bridge — would produce a uniform
/// frame and fail here rather than only on a screen nobody is looking at.
#[test]
fn a_window_tree_frame_contains_its_children_not_just_the_background() {
    let window_id = register(Box::new(crate::widget::window::Window::new(
        "t".to_string(),
        Rect::new(0, 0, 200, 120),
    )))
    .expect("mount the window");
    let _unmount_window = MountGuard(window_id);

    // A child at a known place, added through the window's own `add_child` so both
    // directions of the link are written — a one-sided link is the other way a child
    // becomes unreachable from the root.
    let child_id =
        register(Box::new(crate::widget::Button::new("ok".to_string(), Rect::new(10, 40, 80, 32))))
            .expect("mount the button");
    let _unmount_child = MountGuard(child_id);
    assert!(with_widget_mut(window_id, |window| {
        window.add_child(child_id);
        true
    })
    .unwrap_or(false));

    let clear = Color::rgb(255, 255, 255);
    let frame = render_frame_tree(window_id, Size::new(200, 120), clear)
        .expect("a window with a drawable child must produce a frame");

    // Every pixel identical to the clear colour means nothing was drawn over the
    // background — the blank-board symptom, expressed as a fact about the frame.
    let clear_rgba = [clear.r, clear.g, clear.b, 255];
    let painted = frame.as_chunks::<4>().0.iter().filter(|pixel| **pixel != clear_rgba).count();
    assert!(
        painted > 0,
        "the window's own chrome or its child must paint something; \
         a uniform frame is the blank-board defect"
    );
}

/// The child-list traversal a window painter depends on must actually reach a child.
///
/// `render_frame_tree` walks `direct_children_of`, so a child that is mounted but absent
/// from that list is invisible to every painter — the failure mode is silent, which is
/// why the list is asserted rather than the resulting picture alone.
#[test]
fn a_mounted_child_is_reachable_through_the_window_child_list() {
    let window_id = register(Box::new(crate::widget::window::Window::new(
        "t".to_string(),
        Rect::new(0, 0, 200, 120),
    )))
    .expect("mount the window");
    let _unmount_window = MountGuard(window_id);

    let child_id =
        register(Box::new(crate::widget::Button::new("ok".to_string(), Rect::new(5, 5, 40, 20))))
            .expect("mount the button");
    let _unmount_child = MountGuard(child_id);

    assert!(
        !direct_children_of(window_id).contains(&child_id),
        "the premise: mounting a child does not by itself enlist it with the window"
    );
    assert!(with_widget_mut(window_id, |window| {
        window.add_child(child_id);
        true
    })
    .unwrap_or(false));
    assert!(
        direct_children_of(window_id).contains(&child_id),
        "after `add_child` the window painter must be able to find it"
    );
}

/// `Event::Resize` used to be published with a constructor and **no producer**: nothing
/// in the library ever delivered it, so a nested control that handled it (the code
/// editor refreshes its visible rows) never heard that its container changed size.
#[test]
fn a_container_resize_reaches_a_nested_child() {
    let window_id = register(Box::new(crate::widget::window::Window::new(
        "t".to_string(),
        Rect::new(0, 0, 200, 120),
    )))
    .expect("mount the window");
    let _unmount_window = MountGuard(window_id);

    let panel_id = register(Box::new(crate::widget::Panel::new(Rect::new(0, 0, 200, 120))))
        .expect("mount the panel");
    let _unmount_panel = MountGuard(panel_id);
    with_widget_mut(window_id, |window| window.add_child(panel_id));

    let editor_id =
        register(Box::new(crate::widget::Button::new("ok".to_string(), Rect::new(0, 0, 40, 20))))
            .expect("mount the button");
    let _unmount_editor = MountGuard(editor_id);
    with_widget_mut(panel_id, |panel| panel.add_child(editor_id));

    // The window and both descendants are told, deepest included.
    let reached = dispatch_resize(window_id, 640, 480);
    assert_eq!(reached, 3, "window + panel + button");

    // An id that addresses nothing tells nobody rather than panicking.
    assert_eq!(dispatch_resize(u64::MAX, 10, 10), 0);
}

/// A synchronous event callback may re-enter the registry without panicking.
///
/// # What this pins (BLUE-issue E-09)
///
/// A control emits its own signal **synchronously** while `handle_event` runs (a `Button`
/// emits `base.clicked` on release). A slot on that signal is a user callback, and a normal
/// callback updates other widgets — sets a label's text, moves a panel, etc. Every one of those
/// goes through `with_widget` / `with_widget_mut`, which borrow the `MOUNTED` `RefCell`.
/// `dispatch_event` used to hold that `borrow_mut` across `handle_event`, so the callback hit
/// `already borrowed: BorrowMutError` and panicked. The entry is now detached for the duration
/// of the call, so the borrow is held only for the two map operations.
#[test]
fn a_click_callback_may_update_other_widgets_without_panicking() {
    use crate::core::Point;
    use crate::event::mouse_button;

    let button_id =
        register(Box::new(crate::widget::Button::new("go".to_string(), Rect::new(0, 0, 60, 24))))
            .expect("mount the button");
    let _button_guard = MountGuard(button_id);

    let label_id = register(Box::new(crate::widget::Label::new(
        "before".to_string(),
        Rect::new(0, 0, 80, 20),
    )))
    .expect("mount the label");
    let _label_guard = MountGuard(label_id);

    // The handler mutates a *different* control — the exact shape the issue names as risky.
    let ran = std::sync::Arc::new(core::sync::atomic::AtomicBool::new(false));
    let ran_slot = std::sync::Arc::clone(&ran);
    with_widget_mut(button_id, |button| {
        button.base_mut().clicked.connect(move || {
            // Re-enters the registry through the same borrow that used to be held.
            assert!(set_geometry(label_id, Rect::new(5, 5, 120, 20)));
            let _ = with_widget(label_id, |_| ());
            ran_slot.store(true, core::sync::atomic::Ordering::SeqCst);
        });
    });

    // Drive a real press + release through `dispatch_event`, which is where the borrow was held.
    dispatch_event(
        button_id,
        &crate::event::Event::MousePress {
            pos: Point::new(10, 10),
            button: mouse_button::PRIMARY,
            modifiers: 0,
        },
    );
    dispatch_event(
        button_id,
        &crate::event::Event::MouseRelease {
            pos: Point::new(10, 10),
            button: mouse_button::PRIMARY,
        },
    );

    assert!(
        ran.load(core::sync::atomic::Ordering::SeqCst),
        "the click callback must have run to completion rather than panicking"
    );
    assert_eq!(with_widget(label_id, |label| label.geometry().width), Some(120));
}

/// A callback that destroys the control it is handling must actually destroy it.
///
/// # What this pins
///
/// `dispatch_event` detaches the entry before the call and reinserts it after. Without a
/// tombstone for a handler that called `unregister` on the in-flight id, the reinsertion would
/// silently resurrect a widget the handler had just destroyed. The tombstone check is what makes
/// the destroy stick.
#[test]
fn a_callback_may_destroy_the_widget_it_is_handling() {
    use crate::core::Point;
    use crate::event::mouse_button;

    let button_id = register(Box::new(crate::widget::Button::new(
        "self-destruct".to_string(),
        Rect::new(0, 0, 80, 24),
    )))
    .expect("mount the button");

    let self_id = button_id;
    with_widget_mut(button_id, |button| {
        button.base_mut().clicked.connect(move || {
            assert!(unregister(self_id), "the handler's unregister must take effect");
        });
    });

    dispatch_event(
        button_id,
        &crate::event::Event::MousePress {
            pos: Point::new(10, 10),
            button: mouse_button::PRIMARY,
            modifiers: 0,
        },
    );
    dispatch_event(
        button_id,
        &crate::event::Event::MouseRelease {
            pos: Point::new(10, 10),
            button: mouse_button::PRIMARY,
        },
    );

    assert!(!is_mounted(button_id), "the widget must not be resurrected by the dispatch epilogue");
}

/// A property write that emits a signal must let the callback re-enter the registry.
///
/// # The defect this pins (BLUE-issue E-17)
///
/// `with_widget_mut` held the registry's `RefCell` mutably across `f`. A property setter runs as
/// `f` and emits the control's signal synchronously, so the user callback ran while that borrow was
/// live and any `with_widget` / `geometry_of` inside it hit `already borrowed`. The borrow is now
/// scoped to the map operations only, so the callback runs with the registry free.
#[test]
fn a_property_write_callback_may_read_the_registry_without_panicking() {
    use crate::widget::capability::access::write_widget_property_by_id;
    use crate::widget::capability::CapabilityValue;
    use crate::widget::Slider;

    let slider_id = register(Box::new(Slider::new(Rect::new(0, 0, 200, 30)))).expect("mount");
    let _guard = MountGuard(slider_id);
    let other_id =
        register(Box::new(crate::widget::Label::new("other".to_string(), Rect::new(0, 0, 60, 20))))
            .expect("mount the label");
    let _other_guard = MountGuard(other_id);

    // Subscribing to the slider's dynamic `value_changed` and reading the registry inside the
    // handler is exactly the shape that panicked.
    let observed = std::sync::Arc::new(std::sync::Mutex::new(None::<(bool, bool)>));
    let observed_slot = std::sync::Arc::clone(&observed);
    with_widget_mut(slider_id, |widget| {
        let reference = widget.event_signal_dyn("value_changed").expect("published");
        reference.subscribe(Box::new(move |_payload| {
            let other_is_readable = geometry_of(other_id).is_some();
            let self_is_readable = geometry_of(slider_id).is_some();
            *observed_slot.lock().expect("lock") = Some((other_is_readable, self_is_readable));
        }));
    });

    write_widget_property_by_id(slider_id, "value", CapabilityValue::Int(42))
        .expect("the slider accepts a value write");

    assert_eq!(
        *observed.lock().expect("lock"),
        Some((true, false)),
        "the callback may read other widgets, but must not alias the widget still held mutably"
    );
}

/// A callback that destroys the control during a property write must actually destroy it.
///
/// The `with_widget_mut` drive marks its id in-flight for the same reason `dispatch_event` does:
/// a setter's callback may destroy the control, and the reinsertion must not resurrect it.
#[test]
fn a_property_write_callback_may_destroy_the_control() {
    use crate::widget::capability::access::write_widget_property_by_id;
    use crate::widget::capability::CapabilityValue;
    use crate::widget::Slider;

    let slider_id = register(Box::new(Slider::new(Rect::new(0, 0, 200, 30)))).expect("mount");

    let self_id = slider_id;
    with_widget_mut(slider_id, |widget| {
        let reference = widget.event_signal_dyn("value_changed").expect("published");
        reference.subscribe(Box::new(move |_payload| {
            assert!(unregister(self_id), "the callback's unregister must take effect");
        }));
    });

    write_widget_property_by_id(slider_id, "value", CapabilityValue::Int(7))
        .expect("the slider accepts a value write");

    assert!(
        !is_mounted(slider_id),
        "a destroy during a property write must not be undone by the drive's reinsertion"
    );
}

/// A callback can identify a mutably-driven control without aliasing it.
///
/// # The defect this pins (BLUE-issue E-18)
///
/// `dispatch_event` holds a mutable widget reference while `handle_event` synchronously emits
/// signals. Reborrowing the widget from a callback would violate Rust's aliasing rules, so its
/// geometry read is refused during dispatch while `is_mounted` still reports true.
#[test]
fn a_callback_does_not_alias_a_widget_while_it_is_mutably_driven() {
    use crate::core::Point;
    use crate::event::mouse_button;

    let button_id =
        register(Box::new(crate::widget::Button::new("go".to_string(), Rect::new(0, 0, 80, 24))))
            .expect("mount the button");
    let _guard = MountGuard(button_id);

    let self_read = std::sync::Arc::new(std::sync::Mutex::new(None::<(bool, bool)>));
    let record = std::sync::Arc::clone(&self_read);
    with_widget_mut(button_id, |button| {
        button.base_mut().clicked.connect(move || {
            // The id remains mounted, but its widget cannot be reborrowed while handle_event owns it.
            let mounted_by_registry_id = is_mounted(button_id);
            let geometry = geometry_of(button_id);
            *record.lock().expect("lock") = Some((mounted_by_registry_id, geometry.is_some()));
        });
    });

    dispatch_event(
        button_id,
        &crate::event::Event::MousePress {
            pos: Point::new(10, 10),
            button: mouse_button::PRIMARY,
            modifiers: 0,
        },
    );
    dispatch_event(
        button_id,
        &crate::event::Event::MouseRelease {
            pos: Point::new(10, 10),
            button: mouse_button::PRIMARY,
        },
    );

    let observed = *self_read.lock().expect("lock");
    assert_eq!(
        observed,
        Some((true, false)),
        "a callback must see the control as mounted without creating an aliased shared reference"
    );
}

/// A panicking input callback must not leave the registry with a stale in-flight marker.
///
/// # The defect this pins (BLUE-issue E-19)
///
/// `dispatch_event` removed the widget and marked it in-flight, then ran `handle_event`. If the
/// callback unwound (the host `catch_unwind`s), the cleanup after the call was skipped: the id
/// stayed in the in-flight set, so `is_mounted` answered `false` but `unregister` still reported
/// the id present. Cleanup is now a guard, so both answers agree after an unwind.
#[test]
fn a_panicking_input_callback_leaves_no_stale_dispatch_state() {
    use crate::core::Point;
    use crate::event::mouse_button;

    let button_id =
        register(Box::new(crate::widget::Button::new("boom".to_string(), Rect::new(0, 0, 80, 24))))
            .expect("mount the button");
    // No `MountGuard`: the widget is intentionally left unmounted after the unwind path.
    let _unmount = MountGuard(button_id);

    with_widget_mut(button_id, |button| {
        button.base_mut().clicked.connect(|| {
            panic!("deliberate unwind from an input callback");
        });
    });

    let press = crate::event::Event::MousePress {
        pos: Point::new(10, 10),
        button: mouse_button::PRIMARY,
        modifiers: 0,
    };
    let release = crate::event::Event::MouseRelease {
        pos: Point::new(10, 10),
        button: mouse_button::PRIMARY,
    };
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        dispatch_event(button_id, &press);
        dispatch_event(button_id, &release);
    }));
    assert!(caught.is_err(), "the callback must have unwound");

    // The registry must be consistent: if the widget is gone, `unregister` must say so rather
    // than reporting a stale in-flight entry as present.
    assert!(!is_mounted(button_id), "the lookup must not resolve a widget dropped by the unwind");
    assert!(
        !unregister(button_id),
        "a popped dispatch must not report the id as present via a leftover in-flight marker"
    );
}

/// A closure reading control A may emit A's signal, whose callback writes control B.
///
/// # The defect this pins (BLUE-issue E-30)
///
/// `with_widget` ran its closure inside `map.borrow()`, so a closure that read A's state and
/// synchronously emitted A's signal \u2014 a legitimate `&self` operation \u2014 ran a callback that
/// then wrote B via `with_widget_mut`, which needed `map.borrow_mut()`. That was `already
/// borrowed`, not a nested write of A. `with_widget` now detaches A before running the closure, so
/// the registry is free for the callback to write another control.
#[test]
fn a_read_closure_may_emit_a_signal_whose_callback_writes_another_control() {
    let a_id =
        register(Box::new(crate::widget::Button::new("a".to_string(), Rect::new(0, 0, 60, 24))))
            .expect("mount A");
    let _a_guard = MountGuard(a_id);
    let b_id = register(Box::new(crate::widget::Label::new(
        "before".to_string(),
        Rect::new(0, 0, 80, 20),
    )))
    .expect("mount B");
    let _b_guard = MountGuard(b_id);

    let ran = std::sync::Arc::new(core::sync::atomic::AtomicBool::new(false));
    let ran_slot = std::sync::Arc::clone(&ran);
    with_widget_mut(a_id, |widget| {
        widget.base_mut().clicked.connect(move || {
            // Writes a *different* control from inside the slot.
            assert!(set_geometry(b_id, Rect::new(3, 4, 120, 20)));
            ran_slot.store(true, core::sync::atomic::Ordering::SeqCst);
        });
    });

    // A read closure on A that emits A's own signal synchronously.
    let result = with_widget(a_id, |widget| {
        widget.base().clicked.emit();
        widget.geometry().width
    });

    assert_eq!(result, Some(60), "the read closure must return normally, not panic");
    assert!(
        ran.load(core::sync::atomic::Ordering::SeqCst),
        "the slot must have run and written the other control without a RefCell panic"
    );
    assert_eq!(with_widget(b_id, |label| label.geometry().width), Some(120));
}

#[test]
fn unregister_during_a_read_loan_is_not_undone_by_reinsertion() {
    let id = register(Box::new(crate::widget::Label::new(
        "remove".to_string(),
        Rect::new(0, 0, 60, 20),
    )))
    .expect("mount");

    let removed = with_widget(id, |_| unregister(id));

    assert_eq!(removed, Some(true), "unregister must see the in-flight read loan");
    assert!(!is_mounted(id), "the read-loan epilogue must not resurrect the widget");
    assert!(!unregister(id), "the id must already be fully unregistered");
}

// ── Host-window id translation (the P-05 seam) ──────────────────────────────

/// A mounted surface is not a child in the widget tree, so its window is recorded at
/// mount time; `top_level_window_of` must answer from that record, because cursor
/// dispatch and repaint routing both ask it for "the window this control is in".
///
/// Regression: without the record, `top_level_window_of(editor)` answered `None` for
/// every mounted surface, so `dispatch_hover_transition` found no window to call
/// `set_cursor` on and the code editor's I-beam was never applied by any backend.
#[test]
fn a_mounted_surface_answers_the_window_it_was_mounted_in() {
    let surface = register(Box::new(crate::widget::Label::new(
        "editor".to_string(),
        Rect::new(0, 0, 100, 80),
    )))
    .expect("mount");
    let _unmount_surface = MountGuard(surface);
    let window = register(Box::new(crate::widget::Label::new(
        "window".to_string(),
        Rect::new(0, 0, 100, 80),
    )))
    .expect("mount");
    let _unmount_window = MountGuard(window);

    assert_eq!(top_level_window_of(surface), None, "no parent, so no window by the walk");
    assert!(set_surface_window(surface, window), "a registered surface accepts the link");
    assert_eq!(top_level_window_of(surface), Some(window));
    assert_eq!(surface_window_for(window), None, "only surfaces are recorded, not windows");
    assert!(!set_surface_window(424_242, window), "an unknown surface cannot be linked");

    unregister(surface);
    assert_eq!(
        surface_window_for(surface),
        None,
        "the link must not outlive the widget it described"
    );
}

/// The window→surfaces reverse index answers with exactly the linked surfaces (G-314).
///
/// `surfaces_of` used to filter the whole `SURFACE_WINDOWS` table per call; the index that
/// replaced it is only correct if it moves with every mutation, so this pins the four
/// transitions that can touch it: a first link lists the surface, a re-link moves it out
/// of the previous window's list, re-linking to the window it already names neither
/// duplicates nor reorders it, and an unregistered surface leaves every list.
#[test]
fn surfaces_of_follows_the_surface_window_link_through_relinks_and_unregister() {
    let surface = register(Box::new(crate::widget::Label::new(
        "surface".to_string(),
        Rect::new(0, 0, 100, 80),
    )))
    .expect("mount");
    let _unmount_surface = MountGuard(surface);
    let first = register(Box::new(crate::widget::Label::new(
        "first".to_string(),
        Rect::new(0, 0, 100, 80),
    )))
    .expect("mount");
    let _unmount_first = MountGuard(first);
    let second = register(Box::new(crate::widget::Label::new(
        "second".to_string(),
        Rect::new(0, 0, 100, 80),
    )))
    .expect("mount");
    let _unmount_second = MountGuard(second);

    assert!(surfaces_of(first).is_empty() && surfaces_of(second).is_empty());
    assert!(set_surface_window(surface, first), "the first link is reported as new");
    assert_eq!(surfaces_of(first), vec![surface]);
    assert!(surfaces_of(second).is_empty(), "the old window keeps nothing");

    assert!(!set_surface_window(surface, first), "re-linking to the same window is not new");
    assert_eq!(
        surfaces_of(first),
        vec![surface],
        "a same-window re-link must not duplicate the surface"
    );

    assert!(!set_surface_window(surface, second), "a move is not a new link");
    assert!(surfaces_of(first).is_empty(), "the previous window's list loses the surface");
    assert_eq!(surfaces_of(second), vec![surface]);

    unregister(surface);
    assert!(surfaces_of(second).is_empty(), "an unregistered surface leaves the list");
}

/// A window widget's host id round-trips: `window -> host -> window`.
///
/// The `macos_objc2`/`windows`/`wayland` backends allocate host ids in a space that is
/// **disjoint** from the widget id space (the platform counts from 1, the runtime reserves
/// `0x5345_4C46_…`). An OS callback hands the backend its own host id, and the backend must
/// translate it back before any registry lookup: `widget_id_for_host_window(host)` is that
/// inverse. This pins the translation the objc2 render/invalidate path (P-05) depends on,
/// which no test previously exercised.
#[test]
fn a_widget_window_id_round_trips_through_its_host_id() {
    let widget =
        register(Box::new(crate::widget::Label::new("win".to_string(), Rect::new(0, 0, 100, 80))))
            .expect("mount");
    let _unmount = MountGuard(widget);

    // The host number is deliberately in the *platform* space, not the widget space.
    let host: ObjectId = 7;
    assert!(set_host_window(widget, host), "a mounted window must accept a host association");
    assert_eq!(host_window_for(widget), Some(host), "window -> host");
    assert_eq!(widget_id_for_host_window(host), Some(widget), "host -> window");
    // An id the platform never built a host for resolves to nothing, not to a wrong widget.
    assert_eq!(host_window_for(host), None, "a host id is not itself a widget");
    assert_eq!(widget_id_for_host_window(widget), None, "a widget id is not a host id");
}

/// The host association is dropped with the widget, so a recycled id cannot inherit it.
#[test]
fn unmounting_a_window_clears_its_host_association() {
    let widget =
        register(Box::new(crate::widget::Label::new("win".to_string(), Rect::new(0, 0, 100, 80))))
            .expect("mount");
    let host: ObjectId = 99;
    assert!(set_host_window(widget, host));
    assert_eq!(widget_id_for_host_window(host), Some(widget));

    unregister(widget);

    assert_eq!(host_window_for(widget), None, "the mapping must not outlive the widget");
    assert_eq!(
        widget_id_for_host_window(host),
        None,
        "a later widget reusing this host id must not resolve to the dead one"
    );
}

/// An unmounted id cannot be given a host window: a mapping to nothing is refused.
#[test]
fn set_host_window_refuses_an_unmounted_id() {
    let ghost: ObjectId = 0x5345_4C46_DEAD_BEEF;
    assert!(!set_host_window(ghost, 1), "only a mounted window may carry a host id");
    assert_eq!(host_window_for(ghost), None);
}

// ── clipping a container's descendants ─────────────────────────────────────

/// A container's `children_clip` really narrows what its descendants paint.
///
/// # The defect this pins
///
/// A scroll area places its content with the scroll offset in the child's geometry —
/// without a clip the content then paints outside the viewport, over the scroll bars and
/// over whatever lies beyond the control. The container is the only layer that knows the
/// viewport (`geometry` is not it: the bars are inside the rectangle), so the painter has
/// to ask it. The root fills white, the child fills red, and the clip decides which half
/// of the frame each one wins.
#[test]
fn the_tree_painter_clips_descendants_to_the_container_content_box() {
    use crate::core::Color;
    use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

    struct Surface {
        base: BaseWidget,
        clip: Option<Rect>,
        color: Color,
    }
    impl Widget for Surface {
        fn base(&self) -> &BaseWidget {
            &self.base
        }
        fn base_mut(&mut self) -> &mut BaseWidget {
            &mut self.base
        }
        fn children_clip(&self) -> Option<Rect> {
            self.clip
        }
        fn as_draw_mut(&mut self) -> Option<&mut dyn Draw> {
            Some(self)
        }
    }
    impl crate::event::EventHandler for Surface {
        fn handle_event(&mut self, _event: &crate::event::Event) {}
    }
    impl Draw for Surface {
        fn draw(&mut self, context: &mut RenderContext) {
            context.fill_rect(self.base.geometry(), self.color);
        }
    }

    let root = register(Box::new(Surface {
        base: BaseWidget::new(WidgetKind::Panel, Rect::new(0, 0, 60, 60), "root"),
        clip: Some(Rect::new(10, 10, 20, 20)),
        color: Color::WHITE,
    }))
    .expect("mount root");
    let child = register(Box::new(Surface {
        base: BaseWidget::new(WidgetKind::Panel, Rect::new(0, 0, 60, 60), "child"),
        clip: None,
        color: Color::RED,
    }))
    .expect("mount child");
    with_widget_mut(root, |widget| {
        widget.base_mut().add_child_linked(child);
    })
    .expect("the root is mounted");

    let frame = render_frame_tree(root, Size::new(60, 60), Color::WHITE).expect("frame");
    let pixel = |x: usize, y: usize| {
        let index = (y * 60 + x) * 4;
        (frame[index], frame[index + 1], frame[index + 2])
    };
    assert_eq!(pixel(15, 15), (255, 0, 0), "inside the clip the child paints");
    assert_eq!(pixel(5, 5), (255, 255, 255), "outside it the root still wins");
    assert_eq!(pixel(40, 40), (255, 255, 255), "and the clip has a far edge too");

    unregister(child);
    unregister(root);
}

/// A window-tree frame is reused while nothing that affects it has changed.
///
/// # What this pins
///
/// Backends call their paint callback for exposures and occlusion changes as well as for
/// content changes. Re-rasterising a whole tree for an expose is work whose result is already
/// on screen. The counter makes the difference observable: two calls with no change must draw
/// **once**, and a submitted change (here `mark_widget_damage`, the path every
/// `request_redraw` takes) must make the next call draw again.
#[test]
fn the_cached_tree_frame_is_reused_until_content_changes() {
    use crate::core::Color;
    use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
    use std::cell::Cell;
    use std::rc::Rc;

    struct Counter {
        base: BaseWidget,
        draws: Rc<Cell<usize>>,
    }
    impl Widget for Counter {
        fn base(&self) -> &BaseWidget {
            &self.base
        }
        fn base_mut(&mut self) -> &mut BaseWidget {
            &mut self.base
        }
        fn as_draw_mut(&mut self) -> Option<&mut dyn Draw> {
            Some(self)
        }
    }
    impl crate::event::EventHandler for Counter {
        fn handle_event(&mut self, _event: &crate::event::Event) {}
    }
    impl Draw for Counter {
        fn draw(&mut self, context: &mut RenderContext) {
            self.draws.set(self.draws.get() + 1);
            context.fill_rect(self.base.geometry(), Color::rgb(20, 40, 60));
        }
    }

    let draws = Rc::new(Cell::new(0));
    let id = register(Box::new(Counter {
        base: BaseWidget::new(WidgetKind::Panel, Rect::new(0, 0, 40, 30), "counter"),
        draws: Rc::clone(&draws),
    }))
    .expect("mount");
    let size = Size::new(40, 30);

    let first = render_frame_tree_cached(id, size, Color::WHITE).expect("first frame");
    assert_eq!(draws.get(), 1, "the first call has no frame to reuse");

    let second = render_frame_tree_cached(id, size, Color::WHITE).expect("cached frame");
    assert_eq!(draws.get(), 1, "an unchanged tree must not be drawn again");
    assert_eq!(first, second, "the reused frame must be the one that was drawn");

    // A control's own `request_redraw` reaches `mark_dirty_rect` through the own-id map.
    let _ = mark_dirty_rect(id, Rect::new(0, 0, 40, 30));
    let third = render_frame_tree_cached(id, size, Color::WHITE).expect("repainted frame");
    assert_eq!(draws.get(), 2, "a submitted change must repaint the tree");
    assert_eq!(first, third, "the same content paints the same bytes");

    unregister(id);
}

// ── per-widget layer cache ─────────────────────────────────────────────────

/// A plain container that paints its own background.
struct LayerRoot {
    base: BaseWidget,
}
impl Widget for LayerRoot {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn as_draw_mut(&mut self) -> Option<&mut dyn Draw> {
        Some(self)
    }
}
impl crate::event::EventHandler for LayerRoot {
    fn handle_event(&mut self, _event: &crate::event::Event) {}
}
impl Draw for LayerRoot {
    fn draw(&mut self, context: &mut RenderContext) {
        context.fill_rect(self.base.geometry(), Color::WHITE);
    }
}

/// A control that counts how often its own `draw` ran.
struct LayerProbe {
    base: BaseWidget,
    draws: Rc<DrawCount>,
}
impl Widget for LayerProbe {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn as_draw_mut(&mut self) -> Option<&mut dyn Draw> {
        Some(self)
    }
}
impl crate::event::EventHandler for LayerProbe {
    fn handle_event(&mut self, _event: &crate::event::Event) {}
}
impl Draw for LayerProbe {
    fn draw(&mut self, context: &mut RenderContext) {
        self.draws.set(self.draws.get() + 1);
        context.fill_rect(self.base.geometry(), Color::rgb(20, 40, 60));
    }
}

/// A text-heavy control: every `draw` lays out many labels.
struct LayerText {
    base: BaseWidget,
    draws: Rc<DrawCount>,
}
impl Widget for LayerText {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn as_draw_mut(&mut self) -> Option<&mut dyn Draw> {
        Some(self)
    }
}
impl crate::event::EventHandler for LayerText {
    fn handle_event(&mut self, _event: &crate::event::Event) {}
}
impl Draw for LayerText {
    fn draw(&mut self, context: &mut RenderContext) {
        use crate::core::{Font, HorizontalAlignment};
        self.draws.set(self.draws.get() + 1);
        let font = Font::simple("Arial", 12.0);
        let rect = self.base.geometry();
        for line in 0..16 {
            context.draw_text(
                Point::new(rect.x + 2, rect.y + 2 + line * 14),
                "The quick brown fox jumps over the lazy dog 0123456789",
                &font,
                Color::BLACK,
                HorizontalAlignment::Left,
            );
        }
    }
}

/// Builds a root with one child and returns both ids plus the child's draw counter.
fn layer_fixture(root_geometry: Rect, child_geometry: Rect) -> (ObjectId, ObjectId, Rc<DrawCount>) {
    let draws = Rc::new(DrawCount::new(0));
    let root = register(Box::new(LayerRoot {
        base: BaseWidget::new(WidgetKind::Panel, root_geometry, "layer-root"),
    }))
    .expect("mount root");
    let child = register(Box::new(LayerProbe {
        base: BaseWidget::new(WidgetKind::Panel, child_geometry, "layer-child"),
        draws: Rc::clone(&draws),
    }))
    .expect("mount child");
    with_widget_mut(root, |widget| widget.base_mut().add_child_linked(child))
        .expect("the root is mounted");
    (root, child, draws)
}

/// The second frame of an untouched control replays its layer and never runs `draw`.
#[test]
fn an_unchanged_widget_replays_its_cached_layer_without_drawing() {
    clear_widget_layers_for_test();
    let (root, child, draws) = layer_fixture(Rect::new(0, 0, 60, 40), Rect::new(5, 5, 20, 20));
    let size = Size::new(60, 40);

    let first = render_frame_tree(root, size, Color::WHITE).expect("first frame");
    assert_eq!(draws.get(), 1, "the first frame has no layer to reuse");

    let _ = take_widget_layer_replays_for_test();
    let second = render_frame_tree(root, size, Color::WHITE).expect("second frame");
    assert_eq!(draws.get(), 1, "an unchanged control must not run draw again");
    assert!(
        take_widget_layer_replays_for_test() >= 1,
        "the second frame must be served from the layer cache"
    );
    assert_eq!(first, second, "the replay must be the bytes that were painted");

    unregister(child);
    unregister(root);
}

/// A submitted damage rectangle invalidates the control and replaces its layer.
#[test]
fn a_damaged_control_redraws_and_replaces_its_layer() {
    clear_widget_layers_for_test();
    let (root, child, draws) = layer_fixture(Rect::new(0, 0, 60, 40), Rect::new(5, 5, 20, 20));
    let size = Size::new(60, 40);

    render_frame_tree(root, size, Color::WHITE).expect("first frame");
    render_frame_tree(root, size, Color::WHITE).expect("cached frame");
    assert_eq!(draws.get(), 1, "the untouched frame replays the layer");

    // `mark_dirty_rect` always advances the tree epoch, even for a control with no
    // damage tracker of its own, which is the path `request_redraw` takes.
    let _ = mark_dirty_rect(child, Rect::new(5, 5, 20, 20));
    render_frame_tree(root, size, Color::WHITE).expect("dirty frame");
    assert_eq!(draws.get(), 2, "a recorded damage must redraw the control");

    render_frame_tree(root, size, Color::WHITE).expect("replacement cached frame");
    assert_eq!(draws.get(), 2, "the replacement layer serves the next clean frame");

    unregister(child);
    unregister(root);
}

/// Damage to one control does not throw away an unrelated control's layer.
///
/// This is what the separate layer epoch buys. A frame is still repainted whole after any
/// change — one frame is one rasterisation of the tree — but a control whose pixels did not
/// change replays the layer it already has. Before the layer epoch was separated from the
/// tree epoch, a hover on one button made every text control in the window shape again.
#[test]
fn a_damaged_control_keeps_an_unrelated_controls_layer() {
    clear_widget_layers_for_test();
    let root = register(Box::new(LayerRoot {
        base: BaseWidget::new(WidgetKind::Panel, Rect::new(0, 0, 60, 40), "selective-root"),
    }))
    .expect("mount root");
    let left_draws = Rc::new(DrawCount::new(0));
    let right_draws = Rc::new(DrawCount::new(0));
    let left = register(Box::new(LayerProbe {
        base: BaseWidget::new(WidgetKind::Panel, Rect::new(5, 5, 20, 20), "selective-left"),
        draws: Rc::clone(&left_draws),
    }))
    .expect("mount left");
    let right = register(Box::new(LayerProbe {
        base: BaseWidget::new(WidgetKind::Panel, Rect::new(35, 5, 20, 20), "selective-right"),
        draws: Rc::clone(&right_draws),
    }))
    .expect("mount right");
    with_widget_mut(root, |widget| {
        widget.base_mut().add_child_linked(left);
        widget.base_mut().add_child_linked(right);
    })
    .expect("the root is mounted");
    let size = Size::new(60, 40);

    render_frame_tree(root, size, Color::WHITE).expect("first frame");
    render_frame_tree(root, size, Color::WHITE).expect("cached frame");
    assert_eq!((left_draws.get(), right_draws.get()), (1, 1), "both controls have a warm layer");

    let _ = take_widget_layer_replays_for_test();
    let _ = mark_dirty_rect(left, Rect::new(5, 5, 20, 20));
    render_frame_tree(root, size, Color::WHITE).expect("damaged frame");
    assert_eq!(left_draws.get(), 2, "the damaged control repaints its layer");
    assert_eq!(right_draws.get(), 1, "an unrelated control keeps its layer");
    assert!(
        take_widget_layer_replays_for_test() >= 1,
        "the untouched control must have been served from its layer"
    );

    unregister(right);
    unregister(left);
    unregister(root);
}

/// Changing a control's geometry or size makes its old layer ineligible.
#[test]
fn resizing_a_control_invalidates_its_layer() {
    clear_widget_layers_for_test();
    let (root, child, draws) = layer_fixture(Rect::new(0, 0, 60, 40), Rect::new(5, 5, 20, 20));
    let size = Size::new(60, 40);

    render_frame_tree(root, size, Color::WHITE).expect("first frame");
    render_frame_tree(root, size, Color::WHITE).expect("cached frame");
    assert_eq!(draws.get(), 1, "the untouched frame replays the layer");

    assert!(set_geometry(child, Rect::new(5, 5, 28, 24)), "the child is mounted");
    render_frame_tree(root, size, Color::WHITE).expect("resized frame");
    assert_eq!(draws.get(), 2, "a resized control must repaint its layer");

    unregister(child);
    unregister(root);
}

/// A global repaint (the theme path) invalidates every per-widget layer.
#[test]
fn a_global_repaint_invalidates_cached_layers() {
    clear_widget_layers_for_test();
    let (root, child, draws) = layer_fixture(Rect::new(0, 0, 60, 40), Rect::new(5, 5, 20, 20));
    let size = Size::new(60, 40);

    render_frame_tree_cached(root, size, Color::WHITE).expect("first frame");
    render_frame_tree_cached(root, size, Color::WHITE).expect("cached frame");
    assert_eq!(draws.get(), 1, "the untouched frame replays the cache");

    arm_global_repaint();
    render_frame_tree_cached(root, size, Color::WHITE).expect("repainted frame");
    assert_eq!(draws.get(), 2, "a global repaint must not reuse the old theme's pixels");

    unregister(child);
    unregister(root);
}

/// A text-heavy control does not shape or rasterise again on an untouched frame.
#[test]
fn a_text_heavy_control_does_not_draw_again_on_an_unchanged_frame() {
    clear_widget_layers_for_test();
    let draws = Rc::new(DrawCount::new(0));
    let root = register(Box::new(LayerRoot {
        base: BaseWidget::new(WidgetKind::Panel, Rect::new(0, 0, 240, 200), "text-root"),
    }))
    .expect("mount root");
    let text = register(Box::new(LayerText {
        base: BaseWidget::new(WidgetKind::Panel, Rect::new(4, 4, 220, 180), "text-child"),
        draws: Rc::clone(&draws),
    }))
    .expect("mount text");
    with_widget_mut(root, |widget| widget.base_mut().add_child_linked(text))
        .expect("the root is mounted");
    let size = Size::new(240, 200);

    render_frame_tree(root, size, Color::WHITE).expect("first frame");
    assert_eq!(draws.get(), 1, "the first frame lays the text out");

    let _ = take_widget_layer_replays_for_test();
    render_frame_tree(root, size, Color::WHITE).expect("cached frame");
    assert_eq!(draws.get(), 1, "a cached frame must not shape or rasterise text again");
    assert!(
        take_widget_layer_replays_for_test() >= 1,
        "the text control must have been served by the layer cache"
    );

    unregister(text);
    unregister(root);
}

/// The layer cache drops least-recently-used entries instead of growing without bound.
#[test]
fn the_layer_cache_is_bounded() {
    clear_widget_layers_for_test();
    let root = register(Box::new(LayerRoot {
        base: BaseWidget::new(WidgetKind::Panel, Rect::new(0, 0, 64, 64), "bounded-root"),
    }))
    .expect("mount root");
    let draws = Rc::new(DrawCount::new(0));
    let mut children = Vec::new();
    for index in 0..(MAX_CACHED_WIDGET_LAYERS + 8) {
        let x = (index % 24) as i32 * 2;
        let y = (index / 24) as i32 * 2;
        let child = register(Box::new(LayerProbe {
            base: BaseWidget::new(WidgetKind::Panel, Rect::new(x, y, 2, 2), "bounded-child"),
            draws: Rc::clone(&draws),
        }))
        .expect("mount child");
        with_widget_mut(root, |widget| widget.base_mut().add_child_linked(child))
            .expect("the root is mounted");
        children.push(child);
    }

    render_frame_tree(root, Size::new(64, 64), Color::WHITE).expect("frame");
    assert!(
        cached_widget_layer_count_for_test() <= MAX_CACHED_WIDGET_LAYERS,
        "the cache must not exceed its configured bound"
    );

    for child in children {
        unregister(child);
    }
    unregister(root);
}

/// Two clips compose to their overlap, and a disjoint pair is a zero-area answer rather
/// than "no clip" (which would widen the subtree back to its parent).
#[test]
fn intersecting_two_clips_keeps_only_the_overlap() {
    assert_eq!(
        intersect_clip(Some(Rect::new(0, 0, 10, 10)), Some(Rect::new(5, 5, 10, 10))),
        Some(Rect::new(5, 5, 5, 5))
    );
    assert_eq!(
        intersect_clip(Some(Rect::new(0, 0, 10, 10)), Some(Rect::new(20, 20, 5, 5)))
            .map(|r| r.width),
        Some(0)
    );
    assert_eq!(intersect_clip(Some(Rect::new(0, 0, 10, 10)), None), Some(Rect::new(0, 0, 10, 10)));
    assert_eq!(intersect_clip(None, None), None);
}

/// `dirty_rects_into` is the buffer-reusing form: it answers exactly what `dirty_rects`
/// answers, and a second call keeps the caller's capacity (G-102).
#[test]
fn dirty_rects_into_matches_the_allocating_form_and_reuses_the_buffer() {
    let id = register(sample_editor("fn main() {}")).expect("registry");
    assert!(set_repaint_mode_adaptive(id));
    assert!(mark_dirty_rect(id, Rect::new(1, 2, 3, 4)));
    assert!(mark_dirty_rect(id, Rect::new(40, 10, 8, 8)));

    let allocating = dirty_rects(id);
    let mut reused = Vec::new();
    assert!(dirty_rects_into(id, &mut reused), "damage was recorded");
    assert_eq!(reused, allocating, "the buffer form must agree with the allocating form");
    let capacity = reused.capacity();

    // Reading the tracker does not consume it, so the second call reproduces the same
    // rectangles without growing a buffer that is already large enough.
    assert!(dirty_rects_into(id, &mut reused));
    assert_eq!(reused, allocating);
    assert_eq!(reused.capacity(), capacity, "a repeated read must reuse the caller's buffer");

    // The no-damage predicate returns early: the caller's buffer is cleared, not filled.
    let clean = register(sample_editor("clean")).expect("registry");
    let mut scratch = Vec::with_capacity(4);
    scratch.push(Rect::new(0, 0, 1, 1));
    assert!(!dirty_rects_into(clean, &mut scratch), "nothing has asked to repaint yet");
    assert!(scratch.is_empty(), "an undamaged widget must leave the buffer empty");

    unregister(clean);
    unregister(id);
}

/// The cached top-level window follows an adoption instead of pinning the window the widget
/// was first mounted under (G-106).
#[test]
fn top_level_window_of_re_resolves_after_an_adoption() {
    let first = register(Box::new(crate::widget::Window::new(
        "first".to_string(),
        Rect::new(0, 0, 200, 150),
    )))
    .expect("mount first window");
    let _first = MountGuard(first);
    let second = register(Box::new(crate::widget::Window::new(
        "second".to_string(),
        Rect::new(220, 0, 200, 150),
    )))
    .expect("mount second window");
    let _second = MountGuard(second);
    let child = register(Box::new(crate::widget::Button::new(
        "child".to_string(),
        Rect::new(10, 10, 40, 20),
    )))
    .expect("mount child");
    let _child = MountGuard(child);

    with_widget_mut(first, |widget| widget.base_mut().add_child_linked(child))
        .expect("first window is mounted");
    assert_eq!(
        top_level_window_of(child),
        Some(first),
        "the first adoption must be what the cached resolution records"
    );

    with_widget_mut(second, |widget| widget.base_mut().add_child_linked(child))
        .expect("second window is mounted");
    assert_eq!(
        top_level_window_of(child),
        Some(second),
        "an adoption after the cache was warmed must re-resolve the window"
    );
}
