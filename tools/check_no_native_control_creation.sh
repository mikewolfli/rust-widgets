#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
# ============================================================================
# check_no_native_control_creation.sh
# ============================================================================
# The library paints every `WidgetKind` itself on every backend. Both READMEs
# state that as a fact about the source, and a host relies on it — it is why the
# same widget looks identical everywhere, why `mini` needs no GUI toolkit, and
# why the SVG/PNG backends exist at all.
#
# Nothing checked it, and the claim had already drifted: the READMEs listed
# `CreateWindowExW` among the things the crate does not contain, while five
# occurrences existed — because **window creation** legitimately uses it. The
# claim was over-broad, not the code.
#
# This gate draws the line the architecture draws:
#
#   allowed  — creating a window or a drawing surface
#   forbidden — creating a per-kind control (`gtk_button_new`, `NSButton` alloc,
#               `UIButton`, an `android.widget.*` class string, WC_BUTTON, …)
#
# Comments are stripped before the search: a backend's rationale names exactly
# these functions to explain why they are *not* used, and counting those would
# make the gate pass on the strength of the prose explaining the absence.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
. "$ROOT_DIR/tools/lib_python.sh"

"$PYTHON" tools/check_no_native_control_creation.py

echo "✅ no native control creation: the library paints every WidgetKind on every backend"
