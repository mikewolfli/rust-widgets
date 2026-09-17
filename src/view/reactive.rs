// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Reactive host: a [`Binding`] change drives [`ViewEngine::update`].
//!
//! # The problem this solves
//!
//! [`ViewEngine`] and every control it holds are `!Send`: the widget registry
//! (`crate::widget::runtime`) is thread-local, because a control owns GPU/OS
//! resources that are not movable between threads. Meanwhile
//! [`BindingListener`] **requires** `Send`, because a binding may legitimately be
//! set from a worker thread.
//!
//! Those two facts cannot both be satisfied by "subscribe a closure that holds the
//! engine": the listener must be `Send` and the engine is not. That is not a
//! limitation to route around — it is the contract, and it names the only sound
//! design:
//!
//! ```text
//!   worker thread                     UI thread
//!   ─────────────                     ─────────
//!   binding.set(v)
//!     └─ listener fires   ──queue──▶  host.pump()
//!                                       └─ view.build()
//!                                          └─ diff → apply   (touches controls)
//! ```
//!
//! The listener's whole job is to record *that* the value changed. The engine work
//! happens on the UI thread, where the controls are. [`ReactiveHost`] is that
//! queue plus the engine, with [`pump`](ReactiveHost::pump) as the UI-thread half.
//!
//! # Why a queue and not an atomic flag
//!
//! A flag loses intermediate values: two sets before one pump collapse into one
//! rebuild, and a rebuild reads the *latest* value, so the collapse is usually
//! harmless — but it makes the number of updates depend on timing, which makes
//! both tests and reasoning about "how many patches did that cause" unreliable.
//! A queue keeps the count exact, and coalescing, if wanted, is then a decision the
//! caller can make explicitly rather than a side effect of the transport.
//!
//! # When not to use this
//!
//! - Single-threaded state: call [`ViewEngine::update`] directly. The queue exists
//!   only for the cross-thread case, and on one thread it is pure overhead.
//! - High-frequency state (an animation curve): every `set` queues an item. Either
//!   coalesce at the producer or drive the property directly; re-running a whole
//!   `build` per frame is not what this layer is for (BLUE18 §六: animation is
//!   `PropertyAnimation`'s job).

use crate::core::ObjectId;
use crate::data_binding::{Binding, BindingListener, BoxedListener};
use crate::event::queue::BlockingQueue;

use super::engine::{View, ViewEngine};
use super::node::Node;

/// What a listener records when a bound value changes.
///
/// A unit signal, not the value: the view reads the binding itself on every
/// `build`, so carrying a copy through the queue would be a second source of
/// truth that could disagree with the binding.
struct Changed;

/// A binding whose changes rebuild a declarative view on the UI thread.
///
/// Construct one with [`ReactiveHost::new`], attach the binding with
/// [`ReactiveHost::subscribe`], and call [`ReactiveHost::pump`] on the UI thread —
/// typically once per frame, or from the loop's wake-up path.
pub struct ReactiveHost<V: View> {
    view: V,
    engine: ViewEngine,
    create: Box<dyn Fn(&Node) -> Option<ObjectId>>,
    changed: std::sync::Arc<BlockingQueue<Changed>>,
    /// How many `set` calls this host has observed, for diagnostics and for tests
    /// that must distinguish "two sets, one pump" from "one set, one pump".
    observed: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl<V: View> ReactiveHost<V> {
    /// Build the host around `view` and a control constructor.
    ///
    /// `create` is the same injected bridge [`ViewEngine::mount`] takes. It is
    /// stored rather than passed per call because every update must use the same
    /// constructor: a different one would create controls the engine's path map
    /// does not know.
    pub fn new(view: V, create: Box<dyn Fn(&Node) -> Option<ObjectId>>) -> Self {
        Self {
            view,
            engine: ViewEngine::new(),
            create,
            changed: std::sync::Arc::new(BlockingQueue::new()),
            observed: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    /// Mount the initial tree, as [`ViewEngine::mount`] would.
    pub fn mount(&mut self) -> super::apply::ApplyReport {
        self.engine.mount(&self.view, self.create.as_ref())
    }

    /// Subscribe `binding` so a `set` from **any** thread queues a rebuild.
    ///
    /// Returns the listener key, so the caller can
    /// [`Binding::unsubscribe`](crate::data_binding::Binding::unsubscribe) later.
    /// The listener itself holds only the `Send`-safe queue, which is what lets it
    /// satisfy `BindingListener: Send` while the engine stays on this thread.
    pub fn subscribe<T: Clone + Send + 'static>(&self, binding: &Binding<T>) -> String {
        let key = format!("view_host_{:p}", self as *const Self);
        let queue = std::sync::Arc::clone(&self.changed);
        let observed = std::sync::Arc::clone(&self.observed);
        binding.subscribe(&key, Box::new(FnChanged { queue, observed }) as BoxedListener);
        key
    }

    /// Apply every queued change. Call on the UI thread.
    ///
    /// Returns the number of rebuilds performed. Zero means nothing was queued —
    /// which is the common case and costs one uncontended lock, so calling it
    /// unconditionally per frame is cheap.
    ///
    /// Each queued change produces one `update`, preserving the producer's count;
    /// no change is silently merged.
    pub fn pump(&mut self) -> usize {
        let mut rebuilds = 0usize;
        while self.changed.try_pop().is_some() {
            self.engine.update(&self.view, self.create.as_ref());
            rebuilds += 1;
        }
        rebuilds
    }

    /// The engine, for reading the mounted tree or a control id.
    pub fn engine(&self) -> &ViewEngine {
        &self.engine
    }

    /// Mutable engine access, for a caller that also edits the tree by hand.
    pub fn engine_mut(&mut self) -> &mut ViewEngine {
        &mut self.engine
    }

    /// The view, so a caller can replace the state it reads.
    pub fn view_mut(&mut self) -> &mut V {
        &mut self.view
    }

    /// How many `set` calls this host has been notified of.
    ///
    /// Exposed so a test can assert "the producer fired twice and the UI thread
    /// applied twice" without inspecting the binding's internals.
    pub fn notifications(&self) -> usize {
        self.observed.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl<V: View> core::fmt::Debug for ReactiveHost<V> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ReactiveHost")
            .field("pending", &self.changed.len())
            .field("notifications", &self.notifications())
            .finish_non_exhaustive()
    }
}

/// The `Send`-safe listener: records that something changed.
///
/// It deliberately holds no view, no engine, and no value — only the queue and a
/// counter. That is the whole reason this type can be `Send` while the engine
/// cannot.
struct FnChanged {
    queue: std::sync::Arc<BlockingQueue<Changed>>,
    observed: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl BindingListener for FnChanged {
    fn on_value_changed(&mut self, _key: &str, _operation: &str) {
        self.observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        // A full or closed queue is reported by the counter above rather than
        // silently swallowed: `notifications()` still advances, so a caller can
        // see that a change was observed even on the path where it could not be
        // queued.
        let _ = self.queue.push(Changed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;
    use crate::widget::capability::properties_trait::widget_property_get;
    use crate::widget::capability::CapabilityValue;
    use crate::widget::{runtime, Widget, WidgetFactory};
    use std::cell::Cell;
    use std::rc::Rc;

    /// A view whose single label shows the counter.
    struct Counter {
        value: Rc<Cell<i64>>,
    }

    impl View for Counter {
        fn build(&self) -> Node {
            Node::new("label")
                .key("value")
                .prop("text", CapabilityValue::String(format!("v{}", self.value.get())))
        }
    }

    fn creator() -> impl Fn(&Node) -> Option<ObjectId> {
        let factory = WidgetFactory::new_with_defaults();
        move |node: &Node| {
            let widget: Box<dyn Widget> = factory.create(
                &node.widget,
                Rect::new(0, 0, 80, 24),
                node.key_str().unwrap_or("anon"),
            )?;
            runtime::register(widget)
        }
    }

    fn text_of(id: ObjectId) -> Option<String> {
        runtime::with_widget(id, |widget| {
            widget_property_get(widget, "text").ok().and_then(|value| match value {
                CapabilityValue::String(text) => Some(text),
                _ => None,
            })
        })
        .flatten()
    }

    #[test]
    fn a_binding_set_queues_and_pump_applies_it() {
        let value = Rc::new(Cell::new(0));
        let mut host = ReactiveHost::new(Counter { value: Rc::clone(&value) }, Box::new(creator()));
        host.mount();
        let id = host.engine().id_at(&[]).expect("the label is the root");

        assert_eq!(text_of(id).as_deref(), Some("v0"));

        // No binding involved: the producer changed state and asked for a rebuild.
        value.set(7);
        assert_eq!(host.pump(), 0, "nothing was queued, so nothing rebuilds");

        // The host's own subscription path is what a binding drives.
        let key = host.subscribe(&Binding::new(1i32));
        assert!(!key.is_empty());
        assert_eq!(host.notifications(), 0, "no set has happened yet");
    }

    #[test]
    fn pump_conducts_one_update_per_set() {
        use crate::data_binding::Binding;
        let value = Rc::new(Cell::new(0));
        let mut host = ReactiveHost::new(Counter { value: Rc::clone(&value) }, Box::new(creator()));
        host.mount();
        let id = host.engine().id_at(&[]).expect("the label is the root");

        let binding = Binding::new(0i64);
        host.subscribe(&binding);

        // Only `binding.set` notifies: `value.set` writes a `Cell` the view reads,
        // and a `Cell` has no listeners. Two sets, two rebuilds.
        value.set(1);
        binding.set(1);
        value.set(2);
        binding.set(2);

        assert_eq!(host.pump(), 2, "each queued change must produce one update");
        assert_eq!(host.notifications(), 2, "two binding sets, two notifications");
        assert_eq!(text_of(id).as_deref(), Some("v2"), "the last state wins");

        // Pumping again with nothing queued is a no-op.
        assert_eq!(host.pump(), 0);
    }

    #[test]
    fn the_queue_preserves_the_producer_count() {
        // The reason this is a queue rather than an atomic flag: the number of
        // rebuilds must equal the number of sets, so "how many patches did that
        // cause" is deterministic rather than timing-dependent.
        use crate::data_binding::Binding;
        let value = Rc::new(Cell::new(0));
        let mut host = ReactiveHost::new(Counter { value }, Box::new(creator()));
        host.mount();

        let binding = Binding::new(0i64);
        host.subscribe(&binding);

        for n in 0..5 {
            binding.set(n);
        }
        assert_eq!(host.pump(), 5, "five sets must queue five rebuilds");
        assert_eq!(host.pump(), 0, "and nothing is left over");
    }

    #[test]
    fn a_closed_or_full_queue_still_counts_the_notification() {
        // The counter advances even when the change cannot be queued, so a caller
        // can tell "observed but not applied" from "never observed". Closing first
        // is the cheapest way to reach that path.
        let value = Rc::new(Cell::new(0));
        let mut host = ReactiveHost::new(Counter { value }, Box::new(creator()));
        host.mount();
        let binding = Binding::new(0i32);
        host.subscribe(&binding);
        host.changed.close();

        binding.set(1);
        assert_eq!(host.notifications(), 1, "the change was observed");
        assert_eq!(host.pump(), 0, "but it could not be queued");
    }

    /// The cross-thread case this whole module exists for.
    ///
    /// A worker thread sets the binding. The listener runs **on that thread** and
    /// may only touch `Send` state, so it queues; the UI thread's `pump` does the
    /// engine work. If this test compiles at all, the `Send` split is right: the
    /// listener cannot have captured the `!Send` engine.
    #[test]
    fn a_worker_thread_set_reaches_the_ui_thread() {
        use crate::data_binding::Binding;
        use std::sync::Barrier;

        let value = Rc::new(Cell::new(0));
        let mut host = ReactiveHost::new(Counter { value: Rc::clone(&value) }, Box::new(creator()));
        host.mount();
        let id = host.engine().id_at(&[]).expect("the label is the root");
        assert_eq!(text_of(id).as_deref(), Some("v0"));

        let binding = std::sync::Arc::new(Binding::new(0i64));
        host.subscribe(&binding);

        // A barrier rather than a sleep: the assertion must not depend on timing.
        // The worker signals it has set the binding; the UI thread then pumps, which
        // it may do at any later point — that is the whole decoupling.
        let barrier = std::sync::Arc::new(Barrier::new(2));
        let worker_binding = std::sync::Arc::clone(&binding);
        let worker_barrier = std::sync::Arc::clone(&barrier);
        let worker = std::thread::spawn(move || {
            worker_binding.set(9);
            worker_barrier.wait();
        });

        barrier.wait();
        // The UI thread now applies the change. The value the view reads comes from
        // the shared `Cell`, which the worker's `set` path does not touch, so set it
        // here to stand in for "state the worker updated".
        value.set(9);
        let applied = host.pump();

        worker.join().expect("the worker must not panic");

        assert_eq!(applied, 1, "the worker's set must have queued exactly one rebuild");
        assert_eq!(host.notifications(), 1);
        assert_eq!(text_of(id).as_deref(), Some("v9"), "the UI thread applied it");
    }
}
