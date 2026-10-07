// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Event loop implementation.
use super::event_queue::{EventQueue, EventSender};
use super::timer::IdleTask;
use super::timer::TimerManager;
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

/// Routes an event through the gesture engine owned by its interaction target.
///
/// The loop used to share one [`GestureEngine`] across every target. Recognisers that remember
/// state — the double-tap's "first tap seen" — therefore leaked from one target to the next: a
/// single tap on target A followed by a single tap at the same coordinates on target B was reported
/// as a double-tap on B, whose own recogniser had never seen the first tap. Keeping one engine per
/// [`ObjectId`] scopes that state to the interaction owner. The registry is created on demand and
/// dropped when the loop thread ends, so stopping the loop clears all gesture state.
#[cfg(all(feature = "touch", not(alloc_frugal)))]
fn process_gesture(
    engines: &mut HashMap<ObjectId, GestureEngine>,
    target: ObjectId,
    event: &Event,
    now_ms: u64,
) -> Option<Event> {
    engines.entry(target).or_default().process(event, now_ms)
}

/// Canonical event name for animation frame requests.
/// Used instead of a string literal to avoid fragile string matching.
pub const ANIMATION_FRAME_EVENT_NAME: &str = "animation_frame";

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
    /// The Idle-task phase used to restart at index 0 every round and break on the 5ms
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
        let mut gesture_engines: HashMap<ObjectId, GestureEngine> = HashMap::new();
        let native_pump = self.native_pump.clone();
        // Idle tasks run on the loop thread, so they move with it. `mem::take` leaves the
        // field empty; a restart after `stop()` therefore begins with none, which is the
        // honest state — the caller re-registers what it still wants.
        let mut idle_tasks = core::mem::take(&mut self.idle_tasks);
        // The loop thread consults this state at dispatch time; it is shared rather than moved so
        // `cancel_animation_frame` (called from the host's thread) mutates the same one.
        let cancelled_anim_frames = Arc::clone(&self.cancelled_anim_frames);
        let idle_task_cursor = Arc::clone(&self.idle_task_cursor);
        let handle = thread::spawn(move || {
            while *lock(&running) {
                // Phase 0: Pump native platform events (e.g., Wayland dispatch)
                if let Some(ref pump) = native_pump {
                    pump();
                }
                // Phase 0a: Drain any pending scheduled tasks
                crate::event::types::drain_tasks();
                // Phase 1a: Drain all available events into a buffer so we can
                // dispatch them in strict priority order (High > Normal > Idle).
                let mut had_work = false;
                let mut priority_buffer: Vec<(ObjectId, Event, EventPriority)> = Vec::new();
                let mut idle_events: Vec<(ObjectId, Event)> = Vec::new();
                while let Some(entry) = lock(&queue).dequeue() {
                    had_work = true;
                    priority_buffer.push(entry);
                }

                // Phase 1b: Dispatch High-priority events first.
                for (target, event, priority) in &priority_buffer {
                    if *priority != EventPriority::High {
                        continue;
                    }

                    #[cfg(feature = "touch")]
                    let maybe_gesture_event = if feeds_the_gesture_engine(event) {
                        process_gesture(&mut gesture_engines, *target, event, now_ms())
                    } else {
                        None
                    };

                    if let Some(ref dispatch) = dispatch_fn {
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            dispatch(*target, event);
                            #[cfg(feature = "touch")]
                            if let Some(ref gesture) = maybe_gesture_event {
                                dispatch(*target, gesture);
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
                }

                // Phase 1c: Dispatch Normal-priority events second.
                for (target, event, priority) in &priority_buffer {
                    if *priority != EventPriority::Normal {
                        continue;
                    }

                    // A cancelled animation frame is discarded here, before any dispatch, so a
                    // host that cancelled never sees the callback. See
                    // `cancel_animation_frame` for why this is checked at dispatch rather than
                    // removed from the queue. Every delivered id is also marked dispatched so a
                    // later cancel of the same handle reports `false`.
                    let anim_id = animation_frame_id(event);
                    let skip = match anim_id {
                        Some(id) => lock(&cancelled_anim_frames).dispatched_and_cancelled(id),
                        None => false,
                    };
                    if skip {
                        continue;
                    }

                    #[cfg(feature = "touch")]
                    let maybe_gesture_event = if feeds_the_gesture_engine(event) {
                        process_gesture(&mut gesture_engines, *target, event, now_ms())
                    } else {
                        None
                    };

                    if let Some(ref dispatch) = dispatch_fn {
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            dispatch(*target, event);
                            #[cfg(feature = "touch")]
                            if let Some(ref gesture) = maybe_gesture_event {
                                dispatch(*target, gesture);
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
                }

                // Phase 1d: Buffer Idle events for budgeted processing.
                for (target, event, priority) in priority_buffer {
                    if priority == EventPriority::Idle {
                        idle_events.push((target, event));
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
                        // Same cancellation rule the Normal phase applies; an animation frame may
                        // be posted at either priority.
                        let anim_id = animation_frame_id(event);
                        let skip = match anim_id {
                            Some(id) => lock(&cancelled_anim_frames).dispatched_and_cancelled(id),
                            None => false,
                        };
                        if skip {
                            processed += 1;
                            continue;
                        }
                        #[cfg(feature = "touch")]
                        let maybe_gesture_event = if feeds_the_gesture_engine(event) {
                            process_gesture(&mut gesture_engines, *target, event, now_ms())
                        } else {
                            None
                        };

                        if let Some(ref dispatch) = dispatch_fn {
                            let result =
                                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                    dispatch(*target, event);
                                    #[cfg(feature = "touch")]
                                    if let Some(ref gesture) = maybe_gesture_event {
                                        dispatch(*target, gesture);
                                    }
                                }));
                            if let Err(e) = result {
                                log::error!("[event-loop] Dispatch panicked: {e:?}");
                            }
                        } else {
                            log::warn!(
                                "[event-loop] No dispatch_fn set — dropping idle event {event:?} for target {target:?}"
                            );
                        }
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
    /// This is the synchronous counterpart of the threaded loop's Normal phase, and it
    /// honours the same animation-frame contract: an event carrying a cancelled frame id
    /// is discarded before dispatch and its cancellation entry is consumed, exactly as the
    /// threaded loop does.
    #[cfg(alloc_frugal)]
    pub fn pump_once(&mut self) -> bool {
        if !self.is_running() {
            return false;
        }
        self.timer_manager.pump();
        let next = lock(&self.queue).dequeue();
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
        let event = Event::Custom {
            name: ANIMATION_FRAME_EVENT_NAME.to_string(),
            payload: id.to_le_bytes().to_vec(),
        };
        self.post_event(target, event, EventPriority::Normal)?;
        // Record the request as pending so a subsequent cancel can tell a live request from
        // one that was already dispatched (or never issued through this loop).
        lock(&self.cancelled_anim_frames).request(id);
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

    /// Gesture recogniser state is scoped to the interaction target: a tap on one target must
    /// not arm a double-tap on another.
    #[test]
    #[cfg(all(feature = "touch", not(alloc_frugal)))]
    fn gesture_state_is_scoped_to_its_interaction_target() {
        use crate::compat::HashMap as CompatHashMap;
        use crate::gesture::GestureEngine as Gesture;

        let mut engines: CompatHashMap<ObjectId, Gesture> = CompatHashMap::new();

        // Target 1: a single tap produces a Tap.
        let _ = process_gesture(&mut engines, 1, &Event::touch_begin(10, 10, 1), 0);
        let first = process_gesture(&mut engines, 1, &Event::touch_end(10, 10, 1), 100);
        assert!(matches!(first, Some(Event::Tap { .. })), "a single tap on target 1 is a Tap");

        // Target 2: the same tap must not inherit target 1's first tap into a DoubleTap.
        let _ = process_gesture(&mut engines, 2, &Event::touch_begin(10, 10, 2), 200);
        let second = process_gesture(&mut engines, 2, &Event::touch_end(10, 10, 2), 250);
        assert!(
            !matches!(second, Some(Event::DoubleTap { .. })),
            "a first tap on target 2 must not inherit target 1's tap"
        );
        assert!(matches!(second, Some(Event::Tap { .. })), "target 2's own tap is still a Tap");

        // A second tap on target 2, within the window, does complete a double-tap.
        let _ = process_gesture(&mut engines, 2, &Event::touch_begin(10, 10, 3), 300);
        let third = process_gesture(&mut engines, 2, &Event::touch_end(10, 10, 3), 350);
        assert!(
            matches!(third, Some(Event::DoubleTap { .. })),
            "two taps on the same target still complete a double tap"
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
