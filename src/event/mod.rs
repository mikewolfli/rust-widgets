//! Event queue and dispatch.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::collections::HashMap;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender, TryRecvError, unbounded};

use crate::core::{ObjectId, Point, Size};
use crate::platform::{WidgetTriggerKind, get_platform};
use crate::signal::{ConnectionHandle, GenericSignal};

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

pub trait EventHandler {
    /// Handle a single dispatched event.
    fn handle_event(&mut self, event: &Event);
}

#[derive(Clone)]
pub struct EventSender {
    inner: Sender<(ObjectId, Event)>,
}

impl EventSender {
    /// Post event for a target object id.
    pub fn post(&self, object_id: ObjectId, event: Event) -> Result<(), crossbeam_channel::SendError<(ObjectId, Event)>> {
        self.inner.send((object_id, event))
    }
}

pub struct EventQueue {
    sender: EventSender,
    receiver: Receiver<(ObjectId, Event)>,
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

    pub fn sender(&self) -> EventSender {
        self.sender.clone()
    }

    /// Non-blocking single event poll.
    pub fn poll(&self) -> Option<(ObjectId, Event)> {
        self.receiver.try_recv().ok()
    }

    /// Drain currently available events and invoke callback for each.
    pub fn drain(&self, handler: &mut dyn FnMut(ObjectId, &Event)) {
        loop {
            match self.receiver.try_recv() {
                Ok((id, event)) => handler(id, &event),
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            }
        }
    }
}

pub struct EventLoop {
    queue: EventQueue,
    running: Arc<AtomicBool>,
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
        }
    }

    pub fn sender(&self) -> EventSender {
        self.queue.sender()
    }

    /// Start dispatch loop until `stop()` is called.
    pub fn start(&self, mut handler: impl FnMut(ObjectId, &Event) + Send + 'static) {
        self.running.store(true, Ordering::SeqCst);
        while self.running.load(Ordering::Relaxed) {
            self.queue.drain(&mut handler);
            thread::sleep(Duration::from_millis(8));
        }
    }

    /// Request event loop shutdown.
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}
