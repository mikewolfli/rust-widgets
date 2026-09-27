#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_alias_tables_agree.sh — BLUE25 C-7
# ============================================================================
# `src/widget/capability.rs` resolves an alias `WidgetKind` two ways, and the two
# tables are compiled under different `cfg`s so no single build sees both:
#
#   * `alias_factory_name` — a `match` on variants, `full_widgets`;
#   * `alias_for_name`     — a `match` on canonical spellings, `not(full_widgets)`.
#
# The module comment used to claim the two "cannot disagree"; they could, because
# nothing compared them. This gate parses both tables from source and requires them
# to name the same `(spelling, target)` pairs, which is the only place that can see
# them together.
#
# Reverse injection: deleting a row from either table must make this fail with that
# pair named.
#
# Usage: tools/check_alias_tables_agree.sh
# Exit 0 = the two alias tables name the same pairs.
# Exit 1 = a finding (each disagreeing pair is named).
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
. "$ROOT_DIR/tools/lib_python.sh"

"$PYTHON" tools/check_alias_tables_agree.py

echo "✅ alias tables agree: src/widget/capability.rs"
