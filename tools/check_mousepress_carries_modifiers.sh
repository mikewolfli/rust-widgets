#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# Gate: `Event::MousePress` must carry the modifier bitmask end to end, so that a
# `Shift`/`Ctrl` click can implement `SelectionMode::Extended`'s range/toggle behaviour.
#
# See tools/check_mousepress_carries_modifiers.py for what is asserted and why.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# Resolve Python through the shared helper rather than exec'ing `python3`.
#
# On Windows, `python3` is often the Microsoft Store execution alias, which blocks on the
# Store UI instead of running the script — the gate hangs rather than failing (see
# tools/lib_python.sh). The helper probes candidates under a wall-clock bound and picks a
# real Python 3.
# shellcheck source=tools/lib_python.sh
. "$here/lib_python.sh"
exec "$PYTHON" "$here/check_mousepress_carries_modifiers.py" "$@"
