#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_event_variants_have_a_producer.sh — principle #75, for the whole enum
# ============================================================================
# `check_event_producers.sh` proves every *gesture* event can be produced. It
# deliberately derives its list from the recognizer modules, so the rest of the
# `Event` enum was never audited — and the same defect class was sitting there:
#
#   * `GamepadPress` / `GamepadRelease` / `GamepadAxis` / `GamepadConnected` /
#     `GamepadDisconnected`: five variants, five constructors, five constructor
#     tests, **zero producers and zero consumers**. Removed.
#   * `ImeCommit` / `ImePreedit`: matched by the code editor, `tag_input` and
#     `search_bar`, produced by nobody — the platform `ImeBridge` drove the OS
#     composition but nothing joined it to the widget event layer. Now produced
#     by `platform::ime::deliver_composition` / `deliver_commit`.
#   * `OrientationChanged`: no producer, no consumer (the host owns the observer).
#
# The check is `tools/check_event_variants_have_a_producer.py`; see its
# docstring for what counts as a producer and why a host-produced variant must
# be declared with a reason rather than assumed.
#
# Reverse injection: adding a variant with no producer must make this fail,
# naming it.
#
# Usage: tools/check_event_variants_have_a_producer.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

if ! "$PYTHON" tools/check_event_variants_have_a_producer.py; then
    echo "FAIL: an Event variant has no producer"
    exit 1
fi

echo "every Event variant has a producer or a declared host producer."
