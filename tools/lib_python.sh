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
# Each candidate is *tested* rather than trusted: it is accepted only if it
# actually runs and reports Python 3. A Store alias fails that probe (it exits
# without producing the check's status), so it is skipped automatically on any
# host.
#
# Usage — source it, then use `"$PYTHON"`:
#
#   ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
#   cd "$ROOT_DIR"
#   . "$ROOT_DIR/tools/lib_python.sh"
#   "$PYTHON" tools/some_generator.py

# Prints the first interpreter that really is Python 3, or fails.
find_python() {
  local candidate
  for candidate in python3 python py; do
    if command -v "$candidate" >/dev/null 2>&1 &&
      "$candidate" -c 'import sys; sys.exit(0 if sys.version_info[0] == 3 else 1)' \
        >/dev/null 2>&1; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done
  printf 'no working Python 3 interpreter found (tried: python3, python, py)\n' >&2
  return 1
}

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
