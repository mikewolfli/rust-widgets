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

EXISTING_PATHS=()
for path in "${SEARCH_PATHS[@]}"; do
  if [[ -d "$path" ]]; then
    EXISTING_PATHS+=("$path")
  fi
done

BLOCKED_PATTERN='wxBEGIN_EVENT_TABLE|wxEND_EVENT_TABLE|BEGIN_EVENT_TABLE|END_EVENT_TABLE|DECLARE_EVENT_TABLE|EVT_[A-Z0-9_]+'

if [[ ${#EXISTING_PATHS[@]} -gt 0 ]] && grep -R -n -E --exclude-dir=target --exclude-dir=.git --exclude-dir=.code-search "$BLOCKED_PATTERN" "${EXISTING_PATHS[@]}"; then
  echo
  echo "❌ Found blocked event-table patterns."
  echo "Use signal/slot routes (Signal<T>/GenericSignal + connect/emit), not wxWidgets-style tables."
  exit 1
fi

echo "✅ No wxWidgets-style event-table patterns detected."

echo "[signal-first gate] validating signal core presence"

if ! grep -q "pub struct GenericSignal" src/signal/generic_signal.rs; then
  echo "❌ Missing GenericSignal core definition in src/signal/generic_signal.rs"
  exit 1
fi

if ! grep -q "trait EventHandler" src/event/types.rs; then
  echo "❌ Missing EventHandler dispatch contract in src/event/types.rs"
  exit 1
fi

echo "✅ Signal-first event model guard passed."
