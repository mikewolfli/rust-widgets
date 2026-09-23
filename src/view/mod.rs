// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Declarative-retained view layer: state → [`Node`](crate::view::Node) →
//! [`diff`](crate::view::diff) → [`Patch`](crate::view::Patch) → retained tree.
//!
//! # What this module is
//!
//! The rest of this library is **retained-mode**: a control is a long-lived object with an
//! [`ObjectId`](crate::core::ObjectId), and callers mutate it in place. That is not in
//! tension with being **declarative** — React, SwiftUI, and the view layers of the platform toolkits are all declarative
//! *and* retained. The two questions are orthogonal (BLUE18 rule #85):
//!
//! | Axis | This library |
//! |---|---|
//! | **Who owns state?** | Retained: controls persist, carrying focus, scroll, and internal state |
//! | **Who describes structure?** | *This module's callers*: a [`View`](crate::view::View) describes the tree as a function of state |
//!
//! [`Node`](crate::view::Node) is the declarative half — a plain value describing a tree.
//! [`diff`](crate::view::diff) compares two such values and produces
//! [`Patch`](crate::view::Patch)es, and [`apply`](crate::view::apply) carries them onto the
//! live controls, which stay exactly as they were.
//!
//! # Additive, never a rewrite
//!
//! Nothing here changes an existing control, the [`WidgetKind`](crate::widget::WidgetKind)
//! enum, the widget factory, or the property contract (BLUE18 rule #86). This layer
//! **consumes** those contracts:
//!
//! - widget type names resolve through the same table the JSON loader uses;
//! - `Patch::SetProperty` writes through each control's published property contract;
//! - a tree can equally well be built by hand with `add_child`, because that path is
//!   untouched.
//!
//! A caller who never wants to rebuild a tree pays nothing: `Node` and `diff` are inert
//! data and arithmetic with no registration, no global state, and no platform calls.
//!
//! # `Node` is not a `WidgetKind`
//!
//! A [`WidgetKind`](crate::widget::WidgetKind) *classifies a control that already exists*.
//! A [`Node`](crate::view::Node) *names a control to create*. Several names can resolve to
//! one kind (aliases), and a declarative name may produce no control at all (a `spacer`), so
//! collapsing the two would lose the distinction the loader relies on (BLUE18 rule #49).
//!
//! # When not to use this
//!
//! - Building a tree **once** and never changing it: `JsonLoader::load` already does that,
//!   and a diff over a tree that never changes is pure overhead. Call
//!   [`diff`](crate::view::diff) only when there is a previous tree to compare against.
//! - One-off imperative edits (`btn.set_text("x")`): a single property write is cheaper
//!   than describing a whole tree to produce one patch.
//! - Animating a value frame by frame: animation is [`PropertyAnimation`]'s job. This layer
//!   expresses *target* state, and mixing in time would make the diff non-deterministic.
//!
//! [`PropertyAnimation`]: crate::style::PropertyAnimation
//!
//! # Context propagation
//!
//! A [`View`] describes its tree from its own state, plus an optional [`Context`]: a
//! small `String`-keyed map a root supplies so a value can be **shared by an entire
//! subtree** without being threaded through every intermediate node by hand (a web UI
//! framework's `createContext`/`useContext`, Material's `InheritedWidget`, SwiftUI's
//! `@Environment`, resolved once here).
//!
//! The key property is **when** it is read: [`View::build_with`] resolves context values
//! into concrete node properties *before* the tree is returned, so `diff` sees an ordinary
//! value-settled tree and never needs the context to compare two builds. If nodes held a
//! live "look upward" reference instead, `build` would stop being a pure function of what
//! it can see and the diff could no longer settle.
//!
//! # Reachability
//!
//! **State:** Reserved: the declarative half of the retained/declarative split, complete and
//! tested, whose consumers today are this repository's own tests. Retained deliberately
//! rather than deleted because it is the only implementation of a capability the library
//! otherwise lacks: expressing a tree as a function of state and carrying only the
//! differences onto the live controls. Its cost is bounded — `Node` and `diff` are inert data
//! and arithmetic with no registration, no global state, and no platform calls, so a caller
//! that never rebuilds a tree links nothing extra and pays nothing at runtime.
//!
//! Not duplicated by `crate::json`. The JSON loader parses *one* tree and instantiates it
//! once; it has no previous tree to compare against, so it cannot preserve an identity across
//! an update. This module starts where that ends — it consumes the same widget factory and
//! the same property contract (`widget_property_set`), and `BoundJsonLayout`'s structural
//! indexes. `grep -c "diff\|Patch" src/json/loader.rs` is `0`: the two do not overlap.
//!
//! Removal condition: if no view consumer appears by the time the declarative path is
//! re-evaluated, and `BoundJsonLayout`'s tree indexes (their only consumer) are retired with
//! it, this module goes too.
//!
//! # Platform availability (BLUE18 rule #92)
//!
//! This module is a **heavyweight optional part** and is compiled only where a device
//! profile supplies a real widget tree to carry it:
//!
//! | Profile | Structure description | State ownership | This module |
//! |---|---|---|---|
//! | `desktop` / `tablet` / `mobile` | **declarative** (`View` + `diff`) **or** imperative (`add_child`), mixable | retained | ✅ compiled |
//! | `embedded` | **imperative** (`add_child` / `create_*`) | retained | ❌ **absent** |
//! | `mini` | **imperative**, under `alloc_frugal` | retained | ❌ **absent** |
//!
//! The gate is the same expression `crate::json` uses — a device profile **and**
//! `widgets_unstripped` — so a build with no device profile (`--no-default-features
//! --features gpu`) and a stripped build both leave this module out. Widening it to
//! `any(desktop, tablet, mobile)` alone would bring the declarative layer into `mini`
//! and `embedded`, where its `String`/`Vec`/`HashMap` allocation budget does not fit
//! and where there is no caller that re-evaluates a view per frame, so a diff would
//! have no consumer (rules #28/#93).
//!
//! A reader on `mini`/`embedded` should treat this module as **not existing**: the
//! supported structure API there is `add_child` and the `create_*` functions.
//! `tools/check_view_platform_gate.sh` asserts both directions of that claim.

mod apply;
mod diff;
mod engine;
mod node;
mod reactive;

pub use apply::{apply, ApplyReport, ViewError};
pub use diff::{diff, DiffReport, Patch};
pub use engine::{Context, View, ViewEngine};
pub use node::{Host, Node};
pub use reactive::ReactiveHost;

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
