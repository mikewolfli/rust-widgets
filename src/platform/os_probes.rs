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

use crate::compat::{fmt, format, String, ToString};

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
/// # Why this waits for the spooler
///
/// The job file is a temporary file owned by the caller, which deletes it as soon
/// as this returns. `spawn` alone therefore *reports* success while the spooler is
/// still opening the file — `lpr` then fails with "cannot access …: No such file
/// or directory" and the document is silently never printed. The child is reaped
/// here so that the caller only ever deletes a file the spooler has finished with,
/// and so that a spooler which rejects the job is reported instead of looking like
/// a successful submission.
///
/// The wait is unbounded on purpose: `lpr` talking to a slow CUPS server can take
/// seconds, and a timeout would reintroduce the exact race this exists to remove.
///
/// Windows uses the PowerShell spooler instead and does not call this.
pub fn spawn_print_job(job_file: &std::path::Path) -> Result<(), String> {
    let attempts: [&str; 2] = ["lpr", "lp"];
    let mut last_error = String::from("no print spooler found");
    let mut attempted = false;
    for program in attempts {
        match run_spooler(program, job_file) {
            Ok(()) => return Ok(()),
            Err(error) => {
                // Distinguish "this program is not installed" from "this program
                // rejected the job": only the former leaves `attempted` false.
                if error.is_rejection() {
                    attempted = true;
                }
                last_error = error.to_string();
            }
        }
    }
    if attempted {
        Err(last_error)
    } else {
        Err(format!(
            "no print spooler found: none of the known spooler commands could be spawned \
             ({last_error}); install CUPS (`cups-client`) or a compatible `lpr` to print"
        ))
    }
}

/// Why a single spooler attempt did not produce a printed job.
#[derive(Debug)]
enum SpoolerFailure {
    /// The program could not be started — it is not installed (or not executable).
    Unavailable(String),
    /// The program ran and refused the job; the message is its own diagnostics.
    Rejected(String),
}

impl SpoolerFailure {
    fn is_rejection(&self) -> bool {
        matches!(self, Self::Rejected(_))
    }
}

impl fmt::Display for SpoolerFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(message) | Self::Rejected(message) => f.write_str(message),
        }
    }
}

/// Runs one spooler program to completion and reports whether it accepted the job.
///
/// Split from [`spawn_print_job`] so the waiting behaviour can be tested against a
/// stand-in program without mutating the process-global `PATH`.
fn run_spooler(program: &str, job_file: &std::path::Path) -> Result<(), SpoolerFailure> {
    let output = std::process::Command::new(program).arg(job_file).output().map_err(|error| {
        SpoolerFailure::Unavailable(format!(
            "spooler command '{program}' could not be spawned: {error} (check that it is \
                 installed and on PATH)"
        ))
    })?;

    if output.status.success() {
        return Ok(());
    }

    // Surface the spooler's own diagnosis. An empty stderr still has to name the
    // program and its exit status, otherwise the caller learns nothing.
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr = stderr.trim();
    let message = if stderr.is_empty() {
        format!("{program}: exited with {}", output.status)
    } else {
        format!("{program}: failed: {stderr}")
    };
    Err(SpoolerFailure::Rejected(message))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The interpreter, script file name and script body of a stand-in spooler.
    ///
    /// A real spooler is invoked as `program <job file>` — one argument, no process
    /// globals touched (mutating `PATH` from a test would race every other test in
    /// the binary). Both interpreters below take the program to run as their single
    /// argument, so the fixture keeps that shape on every host: `sh spool.sh` on
    /// unix, `cscript spool.vbs` on Windows.
    ///
    /// The script pauses, then copies the job file it was given to `record`. A copy
    /// that fails (because the caller already deleted the job file) must leave
    /// `record` unwritten, which is what makes the wait observable.
    #[cfg(any(unix, windows))]
    fn stand_in_spooler(
        job: &std::path::Path,
        record: &std::path::Path,
    ) -> (&'static str, &'static str, String) {
        #[cfg(unix)]
        {
            let script = format!(
                "sleep 0.2\ncat \"{}\" > \"{}\" 2>/dev/null || exit 1\n",
                job.display(),
                record.display()
            );
            ("sh", "spool.sh", script)
        }
        #[cfg(windows)]
        {
            // VBScript via `cscript`: an unhandled runtime error (the missing job
            // file) aborts the script before the record is written — verified by
            // running it, not assumed. `wscript` is deliberately not used, as it
            // reports errors in a modal dialog instead of on stderr.
            let script = format!(
                "Dim fso, src, dst\n\
                 WScript.Sleep 200\n\
                 Set fso = CreateObject(\"Scripting.FileSystemObject\")\n\
                 Set src = fso.OpenTextFile(\"{}\", 1)\n\
                 Set dst = fso.CreateTextFile(\"{}\", True)\n\
                 dst.Write src.ReadAll\n\
                 dst.Close\n\
                 src.Close\n\
                 WScript.Quit 0\n",
                job.display(),
                record.display()
            );
            ("cscript", "spool.vbs", script)
        }
    }

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

    /// The spooler must have finished reading the job file before this returns.
    ///
    /// This is the guard for a silent data-loss bug: the caller deletes the job file
    /// as soon as this function returns, and the original implementation used
    /// `Command::spawn` without waiting. With a real spooler present that returns
    /// `Ok(())` while `lpr` is still opening the file — the spooler then prints
    /// nothing and the failure is only visible in its own stderr.
    ///
    /// A stand-in spooler is used so the race can be observed deterministically: it
    /// pauses, then copies the job file back out. The caller deletes the job file the
    /// moment this returns, so an empty record can only mean the copy happened after
    /// the deletion — i.e. that `run_spooler` did not wait.
    ///
    /// The stand-in runs on every host, not just unix: the interpreter is handed the
    /// program to run as its *only* argument, which is the shape `run_spooler` uses
    /// for a real spooler (`lpr <job>`). `sh`/`cscript` both satisfy it, so the check
    /// is not skipped on a Windows checkout — where the equivalent silent loss lives
    /// in a different submission mechanism and is therefore easy to leave unguarded.
    ///
    /// Gated on `any(unix, windows)` because it *is* the gate for
    /// [`stand_in_spooler`]: without it, a target that is neither (wasm, bare metal —
    /// where `run_spooler` and `stand_in_spooler` do not exist) failed to compile
    /// `--all-targets` with `cannot find function stand_in_spooler`. The property under
    /// test is a property of the spooler submission path, which only exists there, so
    /// skipping is honest rather than a lost assertion.
    #[cfg(any(unix, windows))]
    #[test]
    fn print_job_waits_for_the_spooler_before_reading_back() {
        use std::io::Write as _;

        let dir = std::env::temp_dir().join(format!("rw_spool_probe_{}", std::process::id()));
        if std::fs::create_dir_all(&dir).is_err() {
            return;
        }
        let record = dir.join("read_back.txt");
        let _ = std::fs::remove_file(&record);

        let job = dir.join("job.txt");
        if std::fs::write(&job, "page:1\n").is_err() {
            return;
        }

        // A stand-in spooler that only succeeds if the file is still there after a
        // pause long enough for a non-waiting caller to have deleted it.
        let (program, script_name, script) = stand_in_spooler(&job, &record);
        let fake = dir.join(script_name);
        if std::fs::File::create(&fake)
            .and_then(|mut file| file.write_all(script.as_bytes()))
            .is_err()
        {
            return;
        }

        let result = run_spooler(program, &fake);
        // The caller's next action, verbatim from `print_to_printer`.
        let removed = std::fs::remove_file(&job);

        let read_back = std::fs::read_to_string(&record).unwrap_or_default();
        let _ = std::fs::remove_file(&fake);

        assert!(result.is_ok(), "the stand-in spooler must report success, got {result:?}");
        assert!(
            removed.is_ok(),
            "the caller must be able to delete the job file straight after submission: \
             {removed:?}"
        );
        assert_eq!(
            read_back.trim(),
            "page:1",
            "the spooler read the job file back as {read_back:?} — an empty value means \
             `run_spooler` returned before the spooler had finished reading, so the \
             caller deleted the file underneath it and nothing was printed"
        );
    }

    /// A program that is not installed must be reported as missing, not as a rejected
    /// job — the two lead to different caller-visible messages.
    #[test]
    fn an_absent_spooler_is_reported_as_unavailable() {
        let job = std::path::Path::new("/tmp/rw_probe_absent_job.txt");
        let result = run_spooler("rw_definitely_not_a_spooler", job);
        match result {
            Err(failure) => assert!(
                !failure.is_rejection(),
                "an uninstalled program must not be reported as rejecting the job: {failure}"
            ),
            Ok(()) => panic!("a non-existent spooler must not report success"),
        }
    }

    /// A file that does not exist must be reported as a failure when a spooler is
    /// present, and as "no spooler" when none is — never as a success.
    ///
    /// This test previously accepted **any** outcome (`let _ = result;`) on the
    /// reasoning that a real spooler rejects the file asynchronously. That leniency
    /// is what let a genuine race through: the caller deletes the job file as soon
    /// as this function returns, and `spawn` returned before the spooler had read
    /// it, so every print job on a host with `lpr` installed failed silently. The
    /// only acceptable answer now is an error.
    #[test]
    fn print_job_reports_failure_for_a_missing_spooler_or_file() {
        let missing = std::path::Path::new("/nonexistent/rust_widgets_probe_job.txt");
        let result = spawn_print_job(missing);
        assert!(
            result.is_err(),
            "submitting a non-existent job file must not report success, got {result:?}"
        );
    }
}
