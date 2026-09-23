#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_timeout.sh"
. "$ROOT_DIR/tools/lib_cargo_cache.sh"

# A snapshot test compiles the default feature set and renders SVG; the bound
# covers a cold compile. Without it a wedged test binary leaves this gate silent.
SNAPSHOT_TIMEOUT=900

REPORT_DIR="target/qa"
REPORT_FILE="$REPORT_DIR/visual_regression_report.md"
mkdir -p "$REPORT_DIR"

{
  echo "# rust_widgets visual regression report"
  echo
  echo "Generated at: $(date -u +"%Y-%m-%dT%H:%M:%SZ")"
  echo
  echo "## Snapshot tests"
} > "$REPORT_FILE"

# Runs one snapshot test and records the result.
#
# # Why this asserts that a test actually ran
#
# The filters used to be `chart::tests::svg_snapshot_*`, a path that has not
# existed since the chart controls moved under `src/widget/chart_widgets/`. A
# `cargo test <filter>` that matches nothing **exits 0**, so this gate printed
# "✅ snapshot stable" for both charts, wrote a report claiming coverage, and
# passed while running zero tests — the exact false-green failure mode that was
# previously found and fixed in `check_behavior_matrix.sh`. A gate that cannot
# fail is worse than no gate, because it is read as evidence.
#
# The name is therefore taken from the test binary's own output rather than
# trusted, and a missing match is a failure.
run_snapshot() {
  local label="$1"
  local filter="$2"
  local out

  echo "[*] $label ($filter)"
  if ! out="$(rw_cargo_cached "$SNAPSHOT_TIMEOUT" test "$filter" 2>&1)"; then
    printf '%s\n' "$out" | grep -E "^test |test result|error|panicked" | tail -20 >&2
    echo "❌ $label FAILED (see above)" >&2
    return 1
  fi

  if ! printf '%s\n' "$out" | grep -qE "^test ${filter} \.\.\. ok"; then
    echo "  ❌ the filter '$filter' matched no test, so this gate would be vacuous:" >&2
    printf '%s\n' "$out" | grep -E "test result|^error" | tail -5 >&2
    echo "  Fix the filter to the test's real module path (cargo test exits 0 on no match)." >&2
    return 1
  fi

  printf -- "- ✅ %s (%s)\n" "$label" "$filter" >> "$REPORT_FILE"
  return 0
}

fail=0

run_snapshot "line chart SVG snapshot stable" \
  widget::chart_widgets::tests::svg_snapshot_line_chart_stable || fail=1

run_snapshot "bar chart SVG snapshot stable" \
  widget::chart_widgets::tests::svg_snapshot_bar_chart_stable || fail=1

echo >> "$REPORT_FILE"
if [ "$fail" -ne 0 ]; then
  echo "Visual regression checks FAILED." >> "$REPORT_FILE"
  echo "Visual regression checks FAILED."
  exit 1
fi

echo "All visual regression checks passed." >> "$REPORT_FILE"
echo "Visual regression report written to $REPORT_FILE"
