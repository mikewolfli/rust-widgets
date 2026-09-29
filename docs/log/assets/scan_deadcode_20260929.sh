#!/usr/bin/env bash
# Probe: enumerate suspicious patterns for the 2026-09-29 deep-scan round.
# Read-only. No cargo. Emits counts + first N offenders per category.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

emit() { # name pattern include max
  local name="$1" pat="$2" inc="$3" max="$4"
  local n
  n=$(grep -rnE "$pat" --include="$inc" src demo 2>/dev/null | wc -l | tr -d ' ')
  printf '\n=== %s : %s\n' "$name" "$n"
  if [ "$n" != "0" ]; then
    grep -rnE "$pat" --include="$inc" src demo 2>/dev/null | head -"$max"
  fi
}

emit "allow(dead_code)"            'allow\(dead_code\)'                       '*.rs' 40
emit "allow(unused"                'allow\(unused'                            '*.rs' 30
emit "todo!/unimplemented!"        'todo!\(|unimplemented!\('                 '*.rs' 30
emit "placeholder-ish identifiers" '\b(placeholder|stub|FIXME|XXX|HACK)\b'    '*.rs' 40
emit "cfg(target_os) outside platform" 'cfg\(target_os'                       '*.rs' 60
emit "empty fn body"               'fn [a-z_0-9]+\([^)]*\) *\{\s*\}\s*$'     '*.rs' 40
emit "deprecated attr"             '#\[deprecated'                            '*.rs' 30
emit "unwrap in lib non-test"      '\.unwrap\(\)'                             '*.rs' 20
