#!/usr/bin/env bash
# Read-only regex inventory probe. No cargo. Second attempt with corrected paths.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT"

emit() { # name pattern max
  local name="$1" pat="$2" max="$3"
  local n
  n=$(grep -rnE "$pat" --include='*.rs' src demo 2>/dev/null | wc -l | tr -d ' ')
  printf '\n=== %-46s %s\n' "$name" "$n"
  if [ "$n" != "0" ]; then
    grep -rnE "$pat" --include='*.rs' src demo 2>/dev/null | head -"$max"
  fi
}

emit "allow(dead_code)"                 'allow\(dead_code\)'                       30
emit "allow(unused"                     'allow\(unused'                            20
emit "todo!/unimplemented!"             'todo!\(|unimplemented!\('                 20
emit "placeholder/stub-fn names"        'fn _*[a-z_0-9]*(placeholder|stub)'        20
emit "cfg(target_os)"                   'cfg\(target_os'                           40
emit "deprecated attr"                  '#\[deprecated'                            20
emit "let _ =  (discards)"              'let _ = '                                 10
emit "#[ignore]"                        '#\[ignore'                                20
emit "empty default trait impl"         'fn [a-z_0-9]+\([^)]*\) *\{\s*\}'          5
