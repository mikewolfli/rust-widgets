#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# check_widget_registration_fidelity.sh -- every `WidgetKind` is reachable, aliased or exempt.
#
# Delegates to `tools/check_widget_registration_fidelity.py`, which is the documentation of record
# for what the check proves and how it is reverse-injected.
#
# The Python gate bounds each of its two subprocess calls at 900 s. This wrapper's budget is
# larger so the inner bound produces the verdict first (principle #104), while a direct
# invocation is still bounded if the Python process itself wedges (principle #58).
#
# rw-gate-budget: 1900

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
. "$ROOT_DIR/tools/lib_python.sh"
. "$ROOT_DIR/tools/lib_timeout.sh"

rw_run_bounded 1900 "$PYTHON" tools/check_widget_registration_fidelity.py "$@"
