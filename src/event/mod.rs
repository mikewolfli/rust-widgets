//! Event queue and dispatch.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::atomic::AtomicU32;
use std::sync::Arc;
use std::collections::HashMap;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, TryRecvError, unbounded};

use crate::core::{ObjectId, Point, Size};
use crate::platform::{WidgetTriggerKind, get_platform};
use crate::signal::{ConnectionHandle, GenericSignal};

/// Event payload variants routed through the event loop.
#[derive(Debug, Clone)]
pub enum Event {
    /// Pointer moved inside active surface.
    MouseMove { pos: Point },
    /// Pointer/button press.
    MousePress { pos: Point, button: u32 },
    /// Pointer/button release.
    MouseRelease { pos: Point, button: u32 },
    /// Pointer entered widget bounds.
    MouseEnter { pos: Point },
    /// Pointer left widget bounds.
    MouseLeave { pos: Point },
    /// Keyboard key press.
    KeyPress { key: u32, modifiers: u32 },
    /// Keyboard key release.
    KeyRelease { key: u32, modifiers: u32 },
    /// Repaint request.
    Paint,
    /// Resize notification.
    Resize { size: Size },
    /// Timer fired.
    Timer { id: u32 },
    /// Free-form custom event payload.
    Custom { name: String, payload: Vec<u8> },
    /// Event loop shutdown signal.
    Quit,
}

/// Trait implemented by event targets.
pub trait EventHandler {
    /// Handle a single dispatched event.
    fn handle_event(&mut self, event: &Event);
}

/// Scheduling priority for queued events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventPriority {
    /// High-priority events (e.g. quit/system control).
    High,
    /// Normal-priority events (default user/application traffic).
    Normal,
    /// Idle-priority events, delivered when no higher-priority events exist.
    Idle,
}

#[derive(Debug, Clone)]
struct EventEnvelope {
    target: ObjectId,
    event: Event,
    priority: EventPriority,
}

/// Sender handle used to enqueue events.
#[derive(Clone)]
pub struct EventSender {
    inner: Sender<EventEnvelope>,
}

impl EventSender {
    /// Post event for a target object id.
    pub fn post(&self, object_id: ObjectId, event: Event) -> Result<(), String> {
        self.post_with_priority(object_id, event, EventPriority::Normal)
    }

    /// Post event with explicit priority.
    pub fn post_with_priority(
        &self,
        object_id: ObjectId,
        event: Event,
        priority: EventPriority,
    ) -> Result<(), String> {
        self.inner
            .send(EventEnvelope {
                target: object_id,
                event,
                priority,
            })
            .map_err(|_| "event queue disconnected".to_string())
    }

    /// Post idle-priority event.
    pub fn post_idle(&self, object_id: ObjectId, event: Event) -> Result<(), String> {
        self.post_with_priority(object_id, event, EventPriority::Idle)
    }
}

/// Queue pair used by `EventLoop` internals.
pub struct EventQueue {
    sender: EventSender,
    receiver: Receiver<EventEnvelope>,
}

impl EventQueue {
    /// Create unbounded queue and sender/receiver pair.
    pub fn new() -> Self {
        let (tx, rx) = unbounded();
        Self {
            sender: EventSender { inner: tx },
            receiver: rx,
        }
    }

    /// Returns a cloneable sender handle for posting events.
    pub fn sender(&self) -> EventSender {
        self.sender.clone()
    }

}

struct TimerEntry {
    target: ObjectId,
    interval: Duration,
    next_fire: Instant,
    repeat: bool,
    priority: EventPriority,
}

/// Cooperative event loop with priority queues and timers.
pub struct EventLoop {
    queue: EventQueue,
    running: Arc<AtomicBool>,
    high: Mutex<VecDeque<(ObjectId, Event)>>,
    normal: Mutex<VecDeque<(ObjectId, Event)>>,
    idle: Mutex<VecDeque<(ObjectId, Event)>>,
    timers: Mutex<HashMap<u32, TimerEntry>>,
    next_timer_id: AtomicU32,
}

/// Bridge that maps native platform triggers into Rust signal-slot callbacks.
///
/// This allows desktop-native widget ids to participate in a lightweight
/// signal pipeline without requiring a separate widget object registry.
pub struct NativeSignalBridge {
    clicked_signals: Mutex<HashMap<ObjectId, GenericSignal>>,
    value_changed_signals: Mutex<HashMap<ObjectId, GenericSignal>>,
    menu_trigger_signals: Mutex<HashMap<ObjectId, GenericSignal>>,
}

impl NativeSignalBridge {
    /// Create an empty bridge.
    pub fn new() -> Self {
        Self {
            clicked_signals: Mutex::new(HashMap::new()),
            value_changed_signals: Mutex::new(HashMap::new()),
            menu_trigger_signals: Mutex::new(HashMap::new()),
        }
    }

    /// Connect slot to widget clicked trigger.
    pub fn connect_clicked<F>(&self, widget_id: ObjectId, slot: F) -> ConnectionHandle
    where
        F: FnMut() + Send + 'static,
    {
        let signal = {
            let mut map = self
                .clicked_signals
                .lock()
                .expect("native bridge clicked lock poisoned");
            map.entry(widget_id).or_default().clone()
        };
        signal.connect(slot)
    }

    /// Connect slot to widget value-changed trigger.
    pub fn connect_value_changed<F>(&self, widget_id: ObjectId, slot: F) -> ConnectionHandle
    where
        F: FnMut() + Send + 'static,
    {
        let signal = {
            let mut map = self
                .value_changed_signals
                .lock()
                .expect("native bridge value-changed lock poisoned");
            map.entry(widget_id).or_default().clone()
        };
        signal.connect(slot)
    }

    /// Connect slot to menu-item trigger.
    pub fn connect_menu_trigger<F>(&self, menu_item_id: ObjectId, slot: F) -> ConnectionHandle
    where
        F: FnMut() + Send + 'static,
    {
        let signal = {
            let mut map = self
                .menu_trigger_signals
                .lock()
                .expect("native bridge menu lock poisoned");
            map.entry(menu_item_id).or_default().clone()
        };
        signal.connect(slot)
    }

    /// Poll platform once and emit mapped signals.
    pub fn pump_once(&self) -> bool {
        if let Some(menu_item_id) = get_platform().poll_menu_triggered() {
            let signal = self
                .menu_trigger_signals
                .lock()
                .expect("native bridge menu lock poisoned")
                .get(&menu_item_id)
                .cloned();
            if let Some(signal) = signal {
                signal.emit();
                return true;
            }
        }

        if let Some(event) = get_platform().poll_widget_trigger_event() {
            let signal = match event.kind {
                WidgetTriggerKind::Clicked => self
                    .clicked_signals
                    .lock()
                    .expect("native bridge clicked lock poisoned")
                    .get(&event.widget_id)
                    .cloned(),
                WidgetTriggerKind::ValueChanged => self
                    .value_changed_signals
                    .lock()
                    .expect("native bridge value-changed lock poisoned")
                    .get(&event.widget_id)
                    .cloned(),
                WidgetTriggerKind::Unknown => None,
            };

            if let Some(signal) = signal {
                signal.emit();
                return true;
            }
        }

        false
    }

    /// Poll platform repeatedly until no pending signal is emitted.
    pub fn pump_all(&self) -> usize {
        let mut count = 0;
        while self.pump_once() {
            count += 1;
        }
        count
    }
}

impl Default for NativeSignalBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl EventLoop {
    /// Create event loop in stopped state.
    pub fn new() -> Self {
        Self {
            queue: EventQueue::new(),
            running: Arc::new(AtomicBool::new(false)),
            high: Mutex::new(VecDeque::new()),
            normal: Mutex::new(VecDeque::new()),
            idle: Mutex::new(VecDeque::new()),
            timers: Mutex::new(HashMap::new()),
            next_timer_id: AtomicU32::new(1),
        }
    }

    /// Returns event sender associated with this event loop.
    pub fn sender(&self) -> EventSender {
        self.queue.sender()
    }

    /// Register timer and return timer id.
    pub fn register_timer(
        &self,
        target: ObjectId,
        interval: Duration,
        repeat: bool,
        priority: EventPriority,
    ) -> u32 {
        let timer_id = self.next_timer_id.fetch_add(1, Ordering::Relaxed);
        self.timers
            .lock()
            .expect("event loop timer lock poisoned")
            .insert(
                timer_id,
                TimerEntry {
                    target,
                    interval: interval.max(Duration::from_millis(1)),
                    next_fire: Instant::now() + interval.max(Duration::from_millis(1)),
                    repeat,
                    priority,
                },
            );
        timer_id
    }

    /// Cancel timer by id.
    pub fn cancel_timer(&self, timer_id: u32) -> bool {
        self.timers
            .lock()
            .expect("event loop timer lock poisoned")
            .remove(&timer_id)
            .is_some()
    }

    fn enqueue(&self, target: ObjectId, event: Event, priority: EventPriority) {
        match priority {
            EventPriority::High => self
                .high
                .lock()
                .expect("event loop high queue lock poisoned")
                .push_back((target, event)),
            EventPriority::Normal => self
                .normal
                .lock()
                .expect("event loop normal queue lock poisoned")
                .push_back((target, event)),
            EventPriority::Idle => self
                .idle
                .lock()
                .expect("event loop idle queue lock poisoned")
                .push_back((target, event)),
        }
    }

    fn poll_next_dispatch(&self) -> Option<(ObjectId, Event)> {
        if let Some(item) = self
            .high
            .lock()
            .expect("event loop high queue lock poisoned")
            .pop_front()
        {
            return Some(item);
        }
        if let Some(item) = self
            .normal
            .lock()
            .expect("event loop normal queue lock poisoned")
            .pop_front()
        {
            return Some(item);
        }
        self.idle
            .lock()
            .expect("event loop idle queue lock poisoned")
            .pop_front()
    }

    fn drain_incoming(&self) {
        loop {
            match self.queue.receiver.try_recv() {
                Ok(envelope) => self.enqueue(envelope.target, envelope.event, envelope.priority),
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            }
        }
    }

    fn pump_timers(&self) {
        let now = Instant::now();
        let mut fired = Vec::new();
        {
            let mut timers = self.timers.lock().expect("event loop timer lock poisoned");
            let mut to_remove = Vec::new();
            for (timer_id, timer) in timers.iter_mut() {
                if now >= timer.next_fire {
                    fired.push((*timer_id, timer.target, timer.priority));
                    if timer.repeat {
                        timer.next_fire = now + timer.interval;
                    } else {
                        to_remove.push(*timer_id);
                    }
                }
            }
            for timer_id in to_remove {
                timers.remove(&timer_id);
            }
        }

        for (timer_id, target, priority) in fired {
            self.enqueue(target, Event::Timer { id: timer_id }, priority);
        }
    }

    /// Pump one scheduling cycle, returns true if at least one event was dispatched.
    pub fn pump_once(&self, handler: &mut dyn FnMut(ObjectId, &Event)) -> bool {
        self.pump_timers();
        self.drain_incoming();
        if let Some((id, event)) = self.poll_next_dispatch() {
            handler(id, &event);
            true
        } else {
            false
        }
    }

    /// Start dispatch loop until `stop()` is called.
    pub fn start(&self, mut handler: impl FnMut(ObjectId, &Event) + Send + 'static) {
        self.running.store(true, Ordering::SeqCst);
        while self.running.load(Ordering::Relaxed) {
            let dispatched = self.pump_once(&mut handler);
            if !dispatched {
                thread::sleep(Duration::from_millis(2));
            }
        }
    }

    /// Run nested loop until predicate returns true.
    pub fn run_nested_until(
        &self,
        mut handler: impl FnMut(ObjectId, &Event) + Send + 'static,
        mut should_exit: impl FnMut() -> bool,
    ) {
        while !should_exit() && self.running.load(Ordering::Relaxed) {
            let dispatched = self.pump_once(&mut handler);
            if !dispatched {
                thread::sleep(Duration::from_millis(2));
            }
        }
    }

    /// Run modal loop scoped to modal target while preserving deferred events.
    pub fn run_modal_until(
        &self,
        modal_target: ObjectId,
        mut handler: impl FnMut(ObjectId, &Event) + Send + 'static,
        mut should_close: impl FnMut() -> bool,
    ) {
        let mut deferred = VecDeque::new();
        while !should_close() && self.running.load(Ordering::Relaxed) {
            self.pump_timers();
            self.drain_incoming();

            if let Some((target, event)) = self.poll_next_dispatch() {
                if target == modal_target || matches!(event, Event::Quit | Event::Timer { .. }) {
                    handler(target, &event);
                } else {
                    deferred.push_back((target, event));
                }
            } else {
                thread::sleep(Duration::from_millis(2));
            }
        }

        for (target, event) in deferred {
            self.enqueue(target, event, EventPriority::Normal);
        }
    }

    /// Request event loop shutdown.
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn priority_dispatch_order() {
        let loop_ = EventLoop::new();
        let sender = loop_.sender();
        let _ = sender.post(1, Event::Custom { name: "normal".to_string(), payload: vec![] });
        let _ = sender.post_idle(1, Event::Custom { name: "idle".to_string(), payload: vec![] });
        let _ = sender.post_with_priority(
            1,
            Event::Custom {
                name: "high".to_string(),
                payload: vec![],
            },
            EventPriority::High,
        );

        let mut seen = Vec::new();
        for _ in 0..3 {
            let _ = loop_.pump_once(&mut |_, event| {
                if let Event::Custom { name, .. } = event {
                    seen.push(name.clone());
                }
            });
        }
        assert_eq!(seen, vec!["high", "normal", "idle"]);
    }

    #[test]
    fn timer_event_fires() {
        let loop_ = EventLoop::new();
        let _timer_id = loop_.register_timer(7, Duration::from_millis(1), false, EventPriority::Normal);

        let mut got = false;
        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(50) {
            let dispatched = loop_.pump_once(&mut |target, event| {
                if target == 7 && matches!(event, Event::Timer { .. }) {
                    got = true;
                }
            });
            if got {
                break;
            }
            if !dispatched {
                thread::sleep(Duration::from_millis(1));
            }
        }
        assert!(got);
    }

    #[test]
    fn nested_loop_exits_by_predicate() {
        let loop_ = EventLoop::new();
        loop_.running.store(true, Ordering::SeqCst);
        let sender = loop_.sender();
        let exit_count = Arc::new(AtomicUsize::new(0));
        let exit_count_clone = Arc::clone(&exit_count);

        let _ = sender.post(1, Event::Custom { name: "n1".to_string(), payload: vec![] });

        loop_.run_nested_until(
            move |_, _| {
                exit_count_clone.fetch_add(1, Ordering::SeqCst);
            },
            || exit_count.load(Ordering::SeqCst) >= 1,
        );

        assert_eq!(exit_count.load(Ordering::SeqCst), 1);
        loop_.running.store(false, Ordering::SeqCst);
    }
}
