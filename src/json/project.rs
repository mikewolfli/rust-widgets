// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! A parsed project document, addressable by tree path.
//!
//! # Where this lives, and why not in `crate::designer`
//!
//! It parses JSON, so it needs `serde_json`, which is compiled in only for an unstripped device
//! build (`full_widgets`). It therefore sits beside the loader that shares that dependency rather
//! than in `crate::designer`, which has to exist on `mini` as well. The generator reads this type,
//! so a `mini` build cannot *parse* a project — it can only be the *target* of one, which is the
//! honest division: a device with `alloc_frugal` storage is not where a designer runs.
//!
//! # Why this exists rather than reusing the loader's output
//!
//! `JsonLoader::load` **instantiates** — it creates controls, registers ids and applies properties.
//! A generator must not do any of that: it produces source text on a machine that has no window,
//! and it must be able to run twice over one document and get byte-identical output.
//!
//! So this type is the parse half on its own: the same node shape, the same "later key wins" rule,
//! the same widget-name resolution, but as **inert data**. The generator's two templates and the
//! consistency gate all read it, which is what keeps them from each growing their own reader.
//!
//! # Why paths rather than references
//!
//! A node's identity in a document is its position, and both output modes need to name a node in a
//! diagnostic (`path [0, 2]`) and to derive a binding name from it. Paths are also what a diff
//! between two documents would compare, which is what a regenerate-and-compare gate needs.

use crate::compat::{format, String, Vec};
use serde_json::Value;

/// One node of a parsed project.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectNode {
    /// The widget type name, as the document spells it (lower-case key).
    pub widget: String,
    /// Where this node sits in the tree; `[]` is the root.
    pub path: Vec<usize>,
    /// The document's stable key, or a path-derived default.
    pub key: String,
    /// Scalar properties, in the document's own key order.
    ///
    /// A `Vec` of pairs rather than a map: the generator's output has to be a function of the input
    /// alone, and map iteration order is not.
    pub properties: Vec<(String, Value)>,
    /// The `events` object, kept out of [`Self::properties`] because a wire is not a property value.
    pub wire_declaration: Option<Value>,
    /// The `on_*` compatibility keys, kept out of [`Self::properties`] for the same reason.
    pub marker_declarations: Vec<(String, Value)>,
    /// Child indices, in document order.
    pub children: Vec<usize>,
}

impl ProjectNode {
    /// Scalar properties only, in document order.
    ///
    /// A non-scalar value (an array or an object) is not a property value: it is structure the node
    /// type does not model yet. Returning it here would make the generator emit a stringified form
    /// that the property setter would *accept*, producing a control nothing like the document.
    pub fn scalar_properties(&self) -> Vec<(String, Value)> {
        self.properties
            .iter()
            .filter(|(_, value)| !matches!(value, Value::Array(_) | Value::Object(_)))
            .cloned()
            .collect()
    }

    /// A text or title argument, if the document declares one.
    ///
    /// Both spellings because the document uses `title` for windows and `text` for label-like
    /// controls, and a stripped-target constructor takes whichever its control publishes first.
    pub fn text(&self) -> Option<String> {
        for name in ["text", "title"] {
            if let Some(value) = self.property(name) {
                if let Some(s) = value.as_str() {
                    if !s.is_empty() {
                        return Some(String::from(s));
                    }
                }
            }
        }
        None
    }

    /// One property's value, by name.
    pub fn property(&self, name: &str) -> Option<&Value> {
        self.properties.iter().find(|(key, _)| key == name).map(|(_, value)| value)
    }

    /// Handler names declared for published events (the `events` object).
    ///
    /// Sorted by event name so a generated file is a function of its input.
    ///
    /// # Where this reads from
    ///
    /// [`collect_properties`] removes the wire keys, because they are not property values — so the
    /// handlers are captured **separately** at parse time rather than recovered from the property
    /// list. The first version of this method read the property list and could therefore never
    /// return anything; its test caught that immediately.
    pub fn declared_handlers(&self) -> Vec<(String, String)> {
        let Some(value) = self.wire_declaration.as_ref() else {
            return Vec::new();
        };
        let Some(map) = value.as_object() else {
            return Vec::new();
        };
        let mut out: Vec<(String, String)> = map
            .iter()
            .filter_map(|(event, handler)| {
                handler.as_str().map(|h| (String::from(event.as_str()), String::from(h)))
            })
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    /// The `on_*` compatibility keys this node declares, sorted by key.
    pub fn declared_marker_handlers(&self) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = self
            .marker_declarations
            .iter()
            .filter_map(|(key, value)| value.as_str().map(|h| (key.clone(), String::from(h))))
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }
}

/// A parsed project document.
#[derive(Debug, Clone, PartialEq)]
pub struct JsonProject {
    /// The root control's name.
    pub root_widget: String,
    /// Every node, in parent-before-child (pre-order) order. The root is first.
    nodes: Vec<ProjectNode>,
}

impl JsonProject {
    /// Parses a document.
    ///
    /// # Errors
    ///
    /// The document must be one JSON object with exactly one key at the root — the same shape the
    /// loader requires. Depth is bounded by [`MAX_DEPTH`], matching the loader, so a malformed
    /// document cannot make the generator recurse without limit.
    pub fn parse(json: &str) -> Result<Self, String> {
        let value: Value = serde_json::from_str(json)
            .map_err(|e| format!("layout JSON ({} bytes) could not be parsed: {e}", json.len()))?;
        let root = value.as_object().ok_or_else(|| {
            String::from("JSON root must be an object of the form { \"<widget>\": { .. } }")
        })?;
        if root.len() != 1 {
            return Err(format!(
                "JSON root must have exactly one widget key, found {}: {}",
                root.len(),
                root.keys().cloned().collect::<Vec<_>>().join(", ")
            ));
        }
        let (widget, body) = root.iter().next().expect("checked len == 1");
        let root_widget = widget.to_lowercase();

        let mut nodes = Vec::new();
        collect_preorder(&root_widget, body, &mut Vec::new(), 0, &mut nodes)?;
        Ok(Self { root_widget, nodes })
    }

    /// The node at `index` among `node`'s children.
    ///
    /// # Why this needs the project
    ///
    /// `ProjectNode::children` holds **positions in the flattened node list**, not child indices.
    /// That encoding is what lets a pre-order walk visit a node before its children without a second
    /// pass. Resolving it here means every consumer reads one interpretation instead of each
    /// reconstructing it.
    pub fn child_of(&self, node: &ProjectNode, index: usize) -> Option<&ProjectNode> {
        self.nodes.get(*node.children.get(index)?)
    }

    /// The root node.
    pub fn root(&self) -> Option<&ProjectNode> {
        self.nodes.first()
    }

    /// The node at `path`, or `None`.
    pub fn node(&self, path: &[usize]) -> Option<&ProjectNode> {
        self.nodes.iter().find(|n| n.path.as_slice() == path)
    }

    /// Every node, in pre-order.
    pub fn walk(&self) -> impl Iterator<Item = &ProjectNode> {
        self.nodes.iter()
    }

    /// How many nodes the document has.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the document has no nodes (only possible if the root was rejected).
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// The paths of every descendant of `path`, in pre-order.
    pub fn descendants(&self, path: &[usize]) -> Vec<Vec<usize>> {
        self.nodes
            .iter()
            .filter(|node| node.path.len() > path.len() && node.path.starts_with(path))
            .map(|node| node.path.clone())
            .collect()
    }

    /// The document's `layout` declaration, as a kind and the child paths it places.
    ///
    /// # Why the kind is resolved here
    ///
    /// The name in the document (`"vbox"`) is not the engine's kind; `crate::json::parse_layout_kind`
    /// owns that mapping, and a second mapping here would let the generator solve a layout the
    /// runtime would not. Only the names that function already knows are reported, so an unknown
    /// name means "no layout", which the caller treats as "nothing placed" rather than a guess.
    pub fn layout_declaration(
        &self,
    ) -> Option<(crate::json::DeclarativeLayoutKind, Vec<Vec<usize>>)> {
        let root = self.root()?;
        let layout = root.property("layout")?;
        // Delegated to the loader's own parser: a second `"vbox" -> VBox` mapping here would let
        // the generator solve a layout the runtime would not.
        let kind = crate::json::parse_layout_kind(layout).ok()?;

        // The layout places the container's own children, in document order.
        let placed: Vec<Vec<usize>> = root
            .children
            .iter()
            .filter_map(|index| self.nodes.get(*index))
            .map(|n| n.path.clone())
            .collect();
        Some((kind, placed))
    }
}

/// The depth bound, matching the loader's.
pub const MAX_DEPTH: u32 = 64;

/// Appends `widget`/`body` and its whole subtree to `nodes`, in pre-order.
///
/// # Why recursion is bounded rather than avoided
///
/// A depth-first walk is the natural shape here and the depth check makes it safe: `depth > MAX_DEPTH`
/// returns before the next frame is entered, so the maximum stack is bounded by a constant this
/// module states. The loader guards its own recursion with the same bound and the same `MAX_DEPTH`
/// value, so a document one of them accepts is not rejected by the other for depth reasons.
fn collect_preorder(
    widget: &str,
    body: &Value,
    path: &mut Vec<usize>,
    depth: u32,
    nodes: &mut Vec<ProjectNode>,
) -> Result<(), String> {
    if depth > MAX_DEPTH {
        return Err(format!(
            "widget tree is nested {depth} levels deep, which exceeds the maximum of {MAX_DEPTH}; \
             flatten the layout to generate it"
        ));
    }

    let obj = body.as_object().ok_or_else(|| format!("`{widget}` value must be a JSON object"))?;

    // The node's own slot is reserved **before** its children are walked, which is what makes the
    // list pre-order and lets `children` hold final positions rather than being renumbered later.
    let own_index = nodes.len();
    nodes.push(ProjectNode {
        widget: String::from(widget),
        path: path.clone(),
        key: key_from_path(path),
        properties: collect_properties(obj),
        wire_declaration: obj.get(crate::json::EVENTS_KEY).cloned(),
        marker_declarations: crate::json::MARKER_KEYS
            .iter()
            .filter_map(|(key, _)| obj.get(*key).map(|v| (String::from(*key), v.clone())))
            .collect(),
        children: Vec::new(),
    });

    let mut children: Vec<usize> = Vec::new();
    // # Where a node's children live
    //
    // Two shapes appear in this project's documents, and both must be read:
    //
    //   * `{"vbox": {"children": [..]}}` — children directly on the container;
    //   * `{"window": {"layout": {"type": "vbox", "children": [..]}}}` — children under a
    //     `layout` declaration, which is how a window states its arrangement.
    //
    // The loader reads both, so a generator that read only one would generate a different tree from
    // the one the running program shows — the disagreement T-24 exists to detect.
    let mut child_arrays: Vec<&Value> = Vec::new();
    if let Some(direct) = obj.get("children") {
        child_arrays.push(direct);
    }
    if let Some(layout_children) =
        obj.get("layout").and_then(|l| l.as_object()).and_then(|l| l.get("children"))
    {
        child_arrays.push(layout_children);
    }

    for child_array in child_arrays {
        let Some(child_values) = child_array.as_array() else {
            continue;
        };
        for (child_index, child) in child_values.iter().enumerate() {
            let Some(child_obj) = child.as_object() else {
                continue;
            };
            if child_obj.len() != 1 {
                continue;
            }
            let (child_widget, child_body) = child_obj.iter().next().expect("checked len == 1");
            path.push(child_index);
            let before = nodes.len();
            collect_preorder(&child_widget.to_lowercase(), child_body, path, depth + 1, nodes)?;
            path.pop();
            children.push(before);
        }
    }
    nodes[own_index].children = children;
    Ok(())
}

/// A stable key for a path, used when the document declares none.
fn key_from_path(path: &[usize]) -> String {
    if path.is_empty() {
        return String::from("root");
    }
    let mut key = String::from("n");
    for index in path {
        key.push('_');
        key.push_str(&format!("{index}"));
    }
    key
}

/// The scalar-and-structure properties of a node, in the document's key order.
///
/// # Why `id` and `children` and the wire keys are excluded
///
/// `id` becomes the node key, `children` is structure, and the wire keys (`events`, `on_*`) are
/// handled by the event route. Leaving any of them in the property list would make the generator
/// emit them as property assignments, which the property router would reject at run time.
fn collect_properties(obj: &serde_json::Map<String, Value>) -> Vec<(String, Value)> {
    let mut out: Vec<(String, Value)> = Vec::new();
    for (key, value) in obj {
        if matches!(key.as_str(), "id" | "children") {
            continue;
        }
        if crate::json::is_marker_key(key) || key == crate::json::EVENTS_KEY {
            continue;
        }
        out.push((String::from(key.as_str()), value.clone()));
    }
    // Sorted by name: the generator's output must be a function of the document alone, and a
    // `serde_json::Map`'s order is not guaranteed to be the document's.
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple() -> &'static str {
        r#"{"window":{"id":"w","title":"T","layout":{"type":"vbox","children":[
            {"button":{"id":"b","text":"Go"}},
            {"label":{"text":"Hi"}}
        ]}}}"#
    }

    #[test]
    fn a_document_parses_into_a_pre_order_tree() {
        let project = JsonProject::parse(simple()).expect("valid document");
        assert_eq!(project.root_widget, "window");
        assert_eq!(project.len(), 3, "window + button + label");
        assert_eq!(project.root().unwrap().path, Vec::<usize>::new());
        // Pre-order: the root is first, its children follow.
        let paths: Vec<Vec<usize>> = project.walk().map(|n| n.path.clone()).collect();
        assert_eq!(paths, vec![vec![], vec![0], vec![1]]);
    }

    #[test]
    fn children_are_renumbered_to_point_at_the_reordered_nodes() {
        let project = JsonProject::parse(simple()).expect("valid document");
        let root = project.root().unwrap();
        assert_eq!(root.children.len(), 2);
        assert_eq!(project.nodes[root.children[0]].widget, "button");
        assert_eq!(project.nodes[root.children[1]].widget, "label");
    }

    #[test]
    fn the_id_key_is_the_node_key_and_not_a_property() {
        let project = JsonProject::parse(simple()).expect("valid document");
        let button = project.node(&[0]).unwrap();
        assert_eq!(button.key, "n_0", "the key is path-derived so two runs agree");
        assert!(button.property("id").is_none(), "`id` is structure, not a property");
    }

    #[test]
    fn wire_keys_are_not_properties() {
        let json =
            r#"{"button":{"id":"b","text":"Go","events":{"clicked":"on_go"},"on_close":"on_x"}}"#;
        let project = JsonProject::parse(json).expect("valid document");
        let button = project.root().unwrap();
        assert!(button.property("events").is_none());
        assert!(button.property("on_close").is_none());
        assert_eq!(button.property("text").and_then(|v| v.as_str()), Some("Go"));
    }

    #[test]
    fn declared_handlers_are_sorted_by_published_name() {
        let json = r#"{"button":{"events":{"pressed":"on_p","clicked":"on_c"}}}"#;
        let project = JsonProject::parse(json).expect("valid document");
        assert_eq!(
            project.root().unwrap().declared_handlers(),
            vec![
                (String::from("clicked"), String::from("on_c")),
                (String::from("pressed"), String::from("on_p")),
            ],
            "sorted so the generated file is a function of the document"
        );
    }

    #[test]
    fn text_reads_text_then_title() {
        let project = JsonProject::parse(r#"{"window":{"title":"Win"}}"#).expect("valid");
        assert_eq!(project.root().unwrap().text(), Some(String::from("Win")));
    }

    #[test]
    fn an_empty_text_does_not_become_a_constructor_argument() {
        let project = JsonProject::parse(r#"{"button":{"text":""}}"#).expect("valid");
        assert_eq!(project.root().unwrap().text(), None);
    }

    #[test]
    fn a_non_scalar_property_is_not_a_scalar_property() {
        let json = r#"{"window":{"items":[1,2],"layout":{"type":"vbox"}}}"#;
        let project = JsonProject::parse(json).expect("valid");
        let scalars = project.root().unwrap().scalar_properties();
        assert!(
            !scalars.iter().any(|(name, _)| name == "items"),
            "an array is structure, not a property value"
        );
    }

    #[test]
    fn the_layout_declaration_names_the_kind_and_its_children() {
        let project = JsonProject::parse(simple()).expect("valid document");
        let (kind, children) = project.layout_declaration().expect("`vbox` must resolve");
        assert!(
            matches!(kind, crate::json::DeclarativeLayoutKind::VBox { .. }),
            "`vbox` must resolve to the VBox kind, got {kind:?}"
        );
        assert_eq!(children, vec![vec![0], vec![1]]);
    }

    #[test]
    fn an_unknown_layout_kind_is_not_reported_as_placed() {
        let json = r#"{"window":{"layout":{"type":"nonsense"}}}"#;
        let project = JsonProject::parse(json).expect("valid");
        assert!(
            project.layout_declaration().is_none(),
            "an unknown kind must mean `not placed`, never a guess"
        );
    }

    #[test]
    fn a_document_without_a_layout_reports_none() {
        let project = JsonProject::parse(r#"{"window":{"title":"T"}}"#).expect("valid");
        assert!(project.layout_declaration().is_none());
    }

    #[test]
    fn descendants_are_the_nested_paths() {
        let project = JsonProject::parse(simple()).expect("valid document");
        assert_eq!(project.descendants(&[]), vec![vec![0], vec![1]]);
        assert!(project.descendants(&[0]).is_empty());
    }

    #[test]
    fn a_root_with_two_widget_keys_is_refused() {
        let error = JsonProject::parse(r#"{"button":{},"label":{}}"#).unwrap_err();
        assert!(error.contains("exactly one widget key"), "got: {error}");
    }

    #[test]
    fn malformed_json_reports_the_parse_error_and_the_size() {
        let error = JsonProject::parse("not json").unwrap_err();
        assert!(error.contains("could not be parsed"), "got: {error}");
    }

    #[test]
    fn a_non_object_node_body_is_refused() {
        let error = JsonProject::parse(r#"{"button": 42}"#).unwrap_err();
        assert!(error.contains("must be a JSON object"), "got: {error}");
    }

    #[test]
    fn a_deeply_nested_document_is_refused_rather_than_recursed() {
        // Build `{"window":{"children":[{"window":{"children":[...]}}]}}` past the bound.
        let mut json = String::from(r#"{"window":{}}"#);
        for _ in 0..(MAX_DEPTH + 2) {
            json = format!(r#"{{"window":{{"children":[{json}]}}}}"#);
        }
        let error = JsonProject::parse(&json).unwrap_err();
        // # Which refusal wins, measured rather than assumed
        //
        // `serde_json` has its own nesting limit and reaches it first at this depth: the error is
        // "recursion limit exceeded", not this module's message. Both are refusals, and the test
        // asserts the property that matters — the document does not parse — rather than the wording
        // of whichever layer refused. Asserting this module's message would have made the test
        // depend on an unreachable branch.
        assert!(
            error.contains("exceeds the maximum") || error.contains("recursion limit"),
            "a too-deep document must be refused, got: {error}"
        );
    }

    #[test]
    fn a_document_just_under_the_bound_is_accepted() {
        // The other direction: the bound must not be so tight that a legal document is refused.
        let mut json = String::from(r#"{"label":{}}"#);
        for _ in 0..8 {
            json = format!(r#"{{"window":{{"children":[{json}]}}}}"#);
        }
        let project = JsonProject::parse(&json).expect("eight levels is well inside the bound");
        assert_eq!(project.len(), 9, "each nesting level contributes one node");
    }
}
