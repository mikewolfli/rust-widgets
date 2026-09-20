// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Contract tests for [`super`] — the Darwin probes must measure, not guess.
//!
//! The defect these tests exist to prevent: four Apple backends forwarded their memory
//! and power probes to `crate::platform::os_probes` (a `/proc` + `/sys` parser) and so
//! returned `None` / `false` on every Apple host. Nothing failed, because "unknown" is a
//! legal answer. A test that only asserted "no panic" would have passed throughout.
//!
//! Every assertion below therefore pins an *observable measurement* on a real Darwin
//! host: the argv selectors must stay BSD-spelled, and the figures must be positive.

use super::{PMSET_ARGUMENTS, PS_SELECTOR};
// Imported through `crate::compat` rather than `std` so the file compiles under the
// `alloc_frugal` profiles (`mini`), where there is no `std` prelude to supply these.
use crate::compat::{String, ToString};

/// The host must report a plausible, positive amount of installed memory.
///
/// `sysconf(_SC_PHYS_PAGES)` returns the machine's page budget; on every Darwin machine
/// this is at least a few hundred mebibytes and below a terabyte-scale ceiling. The
/// bounds are deliberately loose — they are not a hardware claim, they are a check that
/// the probe returned a *measurement* rather than the `None` the `/proc` path produced.
#[test]
fn total_memory_is_measured_and_plausible() {
    let mb = super::total_memory_mb().expect(
        "sysconf(_SC_PHYS_PAGES) must yield installed memory on a Darwin host; a None here \
         means the probe regressed to an OS source this platform does not have",
    );
    assert!(mb >= 256, "installed memory {mb} MiB is implausibly small");
    assert!(mb <= 4 * 1024 * 1024, "installed memory {mb} MiB exceeds any plausible host");
}

/// The RSS/VSZ ratio must come back as a clamped fraction, not `None`.
///
/// `ps -o rss=,vsz=` always has an answer for a live pid, so `None` here is a probe
/// failure rather than an unknown. The ratio is a fraction by definition, and RSS never
/// exceeds VSZ, so the natural bound is `[0, 1]`.
#[test]
fn process_memory_utilization_is_a_fraction() {
    let ratio = super::process_memory_utilization().expect(
        "ps -o rss=,vsz= must answer for the current pid; a None here means the selector \
         or the output parsing regressed",
    );
    assert!((0.0..=1.0).contains(&ratio), "memory utilization {ratio} is outside [0, 1]");
    assert!(ratio > 0.0, "a running process must have a non-zero resident share");
}

/// `is_on_battery` must agree with what `pmset` actually printed.
///
/// Rather than assert a host-specific value (this machine may or may not have a
/// battery), the test re-runs the documented command and requires the probe to match the
/// banner. That pins the *contract* — the probe answers from `pmset`, not from a
/// hardcoded `false` — without making the suite depend on the host's power source.
#[test]
fn battery_answer_matches_the_pmset_banner() {
    let Ok(output) = std::process::Command::new("pmset").args(PMSET_ARGUMENTS).output() else {
        // No `pmset` (a stripped container): the probe is documented to answer `false`.
        assert!(!super::is_on_battery(), "without pmset the probe must answer false");
        return;
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let expected = text.contains("'Battery Power'");
    assert_eq!(
        super::is_on_battery(),
        expected,
        "probe disagreed with the pmset banner it is documented to read"
    );
}

/// `ps` must reject the selector list if a future edit "tidies" it into GNU spelling.
///
/// BSD `ps` treats a selector list without a leading `-` as the format argument; the
/// trailing `=` suppresses the header row. Both halves matter: without the `=`, the first
/// whitespace-separated token would be the literal `RSS` and parsing would fail.
#[test]
fn ps_selector_is_bsd_spelled() {
    assert_eq!(PS_SELECTOR, ["-o", "rss=,vsz="]);
    let output = std::process::Command::new("ps")
        .args(PS_SELECTOR)
        .arg("-p")
        .arg(std::process::id().to_string())
        .output()
        .expect("ps must be spawnable on Darwin");
    assert!(output.status.success(), "ps rejected {PS_SELECTOR:?}");
    let text = String::from_utf8_lossy(&output.stdout);
    let first = text.split_whitespace().next().expect("ps printed a figure for a live pid");
    assert!(
        first.parse::<u64>().is_ok(),
        "first token '{first}' is not a figure — the header suppression `=` was lost"
    );
}

/// The parse-failure message must name the raw output so a log reader can diagnose it.
#[test]
fn parse_error_message_names_the_offending_output() {
    let message = super::ps_parse_error("  RSS   VSZ\n");
    assert!(message.contains("RSS"), "message dropped the raw output: {message}");
    assert!(message.contains("rss vsz"), "message does not name the expected pair: {message}");
}
