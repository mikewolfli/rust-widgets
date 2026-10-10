// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Event handler mapping for JSON-declared `on_click` / `on_change` handlers.
//!
//! When a JSON node declares `"on_click": "handler_name"`, the string
//! `"handler_name"` is stored in the node's properties. After widget
//! instantiation, the [`EventHandlerMap`] connects those names to Rust
//! closures.

use crate::compat::HashMap;
use core::cell::RefCell;

use crate::WidgetTriggerEvent;

/// Context passed to every event handler invocation.
pub struct EventHandlerContext {
    /// The raw trigger event that fired this handler.
    pub trigger: WidgetTriggerEvent,
    /// The event's **payload**, delivered alongside the trigger.
    ///
    /// # Why this exists
    ///
    /// A typed event (a slider's `value_changed`, an editor's `text_changed`) carries a value, and
    /// a handler that receives only a trigger cannot act on it — the earlier shape discarded the
    /// value, so the only way a handler could learn it was to re-read some *other* property (for a
    /// slider, its caption, which is not the value at all). That is the defect this field closes: a
    /// handler registered through `events: { "value_changed": … }` now receives exactly the value
    /// the control emitted, in the same [`CapabilityValue`] representation the capability/schema
    /// layer declares (rule #95). A payload-free event carries [`CapabilityValue::Null`], which is
    /// the honest "this event reported nothing" value rather than a fabricated zero.
    ///
    /// [`CapabilityValue`]: crate::widget::capability::CapabilityValue
    pub payload: crate::widget::capability::CapabilityValue,
    /// Opaque user data pointer (e.g. a `BoundJsonLayout` reference cast to `*mut c_void`).
    pub user_data: Option<*mut std::ffi::c_void>,
}

// SAFETY: raw pointers are `!Send + !Sync`, so these impls textually widen the
// auto-traits. They are here because the signal closures in `json/loader.rs`
// store a `ButtonHandle` callback that constructs an `EventHandlerContext` on
// the stack and passes it by reference to `invoke_global_handler`; that closure
// must satisfy the registry's `Send` bound.
//
// `Send`: the context never crosses threads. It is created inside the callback,
// used within that same call, and dropped before the callback returns.
//
// `Sync`: deliberately NOT implemented. Nothing in the library requires it, and
// `user_data<T>()` hands out `&T` derived from an unowned, untyped pointer. If
// the pointee is ever mutated through the unsafe `user_data_mut` path while
// another thread holds an `&T` from `user_data()`, that is a data race. Omitting
// `Sync` keeps that aliasing impossible to construct across threads and costs
// nothing, because no API asks for it.
unsafe impl Send for EventHandlerContext {}

impl EventHandlerContext {
    /// Create a new event handler context with a payload-free trigger.
    ///
    /// The payload is [`CapabilityValue::Null`], matching a `unit` event: the trigger happened and
    /// carried nothing. Call [`Self::with_payload`] to attach a typed value.
    pub fn new(trigger: WidgetTriggerEvent) -> Self {
        Self { trigger, payload: crate::widget::capability::CapabilityValue::Null, user_data: None }
    }

    /// Attach the event's payload.
    ///
    /// Used by the wiring path that receives a [`CapabilityValue`] from the control's dynamic
    /// signal, so the handler sees the value the control actually emitted rather than a default.
    ///
    /// [`CapabilityValue`]: crate::widget::capability::CapabilityValue
    pub fn with_payload(mut self, payload: crate::widget::capability::CapabilityValue) -> Self {
        self.payload = payload;
        self
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
        Self { handlers: HashMap::new() }
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
    ///
    /// # Re-entrancy
    ///
    /// This borrows the registry for the duration of the call, so a handler that mutates the map it
    /// is stored in must use [`Self::take`] (which the global registry does) rather than call this
    /// through a `RefCell`. The direct `&self` case has no `RefCell` to conflict with, so it stays a
    /// plain lookup-and-call.
    pub fn invoke(&self, name: &str, ctx: &EventHandlerContext) -> bool {
        if let Some(handler) = self.handlers.get(name) {
            handler(ctx);
            true
        } else {
            false
        }
    }

    /// Removes a handler by name and returns it, so a caller can run it **outside** any borrow of
    /// this map and re-insert it afterwards. Used by the global registry to let a handler register
    /// or clear other handlers without re-borrowing the same `RefCell`.
    pub fn take(&mut self, name: &str) -> Option<EventHandler> {
        self.handlers.remove(name)
    }

    /// Re-inserts a handler removed by [`Self::take`].
    ///
    /// Only overwrites an entry if the name is still absent: a handler that re-registered its own
    /// name while it was running has installed a newer closure, which must win over the older one
    /// being restored.
    pub fn restore(&mut self, name: impl Into<String>, handler: EventHandler) {
        self.handlers.entry(name.into()).or_insert(handler);
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

crate::impl_default_via_new!(EventHandlerMap);

// ── Global thread-local event handler map ──────────────────────

// `HashMap::new()` allocates, so `EventHandlerMap::new()` is not a `const fn` and this entry's
// initializer genuinely cannot be made const. Clippy's `missing_const_for_thread_local` fires
// only against the OpenHarmony std (the host toolchain does not raise it), and `#[allow]` applied
// to the `thread_local!` invocation itself is ignored — it has to sit on the inner `static`, which
// is why the attribute is written there rather than on the macro.
thread_local! {
    #[allow(clippy::missing_const_for_thread_local)]
    static GLOBAL_EVENT_HANDLERS: RefCell<EventHandlerMap> = RefCell::new(EventHandlerMap::new());

    /// Bumped by [`clear_global_handlers`], so an in-flight [`invoke_global_handler`] can tell that the
    /// registry was cleared while its handler ran and **not** re-insert the handler afterwards.
    ///
    /// # The defect this closes (BLUE-issue E-28)
    ///
    /// `invoke_global_handler` moves the running handler out of the map and restores it when the call
    /// returns. A handler that called `clear_global_handlers` — a layout tearing itself down from inside
    /// a callback — removed every *other* handler, but the `RestoreGuard` then put the running one back:
    /// "clear all" was honoured for everything except the handler asking for it, and its captured
    /// resources stayed alive. Comparing the generation captured at invoke time against the current one
    /// makes a clear invalidate the pending restore, so the running handler is dropped like the rest.
    #[allow(clippy::missing_const_for_thread_local)]
    static HANDLER_GENERATION: core::cell::Cell<u64> = const { core::cell::Cell::new(0) };
}

/// The current global-handler generation, used to detect a clear during a handler.
fn handler_generation() -> u64 {
    HANDLER_GENERATION.with(|generation| generation.get())
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
///
/// # Why the handler is taken out before it runs
///
/// A handler may legitimately re-enter this registry: registering another handler, or clearing the
/// layout's handlers when it tears a screen down. When the map was borrowed for the whole call —
/// `handlers.borrow().invoke(...)` — that re-entry hit `RefCell already borrowed` and panicked, so a
/// handler that registered anything crashed (BLUE-issue E-16). The entry is therefore moved out of
/// the map under a short mutable borrow, the borrow is dropped, the handler runs, and a guard
/// re-inserts it on both the normal return and an unwind.
///
/// The one case this does not support is a handler that invokes **its own name** while running: the
/// entry is out of the map, so that call reports `false` rather than recursing (which would recurse
/// without bound anyway).
pub fn invoke_global_handler(name: &str, ctx: &EventHandlerContext) -> bool {
    let taken = GLOBAL_EVENT_HANDLERS.with(|handlers| handlers.borrow_mut().take(name));
    let Some(handler) = taken else {
        return false;
    };
    // Capture the generation so a `clear_global_handlers` during the call can invalidate the restore.
    let generation_at_invoke = handler_generation();

    // Re-insert on both the normal return and an unwind, **unless** the registry was cleared while the
    // handler ran. A `clear` means "remove every registered handler", and the running handler is one of
    // them, so restoring it would contradict the call that just asked to clear it (BLUE-issue E-28).
    // `restore` will not clobber a handler the callback re-registered under this same name.
    struct RestoreGuard<'a> {
        name: &'a str,
        handler: Option<EventHandler>,
        generation_at_invoke: u64,
    }
    impl Drop for RestoreGuard<'_> {
        fn drop(&mut self) {
            let handler = match self.handler.take() {
                Some(handler) => handler,
                None => return,
            };
            if handler_generation() != self.generation_at_invoke {
                // The registry was cleared while this handler ran: the clear's `clear()` removed the
                // other handlers, and the generation bump invalidates this restore, so the running
                // handler is dropped too. Do not put it back.
                return;
            }
            GLOBAL_EVENT_HANDLERS
                .with(|handlers| handlers.borrow_mut().restore(self.name, handler));
        }
    }
    let guard = RestoreGuard { name, handler: Some(handler), generation_at_invoke };
    if let Some(handler) = guard.handler.as_ref() {
        handler(ctx);
    }
    true
}

/// Clear all registered global handlers.
///
/// # A clear during a handler also clears that handler (BLUE-issue E-28)
///
/// The clear bumps the handler generation **before** emptying the map. An
/// [`invoke_global_handler`] running on this same thread captured the previous generation and will not
/// re-insert its handler, so "clear all registered handlers" holds for the handler asking for the
/// clear too — the API's plain reading. A handler re-registered by name after the clear is a *new*
/// registration and survives, because it is inserted after the generation bump.
pub fn clear_global_handlers() {
    HANDLER_GENERATION.with(|generation| generation.set(generation.get().wrapping_add(1)));
    GLOBAL_EVENT_HANDLERS.with(|handlers| {
        handlers.borrow_mut().clear();
    });
}

// ── Handler thread affinity (BLUE-issue E-25) ──────────────────

// The global handler registry is thread-local, which is the right design for a GUI: a handler
// typically touches controls on the UI thread and must not run on a worker. The problem was not the
// thread-local itself but that the *dynamic bridge* never said so: a signal emitted from another
// thread looked up that thread's (empty) registry, found nothing, and the handler was skipped
// silently — indistinguishable from "no handler was ever registered". A caller could not tell a
// delivered binding from one that silently does nothing off-thread.

/// Number of handler invocations that were skipped because they fired on a thread other than the
/// one that bound them.
///
/// A process-wide counter rather than per-binding, because the question a host asks is "did anything
/// get silently dropped off-thread?", and because the binding id is already named in the `warn!`
/// that accompanies each increment.
static CROSS_THREAD_SKIPS: core::sync::atomic::AtomicUsize =
    core::sync::atomic::AtomicUsize::new(0);

/// Records that `handler` fired off its binding thread, reporting the fact once per occurrence.
///
/// Returns the new total, so a caller that wants to surface it (a diagnostics panel) has the count
/// without a second call. The `warn!` names the handler and both thread ids so the author can act;
/// it is emitted every time because a dropped event is not routine and a single summary at shutdown
/// would hide how often it happened.
pub fn record_cross_thread_skip(handler: &str, bound_thread: &str, fired_thread: &str) -> usize {
    log::warn!(
        "JSON handler `{handler}` was bound on thread {bound_thread} but fired on thread \
         {fired_thread}; the handler registry is thread-local, so the binding was skipped rather \
         than run cross-thread. Emit the control's signal on its owning thread, or move the \
         binding to the emitting thread."
    );
    CROSS_THREAD_SKIPS.fetch_add(1, core::sync::atomic::Ordering::SeqCst) + 1
}

/// How many dynamic JSON bindings have been skipped for firing off their binding thread.
pub fn cross_thread_skips() -> usize {
    CROSS_THREAD_SKIPS.load(core::sync::atomic::Ordering::SeqCst)
}

/// Resets the cross-thread skip counter. For tests and for a host that samples and clears.
pub fn reset_cross_thread_skips() {
    CROSS_THREAD_SKIPS.store(0, core::sync::atomic::Ordering::SeqCst);
}

/// A human-readable identifier for the current thread, used in the cross-thread diagnostic.
///
/// Prefers the thread's own name (a host that names its UI thread gets a useful message) and falls
/// back to the debug form of the id, which is always available and unique per thread. **Diagnostic
/// only** — it is not unique (two threads may share a name), so it must never be used to decide
/// identity (BLUE-issue E-26; use [`current_thread_id`] for that).
pub fn current_thread_name() -> alloc::string::String {
    // `std::thread` is available wherever this module is compiled: the JSON loader is gated on a
    // device profile, which implies `std`.
    let current = std::thread::current();
    match current.name() {
        Some(name) => alloc::string::String::from(name),
        None => alloc::format!("{:?}", current.id()),
    }
}

/// The identity of the current thread.
///
/// # Why a `ThreadId` and not a name (BLUE-issue E-26)
///
/// Rust does not require thread names to be unique. Two different threads both named `same-ui-name`
/// compared **equal** under a name-based affinity check, so a cross-thread emission from one was
/// mistaken for a same-thread delivery and the other thread's same-named handler ran. `ThreadId` is
/// assigned by the runtime and unique for the lifetime of the process, so it is the correct key for
/// "is this the thread the binding was made on?". The name is still reported in the diagnostic.
pub fn current_thread_id() -> std::thread::ThreadId {
    std::thread::current().id()
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
        let captured_trigger =
            std::rc::Rc::new(std::cell::RefCell::new(None::<crate::platform::WidgetTriggerKind>));
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

    /// A handler may register another handler while it runs, without a `RefCell` panic.
    ///
    /// # The defect this pins (BLUE-issue E-16)
    ///
    /// `invoke_global_handler` held `handlers.borrow()` across the user closure, so a handler that
    /// registered or cleared handlers hit `RefCell already borrowed` and panicked. The registry now
    /// takes the invoked handler out before running it, so re-entry sees a free `RefCell`.
    #[test]
    fn a_global_handler_may_register_another_handler_while_running() {
        clear_global_handlers();
        let inner_ran = std::rc::Rc::new(std::cell::Cell::new(false));
        let inner_flag = inner_ran.clone();
        register_global_handler("outer", move |_ctx| {
            // The exact re-entry the previous implementation panicked on. `Rc::clone` is a `&self`
            // operation, so the outer closure stays `Fn` while handing an owned clone inward.
            let inner_flag = inner_flag.clone();
            register_global_handler("inner", move |_ctx| {
                inner_flag.set(true);
            });
        });

        let ctx = EventHandlerContext::new(crate::WidgetTriggerEvent {
            widget_id: 1,
            kind: crate::platform::WidgetTriggerKind::Clicked,
        });
        assert!(invoke_global_handler("outer", &ctx), "the outer handler must run");
        assert!(
            invoke_global_handler("inner", &ctx),
            "the handler registered by the outer must exist"
        );
        assert!(inner_ran.get(), "and must be callable");
        clear_global_handlers();
    }

    /// A `clear` inside a handler also removes that handler, so "clear all" means all.
    ///
    /// # The contract this pins (BLUE-issue E-28)
    ///
    /// The invoked handler is moved out of the map while it runs and restored when the call returns.
    /// A handler that called `clear_global_handlers` used to have itself restored afterwards, so
    /// "clear all registered handlers" was true for every handler except the one asking for the clear
    /// — and its captured resources stayed alive. A clear now invalidates the pending restore, so the
    /// running handler is dropped with the rest. The second invoke therefore reports `false`.
    #[test]
    fn a_global_handler_may_clear_the_registry_while_running() {
        clear_global_handlers();
        let ran = std::rc::Rc::new(std::cell::Cell::new(false));
        let ran_flag = ran.clone();
        register_global_handler("clears", move |_ctx| {
            clear_global_handlers();
            ran_flag.set(true);
        });

        let ctx = EventHandlerContext::new(crate::WidgetTriggerEvent {
            widget_id: 1,
            kind: crate::platform::WidgetTriggerKind::Clicked,
        });
        assert!(invoke_global_handler("clears", &ctx), "the handler must run despite clearing");
        assert!(ran.get());
        // The clear ran *during* the handler and removed it too, so it must not be usable again.
        assert!(
            !invoke_global_handler("clears", &ctx),
            "a handler that cleared the registry must not be restored by the clear's own frame"
        );
        clear_global_handlers();
    }

    /// A handler re-registers itself by name after clearing; the new registration must survive.
    ///
    /// The clear's generation bump invalidates only the *pending restore* of the old handler, not a
    /// fresh `register_global_handler` made after the clear. A layout that clears and immediately
    /// re-installs a handler must end up with the new one.
    #[test]
    fn a_handler_reregistered_after_a_clear_survives() {
        clear_global_handlers();
        let calls = std::rc::Rc::new(std::cell::Cell::new(0u32));
        let calls_slot = calls.clone();
        register_global_handler("cycle", move |_ctx| {
            clear_global_handlers();
            let inner = calls_slot.clone();
            register_global_handler("cycle", move |_ctx| {
                inner.set(inner.get() + 1);
            });
            clear_global_handlers();
            let inner = calls_slot.clone();
            register_global_handler("cycle", move |_ctx| {
                inner.set(inner.get() + 1);
            });
        });

        let ctx = EventHandlerContext::new(crate::WidgetTriggerEvent {
            widget_id: 1,
            kind: crate::platform::WidgetTriggerKind::Clicked,
        });
        // First call runs the outer handler, which clears and installs a new `cycle`.
        assert!(invoke_global_handler("cycle", &ctx));
        // The new registration is present and callable (it clears/registers again, then increments).
        assert!(invoke_global_handler("cycle", &ctx), "the re-registered handler must survive");
        clear_global_handlers();
    }

    /// A handler that re-registers its **own name without clearing** must not be clobbered by the
    /// restore of the older handler.
    ///
    /// Acceptance case "避免恢复旧 handler 覆盖新注册": the running handler is moved out and restored
    /// when the call returns, but `restore` uses `entry(..).or_insert(..)`, so a handler installed under
    /// the same name while it ran wins. Without that rule the stale closure would overwrite the new one.
    #[test]
    fn a_same_name_reregistration_without_a_clear_is_not_clobbered() {
        clear_global_handlers();
        let which = std::rc::Rc::new(std::cell::Cell::new(0u8));
        // The first handler, when run, replaces itself with a second that records `2`.
        let which_first = which.clone();
        register_global_handler("slot", move |_ctx| {
            which_first.set(1);
            let which_second = which_first.clone();
            register_global_handler("slot", move |_ctx| {
                which_second.set(2);
            });
        });

        let ctx = EventHandlerContext::new(crate::WidgetTriggerEvent {
            widget_id: 1,
            kind: crate::platform::WidgetTriggerKind::Clicked,
        });
        assert!(invoke_global_handler("slot", &ctx), "the first handler runs");
        assert_eq!(which.get(), 1, "the first handler recorded itself");
        // The second invoke must reach the NEW handler, not the restored old one.
        assert!(invoke_global_handler("slot", &ctx));
        assert_eq!(
            which.get(),
            2,
            "the same-name re-registration must win over the restored older handler"
        );
        clear_global_handlers();
    }

    /// A handler that panics is restored on the unwind path too, so a later emit still reaches it.
    ///
    /// Acceptance case "panic/unwind": `RestoreGuard` is a `Drop` guard, so the restore decision runs
    /// on both the normal return and an unwind. With no clear the handler must come back; the restore is
    /// observed by invoking it again after catching the panic.
    #[test]
    fn a_panicking_handler_is_restored_on_the_unwind_path() {
        clear_global_handlers();
        let calls = std::rc::Rc::new(std::cell::Cell::new(0u32));
        let calls_slot = calls.clone();
        let should_panic = std::rc::Rc::new(std::cell::Cell::new(true));
        let panic_slot = should_panic.clone();
        register_global_handler("boom", move |_ctx| {
            calls_slot.set(calls_slot.get() + 1);
            if panic_slot.get() {
                panic!("deliberate unwind from a global handler");
            }
        });

        let ctx = EventHandlerContext::new(crate::WidgetTriggerEvent {
            widget_id: 1,
            kind: crate::platform::WidgetTriggerKind::Clicked,
        });
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            invoke_global_handler("boom", &ctx)
        }));
        assert!(caught.is_err(), "the handler must have unwound");
        assert_eq!(calls.get(), 1, "the handler ran once before unwinding");

        // The guard's Drop restored it on the unwind path; a later invoke must reach it.
        should_panic.set(false);
        assert!(
            invoke_global_handler("boom", &ctx),
            "a panicking handler must be restored so later emits still reach it"
        );
        assert_eq!(calls.get(), 2, "the restored handler ran again");
        clear_global_handlers();
    }

    /// A handler may invoke a **different** handler by name while it runs (nested invoke).
    ///
    /// Acceptance case "嵌套 invoke": the outer handler is out of the map while it runs, and the inner
    /// one is taken and restored independently. The inner handler must run and be restored; the outer
    /// must also be restored when the whole call returns.
    #[test]
    fn a_handler_may_invoke_a_nested_handler_while_running() {
        clear_global_handlers();
        let inner_calls = std::rc::Rc::new(std::cell::Cell::new(0u32));
        let inner_slot = inner_calls.clone();
        register_global_handler("inner", move |_ctx| {
            inner_slot.set(inner_slot.get() + 1);
        });

        let outer_calls = std::rc::Rc::new(std::cell::Cell::new(0u32));
        let outer_slot = outer_calls.clone();
        register_global_handler("outer", move |ctx| {
            outer_slot.set(outer_slot.get() + 1);
            // Nested invoke of a *different* name, while `outer` is out of the map.
            assert!(invoke_global_handler("inner", ctx), "the nested handler must run");
        });

        let ctx = EventHandlerContext::new(crate::WidgetTriggerEvent {
            widget_id: 1,
            kind: crate::platform::WidgetTriggerKind::Clicked,
        });
        assert!(invoke_global_handler("outer", &ctx));
        assert_eq!(outer_calls.get(), 1, "the outer handler ran once");
        assert_eq!(inner_calls.get(), 1, "the nested inner handler ran once");
        // Both were restored: the inner by its own guard, the outer by its guard on return. The second
        // outer invoke nests the inner again (inner goes to 2), then an explicit inner invoke makes 3 —
        // which proves the inner handler was restored rather than lost.
        assert!(invoke_global_handler("outer", &ctx));
        assert!(invoke_global_handler("inner", &ctx));
        assert_eq!(outer_calls.get(), 2);
        assert_eq!(inner_calls.get(), 3, "inner: 1 nested + 1 nested again + 1 explicit");
        clear_global_handlers();
    }
}
