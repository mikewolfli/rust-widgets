// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Child-forwarding table for container widgets.
//!
//! # What this module is, and what its name must not be read as
//!
//! [`SimpleRegistry`] is a **closure forwarding table**, not an ownership
//! registry. It maps an [`ObjectId`] to a draw closure, an event closure and an
//! optional geometry callback, so a container can paint and dispatch to a child
//! it holds only by id.
//!
//! The crate has three different things whose names sound alike, and conflating
//! them is the defect this note exists to prevent:
//!
//! | Name | Answers | Owner |
//! |---|---|---|
//! | [`SimpleRegistry`] (here) | "how do I draw/forward to this id?" | the container that holds it |
//! | `WidgetFactory` (`widget::capability`) | "how do I construct control *name*?" | the crate; one shared table |
//! | `widget::runtime` registry | "where is the live widget with this id?" | the runtime, which owns mounting |
//!
//! Ownership of a live control therefore lives in `widget::runtime`, never in
//! this table: dropping a [`SimpleRegistry`] removes its forwarding closures and
//! does not destroy a widget. A reader looking for "the registry" that owns the
//! widget tree should start at `widget::runtime`.
use crate::compat::{Box, HashMap};
use crate::core::{ObjectId, Rect};
use crate::event::Event;
use crate::render::RenderContext;

/// A forwarding table that maps an [`ObjectId`] to draw + event closures.
///
/// Container widgets use it to reach children they track only by id, without
/// requiring the event-dispatch layer to know their concrete type. It is a
/// **closure table, not an ownership registry** — see the module docs for the
/// three registry-shaped concepts in this crate and which one owns a live widget.
///
/// The name is historical. Renaming it would break the public surface for a
/// change that is purely about reading, so the distinction is stated here
/// instead; treat every "registry" mention in a container's documentation as
/// this table rather than as the runtime's widget map.
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

/// Formats the table as its size.
///
/// The closures cannot be printed, and a caller that puts a container into a `Debug`
/// summary wants "how many children are forwarded" rather than the boxes. Implemented by
/// hand because a derive would require every closure type to be `Debug`, which no `FnMut`
/// is.
impl core::fmt::Debug for SimpleRegistry {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.debug_struct("SimpleRegistry").field("registered", &self.entries.len()).finish()
    }
}

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
