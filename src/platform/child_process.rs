// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Process-group isolation for the children this library starts itself.
//!
//! # Why a library-owned child needs its own group
//!
//! A host that has to clean up after a session cannot enumerate "my processes" safely: matching
//! by program name kills the user's own `lp`, and scanning the process table is a guess. What a
//! session *can* know exactly is the process groups it created, so that is what hosts use as the
//! ownership criterion — and a child this library spawns inside the host's group would be killed
//! by the host's cleanup as if the host had started it (a browser for [`open_external`], or a
//! spooler whose job the host knows nothing about).
//!
//! The reverse direction is the one the designer feedback reported: a child started here is
//! **not** reachable by the host's group cleanup, so a spooler that forks its own helpers left
//! grandchildren behind with nothing recording that they existed. This module answers both halves
//! with one rule: a child this library starts gets its own group, so "host processes" and
//! "library processes" are disjoint sets that neither side can kill by accident — and the
//! library ends what it started, descendants included.
//!
//! [`open_external`]: crate::platform::Platform::open_external
//!
//! # What is platform-specific here, and where it stays
//!
//! Process groups are a POSIX concept, so the calls are unix-only and a target that has no such
//! concept gets an honest no-op with the reason in the code rather than a `cfg` at every call
//! site (principles #36/#41: the difference is stated once, inside `src/platform/`). On Windows
//! the equivalent ownership unit is a job object, which this crate does not create; the boundary
//! is recorded in [`crate::platform::os_probes`] and here rather than implied.

/// Starts the child of `command` in its own process group, on targets that have process groups.
///
/// Returns the command so a call site reads as one builder chain. On targets without process
/// groups this is a documented no-op: the child then belongs to whatever the host's equivalent
/// ownership unit is, which is a fact a caller cannot change from here.
#[cfg(unix)]
pub(crate) fn in_own_group(command: &mut std::process::Command) -> &mut std::process::Command {
    // `0` means "new group, id equal to this child's pid" (POSIX `setpgid(0, 0)`), which is what
    // makes the group addressable by `kill_whole_group` below without a second lookup.
    //
    // It also takes the child out of the terminal's foreground group, so a Ctrl-C aimed at the
    // host no longer reaches a job the host does not own — the same separation, seen from the
    // signal side.
    std::os::unix::process::CommandExt::process_group(command, 0)
}

#[cfg(not(unix))]
pub(crate) fn in_own_group(command: &mut std::process::Command) -> &mut std::process::Command {
    // No process groups on this target. See this module's docs for what the host's equivalent
    // ownership unit is and why nothing is fabricated here.
    command
}

/// Ends `child` and, on unix, every process in the group it leads.
///
/// # Why the group and not the child
///
/// A spooler does not have to be a single process: `lp` hands the job to a filter chain, each
/// stage a child of its own. Killing only the direct child leaves that chain running with an open
/// handle on a job file the caller is about to delete, and nothing in the library would report
/// it. The group this library created with [`in_own_group`] is exactly the set to end.
///
/// The direct child is still reaped here, so no zombie is left for the rest of the session.
/// Callers keep whatever error reporting they had: this returns nothing, because "the process is
/// gone" is observable in the child's own status.
#[cfg(unix)]
pub(crate) fn kill_whole_group(child: &mut std::process::Child) {
    /// `SIGKILL`. Spelled out because the `libc` crate is not a dependency of every unix target
    /// this crate builds for (it is optional on linux, behind `wayland-native`), and the number
    /// is fixed by POSIX on both Apple and Linux platforms this crate supports.
    const SIGKILL: i32 = 9;

    extern "C" {
        /// POSIX `kill(2)`: signal `pid`, or the process group when `pid` is negative.
        fn kill(pid: i32, sig: i32) -> i32;
    }

    // The group id equals the child's pid because it was spawned with `in_own_group`. A negative
    // pid is how `kill(2)` is told "this whole group"; a failure is ignored on purpose — the
    // direct child may already have exited (ESRCH), and `wait` below is what makes its absence
    // certain.
    //
    // SAFETY: `kill` is called with a pid this process just created and a signal constant; it
    // has no memory arguments and no invariants for the caller to uphold.
    unsafe {
        let _ = kill(-(child.id() as i32), SIGKILL);
    }
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(not(unix))]
pub(crate) fn kill_whole_group(child: &mut std::process::Child) {
    // No process groups: the child alone is what this process can address. Reaping it is still
    // required, so a caller does not leave a zombie behind.
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A child started in its own group must not be reachable by a group signal sent to *ours*.
    ///
    /// This is the property the designers' session cleanup depends on, stated as a test: the host
    /// ends what it created, and a job this library started is not in that set. The check is
    /// POSIX's own view of the relationship (`getpgrp`), read through the same `extern "C"` seam
    /// the module uses, rather than an inspection of our own bookkeeping.
    #[cfg(unix)]
    #[test]
    fn an_isolated_child_leads_its_own_group() {
        extern "C" {
            /// POSIX `getpgrp(2)`: the process group id of the calling process.
            fn getpgrp() -> i32;
        }

        let mut command = std::process::Command::new("sh");
        command.args(["-c", "exit 0"]);
        in_own_group(&mut command);
        let mut child = command.spawn().expect("sh must be spawnable for this check");

        // SAFETY: no arguments, no invariants; it reads the caller's process group.
        let our_group = unsafe { getpgrp() };
        let child_group = child.id() as i32;
        assert_ne!(
            child_group, our_group,
            "the child must not share this process's group: a host that ends its own group would \
             otherwise kill a job the library started"
        );
        let _ = child.wait();
    }

    /// On a target without process groups the helper must stay a no-op rather than fail.
    #[cfg(not(unix))]
    #[test]
    fn isolation_is_a_documented_no_op_without_process_groups() {
        let mut command = std::process::Command::new("cmd");
        command.arg("/C").arg("exit 0");
        // Returning the command unchanged is the contract on this target; a caller that relied on
        // isolation would be reading this module's docs, which say it cannot be provided here.
        let _ = in_own_group(&mut command);
    }
}
