//! Event loop implementation.
use super::event_queue::{EventQueue, EventSender};
use super::timer::TimerManager;
use super::types::{Event, EventPriority};
use crate::compat::Mutex;
use crate::core::ObjectId;
#[cfg(all(feature = "touch", not(feature = "mini")))]
use crate::gesture::GestureEngine;
use alloc::sync::Arc;
use core::sync::atomic::AtomicU64;
use core::sync::atomic::Ordering;
use core::time::Duration;
#[cfg(not(feature = "mini"))]
use std::thread;
#[cfg(all(feature = "touch", not(feature = "mini")))]
use std::time::{SystemTime, UNIX_EPOCH};

/// Type alias for event dispatch function.
pub type EventDispatchFn = Arc<dyn Fn(ObjectId, &Event) + Send + Sync>;

/// Helper to recover from a poisoned mutex by extracting the inner value.
#[cfg(not(feature = "mini"))]
fn recover_lock<T>(
    e: std::sync::PoisonError<crate::compat::MutexGuard<'_, T>>,
) -> crate::compat::MutexGuard<'_, T> {
    e.into_inner()
}

/// Returns the current timestamp in milliseconds since UNIX epoch.
#[cfg(all(feature = "touch", not(feature = "mini")))]
fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

/// Canonical event name for animation frame requests.
/// Used instead of a string literal to avoid fragile string matching.
pub const ANIMATION_FRAME_EVENT_NAME: &str = "animation_frame";

/// A handle returned by `request_animation_frame` that can be used to cancel the request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnimationFrameRequest {
    /// Unique identifier for this animation frame request.
    pub id: u64,
}

/// Main event loop for processing events.
pub struct EventLoop {
    /// Event queue for processing.
    #[cfg_attr(feature = "mini", allow(dead_code))]
    // Only read by the non-mini `start()`; kept to mirror the desktop API.
    // Under mini the queue uses a single-threaded channel, so the Arc is not
    // Send/Sync — that is intentional for the mini (single-threaded) profile.
    #[cfg_attr(feature = "mini", allow(clippy::arc_with_non_send_sync))]
    queue: Arc<Mutex<EventQueue>>,
    /// Independent sender for posting events without locking the queue.
    /// Avoids deadlock with the event loop thread which holds the queue mutex
    /// while blocked on `dequeue_blocking()`.
    sender: EventSender,
    /// Shared flag indicating if the loop is running.
    running: Arc<Mutex<bool>>,
    /// Processing thread handle.
    #[cfg(not(feature = "mini"))]
    thread_handle: Option<thread::JoinHandle<()>>,
    /// Processing thread handle (mini stub).
    #[cfg(feature = "mini")]
    #[cfg_attr(feature = "mini", allow(dead_code))]
    // kept to mirror the non-mini API
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
}

impl EventLoop {
    /// Creates a new event loop.
    #[cfg_attr(feature = "mini", allow(clippy::arc_with_non_send_sync))]
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
        }
    }

    /// Starts the event loop in a separate thread.
    #[cfg(not(feature = "mini"))]
    pub fn start(&mut self) {
        if *self.running.lock().unwrap_or_else(recover_lock) {
            return;
        }
        *self.running.lock().unwrap_or_else(recover_lock) = true;
        let running = Arc::clone(&self.running);
        let queue = Arc::clone(&self.queue);
        let dispatch_fn = self.dispatch_fn.clone();
        #[cfg(feature = "touch")]
        let mut gesture_engine = GestureEngine::new();
        let native_pump = self.native_pump.clone();
        let handle = thread::spawn(move || {
            while *running.lock().unwrap_or_else(recover_lock) {
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
                while let Some(entry) = queue.lock().unwrap_or_else(recover_lock).dequeue() {
                    had_work = true;
                    priority_buffer.push(entry);
                }

                // Phase 1b: Dispatch High-priority events first.
                for (target, event, priority) in &priority_buffer {
                    if *priority != EventPriority::High {
                        continue;
                    }

                    #[cfg(feature = "touch")]
                    let maybe_gesture_event = if event.is_touch() {
                        gesture_engine.process(event, now_ms())
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

                    #[cfg(feature = "touch")]
                    let maybe_gesture_event = if event.is_touch() {
                        gesture_engine.process(event, now_ms())
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

                // Phase 1b: Process buffered idle events with a 5ms time budget.
                // This prevents idle processing from starving frame-critical work.
                if !idle_events.is_empty() {
                    #[cfg(not(feature = "mini"))]
                    let idle_budget_start = std::time::Instant::now();
                    for (target, event) in idle_events {
                        #[cfg(not(feature = "mini"))]
                        if idle_budget_start.elapsed().as_millis() >= 5 {
                            break; // budget exhausted, remaining idle events are dropped
                        }
                        #[cfg(feature = "touch")]
                        let maybe_gesture_event = if event.is_touch() {
                            gesture_engine.process(&event, now_ms())
                        } else {
                            None
                        };

                        if let Some(ref dispatch) = dispatch_fn {
                            let result =
                                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                    dispatch(target, &event);
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
                                "[event-loop] No dispatch_fn set — dropping idle event {event:?} for target {target:?}"
                            );
                        }
                    }
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
    #[cfg(feature = "mini")]
    pub fn start(&mut self) {
        *self.running.lock().unwrap_or_else(|p| p.into_inner()) = true;
    }

    /// Stops the event loop.
    #[cfg(not(feature = "mini"))]
    pub fn stop(&mut self) {
        *self.running.lock().unwrap_or_else(recover_lock) = false;
        self.timer_manager.clear();
        // Post a wake event so the event loop thread unblocks from
        // dequeue_blocking() and can observe the running flag.
        let _ =
            self.sender.post(0, Event::Custom { name: "__stop_wake".to_string(), payload: vec![] });
        if let Some(handle) = self.thread_handle.take() {
            if let Err(e) = handle.join() {
                log::error!("[event-loop] Thread join failed: {e:?}");
            }
        }
    }

    /// Stops the event loop (mini stub).
    #[cfg(feature = "mini")]
    pub fn stop(&mut self) {
        *self.running.lock().unwrap_or_else(|p| p.into_inner()) = false;
        self.timer_manager.clear();
    }

    /// Posts an event to the event loop.
    ///
    /// Uses an independent sender that does not lock the queue mutex,
    /// avoiding deadlock with the event loop thread (which holds the mutex
    /// while blocked on `dequeue_blocking()`).
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
    /// Returns an `AnimationFrameRequest` handle that can be used to identify or cancel
    /// the request. This is similar to `window.requestAnimationFrame()` in browsers.
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
        Ok(AnimationFrameRequest { id })
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
        #[cfg(not(feature = "mini"))]
        {
            *self.running.lock().unwrap_or_else(recover_lock)
        }
        #[cfg(feature = "mini")]
        {
            *self.running.lock().unwrap_or_else(|p| p.into_inner())
        }
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

impl Default for EventLoop {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::types::Event;
    use crate::event::EventPriority;
    use crate::event::EventQueue;
    #[cfg(not(feature = "mini"))]
    use alloc::sync::Arc;
    #[cfg(not(feature = "mini"))]
    use core::sync::atomic::{AtomicBool, Ordering};

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

    #[cfg(all(not(feature = "mini"), not(target_arch = "wasm32")))]
    #[test]
    fn test_native_pump_called_on_empty_queue() {
        let mut el = EventLoop::new();
        let pump_called = Arc::new(AtomicBool::new(false));
        let pump_called_clone = pump_called.clone();

        el.set_native_pump(Box::new(move || {
            pump_called_clone.store(true, Ordering::SeqCst);
        }));

        el.start();
        #[cfg(not(feature = "mini"))]
        std::thread::sleep(std::time::Duration::from_millis(50));
        el.stop();

        assert!(
            pump_called.load(Ordering::SeqCst),
            "native pump should have been called during loop iteration"
        );
    }

    #[cfg(all(not(feature = "mini"), not(target_arch = "wasm32")))]
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
        #[cfg(not(feature = "mini"))]
        std::thread::sleep(Duration::from_millis(150));
        el.stop();

        assert!(
            timer_fired.load(Ordering::SeqCst),
            "timer should have fired and been dispatched through the event loop"
        );
    }

    #[cfg(all(not(feature = "mini"), not(target_arch = "wasm32")))]
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
        #[cfg(not(feature = "mini"))]
        std::thread::sleep(Duration::from_millis(100));
        el.stop();

        assert!(
            anim_fired.load(Ordering::SeqCst),
            "animation frame event should have been dispatched through the event loop"
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
        #[cfg(not(feature = "mini"))]
        std::thread::sleep(Duration::from_millis(30));
        el.stop();
    }
}
