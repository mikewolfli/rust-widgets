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
exec python3 "$here/check_mousepress_carries_modifiers.py" "$@"
