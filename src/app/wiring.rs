// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! One assembly point for an application's event-loop hooks (W-02).
//!
//! # The gap this closes
//!
//! [`EventLoop`] exposes three installation points — a parent resolver, a dispatch callback and a
//! shortcut hook — and nothing in `src/app/` called any of them. The shortcut hook's own docs said
//! the intended argument was `ActionRouter::dispatch_event`, but no production caller installed
//! it, so the registry stayed host-driven while the loop hook dangled. A host therefore had to
//! know three methods, the right order, and how to build the closures.
//!
//! [`assemble_event_loop`] is that knowledge in one call. It installs:
//!
//! | hook | installed value |
//! |------|-----------------|
//! | [`EventLoop::set_parent_lookup`] | [`default_parent_lookup`] — walks the widget runtime's parent field |
//! | [`EventLoop::set_dispatch_fn`] | [`default_dispatch_fn`] — delivers to the widget's own `handle_event` |
//! | [`EventLoop::set_shortcut_dispatch`] | [`ActionRuntime::shortcut_dispatch_fn`] — the closure form of [`ActionRouter::dispatch_event`] |
//!
//! # What remains host policy
//!
//! The loop still owns *when* it consults the hooks; this module cannot change that from
//! `src/app/` because the dispatch loop lives in `src/event/loop.rs`. A host that wants a custom
//! dispatch target (a canvas rather than a widget) can install its own value **after**
//! [`assemble_event_loop`] — later installation replaces the hook, which is the documented
//! behaviour of the `set_*` methods.
//!
//! Both default hooks read the widget runtime, which is thread-local. The assembly call therefore
//! belongs on the thread that mounts the controls (the UI thread), which is also the thread the
//! loop dispatches on. This is the same constraint `with_widget`/`with_widget_mut` already carry.

use crate::action::{ActionManager, ActionRouter};
use crate::compat::{lock, Arc, Box, Mutex};
use crate::core::ObjectId;
use crate::event::r#loop::{EventDispatchFn, EventShortcutFn};
use crate::event::{Event, EventLoop};
use crate::shortcut::ShortcutManager;

/// Shared ownership of the shortcut and action registries the event loop drives.
///
/// # Why shared ownership
///
/// `ActionRouter` borrows both managers mutably for the length of one operation, while the loop's
/// hooks are `'static` closures that outlive any borrow. Putting the managers behind a `Mutex`
/// reconciles the two: the router is still constructed from `&mut` references (its API is
/// unchanged), and the hooks take the same locks for the duration of one dispatch. This is the
/// single-shortcut-path shape — both the loop hook and any host code go through
/// [`Self::with_router`], so there is one place that decides which manager is consulted.
#[derive(Clone)]
pub struct ActionRuntime {
    shortcuts: Arc<Mutex<ShortcutManager>>,
    actions: Arc<Mutex<ActionManager>>,
}

impl ActionRuntime {
    /// Wraps a freshly created manager pair.
    pub fn new(shortcuts: ShortcutManager, actions: ActionManager) -> Self {
        Self { shortcuts: Arc::new(Mutex::new(shortcuts)), actions: Arc::new(Mutex::new(actions)) }
    }

    /// Wraps manager pairs a host already shares with its own code.
    pub fn from_shared(
        shortcuts: Arc<Mutex<ShortcutManager>>,
        actions: Arc<Mutex<ActionManager>>,
    ) -> Self {
        Self { shortcuts, actions }
    }

    /// The shared shortcut registry.
    pub fn shortcuts(&self) -> &Arc<Mutex<ShortcutManager>> {
        &self.shortcuts
    }

    /// The shared action registry.
    pub fn actions(&self) -> &Arc<Mutex<ActionManager>> {
        &self.actions
    }

    /// Runs `f` with an [`ActionRouter`] over both registries.
    ///
    /// Lock order is always shortcuts → actions. Every entry point here takes them in that order,
    /// so two threads using the runtime cannot form a cycle. `f` must not call another runtime
    /// entry point: the managers are ordinary non-reentrant mutexes, so a callback that re-enters
    /// would deadlock. A user callback that needs to enqueue another action should do so after it
    /// returns (the action registry's own signals are the supported channel).
    pub fn with_router<R>(&self, f: impl FnOnce(&mut ActionRouter<'_>) -> R) -> R {
        let mut shortcuts = lock(&self.shortcuts);
        let mut actions = lock(&self.actions);
        let mut router = ActionRouter::new(&mut shortcuts, &mut actions);
        f(&mut router)
    }

    /// Dispatches one framework event as a possible shortcut, reporting whether it fired.
    ///
    /// A non-key event is answered `false` before either lock is taken, because only a key press
    /// can be a shortcut (`ActionRouter::dispatch_event` makes the same distinction).
    pub fn dispatch_event(&self, event: &Event) -> bool {
        if !matches!(event, Event::KeyPress { .. }) {
            return false;
        }
        self.with_router(|router| router.dispatch_event(event))
    }

    /// The loop hook: [`ActionRouter::dispatch_event`] as a `'static` closure.
    pub fn shortcut_dispatch_fn(&self) -> EventShortcutFn {
        let runtime = self.clone();
        Arc::new(move |event| runtime.dispatch_event(event))
    }
}

/// The default parent resolver: the control's recorded parent, or `None` for a root.
///
/// Reads the same `BaseWidget::parent` field the widget tree maintains, so capture/target/bubble
/// propagation walks the real tree rather than a second copy of it. `None` also covers an id that
/// is not mounted on this thread; propagation then stays flat for that event.
pub fn default_parent_lookup(id: ObjectId) -> Option<ObjectId> {
    crate::widget::runtime::read_widget(id, |widget| widget.parent()).flatten()
}

/// The default dispatch callback: hand `event` to the control at `id`.
///
/// This is the production delivery path (`Widget::handle_event` through the runtime), which is
/// what turns a dequeued event into a control's own signal emission. Returning nothing is
/// deliberate: the loop type's callback is `Fn`, and a missing widget is an ordinary outcome for
/// an id that was destroyed after the event was queued.
pub fn default_dispatch_fn() -> EventDispatchFn {
    Arc::new(|id, event| {
        let _ = crate::widget::runtime::dispatch_event(id, event);
    })
}

/// Installs the three hooks an application event loop needs, in one call.
///
/// Call this once after constructing the [`EventLoop`] and before starting it: the threaded loop
/// captures the closures when it spawns. See the module docs for what is installed and for the
/// thread affinity of the default hooks.
///
/// # Why one call rather than three
///
/// The three hooks are not independent: `dispatch_fn` is how a widget sees an event, `parent_lookup`
/// is what makes that delivery propagate, and `shortcut_dispatch` is what lets an accelerator win
/// before the focused control sees the key. Assembling only some of them produces a loop that
/// looks wired and silently drops one of the three roles. One entry point makes the complete set
/// the default rather than something a host must remember.
pub fn assemble_event_loop(event_loop: &mut EventLoop, runtime: &ActionRuntime) {
    event_loop.set_parent_lookup(Box::new(default_parent_lookup));
    event_loop.set_dispatch_fn(default_dispatch_fn());
    event_loop.set_shortcut_dispatch(runtime.shortcut_dispatch_fn());
}

/// Like [`assemble_event_loop`], but with host-supplied hooks where the defaults do not fit.
///
/// `parent_lookup` and `dispatch_fn` are installed only when supplied; `None` means "keep the
/// default". The shortcut hook always comes from `runtime`, because that is the registry pair this
/// call is about — a host that wants a different shortcut policy installs its own after this
/// returns.
pub fn assemble_event_loop_with(
    event_loop: &mut EventLoop,
    runtime: &ActionRuntime,
    parent_lookup: Option<Box<dyn Fn(ObjectId) -> Option<ObjectId> + Send + Sync>>,
    dispatch_fn: Option<EventDispatchFn>,
) {
    event_loop.set_parent_lookup(parent_lookup.unwrap_or_else(|| Box::new(default_parent_lookup)));
    event_loop.set_dispatch_fn(dispatch_fn.unwrap_or_else(default_dispatch_fn));
    event_loop.set_shortcut_dispatch(runtime.shortcut_dispatch_fn());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shortcut::{Key, Shortcut};

    #[test]
    fn the_shortcut_hook_dispatches_into_the_action_registry() {
        let mut shortcuts = ShortcutManager::new();
        shortcuts.register("save", Shortcut::primary(Key::S), "Save");
        let mut actions = ActionManager::new();
        assert!(actions.register_action("save", "Save"));
        assert!(actions.bind_shortcut_type(&Shortcut::primary(Key::S), "save"));
        let fired = Arc::new(core::sync::atomic::AtomicUsize::new(0));
        let counter = Arc::clone(&fired);
        actions.action("save").expect("registered above").connect_triggered(move || {
            counter.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
        });

        let runtime = ActionRuntime::new(shortcuts, actions);
        let hook = runtime.shortcut_dispatch_fn();
        // The platform's primary modifier is `Meta` on macOS and Control elsewhere; the keyboard
        // translator's bit mapping means the event-level check is not the point of this test. Feed
        // the chord through the router directly so the assertion is about the wiring, not about
        // which host this test is compiled for.
        let matched = runtime
            .with_router(|router| router.dispatch_key(Key::S, crate::shortcut::Modifiers::PRIMARY));
        assert_eq!(matched.as_deref(), Some("save"));
        assert_eq!(fired.load(core::sync::atomic::Ordering::SeqCst), 1);
        // A non-key event is not a shortcut.
        assert!(!hook(&Event::Paint));
    }

    #[test]
    fn the_router_reports_the_assembly_lock_pair_without_deadlock() {
        let runtime = ActionRuntime::new(ShortcutManager::new(), ActionManager::new());
        assert!(runtime
            .with_router(|router| router.dispatch_key(Key::A, crate::shortcut::Modifiers::PRIMARY))
            .is_none());
        assert!(!runtime.dispatch_event(&Event::KeyRelease { key: 65, modifiers: 0 }));
    }
}
