#!/usr/bin/env bash
# The generator must reuse the runtime's wire-compatibility rules, not restate them (T-23 DoD).
#
# # Why this gate exists
#
# T-23's completion criteria include:
#
# > 生成器复用 T-3 的类型兼容规则（**不是**另写一套）
#
# The violation is not a missing call — it is a **second table** that happens to agree today. A
# generator with its own arms agrees with `WIRE_RULES` until someone adds a `PropertyValueKind`
# variant, at which point the designer accepts a wire the generated program rejects. Nothing about
# that shows up in the generator's own tests.
#
# The check is `tools/check_generator_reuses_wire_rules.py`; see its docstring for each step.
#
# # Reverse injection
#
# Step 2 plants a local verdict in the generator and requires the gate to FAIL. Without it, step [1]
# of the check ("does it name the table?") passes against a generator that names it and ignores it —
# the decorative-reuse failure this gate is specifically for.
#
# Usage: tools/check_generator_reuses_wire_rules.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

echo "=== [1/2] the generator consumes the runtime's wire rules ==="
if ! "$PYTHON" tools/check_generator_reuses_wire_rules.py; then
    echo "FAIL: the generator has its own wire-compatibility rules"
    exit 1
fi

echo ""
echo "=== [2/2] reverse injection: a local verdict must fail the gate ==="
BACKUP="$(mktemp)"
cp src/designer/generator.rs "$BACKUP"
# shellcheck disable=SC2064
trap "cp '$BACKUP' src/designer/generator.rs; rm -f '$BACKUP'" EXIT

"$PYTHON" - "$ROOT_DIR/src/designer/generator.rs" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
text = path.read_text()
needle = "pub fn shared_wire_rule_count() -> usize {"
assert needle in text, "the injection point moved; update this script"
# A local verdict: exactly what a second rule set looks like from the outside.
text = text.replace(
    needle,
    "/// INJECTED: a second compatibility verdict.\n"
    "fn injected_local_verdict() -> rust_widgets::widget::capability::WireCompatibility {\n"
    "    WireCompatibility::Rejected(\"local\")\n"
    "}\n\n" + needle,
    1,
)
path.write_text(text)
PY

if "$PYTHON" tools/check_generator_reuses_wire_rules.py > id.1.diag 2>&1; then
    cat id.1.diag
    rm -f id.1.diag
    echo "FAIL: a second compatibility verdict does not fail the gate" >&2
    exit 1
fi
rm -f id.1.diag
echo "  injection detected as expected"

cp "$BACKUP" src/designer/generator.rs
if ! "$PYTHON" tools/check_generator_reuses_wire_rules.py > /dev/null; then
    echo "FAIL: the gate does not pass again after the injection is reverted" >&2
    exit 1
fi
rm -f "$BACKUP"
trap - EXIT

echo ""
echo "generator wire-rule reuse checks passed."
