#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_i18n_catalogue.sh — language catalogues agree and name real controls
# ============================================================================
# The rule this guards (W7-04): `language/*.json` is a third parallel mechanism
# beside the capability table and the JSON loader, and nothing reconciled it with
# the registry. 61 `widget.*.label` keys named controls that do not exist, and
# 175 keys in all had no reader. The Python gate is the reconciliation; this
# wrapper puts it in the gate set so `run_all_gates.sh` (and CI that runs it)
# sees the same judgement.
#
# Usage: tools/check_i18n_catalogue.sh
# Exit 0 = catalogues agree and every control label is canonical.
# Exit 1 = a finding.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
. "$ROOT_DIR/tools/lib_python.sh"

"$PYTHON" tools/check_i18n_catalogue.py "$@"
