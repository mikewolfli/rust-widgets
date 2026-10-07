#!/usr/bin/env bash
# Every gesture event must have a production construction site (principle #75).
#
# # Why this gate exists
#
# `Event::DoubleTap` was impossible to produce: `GestureEngine::process` returned on the
# first recognizer that fired, and `TapGesture` — which sits earlier in the chain —
# satisfies every second tap of a double-tap. The recognizer itself was correct and its
# own tests passed, because they drove it directly instead of through the engine.
#
# No existing gate could see that. `check_behavior_matrix.sh` runs selected contracts and
# `check_control_has_tests.sh` asks whether each control is built by a test; neither asks
# whether an event a recognizer promises to emit can reach a consumer through the real
# path. A recognizer that can never fire has tests, a doc comment advertising the event,
# and looks like a working feature.
#
# The check is `tools/check_event_producers.py`; see its docstring for what counts as a
# producer and why test-only construction deliberately does not.
#
# # Why the validator also runs bounded, and its output is checked
#
# The gate used to accept any zero exit from the Python validator. A "pass" that
# produced no output is not distinguishable from a validator that never really
# ran — the same vacuity this file's own interpreter probe guards against. So the
# validator runs under `rw_run_bounded` (a wedged interpreter must not take the
# gate with it) and its result contract is asserted: on success it must print the
# `gesture events with a reachable producer:` summary line. A zero exit without
# that line is a FAIL.
#
# `tools/lib_python_selftest.sh` proves the interpreter probe rejects stubs.
#
# Usage: tools/check_event_producers.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

# Prove the probe still discriminates before trusting the interpreter it chose.
bash "$ROOT_DIR/tools/lib_python_selftest.sh"

. "$ROOT_DIR/tools/lib_python.sh"

OUTPUT=""
if ! OUTPUT="$(rw_run_bounded 600 "$PYTHON" tools/check_event_producers.py)"; then
    printf '%s\n' "$OUTPUT"
    echo "FAIL: a gesture event has no reachable producer"
    exit 1
fi

# The result contract: a passing run announces how many producers it verified.
# Without it a silent/interrupted interpreter would look identical to a pass.
if ! printf '%s\n' "$OUTPUT" | grep -q 'gesture events with a reachable producer:'; then
    printf '%s\n' "$OUTPUT"
    echo "FAIL: check_event_producers.py exited 0 without its result summary"
    exit 1
fi

printf '%s\n' "$OUTPUT"
echo "event producer checks passed."
