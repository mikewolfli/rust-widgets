// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The JSON loader's **incremental** path (G-406 / RW-43).
//!
//! # What existed before this module, and what was missing
//!
//! [`JsonLoader::load`](crate::json::JsonLoader::load) parses a document and instantiates it
//! **once**. Loading the same edited document a second time builds a second independent tree, so
//! every control is new: focus, scroll offset, text selection and animation state are gone. That
//! is the right answer for "open a document" and the wrong answer for a designer's
//! edit-reload loop.
//!
//! [`crate::view::ViewEngine`] is the retained-tree engine that solves the second problem: it
//! diffs a rebuilt [`Node`] tree against the mounted one and patches only the difference, so an
//! unchanged control keeps its identity — and therefore its state. Its `mount_tree`/`update_tree`
//! doors are explicitly documented as *the* forwarding point for a loader that already owns a
//! `Node` tree; this module is that caller.
//!
//! # The pipeline
//!
//! ```text
//! JSON text ──JsonProject::parse──▶ parsed tree ──convert──▶ view::Node ──ViewEngine──▶ controls
//! ```
//!
//! Parsing is delegated to [`crate::json::project::JsonProject`], the same parser the designer's
//! generator uses, so the interpreted and generated front ends cannot disagree about what a
//! document *means*. Conversion only maps the parsed facts onto [`Node`] values; identity, diffing
//! and patching stay in `crate::view`.
//!
//! # What this path carries, stated rather than implied
//!
//! A `Node` property is written through the control's **published property contract**. Keys the
//! JSON loader applies through a dedicated route (`x`/`y`/`width`/`height` geometry, `padding`,
//! hex colours, construction-only switches such as `password`) are not properties, so this path
//! does not invent writes for them: every one is recorded in [`JsonViewDrop`] with the reason.
//! The full loader remains the path that applies them; the incremental path is for previewing an
//! edit without losing the controls the edit did not touch.
//!
//! Wires *are* carried, under the same `__wire_` carrier a generated program uses, and bound by
//! the library's own [`bind_published_event`](crate::json::bind_published_event) /
//! [`bind_marker_event`](crate::json::bind_marker_event). A wire therefore resolves exactly as it
//! does on the load path, with no second rule table here.
//!
//! # Reachability
//!
//! **State:** Enabling API. The host's edit-reload loop is its consumer; this repository's tests
//! exercise it end to end (`mount` → unchanged `update` → `update` that removes a node). The
//! existing full-mount loader path is untouched: this module adds a session around the same
//! parser and creates controls through the same factory.

use crate::compat::{format, Rc, String, Vec};
use crate::core::ObjectId;
use crate::json::project::{JsonProject, ProjectNode};
use crate::view::{DiffReport, Node, ViewEngine};

/// One document key the incremental path could not carry, and why.
///
/// Recorded rather than dropped silently (rule #12): a preview that quietly omits `password` or
/// `padding` is a preview that lies about the document it is showing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonViewDrop {
    /// The tree path the key was declared on (`[]` = root).
    pub path: Vec<usize>,
    /// The node key, so a panel can identify the control without resolving the path.
    pub key: String,
    /// The document key.
    pub property: String,
    /// Why the incremental path did not carry it.
    pub reason: String,
}

/// A document converted to the declarative [`Node`] tree the engine consumes.
#[derive(Debug, Clone)]
pub struct JsonViewTree {
    /// The root node.
    pub root: Node,
    /// How many nodes the document declares (the root included).
    pub nodes: usize,
    /// Keys that were not carried, with the reason.
    pub dropped: Vec<JsonViewDrop>,
}

/// What one mount or update did.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct JsonViewRunReport {
    /// How many nodes the submitted document declares.
    pub nodes_declared: usize,
    /// Keys the incremental path did not carry.
    pub dropped: Vec<JsonViewDrop>,
    /// The diff the engine produced. Empty (`is_unchanged`) for a mount, and empty for an update
    /// that found nothing to do — in which case no control was touched.
    pub diff: DiffReport,
    /// The controls the update destroyed, subtree members included (RW-43).
    ///
    /// This is [`DiffReport::removed`] copied out for the caller that only wants the ids: a host
    /// drops its own per-control state when a control is gone, and asking a second question of
    /// the diff to find out which ones would be a chance to use the wrong field.
    pub removed_ids: Vec<ObjectId>,
    /// Whether this run updated a mounted tree (`true`) or mounted (`false`).
    pub updated: bool,
    /// The engine's apply report for a **mount**, or `None` for an update.
    ///
    /// A mount creates the whole tree, so this is where a refused construction or property write
    /// is visible. An update's writes are folded into [`Self::diff`] and the engine logs refused
    /// patches itself, so there is no second apply report to return; reporting a default one
    /// would claim "zero refusals" for a batch that may have had some.
    pub mount_report: Option<crate::view::ApplyReport>,
}

impl JsonViewRunReport {
    /// Whether an update found the document identical to the mounted one.
    ///
    /// A mount is never "unchanged": it created the tree. A report with `updated: false` and no
    /// patches is the mount case, not a no-op edit.
    pub fn is_unchanged(&self) -> bool {
        self.updated && self.diff.is_unchanged() && self.removed_ids.is_empty()
    }
}

/// Converts a project document to a declarative [`Node`] tree.
///
/// # Errors
///
/// Returns the parser's error when the document is malformed. A document that parses always
/// converts: a key the property contract does not publish is reported in
/// [`JsonViewTree::dropped`] rather than failing the whole preview.
pub fn json_to_view_tree(json: &str) -> Result<JsonViewTree, String> {
    let project = JsonProject::parse(json)?;
    let factory = crate::widget::capability::WidgetFactory::shared();
    let mut dropped = Vec::new();
    let root = project
        .root()
        .map(|node| convert_node(&project, node, factory, &mut dropped))
        .ok_or_else(|| String::from("the parsed document has no root node"))?;
    Ok(JsonViewTree { root, nodes: project.len(), dropped })
}

/// Creates one control for a generated/converted node through the widget factory.
///
/// The geometry is a zero rect: the layout pass owns placement, and the view engine writes the
/// properties a control publishes — not its rectangle. The text a text-bearing constructor needs
/// is read from `text`/`title`, the same two keys the generated `create_for` reads, so the two
/// trees behave the same.
///
/// Wires the node carries are bound here, for the reason the generated `create_for` documents: the
/// node is the only thing that knows which handler the document declared, and binding at creation
/// is what makes `events: { clicked: "on_save" }` mean something.
pub fn create_control_from_factory(node: &Node) -> Option<ObjectId> {
    let text = node
        .prop_value("text")
        .or_else(|| node.prop_value("title"))
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let control = crate::widget::capability::WidgetFactory::shared().create(
        node.widget.as_str(),
        crate::core::Rect::new(0, 0, 0, 0),
        text,
    )?;
    let id = crate::widget::runtime::register(control)?;
    bind_wires(id, node);
    Some(id)
}

/// Binds every wire a converted node carries (`events.*` and `on_*` carriers).
fn bind_wires(id: ObjectId, node: &Node) {
    for (name, value) in node.props.iter() {
        let Some(suffix) = crate::view::generated_wire_suffix(name) else { continue };
        let Some(handler) = value.as_str() else { continue };
        if let Some(event) = suffix.strip_prefix("events.") {
            crate::json::bind_published_event(id, node.widget.as_str(), event, handler);
        } else if let Some(marker) = crate::json::marker_for_key(suffix) {
            crate::json::bind_marker_event(id, suffix, marker, handler);
        }
    }
}

/// Converts one parsed node and its subtree.
fn convert_node(
    project: &JsonProject,
    node: &ProjectNode,
    factory: &crate::widget::capability::WidgetFactory,
    dropped: &mut Vec<JsonViewDrop>,
) -> Node {
    // The document's `id` is the designer's identity and survives a sibling insertion; the
    // path-shaped key is the fallback for a document that declared none. This is the same
    // choice the runtime loader makes (it records a node by its `id`).
    let node_key = node.id.clone().unwrap_or_else(|| node.key.clone());
    let mut converted = Node::new(node.widget.clone()).key(node_key.clone());

    for (name, value) in &node.properties {
        // The layout declaration is structure (its children are emitted as this node's children
        // below), not a property of any control.
        if name == "layout" {
            continue;
        }
        // A wire is not a property; it travels on the runtime's own carrier so the engine skips
        // it as a property write and `bind_wires` can read it back.
        if crate::json::is_marker_key(name) || name == crate::json::EVENTS_KEY {
            continue;
        }
        // `text`/`title` are loader-owned on the full-mount path, but they are also the two keys
        // a text-bearing constructor and the property contract both need. The generated
        // `create_for` reads them from the node for exactly this reason, so the incremental
        // path carries them when the control publishes the name (the schema check below decides).
        if crate::json::is_loader_owned_key(name) && !matches!(name.as_str(), "text" | "title") {
            dropped.push(JsonViewDrop {
                path: node.path.clone(),
                key: node_key.clone(),
                property: name.clone(),
                reason: String::from(
                    "the JSON loader applies this key through a dedicated route (geometry, \
                     spacing, colour or a construction-time switch); the incremental view path \
                     does not invent a property write for it",
                ),
            });
            continue;
        }
        let Some(schema) = factory.property_schema(&node.widget, name).ok() else {
            dropped.push(JsonViewDrop {
                path: node.path.clone(),
                key: node_key.clone(),
                property: name.clone(),
                reason: format!("`{}` is not a published property of `{}`", name, node.widget),
            });
            continue;
        };
        if !schema.writable {
            dropped.push(JsonViewDrop {
                path: node.path.clone(),
                key: node_key.clone(),
                property: name.clone(),
                reason: String::from("the control publishes this property as read-only"),
            });
            continue;
        }
        let converted_value = crate::json::to_capability_value_with_record_schema(
            value,
            Some(schema.value_kind),
            schema.record_schema,
        );
        match converted_value {
            Some(value) => {
                converted = converted.prop(name.clone(), value);
            }
            None => dropped.push(JsonViewDrop {
                path: node.path.clone(),
                key: node_key.clone(),
                property: name.clone(),
                reason: format!(
                    "the declared value does not have the shape `{}` expects",
                    crate::widget::capability::value_kind_token(schema.value_kind)
                ),
            }),
        }
    }

    // Wires are carried, not dropped: `bind_wires` reads them back at creation.
    for (event, handler) in node.declared_handlers() {
        converted = converted.prop(
            crate::view::generated_wire_prop(&format!("{}.{}", crate::json::EVENTS_KEY, event)),
            crate::widget::capability::CapabilityValue::String(handler),
        );
    }
    for (key, handler) in node.declared_marker_handlers() {
        converted = converted.prop(
            crate::view::generated_wire_prop(&key),
            crate::widget::capability::CapabilityValue::String(handler),
        );
    }

    for index in 0..node.children.len() {
        if let Some(child) = project.child_of(node, index) {
            let child = convert_node(project, child, factory, dropped);
            converted = converted.child(child);
        }
    }
    converted
}

/// A JSON document kept alive through the declarative engine, with an incremental update path.
///
/// # Identity is the engine's, not this type's
///
/// The session owns one [`ViewEngine`] and hands it a freshly converted tree on every load. Two
/// documents that describe the same keyed node produce the same `Node` declaration, so the engine
/// emits no patch for it and the live control keeps its id — and with it focus, scroll and
/// selection. A node that disappeared produces a removal whose ids are reported; a node that
/// appeared produces an insert whose control is created through `create`.
pub struct JsonViewSession {
    engine: ViewEngine,
}

impl JsonViewSession {
    /// Creates a session with nothing mounted.
    pub fn new() -> Self {
        Self { engine: ViewEngine::new() }
    }

    /// The engine, for a host that also needs its layers, paths or layout binding.
    pub fn engine(&self) -> &ViewEngine {
        &self.engine
    }

    /// Mutable access to the engine.
    pub fn engine_mut(&mut self) -> &mut ViewEngine {
        &mut self.engine
    }

    /// The tree currently mounted, if any.
    pub fn current(&self) -> Option<&Node> {
        self.engine.current()
    }

    /// Whether a tree is mounted.
    pub fn is_mounted(&self) -> bool {
        self.engine.current().is_some()
    }

    /// Mounts `json`, replacing any mounted tree.
    ///
    /// Mounting is authoritative (the engine's own contract): the previous controls are torn down.
    /// Use [`Self::update`] for an edit that should keep unchanged controls alive.
    pub fn mount(&mut self, json: &str) -> Result<JsonViewRunReport, String> {
        self.mount_with(json, &create_control_from_factory)
    }

    /// [`Self::mount`] with an injected control factory.
    ///
    /// Injected for the same reason the engine's `create` argument is: a test, or a host with its
    /// own construction policy, must be able to supply it without this module guessing.
    pub fn mount_with(
        &mut self,
        json: &str,
        create: &dyn Fn(&Node) -> Option<ObjectId>,
    ) -> Result<JsonViewRunReport, String> {
        let tree = json_to_view_tree(json)?;
        let nodes_declared = tree.nodes;
        let dropped = tree.dropped;
        // Mounting is authoritative; a previous tree's ids are torn down by the engine and are
        // not an edit's removals, so the report names no removed ids here.
        let apply = self.engine.mount_tree_shared(Rc::new(tree.root), create);
        Ok(JsonViewRunReport {
            nodes_declared,
            dropped,
            diff: DiffReport::default(),
            removed_ids: Vec::new(),
            updated: false,
            mount_report: Some(apply),
        })
    }

    /// Applies `json` as an edit of the mounted tree, creating the session first when nothing is
    /// mounted.
    ///
    /// The returned report names every control the edit destroyed in
    /// [`JsonViewRunReport::removed_ids`], and its diff is empty when the edit changed nothing.
    pub fn update(&mut self, json: &str) -> Result<JsonViewRunReport, String> {
        self.update_with(json, &create_control_from_factory)
    }

    /// [`Self::update`] with an injected control factory.
    pub fn update_with(
        &mut self,
        json: &str,
        create: &dyn Fn(&Node) -> Option<ObjectId>,
    ) -> Result<JsonViewRunReport, String> {
        let tree = json_to_view_tree(json)?;
        let was_mounted = self.engine.current().is_some();
        let diff = self.engine.update_tree_shared(Rc::new(tree.root), create);
        let removed_ids = diff.removed.clone();
        Ok(JsonViewRunReport {
            nodes_declared: tree.nodes,
            dropped: tree.dropped,
            diff,
            removed_ids,
            updated: was_mounted,
            mount_report: None,
        })
    }

    /// Mounts when nothing is mounted, otherwise updates — the reload entry an edit loop wants.
    pub fn reload(&mut self, json: &str) -> Result<JsonViewRunReport, String> {
        self.reload_with(json, &create_control_from_factory)
    }

    /// [`Self::reload`] with an injected control factory.
    pub fn reload_with(
        &mut self,
        json: &str,
        create: &dyn Fn(&Node) -> Option<ObjectId>,
    ) -> Result<JsonViewRunReport, String> {
        if self.is_mounted() {
            self.update_with(json, create)
        } else {
            self.mount_with(json, create)
        }
    }
}

impl Default for JsonViewSession {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::{vec, RefCell};

    const DOCUMENT: &str = r#"{
      "window": {
        "id": "root",
        "title": "Session",
        "width": 400,
        "height": 300,
        "layout": {
          "type": "vbox",
          "children": [
            { "label": { "id": "title", "text": "Hello" } },
            { "button": { "id": "go", "text": "Go", "events": { "clicked": "on_save" } } }
          ]
        }
      }
    }"#;

    /// A stub creator, so identity is asserted without a window or a platform backend.
    fn creator() -> impl Fn(&Node) -> Option<ObjectId> {
        let next = RefCell::new(100u64);
        move |_node: &Node| {
            let mut next = next.borrow_mut();
            let id = *next;
            *next += 1;
            Some(id)
        }
    }

    /// RW-43: the conversion keeps the parsed facts a diff needs — keys, text and wire carriers.
    #[test]
    fn a_document_becomes_a_keyed_node_tree() {
        let tree = json_to_view_tree(DOCUMENT).expect("the document parses");
        assert_eq!(tree.nodes, 3, "window + label + button");
        assert_eq!(tree.root.key.as_deref(), Some("root"));
        assert_eq!(tree.root.children.len(), 2);

        let label = &tree.root.children[0];
        assert_eq!(label.key.as_deref(), Some("title"));
        assert_eq!(label.prop_value("text").and_then(|value| value.as_str()), Some("Hello"));

        let button = &tree.root.children[1];
        let wire = button.prop_value("__wire_events.clicked").and_then(|value| value.as_str());
        assert_eq!(wire, Some("on_save"), "a declared wire must travel on the carrier");
        // The geometry shorthand is loader-owned and must be reported, not invented.
        assert!(
            tree.dropped.iter().any(|drop| drop.property == "width"),
            "the root's width must be reported as not carried: {:?}",
            tree.dropped
        );
    }

    /// RW-43: an identical reload touches nothing — the ids are the same controls.
    #[test]
    fn an_unchanged_update_keeps_every_control() {
        let create = creator();
        let mut session = JsonViewSession::new();
        let mounted = session.mount_with(DOCUMENT, &create).expect("the document parses");
        assert!(!mounted.updated);
        let apply = mounted.mount_report.as_ref().expect("a mount reports what it applied");
        assert_eq!(apply.widgets_created, 3, "window + label + button");
        let root_id = session.engine().id_at(&[]).expect("the root is mounted");
        let first_child = session.engine().id_at(&[0]).expect("the label is mounted");

        let updated = session.update_with(DOCUMENT, &create).expect("the document parses");
        assert!(updated.updated);
        assert!(updated.is_unchanged(), "{updated:?}");
        assert!(updated.diff.patches.is_empty());
        assert!(updated.removed_ids.is_empty());
        assert_eq!(session.engine().id_at(&[]), Some(root_id));
        assert_eq!(
            session.engine().id_at(&[0]),
            Some(first_child),
            "an unchanged control must keep its identity (and therefore its state)"
        );
    }

    /// RW-43: a removed node names the control it cost.
    #[test]
    fn a_removal_reports_the_ids_it_destroyed() {
        const EDITED: &str = r#"{
          "window": {
            "id": "root",
            "title": "Session",
            "width": 400,
            "height": 300,
            "layout": {
              "type": "vbox",
              "children": [
                { "label": { "id": "title", "text": "Hello" } }
              ]
            }
          }
        }"#;
        let create = creator();
        let mut session = JsonViewSession::new();
        session.mount_with(DOCUMENT, &create).expect("the document parses");
        let button_id = session.engine().id_at(&[1]).expect("the button is mounted");

        let updated = session.update_with(EDITED, &create).expect("the edit parses");
        assert!(!updated.is_unchanged());
        assert_eq!(updated.removed_ids, vec![button_id]);
        assert!(session.engine().id_at(&[1]).is_none(), "the removed path is gone");
    }

    /// RW-43: a document that does not parse is refused, and the mounted tree is left as it was.
    #[test]
    fn a_malformed_update_is_refused_without_unmounting() {
        let create = creator();
        let mut session = JsonViewSession::new();
        session.mount_with(DOCUMENT, &create).expect("the document parses");
        let root_id = session.engine().id_at(&[]).expect("the root is mounted");
        assert!(session.update_with("{ not json", &create).is_err());
        assert_eq!(
            session.engine().id_at(&[]),
            Some(root_id),
            "a refused edit must not disturb the mounted tree"
        );
    }

    /// RW-43: `reload` picks the right door for the session's state.
    #[test]
    fn reload_mounts_then_updates() {
        let create = creator();
        let mut session = JsonViewSession::new();
        let first = session.reload_with(DOCUMENT, &create).expect("the document parses");
        assert!(!first.updated);
        let second = session.reload_with(DOCUMENT, &create).expect("the document parses");
        assert!(second.updated);
        assert!(second.is_unchanged());
    }

    /// RW-43: identity is the document's `id`, so inserting a sibling does not renumber the
    /// controls after it. Every unchanged control keeps its id — that is what preserves focus
    /// and scroll across an edit.
    #[test]
    fn inserting_a_sibling_keeps_the_other_controls_identity() {
        const INSERTED: &str = r#"{
          "window": {
            "id": "root",
            "title": "Session",
            "width": 400,
            "height": 300,
            "layout": {
              "type": "vbox",
              "children": [
                { "label": { "id": "note", "text": "New" } },
                { "label": { "id": "title", "text": "Hello" } },
                { "button": { "id": "go", "text": "Go", "events": { "clicked": "on_save" } } }
              ]
            }
          }
        }"#;
        let create = creator();
        let mut session = JsonViewSession::new();
        session.mount_with(DOCUMENT, &create).expect("the document parses");
        let root_id = session.engine().id_at(&[]).expect("root is mounted");
        let label_id = session.engine().id_at(&[0]).expect("label is mounted");
        let button_id = session.engine().id_at(&[1]).expect("button is mounted");

        let updated = session.update_with(INSERTED, &create).expect("the edit parses");
        assert!(updated.removed_ids.is_empty(), "nothing was removed: {updated:?}");
        assert_eq!(session.engine().id_at(&[]), Some(root_id));
        assert_eq!(
            session.engine().id_at(&[1]),
            Some(label_id),
            "the label moved down but kept its control"
        );
        assert_eq!(
            session.engine().id_at(&[2]),
            Some(button_id),
            "the button kept its control despite the inserted sibling"
        );
    }
}
