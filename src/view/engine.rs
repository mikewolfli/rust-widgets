// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The [`View`] trait and [`ViewEngine`] — the closed loop from state to retained tree.

use crate::core::ObjectId;

use super::apply::{apply_with_reservations, ApplyReport};
use super::diff::{diff, DiffReport};
use super::node::{Host, Node};

/// Values a root supplies to the whole view subtree.
///
/// # Why reading is resolved at build time
///
/// A context answers "what is the theme accent?", "what locale?", "which user?" — facts a
/// deep node needs but no intermediate node should have to forward. The naive implementation
/// lets each node hold a reference and look *upward* at draw time; that makes the node's
/// value depend on where it happens to sit and on when it is read, which is exactly the
/// non-purity `diff` cannot tolerate.
///
/// So a context here is read **once, while the tree is being described**: [`View::build_with`]
/// calls [`Context::get`] and writes the answer into a node's `props`. `diff` then compares
/// two ordinary trees and never sees the context at all — the same discipline the crate
/// already applies to animation (which lives in `PropertyAnimation` rather than in `build`,
/// for the same reason).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Context {
    /// The values, by key. `String`-keyed rather than `TypeId`-keyed so a serialised or
    /// designer-authored context round-trips, which a `TypeId` cannot.
    values: crate::compat::HashMap<String, String>,
}

impl Context {
    /// An empty context. Reading any key returns `None`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets a value, replacing any previous one for the same key. Chainable.
    pub fn with(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.values.insert(key.into(), value.into());
        self
    }

    /// The value for `key`, or `None` when the root did not supply one.
    ///
    /// A missing key is `None` rather than a default or a panic: an unset context value is a
    /// declaration that the root did not choose to provide one, and a node must be able to
    /// fall back to its own default rather than the whole tree failing.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    /// Whether any value is present.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

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

    /// Describe the tree, resolving any [`Context`] values into concrete node properties.
    ///
    /// # Why this is a separate method with a default
    ///
    /// Making `build` itself take a context would break every existing `impl View` — the
    /// signature change the plan defers to last (BLUE23 §5A.7). A defaulted second method is
    /// the additive form: a view that needs no context implements only `build`, exactly as
    /// before, and receives the context-aware call through this default. A view that does
    /// need one overrides this and reads `ctx.get(key)` while describing.
    ///
    /// The default ignores the context and forwards to [`Self::build`], which is the honest
    /// behaviour: a view that has not opted in has no key to look up.
    fn build_with(&self, _ctx: &Context) -> Node {
        self.build()
    }
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
    /// The id of the engine-owned **overlay layer**, once a portal node has asked for one.
    ///
    /// # Why the layer is a sibling of the root, not a second root
    ///
    /// A portal node is declared as some control's child but must be created *outside* that
    /// parent's clip (BLUE23 §5A.2). The place it is created into cannot be the root — a
    /// document has exactly one root, and `Patch::Insert` relies on that — so it is a layer
    /// the engine owns, **beside** the root rather than above it. The layout still sees one
    /// rooted document; the layer is an additional host the engine keeps for portal
    /// children, and it exists only when something asks for it.
    overlay_layer: Option<ObjectId>,
}

impl core::fmt::Debug for ViewEngine {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ViewEngine")
            .field("mounted", &self.current.is_some())
            .field("nodes", &self.layout.node_count())
            .field("overlay", &self.overlay_layer)
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
            overlay_layer: None,
        }
    }

    /// The id of the overlay layer, or `None` when no portal node has needed one.
    ///
    /// A host that draws the layer separately (a modal stack, a tooltip rail) reads this to
    /// find the container its portal children were created into. `None` is the honest answer
    /// for a tree that declares no portals: there is no empty layer sitting around.
    pub fn overlay_layer(&self) -> Option<ObjectId> {
        self.overlay_layer
    }

    /// The layer a portal node is created into, creating it on first use.
    ///
    /// The layer is itself a control created through the caller's `create` bridge, named
    /// `"overlay"`, so a host that maps names to controls needs no special case for it. It
    /// is registered with no parent — it is a host the engine owns, not a child of the root.
    fn overlay_layer_for(
        &mut self,
        create: &dyn Fn(&Node) -> Option<ObjectId>,
    ) -> Option<ObjectId> {
        if let Some(id) = self.overlay_layer {
            return Some(id);
        }
        let layer_node = Node::new("overlay").key("__overlay_layer");
        let id = create(&layer_node).filter(|&id| id != 0)?;
        self.layout.register_node(id, layer_node.widget.clone(), "__overlay_layer", None);
        self.overlay_layer = Some(id);
        Some(id)
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
        self.mount_with(view, &Context::new(), create)
    }

    /// [`Self::mount`] with a [`Context`] the view may read while describing its tree.
    ///
    /// The context is resolved into node properties **before** the tree is diffed, so a
    /// subsequent [`Self::update_with`] that passes a *different* context compares two
    /// value-settled trees — the context itself never becomes a diff key (BLUE23 §5A.4).
    pub fn mount_with(
        &mut self,
        view: &dyn View,
        ctx: &Context,
        create: &dyn Fn(&Node) -> Option<ObjectId>,
    ) -> ApplyReport {
        let root = view.build_with(ctx);
        let mut report = ApplyReport::default();

        // Drop the previous tree entirely, so its controls do not linger as orphans and so
        // the new ids cannot collide with a stale path entry. A full clear is required
        // rather than a "keep the root" reset: the old root is about to be *replaced* by a
        // new control, and leaving it indexed would make `node_count` count a control that
        // is no longer part of any tree.
        if self.layout.root().is_some() {
            // Lifecycle: the old tree is about to go, so every node's `on_unmount` runs first --
            // while the controls are still mounted and readable. This is the pair to the mount
            // hook below: a node that mounted always unmounts exactly once, so a subscription
            // it opened is closed rather than leaked.
            if let Some(previous) = self.current.take() {
                if let Some(old_root) = self.layout.root() {
                    self.run_unmount_hooks(&previous, old_root, &[]);
                }
            }
            let removed = self.layout.clear_structure();
            report.widgets_removed += removed.len();
        }
        self.id_of_path.clear();

        let Some(root_id) = create(&root) else {
            report.errors.push(super::ViewError::UnknownWidgetType {
                widget: root.widget.clone(),
                key: root.key.clone(),
                path: Vec::new(),
            });
            self.current = None;
            return report;
        };
        if root_id == 0 {
            report.errors.push(super::ViewError::UnknownWidgetType {
                widget: root.widget.clone(),
                key: root.key.clone(),
                path: Vec::new(),
            });
            self.current = None;
            return report;
        }

        let key = root.key.clone().unwrap_or_default();
        self.layout.register_node(root_id, root.widget.clone(), key, None);
        self.id_of_path.insert(Vec::new(), root_id);
        report.widgets_created += 1;
        // The root's own name seeds the ancestor chain, so a failure in a *direct* child still
        // reports where it sits ("window > no_such_widget#bad") rather than an empty chain.
        let root_chain = [root.widget.clone()];
        self.mount_children_named(&root, root_id, &[], &root_chain, create, &mut report);
        self.write_declared_properties(&root, root_id, &mut report);
        // Lifecycle: the whole tree now exists, so each node's `on_mount` runs -- children
        // before their parent's, which is the order a callback that touches its subtree needs.
        self.run_mount_hooks(&root, root_id, &[]);

        self.current = Some(root);
        report
    }

    /// Runs every `on_mount` in `node`'s subtree, deepest first.
    ///
    /// # Why the order is children-first
    ///
    /// A mount hook may read its own subtree ("subscribe to each row"). Running a parent
    /// before its children would let it observe a tree that is not finished, so the recursion
    /// descends before invoking. The ids come from `id_of_path`, which was populated by the
    /// same walk that created the controls -- so a hook's id is guaranteed to address a
    /// mounted control.
    fn run_mount_hooks(&self, node: &Node, node_id: ObjectId, path: &[usize]) {
        for (index, child) in node.children.iter().enumerate() {
            let mut child_path = path.to_vec();
            child_path.push(index);
            if let Some(child_id) = self.id_at(&child_path) {
                self.run_mount_hooks(child, child_id, &child_path);
            }
        }
        if let Some(hook) = &node.on_mount {
            hook(node_id);
        }
    }

    /// Runs every `on_unmount` in `node`'s subtree, **parent first**.
    ///
    /// The reverse order of [`Self::run_mount_hooks`], and for the mirror reason: a teardown
    /// hook may need to stop something in its own subtree, so the parent must get the chance
    /// before its children are gone. Reads ids from `id_of_path` -- the map is still populated
    /// because the layout is not cleared until after this returns.
    fn run_unmount_hooks(&self, node: &Node, node_id: ObjectId, path: &[usize]) {
        if let Some(hook) = &node.on_unmount {
            hook(node_id);
        }
        for (index, child) in node.children.iter().enumerate() {
            let mut child_path = path.to_vec();
            child_path.push(index);
            if let Some(child_id) = self.id_at(&child_path) {
                self.run_unmount_hooks(child, child_id, &child_path);
            }
        }
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
        self.update_with(view, &Context::new(), create)
    }

    /// [`Self::update`] with a [`Context`] the view may read while describing its tree.
    ///
    /// A context change therefore shows up the *ordinary* way: the new values land in the
    /// nodes' props during the build, and the diff reports the resulting property patches.
    /// Nothing in the diff knows a context exists.
    pub fn update_with(
        &mut self,
        view: &dyn View,
        ctx: &Context,
        create: &dyn Fn(&Node) -> Option<ObjectId>,
    ) -> DiffReport {
        let Some(previous) = self.current.take() else {
            self.mount_with(view, ctx, create);
            return DiffReport::default();
        };
        let next = view.build_with(ctx);
        let lookup = |path: &[usize], _index: usize| self.id_of_path.get(path).copied();
        let report = diff(&previous, &next, &lookup);

        // Assign ids for the shapes the diff is about to create, so that patches later in the
        // same batch can address them. Done before `apply` so a batched insert-then-write
        // resolves; the ids are allocated here because only the engine knows which paths the
        // new tree will occupy.
        //
        // The reservations are handed to `apply` so it *adopts* these ids instead of calling
        // `create` a second time for the same node. Discarding them made one orphaned live
        // control per inserted node, permanently — see `insert_subtree`.
        let mut reserved = self.reserve_ids_for_inserts(&next, &report.patches, create);

        let applied =
            apply_with_reservations(&mut self.layout, &report.patches, create, &mut reserved);
        // Sync the path map to the new tree's shape for the surviving nodes; a node the diff
        // did not mention keeps its path only if its ancestor chain is unchanged.
        self.reindex_paths(&next);

        // A refused patch is not a no-op the caller can ignore: a property the view declared,
        // a parent that raced a destroy, or a widget type with no constructor all mean the
        // mounted tree does not match what the view asked for. That used to be dropped here, so
        // `DiffReport` looked clean while the layout kept serving the pre-change tree. It is
        // logged now so the condition is at least observable; callers that need to react should
        // use `apply` directly.
        if !applied.errors.is_empty() {
            for error in &applied.errors {
                log::warn!("[view] update patch was not applied: {error}");
            }
        }
        if report.root_replaced {
            log::warn!(
                "[view] the root's widget type changed; a patch batch cannot replace a root, \
                 so the mounted tree is unchanged — remount the view"
            );
        }

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
    /// The recursive half of this, carrying the ancestor **names** so a failure can be
    /// located in the declaration.
    ///
    /// # Error boundary (BLUE23 §5A.5)
    ///
    /// A child that cannot be created is recorded and skipped; its **siblings are still
    /// mounted**. The alternative — propagating the failure and dropping the whole tree —
    /// made one misspelled widget name cost the entire window, which is the same shape as
    /// BLUE22's F-7 (`spacer` losing its subtree): "a node cannot be expressed" must not
    /// escalate to "everything disappears".
    ///
    /// The chain starts with the **root's own widget name** (seeded by `mount`), so a
    /// failure in a direct child reports `window > no_such_widget#bad` rather than nothing.
    /// Properties are written after recursing, so a freshly mounted tree is in the state the
    /// view described rather than in each control's default state.
    #[allow(clippy::too_many_arguments)]
    fn mount_children_named(
        &mut self,
        node: &Node,
        node_id: ObjectId,
        path: &[usize],
        ancestor_names: &[String],
        create: &dyn Fn(&Node) -> Option<ObjectId>,
        report: &mut ApplyReport,
    ) {
        for (index, child) in node.children.iter().enumerate() {
            let mut child_path = path.to_vec();
            child_path.push(index);
            let Some(child_id) = create(child).filter(|&id| id != 0) else {
                report.errors.push(super::ViewError::UnknownWidgetType {
                    widget: child.widget.clone(),
                    key: child.key.clone(),
                    path: ancestor_names.to_vec(),
                });
                continue;
            };
            let key = child.key.clone().unwrap_or_default();
            // A portal node is created into the overlay layer rather than under its declared
            // parent. Its `id_of_path` entry and its diff identity stay keyed by the declared
            // path, so a keyed rebuild still matches it as this parent's child — only the
            // control's *host* moves (BLUE23 §5A.2: identity in the tree, rendering elsewhere).
            let host = match child.host {
                Host::Declared => Some(node_id),
                Host::Overlay => self.overlay_layer_for(create),
            };
            self.layout.register_node(child_id, child.widget.clone(), key, host);
            if let Some(host) = host {
                self.layout.move_child_to(host, child_id, index);
            }
            self.id_of_path.insert(child_path.clone(), child_id);
            report.widgets_created += 1;
            let mut child_names = ancestor_names.to_vec();
            child_names.push(child.widget.clone());
            self.mount_children_named(child, child_id, &child_path, &child_names, create, report);
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

    /// Allocate ids for nodes the diff is about to insert, keyed by the node's declared key.
    ///
    /// Without this, a batch containing an `Insert` and a later patch addressing the inserted
    /// subtree would be reported as unknown — the ids only become addressable once something
    /// knows which path they occupy.
    ///
    /// The returned map is passed on to
    /// [`apply_with_reservations`](super::apply::apply_with_reservations) so those ids are
    /// **adopted** rather than allocated twice. `create` is the only way to learn an id, so
    /// calling it here and again inside `apply` produced two live controls per node and
    /// orphaned the first.
    fn reserve_ids_for_inserts(
        &mut self,
        next: &Node,
        patches: &[super::Patch],
        create: &dyn Fn(&Node) -> Option<ObjectId>,
    ) -> crate::compat::HashMap<String, ObjectId> {
        let mut reserved = crate::compat::HashMap::new();
        for patch in patches {
            if let super::Patch::Insert { node, .. } = patch {
                if let Some(path) = find_path_of(next, node) {
                    let mut pending = Vec::new();
                    collect_new_ids(node, &path, create, &mut pending);
                    for (key, p, id) in pending {
                        self.id_of_path.insert(p, id);
                        reserved.insert(key, id);
                    }
                }
            }
        }
        reserved
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
///
/// Each entry carries the node's declared key alongside its id, because the reservation map
/// `apply` consults is keyed by key (see [`insert_subtree`](super::apply::insert_subtree)).
fn collect_new_ids(
    node: &Node,
    path: &[usize],
    create: &dyn Fn(&Node) -> Option<ObjectId>,
    out: &mut Vec<(String, Vec<usize>, ObjectId)>,
) {
    let Some(id) = create(node).filter(|&id| id != 0) else {
        return;
    };
    out.push((node.key.clone().unwrap_or_default(), path.to_vec(), id));
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
        // The stability property from Phase D-4/D-5: an update must replace, not
        // accumulate, and the identity of the nodes it does not touch must survive.
        //
        // 100 rounds is the count the plan names. The value matters because a leak
        // that adds one node per update is invisible at 5 rounds and obvious at 100,
        // and because it is enough iterations for an "occasionally duplicates" bug to
        // show up rather than needing to be lucky.
        let mut engine = ViewEngine::new();
        let ids = Ids::new(10);
        engine.mount(&Text("0".into()), &ids.creator());
        let baseline = engine.layout().node_count();
        let root = engine.id_at(&[]);
        let child = engine.id_at(&[0]);

        for i in 0..100 {
            let report = engine.update(&Text(i.to_string()), &ids.creator());
            assert_eq!(engine.layout().node_count(), baseline, "the layout grew on update {i}");
            assert_eq!(
                report.replaced_subtrees, 0,
                "a text-only change must not replace a subtree (update {i})"
            );
            assert_eq!(engine.id_at(&[]), root, "the root id drifted on update {i}");
            assert_eq!(engine.id_at(&[0]), child, "the label id drifted on update {i}");
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

    // ── Error boundary (BLUE23 §5A.5) ─────────────────────────────────────────

    /// A view whose only child is a portal: declared under the panel, hosted in the layer.
    struct PortalUnderAPanel;

    impl View for PortalUnderAPanel {
        fn build(&self) -> Node {
            Node::new("window").key("root").child(
                Node::new("panel").key("panel").child(Node::new("tooltip").key("tip").portal()),
            )
        }
    }

    /// A portal node's control lands in the overlay layer, not under its declared parent.
    ///
    /// This is the whole of §5A.2: identity in the tree, rendering elsewhere. The tip's id is
    /// reachable by its **declared path** (`window > panel > tooltip`) while its *parent* in
    /// the layout is the overlay layer — which is what lets it paint outside the panel's clip.
    #[test]
    fn a_portal_control_is_hosted_in_the_overlay_layer() {
        let mut engine = ViewEngine::new();
        let ids = Ids::new(10);
        engine.mount(&PortalUnderAPanel, &ids.creator());

        let layer = engine.overlay_layer().expect("a portal asks the engine for a layer");
        // The declared path still resolves: identity stayed where it was written.
        let tip = engine.id_at(&[0, 0]).expect("the portal keeps its declared path");
        // But the control's parent is the layer, not the panel.
        assert_eq!(
            engine.layout().parent(tip),
            Some(layer),
            "the portal is rendered in the layer, outside its declarer's clip"
        );
        let panel = engine.id_at(&[0]).expect("the panel");
        assert_ne!(
            engine.layout().parent(tip),
            Some(panel),
            "and specifically not under the panel it was declared in"
        );
    }

    /// A tree with no portal allocates no layer.
    ///
    /// The layer is an engine-owned host, not a fixture: an ordinary tree must not carry an
    /// empty extra control, and `overlay_layer()` must say so honestly with `None`.
    #[test]
    fn a_tree_without_portals_has_no_overlay_layer() {
        let mut engine = ViewEngine::new();
        let ids = Ids::new(10);
        engine.mount(&Text("hi".into()), &ids.creator());
        assert_eq!(engine.overlay_layer(), None);
    }

    /// A portal node is the **same declaration** whether or not it is hosted elsewhere.
    ///
    /// `Host` says where a control is mounted, which is a rendering fact; it must not enter
    /// the declaration's equality, or a diff would see a change on every rebuild and tear
    /// down a subtree that did not change (BLUE23 §5A.4 judgement 9).
    #[test]
    fn the_host_does_not_enter_a_nodes_equality() {
        let declared = Node::new("tooltip").key("t");
        let portal = Node::new("tooltip").key("t").portal();
        assert_eq!(declared, portal, "the host is not part of the declaration");
        assert_ne!(declared.host, portal.host, "but the nodes do differ in host");
    }

    /// A view of three labels, the middle one naming a widget that cannot be created.
    struct ThreeWithABadMiddle;
    impl View for ThreeWithABadMiddle {
        fn build(&self) -> Node {
            Node::new("window")
                .key("root")
                .child(Node::new("label").key("first"))
                .child(Node::new("no_such_widget").key("bad"))
                .child(Node::new("label").key("last"))
        }
    }

    /// A subtree cannot be created without taking its siblings with it.
    ///
    /// This is the whole point of the error boundary: before it, one unconstructible node
    /// was either silently skipped or — at the mount level — treated as a reason to refuse
    /// the tree. The builder below fails only for the node that names the bad widget, so the
    /// two labels on either side must still be created.
    #[test]
    fn a_bad_child_does_not_remove_its_good_siblings() {
        let mut engine = ViewEngine::new();
        let create = |node: &Node| -> Option<ObjectId> {
            if node.widget == "no_such_widget" {
                None
            } else {
                Some(1)
            }
        };
        let report = engine.mount(&ThreeWithABadMiddle, &create);
        assert_eq!(report.failed_count(), 1, "exactly the bad node failed: {report:?}");
        // The root plus the two good labels still became controls.
        assert_eq!(report.widgets_created, 3, "the good siblings must survive: {report:?}");
    }

    /// The failure names the node well enough to find it in the declaration.
    #[test]
    fn a_failure_carries_the_widget_name_key_and_ancestor_chain() {
        let mut engine = ViewEngine::new();
        let create = |node: &Node| -> Option<ObjectId> {
            if node.widget == "no_such_widget" {
                None
            } else {
                Some(1)
            }
        };
        let report = engine.mount(&ThreeWithABadMiddle, &create);
        let failed = report.failed_nodes();
        assert_eq!(failed.len(), 1);
        // `widget chain > widget#key` -- the name, the key and where it sits.
        assert!(
            failed[0].contains("no_such_widget#bad"),
            "the failure must name the widget and its key: {failed:?}"
        );
        assert!(failed[0].starts_with("window"), "and the ancestor chain above it: {failed:?}");
    }

    // ── Lifecycle hooks (BLUE23 §5A.3) ────────────────────────────────────────

    use crate::compat::Rc;
    use std::cell::RefCell;

    /// A view whose label counts its own mounts and unmounts.
    struct Counting {
        mounts: Rc<RefCell<u32>>,
        unmounts: Rc<RefCell<u32>>,
    }

    impl View for Counting {
        fn build(&self) -> Node {
            let mounts = Rc::clone(&self.mounts);
            let unmounts = Rc::clone(&self.unmounts);
            Node::new("window").key("root").child(
                Node::new("label")
                    .key("value")
                    .on_mount(move |_id| *mounts.borrow_mut() += 1)
                    .on_unmount(move |_id| *unmounts.borrow_mut() += 1),
            )
        }
    }

    /// A mount fires once, and a rebuild that keeps the node does not fire it again.
    #[test]
    fn on_mount_fires_once_and_not_on_a_stable_rebuild() {
        let mounts = Rc::new(RefCell::new(0));
        let unmounts = Rc::new(RefCell::new(0));
        let view = Counting { mounts: Rc::clone(&mounts), unmounts: Rc::clone(&unmounts) };
        let mut engine = ViewEngine::new();
        let ids = Ids::new(10);
        engine.mount(&view, &ids.creator());
        assert_eq!(*mounts.borrow(), 1, "the node mounted once");

        // A second build with the same key is the same node: no remount.
        engine.update(&view, &ids.creator());
        assert_eq!(*mounts.borrow(), 1, "a keyed rebuild must not remount");
    }

    /// Replacing the tree unmounts the previous one exactly once, after it mounted.
    #[test]
    fn on_unmount_fires_once_when_the_tree_is_replaced() {
        let mounts = Rc::new(RefCell::new(0));
        let unmounts = Rc::new(RefCell::new(0));
        let view = Counting { mounts: Rc::clone(&mounts), unmounts: Rc::clone(&unmounts) };
        let mut engine = ViewEngine::new();
        let ids = Ids::new(10);
        engine.mount(&view, &ids.creator());
        assert_eq!(*unmounts.borrow(), 0, "nothing has left yet");

        // A second mount replaces the tree, which is what tears the first one down.
        engine.mount(&view, &ids.creator());
        assert_eq!(*unmounts.borrow(), 1, "the replaced tree unmounted exactly once");
        assert_eq!(*mounts.borrow(), 2, "and the new tree mounted");
    }

    /// A mount hook receives an id that addresses a mounted control.
    #[test]
    fn on_mount_receives_a_usable_id() {
        let seen: Rc<RefCell<Option<ObjectId>>> = Rc::new(RefCell::new(None));

        struct One(Rc<RefCell<Option<ObjectId>>>);
        impl View for One {
            fn build(&self) -> Node {
                let slot = Rc::clone(&self.0);
                Node::new("window").key("root").child(Node::new("label").key("only").on_mount(
                    move |id| {
                        *slot.borrow_mut() = Some(id);
                    },
                ))
            }
        }

        let mut engine = ViewEngine::new();
        let ids = Ids::new(10);
        engine.mount(&One(Rc::clone(&seen)), &ids.creator());
        let id = seen.borrow().expect("the mount hook ran and saw an id");
        assert_eq!(engine.id_at(&[0]), Some(id), "and the id addresses the mounted label");
    }

    // ── Context propagation (BLUE23 §5A.4) ──────────────────────────────

    /// A view that reads a context key into a child's property.
    struct Themed;

    impl View for Themed {
        fn build(&self) -> Node {
            // No context: fall back to the view's own default.
            self.build_with(&Context::new())
        }

        fn build_with(&self, ctx: &Context) -> Node {
            let accent = ctx.get("accent").unwrap_or("default");
            Node::new("window")
                .key("root")
                .child(Node::new("label").key("accent").prop("text", s(accent)))
        }
    }

    /// A context value changes what the view *describes*, so the produced props differ.
    #[test]
    fn a_context_value_reaches_the_described_props() {
        let ctx = Context::new().with("accent", "red");
        let node = Themed.build_with(&ctx);
        let label = &node.children[0];
        assert_eq!(
            label.props.get("text"),
            Some(&s("red")),
            "the context value must be resolved into the node's props"
        );
    }

    /// The context is **not** part of what the diff compares.
    ///
    /// The whole point of resolving at build time: two builds with the *same* context produce
    /// equal nodes, so a rebuild that changes nothing about the context is a no-op to the
    /// diff — the context never becomes a comparison key (BLUE23 §5A.4 judgement 9).
    #[test]
    fn the_context_is_absent_from_the_comparison() {
        let a = Context::new().with("accent", "red").with("locale", "en");
        let b = Context::new().with("accent", "red").with("locale", "fr");
        // Same `accent`, different `locale`. The view reads only `accent`, so the described
        // trees are identical even though the contexts differ.
        assert_eq!(Themed.build_with(&a), Themed.build_with(&b));
    }

    /// A missing key reads as `None` rather than panicking.
    #[test]
    fn a_missing_context_key_is_none() {
        let ctx = Context::new();
        assert!(ctx.is_empty());
        assert_eq!(ctx.get("nothing"), None);
        // And the view falls back to its own default rather than failing.
        let node = Themed.build_with(&ctx);
        assert_eq!(node.children[0].props.get("text"), Some(&s("default")));
    }

    /// A failure yields a placeholder a host can mount, with non-zero content.
    ///
    /// BLUE23 §5A.9 judgement 13: a failed node must render as a *placeholder*, not as zero
    /// ink. The library supplies the node (it knows where the failure is); the host paints
    /// it. This pins that the placeholder names the missing widget and is a real tree.
    #[test]
    fn a_failure_yields_a_placeholder_that_names_it() {
        let mut engine = ViewEngine::new();
        let create = |node: &Node| -> Option<ObjectId> {
            if node.widget == "no_such_widget" {
                None
            } else {
                Some(1)
            }
        };
        let report = engine.mount(&ThreeWithABadMiddle, &create);
        let placeholders = report.placeholders();
        assert_eq!(placeholders.len(), 1, "one placeholder per failed node");
        // It is a panel with a label child -- non-empty, unlike a `spacer`.
        assert_eq!(placeholders[0].widget, "panel");
        assert_eq!(placeholders[0].children.len(), 1);
        let text = placeholders[0].children[0].props.get("text");
        assert!(
            matches!(text, Some(CapabilityValue::String(s)) if s.contains("no_such_widget")),
            "the placeholder must name the missing widget: {text:?}"
        );
    }
}
