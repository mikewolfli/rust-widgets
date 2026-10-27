// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! A gate's probe, not a test: `tools/check_view_platform_gate.sh` owns the assertion.
//!
//! # What the gate does with this file
//!
//! It saves this file, overwrites it with a copy whose `use` is **forced unconditional**, compiles
//! that copy under the six profiles it knows, and restores this version on exit. Resolving the
//! name is the assertion that `crate::view` is in the compilation unit; failing to resolve is the
//! assertion that it is absent.
//!
//! # Why the *committed* form is `cfg`-gated
//!
//! This file is a `[[test]]` target, so it is built by every `cargo check --all-targets` and
//! `cargo test` in the repo — including on `mini`/`embedded`, where `crate::view` does not exist.
//! The committed copy therefore carries no assertion of its own: it is gated on the same
//! `declarative_view` alias `src/lib.rs` uses, so it compiles trivially everywhere and the gate's
//! forced copy is the only thing that ever makes a claim.
//!
//! # Why this comment exists at all
//!
//! The tree once carried the **forced** copy as the committed file — a run that was killed before
//! its `trap … EXIT` could restore the original. Nothing noticed until `check_harmony_cross.sh`
//! built every target of a stripped profile and failed on a "test" that only the gate should ever
//! build. That is what the gate's save/restore is for, and what this note is here to prevent
//! someone "simplifying" the gate away.

#[cfg(declarative_view)]
use rust_widgets::view::VIEW_GATE_PROBE;

#[cfg(declarative_view)]
#[test]
fn view_gate_probe_resolves() {
    assert!(!VIEW_GATE_PROBE.is_empty());
}
