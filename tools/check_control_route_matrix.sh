#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
check_control_route_matrix.sh -- every registered kind must have a real implementation route.
#
# Generates the control-route matrix into `target/qa/control_route_matrix.md` and fails on a
# placeholder or a contract miss, so a kind that is registered but unrouted cannot ship.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
. "$ROOT_DIR/tools/lib_python.sh"

"$PYTHON" tools/generate_control_route_matrix.py \
  --kind src/widget/kind.rs \
  --routing src/control_backend/routing.rs \
  --trait src/control_backend/trait_def/trait_def \
  --native src/control_backend/native.rs \
  --custom src/control_backend/custom/create_widgets.rs \
  --output target/qa/control_route_matrix.md \
  --fail-on-placeholder \
  --fail-on-contract-miss

echo "Control route matrix generated at target/qa/control_route_matrix.md"
