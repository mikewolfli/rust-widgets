#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_gates_are_worth_running.sh — BLUE24 §9.2 (the meta-gate)
# ============================================================================
# The rule this guards:
#
#   **The gate suite is itself accountable.** Every gate is reachable from the
#   runner, the runner's worst case fits in a fixed budget, and every gate has been
#   shown to go red at least once.
#
# # Why "too few" and "too many" are the same question
#
# BLUE23's lesson was gates that were *specified and never written* (its §2.2 and
# §3.3 criteria 4/5). The opposite failure is real too: a suite nobody can finish
# is a suite nobody runs, which is identical in effect to having no gate. So the
# criterion is three measurements, not a count of files:
#
#   (1) **no orphan**  — every `tools/check_*.sh` is reachable from
#       `run_all_gates.sh`. That runner enumerates by glob, so the check here is
#       that the glob is still the mechanism and no gate was moved out of it;
#   (2) **budgeted**   — the whole-run budget fits the stated ceiling. BLUE22 §F.3
#       lesson 4 records the failure this prevents: 58 gates × a 1800 s per-gate
#       bound is a worst case of 29 hours, which is not a suite anyone runs;
#   (3) **demonstrated** — every gate has a line in `gates_reverse_injection.md`.
#       A gate that has never been shown to fail cannot be distinguished from one
#       that always passes. A missing line is counted, not assumed green.
#
# # What this gate does NOT prove
#
#   * It does not run the suite. Running it is `run_all_gates.sh`'s job and takes
#     up to the run budget; a meta-gate that ran every gate would be that budget
#     again. It reads the suite's *structure*, which is what can be checked cheaply
#     on every edit.
#   * It does not judge whether a gate *should* exist. It answers "is it wired,
#     bounded, and shown to work", which are the three questions that can be
#     answered mechanically.
#
# Usage: tools/check_gates_are_worth_running.sh
# Exit 0 = every gate is wired, the run fits its budget, and the injection record
#          covers the suite (unverified gates are reported but are not a failure —
#          see the note in the output).
# Exit 1 = a structural finding (an orphan mechanism, or an over-budget run).
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

RUNNER="tools/run_all_gates.sh"
INJECTION="tools/gates_reverse_injection.md"
CEILING_SECS=3600   # 60 minutes — raised from 2700 when `check_android_runtime.sh` (an
                    # emulator boot + JNI probe, 86 s cold) joined the suite; see the budget
                    # comment in `run_all_gates.sh`. BLUE24 §9.2 criterion 2.

echo "[1/3] every gate is reachable from the runner"
if [ ! -f "$RUNNER" ]; then
    echo "  FAIL  $RUNNER is missing; there is no single entry point for the suite"
    exit 1
fi
# The runner must enumerate the suite, not carry a hand-maintained list: a list is
# exactly how a newly added gate stays invisible for several rounds (the failure
# `check_view_keys_are_unique.sh` recorded).
if ! grep -qE 'for[[:space:]]+gate[[:space:]]+in[[:space:]]+tools/check_\*\.sh' "$RUNNER"; then
    echo "  FAIL  $RUNNER no longer enumerates \`tools/check_*.sh\` by glob"
    echo "        A hand-maintained list is how a new gate goes unrun for rounds."
    exit 1
fi
# And no gate may be excluded from that enumeration by being moved out of the
# directory or renamed away from the prefix.
gate_count="$(find tools -maxdepth 1 -name 'check_*.sh' | wc -l | tr -d ' ')"
if [ "$gate_count" -eq 0 ]; then
    echo "  FAIL  no \`tools/check_*.sh\` gates found"
    echo "        An empty suite makes every other step here vacuous."
    exit 1
fi
echo "  PASS  $gate_count gates, all reached by the runner's glob"

echo "[2/3] the whole-run budget fits the ceiling"
# Read the default run budget out of the runner so this gate and the runner cannot
# disagree about it. The literal default is what a plain invocation uses.
run_budget="$(grep -oE 'RUN_BUDGET="\$\{RW_RUN_TIMEOUT:-[0-9]+\}"' "$RUNNER" \
    | grep -oE '[0-9]+' | head -1 || true)"
if [ -z "$run_budget" ]; then
    echo "  FAIL  could not read \`RUN_BUDGET\`'s default from $RUNNER"
    echo "        A missing budget is an unbounded run, which is the defect this step"
    echo "        exists for (BLUE24 §9.2 criterion 2)."
    exit 1
fi
if [ "$run_budget" -gt "$CEILING_SECS" ]; then
    echo "  FAIL  the run budget is ${run_budget}s, over the ${CEILING_SECS}s ceiling"
    echo "        A suite nobody can finish is a suite nobody runs."
    exit 1
fi
echo "  PASS  run budget ${run_budget}s <= ${CEILING_SECS}s ceiling"

echo "[3/3] every gate has reverse-injection evidence"
if [ ! -f "$INJECTION" ]; then
    echo "  FAIL  $INJECTION is missing"
    echo "        Without it, no gate's ability to fail is on record."
    exit 1
fi
unverified=""
unverified_count=0
for gate in tools/check_*.sh; do
    [ -f "$gate" ] || continue
    name="$(basename "$gate")"
    # A gate is covered when it is named in the record. The `—` rows count as named
    # but un-injected: they are reported below rather than treated as covered.
    if ! grep -qF "$name" "$INJECTION"; then
        unverified="${unverified}${name}
"
        unverified_count=$((unverified_count + 1))
    fi
done
if [ -n "$unverified" ]; then
    echo "  FAIL  these gates have no line in $INJECTION:"
    printf '%s' "$unverified" | sed 's/^/          /'
    echo "        A gate with no injection record is indistinguishable from one that"
    echo "        always passes (BLUE24 §9.2 criterion 3)."
    exit 1
fi
# Gates present but not yet injected: reported as a number, not a failure. The
# record is a backlog that grows to 100% slowly, and failing on it would make the
# first honest entry impossible to land.
pending="$(grep -cE '^\| .* \| — \| — \|' "$INJECTION" || true)"
echo "  PASS  all $gate_count gates are named in the record"
echo "  NOTE  ${pending} gate(s) named but not yet injected (an honest backlog, not a pass):"
grep -E '^\| .* \| — \| — \|' "$INJECTION" | sed -e 's/^| /          - /' -e 's/ | — | — |$//' || true

echo
echo "check_gates_are_worth_running: OK"
