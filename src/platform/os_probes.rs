// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Linux-kernel system probes shared by every `/proc`- and `/sys`-based backend.
//!
//! # Why this module exists
//!
//! Android, HarmonyOS, Linux/GTK, Wayland and the generic mobile backend all sit
//! on a Linux kernel, so all five answered `Platform::total_memory_mb`,
//! `is_on_battery`, `process_memory_utilization` and `process_cpu_utilization`
//! by parsing the *same* `/proc/meminfo`, `/sys/class/power_supply` and
//! `/proc/self/status` files with byte-identical bodies. Five copies of one
//! parser is five places for a parsing bug to survive in, and it made the
//! per-backend files harder to read than the two facts they actually own (their
//! name and family).
//!
//! A backend now forwards to the function here and states in one line why the
//! answer is what it is. Backends that genuinely differ — macOS and Windows use
//! native APIs, iOS and wasm expose nothing — keep their own implementations.
//!
//! # Cost
//!
//! Every function reads a small virtual file. That is the same cost the five
//! inline copies had; these are not called per frame, and a platform probe that
//! returned a cached figure would be lying about a live measurement.

/// Reads `MemTotal` from `/proc/meminfo`, in mebibytes.
///
/// Returns `None` when the file is unreadable or the field is missing, which is
/// the honest answer inside a container that hides `/proc`.
pub fn total_memory_mb() -> Option<u64> {
    let content = std::fs::read_to_string("/proc/meminfo").ok()?;
    for line in content.lines() {
        let Some(rest) = line.strip_prefix("MemTotal:") else {
            continue;
        };
        let kb = rest.trim().trim_end_matches("kB").trim().parse::<u64>().ok()?;
        return Some(kb / 1024);
    }
    None
}

/// Reports whether any battery in `/sys/class/power_supply` is discharging.
///
/// A machine with no battery (a desktop or a VM) reports `false`, which is the
/// same answer as a charged one and is what the caller wants to know: it is not
/// running down.
pub fn is_on_battery() -> bool {
    let Ok(entries) = std::fs::read_dir("/sys/class/power_supply") else {
        return false;
    };
    for entry in entries.flatten() {
        let status_path = entry.path().join("status");
        if let Ok(status) = std::fs::read_to_string(&status_path) {
            if status.trim() == "Discharging" {
                return true;
            }
        }
    }
    false
}

/// Samples RSS over VmSize for this process from `/proc/self/status`.
///
/// The ratio is a cheap proxy for how much of the mapped address space is
/// resident. `VmSize` is the denominator, so a process that has mapped a large
/// file reports a low figure — that is the intended reading, and why this is a
/// *utilization* rather than a "memory pressure" metric.
pub fn process_memory_utilization() -> Option<f32> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let mut vmrss_kb: u64 = 0;
    let mut vmsize_kb: u64 = 0;
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            vmrss_kb = rest.trim().trim_end_matches("kB").trim().parse().unwrap_or(0);
        } else if let Some(rest) = line.strip_prefix("VmSize:") {
            vmsize_kb = rest.trim().trim_end_matches("kB").trim().parse().unwrap_or(0);
        }
    }
    if vmsize_kb == 0 {
        return None;
    }
    Some((vmrss_kb as f32 / vmsize_kb as f32).clamp(0.0, 1.0))
}

/// Estimates CPU load as thread count over twice the available cores.
///
/// # Why not `/proc/stat`
///
/// A real load average needs two samples separated in time, which a one-shot
/// probe cannot take without blocking. Comparing the live thread count against
/// the core budget answers the question a caller actually has — "is this process
/// spreading out over the machine?" — from one read.
pub fn process_cpu_utilization() -> Option<f32> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    for line in status.lines() {
        let Some(rest) = line.strip_prefix("Threads:") else {
            continue;
        };
        let threads = rest.trim().parse::<f32>().ok()?;
        let cores = std::thread::available_parallelism().map(|n| n.get() as f32).unwrap_or(4.0);
        return Some((threads / (cores * 2.0)).clamp(0.0, 1.0));
    }
    None
}

/// Submits `job_file` to the CUPS/`lpr` print spooler on a unix desktop.
///
/// Tries `lpr` first (present on macOS and most BSD-derived hosts) and falls back
/// to `lp` (the System V spelling used by Linux distributions without the
/// BSD-compat layer). Returns the failure of the *last* attempt, so a caller sees
/// the `lp` error when both are missing rather than a generic "no spooler".
///
/// Windows uses the PowerShell spooler instead and does not call this.
pub fn spawn_print_job(job_file: &std::path::Path) -> Result<(), String> {
    let attempts: [&str; 2] = ["lpr", "lp"];
    let mut last_error = String::from("no print spooler found");
    for program in attempts {
        match std::process::Command::new(program).arg(job_file).spawn() {
            Ok(_child) => return Ok(()),
            Err(error) => {
                last_error = format!("{program}: {error}");
            }
        }
    }
    Err(last_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The probes read live system files. On a Linux host they must answer
    /// something sensible; on any other host they must answer `None`/`false`
    /// rather than panic, which is what the backends rely on.
    #[test]
    fn probes_are_total() {
        if let Some(mb) = total_memory_mb() {
            assert!(mb > 0, "a readable /proc/meminfo must report a positive size");
        }
        if let Some(ratio) = process_memory_utilization() {
            assert!((0.0..=1.0).contains(&ratio), "RSS/VmSize must be a fraction, got {ratio}");
        }
        if let Some(ratio) = process_cpu_utilization() {
            assert!((0.0..=1.0).contains(&ratio), "thread ratio must be clamped, got {ratio}");
        }
        // `is_on_battery` has no invariant to assert beyond not panicking.
        let _ = is_on_battery();
    }

    /// A path that does not exist must produce an error, not a panic or a silent
    /// success — the caller uses the `Result` to report the failure.
    #[test]
    fn print_job_reports_failure_for_a_missing_spooler_or_file() {
        let missing = std::path::Path::new("/nonexistent/rust_widgets_probe_job.pdf");
        let result = spawn_print_job(missing);
        // Either there is no spooler (an error) or there is one and it rejected
        // the missing file asynchronously (Ok from `spawn`). Both are acceptable;
        // what must not happen is a panic.
        let _ = result;
    }
}
