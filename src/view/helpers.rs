// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Predicates and arithmetic shared by `apply`, `diff` and `engine`.
//!
//! Kept out of `mod.rs` (principle #8). Every one of these is *deliberately* a single
//! definition consumed by three layers: the comments below record the drift each replaced.

use super::node::Node;

/// Whether `name` is the carrier a **generated** program uses to travel a declared wire on a
/// [`Node`].
///
/// # Why this lives here rather than in the generator
///
/// The prefix is written by `crate::designer`, but the thing that has to *recognise* it is the
/// engine: a wire carrier is not a control property, so every path that writes a node's props has
/// to leave it alone. Spelling the prefix at each of those sites is how one of them comes to be
/// missed — which is exactly what happened (`ViewEngine::write_declared_properties` wrote it and
/// drew a `PropertyRefused` for every wired node). One predicate, called from every writer, is the
/// shape that cannot drift.
///
/// # Why the literal, and not `crate::designer::WIRE_PROP_PREFIX`
///
/// `crate::designer` is gated on `designer_tooling` (it parses a project document, so it needs
/// `serde_json`), while `crate::view` exists on every device profile. Reading the constant from
/// there made `view` fail to compile on `tablet`/`mobile`, where the generator's module is absent —
/// the probe crate in `check_generator_output_compiles.sh` caught it. `tools/check_generator_agrees_with_registry.sh`
/// is not enough on its own here: it runs under `desktop`, where `designer` *is* present.
///
/// The literal is therefore duplicated deliberately, and `tests/`-level agreement is what keeps the
/// two in step: `crate::designer` asserts in its own tests that
/// `is_generated_wire_prop(WIRE_PROP_PREFIX)` is true, so changing one without the other fails.
///
/// Kept as a free `pub(crate)` function rather than a method on `Node`, because the carrier is the
/// generator's vocabulary and not part of the declarative model: a caller of `crate::view` never
/// sets it, only reads the tree the generator built (principle #4 — mechanism stays out of the
/// model the caller describes).
pub(crate) fn is_generated_wire_prop(name: &str) -> bool {
    name.starts_with(GENERATED_WIRE_PROP_PREFIX)
}

/// The wire-carrier prefix a generated program uses, spelled once for this module.
///
/// See [`is_generated_wire_prop`] for why this is not read from `crate::designer`.
pub(crate) const GENERATED_WIRE_PROP_PREFIX: &str = "__wire_";

/// Whether a node is **transparent**: it declares layout intent but produces no control.
///
/// # Why this is one predicate rather than three call sites
///
/// A transparent node's children are still created — they are hosted by the nearest
/// ancestor that *did* produce a control, so they take the transparent node's place in
/// that ancestor's child list. Three different layers have to agree about both halves of
/// that rule:
///
/// - [`apply`](crate::view::apply) inserts a transparent node's children into the parent at
///   the slot the node itself would have occupied;
/// - [`ViewEngine::mount`](crate::view::ViewEngine::mount) creates them the same way, and
///   [`ViewEngine::reindex_paths`](crate::view::ViewEngine) resolves their declared paths
///   against that same host and slot;
/// - [`diff`](crate::view::diff) compares them in place instead of re-inserting the
///   subtree, which is what keeps an unchanged gap from being rebuilt on every frame.
///
/// Spelling the name at each of those sites is how one of them drifts — and it already had:
/// `apply` knew about `spacer`, `mount` did not, so a spacer mounted through the engine
/// reported `UnknownWidgetType` and lost the subtree its declaration describes.
///
/// # Why `spacer` only
///
/// The JSON front end's transparent node is `spacer` (`json/loader.rs`), and `apply` has
/// always matched exactly that name. A "layout" name is not a node in either front end —
/// layout is the `layout` *property* of a container — so accepting it here would invent a
/// fourth vocabulary. A pseudo-widget nobody can declare is a name no caller can reach.
pub(crate) fn is_transparent_node(node: &Node) -> bool {
    node.widget.eq_ignore_ascii_case("spacer")
}

/// How many controls `node` contributes to its parent's child list: one, or — for a
/// transparent node — the sum contributed by its children.
///
/// This is the arithmetic behind "a gap still occupies a slot": a declared index and a
/// *mounted* slot differ exactly by the flattened size of the transparent siblings before
/// them, so `move_child_to`, `Patch::Insert { index }` and the reindex walk all have to use
/// the same sum. It is a free function rather than three private copies for that reason.
///
/// Recursion is bounded by the declaration: `spacer` inside `spacer` collapses the same way,
/// and the deepest chain a view can express is its own nesting depth.
pub(crate) fn mounted_slots(node: &Node) -> usize {
    if is_transparent_node(node) {
        node.children.iter().map(mounted_slots).sum()
    } else {
        1
    }
}

/// A compile-time probe for the platform gate (BLUE18 rule #92 / #94).
///
/// It exists so the gate has an **executable** criterion rather than a source grep:
/// `tools/check_view_platform_gate.sh` compiles a tiny program that names this
/// constant, and the build succeeds on the profiles that must have the declarative
/// layer and fails on the ones that must not. A grep for `src/view` in a build log
/// would also match a path in an unrelated diagnostic, so it is not the criterion.
///
/// The string is the module path, which makes a stray `println!` of it in a failing
/// gate run say exactly what was linked in.
///
/// It is `pub` and never read in the crate: that is the point — its only purpose is
/// to be *resolvable or not*. The gate is what consumes it.
#[doc(hidden)]
pub const VIEW_GATE_PROBE: &str = "rust_widgets::view";
