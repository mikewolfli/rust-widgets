// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The JSON loader's **incremental** path (G-406 / RW-43 / RW-117).
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
//! Three declarative facts are carried **as structure** rather than as properties, because they
//! describe how the tree is arranged rather than what one control holds:
//!
//! * the per-container `layout` declaration, retained in [`JsonViewLayout`] and installed into the
//!   library's one layout registry ([`crate::layout::declarative`]) after every mount/update;
//! * the per-node `breakpoints` list, which selects whether the node exists in the tier
//!   [`crate::view::Breakpoint::current`] names;
//! * a child's layout attributes (`stretch`, `col`/`row`, `region`, …), retained with the
//!   declaration so the solve sees the same placement the full loader would.
//!
//! Wires *are* carried, under the same `__wire_` carrier a generated program uses, and bound by
//! the library's own [`bind_published_event`](crate::json::bind_published_event) /
//! [`bind_marker_event`](crate::json::bind_marker_event). A wire therefore resolves exactly as it
//! does on the load path, with no second rule table here.
//!
//! # Viewport and re-solve
//!
//! [`JsonViewSession::set_viewport_rect`] is the entry a paint path calls: it reports the rect to
//! [`ViewEngine::set_viewport`] (which selects the breakpoint tier) **and** re-runs the installed
//! layouts against that rect, so a canvas resized between two paints draws controls at the new
//! geometry rather than the rectangles they were created with (RW-118). A tier change can select
//! a different breakpoint subtree, which needs the document again; call
//! [`JsonViewSession::reload`] with the same text after a `set_viewport_rect` whose return value
//! indicates the tier moved.
//!
//! # Reachability
//!
//! **State:** Enabling API. The host's edit-reload loop is its consumer; this repository's tests
//! exercise it end to end (`mount` → unchanged `update` → `update` that removes a node) and its
//! layout/breakpoint paths. The existing full-mount loader path is untouched: this module adds a
//! session around the same parser and creates controls through the same factory.

use crate::compat::{format, Rc, String, Vec};
use crate::core::{ObjectId, Rect};
use crate::json::project::{JsonProject, ProjectNode};
use crate::view::{Breakpoint, DiffReport, Node, ViewEngine};

/// The document key that declares a node's visible size tiers.
///
/// Spelled once so the conversion and the diagnostic cannot disagree about which key is structure
/// rather than a property.
const BREAKPOINTS_KEY: &str = "breakpoints";

/// One child the owning layout places.
///
/// `index` addresses the owner's **converted** child list, which is what the view engine's paths
/// and [`JsonViewSession::engine`]'s `id_at` use. It is deliberately not the project parser's path:
/// a breakpoint-excluded sibling or a flattened `layout` node shifts the view child index, and
/// mixing the two numberings is how a layout would place the wrong control.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonViewLayoutChild {
    /// Index in the owning node's converted child list.
    pub index: usize,
    /// Whether this declaration is the `spacer` pseudo-widget (which owns no control).
    pub spacer: bool,
    /// The child's declared layout attributes, read by the one JSON reader.
    pub attrs: crate::json::ChildLayoutAttrs,
}

/// One declarative layout a document declares, retained after conversion.
///
/// The full loader stores a layout and forgets the declaration; a preview that cannot answer
/// "what layout did the document ask for?" makes a host re-parse the document to find out. This
/// value is the queryable form: it names the owning view path, the kind and the placed children.
#[derive(Debug, Clone, PartialEq)]
pub struct JsonViewLayout {
    /// The view path of the control that owns the layout (`[]` = the root).
    pub path: Vec<usize>,
    /// The owner's node key, so a panel can name the control without resolving the path.
    pub key: String,
    /// The declared kind and its parameters.
    pub kind: crate::json::DeclarativeLayoutKind,
    /// The owning control's declared rectangle, when the document states a usable one.
    ///
    /// The runtime loader solves a nested layout against the owner's `x`/`y`/`width`/`height`.
    /// The incremental path does not write those keys as properties, so it keeps the rectangle
    /// here: a layout whose owner no enclosing layout places would otherwise be solved against
    /// the zero creation rect and its children would never move (RW-117).
    pub declared_rect: Option<Rect>,
    /// The children the layout places, in solve order (spacers included).
    pub children: Vec<JsonViewLayoutChild>,
}

/// What one viewport report did (RW-118).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct JsonViewportApply {
    /// Whether the reported rect moved the classification to a **different** breakpoint tier.
    ///
    /// A caller that sees `true` must reload/update with the document text to re-select the
    /// subtree: a tier change can add or remove nodes, which a geometry solve alone cannot do.
    pub tier_changed: bool,
    /// The child rectangles the solve wrote, deduped by control id.
    pub geometry: Vec<(ObjectId, Rect)>,
}

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
    /// Every layout declaration the document carries, in document order.
    pub layouts: Vec<JsonViewLayout>,
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
/// [`JsonViewTree::dropped`] rather than failing the whole preview. The one structural failure is
/// a root node excluded by its own `breakpoints` list, which names the reason.
pub fn json_to_view_tree(json: &str) -> Result<JsonViewTree, String> {
    let project = JsonProject::parse(json)?;
    let mut conversion = Conversion {
        project: &project,
        factory: crate::widget::capability::WidgetFactory::shared(),
        dropped: Vec::new(),
        layouts: Vec::new(),
    };
    let root =
        project.root().ok_or_else(|| String::from("the parsed document has no root node"))?;
    let converted = conversion.convert_node(root, &[]).ok_or_else(|| {
        String::from(
            "the root node is excluded by its own `breakpoints` list; a preview needs a root",
        )
    })?;
    Ok(JsonViewTree {
        root: converted,
        nodes: project.len(),
        dropped: conversion.dropped,
        layouts: conversion.layouts,
    })
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

/// The conversion's per-document state.
///
/// Bundled so the recursive walk has one place to record a diagnostic or a layout declaration,
/// instead of threading two `&mut Vec`s through every call.
struct Conversion<'a> {
    project: &'a JsonProject,
    factory: &'a crate::widget::capability::WidgetFactory,
    dropped: Vec<JsonViewDrop>,
    layouts: Vec<JsonViewLayout>,
}

impl Conversion<'_> {
    /// Whether the node exists in the breakpoint tier currently in force.
    ///
    /// An absent list means "every tier", which is the pre-existing behaviour and the honest
    /// reading of a document that says nothing. A malformed list is reported and treated as
    /// absent — removing a node because its tier spelling was mistyped would be a silent edit.
    /// An unknown tier token is reported and ignored; when no known token matches the current
    /// tier the node is excluded, which is exactly what the declared list asks for.
    fn in_breakpoints(&mut self, node: &ProjectNode, key: &str) -> bool {
        let Some(value) = node.property(BREAKPOINTS_KEY) else {
            return true;
        };
        let Some(tiers) = value.as_array() else {
            self.dropped.push(JsonViewDrop {
                path: node.path.clone(),
                key: String::from(key),
                property: String::from(BREAKPOINTS_KEY),
                reason: String::from(
                    "`breakpoints` must be an array of tier names (compact, medium, expanded); \
                     the node was kept in every tier",
                ),
            });
            return true;
        };
        let current = Breakpoint::current();
        let mut included = false;
        for entry in tiers {
            let Some(token) = entry.as_str() else {
                self.dropped.push(JsonViewDrop {
                    path: node.path.clone(),
                    key: String::from(key),
                    property: String::from(BREAKPOINTS_KEY),
                    reason: String::from("a `breakpoints` entry is not a string; it was ignored"),
                });
                continue;
            };
            match Breakpoint::from_token(token) {
                Some(tier) => {
                    if tier == current {
                        included = true;
                    }
                }
                None => self.dropped.push(JsonViewDrop {
                    path: node.path.clone(),
                    key: String::from(key),
                    property: String::from(BREAKPOINTS_KEY),
                    reason: format!(
                        "`{token}` is not a breakpoint tier (expected compact, medium or expanded)"
                    ),
                }),
            }
        }
        included
    }

    /// Records a layout declaration, replacing one already recorded for the same path.
    ///
    /// The full loader's order is: direct children first, then an inline `layout` declaration
    /// (`store_layout` replaces, so the inline one wins). This mirror of that order is what makes
    /// a document with both spellings render the same in the preview and at runtime.
    fn push_layout(&mut self, layout: JsonViewLayout) {
        if let Some(existing) = self.layouts.iter_mut().find(|entry| entry.path == layout.path) {
            *existing = layout;
        } else {
            self.layouts.push(layout);
        }
    }

    /// Converts one parsed node and its subtree.
    fn convert_node(&mut self, node: &ProjectNode, path: &[usize]) -> Option<Node> {
        // The document's `id` is the designer's identity and survives a sibling insertion; the
        // path-shaped key is the fallback for a document that declared none. This is the same
        // choice the runtime loader makes (it records a node by its `id`).
        let node_key = node.id.clone().unwrap_or_else(|| node.key.clone());
        if !self.in_breakpoints(node, &node_key) {
            return None;
        }
        // A `spacer` is the transparent gap pseudo-widget: it owns no control and its `stretch`
        // travels through the layout declaration below, not through the property contract. Asking
        // the factory for a `spacer` property would report a drop for a key the document is
        // entitled to write.
        if is_spacer(node) {
            return Some(Node::new(node.widget.clone()).key(node_key));
        }
        let mut converted = Node::new(node.widget.clone()).key(node_key.clone());

        for (name, value) in &node.properties {
            // The layout declaration is structure (its children are emitted as this node's
            // children below), not a property of any control.
            if name == "layout" || name == BREAKPOINTS_KEY {
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
            if crate::json::is_loader_owned_key(name) && !matches!(name.as_str(), "text" | "title")
            {
                self.dropped.push(JsonViewDrop {
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
            let Some(schema) = self.factory.property_schema(&node.widget, name).ok() else {
                self.dropped.push(JsonViewDrop {
                    path: node.path.clone(),
                    key: node_key.clone(),
                    property: name.clone(),
                    reason: format!("`{}` is not a published property of `{}`", name, node.widget),
                });
                continue;
            };
            if !schema.writable {
                self.dropped.push(JsonViewDrop {
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
                None => self.dropped.push(JsonViewDrop {
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

        // A node's inline `layout` places the children under `layout.children`; the remaining
        // direct children keep their own geometry. The project parser emits both groups in that
        // order, so the first `direct_count` children are not layout members.
        // The project parser records which accepted children came from the direct `children`
        // array; the rest belong to the inline layout. Reading that count, rather than
        // subtracting the declared length of `layout.children`, keeps a rejected child from
        // shifting a direct sibling into the layout.
        let direct_count = node.direct_children;
        let inline_layout = node.property("layout").cloned();
        let mut inline_children: Vec<JsonViewLayoutChild> = Vec::new();
        let mut pseudo_layout: Option<JsonViewLayout> = None;
        let mut view_index = 0usize;

        for index in 0..node.children.len() {
            let Some(child) = self.project.child_of(node, index) else {
                continue;
            };
            let child_key = child.id.clone().unwrap_or_else(|| child.key.clone());

            // `{"layout": { "type": …, "children": [..] }}` is a transparent node in the runtime
            // loader: it creates no control, stores the layout on the *parent*, and hosts its
            // children there. Keeping it as a `Node` would ask the factory to build a control
            // called "layout" (`UnknownWidgetType`) and lose the arrangement.
            if child.widget.eq_ignore_ascii_case("layout") {
                let kind = self.pseudo_layout_kind(child, &child_key);
                let mut placed: Vec<JsonViewLayoutChild> = Vec::new();
                for child_index in 0..child.children.len() {
                    let Some(grandchild) = self.project.child_of(child, child_index) else {
                        continue;
                    };
                    let mut child_path = path.to_vec();
                    child_path.push(view_index);
                    if let Some(converted_child) = self.convert_node(grandchild, &child_path) {
                        placed.push(JsonViewLayoutChild {
                            index: view_index,
                            spacer: is_spacer(grandchild),
                            attrs: child_layout_attrs(grandchild),
                        });
                        converted = converted.child(converted_child);
                        view_index += 1;
                    }
                }
                if let Some(kind) = kind {
                    pseudo_layout = Some(JsonViewLayout {
                        path: path.to_vec(),
                        key: node_key.clone(),
                        kind,
                        declared_rect: declared_rect(node),
                        children: placed,
                    });
                }
                continue;
            }

            let mut child_path = path.to_vec();
            child_path.push(view_index);
            let child_is_managed = index >= direct_count;
            if let Some(converted_child) = self.convert_node(child, &child_path) {
                if child_is_managed {
                    inline_children.push(JsonViewLayoutChild {
                        index: view_index,
                        spacer: is_spacer(child),
                        attrs: child_layout_attrs(child),
                    });
                }
                converted = converted.child(converted_child);
                view_index += 1;
            }
        }

        // Direct children ran first (they may include a pseudo `layout` node) and the inline
        // declaration is stored last, matching the loader's `store_layout` order.
        if let Some(layout) = pseudo_layout {
            self.push_layout(layout);
        }
        if let Some(value) = inline_layout {
            match crate::json::parse_layout_kind(&value) {
                Ok(kind) => self.push_layout(JsonViewLayout {
                    path: path.to_vec(),
                    key: node_key,
                    kind,
                    declared_rect: declared_rect(node),
                    children: inline_children,
                }),
                Err(reason) => self.dropped.push(JsonViewDrop {
                    path: node.path.clone(),
                    key: node_key,
                    property: String::from("layout"),
                    reason: format!("the `layout` declaration could not be read: {reason}"),
                }),
            }
        }

        Some(converted)
    }

    /// The kind of a `{"layout": …}` pseudo-node, or `None` with a diagnostic.
    ///
    /// The project parser keeps the layout object's scalar keys in `properties` and its children
    /// in `children`; rebuilding the object is how the parser's one kind table
    /// ([`crate::json::parse_layout_kind`]) reads it.
    fn pseudo_layout_kind(
        &mut self,
        node: &ProjectNode,
        key: &str,
    ) -> Option<crate::json::DeclarativeLayoutKind> {
        let value = scalar_object(&node.properties);
        match crate::json::parse_layout_kind(&value) {
            Ok(kind) => Some(kind),
            Err(reason) => {
                self.dropped.push(JsonViewDrop {
                    path: node.path.clone(),
                    key: String::from(key),
                    property: String::from("layout"),
                    reason: format!("the `layout` node could not be read: {reason}"),
                });
                None
            }
        }
    }
}

/// The child's view path: its owner's path plus its index in the converted child list.
fn child_path(parent: &[usize], index: usize) -> Vec<usize> {
    let mut path = parent.to_vec();
    path.push(index);
    path
}

/// Whether a project node is the transparent gap pseudo-widget.
fn is_spacer(node: &ProjectNode) -> bool {
    node.widget.eq_ignore_ascii_case("spacer")
}

/// A project node's scalar properties as a JSON object.
///
/// Layout parameters and child attributes are all scalars; an array or object key is structure
/// (`children`, `items`) and must not travel into either reader.
fn scalar_object(properties: &[(String, serde_json::Value)]) -> serde_json::Value {
    let mut object = serde_json::Map::new();
    for (key, value) in properties {
        if matches!(value, serde_json::Value::Array(_) | serde_json::Value::Object(_)) {
            continue;
        }
        object.insert(key.clone(), value.clone());
    }
    serde_json::Value::Object(object)
}

/// The layout attributes a child declares, read by the loader's own parser.
fn child_layout_attrs(node: &ProjectNode) -> crate::json::ChildLayoutAttrs {
    crate::json::ChildLayoutAttrs::from_value(&scalar_object(&node.properties))
}

/// A control's declared rectangle, when the document states a usable one.
///
/// The loader writes these keys through its geometry route; the incremental path does not, so this
/// is where a nested layout owner's solve rectangle comes from.
fn declared_rect(node: &ProjectNode) -> Option<Rect> {
    let axis = |name: &str| {
        node.property(name)
            .and_then(|value| value.as_i64())
            .and_then(|value| i32::try_from(value).ok())
            .unwrap_or(0)
    };
    let extent = |name: &str| {
        node.property(name)
            .and_then(|value| value.as_u64())
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or(0)
    };
    let width = extent("width");
    let height = extent("height");
    (width > 0 && height > 0).then(|| Rect::new(axis("x"), axis("y"), width, height))
}

/// The node at `path` in a converted tree.
fn node_at<'a>(root: &'a Node, path: &[usize]) -> Option<&'a Node> {
    let mut node = root;
    for index in path {
        node = node.children.get(*index)?;
    }
    Some(node)
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
    /// The layout declarations the mounted document carries, in document order.
    layouts: Vec<JsonViewLayout>,
    /// The rect the host last reported, if it has.
    viewport_rect: Option<Rect>,
    /// The rectangles the last solve produced, in solve order.
    geometry: Vec<(ObjectId, Rect)>,
    /// What is currently installed in the thread-local layout registry.
    installed: Vec<InstalledLayout>,
}

/// One layout this session stored in the registry, with the identity of its members.
///
/// The signature is what lets an unchanged update keep the layout manager — and with it a
/// splitter's handle position — instead of replacing it on every keystroke in the document.
struct InstalledLayout {
    layout: JsonViewLayout,
    parent: ObjectId,
    child_ids: Vec<ObjectId>,
}

impl JsonViewSession {
    /// Creates a session with nothing mounted.
    pub fn new() -> Self {
        Self {
            engine: ViewEngine::new(),
            layouts: Vec::new(),
            viewport_rect: None,
            geometry: Vec::new(),
            installed: Vec::new(),
        }
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

    /// The layout declarations the mounted document carries (RW-117).
    ///
    /// This is the queryable form of what the full loader stores and forgets: a host can ask
    /// which container has which layout without re-parsing the document.
    pub fn layouts(&self) -> &[JsonViewLayout] {
        &self.layouts
    }

    /// The rect the host last reported through [`Self::set_viewport_rect`].
    pub fn viewport_rect(&self) -> Option<Rect> {
        self.viewport_rect
    }

    /// The rectangles the last solve wrote onto the mounted controls (RW-118).
    ///
    /// Empty before a viewport rect is known or when the document declares no layout.
    pub fn layout_geometry(&self) -> &[(ObjectId, Rect)] {
        &self.geometry
    }

    /// Reports the surface rect a paint pass is about to draw into and re-solves every installed
    /// layout against it (RW-118).
    ///
    /// The rect also updates the breakpoint tier [`ViewEngine::set_viewport`] classifies against.
    /// [`JsonViewportApply::tier_changed`] is the key part of the answer: a tier change can select
    /// a different breakpoint subtree, so a caller that needs the subtree re-selected calls
    /// [`Self::reload`] with the same document afterwards. The returned geometry is what the solve
    /// just wrote, available even when no tier moved.
    pub fn set_viewport_rect(&mut self, rect: Rect) -> JsonViewportApply {
        let tier_changed = self.engine.set_viewport(rect.size());
        self.viewport_rect = Some(rect);
        self.solve_layouts();
        JsonViewportApply { tier_changed, geometry: self.geometry.clone() }
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
        let tree = self.convert(json)?;
        let nodes_declared = tree.nodes;
        let dropped = tree.dropped;
        // Mounting is authoritative; a previous tree's ids are torn down by the engine and are
        // not an edit's removals, so the report names no removed ids here.
        self.forget_installed_layouts();
        let apply = self.engine.mount_tree_shared(Rc::new(tree.root), create);
        self.layouts = tree.layouts;
        self.install_layouts();
        self.refresh_geometry();
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
        let tree = self.convert(json)?;
        let was_mounted = self.engine.current().is_some();
        let diff = self.engine.update_tree_shared(Rc::new(tree.root), create);
        let removed_ids = diff.removed.clone();
        self.layouts = tree.layouts;
        self.install_layouts();
        self.refresh_geometry();
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

    /// Converts `json` in the viewport's breakpoint tier.
    ///
    /// A `Node::breakpoint` branch and this module's `breakpoints` list both read
    /// [`Breakpoint::current`], so the tier has to be in force *during* conversion. The scope
    /// restores the previous tier however conversion exits.
    fn convert(&self, json: &str) -> Result<JsonViewTree, String> {
        let tier = self
            .engine
            .viewport()
            .map(Breakpoint::of)
            .or_else(|| self.viewport_rect.map(|rect| Breakpoint::of(rect.size())))
            .unwrap_or_else(Breakpoint::current);
        crate::view::with_breakpoint(tier, || json_to_view_tree(json))
    }

    /// Forgets every layout this session stored, before a mount replaces the controls.
    fn forget_installed_layouts(&mut self) {
        for entry in self.installed.drain(..) {
            crate::layout::declarative::forget_layout(entry.parent);
        }
    }

    /// The ids of a declaration's placed children, in solve order (`SPACER_ID` for gaps).
    ///
    /// A child the engine could not create has no id (`0`). It stays in the signature so a later
    /// update that creates it is seen as a change.
    fn layout_child_ids(&self, layout: &JsonViewLayout) -> Vec<ObjectId> {
        layout
            .children
            .iter()
            .map(|child| {
                if child.spacer {
                    crate::layout::declarative::SPACER_ID
                } else {
                    self.engine.id_at(&child_path(&layout.path, child.index)).unwrap_or(0)
                }
            })
            .collect()
    }

    /// The widget name of a placed child, for a placement warning.
    fn child_widget_name(&self, parent: &[usize], index: usize) -> String {
        self.engine
            .current()
            .and_then(|root| node_at(root, parent))
            .and_then(|node| node.children.get(index))
            .map(|node| node.widget.clone())
            .unwrap_or_default()
    }

    /// Installs or refreshes every declared layout, keeping unchanged managers.
    fn install_layouts(&mut self) {
        let declarations = self.layouts.clone();
        let mut installed: Vec<InstalledLayout> = Vec::with_capacity(declarations.len());
        for layout in declarations {
            let Some(parent) = self.engine.id_at(&layout.path) else {
                // The owning control was not created (a refused construction), so there is no
                // parent to place children in. The mount report already names that refusal.
                continue;
            };
            let child_ids = self.layout_child_ids(&layout);
            let unchanged = self.installed.iter().any(|entry| {
                entry.layout.path == layout.path
                    && entry.parent == parent
                    && entry.layout == layout
                    && entry.child_ids == child_ids
            });
            if unchanged {
                installed.push(InstalledLayout { layout, parent, child_ids });
                continue;
            }
            // A path can address a different parent after a structural edit; the old parent's
            // registry entry would otherwise keep placing ids that are gone.
            if let Some(previous) =
                self.installed.iter().find(|entry| entry.layout.path == layout.path)
            {
                if previous.parent != parent {
                    crate::layout::declarative::forget_layout(previous.parent);
                }
            }
            let manager = crate::json::create_layout_from_kind(&layout.kind);
            crate::layout::declarative::store_layout(parent, manager);
            for child in &layout.children {
                if child.spacer {
                    crate::layout::declarative::add_spacer_to_layout(child.attrs.stretch, parent);
                    continue;
                }
                let Some(child_id) = self.engine.id_at(&child_path(&layout.path, child.index))
                else {
                    continue;
                };
                if child_id == 0 {
                    continue;
                }
                crate::layout::declarative::add_widget_to_layout(
                    child_id,
                    child.attrs.stretch,
                    parent,
                );
                let child_type = self.child_widget_name(&layout.path, child.index);
                crate::json::layout::apply_child_layout_placement(
                    child_id,
                    &child.attrs,
                    &child_type,
                    parent,
                );
            }
            installed.push(InstalledLayout { layout, parent, child_ids });
        }
        // A declaration that disappeared must not leave its manager behind.
        for previous in &self.installed {
            let still_installed = installed.iter().any(|entry| {
                entry.layout.path == previous.layout.path && entry.parent == previous.parent
            });
            if !still_installed {
                crate::layout::declarative::forget_layout(previous.parent);
            }
        }
        self.installed = installed;
    }

    /// Re-solves every declared layout against the reported rect.
    ///
    /// Every declaration is solved, in document order, and the resulting rectangles are deduped by
    /// `ObjectId` (last write wins). Solving only "topmost" declarations was wrong for a document
    /// that mixes direct `children` with `layout.children`: a direct child is not a member of the
    /// parent's layout, so `apply_layout`'s recursion never visits it and its own layout would
    /// never be solved. Deduping keeps the returned list one entry per control.
    fn solve_layouts(&mut self) {
        let Some(rect) = self.viewport_rect else {
            self.geometry.clear();
            return;
        };
        let declarations: Vec<(Vec<usize>, Option<Rect>)> =
            self.layouts.iter().map(|layout| (layout.path.clone(), layout.declared_rect)).collect();
        let mut geometry: Vec<(ObjectId, Rect)> = Vec::new();
        for (path, declared) in declarations {
            let Some(parent) = self.engine.id_at(&path) else {
                continue;
            };
            let solve_rect = if path.is_empty() {
                rect
            } else {
                // A nested layout is solved against the rectangle its control currently holds —
                // written by an enclosing layout if one placed it, otherwise the document's own
                // declared rectangle. Falling back to the zero creation rect is the honest last
                // answer: the document did not state a size either.
                crate::widget::runtime::geometry_of(parent)
                    .filter(|rect| rect.width > 0 && rect.height > 0)
                    .or(declared)
                    .unwrap_or(Rect::new(0, 0, 0, 0))
            };
            for (child, child_rect) in crate::layout::declarative::apply_layout(parent, solve_rect)
            {
                if let Some(existing) = geometry.iter_mut().find(|(id, _)| *id == child) {
                    existing.1 = child_rect;
                } else {
                    geometry.push((child, child_rect));
                }
            }
        }
        self.geometry = geometry;
    }

    /// Solves immediately when a rect is known, including one a host reported on the engine.
    fn refresh_geometry(&mut self) {
        // A host may report a size through `engine_mut().set_viewport` instead of
        // `set_viewport_rect`; the geometry has to follow the size the engine is actually
        // classifying against, not a stale rect from the last paint call.
        if let Some(size) = self.engine.viewport() {
            let recorded = self.viewport_rect.map(|rect| rect.size());
            if recorded != Some(size) {
                self.viewport_rect = Some(Rect::new(0, 0, size.width, size.height));
            }
        }
        self.solve_layouts();
    }
}

impl Default for JsonViewSession {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for JsonViewSession {
    /// Releases the layouts this session stored.
    ///
    /// The registry is thread-local and keyed by parent id; without this, every dropped preview
    /// would leave managers addressing ids the runtime may later reuse.
    fn drop(&mut self) {
        self.forget_installed_layouts();
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

    /// RW-117: the root `layout` is retained as queryable state and the children are solved
    /// against the host's viewport rect — not left at their creation rectangles.
    #[test]
    fn the_root_layout_is_queryable_and_solves_against_the_viewport() {
        let create = creator();
        let mut session = JsonViewSession::new();
        session.mount_with(DOCUMENT, &create).expect("the document parses");
        assert_eq!(session.layouts().len(), 1, "one root layout: {:?}", session.layouts());
        let layout = &session.layouts()[0];
        assert_eq!(layout.path, Vec::<usize>::new());
        assert_eq!(layout.key, "root");
        assert_eq!(layout.children.len(), 2);

        let applied = session.set_viewport_rect(Rect::new(0, 0, 400, 300));
        let geometry = &applied.geometry;
        assert_eq!(geometry.len(), 2, "the two placed children: {geometry:?}");
        assert_eq!(session.viewport_rect(), Some(Rect::new(0, 0, 400, 300)));
        // A vertical box stacks children; the second one starts below the first.
        assert_eq!(geometry[0].1.y, 0);
        assert!(geometry[1].1.y > geometry[0].1.y, "{geometry:?}");
        assert_eq!(geometry[0].1.width, 400, "the solve uses the reported width: {geometry:?}");
        assert_eq!(
            session.layout_geometry().len(),
            2,
            "the last solve is queryable independently of the returned value"
        );
    }

    /// RW-117: a `{"layout": …}` pseudo-node is transparent, as it is on the runtime loader.
    /// Keeping it would mean asking the factory for a control called "layout".
    #[test]
    fn a_layout_pseudo_node_is_flattened_into_its_parent() {
        const NESTED: &str = r#"{
          "window": {
            "id": "root",
            "title": "Nested",
            "children": [
              { "layout": { "type": "hbox", "children": [
                { "button": { "id": "a", "text": "A" } },
                { "button": { "id": "b", "text": "B" } }
              ] } }
            ]
          }
        }"#;
        let tree = json_to_view_tree(NESTED).expect("the document parses");
        assert_eq!(tree.root.children.len(), 2, "the pseudo node creates no control");
        assert_eq!(tree.root.children[0].key.as_deref(), Some("a"));
        assert_eq!(tree.root.children[1].key.as_deref(), Some("b"));
        assert!(
            tree.layouts.iter().any(|layout| layout.kind
                == crate::json::DeclarativeLayoutKind::HBox { spacing: 0, margin: 0 }),
            "the pseudo node's layout must be recorded on its parent: {:?}",
            tree.layouts
        );
    }

    /// RW-117: a `breakpoints` list selects whether the node exists in the current tier, and an
    /// unknown tier is named rather than silently ignored.
    #[test]
    fn a_breakpoints_list_selects_the_node_and_names_an_unknown_tier() {
        const COMPACT_ONLY: &str = r#"{
          "window": {
            "id": "root",
            "title": "Tiers",
            "layout": { "type": "vbox", "children": [
              { "label": { "id": "wide", "text": "Wide", "breakpoints": ["expanded"] } },
              { "label": { "id": "narrow", "text": "Narrow", "breakpoints": ["compact", "tablet"] } }
            ] }
          }
        }"#;
        let compact =
            crate::view::with_breakpoint(Breakpoint::Compact, || json_to_view_tree(COMPACT_ONLY))
                .expect("the document parses");
        assert_eq!(compact.root.children.len(), 1);
        assert_eq!(compact.root.children[0].key.as_deref(), Some("narrow"));
        assert!(
            compact
                .dropped
                .iter()
                .any(|drop| drop.property == BREAKPOINTS_KEY && drop.reason.contains("tablet")),
            "the unknown tier must be named: {:?}",
            compact.dropped
        );

        let expanded =
            crate::view::with_breakpoint(Breakpoint::Expanded, || json_to_view_tree(COMPACT_ONLY))
                .expect("the document parses");
        assert_eq!(expanded.root.children.len(), 1);
        assert_eq!(expanded.root.children[0].key.as_deref(), Some("wide"));
    }

    /// RW-117: `set_viewport_rect` moves the tier the next conversion sees, so an update after a
    /// resize selects the same subtree the paint path will draw.
    #[test]
    fn the_reported_viewport_tier_selects_breakpoint_nodes_on_update() {
        const TIERED: &str = r#"{
          "window": {
            "id": "root",
            "title": "Tiers",
            "layout": { "type": "vbox", "children": [
              { "label": { "id": "wide", "text": "Wide", "breakpoints": ["expanded"] } },
              { "label": { "id": "narrow", "text": "Narrow", "breakpoints": ["compact"] } }
            ] }
          }
        }"#;
        let create = creator();
        let mut session = JsonViewSession::new();
        session.mount_with(TIERED, &create).expect("the document parses");
        // No viewport yet: the honest default is Expanded, so only `wide` exists.
        assert_eq!(session.current().expect("mounted").children.len(), 1);

        session.set_viewport_rect(Rect::new(0, 0, 320, 640));
        let report = session.update_with(TIERED, &create).expect("the document parses");
        assert!(!report.removed_ids.is_empty(), "the expanded-only label is gone: {report:?}");
        let mounted = session.current().expect("mounted");
        assert_eq!(mounted.children.len(), 1);
        assert_eq!(mounted.children[0].key.as_deref(), Some("narrow"));
    }

    /// RW-117: a `spacer` keeps its slot in the layout declaration but creates no control, exactly
    /// as the runtime loader's `spacer` branch does.
    #[test]
    fn a_spacer_occupies_a_layout_slot_without_creating_a_control() {
        const WITH_SPACER: &str = r#"{
          "window": {
            "id": "root",
            "title": "Gap",
            "layout": { "type": "vbox", "children": [
              { "button": { "id": "a", "text": "A" } },
              { "spacer": { "stretch": 1 } },
              { "button": { "id": "b", "text": "B" } }
            ] }
          }
        }"#;
        let create = creator();
        let mut session = JsonViewSession::new();
        let report = session.mount_with(WITH_SPACER, &create).expect("the document parses");
        let apply = report.mount_report.as_ref().expect("a mount reports what it applied");
        assert_eq!(apply.widgets_created, 3, "window + two buttons; the spacer owns no control");
        assert_eq!(session.layouts()[0].children.len(), 3, "the spacer keeps its slot");
        assert!(session.layouts()[0].children[1].spacer);

        let applied = session.set_viewport_rect(Rect::new(0, 0, 200, 120));
        assert_eq!(applied.geometry.len(), 2, "only the two real controls are reported");
    }

    /// RW-117: a nested layout whose owner no enclosing layout places is solved against the
    /// owner's **declared** rectangle, which is what the runtime loader would use.
    #[test]
    fn an_unplaced_nested_layout_uses_its_owner_declared_rect() {
        const NESTED_OWNER: &str = r#"{
          "window": {
            "id": "root",
            "title": "Nested owner",
            "children": [
              { "panel": { "id": "outer", "x": 10, "y": 20, "width": 200, "height": 100,
                "layout": { "type": "vbox", "children": [
                  { "button": { "id": "a", "text": "A" } },
                  { "button": { "id": "b", "text": "B" } }
                ] } } }
            ]
          }
        }"#;
        let create = creator();
        let mut session = JsonViewSession::new();
        session.mount_with(NESTED_OWNER, &create).expect("the document parses");
        let layout = session.layouts().first().expect("the panel carries the layout");
        assert_eq!(layout.path, crate::compat::vec![0]);
        assert_eq!(layout.declared_rect, Some(Rect::new(10, 20, 200, 100)));

        let applied = session.set_viewport_rect(Rect::new(0, 0, 400, 300));
        assert_eq!(applied.geometry.len(), 2, "two buttons: {:?}", applied.geometry);
        assert_eq!(applied.geometry[0].1.width, 200, "the owner's declared width is used");
        assert_eq!(applied.geometry[0].1.x, 10, "and its declared origin");
        assert_eq!(applied.geometry[0].1.y, 20);
    }
}
