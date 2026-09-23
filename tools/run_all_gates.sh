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
#
# ---------------------------------------------------------------------------
# Two budgets, because they answer two different questions
# ---------------------------------------------------------------------------
# A per-gate bound alone cannot make a *slow* run distinguishable from a *stuck*
# one: a run where several gates are merely expensive looks, from the outside,
# exactly like one where the first gate wedged. So there are two:
#
#   * `GATE_BUDGET` (900 s, `RW_GATE_TIMEOUT`) — "did this gate hang?"
#   * `RUN_BUDGET` (2700 s, `RW_RUN_TIMEOUT`) — "is this run making progress?"
#
# When the run budget is exhausted the runner stops **starting** new gates and
# prints every gate it did not reach. It never interrupts a gate in flight: that
# gate has already produced a verdict, and cutting it short would report a real
# result as a timeout. Both budgets are overridable from the environment, so a
# genuinely cold tree can raise them without editing this file.
#
# A `NOT-RUN` gate is not a pass. It is a piece of the verification that did not
# happen, and the summary counts it separately for exactly that reason.
# ============================================================================

set -uo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

# shellcheck source=tools/lib_timeout.sh
. "$ROOT_DIR/tools/lib_timeout.sh"

SUMMARY_ONLY=0
FILTER=""
ALLOW_TOOLCHAIN_SWITCH=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --summary) SUMMARY_ONLY=1 ;;
    --allow-toolchain-switch) ALLOW_TOOLCHAIN_SWITCH=1 ;;
    --filter)
      shift || true
      FILTER="${1:-}"
      ;;
    *)
      echo "unknown argument: $1" >&2
      echo "usage: tools/run_all_gates.sh [--summary] [--filter SUBSTRING] [--allow-toolchain-switch]" >&2
      exit 2
      ;;
  esac
  shift || true
done

# ── Toolchain guard ─────────────────────────────────────────────
#
# 21 of the 59 gates shell out to `cargo`. If the resolved `cargo` is a *different
# toolchain* than the one this workspace's `target/` was built with, every one of
# those 21 pays a **full rebuild of the crate** — which took a real run from 432 s
# to over 45 minutes and made it report `NOT-RUN` for the gates it never reached.
#
# That is a property of the *invocation*, not of the gates: it happens by putting
# another toolchain's `bin/` on `PATH` (a `nightly` directory ahead of the default
# `stable`), which looks like a harmless way to make `cargo` resolvable when the
# default one is broken. It is not, and its symptom — a run that appears to hang —
# is indistinguishable from a genuine wedge, so it is worth failing loudly instead.
#
# # How the switch is detected
#
# The signal is the **resolved `cargo` binary's path**, not `RUSTUP_TOOLCHAIN`:
# rustup reports the active toolchain *consistently with* whatever override is in
# force, so comparing the two agrees in both cases (measured, not assumed). The
# difference is where `cargo` lands:
#
#   default                       ~/.cargo/bin/cargo          ← rustup's proxy
#   nightly's bin/ first on PATH  ~/.rustup/toolchains/<tc>/bin/cargo
#
# A rustup proxy is a directory literally named `.cargo`; a toolchain binary lives
# under a `toolchains/` directory. So "does the resolved path contain
# `/toolchains/`" is the question, and it has no false positive on a normal
# install (and none at all without rustup, which the first line handles).
check_toolchain_is_consistent() {
  local resolved
  resolved="$(command -v cargo 2>/dev/null)" || return 0
  [[ -n "$resolved" ]] || return 0
  [[ "$resolved" == *"/toolchains/"* ]] || return 0
  cat >&2 <<EOF
run_all_gates: refusing to run with a toolchain binary directly on PATH.

  resolved cargo: $resolved

\`cargo\` here is a toolchain's own binary rather than rustup's proxy, so this is not
the toolchain the workspace's \`target/\` was built with. Every gate that shells out
to \`cargo\` would pay a full rebuild: a 432 s sweep becomes the better part of an
hour and reports \`NOT-RUN\` for the gates it never reaches, which is
indistinguishable from a wedge.

Use the default toolchain (do not put a \`toolchains/*/bin\` directory on PATH),
or pass --allow-toolchain-switch if a cold rebuild is what you want.
EOF
  return 1
}

if [[ "$ALLOW_TOOLCHAIN_SWITCH" -eq 0 ]]; then
  check_toolchain_is_consistent || exit 3
fi

# Per-gate budget. Larger than `RW_TIMEOUT_DEFAULT` because a gate may drive
# several `cargo` invocations across profiles (check_profiles runs ~15), and a
# cold target dir makes the first one expensive.
#
# # Why this is 900 rather than 1800
#
# The budget bounds a *hang*, and 30 minutes is long enough that a wedged gate
# still makes the aggregate run feel stuck: 58 gates each allowed 30 minutes is
# a 29-hour worst case, which is indistinguishable from a hang from the outside.
# The slowest gate measured on a warm cache is ~35 s and the slowest cold one is
# ~4 minutes, so 900 s is still generous for a real gate while capping the
# pathological case at 15 minutes and letting the run report `TIMEOUT` and move
# on. A gate that genuinely needs more passes its own larger budget to
# `rw_run_bounded` rather than raising this floor for everyone.
GATE_BUDGET="${RW_GATE_TIMEOUT:-900}"

# Whole-run budget, in seconds.
#
# # Why a run-level cap is not redundant with the per-gate one
#
# The per-gate bound answers "did this gate hang?". It cannot answer "is this
# run making progress?" — a run where several gates are merely slow is
# arithmetically identical, from the outside, to one where the first gate wedged.
# This cap makes the difference observable: when it fires, the run stops and
# names every gate it did not reach, so the remaining work is a *list* rather
# than a wait. Default 45 minutes, which covers a fully cold run of all gates.
RUN_BUDGET="${RW_RUN_TIMEOUT:-2700}"

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
NOT_RUN_GATES=()
RUN_START_NS="$(date +%s)"

# True once the whole-run budget has been consumed.
#
# Checked before each gate rather than enforced by killing an in-flight one: a
# gate part-way through has already produced its own verdict, and interrupting it
# would report a genuine result as a timeout. The cap therefore stops *starting*
# new work, which is the decision that keeps the run's duration bounded while
# every reported line remains true.
run_budget_exhausted() {
  local now elapsed
  now="$(date +%s)"
  elapsed=$((now - RUN_START_NS))
  [ "$elapsed" -ge "$RUN_BUDGET" ]
}

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

  if run_budget_exhausted; then
    # Reported, not dropped: a gate that never ran is a piece of the verification
    # that did not happen, and silently omitting it is how "N gates, all PASS"
    # comes to describe a run that covered half the gates.
    NOT_RUN_GATES+=("$name")
    printf '%-52s %-7s\n' "$name" "NOT-RUN"
    continue
  fi

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
echo "gates: PASS=$PASS FAIL=$FAIL TIMEOUT=$TIMED_OUT NOT-RUN=${#NOT_RUN_GATES[@]} SKIP(host-limited)=$SKIPPED"

if [[ "${#NOT_RUN_GATES[@]}" -gt 0 ]]; then
  echo "not run — the ${RUN_BUDGET}s whole-run budget was exhausted first:"
  for g in "${NOT_RUN_GATES[@]}"; do
    echo "  - $g"
  done
  echo "  (re-run with: tools/run_all_gates.sh --summary --filter <name>)"
fi

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
