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
//!
//! # The model half (VW-001)
//!
//! [`RowChange`] says *that* a list changed; a [`RowModel`] is what a list is built **from**.
//! The interface is deliberately the three things a model-backed control needs and nothing
//! more: an **index** (`0..len`), a **role** ([`RowRole`], what a given datum of a row is for),
//! and **change notification** ([`RowModel::changes`], published as a [`RowSignal`]). A row
//! value is a [`CapabilityValue`], so the view can put it straight into a node property
//! without inventing a second value vocabulary.
//!
//! This is a *model*, not a second view layer: the model reports rows, the view decides what
//! nodes a row becomes ([`Node::children_model`](crate::view::Node::children_model)), and the
//! engine does the diff. A list that has no model keeps using
//! [`Node::children_keyed`](crate::view::Node::children_keyed) over its own array — that path is
//! the default implementation and is byte-for-byte what it was, so nothing has to adopt a model
//! to keep working.
//!
//! # Delegation is out of scope, on purpose
//!
//! A full model/view contract also names a *delegate* that draws a row. Drawing belongs to the
//! control (a control knows its own painters and its own available widget set), so the delegate
//! would be a pass-through: the view would turn a row into a `Node`, the control would draw that
//! node's control with its own code. Leaving it out keeps this interface implementable by a
//! plain data source, which is the whole point of "lightweight".

use crate::compat::{String, Vec};
use crate::widget::capability::CapabilityValue;

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

/// What a datum of a row is for: the "role" half of the [`RowModel`] interface.
///
/// The built-in roles are the ones a list control actually asks for. The property name in
/// parentheses is the conventional one a row builder feeds, not a rule: a view decides the
/// mapping when it builds the row, so the model never has to know a control's property
/// vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowRole {
    /// The text the row shows (`text`).
    Display,
    /// The image or icon the row shows (`icon`).
    Decoration,
    /// The row's tooltip (`tooltip`).
    ToolTip,
    /// Whether the row is selected (`selected`).
    Selected,
    /// Whether the row's children are expanded (`expanded`), for a tree-shaped row.
    Expanded,
}

/// A read-only row model: the index + role interface, plus change notification (VW-001).
///
/// # Contract
///
/// [`len`](Self::len), [`key`](Self::key) and [`value`](Self::value) are pure reads: the same
/// model state must answer the same way, because a view builds from them and the diff relies on
/// that being a function of state. A model that mutates while a build runs would make the
/// description non-deterministic, exactly as a [`View`](crate::view::View) that reads a clock
/// would.
///
/// [changes](Self::changes) is the notification half and is **not** read by a build: the
/// producer publishes it through [`RowSignal::of_model`], and the host applies the scoped diff.
/// The default answers "nothing changed", which is the honest answer for a model that has no
/// notification channel yet — it still builds, it just always goes through a full rebuild.
///
/// A model is also usable by reference (`&dyn RowModel`), which is what lets a view hold one
/// handle and describe from it every frame.
pub trait RowModel {
    /// How many rows the model currently has.
    fn len(&self) -> usize;

    /// The stable identity of row `index`, or `None` when the row has none.
    ///
    /// The key becomes the node's [`key`](crate::view::Node::key), which is what makes a row's
    /// control survive a neighbour being inserted: a position is not an identity (BLUE18 rule
    /// #87), so a model that can supply a key should. `None` is honoured as "match me by
    /// position" and the diff reports the positional matches it had to fall back on.
    fn key(&self, index: usize) -> Option<String>;

    /// The value of `role` for row `index`, or `None` when the row does not carry that role.
    fn value(&self, index: usize, role: RowRole) -> Option<&CapabilityValue>;

    /// Changes observed since the host last published them, in observation order.
    ///
    /// Empty by default: a model without a notification channel reports nothing rather than
    /// claiming an unchanged list, which is why the caller to publish is
    /// [`RowSignal::of_model`] rather than a build-time drain.
    fn changes(&self) -> &[RowChange] {
        &[]
    }

    /// Whether the model has no rows.
    ///
    /// Provided so a view can branch without `len() == 0` at every call site; a model whose
    /// rows are cheap to count need not override it.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// One row addressed by index, together with the model it came from.
///
/// This is what [`Node::children_model`](crate::view::Node::children_model) hands the row
/// builder, so the builder reads `index`/`key`/`value(role)` without having to be generic over
/// the model type: a view keeps the model behind a `&dyn RowModel` and still writes one closure.
#[derive(Clone, Copy)]
pub struct RowRef<'a> {
    model: &'a dyn RowModel,
    index: usize,
}

impl<'a> RowRef<'a> {
    /// Address row `index` of `model`.
    pub fn new(model: &'a dyn RowModel, index: usize) -> Self {
        Self { model, index }
    }

    /// The row's position in the model.
    pub fn index(&self) -> usize {
        self.index
    }

    /// The row's identity, as [`RowModel::key`] answers it.
    pub fn key(&self) -> Option<String> {
        self.model.key(self.index)
    }

    /// The value of `role`, as [`RowModel::value`] answers it.
    pub fn value(&self, role: RowRole) -> Option<&'a CapabilityValue> {
        self.model.value(self.index, role)
    }

    /// The row's display text, the role a plain list shows.
    pub fn text(&self) -> Option<&'a CapabilityValue> {
        self.value(RowRole::Display)
    }
}

impl core::fmt::Debug for RowRef<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RowRef").field("index", &self.index).finish_non_exhaustive()
    }
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

    /// The signal a model's own change list produces for `container` (VW-001).
    ///
    /// A model reports changes in the [`RowChange`] vocabulary already, so publishing one is a
    /// copy of that list rather than a second translation — the list is a handful of entries,
    /// while the rows it describes are not copied at all. A model that reports nothing yields
    /// an empty signal ([`Self::is_empty`]), which the host treats as "nothing to rebuild".
    ///
    /// ```
    /// use rust_widgets::view::{RowChange, RowModel, RowRole, RowSignal};
    /// use rust_widgets::widget::capability::CapabilityValue;
    ///
    /// struct Names {
    ///     values: Vec<CapabilityValue>,
    ///     changes: Vec<RowChange>,
    /// }
    ///
    /// impl RowModel for Names {
    ///     fn len(&self) -> usize { self.values.len() }
    ///     fn key(&self, index: usize) -> Option<String> { Some(index.to_string()) }
    ///     fn value(&self, index: usize, role: RowRole) -> Option<&CapabilityValue> {
    ///         match role { RowRole::Display => self.values.get(index), _ => None }
    ///     }
    ///     fn changes(&self) -> &[RowChange] { &self.changes }
    /// }
    ///
    /// let model = Names {
    ///     values: vec![CapabilityValue::String("a".into())],
    ///     changes: vec![RowChange::Insert { at: 1, count: 1 }],
    /// };
    /// let signal = RowSignal::of_model("rows", &model);
    /// assert_eq!(signal.container, "rows");
    /// assert_eq!(signal.changes(), &[RowChange::Insert { at: 1, count: 1 }]);
    /// ```
    pub fn of_model(container: impl Into<String>, model: &dyn RowModel) -> Self {
        Self { container: container.into(), changes: Vec::from(model.changes()) }
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
    use crate::view::Node;

    /// A tiny model: one display value per row, optional keys, and a change list.
    struct Names {
        values: Vec<CapabilityValue>,
        changes: Vec<RowChange>,
        keys: bool,
    }

    impl RowModel for Names {
        fn len(&self) -> usize {
            self.values.len()
        }

        fn key(&self, index: usize) -> Option<String> {
            if self.keys {
                Some(format!("n{index}"))
            } else {
                None
            }
        }

        fn value(&self, index: usize, role: RowRole) -> Option<&CapabilityValue> {
            match role {
                RowRole::Display => self.values.get(index),
                _ => None,
            }
        }

        fn changes(&self) -> &[RowChange] {
            &self.changes
        }
    }

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

    /// VW-001: a model is asked by index and role, and the view decides the node a row becomes.
    #[test]
    fn a_model_builds_one_keyed_child_per_row() {
        let model = Names {
            values: Vec::from([
                CapabilityValue::String(String::from("a")),
                CapabilityValue::String(String::from("b")),
            ]),
            changes: Vec::new(),
            keys: true,
        };
        let list = Node::new("list").children_model(&model, |row| {
            let mut node = Node::new("label");
            if let Some(text) = row.text() {
                node = node.prop("text", text.clone());
            }
            node
        });

        assert_eq!(list.children.len(), 2, "one child per row");
        assert_eq!(list.children[0].key_str(), Some("n0"), "the model supplies the identity");
        assert_eq!(list.children[1].key_str(), Some("n1"));
        assert_eq!(
            list.children[1].prop_value("text"),
            Some(&CapabilityValue::String(String::from("b"))),
            "the Display role reached the property"
        );
    }

    /// VW-001: a model without keys degrades to positional matching instead of inventing one.
    #[test]
    fn a_model_without_keys_leaves_the_child_keyless() {
        let model = Names {
            values: Vec::from([CapabilityValue::Bool(true)]),
            changes: Vec::new(),
            keys: false,
        };
        let list = Node::new("list").children_model(&model, |row| {
            Node::new("checkbox").prop(
                "checked",
                row.value(RowRole::Display).cloned().unwrap_or(CapabilityValue::Bool(false)),
            )
        });

        assert_eq!(list.children.len(), 1);
        assert_eq!(list.children[0].key_str(), None, "no key was invented for the row");
        assert_eq!(
            list.children[0].prop_value("checked"),
            Some(&CapabilityValue::Bool(true)),
            "a non-Display role is not the text"
        );
    }

    /// VW-001: the model's change notification is publishable as the existing row signal.
    #[test]
    fn a_model_signal_carries_the_changes_the_model_reported() {
        let model = Names {
            values: Vec::from([CapabilityValue::String(String::from("a"))]),
            changes: Vec::from([RowChange::Update { at: 1, count: 2 }]),
            keys: true,
        };
        let signal = RowSignal::of_model("rows", &model);
        assert_eq!(signal.container, "rows");
        assert_eq!(signal.changes(), &[RowChange::Update { at: 1, count: 2 }]);
        assert!(!signal.is_empty());
        assert!(!model.is_empty(), "one row, so the model is not empty");

        let quiet = Names { values: Vec::new(), changes: Vec::new(), keys: true };
        assert!(RowSignal::of_model("rows", &quiet).is_empty(), "a quiet model reports nothing");
        assert!(quiet.is_empty(), "the provided len-based is_empty is the default");
    }
}
