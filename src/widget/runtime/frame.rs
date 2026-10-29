// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::*;

/// What one call to [`drive_frame`] did, kept for diagnostics and tests.
///
/// # Why this is a record and not a `bool`
///
/// "Did anything happen?" cannot answer the only question a frame loop ever has to
/// answer -- *should I schedule another frame?* -- without over-scheduling. A hover
/// that has finished still needs **one** more frame to be painted in its settled
/// state, and then no more; a window with nothing moving needs none. Both cases
/// return `false` from a bare `bool`, so a loop built on one either spins forever or
/// drops the frame that draws the settle. [`FrameOutcome::needs_another_frame`] is
/// computed from the bus *after* this frame's work, so the frame that observes a
/// settle reports `true` and the one after it reports `false`.
///
/// The remaining fields exist because "this frame cost something" should be
/// answerable *by cause*: how many events woke it, how many controls interpolated,
/// how many platform repaints went out. That is the same grouping BLUE24 §8 asks of
/// `FrameStats`, measured one second earlier in the pipeline.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrameOutcome {
    /// Events drained from the trigger queue and dispatched this frame.
    pub events_dispatched: usize,
    /// Controls that answered `is_animating()` and were advanced by `delta_ms`.
    pub controls_ticked: usize,
    /// Repaint requests submitted to the platform this frame.
    pub repaints_submitted: usize,
    /// Whether the host should schedule another frame.
    pub needs_another_frame: bool,
}

/// The decode accounting for a build with no decoder linked in: always zero.
///
/// # Why a real type and not `(0, 0, 0)`
///
/// `drive_frame` reads the cache counters twice per frame and takes the difference. On a build
/// without `feature = "image"` (`android`, `embedded`) there is no `crate::image` module to ask, and
/// naming it did not compile. The three readings could have been written as three separate `cfg`
/// blocks with literal zeros, but a type keeps the two readings structurally identical to the
/// gated arm: the same field names, the same subtraction below, one `cfg` on each read.
///
/// The zero is the *true* answer rather than a placeholder. With no decoder compiled in, no decode
/// can occur, so a frame's share is genuinely zero -- the same reasoning
/// `avatar::resolved_image_source` records for its own no-decoder arm. It is not a "we could not
/// measure this" stand-in, which would be a dishonest number in exactly the place this crate's
/// frame ledger exists to be honest about.
#[cfg(not(all(feature = "image", not(alloc_frugal))))]
#[derive(Default)]
pub(crate) struct NoDecoderStats {
    pub(crate) requests: u64,
    pub(crate) hits: u64,
    pub(crate) misses: u64,
}

/// A frame's cost, grouped by **what produced it** rather than as one total.
///
/// # Why not a duration
///
/// "This frame took 12 ms" cannot be acted on: it might be one control repainting the whole
/// surface, a damage merge that failed, or an animation running more frames than it needs. So
/// the ledger groups by **consumer** -- who was advanced, who was redrawn, how much merging
/// saved -- which is the form a reader can act on (BLUE24 §8.1).
///
/// # Why it is a second type beside [`FrameOutcome`]
///
/// [`FrameOutcome`] answers the frame loop's one question (*schedule another frame?*) and is
/// `Copy` because a loop holds it every iteration. This carries a `Vec` of causes, so it is
/// not `Copy` and is taken **on demand** -- a host that wants the account asks for it, and a
/// host that only drives frames never pays for the allocation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FrameStats {
    /// Monotonic frame counter, so two readings can be compared without a clock.
    pub index: u64,
    /// The delta this frame advanced animations by.
    pub delta_ms: u32,
    /// Events drained and dispatched this frame.
    pub events: usize,
    /// Controls that answered `is_animating()` and were advanced.
    pub controls_ticked: usize,
    /// Surfaces submitted for repaint this frame (after coalescing).
    pub controls_drawn: usize,
    /// Repaint requests **folded into** an earlier request for the same surface this frame.
    ///
    /// The saving, not the work: five requests for one surface submit once and coalesce four.
    pub repaints_coalesced: usize,
    /// Why each repainted surface was repainted, last cause first per surface.
    ///
    /// One entry per surface -- the reason it was repainted -- so a reader can answer "why is
    /// this still redrawing?" without a log. A surface with no entry is one that was not
    /// repainted, which is why "no anonymous repaint" (BLUE24 §8 criterion 4) is a statement
    /// about this list being complete rather than about it being non-empty.
    pub last_repaint_reason: alloc::vec::Vec<(ObjectId, RepaintReason)>,
    /// Image decodes the frame avoided, by sharing a decode an earlier frame already did.
    ///
    /// # Why an image decode belongs in a frame's account
    ///
    /// A decode is the most expensive thing a control can do inside `draw` — a full PNG inflate, or
    /// a JPEG pass, on the thread that is supposed to be presenting. "This frame was slow" is not
    /// answerable without knowing whether it decoded a photograph, and "is the cache helping" is not
    /// answerable without knowing whether the decodes went away. Both are one number.
    ///
    /// # These are deltas, and the counters they come from are cumulative
    ///
    /// [`crate::image::cache::stats`] is process-wide and monotonic, because it answers "what has
    /// this process saved". A frame reports its own share: the difference between the two readings
    /// taken around the frame's work. `decode_hits + decode_misses <= decode_requests` holds within a
    /// frame; across the process the counters balance exactly.
    ///
    /// A build without the `image` feature reports zeros rather than omitting the fields, so a host
    /// that reads them does not have to be compiled differently to do it.
    pub decode_requests: u64,
    /// Decode requests answered without decoding.
    pub decode_hits: u64,
    /// Decode requests that had to decode — the ones that cost this frame real time.
    pub decode_misses: u64,
}

/// The frame account as text, so it can leave the process.
///
/// # Why this is the exportable half of BLUE24 §12 U-8
///
/// U-8 (a remote / off-screen renderer) asks for the frame account to be a **product** rather than
/// an in-memory value: pixels can be sent to another machine, but "why this frame looks like this"
/// can only be transmitted if the account has a form that survives leaving the process. A `Debug`
/// derive is not that form — it is a debugger's view, unstable across versions and unreadable in a
/// log. This is the stable one: the fields in a fixed order, the causes by their published tokens.
///
/// It is also the answer to a defect that exists **today, in this process**: `last_frame_stats()`
/// had no consumer anywhere in the crate, so the ledger's whole purpose — answering "why is this
/// still redrawing?" — was reachable only from a Rust expression. A host logging a slow frame now
/// has something to log.
///
/// # Why one line and not JSON
///
/// The crate's JSON path is hand-written (`capability::designer_manifest`), and adding a second
/// serializer for a value with no JSON consumer would be a second mechanism for one question
/// (principle #101). A single line of `key=value` is what a log wants, diffs cleanly, and needs no
/// dependency in a `no_std` build.
impl core::fmt::Display for FrameStats {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "frame={} delta_ms={} events={} ticked={} drawn={} coalesced={} decode={}/{}/{}",
            self.index,
            self.delta_ms,
            self.events,
            self.controls_ticked,
            self.controls_drawn,
            self.repaints_coalesced,
            self.decode_requests,
            self.decode_hits,
            self.decode_misses,
        )?;
        // The causes are the part a reader scans for, so they are named rather than counted: a frame
        // that says "drawn=3" without saying why is the anonymous repaint BLUE24 §8 criterion 4
        // exists to prevent.
        for (surface, reason) in &self.last_repaint_reason {
            write!(f, " repaint[{}]={}", surface, reason.as_str())?;
        }
        Ok(())
    }
}

/// Performs every library-side task of one frame, in a fixed order, exactly once.
///
/// # Why a *frame* and not just [`tick_animations`]
///
/// Three entry points each did part of a frame, in an order that differed per platform:
/// [`crate::drain_triggers`] emptied the input queue, [`tick_animations`] advanced the
/// animation bus, and the paint path (`draw_bridge::draw_of`) advanced a control *again*
/// as a side effect of drawing it. So how many times a control was interpolated in one
/// frame depended on how many layers the host happened to call, when the only correct
/// answer is **once**, and **before** painting -- otherwise every frame paints the state
/// it had one frame ago and the animation visibly lags its own input.
///
/// This function is the one place that answers that ordering, so the answer is the same
/// on every backend.
///
/// # The order, and why no step may move
///
/// 0. **Refresh the device facts.** One snapshot per frame, read before anything else, so every
///    control in this frame sees the same text scale, motion preference and direction. A host
///    that changed a system setting sees it apply on the next frame, never part-way through a
///    paint (see `crate::style::environment`'s module docs).
/// 1. **Drain input.** Pointer, key and focus events that arrived since the last frame
///    land first, because an animation's *target* comes from state and state comes from
///    these events. Reversed, a control would not begin reacting until the following
///    frame -- the input latency the ordering exists to remove.
/// 2. **Advance the animation bus once.** Controls answering `is_animating()` take
///    `delta_ms`. Because this precedes painting, the frame that a hover starts on is
///    already one step in.
/// 3. **Report whether another frame is owed.** Taken *after* steps 1-2, which is what
///    makes the frame that observes a control settling report `true` (so the settle gets
///    painted) and the next one report `false` (so an idle window stops paying).
///
/// # Two loop-owned jobs run before step 0
///
/// Two things here are not *frame* tasks, and both belong to the loop rather than to a frame:
/// work another thread handed to this thread ([`crate::platform::main_thread`]) and a loop-stop
/// request made from another thread ([`crate::platform::stop`]). They are done at the top of this
/// function because this is the one function every loop this library owns calls **on the thread
/// that owns the loop** — GTK's timeout closure, the polling loops of the
/// state/wayland/windows/mobile backends, and AppKit's per-window `NSTimer` — which is the only
/// thread a toolkit permits either a modal-free callback or a stop on. Running them first means a
/// loop already on its way out is not asked for one more frame of work before it may leave.
///
/// # `repaints_submitted`: why it is counted rather than performed here
///
/// Damage is submitted where the damage is *discovered* -- `request_repaint`,
/// `mark_dirty_rect`, and the platform's own native redraw path -- so that a control
/// which redraws mid-frame does not wait for the next one. This function therefore
/// accounts for the submissions made during this frame's steps 1-2 rather than making
/// them itself; a frame loop learns "this frame invalidated N surfaces" without the
/// library having to buffer damage it has nowhere to wait for.
///
/// # Static frames are free
///
/// With nothing queued and nothing animating, this is two `try_with` lookups plus a
/// registry sweep of `is_animating()`, returns `needs_another_frame == false`, and the
/// host sleeps until the next event. That property is what lets the loop run at the
/// display's rate while the window is still, and it is asserted by this module's tests.
pub fn drive_frame(delta_ms: u32) -> FrameOutcome {
    // The two jobs that belong to the *loop* rather than to a frame are done here, on the
    // thread that owns the loop, before the frame's own work: work another thread handed to this
    // thread (`platform::main_thread`), then a stop request another thread made
    // (`platform::stop`). See this function's docs for why the tick is the place for both.
    crate::platform::main_thread::run_due_on_loop_thread();
    crate::platform::stop::honour_request_on_loop_thread();

    // BLUE28 B28.24: a suspended app has no animation clock. Input is still drained (so state
    // changes are not queued for the whole background period), but no control advances and no
    // frame is owed. The frame after a resume uses a zero delta so the background gap is not
    // replayed as one enormous step.
    let suspended = animation_suspended();
    let delta_ms = if take_resume_reset() { 0 } else { delta_ms };

    // The decode counters, read before anything else in the frame so this frame's share is exactly
    // the work done between here and the reading below -- and not whatever a previous frame left.
    //
    // # Why this is two `cfg` arms rather than one call
    //
    // `crate::image` is gated on `feature = "image"`, while this function is not: `full_widgets`
    // profiles like `android` and `embedded` have the widget tree but not the decoder. Naming
    // `crate::image::cache::stats()` unconditionally therefore failed to *compile* on those targets
    // (`unresolved module image`), which is why `check_android_cross.sh` reported a cross-target
    // break on `aarch64-linux-android` while every host build was green.
    //
    // The zero is not a placeholder. With no decoder compiled in, no decode *can* happen, so
    // "this frame decoded nothing" is the true count rather than a stand-in for an unknown one --
    // the same reasoning `avatar::resolved_image_source` uses for its no-decoder arm.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    let decodes_before = crate::image::cache::stats();
    #[cfg(not(all(feature = "image", not(alloc_frugal))))]
    let decodes_before = NoDecoderStats::default();

    // Step 0 -- the device facts, so every control in this frame sees one set of them.
    crate::style::environment::refresh_environment();

    // Step 0a -- controls whose `theme_changed` subscription fired since the last frame are
    // re-merged with the active theme (G-404). The notification arrives while the switching
    // caller holds the theme mutex, so it can only record intent; the merge happens here, where
    // no theme lock is held, and the global repaint below publishes the re-merged styles. A
    // switch path that restyles synchronously (the FFI entry points, the environment follow-up)
    // drains first, which leaves this a no-op.
    #[cfg(device_profile)]
    drain_pending_theme_restyle();

    // Step 0b -- a global input changed (theme, text scale, locale) since the last frame:
    // repaint every window and mounted surface once. Armed instead of performed at the
    // mutation site to keep the non-reentrant theme lock out of the platform path.
    flush_global_repaint();

    // Step 1 -- input first, so this frame's animation targets are this frame's state.
    let events_dispatched = crate::drain_triggers();

    // Step 2 -- exactly one advance per animating control, before any paint.
    //
    // The sweep also counts what it advanced, and reports whether anything is still moving: the
    // frame used to re-walk the whole registry for the count and a third time for
    // `needs_another_frame`, so one still frame cost three full-tree scans.
    let (controls_ticked, sweep_animating) = if suspended {
        (count_animating_widgets(), false)
    } else {
        tick_animations_sweep(delta_ms)
    };
    // A narrow submission queued while a control was lent out for its `tick` is flushed here,
    // before the ledger below reads the frame's repaint counts, so the frame that made the
    // request is the frame that reports it. The sweep already flushes its own; this also covers
    // a suspended frame, where no sweep runs.
    flush_deferred_damage();

    // Step 3 -- the bus answers for the *next* frame, so a settle is painted once more
    // and then scheduling stops. `sweep_animating` is this frame's answer from the sweep just
    // above; the host-owned fact and the settle latch are folded in by the helper.
    let needs_another_frame =
        !suspended && animation_bus_needs_another_frame_given(sweep_animating);

    // Step 4 -- translations that changed on disk, so an edited catalogue reaches the
    // screen without a restart. This runs *after* the animation step and before the
    // repaint counts are taken, because applying a reload can invalidate text and thus
    // submit a repaint; taking the counts first would attribute that repaint to the
    // next frame and make this frame's ledger understate its own work.
    //
    // It is unconditionally cheap when hot reload is off: `pump_hot_reload` is one
    // mutex acquisition and a `None` check. That matters because every platform loop
    // calls `drive_frame` on a timer, and a `cfg` here would fork the frame driver per
    // profile for a saving that is not measurable.
    #[cfg(feature = "i18n")]
    crate::i18n::pump_hot_reload();

    // The numbers the frame ledger will report. Taken together so the counts and the causes
    // describe the same frame: taking them apart would let a repaint be attributed to the
    // frame after the one that submitted it.
    let repaints_submitted = take_frame_repaint_count();
    let repaints_coalesced = take_frame_coalesced_count();
    let last_repaint_reason = take_frame_repaint_reasons();
    let index = FRAME_INDEX.try_with(|frame| frame.replace(frame.get() + 1)).unwrap_or(0);

    // This frame's share of the process-wide decode accounting. A delta rather than the totals,
    // because the question a frame ledger answers is about *this* frame; see `FrameStats`.
    //
    // Read **after** the draw work rather than with the repaint numbers above, because a decode
    // happens inside `draw` and the whole point is to account for it. (The repaint counts are taken
    // before painting for the opposite reason: they report what the *input* steps invalidated, and a
    // submission made while painting belongs to the frame that will paint it.)
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    let decodes_after = crate::image::cache::stats();
    #[cfg(not(all(feature = "image", not(alloc_frugal))))]
    let decodes_after = NoDecoderStats::default();
    let decode_requests = decodes_after.requests.saturating_sub(decodes_before.requests);
    let decode_hits = decodes_after.hits.saturating_sub(decodes_before.hits);
    let decode_misses = decodes_after.misses.saturating_sub(decodes_before.misses);

    let _ = LAST_FRAME_STATS.try_with(|slot| {
        slot.replace(Some(FrameStats {
            index,
            delta_ms,
            events: events_dispatched,
            controls_ticked,
            controls_drawn: repaints_submitted,
            repaints_coalesced,
            last_repaint_reason,
            decode_requests,
            decode_hits,
            decode_misses,
        }));
    });

    FrameOutcome { events_dispatched, controls_ticked, repaints_submitted, needs_another_frame }
}

thread_local! {
    /// The most recent frame's ledger, for [`last_frame_stats`].
    ///
    /// Stored rather than returned because [`drive_frame`]'s return type is part of the frame
    /// contract a loop calls every iteration (and is `Copy`); a host that wants the account
    /// reads it separately, so a host that does not want it pays nothing.
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static LAST_FRAME_STATS: core::cell::RefCell<Option<FrameStats>> =
        const { core::cell::RefCell::new(None) };
}

/// The ledger for the most recent [`drive_frame`], or `None` when no frame has run.
///
/// # Why "why" is answerable at all
///
/// Because every repaint submission in the crate names a [`RepaintReason`] at its one point of
/// submission, and this returns those reasons grouped by surface. A frame that repainted
/// seven surfaces because an animation is in flight says so; if instead it says nothing, no
/// repaint happened, which is the still-frame guarantee being *observable* rather than asserted
/// (BLUE24 §8 criterion 1).
pub fn last_frame_stats() -> Option<FrameStats> {
    LAST_FRAME_STATS.try_with(|slot| slot.borrow().clone()).ok().flatten()
}

/// Counts mounted controls whose last observed animation bit is set.
///
/// Both readers of the fact -- [`drive_frame`]'s diagnostic count and
/// [`has_animating_widgets`]'s boolean -- read the **same registry** the sweep drives, so they
/// cannot disagree with what a frame will actually tick (G-06).
pub(crate) fn count_animating_widgets() -> usize {
    ANIMATING.try_with(|set| set.borrow().len()).unwrap_or(0)
}

thread_local! {
    /// Repaint requests submitted since the last [`drive_frame`].
    ///
    /// A counter rather than a log: the frame only needs "how many", and the reasons
    /// belong to the control that asked (BLUE24 §8's `RepaintReason` is where that
    /// question is answered, not here). Reset by [`take_frame_repaint_count`].
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static FRAME_REPAINTS: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };

    /// Repaint requests that were **folded into an earlier request for the same surface**
    /// this frame, so they cost the platform nothing extra.
    ///
    /// A coalesced request is not a lost one: the surface is already going to be repainted,
    /// so asking again would be a second submission of the same work. Counting it is what
    /// lets a frame answer "how much did merging save?" rather than only "how many went
    /// out?" -- BLUE24 §8 criterion 3 (five controls asking for one rectangle submit once
    /// and coalesce four).
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static FRAME_REPAINTS_COALESCED: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };

    /// The last reason each surface was repainted, for BLUE24 §8's `FrameStats`.
    ///
    /// One entry per surface, overwritten (not appended), so a surface repainted fifty times
    /// in a frame reads as "the reason it was repainted" rather than fifty rows. `Explicit`
    /// means "a caller asked, and named no cause" — which is itself a fact worth being able
    /// to see, because it is the one reason a gate cannot attribute to a mechanism.
    ///
    /// This map is also the coalescing set: "has this surface already been repainted this
    /// frame?" is `contains_key`. Keeping one ordered map rather than a `Vec` of surfaces plus
    /// a `Vec` of reasons makes membership O(log n) instead of a linear scan on every repaint
    /// request, which is what made a full-tree invalidate O(n²) in the number of surfaces.
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static FRAME_REPAINT_REASONS: core::cell::RefCell<
        alloc::collections::BTreeMap<ObjectId, RepaintReason>,
    > = const { core::cell::RefCell::new(alloc::collections::BTreeMap::new()) };

    /// Monotonic frame counter, incremented once per [`drive_frame`].
    ///
    /// A counter rather than a clock: two readings of [`FrameStats`] are comparable by
    /// subtracting indices, which needs no `SystemTime` and therefore works under `mini`
    /// (BLUE24 §4's "no device fact from a wall clock" rule applies to the frame ledger too).
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static FRAME_INDEX: core::cell::Cell<u64> = const { core::cell::Cell::new(0) };
}

/// Whether the animation clock is stopped because the host app is in the background.
///
/// A process-wide flag rather than a thread-local: the lifecycle callback that sets it arrives
/// on the platform thread, while a frame loop may run on another thread (and `mini` has no
/// thread-local frame ledger at all). Suspend/resume is a fact about the app, not about a thread.
pub(crate) static ANIMATIONS_SUSPENDED: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

/// Set by [`set_animation_suspended(false)`](set_animation_suspended); the next frame advances by
/// zero so a long background gap is not replayed as one huge delta.
pub(crate) static ANIMATION_RESUME_RESET: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

/// Stops or restarts the animation clock (BLUE28 B28.24).
///
/// `true` is what a host calls from `onPause`/`sceneWillResignActive`: no control advances while
/// the app is in the background, so a transition that was in flight resumes from the phase it
/// reached rather than jumping to its end. `false` is `onResume`/`didBecomeActive`: the next
/// frame is advanced with a zero delta so the time spent in the background is not treated as one
/// frame's worth of animation.
///
/// Calling either transition repeatedly is harmless; only the value matters.
pub fn set_animation_suspended(suspended: bool) {
    use core::sync::atomic::Ordering;
    if suspended {
        ANIMATIONS_SUSPENDED.store(true, Ordering::SeqCst);
    } else {
        ANIMATIONS_SUSPENDED.store(false, Ordering::SeqCst);
        ANIMATION_RESUME_RESET.store(true, Ordering::SeqCst);
    }
}

/// Whether the animation clock is currently stopped.
pub fn animation_suspended() -> bool {
    ANIMATIONS_SUSPENDED.load(core::sync::atomic::Ordering::SeqCst)
}

/// Reads and clears the one-frame zero-delta request left by a resume.
pub(crate) fn take_resume_reset() -> bool {
    ANIMATION_RESUME_RESET.swap(false, core::sync::atomic::Ordering::SeqCst)
}

/// Why a surface was asked to repaint.
///
/// # Why a reason and not just a count
///
/// "This frame submitted seven repaints" cannot be acted on. "Seven repaints, all because an
/// animation is still in flight" says the animation is the cost; "seven, all because a control
/// changed its theme state" says the styles are; "seven, all `Explicit`" says a caller is
/// repainting by hand and no library mechanism can explain it. BLUE24 §8 asks the frame ledger
/// to answer by **cause**, and this is the cause.
///
/// The set is deliberately closed, and each variant is a mechanism a reader can go and find:
/// an implementation that adds a repaint must pick the variant that names *why*, so the ledger
/// cannot silently grow an "other" bucket that explains nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepaintReason {
    /// A control's self-driven animation advanced and needs to be seen.
    Animation,
    /// A control's interaction or semantic state changed (`widget_state` / `semantic_state`).
    State,
    /// An overlay's content changed (a tooltip, a popup, a menu).
    Overlay,
    /// A caller asked directly (a property write, an imperative redraw) and named no mechanism.
    Explicit,
    /// The platform itself asked for its surface to be redrawn.
    ///
    /// Distinct from every variant above because it is the *other* world in a mixed
    /// native/self-painted window (BLUE24 §7): the platform owns the pixels there, and this is
    /// how the library comes to know about it.
    Native,
}

impl RepaintReason {
    /// Whether this reason means the surface needs another frame after this one.
    ///
    /// Only [`RepaintReason::Animation`] does: it is the one cause that keeps producing new
    /// pixels frame after frame. The others describe a change that is now complete, and a
    /// frame loop that scheduled another frame for them would spin on a still window.
    pub fn keeps_animating(self) -> bool {
        matches!(self, RepaintReason::Animation)
    }

    /// The reason's stable spelling, for a frame account that leaves the process.
    ///
    /// # Why a cause needs a name at all
    ///
    /// [`FrameStats`] carries `last_repaint_reason` — one entry per repainted surface — because
    /// "why is this still redrawing?" is the question the ledger exists to answer. But until this
    /// function existed the answer could not be **written down**: the reason is an enum, and a host
    /// logging the account or showing it in a diagnostics pane had no spelling to print. The field
    /// was therefore complete for a Rust reader and useless to any other kind.
    ///
    /// The tokens are lower-case and stable, because they are what a log line or an exported
    /// account will be read and grepped for. They match [`RepaintReason::parse`], so the two cannot
    /// drift into a spelling that only one of them accepts.
    pub fn as_str(self) -> &'static str {
        match self {
            RepaintReason::Animation => "animation",
            RepaintReason::State => "state",
            RepaintReason::Overlay => "overlay",
            RepaintReason::Explicit => "explicit",
            RepaintReason::Native => "native",
        }
    }

    /// Parses a token produced by [`RepaintReason::as_str`].
    ///
    /// `None` rather than a default: an account that named a cause the library does not publish is
    /// a fact about a version mismatch, and silently folding it into `Explicit` would make a reader
    /// believe the library asked for a repaint it did not. The same rule the crate's other token
    /// parsers follow (`BevelDirection::parse`, `Elevation::parse`).
    pub fn parse(token: &str) -> Option<Self> {
        match token {
            "animation" => Some(RepaintReason::Animation),
            "state" => Some(RepaintReason::State),
            "overlay" => Some(RepaintReason::Overlay),
            "explicit" => Some(RepaintReason::Explicit),
            "native" => Some(RepaintReason::Native),
            _ => None,
        }
    }
}

/// Records that this frame submitted one repaint to the platform, for `reason` on `surface`.
///
/// Called from the damage-tracking paths, which are the single places a repaint is
/// requested, so the count cannot miss a submission made through the library.
///
/// # Coalescing
///
/// A second request for a surface already repainted this frame is **counted as coalesced
/// rather than submitted**. That is what makes BLUE24 §8 criterion 3 true: five controls
/// announcing damage on one surface cost the platform one submission, and the frame can
/// report the four it saved. The surface, not the control, is the unit because a submission
/// is per surface -- two controls drawing into one window are one repaint.
///
/// The reason recorded is the one from the **first** request for that surface this frame:
/// it is the cause that already made the frame happen, and overwriting it with a later, more
/// incidental reason would make the ledger describe the last caller rather than the cost.
pub(crate) fn note_frame_repaint_reason(surface: ObjectId, reason: RepaintReason) {
    // `true` when this surface was already recorded this frame. The stored reason is the
    // first one, which is the cause that made the frame happen.
    let already = FRAME_REPAINT_REASONS
        .try_with(|reasons| {
            let mut reasons = reasons.borrow_mut();
            match reasons.entry(surface) {
                alloc::collections::btree_map::Entry::Occupied(_) => true,
                alloc::collections::btree_map::Entry::Vacant(slot) => {
                    slot.insert(reason);
                    false
                }
            }
        })
        .unwrap_or(false);

    if already {
        let _ = FRAME_REPAINTS_COALESCED.try_with(|count| count.set(count.get().saturating_add(1)));
        return;
    }

    let _ = FRAME_REPAINTS.try_with(|count| count.set(count.get().saturating_add(1)));
}

/// Takes (and clears) the number of repaints submitted since the last call.
pub(crate) fn take_frame_repaint_count() -> usize {
    FRAME_REPAINTS.try_with(|count| count.replace(0)).unwrap_or(0)
}

/// Takes (and clears) the number of repaints folded into an earlier one this frame.
pub(crate) fn take_frame_coalesced_count() -> usize {
    FRAME_REPAINTS_COALESCED.try_with(|count| count.replace(0)).unwrap_or(0)
}

/// Takes (and clears) why each surface was repainted this frame, and resets the surfaces set.
///
/// The surfaces set is cleared here rather than in [`take_frame_repaint_count`] so the two
/// takers are called together by [`drive_frame`] and the coalescing window is exactly one
/// frame: clearing it early would let a second request for the same surface be counted as a
/// fresh submission within the same frame.
pub(crate) fn take_frame_repaint_reasons() -> alloc::vec::Vec<(ObjectId, RepaintReason)> {
    FRAME_REPAINT_REASONS
        .try_with(|reasons| core::mem::take(&mut *reasons.borrow_mut()).into_iter().collect())
        .unwrap_or_default()
}

/// Advances every animating control by `delta_ms`, reporting whether another frame is needed.
///
/// # Why this is the crate's *only* frame driver
///
/// `src/style/animation.rs` and eleven controls' own `tick` state machines were all fully
/// implemented and all unreachable: nothing in `src/widget/`, `src/app/`, `src/render/` or
/// `src/platform/` ever called one, so a hover faded nowhere and a caret never blinked. The
/// missing piece is a single driver, and "who drives" has two wrong answers:
///
/// * **a timer per control** -- a hundred controls, a hundred timers, none of them aligned to
///   the frame, and each one a chance to leak;
/// * **each backend driving** -- the same button advanced twice in one frame, so its animation
///   runs at a speed that depends on how many layers happened to call in.
///
/// So the driver is this one function, called once per frame by the host *before* it paints.
/// The set of controls advanced is the runtime's **animating registry** (`ANIMATING`), not a
/// fresh virtual `is_animating()` call for every mounted control. Membership is the control's
/// animating bit; it is set whenever the runtime observes a state change that leaves the control
/// in flight (mount, event dispatch, property write, tick) and cleared when a tick observes the
/// control settled. `unregister` clears it too, so a destroyed control cannot keep the registry
/// (or the frame loop) alive (G-06).
///
/// # The return value is the whole point
///
/// `true` means "at least one control still owes frames", which is exactly when the host
/// should schedule another one. A window showing only resting controls therefore settles at
/// `false` and pays nothing per frame -- a still application does not burn a core, which is
/// the half of "smooth" that is easy to forget.
///
/// # Where a control can be, and why the sweep has to cover both
///
/// A control lives in exactly one of two places, and they are reached by completely
/// different APIs:
///
/// * **mounted** on the `MOUNTED` registry, which is how a host that built its tree
///   through [`crate::app`] holds every control; the window tree sweeps it;
/// * **owned by the caller**, as the `Box<dyn Widget>` that
///   [`crate::widget::WidgetFactory::create`] returns and that the documented embedding
///   pattern holds directly.
///
/// The first version of this driver swept only the registry. That is wrong for the owned
/// case, and wrong in the worst possible way: a control that is not mounted was never
/// advanced, yet nobody notices, because the control still *draws* -- it simply draws its
/// start state forever. A `CheckBox` ticked on program output, a `Switch` set from a
/// settings file, a caret in a directly-held `LineEdit` all sat motionless and reported no
/// error. The `census`/snapshot path in this very crate holds controls that way
/// (`draw_bridge::draw_of` takes `&mut dyn Widget`), which is why the gap is reachable from
/// the crate's own code and not only from a host's.
///
/// So a control that is advanced while the registry is being swept is recorded here, and
/// anything that subsequently asks *"should the next frame be scheduled?"* gets the union
/// of the two populations. That is what makes
/// [`crate::widget::draw_bridge::draw_of`] -- the single entry point for painting a
/// directly-owned control -- able to drive the frames such a control needs. Without it the
/// animation bus would be complete for mounted trees and silently inert for owned ones.
pub fn tick_animations(delta_ms: u32) -> bool {
    tick_animations_sweep(delta_ms).1
}

// The ids one animation sweep advanced. Reused across frames: a still frame used to allocate an
// empty `Vec` every time the sweep ran, so the "sixty still frames" promise of a cheap idle frame
// still paid one allocation per frame (G-37).
thread_local! {
    pub(crate) static ANIMATION_MOVED: RefCell<Vec<ObjectId>> = const { RefCell::new(Vec::new()) };
}

/// The sweep behind [`tick_animations`], also reporting how many controls it advanced.
///
/// `drive_frame` needs both numbers. Asking the registry a second time to count what the sweep
/// just saw made every frame walk the whole tree twice, and a third time to answer
/// "is anything still animating?". One walk now answers all of it.
pub(crate) fn tick_animations_sweep(delta_ms: u32) -> (usize, bool) {
    // A theme crossfade is a frame-driven animation too, and its tokens are read by every
    // control's paint. Advancing it here is what makes `set_theme_faded` visible without a
    // second timer; the manager arms the global repaint through `note_theme_mutation`.
    //
    // The theme module exists only on a device profile, so the sweep must compile without it:
    // a build with no profile has no theme to cross-fade, and reporting "not moving" is the
    // honest answer rather than a call the linker could not resolve.
    #[cfg(device_profile)]
    let theme_moving = if delta_ms == 0 {
        false
    } else {
        // H-205: the theme mutex may only be held while taking the frame-wait lock, never the
        // reverse. `tick_theme_crossfade` therefore advances the revision without arming the
        // repaint, and the arm happens after this guard is dropped.
        let (moving, crossfaded) = {
            let mut manager = crate::theme::global_theme_manager();
            let crossfaded = manager.is_crossfading();
            (manager.tick_theme_crossfade(delta_ms), crossfaded)
        };
        #[cfg(not(alloc_frugal))]
        if crossfaded {
            arm_global_repaint();
        }
        #[cfg(alloc_frugal)]
        let _ = crossfaded;
        moving
    };
    #[cfg(not(device_profile))]
    let theme_moving = {
        let _ = delta_ms;
        false
    };
    let mut still_animating = theme_moving;
    let mut ticked = 0usize;
    // Ids that advanced this sweep. Collected rather than invalidated in place: the sweep
    // holds each widget out of the registry for its `tick`, and a repaint request walks the
    // parent chain (and may call back into the registry), so doing it inside the loop would
    // nest two registry drives. The buffer is taken from its thread-local slot and put back
    // afterwards, so a frame with nothing to do allocates nothing (G-37).
    let mut moved: alloc::vec::Vec<ObjectId> =
        ANIMATION_MOVED.with(|slot| core::mem::take(&mut *slot.borrow_mut()));
    moved.clear();
    // The sweep visits the animating registry, not the whole tree (G-06). The ids are copied out
    // first because ticking re-enters the registry and may change the set; the buffer is reused
    // across frames.
    let mut candidates: alloc::vec::Vec<ObjectId> =
        ANIMATION_IDS.with(|slot| core::mem::take(&mut *slot.borrow_mut()));
    candidates.clear();
    ANIMATING.with(|set| candidates.extend(set.borrow().iter().copied()));
    for id in candidates.iter().copied() {
        // `with_widget_mut` detaches the control for its tick, which is the same contract the old
        // `for_each_mounted_widget` sweep relied on, and records the resulting animation bit on
        // the way out. A control that is no longer mounted (destroyed by a callback) is dropped
        // from the registry here.
        let outcome = with_widget_mut(id, |widget| {
            if !widget.is_animating() {
                return None;
            }
            let moving = widget.tick(delta_ms);
            Some((moving, widget.is_animating()))
        });
        match outcome {
            Some(Some((moving, still))) => {
                // `tick` also reports "moving"; either answer keeps the frame coming, because a
                // control can be mid-flight while its own `is_animating` is a coarser fact.
                if moving || still {
                    still_animating = true;
                }
                // Counted *after* the tick, matching the ledger's pre-refactor meaning: a control
                // that settles on this frame is not reported as still ticking.
                if still {
                    ticked += 1;
                }
                // Submitted even on the settle frame, where `tick` has just placed the value at
                // its target and both answers are now `false`. That frame is the one that paints
                // the **finished** state; skipping it left the last drawn frame one step short —
                // measured as a menu popup frozen at 70% of its height after the reveal had
                // already settled in the model.
                moved.push(id);
            }
            Some(None) => {
                // The bit became false between the last observation and this sweep.
            }
            None => {
                // Not mounted any more. A lent-out control (`is_mounted` still true) is refused
                // rather than gone, so it keeps its bit for a later sweep.
                if !is_mounted(id) {
                    note_animating(id, false);
                }
            }
        }
    }
    ANIMATION_IDS.with(|slot| {
        let mut slot = slot.borrow_mut();
        candidates.clear();
        if candidates.capacity() > slot.capacity() {
            *slot = candidates;
        }
    });

    // # Why a tick must submit a repaint
    //
    // Advancing the model is only half a frame. Every backend's loop calls `drive_frame` on a
    // timer and presents only when the host says a surface needs drawing; when the sweep moved
    // a control and submitted nothing, the value advanced while the pixels stayed on the frame
    // the change started in. Measured on the GTK demo: a menu's open reveal reached ~28% over
    // four frames and then froze, because the click that opened it was the last thing that
    // invalidated the window. The same split applied to every animated control in a painted
    // window tree. Submitting the repaint here — with the cause named — is what makes
    // `needs_another_frame` mean "another frame will be *drawn*", not just "another frame will
    // be computed".
    submit_animation_repaints(&moved);
    // Return the buffer to its slot, keeping the larger of the two capacities so the next
    // sweep does not have to grow it again.
    ANIMATION_MOVED.with(|slot| {
        let mut slot = slot.borrow_mut();
        moved.clear();
        if moved.capacity() > slot.capacity() {
            *slot = moved;
        }
    });
    flush_deferred_damage();
    // The latch is written **both ways**, and that is the whole point.
    //
    // # The defect this replaces
    //
    // It used to be set when `still_animating` and never cleared. The doc above explains what it
    // is for: scheduling the one frame that paints a control's *settled* state. But a flag whose
    // only clearer is the teardown function (`animation_bus_reset_host_owned`) is not a "one more
    // frame" request — it is a latch that turns `needs_another_frame` into a permanent `true`
    // from the first frame in which anything moved. A window that had ever shown a spinner could
    // never sleep again, which is the exact opposite of what `drive_frame` promises
    // (BLUE24 §1: "the cost of a still frame is one emptiness check").
    //
    // Writing `still_animating` unconditionally makes the flag mean what its name and its doc
    // say: *this* sweep found something unsettled. The settle frame still gets scheduled, because
    // the answer is read **after** the sweep: the last frame in which a control moved sets the
    // flag and is told `true`, the next sweep finds nothing and clears it, so the frame after
    // that is free. That is one extra frame, not an infinite stream.
    //
    // It cannot clobber the host-owned fact: `HOST_OWNED_ANIMATING` is a separate cell, written by
    // `draw_bridge` when it advances an owned control, and `animation_bus_needs_another_frame` ORs
    // the two.
    let _ = LAST_SWEEP_HAD_UNSETTLED_MOUNTED.try_with(|flag| flag.set(still_animating));
    (ticked, still_animating)
}

/// Records that a **host-owned** control was advanced this frame, and reports whether the
/// animation bus should currently consider one to be in flight.
///
/// # Why this is a pair of free functions and not a method
///
/// [`tick_animations`] hands a control `delta_ms` and receives "do I need another frame?"
/// back. A host that owns its controls and calls their `tick` directly never goes through
/// that function, so it has no way to tell it what it saw. These two functions are that
/// channel: the control's owner reports the fact, and the bus folds it into the answer a
/// frame loop asks for.
///
/// The state is deliberately **not** cleared by [`tick_animations`]: a frame loop that calls
/// the bus for its mounted tree and calls `tick` on its owned controls usually asks the
/// question *after* tickling them, so clearing on the sweep would erase the answer the
/// caller had just reported. It is cleared when a host-owned control is observed to have
/// settled ([`animation_bus_note_host_owned_settled`]) or by
/// [`animation_bus_reset_host_owned`].
pub fn animation_bus_note_host_owned_animating(animating: bool) {
    let _ = HOST_OWNED_ANIMATING.try_with(|flag| flag.set(animating));
}

/// Clears the host-owned in-flight fact once such a control has settled.
pub fn animation_bus_note_host_owned_settled() {
    let _ = HOST_OWNED_ANIMATING.try_with(|flag| flag.set(false));
}

/// Forgets every host-owned in-flight fact, including the record of the last registry sweep.
///
/// A frame loop that has torn down its controls calls this so a settled window cannot be kept
/// awake by a control that no longer exists.
pub fn animation_bus_reset_host_owned() {
    let _ = HOST_OWNED_ANIMATING.try_with(|flag| flag.set(false));
    let _ = LAST_SWEEP_HAD_UNSETTLED_MOUNTED.try_with(|flag| flag.set(false));
}

/// Whether the animation bus should schedule another frame right now.
///
/// # What it covers, and why it is not just [`has_animating_widgets`]
///
/// Two populations can be animating, and a host that owns some of its controls has to be
/// able to hear about both:
///
/// * the **mounted registry**, answered by [`has_animating_widgets`];
/// * a **host-owned** control that was advanced this frame, which the host reports with
///   [`animation_bus_note_host_owned_animating`] -- and, on the frame it settles, which is
///   the frame the old answer was `true` for a control that has since stopped, which is why
///   `LAST_SWEEP_HAD_UNSETTLED_MOUNTED` is kept: it is the only way to schedule the frame
///   that observes the settle.
///
/// ["smooth"](super) has two halves and this function is what keeps the second one honest:
/// a host asks *this*, not a constant `true`, so a window with nothing moving pays nothing.
pub fn animation_bus_needs_another_frame() -> bool {
    let mounted_animating = has_animating_widgets();
    animation_bus_needs_another_frame_given(mounted_animating)
}

/// [`animation_bus_needs_another_frame`] when the caller already knows whether a mounted control
/// is animating.
///
/// [`drive_frame`] gets that answer from the sweep it just ran, so it must not pay for a second
/// full-tree scan to ask the same question.
pub(crate) fn animation_bus_needs_another_frame_given(mounted_animating: bool) -> bool {
    let host_owned = HOST_OWNED_ANIMATING.try_with(|flag| flag.get()).unwrap_or(false);
    let sweep_unsettled =
        LAST_SWEEP_HAD_UNSETTLED_MOUNTED.try_with(|flag| flag.get()).unwrap_or(false);
    host_owned || sweep_unsettled || mounted_animating
}

/// Whether any mounted control is currently animating.
///
/// A host can ask this before deciding whether to enter a continuous-frame mode, without
/// paying for a full-tree walk: the answer is the animation registry's emptiness (G-06).
pub fn has_animating_widgets() -> bool {
    ANIMATING.try_with(|set| !set.borrow().is_empty()).unwrap_or(false)
}
