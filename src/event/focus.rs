// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Focus management for widgets.
use crate::compat::{BTreeSet, Box, HashMap, Vec};
use crate::core::ObjectId;
use crate::event::FocusReason;
use crate::signal::GenericSignal;

/// Which input may move focus to a control.
///
/// # Why a per-control policy is needed
///
/// "Focusable" is not one question. A text field takes focus from a click *and* from Tab; a
/// toolbar's row container might take it only from a click; a decorative label should take it
/// from neither. With one boolean, the only options are "Tab stops on the label" (wrong) or
/// "the label is skipped by every route including a click on the row" (also wrong). The policy
/// splits the answer into the two routes that exist.
///
/// A control that is not registered at all is not focusable, so [`FocusPolicy::NoFocus`] is
/// mainly useful for a registered control that should stop accepting input temporarily without
/// losing its place in the tab order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusPolicy {
    /// Never accepts focus from either route.
    NoFocus,
    /// Accepts focus from a pointer press only.
    ClickFocus,
    /// Accepts focus from keyboard traversal only.
    TabFocus,
    /// Accepts focus from both routes. The default, so controls registered without an explicit
    /// policy keep the behaviour they had before policies existed.
    #[default]
    StrongFocus,
}

impl FocusPolicy {
    /// Whether a pointer press may focus a control with this policy.
    pub const fn accepts_click(self) -> bool {
        matches!(self, Self::ClickFocus | Self::StrongFocus)
    }

    /// Whether keyboard traversal may focus a control with this policy.
    pub const fn accepts_tab(self) -> bool {
        matches!(self, Self::TabFocus | Self::StrongFocus)
    }

    /// Whether `reason` is a route this policy accepts.
    ///
    /// The mapping is stated once here rather than at each call site: [`FocusReason::Pointer`]
    /// is the click route; every other reason in the enum originates from keyboard navigation
    /// or application code that is deliberately acting *as* keyboard navigation
    /// ([`FocusReason::Programmatic`] does, because a dialog moving focus must be able to reach
    /// a field the user cannot click yet).
    pub const fn accepts(self, reason: FocusReason) -> bool {
        match reason {
            FocusReason::Pointer => self.accepts_click(),
            FocusReason::Tab
            | FocusReason::BackTab
            | FocusReason::Shortcut
            | FocusReason::Programmatic => self.accepts_tab(),
        }
    }
}
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
    /// Membership index for [`Self::registration_order`].
    ///
    /// Registration used `Vec::contains`, which made registering n widgets O(n²) comparisons;
    /// this is O(log n) and keeps the canonical order untouched.
    registered: BTreeSet<ObjectId>,
    /// Per-control focus policies. A control absent from the map keeps
    /// [`FocusPolicy::StrongFocus`], which is what every control did before the policy existed.
    policies: HashMap<ObjectId, FocusPolicy>,
    /// W-P02: each widget's slot in [`Self::focusable_widgets`], so a traversal's start position
    /// is a map hit instead of an O(n) `position` scan.
    ///
    /// Rebuilt in the same pass that mutates the effective order (registration, unregister's
    /// partition, reorder), so it cannot drift from the list it indexes. The first occurrence of
    /// a duplicated id wins — `set_focus_order` accepts duplicates — matching the `position`
    /// scan this replaces.
    positions: HashMap<ObjectId, usize>,
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
            registered: BTreeSet::new(),
            policies: HashMap::new(),
            positions: HashMap::new(),
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
        if !self.registered.insert(id) {
            // Already registered: the effective order is unchanged, so re-deriving it would only
            // allocate and sort for nothing.
            return;
        }
        self.registration_order.push(id);
        // Under `TabOrder` the effective order is the canonical one, so append in place rather
        // than re-deriving (and cloning) the whole list on every registration. A spatial policy
        // still re-sorts, because the new widget's position may belong before existing entries.
        match self.traversal_strategy {
            FocusTraversalStrategy::TabOrder => {
                self.focusable_widgets.push(id);
                self.positions.entry(id).or_insert(self.focusable_widgets.len() - 1);
            }
            _ => self.reorder_by_strategy(),
        }
    }

    /// Remove a widget from the focusable order.
    pub fn unregister_focusable(&mut self, id: ObjectId) {
        if !self.registered.remove(&id) {
            return;
        }
        // W-P02: the two `retain` passes are two full scans with a compare-and-shift per
        // element; each list is now partitioned once in place, and the effective order's pass
        // rebuilds the position index for the survivors as it assigns their new slots.
        Self::partition_out(&mut self.registration_order, id);
        Self::partition_indexed(&mut self.focusable_widgets, &mut self.positions, id);
        self.policies.remove(&id);
        // EV-X04: the position map had no removal path, so a recreated control that reused an id
        // inherited the dead control's coordinates (or was ordered at `(0, 0)`) and the map grew
        // without bound across create/destroy cycles.
        self.widget_positions.remove(&id);
        // A removal cannot move the remaining widgets relative to each other, so the effective
        // order is already correct under `TabOrder`. Spatial strategies keep their sort because
        // the positions of the remaining widgets are unchanged.
        if !matches!(self.traversal_strategy, FocusTraversalStrategy::TabOrder) {
            self.reorder_by_strategy();
        }
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
        self.registered = self.registration_order.iter().copied().collect();
        // A full replacement must respect an enabled spatial policy on the next
        // Tab too, exactly as dynamic registration does.
        self.reorder_by_strategy();
    }

    /// Moves focus to the next widget in the tab order (wraps around).
    /// Returns the newly focused widget, if any.
    pub fn focus_next(&mut self) -> Option<ObjectId> {
        if self.focusable_widgets.is_empty() {
            return None;
        }
        let current = self.focused_widget;
        let pos = current.and_then(|id| self.positions.get(&id).copied());
        let next_index = match pos {
            Some(idx) => (idx + 1) % self.focusable_widgets.len(),
            None => 0,
        };
        let next = self.focusable_widgets[next_index];
        self.set_focus(next);
        Some(next)
    }

    /// Move focus to the previous widget in tab order (wraps around).
    /// Returns the newly focused widget, if any.
    pub fn focus_previous(&mut self) -> Option<ObjectId> {
        self.focus_next_matching(false, |_| true)
    }

    /// Move focus forward or backward to the next widget `accept` allows, wrapping around.
    ///
    /// # Why the order is not filtered instead
    ///
    /// [`Widget::is_focusable`](crate::widget::Widget::is_focusable) deliberately keeps a
    /// disabled control in the tab order — a user must be able to move *past* it — so the skip
    /// belongs in the traversal, not in the registration. Without it, Tab landed on a disabled
    /// (or hidden) control, told the IME to open a candidate window for it, and delivered the
    /// following keystrokes to a handler that returns early: the key was swallowed with no
    /// visible focus change anywhere.
    ///
    /// `accept` is asked at most once per candidate and the walk stops after a full cycle, so a
    /// caller whose predicate rejects everything gets `None` and focus does not move.
    pub fn focus_next_matching(
        &mut self,
        forward: bool,
        mut accept: impl FnMut(ObjectId) -> bool,
    ) -> Option<ObjectId> {
        let len = self.focusable_widgets.len();
        if len == 0 {
            return None;
        }
        let start = self.focused_widget.and_then(|id| self.positions.get(&id).copied());
        for step in 1..=len {
            let index = match (start, forward) {
                (Some(index), true) => (index + step) % len,
                (Some(index), false) => (index + len - (step % len)) % len,
                (None, true) => step - 1,
                (None, false) => len - step,
            };
            let candidate = self.focusable_widgets[index];
            if accept(candidate) {
                self.set_focus(candidate);
                return Some(candidate);
            }
        }
        None
    }

    // --- Accessibility callback wiring ---

    /// Sets the input routes `id` accepts, replacing any previous policy.
    ///
    /// A policy does not add or remove the control from the tab list: registration decides
    /// *whether* the control is in the order, and the policy decides *which routes* may land on
    /// it. That split is what lets [`FocusPolicy::NoFocus`] keep a control's place while
    /// disabling it temporarily, and what makes reverting to [`FocusPolicy::StrongFocus`] a
    /// single call rather than a re-registration.
    pub fn set_focus_policy(&mut self, id: ObjectId, policy: FocusPolicy) {
        if policy == FocusPolicy::default() {
            // The default is the absence of an entry, so storing it would be a second
            // representation of the same fact and a map that grows for no reason.
            self.policies.remove(&id);
        } else {
            self.policies.insert(id, policy);
        }
    }

    /// The policy for `id`, defaulting to [`FocusPolicy::StrongFocus`] when none was set.
    pub fn focus_policy(&self, id: ObjectId) -> FocusPolicy {
        self.policies.get(&id).copied().unwrap_or_default()
    }

    /// Whether `id` accepts focus arriving from `reason`.
    pub fn accepts_focus(&self, id: ObjectId, reason: FocusReason) -> bool {
        self.focus_policy(id).accepts(reason)
    }

    /// Moves focus forward or backward to the next widget that accepts `reason`, wrapping.
    ///
    /// The policy-aware counterpart of [`Self::focus_next_matching`]: a pointer click that
    /// asks for the next focusable cannot land on a [`FocusPolicy::TabFocus`] control, and Tab
    /// cannot land on a [`FocusPolicy::ClickFocus`] one. Returns the newly focused widget, or
    /// `None` when no candidate in the whole ring accepts the reason (focus then does not move,
    /// which is the same rule `focus_next_matching` documents).
    pub fn focus_next_for(&mut self, forward: bool, reason: FocusReason) -> Option<ObjectId> {
        let len = self.focusable_widgets.len();
        if len == 0 {
            return None;
        }
        let start = self.focused_widget.and_then(|id| self.positions.get(&id).copied());
        let mut chosen = None;
        for step in 1..=len {
            let index = match (start, forward) {
                (Some(index), true) => (index + step) % len,
                (Some(index), false) => (index + len - (step % len)) % len,
                (None, true) => step - 1,
                (None, false) => len - step,
            };
            let candidate = self.focusable_widgets[index];
            if self.accepts_focus(candidate, reason) {
                chosen = Some(candidate);
                break;
            }
        }
        if let Some(candidate) = chosen {
            self.set_focus(candidate);
        }
        chosen
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
        // W-P02: keep the traversal-start index in step with the effective order — one pass,
        // first occurrence wins for a duplicated id, exactly like the `position` scan did.
        self.positions.clear();
        for (index, &id) in self.focusable_widgets.iter().enumerate() {
            self.positions.entry(id).or_insert(index);
        }
    }

    /// Removes every occurrence of `id` from `order` in one in-place pass, preserving the
    /// relative order of the survivors (W-P02: the same outcome as `Vec::retain`, written as an
    /// explicit partition so the effective order's pass can also rebuild the position index).
    fn partition_out(order: &mut Vec<ObjectId>, id: ObjectId) {
        let mut write = 0;
        for read in 0..order.len() {
            let current = order[read];
            if current == id {
                continue;
            }
            if write != read {
                order[write] = current;
            }
            write += 1;
        }
        order.truncate(write);
    }

    /// [`Self::partition_out`] for the effective order, rebuilding `positions` in the same pass:
    /// each survivor's new slot is recorded as the pass assigns it, so the index never needs a
    /// second traversal.
    fn partition_indexed(
        order: &mut Vec<ObjectId>,
        positions: &mut HashMap<ObjectId, usize>,
        id: ObjectId,
    ) {
        positions.clear();
        let mut write = 0;
        for read in 0..order.len() {
            let current = order[read];
            if current == id {
                continue;
            }
            if write != read {
                order[write] = current;
            }
            positions.entry(current).or_insert(write);
            write += 1;
        }
        order.truncate(write);
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

    /// An unconfigured control accepts both routes, which is what every control did before
    /// policies existed.
    #[test]
    fn the_default_policy_accepts_both_routes() {
        let fm = FocusManager::new();
        assert_eq!(fm.focus_policy(1), FocusPolicy::StrongFocus);
        assert!(fm.accepts_focus(1, FocusReason::Pointer));
        assert!(fm.accepts_focus(1, FocusReason::Tab));
        assert!(fm.accepts_focus(1, FocusReason::Programmatic));
    }

    /// A tab-only control is skipped by the pointer route and reached by keyboard navigation.
    #[test]
    fn a_tab_only_policy_is_skipped_by_the_pointer_route() {
        let mut fm = FocusManager::new();
        fm.set_focus_order(vec![1, 2]);
        fm.set_focus_policy(1, FocusPolicy::TabFocus);

        // Not focused yet: the pointer route starts at index 0, rejects 1, then takes 2.
        assert_eq!(fm.focus_next_for(true, FocusReason::Pointer), Some(2));
        fm.clear_focus();
        assert_eq!(fm.focus_next_for(true, FocusReason::Tab), Some(1));
    }

    /// A no-focus control is skipped by both routes; if nothing accepts, focus does not move.
    #[test]
    fn a_no_focus_policy_is_skipped_by_both_routes() {
        let mut fm = FocusManager::new();
        fm.set_focus_order(vec![1]);
        fm.set_focus_policy(1, FocusPolicy::NoFocus);
        assert_eq!(fm.focus_next_for(true, FocusReason::Tab), None);
        assert_eq!(fm.focus_next_for(true, FocusReason::Pointer), None);
        assert_eq!(fm.focused_widget(), None);
    }

    /// Unregistering drops the policy with the control, so a reused id starts clean.
    #[test]
    fn unregistering_a_control_drops_its_policy() {
        let mut fm = FocusManager::new();
        fm.register_focusable(7);
        fm.set_focus_policy(7, FocusPolicy::ClickFocus);
        assert_eq!(fm.focus_policy(7), FocusPolicy::ClickFocus);
        fm.unregister_focusable(7);
        assert_eq!(fm.focus_policy(7), FocusPolicy::StrongFocus);
    }

    /// W-P02: traversal starts from the position index, so the start slot must stay exact across
    /// unregister, reorders and full order replacements — and an unknown start (a focused widget
    /// that is not in the order) still begins at index 0, as the `position` scan did.
    #[test]
    fn traversal_positions_stay_exact_across_mutations() {
        let mut fm = FocusManager::new();
        fm.register_focusable(10);
        fm.register_focusable(20);
        fm.register_focusable(30);

        fm.set_focus(30);
        fm.unregister_focusable(20);
        // 30 now sits at index 1 of [10, 30]: forward wraps to 10, backward lands on 10 too.
        assert_eq!(fm.focus_next(), Some(10));
        fm.set_focus(10);
        assert_eq!(fm.focus_previous(), Some(30));

        // A full replacement re-derives the positions: forward runs 30 → 40 → 10 → 30.
        fm.set_focus_order(vec![30, 40, 10]);
        fm.set_focus(30);
        assert_eq!(fm.focus_next(), Some(40));
        assert_eq!(fm.focus_next(), Some(10));
        assert_eq!(fm.focus_next(), Some(30));

        // A focused widget that is not registered starts from index 0.
        fm.set_focus(99);
        assert_eq!(fm.focus_next(), Some(30));
    }

    /// W-P02: the position index follows a spatial reorder, so traversal after a strategy
    /// switch starts from the right slot.
    #[test]
    fn spatial_reorder_keeps_traversal_positions_exact() {
        let mut fm = FocusManager::new();
        fm.register_focusable(2);
        fm.register_focusable(1);
        fm.set_widget_position(1, 0, 0);
        fm.set_widget_position(2, 10, 0);
        fm.set_traversal_strategy(FocusTraversalStrategy::RowMajor);
        assert_eq!(fm.focusable_widgets(), &[1, 2], "RowMajor sorts spatially");

        fm.set_focus(2);
        assert_eq!(fm.focus_next(), Some(1), "index 1 wraps to index 0");
        fm.set_focus(1);
        assert_eq!(fm.focus_next(), Some(2));
    }

    /// W-P02: `focus_next_matching` and `focus_next_for` share the position index, so their
    /// walks stay exact after removals, and a rejecting predicate skips without moving focus.
    #[test]
    fn matching_traversals_use_the_position_index_after_removals() {
        let mut fm = FocusManager::new();
        for id in [10, 20, 30, 40] {
            fm.register_focusable(id);
        }
        fm.unregister_focusable(20);
        fm.unregister_focusable(40);

        fm.set_focus(30);
        // [10, 30]: forward from 30 wraps to 10; backward from 10 reaches 30.
        assert_eq!(fm.focus_next_matching(true, |_| true), Some(10));
        fm.set_focus(10);
        assert_eq!(fm.focus_next_matching(false, |_| true), Some(30));

        // From 30, only 10 is acceptable: backward stops there without moving focus otherwise.
        fm.set_focus(30);
        assert_eq!(fm.focus_next_matching(false, |id| id == 10), Some(10));
        assert_eq!(fm.focus_next_for(true, FocusReason::Programmatic), Some(30));
    }
}
