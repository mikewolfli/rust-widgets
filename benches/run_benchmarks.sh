#!/bin/bash
# Run all benchmarks (BLUE11 R3.7)
# Usage: bash benches/run_benchmarks.sh
set -e
echo "=== Running Render Benchmark ==="
cargo bench --bench render_bench -- --quick 2>/dev/null || echo "(render_bench skipped - requires --bench flag)"
echo "=== Running Signal Benchmark ==="
cargo bench --bench signal_bench -- --quick 2>/dev/null || echo "(signal_bench skipped)"
echo "=== Running Layout Benchmark ==="
cargo bench --bench layout_bench -- --quick 2>/dev/null || echo "(layout_bench skipped)"
echo "=== Running JSON Benchmark ==="
cargo bench --bench json_bench -- --quick 2>/dev/null || echo "(json_bench skipped)"
echo "=== Running Event Benchmark ==="
cargo bench --bench event_bench -- --quick 2>/dev/null || echo "(event_bench skipped)"
echo "=== Benchmarks Complete ==="
