#!/usr/bin/env bash
# A container that owns programmatic mutators must honour `enabled` in the emitter.
#
# # Why this gate exists
#
# `check_enabled_is_honoured.sh` gates `handle_event`: a control that handles user input must
# consult `is_enabled()`. That check has a structural blind spot -- the container that owns a
# *programmatic* mutator. `StackedWidget` was allowlisted as "passive: its handler only
# delegates to the base"; the handler did delegate, but `set_current_index` emitted
# `current_changed` while the control was disabled, so a subscriber reloaded a page the user
# could not reach. `handle_event` was never on the path, so the handler gate could not see it.
#
# This gate covers the exit point (signals this library emits) the way the handler gate covers
# the entry point (events this library consumes). Same defect class, one layer down.
#
# The check is `tools/check_enabled_is_honoured_containers.py`; it imports the allowlist from
# its sibling so the two lists cannot drift.
#
# Usage: tools/check_enabled_is_honoured_containers.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

if ! "$PYTHON" tools/check_enabled_is_honoured_containers.py; then
    echo "FAIL: a container emits a programmatic signal while it may be disabled"
    exit 1
fi

echo "enabled contract (containers) checks passed."
