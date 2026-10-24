// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Row-level invalidation for a declarative list.
//!
//! # The gap this closes
//!
//! A [`View`](crate::view::View) rebuilds by describing its whole tree and diffing that against
//! the previous one. For a list that is usually the right trade: the diff stops at an unchanged
//! subtree, so an idle rebuild is cheap. It is the wrong trade for a **large list whose rows
//! arrive from a model**: inserting one row at index 400 of 10 000 makes the build walk every
//! row it did not change, and the diff walk them again to discover that.
//!
//! A [`RowSignal`] narrows that work. It says *which keyed container changed* and *how*, so the
//! engine can diff only that container's subtree. The build still describes the tree it
//! declares — the view stays a pure function of its state — but the **comparison** is scoped,
//! which is where the O(tree) cost was.
//!
//! # What the caller promises
//!
//! A signal is an assertion: "nothing outside `container`'s subtree changed since the last
//! build". The engine cannot verify that cheaply — verifying it is the full diff the signal
//! exists to avoid — so it is the producer's contract. A producer that gets it wrong sees a
//! stale tree, which is why the type is named for the assertion rather than for the mechanism.
//!
//! A signal whose container is not present in both trees (the key changed, the branch was
//! removed) falls back to a full diff. That is the safe direction: a wrong signal costs a
//! full rebuild, never a missed one.
//!
//! # Shape
//!
//! The change vocabulary mirrors what a model reports — insert, remove, move, update, reset —
//! rather than the patches the diff emits. A model knows rows were inserted; it does not know
//! which controls those rows materialised into. Translating between the two is precisely the
//! diff's job, so the signal never names an `ObjectId`.

use crate::compat::{String, Vec};

/// One structural change inside a keyed container.
///
/// Indices are positions in the **new** row list for [`Insert`](Self::Insert), and positions in
/// the **pre-change** list for [`Remove`](Self::Remove) and [`Move`](Self::Move), matching the
/// convention a model's own change notification carries. The engine only needs to know that the
/// container changed; it re-derives the exact patch set from the two trees.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowChange {
    /// `count` rows were inserted starting at `at`.
    Insert {
        /// First new index.
        at: usize,
        /// How many rows appeared.
        count: usize,
    },
    /// `count` rows were removed starting at `at`.
    Remove {
        /// First removed index.
        at: usize,
        /// How many rows disappeared.
        count: usize,
    },
    /// `count` rows moved from `from` to `to`.
    Move {
        /// Source index before the move.
        from: usize,
        /// Destination index after the move.
        to: usize,
        /// How many rows moved.
        count: usize,
    },
    /// `count` existing rows changed their contents starting at `at`.
    Update {
        /// First changed index.
        at: usize,
        /// How many rows changed.
        count: usize,
    },
    /// The whole list was replaced: every row must be considered changed.
    Reset,
}

/// A scoped invalidation: `container` changed, and this is how.
///
/// `container` is the [`Node::key`](crate::view::Node::key) of the list's host node. Keys are
/// only required to be unique among siblings, so a document with two lists that share a key
/// would make the scope ambiguous; the engine resolves the first match and the contract is the
/// same one [`Node::duplicate_sibling_keys`](crate::view::Node::duplicate_sibling_keys) reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowSignal {
    /// The key of the container whose rows changed.
    pub container: String,
    /// The changes, in the order the producer observed them. The engine does not interpret
    /// individual indices — the two trees decide the patches — but keeps the list so a
    /// diagnostic or a test can assert what was reported.
    pub changes: Vec<RowChange>,
}

impl RowSignal {
    /// A signal for `container` with no changes yet; add them with [`Self::push`].
    pub fn new(container: impl Into<String>) -> Self {
        Self { container: container.into(), changes: Vec::new() }
    }

    /// A whole-list reset for `container`.
    pub fn reset(container: impl Into<String>) -> Self {
        Self { container: container.into(), changes: Vec::from([RowChange::Reset]) }
    }

    /// Records a change and returns `self`, for a chain at the reporting site.
    pub fn push(mut self, change: RowChange) -> Self {
        self.changes.push(change);
        self
    }

    /// Whether this signal carries no change, in which case there is nothing to rebuild.
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// The changes as a slice, for a caller that reports rather than builds.
    pub fn changes(&self) -> &[RowChange] {
        &self.changes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_signal_records_the_changes_it_was_given() {
        let signal = RowSignal::new("tasks")
            .push(RowChange::Insert { at: 3, count: 2 })
            .push(RowChange::Update { at: 0, count: 1 });
        assert_eq!(signal.container, "tasks");
        assert_eq!(signal.changes().len(), 2);
        assert_eq!(signal.changes()[0], RowChange::Insert { at: 3, count: 2 });
        assert!(!signal.is_empty());
    }

    #[test]
    fn an_empty_signal_answers_is_empty_and_a_reset_is_not_empty() {
        assert!(RowSignal::new("tasks").is_empty());
        assert!(!RowSignal::reset("tasks").is_empty());
        assert_eq!(RowSignal::reset("tasks").changes(), &[RowChange::Reset]);
    }
}
