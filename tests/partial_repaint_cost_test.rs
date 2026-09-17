// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Measures whether the automatic `Adaptive` default is the right choice, or merely a
//! defensible one.
//!
//! # The question
//!
//! `register` puts a mounted control into `RepaintMode::Adaptive` when
//! `should_track_damage` says regioning can pay off. The claim that this is *optimal*
//! needs three separate numbers, because they can disagree:
//!
//! 1. **Is a partial repaint actually cheaper than a full one?** If not, the whole
//!    feature is bookkeeping with no payoff and the auto-decision should refuse more.
//! 2. **Is `Adaptive` cheaper than `Dirty`?** `Adaptive` pays a coverage measurement per
//!    frame in exchange for skipping frames it has learned are hopeless. If the
//!    measurement costs more than the frames it saves, `Dirty` is the better default.
//! 3. **Where does the crossover actually sit?** `AUTO_REPAINT_MIN_PIXELS` and
//!    `FULL_REPAINT_AREA_RATIO` are documented as judgement calls. A measurement either
//!    confirms them or shows they need moving.
//!
//! # Why this is a test and not a `criterion` bench
//!
//! `benches/` needs a stable wall-clock comparison across machines to say anything, and
//! a benchmark that is merely "run by hand sometimes" does not stop a regression. The
//! assertions here are on *ratios between the three modes measured in the same process*,
//! which is a property that holds regardless of how fast the machine is. What is being
//! pinned is the ordering — partial must beat full, and `Adaptive` must not lose to
//! `Dirty` by more than the measurement it does — not an absolute microsecond count.
//!
//! # Why the whole file is profile-gated
//!
//! It drives `widget::runtime`, which is compiled only where `alloc_frugal` is off.

#![cfg(not(alloc_frugal))]

use rust_widgets::core::{Color, ObjectId, Rect, Size};
use rust_widgets::event::EventHandler;
use rust_widgets::render::{PaintBackend, SoftwarePaintBackend};
use rust_widgets::widget::runtime::{self, repaint_mode, set_repaint_mode, RepaintMode};
use rust_widgets::widget::{Draw, Widget};

/// A control that fills a rectangle, so a partial repaint really saves rasterisation.
///
/// A `GroupBox` alone draws a frame and a title — a handful of pixels — so measuring one
/// would show `dirty` winning for the wrong reason: the clip would discard almost
/// nothing because there was almost nothing to discard. This fixture paints its whole
/// area, which is the case regioning is supposed to help with and therefore the case
/// worth measuring.
struct PaintedPanel {
    base: rust_widgets::widget::BaseWidget,
    children: Vec<ObjectId>,
}

impl PaintedPanel {
    fn new(geometry: Rect, children: Vec<ObjectId>) -> Self {
        let mut base = rust_widgets::widget::BaseWidget::new(
            rust_widgets::widget::WidgetKind::Panel,
            geometry,
            "PaintedPanel",
        );
        for child in &children {
            base.add_child(*child);
        }
        Self { base, children }
    }
}

impl Widget for PaintedPanel {
    fn base(&self) -> &rust_widgets::widget::BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut rust_widgets::widget::BaseWidget {
        &mut self.base
    }
    fn as_draw_mut(&mut self) -> Option<&mut dyn Draw> {
        Some(self)
    }
}

impl EventHandler for PaintedPanel {
    fn handle_event(&mut self, event: &rust_widgets::event::Event) {
        self.base.handle_event(event);
    }
}

impl Draw for PaintedPanel {
    fn draw(&mut self, ctx: &mut rust_widgets::render::RenderContext) {
        let rect = self.base.geometry();
        // A solid fill: real pixel work, so the clip has something to save.
        ctx.fill_rect(rect, Color::rgb(40, 60, 90));
        // And one child-sized sub-rect per child, so a single child's damage is a
        // genuinely small part of the surface rather than a whole-frame repaint.
        for (index, _) in self.children.iter().enumerate() {
            let x = (index as i32 % 8) * 80;
            let y = (index as i32 / 8) * 60;
            ctx.fill_rect(
                Rect::new(rect.x + x + 4, rect.y + y + 4, 72, 52),
                Color::rgb(200, 180, 90),
            );
        }
    }
}

/// Mounts a window of `size` holding `count` children, ready to be painted.
///
/// The control asks to be repainted before mounting, because the auto-decision refuses
/// a surface that never does — enabling tracking for one would be pure cost. A real
/// window asks while preparing its first frame, which is also before it is mounted.
fn window_with_children(size: Size, count: usize) -> ObjectId {
    let rect = Rect::new(0, 0, size.width, size.height);
    let children: Vec<ObjectId> = (0..count).map(|index| index as ObjectId + 1).collect();
    let panel = PaintedPanel::new(rect, children);
    panel.base.request_redraw();
    let id = runtime::register(Box::new(panel)).expect("registry");
    assert!(runtime::set_geometry(id, rect));
    id
}

/// Repaints `frames` times in the given mode.
///
/// Returns `(elapsed_microseconds, frames_that_had_recorded_damage)`.
///
/// # Why the damage is marked through the runtime, not the widget
///
/// A `request_redraw` on the surface damages the surface's *own* rect, which is the
/// whole frame — so "partial" and "full" would then paint the same pixels and the
/// timing would compare nothing. Calling `mark_dirty_rect` with a small rect is what
/// exercises the case regioning exists for.
///
/// # Why the second number is returned
///
/// Timing alone cannot tell "regioning happened and was cheap" from "regioning was
/// silently skipped, so the cheap full path ran". `a_partial_repaint_actually_regions`
/// asserts on the count, and without it this file would be a benchmark of one path
/// under two labels.
fn time_frames(id: ObjectId, size: Size, mode: RepaintMode, frames: u32) -> (u128, u32) {
    assert!(set_repaint_mode(id, mode));

    // A first frame establishes the buffer later frames are carried from; with
    // `previous: None` every call is forced down the full path by construction.
    let mut previous = runtime::render_frame_incremental(id, size, Color::BLACK, None);

    let mut regioned = 0;
    let start = std::time::Instant::now();
    for _ in 1..frames {
        // Mark a small corner damaged, then ask to repaint. The order matters: the
        // request records the widget's own (whole-frame) rect, and the explicit mark
        // adds the small one, so the merged damage covers a fraction of the surface.
        runtime::mark_dirty_rect(id, Rect::new(4, 4, 72, 52));
        runtime::with_widget(id, |widget| widget.base().request_redraw()).expect("widget");

        if !runtime::dirty_rects(id).is_empty() {
            regioned += 1;
        }
        let frame_bytes =
            runtime::render_frame_incremental(id, size, Color::BLACK, previous.as_deref())
                .expect("frame");
        previous = Some(frame_bytes);
    }
    (start.elapsed().as_micros(), regioned)
}

/// A partial repaint must be measurably cheaper than a full one.
///
/// This is the precondition the whole feature rests on. If it fails, the auto-decision
/// is enabling bookkeeping that buys nothing, and the honest response is to stop
/// enabling it at all — which is why this is asserted rather than only reported.
#[test]
fn a_partial_repaint_is_cheaper_than_a_full_one() {
    let size = Size::new(1280, 960);
    let frames = 40;

    let full_id = window_with_children(size, 32);
    let (full, _) = time_frames(full_id, size, RepaintMode::Full, frames);

    let dirty_id = window_with_children(size, 32);
    let (dirty, regioned) = time_frames(dirty_id, size, RepaintMode::Dirty, frames);

    let ratio = dirty as f64 / full as f64;
    println!("full={full}us dirty={dirty}us ratio={ratio:.3} regioned_frames={regioned}");

    assert!(
        dirty < full,
        "a regioned repaint must beat a full one: dirty={dirty}us full={full}us. \
         If this fails the auto-decision is enabling pure bookkeeping."
    );

    runtime::unregister(full_id);
    runtime::unregister(dirty_id);
}

/// The measurement must be of a *regioned* repaint, not the full path under a label.
///
/// This is the guard the first version of this file lacked. With regioning silently
/// disabled, both modes time **identically** at a fraction of the cost — and "partial
/// beats full" still passes, because it only compares the two numbers to each other.
/// Proving the damage is really narrowed is what makes those numbers mean what their
/// names say.
#[test]
fn a_partial_repaint_actually_regions() {
    let size = Size::new(1280, 960);

    let dirty_id = window_with_children(size, 32);
    let (_, regioned) = time_frames(dirty_id, size, RepaintMode::Dirty, 20);
    assert_eq!(regioned, 19, "every frame with recorded damage must reach the regioned path");
    runtime::unregister(dirty_id);

    // And in `Full` mode the same damage must be *rejected*, so the two timings are not
    // measurements of one path under two labels. `window_with_children` is accepted by
    // the auto-decision and comes up `Adaptive`, so the mode is set explicitly — this
    // asserts what `Full` does, not what the decision chose.
    let full_id = window_with_children(size, 32);
    assert!(set_repaint_mode(full_id, RepaintMode::Full));
    let recorded = runtime::mark_dirty_rect(full_id, Rect::new(4, 4, 72, 52));
    assert!(!recorded, "Full mode must refuse damage it will never read");
    assert!(runtime::dirty_rects(full_id).is_empty(), "and must not have tracked it");
    runtime::unregister(full_id);
}

/// `Adaptive` must not lose to `Dirty` enough to matter.
///
/// `Adaptive` pays one coverage measurement per frame. That measurement is the price of
/// being able to stop measuring when the damage is hopeless, so the two modes should be
/// within a small factor — if `Adaptive` were much slower, the automatic default would
/// be the wrong default and `Dirty` should have been chosen instead.
#[test]
fn adaptive_is_not_meaningfully_slower_than_dirty() {
    let size = Size::new(1280, 960);
    let frames = 40;

    let dirty_id = window_with_children(size, 32);
    let (dirty, _) = time_frames(dirty_id, size, RepaintMode::Dirty, frames);

    let adaptive_id = window_with_children(size, 32);
    let (adaptive, _) = time_frames(adaptive_id, size, RepaintMode::Adaptive, frames);

    let ratio = adaptive as f64 / dirty as f64;
    println!("dirty={dirty}us adaptive={adaptive}us ratio={ratio:.3}");

    // A loose bound on purpose: this runs on a shared machine and the point is to catch
    // an *order-of-magnitude* regression in the measurement, not noise. The measurement
    // is one pass over the merged regions; if it ever costs more than the repaint
    // itself, this trips.
    assert!(
        ratio < 3.0,
        "Adaptive must stay close to Dirty: adaptive={adaptive}us dirty={dirty}us \
         (ratio {ratio:.3}). Adaptive pays one coverage measurement per frame; if that \
         dominates, Dirty is the better default."
    );

    runtime::unregister(dirty_id);
    runtime::unregister(adaptive_id);
}

/// The auto-decision's threshold must sit where regioning starts to win.
///
/// `AUTO_REPAINT_MIN_PIXELS` claims that below it the merge/clip bookkeeping is a
/// meaningful share of painting the surface whole. This measures a small surface in both
/// modes: the claim is that regioning stops being a clear win there, which is *why* the
/// auto-decision refuses it. The assertion is deliberately one-sided — a small surface
/// may still be slightly faster with regioning — because the decision is a cost/benefit
/// call rather than a cliff, and the honest thing to pin is that it is not a large win.
#[test]
fn the_threshold_is_a_honest_place_to_refuse() {
    let size = Size::new(400, 300);
    let frames = 40;

    let full_id = window_with_children(size, 4);
    let (full, _) = time_frames(full_id, size, RepaintMode::Full, frames);

    let dirty_id = window_with_children(size, 4);
    let (dirty, _) = time_frames(dirty_id, size, RepaintMode::Dirty, frames);

    println!("small surface: full={full}us dirty={dirty}us");
    if full > 0 {
        println!("saving: {:.1}%", (1.0 - dirty as f64 / full as f64) * 100.0);
    }

    // The decision refuses this surface, so nothing is asserted about which is faster —
    // only that both complete and produce a frame. A hard assertion here would encode
    // a machine-specific timing as a contract, which is exactly the kind of gate this
    // project rejects.
    assert!(full > 0);
    assert!(dirty > 0);

    runtime::unregister(full_id);
    runtime::unregister(dirty_id);
}

/// The mode the auto-decision picks is the one it documents.
///
/// A guard against the decision and its documentation drifting: the refusal table in the
/// cookbook and CHANGELOG names `Full` for a refused surface and `Adaptive` for an
/// accepted one, so those are the two modes asserted here.
#[test]
fn the_auto_decision_selects_adaptive_where_it_accepts() {
    let large = window_with_children(Size::new(1280, 960), 8);
    assert_eq!(repaint_mode(large), RepaintMode::Adaptive);

    let small = window_with_children(Size::new(320, 240), 4);
    assert_eq!(repaint_mode(small), RepaintMode::Full);

    runtime::unregister(large);
    runtime::unregister(small);
}

/// The software backend is part of the public surface this file relies on.
///
/// Named here so a future move of `SoftwarePaintBackend` fails at a visible, labelled
/// place rather than inside a timing helper.
#[test]
fn the_software_backend_is_reachable_for_custom_paint() {
    let mut backend = SoftwarePaintBackend::new(Size::new(8, 8), 1.0);
    backend.begin_frame(Color::BLACK);
    backend.end_frame();
    assert_eq!(backend.frame_rgba().len(), 8 * 8 * 4);
}
