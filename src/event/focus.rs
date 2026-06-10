//! Focus management for widgets.
use crate::core::ObjectId;
use crate::signal::{ConnectionScope, GenericSignal};
/// Manages keyboard focus across widgets.
pub struct FocusManager {
    /// Currently focused widget, if any.
    focused_widget: Option<ObjectId>,
    /// Signal emitted when focus changes.
    pub focus_changed: GenericSignal,
    /// Scoped connections for focus tracking.
    _connection_scope: ConnectionScope,
    /// Ordered list of focusable widgets for tab-order traversal.
    focusable_widgets: Vec<ObjectId>,
    /// Callback invoked when focus changes (used by AccessibilityBridge).
    on_focus_changed: Option<Box<dyn Fn(ObjectId)>>,
    /// Focus traversal strategy (BLUE11 R7.2).
    traversal_strategy: FocusTraversalStrategy,
    /// Widget positions for RowMajor/ColumnMajor traversal.
    widget_positions: std::collections::HashMap<ObjectId, (i32, i32)>,
}
impl FocusManager {
    /// Creates a new focus manager.
    pub fn new() -> Self {
        Self {
            focused_widget: None,
            focus_changed: GenericSignal::new(),
            _connection_scope: ConnectionScope::new(),
            focusable_widgets: Vec::new(),
            on_focus_changed: None,
            traversal_strategy: FocusTraversalStrategy::TabOrder,
            widget_positions: std::collections::HashMap::new(),
        }
    }
    /// Returns the currently focused widget, if any.
    pub fn focused_widget(&self) -> Option<ObjectId> {
        self.focused_widget
    }
    /// Sets focus to a widget.
    pub fn set_focus(&mut self, widget_id: ObjectId) -> bool {
        if self.focused_widget == Some(widget_id) {
            return false;
        }
        self.focused_widget = Some(widget_id);
        self.focus_changed.emit();
        if let Some(ref cb) = self.on_focus_changed {
            (cb)(widget_id);
        }
        true
    }
    /// Clears focus from any widget.
    pub fn clear_focus(&mut self) -> bool {
        if self.focused_widget.is_none() {
            return false;
        }
        let old = self.focused_widget;
        self.focused_widget = None;
        self.focus_changed.emit();
        if let Some(ref cb) = self.on_focus_changed {
            if let Some(id) = old {
                (cb)(id);
            }
        }
        true
    }
    /// Checks if a widget has focus.
    pub fn has_focus(&self, widget_id: ObjectId) -> bool {
        self.focused_widget == Some(widget_id)
    }

    // --- Focusable widget registration (tab order) ---

    /// Returns the current focusable widget order.
    pub fn focusable_widgets(&self) -> &[ObjectId] {
        &self.focusable_widgets
    }

    /// Register a widget as focusable, appending it to the tab order.
    pub fn register_focusable(&mut self, id: ObjectId) {
        if !self.focusable_widgets.contains(&id) {
            self.focusable_widgets.push(id);
        }
    }

    /// Remove a widget from the focusable order.
    pub fn unregister_focusable(&mut self, id: ObjectId) {
        self.focusable_widgets.retain(|&x| x != id);
        // If the removed widget was focused, clear focus.
        if self.focused_widget == Some(id) {
            self.focused_widget = None;
            self.focus_changed.emit();
        }
    }

    /// Set the entire focus order (replaces any existing order).
    pub fn set_focus_order(&mut self, ids: Vec<ObjectId>) {
        self.focusable_widgets = ids;
    }

    /// Move focus to the next widget in the tab order (wraps around).
    /// Returns the newly focused widget, if any.
    pub fn focus_next(&mut self) -> Option<ObjectId> {
        if self.focusable_widgets.is_empty() {
            return None;
        }
        let current = self.focused_widget;
        let pos = current.and_then(|id| self.focusable_widgets.iter().position(|&x| x == id));
        let next_index = match pos {
            Some(idx) => (idx + 1) % self.focusable_widgets.len(),
            None => 0,
        };
        let next = self.focusable_widgets[next_index];
        self.set_focus(next);
        Some(next)
    }

    /// Move focus to the previous widget in the tab order (wraps around).
    /// Returns the newly focused widget, if any.
    pub fn focus_previous(&mut self) -> Option<ObjectId> {
        if self.focusable_widgets.is_empty() {
            return None;
        }
        let current = self.focused_widget;
        let pos = current.and_then(|id| self.focusable_widgets.iter().position(|&x| x == id));
        let prev_index = match pos {
            Some(idx) => {
                if idx == 0 {
                    self.focusable_widgets.len() - 1
                } else {
                    idx - 1
                }
            }
            None => self.focusable_widgets.len() - 1,
        };
        let prev = self.focusable_widgets[prev_index];
        self.set_focus(prev);
        Some(prev)
    }

    // --- Accessibility callback wiring ---

    /// Set the callback invoked when focus changes (used by AccessibilityBridge).
    pub fn set_a11y_callback(&mut self, cb: Box<dyn Fn(ObjectId)>) {
        self.on_focus_changed = Some(cb);
    }
}
impl Default for FocusManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Focus traversal order strategy (BLUE11 R7.2).
pub enum FocusTraversalStrategy {
    /// Tab order (linear).
    TabOrder,
    /// Row-major (left-to-right, top-to-bottom).
    RowMajor,
    /// Column-major (top-to-bottom, left-to-right).
    ColumnMajor,
}

impl FocusManager {
    /// Set the focus traversal strategy.
    pub fn set_traversal_strategy(&mut self, strategy: FocusTraversalStrategy) {
        self.traversal_strategy = strategy;
        self.reorder_by_strategy();
    }

    /// Record a widget's position for use by RowMajor/ColumnMajor traversal.
    pub fn set_widget_position(&mut self, id: ObjectId, x: i32, y: i32) {
        self.widget_positions.insert(id, (x, y));
        if !matches!(self.traversal_strategy, FocusTraversalStrategy::TabOrder) {
            self.reorder_by_strategy();
        }
    }

    /// Reorder the focusable list according to the current traversal strategy.
    fn reorder_by_strategy(&mut self) {
        match self.traversal_strategy {
            FocusTraversalStrategy::TabOrder => {
                // TabOrder is the default insertion order; no reordering needed.
            }
            FocusTraversalStrategy::RowMajor => {
                // Sort by y first (top-to-bottom), then x (left-to-right).
                self.focusable_widgets.sort_by(|a, b| {
                    let pos_a = self.widget_positions.get(a).copied().unwrap_or((0, 0));
                    let pos_b = self.widget_positions.get(b).copied().unwrap_or((0, 0));
                    pos_a.1.cmp(&pos_b.1).then(pos_a.0.cmp(&pos_b.0))
                });
            }
            FocusTraversalStrategy::ColumnMajor => {
                // Sort by x first (left-to-right), then y (top-to-bottom).
                self.focusable_widgets.sort_by(|a, b| {
                    let pos_a = self.widget_positions.get(a).copied().unwrap_or((0, 0));
                    let pos_b = self.widget_positions.get(b).copied().unwrap_or((0, 0));
                    pos_a.0.cmp(&pos_b.0).then(pos_a.1.cmp(&pos_b.1))
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_focusable() {
        let mut fm = FocusManager::new();
        fm.register_focusable(10);
        fm.register_focusable(20);
        fm.register_focusable(30);
        assert_eq!(fm.focusable_widgets(), &[10, 20, 30]);
    }

    #[test]
    fn test_register_focusable_duplicate() {
        let mut fm = FocusManager::new();
        fm.register_focusable(10);
        fm.register_focusable(10); // duplicate
        assert_eq!(fm.focusable_widgets(), &[10]);
    }

    #[test]
    fn test_focus_next_cycles() {
        let mut fm = FocusManager::new();
        fm.register_focusable(10);
        fm.register_focusable(20);
        fm.register_focusable(30);

        // No focus set yet -- focus_next should pick the first
        let focused = fm.focus_next();
        assert_eq!(focused, Some(10));
        assert_eq!(fm.focused_widget(), Some(10));

        // Cycle forward
        assert_eq!(fm.focus_next(), Some(20));
        assert_eq!(fm.focus_next(), Some(30));

        // Wrap around
        assert_eq!(fm.focus_next(), Some(10));
    }

    #[test]
    fn test_focus_previous_cycles() {
        let mut fm = FocusManager::new();
        fm.register_focusable(10);
        fm.register_focusable(20);
        fm.register_focusable(30);

        // No focus set yet -- focus_previous should pick the last
        assert_eq!(fm.focus_previous(), Some(30));

        // Cycle backward
        assert_eq!(fm.focus_previous(), Some(20));
        assert_eq!(fm.focus_previous(), Some(10));

        // Wrap around
        assert_eq!(fm.focus_previous(), Some(30));
    }

    #[test]
    fn test_unregister_removes() {
        let mut fm = FocusManager::new();
        fm.register_focusable(10);
        fm.register_focusable(20);
        fm.register_focusable(30);

        fm.unregister_focusable(20);
        assert_eq!(fm.focusable_widgets(), &[10, 30]);
    }

    #[test]
    fn test_unregister_clears_focus_if_focused() {
        let mut fm = FocusManager::new();
        fm.register_focusable(10);
        fm.register_focusable(20);
        fm.set_focus(10);
        assert!(fm.has_focus(10));

        fm.unregister_focusable(10);
        assert!(fm.focused_widget().is_none());
        assert_eq!(fm.focusable_widgets(), &[20]);
    }

    #[test]
    fn test_focus_next_empty_order() {
        let mut fm = FocusManager::new();
        assert_eq!(fm.focus_next(), None);
    }

    #[test]
    fn test_focus_previous_empty_order() {
        let mut fm = FocusManager::new();
        assert_eq!(fm.focus_previous(), None);
    }

    #[test]
    fn test_set_focus_order() {
        let mut fm = FocusManager::new();
        fm.set_focus_order(vec![1, 2, 3, 4]);
        assert_eq!(fm.focusable_widgets(), &[1, 2, 3, 4]);

        // Replaces old order
        fm.set_focus_order(vec![5, 6]);
        assert_eq!(fm.focusable_widgets(), &[5, 6]);
    }

    #[test]
    fn test_a11y_callback_fires() {
        use alloc::sync::Arc;
        use core::sync::atomic::{AtomicU64, Ordering};

        let mut fm = FocusManager::new();
        let last_id = Arc::new(AtomicU64::new(0));
        let cb_last = Arc::clone(&last_id);
        fm.set_a11y_callback(Box::new(move |id| {
            cb_last.store(id, Ordering::SeqCst);
        }));

        fm.set_focus(42);
        assert_eq!(last_id.load(Ordering::SeqCst), 42);

        fm.set_focus(99);
        assert_eq!(last_id.load(Ordering::SeqCst), 99);
    }

    #[test]
    fn test_a11y_callback_fires_on_clear() {
        use alloc::sync::Arc;
        use core::sync::atomic::{AtomicU64, Ordering};

        let mut fm = FocusManager::new();
        let last_id = Arc::new(AtomicU64::new(0));
        let cb_last = Arc::clone(&last_id);
        fm.set_a11y_callback(Box::new(move |id| {
            cb_last.store(id, Ordering::SeqCst);
        }));

        fm.set_focus(77);
        assert_eq!(last_id.load(Ordering::SeqCst), 77);

        fm.clear_focus();
        // clear_focus emits callback with the previously focused id
        assert_eq!(last_id.load(Ordering::SeqCst), 77);
    }
}
