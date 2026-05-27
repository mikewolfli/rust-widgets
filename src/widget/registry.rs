//! Widget registry for container child forwarding.
//!
//! Provides a mechanism for container widgets (Frame, TabWidget, etc.) to forward
//! Draw and EventHandler calls to child widgets identified by ObjectId.
//!
//! This bridges the gap between the ObjectId-based child tracking and the
//! trait-object-based rendering/event dispatch.
use crate::core::ObjectId;
use crate::event::Event;
use crate::render::RenderContext;
use std::collections::HashMap;

/// A registry that maps ObjectId to draw + event handler closures.
///
/// This allows container widgets to forward rendering and events to their
/// child widgets without requiring architectural changes to the event dispatch
/// system.
pub struct SimpleRegistry {
    entries: HashMap<ObjectId, (DrawClosure, EventClosure)>,
}

type DrawClosure = Box<dyn FnMut(&mut RenderContext) + Send>;
type EventClosure = Box<dyn FnMut(&Event) + Send>;

// SAFETY: SimpleRegistry only stores closures that are Send.
// It is used in single-threaded contexts via Rc<RefCell<...>>,
// but the Send bound allows use in Arc<Mutex<...>> contexts.
// The interior HashMap is only accessed via &mut self, providing
// exclusive access, so there is no data race risk.
unsafe impl Send for SimpleRegistry {}
unsafe impl Sync for SimpleRegistry {}

impl Default for SimpleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl SimpleRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Register a widget's draw and event handler by ObjectId.
    pub fn register<D, E>(&mut self, id: ObjectId, draw: D, event: E)
    where
        D: FnMut(&mut RenderContext) + Send + 'static,
        E: FnMut(&Event) + Send + 'static,
    {
        self.entries.insert(id, (Box::new(draw), Box::new(event)));
    }

    /// Remove a widget from the registry.
    pub fn unregister(&mut self, id: ObjectId) {
        self.entries.remove(&id);
    }

    /// Draw the widget identified by `id`. Returns true if found and drawn.
    pub fn draw_widget(&mut self, id: ObjectId, context: &mut RenderContext) -> bool {
        if let Some((draw_fn, _)) = self.entries.get_mut(&id) {
            (draw_fn)(context);
            true
        } else {
            false
        }
    }

    /// Forward event to widget identified by `id`. Returns true if found.
    pub fn forward_event(&mut self, id: ObjectId, event: &Event) -> bool {
        if let Some((_, event_fn)) = self.entries.get_mut(&id) {
            (event_fn)(event);
            true
        } else {
            false
        }
    }

    /// Check if an id is registered.
    pub fn contains(&self, id: ObjectId) -> bool {
        self.entries.contains_key(&id)
    }

    /// Returns the number of registered widgets.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
