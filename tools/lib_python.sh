#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# Portable Python 3 resolution for the QA gates.
#
# # Why `python3` cannot be assumed
#
# `python3` is the interpreter name on Linux and macOS, so the gates called it
# directly. On Windows that name is frequently not Python at all: the Microsoft
# Store installs an execution alias at
# `%LOCALAPPDATA%\Microsoft\WindowsApps\python3.exe` which, invoked without an
# interactive console, blocks on the Store UI and never runs the script. A gate
# therefore *hangs* instead of failing — the worst failure mode for CI, because
# nothing is reported. Meanwhile the real interpreter is installed as `python`
# (and the launcher `py`).
#
# # How this resolves it
#
# Each candidate is *tested* rather than trusted, and the test checks two
# independent things:
#
#   1. It must actually *print* the marker `3` — the probe runs
#      `-c 'import sys; print(sys.version_info[0])'` and compares the captured
#      output to `3`. Exit status alone is not enough: a stub that is not Python
#      (or a wrong-major `python`) can return 0 while doing nothing, and would
#      then be chosen as the gate interpreter. Requiring the marker means the
#      accepted program really is a Python 3 that executes code.
#   2. It must complete inside a wall-clock bound (`rw_run_bounded`). The Windows
#      Store alias named at the top does not exit — it *blocks* on the Store UI —
#      so a probe that waited on it would hang the gate, the exact failure this
#      file documents. A bounded probe skips a hanging candidate and tries the
#      next one; if every candidate hangs or fails, this fails explicitly rather
#      than silently.
#
# Usage — source it, then use `"$PYTHON"`:
#
#   ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
#   cd "$ROOT_DIR"
#   . "$ROOT_DIR/tools/lib_python.sh"
#   "$PYTHON" tools/some_generator.py
#
# # Injecting a fake interpreter (self-test)
#
# The probe can be exercised without a broken host by putting a decoy earlier on
# PATH. A stub that exits 0 without printing `3` must be *rejected*:
#
#   mkdir -p /tmp/fakebin
#   printf '#!/bin/sh\nexit 0\n' > /tmp/fakebin/python3
#   chmod +x /tmp/fakebin/python3
#   PATH="/tmp/fakebin:$PATH" bash -c '
#     . tools/lib_python.sh
#     printf "chosen=%s\n" "$PYTHON"'
#   # chosen must NOT be the decoy under /tmp/fakebin
#
# tools/lib_python_selftest.sh automates exactly this (exit-status-only stub and
# a silent stub) and is run by check_event_producers.sh.

# The minimum wall-clock budget for a single interpreter probe, in seconds. The
# probe is a trivial `-c` one-liner on a working interpreter (well under a
# second); the budget only has to be short enough that a hanging Store alias is
# abandoned quickly while still leaving room for a cold interpreter start.
#
# Respects a pre-set value (`RW_PYTHON_PROBE_TIMEOUT=2 …`, used by the
# self-test to keep the hanging-stub case cheap) instead of clobbering it.
RW_PYTHON_PROBE_TIMEOUT="${RW_PYTHON_PROBE_TIMEOUT:-10}"

# The printed marker that proves a candidate is a working Python 3.
RW_PYTHON_PROBE_MARKER=3

# Prints the first interpreter that really is Python 3, or fails.
#
# Bounded by `rw_run_bounded` (from `tools/lib_timeout.sh`, sourced below) so a
# candidate that blocks — the Store alias — is skipped rather than hanging the
# gate. A timed-out candidate yields status 124 and is treated as "not Python".
find_python() {
  local candidate output status
  for candidate in python3 python py; do
    if ! command -v "$candidate" >/dev/null 2>&1; then
      continue
    fi
    # Capture the marker, discarding stderr so a candidate's own warnings cannot
    # be mistaken for the version marker.
    output="$(rw_run_bounded "$RW_PYTHON_PROBE_TIMEOUT" \
      "$candidate" -c 'import sys; print(sys.version_info[0])' 2>/dev/null)"
    status=$?
    if [ "$status" -eq 0 ] && [ "$output" = "$RW_PYTHON_PROBE_MARKER" ]; then
      printf '%s\n' "$candidate"
      return 0
    fi
    if [ "$status" -eq 124 ]; then
      printf 'skipping %s: probe timed out after %ss\n' "$candidate" "$RW_PYTHON_PROBE_TIMEOUT" >&2
    fi
  done
  printf 'no working Python 3 interpreter found (tried: python3, python, py)\n' >&2
  return 1
}

# `rw_run_bounded` bounds each candidate probe. Sourced here rather than by each
# gate so the dependency travels with the probe that needs it.
_LIB_PYTHON_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=tools/lib_timeout.sh
. "$_LIB_PYTHON_DIR/lib_timeout.sh"

PYTHON="$(find_python)" || {
  printf 'This gate needs Python 3; install it and make sure it is on PATH.\n' >&2
  exit 127
}

# # Why the locale codec is also forced to UTF-8
#
# A gate's own output is not ASCII: success and failure lines carry the ✅/❌
# glyphs, and the generators read and write documents containing CJK text. On
# Windows Python defaults to the ANSI code page (cp936 on a Chinese install),
# which can neither encode those glyphs on stdout nor decode UTF-8 sources:
#
#   UnicodeEncodeError: 'gbk' codec can't encode character '\u2705'
#   UnicodeDecodeError: 'gbk' codec can't decode byte 0x94
#
# Either one turns a passing gate into a crash. UTF-8 mode makes the child's
# default encoding explicit and host-independent. File I/O in the tools still
# passes `encoding="utf-8"` explicitly — the environment is the safety net for
# `print`, not a substitute for saying what encoding a file is.
export PYTHONUTF8=1
export PYTHONIOENCODING=utf-8
