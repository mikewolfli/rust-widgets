#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

python3 tools/generate_control_route_matrix.py \
  --kind src/widget/kind.rs \
  --routing src/control_backend/routing.rs \
  --trait src/control_backend/trait_def.rs \
  --native src/control_backend/native.rs \
  --custom src/control_backend/custom.rs \
  --output target/qa/control_route_matrix.md \
  --fail-on-placeholder \
  --fail-on-contract-miss

echo "Control route matrix generated at target/qa/control_route_matrix.md"
