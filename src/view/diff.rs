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
    /// How many nodes were matched by **position** rather than by key.
    ///
    /// A non-zero value is not an error, but it is the exact measure of how much
    /// identity stability the caller gave up by not declaring keys: a keyless child
    /// whose position happened to line up keeps its control (and its focus, scroll
    /// offset and selection), but that is luck rather than identity — insert one row at
    /// the head and every subsequent child shifts.
    pub positional_matches: usize,
    /// How many keyless children were **new**: they had no key and no positional match,
    /// so a control is created for them where the old tree had none.
    ///
    /// Split out from [`Self::positional_matches`] because the two answer different
    /// questions and the combined counter answered neither: "how much identity did I give
    /// up" (every keyless match, including a perfectly stable list) versus "how much is
    /// being built from nothing" (a genuine insertion). Reporting one number for both made
    /// a stable keyless list and a list that appends twice per frame look identical.
    pub new_keyless_children: usize,
    /// How many subtrees were torn down and rebuilt.
    ///
    /// Each one is state that was destroyed: focus inside it, scroll offset, text
    /// selection, animation progress.
    pub replaced_subtrees: usize,
    /// How many sibling keys were claimed by more than one child of the same parent.
    ///
    /// A duplicate key identifies none of its claimants, so the diff cannot match,
    /// move or remove either one unambiguously; those children take the "new" path and
    /// their old counterparts are removed. That is a real state loss, and it used to
    /// happen with every counter reading clean — `positional_matches` and
    /// `replaced_subtrees` both stayed at zero, so the report actively implied the
    /// highest-quality match had been found. This field is what makes the condition
    /// visible to a caller.
    ///
    /// [`crate::view::Node::duplicate_sibling_keys`] names the offending keys.
    pub duplicate_keys: usize,
    /// The root's widget type changed, which a patch batch cannot express.
    ///
    /// A document has exactly one root and it has no parent, so there is no
    /// `Replace { parent }` to emit: the batch would have to name a parent that does not
    /// exist. This used to be papered over with `parent: 0`, a sentinel that
    /// `apply_one` rejects (`is_mounted` is false for id `0`), so the resulting
    /// `ViewError::UnknownParent` was the *only* sign anything had happened — and
    /// `ViewEngine::update` discards the `ApplyReport`. The diff reported one patch, the
    /// apply did nothing, and the engine kept serving the pre-change root.
    ///
    /// The condition is therefore surfaced here instead, and **no patch is emitted** for
    /// it: a caller that sees this flag must remount the tree (`ViewEngine::mount`) rather
    /// than expect the root to have changed.
    pub root_replaced: bool,
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

/// Whether a schema-declared property can be **written** at all.
///
/// Used to suppress a patch that is guaranteed to be refused. The diff's deletion pass emits
/// `SetProperty { value: Null }` for every name the old node had and the new one does not,
/// on the reasoning that `Null` is the documented "not set" value and the contract decides
/// what it means. That is true for a *writable* property — but `geometry` is declared
/// `false, false` in every schema, and `properties_trait.rs` answers `ReadOnlyProperty` for it
/// unconditionally, so the patch could **never** succeed. It was logged as a warning and
/// dropped, which made the report describe a write the engine silently discarded.
///
/// `true` is the answer for a name the factory cannot resolve: an unknown control or property
/// is not evidence that the write is impossible, and suppressing it would hide a real
/// mismatch behind silence.
fn property_is_writable(widget: &str, name: &str) -> bool {
    #[cfg(not(alloc_frugal))]
    {
        let factory = crate::widget::WidgetFactory::new_with_defaults();
        if let Ok(schema) = factory.property_schema(widget, name) {
            return schema.writable;
        }
    }
    true
}

/// Whether two creation names denote the same control.
///
/// `Node.widget` is a name, not a `WidgetKind`, and the factory accepts several
/// spellings for one kind. `WidgetFactory::capability` already owns that resolution
/// (it normalises through the same name normaliser the registry was built with, and
/// applies the alias table), so asking it is the single source of truth for "are these
/// the same control?".
///
/// # Fallback
///
/// A build without the capability registry, or a name the factory does not know, cannot
/// be resolved. Those fall back to a separator-and-case-insensitive comparison, which is
/// strictly weaker but never claims two genuinely different controls are the same one —
/// an unresolvable name compares equal only to itself modulo punctuation.
fn same_control_kind(left: &str, right: &str) -> bool {
    #[cfg(not(alloc_frugal))]
    {
        let factory = crate::widget::WidgetFactory::new_with_defaults();
        if let (Some(a), Some(b)) = (factory.capability(left), factory.capability(right)) {
            return a.kind == b.kind;
        }
    }
    use crate::widget::capability::coercion::normalize_key;
    normalize_key(left) == normalize_key(right)
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
    //
    // The comparison is on the resolved **kind**, not on the declared name. `Node.widget`
    // is a *creation* name and several names resolve to one kind — `scrollarea`,
    // `scroll_area` and `scroll widget` all mean `ScrollArea`, and the factory accepts
    // all of them. Comparing names made a view that only respelled a node (a rename, a
    // formatting change) look like a type change, so the control was destroyed and
    // rebuilt — losing exactly the focus and scroll state `Patch::Replace` is documented
    // to cost. Also compare the keyless positional rule's `widget` field below: it has
    // the same requirement, so both go through `same_control_kind`.
    let type_changed = !same_control_kind(&old.widget, &new.widget);

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
            // The root has no parent to insert into, and no `Replace` parent to name:
            // a document has one root, so a root type change is a wholesale remount that
            // only the caller can perform. It used to be emitted as
            // `Replace { parent: 0 }`, a parent id that `apply_one` always rejects — so
            // the patch could never succeed and the only trace was an
            // `UnknownParent` error that `ViewEngine::update` throws away. Recording it
            // and emitting nothing makes the condition visible instead of inert.
            //
            // A root whose old control produced no id (a `spacer`/transparent node) and
            // whose new node is a real control is the *same* condition and was reported as
            // "unchanged": the old tree had nothing mounted and the new one needs something,
            // so the trees differ in the most consequential way possible — there is a control
            // now where there was none. The separate arm below records it. Reaching `(None,
            // None)` therefore means neither side produced a control, which is the only case
            // where there is genuinely nothing to report.
            (None, None) => {
                if !same_control_kind(&old.widget, &new.widget) {
                    report.root_replaced = true;
                }
            }
            (Some(_id), None) => {
                report.root_replaced = true;
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
            // `Null` is the documented "not set" value and the contract's `set` path
            // decides what it means per property.
            //
            // A bare `Null` is only meaningful where a control explicitly handles it
            // (about twenty do); most properties write through a typed extractor that
            // rejects it. `apply` therefore substitutes the schema-declared default
            // before writing — it is the layer that can see the live control's kind,
            // which is what the schema lookup is keyed on. See `apply_one`.
            //
            // A name the schema declares non-writable is skipped entirely. Emitting it
            // produced a patch that could not succeed — `geometry` is the case in point,
            // so a view that merely stopped declaring a `geometry` binding always
            // generated one refused write, and the engine's only trace was a warning it
            // discarded. A report should not describe patches the contract forbids.
            if !property_is_writable(&old.widget, name) {
                continue;
            }
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
    //
    // A key claimed by more than one sibling is recorded as **unmappable** rather than
    // last-write-wins. Previously this map was a plain insert, so with
    // `old = [A(k="dup"), B(k="dup"), C]` both entries for `"dup"` resolved to B's
    // index: the first new child consumed it, the second found it consumed and —
    // because a *keyed* child deliberately does not fall through to positional
    // matching — was treated as brand new and `Insert`ed, while the still-unconsumed
    // original was `Remove`d. A stable list whose keys accidentally collided therefore
    // destroyed and recreated controls (losing focus and scroll) while
    // `positional_matches` and `replaced_subtrees` both read clean.
    //
    // The ambiguity is genuine: two siblings cannot share one identity. Recording it as
    // unmappable means such a child takes the "genuinely new" path deterministically,
    // and `duplicate_keys` makes the condition visible to the caller through
    // `DiffReport` instead of hiding it.
    let mut old_key_index: HashMap<&str, usize> = HashMap::new();
    let mut duplicated_keys: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for (i, child) in old.children.iter().enumerate() {
        if let Some(k) = child.key_str() {
            if old_key_index.insert(k, i).is_some() {
                duplicated_keys.insert(k);
            }
        }
    }
    report.duplicate_keys += duplicated_keys.len();

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
            Some(k) => {
                // A key shared by several old siblings identifies none of them, so it is
                // unmappable and this child is treated as new — the same answer the
                // key-not-found path gives, and the only deterministic one available.
                if duplicated_keys.contains(k) {
                    None
                } else {
                    old_key_index.get(k).copied().filter(|&i| !consumed[i])
                }
            }
            None => {
                // Keyless children have no identity to honour, so they match the first
                // unconsumed old child of the same widget type. Restricting by type keeps a
                // reordered list from matching a `label` to a `button`, which would then
                // emit a cascade of refused writes.
                //
                // "Same type" is the resolved kind, not the declared spelling: an alias
                // respelling must not turn a positional match into a `Replace`.
                let positional =
                    old.children.iter().enumerate().position(|(i, c)| {
                        !consumed[i] && same_control_kind(&c.widget, &child.widget)
                    });
                // This is the branch the field measures, so this is where it is counted.
                //
                // `positional_matches` used to be incremented in the *not-found* arm below,
                // which inverted its meaning: a stable keyless list — every child matched by
                // position, exactly the case the field documents — reported **0**, while a
                // genuinely appended child reported 1. A keyed match is not a positional one
                // and is not counted here; `b4_8b_keyed_children_do_not_count_as_positional`
                // pins that.
                if positional.is_some() {
                    report.positional_matches += 1;
                }
                positional
            }
        };
        if let Some(i) = found {
            consumed[i] = true;
        } else if child.key.is_none() {
            // A keyless child with no positional match is genuinely new: a control is created
            // for it where the old tree had none.
            report.new_keyless_children += 1;
        }
        matches.push(found);
    }

    // Step 2: emit for each new child, in order.
    let mut last_matched_old: Option<usize> = None;
    for (new_index, child) in new.children.iter().enumerate() {
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
                // Recurse against the child's **old** index path.
                //
                // The old path is the right one to pass: `old` is the tree the previous build
                // produced, `id_of` addresses that build, and the child being updated still
                // occupies its old slot in it. The two indices differ exactly when a move
                // happened — the case this branch exists for — so passing the new index would
                // have the recursion read a different child's properties and emit writes
                // against the wrong control. The comment here previously said the opposite of
                // what the code did, and a dead `child_path` for the new index kept the
                // misleading shape alive; both are gone.
                let mut old_child_path = path.to_vec();
                old_child_path.push(old_index);
                diff_node(
                    &old.children[old_index],
                    child,
                    &old_child_path,
                    id_of(&old_child_path, old_index),
                    new_index,
                    id_of,
                    report,
                );
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
    for (i, _child) in old.children.iter().enumerate() {
        if consumed[i] {
            continue;
        }
        let mut p = path.to_vec();
        p.push(i);
        if let Some(id) = id_of(&p, i) {
            report.patches.push(Patch::Remove { id });
        }
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
            // Only the path and the node matter here: this fixture mirrors a pre-order walk,
            // and a child's own index is already encoded in the path pushed onto the stack.
            let mut stack: Vec<(Vec<usize>, &Node)> = vec![(Vec::new(), server)];
            while let Some((path, node)) = stack.pop() {
                self.by_path.insert(path.clone(), self.next);
                self.next += 1;
                for (i, child) in node.children.iter().enumerate().rev() {
                    let mut p = path.clone();
                    p.push(i);
                    stack.push((p, child));
                }
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

    // ── The conditional/list completeness primitives drive the diff ───────────

    /// Toggling `child_if` on must read as an `Insert`, not as a property write on a node
    /// that was always there. This is the whole reason the helper omits the node rather
    /// than marking it hidden: a hidden node would keep its identity, so the diff would
    /// find a match and the control would never actually appear.
    #[test]
    fn a_toggled_child_if_is_an_insert_not_a_hidden_match() {
        let old = Node::new("row").key("root").child(Node::new("label").key("a"));
        let new = old.clone().child_if(true, Node::new("badge").key("b"));
        let report = run(&old, &new);
        assert_eq!(report.patch_count(), 1, "got {:?}", report.patches);
        match &report.patches[0] {
            Patch::Insert { index, node, .. } => {
                assert_eq!(*index, 1);
                assert_eq!(node.key_str(), Some("b"));
            }
            other => panic!("expected Insert, got {other:?}"),
        }

        // And the reverse direction removes exactly that control.
        let report = run(&new, &old);
        assert_eq!(report.patch_count(), 1, "got {:?}", report.patches);
        assert!(matches!(report.patches[0], Patch::Remove { .. }));
    }

    /// `child_if_else` swapping branches must be a replacement, because the two branches
    /// are different controls — not a property write on one control that happens to be
    /// reused.
    #[test]
    fn a_swapped_child_if_else_branch_is_not_a_property_write() {
        let loading = Node::new("body").key("root").child_if_else(
            true,
            Node::new("spinner").key("s"),
            Node::new("content").key("c"),
        );
        let ready = Node::new("body").key("root").child_if_else(
            false,
            Node::new("spinner").key("s"),
            Node::new("content").key("c"),
        );
        let report = run(&loading, &ready);
        // The two branches carry different keys, so the correct reading is "the old node
        // went away and the new one appeared" — an `Insert` plus a `Remove`, each naming
        // the control it acts on. (`Replace` is for a node that *matched* — same key or
        // position — and then changed type; there the identity is meant to be kept and only
        // the control swapped.) What would be wrong in every reading is a `SetProperty`,
        // because that would mean one branch's control had been silently reused as the
        // other.
        assert!(
            report.patches.iter().all(|p| !matches!(p, Patch::SetProperty { .. })),
            "branches must not be merged into a property update: {:?}",
            report.patches
        );
        assert_eq!(report.patches_of_kind("Insert").len(), 1, "got {:?}", report.patches);
        assert_eq!(report.patches_of_kind("Remove").len(), 1, "got {:?}", report.patches);
        assert!(!report.root_replaced, "the root is the same body");
    }

    /// A keyed list must survive a head insertion without renumbering its siblings — the
    /// property `children_keyed` exists to make the default rather than an opt-in.
    #[test]
    fn a_children_keyed_list_head_insert_does_not_renumber_its_siblings() {
        let keys = ["b", "c"];
        let old = Node::new("list").key("root").children_keyed(
            &keys,
            |k| (*k).to_string(),
            |k| Node::new("label").prop("text", s(k)),
        );
        let grown = ["a", "b", "c"];
        let new = Node::new("list").key("root").children_keyed(
            &grown,
            |k| (*k).to_string(),
            |k| Node::new("label").prop("text", s(k)),
        );
        let report = run(&old, &new);
        assert_eq!(report.patch_count(), 1, "one insert, no renumbering: {:?}", report.patches);
        assert_eq!(report.positional_matches, 0, "every node was matched by key");
        match &report.patches[0] {
            Patch::Insert { index, node, .. } => {
                assert_eq!(*index, 0, "the new item goes to the head");
                assert_eq!(node.key_str(), Some("a"));
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
        // The root has no parent, so this is the *unexpressible* case: see
        // `b4_7c_a_root_type_change_is_reported_rather_than_patched`. A child is where a
        // `Replace` is actually applicable, and it is covered by
        // `b4_7b_a_type_change_on_a_child_is_replaced_in_place`.
        //
        // This test used to assert `replaced_subtrees == 1` and one `Replace` patch for a
        // **parentless** node. That patch carried `parent: 0`, which `apply_one` always rejects
        // (`is_mounted` is false for id `0`), so the assertion was pinning an unsatisfiable
        // patch as correct behaviour.
        let old = Node::new("label").key("a").prop("text", s("A"));
        let new = Node::new("button").key("a").prop("text", s("A"));
        let report = run(&old, &new);
        assert!(report.root_replaced, "a root type change must be reported");
        assert!(
            report.patches_of_kind("Replace").is_empty(),
            "no patch can replace a root; emitting one would be unsatisfiable: {:?}",
            report.patches
        );
        assert_eq!(report.replaced_subtrees, 0, "nothing was replaced — the caller must remount");
    }

    /// A root type change is reported, and no patch pretends to carry it out.
    ///
    /// The old behaviour emitted `Replace { id, parent: 0, .. }`. Applying it produced
    /// `ViewError::UnknownParent { parent: 0 }` and changed nothing, while `ViewEngine::update`
    /// discarded that error — so `DiffReport` looked successful and the engine kept serving the
    /// pre-change root. `root_replaced` is the honest channel.
    #[test]
    fn b4_7c_a_root_type_change_is_reported_rather_than_patched() {
        let old = Node::new("panel").key("root");
        let new = Node::new("vbox").key("root");
        let report = run(&old, &new);
        assert!(report.root_replaced);
        assert!(report.patches.is_empty(), "{:?}", report.patches);

        // The converse: an unchanged root is not reported.
        let same = Node::new("panel").key("root");
        assert!(!run(&old, &same).root_replaced);
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
        // …and the counter has to say *how* that match was made. This test is precisely the
        // scenario `positional_matches` documents — a stable keyless child found by position —
        // and it asserted nothing about the field, which is how the field came to be counted in
        // the opposite branch and read 0 here.
        assert_eq!(
            report.positional_matches, 1,
            "the keyless label was matched by position, so the field must report one"
        );
        assert_eq!(
            report.new_keyless_children, 0,
            "nothing was created, so nothing may be reported as new"
        );
    }

    #[test]
    fn an_appended_keyless_child_is_new_rather_than_a_positional_match() {
        // The other side of the same pair of counters. An appended row has no positional match
        // at all: a control is created for it. Reporting that as a "positional match" is what
        // the combined counter used to do, and it told a caller the opposite of what happened —
        // "your identity is holding" instead of "you are rebuilding a row every update".
        let old = Node::new("vbox").key("root").child(Node::new("label").prop("text", s("a")));
        let new = Node::new("vbox")
            .key("root")
            .child(Node::new("label").prop("text", s("a")))
            .child(Node::new("label").prop("text", s("b")));
        let report = run(&old, &new);
        assert_eq!(report.positional_matches, 1, "only the surviving row matched");
        assert_eq!(report.new_keyless_children, 1, "the appended row is new");
    }

    // ── The root arms (E4) ─────────────────────────────────

    #[test]
    fn a_root_that_gains_a_control_is_reported_as_replaced() {
        // Old root produces no control (`spacer` is a transparent node with no capability, so
        // it has no live id) and the new root is a real control. The trees differ in the most
        // consequential way there is — something is mounted where nothing was — and the arm that
        // handles `(None, None)` used to return without setting `root_replaced`, so `update`
        // reported the view unchanged and the control never appeared.
        let old = Node::new("spacer").key("root");
        let new = Node::new("button").key("root");
        let report = run(&old, &new);
        assert!(
            report.root_replaced,
            "a root that goes from no control to a control is a remount, not an unchanged view"
        );
    }

    #[test]
    fn a_root_with_no_control_on_either_side_is_genuinely_unchanged() {
        // The reverse direction, so the fix above cannot be satisfied by always reporting a
        // remount: two roots that both produce nothing have nothing to rebuild.
        let old = Node::new("spacer").key("root");
        let new = Node::new("spacer").key("root");
        let report = run(&old, &new);
        assert!(!report.root_replaced);
        assert!(report.is_unchanged());
    }

    #[test]
    fn a_root_type_change_whose_old_side_has_no_id_is_reported_as_replaced() {
        let old = Node::new("spacer").key("root");
        let new = Node::new("label").key("root").prop("text", s("x"));
        let report = run(&old, &new);
        assert!(report.root_replaced);
    }

    // ── The deletion pass does not emit impossibilities (E3) ─

    #[test]
    fn dropping_a_read_only_binding_emits_no_patch() {
        // `geometry` is declared `false, false` in every schema and `properties_trait.rs`
        // answers `ReadOnlyProperty` for it unconditionally, so a `Null` write against it can
        // never succeed. The deletion pass used to emit one anyway, which made the report
        // describe a patch the engine could only warn about and drop — a wrong description of
        // what the update does, in the direction that hides a real problem: a caller reading
        // `patch_count()` saw a write that never reached a control.
        let old = Node::new("label").key("a").prop("geometry", s("0,0,10,10"));
        let new = Node::new("label").key("a");
        let report = run(&old, &new);
        assert!(
            report.patches.is_empty(),
            "a read-only property cannot be reset, so no patch may be promised: {:?}",
            report.patches
        );
    }

    #[test]
    fn dropping_a_writable_binding_still_emits_the_reset() {
        // The reverse direction, so the fix above cannot pass by suppressing every deletion.
        // `text` is writable, so removing the binding is a real change that must be emitted.
        let old = Node::new("label").key("a").prop("text", s("hello"));
        let new = Node::new("label").key("a");
        let report = run(&old, &new);
        assert_eq!(report.patches.len(), 1);
        assert_eq!(report.written_properties(), ["text"]);
        assert!(matches!(
            report.patches[0],
            Patch::SetProperty { value: CapabilityValue::Null, .. }
        ));
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
    fn duplicate_sibling_keys_are_reported_rather_than_silently_mismatched() {
        // A key shared by two siblings identifies neither of them. The diff used to
        // resolve it last-write-wins, so a reorder of the duplicated pair matched the
        // wrong node, `Insert`ed the other, and `Remove`d the leftover — losing the
        // control's state while `positional_matches` and `replaced_subtrees` both read
        // zero, i.e. while the report implied the *best* match quality.
        //
        // The condition is now named in the report, and a caller can look up which keys
        // collided with `Node::duplicate_sibling_keys`.
        let old = Node::new("vbox")
            .key("root")
            .child(Node::new("label").key("dup").prop("text", s("A")))
            .child(Node::new("label").key("dup").prop("text", s("B")))
            .child(Node::new("label").key("c"));
        let new = Node::new("vbox")
            .key("root")
            .child(Node::new("label").key("dup").prop("text", s("B")))
            .child(Node::new("label").key("dup").prop("text", s("A")))
            .child(Node::new("label").key("c"));

        assert_eq!(
            old.duplicate_sibling_keys(),
            vec![("dup", 2)],
            "the reporter must name the colliding key"
        );

        let report = run(&old, &new);
        assert!(
            report.duplicate_keys > 0,
            "a duplicate key must be reported, not silently mismatched: {:?}",
            report.patches
        );
    }

    /// A tree with unique keys reports no duplicates, so the counter above is not a
    /// constant.
    #[test]
    fn unique_sibling_keys_report_no_duplicates() {
        let old = Node::new("vbox")
            .key("root")
            .child(Node::new("label").key("a"))
            .child(Node::new("label").key("b"));
        let new = Node::new("vbox")
            .key("root")
            .child(Node::new("label").key("b"))
            .child(Node::new("label").key("a"));
        assert!(old.duplicate_sibling_keys().is_empty());
        let report = run(&old, &new);
        assert_eq!(report.duplicate_keys, 0);
        // A pure reorder of keyed, uniquely-named siblings is a `Move`, not a rebuild.
        assert!(!report.patches_of_kind("Move").is_empty());
        assert!(report.patches_of_kind("Replace").is_empty());
    }

    #[test]
    fn respelling_a_node_with_an_alias_is_an_update_not_a_replace() {
        // `Node.widget` is a *creation* name, and the factory accepts several spellings
        // for one kind. Comparing declared names made a view that only respelled a node
        // look like a type change, so the control was replaced — destroying focus and
        // scroll state for a control that did not change kind at all.
        let old = Node::new("scrollarea")
            .key("body")
            .prop("scroll_position_y", CapabilityValue::Int(120));
        let new = Node::new("scroll_area")
            .key("body")
            .prop("scroll_position_y", CapabilityValue::Int(120));
        let report = run(&old, &new);
        assert!(
            report.patches_of_kind("Replace").is_empty(),
            "an alias respelling must not replace the control: {:?}",
            report.patches
        );
        assert_eq!(report.replaced_subtrees, 0);
    }

    /// The converse: a genuinely different kind **must** still be replaced.
    ///
    /// A child, not a root — a `Replace` names a parent, and a root has none.
    #[test]
    fn a_different_control_under_another_name_is_still_replaced() {
        let old =
            Node::new("panel").key("root").child(Node::new("label").key("a").prop("text", s("A")));
        let new = Node::new("panel").key("root").child(Node::new("button").key("a"));
        let report = run(&old, &new);
        assert_eq!(report.patches_of_kind("Replace").len(), 1, "{:?}", report.patches);
        assert_eq!(report.replaced_subtrees, 1);
        assert!(!report.root_replaced);
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
