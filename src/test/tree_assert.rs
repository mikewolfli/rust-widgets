// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Tree-level assertions for generated whole trees (RW-57).
//!
//! # Why the existing kit is not enough
//!
//! [`TestHarness`] and [`WidgetTester`] test
//! **one control**: they own a widget, drive events into it and compare its own state. A designer
//! generates a whole tree, and the questions a test of that tree asks are different — "does the
//! node I keyed `save` exist?", "what text did the generator give it?", "where did the layout put
//! it?". Answering those by walking the tree in every test is the same code written N times and
//! quietly diverging, so it lives here once.
//!
//! # Two trees, two questions
//!
//! * [`TreeAssert`] reads the **declaration**: a `Node` tree, which is what a generated program
//!   builds before anything is mounted. Keys and properties are exactly what the generator wrote.
//! * [`MountedTreeAssert`] reads the **live** tree: it resolves a key to the control the engine
//!   created and reads that control's rectangle from the widget runtime. This is the half a
//!   snapshot or an interaction test needs, and the half a declaration tree cannot answer.
//!
//! # Key scoping, stated
//!
//! In the declarative model a key is scoped to its siblings, so two lists may each hold a row
//! keyed `total`. The search here is pre-order from the root and answers with the **first** match;
//! that is the whole document when keys are unique (which a generated tree always is, because a
//! key doubles as the design-time id), and it is a deliberate first-match rather than an error
//! when they are not. A caller that needs the scoped answer asks for a path.

use crate::compat::{format, String, Vec};
use crate::core::{ObjectId, Rect};
use crate::view::{Node, ViewEngine};
use crate::widget::capability::CapabilityValue;

/// Assertions over a declarative [`Node`] tree.
pub struct TreeAssert<'a> {
    root: &'a Node,
}

impl<'a> TreeAssert<'a> {
    /// Creates an assertion view over `root`.
    pub fn new(root: &'a Node) -> Self {
        Self { root }
    }

    /// The tree this view reads.
    pub fn root(&self) -> &'a Node {
        self.root
    }

    /// The first node (pre-order) whose key is `key`.
    pub fn find(&self, key: &str) -> Option<&'a Node> {
        find_node(self.root, key)
    }

    /// The tree path of the first node whose key is `key`.
    pub fn path_of(&self, key: &str) -> Option<Vec<usize>> {
        let mut path = Vec::new();
        find_path(self.root, key, &mut path)
    }

    /// A property value the node declares.
    pub fn prop(&self, key: &str, name: &str) -> Option<&'a CapabilityValue> {
        self.find(key)?.prop_value(name)
    }

    /// The `text` (or `title`) the node declares, if any.
    pub fn text(&self, key: &str) -> Option<String> {
        let node = self.find(key)?;
        node.prop_value("text")
            .or_else(|| node.prop_value("title"))
            .and_then(|value| value.as_str())
            .map(String::from)
    }

    /// The rectangle the node declares, when its properties state one.
    ///
    /// Two spellings are accepted because two front ends write them: a `geometry` property of kind
    /// [`Rect`](CapabilityValue::Rect) (what the property contract publishes), or the document
    /// shorthand `x`/`y`/`width`/`height` (what a JSON node may carry before the loader applies
    /// it). A node that declares neither — every node of the default generated template, whose
    /// coordinates are solved by the layout at run time — answers `None`; use
    /// [`MountedTreeAssert::geometry_of`] for the live rectangle.
    pub fn geometry(&self, key: &str) -> Option<Rect> {
        let node = self.find(key)?;
        if let Some(CapabilityValue::Rect(rect)) = node.prop_value("geometry") {
            return Some(*rect);
        }
        let axis = |name: &str| -> Option<i32> {
            match node.prop_value(name) {
                Some(CapabilityValue::Int(value)) => i32::try_from(*value).ok(),
                Some(CapabilityValue::UInt(value)) => i32::try_from(*value).ok(),
                Some(CapabilityValue::Float(value)) if value.is_finite() => {
                    Some(value.round().clamp(i32::MIN as f64, i32::MAX as f64) as i32)
                }
                _ => None,
            }
        };
        let x = axis("x")?;
        let y = axis("y")?;
        let width = u32::try_from(axis("width")?).ok()?;
        let height = u32::try_from(axis("height")?).ok()?;
        Some(Rect::new(x, y, width, height))
    }

    /// Every key the tree declares, in pre-order.
    pub fn keys(&self) -> Vec<String> {
        let mut keys = Vec::new();
        collect_keys(self.root, &mut keys);
        keys
    }

    /// Asserts that `key` names a node, and returns it.
    ///
    /// # Panics
    ///
    /// Panics with the keys the tree *does* declare, so a failure names the typo rather than
    /// requiring the reader to dump the tree.
    pub fn assert_exists(&self, key: &str) -> &'a Node {
        self.find(key)
            .unwrap_or_else(|| panic!("no node keyed {key:?} in the tree; keys: {:?}", self.keys()))
    }

    /// Asserts the node's text (or title).
    ///
    /// # Panics
    ///
    /// Panics when the node is missing or its text is not `expected`.
    pub fn assert_text(&self, key: &str, expected: &str) {
        self.assert_exists(key);
        let actual = self.text(key);
        assert_eq!(
            actual.as_deref(),
            Some(expected),
            "node {key:?} declared text {actual:?}, expected {expected:?}"
        );
    }

    /// Asserts a node's declared rectangle.
    ///
    /// # Panics
    ///
    /// Panics when the node is missing or its declared rectangle is not `expected`.
    pub fn assert_geometry(&self, key: &str, expected: Rect) {
        self.assert_exists(key);
        let actual = self.geometry(key);
        assert_eq!(
            actual,
            Some(expected),
            "node {key:?} declared geometry {actual:?}, expected {expected:?}"
        );
    }

    /// Asserts that no node declares `key`.
    ///
    /// Used for the negative half of an edit: a removed control must not survive as a keyed node.
    pub fn assert_absent(&self, key: &str) {
        assert!(self.find(key).is_none(), "node {key:?} must not exist; keys: {:?}", self.keys());
    }
}

/// Assertions over the tree a [`ViewEngine`] currently has mounted.
pub struct MountedTreeAssert<'a> {
    engine: &'a ViewEngine,
}

impl<'a> MountedTreeAssert<'a> {
    /// Creates an assertion view over `engine`'s mounted tree.
    pub fn new(engine: &'a ViewEngine) -> Self {
        Self { engine }
    }

    /// The engine being inspected.
    pub fn engine(&self) -> &'a ViewEngine {
        self.engine
    }

    /// The declaration tree, when something is mounted.
    pub fn tree(&self) -> Option<TreeAssert<'a>> {
        self.engine.current().map(TreeAssert::new)
    }

    /// The path of the first node keyed `key`.
    pub fn path_of(&self, key: &str) -> Option<Vec<usize>> {
        self.tree()?.path_of(key)
    }

    /// The control id the engine assigned to the node keyed `key`.
    pub fn id_of(&self, key: &str) -> Option<ObjectId> {
        self.engine.id_at(&self.path_of(key)?)
    }

    /// The live control's rectangle, read from the widget runtime.
    ///
    /// This is the **placed** rectangle, not a declaration: it reflects the layout pass that ran,
    /// which is the question a tree-level test usually has. `None` means the node is unmounted or
    /// its control was never registered on this thread.
    pub fn geometry_of(&self, key: &str) -> Option<Rect> {
        crate::widget::runtime::geometry_of(self.id_of(key)?)
    }

    /// The text the node declared (the declaration is the source of truth for text, because not
    /// every control publishes a generic text getter).
    pub fn text_of(&self, key: &str) -> Option<String> {
        self.tree()?.text(key)
    }

    /// Asserts that `key` names a mounted node, and returns its control id.
    ///
    /// # Panics
    ///
    /// Panics with the mounted keys when the key is missing.
    pub fn assert_exists(&self, key: &str) -> ObjectId {
        self.id_of(key).unwrap_or_else(|| {
            let keys = self.tree().map(|tree| tree.keys()).unwrap_or_default();
            panic!("no mounted node keyed {key:?}; mounted keys: {keys:?}")
        })
    }

    /// Asserts a node's placed rectangle.
    ///
    /// # Panics
    ///
    /// Panics when the node is missing or its live rectangle is not `expected`.
    pub fn assert_geometry(&self, key: &str, expected: Rect) {
        self.assert_exists(key);
        let actual = self.geometry_of(key);
        assert_eq!(
            actual,
            Some(expected),
            "mounted node {key:?} resolved to {actual:?}, expected {expected:?}"
        );
    }

    /// A readable one-line description of the mounted tree, for a failing assertion.
    pub fn describe(&self) -> String {
        match self.tree() {
            Some(tree) => format!("mounted tree keys: {:?}", tree.keys()),
            None => String::from("nothing is mounted"),
        }
    }
}

/// The first node in `node`'s subtree with `key` (pre-order).
fn find_node<'a>(node: &'a Node, key: &str) -> Option<&'a Node> {
    if node.key.as_deref() == Some(key) {
        return Some(node);
    }
    node.children.iter().find_map(|child| find_node(child, key))
}

/// The path of the first node in `node`'s subtree with `key` (pre-order).
fn find_path(node: &Node, key: &str, path: &mut Vec<usize>) -> Option<Vec<usize>> {
    if node.key.as_deref() == Some(key) {
        return Some(path.clone());
    }
    for (index, child) in node.children.iter().enumerate() {
        path.push(index);
        if let Some(found) = find_path(child, key, path) {
            return Some(found);
        }
        path.pop();
    }
    None
}

/// Appends every declared key in pre-order.
fn collect_keys(node: &Node, keys: &mut Vec<String>) {
    if let Some(key) = &node.key {
        keys.push(key.clone());
    }
    for child in &node.children {
        collect_keys(child, keys);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::vec;

    fn sample_tree() -> Node {
        Node::new("window")
            .key("root")
            .prop("text", CapabilityValue::String(String::from("Window")))
            .child(
                Node::new("panel")
                    .key("card")
                    .prop("x", CapabilityValue::Int(12))
                    .prop("y", CapabilityValue::Int(18))
                    .prop("width", CapabilityValue::UInt(200))
                    .prop("height", CapabilityValue::UInt(120))
                    .child(
                        Node::new("button")
                            .key("save")
                            .prop("text", CapabilityValue::String(String::from("Save"))),
                    ),
            )
    }

    /// RW-57: a key finds the node, and a missing key names the keys that do exist.
    #[test]
    fn a_key_finds_the_node_and_its_path() {
        let tree = sample_tree();
        let tree_assert = TreeAssert::new(&tree);
        assert_eq!(
            tree_assert.keys(),
            vec![String::from("root"), String::from("card"), String::from("save")]
        );
        assert_eq!(tree_assert.path_of("save"), Some(vec![0, 0]));
        assert_eq!(tree_assert.path_of("missing"), None);
        assert_eq!(tree_assert.assert_exists("save").widget, "button");
        tree_assert.assert_absent("missing");
    }

    /// RW-57: text is read from `text`, falling back to `title`, exactly as the generator writes it.
    #[test]
    fn text_is_read_from_the_declaration() {
        let tree = sample_tree();
        let tree_assert = TreeAssert::new(&tree);
        assert_eq!(tree_assert.text("save").as_deref(), Some("Save"));
        assert_eq!(tree_assert.text("card"), None);
        tree_assert.assert_text("root", "Window");

        let titled = Node::new("group_box")
            .key("box")
            .prop("title", CapabilityValue::String(String::from("Group")));
        assert_eq!(TreeAssert::new(&titled).text("box"), Some(String::from("Group")));
    }

    /// RW-57: a declared rectangle is read from either spelling the front ends use.
    #[test]
    fn geometry_is_read_from_the_declaration() {
        let tree = sample_tree();
        let tree_assert = TreeAssert::new(&tree);
        tree_assert.assert_geometry("card", Rect::new(12, 18, 200, 120));
        assert_eq!(tree_assert.geometry("save"), None);

        let contract = Node::new("frame")
            .key("frame")
            .prop("geometry", CapabilityValue::Rect(Rect::new(1, 2, 3, 4)));
        assert_eq!(TreeAssert::new(&contract).geometry("frame"), Some(Rect::new(1, 2, 3, 4)));
    }

    /// RW-57: the mounted half resolves the key to the live control id.
    #[test]
    fn a_mounted_tree_resolves_keys_to_control_ids() {
        let tree = sample_tree();
        let mut engine = ViewEngine::new();
        engine.mount_tree(&tree, &|node: &Node| match node.key.as_deref() {
            Some("root") => Some(1),
            Some("card") => Some(2),
            Some("save") => Some(3),
            _ => None,
        });

        let tree_assert = MountedTreeAssert::new(&engine);
        assert_eq!(tree_assert.id_of("save"), Some(3));
        assert_eq!(tree_assert.text_of("save"), Some(String::from("Save")));
        // No live control was registered for the stub id, so no rectangle is claimed.
        assert_eq!(tree_assert.geometry_of("save"), None);
        assert_eq!(tree_assert.assert_exists("card"), 2);
        assert!(tree_assert.describe().contains("save"));
    }
}
