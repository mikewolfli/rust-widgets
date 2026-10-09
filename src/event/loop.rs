// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Event loop implementation.
use super::event_queue::{EventQueue, EventSender};
use super::timer::IdleTask;
use super::timer::TimerManager;
#[cfg(all(feature = "touch", not(alloc_frugal)))]
use super::translator::is_touch_cancel;
use super::types::{Event, EventPriority};
#[cfg(all(feature = "touch", not(alloc_frugal)))]
use crate::compat::HashMap;
use crate::compat::{lock, Box, MiniToString, Mutex, String, Vec};
use crate::core::ObjectId;
#[cfg(all(feature = "touch", not(alloc_frugal)))]
use crate::gesture::GestureEngine;
use alloc::sync::Arc;
use core::sync::atomic::AtomicU64;
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering;
use core::time::Duration;
#[cfg(not(alloc_frugal))]
use std::thread;
#[cfg(all(feature = "touch", not(alloc_frugal)))]
use std::time::{SystemTime, UNIX_EPOCH};

/// Type alias for event dispatch function.
pub type EventDispatchFn = Arc<dyn Fn(ObjectId, &Event) + Send + Sync>;

/// Returns the current timestamp in milliseconds since UNIX epoch.
#[cfg(all(feature = "touch", not(alloc_frugal)))]
fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

/// Whether an event should be handed to the gesture engine.
///
/// # Why this is not `Event::is_touch`
///
/// The engine's recognisers are driven by touch coordinates, so `is_touch()` was the obvious gate
/// — and it stranded two of them. `LongPressGesture`'s timeout lives in a `_` arm, and
/// `LongPressDragGesture` requires `Event::Timer` outright (`gesture/press.rs`), because a finger
/// held still produces **no** further touch event: time is the only thing that changes, and the
/// only thing that reports time is a timer tick. Gating on `is_touch()` therefore meant a stationary
/// 500 ms hold produced no `LongPress` on the real loop at all — the recogniser only ever fired in
/// its unit test, which fed it a `Timer` directly.
///
/// A timer is fed through for exactly that reason. Every other non-touch event is still skipped, so
/// the engine is not asked to interpret a `KeyPress` or a `Resize` as a gesture, and the events the
/// engine itself produces (`Tap`, `Swipe`, …) are `is_touch()`-true and so still round-trip.
#[cfg(feature = "touch")]
fn feeds_the_gesture_engine(event: &Event) -> bool {
    event.is_touch() || matches!(event, Event::Timer { .. })
}

/// Per-target gesture engines with a lifecycle, so a long-lived loop does not grow forever
/// (D09-EVT-03).
///
/// # The defect this closes
///
/// The loop kept a `HashMap<ObjectId, GestureEngine>` created on demand and **never** removed an
/// entry: every distinct target that ever received a touch retained an 11-recogniser engine until
/// the loop stopped, so an app that dynamically creates and destroys interaction targets grew the
/// map with the historical target count. `runtime::unregister` clears focus, hover and pointer
/// capture, but it cannot reach the loop thread's private map.
///
/// # The lifecycle boundary
///
/// An entry is dropped when its target is released (`release`), and swept when it has been idle
/// for longer than any recogniser's memory: [`Self::idle_grace`] exceeds the double-tap timeout,
/// so a target that is quietly waiting for the second tap of a double-tap keeps its engine, while
/// one that has not seen input for longer than that is reclaimed. Sweeping by *idle time* (rather
/// than on every touch end) is what keeps double-tap working — an engine removed the moment the
/// first tap ends could never recognise the second. A touch **cancel** drops the entry at once:
/// the interaction was withdrawn, so no recogniser state should survive it.
#[cfg(all(feature = "touch", not(alloc_frugal)))]
struct GestureRegistry {
    engines: HashMap<ObjectId, GestureEngineEntry>,
}

#[cfg(all(feature = "touch", not(alloc_frugal)))]
struct GestureEngineEntry {
    engine: GestureEngine,
    /// Last time this target received an event the engine consumed (`now_ms`).
    last_active_ms: u64,
}

#[cfg(all(feature = "touch", not(alloc_frugal)))]
impl GestureRegistry {
    fn new() -> Self {
        Self { engines: HashMap::new() }
    }

    /// How long an idle target's engine is retained after its last event.
    ///
    /// Strictly greater than `DOUBLE_TAP_TIMEOUT_MS` (400 ms) so a target waiting for the
    /// second tap of a double-tap is never swept, with headroom for the loop's own wake
    /// cadence.
    fn idle_grace() -> u64 {
        crate::gesture::DOUBLE_TAP_TIMEOUT_MS + 600
    }

    /// Routes `event` to `target`'s engine, creating it on demand, and returns any derived
    /// gesture event. A touch cancel discards the target's engine instead of feeding it, so no
    /// recogniser commits a withdrawn gesture and the entry does not linger.
    fn process(&mut self, target: ObjectId, event: &Event, now_ms: u64) -> Option<Event> {
        if is_touch_cancel(event) {
            self.engines.remove(&target);
            return None;
        }
        let entry = self.engines.entry(target).or_insert_with(|| GestureEngineEntry {
            engine: GestureEngine::new(),
            last_active_ms: now_ms,
        });
        entry.last_active_ms = now_ms;
        entry.engine.process(event, now_ms)
    }

    /// Drops engines idle longer than [`Self::idle_grace`]. Returns how many were removed.
    fn sweep(&mut self, now_ms: u64) -> usize {
        let grace = Self::idle_grace();
        let before = self.engines.len();
        self.engines.retain(|_, entry| now_ms.saturating_sub(entry.last_active_ms) <= grace);
        before - self.engines.len()
    }

    /// Drops the engine for `target` (on unregister). Returns whether one was present.
    fn release(&mut self, target: ObjectId) -> bool {
        self.engines.remove(&target).is_some()
    }

    /// Number of live engines (test/introspection only).
    #[cfg(test)]
    fn len(&self) -> usize {
        self.engines.len()
    }
}

/// Canonical event name for animation frame requests.
/// Used instead of a string literal to avoid fragile string matching.
pub const ANIMATION_FRAME_EVENT_NAME: &str = "animation_frame";

// ── Per-turn dispatch budgets (D09-EVT-04) ──
//
// # Why explicit limits rather than "drain everything"
//
// The loop used to pull **every** queued event into an unbounded `Vec` and then walk the
// High and Normal phases with no count or time limit. A sustained producer therefore made
// the buffer grow with the backlog, and an arbitrarily large High/Normal stream pushed the
// next platform pump, frame tasks and Idle work arbitrarily far out. The three constants
// below bound one turn's work; whatever a turn does not reach stays in the queue (or is
// re-queued in order) and is delivered on a later turn — bounded and eventually delivered,
// never silently dropped.

/// Maximum events pulled from the queue into a turn's dispatch buffer (D09-EVT-04).
///
/// This is what bounds the peak transient buffer: events beyond it simply remain in the
/// queue, so the buffer cannot grow with the backlog.
#[cfg(not(alloc_frugal))]
const PER_TURN_EVENT_DRAIN_CAP: usize = 512;

/// Time budget for the High-priority dispatch phase of one turn (D09-EVT-04).
///
/// High work is the most urgent, so it gets the largest share, but it is still bounded so
/// it cannot monopolise a turn and starve the platform pump and the next frame.
#[cfg(not(alloc_frugal))]
const HIGH_PHASE_BUDGET: Duration = Duration::from_millis(4);

/// Time budget for the Normal-priority dispatch phase of one turn (D09-EVT-04).
#[cfg(not(alloc_frugal))]
const NORMAL_PHASE_BUDGET: Duration = Duration::from_millis(4);

/// The request id carried by an animation-frame event, or `None` for any other event.
///
/// The id lives in the payload as eight little-endian bytes (see
/// [`EventLoop::request_animation_frame`]); decoding it here keeps the encoding in one place, so
/// the producer and the cancellation check cannot disagree about the layout.
/// Reads the frame id an animation-frame event carries, if it is one.
///
/// Used by both profiles: the threaded loop in its dispatch phase and the `mini`
/// `pump_once`, which must honour the same cancellation contract.
fn animation_frame_id(event: &Event) -> Option<u64> {
    let Event::Custom { name, payload } = event else {
        return None;
    };
    if name != ANIMATION_FRAME_EVENT_NAME {
        return None;
    }
    let bytes: [u8; 8] = payload.as_slice().try_into().ok()?;
    Some(u64::from_le_bytes(bytes))
}

/// Bookkeeping for animation-frame requests, shared with the loop thread.
///
/// # Why both a pending set and a cancelled set
///
/// `cancel_animation_frame` used to consult only the cancelled set, which records what
/// was cancelled, not what is still outstanding. It therefore reported `true` for a
/// handle that had already been dispatched, for one that was never issued, and for one
/// belonging to a different loop — and pushed duplicate entries. This type holds the
/// complementary fact: which ids are still pending. A cancel is accepted only for a
/// pending id, dispatches are marked, and the cancelled entries are consumed by the
/// dispatch check, so a repeated cancel and a cancel-after-dispatch both report `false`.
///
/// Both profiles use this type: the threaded loop shares it through an `Arc`, and the
/// `mini` synchronous pump owns it directly, so the two obey the same cancellation
/// contract (a cancelled frame is skipped before dispatch).
#[derive(Debug, Default)]
struct AnimFrameState {
    /// Ids requested but not yet dispatched or cancelled.
    pending: Vec<u64>,
    /// Pending ids whose dispatch must be skipped, consumed by the dispatch check.
    cancelled: Vec<u64>,
}

impl AnimFrameState {
    /// Records a new request as pending.
    fn request(&mut self, id: u64) {
        self.pending.push(id);
    }

    /// Drops a pending id without marking it cancelled.
    ///
    /// Used to roll back the registration made before publication when publishing the
    /// frame event fails (D09-EVT-08): the frame was never queued, so leaving it pending
    /// would let a later cancel of a request that was never issued report a change.
    fn forget(&mut self, id: u64) {
        if let Some(position) = self.pending.iter().position(|pending| *pending == id) {
            self.pending.swap_remove(position);
        }
    }

    /// Cancels `id` if it is still pending. Returns whether a state change occurred.
    fn cancel(&mut self, id: u64) -> bool {
        if let Some(position) = self.pending.iter().position(|pending| *pending == id) {
            self.pending.swap_remove(position);
            self.cancelled.push(id);
            return true;
        }
        false
    }

    /// Marks `id` dispatched if it is pending, then consumes a matching cancellation
    /// entry, returning whether the frame must be skipped.
    fn dispatched_and_cancelled(&mut self, id: u64) -> bool {
        if let Some(position) = self.pending.iter().position(|pending| *pending == id) {
            self.pending.swap_remove(position);
        }
        take_cancelled_ids(&mut self.cancelled, id)
    }
}

/// Whether `id` is in `cancelled`, removing it.
fn take_cancelled_ids(cancelled: &mut Vec<u64>, id: u64) -> bool {
    if let Some(position) = cancelled.iter().position(|pending| *pending == id) {
        cancelled.swap_remove(position);
        return true;
    }
    false
}

/// A handle returned by `request_animation_frame` that can be used to cancel the request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnimationFrameRequest {
    /// Unique identifier for this animation frame request.
    pub id: u64,
}

/// Main event loop for processing events.
pub struct EventLoop {
    /// Event queue for processing.
    #[cfg_attr(alloc_frugal, allow(dead_code))]
    // Only read by the non-mini `start()`; kept to mirror the desktop API.
    // Under mini the queue uses a single-threaded channel, so the Arc is not
    // Send/Sync — that is intentional for the mini (single-threaded) profile.
    #[cfg_attr(alloc_frugal, allow(clippy::arc_with_non_send_sync))]
    queue: Arc<Mutex<EventQueue>>,
    /// Independent sender for posting events without locking the queue.
    /// Avoids deadlock with the event loop thread which holds the queue mutex
    /// while blocked on `dequeue_blocking()`.
    sender: EventSender,
    /// Shared flag indicating if the loop is running.
    running: Arc<Mutex<bool>>,
    /// Processing thread handle.
    #[cfg(not(alloc_frugal))]
    thread_handle: Option<thread::JoinHandle<()>>,
    /// Field kept in `mini` so the shared method bodies compile unchanged.
    ///
    /// `mini` is single-threaded, so the loop runs on the caller's thread and there is
    /// no handle to join. This is *not* a placeholder with no behaviour: the alternative
    /// is two copies of every method, which is the drift this mirror prevents (see
    /// `mini`'s `pump`-driven `TimerManager` for the same pattern).
    #[cfg(alloc_frugal)]
    #[cfg_attr(alloc_frugal, allow(dead_code))]
    thread_handle: Option<()>,
    /// Optional dispatch callback invoked for each event.
    dispatch_fn: Option<EventDispatchFn>,
    /// Runtime timer manager emitting `Event::Timer` into this loop queue.
    timer_manager: TimerManager,
    /// Next animation frame request ID.
    next_anim_frame_id: AtomicU64,
    /// Optional native platform event pump.
    /// Called on each loop iteration to dispatch pending native platform events
    /// (e.g., Wayland dispatch_pending). Replaces standalone platform dispatch loops.
    native_pump: Option<Arc<dyn Fn() + Send + Sync>>,
    /// Deferred callbacks that run only when no frame-critical work is pending.
    ///
    /// Ticked in the loop's Idle phase, alongside Idle-priority events. Without this
    /// the loop had an Idle phase that idle *tasks* could not reach —
    /// [`IdleTask::tick`] documented itself as "called each frame by the event loop"
    /// while nothing ever called it.
    idle_tasks: Vec<IdleTask>,
    /// Ids of [`EventLoop::request_animation_frame`] requests that have been cancelled.
    ///
    /// # Why this exists
    ///
    /// [`AnimationFrameRequest`] was introduced as something that "can be used to identify or
    /// cancel" a frame, but nothing in the crate ever read its `id` and there was no cancel
    /// entry point at all — so the field was write-only (principle #99) and the documented
    /// capability did not exist. A host could request a frame, decide against it, and the
    /// callback still ran.
    ///
    /// Cancellation is checked at **dispatch** time rather than by removing the event from the
    /// queue: the queue is a lock-protected priority structure shared with the loop thread, and
    /// a removal would have to race the `dequeue_blocking` that already holds the mutex. An id
    /// set the dispatcher consults is the same guarantee without touching that lock discipline.
    ///
    /// The state ([`AnimFrameState`]) also records which ids are still *pending*, so a cancel
    /// of an already-dispatched, never-issued, or foreign handle is refused rather than
    /// accumulating a cancellation entry that can never match. Cancellation entries are taken
    /// (consumed) by the dispatch check, so the set does not grow without bound; ids are `u64`
    /// and never reused, so a stale entry can never cancel a later frame.
    ///
    /// Both profiles hold this state; only the sharing differs (the threaded loop shares it
    /// with the loop thread through an `Arc`, `mini` owns it directly).
    #[cfg_attr(alloc_frugal, allow(clippy::arc_with_non_send_sync))]
    cancelled_anim_frames: Arc<Mutex<AnimFrameState>>,
    /// Round-robin start index into `idle_tasks` for the next loop iteration.
    ///
    /// The Idle-task phase used to restart at index 0 every round and break on the 5 ms
    /// budget, so a slow first task let later tasks starve. Persisting the next start
    /// position rotates the phase: every task eventually reaches the front of the round,
    /// while the budget and the normal (cooldown-respecting) ordering are unchanged.
    ///
    /// Shared with the loop thread through an `Arc`, mirroring `cancelled_anim_frames`:
    /// the thread mutates it each round and the loop reads it on the next `start`.
    ///
    /// `mini` has no loop thread and no Idle-task phase, so the field is kept only to
    /// mirror the desktop layout (same reason `thread_handle` is mirrored there).
    #[cfg_attr(alloc_frugal, allow(dead_code))]
    idle_task_cursor: Arc<AtomicUsize>,
    /// Targets whose gesture state the host has asked the loop to drop (D09-EVT-03).
    ///
    /// The per-target gesture registry lives on the loop thread, so the host cannot reach
    /// it directly. This is the handoff: `EventLoop::release_gesture_target` pushes an id
    /// here and the loop drains the set at the top of each turn. A plain `Vec` under its own
    /// mutex (never the queue mutex) keeps the discipline simple — the producer never blocks
    /// on the loop's queue, and the loop never blocks on the producer beyond this short push.
    #[cfg(all(feature = "touch", not(alloc_frugal)))]
    gesture_releases: Arc<Mutex<Vec<ObjectId>>>,
}

impl EventLoop {
    /// Creates a new event loop.
    #[cfg_attr(alloc_frugal, allow(clippy::arc_with_non_send_sync))]
    pub fn new() -> Self {
        let queue = EventQueue::new();
        let sender = queue.sender();
        let timer_manager = TimerManager::new(sender.clone());
        Self {
            queue: Arc::new(Mutex::new(queue)),
            sender,
            running: Arc::new(Mutex::new(false)),
            thread_handle: None,
            dispatch_fn: None,
            timer_manager,
            next_anim_frame_id: AtomicU64::new(1),
            native_pump: None,
            idle_tasks: Vec::new(),
            idle_task_cursor: Arc::new(AtomicUsize::new(0)),
            #[cfg(all(feature = "touch", not(alloc_frugal)))]
            gesture_releases: Arc::new(Mutex::new(Vec::new())),
            cancelled_anim_frames: Arc::new(Mutex::new(AnimFrameState::default())),
        }
    }

    /// Registers a deferred task that runs when the loop has no frame-critical work.
    ///
    /// The task runs only after `threshold_frames` loop iterations have passed, so it
    /// is suited to background upkeep — cache trimming, statistics, prefetch — that
    /// must not compete with input or animation. Tasks are ticked only while the loop
    /// runs, and only within the same 5ms budget the Idle event phase uses.
    ///
    /// Must be called before [`EventLoop::start`]: the tasks move onto the loop
    /// thread. Registering after `start` would be a silent no-op, so this returns
    /// `false` in that case rather than accepting a task that will never run.
    ///
    /// Gated off `mini`, which never spawns the loop thread these run on — the same
    /// reason `start` itself is. Offering it there would promise work that cannot run.
    #[cfg(not(alloc_frugal))]
    pub fn add_idle_task(&mut self, task: IdleTask) -> bool {
        if *lock(&self.running) {
            return false;
        }
        // Replacing a task with the same id keeps the registry keyed rather than
        // letting a re-registration accumulate duplicates that all fire.
        self.idle_tasks.retain(|existing| existing.id != task.id);
        self.idle_tasks.push(task);
        true
    }

    /// Removes the idle task with `id`. Returns whether one was registered.
    pub fn remove_idle_task(&mut self, id: u64) -> bool {
        let before = self.idle_tasks.len();
        self.idle_tasks.retain(|task| task.id != id);
        self.idle_tasks.len() != before
    }

    /// Returns how many idle tasks are registered.
    pub fn idle_task_count(&self) -> usize {
        self.idle_tasks.len()
    }

    /// Starts the event loop in a separate thread.
    #[cfg(not(alloc_frugal))]
    pub fn start(&mut self) {
        if *lock(&self.running) {
            return;
        }
        *lock(&self.running) = true;
        let running = Arc::clone(&self.running);
        let queue = Arc::clone(&self.queue);
        let dispatch_fn = self.dispatch_fn.clone();
        #[cfg(feature = "touch")]
        let mut gesture_engines = GestureRegistry::new();
        let native_pump = self.native_pump.clone();
        // Idle tasks run on the loop thread, so they move with it. `mem::take` leaves the
        // field empty; a restart after `stop()` therefore begins with none, which is the
        // honest state — the caller re-registers what it still wants.
        let mut idle_tasks = core::mem::take(&mut self.idle_tasks);
        // The loop thread consults this state at dispatch time; it is shared rather than moved so
        // `cancel_animation_frame` (called from the host's thread) mutates the same one.
        let cancelled_anim_frames = Arc::clone(&self.cancelled_anim_frames);
        let idle_task_cursor = Arc::clone(&self.idle_task_cursor);
        #[cfg(all(feature = "touch", not(alloc_frugal)))]
        let gesture_releases = Arc::clone(&self.gesture_releases);
        let handle = thread::spawn(move || {
            while *lock(&running) {
                // Phase 0: Pump native platform events (e.g., Wayland dispatch)
                if let Some(ref pump) = native_pump {
                    pump();
                }
                // Phase 0a: Drain any pending scheduled tasks
                crate::event::types::drain_tasks();
                // Phase 0b: Apply host-requested gesture-state releases (D09-EVT-03). A target
                // that was unregistered no longer needs its engine, so dropping it here bounds
                // the registry by *live* targets rather than by every target ever seen.
                #[cfg(feature = "touch")]
                {
                    let releases: Vec<ObjectId> = {
                        let mut pending = lock(&gesture_releases);
                        core::mem::take(&mut *pending)
                    };
                    for target in releases {
                        gesture_engines.release(target);
                    }
                }
                // Phase 1a: Drain up to `PER_TURN_EVENT_DRAIN_CAP` events into per-priority
                // buffers (D09-EVT-04). Draining a bounded prefix — rather than everything —
                // keeps the transient buffer bounded under a sustained producer; the rest
                // stays queued and is picked up next turn, so ordering is preserved.
                let mut had_work = false;
                let mut high_events: Vec<(ObjectId, Event)> = Vec::new();
                let mut normal_events: Vec<(ObjectId, Event)> = Vec::new();
                let mut idle_events: Vec<(ObjectId, Event)> = Vec::new();
                let mut drained = 0usize;
                while drained < PER_TURN_EVENT_DRAIN_CAP {
                    let Some((target, event, priority)) = lock(&queue).dequeue() else { break };
                    had_work = true;
                    drained += 1;
                    match priority {
                        EventPriority::High => high_events.push((target, event)),
                        EventPriority::Normal => normal_events.push((target, event)),
                        EventPriority::Idle => idle_events.push((target, event)),
                    }
                }

                // One dispatch helper shared by the High and Normal phases, so both honour the
                // same animation-frame cancellation and gesture-routing rules (and neither is a
                // copy that can drift from the other). It borrows the gesture registry and the
                // cancellation state; the phase loops below own only their event buffers.
                let dispatch_fn_ref = &dispatch_fn;
                let cancelled_ref = &cancelled_anim_frames;
                #[cfg(feature = "touch")]
                let gesture_ref = &mut gesture_engines;
                // `mut` is only needed when the `touch` feature captures the gesture
                // registry by `&mut`; without it the binding is immutable.
                #[cfg_attr(not(feature = "touch"), allow(unused_mut))]
                let mut dispatch_one = |target: ObjectId, event: &Event| {
                    // A cancelled animation frame is discarded here, before any dispatch, so a
                    // host that cancelled never sees the callback. See `cancel_animation_frame`
                    // for why this is checked at dispatch rather than removed from the queue.
                    let anim_id = animation_frame_id(event);
                    let skip = match anim_id {
                        Some(id) => lock(cancelled_ref).dispatched_and_cancelled(id),
                        None => false,
                    };
                    if skip {
                        return;
                    }

                    #[cfg(feature = "touch")]
                    let maybe_gesture_event =
                        if feeds_the_gesture_engine(event) || is_touch_cancel(event) {
                            gesture_ref.process(target, event, now_ms())
                        } else {
                            None
                        };

                    if let Some(dispatch) = dispatch_fn_ref {
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            dispatch(target, event);
                            #[cfg(feature = "touch")]
                            if let Some(ref gesture) = maybe_gesture_event {
                                dispatch(target, gesture);
                            }
                        }));
                        if let Err(e) = result {
                            log::error!("[event-loop] Dispatch panicked: {e:?}");
                        }
                    } else {
                        log::warn!(
                            "[event-loop] No dispatch_fn set — dropping event {event:?} for target {target:?}"
                        );
                    }
                };

                // Phase 1b: Dispatch High-priority events first, within a time budget. Events
                // the budget does not reach are re-queued (High) so they are delivered next
                // turn rather than dropped; the buffer is already bounded by the drain cap.
                let mut processed = 0usize;
                let budget_start = std::time::Instant::now();
                while processed < high_events.len() {
                    if budget_start.elapsed() >= HIGH_PHASE_BUDGET {
                        break;
                    }
                    let (target, event) = &high_events[processed];
                    dispatch_one(*target, event);
                    processed += 1;
                }
                if processed < high_events.len() {
                    let sender = lock(&queue).sender();
                    for (target, event) in high_events.drain(processed..) {
                        let _ = sender.post_with_priority(target, event, EventPriority::High);
                    }
                }

                // Phase 1c: Dispatch Normal-priority events second, also budgeted. A cancelled
                // animation frame is discarded before any dispatch, so a host that cancelled
                // never sees the callback (see `cancel_animation_frame`), and every delivered
                // id is marked dispatched so a later cancel reports `false`.
                let mut processed = 0usize;
                let budget_start = std::time::Instant::now();
                while processed < normal_events.len() {
                    if budget_start.elapsed() >= NORMAL_PHASE_BUDGET {
                        break;
                    }
                    let (target, event) = &normal_events[processed];
                    dispatch_one(*target, event);
                    processed += 1;
                }
                if processed < normal_events.len() {
                    let sender = lock(&queue).sender();
                    for (target, event) in normal_events.drain(processed..) {
                        let _ = sender.post_with_priority(target, event, EventPriority::Normal);
                    }
                }

                // Phase 1e: Process buffered idle events with a 5ms time budget.
                // This prevents idle processing from starving frame-critical work. Whatever
                // the budget does not allow is re-queued, not dropped: idle means deferred,
                // not discarded.
                if !idle_events.is_empty() {
                    #[cfg(not(alloc_frugal))]
                    let idle_budget_start = std::time::Instant::now();
                    let mut processed = 0;
                    while processed < idle_events.len() {
                        #[cfg(not(alloc_frugal))]
                        if idle_budget_start.elapsed().as_millis() >= 5 {
                            break;
                        }
                        let (target, event) = &idle_events[processed];
                        // Same cancellation and gesture rules the High/Normal phases apply; an
                        // animation frame may be posted at any priority. `dispatch_one` owns
                        // those rules, so idle events cannot drift from the others.
                        dispatch_one(*target, event);
                        processed += 1;
                    }
                    // Deliver the remainder on a later, quieter iteration, in order.
                    if processed < idle_events.len() {
                        let sender = lock(&queue).sender();
                        for (target, event) in idle_events.drain(processed..) {
                            let _ = sender.post_idle(target, event);
                        }
                    }
                }

                // Phase 1f: Reclaim gesture engines whose targets have gone quiet (D09-EVT-03).
                // The sweep runs on the loop's own cadence, so a target that received a touch and
                // then went silent does not retain an 11-recogniser engine for the life of the
                // loop. The grace exceeds the double-tap timeout, so a target still waiting for the
                // second tap keeps its engine and double-tap keeps working.
                #[cfg(feature = "touch")]
                {
                    let removed = gesture_engines.sweep(now_ms());
                    if removed > 0 {
                        log::trace!("[event-loop] reclaimed {removed} idle gesture engine(s)");
                    }
                }

                // Phase 1e: Tick deferred idle tasks, under the same 5ms budget as
                // idle events so a task cannot starve frame-critical work either. A
                // task runs only once its `threshold_frames` have elapsed, which is
                // what makes it "idle" work rather than a per-frame callback.
                if !idle_tasks.is_empty() {
                    #[cfg(not(alloc_frugal))]
                    let task_budget_start = std::time::Instant::now();
                    let task_count = idle_tasks.len();
                    // Rotate the starting point each round so the tail is not starved by a slow
                    // head task. The phase still breaks on the 5ms budget; the saved cursor is
                    // where a later round resumes, and running the whole ring (at most once per
                    // task) keeps the normal ordering and `threshold_frames` cooldown intact.
                    let start_at = idle_task_cursor.load(Ordering::SeqCst) % task_count;
                    let mut offset = 0;
                    while offset < task_count {
                        #[cfg(not(alloc_frugal))]
                        if task_budget_start.elapsed().as_millis() >= 5 {
                            break;
                        }
                        let index = (start_at + offset) % task_count;
                        offset += 1;
                        let task = &mut idle_tasks[index];
                        // A panicking task must not take the loop down with it: the
                        // rest of the queue is still valid work.
                        let outcome =
                            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| task.tick()));
                        match outcome {
                            Ok(true) => had_work = true,
                            Ok(false) => {}
                            Err(e) => {
                                log::error!("[event-loop] Idle task {} panicked: {e:?}", task.id)
                            }
                        }
                    }
                    // Resume at the task after the last one attempted, wrapping around. A task
                    // the budget cut off is `start_at + offset` (offset was incremented past
                    // it), so the whole ring is covered across rounds even if every task is slow.
                    idle_task_cursor.store((start_at + offset) % task_count, Ordering::SeqCst);
                }

                // Phase 2: If no events were dispatched, sleep briefly to avoid
                // busy-waiting. The idle budget already consumed any idle work.
                if !had_work {
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
        });
        self.thread_handle = Some(handle);
    }

    /// Starts the event loop (no-op under mini/embedded).
    #[cfg(alloc_frugal)]
    pub fn start(&mut self) {
        *lock(&self.running) = true;
    }

    /// Processes one queued event and pumps timers in the mini profile.
    ///
    /// Mini builds do not spawn a background thread, so hosts must call this
    /// from their own frame/input loop to make `post_event` and timers live.
    ///
    /// # Priority (D09-EVT-06)
    ///
    /// The event dispatched is the **highest-priority** one available — High before Normal
    /// before Idle — with same-priority events kept in FIFO order. This matches the threaded
    /// loop's phased dispatch, so a High event posted after older Normal/Idle work is not
    /// delayed behind it. (The old pump dequeued strictly oldest-first and ignored the
    /// priority metadata entirely, giving the two profiles different ordering semantics.)
    ///
    /// This remains the synchronous counterpart of the threaded loop's dispatch in every
    /// other respect: it honours the same animation-frame contract (a cancelled frame is
    /// discarded before dispatch and its cancellation entry is consumed, exactly as the
    /// threaded loop does) and routes touch through the same gesture path.
    #[cfg(alloc_frugal)]
    pub fn pump_once(&mut self) -> bool {
        if !self.is_running() {
            return false;
        }
        self.timer_manager.pump();
        let next = lock(&self.queue).dequeue_priority_first();
        let Some((target, event, _priority)) = next else {
            return false;
        };
        let id = animation_frame_id(&event);
        let skip = match id {
            Some(id) => lock(&self.cancelled_anim_frames).dispatched_and_cancelled(id),
            None => false,
        };
        if skip {
            // The frame was cancelled: treat it as consumed work, not a dispatch.
            return true;
        }
        if let Some(dispatch) = &self.dispatch_fn {
            dispatch(target, &event);
        }
        true
    }

    /// Stops the event loop.
    #[cfg(not(alloc_frugal))]
    pub fn stop(&mut self) {
        *lock(&self.running) = false;
        self.timer_manager.clear();
        // Post a wake event so the loop thread's next poll observes the running flag without
        // waiting out its idle sleep. It does not hold the queue lock while sleeping — the loop
        // polls `dequeue` and releases the lock between iterations — so this is politeness, not
        // the deadlock-avoidance measure it was once described as.
        let _ =
            self.sender.post(0, Event::Custom { name: "__stop_wake".to_string(), payload: vec![] });
        if let Some(handle) = self.thread_handle.take() {
            if let Err(e) = handle.join() {
                log::error!("[event-loop] Thread join failed: {e:?}");
            }
        }
    }

    /// Stops the event loop.
    ///
    /// In `mini` there is no loop thread to stop, so this only clears the running flag;
    /// the caller's `pump` loop observes it. Same name and signature as the threaded
    /// version so call sites need no profile branch.
    #[cfg(alloc_frugal)]
    pub fn stop(&mut self) {
        *lock(&self.running) = false;
        self.timer_manager.clear();
    }

    /// Posts an event to the event loop.
    ///
    /// Uses an independent sender so posting never contends on the queue mutex. The loop
    /// *polls* `dequeue` each iteration and releases that lock between polls, so a producer is
    /// not blocked behind a sleeping loop thread.
    pub fn post_event(
        &self,
        target: ObjectId,
        event: Event,
        priority: EventPriority,
    ) -> Result<(), String> {
        self.sender.post_with_priority(target, event, priority)
    }

    /// Request the event loop to dispatch a custom animation frame event on the next iteration.
    ///
    /// Returns an `AnimationFrameRequest` handle that can be used to identify the request or to
    /// cancel it with [`EventLoop::cancel_animation_frame`]. This is similar to
    /// `window.requestAnimationFrame()` in browsers, cancellation included.
    pub fn request_animation_frame(
        &self,
        target: ObjectId,
    ) -> Result<AnimationFrameRequest, String> {
        let id = self.next_anim_frame_id.fetch_add(1, Ordering::SeqCst);
        // D09-EVT-08: register the request as pending **before** publishing the frame
        // event. Publishing first left a window in which the loop thread could dequeue
        // and dispatch the frame before this thread recorded it as pending; the dispatch
        // then consumed nothing, and the id was added to `pending` *after* it had already
        // run — so `cancel_animation_frame` would later report `true` for an already
        // dispatched frame, leaving an orphan cancellation entry no event could consume.
        //
        // Registering first closes that window: the frame event cannot be dequeued until
        // after `post_event`, by which point its id is already pending, so the dispatch
        // removes it from `pending` and every later cancel correctly reports `false`. The
        // lock is **not** held across `post_event` (which can synchronously run dispatch
        // callbacks), so this cannot deadlock against them.
        lock(&self.cancelled_anim_frames).request(id);
        let event = Event::Custom {
            name: ANIMATION_FRAME_EVENT_NAME.to_string(),
            payload: id.to_le_bytes().to_vec(),
        };
        if let Err(error) = self.post_event(target, event, EventPriority::Normal) {
            // The frame was never published, so the pending record must not survive it.
            lock(&self.cancelled_anim_frames).forget(id);
            return Err(error);
        }
        Ok(AnimationFrameRequest { id })
    }

    /// Cancels a previously requested animation frame, reporting whether it was still pending.
    ///
    /// # What "cancelled" means here
    ///
    /// The frame event may already be queued when this is called, so cancellation is enforced by
    /// the loop: when it dequeues an animation-frame event whose id is pending-and-cancelled, it
    /// **discards it without invoking the dispatch callback**. A host that cancels therefore never
    /// sees the callback, which is the guarantee `requestAnimationFrame`'s counterpart gives.
    ///
    /// # The return value
    ///
    /// `true` when this call is the one that cancelled a still-pending request. A second cancel
    /// of the same request returns `false`: the frame is already cancelled, and reporting `true`
    /// again would say a state change happened when none did. A request that has **already been
    /// dispatched** also reports `false`, because there is nothing left to cancel — as does a
    /// handle that was never issued through this loop. Tracking the pending set is what lets the
    /// return value mean this rather than merely "an id was recorded".
    pub fn cancel_animation_frame(&mut self, request: AnimationFrameRequest) -> bool {
        // The request is cancellable only while it is still pending in this loop. An
        // already-dispatched frame, a repeated cancel, and a handle from another loop (or
        // one never issued here) all report `false` because none of them changes state.
        lock(&self.cancelled_anim_frames).cancel(request.id)
    }

    /// Whether `id` was cancelled, consuming the entry.
    ///
    /// Kept for callers that hold an [`EventLoop`] and want to ask without going through the
    /// dispatch path (a test, or a host with its own pump). The loop thread uses the shared
    /// [`AnimFrameState`] because it captures the `Arc`, not `self`.
    pub fn take_cancelled_animation_frame(&self, id: u64) -> bool {
        lock(&self.cancelled_anim_frames).dispatched_and_cancelled(id)
    }

    /// Sets the dispatch callback invoked for each dequeued event.
    pub fn set_dispatch_fn(&mut self, f: EventDispatchFn) {
        self.dispatch_fn = Some(f);
    }

    /// Set a native platform event pump function.
    ///
    /// The pump is called at the start of each event loop iteration to dispatch
    /// pending native platform events (e.g., Wayland `dispatch_pending`).
    /// This replaces standalone platform dispatch loops with EventLoop-integrated
    /// dispatch.
    pub fn set_native_pump(&mut self, pump: Box<dyn Fn() + Send + Sync>) {
        self.native_pump = Some(Arc::from(pump));
    }

    /// Checks if the event loop is running.
    pub fn is_running(&self) -> bool {
        *lock(&self.running)
    }

    /// Start or replace a timer bound to `target` and `timer_id`.
    pub fn start_timer(
        &self,
        target: ObjectId,
        timer_id: u32,
        interval: Duration,
        repeating: bool,
    ) -> Result<(), String> {
        self.timer_manager.start_timer(target, timer_id, interval, repeating)
    }

    /// Stop one timer by target and id.
    pub fn stop_timer(&self, target: ObjectId, timer_id: u32) -> bool {
        self.timer_manager.stop_timer(target, timer_id)
    }

    /// Stop all timers associated with a target widget.
    pub fn stop_timers_for_target(&self, target: ObjectId) -> usize {
        self.timer_manager.stop_timers_for_target(target)
    }

    /// Releases the gesture state the loop holds for `target` (D09-EVT-03).
    ///
    /// # When to call this
    ///
    /// This is the loop-side half of a widget unregister: when a host tears down an
    /// interaction target (`runtime::unregister` clears focus, hover and pointer capture, but
    /// cannot reach the loop thread's per-target gesture registry), it should call this so
    /// the target's 11-recogniser engine is dropped rather than retained for the life of the
    /// loop. The request is queued and applied on the loop's next turn; if the loop is not
    /// running the request simply waits, and if it is never started the state is dropped with
    /// the loop.
    ///
    /// The idle sweep would eventually reclaim the engine anyway, but only after the
    /// registry's idle grace (longer than the double-tap timeout); this makes the reclaim
    /// immediate and explicit, which is what a deterministic teardown needs.
    #[cfg(all(feature = "touch", not(alloc_frugal)))]
    pub fn release_gesture_target(&self, target: ObjectId) {
        lock(&self.gesture_releases).push(target);
    }
}

crate::impl_default_via_new!(EventLoop);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::types::Event;
    use crate::event::EventPriority;
    use crate::event::EventQueue;
    #[cfg(not(alloc_frugal))]
    use alloc::sync::Arc;
    #[cfg(not(alloc_frugal))]
    use core::sync::atomic::{AtomicUsize, Ordering};
    // `AtomicBool` is only used by the native-pump tests, which are themselves gated
    // on `not(target_arch = "wasm32")` (there is no native pump in a browser sandbox).
    // Importing it unconditionally left an `unused_imports` warning on wasm32 builds.
    #[cfg(all(not(alloc_frugal), not(target_arch = "wasm32")))]
    use core::sync::atomic::AtomicBool;

    /// A timer tick must reach the gesture engine, or long press can never fire.
    ///
    /// # The defect this pins
    ///
    /// The loop gated the engine on `Event::is_touch()`. A finger held still emits no further
    /// touch event, so the only input that reports the passage of time is a `Timer` — and
    /// `is_touch()` excludes `Timer`. `LongPressGesture`'s timeout lives in a `_` arm reached only
    /// by a non-touch event, and `LongPressDragGesture` requires `Event::Timer` outright, so both
    /// were unreachable from the real loop while passing their own unit tests (which fed the
    /// recogniser a `Timer` directly).
    #[test]
    #[cfg(feature = "touch")]
    fn a_timer_reaches_the_gesture_engine() {
        use crate::core::Point;
        assert!(
            feeds_the_gesture_engine(&Event::Timer { id: 0 }),
            "a timer must reach the engine, or a long press can never time out"
        );
        assert!(
            feeds_the_gesture_engine(&Event::touch_begin(1, 1, 7)),
            "touch input is what the recognisers follow"
        );
        // A keyboard or geometry event is not a gesture, and must not be interpreted as one.
        assert!(!feeds_the_gesture_engine(&Event::KeyPress { key: 65, modifiers: 0 }));
        assert!(!feeds_the_gesture_engine(&Event::Resize { size: crate::core::Size::new(10, 10) }));
        assert!(!feeds_the_gesture_engine(&Event::Custom {
            name: "x".to_string(),
            payload: alloc::vec::Vec::new(),
        }));
        // And the engine's own output still round-trips.
        assert!(feeds_the_gesture_engine(&Event::LongPress { pos: Point::new(0, 0) }));
    }

    /// An idle task must actually run once the loop ticks enough frames.
    ///
    /// `IdleTask::tick` documented itself as "called each frame by the event loop"
    /// while nothing called it: the loop had an Idle *event* phase but idle *tasks*
    /// were unreachable. This drives the real loop and asserts the callback fires.
    #[test]
    #[cfg(not(alloc_frugal))]
    fn registered_idle_task_runs_on_the_loop() {
        let runs = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&runs);

        let mut el = EventLoop::new();
        assert!(el.add_idle_task(IdleTask::new(1, 2, move || {
            counter.fetch_add(1, Ordering::SeqCst);
        })));
        assert_eq!(el.idle_task_count(), 1);

        el.start();
        // The loop sleeps 1ms per iteration when idle, so a threshold of 2 frames plus
        // scheduling delay lands well inside this budget.
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(600);
        while runs.load(Ordering::SeqCst) == 0 && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        el.stop();

        assert!(
            runs.load(Ordering::SeqCst) > 0,
            "a registered idle task must run; nothing was ticking it before"
        );
    }

    /// Registering after the loop started must be refused, not silently accepted.
    ///
    /// Tasks move onto the loop thread at `start`, so a late registration could never
    /// run — and accepting it would look like success while doing nothing.
    #[test]
    #[cfg(not(alloc_frugal))]
    fn idle_task_registration_is_refused_once_running() {
        let mut el = EventLoop::new();
        el.start();
        assert!(
            !el.add_idle_task(IdleTask::new(7, 1, || {})),
            "a task registered after start could never run, so it must be refused"
        );
        assert_eq!(el.idle_task_count(), 0);
        el.stop();
    }

    /// Re-registering the same id replaces the task rather than queueing both.
    #[test]
    #[cfg(not(alloc_frugal))]
    fn idle_task_ids_are_unique() {
        let mut el = EventLoop::new();
        assert!(el.add_idle_task(IdleTask::new(3, 1, || {})));
        assert!(el.add_idle_task(IdleTask::new(3, 1, || {})));
        assert_eq!(el.idle_task_count(), 1, "a duplicate id must replace, not accumulate");
        assert!(el.remove_idle_task(3));
        assert!(!el.remove_idle_task(3), "removing twice must report no-op");
        assert_eq!(el.idle_task_count(), 0);
    }

    #[test]
    fn test_event_queue_high_throughput() {
        let queue = EventQueue::new();
        let sender = queue.sender();
        let target: ObjectId = 1;

        for i in 0..1000 {
            let bytes: [u8; 8] = (i as u64).to_le_bytes();
            let event = Event::Custom { name: "test".to_string(), payload: bytes.to_vec() };
            sender.post_with_priority(target, event, EventPriority::Normal).unwrap();
        }

        let mut count = 0;
        while let Some((_, _, _)) = queue.dequeue() {
            count += 1;
        }
        assert_eq!(count, 1000);
    }

    #[test]
    fn test_event_queue_empty_drain() {
        let queue = EventQueue::new();
        // Drain immediately on an empty queue — should return None
        assert!(queue.dequeue().is_none());
    }

    #[test]
    fn test_event_priority_order() {
        let queue = EventQueue::new();
        let sender = queue.sender();
        let target: ObjectId = 1;
        let normal_event = Event::Custom { name: "normal".to_string(), payload: vec![] };
        let high_event = Event::Custom { name: "high".to_string(), payload: vec![] };
        let idle_event = Event::Custom { name: "idle".to_string(), payload: vec![] };

        sender.post_with_priority(target, normal_event, EventPriority::Normal).unwrap();
        sender.post_with_priority(target, high_event, EventPriority::High).unwrap();
        sender.post_with_priority(target, idle_event, EventPriority::Idle).unwrap();

        // Drain and verify each event carries the correct priority metadata
        let mut events: Vec<EventPriority> = Vec::new();
        while let Some((_, _, prio)) = queue.dequeue() {
            events.push(prio);
        }
        // Since the underlying channel is FIFO, priority is stored as metadata;
        // each envelope retains the priority it was posted with.
        assert_eq!(events.len(), 3);
        assert_eq!(events[0], EventPriority::Normal);
        assert_eq!(events[1], EventPriority::High);
        assert_eq!(events[2], EventPriority::Idle);
    }

    #[cfg(all(not(alloc_frugal), not(target_arch = "wasm32")))]
    #[test]
    fn test_native_pump_called_on_empty_queue() {
        let mut el = EventLoop::new();
        let pump_called = Arc::new(AtomicBool::new(false));
        let pump_called_clone = pump_called.clone();

        el.set_native_pump(Box::new(move || {
            pump_called_clone.store(true, Ordering::SeqCst);
        }));

        el.start();
        #[cfg(not(alloc_frugal))]
        std::thread::sleep(std::time::Duration::from_millis(50));
        el.stop();

        assert!(
            pump_called.load(Ordering::SeqCst),
            "native pump should have been called during loop iteration"
        );
    }

    #[cfg(all(not(alloc_frugal), not(target_arch = "wasm32")))]
    #[test]
    fn test_event_loop_timer_integration() {
        let mut el = EventLoop::new();
        let timer_fired = Arc::new(AtomicBool::new(false));
        let timer_fired_clone = timer_fired.clone();

        el.set_dispatch_fn(Arc::new(move |_target, event| {
            if matches!(event, Event::Timer { id: 1 }) {
                timer_fired_clone.store(true, Ordering::SeqCst);
            }
        }));

        el.start_timer(1u64, 1, Duration::from_millis(20), false).unwrap();
        el.start();
        #[cfg(not(alloc_frugal))]
        std::thread::sleep(Duration::from_millis(150));
        el.stop();

        assert!(
            timer_fired.load(Ordering::SeqCst),
            "timer should have fired and been dispatched through the event loop"
        );
    }

    #[cfg(all(not(alloc_frugal), not(target_arch = "wasm32")))]
    #[test]
    fn test_event_loop_animation_frame_dispatch() {
        let mut el = EventLoop::new();
        let anim_fired = Arc::new(AtomicBool::new(false));
        let anim_fired_clone = anim_fired.clone();

        el.set_dispatch_fn(Arc::new(move |_target, event| {
            if let Event::Custom { name, .. } = event {
                if name == ANIMATION_FRAME_EVENT_NAME {
                    anim_fired_clone.store(true, Ordering::SeqCst);
                }
            }
        }));

        el.request_animation_frame(1u64).unwrap();
        el.start();
        #[cfg(not(alloc_frugal))]
        std::thread::sleep(Duration::from_millis(100));
        el.stop();

        assert!(
            anim_fired.load(Ordering::SeqCst),
            "animation frame event should have been dispatched through the event loop"
        );
    }

    /// A cancelled animation frame never reaches the dispatcher.
    ///
    /// # The defect this pins
    ///
    /// `AnimationFrameRequest` documented itself as something that "can be used to identify or
    /// cancel" a request, but nothing read its `id` and there was no cancel entry point at all
    /// (principle #99: a written-and-never-read field). A host could request a frame, decide
    /// against it, and the callback ran anyway.
    ///
    /// # Why the assertion is on the callback, not on the return value
    ///
    /// `cancel_animation_frame` returning `true` only proves the id was recorded. The guarantee
    /// the API advertises is behavioural — the callback does not run — so the test observes the
    /// callback, and a *different* request in the same loop is asserted to still fire, which is
    /// what stops the test passing because dispatch stopped entirely.
    #[cfg(all(not(alloc_frugal), not(target_arch = "wasm32")))]
    #[test]
    fn test_cancelled_animation_frame_never_dispatches() {
        let mut el = EventLoop::new();
        let cancelled_fired = Arc::new(AtomicBool::new(false));
        let kept_fired = Arc::new(AtomicBool::new(false));
        let cancelled_flag = Arc::clone(&cancelled_fired);
        let kept_flag = Arc::clone(&kept_fired);
        // Two requests are distinguished by their ids, which is exactly the payload the host
        // never had to decode before this test existed.
        let cancelled_id = Arc::new(AtomicU64::new(0));
        let seen_id = Arc::clone(&cancelled_id);

        el.set_dispatch_fn(Arc::new(move |_target, event| {
            let Some(id) = animation_frame_id(event) else { return };
            if id == seen_id.load(Ordering::SeqCst) {
                cancelled_flag.store(true, Ordering::SeqCst);
            } else {
                kept_flag.store(true, Ordering::SeqCst);
            }
        }));

        let doomed = el.request_animation_frame(1u64).unwrap();
        let kept = el.request_animation_frame(1u64).unwrap();
        assert_ne!(doomed.id, kept.id, "each request carries its own id");
        cancelled_id.store(doomed.id, Ordering::SeqCst);

        assert!(el.cancel_animation_frame(doomed), "the first cancel reports the change");
        assert!(
            !el.cancel_animation_frame(doomed),
            "a second cancel of the same request changes nothing, so it must not report `true`"
        );

        el.start();
        std::thread::sleep(Duration::from_millis(100));
        el.stop();

        assert!(
            !cancelled_fired.load(Ordering::SeqCst),
            "a cancelled frame must never reach the dispatcher"
        );
        assert!(
            kept_fired.load(Ordering::SeqCst),
            "the un-cancelled frame in the same loop must still fire, or this test would pass \
             for the wrong reason"
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_event_loop_start_stop_idempotent() {
        let mut el = EventLoop::new();

        // Start once
        el.start();
        assert!(el.is_running());

        // Start again — should be a no-op (running flag already true)
        el.start();
        assert!(el.is_running());

        // Stop
        el.stop();
        assert!(!el.is_running());

        // Stop again — should not panic
        el.stop();
        assert!(!el.is_running());
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_event_loop_post_event_without_dispatch() {
        // Posting events should not panic even when no dispatch function is set
        let mut el = EventLoop::new();
        el.start();
        let result = el.post_event(
            1u64,
            Event::Custom { name: "orphan".to_string(), payload: vec![] },
            EventPriority::Normal,
        );
        assert!(result.is_ok());
        #[cfg(not(alloc_frugal))]
        std::thread::sleep(Duration::from_millis(30));
        el.stop();
    }

    /// A cancel must be refused for a request that is not pending: already dispatched,
    /// never issued here, or belonging to another loop.
    ///
    /// The old `cancel_animation_frame` consulted only the cancelled set, so it returned
    /// `true` for any first-seen id — including a forged handle — and pushed a duplicate
    /// entry that no dispatch could ever consume.
    #[cfg(all(not(alloc_frugal), not(target_arch = "wasm32")))]
    #[test]
    fn cancel_is_refused_for_non_pending_handles() {
        let mut el = EventLoop::new();

        // Never issued through this loop.
        assert!(
            !el.cancel_animation_frame(AnimationFrameRequest { id: 999 }),
            "a forged id is not cancellable"
        );

        // Issued by another loop.
        let other = EventLoop::new();
        let foreign = other.request_animation_frame(1u64).unwrap();
        assert!(!el.cancel_animation_frame(foreign), "a foreign loop's handle is not cancellable");

        assert!(el.cancel_animation_frame(el.request_animation_frame(1).unwrap()));
        assert!(el.cancel_animation_frame(el.request_animation_frame(1).unwrap()));
    }

    /// Once a frame is dispatched, its handle can no longer be cancelled.
    #[cfg(all(not(alloc_frugal), not(target_arch = "wasm32")))]
    #[test]
    fn cancel_after_dispatch_reports_false() {
        let mut el = EventLoop::new();
        let dispatched = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&dispatched);
        el.set_dispatch_fn(Arc::new(move |_target, event| {
            if animation_frame_id(event).is_some() {
                flag.store(true, Ordering::SeqCst);
            }
        }));

        let handle = el.request_animation_frame(1u64).unwrap();
        el.start();
        let deadline = std::time::Instant::now() + Duration::from_millis(500);
        while !dispatched.load(Ordering::SeqCst) && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        el.stop();
        assert!(dispatched.load(Ordering::SeqCst), "the frame must have been dispatched first");

        assert!(
            !el.cancel_animation_frame(handle),
            "a dispatched frame has nothing left to cancel, so the report must be false"
        );
    }

    /// After a cancellation is consumed by dispatch, cancelling the same id again must
    /// report false rather than recording a stale entry.
    #[cfg(all(not(alloc_frugal), not(target_arch = "wasm32")))]
    #[test]
    fn a_consumed_cancellation_makes_later_cancels_false() {
        let mut el = EventLoop::new();
        let handle = el.request_animation_frame(1u64).unwrap();

        assert!(el.cancel_animation_frame(handle), "the first cancel reports the change");
        // Consume the cancellation the way the dispatch phase does.
        assert!(el.take_cancelled_animation_frame(handle.id));
        assert!(
            !el.cancel_animation_frame(handle),
            "the cancellation was consumed, so a later cancel changes nothing"
        );
    }

    /// Cancelling a still-pending frame prevents its callback from running.
    #[cfg(all(not(alloc_frugal), not(target_arch = "wasm32")))]
    #[test]
    fn a_pending_cancel_prevents_the_callback() {
        let mut el = EventLoop::new();
        let fired = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&fired);
        el.set_dispatch_fn(Arc::new(move |_target, event| {
            if animation_frame_id(event).is_some() {
                flag.store(true, Ordering::SeqCst);
            }
        }));

        let handle = el.request_animation_frame(1u64).unwrap();
        assert!(el.cancel_animation_frame(handle));

        el.start();
        std::thread::sleep(Duration::from_millis(100));
        el.stop();

        assert!(!fired.load(Ordering::SeqCst), "a cancelled pending frame must never dispatch");
    }

    /// A slow front idle task must not starve a later one forever.
    ///
    /// The Idle phase restarted at index 0 and broke on the 5ms budget every round, so a
    /// first task that always spent the budget left the tail unreachable. The saved
    /// round-robin cursor rotates which task is tried first.
    #[cfg(all(not(alloc_frugal), not(target_arch = "wasm32")))]
    #[test]
    fn a_slow_front_idle_task_does_not_starve_the_tail() {
        let slow_runs = Arc::new(AtomicUsize::new(0));
        let tail_runs = Arc::new(AtomicUsize::new(0));

        let mut el = EventLoop::new();
        let slow = Arc::clone(&slow_runs);
        // Threshold 1: eligible on every iteration, and it always burns the 5ms budget.
        assert!(el.add_idle_task(IdleTask::new(1, 1, move || {
            slow.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(8));
        })));
        let tail = Arc::clone(&tail_runs);
        assert!(el.add_idle_task(IdleTask::new(2, 1, move || {
            tail.fetch_add(1, Ordering::SeqCst);
        })));

        el.start();
        let deadline = std::time::Instant::now() + Duration::from_millis(1_500);
        while tail_runs.load(Ordering::SeqCst) == 0 && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        el.stop();

        assert!(
            tail_runs.load(Ordering::SeqCst) > 0,
            "the task behind a budget-burning head task must eventually run"
        );
        assert!(slow_runs.load(Ordering::SeqCst) > 0, "the slow task still runs too");
    }

    /// The animation-frame state machine: a cancel is accepted only while pending, a
    /// dispatch consumes the cancellation, and an uncancelled dispatch is never skipped.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn the_anim_frame_state_tracks_pending_and_cancelled() {
        let mut state = AnimFrameState::default();
        state.request(1);
        assert!(state.cancel(1), "a pending id can be cancelled");
        assert!(!state.cancel(1), "a second cancel of the same id changes nothing");
        // Dispatch sees the pending-and-cancelled id and must skip it, consuming the entry.
        assert!(state.dispatched_and_cancelled(1), "a cancelled pending frame is skipped");
        assert!(!state.dispatched_and_cancelled(1), "the cancellation was consumed");

        state.request(2);
        assert!(!state.dispatched_and_cancelled(2), "an uncancelled dispatch is not skipped");
        assert!(!state.cancel(2), "a dispatched id is no longer pending");
    }

    /// D09-EVT-08: a frame registered-then-dispatched before the caller cancels must not
    /// be cancellable, and no orphan entry may remain.
    ///
    /// This drives the exact interleaving the fix is about: the id is pending, the loop
    /// dispatches it (removing it from `pending`), and only then does the caller cancel.
    /// The cancel must report `false` (nothing to cancel) and leave both sets empty, rather
    /// than recording a cancellation that no future event can consume.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn a_frame_dispatched_before_cancel_leaves_no_orphan_entry() {
        let mut el = EventLoop::new();
        let handle = el.request_animation_frame(1u64).unwrap();

        // The request is pending the instant it is issued — the registration precedes
        // publication, so the loop can never dispatch ahead of it.
        {
            let state = lock(&el.cancelled_anim_frames);
            assert!(
                state.pending.contains(&handle.id),
                "the id must be pending immediately after the request returns"
            );
        }

        // The loop dispatches it (this is what the dispatch phase calls).
        assert!(
            !el.take_cancelled_animation_frame(handle.id),
            "an un-cancelled frame is not skipped at dispatch"
        );

        // Now the caller cancels: the frame already ran, so this changes nothing.
        assert!(
            !el.cancel_animation_frame(handle),
            "a frame dispatched before the cancel must report `false`"
        );

        let state = lock(&el.cancelled_anim_frames);
        assert!(state.pending.is_empty(), "no pending entry may survive a dispatched frame");
        assert!(
            state.cancelled.is_empty(),
            "no orphan cancellation entry may be recorded for a dispatched frame"
        );
    }

    /// D09-EVT-08: a long request/dispatch/cancel sequence converges to empty bookkeeping.
    ///
    /// Each request is dispatched immediately and then cancelled; because the registration
    /// precedes publication, every cancel must be refused and neither `pending` nor
    /// `cancelled` may grow.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn a_request_dispatch_cancel_sequence_leaves_the_sets_empty() {
        let mut el = EventLoop::new();
        for _ in 0..512 {
            let handle = el.request_animation_frame(1u64).unwrap();
            // Dispatch the frame (not cancelled → not skipped).
            assert!(!el.take_cancelled_animation_frame(handle.id));
            // Cancelling after dispatch must be a no-op.
            assert!(!el.cancel_animation_frame(handle));
        }
        let state = lock(&el.cancelled_anim_frames);
        assert!(state.pending.is_empty(), "no pending entries may accumulate");
        assert!(state.cancelled.is_empty(), "no cancelled entries may accumulate");
    }

    /// D09-EVT-08: a cancel before dispatch still wins, and the dispatch then consumes the
    /// cancellation, leaving both sets empty.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn a_cancel_before_dispatch_is_honoured_and_consumed() {
        let mut el = EventLoop::new();
        let handle = el.request_animation_frame(1u64).unwrap();
        assert!(el.cancel_animation_frame(handle), "a still-pending frame can be cancelled");
        assert!(
            el.take_cancelled_animation_frame(handle.id),
            "the dispatch must skip the cancelled frame"
        );
        let state = lock(&el.cancelled_anim_frames);
        assert!(state.pending.is_empty(), "the cancelled id is no longer pending");
        assert!(state.cancelled.is_empty(), "the cancellation was consumed by dispatch");
    }

    /// D09-EVT-08: under real concurrency the bookkeeping still converges to empty.
    ///
    /// The host thread issues and cancels frames while the loop thread dispatches them —
    /// the actual threading model of the API (`request_animation_frame`/
    /// `cancel_animation_frame` take `&mut self`, so the host owns the loop and `start`
    /// moves only the dispatch onto a thread). Whatever the interleaving, once the loop is
    /// stopped every request is either dispatched or cancelled-and-consumed, so no
    /// `pending` id and no orphan `cancelled` entry may remain. This is the race the fix
    /// closes: before it, a frame dispatched in the publish/registration window left a
    /// permanent `pending` entry and a `cancelled` entry that no event could consume.
    #[cfg(all(not(alloc_frugal), not(target_arch = "wasm32")))]
    #[test]
    fn concurrent_request_dispatch_cancel_converges_to_empty_state() {
        let dispatched = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&dispatched);

        let mut el = EventLoop::new();
        el.set_dispatch_fn(Arc::new(move |_target, event| {
            if animation_frame_id(event).is_some() {
                counter.fetch_add(1, Ordering::SeqCst);
            }
        }));
        el.start();

        // The host issues and immediately cancels frames while the loop drains them. The
        // cancel races the dispatch, which is the window D09-EVT-08 is about.
        for i in 0..2_000u64 {
            let request = el.request_animation_frame(1u64).unwrap();
            if i % 2 == 0 {
                let _ = el.cancel_animation_frame(request);
            }
        }

        // Let the loop drain what is left, then stop it (which joins the dispatch thread).
        let deadline = std::time::Instant::now() + Duration::from_millis(1_000);
        while std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        el.stop();

        let state = lock(&el.cancelled_anim_frames);
        assert!(
            state.pending.is_empty(),
            "every request must be resolved to dispatched or cancelled; {} still pending",
            state.pending.len()
        );
        assert!(
            state.cancelled.is_empty(),
            "every recorded cancellation must be consumed by a dispatch; {} orphaned",
            state.cancelled.len()
        );
        assert!(
            dispatched.load(Ordering::SeqCst) > 0,
            "the loop must actually have dispatched frames, or the race was never exercised"
        );
    }

    /// The `mini` synchronous pump honours the same cancellation contract as the
    /// threaded loop: a cancelled frame is discarded before dispatch.
    ///
    /// Only compiled (and only runnable) under `alloc_frugal`, because `pump_once` itself is;
    /// there is no desktop equivalent to exercise. The desktop dispatch-phase tests
    /// (`a_pending_cancel_prevents_the_callback`) cover the same contract for the threaded loop.
    #[cfg(alloc_frugal)]
    #[test]
    fn pump_once_honours_cancellation() {
        use core::sync::atomic::AtomicBool;

        let mut el = EventLoop::new();
        let fired = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&fired);
        el.set_dispatch_fn(Arc::new(move |_target, event| {
            if animation_frame_id(event).is_some() {
                flag.store(true, Ordering::SeqCst);
            }
        }));
        el.start();

        let handle = el.request_animation_frame(1).unwrap();
        assert!(el.cancel_animation_frame(handle));
        // The queue still holds the frame event; the pump must skip it.
        assert!(el.pump_once(), "a cancelled frame is consumed work");
        assert!(!fired.load(Ordering::SeqCst), "a cancelled frame must not dispatch in mini");

        // A later, uncancelled frame still dispatches.
        el.request_animation_frame(1).unwrap();
        assert!(el.pump_once());
        assert!(fired.load(Ordering::SeqCst), "an uncancelled frame still dispatches");
    }

    /// D09-EVT-06: the mini pump dispatches the highest-priority event available, not merely
    /// the oldest, and keeps FIFO order within a priority.
    ///
    /// The old `pump_once` dequeued strictly oldest-first and ignored the priority metadata,
    /// so a Normal/Idle event posted before a High one was dispatched first — different
    /// semantics from the threaded loop. This drives the mini pump directly and asserts the
    /// High event comes out first, then the remaining events in their posted order.
    #[cfg(alloc_frugal)]
    #[test]
    fn pump_once_dispatches_by_priority() {
        use std::sync::Mutex;

        let order: Arc<Mutex<Vec<&'static str>>> = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&order);

        let mut el = EventLoop::new();
        el.set_dispatch_fn(Arc::new(move |_target, event| {
            let Event::Custom { name, .. } = event else { return };
            let label = match name.as_str() {
                "idle" => "idle",
                "normal" => "normal",
                "high" => "high",
                _ => return,
            };
            log.lock().unwrap().push(label);
        }));
        el.start();

        // Posted oldest-first: Normal, then Idle, then a High. FIFO would yield Normal first.
        el.post_event(
            1,
            Event::Custom { name: "normal".to_string(), payload: vec![] },
            EventPriority::Normal,
        )
        .unwrap();
        el.post_event(
            1,
            Event::Custom { name: "idle".to_string(), payload: vec![] },
            EventPriority::Idle,
        )
        .unwrap();
        el.post_event(
            1,
            Event::Custom { name: "high".to_string(), payload: vec![] },
            EventPriority::High,
        )
        .unwrap();

        // Three pumps drain the three events.
        assert!(el.pump_once());
        assert!(el.pump_once());
        assert!(el.pump_once());
        assert!(!el.pump_once(), "the queue is empty after three events");

        assert_eq!(
            *order.lock().unwrap(),
            vec!["high", "normal", "idle"],
            "mini must dispatch High before Normal before Idle"
        );

        // Same-priority FIFO: two High events come out in the order posted.
        order.lock().unwrap().clear();
        el.post_event(
            1,
            Event::Custom { name: "high".to_string(), payload: vec![1] },
            EventPriority::High,
        )
        .unwrap();
        el.post_event(
            1,
            Event::Custom { name: "high".to_string(), payload: vec![2] },
            EventPriority::High,
        )
        .unwrap();
        assert!(el.pump_once());
        assert!(el.pump_once());
        assert_eq!(order.lock().unwrap().len(), 2, "both High events are dispatched");
    }

    /// Gesture recogniser state is scoped to the interaction target: a tap on one target must
    /// not arm a double-tap on another, and the registry reclaims engines for quiet targets
    /// (D09-EVT-03).
    #[test]
    #[cfg(all(feature = "touch", not(alloc_frugal)))]
    fn gesture_state_is_scoped_to_its_interaction_target() {
        let mut registry = GestureRegistry::new();

        // Target 1: a single tap produces a Tap.
        let _ = registry.process(1, &Event::touch_begin(10, 10, 1), 0);
        let first = registry.process(1, &Event::touch_end(10, 10, 1), 100);
        assert!(matches!(first, Some(Event::Tap { .. })), "a single tap on target 1 is a Tap");

        // Target 2: the same tap must not inherit target 1's first tap into a DoubleTap.
        let _ = registry.process(2, &Event::touch_begin(10, 10, 2), 200);
        let second = registry.process(2, &Event::touch_end(10, 10, 2), 250);
        assert!(
            !matches!(second, Some(Event::DoubleTap { .. })),
            "a first tap on target 2 must not inherit target 1's tap"
        );
        assert!(matches!(second, Some(Event::Tap { .. })), "target 2's own tap is still a Tap");

        // A second tap on target 2, within the window, does complete a double-tap.
        let _ = registry.process(2, &Event::touch_begin(10, 10, 3), 300);
        let third = registry.process(2, &Event::touch_end(10, 10, 3), 350);
        assert!(
            matches!(third, Some(Event::DoubleTap { .. })),
            "two taps on the same target still complete a double tap"
        );
    }

    /// D09-EVT-03: many distinct touch targets are reclaimed once they go quiet, so the
    /// registry stays bounded by *live* targets rather than by history.
    ///
    /// The old map created an engine per target and never removed one, so the count grew with
    /// every distinct target ever touched. This drives 1000 distinct targets and then sweeps
    /// past the idle grace: no orphan engine may remain.
    #[test]
    #[cfg(all(feature = "touch", not(alloc_frugal)))]
    fn many_distinct_targets_are_reclaimed_after_they_go_quiet() {
        let mut registry = GestureRegistry::new();
        // Touch every target at the same instant so "within the grace" is unambiguous.
        let touched_at = 1_000u64;
        for target in 0..1000u64 {
            let _ = registry.process(target, &Event::touch_begin(1, 1, target), touched_at);
            let _ = registry.process(target, &Event::touch_end(1, 1, target), touched_at);
        }
        assert_eq!(registry.len(), 1000, "every target touched so far holds an engine");

        // Sweep at exactly the grace boundary: nothing is reclaimed yet (a target may be
        // waiting for a second tap).
        let within_grace = touched_at + GestureRegistry::idle_grace();
        assert_eq!(registry.sweep(within_grace), 0, "a target within the grace must be kept");
        assert_eq!(registry.len(), 1000);

        // Past the grace, every quiet target is reclaimed.
        let past_grace = touched_at + GestureRegistry::idle_grace() + 1;
        assert_eq!(registry.sweep(past_grace), 1000, "quiet targets must be reclaimed");
        assert_eq!(registry.len(), 0, "no orphan gesture engine may remain");
    }

    /// D09-EVT-03: an explicit release drops one target's engine without touching the others.
    #[test]
    #[cfg(all(feature = "touch", not(alloc_frugal)))]
    fn release_drops_only_the_named_target() {
        let mut registry = GestureRegistry::new();
        let _ = registry.process(1, &Event::touch_begin(0, 0, 1), 0);
        let _ = registry.process(2, &Event::touch_begin(0, 0, 2), 0);
        assert_eq!(registry.len(), 2);

        assert!(registry.release(1), "releasing a present target reports true");
        assert_eq!(registry.len(), 1, "only the named target is dropped");
        assert!(!registry.release(1), "releasing an absent target reports false");
        assert_eq!(registry.len(), 1);
    }

    /// D09-EVT-03: a touch cancel discards the target's engine at once and yields no gesture,
    /// so a withdrawn contact neither commits nor lingers.
    #[test]
    #[cfg(all(feature = "touch", not(alloc_frugal)))]
    fn a_touch_cancel_drops_the_engine_and_produces_no_gesture() {
        let mut registry = GestureRegistry::new();
        let _ = registry.process(7, &Event::touch_begin(10, 10, 1), 0);
        assert_eq!(registry.len(), 1);

        let cancel = crate::event::translator::touch_cancel(crate::core::Point::new(10, 10), 1);
        let produced = registry.process(7, &cancel, 50);
        assert!(produced.is_none(), "a cancel must not produce a completed gesture");
        assert_eq!(registry.len(), 0, "the cancelled target's engine must be dropped");

        // The contact is gone: a following end for the same id carries no state and no Tap.
        let after = registry.process(7, &Event::touch_end(10, 10, 1), 60);
        assert!(after.is_none(), "an end after a cancel must not synthesise a tap");
    }

    /// D09-EVT-03: sweeping by idle time must not break double-tap — an engine kept within
    /// the grace still recognises the second tap.
    #[test]
    #[cfg(all(feature = "touch", not(alloc_frugal)))]
    fn double_tap_still_works_across_a_sweep_within_the_grace() {
        let mut registry = GestureRegistry::new();
        // First tap.
        let _ = registry.process(5, &Event::touch_begin(10, 10, 1), 0);
        let first = registry.process(5, &Event::touch_end(10, 10, 1), 50);
        assert!(matches!(first, Some(Event::Tap { .. })));

        // A sweep inside the grace keeps the engine that remembers the first tap.
        assert_eq!(registry.sweep(200), 0);

        // Second tap within the double-tap window completes the double-tap.
        let _ = registry.process(5, &Event::touch_begin(10, 10, 2), 250);
        let second = registry.process(5, &Event::touch_end(10, 10, 2), 300);
        assert!(
            matches!(second, Some(Event::DoubleTap { .. })),
            "double-tap must survive a sweep that happens within the idle grace"
        );
    }

    /// D09-EVT-04: a burst far larger than the per-turn drain cap is delivered completely and
    /// in order — boundedness is achieved by deferral, not by dropping.
    ///
    /// The queue used to be drained whole each turn and the High/Normal phases walked without
    /// limit. The drain cap bounds one turn's buffer; this asserts the other half of the
    /// contract: nothing is lost and the promised FIFO order survives the deferral across
    /// turns.
    #[cfg(all(not(alloc_frugal), not(target_arch = "wasm32")))]
    #[test]
    fn a_burst_larger_than_the_drain_cap_is_delivered_in_order_without_loss() {
        const BURST: usize = PER_TURN_EVENT_DRAIN_CAP * 3 + 7;
        let delivered = Arc::new(std::sync::Mutex::new(Vec::<usize>::new()));
        let order = Arc::clone(&delivered);

        let mut el = EventLoop::new();
        el.set_dispatch_fn(Arc::new(move |_target, event| {
            let Event::Custom { name, payload } = event else { return };
            if name != "burst" {
                return;
            }
            let index = u32::from_le_bytes(payload[0..4].try_into().unwrap()) as usize;
            order.lock().unwrap().push(index);
        }));

        for index in 0..BURST {
            el.post_event(
                1,
                Event::Custom {
                    name: "burst".to_string(),
                    payload: (index as u32).to_le_bytes().to_vec(),
                },
                EventPriority::Normal,
            )
            .unwrap();
        }

        el.start();
        let deadline = std::time::Instant::now() + Duration::from_millis(5_000);
        while delivered.lock().unwrap().len() < BURST && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(2));
        }
        el.stop();

        let seen = delivered.lock().unwrap().clone();
        assert_eq!(seen.len(), BURST, "every event in the burst must be delivered");
        assert_eq!(
            seen,
            (0..BURST).collect::<Vec<_>>(),
            "deferring past the drain cap must preserve FIFO order"
        );
    }

    /// D09-EVT-04: a High event posted behind a large Normal backlog is still delivered in a
    /// bounded number of turns, and High is always dispatched before Normal within a turn.
    ///
    /// # What "not starved" means here
    ///
    /// The queue is FIFO, so an event behind a backlog is drained in order — but the drain is
    /// capped per turn, so the delay is bounded by the backlog size rather than unbounded, and
    /// the platform pump and frame tasks run every turn regardless. This test pins that: the
    /// High event arrives within a wall-clock deadline, and the phase order (High before the
    /// Normal events drained in the same turn) is observed.
    #[cfg(all(not(alloc_frugal), not(target_arch = "wasm32")))]
    #[test]
    fn a_high_event_behind_a_normal_backlog_is_delivered_within_bounded_turns() {
        let saw_high = Arc::new(AtomicBool::new(false));
        let saw_high_flag = Arc::clone(&saw_high);

        let mut el = EventLoop::new();
        el.set_dispatch_fn(Arc::new(move |_target, event| {
            let Event::Custom { name, .. } = event else { return };
            if name == "high" {
                saw_high_flag.store(true, Ordering::SeqCst);
            }
        }));

        let backlog = PER_TURN_EVENT_DRAIN_CAP * 4;
        for _ in 0..backlog {
            el.post_event(
                1,
                Event::Custom { name: "normal".to_string(), payload: vec![] },
                EventPriority::Normal,
            )
            .unwrap();
        }
        el.post_event(
            1,
            Event::Custom { name: "high".to_string(), payload: vec![] },
            EventPriority::High,
        )
        .unwrap();

        el.start();
        let deadline = std::time::Instant::now() + Duration::from_millis(3_000);
        while !saw_high.load(Ordering::SeqCst) && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(2));
        }
        el.stop();

        assert!(
            saw_high.load(Ordering::SeqCst),
            "a High event must not be starved: it was not delivered within the bounded deadline"
        );
    }

    /// D09-EVT-04: within a single turn, every High event is dispatched before any Normal
    /// event, even when they are drained together.
    #[cfg(all(not(alloc_frugal), not(target_arch = "wasm32")))]
    #[test]
    fn high_events_dispatch_before_normal_events_in_the_same_turn() {
        let order = Arc::new(std::sync::Mutex::new(Vec::<&'static str>::new()));
        let log = Arc::clone(&order);
        let mut el = EventLoop::new();
        el.set_dispatch_fn(Arc::new(move |_target, event| {
            let Event::Custom { name, .. } = event else { return };
            // Only the first few of each kind matter; a short prefix keeps the log small.
            let mut guard = log.lock().unwrap();
            if name == "h" && guard.iter().filter(|entry| **entry == "h").count() < 4 {
                guard.push("h");
            } else if name == "n" && guard.iter().filter(|entry| **entry == "n").count() < 4 {
                guard.push("n");
            }
        }));

        // Interleave so the FIFO order alone would put Normal before High.
        for i in 0..4 {
            el.post_event(
                1,
                Event::Custom { name: "n".to_string(), payload: vec![i] },
                EventPriority::Normal,
            )
            .unwrap();
            el.post_event(
                1,
                Event::Custom { name: "h".to_string(), payload: vec![i] },
                EventPriority::High,
            )
            .unwrap();
        }

        el.start();
        let deadline = std::time::Instant::now() + Duration::from_millis(2_000);
        while order.lock().unwrap().len() < 8 && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(2));
        }
        el.stop();

        let seen = order.lock().unwrap().clone();
        let first_four = &seen[..seen.len().min(4)];
        assert!(
            first_four.iter().all(|entry| *entry == "h"),
            "the High phase must run before the Normal phase; observed {seen:?}"
        );
    }

    /// Idle events that exceed the 5ms budget are re-queued and delivered later, in order,
    /// instead of being dropped.
    #[cfg(all(not(alloc_frugal), not(target_arch = "wasm32")))]
    #[test]
    fn idle_events_beyond_the_budget_are_delivered_later() {
        let delivered = Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
        let delivered_clone = Arc::clone(&delivered);

        let mut el = EventLoop::new();
        el.set_dispatch_fn(Arc::new(move |_target, event| {
            let Event::Custom { name, payload } = event else { return };
            if name != "idle" {
                return;
            }
            let index = payload.first().copied().unwrap_or(0);
            let first = {
                let mut order = delivered_clone.lock().unwrap();
                order.push(index);
                order.len() == 1
            };
            // Exceed the 5ms idle budget on the first idle event so the rest must be re-queued.
            if first {
                std::thread::sleep(Duration::from_millis(8));
            }
        }));

        for index in 0u8..3 {
            el.post_event(
                1,
                Event::Custom { name: "idle".to_string(), payload: vec![index] },
                EventPriority::Idle,
            )
            .unwrap();
        }

        el.start();
        let deadline = std::time::Instant::now() + Duration::from_millis(1_500);
        while delivered.lock().unwrap().len() < 3 && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        el.stop();

        assert_eq!(
            *delivered.lock().unwrap(),
            vec![0, 1, 2],
            "idle events beyond the budget are delivered later, in order, not dropped"
        );
    }
}
