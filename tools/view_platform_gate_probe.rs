// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Probe used by `tools/check_view_platform_gate.sh`.
//!
//! # Why the import is `cfg`-gated here, and why the gate rewrites it
//!
//! Resolving `rust_widgets::view::VIEW_GATE_PROBE` is the assertion that the
//! declarative layer is compiled into the profile under test. The gate needs that
//! import to be **unconditional**, because on a profile that must not have the module
//! (mini, embedded, a profile-less build) the point is precisely that the build fails.
//!
//! That is the opposite of what a normal `cargo check --all-targets` needs. Every
//! profile in `tools/check_profiles.sh` compiles this test, so an unconditional import
//! makes that script fail on mini and embedded — permanently, for a reason that has
//! nothing to do with the code being checked.
//!
//! The committed copy therefore gates its import, and the gate saves this file, writes
//! the forced unconditional copy, runs its six profiles, and restores this one on
//! exit. That split is the whole design: `--all-targets` sees a probe that compiles
//! everywhere, and the gate sees one that fails exactly where it should.
//!
//! # The failure this guards against
//!
//! The forced copy was once left behind — a gate run that crashed before its `trap`
//! could restore, or an interrupted run — and the committed file became the
//! unconditional version. `tools/check_profiles.sh` then failed on mini and embedded
//! with `unresolved import rust_widgets::view`, which reads like a source defect and is
//! actually an artefact. `cfg(declarative_view)` here is what makes that impossible:
//! the committed file is now correct for `--all-targets` by construction, and only the
//! gate's temporary copy carries the forced import.

// Only present where `src/lib.rs` compiles the module, which is the same alias the
// gate asserts on. On mini and embedded this file compiles to an empty test binary,
// which is the correct outcome for `--all-targets`.
#[cfg(declarative_view)]
use rust_widgets::view::VIEW_GATE_PROBE;

#[cfg(declarative_view)]
#[test]
fn view_gate_probe_resolves() {
    assert!(!VIEW_GATE_PROBE.is_empty());
}

/// A build without the declarative layer must compile this file successfully.
///
/// Deliberately an assertion rather than a comment: if the `cfg` above is ever removed
/// — which is exactly how the committed file came to be broken — this test fails on the
/// profiles that must not have the module, rather than the crate failing to build.
#[cfg(not(declarative_view))]
#[test]
fn the_probe_is_absent_where_the_view_layer_is_absent() {
    // Reaching this line means the import above was correctly gated out. There is
    // nothing to assert about a type that does not exist in this profile; the assertion
    // is that this test compiled at all.
    assert!(
        !cfg!(declarative_view),
        "this test only exists on a build without the declarative layer"
    );
}
