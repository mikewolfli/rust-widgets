use crate::compat::HashMap;
use crate::data_binding::traits::*;

/// An observable list that notifies listeners on mutations.
///
/// Provides standard collection operations (`push`, `pop`, `insert`, `remove`,
/// `clear`) that each trigger listener notifications with a key describing the
/// operation.
pub struct ObservableList<T: Clone + Send + 'static> {
    items: Vec<T>,
    listeners: HashMap<String, BoxedListener>,
}

impl<T: Clone + Send + 'static> ObservableList<T> {
    /// Create an empty observable list.
    pub fn new() -> Self {
        Self { items: Vec::new(), listeners: HashMap::new() }
    }

    /// Create an observable list pre-populated with items.
    pub fn with_items(items: Vec<T>) -> Self {
        Self { items, listeners: HashMap::new() }
    }

    /// Append an item to the end of the list and notify listeners.
    pub fn push(&mut self, item: T) {
        self.items.push(item);
        self.notify("push");
    }

    /// Remove and return the last element, or `None` if empty.
    pub fn pop(&mut self) -> Option<T> {
        let result = self.items.pop();
        if result.is_some() {
            self.notify("pop");
        }
        result
    }

    /// Insert an item at the given index and notify listeners.
    ///
    /// # Panics
    /// Panics if `index > len`.
    pub fn insert(&mut self, index: usize, item: T) {
        self.items.insert(index, item);
        self.notify("insert");
    }

    /// Remove and return the element at the given index and notify listeners.
    ///
    /// # Panics
    /// Panics if `index` is out of bounds.
    pub fn remove(&mut self, index: usize) -> T {
        let result = self.items.remove(index);
        self.notify("remove");
        result
    }

    /// Remove all elements and notify listeners.
    pub fn clear(&mut self) {
        if !self.items.is_empty() {
            self.items.clear();
            self.notify("clear");
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
    /// The listener receives a key describing which operation occurred
    /// (`"push"`, `"pop"`, `"insert"`, `"remove"`, or `"clear"`).
    pub fn subscribe(&mut self, key: &str, listener: BoxedListener) {
        self.listeners.insert(key.to_string(), listener);
    }

    fn notify(&mut self, operation: &str) {
        let keys: Vec<String> = self.listeners.keys().cloned().collect();
        for key in &keys {
            if let Some(listener) = self.listeners.get_mut(key) {
                listener.on_value_changed(operation);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::Mutex;
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
            Box::new(FnListener::new(move |op| {
                *no.lock().unwrap() = op.to_string();
            })),
        );
        list.push(42);
        assert_eq!(*notified_op.lock().unwrap(), "push");
    }

    #[test]
    fn test_list_pop_notification() {
        let mut list = ObservableList::with_items(vec![1, 2, 3]);
        let notified_op = Arc::new(Mutex::new(String::new()));
        let no = notified_op.clone();
        list.subscribe(
            "test",
            Box::new(FnListener::new(move |op| {
                *no.lock().unwrap() = op.to_string();
            })),
        );
        list.pop();
        assert_eq!(*notified_op.lock().unwrap(), "pop");
    }

    #[test]
    fn test_list_insert_notification() {
        let mut list = ObservableList::with_items(vec![1, 3]);
        let notified_op = Arc::new(Mutex::new(String::new()));
        let no = notified_op.clone();
        list.subscribe(
            "test",
            Box::new(FnListener::new(move |op| {
                *no.lock().unwrap() = op.to_string();
            })),
        );
        list.insert(1, 2);
        assert_eq!(*notified_op.lock().unwrap(), "insert");
    }

    #[test]
    fn test_list_remove_notification() {
        let mut list = ObservableList::with_items(vec![1, 2, 3]);
        let count = Arc::new(AtomicI32::new(0));
        let c = count.clone();
        list.subscribe(
            "test",
            Box::new(FnListener::new(move |_| {
                c.fetch_add(1, Ordering::SeqCst);
            })),
        );
        list.remove(0);
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }
}
