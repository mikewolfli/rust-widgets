// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The read-only map from a mounted tree's **stable identity** to its live `ObjectId`
//! (BLUE26 B26.04).
//!
//! # What problem this solves
//!
//! A designer's runtime inspector draws the tree beside the model that produced it and must answer
//! "which live control is this model node?". [`ViewEngine::id_at`](crate::view::ViewEngine::id_at)
//! answers that for a **path**, but a path is not an identity: inserting one sibling moves every
//! later node to a new path, so a path captured before the update addresses a different node after
//! it. The engine already re-derives its map by identity internally (see
//! [`ViewEngine::reindex_paths`](crate::view::ViewEngine)); what was missing is a *public, read-only*
//! contract for a caller to query — with the rules stated, so a host does not have to reverse
//! engineer them.
//!
//! # The two identities, and their rules
//!
//! * **key** — the explicit identity a caller declared with [`Node::key`](crate::view::Node::key).
//!   It is **scoped to its siblings**: two lists may each hold a row keyed `"total"`. Within one
//!   parent it must be **unique**; a duplicate makes both claimants unmappable
//!   ([`TreeIndex::id_of_key`] answers `None`, and [`TreeIndex::duplicate_keys`] names the colliding
//!   key so a caller can say *why*; the diff reports the same condition as
//!   `duplicate_keys`). A keyed node is matched across a rebuild **only** by its key, never
//!   positionally.
//! * **path** — a sequence of child indices from the root. It is the addressing the diff uses, and
//!   it is **not stable across insertion**: a path is a snapshot, valid for the tree it was taken
//!   from.
//!
//! A caller resolving a model node should therefore hold the **key**, not the path, and ask for the
//! key. Ask for a path only when the tree has not changed since the path was taken.
//!
//! # Invalidation
//!
//! An index is a **snapshot** of the engine at the moment it was built. It holds for exactly as
//! long as the tree does not change. To re-query after an update, rebuild the index with
//! [`ViewEngine::tree_index`](crate::view::ViewEngine::tree_index). A stale index does not silently
//! answer wrongly: it answers about the tree it was built from, and
//! [`TreeIndex::generation`](TreeIndex::generation) lets a caller detect that the engine has moved
//! on.
//!
//! # `ObjectId` is not a `NodeId`
//!
//! An [`ObjectId`] is a **runtime handle**: it is valid only while the control is alive, and the
//! engine may reuse the value. It must never be persisted, written into a project file, or compared
//! across sessions. A designer that needs a durable identity uses the **key** (or the path in the
//! document), and resolves it to an `ObjectId` at run time through this index.

use crate::compat::{HashMap, String, Vec};
use crate::core::ObjectId;

/// A read-only snapshot of a mounted tree's identity map.
///
/// Built by [`ViewEngine::tree_index`](crate::view::ViewEngine::tree_index); see the module doc for
/// the identity rules and the invalidation contract.
#[derive(Debug, Clone, Default)]
pub struct TreeIndex {
    /// Every node's `ObjectId`, by its path from the root.
    path_to_id: HashMap<Vec<usize>, ObjectId>,
    /// Every **keyed** node's `ObjectId`, by `(parent id, key)`.
    ///
    /// Keyed by the parent's id rather than the parent's path because a key is scoped to its
    /// siblings, and the parent's id survives an insertion above it while its path does not.
    key_to_id: HashMap<(Option<ObjectId>, String), ObjectId>,
    /// The reverse of `path_to_id`, for the "what is this control in the model?" direction.
    id_to_path: HashMap<ObjectId, Vec<usize>>,
    /// Keys claimed more than once under one parent, sorted for a stable report.
    ///
    /// A duplicated key identifies **neither** claimant, so neither is entered in `key_to_id`: this
    /// list is what keeps the collision from being resolved arbitrarily, and what lets a caller
    /// explain the missing mapping instead of guessing.
    duplicated_keys: Vec<(Option<ObjectId>, String)>,
    /// The widget name of each node, so a query can report the node *type* without a second walk.
    widget_of: HashMap<ObjectId, String>,
    /// The parent of each node, or absent for the root.
    parent_of: HashMap<ObjectId, ObjectId>,
    /// The children of each node, in order, so a query can walk without the engine.
    children_of: HashMap<ObjectId, Vec<ObjectId>>,
    /// The engine instance generation this snapshot was taken at.
    generation: u64,
    /// The engine's root id, if a tree is mounted.
    root: Option<ObjectId>,
}

impl TreeIndex {
    /// The engine generation this snapshot describes.
    ///
    /// Compare it against [`ViewEngine::generation`](crate::view::ViewEngine::generation) to tell
    /// whether the engine has changed since the snapshot — a stale index answers about the tree it
    /// was built from, which is exactly what this lets a caller detect.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// The root control's id, or `None` when no tree is mounted.
    pub fn root(&self) -> Option<ObjectId> {
        self.root
    }

    /// How many nodes the snapshot holds.
    pub fn len(&self) -> usize {
        self.path_to_id.len()
    }

    /// Whether the snapshot holds no nodes.
    pub fn is_empty(&self) -> bool {
        self.path_to_id.is_empty()
    }

    /// The `ObjectId` at `path`, or `None` when the path addresses nothing in this snapshot.
    pub fn id_at(&self, path: &[usize]) -> Option<ObjectId> {
        self.path_to_id.get(path).copied()
    }

    /// The `ObjectId` of the keyed node `key` under `parent`, or `None`.
    ///
    /// `parent` is `None` for the root's own children. Because a key is scoped to its siblings, the
    /// caller must supply the parent (which it can get from [`Self::id_at`] or from a previous
    /// resolution).
    ///
    /// A key claimed by more than one sibling under `parent` answers `None`, because it identifies
    /// neither of them; [`Self::duplicate_keys`] names the collision. Answering with one of the two
    /// would send the caller to a control at random, which is worse than saying "not mappable".
    pub fn id_of_key(&self, parent: Option<ObjectId>, key: &str) -> Option<ObjectId> {
        self.key_to_id.get(&(parent, String::from(key))).copied()
    }

    /// Every key that more than one sibling claims, sorted, with its parent.
    ///
    /// A caller that asked for a key and got `None` from [`Self::id_of_key`] asks this to tell
    /// "there is no such node" from "the document claims that key twice" — two conditions with two
    /// different fixes, and the second is the one a designer must show.
    pub fn duplicate_keys(&self) -> &[(Option<ObjectId>, String)] {
        &self.duplicated_keys
    }

    /// The path of `id` in this snapshot, or `None` when the id is not part of it.
    pub fn path_of(&self, id: ObjectId) -> Option<&[usize]> {
        self.id_to_path.get(&id).map(Vec::as_slice)
    }

    /// The widget type name of `id`, or `None` when the id is not part of this snapshot.
    pub fn widget_of(&self, id: ObjectId) -> Option<&str> {
        self.widget_of.get(&id).map(String::as_str)
    }

    /// The key of `id`, if it declared one and is part of this snapshot.
    pub fn key_of(&self, id: ObjectId) -> Option<&str> {
        self.key_to_id.iter().find(|(_, v)| **v == id).map(|((_, k), _)| k.as_str())
    }

    /// The parent of `id`, or `None` for the root or an unknown id.
    pub fn parent_of(&self, id: ObjectId) -> Option<ObjectId> {
        self.parent_of.get(&id).copied()
    }

    /// The children of `id`, in order, or an empty slice for a leaf or unknown id.
    pub fn children_of(&self, id: ObjectId) -> &[ObjectId] {
        self.children_of.get(&id).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Every keyed node as `(parent, key, id)`, sorted by `(parent, key)` for a stable report.
    ///
    /// A duplicated key is absent: it maps to no single node. Ask [`Self::duplicate_keys`] for
    /// those.
    pub fn keys(&self) -> Vec<(Option<ObjectId>, &str, ObjectId)> {
        let mut out: Vec<(Option<ObjectId>, &str, ObjectId)> =
            self.key_to_id.iter().map(|((parent, key), id)| (*parent, key.as_str(), *id)).collect();
        out.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
        out
    }

    /// Build a snapshot from the parts the engine owns.
    ///
    /// Crate-internal: the engine is the only thing that knows all of these at once, and exposing a
    /// public constructor would let a caller build an index that disagrees with the tree it claims
    /// to describe.
    pub(crate) fn build(
        path_to_id: HashMap<Vec<usize>, ObjectId>,
        widget_of: HashMap<ObjectId, String>,
        key_of: HashMap<ObjectId, String>,
        parent_of: HashMap<ObjectId, ObjectId>,
        children_of: HashMap<ObjectId, Vec<ObjectId>>,
        root: Option<ObjectId>,
        generation: u64,
    ) -> Self {
        // The key map is derived here rather than maintained by the engine, so it cannot drift from
        // `path_to_id`: a node is in it exactly when it is in the snapshot and declared a key —
        // unless the key is claimed twice under one parent, in which case it identifies neither
        // claimant and belongs in `duplicated_keys` instead.
        //
        // The ambiguity is decided by *removing* the first claimant's entry when a second arrives
        // and then ignoring every later claimant, rather than by testing whether the map happens to
        // hold the key: `path_to_id` is a `HashMap`, so this walk's order is unspecified, and a
        // test on the map's contents would resolve a key claimed three times back to whichever
        // claimant the walk visited last.
        let mut key_to_id: HashMap<(Option<ObjectId>, String), ObjectId> = HashMap::new();
        let mut duplicated_keys: Vec<(Option<ObjectId>, String)> = Vec::new();
        for id in path_to_id.values() {
            if let Some(key) = key_of.get(id) {
                let claimed = (parent_of.get(id).copied(), key.clone());
                if duplicated_keys.contains(&claimed) {
                    // Already known to be ambiguous: a third claimant changes nothing.
                    continue;
                }
                if key_to_id.remove(&claimed).is_some() {
                    // A second claimant: keeping either id would answer with one of two
                    // equally-valid candidates, so the key becomes unmappable.
                    duplicated_keys.push(claimed);
                } else {
                    key_to_id.insert(claimed, *id);
                }
            }
        }
        duplicated_keys.sort();
        let mut id_to_path: HashMap<ObjectId, Vec<usize>> = HashMap::new();
        for (path, id) in &path_to_id {
            id_to_path.insert(*id, path.clone());
        }
        Self {
            path_to_id,
            key_to_id,
            duplicated_keys,
            id_to_path,
            widget_of,
            parent_of,
            children_of,
            generation,
            root,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_index() -> TreeIndex {
        // A tiny two-level tree: root(1) -> panel(2, key "sidebar"), panel(2) -> label(3, key "title").
        let mut path_to_id = HashMap::new();
        path_to_id.insert(Vec::new(), 1u64);
        path_to_id.insert(vec![0usize], 2u64);
        path_to_id.insert(vec![0usize, 0usize], 3u64);
        let mut widget_of = HashMap::new();
        widget_of.insert(1u64, String::from("window"));
        widget_of.insert(2u64, String::from("panel"));
        widget_of.insert(3u64, String::from("label"));
        let mut key_of = HashMap::new();
        key_of.insert(2u64, String::from("sidebar"));
        key_of.insert(3u64, String::from("title"));
        let mut parent_of = HashMap::new();
        parent_of.insert(2u64, 1u64);
        parent_of.insert(3u64, 2u64);
        let mut children_of: HashMap<ObjectId, Vec<ObjectId>> = HashMap::new();
        children_of.insert(1u64, vec![2u64]);
        children_of.insert(2u64, vec![3u64]);
        TreeIndex::build(path_to_id, widget_of, key_of, parent_of, children_of, Some(1), 7)
    }

    /// Both directions must resolve: path → id and id → path.
    #[test]
    fn path_and_id_resolve_both_ways() {
        let index = sample_index();
        assert_eq!(index.root(), Some(1));
        assert_eq!(index.id_at(&[0]), Some(2));
        assert_eq!(index.path_of(3), Some(&[0usize, 0usize][..]));
        assert_eq!(index.id_at(&[9]), None);
        assert_eq!(index.path_of(999), None);
    }

    /// A key must resolve within its own parent, and not leak across parents.
    #[test]
    fn a_key_resolves_within_its_parent_only() {
        let index = sample_index();
        assert_eq!(index.id_of_key(Some(1), "sidebar"), Some(2));
        assert_eq!(index.id_of_key(Some(2), "title"), Some(3));
        // The root's children never contain `title` — a key is scoped, not global.
        assert_eq!(index.id_of_key(Some(1), "title"), None);
        assert_eq!(index.id_of_key(Some(2), "sidebar"), None);
    }

    /// The node type and parent must be answerable from the snapshot alone.
    #[test]
    fn node_type_and_parent_are_queryable() {
        let index = sample_index();
        assert_eq!(index.widget_of(2), Some("panel"));
        assert_eq!(index.widget_of(3), Some("label"));
        assert_eq!(index.parent_of(3), Some(2));
        assert_eq!(index.parent_of(1), None);
        assert_eq!(index.children_of(2), &[3u64]);
        assert_eq!(index.children_of(3), &[] as &[ObjectId]);
    }

    /// `keys` must be sorted and complete, so a report is stable.
    #[test]
    fn keys_are_sorted_and_complete() {
        let index = sample_index();
        let keys = index.keys();
        assert_eq!(keys.len(), 2);
        assert_eq!(keys[0], (Some(1), "sidebar", 2));
        assert_eq!(keys[1], (Some(2), "title", 3));
    }

    /// The generation must be reported, so a stale snapshot is detectable.
    #[test]
    fn the_generation_is_reported() {
        let index = sample_index();
        assert_eq!(index.generation(), 7);
        assert_eq!(index.len(), 3);
        assert!(!index.is_empty());
    }

    /// A key claimed by two siblings must map to **neither** of them, and must be nameable.
    ///
    /// The earlier behaviour kept whichever id the map happened to write last, so a lookup for an
    /// ambiguous key silently sent the inspector to one of two equally-valid controls — and said
    /// nothing. `id_of_key` answering `None` plus `duplicate_keys` is the honest pair.
    #[test]
    fn a_duplicated_key_is_mappable_to_neither_claimant() {
        let mut path_to_id = HashMap::new();
        path_to_id.insert(Vec::new(), 1u64);
        path_to_id.insert(vec![0usize], 2u64);
        path_to_id.insert(vec![1usize], 3u64);
        let mut widget_of = HashMap::new();
        widget_of.insert(1u64, String::from("window"));
        widget_of.insert(2u64, String::from("label"));
        widget_of.insert(3u64, String::from("label"));
        let mut key_of = HashMap::new();
        key_of.insert(2u64, String::from("dup"));
        key_of.insert(3u64, String::from("dup"));
        let mut parent_of = HashMap::new();
        parent_of.insert(2u64, 1u64);
        parent_of.insert(3u64, 1u64);
        let mut children_of: HashMap<ObjectId, Vec<ObjectId>> = HashMap::new();
        children_of.insert(1u64, vec![2u64, 3u64]);
        let index =
            TreeIndex::build(path_to_id, widget_of, key_of, parent_of, children_of, Some(1), 1);

        assert_eq!(index.id_of_key(Some(1), "dup"), None, "the key identifies neither claimant");
        assert_eq!(index.duplicate_keys(), &[(Some(1), String::from("dup"))]);
        // Neither claimant is reachable *by that key*, so `keys()` must not list a winner either.
        assert!(index.keys().is_empty(), "{:?}", index.keys());
        assert_eq!(index.key_of(2), None);
        assert_eq!(index.key_of(3), None);
        // Both are still addressable by path — the collision is about the key, not the tree.
        assert_eq!(index.id_at(&[0]), Some(2));
        assert_eq!(index.id_at(&[1]), Some(3));
    }

    /// A key claimed by **three** siblings must stay unmappable.
    ///
    /// The snapshot is built by walking a `HashMap`, so a build that decided ambiguity by testing
    /// whether the map happened to hold the key would resolve a triple collision back to whichever
    /// claimant the walk reached last. This pins the order-independent answer.
    #[test]
    fn a_key_claimed_three_times_is_still_unmappable() {
        let mut path_to_id = HashMap::new();
        path_to_id.insert(Vec::new(), 1u64);
        path_to_id.insert(vec![0usize], 2u64);
        path_to_id.insert(vec![1usize], 3u64);
        path_to_id.insert(vec![2usize], 4u64);
        let mut widget_of = HashMap::new();
        for id in [1u64, 2, 3, 4] {
            widget_of.insert(id, String::from("label"));
        }
        let mut key_of = HashMap::new();
        for id in [2u64, 3, 4] {
            key_of.insert(id, String::from("dup"));
        }
        let mut parent_of = HashMap::new();
        for id in [2u64, 3, 4] {
            parent_of.insert(id, 1u64);
        }
        let mut children_of: HashMap<ObjectId, Vec<ObjectId>> = HashMap::new();
        children_of.insert(1u64, vec![2u64, 3u64, 4u64]);
        let index =
            TreeIndex::build(path_to_id, widget_of, key_of, parent_of, children_of, Some(1), 2);

        assert_eq!(index.id_of_key(Some(1), "dup"), None, "three claimants map to none of them");
        assert_eq!(index.duplicate_keys(), &[(Some(1), String::from("dup"))]);
        assert_eq!(index.keys().len(), 0, "{:?}", index.keys());
    }
}
