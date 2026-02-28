//! Event queue and dispatch.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender, TryRecvError, unbounded};

use crate::core::{ObjectId, Point, Size};

#[derive(Debug, Clone)]
pub enum Event {
    MouseMove { pos: Point },
    MousePress { pos: Point, button: u32 },
    MouseRelease { pos: Point, button: u32 },
    MouseEnter { pos: Point },
    MouseLeave { pos: Point },
    KeyPress { key: u32, modifiers: u32 },
    KeyRelease { key: u32, modifiers: u32 },
    Paint,
    Resize { size: Size },
    Timer { id: u32 },
    Custom { name: String, payload: Vec<u8> },
    Quit,
}

pub trait EventHandler {
    fn handle_event(&mut self, event: &Event);
}

#[derive(Clone)]
pub struct EventSender {
    inner: Sender<(ObjectId, Event)>,
}

impl EventSender {
    pub fn post(&self, object_id: ObjectId, event: Event) -> Result<(), crossbeam_channel::SendError<(ObjectId, Event)>> {
        self.inner.send((object_id, event))
    }
}

pub struct EventQueue {
    sender: EventSender,
    receiver: Receiver<(ObjectId, Event)>,
}

impl EventQueue {
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

    pub fn poll(&self) -> Option<(ObjectId, Event)> {
        self.receiver.try_recv().ok()
    }

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

impl EventLoop {
    pub fn new() -> Self {
        Self {
            queue: EventQueue::new(),
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn sender(&self) -> EventSender {
        self.queue.sender()
    }

    pub fn start(&self, mut handler: impl FnMut(ObjectId, &Event) + Send + 'static) {
        self.running.store(true, Ordering::SeqCst);
        while self.running.load(Ordering::Relaxed) {
            self.queue.drain(&mut handler);
            thread::sleep(Duration::from_millis(8));
        }
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}
