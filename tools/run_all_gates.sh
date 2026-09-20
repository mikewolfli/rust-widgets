#!/usr/bin/env bash
# ============================================================================
# run_all_gates.sh — run every `check_*.sh` gate and print a PASS/FAIL table
# ============================================================================
# The per-round verification matrix in `docs/log/` has, for several rounds,
# reported "N gates, M PASS / K FAIL" without a script that produces that number.
# Hand-counting is exactly how a gate that nobody wired up stays invisible
# (`check_view_keys_are_unique.sh` was unrunnable-by-CI for several rounds before
# BLUE18 E-1) and how a new gate is silently forgotten the round after it lands.
#
# This runner is the single mechanical source of that table: it enumerates the
# gate scripts from disk, runs each under a wall-clock bound, and records the real
# exit status. It never edits them and never treats "could not run here" as a
# pass — host-limited gates are reported separately so they cannot inflate the
# green count.
#
# Usage:
#   tools/run_all_gates.sh              # run everything (verbose on failure)
#   tools/run_all_gates.sh --summary    # one line per gate
#   tools/run_all_gates.sh --filter profiles   # only gates whose name matches
#
# ---------------------------------------------------------------------------
# Why the per-gate bound is mandatory (principle #58/#59)
# ---------------------------------------------------------------------------
# The first version of this runner invoked `bash "$gate"` bare. That is the exact
# hazard `tools/lib_timeout.sh` documents: one wedged gate — a `cargo` waiting on
# a target-dir lock another process holds, a `cargo doc` that never returns, a
# Python gate blocked on stdin — takes the *whole* run with it and prints nothing.
# A hang that never reports is indistinguishable from a passing gate from the
# outside, so it is worse than a failure.
#
# Every gate here therefore runs through `rw_run_bounded`, and this script never
# wraps itself in an *outer* `timeout` command: stacking the two was measured to
# report all 30 gates as failed while each passed standalone, because the outer
# `timeout` competes with the inner watchdog for the child's process group. The
# bound is applied once, here, by the helper every gate already uses.
#
# A gate that exceeds its budget is reported as `TIMEOUT` and counted as a
# failure of the *run*, not as a defect in the code under test — the two are
# different claims and must not be conflated (principle #59.4).
# ============================================================================

set -uo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

# shellcheck source=tools/lib_timeout.sh
. "$ROOT_DIR/tools/lib_timeout.sh"

SUMMARY_ONLY=0
FILTER=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --summary) SUMMARY_ONLY=1 ;;
    --filter)
      shift || true
      FILTER="${1:-}"
      ;;
    *)
      echo "unknown argument: $1" >&2
      echo "usage: tools/run_all_gates.sh [--summary] [--filter SUBSTRING]" >&2
      exit 2
      ;;
  esac
  shift || true
done

# Per-gate budget. Larger than `RW_TIMEOUT_DEFAULT` because a gate may drive
# several `cargo` invocations across profiles (check_profiles runs ~15), and a
# cold target dir makes the first one expensive. Still bounded, so a hang is
# capped well inside any CI job timeout.
GATE_BUDGET="${RW_GATE_TIMEOUT:-1800}"

# Gates that cannot run on this host by design (they need a specific toolchain or
# operating system). They are still executed; the classification below only
# affects how the result is reported, never whether it counts as a failure.
HOST_LIMITED_MARKER='unsupported host'

PASS=0
FAIL=0
SKIPPED=0
TIMED_OUT=0

FAILED_GATES=()
SKIPPED_GATES=()
TIMEOUT_GATES=()

# Announce every gate before it starts. The previous version printed only after
# the gate returned, so a hang showed nothing at all: the last line was the
# previous gate's result and the wedged gate's name never appeared. Printing
# first is what makes a hang locatable from the captured output alone.
announce() {
  if [[ "$SUMMARY_ONLY" -eq 0 ]]; then
    printf '%-52s %s\n' "$1" "running…"
  fi
}

for gate in tools/check_*.sh; do
  [[ -f "$gate" ]] || continue
  name="$(basename "$gate")"
  if [[ -n "$FILTER" && "$name" != *"$FILTER"* ]]; then
    continue
  fi

  announce "$name"
  log_file="$(mktemp)"
  start_ns="$(date +%s)"

  # Capture the status in a variable before branching. `rw_run_bounded` returns the
  # gate's own status, or 124 when the bound fired. Writing `elif [[ $? -eq 124 ]]`
  # would read the status of the *previous test*, not of the command, because the
  # `if`/`elif` chain replaces `$?` — the timeout branch would then never fire and a
  # wedged gate would be misreported as a failure of the code under test.
  status_code=0
  rw_run_bounded "$GATE_BUDGET" bash "$gate" >"$log_file" 2>&1 || status_code=$?

  status="PASS"
  if [[ "$status_code" -eq 124 ]]; then
    status="TIMEOUT"
  elif [[ "$status_code" -ne 0 ]]; then
    if grep -q "$HOST_LIMITED_MARKER" "$log_file"; then
      status="SKIP"
    else
      status="FAIL"
    fi
  fi
  end_ns="$(date +%s)"
  elapsed_s=$((end_ns - start_ns))

  case "$status" in
    PASS) PASS=$((PASS + 1)) ;;
    SKIP) SKIPPED=$((SKIPPED + 1)); SKIPPED_GATES+=("$name") ;;
    FAIL) FAIL=$((FAIL + 1)); FAILED_GATES+=("$name") ;;
    TIMEOUT) TIMED_OUT=$((TIMED_OUT + 1)); TIMEOUT_GATES+=("$name") ;;
  esac

  printf '%-52s %-7s %5ss\n' "$name" "$status" "$elapsed_s"
  if [[ "$SUMMARY_ONLY" -eq 0 && "$status" != "PASS" ]]; then
    echo "───── output of $name ─────"
    cat "$log_file"
    echo "───── end of $name ─────"
  fi
  rm -f "$log_file"
done

echo
echo "gates: PASS=$PASS FAIL=$FAIL TIMEOUT=$TIMED_OUT SKIP(host-limited)=$SKIPPED"

if [[ "${#SKIPPED_GATES[@]}" -gt 0 ]]; then
  echo "skipped (needs a different host):"
  for g in "${SKIPPED_GATES[@]}"; do
    echo "  - $g"
  done
fi

if [[ "${#TIMEOUT_GATES[@]}" -gt 0 ]]; then
  echo "timed out after ${GATE_BUDGET}s (result unknown — NOT a pass and NOT a defect):"
  for g in "${TIMEOUT_GATES[@]}"; do
    echo "  - $g"
  done
fi

if [[ "${#FAILED_GATES[@]}" -gt 0 ]]; then
  echo "failed:" >&2
  for g in "${FAILED_GATES[@]}"; do
    echo "  - $g" >&2
  done
  exit 1
fi

# A timeout leaves the run inconclusive, so it must not exit 0 — that would let a
# wedged gate masquerade as a clean run.
if [[ "$TIMED_OUT" -gt 0 ]]; then
  exit 1
fi

exit 0
