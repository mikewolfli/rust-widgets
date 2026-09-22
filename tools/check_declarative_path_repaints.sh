#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# The declarative property-write path must ask for a repaint (BLUE21 E1 / P3-1f).
#
# # Why this gate exists
#
# `runtime::with_widget_mut` mutates a control in place and invalidates nothing — its own
# documentation requires the caller to follow a mutation with `request_repaint`. The
# name-based write path obeys that; `view/apply.rs` did not, so the *same* logical
# `SetProperty` repainted when it arrived through a name and left a stale frame when it
# arrived through a `Node`. The control toggled; the user saw the old value.
#
# It is exactly the class of defect no other gate can see: the widget state is correct, the
# declarations are correct, and only the frame is wrong.
#
# The reasoning, the reachability check and the allowlist live in the Python checker.
#
# Usage: tools/check_declarative_path_repaints.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

if ! "$PYTHON" tools/check_declarative_path_repaints.py; then
    echo "FAIL: the declarative property-write path does not request a repaint"
    exit 1
fi
