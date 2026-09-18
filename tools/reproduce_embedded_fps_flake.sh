#!/usr/bin/env bash
# Reproduce the `embedded_target_fps_clamps` flake.
#
# The hypothesis: `embedded_engine.rs::test_guard` is a *module-local* mutex over the
# same process-wide embedded engine that `embedded.rs::embedded_test_guard` protects.
# Two locks over one resource exclude nothing, so `embedded_engine.rs`'s tests can set
# the target FPS to 120 while `embedded_target_fps_clamps` is asserting 72.
#
# Before this file is trusted, `git stash` the fix and run it: it must fail. After the
# fix it must pass every iteration. A reproducer that cannot fail proves nothing.
#
# Usage: tools/reproduce_embedded_fps_flake.sh [iterations]
set -uo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ITERATIONS="${1:-10}"
FAILED=0

echo "Running the embedded engine tests ${ITERATIONS}x, looking for a contended FPS value..."

for iteration in $(seq 1 "$ITERATIONS"); do
  # `--test-threads` is left at the default on purpose: the flake needs the two modules
  # to run concurrently, which is the condition a developer sees. The filter selects both
  # modules so they are scheduled in the same process.
  output="$(cargo test --lib --no-default-features --features embedded embedded_ 2>&1)"
  if printf '%s' "$output" | grep -q "test result: FAILED"; then
    FAILED=$((FAILED + 1))
    echo "  iteration $iteration: FAILED"
    printf '%s\n' "$output" | grep -E "panicked at|left:|right:|^---- " | head -6
  else
    printf '  iteration %s: ok\n' "$iteration"
  fi
done

echo
if [[ "$FAILED" -gt 0 ]]; then
  echo "reproduced: ${FAILED}/${ITERATIONS} iterations failed"
  exit 1
fi
echo "no contention observed in ${ITERATIONS} iterations"
