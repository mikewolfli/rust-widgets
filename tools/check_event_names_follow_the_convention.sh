#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_event_names_follow_the_convention.sh — DX-10
# ============================================================================
# The rule this guards:
#
#   **A published event name describes the event, not a listener method.**
#
# `on_` is a handler prefix in other toolkits (`on_click=`), not an event name. The crate
# published exactly one such name (`on_line_crossed`), which is how DX-10 found that the
# vocabulary was drifting by *prefix* rather than by concept — and a name is the whole
# contract on the wire: a document, a designer manifest and every binding spell it.
#
# What it checks
# --------------
#   (1) `tools/event_published_census.txt` (the source of truth for published names) is
#       parsable and non-empty;
#   (2) no published name starts with `on_`;
#   (3) the generated payload table agrees — a prefixed name there would reach a designer
#       even if the census were right.
#
# What it does NOT check
# ---------------------
#   * It does not judge whether a name is *good*, only that it is not a listener prefix.
#   * It does not forbid a legacy `on_` spelling in `EVENT_ALIASES`: an alias is the record
#     of an old name, and `event_signal_dyn` answering it is what keeps old documents bound.
#
# Reverse injection: add `on_bogus` to a census row and this gate names it (see
# `tools/gates_reverse_injection.md`).
#
# Usage: tools/check_event_names_follow_the_convention.sh
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

if ! "$PYTHON" tools/check_event_names_follow_the_convention.py; then
    echo "FAIL: a published event name does not follow the vocabulary convention"
    exit 1
fi

echo "event name convention checks passed."
