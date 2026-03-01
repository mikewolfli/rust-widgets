#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

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

echo "[1/2] line chart SVG snapshot"
cargo test chart::tests::svg_snapshot_line_chart_stable
printf -- "- ✅ line chart SVG snapshot stable\n" >> "$REPORT_FILE"

echo "[2/2] bar chart SVG snapshot"
cargo test chart::tests::svg_snapshot_bar_chart_stable
printf -- "- ✅ bar chart SVG snapshot stable\n" >> "$REPORT_FILE"

echo >> "$REPORT_FILE"
echo "All visual regression checks passed." >> "$REPORT_FILE"

echo "Visual regression report written to $REPORT_FILE"
