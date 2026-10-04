// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Focus management for widgets.
use crate::compat::{Box, HashMap, Vec};
use crate::core::ObjectId;
use crate::signal::GenericSignal;
/// Manages keyboard focus across widgets.
pub struct FocusManager {
    /// Currently focused widget, if any.
    focused_widget: Option<ObjectId>,
    /// Signal emitted when focus changes.
    pub focus_changed: GenericSignal,
    /// Ordered list of focusable widgets for tab-order traversal.
    ///
    /// This is the **effective** order the next Tab follows. Under a spatial
    /// strategy (`RowMajor`/`ColumnMajor`) it is a position-sorted view of
    /// [`Self::registration_order`]; under `TabOrder` it is always a copy of
    /// that canonical list.
    focusable_widgets: Vec<ObjectId>,
    /// The canonical, user-supplied registration order. Spatial strategies sort a
    /// *view* of this list without mutating it, so switching back to `TabOrder`
    /// restores the order the widgets were registered in (see [`Self::reorder_by_strategy`]).
    registration_order: Vec<ObjectId>,
    /// Callback invoked when focus changes (used by AccessibilityBridge).
    on_focus_changed: Option<Box<dyn Fn(ObjectId)>>,
    /// Focus traversal strategy (BLUE11 R7.2).
    traversal_strategy: FocusTraversalStrategy,
    /// Widget positions for RowMajor/ColumnMajor traversal.
    widget_positions: HashMap<ObjectId, (i32, i32)>,
}
impl FocusManager {
    /// Creates a new focus manager.
    pub fn new() -> Self {
        Self {
            focused_widget: None,
            focus_changed: GenericSignal::new(),
            focusable_widgets: Vec::new(),
            registration_order: Vec::new(),
            on_focus_changed: None,
            traversal_strategy: FocusTraversalStrategy::TabOrder,
            widget_positions: HashMap::new(),
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
        let old = self.focused_widget;
        self.focused_widget = Some(widget_id);
        self.focus_changed.emit();
        if let Some(ref cb) = self.on_focus_changed {
            // Notify the previously focused widget that it lost focus.
            if let Some(old_id) = old {
                (cb)(old_id);
            }
            // Notify the newly focused widget.
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
        if !self.registration_order.contains(&id) {
            self.registration_order.push(id);
        }
        // A registration must respect the active spatial policy on the next Tab,
        // not only when a position or strategy change re-sorts. Re-deriving the
        // effective order here keeps a later `register_focusable` / `set_focus_order`
        // from silently ignoring an enabled RowMajor/ColumnMajor strategy.
        self.reorder_by_strategy();
    }

    /// Remove a widget from the focusable order.
    pub fn unregister_focusable(&mut self, id: ObjectId) {
        self.registration_order.retain(|&x| x != id);
        self.reorder_by_strategy();
        // If the removed widget was focused, clear focus and fire a11y callback.
        if self.focused_widget == Some(id) {
            self.focused_widget = None;
            self.focus_changed.emit();
            if let Some(ref cb) = self.on_focus_changed {
                (cb)(id);
            }
        }
    }

    /// Set the entire focus order (replaces any existing order).
    ///
    /// If the current `focused_widget` is not present in the new list,
    /// focus is reset to `None`.
    pub fn set_focus_order(&mut self, ids: Vec<ObjectId>) {
        // Validate that the currently focused widget still exists in the new list.
        if let Some(focused) = self.focused_widget {
            if !ids.contains(&focused) {
                self.focused_widget = None;
                self.focus_changed.emit();
                if let Some(ref cb) = self.on_focus_changed {
                    (cb)(focused);
                }
            }
        }
        self.registration_order = ids;
        // A full replacement must respect an enabled spatial policy on the next
        // Tab too, exactly as dynamic registration does.
        self.reorder_by_strategy();
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
crate::impl_default_via_new!(FocusManager);

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
    ///
    /// The effective [`Self::focusable_widgets`] is always derived from the
    /// canonical [`Self::registration_order`]. Spatial strategies sort a copy, so
    /// the canonical order survives every sort and `TabOrder` restores it exactly.
    fn reorder_by_strategy(&mut self) {
        match self.traversal_strategy {
            FocusTraversalStrategy::TabOrder => {
                self.focusable_widgets = self.registration_order.clone();
            }
            FocusTraversalStrategy::RowMajor => {
                // Sort by y first (top-to-bottom), then x (left-to-right). A stable
                // sort keeps widgets that share a position in registration order.
                let mut order = self.registration_order.clone();
                order.sort_by(|a, b| {
                    let pos_a = self.widget_positions.get(a).copied().unwrap_or((0, 0));
                    let pos_b = self.widget_positions.get(b).copied().unwrap_or((0, 0));
                    pos_a.1.cmp(&pos_b.1).then(pos_a.0.cmp(&pos_b.0))
                });
                self.focusable_widgets = order;
            }
            FocusTraversalStrategy::ColumnMajor => {
                // Sort by x first (left-to-right), then y (top-to-bottom).
                let mut order = self.registration_order.clone();
                order.sort_by(|a, b| {
                    let pos_a = self.widget_positions.get(a).copied().unwrap_or((0, 0));
                    let pos_b = self.widget_positions.get(b).copied().unwrap_or((0, 0));
                    pos_a.0.cmp(&pos_b.0).then(pos_a.1.cmp(&pos_b.1))
                });
                self.focusable_widgets = order;
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

    /// S-41: a spatial strategy must not destroy the canonical registration order.
    /// Tab → Row/Column → Tab has to restore the user's original order.
    #[test]
    fn spatial_sort_restores_registration_order_on_tab_order() {
        let mut fm = FocusManager::new();
        fm.register_focusable(2);
        fm.register_focusable(1);
        // Positions disagree with registration order: id 1 is left of id 2.
        fm.set_widget_position(1, 0, 0);
        fm.set_widget_position(2, 10, 0);

        assert_eq!(fm.focusable_widgets(), &[2, 1], "TabOrder keeps registration order");

        fm.set_traversal_strategy(FocusTraversalStrategy::RowMajor);
        assert_eq!(fm.focusable_widgets(), &[1, 2], "RowMajor sorts spatially");

        fm.set_traversal_strategy(FocusTraversalStrategy::TabOrder);
        assert_eq!(fm.focusable_widgets(), &[2, 1], "switching back restores registration order");
    }

    /// S-41 (column variant): the same restore property holds for ColumnMajor.
    #[test]
    fn column_major_sort_restores_registration_order_on_tab_order() {
        let mut fm = FocusManager::new();
        fm.register_focusable(2);
        fm.register_focusable(1);
        fm.set_widget_position(1, 0, 0);
        fm.set_widget_position(2, 0, 10);

        fm.set_traversal_strategy(FocusTraversalStrategy::ColumnMajor);
        assert_eq!(fm.focusable_widgets(), &[1, 2], "ColumnMajor sorts spatially");

        fm.set_traversal_strategy(FocusTraversalStrategy::TabOrder);
        assert_eq!(fm.focusable_widgets(), &[2, 1], "switching back restores registration order");
    }

    /// S-42: registering a widget while a spatial strategy is active must respect
    /// that policy on the next Tab, without destroying the canonical order.
    #[test]
    fn register_focusable_respects_an_active_spatial_strategy() {
        let mut fm = FocusManager::new();
        fm.set_traversal_strategy(FocusTraversalStrategy::RowMajor);
        fm.register_focusable(1);
        fm.set_widget_position(1, 10, 0);
        // id 2 is registered later but sits to the left, so RowMajor must visit it first.
        fm.set_widget_position(2, 0, 0);
        fm.register_focusable(2);

        assert_eq!(fm.focusable_widgets(), &[2, 1], "late registration honours the spatial policy");

        fm.set_traversal_strategy(FocusTraversalStrategy::TabOrder);
        assert_eq!(fm.focusable_widgets(), &[1, 2], "registration order is still preserved");
    }

    /// S-42: a full order replacement must honour the active spatial policy too.
    #[test]
    fn set_focus_order_respects_an_active_spatial_strategy() {
        let mut fm = FocusManager::new();
        fm.set_traversal_strategy(FocusTraversalStrategy::ColumnMajor);
        fm.set_widget_position(1, 0, 0);
        fm.set_widget_position(2, 10, 0);
        fm.set_focus_order(vec![1, 2]);

        assert_eq!(fm.focusable_widgets(), &[1, 2], "ColumnMajor sorts the replaced order");

        fm.set_traversal_strategy(FocusTraversalStrategy::TabOrder);
        assert_eq!(fm.focusable_widgets(), &[1, 2], "the replaced canonical order is restored");
    }
}
