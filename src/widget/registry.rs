// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Widget registry for container child forwarding.
//!
//! Provides a mechanism for container widgets (Frame, TabWidget, etc.) to forward
//! Draw and EventHandler calls to child widgets identified by ObjectId.
//!
//! This bridges the gap between the ObjectId-based child tracking and the
//! trait-object-based rendering/event dispatch.
use crate::core::{ObjectId, Rect};
use crate::event::Event;
use crate::render::RenderContext;
use std::collections::HashMap;

/// A registry that maps ObjectId to draw + event handler closures.
///
/// This allows container widgets to forward rendering and events to their
/// child widgets without requiring architectural changes to the event dispatch
/// system.
pub struct SimpleRegistry {
    entries: HashMap<ObjectId, RegistryEntry>,
}

type DrawClosure = Box<dyn FnMut(&mut RenderContext) + Send>;
type EventClosure = Box<dyn FnMut(&Event) + Send>;
type GeometryClosure = Box<dyn FnMut(Rect) + Send>;

struct RegistryEntry {
    draw: DrawClosure,
    event: EventClosure,
    geometry: Option<GeometryClosure>,
}

// SAFETY: All types contained within SimpleRegistry implement Send:
//   - HashMap<ObjectId, (DrawClosure, EventClosure)> where:
//       * ObjectId: wraps a u64 — trivially Send.
//       * DrawClosure = Box<dyn FnMut(&mut RenderContext) + Send>: Send because
//         the closure is bounded by +Send and Box<dyn ... + Send> auto-implements Send.
//       * EventClosure = Box<dyn FnMut(&Event) + Send>: same reasoning.
//   - HashMap<K, V> implements Send when K: Send and V: Send, which holds here.
//
// SimpleRegistry is used in single-threaded contexts via Rc<RefCell<...>>,
// but the Send bound allows use in Arc<Mutex<...>> contexts. The interior
// HashMap is only accessed via &mut self, providing exclusive access, so
// there is no data race risk.
//
// NOTE: We do NOT implement Sync because Box<dyn FnMut(...) + Send> is
// not Sync — FnMut is not Sync. The &self methods (contains, len, is_empty)
// do not invoke closures, so this is safe in practice, but implementing
// Sync would be unsound according to Rust's type system because &self
// access to the HashMap requires V: Sync.
unsafe impl Send for SimpleRegistry {}

crate::impl_default_via_new!(SimpleRegistry);

impl SimpleRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        Self { entries: HashMap::new() }
    }

    /// Register a widget's draw and event handler by ObjectId.
    pub fn register<D, E>(&mut self, id: ObjectId, draw: D, event: E)
    where
        D: FnMut(&mut RenderContext) + Send + 'static,
        E: FnMut(&Event) + Send + 'static,
    {
        self.entries.insert(
            id,
            RegistryEntry { draw: Box::new(draw), event: Box::new(event), geometry: None },
        );
    }

    /// Register a widget with an optional geometry synchronization callback.
    pub fn register_with_geometry<D, E, G>(&mut self, id: ObjectId, draw: D, event: E, geometry: G)
    where
        D: FnMut(&mut RenderContext) + Send + 'static,
        E: FnMut(&Event) + Send + 'static,
        G: FnMut(Rect) + Send + 'static,
    {
        self.entries.insert(
            id,
            RegistryEntry {
                draw: Box::new(draw),
                event: Box::new(event),
                geometry: Some(Box::new(geometry)),
            },
        );
    }

    /// Remove a widget from the registry.
    pub fn unregister(&mut self, id: ObjectId) {
        self.entries.remove(&id);
    }

    /// Draw the widget identified by `id`. Returns true if found and drawn.
    pub fn draw_widget(&mut self, id: ObjectId, context: &mut RenderContext) -> bool {
        if let Some(entry) = self.entries.get_mut(&id) {
            (entry.draw)(context);
            true
        } else {
            false
        }
    }

    /// Forward event to widget identified by `id`. Returns true if found.
    pub fn forward_event(&mut self, id: ObjectId, event: &Event) -> bool {
        if let Some(entry) = self.entries.get_mut(&id) {
            (entry.event)(event);
            true
        } else {
            false
        }
    }

    /// Synchronize a registered child geometry when it provided a callback.
    pub fn set_widget_geometry(&mut self, id: ObjectId, geometry: Rect) -> bool {
        if let Some(entry) = self.entries.get_mut(&id) {
            if let Some(callback) = entry.geometry.as_mut() {
                callback(geometry);
                return true;
            }
        }
        false
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
