#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# Self-test for `tools/lib_python.sh`'s interpreter probe (N-G-03, N-G-04).
#
# The probe exists because `python3` cannot be trusted: on Windows it is often a
# Microsoft Store *alias* that blocks, and a wrong-major `python` can exit 0
# without being the interpreter a gate needs. A probe that only checked the exit
# status would accept either. This script proves, on a real host with a real
# Python 3, that `find_python` rejects both decoys:
#
#   1. an `exit-status-only` stub — exits 0 but prints nothing;
#   2. a `silent` stub — prints a non-3 value (a wrong-major marker).
#
# and that it still accepts the real interpreter.
#
# It also proves the hang case: a stub that never exits must be *skipped* inside
# the probe's bound rather than hanging the whole run.
#
# Run directly (`bash tools/lib_python_selftest.sh`) or from
# `tools/check_event_producers.sh`, which depends on the probe.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

# Shrink the probe budget so the hanging-stub case does not cost the default 10s.
export RW_PYTHON_PROBE_TIMEOUT="${RW_PYTHON_PROBE_TIMEOUT:-2}"

DECOY_DIR="$(mktemp -d)"
trap 'rm -rf "$DECOY_DIR"' EXIT

# `find_python` in a subshell with the decoys first on PATH. Prints either the
# chosen interpreter name or, on failure, the "no working" message to stderr.
find_with_decoys() {
  PATH="$DECOY_DIR:$PATH" bash -c '
    . "$1/tools/lib_python.sh"
    printf "%s\n" "$PYTHON"
  ' _ "$ROOT_DIR"
}

# ── Case 1: an exit-status-only stub that prints nothing ──────────────────────
cat > "$DECOY_DIR/python3" <<'STUB'
#!/bin/sh
# Exit 0 without being Python. The old probe accepted this.
exit 0
STUB
chmod +x "$DECOY_DIR/python3"
chosen="$(find_with_decoys)"
if [ "$chosen" = "$DECOY_DIR/python3" ] || [ "$chosen" = "python3" -a -x "$DECOY_DIR/python3" ]; then
  echo "FAIL: exit-status-only stub was accepted as the interpreter" >&2
  exit 1
fi
echo "  ok: exit-status-only stub rejected (chose: $chosen)"

# ── Case 2: a stub that prints a wrong-major marker ───────────────────────────
cat > "$DECOY_DIR/python3" <<'STUB'
#!/bin/sh
# A wrong-major interpreter: exits 0 and reports version 2.
echo 2
exit 0
STUB
chmod +x "$DECOY_DIR/python3"
chosen="$(find_with_decoys)"
if [ "$chosen" = "$DECOY_DIR/python3" ] || [ "$chosen" = "python3" -a -x "$DECOY_DIR/python3" ]; then
  echo "FAIL: wrong-major marker was accepted as the interpreter" >&2
  exit 1
fi
echo "  ok: wrong-major marker rejected (chose: $chosen)"

# ── Case 3: a stub that hangs must be skipped, not waited on ──────────────────
cat > "$DECOY_DIR/python3" <<'STUB'
#!/bin/sh
# The Store-alias failure mode: never exits.
sleep 3600
STUB
chmod +x "$DECOY_DIR/python3"
start=$SECONDS
chosen="$(find_with_decoys)"
elapsed=$((SECONDS - start))
if [ "$chosen" = "$DECOY_DIR/python3" ] || [ "$chosen" = "python3" -a -x "$DECOY_DIR/python3" ]; then
  echo "FAIL: a hanging stub was accepted as the interpreter" >&2
  exit 1
fi
# It must have been abandoned inside the bound (plus a little scheduling slack),
# not after the stub's own 3600s sleep.
if [ "$elapsed" -gt $((RW_PYTHON_PROBE_TIMEOUT + 15)) ]; then
  echo "FAIL: probe did not bound the hanging stub (took ${elapsed}s)" >&2
  exit 1
fi
echo "  ok: hanging stub skipped within the bound (${elapsed}s, chose: $chosen)"

# ── Case 4: the real host interpreter is still accepted ───────────────────────
PYTHON="$(bash -c '
  . "$1/tools/lib_python.sh"
  printf "%s\n" "$PYTHON"
' _ "$ROOT_DIR")"
if [ -z "$PYTHON" ]; then
  echo "FAIL: no real interpreter was found with the decoys removed" >&2
  exit 1
fi
real_marker="$("$PYTHON" -c 'import sys; print(sys.version_info[0])')"
if [ "$real_marker" != "3" ]; then
  echo "FAIL: chosen real interpreter '$PYTHON' does not report Python 3" >&2
  exit 1
fi
echo "  ok: real interpreter accepted (chose: $PYTHON)"

echo "python probe self-test passed."
