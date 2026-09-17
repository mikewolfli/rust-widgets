// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! JSON-node → widget instantiation and typed handle access.
//!
//! This module provides [`BoundJsonLayout`], the binding between a JSON
//! declarative layout (defined via BLUE4.md spec) and the live widget tree.
//!
//! After [`JsonLoader::load`] parses a JSON string and instantiates all
//! widgets, it returns a `BoundJsonLayout`. Callers retrieve typed widget
//! handles by the JSON `"id"` attribute, e.g.:
//!
//! ```text
//! let layout = JsonLoader::load(json_str)?;
//! let btn = layout.widget_by_name::<ButtonHandle>("ok_btn")?;
//! btn.set_text("Confirm");
//! ```
//!
//! # Two views of the same binding
//!
//! A `BoundJsonLayout` answers two different questions, and both are needed by the
//! declarative-retained pipeline ([`crate::view`]):
//!
//! 1. **"Which widget did the JSON id `ok_btn` become?"** — [`id`](BoundJsonLayout::id) and
//!    [`widget_by_name`](BoundJsonLayout::widget_by_name), backed by `name_map`.
//! 2. **"What structure did the JSON describe?"** — [`parent`](BoundJsonLayout::parent),
//!    [`children`](BoundJsonLayout::children), [`sibling_index`](BoundJsonLayout::sibling_index),
//!    backed by the tree indexes (`root` / `parent_of` / `children_of` / `kind_of` / `key_of`).
//!
//! Question 2 is what makes a `state → Node → diff → patch` pipeline possible: a diff
//! can only decide which nodes are the *same* control as before if it can walk the
//! shape of the previous tree. Answering it by re-parsing the JSON string is not
//! equivalent — the string says what was *asked for*, while these indexes say what was
//! actually *created* (a `"spacer"` produces no widget, an unnamed child gets an
//! auto-generated label, a layout node collapses into its parent).
//!
//! The tree indexes are **additive**: every pre-existing accessor keeps its exact
//! behaviour, and a layout built by [`register`](BoundJsonLayout::register)
//! alone still works — it simply has no structure to report, which the queries express
//! as `None`/empty rather than as a panic.

use crate::compat::HashMap;

use core::fmt;

use crate::app::{
    ButtonHandle, CheckBoxHandle, ComboBoxHandle, FrameHandle, GridWidgetHandle, LabelHandle,
    LineEditHandle, ListBoxHandle, ListViewHandle, PanelHandle, ProgressBarHandle,
    RadioButtonHandle, ScrollAreaHandle, ScrollBarHandle, SliderHandle, SpinBoxHandle,
    TabWidgetHandle, TextEditHandle, WidgetHandle, WindowHandle,
};
use crate::core::ObjectId;

/// A named widget layout: JSON `"id"` attributes → `ObjectId`, plus the tree
/// structure that was actually created.
///
/// Created by `JsonLoader::load` after instantiating a JSON layout.
/// Provides typed widget access via [`widget_by_name`](BoundJsonLayout::widget_by_name)
/// and structural queries via [`parent`](Self::parent) / [`children`](Self::children).
pub struct BoundJsonLayout {
    name_map: HashMap<String, ObjectId>,
    root: Option<ObjectId>,
    parent_of: HashMap<ObjectId, ObjectId>,
    children_of: HashMap<ObjectId, Vec<ObjectId>>,
    kind_of: HashMap<ObjectId, String>,
    key_of: HashMap<ObjectId, String>,
}

impl fmt::Debug for BoundJsonLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BoundJsonLayout")
            .field("len", &self.len())
            .field("has_root", &self.root.is_some())
            .field("nodes", &self.parent_of.len())
            .finish()
    }
}

impl BoundJsonLayout {
    /// Create a new empty layout binding.
    pub fn new() -> Self {
        Self {
            name_map: HashMap::new(),
            root: None,
            parent_of: HashMap::new(),
            children_of: HashMap::new(),
            kind_of: HashMap::new(),
            key_of: HashMap::new(),
        }
    }

    /// Register a name-to-id mapping (called during instantiation).
    pub fn register(&mut self, name: impl Into<String>, id: ObjectId) {
        self.name_map.insert(name.into(), id);
    }

    // ── Tree structure ─────────────────────────────────────

    /// Record that `id` is a node of `widget_type`, carrying `key`, under `parent`.
    ///
    /// Called by [`JsonLoader`](crate::json::JsonLoader) as each node is created.
    /// `parent` is `None` for the root; a node whose parent is later removed is
    /// detached by [`detach`](Self::detach) rather than left dangling.
    ///
    /// Re-registering the same `id` **replaces** its structural entry, matching
    /// [`register`](Self::register)'s overwrite semantics for names — an id identifies
    /// one node, so two entries for it would make every query ambiguous.
    pub fn register_node(
        &mut self,
        id: ObjectId,
        widget_type: impl Into<String>,
        key: impl Into<String>,
        parent: Option<ObjectId>,
    ) {
        if id == 0 {
            return;
        }
        if let Some(old_parent) = self.parent_of.remove(&id) {
            self.unlink_child(old_parent, id);
        }
        self.kind_of.insert(id, widget_type.into());
        let key = key.into();
        if key.is_empty() {
            self.key_of.remove(&id);
        } else {
            self.key_of.insert(id, key);
        }
        self.children_of.entry(id).or_default();
        match parent {
            Some(p) if p != 0 => {
                self.parent_of.insert(id, p);
                let siblings = self.children_of.entry(p).or_default();
                if !siblings.contains(&id) {
                    siblings.push(id);
                }
            }
            _ => {
                if self.root.is_none() {
                    self.root = Some(id);
                }
            }
        }
    }

    /// Remove the named entry for `id` from the name map.
    fn unregister_name(&mut self, id: ObjectId) {
        self.name_map.retain(|_, &mut v| v != id);
    }

    /// Remove `id` from `parent`'s child list, if present.
    fn unlink_child(&mut self, parent: ObjectId, id: ObjectId) {
        if let Some(siblings) = self.children_of.get_mut(&parent) {
            siblings.retain(|&x| x != id);
        }
    }

    /// Remove a node's structural entry. Low-level; callers usually want [`detach`](Self::detach).
    fn forget_node(&mut self, id: ObjectId) {
        if let Some(parent) = self.parent_of.remove(&id) {
            self.unlink_child(parent, id);
        }
        self.children_of.remove(&id);
        self.kind_of.remove(&id);
        self.key_of.remove(&id);
        if self.root == Some(id) {
            self.root = None;
        }
        self.unregister_name(id);
    }

    /// Move `child` to position `index` within `parent`'s child list, keeping its id.
    ///
    /// An `index` past the end appends, and a child that is not currently under `parent`
    /// is attached by this call — both are what an `Insert`/`Move` patch means when the
    /// diff's notion of position and the tree's have not been reconciled yet. Neither
    /// case panics, because a view that is one edit behind should still render.
    ///
    /// Only the moved child takes a new position: everything else keeps its relative
    /// order, which is what makes a reorder a reorder rather than a renumbering.
    pub fn move_child_to(&mut self, parent: ObjectId, child: ObjectId, index: usize) {
        if child == 0 || child == parent {
            return;
        }
        let siblings = self.children_of.entry(parent).or_default();
        siblings.retain(|&x| x != child);
        let at = index.min(siblings.len());
        siblings.insert(at, child);
        self.parent_of.insert(child, parent);
        self.children_of.entry(child).or_default();
    }

    /// Detach `id` and its entire subtree from the structural indexes.
    ///
    /// Returns the ids that were removed. Every index is cleaned: the node is unlinked from
    /// its parent, its descendants are removed recursively, and its name registration is
    /// dropped.
    ///
    /// Deleting a subtree without this cleanup would leave `children(id)` naming controls
    /// that no longer exist — which is exactly the state a diff cannot recover from, because
    /// it would then believe those ids are still live.
    ///
    /// A subtree is collected **before** anything is forgotten: `forget_node` drops a node's
    /// child list, so removing while walking would stop at the first level.
    pub fn detach(&mut self, id: ObjectId) -> Vec<ObjectId> {
        let mut removed: Vec<ObjectId> = Vec::new();
        let mut stack = vec![id];
        while let Some(current) = stack.pop() {
            if !removed.contains(&current) {
                removed.push(current);
            }
            if let Some(kids) = self.children_of.get(&current) {
                stack.extend(kids.iter().copied());
            }
        }
        removed.retain(|node| self.kind_of.contains_key(node) || self.root == Some(*node));
        for node in &removed {
            self.forget_node(*node);
        }
        removed
    }

    /// Reset the structure to an empty layout, keeping the name map.
    ///
    /// The counterpart to [`detach_all_except`](Self::detach_all_except) for a caller that
    /// is replacing the whole tree and has no node to preserve: a "keep the root" reset is
    /// wrong there, because the old root is precisely what is being replaced, and leaving
    /// it indexed would make the structure count a control no longer in any tree.
    ///
    /// The `name_map` survives on purpose: a caller may be holding handles resolved from
    /// earlier names, and those handles address `ObjectId`s rather than this map.
    pub fn clear_structure(&mut self) -> Vec<ObjectId> {
        let mut removed: Vec<ObjectId> = self.kind_of.keys().copied().collect();
        for root in self.root.iter() {
            if !removed.contains(root) {
                removed.push(*root);
            }
        }
        removed.sort_unstable();
        self.root = None;
        self.parent_of.clear();
        self.children_of.clear();
        self.kind_of.clear();
        self.key_of.clear();
        removed
    }

    /// Detach every node except `keep`, i.e. reset the structure to a single root.
    ///
    /// Returns the ids that were removed. Used when a view engine replaces a tree
    /// wholesale but wants to keep the binding object itself — the alternative (dropping
    /// the binding) would also lose the `name_map` entries the caller may already hold
    /// handles against.
    pub fn detach_all_except(&mut self, keep: ObjectId) -> Vec<ObjectId> {
        let mut removed: Vec<ObjectId> =
            self.kind_of.keys().copied().filter(|&k| k != keep).collect();
        for node in &removed {
            self.forget_node(*node);
        }
        // `forget_node` unlinks each removed id from its parent's list; `keep` was not
        // removed, so its own list still holds the now-forgotten ids and must be reset.
        self.children_of.insert(keep, Vec::new());
        removed.sort_unstable();
        removed
    }

    /// The root node of the instantiated tree, if the binding has structure.
    pub fn root(&self) -> Option<ObjectId> {
        self.root
    }

    /// The parent of `id`. `None` for the root and for unknown ids.
    pub fn parent(&self, id: ObjectId) -> Option<ObjectId> {
        self.parent_of.get(&id).copied()
    }

    /// The children of `id`, in the order the JSON declared them.
    ///
    /// Order is load-bearing: it is what a diff compares positionally when a sibling
    /// carries no key, so this must not be sorted or deduplicated.
    pub fn children(&self, id: ObjectId) -> &[ObjectId] {
        self.children_of.get(&id).map_or(&[], Vec::as_slice)
    }

    /// The position of `id` among its parent's children, or `None` for the root
    /// (which has no siblings) and for unknown ids.
    pub fn sibling_index(&self, id: ObjectId) -> Option<usize> {
        let parent = self.parent(id)?;
        self.children(parent).iter().position(|&x| x == id)
    }

    /// The widget type name recorded for `id` (the JSON key that produced it).
    pub fn widget_name(&self, id: ObjectId) -> Option<&str> {
        self.kind_of.get(&id).map(String::as_str)
    }

    /// The `key` recorded for `id`, if the JSON declared one.
    ///
    /// Absence is meaningful rather than an error: a node with no key can only be
    /// matched positionally by a diff, which is a degradation the diff is required to
    /// *report* rather than paper over.
    pub fn node_key(&self, id: ObjectId) -> Option<&str> {
        self.key_of.get(&id).map(String::as_str)
    }

    /// The `ObjectId` whose recorded `key` is `key` within `parent`'s children.
    ///
    /// Exists because a key is scoped to its siblings: two lists may each hold a row
    /// keyed `"total"` without collision.
    pub fn child_by_key(&self, parent: Option<ObjectId>, key: &str) -> Option<ObjectId> {
        let candidates: &[ObjectId] = match parent {
            Some(p) => self.children(p),
            None => self.root.as_slice(),
        };
        candidates.iter().copied().find(|&id| self.node_key(id) == Some(key))
    }

    /// Number of nodes with structure recorded (not the same as [`len`](Self::len),
    /// which counts JSON ids).
    pub fn node_count(&self) -> usize {
        self.kind_of.len()
    }

    /// Depth of `id` below the root, or `None` for unknown ids.
    ///
    /// The root is depth 0. The walk counts steps and stops rather than trusting the
    /// indexes to be acyclic, so a hand-built index cannot hang this call.
    pub fn depth_of(&self, id: ObjectId) -> Option<usize> {
        if !self.kind_of.contains_key(&id) && self.root != Some(id) {
            return None;
        }
        let mut depth = 0usize;
        let mut current = id;
        while let Some(parent) = self.parent(current) {
            depth += 1;
            current = parent;
            if depth > 4096 {
                return Some(depth);
            }
        }
        Some(depth)
    }

    /// Walk the tree depth-first from the root, yielding `(id, depth, sibling_index)`.
    ///
    /// A single traversal point so diagnostics cannot disagree with themselves about
    /// what the tree contains.
    pub fn walk(&self) -> Vec<(ObjectId, usize, usize)> {
        let mut out = Vec::new();
        if let Some(root) = self.root {
            self.walk_into(root, 0, 0, &mut out);
        }
        out
    }

    /// Depth-first helper for [`walk`](Self::walk).
    fn walk_into(
        &self,
        id: ObjectId,
        depth: usize,
        index: usize,
        out: &mut Vec<(ObjectId, usize, usize)>,
    ) {
        out.push((id, depth, index));
        for (i, child) in self.children(id).iter().enumerate() {
            self.walk_into(*child, depth + 1, i, out);
        }
    }

    // ── Name-based access ─────────────────────────────────

    /// Look up an [`ObjectId`] by JSON `"id"` attribute.
    pub fn id(&self, name: &str) -> Option<ObjectId> {
        self.name_map.get(name).copied()
    }

    /// Iterates the registered JSON `"id"` names.
    ///
    /// Exists so a caller that cannot resolve a name can report what *is*
    /// available (e.g. [`widget_by_name`](Self::widget_by_name)'s error), instead
    /// of leaving the reader to guess.
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.name_map.keys().map(String::as_str)
    }

    /// Number of registered widgets.
    pub fn len(&self) -> usize {
        self.name_map.len()
    }

    /// Whether no widgets are registered.
    pub fn is_empty(&self) -> bool {
        self.name_map.is_empty()
    }

    // ── Typed widget access ────────────────────────────────

    /// Retrieve a typed handle for a widget by its JSON `id`.
    ///
    /// Returns `Err` if the widget name is not found.
    ///
    /// ```text
    /// let btn = layout.widget_by_name::<ButtonHandle>("ok_btn")?;
    /// btn.set_text("Confirm");
    /// ```
    pub fn widget_by_name<T: WidgetHandle>(&self, name: &str) -> Result<T, String> {
        let raw_id = self.id(name).ok_or_else(|| {
            format!(
                "layout has no widget named '{name}'; available ids are {:?}",
                self.ids().collect::<Vec<_>>()
            )
        })?;
        Ok(T::from_raw(raw_id))
    }

    /// Convenience: get a button handle by JSON id.
    pub fn button(&self, name: &str) -> Result<ButtonHandle, String> {
        self.widget_by_name(name)
    }

    /// Convenience: get a label handle by JSON id.
    pub fn label(&self, name: &str) -> Result<LabelHandle, String> {
        self.widget_by_name(name)
    }

    /// Convenience: get a line-edit handle by JSON id.
    pub fn line_edit(&self, name: &str) -> Result<LineEditHandle, String> {
        self.widget_by_name(name)
    }

    /// Convenience: get a checkbox handle by JSON id.
    pub fn checkbox(&self, name: &str) -> Result<CheckBoxHandle, String> {
        self.widget_by_name(name)
    }

    /// Convenience: get a radio button handle by JSON id.
    pub fn radio_button(&self, name: &str) -> Result<RadioButtonHandle, String> {
        self.widget_by_name(name)
    }

    /// Convenience: get a combo box handle by JSON id.
    pub fn combo_box(&self, name: &str) -> Result<ComboBoxHandle, String> {
        self.widget_by_name(name)
    }

    /// Convenience: get a list box handle by JSON id.
    pub fn list_box(&self, name: &str) -> Result<ListBoxHandle, String> {
        self.widget_by_name(name)
    }

    /// Convenience: get a slider handle by JSON id.
    pub fn slider(&self, name: &str) -> Result<SliderHandle, String> {
        self.widget_by_name(name)
    }

    /// Convenience: get a progress bar handle by JSON id.
    pub fn progress_bar(&self, name: &str) -> Result<ProgressBarHandle, String> {
        self.widget_by_name(name)
    }

    /// Convenience: get a panel handle by JSON id.
    pub fn panel(&self, name: &str) -> Result<PanelHandle, String> {
        self.widget_by_name(name)
    }

    /// Convenience: get a spin box handle by JSON id.
    pub fn spin_box(&self, name: &str) -> Result<SpinBoxHandle, String> {
        self.widget_by_name(name)
    }

    /// Convenience: get a list view handle by JSON id.
    pub fn list_view(&self, name: &str) -> Result<ListViewHandle, String> {
        self.widget_by_name(name)
    }

    /// Convenience: get a scroll area handle by JSON id.
    pub fn scroll_area(&self, name: &str) -> Result<ScrollAreaHandle, String> {
        self.widget_by_name(name)
    }

    /// Convenience: get a text edit handle by JSON id.
    pub fn text_edit(&self, name: &str) -> Result<TextEditHandle, String> {
        self.widget_by_name(name)
    }

    /// Convenience: get a scroll bar handle by JSON id.
    pub fn scroll_bar(&self, name: &str) -> Result<ScrollBarHandle, String> {
        self.widget_by_name(name)
    }

    /// Convenience: get a tab widget handle by JSON id.
    pub fn tab_widget(&self, name: &str) -> Result<TabWidgetHandle, String> {
        self.widget_by_name(name)
    }

    /// Convenience: get a grid widget handle by JSON id.
    pub fn grid_widget(&self, name: &str) -> Result<GridWidgetHandle, String> {
        self.widget_by_name(name)
    }

    /// Convenience: get a frame handle by JSON id.
    pub fn frame(&self, name: &str) -> Result<FrameHandle, String> {
        self.widget_by_name(name)
    }

    /// Convenience: get a window handle by JSON id.
    pub fn window(&self, name: &str) -> Result<WindowHandle, String> {
        self.widget_by_name(name)
    }
}

crate::impl_default_via_new!(BoundJsonLayout);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_empty_layout() {
        let layout = BoundJsonLayout::new();
        assert_eq!(layout.len(), 0);
        assert!(layout.is_empty());
    }

    #[test]
    fn register_and_retrieve_id() {
        let mut layout = BoundJsonLayout::new();
        let id = 42;
        layout.register("my_widget", id);
        assert_eq!(layout.len(), 1);
        assert!(!layout.is_empty());
        assert_eq!(layout.id("my_widget"), Some(id));
    }

    #[test]
    fn id_returns_none_for_unknown() {
        let layout = BoundJsonLayout::new();
        assert_eq!(layout.id("nonexistent"), None);
    }

    #[test]
    fn register_multiple_widgets() {
        let mut layout = BoundJsonLayout::new();
        layout.register("btn1", 1);
        layout.register("btn2", 2);
        layout.register("label1", 3);
        assert_eq!(layout.len(), 3);
    }

    #[test]
    fn duplicate_name_overwrites() {
        let mut layout = BoundJsonLayout::new();
        let id1 = 10;
        let id2 = 20;
        layout.register("dup", id1);
        layout.register("dup", id2);
        assert_eq!(layout.len(), 1);
        assert_eq!(layout.id("dup"), Some(id2));
    }

    #[test]
    fn widget_by_name_not_found_error() {
        let layout = BoundJsonLayout::new();
        let result = layout.widget_by_name::<LabelHandle>("missing");
        assert!(result.is_err());
        let err = result.unwrap_err();
        // The message must name the missing id (otherwise the reader cannot tell a typo
        // from a widget that was never registered) and list what does exist.
        assert!(err.contains("missing"), "{err}");
        assert!(err.contains("available ids"), "{err}");
    }

    #[test]
    fn convenience_handle_methods() {
        let mut layout = BoundJsonLayout::new();
        let id = 1;
        layout.register("my_btn", id);
        layout.register("my_label", id);
        layout.register("my_edit", id);
        // These should succeed since the widget exists in the name map
        assert!(layout.button("my_btn").is_ok());
        assert!(layout.label("my_label").is_ok());
        assert!(layout.line_edit("my_edit").is_ok());
    }

    #[test]
    fn convenience_handles_return_err_for_missing() {
        let layout = BoundJsonLayout::new();
        assert!(layout.button("no_such").is_err());
        assert!(layout.checkbox("no_such").is_err());
        assert!(layout.combo_box("no_such").is_err());
        assert!(layout.slider("no_such").is_err());
        assert!(layout.progress_bar("no_such").is_err());
        assert!(layout.panel("no_such").is_err());
        assert!(layout.spin_box("no_such").is_err());
        assert!(layout.scroll_area("no_such").is_err());
        assert!(layout.tab_widget("no_such").is_err());
        assert!(layout.grid_widget("no_such").is_err());
        assert!(layout.frame("no_such").is_err());
        assert!(layout.window("no_such").is_err());
    }

    #[test]
    fn default_is_empty() {
        let layout = BoundJsonLayout::default();
        assert!(layout.is_empty());
        assert_eq!(layout.len(), 0);
    }

    #[test]
    fn register_string_and_str() {
        let mut layout = BoundJsonLayout::new();
        let id = 99;
        layout.register("from_str".to_string(), id);
        layout.register("from_ref", id);
        assert_eq!(layout.id("from_str"), Some(id));
        assert_eq!(layout.id("from_ref"), Some(id));
    }
}
