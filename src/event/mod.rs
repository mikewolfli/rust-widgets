//! Event queue and dispatch.

pub mod queue;

use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{unbounded, Receiver, Sender};

use crate::control_backend::get_control_backend;
use crate::core::{ObjectId, Point, Size};
use crate::platform::{get_platform, WidgetTriggerEvent, WidgetTriggerKind};
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

impl Default for EventQueue {
    fn default() -> Self {
        Self::new()
    }
}

struct TimerEntry {
    target: ObjectId,
    interval: Duration,
    next_fire: Instant,
    repeat: bool,
    priority: EventPriority,
}

/// Focus manager for tracking and managing focus ownership.
pub struct FocusManager {
    focused_widget: Mutex<Option<ObjectId>>,
}

impl FocusManager {
    /// Create a new focus manager.
    pub fn new() -> Self {
        Self {
            focused_widget: Mutex::new(None),
        }
    }

    /// Set the focused widget.
    pub fn set_focus(&self, widget_id: ObjectId) -> bool {
        let mut focused = self
            .focused_widget
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if *focused == Some(widget_id) {
            return false;
        }
        *focused = Some(widget_id);
        true
    }

    /// Clear the focused widget.
    pub fn clear_focus(&self) -> bool {
        let mut focused = self
            .focused_widget
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if focused.is_none() {
            return false;
        }
        *focused = None;
        true
    }

    /// Get the currently focused widget.
    pub fn get_focused(&self) -> Option<ObjectId> {
        *self
            .focused_widget
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Check if a widget has focus.
    pub fn has_focus(&self, widget_id: ObjectId) -> bool {
        *self
            .focused_widget
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            == Some(widget_id)
    }
}

impl Default for FocusManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Pointer capture manager for tracking pointer capture ownership.
pub struct PointerCaptureManager {
    captured_widget: Mutex<Option<ObjectId>>,
}

impl PointerCaptureManager {
    /// Create a new pointer capture manager.
    pub fn new() -> Self {
        Self {
            captured_widget: Mutex::new(None),
        }
    }

    /// Set pointer capture to a widget.
    pub fn set_capture(&self, widget_id: ObjectId) -> bool {
        let mut captured = self
            .captured_widget
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if *captured == Some(widget_id) {
            return false;
        }
        *captured = Some(widget_id);
        true
    }

    /// Release pointer capture.
    pub fn release_capture(&self) -> Option<ObjectId> {
        let mut captured = self
            .captured_widget
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        captured.take()
    }

    /// Get the currently captured widget.
    pub fn get_captured(&self) -> Option<ObjectId> {
        *self
            .captured_widget
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Check if a widget has pointer capture.
    pub fn has_capture(&self, widget_id: ObjectId) -> bool {
        *self
            .captured_widget
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            == Some(widget_id)
    }
}

impl Default for PointerCaptureManager {
    fn default() -> Self {
        Self::new()
    }
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
    focus_manager: FocusManager,
    pointer_capture_manager: PointerCaptureManager,
}

/// Bridge that maps native platform triggers into Rust signal-slot callbacks.
///
/// This allows desktop-native widget ids to participate in a lightweight
/// signal pipeline without requiring a separate widget object registry.
pub struct NativeSignalBridge {
    widget_trigger_signals: Mutex<HashMap<(ObjectId, WidgetTriggerKind), GenericSignal>>,
    menu_trigger_signals: Mutex<HashMap<ObjectId, GenericSignal>>,
}

/// Trigger event source abstraction for native/custom control routing.
pub trait TriggerEventSource: Send + Sync {
    /// Poll next menu trigger from source.
    fn poll_menu_triggered(&self) -> Option<ObjectId>;
    /// Poll next typed widget trigger event from source.
    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent>;
}

/// Event source backed by active platform backend.
pub struct PlatformTriggerEventSource;

impl TriggerEventSource for PlatformTriggerEventSource {
    fn poll_menu_triggered(&self) -> Option<ObjectId> {
        get_platform().poll_menu_triggered()
    }

    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        get_platform().poll_widget_trigger_event()
    }
}

/// Event source backed by active control backend.
pub struct ControlBackendTriggerEventSource;

impl TriggerEventSource for ControlBackendTriggerEventSource {
    fn poll_menu_triggered(&self) -> Option<ObjectId> {
        get_control_backend().poll_menu_triggered()
    }

    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        get_control_backend().poll_widget_trigger_event()
    }
}

impl NativeSignalBridge {
    /// Create an empty bridge.
    pub fn new() -> Self {
        Self {
            widget_trigger_signals: Mutex::new(HashMap::new()),
            menu_trigger_signals: Mutex::new(HashMap::new()),
        }
    }

    /// Connect slot to one typed widget trigger route.
    pub fn connect_widget_trigger<F>(
        &self,
        widget_id: ObjectId,
        kind: WidgetTriggerKind,
        slot: F,
    ) -> ConnectionHandle
    where
        F: FnMut() + Send + Sync + 'static,
    {
        eprintln!("[NativeSignalBridge] connect_widget_trigger: widget_id={}, kind={:?}", widget_id, kind);
        let signal = {
            let mut map = self
                .widget_trigger_signals
                .lock()
                .expect("native bridge widget trigger lock poisoned");
            map.entry((widget_id, kind)).or_default().clone()
        };
        eprintln!("[NativeSignalBridge] connect_widget_trigger: connected");
        signal.connect(slot)
    }

    /// Connect slot to widget clicked trigger.
    pub fn connect_clicked<F>(&self, widget_id: ObjectId, slot: F) -> ConnectionHandle
    where
        F: FnMut() + Send + Sync + 'static,
    {
        self.connect_widget_trigger(widget_id, WidgetTriggerKind::Clicked, slot)
    }

    /// Connect slot to widget value-changed trigger.
    pub fn connect_value_changed<F>(&self, widget_id: ObjectId, slot: F) -> ConnectionHandle
    where
        F: FnMut() + Send + Sync + 'static,
    {
        self.connect_widget_trigger(widget_id, WidgetTriggerKind::ValueChanged, slot)
    }

    /// Connect slot to widget selection-changed trigger.
    pub fn connect_selection_changed<F>(&self, widget_id: ObjectId, slot: F) -> ConnectionHandle
    where
        F: FnMut() + Send + Sync + 'static,
    {
        self.connect_widget_trigger(widget_id, WidgetTriggerKind::SelectionChanged, slot)
    }

    /// Connect slot to widget/window closed trigger.
    pub fn connect_closed<F>(&self, widget_id: ObjectId, slot: F) -> ConnectionHandle
    where
        F: FnMut() + Send + Sync + 'static,
    {
        self.connect_widget_trigger(widget_id, WidgetTriggerKind::Closed, slot)
    }

    /// Connect slot to menu-item trigger.
    pub fn connect_menu_trigger<F>(&self, menu_item_id: ObjectId, slot: F) -> ConnectionHandle
    where
        F: FnMut() + Send + Sync + 'static,
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
        self.pump_once_with_source(&PlatformTriggerEventSource)
    }

    /// Poll active control backend once and emit mapped signals.
    pub fn pump_once_from_control_backend(&self) -> bool {
        self.pump_once_with_source(&ControlBackendTriggerEventSource)
    }

    /// Poll one source once and emit mapped signals.
    pub fn pump_once_with_source(&self, source: &dyn TriggerEventSource) -> bool {
        if let Some(menu_item_id) = source.poll_menu_triggered() {
            eprintln!("[NativeSignalBridge] pump_once: got menu event for item {}", menu_item_id);
            let signal = self
                .menu_trigger_signals
                .lock()
                .expect("native bridge menu lock poisoned")
                .get(&menu_item_id)
                .cloned();
            if let Some(signal) = signal {
                eprintln!("[NativeSignalBridge] pump_once: emitting menu signal");
                signal.emit();
                return true;
            } else {
                eprintln!("[NativeSignalBridge] pump_once: no signal connected for menu item {}", menu_item_id);
            }
        }

        if let Some(event) = source.poll_widget_trigger_event() {
            eprintln!("[NativeSignalBridge] pump_once: got widget event for widget {} kind {:?}", event.widget_id, event.kind);
            let signal: Option<GenericSignal> = if event.kind == WidgetTriggerKind::Unknown {
                None
            } else {
                self.widget_trigger_signals
                    .lock()
                    .expect("native bridge widget trigger lock poisoned")
                    .get(&(event.widget_id, event.kind))
                    .cloned()
            };

            if let Some(signal) = signal {
                eprintln!("[NativeSignalBridge] pump_once: emitting widget signal");
                signal.emit();
                return true;
            } else {
                eprintln!("[NativeSignalBridge] pump_once: no signal connected for widget {} kind {:?}", event.widget_id, event.kind);
            }
        }

        false
    }

    /// Poll active control backend repeatedly until no pending signal is emitted.
    pub fn pump_all_from_control_backend(&self) -> usize {
        let mut count = 0;
        while self.pump_once_from_control_backend() {
            count += 1;
        }
        count
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
            focus_manager: FocusManager::new(),
            pointer_capture_manager: PointerCaptureManager::new(),
        }
    }

    /// Get the focus manager.
    pub fn focus_manager(&self) -> &FocusManager {
        &self.focus_manager
    }

    /// Get the pointer capture manager.
    pub fn pointer_capture_manager(&self) -> &PointerCaptureManager {
        &self.pointer_capture_manager
    }

    /// Set focus to a widget.
    pub fn set_focus(&self, widget_id: ObjectId) -> bool {
        self.focus_manager.set_focus(widget_id)
    }

    /// Clear focus.
    pub fn clear_focus(&self) -> bool {
        self.focus_manager.clear_focus()
    }

    /// Get the currently focused widget.
    pub fn get_focused(&self) -> Option<ObjectId> {
        self.focus_manager.get_focused()
    }

    /// Set pointer capture to a widget.
    pub fn set_pointer_capture(&self, widget_id: ObjectId) -> bool {
        self.pointer_capture_manager.set_capture(widget_id)
    }

    /// Release pointer capture.
    pub fn release_pointer_capture(&self) -> Option<ObjectId> {
        self.pointer_capture_manager.release_capture()
    }

    /// Get the currently captured widget.
    pub fn get_pointer_capture(&self) -> Option<ObjectId> {
        self.pointer_capture_manager.get_captured()
    }

    /// Perform hit-test to find the widget at the given point.
    #[cfg(not(feature = "embedded"))]
    pub fn hit_test(&self, point: Point, registry: &crate::xml::WidgetRegistry, root_id: ObjectId) -> Option<ObjectId> {
        fn hit_recursive(point: Point, registry: &crate::xml::WidgetRegistry, widget_id: ObjectId) -> Option<ObjectId> {
            let widget = registry.widget(widget_id)?;
            if !widget.is_visible() || !widget.rect().contains_point(point) {
                return None;
            }
            for &child_id in widget.children() {
                if let Some(hit) = hit_recursive(point, registry, child_id) {
                    return Some(hit);
                }
            }
            Some(widget_id)
        }
        hit_recursive(point, registry, root_id)
    }

    /// Returns event sender associated with this event loop.
    pub fn sender(&self) -> EventSender {
        self.queue.sender()
    }
}

impl Default for EventLoop {
    fn default() -> Self {
        Self::new()
    }
}

impl EventLoop {

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
        while let Ok(envelope) = self.queue.receiver.try_recv() {
            self.enqueue(envelope.target, envelope.event, envelope.priority);
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
        if let Some((mut id, event)) = self.poll_next_dispatch() {
            // Check if there's a pointer capture and the event is a pointer event
            if let Some(captured) = self.get_pointer_capture() {
                match event {
                    Event::MouseMove { .. }
                    | Event::MousePress { .. }
                    | Event::MouseRelease { .. } => {
                        id = captured;
                    }
                    _ => {}
                }
            }
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
        let _ = sender.post(
            1,
            Event::Custom {
                name: "normal".to_string(),
                payload: vec![],
            },
        );
        let _ = sender.post_idle(
            1,
            Event::Custom {
                name: "idle".to_string(),
                payload: vec![],
            },
        );
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
        let _timer_id =
            loop_.register_timer(7, Duration::from_millis(1), false, EventPriority::Normal);

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

        let _ = sender.post(
            1,
            Event::Custom {
                name: "n1".to_string(),
                payload: vec![],
            },
        );

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
