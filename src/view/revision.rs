// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Container-level revision reuse for `View::build` (VW-01).
//!
//! # The gap this closes
//!
//! A [`RowSignal`](crate::view::RowSignal) narrows the **comparison** to one keyed
//! container, but [`View::build`](crate::view::View::build) still describes every row it
//! did not change. For a large model-backed list that build walk is the remaining
//! O(rows) cost: the diff is scoped, the description is not.
//!
//! This module supplies the missing channel. A container declared with
//! [`Node::revision`](crate::view::Node::revision) says "this subtree is version `n`".
//! While a build runs, the engine installs the **previous** tree here; a container whose
//! key, widget type and revision all match asks for the retained children instead of
//! running its `make` closure. The rows are therefore **not rebuilt**, and the tree the
//! build returns is identical to the one a full description would have produced (the
//! caller's revision is the assertion that it is), so `diff`'s equality probe short
//! circuits and the apply/identity machinery needs no change.
//!
//! # Why a thread-local scope rather than a parameter
//!
//! `View::build` takes no engine argument — its signature is fixed and its contract is
//! purity. The same constraint made [`Breakpoint::current`](crate::view::Breakpoint::current)
//! a scoped thread-local, and this follows that precedent: the value is in force for one
//! build, read from the builder that needs it, and restored on exit even on unwind.
//!
//! # What the caller promises
//!
//! The revision must change whenever **any** node in the subtree would describe
//! differently. Nothing here can verify that cheaply (verifying it is the full build the
//! channel exists to avoid), so it is the producer's contract, exactly as
//! [`RowSignal`](crate::view::RowSignal) is. A wrong revision shows a stale subtree; a
//! key that appears under two parents is treated as ambiguous and is never reused, which
//! is the safe direction.

use crate::compat::{HashMap, Rc, Vec};

use super::node::Node;

/// The previous tree, plus the lazy `key -> path` index the queries use.
struct RetainedScope {
    tree: Rc<Node>,
    /// Built on the first query: only containers that declared a revision are indexed.
    ///
    /// `None` as a value means the key is **ambiguous** (two revisioned nodes claimed it),
    /// so no query may resolve it — the same rule the diff applies to duplicate sibling
    /// keys, extended to the whole tree because this index is global.
    index: Option<HashMap<crate::compat::String, Option<Vec<usize>>>>,
}

thread_local! {
    /// The scope in force for this thread's current build.
    #[allow(clippy::missing_const_for_thread_local)]
    static RETAINED: core::cell::RefCell<Option<RetainedScope>> =
        const { core::cell::RefCell::new(None) };
}

/// Runs `f` with `previous` available for container reuse, restoring the outer scope after.
///
/// A build with no previous tree (the first mount) passes `None`, and every
/// [`Node::children_keyed`](crate::view::Node::children_keyed) then builds normally.
/// Restoring on exit — including on unwind, via the scope guard — is what keeps a reused
/// container from leaking into a later independent build.
pub(crate) fn with_retained_subtrees<R>(previous: Option<Rc<Node>>, f: impl FnOnce() -> R) -> R {
    /// Restores the outer scope however the build exits.
    struct Restore(Option<RetainedScope>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let previous = self.0.take();
            let _ = RETAINED.try_with(|cell| {
                *cell.borrow_mut() = previous;
            });
        }
    }
    let saved = RETAINED
        .try_with(|cell| cell.replace(previous.map(|tree| RetainedScope { tree, index: None })))
        .ok()
        .flatten();
    let _restore = Restore(saved);
    f()
}

/// The retained children of the container `widget`/`key` at `revision`, if they may be reused.
///
/// `None` means "describe it again": there is no scope, the key is absent or ambiguous, the
/// previous node is a different control, or its revision differs. Every one of those is a
/// safe fallback — the build simply runs as it did before this channel existed.
pub(crate) fn retained_children(widget: &str, key: &str, revision: u64) -> Option<Vec<Node>> {
    RETAINED
        .try_with(|cell| {
            let mut slot = cell.try_borrow_mut().ok()?;
            let scope = slot.as_mut()?;
            if scope.index.is_none() {
                let index = index_revisioned(&scope.tree);
                scope.index = Some(index);
            }
            // Copy the path out so the `scope.index` borrow ends before the tree is read.
            let path = match scope.index.as_ref()?.get(key)? {
                Some(path) => path.clone(),
                None => return None,
            };
            let node = node_at_path(&scope.tree, &path)?;
            if node.widget != widget || node.subtree_revision != Some(revision) {
                return None;
            }
            Some(node.children.clone())
        })
        .ok()
        .flatten()
}

/// Indexes every revisioned container by its key, recording duplicate keys as ambiguous.
fn index_revisioned(root: &Node) -> HashMap<crate::compat::String, Option<Vec<usize>>> {
    let mut out = HashMap::new();
    index_children(root, &mut Vec::new(), &mut out);
    out
}

fn index_children(
    node: &Node,
    path: &mut Vec<usize>,
    out: &mut HashMap<crate::compat::String, Option<Vec<usize>>>,
) {
    if let (Some(key), Some(_)) = (node.key.as_ref(), node.subtree_revision) {
        if out.contains_key(key) {
            // A second claimant makes the key unusable for reuse: the two containers are
            // different subtrees, and picking one would splice the wrong rows.
            out.insert(key.clone(), None);
        } else {
            out.insert(key.clone(), Some(path.clone()));
        }
    }
    for (index, child) in node.children.iter().enumerate() {
        path.push(index);
        index_children(child, path, out);
        path.pop();
    }
}

/// Reads the node at `path`, or `None` when the path runs past a leaf.
fn node_at_path<'a>(root: &'a Node, path: &[usize]) -> Option<&'a Node> {
    let mut node = root;
    for &index in path {
        node = node.children.get(index)?;
    }
    Some(node)
}
