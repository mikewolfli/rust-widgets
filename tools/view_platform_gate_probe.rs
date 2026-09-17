// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Compile-time probe for `tools/check_view_platform_gate.sh` (BLUE18 rules #92/#94).
//!
//! # What this file is for
//!
//! The gate asserts that the declarative view layer (`crate::view`) is compiled on
//! `desktop` / `tablet` / `mobile` and **absent** on `mini` / `embedded`. Its
//! criterion has to be executable — grepping `src/lib.rs` for the `cfg` text proves
//! only that the text is present, not that it has the intended effect, and grepping
//! a build log for `src/view` also matches a path inside an unrelated diagnostic.
//!
//! So the criterion is name resolution. The `use` below names a symbol exported from
//! the module; it resolves exactly when the module is part of the compilation unit,
//! and fails to resolve when it is not. That is the same resolution a real caller
//! performs, which is what makes this a proof rather than a restatement.
//!
//! # Why the import is `cfg`-gated rather than unconditional
//!
//! A permanent unconditional `use rust_widgets::view::VIEW_GATE_PROBE;` would break
//! every `--all-targets` build on `mini` / `embedded`: those profiles are *supposed*
//! not to have the module, so the import would fail there and report the very
//! condition the gate exists to detect. Gating the import on the same expression the
//! library uses keeps this file buildable everywhere.
//!
//! That gating does not weaken the probe, because the gate does not read this file's
//! *content* — it reads whether a target that names the module **builds**. The gate
//! therefore compiles a *generated* copy of this file with the import forced
//! unconditional, under each profile, and reads the build's exit status. The
//! committed version is the fallback that keeps `--all-targets` green.

#[cfg(all(
    any(feature = "desktop", feature = "tablet", feature = "mobile"),
    widgets_unstripped
))]
#[allow(unused_imports)]
use rust_widgets::view::VIEW_GATE_PROBE;

// With the gate expression satisfied, reading the constant proves the path resolved
// to the module rather than to some other item of the same name.
#[cfg(all(any(feature = "desktop", feature = "tablet", feature = "mobile"), widgets_unstripped))]
#[test]
fn view_gate_probe_resolves_on_a_device_profile() {
    assert!(!VIEW_GATE_PROBE.is_empty());
}

// On a stripped profile the module must be absent, and this file must still build —
// the gate's *negative* assertion is performed by its generated copy, which names
// the module unconditionally and is expected to fail to compile here.
#[cfg(not(all(
    any(feature = "desktop", feature = "tablet", feature = "mobile"),
    widgets_unstripped
)))]
#[test]
fn view_gate_probe_is_skipped_on_a_stripped_profile() {
    // Reaching here means this target built without the declarative layer, which is
    // the correct state for this profile. The gate's generated probe, not this
    // assertion, is what fails if the module is wrongly present.
}
