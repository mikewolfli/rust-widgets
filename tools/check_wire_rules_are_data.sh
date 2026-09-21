#!/usr/bin/env bash
# The wire-compatibility rules must be shared data, not a `match` (BLUE19 T-3).
#
# # Why this gate exists
#
# `blue19.md` §3.3 requires the event→target compatibility rules to be 数据化. Two consumers need
# them: the runtime (validating a wire when a project loads) and the code generator T-23 (deciding
# at generation time whether to emit a direct assignment). A `match` inside the runtime is invisible
# to the generator, so the designer accepts a wire the generated program rejects.
#
# The check is `tools/check_wire_rules_are_data.py`; see its docstring for each step.
#
# # Reverse injection
#
# Step [3] of the check — "does `compatibility` walk the table?" — is the load-bearing one. Step 2
# below replaces the table walk with a hard-coded pair and requires the check to fail. Without it,
# a check that greps for a symbol the fix itself introduced passes against any implementation.
#
# Usage: tools/check_wire_rules_are_data.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

echo "=== [1/2] the wire rules are data both consumers can read ==="
if ! "$PYTHON" tools/check_wire_rules_are_data.py; then
    echo "FAIL: the wire-compatibility rules are not a shared data table"
    exit 1
fi

echo ""
echo "=== [2/2] reverse injection: bypassing the table must fail the check ==="
BACKUP="$(mktemp)"
cp src/widget/capability/wire_rules.rs "$BACKUP"
# shellcheck disable=SC2064
trap "cp '$BACKUP' src/widget/capability/wire_rules.rs; rm -f '$BACKUP'" EXIT

# Replace the table walk with a direct answer: the runtime would then behave identically while the
# generator lost its source of truth, which is precisely the drift this gate is for.
"$PYTHON" - <<'PY'
import pathlib
import re

path = pathlib.Path("src/widget/capability/wire_rules.rs")
text = path.read_text()
text = text.replace(
    "for candidate in WIRE_RULES {",
    "for candidate in WIRE_RULES.iter().take(0) {",
)
path.write_text(text)
PY

if "$PYTHON" tools/check_wire_rules_are_data.py > id.1.diag 2>&1; then
    cat id.1.diag
    rm -f id.1.diag
    echo "FAIL: the table can stop being consulted without the gate noticing" >&2
    exit 1
fi
rm -f id.1.diag
echo "  injection detected as expected"

cp "$BACKUP" src/widget/capability/wire_rules.rs
if ! "$PYTHON" tools/check_wire_rules_are_data.py > /dev/null; then
    echo "FAIL: the gate does not pass again after the injection is reverted" >&2
    exit 1
fi

echo ""
echo "wire rule checks passed."
