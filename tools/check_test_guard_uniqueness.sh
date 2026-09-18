#!/usr/bin/env bash
# ============================================================================
# check_test_guard_uniqueness.sh — one test guard per process-wide singleton
# ============================================================================
# `embedded_target_fps_clamps` failed intermittently because two modules declared
# separate test mutexes over the same process-wide embedded engine. Two locks over
# one resource exclude nothing, so a writer in one module landed inside the other
# module's assertion.
#
# The flake was nanoseconds wide, so it passed on retry and looked environmental.
# This gate turns it into a static property: a second guard over shared state is
# reported before anyone has to reproduce a race to find it.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
. "$ROOT_DIR/tools/lib_python.sh"

"$PYTHON" tools/check_test_guard_uniqueness.py
