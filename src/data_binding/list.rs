// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use crate::compat::{Box, HashMap, MiniToString, String, Vec};
use crate::data_binding::traits::*;

/// A structural mutation of an [`ObservableList`], with the index and item(s) it touched.
///
/// # Why the change carries a payload (BIND-011)
///
/// [`BindingListener`] receives only `(key, operation)`, which is enough for a counter but not for a
/// view that has to update **one row**: "push" does not say which index or which value, so the view
/// rebuilt the whole list. The change is reported separately so existing listeners keep the coarse
/// contract while a view subscribes to this richer one.
#[derive(Debug, Clone, PartialEq)]
pub enum ListChange<T> {
    /// `value` was appended at `index`.
    Push {
        /// The index the item now occupies.
        index: usize,
        /// The appended item.
        value: T,
    },
    /// The item at `index` was removed from the end.
    Pop {
        /// The index the item occupied.
        index: usize,
        /// The removed item.
        value: T,
    },
    /// `value` was inserted at `index`.
    Insert {
        /// The index the item now occupies.
        index: usize,
        /// The inserted item.
        value: T,
    },
    /// The item that occupied `index` was removed.
    Remove {
        /// The index the item occupied.
        index: usize,
        /// The removed item.
        value: T,
    },
    /// Every item was removed; `removed` is the previous contents in order.
    Clear {
        /// The items the list held before it was cleared.
        removed: Vec<T>,
    },
}

/// A listener for [`ListChange`]s.
pub type ListChangeListener<T> = Box<dyn FnMut(&ListChange<T>) + Send>;

/// An observable list that notifies listeners on mutations.
///
/// Provides standard collection operations (`push`, `pop`, `insert`, `remove`,
/// `clear`) that each trigger listener notifications with a key describing the
/// operation.
pub struct ObservableList<T: Clone + Send + 'static> {
    items: Vec<T>,
    listeners: HashMap<String, BoxedListener>,
    change_listeners: HashMap<String, ListChangeListener<T>>,
}

impl<T: Clone + Send + 'static> Default for ObservableList<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone + Send + 'static> ObservableList<T> {
    /// Creates a new empty observable list.
    pub fn new() -> Self {
        Self { items: Vec::new(), listeners: HashMap::new(), change_listeners: HashMap::new() }
    }

    /// Create an observable list pre-populated with items.
    pub fn with_items(items: Vec<T>) -> Self {
        Self { items, listeners: HashMap::new(), change_listeners: HashMap::new() }
    }

    /// Append an item to the end of the list and notify listeners.
    pub fn push(&mut self, item: T) {
        self.items.push(item);
        let index = self.items.len() - 1;
        self.notify("push");
        let value = self.items[index].clone();
        self.notify_change(ListChange::Push { index, value });
    }

    /// Remove and return the last element, or `None` if empty.
    pub fn pop(&mut self) -> Option<T> {
        let index = self.items.len().checked_sub(1)?;
        let result = self.items.pop();
        if let Some(value) = &result {
            self.notify("pop");
            self.notify_change(ListChange::Pop { index, value: value.clone() });
        }
        result
    }

    /// Insert an item at the given index and notify listeners.
    ///
    /// # Panics
    /// Panics if `index > len`.
    pub fn insert(&mut self, index: usize, item: T) {
        let value = item.clone();
        self.items.insert(index, item);
        self.notify("insert");
        self.notify_change(ListChange::Insert { index, value });
    }

    /// Remove and return the element at the given index and notify listeners.
    ///
    /// # Panics
    /// Panics if `index` is out of bounds.
    pub fn remove(&mut self, index: usize) -> T {
        let value = self.items.remove(index);
        self.notify("remove");
        self.notify_change(ListChange::Remove { index, value: value.clone() });
        value
    }

    /// Remove all elements and notify listeners.
    pub fn clear(&mut self) {
        if !self.items.is_empty() {
            let removed = core::mem::take(&mut self.items);
            self.notify("clear");
            self.notify_change(ListChange::Clear { removed });
        }
    }

    /// Get a reference to the element at the given index, or `None`.
    pub fn get(&self, index: usize) -> Option<&T> {
        self.items.get(index)
    }

    /// Return the number of elements.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns `true` if the list contains no elements.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Iterate over all elements.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.items.iter()
    }

    /// Subscribe to list mutation notifications.
    ///
    /// `key` is the subscription identifier. The listener receives `(key, operation)`
    /// where `operation` is `"push"`, `"pop"`, `"insert"`, `"remove"`, or `"clear"`.
    pub fn subscribe(&mut self, key: &str, listener: BoxedListener) {
        self.listeners.insert(key.to_string(), listener);
    }

    /// Remove a listener by its subscription key.
    pub fn unsubscribe(&mut self, key: &str) {
        self.listeners.remove(key);
    }

    /// Subscribe to structural changes **with their index and value** (BIND-011).
    ///
    /// This is the channel a view uses to update one row: the coarse [`Self::subscribe`] listeners
    /// keep receiving `(key, operation)` exactly as before, so adding a change subscriber does not
    /// alter what a document's existing handlers see.
    pub fn subscribe_changes<F>(&mut self, key: &str, listener: F)
    where
        F: FnMut(&ListChange<T>) + Send + 'static,
    {
        self.change_listeners.insert(key.to_string(), Box::new(listener));
    }

    /// Remove a change listener by its subscription key.
    pub fn unsubscribe_changes(&mut self, key: &str) {
        self.change_listeners.remove(key);
    }

    fn notify(&mut self, operation: &str) {
        // Key and listener come from the same entry, so no key clone is needed.
        for (key, listener) in self.listeners.iter_mut() {
            listener.on_value_changed(key, operation);
        }
    }

    fn notify_change(&mut self, change: ListChange<T>) {
        for listener in self.change_listeners.values_mut() {
            listener(&change);
        }
    }
}

#[cfg(all(full_widgets, not(alloc_frugal)))]
mod observable_model {
    //! The bridge from [`ObservableList`] to the view layer's model protocols (RW-9).
    //!
    //! # Why this lives beside the list rather than in the view
    //!
    //! `ObservableList` is the data-binding layer's collection primitive and `ListModel` is the
    //! view layer's read protocol. Neither module may depend on the other's `full_widgets`-gated
    //! surface, but this module already sits on the binding side and can name both, so the bridge
    //! is the one place that knows both vocabularies.

    use super::{ListChange, ObservableList};
    use crate::compat::{format, lock, Arc, MiniToString, Mutex, String, Vec};
    use crate::core::ObjectId;
    use crate::signal::{GenericSignal, Signal1};
    use crate::widget::view_widgets::data_source::ListModelDataSource;
    use crate::widget::view_widgets::list_view::ListModel;
    use crate::widget::view_widgets::table_widget::{TableModel, TableWidget};

    /// An [`ObservableList`] projected as a [`ListModel`], with fine-grained row signals (RW-9).
    ///
    /// # Why a mirrored snapshot instead of a reference
    ///
    /// A `ListModel` is `Send + Sync` and is held by a view through an `Arc`, so it cannot borrow
    /// the list that owns it. The model therefore keeps its own snapshot and every
    /// [`ListChange`] — which already carries the index and the value — applies to that snapshot
    /// incrementally. No change re-reads the whole list.
    ///
    /// # Lifetime
    ///
    /// The change listener the list holds captures an `Arc` of this model, so the model lives at
    /// least as long as the list. [`ObservableList::unsubscribe_changes`] with the same key ends it
    /// early; the key is [`Self::subscription_key`].
    pub struct ObservableListModel<T: Clone + Send + 'static> {
        items: Mutex<Vec<T>>,
        data_changed: GenericSignal,
        row_changed: Signal1<usize>,
        rows_changed: GenericSignal,
        subscription_key: String,
    }

    impl<T: Clone + Send + 'static> ObservableListModel<T> {
        /// Projects `list` and subscribes to its changes, returning the shared model.
        ///
        /// `list` is taken mutably because a subscription is a mutation of the list's listener
        /// table; there is no way to observe an `ObservableList` through a shared reference, and
        /// pretending otherwise (a one-shot snapshot) would be a silent lie about liveness.
        pub fn bind(list: &mut ObservableList<T>) -> Arc<Self> {
            use core::sync::atomic::{AtomicU64, Ordering};
            // Two models over the same list must not overwrite each other's listener, so the key
            // carries a process-unique counter rather than an item count.
            static NEXT_MODEL_ID: AtomicU64 = AtomicU64::new(1);
            let items: Vec<T> = list.iter().cloned().collect();
            let key =
                format!("observable-list-model:{}", NEXT_MODEL_ID.fetch_add(1, Ordering::Relaxed));
            let model = Arc::new(Self {
                items: Mutex::new(items),
                data_changed: GenericSignal::new(),
                row_changed: Signal1::new(),
                rows_changed: GenericSignal::new(),
                subscription_key: key.clone(),
            });
            let sink = Arc::clone(&model);
            list.subscribe_changes(&key, move |change| sink.apply(change));
            model
        }

        /// The subscription key the list's change listener is registered under.
        pub fn subscription_key(&self) -> &str {
            &self.subscription_key
        }

        /// Applies one change to the snapshot and reports it at the finest granularity available.
        ///
        /// A row insertion/removal fires [`Self::rows_changed_signal`] (the projection shrank or
        /// grew) **and** [`Self::row_changed_signal`] for the index that moved; a clear fires only
        /// the structural signal. Every arm also advances the coarse `data_changed` signal the
        /// model protocol already publishes, so an existing view redraws.
        pub fn apply(&self, change: &ListChange<T>) {
            let changed_row = {
                let mut items = lock(&self.items);
                match change {
                    ListChange::Push { index, value } => {
                        debug_assert!(*index <= items.len());
                        items.insert(*index, value.clone());
                        Some(*index)
                    }
                    ListChange::Pop { index, .. } => {
                        if *index < items.len() {
                            items.remove(*index);
                        }
                        None
                    }
                    ListChange::Insert { index, value } => {
                        let at = (*index).min(items.len());
                        items.insert(at, value.clone());
                        Some(*index)
                    }
                    ListChange::Remove { index, .. } => {
                        if *index < items.len() {
                            items.remove(*index);
                        }
                        None
                    }
                    ListChange::Clear { .. } => {
                        items.clear();
                        None
                    }
                }
            };
            self.rows_changed.emit();
            if let Some(row) = changed_row {
                self.row_changed.emit(row);
            }
            self.data_changed.emit();
        }

        /// The mirrored items, in order.
        pub fn items(&self) -> Vec<T> {
            lock(&self.items).clone()
        }

        /// The number of mirrored items.
        pub fn len(&self) -> usize {
            lock(&self.items).len()
        }

        /// Whether the model mirrors no items.
        pub fn is_empty(&self) -> bool {
            lock(&self.items).is_empty()
        }

        /// The signal emitted with the index of an inserted row.
        pub fn row_changed_signal(&self) -> &Signal1<usize> {
            &self.row_changed
        }

        /// The signal emitted when rows are inserted, removed or cleared.
        pub fn rows_changed_signal(&self) -> &GenericSignal {
            &self.rows_changed
        }
    }

    impl<T: Clone + Send + MiniToString + 'static> ListModel for ObservableListModel<T> {
        fn row_count(&self) -> usize {
            lock(&self.items).len()
        }

        fn data(&self, row: usize) -> Option<String> {
            lock(&self.items).get(row).map(MiniToString::to_string)
        }

        fn fill_data(&self, row: usize, out: &mut String) -> bool {
            let items = lock(&self.items);
            match items.get(row) {
                Some(item) => {
                    out.push_str(&item.to_string());
                    true
                }
                None => false,
            }
        }

        fn data_changed_signal(&self) -> Option<&GenericSignal> {
            Some(&self.data_changed)
        }
    }

    /// The one-column table projection, so a [`TableWidget`] can consume the same model (RW-9).
    impl<T: Clone + Send + MiniToString + 'static> TableModel for ObservableListModel<T> {
        fn row_count(&self) -> usize {
            lock(&self.items).len()
        }

        fn column_count(&self) -> usize {
            1
        }

        fn data(&self, row: usize, column: usize) -> Option<String> {
            if column != 0 {
                return None;
            }
            lock(&self.items).get(row).map(MiniToString::to_string)
        }

        fn fill_data(&self, row: usize, column: usize, out: &mut String) -> bool {
            if column != 0 {
                return false;
            }
            let items = lock(&self.items);
            match items.get(row) {
                Some(item) => {
                    out.push_str(&item.to_string());
                    true
                }
                None => false,
            }
        }

        fn data_changed_signal(&self) -> Option<&GenericSignal> {
            Some(&self.data_changed)
        }
    }

    /// Projects `list` and binds the resulting model into the mounted widget at `widget_id` (RW-9).
    ///
    /// The widget is addressed the way every other runtime binding addresses one, and the model is
    /// attached according to what the widget actually is:
    ///
    /// * a [`ListView`](crate::widget::view_widgets::list_view::ListView) gets it through
    ///   `set_model`;
    /// * a [`TableWidget`] gets it through `set_model` (one implied column);
    /// * a [`DataGrid`](crate::widget::view_widgets::data_grid::DataGrid),
    ///   [`VirtualList`](crate::widget::view_widgets::virtual_list::VirtualList) or
    ///   [`VirtualTable`](crate::widget::view_widgets::virtual_table::VirtualTable) gets a
    ///   [`ListModelDataSource`] projection through `set_data_source`.
    ///
    /// A widget of any other type is left untouched — the model is still returned so the caller can
    /// bind it itself — because silently writing to an unrelated property would be worse than an
    /// unconsumed model. The initial rows are pushed by the attach itself; every later change
    /// reaches the view through the model's own change signal, so no per-change work is done here.
    ///
    /// `list` is `&mut` for the reason [`ObservableListModel::bind`] documents: subscribing is a
    /// mutation of the list.
    pub fn bind_widget_model<T>(
        widget_id: ObjectId,
        list: &mut ObservableList<T>,
    ) -> Arc<ObservableListModel<T>>
    where
        T: Clone + Send + MiniToString + 'static,
    {
        let model = ObservableListModel::bind(list);
        let list_model: Arc<dyn ListModel> = model.clone();
        let table_model: Arc<dyn TableModel> = model.clone();
        let _ = crate::widget::runtime::with_widget_mut(widget_id, |widget| {
            let any = widget as &mut dyn core::any::Any;
            if let Some(view) =
                any.downcast_mut::<crate::widget::view_widgets::list_view::ListView>()
            {
                view.set_model(list_model);
            } else if let Some(table) = any.downcast_mut::<TableWidget>() {
                table.set_model(table_model);
            } else if let Some(grid) =
                any.downcast_mut::<crate::widget::view_widgets::data_grid::DataGrid>()
            {
                grid.set_data_source(Arc::new(ListModelDataSource::new(list_model)));
            } else if let Some(list) =
                any.downcast_mut::<crate::widget::view_widgets::virtual_list::VirtualList>()
            {
                list.set_data_source(Arc::new(ListModelDataSource::new(list_model)));
            } else if let Some(table) =
                any.downcast_mut::<crate::widget::view_widgets::virtual_table::VirtualTable>()
            {
                table.set_data_source(Arc::new(ListModelDataSource::new(list_model)));
            }
        });
        model
    }
}

#[cfg(all(full_widgets, not(alloc_frugal)))]
pub use observable_model::{bind_widget_model, ObservableListModel};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::Box;
    use crate::compat::{lock, MiniToString, Mutex};
    use alloc::sync::Arc;
    use core::sync::atomic::{AtomicI32, Ordering};

    #[test]
    fn test_list_push_get() {
        let mut list = ObservableList::new();
        list.push(10);
        list.push(20);
        list.push(30);
        assert_eq!(list.len(), 3);
        assert_eq!(*list.get(0).unwrap(), 10);
        assert_eq!(*list.get(2).unwrap(), 30);
    }

    #[test]
    fn test_list_pop() {
        let mut list = ObservableList::with_items(vec![1, 2, 3]);
        assert_eq!(list.pop(), Some(3));
        assert_eq!(list.len(), 2);
        assert_eq!(list.pop(), Some(2));
        assert_eq!(list.pop(), Some(1));
        assert_eq!(list.pop(), None);
        assert!(list.is_empty());
    }

    #[test]
    fn test_list_insert_remove() {
        let mut list = ObservableList::with_items(vec!["a".to_string(), "c".to_string()]);
        list.insert(1, "b".to_string());
        assert_eq!(list.len(), 3);
        assert_eq!(list.get(1).unwrap(), "b");

        let removed = list.remove(1);
        assert_eq!(removed, "b");
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_list_clear() {
        let mut list = ObservableList::with_items(vec![1, 2, 3, 4, 5]);
        assert!(!list.is_empty());
        list.clear();
        assert!(list.is_empty());
    }

    #[test]
    fn test_list_iter() {
        let list = ObservableList::with_items(vec![1, 2, 3]);
        let collected: Vec<i32> = list.iter().copied().collect();
        assert_eq!(collected, vec![1, 2, 3]);
    }

    #[test]
    fn test_list_push_notification() {
        let mut list = ObservableList::new();
        let notified_op = Arc::new(Mutex::new(String::new()));
        let no = notified_op.clone();
        list.subscribe(
            "test",
            Box::new(FnListener::new(move |_key, op| {
                *lock(&no) = op.to_string();
            })),
        );
        list.push(42);
        assert_eq!(*lock(&notified_op), "push");
    }

    #[test]
    fn test_list_pop_notification() {
        let mut list = ObservableList::with_items(vec![1, 2, 3]);
        let notified_op = Arc::new(Mutex::new(String::new()));
        let no = notified_op.clone();
        list.subscribe(
            "test",
            Box::new(FnListener::new(move |_key, op| {
                *lock(&no) = op.to_string();
            })),
        );
        list.pop();
        assert_eq!(*lock(&notified_op), "pop");
    }

    #[test]
    fn test_list_insert_notification() {
        let mut list = ObservableList::with_items(vec![1, 3]);
        let notified_op = Arc::new(Mutex::new(String::new()));
        let no = notified_op.clone();
        list.subscribe(
            "test",
            Box::new(FnListener::new(move |_key, op| {
                *lock(&no) = op.to_string();
            })),
        );
        list.insert(1, 2);
        assert_eq!(*lock(&notified_op), "insert");
    }

    /// BIND-011: a change subscriber receives the index and value, not only the operation name.
    #[test]
    fn list_changes_carry_the_index_and_value() {
        let mut list = ObservableList::with_items(vec!["a".to_string(), "b".to_string()]);
        let seen = Arc::new(Mutex::new(Vec::<String>::new()));
        let slot = Arc::clone(&seen);
        list.subscribe_changes("view", move |change| {
            let text = match change {
                ListChange::Push { index, value } => format!("push:{index}:{value}"),
                ListChange::Pop { index, value } => format!("pop:{index}:{value}"),
                ListChange::Insert { index, value } => format!("insert:{index}:{value}"),
                ListChange::Remove { index, value } => format!("remove:{index}:{value}"),
                ListChange::Clear { removed } => format!("clear:{}", removed.len()),
            };
            lock(&slot).push(text);
        });

        list.push("c".to_string());
        list.insert(1, "x".to_string());
        list.remove(1);
        assert_eq!(list.pop().as_deref(), Some("c"));
        list.clear();

        assert_eq!(
            *lock(&seen),
            vec![
                String::from("push:2:c"),
                String::from("insert:1:x"),
                String::from("remove:1:x"),
                String::from("pop:2:c"),
                String::from("clear:2"),
            ]
        );
    }

    #[test]
    fn test_list_remove_notification() {
        let mut list = ObservableList::with_items(vec![1, 2, 3]);
        let count = Arc::new(AtomicI32::new(0));
        let c = count.clone();
        list.subscribe(
            "test",
            Box::new(FnListener::new(move |_, _| {
                c.fetch_add(1, Ordering::SeqCst);
            })),
        );
        list.remove(0);
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    /// RW-9: every `ListChange` applies to the model's snapshot once, and the model reports the
    /// row that moved on the fine-grained signal.
    #[cfg(all(full_widgets, not(alloc_frugal)))]
    #[test]
    fn an_observable_list_model_maps_changes_to_rows() {
        use crate::widget::view_widgets::list_view::ListModel;

        let mut list = ObservableList::with_items(vec!["a".to_string(), "b".to_string()]);
        let model = ObservableListModel::bind(&mut list);
        assert_eq!(model.row_count(), 2);
        assert_eq!(model.data(0).as_deref(), Some("a"));
        assert_eq!(model.data(1).as_deref(), Some("b"));
        assert_eq!(model.data(2), None);

        let moved = Arc::new(Mutex::new(Vec::<usize>::new()));
        let structural = Arc::new(AtomicI32::new(0));
        let moved_sink = Arc::clone(&moved);
        let structural_sink = Arc::clone(&structural);
        model.row_changed_signal().connect(move |row| {
            moved_sink.lock().expect("lock").push(*row.as_ref());
        });
        model.rows_changed_signal().connect(move || {
            structural_sink.fetch_add(1, Ordering::SeqCst);
        });

        list.push("c".to_string());
        assert_eq!(model.row_count(), 3);
        assert_eq!(model.data(2).as_deref(), Some("c"));

        list.insert(0, "z".to_string());
        list.remove(1);
        assert_eq!(model.items(), vec!["z".to_string(), "b".to_string(), "c".to_string()]);

        list.clear();
        assert!(model.is_empty());
        assert_eq!(*moved.lock().expect("lock"), vec![2usize, 0usize]);
        assert_eq!(structural.load(Ordering::SeqCst), 4, "one structural report per change");
    }

    /// RW-9: the binding attaches the projected model to a mounted view and later changes reach
    /// that view through the model's own signal, with no per-change work in the binding.
    #[cfg(all(full_widgets, not(alloc_frugal)))]
    #[test]
    fn bind_widget_model_attaches_to_a_mounted_list_view() {
        use crate::widget::view_widgets::list_view::ListView;

        let mut list = ObservableList::with_items(vec!["one".to_string()]);
        let id = crate::widget::runtime::register(Box::new(ListView::new(crate::core::Rect::new(
            0, 0, 120, 120,
        ))))
        .expect("the list view mounts");

        let model = bind_widget_model(id, &mut list);
        assert_eq!(model.len(), 1);

        let rows = crate::widget::runtime::with_widget(id, |widget| {
            (widget as &dyn core::any::Any).downcast_ref::<ListView>().map(|view| view.row_count())
        })
        .expect("the widget is mounted")
        .expect("the widget is a ListView");
        assert_eq!(rows, 1, "the initial rows are attached");

        list.push("two".to_string());
        let rows = crate::widget::runtime::with_widget(id, |widget| {
            (widget as &dyn core::any::Any).downcast_ref::<ListView>().map(|view| view.row_count())
        })
        .expect("the widget is mounted")
        .expect("the widget is a ListView");
        assert_eq!(rows, 2, "a list change reaches the attached view");

        crate::widget::runtime::unregister(id);
    }
}
