#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_plan_tables.sh — every plan's Markdown tables are well formed
# ============================================================================
# `check_plan_tables.py` was a real gate (exit 1 on a short row) with no wrapper
# and no caller, so it only ran when someone remembered it (W8-12). This wrapper
# puts it on the `run_all_gates.sh` glob and therefore on CI.
#
# The scan covers every `docs/plans/**/*.md`, not the one archived plan it started
# from: an active plan's table is the one a reader is acting on.
#
# Usage: tools/check_plan_tables.sh [--list]
# Exit 0 = every table row matches its header. Exit 1 = a finding.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
. "$ROOT_DIR/tools/lib_python.sh"

"$PYTHON" tools/check_plan_tables.py "$@"
