#!/usr/bin/env bash
# Read-only inventory probe. No cargo.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT"
echo "== categories =="
for d in src/widget src/layout src/event src/style src/render src/platform src/view src/app; do
  n=$(find "$d" -name '*.rs' 2>/dev/null | wc -l | tr -d ' ')
  l=$(find "$d" -name '*.rs' -exec cat {} + 2>/dev/null | wc -l | tr -d ' ')
  printf '%-24s files=%-5s lines=%s\n' "$d" "$n" "$l"
done
echo
echo "== largest lib files =="
find src -name '*.rs' -exec wc -l {} + | sort -rn | head -25
echo
echo "== demo tree (depth 2) =="
find demo -maxdepth 2 -name Cargo.toml | sort
echo
echo "== tools count =="
ls tools/*.sh 2>/dev/null | wc -l
