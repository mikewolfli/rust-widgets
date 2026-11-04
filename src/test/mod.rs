// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Test infrastructure and utilities.
//!
//! # Reachability
//!
//! **State:** Reserved: shared test scaffolding exported so integration tests in separate crates can use it. Not part of the library's runtime surface.
//!
//! **Removal condition:** when every in-repo test that uses it has migrated to an external
//! `rust-widgets-testkit` crate, or when the cookbook's harness chapter is retired (XC-07). Until
//! then it stays ungated by design: a feature gate would make the documented test surface
//! disappear from the default build the cookbook compiles against.
/// The in-process test harness: widget mounting, event injection, and
/// teardown assertions.
pub mod harness;
/// Assertion helpers for widget state and geometry.
pub mod matchers;
/// Golden-image / snapshot capture and comparison.
pub mod snapshot;
/// Shared data types used by the harness and matchers.
pub mod test_types;
/// Tree-level assertions for a whole declarative or mounted tree (RW-57).
///
/// Gated on the declarative layer: a `mini`/`embedded` build has no `view` tree to assert on, and
/// the module's subject is exactly that tree.
#[cfg(declarative_view)]
pub mod tree_assert;

pub use harness::*;
pub use matchers::*;
pub use snapshot::*;
pub use test_types::*;
#[cfg(declarative_view)]
pub use tree_assert::*;
