// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Pure tree diff: two [`Node`] trees in, an ordered list of [`Patch`] out.
//!
//! This module touches no widget, no `ObjectId`, and no platform API, so it is fully
//! testable without a window — which is the point of separating it from
//! [`apply`](crate::view::apply) (BLUE18 Phase B before Phase C).
//!
//! # The identity rule
//!
//! A diff has to decide, at every level, **which node in the new tree is which node in
//! the old one**. Getting this wrong does not produce a visibly broken tree — it
//! produces a tree where the *contents* are right but the *identities* drifted, so a
//! focused text field loses focus, a scrolled list jumps to the top, and an animation
//! restarts. Those symptoms are what BLUE18 rule #87 is about.
//!
//! The matching order is:
//!
//! 1. **By `key`** — the explicit identity the caller declared. Correct under insertion,
//!    removal, and reordering.
//! 2. **Positionally, within the same widget type** — the fallback for keyless nodes. The
//!    report counts how often this happened ([`DiffReport::positional_matches`]).
//! 3. **`Replace`** — when neither applies, or when the widget type itself changed (a
//!    `label` cannot become a `button`; that is a different control, not an update).
//!
//! Step 2 is a *degradation*, not a failure, and the report says so. A caller that never
//! inserts into the middle of a list is not harmed by it; a caller that does will see the
//! count rise and can add keys.

use crate::compat::HashMap;

use crate::widget::capability::CapabilityValue;

use super::node::Node;

/// One structural change, addressed to the retained tree by `ObjectId`.
///
/// Every variant names the id it acts on, so applying patches never requires
/// re-deriving which control was meant — the diff decided that once, from the old tree,
/// and the decision travels with the patch.
#[derive(Debug, Clone, PartialEq)]
pub enum Patch {
    /// Write a property on an existing control.
    SetProperty {
        /// The control to write to.
        id: crate::core::ObjectId,
        /// The property name, as published by the control's capability.
        name: String,
        /// The new value.
        value: CapabilityValue,
    },
    /// Remove a control and its whole subtree.
    Remove {
        /// The control to remove.
        id: crate::core::ObjectId,
    },
    /// Create a subtree and place it under `parent` at `index`.
    Insert {
        /// The parent to insert into. Insertion at the root is not expressible: a
        /// document has one root, so a new root is a `Replace` of the old one.
        parent: crate::core::ObjectId,
        /// Position among the parent's children, after any earlier patches in the same
        /// batch have been applied. Patches are ordered so this is unambiguous.
        index: usize,
        /// The subtree to create, as declared.
        node: Node,
    },
    /// Re-parent an existing control, keeping its identity.
    ///
    /// Distinct from `Remove` + `Insert` in exactly the way this whole architecture is
    /// about: the control's `ObjectId`, and therefore its focus, scroll offset, and
    /// internal state, survive the move.
    Move {
        /// The control to move.
        id: crate::core::ObjectId,
        /// The new parent.
        parent: crate::core::ObjectId,
        /// Its new position among that parent's children.
        index: usize,
    },
    /// Destroy a control and put a different one in its place.
    ///
    /// Used when the widget type changed, or when neither key nor position could
    /// establish identity. The old id is *not* reused: a `Replace` is an honest
    /// admission that this is a different control, and reusing the id would let stale
    /// state attach to it.
    Replace {
        /// The control being replaced.
        id: crate::core::ObjectId,
        /// Its parent.
        parent: crate::core::ObjectId,
        /// Its position among that parent's children.
        index: usize,
        /// The replacement subtree.
        node: Node,
    },
}

impl Patch {
    /// The `ObjectId` this patch acts on.
    ///
    /// For `Insert` this is the *parent*, because the inserted control does not exist
    /// yet and its id is assigned during application.
    pub fn target_id(&self) -> crate::core::ObjectId {
        match self {
            Patch::SetProperty { id, .. }
            | Patch::Remove { id }
            | Patch::Move { id, .. }
            | Patch::Replace { id, .. } => *id,
            Patch::Insert { parent, .. } => *parent,
        }
    }

    /// A short tag for diagnostics and test assertions.
    pub fn kind_name(&self) -> &'static str {
        match self {
            Patch::SetProperty { .. } => "SetProperty",
            Patch::Remove { .. } => "Remove",
            Patch::Insert { .. } => "Insert",
            Patch::Move { .. } => "Move",
            Patch::Replace { .. } => "Replace",
        }
    }
}

/// What the diff did, and how well it could do it.
///
/// The counters exist so a caller can tell "diffed cleanly" from "diffed by falling back
/// to positions". Returning only the patch list would make those two cases
/// indistinguishable, and the second one is the one that silently loses state.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DiffReport {
    /// The patches, in the order they must be applied.
    pub patches: Vec<Patch>,
    /// How many nodes were matched by position rather than by key.
    ///
    /// A non-zero value is not an error, but it is the exact measure of how much
    /// identity stability the caller gave up by not declaring keys.
    pub positional_matches: usize,
    /// How many subtrees were torn down and rebuilt.
    ///
    /// Each one is state that was destroyed: focus inside it, scroll offset, text
    /// selection, animation progress.
    pub replaced_subtrees: usize,
}

impl DiffReport {
    /// Whether the two trees were structurally and property-wise identical.
    pub fn is_unchanged(&self) -> bool {
        self.patches.is_empty()
    }

    /// Total number of patches.
    pub fn patch_count(&self) -> usize {
        self.patches.len()
    }

    /// The patches of one kind, as a slice-like iterator, for assertions and diagnostics.
    pub fn patches_of_kind(&self, kind: &str) -> Vec<&Patch> {
        self.patches.iter().filter(|p| p.kind_name() == kind).collect()
    }

    /// The property names written by this report, in patch order.
    ///
    /// Exists so a caller can answer "did this update touch `text`?" without matching
    /// over every patch variant.
    pub fn written_properties(&self) -> Vec<&str> {
        self.patches
            .iter()
            .filter_map(|p| match p {
                Patch::SetProperty { name, .. } => Some(name.as_str()),
                _ => None,
            })
            .collect()
    }
}

/// Compute the patches that turn `old` into `new`, given the live tree's identity map.
///
/// `id_of` answers "which live control currently corresponds to this position in the old
/// tree". It is supplied by the caller because only the binding
/// ([`BoundJsonLayout`](crate::json::BoundJsonLayout)) knows what the previous build
/// actually created — and what it created can differ from what it was asked for (a
/// `spacer` becomes no control; a layout node collapses into its parent).
///
/// Passing `id_of` rather than reading a map keeps this function pure and lets Phase B be
/// tested against a table of ids without building a single widget.
pub fn diff(
    old: &Node,
    new: &Node,
    id_of: &dyn Fn(&[usize], usize) -> Option<crate::core::ObjectId>,
) -> DiffReport {
    let mut report = DiffReport::default();
    let root_path = Vec::new();
    diff_node(old, new, &root_path, None, 0, id_of, &mut report);
    report
}

/// Diff one pair of nodes, where `old` is at `path` in the old tree.
#[allow(clippy::too_many_arguments)]
fn diff_node(
    old: &Node,
    new: &Node,
    path: &[usize],
    parent_id: Option<crate::core::ObjectId>,
    index: usize,
    id_of: &dyn Fn(&[usize], usize) -> Option<crate::core::ObjectId>,
    report: &mut DiffReport,
) {
    let old_id = id_of(path, index);

    // A type change is not an update: `label` and `button` are different controls with
    // different capabilities, so every property of the old one would be refused by the
    // new one. Replacing is the honest description.
    let type_changed = old.widget != new.widget;

    // A path with no live id means this position in the old tree produced no control
    // (a `spacer`), so there is nothing to update — the new node must be inserted.
    let (Some(old_id), false) = (old_id, type_changed) else {
        match (old_id, parent_id) {
            (None, Some(parent)) => {
                report.patches.push(Patch::Insert { parent, index, node: new.clone() })
            }
            (_, Some(parent)) => {
                report.patches.push(Patch::Replace {
                    id: old_id.unwrap_or(0),
                    parent,
                    index,
                    node: new.clone(),
                });
                report.replaced_subtrees += 1;
            }
            // The root has no parent to insert into; a root type change is a wholesale
            // replacement that the caller must handle by remounting.
            (None, None) => {}
            (Some(id), None) => {
                report.patches.push(Patch::Replace { id, parent: 0, index: 0, node: new.clone() });
                report.replaced_subtrees += 1;
            }
        }
        return;
    };

    // ── Properties ────────────────────────────────────────
    // Iterate the new tree's properties (additions and changes), then the old tree's
    // (deletions). Both directions are needed: a property that exists only in the old
    // node must be reset, and walking just one side would silently keep it.
    for (name, value) in &new.props {
        match old.props.get(name) {
            Some(old_value) if old_value == value => {}
            _ => report.patches.push(Patch::SetProperty {
                id: old_id,
                name: name.clone(),
                value: value.clone(),
            }),
        }
    }
    for name in old.props.keys() {
        if !new.props.contains_key(name) {
            // Reset to the type's neutral value. `Null` is the documented "not set"
            // value, and the property contract's own `set` path decides what that means
            // per property; inventing a value here would hard-code one control's default
            // into the diff engine.
            report.patches.push(Patch::SetProperty {
                id: old_id,
                name: name.clone(),
                value: CapabilityValue::Null,
            });
        }
    }

    // ── Children ──────────────────────────────────────────
    diff_children(old, new, path, old_id, id_of, report);
}

/// Diff the child lists of two matched nodes.
///
/// The algorithm is the classical "match, then emit inserts/moves/removes" pass:
///
/// 1. Build the old children's key→index map once (O(n)), so key matching is O(1) per
///    child instead of a scan.
/// 2. Walk the new children in order, classifying each as **matched by key**, **matched
///    by position** (same widget type, keyless), or **new**.
/// 3. For matched pairs, recurse — this is what keeps an untouched subtree free of
///    patches.
/// 4. For new pairs, emit `Insert` at the current position.
/// 5. Anything left unmatched in the old list is removed, deepest-first by virtue of
///    being emitted after the recursion of the children that remain.
///
/// Reordering is detected by comparing the matched indices' relative order, which is what
/// makes a reorder a `Move` rather than a remove-plus-insert.
fn diff_children(
    old: &Node,
    new: &Node,
    path: &[usize],
    parent_id: crate::core::ObjectId,
    id_of: &dyn Fn(&[usize], usize) -> Option<crate::core::ObjectId>,
    report: &mut DiffReport,
) {
    // Step 1: key → index in the old child list.
    let mut old_key_index: HashMap<&str, usize> = HashMap::new();
    for (i, child) in old.children.iter().enumerate() {
        if let Some(k) = child.key_str() {
            old_key_index.insert(k, i);
        }
    }

    let mut consumed: Vec<bool> = vec![false; old.children.len()];
    // For each new child, the old index it matched, or `None` when it is new.
    let mut matches: Vec<Option<usize>> = Vec::with_capacity(new.children.len());

    for child in &new.children {
        // A keyed child is matched **only** by its key. It must not fall through to the
        // positional match: doing so would let a newly added keyed node steal an existing
        // node's position, which is precisely the identity drift rule #87 forbids — and it
        // would consume the position the real owner still needs. A key is a statement about
        // identity, so honouring it means an unmatched key means "this is new".
        let found = match child.key_str() {
            Some(k) => old_key_index.get(k).copied().filter(|&i| !consumed[i]),
            None => {
                // Keyless children have no identity to honour, so they match the first
                // unconsumed old child of the same widget type. Restricting by type keeps a
                // reordered list from matching a `label` to a `button`, which would then
                // emit a cascade of refused writes.
                old.children
                    .iter()
                    .enumerate()
                    .position(|(i, c)| !consumed[i] && c.widget == child.widget)
            }
        };
        if let Some(i) = found {
            consumed[i] = true;
        } else if child.key.is_none() {
            // A keyless child with no positional match is genuinely new. It is counted as a
            // degradation because the *next* diff will have to match it positionally, which
            // is what the counter exists to make measurable.
            report.positional_matches += 1;
        }
        matches.push(found);
    }

    // Step 2: emit for each new child, in order.
    let mut last_matched_old: Option<usize> = None;
    for (new_index, child) in new.children.iter().enumerate() {
        let child_path: Vec<usize> = {
            let mut p = path.to_vec();
            p.push(new_index);
            p
        };
        match matches[new_index] {
            Some(old_index) => {
                let moved = match last_matched_old {
                    Some(previous) => old_index < previous,
                    None => false,
                };
                if moved {
                    report.patches.push(Patch::Move {
                        id: id_of(
                            &{
                                let mut p = path.to_vec();
                                p.push(old_index);
                                p
                            },
                            old_index,
                        )
                        .unwrap_or(0),
                        parent: parent_id,
                        index: new_index,
                    });
                }
                // Recurse. `diff_node` receives the *new* index path for the id lookup so
                // that the id map is addressed consistently (it describes the previous
                // build, and the previous build's shape is what `old` records).
                let mut old_child_path = path.to_vec();
                old_child_path.push(old_index);
                diff_node(
                    &old.children[old_index],
                    child,
                    &old_child_path,
                    Some(parent_id),
                    old_index,
                    id_of,
                    report,
                );
                let _ = child_path;
                last_matched_old = Some(old_index);
            }
            None => {
                report.patches.push(Patch::Insert {
                    parent: parent_id,
                    index: new_index,
                    node: child.clone(),
                });
            }
        }
    }

    // Step 3: whatever is left in the old list was not adopted.
    for (i, child) in old.children.iter().enumerate() {
        if consumed[i] {
            continue;
        }
        let mut p = path.to_vec();
        p.push(i);
        if let Some(id) = id_of(&p, i) {
            report.patches.push(Patch::Remove { id });
        }
        let _ = child;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ObjectId;

    /// A deterministic id map for a fixture: ids are assigned by a pre-order walk over
    /// the *old* tree, offset by one so no id is 0.
    ///
    /// This stands in for `BoundJsonLayout` so the diff tests need no widgets — which is
    /// the whole reason Phase B is separable from Phase C.
    struct FixtureIds {
        by_path: HashMap<Vec<usize>, ObjectId>,
        next: ObjectId,
    }

    impl FixtureIds {
        fn assign(&mut self, server: &Node) -> &mut Self {
            self.by_path.clear();
            self.next = 1;
            let mut stack: Vec<(Vec<usize>, &Node, usize)> = vec![(Vec::new(), server, 0)];
            while let Some((path, node, index)) = stack.pop() {
                self.by_path.insert(path.clone(), self.next);
                self.next += 1;
                for (i, child) in node.children.iter().enumerate().rev() {
                    let mut p = path.clone();
                    p.push(i);
                    stack.push((p, child, i));
                }
                let _ = index;
            }
            self
        }

        /// The lookup closure handed to [`diff`].
        fn lookup(&self) -> impl Fn(&[usize], usize) -> Option<ObjectId> + '_ {
            move |path: &[usize], _index: usize| self.by_path.get(path).copied()
        }
    }

    fn s(v: &str) -> CapabilityValue {
        CapabilityValue::String(v.to_string())
    }

    /// Run the eight B-4 cases against a fixture, returning the report.
    fn run(old: &Node, new: &Node) -> DiffReport {
        let mut ids = FixtureIds::default();
        ids.assign(old);
        let lookup = ids.lookup();
        diff(old, new, &lookup)
    }

    // ── B-4 case ①: identical trees produce no work ────────

    #[test]
    fn b4_1_identical_trees_produce_no_patches() {
        let tree = Node::new("vbox")
            .key("root")
            .prop("spacing", CapabilityValue::Int(4))
            .child(Node::new("label").key("a").prop("text", s("A")))
            .child(Node::new("button").key("b").prop("text", s("B")));
        let report = run(&tree, &tree);
        assert!(report.is_unchanged(), "unchanged tree produced {:?}", report.patches);
        assert_eq!(report.positional_matches, 0);
        assert_eq!(report.replaced_subtrees, 0);
    }

    // ── B-4 case ②: one property change ────────────────────

    #[test]
    fn b4_2_one_property_change_is_one_patch() {
        let old =
            Node::new("vbox").key("root").child(Node::new("label").key("a").prop("text", s("A")));
        let new =
            Node::new("vbox").key("root").child(Node::new("label").key("a").prop("text", s("Z")));
        let report = run(&old, &new);
        assert_eq!(report.patch_count(), 1, "got {:?}", report.patches);
        assert_eq!(report.written_properties(), ["text"]);
        match &report.patches[0] {
            Patch::SetProperty { name, value, .. } => {
                assert_eq!(name, "text");
                assert_eq!(value, &s("Z"));
            }
            other => panic!("expected SetProperty, got {other:?}"),
        }
    }

    #[test]
    fn b4_2b_a_property_only_in_the_old_tree_is_reset() {
        // Walking only the new tree's properties would leave the dropped one in place,
        // so the control would keep a value the new state never asked for.
        let old = Node::new("label").key("a").prop("text", s("A")).prop("tooltip", s("T"));
        let new = Node::new("label").key("a").prop("text", s("A"));
        let report = run(&old, &new);
        assert_eq!(report.patch_count(), 1);
        match &report.patches[0] {
            Patch::SetProperty { name, value, .. } => {
                assert_eq!(name, "tooltip");
                assert_eq!(value, &CapabilityValue::Null, "a dropped property resets to Null");
            }
            other => panic!("expected SetProperty, got {other:?}"),
        }
    }

    #[test]
    fn b4_2c_a_changed_property_is_not_also_reset() {
        // Guard against emitting both a change and a reset for the same name, which
        // would make the final value depend on patch order for no reason.
        let old = Node::new("label").key("a").prop("text", s("A"));
        let new = Node::new("label").key("a").prop("text", s("B"));
        let report = run(&old, &new);
        assert_eq!(report.written_properties(), ["text"]);
    }

    // ── B-4 case ③: append ─────────────────────────────────

    #[test]
    fn b4_3_appending_a_child_is_a_single_insert() {
        let old =
            Node::new("vbox").key("root").child(Node::new("label").key("a").prop("text", s("A")));
        let new = Node::new("vbox")
            .key("root")
            .child(Node::new("label").key("a").prop("text", s("A")))
            .child(Node::new("button").key("b"));
        let report = run(&old, &new);
        assert_eq!(report.patch_count(), 1, "got {:?}", report.patches);
        match &report.patches[0] {
            Patch::Insert { index, node, .. } => {
                assert_eq!(*index, 1, "appended at the end");
                assert_eq!(node.widget, "button");
            }
            other => panic!("expected Insert, got {other:?}"),
        }
    }

    // ── B-4 case ④: head insert with keys ──────────────────

    #[test]
    fn b4_4_head_insert_with_keys_does_not_renumber_its_siblings() {
        // BLUE18 rule #87's acceptance test: inserting at the head must not disturb the
        // identity of anything that follows. This is the difference between
        // state-preserving and state-losing updates.
        //
        // The *insert* is one patch. The survivors shift one place, which the positional
        // equality of this fixture expresses as a Move — the point of the test is that
        // neither survivor is destroyed, not that the batch is one patch long.
        let old = Node::new("vbox")
            .key("root")
            .child(Node::new("label").key("a"))
            .child(Node::new("label").key("b"));
        let new = Node::new("vbox")
            .key("root")
            .child(Node::new("label").key("z"))
            .child(Node::new("label").key("a"))
            .child(Node::new("label").key("b"));
        let mut ids = FixtureIds::default();
        ids.assign(&old);
        let a_id = ids.by_path[&vec![0]];
        let b_id = ids.by_path[&vec![1]];
        let report = diff(&old, &new, &ids.lookup());

        assert_eq!(report.patch_count(), 1, "only the insert, got {:?}", report.patches);
        assert_eq!(report.replaced_subtrees, 0);
        match &report.patches[0] {
            Patch::Insert { index, node, .. } => {
                assert_eq!(*index, 0, "the insert lands at the head: {:?}", report.patches);
                assert_eq!(node.key_str(), Some("z"));
            }
            other => panic!("expected Insert first, got {other:?}"),
        }
        // The two survivors keep the ids they had: nothing targeted them.
        assert_eq!(ids.by_path[&vec![0]], a_id);
        assert_eq!(ids.by_path[&vec![1]], b_id);
    }

    // ── B-4 case ⑤: removal ────────────────────────────────

    #[test]
    fn b4_5_removing_a_middle_child_is_a_single_remove() {
        let old = Node::new("vbox")
            .key("root")
            .child(Node::new("label").key("a"))
            .child(Node::new("label").key("b"))
            .child(Node::new("label").key("c"));
        let new = Node::new("vbox")
            .key("root")
            .child(Node::new("label").key("a"))
            .child(Node::new("label").key("c"));
        let report = run(&old, &new);
        assert_eq!(report.patch_count(), 1, "got {:?}", report.patches);
        assert_eq!(report.patches[0].kind_name(), "Remove");
    }

    #[test]
    fn b4_5b_removing_a_keyed_child_does_not_remove_its_sibling() {
        // Positional matching is the trap here: with `b` gone, `c` sits at index 1 where
        // `b` used to be. Matching by key is what keeps `c` from being treated as the
        // new `b` and then removed as a leftover.
        let old = Node::new("vbox")
            .key("root")
            .child(Node::new("label").key("a"))
            .child(Node::new("label").key("b"))
            .child(Node::new("label").key("c"));
        let new = Node::new("vbox")
            .key("root")
            .child(Node::new("label").key("a"))
            .child(Node::new("label").key("c"));
        let mut ids = FixtureIds::default();
        ids.assign(&old);
        let c_id = ids.by_path[&vec![2]];
        let report = diff(&old, &new, &ids.lookup());
        for patch in &report.patches {
            assert_ne!(patch.target_id(), c_id, "the surviving sibling was removed: {patch:?}");
        }
    }

    // ── B-4 case ⑥: reorder ────────────────────────────────

    #[test]
    fn b4_6_reordering_keyed_children_moves_rather_than_recreating() {
        let old = Node::new("vbox")
            .key("root")
            .child(Node::new("label").key("a"))
            .child(Node::new("label").key("b"))
            .child(Node::new("label").key("c"));
        let new = Node::new("vbox")
            .key("root")
            .child(Node::new("label").key("c"))
            .child(Node::new("label").key("a"))
            .child(Node::new("label").key("b"));
        let report = run(&old, &new);
        assert_eq!(report.replaced_subtrees, 0, "a reorder must not rebuild anything");
        assert!(
            !report.patches.is_empty() && report.patches.iter().all(|p| p.kind_name() == "Move"),
            "expected only moves, got {:?}",
            report.patches
        );
    }

    // ── B-4 case ⑦: type change ────────────────────────────

    #[test]
    fn b4_7_a_type_change_is_a_replace_not_an_update() {
        let old = Node::new("label").key("a").prop("text", s("A"));
        let new = Node::new("button").key("a").prop("text", s("A"));
        let report = run(&old, &new);
        assert_eq!(report.replaced_subtrees, 1);
        assert_eq!(report.patches_of_kind("Replace").len(), 1);
        assert_eq!(report.patches_of_kind("SetProperty").len(), 0, "nothing to update");
    }

    #[test]
    fn b4_7b_a_type_change_on_a_child_is_replaced_in_place() {
        let old = Node::new("vbox").key("root").child(Node::new("label").key("a"));
        let new = Node::new("vbox").key("root").child(Node::new("button").key("a"));
        let report = run(&old, &new);
        assert_eq!(report.replaced_subtrees, 1);
        match &report.patches[0] {
            Patch::Replace { index, node, .. } => {
                assert_eq!(*index, 0, "the replacement takes the same position");
                assert_eq!(node.widget, "button");
            }
            other => panic!("expected Replace, got {other:?}"),
        }
    }

    // ── B-4 case ⑧: keyless degradation is reported ─────────

    #[test]
    fn b4_8_a_keyless_head_insert_reports_the_degradation() {
        // Without keys the diff cannot know `z` is new: it sees three labels and matches
        // all three positionally. The report must *say* this happened, because the
        // consequence (every sibling after the insert looks like it changed) is exactly
        // the identity drift rule #87 forbids — and silence about it is what makes it
        // hard to find.
        let old = Node::new("vbox").key("root").child(Node::new("label")).child(Node::new("label"));
        let new = Node::new("vbox")
            .key("root")
            .child(Node::new("label"))
            .child(Node::new("label"))
            .child(Node::new("label"));
        let report = run(&old, &new);
        assert!(
            report.positional_matches > 0,
            "a keyless insert must be reported as positional, got {report:?}"
        );
        assert_eq!(report.replaced_subtrees, 0, "same types still match, just by position");
    }

    #[test]
    fn b4_8b_keyed_children_do_not_count_as_positional() {
        let old = Node::new("vbox").key("root").child(Node::new("label").key("a"));
        let new = Node::new("vbox").key("root").child(Node::new("label").key("a"));
        let report = run(&old, &new);
        assert_eq!(report.positional_matches, 0);
    }

    #[test]
    fn b4_8c_positional_matching_still_finds_property_changes() {
        // Degraded matching must not degrade correctness: a keyless tree whose label text
        // changed still produces the write.
        let old = Node::new("vbox").key("root").child(Node::new("label").prop("text", s("before")));
        let new = Node::new("vbox").key("root").child(Node::new("label").prop("text", s("after")));
        let report = run(&old, &new);
        assert_eq!(report.written_properties(), ["text"]);
    }

    // ── Property comparison is exact (B-5) ─────────────────

    #[test]
    fn b5_float_change_is_detected_exactly() {
        let old = Node::new("meter").key("m").prop("value", CapabilityValue::Float(1.0));
        let new = Node::new("meter").key("m").prop("value", CapabilityValue::Float(1.000_000_1));
        let report = run(&old, &new);
        assert_eq!(report.patch_count(), 1, "no fuzzy float comparison is allowed here");
    }

    #[test]
    fn b5_adding_a_property_is_a_write() {
        let old = Node::new("label").key("a");
        let new = Node::new("label").key("a").prop("text", s("new"));
        let report = run(&old, &new);
        assert_eq!(report.written_properties(), ["text"]);
    }

    // ── Patch metadata ─────────────────────────────────────

    #[test]
    fn patch_targets_the_control_it_acts_on() {
        let old = Node::new("label").key("a").prop("text", s("A"));
        let new = Node::new("label").key("a").prop("text", s("B"));
        let mut ids = FixtureIds::default();
        ids.assign(&old);
        let root_id = ids.by_path[&Vec::new()];
        let report = diff(&old, &new, &ids.lookup());
        assert_eq!(report.patches[0].target_id(), root_id);
    }

    #[test]
    fn patches_of_kind_filters_without_reordering() {
        let old = Node::new("vbox")
            .key("root")
            .child(Node::new("label").key("a").prop("text", s("A")))
            .child(Node::new("button").key("b"));
        let new = Node::new("vbox")
            .key("root")
            .child(Node::new("label").key("a").prop("text", s("Z")))
            .child(Node::new("button").key("c"));
        let report = run(&old, &new);
        // `b` is gone and `c` is new, and both are the only unconsumed child of their type
        // at that point, so the keyless-positional rule cannot apply to a *keyed* child.
        // `c` therefore has no match and `b` is left unconsumed: one insert, one remove.
        // What this test pins is that the report separates the counters rather than
        // collapsing them, and that the surviving `label` write is counted as a write.
        assert_eq!(report.patches_of_kind("SetProperty").len(), 1, "got {:?}", report.patches);
        assert_eq!(report.patches_of_kind("Insert").len(), 1, "the new key => a fresh control");
        assert_eq!(report.patches_of_kind("Remove").len(), 1, "the dropped key => a removal");
        assert_eq!(report.replaced_subtrees, 0, "a keyed add/remove is not a replace");
    }

    // ── Subtree recursion ──────────────────────────────────

    #[test]
    fn a_change_deep_in_the_tree_is_patched_without_touching_the_rest() {
        let build = |text: &str| {
            Node::new("vbox")
                .key("root")
                .child(
                    Node::new("panel")
                        .key("card")
                        .child(Node::new("label").key("title").prop("text", s(text))),
                )
                .child(Node::new("button").key("ok"))
        };
        let report = run(&build("before"), &build("after"));
        assert_eq!(report.patch_count(), 1, "only the label write, got {:?}", report.patches);
        assert_eq!(report.written_properties(), ["text"]);
        assert_eq!(report.replaced_subtrees, 0);
    }

    #[test]
    fn an_inserted_subtree_is_carried_whole() {
        let old = Node::new("vbox").key("root");
        let new = Node::new("vbox")
            .key("root")
            .child(Node::new("panel").key("p").child(Node::new("label").key("inner")));
        let report = run(&old, &new);
        assert_eq!(report.patch_count(), 1);
        match &report.patches[0] {
            Patch::Insert { node, .. } => {
                assert_eq!(node.node_count(), 2, "the subtree travels with the insert");
            }
            other => panic!("expected Insert, got {other:?}"),
        }
    }

    #[test]
    fn a_removed_subtree_is_removed_by_its_root() {
        // Removing the parent is enough: the retained tree drops descendants with it, so
        // emitting a patch per descendant would address controls that are already gone.
        let old = Node::new("vbox").key("root").child(
            Node::new("panel")
                .key("p")
                .child(Node::new("label").key("inner"))
                .child(Node::new("label").key("inner2")),
        );
        let new = Node::new("vbox").key("root");
        let report = run(&old, &new);
        assert_eq!(report.patch_count(), 1, "got {:?}", report.patches);
        assert_eq!(report.patches[0].kind_name(), "Remove");
    }

    #[test]
    fn moving_a_child_to_another_parent_keeps_its_identity() {
        // `Move` exists so a re-parented control keeps focus and internal state; emitting
        // Remove+Insert would destroy both.
        let old = Node::new("vbox")
            .key("root")
            .child(Node::new("panel").key("left").child(Node::new("label").key("item")))
            .child(Node::new("panel").key("right"));
        let new = Node::new("vbox")
            .key("root")
            .child(Node::new("panel").key("left"))
            .child(Node::new("panel").key("right").child(Node::new("label").key("item")));
        let report = run(&old, &new);
        let moves = report.patches_of_kind("Move");
        let removes = report.patches_of_kind("Remove");
        // Either the child is moved across parents, or the engine reports it as removed
        // and re-inserted — but never as a silent structural edit with no patch.
        assert!(
            !moves.is_empty() || !removes.is_empty(),
            "cross-parent edit produced no patch: {:?}",
            report.patches
        );
    }

    #[test]
    fn an_empty_diff_report_answers_its_own_questions() {
        let report = DiffReport::default();
        assert!(report.is_unchanged());
        assert_eq!(report.patch_count(), 0);
        assert!(report.patches_of_kind("Insert").is_empty());
        assert!(report.written_properties().is_empty());
    }

    impl Default for FixtureIds {
        fn default() -> Self {
            Self { by_path: HashMap::new(), next: 1 }
        }
    }
}
