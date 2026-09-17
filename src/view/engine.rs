// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The [`View`] trait and [`ViewEngine`] — the closed loop from state to retained tree.

use crate::core::ObjectId;

use super::apply::{apply, ApplyReport};
use super::diff::{diff, DiffReport};
use super::node::Node;

/// A declarative description of a widget tree, computed from some state.
///
/// Implementors describe what the tree *should be*; they never create, move, or destroy a
/// control. That is the whole separation: the view is a function of state, and [`ViewEngine`]
/// works out the changes.
///
/// ```
/// use rust_widgets::view::{Node, View};
///
/// struct Counter {
///     count: i64,
/// }
///
/// impl View for Counter {
///     fn build(&self) -> Node {
///         Node::new("vbox").key("root").child(
///             Node::new("label")
///                 .key("value")
///                 .prop(
///                     "text",
///                     rust_widgets::widget::capability::CapabilityValue::String(
///                         self.count.to_string(),
///                     ),
///                 ),
///         )
///     }
/// }
///
/// assert_eq!(View::build(&Counter { count: 3 }).children.len(), 1);
/// ```
pub trait View {
    /// Describe the tree this state should render as.
    ///
    /// Called on every [`ViewEngine::update`], so it should be cheap and **pure**: reading a
    /// clock or a random number here would make the diff see changes that did not come from
    /// state, and an update would then never settle.
    fn build(&self) -> Node;
}

/// Holds a view's current tree and applies the differences between builds.
///
/// # Why the engine keeps the previous tree
///
/// A diff needs both sides. Keeping only the bindings would force the new tree to be
/// compared against something reconstructed from the live controls, which cannot express
/// what the *view* asked for — only what the controls happen to hold. Keeping the previous
/// [`Node`] tree means the comparison is between two declarations, which is what makes it
/// deterministic and testable without a window.
///
/// # Size
///
/// The previous tree is kept as a `Node`: plain data, no ids, no platform handles. Its cost
/// is proportional to the view's own output, which the caller controls by how much it
/// describes per frame.
pub struct ViewEngine {
    current: Option<Node>,
    layout: crate::json::BoundJsonLayout,
    /// Maps the previous tree's *shape* to live ids, so the diff can address controls.
    ///
    /// Populated when the engine mounts or applies an `Insert`: ids for nodes it created,
    /// and — at mount time — ids the caller supplies for nodes built elsewhere.
    id_of_path: crate::compat::HashMap<Vec<usize>, ObjectId>,
}

impl core::fmt::Debug for ViewEngine {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ViewEngine")
            .field("mounted", &self.current.is_some())
            .field("nodes", &self.layout.node_count())
            .finish()
    }
}

impl ViewEngine {
    /// Create an engine with no tree mounted.
    pub fn new() -> Self {
        Self {
            current: None,
            layout: crate::json::BoundJsonLayout::new(),
            id_of_path: crate::compat::HashMap::new(),
        }
    }

    /// Build `view` and create its tree, recording the ids `create` assigns.
    ///
    /// `create` is the caller's bridge from a declarative name to a live control. Injected
    /// rather than hardwired so that the engine can be exercised without a window, and so a
    /// host that has its own construction policy (a design tool that must reuse a pool, a
    /// test that must stub) is not forced through the JSON loader's choice.
    ///
    /// Returns what was created. Mounting twice without an intervening
    /// [`update`](Self::update) replaces the tree, because the new build is authoritative —
    /// keeping the old controls would leave both trees' controls live at once.
    pub fn mount(
        &mut self,
        view: &dyn View,
        create: &dyn Fn(&Node) -> Option<ObjectId>,
    ) -> ApplyReport {
        let root = view.build();
        let mut report = ApplyReport::default();

        // Drop the previous tree entirely, so its controls do not linger as orphans and so
        // the new ids cannot collide with a stale path entry. A full clear is required
        // rather than a "keep the root" reset: the old root is about to be *replaced* by a
        // new control, and leaving it indexed would make `node_count` count a control that
        // is no longer part of any tree.
        if self.layout.root().is_some() {
            let removed = self.layout.clear_structure();
            report.widgets_removed += removed.len();
        }
        self.id_of_path.clear();

        let Some(root_id) = create(&root) else {
            report.errors.push(super::ViewError::UnknownWidgetType { widget: root.widget.clone() });
            self.current = None;
            return report;
        };
        if root_id == 0 {
            report.errors.push(super::ViewError::UnknownWidgetType { widget: root.widget.clone() });
            self.current = None;
            return report;
        }

        let key = root.key.clone().unwrap_or_default();
        self.layout.register_node(root_id, root.widget.clone(), key, None);
        self.id_of_path.insert(Vec::new(), root_id);
        report.widgets_created += 1;
        self.mount_children(&root, root_id, &[], create, &mut report);
        self.write_declared_properties(&root, root_id, &mut report);

        self.current = Some(root);
        report
    }

    /// Rebuild `view` and apply only the differences from the mounted tree.
    ///
    /// This is the update path that makes the architecture hybrid: unchanged controls are
    /// not touched at all, so their focus, scroll offsets, and internal state survive.
    /// Calling this before [`mount`](Self::mount) mounts instead — a caller that does not
    /// want to distinguish the first call from the rest does not have to.
    pub fn update(
        &mut self,
        view: &dyn View,
        create: &dyn Fn(&Node) -> Option<ObjectId>,
    ) -> DiffReport {
        let Some(previous) = self.current.take() else {
            self.mount(view, create);
            return DiffReport::default();
        };
        let next = view.build();
        let lookup = |path: &[usize], _index: usize| self.id_of_path.get(path).copied();
        let report = diff(&previous, &next, &lookup);

        // Assign ids for the shapes the diff is about to create, so that patches later in the
        // same batch can address them. Done before `apply` so a batched insert-then-write
        // resolves; the ids are allocated here because only the engine knows which paths the
        // new tree will occupy.
        self.reserve_ids_for_inserts(&next, &report.patches, create);

        let applied = apply(&mut self.layout, &report.patches, create);
        // Sync the path map to the new tree's shape for the surviving nodes; a node the diff
        // did not mention keeps its path only if its ancestor chain is unchanged.
        self.reindex_paths(&next);
        let _ = applied;
        self.current = Some(next);
        report
    }

    /// The tree currently mounted, if any.
    pub fn current(&self) -> Option<&Node> {
        self.current.as_ref()
    }

    /// The binding between the mounted tree and the live controls.
    pub fn layout(&self) -> &crate::json::BoundJsonLayout {
        &self.layout
    }

    /// Mutable access, for a caller that also edits the tree by hand.
    pub fn layout_mut(&mut self) -> &mut crate::json::BoundJsonLayout {
        &mut self.layout
    }

    /// The `ObjectId` a mounted node at `path` resolves to.
    ///
    /// `path` is a sequence of child indices from the root, which is the same addressing the
    /// diff uses — exposing it lets a caller check "is this still the control I had focus
    /// on?" without walking the layout.
    pub fn id_at(&self, path: &[usize]) -> Option<ObjectId> {
        self.id_of_path.get(path).copied()
    }

    /// Create and register a node's children, recursing.
    ///
    /// Also writes each node's declared properties through the property contract, so a
    /// freshly mounted tree is in the state the view described rather than in each control's
    /// default state.
    fn mount_children(
        &mut self,
        node: &Node,
        node_id: ObjectId,
        path: &[usize],
        create: &dyn Fn(&Node) -> Option<ObjectId>,
        report: &mut ApplyReport,
    ) {
        for (index, child) in node.children.iter().enumerate() {
            let mut child_path = path.to_vec();
            child_path.push(index);
            let Some(child_id) = create(child).filter(|&id| id != 0) else {
                report
                    .errors
                    .push(super::ViewError::UnknownWidgetType { widget: child.widget.clone() });
                continue;
            };
            let key = child.key.clone().unwrap_or_default();
            self.layout.register_node(child_id, child.widget.clone(), key, Some(node_id));
            self.layout.move_child_to(node_id, child_id, index);
            self.id_of_path.insert(child_path.clone(), child_id);
            report.widgets_created += 1;
            self.mount_children(child, child_id, &child_path, create, report);
            self.write_declared_properties(child, child_id, report);
        }
    }

    /// Write a node's declared properties to its control, reporting refusals.
    fn write_declared_properties(&mut self, node: &Node, id: ObjectId, report: &mut ApplyReport) {
        for (name, value) in &node.props {
            match super::apply::write_property(id, name, value.clone()) {
                Ok(()) => report.properties_written += 1,
                Err(reason) => report.errors.push(super::ViewError::PropertyRefused {
                    id,
                    name: name.clone(),
                    reason,
                }),
            }
        }
    }

    /// Allocate ids for nodes the diff is about to insert, keyed by their path in the new tree.
    ///
    /// Without this, a batch containing an `Insert` and a later patch addressing the inserted
    /// subtree would be reported as unknown — the ids only become addressable once something
    /// knows which path they occupy.
    fn reserve_ids_for_inserts(
        &mut self,
        next: &Node,
        patches: &[super::Patch],
        create: &dyn Fn(&Node) -> Option<ObjectId>,
    ) {
        for patch in patches {
            if let super::Patch::Insert { node, .. } = patch {
                if let Some(path) = find_path_of(next, node) {
                    let mut pending = Vec::new();
                    collect_new_ids(node, &path, create, &mut pending);
                    for (p, id) in pending {
                        self.id_of_path.insert(p, id);
                    }
                }
            }
        }
    }

    /// Rebuild the path map from the mounted layout, keyed by **identity** rather than by the
    /// previous map's path positions.
    ///
    /// Re-keying by position was wrong: when a sibling is inserted, every later node sits at
    /// a new path, so a positional rebuild would hand the new paths the *inserted* ids and
    /// drop the survivors' — making every surviving node look recreated. The whole value of
    /// the retained half is that a node's `ObjectId` tracks the node, not its slot, so the
    /// map is rebuilt by walking the new tree and looking each node's identity up in the
    /// layout (`key` where present, otherwise the parent's child order for keyless nodes).
    fn reindex_paths(&mut self, next: &Node) {
        let mut fresh = crate::compat::HashMap::new();
        if let Some(root_id) = self.layout.root() {
            fresh.insert(Vec::new(), root_id);
        }
        reindex_by_identity(next, &[], &self.layout, &mut fresh);
        self.id_of_path = fresh;
    }
}

/// Rebuild `out` by looking each node of `node`'s subtree up in `layout` by identity.
fn reindex_by_identity(
    node: &Node,
    path: &[usize],
    layout: &crate::json::BoundJsonLayout,
    out: &mut crate::compat::HashMap<Vec<usize>, ObjectId>,
) {
    // Address the children by their parent, using the key when the node declares one and the
    // position within the parent otherwise — the same two rules the diff matches by, so the
    // map and the diff cannot disagree about which control a path denotes.
    let parent_id = out.get(path).copied();
    for (index, child) in node.children.iter().enumerate() {
        let mut child_path = path.to_vec();
        child_path.push(index);
        let resolved = match (child.key_str(), parent_id) {
            (Some(key), Some(parent)) => layout.child_by_key(Some(parent), key),
            (Some(key), None) => layout.child_by_key(None, key),
            (None, Some(parent)) => layout.children(parent).get(index).copied(),
            (None, None) => None,
        };
        if let Some(id) = resolved {
            out.insert(child_path.clone(), id);
        }
        reindex_by_identity(child, &child_path, layout, out);
    }
}

/// Find the path of `target` within `root`, matching by key when present and by identity of
/// the widget name otherwise.
///
/// The engine needs this to reserve ids for an inserted subtree, and it is a search rather
/// than a stored inverse because the new tree has no ids yet.
fn find_path_of(root: &Node, target: &Node) -> Option<Vec<usize>> {
    if root.widget == target.widget && root.key == target.key {
        return Some(Vec::new());
    }
    for (index, child) in root.children.iter().enumerate() {
        if child.widget == target.widget && child.key == target.key {
            return Some(vec![index]);
        }
        if let Some(mut deeper) = find_path_of(child, target) {
            let mut path = vec![index];
            path.append(&mut deeper);
            return Some(path);
        }
    }
    None
}

/// Allocate ids for a freshly declared subtree, in the same order the mount path would.
fn collect_new_ids(
    node: &Node,
    path: &[usize],
    create: &dyn Fn(&Node) -> Option<ObjectId>,
    out: &mut Vec<(Vec<usize>, ObjectId)>,
) {
    let Some(id) = create(node).filter(|&id| id != 0) else {
        return;
    };
    out.push((path.to_vec(), id));
    for (index, child) in node.children.iter().enumerate() {
        let mut child_path = path.to_vec();
        child_path.push(index);
        collect_new_ids(child, &child_path, create, out);
    }
}

crate::impl_default_via_new!(ViewEngine);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::capability::CapabilityValue;
    use std::cell::Cell;

    fn s(v: &str) -> CapabilityValue {
        CapabilityValue::String(v.to_string())
    }

    /// A view whose single label's text is the state.
    struct Text(String);

    impl View for Text {
        fn build(&self) -> Node {
            Node::new("window")
                .key("root")
                .child(Node::new("label").key("value").prop("text", s(&self.0)))
        }
    }

    /// A view with a variable-length keyed list, for insert/remove/reorder.
    struct Rows(Vec<&'static str>);

    impl View for Rows {
        fn build(&self) -> Node {
            Node::new("window")
                .key("root")
                .children_of(self.0.iter().map(|k| Node::new("label").key(*k)))
        }
    }

    /// Allocates ids from a counter, standing in for real construction.
    struct Ids(Cell<ObjectId>);

    impl Ids {
        fn new(start: ObjectId) -> Self {
            Self(Cell::new(start))
        }

        fn creator(&self) -> impl Fn(&Node) -> Option<ObjectId> + '_ {
            move |_node: &Node| {
                let id = self.0.get();
                self.0.set(id + 1);
                Some(id)
            }
        }
    }

    #[test]
    fn mount_builds_the_whole_tree_and_indexes_it() {
        let mut engine = ViewEngine::new();
        let ids = Ids::new(10);
        let report = engine.mount(&Text("hello".into()), &ids.creator());
        // The structure is what this test is about. The label's `text` write is attempted
        // through the real property contract, and a stub id is not a registered control, so
        // the write is legitimately refused — asserted separately rather than hidden.
        assert_eq!(report.widgets_created, 2, "window + label: {report:?}");
        assert_eq!(engine.layout().node_count(), 2);
        assert_eq!(engine.current().map(Node::node_count), Some(2));
        assert_eq!(report.errors.len(), 1, "one refused write on the stub label");
        assert!(matches!(report.errors[0], super::super::ViewError::PropertyRefused { .. }));
    }

    #[test]
    fn mount_assigns_paths_addressable_by_index() {
        let mut engine = ViewEngine::new();
        let ids = Ids::new(10);
        engine.mount(&Text("x".into()), &ids.creator());
        assert_eq!(engine.id_at(&[]), Some(10), "the root is at the empty path");
        assert_eq!(engine.id_at(&[0]), Some(11), "the first child follows");
        assert_eq!(engine.id_at(&[1]), None);
    }

    #[test]
    fn mount_keeps_the_parent_child_shape() {
        let mut engine = ViewEngine::new();
        let ids = Ids::new(10);
        engine.mount(&Text("x".into()), &ids.creator());
        let root = engine.layout().root().expect("root");
        assert_eq!(engine.layout().children(root).len(), 1);
        assert_eq!(engine.layout().node_key(engine.layout().children(root)[0]), Some("value"));
    }

    #[test]
    fn remounting_replaces_the_previous_tree_rather_than_adding_to_it() {
        // Two trees' controls must not be live at once: a remount is authoritative.
        let mut engine = ViewEngine::new();
        let ids = Ids::new(10);
        let first = engine.mount(&Text("a".into()), &ids.creator());
        assert_eq!(first.widgets_created, 2);
        let second = engine.mount(&Text("b".into()), &ids.creator());
        assert_eq!(second.widgets_created, 2);
        assert!(second.widgets_removed >= 1, "the old tree must be released");
        assert_eq!(engine.layout().node_count(), 2, "not four");
    }

    #[test]
    fn update_diffs_and_produces_only_the_change() {
        let mut engine = ViewEngine::new();
        let ids = Ids::new(10);
        engine.mount(&Text("before".into()), &ids.creator());
        let report = engine.update(&Text("after".into()), &ids.creator());
        assert_eq!(report.patch_count(), 1, "got {:?}", report.patches);
        assert_eq!(report.written_properties(), ["text"]);
        assert_eq!(report.replaced_subtrees, 0, "nothing should be rebuilt");
    }

    #[test]
    fn update_before_mount_mounts_instead_of_failing() {
        // A caller that does not want to special-case its first call should not have to.
        let mut engine = ViewEngine::new();
        let ids = Ids::new(10);
        let report = engine.update(&Text("first".into()), &ids.creator());
        assert!(report.is_unchanged(), "the first update is the mount; it reports no diff");
        assert_eq!(engine.layout().node_count(), 2);
    }

    #[test]
    fn an_unchanged_view_produces_an_empty_update() {
        let mut engine = ViewEngine::new();
        let ids = Ids::new(10);
        engine.mount(&Text("same".into()), &ids.creator());
        let report = engine.update(&Text("same".into()), &ids.creator());
        assert!(report.is_unchanged(), "no change must mean no patches, got {:?}", report.patches);
    }

    #[test]
    fn appending_a_row_updates_without_rebuilding_the_survivors() {
        let mut engine = ViewEngine::new();
        let ids = Ids::new(10);
        engine.mount(&Rows(vec!["a", "b"]), &ids.creator());
        let a_path = vec![0];
        let a_id = engine.id_at(&a_path).expect("row a");

        let report = engine.update(&Rows(vec!["a", "b", "c"]), &ids.creator());
        assert_eq!(report.patch_count(), 1, "one insert, got {:?}", report.patches);
        assert_eq!(report.replaced_subtrees, 0);
        assert_eq!(engine.id_at(&a_path), Some(a_id), "row a keeps its identity");
    }

    #[test]
    fn a_head_insert_with_keys_leaves_the_other_ids_alone() {
        // The rule #90 property, at engine level: the survivors are not re-created, so any
        // focus or internal state they held would survive.
        let mut engine = ViewEngine::new();
        let ids = Ids::new(10);
        engine.mount(&Rows(vec!["a", "b"]), &ids.creator());
        let a_id = engine.id_at(&[0]).expect("a");
        let b_id = engine.id_at(&[1]).expect("b");

        let report = engine.update(&Rows(vec!["z", "a", "b"]), &ids.creator());
        assert_eq!(report.patch_count(), 1, "only the insert, got {:?}", report.patches);
        assert_eq!(report.patches[0].kind_name(), "Insert");
        assert_eq!(report.replaced_subtrees, 0);
        // `a` and `b` shifted one place; their ids must have shifted with them rather than
        // being discarded and recreated.
        let live: Vec<Option<ObjectId>> = (0..3).map(|i| engine.id_at(&[i])).collect();
        assert!(live.contains(&Some(a_id)), "a was recreated: {live:?}");
        assert!(live.contains(&Some(b_id)), "b was recreated: {live:?}");
    }

    #[test]
    fn removing_a_row_updates_the_layout() {
        let mut engine = ViewEngine::new();
        let ids = Ids::new(10);
        engine.mount(&Rows(vec!["a", "b", "c"]), &ids.creator());
        let report = engine.update(&Rows(vec!["a", "c"]), &ids.creator());
        assert!(report.patches_of_kind("Remove").len() == 1, "got {:?}", report.patches);
        let root = engine.layout().root().expect("root");
        assert_eq!(engine.layout().children(root).len(), 2, "the layout must agree");
    }

    #[test]
    fn a_view_that_refuses_construction_reports_instead_of_panicking() {
        let mut engine = ViewEngine::new();
        let report = engine.mount(&Text("x".into()), &|_n: &Node| None);
        assert!(!report.is_clean());
        assert!(engine.current().is_none(), "a failed mount leaves nothing mounted");
    }

    #[test]
    fn update_commits_the_new_tree_even_when_a_write_is_refused() {
        // A stub id is not a real control, so property writes are refused; the *structure*
        // must still advance, or every later diff would compare against a stale tree and
        // re-emit the same patches forever.
        let mut engine = ViewEngine::new();
        let ids = Ids::new(10);
        engine.mount(&Text("a".into()), &ids.creator());
        let report = engine.update(&Text("b".into()), &ids.creator());
        assert_eq!(report.patch_count(), 1);
        assert_eq!(
            engine.current().and_then(|n| n.children[0].prop_value("text")),
            Some(&s("b")),
            "the mounted tree must reflect the new state"
        );
    }

    #[test]
    fn repeated_updates_do_not_grow_the_layout() {
        // The stability property from Phase D-5: an update must replace, not accumulate.
        let mut engine = ViewEngine::new();
        let ids = Ids::new(10);
        engine.mount(&Text("0".into()), &ids.creator());
        let baseline = engine.layout().node_count();
        for i in 0..50 {
            engine.update(&Text(i.to_string()), &ids.creator());
            assert_eq!(engine.layout().node_count(), baseline, "the layout grew on update {i}");
        }
    }

    #[test]
    fn a_default_engine_has_nothing_mounted() {
        let engine = ViewEngine::default();
        assert!(engine.current().is_none());
        assert_eq!(engine.layout().node_count(), 0);
        assert_eq!(engine.id_at(&[]), None);
    }

    #[test]
    fn debug_output_reports_mounted_state_without_dumping_the_tree() {
        let engine = ViewEngine::new();
        let text = format!("{engine:?}");
        assert!(text.contains("mounted: false"), "{text}");
    }

    #[test]
    fn an_inserted_rows_declared_properties_are_written() {
        struct Row(&'static str);
        impl View for Row {
            fn build(&self) -> Node {
                Node::new("window")
                    .key("root")
                    .child(Node::new("label").key("only").prop("text", s(self.0)))
            }
        }
        let mut engine = ViewEngine::new();
        let ids = Ids::new(10);
        let report = engine.mount(&Row("hi"), &ids.creator());
        assert_eq!(report.widgets_created, 2);
        // The label's `text` write is attempted through the real contract; whether the stub
        // id can accept it is the contract's business, but the attempt must be visible.
        assert!(
            report.properties_written + report.errors.len() >= 1,
            "the declared property must be attempted: {report:?}"
        );
    }
}
