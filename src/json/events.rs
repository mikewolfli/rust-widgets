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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handler_map_new_is_empty() {
        let map = EventHandlerMap::new();
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
    }

    #[test]
    fn register_and_invoke_handler() {
        let mut map = EventHandlerMap::new();
        let invoked = std::rc::Rc::new(std::cell::Cell::new(false));
        let invoked_clone = invoked.clone();
        map.register("test_handler", move |_ctx| {
            invoked_clone.set(true);
        });
        assert!(map.has_handler("test_handler"));
        assert_eq!(map.len(), 1);
        let ctx = EventHandlerContext::new(crate::WidgetTriggerEvent {
            widget_id: 1,
            kind: crate::platform::WidgetTriggerKind::Clicked,
        });
        let result = map.invoke("test_handler", &ctx);
        assert!(result);
        assert!(invoked.get());
    }

    #[test]
    fn invoke_missing_handler_returns_false() {
        let map = EventHandlerMap::new();
        let ctx = EventHandlerContext::new(crate::WidgetTriggerEvent {
            widget_id: 1,
            kind: crate::platform::WidgetTriggerKind::Clicked,
        });
        assert!(!map.invoke("nonexistent", &ctx));
    }

    #[test]
    fn unregister_handler() {
        let mut map = EventHandlerMap::new();
        map.register("to_remove", |_ctx| {});
        assert!(map.has_handler("to_remove"));
        assert!(map.unregister("to_remove"));
        assert!(!map.has_handler("to_remove"));
        assert_eq!(map.len(), 0);
    }

    #[test]
    fn unregister_missing_returns_false() {
        let mut map = EventHandlerMap::new();
        assert!(!map.unregister("never_existed"));
    }

    #[test]
    fn clear_removes_all_handlers() {
        let mut map = EventHandlerMap::new();
        map.register("a", |_ctx| {});
        map.register("b", |_ctx| {});
        map.register("c", |_ctx| {});
        assert_eq!(map.len(), 3);
        map.clear();
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
    }

    #[test]
    fn handler_context_trigger_fields() {
        let id = 42;
        let trigger = crate::WidgetTriggerEvent {
            widget_id: id,
            kind: crate::platform::WidgetTriggerKind::ValueChanged,
        };
        let ctx = EventHandlerContext::new(trigger);
        assert_eq!(ctx.trigger.widget_id, id);
    }

    #[test]
    fn handler_context_with_user_data() {
        let trigger = crate::WidgetTriggerEvent {
            widget_id: 1,
            kind: crate::platform::WidgetTriggerKind::Clicked,
        };
        let mut data: i32 = 123;
        let ctx = EventHandlerContext::new(trigger)
            .with_user_data(&mut data as *mut i32 as *mut std::ffi::c_void);
        let retrieved: &i32 = ctx.user_data().unwrap();
        assert_eq!(*retrieved, 123);
    }

    #[test]
    fn handler_context_user_data_none_when_not_set() {
        let trigger = crate::WidgetTriggerEvent {
            widget_id: 1,
            kind: crate::platform::WidgetTriggerKind::Clicked,
        };
        let ctx = EventHandlerContext::new(trigger);
        assert!(ctx.user_data::<i32>().is_none());
    }

    #[test]
    fn handler_context_user_data_mut() {
        let trigger = crate::WidgetTriggerEvent {
            widget_id: 1,
            kind: crate::platform::WidgetTriggerKind::Clicked,
        };
        let mut data: i32 = 456;
        let mut ctx = EventHandlerContext::new(trigger)
            .with_user_data(&mut data as *mut i32 as *mut std::ffi::c_void);
        {
            let val: &mut i32 = ctx.user_data_mut().unwrap();
            *val = 789;
        }
        assert_eq!(data, 789);
    }

    #[test]
    fn default_is_empty() {
        let map = EventHandlerMap::default();
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
    }

    #[test]
    fn handler_passed_correct_trigger() {
        let mut map = EventHandlerMap::new();
        let captured_trigger = std::rc::Rc::new(std::cell::RefCell::new(
            None::<crate::platform::WidgetTriggerKind>,
        ));
        let captured_clone = captured_trigger.clone();
        map.register("capture", move |ctx| {
            *captured_clone.borrow_mut() = Some(ctx.trigger.kind);
        });
        let trigger = crate::WidgetTriggerEvent {
            widget_id: 7,
            kind: crate::platform::WidgetTriggerKind::SelectionChanged,
        };
        let ctx = EventHandlerContext::new(trigger);
        map.invoke("capture", &ctx);
        assert_eq!(
            captured_trigger.borrow().unwrap(),
            crate::platform::WidgetTriggerKind::SelectionChanged
        );
    }

    #[test]
    fn global_register_and_invoke() {
        clear_global_handlers();
        let invoked = std::rc::Rc::new(std::cell::Cell::new(false));
        let invoked_clone = invoked.clone();
        register_global_handler("global_test", move |_ctx| {
            invoked_clone.set(true);
        });
        let ctx = EventHandlerContext::new(crate::WidgetTriggerEvent {
            widget_id: 1,
            kind: crate::platform::WidgetTriggerKind::Clicked,
        });
        assert!(invoke_global_handler("global_test", &ctx));
        assert!(invoked.get());
    }

    #[test]
    fn global_missing_handler_returns_false() {
        clear_global_handlers();
        let ctx = EventHandlerContext::new(crate::WidgetTriggerEvent {
            widget_id: 1,
            kind: crate::platform::WidgetTriggerKind::Clicked,
        });
        assert!(!invoke_global_handler("not_registered", &ctx));
    }

    #[test]
    fn global_clear_removes_all() {
        clear_global_handlers();
        register_global_handler("g1", |_ctx| {});
        register_global_handler("g2", |_ctx| {});
        clear_global_handlers();
        let ctx = EventHandlerContext::new(crate::WidgetTriggerEvent {
            widget_id: 1,
            kind: crate::platform::WidgetTriggerKind::Clicked,
        });
        assert!(!invoke_global_handler("g1", &ctx));
        assert!(!invoke_global_handler("g2", &ctx));
    }
}
