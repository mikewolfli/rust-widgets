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
//! **State:** the declarative retained-tree implementation. The mounting/updating entry points
//! are [`ViewEngine::mount`], [`ViewEngine::update`], and — for a caller that already owns a
//! [`Node`] tree — [`ViewEngine::mount_tree`] / [`ViewEngine::update_tree`].
//!
//! Not duplicated by `crate::json`: the JSON loader parses *one* tree and instantiates it once,
//! with no previous tree to compare against, so it cannot preserve an identity across an
//! update. This module starts where that ends — it consumes the same widget factory and the
//! same property contract (`widget_property_set`) — and `BoundJsonLayout`'s structural indexes
//! are its binding to the live controls.
//!
//! # One implementation path (G-406)
//!
//! `crate::layout::declarative` still owns a second declarative retained tree: a thread-local
//! `LAYOUT_MAP` keyed by parent id whose identity is the **path** a document happens to place a
//! control at, re-solved in full on every `apply_layout`. This module is the one that carries
//! key identity, minimal-move patching and scoped diffs. The decision (principle #101) is that
//! this engine is the **single** implementation, so the declarative loader must hand its
//! produced [`Node`] tree to [`ViewEngine::mount_tree`] / [`ViewEngine::update_tree`] instead of
//! keeping the second registry, and the engine must own the layout registration that `LAYOUT_MAP`
//! performs today. Until that forward lands, the two paths coexist: this one is complete and
//! tested, and the loader's path-only tree is the one production still applies.
//!
//! Removal condition: this module is not a candidate for removal while the declarative loader
//! has its own path-only retained tree. It goes only if that loader is retired together with
//! `BoundJsonLayout`'s tree indexes (this module's other consumer).
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
mod breakpoint;
mod diff;
mod engine;
mod helpers;
mod identity;
mod node;
mod reactive;
mod revision;
mod row_signal;

pub use apply::{
    apply, apply_with_icon_resolver, apply_with_reserved_overlay_and_icon_resolver, ApplyReport,
    ReservedIds, ViewError,
};
pub use breakpoint::{is_current, with_breakpoint, Breakpoint};
pub use diff::{diff, DiffReport, Patch};
pub use engine::{Context, View, ViewEngine};
pub use identity::TreeIndex;
pub use node::{Host, Node};
pub use reactive::{reactive_pump_event, ReactiveHost, REACTIVE_PUMP_EVENT};
pub use row_signal::{RowChange, RowSignal};

pub(crate) use helpers::{is_generated_wire_prop, is_transparent_node, mounted_slots};
// The prefix itself has exactly one consumer: the designer's test asserting that
// `designer::generator::WIRE_PROP_PREFIX` and this one cannot drift. Re-exporting it only
// for that build keeps the non-test build warning-free (principle #10), and the
// `designer_tooling` half keeps it warning-free on the profiles that have no designer at all
// (tablet/mobile/mini/embedded), where the only reader is compiled out.
#[cfg(all(test, designer_tooling))]
pub(crate) use helpers::GENERATED_WIRE_PROP_PREFIX;
pub use helpers::VIEW_GATE_PROBE;
