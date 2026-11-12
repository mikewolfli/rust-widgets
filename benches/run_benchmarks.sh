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
. "$PROJECT_DIR/tools/lib_timeout.sh"

# Benchmarks are the documented case in `tools/lib_timeout.sh` for an explicit budget
# above `RW_TIMEOUT_DEFAULT`: a criterion run's cost is not bounded in principle the way
# a single `cargo check` is. 1200 s stays inside `benchmark.yml`'s 30-minute job timeout,
# so a wedged bench reports its timeout here instead of the job being cancelled with no
# verdict (principles #58 and #104).
BENCH_BUDGET="${RW_BENCH_TIMEOUT:-1200}"

run_bench() {
    local name="$1"
    local bench_name="$2"
    TOTAL=$((TOTAL + 1))
    echo ""
    echo "╔═══════════════════════════════════════════════════════╗"
    printf "║  %-53s ║\n" "Running $name Benchmark..."
    echo "╚═══════════════════════════════════════════════════════╝"
    echo ""
    local output
    if output="$(rw_run_bounded "$BENCH_BUDGET" cargo bench --bench "$bench_name" -- --quick 2>&1)"; then
        printf '%s\n' "$output"
        # A bench compiled under the default libtest harness exits 0 after "running 0 tests".
        # Criterion always prints a `time:` line; requiring one makes that empty run a failure
        # instead of a green "completed" (T3-02).
        if ! grep -q "time:" <<<"$output"; then
            echo "  ✗ $name benchmark produced no criterion measurement (harness ran 0 benchmarks)" >&2
            return 1
        fi
        echo "  ✓ $name benchmark completed successfully"
        return 0
    else
        echo "  ✗ $name benchmark FAILED (exit code $?)" >&2
        return 1
    fi
}

# A benchmark that needs non-default features (the font benchmark needs a vector face to have
# anything to look up). Same shaping as `run_bench`, one extra argument.
run_bench_featured() {
    local name="$1"
    local bench_name="$2"
    local features="$3"
    TOTAL=$((TOTAL + 1))
    echo ""
    echo "╔═══════════════════════════════════════════════════════╗"
    printf "║  %-53s ║\n" "Running $name Benchmark..."
    echo "╚═══════════════════════════════════════════════════════╝"
    echo ""
    local output
    if output="$(rw_run_bounded "$BENCH_BUDGET" cargo bench --no-default-features --features "$features" --bench "$bench_name" -- --quick 2>&1)"; then
        printf '%s\n' "$output"
        if ! grep -q "time:" <<<"$output"; then
            echo "  ✗ $name benchmark produced no criterion measurement (harness ran 0 benchmarks)" >&2
            return 1
        fi
        echo "  ✓ $name benchmark completed successfully"
        return 0
    else
        echo "  ✗ $name benchmark FAILED (exit code $?)" >&2
        return 1
    fi
}

FAILURES=0
# Incremented by each `run_bench*` call, so the summary follows the execution list instead of
# hardcoding the count it happened to have (BN-1).
TOTAL=0

run_bench "Render" "render_bench" || FAILURES=$((FAILURES + 1))
echo "---"
run_bench "Signal" "signal_bench" || FAILURES=$((FAILURES + 1))
echo "---"
run_bench "Layout" "layout_bench" || FAILURES=$((FAILURES + 1))
echo "---"
run_bench "JSON" "json_bench" || FAILURES=$((FAILURES + 1))
echo "---"
run_bench "Event" "event_bench" || FAILURES=$((FAILURES + 1))
echo "---"
# The font benchmark needs a vector face to have anything to look up; without one every
# lookup is a miss and the numbers measure the empty-list case. The features are passed
# explicitly for that reason, and `desktop` is the device profile CI uses.
run_bench_featured "Font" "font_bench" "desktop,fonts-vector-latin,fonts-complex,fonts-cjk" || FAILURES=$((FAILURES + 1))
echo "---"
# These three are registered with non-default `required-features` in Cargo.toml, so they need the
# same explicit-feature treatment as the font benchmark. Before this they were never executed by
# the script at all, even though it advertises running every benchmark.
run_bench_featured "Tree Index" "tree_index_bench" "desktop" || FAILURES=$((FAILURES + 1))
echo "---"
run_bench_featured "Capability" "capability_bench" "desktop,declarative-view" || FAILURES=$((FAILURES + 1))
echo "---"
run_bench_featured "Wgpu" "wgpu_bench" "gpu-wgpu" || FAILURES=$((FAILURES + 1))

echo ""
echo "═══════════════════════════════════════════════════════════"
echo "  Benchmarks Complete: $((TOTAL - FAILURES))/$TOTAL passed"
echo "═══════════════════════════════════════════════════════════"

if [ "$FAILURES" -gt 0 ]; then
    echo "  ❌ $FAILURES benchmark(s) FAILED" >&2
    exit 1
fi

# Save summary for CI artifact comparison
{
    echo "Benchmark Run: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "Passed: $((TOTAL - FAILURES))"
    echo "Failed: ${FAILURES}"
} > "$SUMMARY_FILE"
echo "  → Summary saved to $SUMMARY_FILE"
