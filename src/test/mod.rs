// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Test infrastructure and utilities.
//!
//! # Reachability
//!
//! **State:** Reserved: shared test scaffolding exported so integration tests in separate crates can use it. Not part of the library's runtime surface. Removal condition: none; it is the deliberate test-support surface.
/// The in-process test harness: widget mounting, event injection, and
/// teardown assertions.
pub mod harness;
/// Assertion helpers for widget state and geometry.
pub mod matchers;
/// Golden-image / snapshot capture and comparison.
pub mod snapshot;
/// Shared data types used by the harness and matchers.
pub mod test_types;

pub use harness::*;
pub use matchers::*;
pub use snapshot::*;
pub use test_types::*;
