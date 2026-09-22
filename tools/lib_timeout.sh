#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# Portable wall-clock bound for the QA gates.
#
# # Why a shared helper
#
# Every gate shells out to `cargo`, `mdbook`, `python3` or a cross toolchain, and
# any of those can hang: a `cargo` waiting on a lock held by another process, a
# `cargo doc` that never returns, a benchmark that spins, a Python gate that
# blocks on stdin. Without a bound, one wedged gate takes the *whole* gate run
# with it and reports nothing — the worst possible failure mode, because a
# timeout that never fires looks identical to a passing gate from the outside.
#
# `tools/run_all_gates.sh` bounds each gate it launches. That protects the
# aggregate run but not a gate invoked directly (`bash tools/check_docs.sh`),
# which is how a developer actually runs one. This file puts the same bound
# inside each gate, so the protection travels with the gate.
#
# # Why not GNU `timeout`
#
# `timeout` is coreutils, and coreutils is *not* part of a default BSD/macOS
# userland: on this project's development host both `timeout` and `gtimeout` are
# absent. A helper that used `timeout 600 …` therefore degraded to running the
# command **unbounded** while looking like it had applied a bound — the exact
# silent-no-op failure this file exists to prevent. `check_perf.sh` carried that
# hazard in its own `TIMEOUT_CMD` fallback.
#
# So the bound is implemented here in pure bash and needs no external command:
# the child runs in the background, and a watchdog subshell kills it (whole
# process group first, then the pid) when the budget expires. GNU `timeout` is
# used only if it happens to be present, since it is marginally better at
# reaping grandchildren.
#
# Usage — source it, then use `rw_run_bounded`:
#
#   . "$ROOT_DIR/tools/lib_timeout.sh"
#   rw_run_bounded 600 cargo check --no-default-features --features embedded
#
# Exit status is the command's own, or 124 when the bound fired (the same
# status GNU `timeout` uses, so callers can treat both paths alike).

# --------------------------------------------------------------------------
# Default budget, in seconds.
#
# Sized from measured gate runtimes: the slowest whole gate is ~2 minutes on a
# warm cache (check_cookbook, which builds three mdbook books), while a single
# `cargo check` step inside a gate can take a few minutes cold. 900s leaves that
# headroom while still capping a hang well inside a CI job's own timeout.
#
# Gates whose cost is genuinely unbounded-in-principle (a criterion benchmark, a
# cross-target link, a full doc build across profiles) pass a larger explicit
# budget rather than relying on this default.
# --------------------------------------------------------------------------
RW_TIMEOUT_DEFAULT=900

# True when this host has GNU `timeout` (coreutils) available.
rw_have_gnu_timeout() {
  command -v timeout >/dev/null 2>&1
}

# Kill a process and everything it spawned.
#
# # Why killing the pid alone is not enough
#
# A gate is a bash script that shells out to `cargo`, which forks `rustc`. A bare
# `kill PID` reaps the script while `cargo`/`rustc` keep running and keep holding
# the **target-directory lock**. The next gate then blocks on that lock for the
# remainder of *its* budget — a hang caused by the timeout rather than prevented
# by it. Since the whole point of the bound is that one wedged gate must not take
# the run with it, the grandchildren have to go too.
#
# Three mechanisms, strongest first:
#
#   1. GNU `timeout` (when present) already handles this, and `rw_run_bounded`
#      prefers it. Nothing here runs in that case.
#   2. `pkill -P` walks the child's own process tree, which is what a macOS bash
#      3.2 host needs: it has no job-control process groups to signal, so
#      `kill -- -PGID` is unavailable and the descendants must be named
#      individually. `pkill` is part of the BSD base system.
#   3. The process group as a last resort, for a host that does group children.
rw_kill_tree() {
  local pid="$1"
  # (2) Descendants first, deepest-listed order from pkill: killing the leaves
  # before the root avoids the root re-parenting them to init, where they would
  # become unreachable from this pid and keep the lock forever.
  if command -v pkill >/dev/null 2>&1; then
    pkill -TERM -P "$pid" 2>/dev/null || true
  fi
  # (3) Then the process group, then the pid itself.
  kill -TERM "-$pid" 2>/dev/null || kill -TERM "$pid" 2>/dev/null || true

  local i=0
  while kill -0 "$pid" 2>/dev/null && [ "$i" -lt 20 ]; do
    sleep 0.25
    i=$((i + 1))
  done

  # Escalate on anything still alive after the grace period, descendants included.
  if command -v pkill >/dev/null 2>&1; then
    pkill -KILL -P "$pid" 2>/dev/null || true
  fi
  kill -KILL "-$pid" 2>/dev/null || kill -KILL "$pid" 2>/dev/null || true

  # A `cargo` that survived even that would wedge the next gate on the target-dir
  # lock. Report it loudly rather than letting the failure reappear as an
  # unexplained slow gate: this is the one case the bound could not handle.
  if command -v pgrep >/dev/null 2>&1 && pgrep -P "$pid" >/dev/null 2>&1; then
    echo "rw_kill_tree: pid $pid still has children after SIGKILL; the next gate may block on a build lock" >&2
  fi
}

# rw_run_bounded <seconds> <command> [args…]
#
# Runs the command under a wall-clock bound. Prints the timeout notice to stderr
# (not stdout) so a gate that captures its command's stdout is not polluted by
# it. Returns the command's status, or 124 if the bound fired.
rw_run_bounded() {
  local budget="${1:-$RW_TIMEOUT_DEFAULT}"
  shift || true
  if [ "$#" -eq 0 ]; then
    echo "rw_run_bounded: no command given" >&2
    return 2
  fi

  # Prefer GNU timeout when present: it handles the process-group kill itself
  # with fewer moving parts than the bash watchdog.
  if rw_have_gnu_timeout; then
    timeout --kill-after=10 "$budget" "$@"
    return $?
  fi

  # Pure-bash fallback. `bash` on a default BSD/macOS userland has neither GNU
  # `timeout` nor job-control process groups that survive a `set +m`, so this
  # cannot rely on `kill -- -PGID`. Instead the watchdog is a background subshell
  # that runs `sleep` and then kills the child if it outlives the bound.
  "$@" &
  local child=$!

  # The watchdog's `sleep` PID is written to a temp file, because a `$!` captured
  # *inside* the subshell is scoped to that subshell and is invisible here. The
  # earlier version tried `trap '' TERM` plus a process-group kill; on macOS bash
  # 3.2 that left the watchdog ignoring TERM with no group to signal, so `wait`
  # blocked until the full `sleep` budget elapsed — the exact hang this helper
  # exists to prevent. Killing the sleep by PID avoids both failure modes.
  local sleeper_file
  sleeper_file="$(mktemp)"
  (
    sleep "$budget" &
    printf '%s' "$!" > "$sleeper_file"
    wait 2>/dev/null
    if kill -0 "$child" 2>/dev/null; then
      echo "rw_run_bounded: timed out after ${budget}s, killing pid $child" >&2
      rw_kill_tree "$child"
    fi
  ) &
  local watchdog=$!

  # `wait` returns the child's status, or 128+signal when it was signalled.
  # 143 (SIGTERM) / 137 (SIGKILL) with the watchdog still armed can only come
  # from the bound firing, so those map to 124 — the status GNU `timeout`
  # reports, which lets callers treat both implementations identically.
  local status=0
  wait "$child" 2>/dev/null || status=$?

  # Reap the watchdog: kill its `sleep` (read from the temp file) and the
  # subshell. Killing the sleep first guarantees the subshell's `wait` returns
  # immediately rather than after the remaining budget, so no stray `sleep` keeps
  # the calling shell's stdout open after the command has returned.
  if [[ -f "$sleeper_file" ]]; then
    local sleeper_pid
    sleeper_pid="$(cat "$sleeper_file" 2>/dev/null || true)"
    [[ -n "$sleeper_pid" ]] && kill "$sleeper_pid" 2>/dev/null || true
  fi
  kill "$watchdog" 2>/dev/null || true
  wait "$watchdog" 2>/dev/null || true
  rm -f "$sleeper_file"

  case "$status" in
    143 | 137) status=124 ;;
  esac
  return "$status"
}
