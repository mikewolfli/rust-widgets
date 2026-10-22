// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Stack layout manager — shows one child page at a time.
use super::Layout;
use crate::compat::{Any, Vec};
use crate::core::{ObjectId, Rect};
/// Stack layout that shows one child page at a time.
#[derive(Debug)]
pub struct StackLayout {
    items: Vec<ObjectId>,
    current: usize,
}
impl StackLayout {
    /// Create stack layout with no pages.
    pub fn new() -> Self {
        Self { items: Vec::new(), current: 0 }
    }
    /// Select visible page by index.
    ///
    /// An index past the end selects the **last** page rather than being ignored. Ignoring it
    /// left the previously visible page on screen, so a caller that computed an index from stale
    /// data saw a page it had not asked for while `current_index()` disagreed with the selection
    /// it thought it had made. Clamping keeps `current_index()` and the drawn page in step.
    ///
    /// Use [`Self::try_set_current_index`] when the caller needs to know the request was out of
    /// range.
    pub fn set_current_index(&mut self, index: usize) {
        self.current = index.min(self.items.len().saturating_sub(1));
    }

    /// Selects a page, reporting whether `index` addressed one.
    ///
    /// Unlike [`Self::set_current_index`], an out-of-range index leaves the current page alone
    /// and returns `false`.
    pub fn try_set_current_index(&mut self, index: usize) -> bool {
        if index < self.items.len() {
            self.current = index;
            true
        } else {
            false
        }
    }

    /// Inserts `widget_id` at `index`, shifting the pages at and after it.
    ///
    /// Returns `false` when `index` is past the end (an insert at `len` is an append, so
    /// `index == len` is accepted). The current page follows its widget: inserting before it
    /// shifts `current` by one so the same page stays visible.
    pub fn insert_at(&mut self, index: usize, widget_id: ObjectId) -> bool {
        if index > self.items.len() {
            return false;
        }
        // Whether the current index addressed a page *before* the insert. An empty stack has
        // `current == 0`, which must not be shifted to 1 by an insert at 0.
        let had_current = self.current < self.items.len();
        self.items.insert(index, widget_id);
        if had_current && index <= self.current {
            self.current += 1;
        }
        true
    }

    /// Removes the page at `index`, returning it.
    ///
    /// The current page stays on the same widget where possible: removing a page before it
    /// shifts `current` down, and removing the current page selects the one that took its place
    /// (or the last page, when the removed one was last).
    pub fn remove_at(&mut self, index: usize) -> Option<ObjectId> {
        if index >= self.items.len() {
            return None;
        }
        let removed = self.items.remove(index);
        if index < self.current {
            self.current -= 1;
        } else if self.current >= self.items.len() {
            self.current = self.items.len().saturating_sub(1);
        }
        Some(removed)
    }
    /// Returns the number of pages (widgets) in the stack.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }
    /// Returns the index of the currently visible page.
    pub fn current_index(&self) -> usize {
        self.current
    }
    /// Returns the widget id at the given index, if in bounds.
    pub fn item_at(&self, index: usize) -> Option<ObjectId> {
        self.items.get(index).copied()
    }
}
crate::impl_default_via_new!(StackLayout);
impl Layout for StackLayout {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn add_widget(&mut self, widget_id: ObjectId, _stretch: u32) {
        self.items.push(widget_id);
    }
    fn remove_widget(&mut self, widget_id: ObjectId) {
        self.items.retain(|id| *id != widget_id);
        if self.current >= self.items.len() {
            self.current = self.items.len().saturating_sub(1);
        }
    }
    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) {
        if let Some(widget_id) = self.items.get(self.current) {
            widgets(*widget_id, rect);
        }
    }
    fn child_ids(&self) -> Vec<ObjectId> {
        self.items.clone()
    }
    fn has_child(&self, id: ObjectId) -> bool {
        self.items.contains(&id)
    }
    fn clear(&mut self) {
        self.items.clear();
        self.current = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack_of(count: u64) -> StackLayout {
        let mut layout = StackLayout::new();
        for id in 1..=count {
            layout.add_widget(id, 0);
        }
        layout
    }

    /// An out-of-range selection reports the page it actually shows.
    ///
    /// It used to be ignored, which left the old page visible while the caller believed it had
    /// selected a different one.
    #[test]
    fn an_out_of_range_selection_clamps_to_the_last_page() {
        let mut layout = stack_of(3);
        layout.set_current_index(99);
        assert_eq!(layout.current_index(), 2, "the index and the shown page must agree");

        layout.set_current_index(1);
        assert_eq!(layout.current_index(), 1);
    }

    /// The strict form reports the miss and leaves the selection alone.
    #[test]
    fn try_set_current_index_reports_an_out_of_range_request() {
        let mut layout = stack_of(2);
        layout.set_current_index(1);
        assert!(!layout.try_set_current_index(5), "5 is not a page of a two-page stack");
        assert_eq!(layout.current_index(), 1, "a refused selection changes nothing");
        assert!(layout.try_set_current_index(0));
        assert_eq!(layout.current_index(), 0);
    }

    /// Inserting before the current page keeps the same page visible.
    #[test]
    fn insert_at_keeps_the_current_page_selected() {
        let mut layout = stack_of(3);
        layout.set_current_index(1);
        assert!(layout.insert_at(0, 99));
        assert_eq!(layout.current_index(), 2, "the page that was at 1 moved to 2");
        assert_eq!(layout.item_at(2), Some(2), "and it is still the same widget");

        // An insert into an empty stack must not shift the index off the first page.
        let mut empty = StackLayout::new();
        assert!(empty.insert_at(0, 7));
        assert_eq!(empty.current_index(), 0);
        assert_eq!(empty.item_at(0), Some(7));
    }

    /// `insert_at` refuses an index past the end; `item_count` is the append position.
    #[test]
    fn insert_at_refuses_an_index_past_the_end() {
        let mut layout = stack_of(2);
        assert!(!layout.insert_at(3, 99), "3 is past the append position of a two-page stack");
        assert!(layout.insert_at(2, 99), "2 is the append position");
        assert_eq!(layout.item_count(), 3);
    }

    /// Removing before the current page shifts the selection with it.
    #[test]
    fn remove_at_keeps_the_same_widget_selected() {
        let mut layout = stack_of(3);
        layout.set_current_index(2);
        assert_eq!(layout.remove_at(0), Some(1));
        assert_eq!(layout.current_index(), 1, "the same widget is now at 1");
        assert_eq!(layout.item_at(1), Some(3));
    }

    /// Removing the current page selects its successor, or the new last page.
    #[test]
    fn remove_at_current_selects_the_next_page() {
        let mut layout = stack_of(3);
        layout.set_current_index(1);
        assert_eq!(layout.remove_at(1), Some(2));
        assert_eq!(layout.current_index(), 1, "the successor took the slot");
        assert_eq!(layout.item_at(1), Some(3));

        assert_eq!(layout.remove_at(1), Some(3));
        assert_eq!(layout.current_index(), 0, "an empty tail falls back to the last page");
    }
}
