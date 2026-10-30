// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Probe that `crate::view` is compiled into the profile being built.
//!
//! This is the **committed** form, and it is `cfg`-gated so that a plain
//! `cargo check --all-targets` stays green on every profile: on `mini`/`embedded` the module does
//! not exist, and an unconditional import there would fail a build that is behaving correctly.
//!
//! `tools/check_view_platform_gate.sh` needs the opposite assertion — a build that *fails* is the
//! proof the module is absent — so it saves this file, writes a copy with the import forced
//! unconditional, runs its profiles, and restores this one on exit.

#![cfg(all(
    any(feature = "desktop", feature = "tablet", feature = "mobile"),
    not(feature = "no-declarative-view")
))]
#![allow(unused_imports)]
#![allow(dead_code)]

use rust_widgets::view::VIEW_GATE_PROBE;

#[test]
fn view_gate_probe_resolves() {
    assert!(!VIEW_GATE_PROBE.is_empty());
}
