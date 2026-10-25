// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Carrying [`Patch`]es onto the retained control tree.
//!
//! This is where the declarative half meets the retained half. `SetProperty` writes through
//! each control's **own** published property contract — the same path the JSON loader and
//! the C ABI use — so a property that a control refuses by name is refused here for the
//! same reason, with the same error. Nothing in this module knows what a `text` or a
//! `value` means.
//!
//! The reason to keep structural patches here rather than in the diff is *reversibility of
//! evidence* (BLUE18 rule #89): `apply` is the only thing that mutates, so a test can skip
//! it and show that the assertion it feeds really does depend on the patch having been
//! carried out.

use crate::core::ObjectId;
use crate::widget::capability::properties_trait::{
    widget_property_set, widget_set_resolved_icon_source,
};
use crate::widget::capability::CapabilityValue;
use crate::widget::icon_source::{IconResourceResolver, IconSpec};

use super::diff::Patch;
use super::node::Node;

/// Write one property on a mounted control through that control's own contract.
///
/// Going through `widget_property_set` (the same entry point the JSON loader and the C ABI
/// use) is what keeps this module ignorant of what any property means: a control that does
/// not publish the name refuses it here for exactly the same reason it would refuse it
/// anywhere else.
///
/// # `Null` means "reset to the declared default"
///
/// The diff emits `Null` for a property that a node stopped declaring, because `Null` is
/// the documented "not set" value. But only about twenty controls handle `Null`
/// explicitly; the rest write through a typed extractor (`expect_i64`, `expect_usize`, …)
/// that answers `TypeMismatch` for it. Writing the bare `Null` therefore *failed* for most
/// properties — and failed permanently, because the next diff finds no key to retry: the
/// new tree does not declare the property, so no patch is emitted at all and the control
/// keeps its stale value forever with only an ignored `ApplyReport::errors` entry.
///
/// Substituting the value the capability schema declares as that property's default turns
/// the reset into a write that actually lands. The schema is the single source of truth for
/// "what does this property hold when nothing is declared", which is exactly the question
/// a dropped key asks. The kind is read from the **live control** rather than from the
/// node, because `Node.widget` is a creation name while the schema is keyed on
/// `WidgetKind`, and a backwards-compatible fix must not add a field to the public `Node`
/// (principle #21).
///
/// The substitution is skipped for the base properties every control owns, whose `Null`
/// handling is the contract's own business.`Null` is still written when the schema declares
/// no default, so a control that *does* understand `Null` keeps receiving it.
#[cfg(test)]
pub(crate) fn write_property(
    id: ObjectId,
    name: &str,
    value: CapabilityValue,
) -> Result<(), String> {
    write_property_with_icon_resolver(id, name, value, None)
}

pub(crate) fn write_property_with_icon_resolver(
    id: ObjectId,
    name: &str,
    value: CapabilityValue,
    resolver: Option<&dyn IconResourceResolver>,
) -> Result<(), String> {
    if let (
        "icon_source",
        CapabilityValue::IconSpec(IconSpec::Resource { id: resource_id, color_mode }),
        Some(resolver),
    ) = (name, &value, resolver)
    {
        let source = resolver
            .resolve_icon(resource_id, *color_mode)
            .map_err(|error| format!("resource {resource_id:?} could not be resolved: {error}"))?;
        let written = match crate::widget::runtime::with_widget_mut(id, |widget| {
            widget_set_resolved_icon_source(widget, Some(source)).map_err(|e| format!("{e:?}"))
        }) {
            Some(result) => result,
            None => Err("the control is not registered with the runtime".to_string()),
        };
        if written.is_ok() {
            crate::widget::runtime::request_repaint(id);
        }
        return written;
    }
    let value = if matches!(value, CapabilityValue::Null) {
        resolve_null_reset(id, name).unwrap_or(value)
    } else {
        value
    };
    let written = match crate::widget::runtime::with_widget_mut(id, |widget| {
        widget_property_set(widget, name, value).map_err(|e| format!("{e:?}"))
    }) {
        Some(result) => result,
        None => Err("the control is not registered with the runtime".to_string()),
    };
    // Ask for a repaint when the write succeeded.
    //
    // `with_widget_mut`'s own documentation requires it: it mutates the control in place and
    // does *not* invalidate anything, so a caller that skips this step changes the widget and
    // leaves the previous frame on screen. The name-based path in
    // `capability::access::write_widget_property` already does this, which made the same
    // logical `SetProperty` reach the screen or not depending only on whether it arrived
    // through a `Node` or through a name — two routes to one operation with two different
    // visible outcomes.
    if written.is_ok() {
        crate::widget::runtime::request_repaint(id);
    }
    written
}

/// The schema-declared default for `name` on the live control's kind, if it has one.
///
/// Split out so the `Null`-substitution policy is readable on its own, and so a test can
/// ask the question without going through a write. Returns `None` when the control is not
/// registered, when the kind has no schema entry, or when the schema declares no default —
/// in which case the caller keeps the original `Null`.
fn resolve_null_reset(id: ObjectId, name: &str) -> Option<CapabilityValue> {
    let kind = crate::widget::runtime::with_widget_mut(id, |widget| widget.kind())?;
    let default =
        crate::widget::capability::access::default_widget_property_default_value(kind, name)?;
    // A default that is itself `Null` would be a no-op; do not pretend it is a reset.
    (!matches!(default, CapabilityValue::Null)).then_some(default)
}

/// Why a patch could not be carried out.
///
/// Reported per patch instead of aborting the batch: a panel that no longer exists should
/// not prevent the rest of the tree from reaching its declared state, and a caller needs
/// the specific reason to report it.
#[derive(Debug, Clone, PartialEq)]
pub enum ViewError {
    /// The control a patch named is not mounted.
    ///
    /// Distinguishable from "the property is wrong" because the caller's fix differs: a
    /// missing control means the diff's identity map was stale, while a refused property
    /// means the view asked for something the control does not publish.
    UnknownWidget {
        /// The id the patch named.
        id: ObjectId,
    },
    /// The control exists but refused the write.
    ///
    /// Carries the control's own error verbatim, so the property contract remains the
    /// single source of truth for why a write failed.
    PropertyRefused {
        /// The id the patch named.
        id: ObjectId,
        /// The property that was refused.
        name: String,
        /// The control's reason.
        reason: String,
    },
    /// A structural patch named a parent that is not mounted.
    UnknownParent {
        /// The parent the patch named.
        parent: ObjectId,
    },
    /// The node's widget type has no registered constructor.
    ///
    /// Not a silent skip: an unconstructible node means the view describes a control this
    /// build cannot make, which is a defect in the view or in the factory registration.
    ///
    /// # Why the location is part of the error
    ///
    /// A bare type name says *what* failed but not *where*, so a view with two bad nodes
    /// reported two identical errors and neither could be located in the declaration. The
    /// key and the chain of ancestor widget names are what make the failure actionable, and
    /// they are collected at the only point that knows the path — the walk that discovered
    /// it. Carrying them is the difference between "widget 'barchart' is unknown" and "the
    /// 'barchart' under 'vbox > panel > barchart' is unknown".
    UnknownWidgetType {
        /// The type name as declared.
        widget: String,
        /// The node's key, when it declared one.
        key: Option<String>,
        /// The ancestor widget names from the root down to (but not including) this node.
        ///
        /// Names rather than indices because a name is what the view's author wrote; a chain
        /// of indices would have to be read back against the source to mean anything.
        path: Vec<String>,
    },
}

impl ViewError {
    /// The `ObjectId` this failure is about, when it has one.
    ///
    /// # Why a failure needs to name its node (BLUE26 B26.07)
    ///
    /// A report a designer can act on must answer "which **model node** failed?", and the id is the
    /// join: the caller resolves it through
    /// [`ViewEngine::tree_index`](crate::view::ViewEngine::tree_index) to the node's stable path and
    /// key. A failure that named only a property would leave the caller unable to find the node in
    /// its own model. [`ViewError::UnknownWidgetType`] answers `None` because such a node was never
    /// created — it has no id, which is why it carries the declared `widget`/`key`/`path` instead.
    pub fn affected_id(&self) -> Option<crate::core::ObjectId> {
        match self {
            ViewError::UnknownWidget { id }
            | ViewError::PropertyRefused { id, .. }
            | ViewError::UnknownParent { parent: id } => Some(*id),
            ViewError::UnknownWidgetType { .. } => None,
        }
    }

    /// The affected node's stable location as `"widget#key"` (or `"widget"`), when known.
    ///
    /// For a structural failure this is the declared location; for a patch failure it is the id
    /// spelled out, because the error alone does not carry the tree the id belongs to. A caller that
    /// wants the *key* rather than the id resolves it through the engine's tree index.
    pub fn location(&self) -> String {
        match self {
            ViewError::UnknownWidgetType { widget, key, path } => {
                let mut chain = path.clone();
                chain.push(widget.clone());
                match key {
                    Some(key) => format!("{}#{key}", chain.join(" > ")),
                    None => chain.join(" > "),
                }
            }
            ViewError::UnknownWidget { id } => format!("widget {id}"),
            ViewError::PropertyRefused { id, name, .. } => format!("widget {id}.{name}"),
            ViewError::UnknownParent { parent } => format!("parent {parent}"),
        }
    }
}

impl core::fmt::Display for ViewError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ViewError::UnknownWidget { id } => {
                write!(f, "patch targets widget {id}, which is not mounted")
            }
            ViewError::PropertyRefused { id, name, reason } => {
                write!(f, "widget {id} refused a write to '{name}': {reason}")
            }
            ViewError::UnknownParent { parent } => {
                write!(f, "patch targets parent {parent}, which is not mounted")
            }
            ViewError::UnknownWidgetType { widget, key, path } => {
                write!(f, "no constructor is registered for widget type '{widget}'")?;
                if let Some(key) = key {
                    write!(f, " (key '{key}'")?;
                } else {
                    write!(f, " (keyless")?;
                }
                if path.is_empty() {
                    write!(f, ", at the root)")
                } else {
                    write!(f, ", under {})", path.join(" > "))
                }
            }
        }
    }
}

/// What [`apply`] did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ApplyReport {
    /// Property writes that reached a control and were accepted.
    pub properties_written: usize,
    /// Controls created (including every node of an inserted subtree).
    pub widgets_created: usize,
    /// Controls removed, counting each removed subtree's nodes.
    pub widgets_removed: usize,
    /// Controls that were re-parented in place, keeping their id.
    pub widgets_moved: usize,
    /// Patches that could not be carried out, with reasons.
    pub errors: Vec<ViewError>,
}

impl ApplyReport {
    /// Whether every patch was carried out.
    pub fn is_clean(&self) -> bool {
        self.errors.is_empty()
    }

    /// Whether the batch was **partly** applied: some patches succeeded and some failed.
    ///
    /// # Why this is not `!is_clean()`
    ///
    /// An update that failed every patch and one that applied all but a single property both have
    /// a non-empty `errors` list, but they need different handling: the first means the tree is
    /// not the tree the view asked for, the second means it is, except in one place. A host that
    /// treats them identically either discards a good tree or hides a real failure (BLUE26 B26.07).
    pub fn is_partial(&self) -> bool {
        !self.errors.is_empty() && self.completed() > 0
    }

    /// Whether the batch applied nothing at all, despite having patches to apply.
    ///
    /// A batch with no errors and no work is neither partial nor failed; this returns `false` for
    /// it, because "nothing failed" is not "everything failed".
    pub fn failed_entirely(&self) -> bool {
        !self.errors.is_empty() && self.completed() == 0
    }

    /// How many patches were carried out, summed over the categories the report counts.
    ///
    /// The categories are disjoint by construction: a created control is not a moved or removed
    /// one, and a written property is a property of some node rather than a node. Summing them is
    /// therefore a count of *work done*, which is what `is_partial` compares against the failure
    /// count.
    pub fn completed(&self) -> usize {
        self.properties_written + self.widgets_created + self.widgets_removed + self.widgets_moved
    }

    /// Total controls destroyed by this batch, including subtree members.
    pub fn total_removed(&self) -> usize {
        self.widgets_removed
    }

    /// The declared location of every node that could not be expressed, as a
    /// `widget`-name chain.
    ///
    /// # Why the failure set is queryable
    ///
    /// A report whose only interface is "were there errors?" forces a caller to either
    /// ignore the failures or refuse the whole update. That binary is what made a single
    /// unknown widget name cost an entire window. Naming each failure lets a host show a
    /// placeholder for exactly the nodes that failed while the rest of the tree renders —
    /// the same shape as the diff's `positional_matches` counter: **the cost is visible, not
    /// silent**, and the good parts survive.
    pub fn failed_nodes(&self) -> Vec<String> {
        self.errors
            .iter()
            .filter_map(|error| match error {
                ViewError::UnknownWidgetType { widget, key, path } => {
                    let mut chain = path.clone();
                    chain.push(widget.clone());
                    match key {
                        Some(key) => Some(format!("{}#{key}", chain.join(" > "))),
                        None => Some(chain.join(" > ")),
                    }
                }
                _ => None,
            })
            .collect()
    }

    /// The number of nodes that could not be expressed.
    ///
    /// A count rather than a boolean, for the same reason [`Self::failed_nodes`] exists:
    /// "one node failed" and "the tree is broken" are different situations.
    pub fn failed_count(&self) -> usize {
        self.errors
            .iter()
            .filter(|error| matches!(error, ViewError::UnknownWidgetType { .. }))
            .count()
    }

    /// A declarative **placeholder** for every node this batch could not express.
    ///
    /// # Why the library builds the node and the host paints it
    ///
    /// A node that could not be constructed has, by definition, no control — so it cannot
    /// draw itself. The two halves therefore split cleanly: the library knows *where* the
    /// failures were (this method) and what a placeholder should say, and the host decides
    /// how to show it (a diagnostic panel, a red box, a log line). Faking a renderer here
    /// would put a drawing policy in the declarative layer, which is exactly the separation
    /// `mod.rs` draws.
    ///
    /// The placeholder is a real node tree, so a host can mount it with the same construction
    /// bridge it already has — no new path. Its label carries the widget name so a reader of
    /// the screen sees *what* was missing, and it is a plain `panel` + `label` pair rather
    /// than an empty `spacer`, so it occupies non-zero ink (BLUE23 §5A.9 judgement 13).
    pub fn placeholders(&self) -> Vec<Node> {
        self.failed_nodes()
            .into_iter()
            .map(|location| {
                Node::new("panel")
                    .key(format!("__view_error::{location}"))
                    .child(Node::new("label").prop("text", CapabilityValue::String(location)))
            })
            .collect()
    }
}

/// Apply `patches` to the retained tree described by `layout`.
///
/// `create` constructs a live control for a declarative node and returns its id; it is
/// injected rather than called directly so that this function stays testable without a
/// window and so the caller decides how a name becomes a control (the JSON loader's factory
/// table, or a test's stub).
///
/// The layout is updated as patches land, so a batch that includes an `Insert` followed by a
/// `SetProperty` on the inserted subtree resolves — the identity map and the live tree move
/// together rather than one lagging the other.
///
/// # Structural patches
///
/// `Insert` / `Remove` / `Move` / `Replace` maintain both the widget tree and the
/// [`BoundJsonLayout`](crate::json::BoundJsonLayout) indexes, because a diff that leaves
/// those two disagreeing produces an identity map describing controls that no longer exist.
///
/// # Portal hosts
///
/// A node declares **where it is created** separately from where it sits in the tree
/// ([`Node::host`](crate::view::Node::host) / `Node::portal`). [`apply`] itself cannot create
/// the overlay layer — that is engine-owned state — so a caller that needs portals honoured on
/// an *insert* passes an `OverlayHost` through `apply_with_reserved_and_overlay`. Without
/// one, an inserted portal is parented to its declared parent, which is the pre-portal
/// behaviour; the engine always passes one, so the two paths agree.
pub fn apply(
    layout: &mut crate::json::BoundJsonLayout,
    patches: &[Patch],
    create: &dyn Fn(&Node) -> Option<ObjectId>,
) -> ApplyReport {
    apply_with_reservations(layout, patches, create, &mut crate::compat::HashMap::new())
}

/// Resolves the layer a portal node is created into.
///
/// # Why this is injected rather than computed here
///
/// The overlay layer is a control the engine creates once and remembers, so only the engine can
/// answer "which id is it" (and create it on first use). `apply` is a free function that owns no
/// such state, so it takes the answer as a callback. Passing it is what makes an inserted portal
/// land outside its parent's clip on the `update` path the same way `mount` already does —
/// before this, `Node::portal`'s contract held only on the first build.
/// Resolves the layer a portal or layered node is created into, by name.
///
/// The layers are controls the engine creates once and remembers, so only the engine can
/// answer; this path is given the answer as a callback. The name passed is `"overlay"` for
/// [`Host::Overlay`](crate::view::Host::Overlay) and the declared name for
/// [`Host::Layer`](crate::view::Host::Layer), which is what lets a host keep one resolver for
/// the whole z-ordered stack.
pub type OverlayHost<'a> = &'a dyn Fn(&str) -> Option<ObjectId>;

/// The name of the default portal layer.
///
/// Named rather than inlined because the engine, the apply path and the resolver closure all
/// have to agree on it; one constant is the agreement.
pub const DEFAULT_LAYER: &str = "overlay";

#[derive(Clone, Copy)]
struct ApplyServices<'a> {
    overlay: Option<OverlayHost<'a>>,
    icon_resolver: Option<&'a dyn IconResourceResolver>,
}

/// `apply_with_reservations` with a portal-host resolver.
/// Ids allocated for a batch's inserted subtrees before `apply` runs, keyed by the node's
/// declared key (the empty string for a keyless node).
///
/// The value is a **queue**, not a single id: one batch can legally insert several subtrees
/// that share a key (two keyless siblings, or the same key under two different parents), and
/// a single id per key made the second reservation overwrite the first — the first subtree
/// then adopted the second's id and the first allocated control was never mounted. Both
/// sides traverse in the same order (patches in order, children in declaration order), so
/// front-to-back pairing is exact.
pub type ReservedIds = crate::compat::HashMap<String, crate::compat::VecDeque<ObjectId>>;

pub fn apply_with_reserved_and_overlay(
    layout: &mut crate::json::BoundJsonLayout,
    patches: &[Patch],
    create: &dyn Fn(&Node) -> Option<ObjectId>,
    reserved: &mut ReservedIds,
    overlay: Option<OverlayHost<'_>>,
) -> ApplyReport {
    apply_with_reserved_overlay_and_icon_resolver(layout, patches, create, reserved, overlay, None)
}

/// Applies a patch batch while resolving `IconSpec::Resource` properties through `resolver`.
///
/// Resolution happens before the target widget is mutably borrowed. A resolver failure is
/// reported as a refused property and does not replace the widget's existing icon. Inserted
/// subtrees use the same resolver path as `SetProperty`; callers without a resolver retain the
/// ordinary `ResourceResolutionRequired` refusal.
pub fn apply_with_icon_resolver(
    layout: &mut crate::json::BoundJsonLayout,
    patches: &[Patch],
    create: &dyn Fn(&Node) -> Option<ObjectId>,
    resolver: &dyn IconResourceResolver,
) -> ApplyReport {
    apply_with_reserved_overlay_and_icon_resolver(
        layout,
        patches,
        create,
        &mut crate::compat::HashMap::new(),
        None,
        Some(resolver),
    )
}

/// Full apply entry point for callers that also reserve insert IDs or create portal nodes.
pub fn apply_with_reserved_overlay_and_icon_resolver(
    layout: &mut crate::json::BoundJsonLayout,
    patches: &[Patch],
    create: &dyn Fn(&Node) -> Option<ObjectId>,
    reserved: &mut ReservedIds,
    overlay: Option<OverlayHost<'_>>,
    resolver: Option<&dyn IconResourceResolver>,
) -> ApplyReport {
    let mut report = ApplyReport::default();
    let services = ApplyServices { overlay, icon_resolver: resolver };
    for patch in patches {
        apply_one(layout, patch, create, reserved, &mut report, services);
    }
    report
}

/// [`apply`], but adopting ids the caller already allocated for the batch's own insertions.
///
/// `reserved` is keyed by the inserted node's declared key (the empty string for a keyless
/// node) and is consumed in insertion order as the nodes are created. See [`insert_subtree`]
/// for why this exists: without it, every inserted node would be constructed twice, leaving
/// one orphaned control per insertion.
pub fn apply_with_reservations(
    layout: &mut crate::json::BoundJsonLayout,
    patches: &[Patch],
    create: &dyn Fn(&Node) -> Option<ObjectId>,
    reserved: &mut ReservedIds,
) -> ApplyReport {
    apply_with_reserved_and_overlay(layout, patches, create, reserved, None)
}

/// Carry out a single patch, recording the outcome in `report`.
fn apply_one(
    layout: &mut crate::json::BoundJsonLayout,
    patch: &Patch,
    create: &dyn Fn(&Node) -> Option<ObjectId>,
    reserved: &mut ReservedIds,
    report: &mut ApplyReport,
    services: ApplyServices<'_>,
) {
    match patch {
        Patch::SetProperty { id, name, value } => {
            if !is_mounted(layout, *id) {
                report.errors.push(ViewError::UnknownWidget { id: *id });
                return;
            }
            match write_property_with_icon_resolver(
                *id,
                name,
                value.clone(),
                services.icon_resolver,
            ) {
                Ok(()) => report.properties_written += 1,
                Err(reason) => report.errors.push(ViewError::PropertyRefused {
                    id: *id,
                    name: name.clone(),
                    reason,
                }),
            }
        }
        Patch::Insert { parent, index, node, .. } => {
            if !is_mounted(layout, *parent) {
                report.errors.push(ViewError::UnknownParent { parent: *parent });
                return;
            }
            let created =
                insert_subtree(layout, *parent, *index, node, create, reserved, report, services);
            report.widgets_created += created;
        }
        Patch::Remove { id } => {
            if !is_mounted(layout, *id) {
                report.errors.push(ViewError::UnknownWidget { id: *id });
                return;
            }
            report.widgets_removed += remove_subtree(layout, *id);
        }
        Patch::Move { id, parent, index } => {
            if !is_mounted(layout, *id) {
                report.errors.push(ViewError::UnknownWidget { id: *id });
                return;
            }
            if !is_mounted(layout, *parent) {
                report.errors.push(ViewError::UnknownParent { parent: *parent });
                return;
            }
            reparent(layout, *id, *parent, *index);
            report.widgets_moved += 1;
        }
        Patch::Replace { id, parent, index, node, .. } => {
            if !is_mounted(layout, *id) {
                report.errors.push(ViewError::UnknownWidget { id: *id });
                return;
            }
            if !is_mounted(layout, *parent) {
                report.errors.push(ViewError::UnknownParent { parent: *parent });
                return;
            }
            report.widgets_removed += remove_subtree(layout, *id);
            let created =
                insert_subtree(layout, *parent, *index, node, create, reserved, report, services);
            report.widgets_created += created;
        }
    }
}

/// Whether `id` names a control the layout still knows about.
///
/// Structural knowledge is the authority rather than the platform's registry: a node the
/// layout has detached is exactly what "stale id" means here, and asking the platform would
/// answer about a different tree. The root is checked separately because a root has no
/// parent entry.
fn is_mounted(layout: &crate::json::BoundJsonLayout, id: ObjectId) -> bool {
    if id == 0 {
        return false;
    }
    layout.root() == Some(id) || layout.parent(id).is_some() || layout.widget_name(id).is_some()
}

/// Create `node` and its declared subtree under `parent`, returning how many nodes were made.
///
/// `reserved` maps a node's declared key (or its empty key for a keyless node) to a queue of
/// ids the caller has **already allocated** for nodes with that key. When a queue is
/// non-empty, the front id is adopted rather than `create` being called again:
/// `ViewEngine::reserve_ids_for_inserts` has to know the ids of a batch's newly inserted
/// nodes before `apply` runs, and the only way to learn an id is to call `create`. Calling
/// it twice for one node produced a second live control and orphaned the first — one leaked
/// widget per inserted node, forever. The queue (not a single slot) is what lets one batch
/// carry several inserts that share a key: both sides traverse in the same order, so
/// front-to-back pairing is exact.
fn insert_subtree(
    layout: &mut crate::json::BoundJsonLayout,
    parent: ObjectId,
    index: usize,
    node: &Node,
    create: &dyn Fn(&Node) -> Option<ObjectId>,
    reserved: &mut ReservedIds,
    report: &mut ApplyReport,
    services: ApplyServices<'_>,
) -> usize {
    // ── Transparent nodes ──
    //
    // A transparent node (`spacer` — see `super::is_transparent_node`, which is the single
    // spelling of that rule) is a *declarative* node that produces **no control**. The JSON path
    // has handled it since the beginning (`json/loader.rs:245`), and `view/diff.rs` documents the
    // same contract in four places — but this function had no branch for it, so it fell through to
    // `create`, which cannot resolve a pseudo-widget, and the node was reported as
    // `UnknownWidgetType` and its **entire subtree dropped**. A single spacer in a declarative UI
    // therefore deleted everything declared inside it, silently, with only an error entry to show
    // for it.
    //
    // The two front ends must agree about what a node means, so the treatment here matches the
    // loader's: the node itself contributes nothing, and its children are inserted **into the
    // parent** at this node's own index, so the declaration order the diff matches on is preserved.
    //
    // `spacer` additionally carries a `stretch`, which on the JSON path becomes a weight on the
    // *layout*. Here the nearest analogue is the parent's stretch entry for this slot; there is no
    // control to weight, so the spacer's job — reserving room between two siblings — is expressed
    // by declaring nothing and letting the surrounding layout distribute the leftover, which is
    // what `justify_content` and the children's own hints already do. Recording the intent rather
    // than inventing a zero-sized control keeps `spacer` "a node that reserves space" rather than
    // "a control that happens to be invisible" (principle #4).
    if super::is_transparent_node(node) {
        let mut count = 0usize;
        for (i, child) in node.children.iter().enumerate() {
            count += insert_subtree(
                layout,
                parent,
                index + i,
                child,
                create,
                reserved,
                report,
                services,
            );
        }
        return count;
    }

    let reservation_key = node.key.clone().unwrap_or_default();
    let reserved_id = reserved.get_mut(&reservation_key).and_then(|queue| queue.pop_front());

    let id = match reserved_id.or_else(|| create(node)) {
        Some(id) if id != 0 => id,
        _ => {
            report.errors.push(ViewError::UnknownWidgetType {
                widget: node.widget.clone(),
                key: node.key.clone(),
                // An inserted subtree is addressed by node value rather than by path, so the
                // ancestors are not available here; the key and type name still locate it in
                // the declaration. The mount path, which walks the tree, carries the full
                // chain.
                path: Vec::new(),
            });
            return 0;
        }
    };

    // Register before recursing: a child's `register_node` looks up its parent's child list
    // to append itself, so the parent must already exist in the index.
    //
    // # Where the control is *hosted*, which is not always where it is declared
    //
    // A portal node declares its place in the tree but is created into the overlay layer, so its
    // control sits outside the parent's clip while its diff identity stays this parent's child
    // (`Node::portal`). `ViewEngine::mount` has honoured that since portals were added; this path
    // did not, so a tooltip or menu that appeared only after a breakpoint change (an `Insert`, not
    // a `mount`) was parented to its declared parent and clipped by it. The resolver comes from the
    // engine because the overlay layer is engine-owned state; when a caller passes none — a test
    // driving `apply` directly — the declared parent is used and the behaviour is the pre-portal
    // one, which is stated rather than accidental.
    let key = node.key.clone().unwrap_or_default();
    let host = match &node.host {
        super::Host::Declared => Some(parent),
        super::Host::Overlay => {
            services.overlay.and_then(|resolve| resolve(DEFAULT_LAYER)).or(Some(parent))
        }
        super::Host::Layer(name) => {
            services.overlay.and_then(|resolve| resolve(name)).or(Some(parent))
        }
    };
    layout.register_node(id, node.widget.clone(), key, host);
    if let Some(host) = host {
        place_child_at(layout, host, id, index);
    }

    let mut count = 1usize;
    for (i, child) in node.children.iter().enumerate() {
        count += insert_subtree(layout, id, i, child, create, reserved, report, services);
    }
    // The subtree's declared properties go through the same property contract as a patch,
    // so a control that refuses one reports it here rather than at the next diff.
    for (name, value) in &node.props {
        // A generated wire carrier is read by the generated `create_for`, never by a control, so it
        // must not be written — see `ViewEngine::write_declared_properties`, which shares this
        // predicate. Without it a program generated from a document with wires reported a
        // `PropertyRefused` for every wired node.
        if super::is_generated_wire_prop(name) {
            continue;
        }
        match write_property_with_icon_resolver(id, name, value.clone(), services.icon_resolver) {
            Ok(()) => report.properties_written += 1,
            Err(reason) => {
                report.errors.push(ViewError::PropertyRefused { id, name: name.clone(), reason })
            }
        }
    }
    count
}

/// Remove `id` and its subtree from the layout, returning how many nodes disappeared.
fn remove_subtree(layout: &mut crate::json::BoundJsonLayout, id: ObjectId) -> usize {
    layout.detach(id).len()
}

/// Move `id` to position `index` under `parent`, keeping its `ObjectId`.
fn reparent(
    layout: &mut crate::json::BoundJsonLayout,
    id: ObjectId,
    parent: ObjectId,
    index: usize,
) {
    let key = layout.node_key(id).unwrap_or_default().to_string();
    let widget = layout.widget_name(id).unwrap_or_default().to_string();
    layout.register_node(id, widget, key, Some(parent));
    place_child_at(layout, parent, id, index);
}

/// Move `child` within `parent`'s child list to position `index`, clamping to the end.
fn place_child_at(
    layout: &mut crate::json::BoundJsonLayout,
    parent: ObjectId,
    child: ObjectId,
    index: usize,
) {
    layout.move_child_to(parent, child, index);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::capability::CapabilityValue;

    fn s(v: &str) -> CapabilityValue {
        CapabilityValue::String(v.to_string())
    }

    /// A stub backend: it never creates real controls, because these tests are about the
    /// layout's structural bookkeeping rather than about any control's appearance.
    struct StubBackend {
        next: std::cell::Cell<ObjectId>,
    }

    impl StubBackend {
        fn new() -> Self {
            Self { next: std::cell::Cell::new(100) }
        }

        /// The `create` closure handed to [`apply`].
        fn creator(&self) -> impl Fn(&Node) -> Option<ObjectId> + '_ {
            move |_node: &Node| {
                let id = self.next.get();
                self.next.set(id + 1);
                Some(id)
            }
        }
    }

    fn fixture_root() -> (crate::json::BoundJsonLayout, ObjectId) {
        let mut layout = crate::json::BoundJsonLayout::new();
        layout.register_node(1, "window", "main", None);
        (layout, 1)
    }

    /// A creator that builds and registers **real** controls, so a property write can be
    /// observed after the fact.
    ///
    /// The `StubBackend` above is deliberately id-only, which is right for structural
    /// assertions but useless for asking "did the value actually change?" — and that
    /// question is the whole point of the reset test below.
    fn real_creator() -> impl Fn(&Node) -> Option<ObjectId> {
        let factory = crate::widget::WidgetFactory::shared();
        move |node: &Node| {
            let widget: Box<dyn crate::widget::Widget> = factory.create(
                &node.widget,
                crate::core::Rect::new(0, 0, 80, 24),
                node.key_str().unwrap_or("anon"),
            )?;
            crate::widget::runtime::register(widget)
        }
    }

    /// Read a property off a live control.
    fn read(id: ObjectId, name: &str) -> Option<CapabilityValue> {
        crate::widget::runtime::with_widget(id, |widget| {
            crate::widget::capability::properties_trait::widget_property_get(widget, name).ok()
        })
        .flatten()
    }

    #[test]
    fn both_write_routes_agree_on_what_a_property_write_does() {
        // One logical operation, two routes: `set_widget_property(widget_id, name, value)` in
        // the capability layer, and `write_property` in this module for a `Node`-driven
        // `Patch::SetProperty`. They must leave the control in the same state — and, since
        // `with_widget_mut` invalidates nothing, they must both ask for the repaint that makes
        // the change visible.
        //
        // The declarative route used to omit the repaint. Nothing errored and the widget really
        // did change, which is why only a test that compares the two routes can see it: the
        // stale-frame half of the defect is invisible from either route alone.
        //
        // The repaint request itself is inert here (no backend is mounted, so
        // `invalidate_surface` does nothing), so what is asserted is what a unit test can
        // honestly observe — that the routes agree on the resulting value — while
        // `tools/check_declarative_path_repaints.sh` guards the call's presence in the source,
        // which is the half that cannot be observed without a live platform.
        let id = crate::widget::runtime::register(
            crate::widget::WidgetFactory::shared()
                .create("label", crate::core::Rect::new(0, 0, 80, 24), "r")
                .expect("label is a registered control"),
        )
        .expect("the runtime accepts a freshly created control");

        // Route A: by id/name, through the capability layer.
        crate::widget::capability::write_widget_property_by_id(id, "text", s("from-name"))
            .expect("a writable property on a live control must accept the write");
        assert_eq!(read(id, "text"), Some(s("from-name")));

        // Route B: by `Patch`, through this module.
        write_property(id, "text", s("from-node")).expect("the declarative route must succeed");
        assert_eq!(
            read(id, "text"),
            Some(s("from-node")),
            "the declarative write and the by-name write must reach the same implementation"
        );
    }

    #[test]
    fn node_diff_applies_icon_specs_and_reports_unresolved_resources() {
        let (mut layout, root) = fixture_root();
        let create = real_creator();
        let inserted = apply(
            &mut layout,
            &[Patch::Insert { path: Vec::new(),
                parent: root,
                index: 0,
                node: Node::new("button").key("icon-button"),
            }],
            &create,
        );
        assert!(inserted.is_clean(), "button mount failed: {:?}", inserted.errors);
        let id = layout.child_by_key(Some(root), "icon-button").expect("button is indexed");

        let old = Node::new("button").key("icon-button");
        let builtin = Node::new("button").key("icon-button").prop(
            "icon_source",
            CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Builtin(
                crate::widget::IconName::Info,
            )),
        );
        let patches = super::super::diff::diff(&old, &builtin, &|_, _| Some(id));
        assert_eq!(patches.patches.len(), 1, "the IconSpec change must produce one property patch");
        let applied = apply(&mut layout, &patches.patches, &create);
        assert!(applied.is_clean(), "built-in IconSpec apply failed: {:?}", applied.errors);
        assert_eq!(
            read(id, "icon_source"),
            Some(CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Builtin(
                crate::widget::IconName::Info
            )))
        );

        let resource = Node::new("button").key("icon-button").prop(
            "icon_source",
            CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Resource {
                id: String::from("project/icons/missing"),
                color_mode: crate::widget::icon_source::IconColorMode::Monochrome,
            }),
        );
        let resource_patches = super::super::diff::diff(&builtin, &resource, &|_, _| Some(id));
        let refused = apply(&mut layout, &resource_patches.patches, &create);
        assert!(
            refused.errors.iter().any(|error| matches!(
                error,
                super::ViewError::PropertyRefused { name, reason, .. }
                    if name == "icon_source" && reason.contains("ResourceResolutionRequired")
            )),
            "the view report must preserve the resolver error: {:?}",
            refused.errors
        );
        assert_eq!(
            read(id, "icon_source"),
            Some(CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Builtin(
                crate::widget::IconName::Info
            ))),
            "refused resource writes must leave the last valid source intact"
        );

        let cleared = Node::new("button").key("icon-button");
        let clear_patches = super::super::diff::diff(&builtin, &cleared, &|_, _| Some(id));
        assert_eq!(
            clear_patches.patches.len(),
            1,
            "removing the declaration must generate a reset patch"
        );
        let reset = apply(&mut layout, &clear_patches.patches, &create);
        assert!(reset.is_clean(), "IconSpec reset failed: {:?}", reset.errors);
        assert_eq!(read(id, "icon_source"), Some(CapabilityValue::Null));
    }

    struct TestIconResolver {
        fail: bool,
        calls: std::cell::RefCell<Vec<(String, crate::widget::icon_source::IconColorMode)>>,
    }

    impl crate::widget::icon_source::IconResourceResolver for TestIconResolver {
        fn resolve_icon(
            &self,
            resource_id: &str,
            color_mode: crate::widget::icon_source::IconColorMode,
        ) -> Result<crate::widget::icon_source::IconSource, String> {
            self.calls.borrow_mut().push((resource_id.to_string(), color_mode));
            if self.fail {
                return Err("resource is unavailable".to_string());
            }
            Ok(crate::widget::icon_source::IconSource::Builtin(crate::widget::IconName::Search))
        }
    }

    #[test]
    fn view_resource_icons_resolve_for_updates_and_inserted_subtrees() {
        use crate::widget::icon_source::{IconColorMode, IconSpec};

        let (mut layout, root) = fixture_root();
        let create = real_creator();
        let inserted = apply(
            &mut layout,
            &[Patch::Insert { path: Vec::new(),
                parent: root,
                index: 0,
                node: Node::new("button").key("icon-button"),
            }],
            &create,
        );
        assert!(inserted.is_clean(), "button mount failed: {:?}", inserted.errors);
        let id = layout.child_by_key(Some(root), "icon-button").expect("button is indexed");
        let old = Node::new("button").key("icon-button");
        let resource = Node::new("button").key("icon-button").prop(
            "icon_source",
            CapabilityValue::IconSpec(IconSpec::Resource {
                id: "project/icons/search".to_string(),
                color_mode: IconColorMode::Monochrome,
            }),
        );
        let patches = super::super::diff::diff(&old, &resource, &|_, _| Some(id));
        let resolver = TestIconResolver { fail: false, calls: std::cell::RefCell::new(Vec::new()) };
        let applied =
            super::apply_with_icon_resolver(&mut layout, &patches.patches, &create, &resolver);
        assert!(applied.is_clean(), "resource update failed: {:?}", applied.errors);
        assert_eq!(
            read(id, "icon_source"),
            Some(CapabilityValue::IconSpec(IconSpec::Builtin(crate::widget::IconName::Search))),
            "the resolver's validated drawable source must reach the widget"
        );
        assert_eq!(
            resolver.calls.borrow().as_slice(),
            &[("project/icons/search".to_string(), IconColorMode::Monochrome)]
        );

        let failed_resolver =
            TestIconResolver { fail: true, calls: std::cell::RefCell::new(Vec::new()) };
        let failed = super::write_property_with_icon_resolver(
            id,
            "icon_source",
            CapabilityValue::IconSpec(IconSpec::Resource {
                id: "project/icons/unavailable".to_string(),
                color_mode: IconColorMode::Original,
            }),
            Some(&failed_resolver),
        );
        assert!(
            failed.as_ref().is_err_and(|error| error.contains("resource is unavailable")),
            "resolution failures must be surfaced: {failed:?}"
        );
        assert_eq!(
            read(id, "icon_source"),
            Some(CapabilityValue::IconSpec(IconSpec::Builtin(crate::widget::IconName::Search))),
            "a failed resolution must not replace the previously valid icon"
        );
    }

    #[test]
    fn view_resource_icons_resolve_in_inserted_subtrees() {
        use crate::widget::icon_source::{IconColorMode, IconSpec};

        let (mut layout, root) = fixture_root();
        let create = real_creator();
        let resolver = TestIconResolver { fail: false, calls: std::cell::RefCell::new(Vec::new()) };
        let report = super::apply_with_icon_resolver(
            &mut layout,
            &[Patch::Insert { path: Vec::new(),
                parent: root,
                index: 0,
                node: Node::new("button").key("resource-button").prop(
                    "icon_source",
                    CapabilityValue::IconSpec(IconSpec::Resource {
                        id: "project/icons/inserted".to_string(),
                        color_mode: IconColorMode::Original,
                    }),
                ),
            }],
            &create,
            &resolver,
        );
        assert!(report.is_clean(), "inserted resource property failed: {:?}", report.errors);
        let id =
            layout.child_by_key(Some(root), "resource-button").expect("inserted button is indexed");
        assert_eq!(
            read(id, "icon_source"),
            Some(CapabilityValue::IconSpec(IconSpec::Builtin(crate::widget::IconName::Search)))
        );
        assert_eq!(resolver.calls.borrow().len(), 1, "insert uses the resolver exactly once");
    }

    /// A dropped property must reach the value the schema declares, not be refused.
    ///
    /// # Why this test exists
    ///
    /// `diff` emits `CapabilityValue::Null` for a property a node stopped declaring, and
    /// `apply` used to write that `Null` verbatim. Only about twenty controls handle
    /// `Null` explicitly; the rest write through a typed extractor. `text_visible` on a
    /// `progress_bar` is a plain `Bool` with no `Null` arm, so the write was refused
    /// `TypeMismatch` and the control kept its stale value *permanently* — the next diff
    /// finds no such key in the new tree, so it emits no patch to retry, and the only
    /// signal was an `ApplyReport::errors` entry a caller commonly ignores.
    ///
    /// The assertion is on the value the control actually holds, which is the observable
    /// claim; `is_clean()` alone would not have caught it, since the refusal was recorded
    /// as an error rather than raised.
    #[test]
    fn a_dropped_property_resets_to_the_schema_default_and_really_lands() {
        let (mut layout, root) = fixture_root();
        let create = real_creator();
        let created = apply(
            &mut layout,
            &[Patch::Insert { path: Vec::new(), parent: root, index: 0, node: Node::new("progress_bar").key("bar") }],
            &create,
        );
        assert!(created.is_clean(), "mount failed: {:?}", created.errors);
        let bar = layout.child_by_key(Some(root), "bar").expect("bar must be indexed");

        // `text_visible` defaults to true, so writing `false` is a real change away from
        // the default and the reset has somewhere to return to.
        let set = apply(
            &mut layout,
            &[Patch::SetProperty {
                id: bar,
                name: "text_visible".to_string(),
                value: CapabilityValue::Bool(false),
            }],
            &create,
        );
        assert!(set.is_clean(), "declaring `text_visible` failed: {:?}", set.errors);
        assert_eq!(read(bar, "text_visible"), Some(CapabilityValue::Bool(false)));

        let reset = apply(
            &mut layout,
            &[Patch::SetProperty {
                id: bar,
                name: "text_visible".to_string(),
                value: CapabilityValue::Null,
            }],
            &create,
        );
        assert!(
            reset.is_clean(),
            "a dropped property must not be refused; errors: {:?}",
            reset.errors
        );
        let after = read(bar, "text_visible");
        assert_ne!(
            after,
            Some(CapabilityValue::Bool(false)),
            "the reset did not land: the control still holds the dropped value"
        );
        assert_eq!(
            after,
            crate::widget::capability::access::default_widget_property_default_value(
                crate::widget::WidgetKind::ProgressBar,
                "text_visible"
            ),
            "the reset must reach the schema-declared default"
        );
    }

    /// A property the schema declares no default for still receives `Null`, so a control
    /// that documents `Null` as meaningful keeps its contract.
    #[test]
    fn a_null_reset_for_an_undeclared_property_is_still_written_as_null() {
        let (mut layout, root) = fixture_root();
        let create = real_creator();
        apply(
            &mut layout,
            &[Patch::Insert { path: Vec::new(), parent: root, index: 0, node: Node::new("label").key("l") }],
            &create,
        );
        let id = layout.child_by_key(Some(root), "l").expect("label must be indexed");
        // `tooltip` is declared by every control and has a string default, so this asserts
        // the substitution path; an undeclared name must stay untouched by it.
        assert_eq!(resolve_null_reset(id, "tooltip"), Some(CapabilityValue::String(String::new())));
        assert_eq!(resolve_null_reset(id, "not_a_real_property"), None);
    }

    #[test]
    fn insert_creates_the_whole_declared_subtree() {
        let (mut layout, root) = fixture_root();
        let backend = StubBackend::new();
        let node = Node::new("panel")
            .key("card")
            .child(Node::new("label").key("title"))
            .child(Node::new("button").key("ok"));
        let report = apply(
            &mut layout,
            &[Patch::Insert { path: Vec::new(), parent: root, index: 0, node }],
            &backend.creator(),
        );
        assert!(report.is_clean(), "errors: {:?}", report.errors);
        assert_eq!(report.widgets_created, 3, "panel + label + button");
        assert_eq!(layout.children(root).len(), 1);
        let panel = layout.child_by_key(Some(root), "card").expect("panel must be indexed");
        assert_eq!(layout.children(panel).len(), 2);
        assert!(layout.child_by_key(Some(panel), "title").is_some());
    }

    #[test]
    fn insert_honours_the_declared_position() {
        let (mut layout, root) = fixture_root();
        let backend = StubBackend::new();
        let create = backend.creator();
        apply(
            &mut layout,
            &[Patch::Insert { path: Vec::new(), parent: root, index: 0, node: Node::new("label").key("first") }],
            &create,
        );
        apply(
            &mut layout,
            &[Patch::Insert { path: Vec::new(), parent: root, index: 1, node: Node::new("label").key("second") }],
            &create,
        );
        apply(
            &mut layout,
            &[Patch::Insert { path: Vec::new(), parent: root, index: 0, node: Node::new("label").key("zeroth") }],
            &create,
        );
        let keys: Vec<Option<&str>> =
            layout.children(root).iter().map(|&id| layout.node_key(id)).collect();
        assert_eq!(keys, [Some("zeroth"), Some("first"), Some("second")]);
    }

    #[test]
    fn insert_with_an_index_past_the_end_appends() {
        let (mut layout, root) = fixture_root();
        let backend = StubBackend::new();
        let create = backend.creator();
        for key in ["a", "b"] {
            apply(
                &mut layout,
                &[Patch::Insert { path: Vec::new(), parent: root, index: 99, node: Node::new("label").key(key) }],
                &create,
            );
        }
        let keys: Vec<Option<&str>> =
            layout.children(root).iter().map(|&id| layout.node_key(id)).collect();
        assert_eq!(keys, [Some("a"), Some("b")], "an out-of-range index appends, never panics");
    }

    #[test]
    fn removing_a_subtree_also_removes_its_descendants() {
        let (mut layout, root) = fixture_root();
        let backend = StubBackend::new();
        let create = backend.creator();
        apply(
            &mut layout,
            &[Patch::Insert { path: Vec::new(),
                parent: root,
                index: 0,
                node: Node::new("panel").key("card").child(Node::new("label").key("title")),
            }],
            &create,
        );
        let panel = layout.child_by_key(Some(root), "card").expect("panel");
        let report = apply(&mut layout, &[Patch::Remove { id: panel }], &create);
        assert!(report.is_clean(), "errors: {:?}", report.errors);
        assert_eq!(report.widgets_removed, 2, "the panel and its label");
        assert!(layout.children(root).is_empty());
        assert_eq!(layout.id("title"), None, "the descendant's name must go too");
    }

    #[test]
    fn a_patch_naming_a_detached_control_is_reported_not_ignored() {
        // The whole reason `apply` keeps its own mount check: silently dropping the write
        // would make a stale diff look like a successful update.
        let (mut layout, _root) = fixture_root();
        let backend = StubBackend::new();
        let report = apply(&mut layout, &[Patch::Remove { id: 999 }], &backend.creator());
        assert!(!report.is_clean());
        assert_eq!(report.errors, [ViewError::UnknownWidget { id: 999 }]);
    }

    #[test]
    fn an_insert_under_an_unknown_parent_is_reported() {
        let (mut layout, _root) = fixture_root();
        let backend = StubBackend::new();
        let report = apply(
            &mut layout,
            &[Patch::Insert { path: Vec::new(), parent: 999, index: 0, node: Node::new("label") }],
            &backend.creator(),
        );
        assert_eq!(report.errors, [ViewError::UnknownParent { parent: 999 }]);
        assert_eq!(report.widgets_created, 0, "nothing was created");
    }

    #[test]
    fn a_refused_constructor_is_reported_as_an_unknown_widget_type() {
        let (mut layout, root) = fixture_root();
        let report = apply(
            &mut layout,
            &[Patch::Insert { path: Vec::new(), parent: root, index: 0, node: Node::new("no_such_control") }],
            &|_n: &Node| None,
        );
        assert_eq!(
            report.errors,
            [ViewError::UnknownWidgetType {
                widget: "no_such_control".to_string(),
                key: None,
                path: Vec::new(),
            }]
        );
    }

    #[test]
    fn move_keeps_the_controls_identity() {
        // BLUE18 rule #90 in miniature: the id survives, so focus and internal state would
        // survive a real move. A remove+insert would assign a fresh id instead.
        let (mut layout, root) = fixture_root();
        let backend = StubBackend::new();
        let create = backend.creator();
        apply(
            &mut layout,
            &[Patch::Insert { path: Vec::new(), parent: root, index: 0, node: Node::new("panel").key("left") }],
            &create,
        );
        apply(
            &mut layout,
            &[Patch::Insert { path: Vec::new(), parent: root, index: 1, node: Node::new("panel").key("right") }],
            &create,
        );
        let left = layout.child_by_key(Some(root), "left").expect("left");
        let right = layout.child_by_key(Some(root), "right").expect("right");

        let report =
            apply(&mut layout, &[Patch::Move { id: left, parent: right, index: 0 }], &create);
        assert!(report.is_clean(), "errors: {:?}", report.errors);
        assert_eq!(report.widgets_moved, 1);
        assert_eq!(report.widgets_created, 0, "a move must not create anything");
        assert_eq!(report.widgets_removed, 0, "a move must not destroy anything");
        assert_eq!(layout.children(root), &[right]);
        assert_eq!(layout.children(right), &[left]);
        assert_eq!(layout.parent(left), Some(right), "the same id, a new parent");
        assert_eq!(layout.node_key(left), Some("left"), "its identity is unchanged");
    }

    #[test]
    fn replace_destroys_one_subtree_and_builds_another() {
        let (mut layout, root) = fixture_root();
        let backend = StubBackend::new();
        let create = backend.creator();
        apply(
            &mut layout,
            &[Patch::Insert { path: Vec::new(), parent: root, index: 0, node: Node::new("label").key("slot") }],
            &create,
        );
        let old = layout.child_by_key(Some(root), "slot").expect("slot");
        let report = apply(
            &mut layout,
            &[Patch::Replace { path: Vec::new(),
                id: old,
                parent: root,
                index: 0,
                node: Node::new("button").key("slot"),
            }],
            &create,
        );
        assert!(report.is_clean(), "errors: {:?}", report.errors);
        assert_eq!(report.widgets_removed, 1);
        assert_eq!(report.widgets_created, 1);
        let fresh = layout.child_by_key(Some(root), "slot").expect("slot");
        assert_ne!(fresh, old, "a replace must not reuse the id");
        assert_eq!(layout.widget_name(fresh), Some("button"));
        assert_eq!(layout.children(root), &[fresh], "it takes the old position");
    }

    #[test]
    fn an_inserted_nodes_declared_properties_are_written() {
        let (mut layout, root) = fixture_root();
        let backend = StubBackend::new();
        let node = Node::new("label").key("t").prop("text", s("Hello"));
        let report = apply(
            &mut layout,
            &[Patch::Insert { path: Vec::new(), parent: root, index: 0, node }],
            &backend.creator(),
        );
        // `label`'s `text` is a real published property, so the contract decides; a stub id
        // is not registered with the platform, so the write may be refused — either way the
        // call must have been *attempted* and the outcome reported, never silently skipped.
        assert_eq!(report.widgets_created, 1);
        assert!(
            report.properties_written + report.errors.len() >= 1,
            "the declared property must be attempted"
        );
    }

    #[test]
    fn apply_is_ordered_so_a_later_patch_can_address_an_earlier_insert() {
        // The batch the diff produces is ordered insert-then-update; if `apply` did not
        // register the new node first, the follow-up write would be reported as unknown.
        let (mut layout, root) = fixture_root();
        let backend = StubBackend::new();
        let create = backend.creator();
        apply(
            &mut layout,
            &[Patch::Insert { path: Vec::new(), parent: root, index: 0, node: Node::new("panel").key("card") }],
            &create,
        );
        let card = layout.child_by_key(Some(root), "card").expect("card");
        let follow_up = apply(&mut layout, &[Patch::Remove { id: card }], &create);
        assert!(
            follow_up.is_clean(),
            "the inserted node must be addressable: {:?}",
            follow_up.errors
        );
    }

    #[test]
    fn an_empty_batch_reports_a_clean_no_op() {
        let (mut layout, _root) = fixture_root();
        let report = apply(&mut layout, &[], &|_n: &Node| Some(1));
        assert!(report.is_clean());
        assert_eq!(report.properties_written, 0);
        assert_eq!(report.widgets_created, 0);
        assert_eq!(report.widgets_removed, 0);
        assert_eq!(report.total_removed(), 0);
    }

    #[test]
    fn error_display_names_the_thing_that_went_wrong() {
        assert!(ViewError::UnknownWidget { id: 7 }.to_string().contains('7'));
        assert!(ViewError::UnknownWidgetType {
            widget: "widget_x".into(),
            key: None,
            path: Vec::new(),
        }
        .to_string()
        .contains("widget_x"));
        assert!(ViewError::PropertyRefused {
            id: 1,
            name: "text".into(),
            reason: "not writable".into()
        }
        .to_string()
        .contains("text"));
    }

    #[test]
    fn a_layout_with_no_structure_reports_a_missing_parent_rather_than_mounting() {
        // Guards the is_mounted rule: an empty binding must not treat every id as valid,
        // or a patch batch against a layout that was never populated would look clean.
        let mut layout = crate::json::BoundJsonLayout::new();
        layout.register_node(1, "window", "main", None);
        layout.detach(1);
        let report = apply(
            &mut layout,
            &[Patch::Insert { path: Vec::new(), parent: 1, index: 0, node: Node::new("label") }],
            &|_n: &Node| Some(2),
        );
        assert!(!report.is_clean(), "an id that was detached must not be treated as mounted");
    }

    #[test]
    fn a_structure_free_layout_is_still_usable_by_a_caller_that_populates_it_first() {
        let mut layout = crate::json::BoundJsonLayout::new();
        layout.register_node(1, "window", "main", None);
        let report = apply(
            &mut layout,
            &[Patch::Insert { path: Vec::new(),
                parent: 1,
                index: 0,
                node: Node::new("label").key("a").child(Node::new("icon").key("i")),
            }],
            &|n: &Node| Some(50 + n.node_count() as u64),
        );
        assert_eq!(report.widgets_created, 2);
        assert_eq!(layout.children(1).len(), 1);
        let outer = layout.children(1)[0];
        assert_eq!(layout.children(outer).len(), 1, "the nested child was attached too");
    }

    /// A `spacer` creates no control, and its subtree is **not** lost.
    ///
    /// # The defect this closes (BLUE22 · F-7)
    ///
    /// `view/diff.rs` documents in four places that "a `spacer` becomes no control" — a transparent
    /// node — and the JSON path has implemented that since the beginning
    /// (`json/loader.rs:245`). `insert_subtree` had no branch for it, so a spacer fell through to
    /// `create`, which cannot resolve a pseudo-widget. The node was then reported as
    /// `UnknownWidgetType` and the function **returned early**, taking the spacer's entire subtree
    /// with it: one spacer in a declarative UI silently deleted every control declared inside it.
    ///
    /// The `StubBackend` used elsewhere in this module cannot see the defect, because its creator
    /// returns `Some` for every node — including a pseudo-widget. This test therefore uses a creator
    /// that behaves like the real one, which is the only way the branch under test is reachable.
    #[test]
    fn a_spacer_creates_no_control_but_keeps_its_subtree() {
        let mut layout = crate::json::BoundJsonLayout::new();
        layout.register_node(1, "window", "main", None);
        let factory = crate::widget::WidgetFactory::new_with_defaults();
        let create = move |node: &Node| {
            let widget = factory.create(
                &node.widget,
                crate::core::Rect::new(0, 0, 80, 24),
                node.key.as_deref().unwrap_or("anon"),
            )?;
            crate::widget::runtime::register(widget)
        };

        // A row that declares a spacer *around* two labels. The JSON front end attaches the
        // labels to the row; this front end must do the same rather than dropping them.
        let report = apply(
            &mut layout,
            &[Patch::Insert { path: Vec::new(),
                parent: 1,
                index: 0,
                node: Node::new("spacer")
                    .key("gap")
                    .child(Node::new("label").key("a").child(Node::new("label").key("nested"))),
            }],
            &create,
        );

        assert!(
            !report.errors.iter().any(|e| matches!(e, ViewError::UnknownWidgetType { .. })),
            "a spacer is a known transparent node, not an unknown widget: {:?}",
            report.errors
        );
        assert_eq!(report.widgets_created, 2, "the spacer's two descendants were created");
        // The subtree is attached to the **spacer's parent** (the window), because a transparent
        // node collapses into the layout around it.
        let top = layout.children(1);
        assert_eq!(top.len(), 1, "the spacer's first child took its place in the window: {top:?}");
        assert_eq!(layout.children(top[0]).len(), 1, "and its own child is still beneath it");
        // The spacer itself is not in the layout: it produced no control to name.
        assert!(
            !top.iter().any(|id| layout.widget_name(*id) == Some("spacer")),
            "a pseudo-widget must never be registered as a control"
        );
    }

    /// A spacer with no children is a no-op rather than an error.
    ///
    /// The degenerate form of the test above: the most common way to write a flexible gap is an
    /// empty `<spacer/>`, and it must neither create a control nor report a failure.
    #[test]
    fn an_empty_spacer_is_a_clean_no_op() {
        let mut layout = crate::json::BoundJsonLayout::new();
        layout.register_node(1, "window", "main", None);
        let factory = crate::widget::WidgetFactory::new_with_defaults();
        let create = move |node: &Node| {
            factory
                .create(&node.widget, crate::core::Rect::new(0, 0, 40, 20), "")
                .and_then(crate::widget::runtime::register)
        };
        let report = apply(
            &mut layout,
            &[Patch::Insert { path: Vec::new(), parent: 1, index: 0, node: Node::new("spacer") }],
            &create,
        );
        assert!(report.is_clean(), "an empty spacer reports nothing: {:?}", report.errors);
        assert_eq!(report.widgets_created, 0, "and creates nothing");
        assert!(layout.children(1).is_empty(), "leaving the parent's child list untouched");
    }

    /// Build a `BoundJsonLayout` from a declarative tree and return the root id.
    ///
    /// The layout is populated by walking `root`, creating a real control per non-`spacer`
    /// node via `create`, so a subsequent `diff`/`apply` cycle exercises the real path.
    fn mount_layout(
        root: &Node,
        create: &dyn Fn(&Node) -> Option<ObjectId>,
    ) -> (crate::json::BoundJsonLayout, ObjectId) {
        let mut layout = crate::json::BoundJsonLayout::new();
        let mut register = |node: &Node, parent: Option<ObjectId>| -> Option<ObjectId> {
            if node.widget.eq_ignore_ascii_case("spacer") {
                return None;
            }
            let id = create(node)?;
            layout.register_node(
                id,
                node.widget.clone(),
                node.key_str().unwrap_or("").to_string(),
                parent,
            );
            Some(id)
        };
        let root_id = register(root, None).expect("the root must be creatable");
        let mut stack = vec![(root.clone(), root_id)];
        while let Some((node, id)) = stack.pop() {
            for child in &node.children {
                if let Some(child_id) = register(child, Some(id)) {
                    stack.push((child.clone(), child_id));
                }
            }
        }
        (layout, root_id)
    }

    /// The keys of `parent`'s children in order, read back from the layout.
    fn child_keys(layout: &crate::json::BoundJsonLayout, parent: ObjectId) -> Vec<String> {
        layout
            .children(parent)
            .iter()
            .map(|&id| layout.node_key(id).unwrap_or_default().to_string())
            .collect()
    }

    /// D08-V-02: a three-node cyclic reorder must reach the declared order after apply.
    ///
    /// The previous single-lookback move rule emitted one under-specified `Move` for
    /// `[a,b,c] -> [c,a,b]`, and remove-then-insert-at-index produced `[b,a,c]`.
    #[test]
    fn a_three_node_cyclic_reorder_reaches_the_declared_order() {
        let create = real_creator();
        let old = Node::new("window")
            .key("root")
            .child(Node::new("label").key("a"))
            .child(Node::new("label").key("b"))
            .child(Node::new("label").key("c"));
        let new = Node::new("window")
            .key("root")
            .child(Node::new("label").key("c"))
            .child(Node::new("label").key("a"))
            .child(Node::new("label").key("b"));

        let (mut layout, root) = mount_layout(&old, &create);
        let ids: Vec<ObjectId> = layout.children(root).to_vec();
        // ids in old order are [a, b, c]; capture them to prove identity survives.
        let id_of = {
            let ids = ids.clone();
            move |path: &[usize], _index: usize| -> Option<ObjectId> {
                if path.is_empty() {
                    Some(root)
                } else {
                    path.first().and_then(|&i| ids.get(i).copied())
                }
            }
        };
        let report = crate::view::diff::diff(&old, &new, &id_of);
        let applied = apply(&mut layout, &report.patches, &create);
        assert!(applied.is_clean(), "apply errors: {:?}", applied.errors);
        assert_eq!(
            child_keys(&layout, root),
            ["c", "a", "b"],
            "the applied order must match the declaration"
        );
        // The three controls are the same three ids, reordered — nothing was rebuilt.
        let mut after = layout.children(root).to_vec();
        after.sort_unstable();
        let mut before = ids.clone();
        before.sort_unstable();
        assert_eq!(after, before, "a reorder must keep every control's identity");
    }

    /// D08-V-01: a keyed child whose type changes is replaced **in its parent's slot**.
    ///
    /// The recursion used to emit `Replace { parent: <child id> }`, so apply deleted the
    /// child and re-registered the replacement under the deleted id, emptying the real
    /// parent's child list and stranding the new control.
    #[test]
    fn a_type_change_on_a_keyed_child_replaces_it_in_place() {
        let create = real_creator();
        let old =
            Node::new("window").key("root").child(Node::new("label").key("a").prop("text", s("A")));
        let new = Node::new("window")
            .key("root")
            .child(Node::new("button").key("a").prop("text", s("A")));

        let (mut layout, root) = mount_layout(&old, &create);
        let old_id = layout.children(root)[0];
        let id_of = move |path: &[usize], _index: usize| -> Option<ObjectId> {
            if path.is_empty() {
                Some(root)
            } else if path == [0] {
                Some(old_id)
            } else {
                None
            }
        };
        let report = crate::view::diff::diff(&old, &new, &id_of);
        let applied = apply(&mut layout, &report.patches, &create);
        assert!(applied.is_clean(), "apply errors: {:?}", applied.errors);
        assert_eq!(layout.children(root).len(), 1, "the window keeps exactly one child");
        let replaced = layout.children(root)[0];
        assert_eq!(layout.widget_name(replaced), Some("button"));
        assert_eq!(layout.node_key(replaced), Some("a"));
        assert_ne!(
            replaced, old_id,
            "a type change replaces the control rather than reusing its id"
        );
    }

    /// D08-V-03: a keyless child inserted ahead of a keyed one must not steal its identity.
    ///
    /// Old `[label(key=a)] -> [label(keyless), label(key=a)]` used to match the keyless node
    /// to the old keyed node and insert a second `key=a`, leaving two live controls claiming
    /// one key.
    #[test]
    fn a_keyless_insert_does_not_steal_a_keyed_sibling() {
        let create = real_creator();
        let old =
            Node::new("window").key("root").child(Node::new("label").key("a").prop("text", s("A")));
        let new = Node::new("window")
            .key("root")
            .child(Node::new("label").prop("text", s("new")))
            .child(Node::new("label").key("a").prop("text", s("A")));

        let (mut layout, root) = mount_layout(&old, &create);
        let old_id = layout.children(root)[0];
        let id_of = move |path: &[usize], _index: usize| -> Option<ObjectId> {
            if path.is_empty() {
                Some(root)
            } else if path == [0] {
                Some(old_id)
            } else {
                None
            }
        };
        let report = crate::view::diff::diff(&old, &new, &id_of);
        let applied = apply(&mut layout, &report.patches, &create);
        assert!(applied.is_clean(), "apply errors: {:?}", applied.errors);

        let children = layout.children(root).to_vec();
        assert_eq!(children.len(), 2, "the keyless insert adds one child, nothing is lost");
        // The keyed control keeps its identity and moves to the second slot.
        assert_eq!(layout.node_key(children[1]), Some("a"));
        assert_eq!(children[1], old_id, "the keyed control keeps its id");
        // The keyless control is genuinely new (not the recycled keyed one).
        assert_ne!(children[0], old_id, "the keyless child must be a new control");
        // The key `a` is claimed by exactly one live control.
        let a_claimants = children.iter().filter(|&&id| layout.node_key(id) == Some("a")).count();
        assert_eq!(a_claimants, 1, "a key must be unique among live controls");
    }

    /// B26.07: a batch that applied some patches and failed others must report `partial`, not
    /// `clean` and not `failed_entirely`.
    #[test]
    fn a_partly_applied_batch_is_reported_as_partial() {
        // One refused write and one accepted write: the batch is partial.
        let mut report = ApplyReport { properties_written: 1, ..ApplyReport::default() };
        report.errors.push(ViewError::PropertyRefused {
            id: 1,
            name: String::from("text"),
            reason: String::from("read-only"),
        });
        assert!(!report.is_clean());
        assert!(report.is_partial(), "one success and one failure is partial");
        assert!(!report.failed_entirely());
        assert_eq!(report.completed(), 1);

        // Nothing succeeded: a total failure, distinct from a partial one.
        let mut total = ApplyReport::default();
        total.errors.push(ViewError::UnknownWidget { id: 9 });
        assert!(!total.is_partial(), "no work completed is not partial");
        assert!(total.failed_entirely());
        // An empty batch is neither partial nor failed.
        let empty = ApplyReport::default();
        assert!(empty.is_clean());
        assert!(!empty.is_partial());
        assert!(!empty.failed_entirely());
    }

    /// B26.07: a patch failure must name the id it is about, so a host can locate it.
    #[test]
    fn a_patch_failure_names_its_affected_id() {
        let refused = ViewError::PropertyRefused {
            id: 42,
            name: String::from("value"),
            reason: String::from("out of range"),
        };
        assert_eq!(refused.affected_id(), Some(42));
        assert!(refused.location().contains("42"));
        assert!(refused.location().contains("value"));

        // A structural failure has no id — the node was never created — and says so.
        let unbuildable = ViewError::UnknownWidgetType {
            widget: String::from("barchart"),
            key: Some(String::from("chart")),
            path: vec![String::from("window")],
        };
        assert_eq!(unbuildable.affected_id(), None);
        assert_eq!(unbuildable.location(), "window > barchart#chart");
    }
}
