#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
check_widget_registration_fidelity.sh -- every `WidgetKind` is reachable, aliased or exempt.
#
# Delegates to `tools/check_widget_registration_fidelity.py`, which is the documentation of record
# for what the check proves and how it is reverse-injected.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
. "$ROOT_DIR/tools/lib_python.sh"

"$PYTHON" tools/check_widget_registration_fidelity.py "$@"
