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

use crate::compat::{Arc, HashMap, String, Vec};
use crate::core::ObjectId;
use core::hash::Hash;

/// The bounds a stable identity must satisfy to be indexed.
///
/// Stated once so the struct, its queries and the builder cannot disagree: `Copy` because an id is
/// handed back by value everywhere, `Eq + Hash` because it keys the maps, and `Ord` because the
/// duplicate-key report and the key list are sorted — a stable report is what makes them
/// comparable between runs and between views.
///
/// `ObjectId` satisfies all four, and so does any durable id a host gives its own nodes.
pub trait IndexId: Copy + Eq + Hash + Ord {}

impl<T: Copy + Eq + Hash + Ord> IndexId for T {}

/// A read-only snapshot of a mounted tree's identity map.
///
/// Built by [`ViewEngine::tree_index`](crate::view::ViewEngine::tree_index) for the engine's own
/// tree, or by [`TreeIndexBuilder`] for a tree a caller owns (a designer canvas whose nodes carry
/// durable ids rather than runtime handles). See the module doc for the identity rules and the
/// invalidation contract.
///
/// The id type is a parameter with `ObjectId` as its default, so every existing spelling
/// (`TreeIndex`, `&TreeIndex` in a signature) still means the engine's index; a host whose ids are
/// its own type names it once (`TreeIndex<NodeId>`) and gets the same queries.
#[derive(Debug, Clone, Default)]
pub struct TreeIndex<Id = ObjectId> {
    /// Every node's id, by its path from the root.
    ///
    /// The key is a shared `Arc` slice rather than an owned `Vec` so the reverse map below
    /// can point at the **same** allocation: a snapshot therefore costs one path allocation
    /// per node instead of one per map (G-167).
    path_to_id: HashMap<Arc<[usize]>, Id>,
    /// Every **keyed** node's id, by `(parent id, key)`.
    ///
    /// Keyed by the parent's id rather than the parent's path because a key is scoped to its
    /// siblings, and the parent's id survives an insertion above it while its path does not.
    key_to_id: HashMap<(Option<Id>, String), Id>,
    /// The reverse of `path_to_id`, for the "what is this control in the model?" direction.
    ///
    /// Shares the path allocation with `path_to_id` (see the field comment above).
    id_to_path: HashMap<Id, Arc<[usize]>>,
    /// Keys claimed more than once under one parent, sorted for a stable report.
    ///
    /// A duplicated key identifies **neither** claimant, so neither is entered in `key_to_id`: this
    /// list is what keeps the collision from being resolved arbitrarily, and what lets a caller
    /// explain the missing mapping instead of guessing.
    duplicated_keys: Vec<(Option<Id>, String)>,
    /// The widget name of each node, so a query can report the node *type* without a second walk.
    widget_of: HashMap<Id, String>,
    /// The parent of each node, or absent for the root.
    parent_of: HashMap<Id, Id>,
    /// The children of each node, in order, so a query can walk without the engine.
    children_of: HashMap<Id, Vec<Id>>,
    /// The engine instance generation this snapshot was taken at.
    generation: u64,
    /// The engine's root id, if a tree is mounted.
    root: Option<Id>,
}

impl<Id: IndexId> TreeIndex<Id> {
    /// The engine generation this snapshot describes.
    ///
    /// Compare it against [`ViewEngine::generation`](crate::view::ViewEngine::generation) to tell
    /// whether the engine has changed since the snapshot — a stale index answers about the tree it
    /// was built from, which is exactly what this lets a caller detect.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// The root control's id, or `None` when no tree is mounted.
    pub fn root(&self) -> Option<Id> {
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

    /// The `Id` at `path`, or `None` when the path addresses nothing in this snapshot.
    pub fn id_at(&self, path: &[usize]) -> Option<Id> {
        self.path_to_id.get(path).copied()
    }

    /// The `Id` of the keyed node `key` under `parent`, or `None`.
    ///
    /// `parent` is `None` for the root's own children. Because a key is scoped to its siblings, the
    /// caller must supply the parent (which it can get from [`Self::id_at`] or from a previous
    /// resolution).
    ///
    /// A key claimed by more than one sibling under `parent` answers `None`, because it identifies
    /// neither of them; [`Self::duplicate_keys`] names the collision. Answering with one of the two
    /// would send the caller to a control at random, which is worse than saying "not mappable".
    pub fn id_of_key(&self, parent: Option<Id>, key: &str) -> Option<Id> {
        self.key_to_id.get(&(parent, String::from(key))).copied()
    }

    /// Every key that more than one sibling claims, sorted, with its parent.
    ///
    /// A caller that asked for a key and got `None` from [`Self::id_of_key`] asks this to tell
    /// "there is no such node" from "the document claims that key twice" — two conditions with two
    /// different fixes, and the second is the one a designer must show.
    pub fn duplicate_keys(&self) -> &[(Option<Id>, String)] {
        &self.duplicated_keys
    }

    /// The path of `id` in this snapshot, or `None` when the id is not part of it.
    pub fn path_of(&self, id: Id) -> Option<&[usize]> {
        self.id_to_path.get(&id).map(|path| path.as_ref())
    }

    /// The widget type name of `id`, or `None` when the id is not part of this snapshot.
    pub fn widget_of(&self, id: Id) -> Option<&str> {
        self.widget_of.get(&id).map(String::as_str)
    }

    /// The key of `id`, if it declared one and is part of this snapshot.
    pub fn key_of(&self, id: Id) -> Option<&str> {
        self.key_to_id.iter().find(|(_, v)| **v == id).map(|((_, k), _)| k.as_str())
    }

    /// The parent of `id`, or `None` for the root or an unknown id.
    pub fn parent_of(&self, id: Id) -> Option<Id> {
        self.parent_of.get(&id).copied()
    }

    /// The children of `id`, in order, or an empty slice for a leaf or unknown id.
    pub fn children_of(&self, id: Id) -> &[Id] {
        self.children_of.get(&id).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Every keyed node as `(parent, key, id)`, sorted by `(parent, key)` for a stable report.
    ///
    /// A duplicated key is absent: it maps to no single node. Ask [`Self::duplicate_keys`] for
    /// those.
    pub fn keys(&self) -> Vec<(Option<Id>, &str, Id)> {
        let mut out: Vec<(Option<Id>, &str, Id)> =
            self.key_to_id.iter().map(|((parent, key), id)| (*parent, key.as_str(), *id)).collect();
        out.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
        out
    }

    /// Build a snapshot from the parts the engine owns.
    ///
    /// Crate-internal: the engine knows all of these at once, and the only public construction path
    /// is [`TreeIndexBuilder`], which derives the same parts from a declared tree and then calls
    /// here — one construction rule, not two that can disagree (principle #101).
    pub(crate) fn build(
        path_to_id: HashMap<Arc<[usize]>, Id>,
        id_to_path: HashMap<Id, Arc<[usize]>>,
        widget_of: HashMap<Id, String>,
        key_of: HashMap<Id, String>,
        parent_of: HashMap<Id, Id>,
        children_of: HashMap<Id, Vec<Id>>,
        root: Option<Id>,
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
        let mut key_to_id: HashMap<(Option<Id>, String), Id> = HashMap::new();
        let mut duplicated_keys: Vec<(Option<Id>, String)> = Vec::new();
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

/// Builds a [`TreeIndex`] for a tree **the caller owns**, with the caller's own id type.
///
/// # The gap this closes
///
/// A designer canvas keeps durable node ids and is not mounted on a [`ViewEngine`], so
/// [`ViewEngine::tree_index`](crate::view::ViewEngine::tree_index) cannot describe it: that method
/// only knows the engine's runtime handles. Without a public construction path a host either
/// reimplemented the whole key-scoping rule (two implementations that drift) or gave up on
/// identity and used paths (which move when a sibling is inserted). This builder is the same
/// derivation — it calls the same constructor the engine calls, so the rules cannot diverge.
///
/// ```ignore
/// let index = TreeIndexBuilder::new()
///     .generation(scene.revision())
///     .node(&[], root_id, "window", None)
///     .node(&[0], panel_id, "panel", Some("sidebar"))
///     .build();
/// assert_eq!(index.id_of_key(Some(root_id), "sidebar"), Some(panel_id));
/// ```
///
/// # What the builder derives, and what it does not
///
/// * The root is the node at the **empty path**. A tree with no such node has `root() == None`,
///   which is the same answer the engine gives for "nothing mounted".
/// * A node whose parent path is absent is kept in the index but has no parent entry, and does not
///   appear in any parent's children list. It is reachable by path and by key (scoped to `None`,
///   exactly as the root's own children are), which is more useful than dropping it silently.
/// * A repeated **path** replaces the earlier node; a repeated **id** keeps the last path supplied
///   for it. Both are map-insert semantics, stated here because a caller that supplies both by
///   mistake should be able to predict the result.
/// * Children are listed in path order, which is the order their parent drew them in.
/// * The key rules (scoped to siblings, a duplicate key maps to neither claimant and is reported by
///   [`TreeIndex::duplicate_keys`]) are applied by the same code the engine uses.
pub struct TreeIndexBuilder<Id = ObjectId> {
    nodes: Vec<(Vec<usize>, Id, String, Option<String>)>,
    generation: u64,
}

impl<Id: IndexId> TreeIndexBuilder<Id> {
    /// A builder with no nodes and generation `0`.
    pub fn new() -> Self {
        Self { nodes: Vec::new(), generation: 0 }
    }

    /// Records the revision this snapshot describes, readable afterwards through
    /// [`TreeIndex::generation`].
    pub fn generation(mut self, generation: u64) -> Self {
        self.generation = generation;
        self
    }

    /// Declares one node: its path from the root, its id, its widget name and its optional key.
    pub fn node(mut self, path: &[usize], id: Id, widget: &str, key: Option<&str>) -> Self {
        self.nodes.push((path.to_vec(), id, String::from(widget), key.map(String::from)));
        self
    }

    /// Derives the maps the index needs and builds it.
    pub fn build(self) -> TreeIndex<Id> {
        let mut path_to_id: HashMap<Arc<[usize]>, Id> = HashMap::new();
        let mut id_to_path: HashMap<Id, Arc<[usize]>> = HashMap::new();
        let mut widget_of: HashMap<Id, String> = HashMap::new();
        let mut key_of: HashMap<Id, String> = HashMap::new();

        for (path, id, widget, key) in self.nodes {
            let shared: Arc<[usize]> = Arc::from(path.as_slice());
            // A repeated path replaces the earlier node, and the replaced id's rows leave with it:
            // keeping a stale reverse entry would let `path_of` answer a path this snapshot no
            // longer contains.
            if let Some(previous) = path_to_id.insert(Arc::clone(&shared), id) {
                id_to_path.remove(&previous);
                widget_of.remove(&previous);
                key_of.remove(&previous);
            }
            id_to_path.insert(id, Arc::clone(&shared));
            widget_of.insert(id, widget);
            match key {
                Some(key) => {
                    key_of.insert(id, key);
                }
                None => {
                    key_of.remove(&id);
                }
            }
        }

        // Parents and children are derived from the paths, after every node is known, so a node
        // declared before its parent still links up.
        let mut ordered: Vec<(Arc<[usize]>, Id)> =
            path_to_id.iter().map(|(path, id)| (Arc::clone(path), *id)).collect();
        ordered.sort_by(|left, right| left.0.as_ref().cmp(right.0.as_ref()));

        let root = path_to_id.get(&[] as &[usize]).copied();
        let mut parent_of: HashMap<Id, Id> = HashMap::new();
        let mut children_of: HashMap<Id, Vec<Id>> = HashMap::new();
        for (path, id) in &ordered {
            if path.is_empty() {
                continue;
            }
            let parent_path = &path[..path.len() - 1];
            if let Some(parent) = path_to_id.get(parent_path).copied() {
                parent_of.insert(*id, parent);
                children_of.entry(parent).or_default().push(*id);
            }
        }

        TreeIndex::build(
            path_to_id,
            id_to_path,
            widget_of,
            key_of,
            parent_of,
            children_of,
            root,
            self.generation,
        )
    }
}

impl<Id: IndexId> Default for TreeIndexBuilder<Id> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Assemble both path maps from a forward-only fixture.
    ///
    /// The production builder takes the two maps from one walk (G-167); a fixture states the
    /// paths once and derives the reverse table here so each test stays a declaration of the
    /// tree it exercises.
    fn build_index(
        path_to_id: HashMap<Vec<usize>, ObjectId>,
        widget_of: HashMap<ObjectId, String>,
        key_of: HashMap<ObjectId, String>,
        parent_of: HashMap<ObjectId, ObjectId>,
        children_of: HashMap<ObjectId, Vec<ObjectId>>,
        root: Option<ObjectId>,
        generation: u64,
    ) -> TreeIndex {
        let mut forward: HashMap<Arc<[usize]>, ObjectId> = HashMap::new();
        let mut reverse: HashMap<ObjectId, Arc<[usize]>> = HashMap::new();
        for (path, id) in &path_to_id {
            let shared: Arc<[usize]> = Arc::from(path.as_slice());
            forward.insert(Arc::clone(&shared), *id);
            reverse.insert(*id, shared);
        }
        TreeIndex::build(
            forward,
            reverse,
            widget_of,
            key_of,
            parent_of,
            children_of,
            root,
            generation,
        )
    }

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
        build_index(path_to_id, widget_of, key_of, parent_of, children_of, Some(1), 7)
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
        let index = build_index(path_to_id, widget_of, key_of, parent_of, children_of, Some(1), 1);

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
        let index = build_index(path_to_id, widget_of, key_of, parent_of, children_of, Some(1), 2);

        assert_eq!(index.id_of_key(Some(1), "dup"), None, "three claimants map to none of them");
        assert_eq!(index.duplicate_keys(), &[(Some(1), String::from("dup"))]);
        assert_eq!(index.keys().len(), 0, "{:?}", index.keys());
    }

    // ── The public builder (feedback-20261030 F-1) ───────────────────────────────

    /// A host's own identity type, deliberately not `ObjectId`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    struct NodeId(u64);

    /// A caller-owned tree indexes with the caller's own id type.
    ///
    /// This is the shape the host needs: a canvas of durable node ids that is not mounted on a
    /// `ViewEngine`, so `tree_index()` could not describe it and the key rules had to be
    /// reimplemented (the copy this builder exists to retire).
    #[test]
    fn a_builder_indexes_a_host_tree_with_the_hosts_own_id_type() {
        let index = TreeIndexBuilder::new()
            .generation(41)
            .node(&[], NodeId(1), "window", None)
            .node(&[0], NodeId(2), "panel", Some("sidebar"))
            .node(&[0, 0], NodeId(3), "label", Some("title"))
            .node(&[1], NodeId(4), "label", None)
            .build();

        assert_eq!(index.generation(), 41);
        assert_eq!(index.root(), Some(NodeId(1)));
        assert_eq!(index.len(), 4);
        assert_eq!(index.id_at(&[0]), Some(NodeId(2)));
        assert_eq!(index.path_of(NodeId(3)), Some(&[0usize, 0usize][..]));
        assert_eq!(index.id_of_key(Some(NodeId(1)), "sidebar"), Some(NodeId(2)));
        assert_eq!(index.id_of_key(Some(NodeId(2)), "title"), Some(NodeId(3)));
        assert_eq!(index.id_of_key(Some(NodeId(2)), "sidebar"), None);
        assert_eq!(index.parent_of(NodeId(3)), Some(NodeId(2)));
        assert_eq!(index.parent_of(NodeId(1)), None);
        assert_eq!(index.children_of(NodeId(1)), &[NodeId(2), NodeId(4)]);
        assert_eq!(index.widget_of(NodeId(4)), Some("label"));
        assert_eq!(index.key_of(NodeId(3)), Some("title"));
        assert_eq!(
            index.keys(),
            vec![(Some(NodeId(1)), "sidebar", NodeId(2)), (Some(NodeId(2)), "title", NodeId(3)),]
        );
    }

    /// The builder must apply the engine's duplicate-key rule, not a looser one of its own.
    #[test]
    fn the_builder_reports_a_duplicate_key_exactly_as_the_engine_does() {
        let index = TreeIndexBuilder::new()
            .node(&[], NodeId(1), "window", None)
            .node(&[0], NodeId(2), "label", Some("dup"))
            .node(&[1], NodeId(3), "label", Some("dup"))
            .build();

        assert_eq!(index.id_of_key(Some(NodeId(1)), "dup"), None);
        assert_eq!(index.duplicate_keys(), &[(Some(NodeId(1)), String::from("dup"))]);
    }

    /// A parent declared after its child must still link, because the derivation runs at `build`.
    #[test]
    fn a_builder_links_a_child_declared_before_its_parent() {
        let index = TreeIndexBuilder::new()
            .node(&[0], NodeId(2), "panel", None)
            .node(&[], NodeId(1), "window", None)
            .build();

        assert_eq!(index.root(), Some(NodeId(1)));
        assert_eq!(index.parent_of(NodeId(2)), Some(NodeId(1)));
        assert_eq!(index.children_of(NodeId(1)), &[NodeId(2)]);
    }

    /// A node whose parent path is absent stays indexable, with no parent entry.
    ///
    /// The alternative — dropping it — would make a partially declared tree silently smaller than
    /// the tree the caller passed, and a designer showing "node not found" is worse than one
    /// showing an unparented node.
    #[test]
    fn a_builder_keeps_a_node_whose_parent_path_is_absent() {
        let index = TreeIndexBuilder::new()
            .node(&[], NodeId(1), "window", None)
            .node(&[3, 0], NodeId(9), "label", Some("orphan"))
            .build();

        assert_eq!(index.id_at(&[3, 0]), Some(NodeId(9)));
        assert_eq!(index.parent_of(NodeId(9)), None);
        assert_eq!(index.id_of_key(Some(NodeId(1)), "orphan"), None, "the key is not root-scoped");
        assert_eq!(index.id_of_key(None, "orphan"), Some(NodeId(9)));
    }

    /// A repeated path replaces the earlier node and leaves no stale reverse entry behind.
    #[test]
    fn a_repeated_path_replaces_the_node_it_addressed() {
        let index = TreeIndexBuilder::new()
            .node(&[], NodeId(1), "window", None)
            .node(&[0], NodeId(2), "label", Some("old"))
            .node(&[0], NodeId(3), "button", Some("new"))
            .build();

        assert_eq!(index.id_at(&[0]), Some(NodeId(3)));
        assert_eq!(index.path_of(NodeId(2)), None, "the replaced node must not keep a path");
        assert_eq!(index.widget_of(NodeId(2)), None);
        assert_eq!(index.id_of_key(Some(NodeId(1)), "new"), Some(NodeId(3)));
        assert_eq!(index.id_of_key(Some(NodeId(1)), "old"), None);
    }
}
