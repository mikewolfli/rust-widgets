#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

python3 tools/generate_feature_completeness_matrix.py \
  --src src \
  --output target/qa/feature_completeness_matrix.md \
  --threshold 1

echo "Feature completeness matrix generated at target/qa/feature_completeness_matrix.md"
