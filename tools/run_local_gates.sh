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
#   covered    := the file name appears in any `.github/workflows/*.yml`
#              OR appears in another `tools/*.sh` (that script invokes it)
#   action     := run every candidate that is not covered
#
# Both tests are textual, which is the right shape here: the claim is "some workflow
# or gate names this script", and a name is exactly what a YAML/`bash` call contains.
#
# Exit codes
#   0  every gate ran and passed (or reported host-limited, exit 2)
#   1  at least one gate failed
#   2  the sweep itself could not start (no gates found)
#
# Usage:
#   tools/run_local_gates.sh            # run the uncovered set
#   tools/run_local_gates.sh --list     # print the set and exit
# ============================================================================
set -u
set -o pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR" || exit 2
. "$ROOT_DIR/tools/lib_timeout.sh"

MODE="${1:-run}"

gates=()
for path in tools/check_*.sh; do
  [ -f "$path" ] || continue
  name="$(basename "$path")"
  # Workflow coverage: the name appears in a workflow file.
  if grep -qF "$name" .github/workflows/*.yml 2>/dev/null; then
    continue
  fi
  # Gate coverage: another tool script invokes it.
  covered=0
  while IFS= read -r caller; do
    [ "$caller" = "$path" ] && continue
    [ "$(basename "$caller")" = "run_all_gates.sh" ] && continue
    [ "$(basename "$caller")" = "run_local_gates.sh" ] && continue
    if grep -qF "$name" "$caller" 2>/dev/null; then
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
for gate in "${gates[@]}"; do
  name="$(basename "$gate")"
  printf '▶ %s\n' "$name"
  set +e
  rw_run_bounded "${RW_GATE_TIMEOUT:-900}" bash "$gate"
  status=$?
  case "$status" in
    0) pass=$((pass + 1)); printf '  PASS %s\n' "$name" ;;
    2) skip=$((skip + 1)); printf '  SKIP(host-limited) %s\n' "$name" ;;
    *)
      fail=$((fail + 1))
      failed_names+=("$name")
      printf '  FAIL %s (exit %s)\n' "$name" "$status"
      ;;
  esac
done

echo
echo "run_local_gates: ${#gates[@]} uncovered gate(s); PASS=$pass FAIL=$fail SKIP=$skip"
if [ "$fail" -ne 0 ]; then
  printf '  failed: %s\n' "${failed_names[*]}"
  exit 1
fi
exit 0
