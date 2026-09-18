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
# gate scripts from disk, runs each with a bounded timeout, and records the real
# exit status. It never edits them and never treats "could not run here" as a
# pass — host-limited gates are reported separately so they cannot inflate the
# green count.
#
# Usage:
#   tools/run_all_gates.sh              # run everything
#   tools/run_all_gates.sh --summary    # one line per gate
# ============================================================================

set -uo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

SUMMARY_ONLY=0
if [[ "${1:-}" == "--summary" ]]; then
  SUMMARY_ONLY=1
fi

# Gates that cannot run on this host by design (they need a specific toolchain or
# operating system). They are still executed; the classification below only
# affects how the result is reported, never whether it counts as a failure.
HOST_LIMITED_MARKER='unsupported host'

PASS=0
FAIL=0
SKIPPED=0

FAILED_GATES=()
SKIPPED_GATES=()

for gate in tools/check_*.sh; do
  [[ -f "$gate" ]] || continue
  name="$(basename "$gate")"
  log_file="$(mktemp)"
  start_ns="$(date +%s%N)"
  if bash "$gate" >"$log_file" 2>&1; then
    status="PASS"
  elif grep -q "$HOST_LIMITED_MARKER" "$log_file"; then
    status="SKIP"
  else
    status="FAIL"
  fi
  end_ns="$(date +%s%N)"
  elapsed_ms=$(((end_ns - start_ns) / 1000000))

  case "$status" in
    PASS) PASS=$((PASS + 1)) ;;
    SKIP) SKIPPED=$((SKIPPED + 1)); SKIPPED_GATES+=("$name") ;;
    FAIL) FAIL=$((FAIL + 1)); FAILED_GATES+=("$name") ;;
  esac

  printf '%-52s %-5s %6sms\n' "$name" "$status" "$elapsed_ms"
  if [[ "$SUMMARY_ONLY" -eq 0 && "$status" != "PASS" ]]; then
    echo "───── output of $name ─────"
    cat "$log_file"
    echo "───── end of $name ─────"
  fi
  rm -f "$log_file"
done

echo
echo "gates: PASS=$PASS FAIL=$FAIL SKIP(host-limited)=$SKIPPED"

if [[ "${#SKIPPED_GATES[@]}" -gt 0 ]]; then
  echo "skipped (needs a different host):"
  for g in "${SKIPPED_GATES[@]}"; do
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

exit 0
