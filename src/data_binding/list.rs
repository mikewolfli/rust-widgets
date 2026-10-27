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
}
