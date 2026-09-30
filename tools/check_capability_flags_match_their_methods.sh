#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
# ============================================================================
# check_capability_flags_match_their_methods.sh
# ============================================================================
# Every `PlatformCapabilities` flag a backend declares must be backed by the
# method it promises. These flags escape the process as a C ABI bitmask, so a
# `true` without a method behind it is a host-visible false claim — the shape
# that was found six times by hand before this gate existed.
#
#   ime         -> Platform::ime_bridge
#   accessibility -> Platform::accessibility_bridge
#   native_menu -> Platform::create_menu_bar and friends
#   typed_widget_trigger -> poll/inject_widget_trigger_event
#   dpi_scaling -> Platform::dpi_scale_factor
#
# Runs the check and then the reverse injection, so the gate proves it still
# reads its input rather than silently passing on an empty parse.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
. "$ROOT_DIR/tools/lib_python.sh"

"$PYTHON" tools/check_capability_flags_match_their_methods.py

echo "✅ capability flags: every declared flag is backed by its promised method"
