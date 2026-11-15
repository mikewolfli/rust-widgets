#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# run_local_gates.sh — run the `check_*.sh` gates that no workflow or gate invokes
# ============================================================================
# `tools/README.md` says a gate that only ever reports PASS is worse than no gate.
# The same is true of a gate that no CI job ever runs: it can fail for months and
# the PR is still green, because nothing asks it the question (N2-02).
#
# `tools/run_all_gates.sh` is the local, glob-driven answer. It is deliberately not
# in CI: a full sweep takes tens of minutes and any one wedged gate used to take the
# whole run with it. This script closes the narrower hole instead, without paying for
# gates CI already runs:
#
#   candidate  := tools/check_*.sh, excluding this file and the runner
#   covered    := the file name is **invoked** by any `.github/workflows/*.yml`
#              OR invoked by another `tools/*.sh`
#   action     := run every candidate that is not covered
#
# "Invoked" means a call shape (`bash|sh|source|.` followed by the path), with
# comments stripped first. The earlier text-only `grep -qF` counted a **comment**
# that mentioned a gate as coverage, so 14 gates were neither run by CI nor by this
# script — the exact hole this file exists to close (W8-11).
#
# `--list` prints the uncovered set and exits.
#
# Exit codes
#   0  every gate ran and passed (or reported host-limited, exit 2)
#   1  at least one gate failed
#   2  the sweep itself could not start (no gates found)
#
# Usage:
#   tools/run_local_gates.sh            # run the uncovered set
#   tools/run_local_gates.sh --list     # print the set and exit
#
# Budgets: RW_GATE_TIMEOUT (per gate, default 900 s) and RW_RUN_TIMEOUT
# (whole sweep, default 2400 s). When the aggregate budget is exhausted the
# remaining gates are listed as NOT RUN and the sweep exits 1.
# ============================================================================
set -u
set -o pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR" || exit 2
. "$ROOT_DIR/tools/lib_timeout.sh"

MODE="${1:-run}"

# True when `file` **invokes** `name`, not merely mentions it. Comments are removed
# first, so a doc line that names a gate cannot count as coverage (W8-11).
invokes() {
  local name="$1" file="$2"
  local escaped
  escaped="$(printf '%s' "$name" | sed 's/[.[\*^$()+?{}|]/\\&/g')"
  # A workflow names the script on a run line; any non-comment line is a step body
  # there, so an exact name match is the right shape.
  case "$file" in
    *.yml|*.yaml)
      sed 's/#.*//' "$file" 2>/dev/null | grep -qE "(^|[^A-Za-z0-9_.-])${escaped}([^A-Za-z0-9_.-]|$)"
      ;;
    *)
      sed 's/#.*//' "$file" 2>/dev/null \
        | grep -qE "(^|[;&|[:space:]])(bash|sh|source|\\.)[[:space:]]+[^[:space:]]*${escaped}([^A-Za-z0-9_.-]|$)"
      ;;
  esac
}

gates=()
for path in tools/check_*.sh; do
  [ -f "$path" ] || continue
  name="$(basename "$path")"
  # Workflow coverage: a workflow step invokes the script.
  covered=0
  for workflow in .github/workflows/*.yml .github/workflows/*.yaml; do
    [ -f "$workflow" ] || continue
    if invokes "$name" "$workflow"; then
      covered=1
      break
    fi
  done
  [ "$covered" -eq 1 ] && continue
  # Gate coverage: another tool script invokes it.
  while IFS= read -r caller; do
    [ "$caller" = "$path" ] && continue
    [ "$(basename "$caller")" = "run_all_gates.sh" ] && continue
    [ "$(basename "$caller")" = "run_local_gates.sh" ] && continue
    if invokes "$name" "$caller"; then
      covered=1
      break
    fi
  done < <(find tools -maxdepth 1 -name '*.sh' -type f | sort)
  [ "$covered" -eq 1 ] && continue
  gates+=("$path")
done

if [ "${#gates[@]}" -eq 0 ]; then
  echo "run_local_gates: found no uncovered gate; the coverage test is stale" >&2
  exit 2
fi

if [ "$MODE" = "--list" ]; then
  printf '%s\n' "${gates[@]}"
  exit 0
fi

pass=0
fail=0
skip=0
failed_names=()
not_run=()
# Aggregate budget (T3-15): per-gate `RW_GATE_TIMEOUT` bounds one hang, but 83 gates × 900 s can
# outlive the CI job's 45-minute limit. Once this many seconds have elapsed, remaining gates are
# recorded as not run and the sweep fails with the explicit left-over list, rather than being
# killed by the job timeout with no diagnostics.
RUN_BUDGET="${RW_RUN_TIMEOUT:-2400}"
run_started=$SECONDS
for gate in "${gates[@]}"; do
  name="$(basename "$gate")"
  if [ $((SECONDS - run_started)) -ge "$RUN_BUDGET" ]; then
    not_run+=("$name")
    continue
  fi
  printf '▶ %s\n' "$name"
  set +e
  gate_log="$(mktemp)"
  rw_run_bounded "${RW_GATE_TIMEOUT:-900}" bash "$gate" 2>&1 | tee "$gate_log"
  status=$?
  # W8-16: `exit 2` alone is not "host-limited"; the gate must say so. This is the
  # same marker predicate `run_all_gates.sh` uses, so the two runners cannot classify
  # the same event differently.
  case "$status" in
    0) pass=$((pass + 1)); printf '  PASS %s\n' "$name" ;;
    2)
      if grep -q 'unsupported host' "$gate_log"; then
        skip=$((skip + 1)); printf '  SKIP(host-limited) %s\n' "$name"
      else
        fail=$((fail + 1)); failed_names+=("$name")
        printf '  FAIL %s (exit 2 without an `unsupported host` marker)\n' "$name"
      fi
      ;;
    *)
      fail=$((fail + 1))
      failed_names+=("$name")
      printf '  FAIL %s (exit %s)\n' "$name" "$status"
      ;;
  esac
  rm -f "$gate_log"
done

echo
echo "run_local_gates: ${#gates[@]} uncovered gate(s); PASS=$pass FAIL=$fail SKIP=$skip"
if [ "${#not_run[@]}" -ne 0 ]; then
  fail=$((fail + 1))
  echo "  NOT RUN (aggregate budget ${RUN_BUDGET}s exhausted): ${not_run[*]}"
fi
if [ "$fail" -ne 0 ]; then
  printf '  failed: %s\n' "${failed_names[*]}"
  exit 1
fi
exit 0
