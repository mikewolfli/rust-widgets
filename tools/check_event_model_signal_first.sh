#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "[signal-first gate] scanning for wxWidgets event-table patterns"

SEARCH_PATHS=(
  "src"
  "demos"
  "examples"
)

BLOCKED_PATTERN='wxBEGIN_EVENT_TABLE|wxEND_EVENT_TABLE|BEGIN_EVENT_TABLE|END_EVENT_TABLE|DECLARE_EVENT_TABLE|EVT_[A-Z0-9_]+'

if grep -R -n -E --exclude-dir=target --exclude-dir=.git --exclude-dir=.code-search "$BLOCKED_PATTERN" "${SEARCH_PATHS[@]}"; then
  echo
  echo "❌ Found blocked event-table patterns."
  echo "Use signal/slot routes (Signal<T>/GenericSignal + connect/emit), not wxWidgets-style tables."
  exit 1
fi

echo "✅ No wxWidgets-style event-table patterns detected."

echo "[signal-first gate] validating signal core presence"

if ! grep -q "pub struct GenericSignal" src/signal/mod.rs; then
  echo "❌ Missing GenericSignal core definition in src/signal/mod.rs"
  exit 1
fi

if ! grep -q "trait EventHandler" src/event/mod.rs; then
  echo "❌ Missing EventHandler dispatch contract in src/event/mod.rs"
  exit 1
fi

echo "✅ Signal-first event model guard passed."
