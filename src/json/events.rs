//! Event handler mapping for JSON-declared `on_click` / `on_change` handlers.
//!
//! When a JSON node declares `"on_click": "handler_name"`, the string
//! `"handler_name"` is stored in the node's properties. After widget
//! instantiation, the [`EventHandlerMap`] connects those names to Rust
//! closures.

use std::cell::RefCell;
use std::collections::HashMap;

use crate::WidgetTriggerEvent;

/// Context passed to every event handler invocation.
pub struct EventHandlerContext {
    /// The raw trigger event that fired this handler.
    pub trigger: WidgetTriggerEvent,
    /// Opaque user data pointer (e.g. a `BoundJsonLayout` reference cast to `*mut c_void`).
    pub user_data: Option<*mut std::ffi::c_void>,
}

// SAFETY: EventHandlerContext is only used on the main thread.
unsafe impl Send for EventHandlerContext {}
unsafe impl Sync for EventHandlerContext {}

impl EventHandlerContext {
    /// Create a new event handler context.
    pub fn new(trigger: WidgetTriggerEvent) -> Self {
        Self {
            trigger,
            user_data: None,
        }
    }

    /// Attach opaque user data.
    pub fn with_user_data(mut self, data: *mut std::ffi::c_void) -> Self {
        self.user_data = Some(data);
        self
    }

    /// Safely access user data as a typed reference.
    ///
    /// Uses unsafe internally to cast the raw pointer, but presents a
    /// safe API. Returns `None` if no user data is set or the cast fails.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `T` matches the type that was
    /// originally stored via [`with_user_data`](Self::with_user_data).
    pub fn user_data<T>(&self) -> Option<&T> {
        let ptr = self.user_data?;
        // SAFETY: Caller guarantees type T matches the original data.
        unsafe { Some(&*(ptr as *const T)) }
    }

    /// Safely access user data as a typed mutable reference.
    ///
    /// Uses unsafe internally to cast the raw pointer, but presents a
    /// safe API. Returns `None` if no user data is set or the cast fails.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `T` matches the type that was
    /// originally stored via [`with_user_data`](Self::with_user_data).
    pub fn user_data_mut<T>(&mut self) -> Option<&mut T> {
        let ptr = self.user_data?;
        // SAFETY: Caller guarantees type T matches the original data.
        unsafe { Some(&mut *(ptr as *mut T)) }
    }
}

/// Named handler function signature.
pub type EventHandler = Box<dyn Fn(&EventHandlerContext)>;

/// Registry mapping JSON-declared handler names to Rust closures.
pub struct EventHandlerMap {
    handlers: HashMap<String, EventHandler>,
}

impl EventHandlerMap {
    /// Create an empty handler registry.
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    /// Register a named handler.
    pub fn register<F>(&mut self, name: impl Into<String>, f: F)
    where
        F: Fn(&EventHandlerContext) + 'static,
    {
        self.handlers.insert(name.into(), Box::new(f));
    }

    /// Invoke a handler by name.
    ///
    /// Returns `true` if the handler was found and executed, `false` if
    /// no handler with that name is registered (the event is silently ignored).
    pub fn invoke(&self, name: &str, ctx: &EventHandlerContext) -> bool {
        if let Some(handler) = self.handlers.get(name) {
            handler(ctx);
            true
        } else {
            false
        }
    }

    /// Check whether a handler name is registered.
    pub fn has_handler(&self, name: &str) -> bool {
        self.handlers.contains_key(name)
    }

    /// Remove a handler by name. Returns `true` if it existed.
    pub fn unregister(&mut self, name: &str) -> bool {
        self.handlers.remove(name).is_some()
    }

    /// Number of registered handlers.
    pub fn len(&self) -> usize {
        self.handlers.len()
    }

    /// Returns true if no handlers are registered.
    pub fn is_empty(&self) -> bool {
        self.handlers.is_empty()
    }

    /// Clear all registered handlers.
    pub fn clear(&mut self) {
        self.handlers.clear();
    }
}

impl Default for EventHandlerMap {
    fn default() -> Self {
        Self::new()
    }
}

// ── Global thread-local event handler map ──────────────────────

thread_local! {
    static GLOBAL_EVENT_HANDLERS: RefCell<EventHandlerMap> = RefCell::new(EventHandlerMap::new());
}

/// Register a global event handler.
pub fn register_global_handler<F>(name: impl Into<String>, f: F)
where
    F: Fn(&EventHandlerContext) + 'static,
{
    GLOBAL_EVENT_HANDLERS.with(|handlers| {
        handlers.borrow_mut().register(name, f);
    });
}

/// Invoke a global event handler by name.
pub fn invoke_global_handler(name: &str, ctx: &EventHandlerContext) -> bool {
    GLOBAL_EVENT_HANDLERS.with(|handlers| handlers.borrow().invoke(name, ctx))
}

/// Clear all registered global handlers.
pub fn clear_global_handlers() {
    GLOBAL_EVENT_HANDLERS.with(|handlers| {
        handlers.borrow_mut().clear();
    });
}
