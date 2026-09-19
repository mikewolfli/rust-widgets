// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Declarative description of a widget tree — the "what" that a [`View`] builds.
//!
//! A [`Node`] is a **plain value**: a widget type name, an optional `key`, a list of
//! properties, and a list of children. It owns no `ObjectId`, holds no live control, and
//! touches no platform API. That is deliberate — it is what lets [`diff`](crate::view::diff)
//! be a pure function over two values, and therefore testable without a window.
//!
//! [`View`]: crate::view::View
//! [`diff`]: crate::view::diff

use crate::compat::HashMap;

use crate::widget::capability::CapabilityValue;

/// A single node in a declarative tree.
///
/// Build one with [`Node::new`] and the chainable accessors:
///
/// ```
/// use rust_widgets::view::Node;
/// use rust_widgets::widget::capability::CapabilityValue;
///
/// let node = Node::new("button")
///     .key("ok")
///     .prop("text", CapabilityValue::String("OK".into()))
///     .child(Node::new("icon").key("ok_icon"));
/// assert_eq!(node.widget, "button");
/// assert_eq!(node.children.len(), 1);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// The widget type name, resolved through the same factory table the JSON loader
    /// uses. Not a [`WidgetKind`](crate::widget::WidgetKind): a kind is a *classification*
    /// of an already-created control, while this is the name used to *create* one, and
    /// several names may resolve to one kind (BLUE18 rule #49).
    pub widget: String,
    /// Stable identity across rebuilds.
    ///
    /// `None` means "match me by position", which is what BLUE18 rule #87 warns about:
    /// an insert then shifts every later sibling's identity. [`diff`](crate::view::diff)
    /// reports how many nodes were matched positionally so the degradation is visible
    /// rather than silent.
    pub key: Option<String>,
    /// Properties to apply, by the names each control's capability publishes.
    ///
    /// A [`HashMap`] rather than an ordered list: the only operations are "what is the
    /// value of `x`" and "enumerate the differences", and a map makes the first O(1) so
    /// the diff stays linear in the number of properties rather than quadratic.
    pub props: HashMap<String, CapabilityValue>,
    /// Children, **in declaration order**.
    ///
    /// Order is load-bearing: it is the last resort a diff uses to match siblings, and
    /// it is what determines where an [`Insert`](crate::view::Patch::Insert) lands.
    pub children: Vec<Node>,
}

impl Node {
    /// Create a leaf node of the given widget type.
    pub fn new(widget: impl Into<String>) -> Self {
        Self { widget: widget.into(), key: None, props: HashMap::new(), children: Vec::new() }
    }

    /// Set the stable key used to match this node across rebuilds.
    ///
    /// Keys must be unique among siblings — see
    /// [`Node::duplicate_sibling_keys`], which is what the key-uniqueness gate reports.
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }

    /// Set one property. Chainable.
    ///
    /// Setting the same name twice keeps the last write, matching the JSON loader's
    /// "later key wins" behaviour so the two front ends cannot disagree.
    pub fn prop(mut self, name: impl Into<String>, value: CapabilityValue) -> Self {
        self.props.insert(name.into(), value);
        self
    }

    /// Append one child. Chainable.
    pub fn child(mut self, child: Node) -> Self {
        self.children.push(child);
        self
    }

    /// Append several children. Chainable.
    pub fn children_of(mut self, children: impl IntoIterator<Item = Node>) -> Self {
        self.children.extend(children);
        self
    }

    /// Read one property back, for tests and for views that branch on their own state.
    pub fn prop_value(&self, name: &str) -> Option<&CapabilityValue> {
        self.props.get(name)
    }

    /// The `key` of this node, if it has one.
    pub fn key_str(&self) -> Option<&str> {
        self.key.as_deref()
    }

    /// Total number of nodes in this subtree, counting `self`.
    ///
    /// Iterative: a deeply nested declarative tree is legal (the JSON loader allows 64
    /// levels), and a recursive count would be the one place in this module that could
    /// overflow a small stack.
    pub fn node_count(&self) -> usize {
        let mut count = 0usize;
        let mut stack = vec![self];
        while let Some(node) = stack.pop() {
            count += 1;
            stack.extend(node.children.iter());
        }
        count
    }

    /// The keys that appear more than once among the **direct** children of this node.
    ///
    /// Reported rather than asserted because a view is data. A duplicate key is
    /// genuinely ambiguous: two siblings would claim the same identity, so a diff could
    /// never move or remove either one unambiguously.
    ///
    /// # Who consumes this
    ///
    /// [`crate::view::diff`] counts collisions itself and reports them through
    /// [`DiffReport::duplicate_keys`](crate::view::diff::DiffReport::duplicate_keys), so
    /// the condition is visible even if a caller never asks. This method is what lets a
    /// caller *name* the offending keys — for a diagnostic, or to reject the view before
    /// mounting it. The `tools/check_view_keys_are_unique.sh` gate checks the same
    /// invariant statically, at the source level, for `Node` builder chains.
    pub fn duplicate_sibling_keys(&self) -> Vec<(&str, usize)> {
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for child in &self.children {
            if let Some(k) = child.key_str() {
                *counts.entry(k).or_insert(0) += 1;
            }
        }
        let mut dups: Vec<(&str, usize)> = counts.into_iter().filter(|(_, n)| *n > 1).collect();
        dups.sort_by(|a, b| a.0.cmp(b.0));
        dups
    }

    /// Depth-first walk of this subtree, yielding `(node, depth)`.
    ///
    /// The single traversal point for diagnostics, so a report cannot disagree with
    /// itself about what the tree contains.
    pub fn walk(&self) -> Vec<(&Node, usize)> {
        let mut out = Vec::new();
        let mut stack = vec![(self, 0usize)];
        while let Some((node, depth)) = stack.pop() {
            out.push((node, depth));
            for child in node.children.iter().rev() {
                stack.push((child, depth + 1));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> CapabilityValue {
        CapabilityValue::String(v.to_string())
    }

    #[test]
    fn new_node_is_a_keyless_leaf() {
        let n = Node::new("label");
        assert_eq!(n.widget, "label");
        assert_eq!(n.key, None);
        assert!(n.props.is_empty());
        assert!(n.children.is_empty());
        assert_eq!(n.node_count(), 1);
    }

    #[test]
    fn builder_chain_sets_key_and_props() {
        let n = Node::new("label").key("title").prop("text", s("Hi"));
        assert_eq!(n.key_str(), Some("title"));
        assert_eq!(n.prop_value("text"), Some(&s("Hi")));
    }

    #[test]
    fn setting_a_property_twice_keeps_the_last_write() {
        // The JSON loader lets a later key win; the declarative builder must agree, or
        // the same layout would mean different things through the two front ends.
        let n = Node::new("label").prop("text", s("first")).prop("text", s("second"));
        assert_eq!(n.prop_value("text"), Some(&s("second")));
    }

    #[test]
    fn children_keep_insertion_order() {
        let n = Node::new("vbox").child(Node::new("a")).child(Node::new("b")).child(Node::new("c"));
        let names: Vec<&str> = n.children.iter().map(|c| c.widget.as_str()).collect();
        assert_eq!(names, ["a", "b", "c"]);
    }

    #[test]
    fn children_of_appends_rather_than_replaces() {
        let n =
            Node::new("vbox").child(Node::new("a")).children_of([Node::new("b"), Node::new("c")]);
        assert_eq!(n.children.len(), 3);
        assert_eq!(n.children[2].widget, "c");
    }

    #[test]
    fn node_count_covers_the_whole_subtree() {
        let n = Node::new("vbox")
            .child(Node::new("a").child(Node::new("a1")).child(Node::new("a2")))
            .child(Node::new("b"));
        assert_eq!(n.node_count(), 5);
    }

    #[test]
    fn walk_is_depth_first_and_reports_depth() {
        let n =
            Node::new("vbox").child(Node::new("a").child(Node::new("a1"))).child(Node::new("b"));
        let seen: Vec<(&str, usize)> =
            n.walk().into_iter().map(|(node, d)| (node.widget.as_str(), d)).collect();
        assert_eq!(seen, [("vbox", 0), ("a", 1), ("a1", 2), ("b", 1)]);
    }

    #[test]
    fn duplicate_sibling_keys_are_reported() {
        let n = Node::new("vbox")
            .child(Node::new("a").key("dup"))
            .child(Node::new("b").key("dup"))
            .child(Node::new("c").key("unique"));
        assert_eq!(n.duplicate_sibling_keys(), [("dup", 2)]);
    }

    #[test]
    fn duplicate_report_ignores_keyless_siblings() {
        // Two keyless nodes are not "duplicates" — they are both simply unmatchable by
        // key, which the diff reports through its positional counter instead.
        let n = Node::new("vbox").child(Node::new("a")).child(Node::new("b"));
        assert!(n.duplicate_sibling_keys().is_empty());
    }

    #[test]
    fn duplicate_report_covers_only_direct_children() {
        // Keys are scoped to siblings, so the same key in a different list is legal.
        let n = Node::new("vbox")
            .child(Node::new("list").key("left").child(Node::new("row").key("total")))
            .child(Node::new("list").key("right").child(Node::new("row").key("total")));
        assert!(n.duplicate_sibling_keys().is_empty());
    }

    #[test]
    fn duplicate_report_is_not_confused_by_grandchildren() {
        let n = Node::new("vbox")
            .child(Node::new("a").key("k").child(Node::new("a1").key("k")))
            .child(Node::new("b").key("k"));
        // `a` and `b` collide; `a`'s child is a different sibling set.
        assert_eq!(n.duplicate_sibling_keys(), [("k", 2)]);
    }

    #[test]
    fn props_are_compared_by_exact_value() {
        // BLUE18 rule B-5: no fuzzy matching. A diff that treated 1.0 and 1.0000001 as
        // equal would silently refuse to apply a real change.
        let a = Node::new("x").prop("v", CapabilityValue::Float(1.0));
        let b = Node::new("x").prop("v", CapabilityValue::Float(1.000_000_1));
        assert_ne!(a.props, b.props);
    }

    #[test]
    fn a_node_is_a_value_that_can_be_cloned_and_compared() {
        let a = Node::new("label").key("k").prop("text", s("t"));
        let b = a.clone();
        assert_eq!(a, b);
    }
}
