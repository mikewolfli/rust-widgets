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
# Usage: tools/check_event_producers.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

if ! "$PYTHON" tools/check_event_producers.py; then
    echo "FAIL: a gesture event has no reachable producer"
    exit 1
fi

echo "event producer checks passed."
