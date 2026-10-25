// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Darwin-family system probes shared by the macOS (cocoa and objc2) and iOS backends.
//!
//! # Why this module exists
//!
//! `Platform::total_memory_mb`, `is_on_battery` and `process_memory_utilization` are
//! answered by every backend, and each answer must come from an API the host actually
//! has. The cocoa backend, the objc2 preview backend, the iOS backend and the generic
//! mobile backend all landed on `crate::platform::os_probes`, which parses
//! `/proc/meminfo`, `/sys/class/power_supply` and `/proc/self/status` — files that only
//! exist on a **Linux** kernel. On Apple hardware every one of those probes silently
//! degraded to the "unknown" answer (`None` / `false`), so a macOS build reported no
//! installed memory and no process footprint while the trait documentation promised
//! `sysctl hw.memsize` and `ps`.
//!
//! The bug was invisible because the "unknown" answer is a *legal* return value: nothing
//! panicked, nothing logged. `Platform::total_memory_mb`'s own doc comment forbids
//! substituting a made-up constant, so the fix cannot be to invent a number either — it
//! has to be an actual Darwin probe.
//!
//! # Why /proc was the wrong answer even where it "works"
//!
//! iOS does not ship a readable `/proc/meminfo` (the sandbox hides it), so the iOS
//! backend was in the same state as macOS. Android *and* HarmonyOS are Linux-kernel
//! systems and keep using [`crate::platform::os_probes`] correctly — that is the
//! documented contract of that module, and this one does not replace it.
//!
//! # Cost
//!
//! `sysconf` is a libc call with no file I/O. The `pmset` and `ps` probes each spawn a
//! short-lived child process, so their answers are cached for a short TTL
//! (`BATTERY_CACHE_TTL`, `MEMORY_CACHE_TTL`): `AdaptivePerformanceMonitor` samples memory
//! every frame, and spawning `ps` at 60 Hz was a measurable share of a frame. A TTL is the
//! honest middle ground between "never ask" and "ask 60 times a second": the figure is live to
//! within a few seconds, and the cache is stamped rather than permanent.

#[cfg(unix)]
use crate::compat::ToString;
use crate::compat::{format, String};

/// How long a `ps` sample is reused.
///
/// Shorter than the battery TTL because memory pressure can change within a frame burst; five
/// seconds is long enough that a sustained render loop spawns one `ps`, not hundreds.
const MEMORY_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(5);

/// How long a `pmset` answer is reused.
///
/// Power source changes on the scale of seconds at the fastest (a user unplugging a laptop), and
/// a stale answer for half a minute cannot strip animations for longer than that.
const BATTERY_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(30);

/// Runs `probe` at most once per `ttl`, returning the cached value in between.
fn cached_probe<T: Copy>(
    slot: &std::sync::Mutex<Option<(std::time::Instant, T)>>,
    ttl: std::time::Duration,
    probe: impl FnOnce() -> T,
) -> T {
    // The lock is never held across `probe`: spawning a child process under a shared lock would
    // serialise unrelated callers for the length of that process.
    let cached = slot
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .and_then(|(stamped, value)| (stamped.elapsed() < ttl).then_some(value));
    if let Some(value) = cached {
        return value;
    }
    let value = probe();
    *slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) =
        Some((std::time::Instant::now(), value));
    value
}

/// Reads installed physical memory in mebibytes via `sysconf(_SC_PHYS_PAGES)`.
///
/// This is the same quantity `sysctl hw.memsize` returns and the same quantity the
/// Linux probe reads from `/proc/meminfo`, so the two backends answer the question
/// comparably. `_SC_PHYS_PAGES` already accounts for the pages the kernel can hand out,
/// which is why it is preferred over `hw.memsize` — no FFI binding is needed for
/// `sysctl` itself.
///
/// Returns `None` when the sysconf query fails, which keeps the "unknown" answer
/// available instead of substituting a constant.
#[cfg(unix)]
pub fn total_memory_mb() -> Option<u64> {
    // SAFETY: `sysconf` is a pure libc query; both names are compile-time constants
    // from the Darwin `<unistd.h>` limit set and take no pointers.
    let pages = unsafe { libc::sysconf(libc::_SC_PHYS_PAGES) };
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    if pages <= 0 || page_size <= 0 {
        return None;
    }
    let bytes = (pages as u64).checked_mul(page_size as u64)?;
    Some(bytes / (1024 * 1024))
}

/// Reports whether the machine is currently drawing from its battery.
///
/// `pmset -g batt` prints `Now drawing from 'AC Power'` or `Now drawing from
/// 'Battery Power'`; the latter is the only discharging answer. A desktop Mac has no
/// battery and prints `'AC Power'`, which is the same `false` a Linux tower reports
/// from an empty `/sys/class/power_supply`.
///
/// Returns `false` when `pmset` cannot be spawned or prints neither phrase — the safe
/// direction, because a wrong "on battery" would silently strip animations.
#[cfg(unix)]
pub fn is_on_battery() -> bool {
    static CACHE: std::sync::Mutex<Option<(std::time::Instant, bool)>> =
        std::sync::Mutex::new(None);
    cached_probe(&CACHE, BATTERY_CACHE_TTL, || {
        let Ok(output) = std::process::Command::new("pmset").args(["-g", "batt"]).output() else {
            return false;
        };
        // The phrase is ASCII, so a lossy decode is sufficient and avoids discarding the
        // answer over an unrelated byte sequence later in the banner.
        let text = String::from_utf8_lossy(&output.stdout);
        text.contains("'Battery Power'")
    })
}

/// Samples this process's resident memory against its virtual size, clamped to `[0, 1]`.
///
/// `ps -o rss=,vsz=` reports both figures in kibibytes for one pid, which is the same
/// RSS/VmSize ratio the Linux probe computes from `/proc/self/status`. The ratio is a
/// cheap proxy for how much of the mapped address space is resident; `vsz` is the
/// denominator, so a process that has mapped a large file reports a low figure.
///
/// Returns `None` when `ps` cannot be spawned, prints no parsable pair, or reports a
/// non-positive virtual size (a zero denominator has no utilisation to speak of).
#[cfg(unix)]
pub fn process_memory_utilization() -> Option<f32> {
    static CACHE: std::sync::Mutex<Option<(std::time::Instant, Option<f32>)>> =
        std::sync::Mutex::new(None);
    cached_probe(&CACHE, MEMORY_CACHE_TTL, || {
        let pid = std::process::id().to_string();
        let output =
            std::process::Command::new("ps").args(["-o", "rss=,vsz=", "-p", &pid]).output().ok()?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let mut fields = text.split_whitespace();
        let rss_kb: f64 = fields.next()?.parse().ok()?;
        let vsz_kb: f64 = fields.next()?.parse().ok()?;
        if vsz_kb <= 0.0 {
            return None;
        }
        Some((rss_kb / vsz_kb).clamp(0.0, 1.0) as f32)
    })
}

/// The `ps` selector this module relies on, exposed so a test can assert the spelling.
///
/// `ps -o rss=,vsz=` is BSD syntax (no leading `-` on the selector list, `=` to drop the
/// header). A future edit that "tidies" it into `--format` would silently make
/// [`process_memory_utilization`] return `None` on every Darwin host, so the selector is
/// pinned by a test rather than left as a literal inside the function.
pub const PS_SELECTOR: [&str; 2] = ["-o", "rss=,vsz="];

/// The `pmset` argument list, exposed for the same reason as [`PS_SELECTOR`].
pub const PMSET_ARGUMENTS: [&str; 2] = ["-g", "batt"];

/// Formats the parse failure the `ps` probe must report rather than guess at.
///
/// Kept here so the probe and its tests agree on one message; returns the text a caller
/// would see in a log when `ps` prints something unparsable.
pub fn ps_parse_error(raw: &str) -> String {
    format!("ps output '{}' did not contain an 'rss vsz' pair", raw.trim())
}

#[cfg(test)]
#[path = "darwin_probes_tests.rs"]
mod tests;
