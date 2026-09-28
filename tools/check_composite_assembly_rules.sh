#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_composite_assembly_rules.sh — BLUE22 §B.6 (composite assembly)
# ============================================================================
# The rule this guards (BLUE22 §B.6):
#
#   **A composite control's children are created through `WidgetFactory::create` and positioned by
#   a `Layout`.** No concrete `Type::new(..)`, no `rect.x + k`.
#
# # Why this one is worth a gate
#
# `src/widget/composite.rs` established the channel — a composite asks a layout where its children
# go and asks each child how big it wants to be — and eleven controls were migrated onto it. What
# was missing is anything that keeps them there. Every one of §B.6's nine rules was a convention,
# and `grep -rn "B.6" tools/` found nothing.
#
# That is the same shape as the defect the refactor fixed. Hand-computed geometry was never wrong
# on purpose; it was simply unchecked, and the same "icon + gap + label" arithmetic was rewritten
# in eleven files before anyone noticed. A convention that nothing enforces is one `cargo fmt`
# away from being gone.
#
# # What it checks, and its honest limits
#
# Two of the nine rules are lexical and mechanical (§B.6 rules 1 and 2); those are enforced here.
# The rest — hints propagating upward, the dirty flag causing a re-arrange, a 4 px child getting a
# 48 px touch target — are properties of behaviour, are covered by the unit tests in
# `composite.rs` and the per-control suites, and would be asserted only in their *spelling* by a
# lexical scan. `tools/check_composite_assembly_rules.py`'s header states that split explicitly.
#
# # Reverse injection
#
# `--inject` removes the layout call from a migrated composite and requires the gate to fail, then
# does the same for the factory call. Without it, a check that had quietly stopped reading its
# input would pass against any tree.
#
# Usage: tools/check_composite_assembly_rules.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_timeout.sh"

# The scan is lexical, so it needs no build; the timeout is here because every gate in this
# directory has one, and a gate that can hang is a gate that gets skipped.
rw_run_bounded "${RW_GATE_TIMEOUT:-120}" python3 "$ROOT_DIR/tools/check_composite_assembly_rules.py"

# The injection is run as part of the gate rather than offered as a flag nobody passes: its whole
# value is that it runs on the tree the other steps just passed.
rw_run_bounded "${RW_GATE_TIMEOUT:-120}" python3 "$ROOT_DIR/tools/check_composite_assembly_rules.py" --inject

echo "composite assembly rule checks passed."
