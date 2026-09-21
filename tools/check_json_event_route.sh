#!/usr/bin/env bash
# The JSON event route must resolve published names against the capability table (BLUE19 T-8/T-10).
#
# # Why this gate exists
#
# `src/json/` grew a second event path: eight hand-matched `on_*` keys against a capability table
# publishing 186 names. The sets were disjoint (`on_click` is not a published name; `clicked` is),
# so adding a published event never made the JSON side support it — and **no gate looked**
# (`grep -rn "on_click" tools/` returned nothing). That is the parallel-mechanism drift rule #101
# forbids, and this gate is what makes it detectable.
#
# The check is `tools/check_json_event_route.py`; see its docstring for what each step asserts.
#
# # Reverse injection
#
# A check that greps for a string the fix itself introduced passes against any loader. Step 2
# deletes the single source of the key list and requires the check to fail.
#
# Usage: tools/check_json_event_route.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

echo "=== [1/2] the JSON event route agrees with the capability table ==="
if ! "$PYTHON" tools/check_json_event_route.py; then
    echo "FAIL: the JSON event route has drifted from the capability event table"
    exit 1
fi

echo ""
echo "=== [2/2] reverse injection: removing the single key source must fail the check ==="
BACKUP="$(mktemp)"
cp src/json/event_route.rs "$BACKUP"
# shellcheck disable=SC2064
trap "cp '$BACKUP' src/json/event_route.rs; rm -f '$BACKUP'" EXIT

# Blank every MARKER_KEYS entry: the table is then empty and the check must report it.
"$PYTHON" - <<'PY'
import pathlib
import re

path = pathlib.Path("src/json/event_route.rs")
text = path.read_text()
text = re.sub(r'\(\s*"on_[a-z_]+"\s*,\s*JsonTriggerMarker::\w+\s*\)', "", text)
path.write_text(text)
PY

if "$PYTHON" tools/check_json_event_route.py > id.1.diag 2>&1; then
    cat id.1.diag
    rm -f id.1.diag
    echo "FAIL: the marker table can be emptied without the gate noticing" >&2
    exit 1
fi
rm -f id.1.diag
echo "  injection detected as expected"

cp "$BACKUP" src/json/event_route.rs
if ! "$PYTHON" tools/check_json_event_route.py > /dev/null; then
    echo "FAIL: the gate does not pass again after the injection is reverted" >&2
    exit 1
fi

echo ""
echo "json event route checks passed."
