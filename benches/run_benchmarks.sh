#!/bin/bash
# Run all benchmarks (BLUE11 R3.7, R9.4)
# Usage: bash benches/run_benchmarks.sh
#
# This script runs all criterion benchmarks with --quick for CI usage.
# For full precision benchmarks, omit the --quick flag.
#
# Expected output format:
#   === Running <Name> Benchmark ===
#   <criterion output with time/throughput measurements>
#   ---
#
# Exit code: 0 if all benchmarks pass, 1 on failure.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
SUMMARY_FILE="${PROJECT_DIR}/target/benchmark_summary.txt"
mkdir -p "${PROJECT_DIR}/target"

cd "$PROJECT_DIR"

run_bench() {
    local name="$1"
    local bench_name="$2"
    echo ""
    echo "╔═══════════════════════════════════════════════════════╗"
    printf "║  %-53s ║\n" "Running $name Benchmark..."
    echo "╚═══════════════════════════════════════════════════════╝"
    echo ""
    if cargo bench --bench "$bench_name" -- --quick 2>&1; then
        echo "  ✓ $name benchmark completed successfully"
        return 0
    else
        echo "  ✗ $name benchmark FAILED (exit code $?)" >&2
        return 1
    fi
}

FAILURES=0

run_bench "Render" "render_bench" || FAILURES=$((FAILURES + 1))
echo "---"
run_bench "Signal" "signal_bench" || FAILURES=$((FAILURES + 1))
echo "---"
run_bench "Layout" "layout_bench" || FAILURES=$((FAILURES + 1))
echo "---"
run_bench "JSON" "json_bench" || FAILURES=$((FAILURES + 1))
echo "---"
run_bench "Event" "event_bench" || FAILURES=$((FAILURES + 1))

echo ""
echo "═══════════════════════════════════════════════════════════"
echo "  Benchmarks Complete: $((5 - FAILURES))/5 passed"
echo "═══════════════════════════════════════════════════════════"

if [ "$FAILURES" -gt 0 ]; then
    echo "  ❌ $FAILURES benchmark(s) FAILED" >&2
    exit 1
fi

# Save summary for CI artifact comparison
{
    echo "Benchmark Run: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "Passed: $((5 - FAILURES))"
    echo "Failed: ${FAILURES}"
} > "$SUMMARY_FILE"
echo "  → Summary saved to $SUMMARY_FILE"
