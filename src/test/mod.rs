// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Test infrastructure and utilities.
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
