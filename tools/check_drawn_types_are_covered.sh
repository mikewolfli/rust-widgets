#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_drawn_types_are_covered.sh — 每个 `impl Draw` 都要有门禁覆盖
# ============================================================================
# Every gate in this crate is built around one number: the 188 controls in
# `capability/properties.rs`. `check_svg_snapshots.sh` counts 188 x 2 snapshot files,
# `check_control_rendering.sh` sweeps the factory registry, `check_control_has_tests.sh`
# walks the same names. A struct that implements `Draw` and matches none of those names is
# outside all three — not by oversight, but by construction.
#
# That is correct for an engine a registered control delegates to (`ChartWidget` behind
# `chart`, `FreeformShapeWidget` behind `freeform_shape`): the registered control's own
# snapshot covers the delegated drawing. It is a defect for a control a host can mount and
# nothing checks. One such type was found while writing this gate — `KeySequenceEdit`, which
# exports a public type, draws six colour literals, and is constructed by nothing.
#
# What it asserts
# ---------------
#   [1] every `impl Draw` type is registered, constructed by `constructors.rs`, or listed in
#       the checker's ACKNOWLEDGED table with the plan item that will connect it
#   [2] an ACKNOWLEDGED entry that has since become covered is reported STALE, so the table
#       can only shrink
#
# Reverse injection
# -----------------
# `--inject=<Type>` pretends the named registered type has lost its registration and requires
# the answer to change, so a search that matched nothing cannot pass. Verified by hand:
#   python3 tools/check_drawn_types_are_covered.py --inject=ChartWidget   -> FAIL, names it
#
# Usage: tools/check_drawn_types_are_covered.sh
# Exit 0 = every drawn type is covered or acknowledged. Exit 1 = a finding.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
. "$ROOT_DIR/tools/lib_python.sh"

if ! "$PYTHON" tools/check_drawn_types_are_covered.py; then
    echo "FAIL: an `impl Draw` type is covered by no gate, or an acknowledgement is stale"
    exit 1
fi

echo "✅ check_drawn_types_are_covered: every drawn type is covered or acknowledged"
