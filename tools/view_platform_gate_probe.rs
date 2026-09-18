// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The `crate::view` presence probe used by `tools/check_view_platform_gate.sh`.
//!
//! Two different consumers need two different versions of this file, so the
//! committed copy is the *tolerant* one:
//!
//! * `cargo check --all-targets` and `check_profiles.sh` compile this committed
//!   file under every profile, including `mini`/`embedded` where `crate::view`
//!   is intentionally absent. The import below is therefore `cfg`-gated on the
//!   `declarative_view` alias, so those runs succeed trivially — they are not
//!   asking the gate question. (Without this gate the file failed to resolve on
//!   `mini`/`embedded` with `E0432: unresolved import rust_widgets::view`, which
//!   turned the sanctioned absence of the layer into a build failure.)
//! * `check_view_platform_gate.sh` needs the *forced* form, because there a build
//!   failure **is** the assertion that the module is absent. The script saves this
//!   file, overwrites it with the unconditional import, runs the eight profiles,
//!   and restores the original on exit.
//!
//! Keeping the gate alias rather than a hand-written conjunction is deliberate:
//! it is the same single source of truth (`build.rs`) that `src/lib.rs` uses.

#![allow(unused_imports)]
#![allow(dead_code)]

#[cfg(declarative_view)]
use rust_widgets::view::VIEW_GATE_PROBE;

/// On a device profile this asserts the probe symbol really is exported; on a
/// stripped profile it is a no-op, matching the sanctioned absence of the layer.
#[test]
fn view_gate_probe_resolves() {
    #[cfg(declarative_view)]
    assert!(!VIEW_GATE_PROBE.is_empty());
}
