#!/usr/bin/env bash
# Performance regression gate for the two paths `docs/plans/TODO.md` names as
# performance-critical: `render_frame` and `dispatch_pointer_event`.
#
# Why a gate and not just a benchmark: a criterion run that nobody compares against
# anything cannot fail, so "the hot paths are still fast" stays an opinion. This
# script runs those two benchmarks, extracts the median, and compares it with the
# recorded baseline below.
#
# The thresholds are deliberately loose (3x). Wall-clock timing on shared CI runners
# varies by far more than that for reasons unrelated to the code, so a tight bound
# would fail constantly and get ignored — which is worse than no gate. 3x still
# catches the class of regression this exists for: an accidental per-frame allocation
# or an O(n) walk introduced into a hot path, which shows up as 10x or worse. Tighten
# only with a dedicated, quiet machine.
#
# Usage: tools/check_perf.sh [--update-baseline]
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

# `lib_python.sh` resolves a working interpreter (and exports UTF-8 mode, which the
# micro-sign parsing below needs).
. "$ROOT_DIR/tools/lib_python.sh"

# Recorded medians from a criterion run on the development host (x86_64 Linux,
# release profile). Regenerate with `--update-baseline` after an intentional change.
BASELINE_RENDER_FRAME_NS=194000   # ~194 us, 400x300
BASELINE_DISPATCH_NS=153          # ~153 ns, 9-widget tree
# Multiplier beyond which the run is considered a regression.
TOLERANCE=3

if [ "${1:-}" = "--update-baseline" ]; then
    echo "Re-running the benchmarks to refresh the recorded baseline..."
fi

run_one() {
    local filter="$1"
    timeout 600 cargo bench --no-default-features --features desktop \
        --bench render_bench -- "$filter" \
        --warm-up-time 0.5 --measurement-time 2 --sample-size 20 2>&1
}

# Criterion prints `name   time:   [lo mid hi]`, wrapping long names onto their own
# line. The unit is one of ns / us (written with a micro sign) / ms / s.
#
# Parsing is delegated to Python on purpose: the unit glyph is multi-byte, and getting
# that right in `sed`/`awk` is exactly the kind of fiddly escaping that silently
# produces a zero — which would make this gate pass everything.
extract_median_ns() {
    local output="$1"
    printf '%s' "$output" | "$PYTHON" -c '
import re, sys

text = sys.stdin.read()
# The first `time:` line is the measurement; later ones are the change report.
m = re.search(r"time:\s*\[\s*([0-9.]+)\s*(\S+?)\s+([0-9.]+)\s*(\S+?)\s+([0-9.]+)", text)
if not m:
    sys.exit(0)

mid = float(m.group(3))
unit = m.group(4)
factors = {"ns": 1, "us": 1_000, "µs": 1_000, "ms": 1_000_000, "s": 1_000_000_000}
if unit not in factors:
    sys.exit(0)
print(int(mid * factors[unit]))
'
}

echo "=== Performance regression gate ==="
echo "  (tolerance: ${TOLERANCE}x the recorded baseline)"
echo

fail=0

echo "[1/2] render_frame 400x300"
output=$(run_one "render_frame 400x300")
measured=$(extract_median_ns "$output")
if [ -z "$measured" ]; then
    echo "  FAIL: could not read a measurement from the benchmark output"
    echo "$output" | tail -5
    fail=1
else
    limit=$((BASELINE_RENDER_FRAME_NS * TOLERANCE))
    if [ "$measured" -gt "$limit" ]; then
        echo "  FAIL: ${measured} ns exceeds ${limit} ns (baseline ${BASELINE_RENDER_FRAME_NS} ns)"
        echo "  A 3x regression on a full frame usually means a new per-frame allocation."
        fail=1
    else
        echo "  ${measured} ns  (baseline ${BASELINE_RENDER_FRAME_NS} ns, limit ${limit} ns)  OK"
    fi
fi

echo
echo "[2/2] dispatch_pointer_event 9-widget tree"
output=$(run_one "dispatch_pointer_event")
measured=$(extract_median_ns "$output")
if [ -z "$measured" ]; then
    echo "  FAIL: could not read a measurement from the benchmark output"
    echo "$output" | tail -5
    fail=1
else
    limit=$((BASELINE_DISPATCH_NS * TOLERANCE))
    if [ "$measured" -gt "$limit" ]; then
        echo "  FAIL: ${measured} ns exceeds ${limit} ns (baseline ${BASELINE_DISPATCH_NS} ns)"
        echo "  A 3x regression here usually means the hit-test walk stopped short-circuiting."
        fail=1
    else
        echo "  ${measured} ns  (baseline ${BASELINE_DISPATCH_NS} ns, limit ${limit} ns)  OK"
    fi
fi

echo
if [ "$fail" -ne 0 ]; then
    echo "Performance gate FAILED."
    echo "  If the change is intentional, re-measure and update the baseline constants"
    echo "  at the top of this script (and say so in the commit message)."
    exit 1
fi
echo "Performance gate passed."
