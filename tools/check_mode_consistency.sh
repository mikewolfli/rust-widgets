#!/usr/bin/env bash
# T-24: mode 1 and mode 2 must describe the same UI, and each must stay in its own profile's API.
#
# # Why a separate gate from the compile check
#
# `tools/check_generator_output_compiles.sh` proves the generated code *builds*. That is necessary and
# not sufficient: a generator that emitted a **smaller correct tree** would compile perfectly while
# losing a control the user drew. This gate proves the two modes describe the same thing, and the
# reverse injection below (dropping a child) is what makes the claim falsifiable.
#
# # What it runs
#
# `tests/mode_consistency_test.rs` — structure, property values, declared handlers, profile purity and
# the capacity bound.
#
# # Reverse injection
#
# Step 2 makes the generator omit the last child of every node and requires the gate to FAIL. BLUE19's
# DoD names this injection by hand: "让生成器丢一个控件 → 门禁必须 FAIL".
#
# Usage: tools/check_mode_consistency.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"
. "$ROOT_DIR/tools/lib_timeout.sh"

GATE_TIMEOUT="${GATE_TIMEOUT:-900}"

run_cases() {
    rw_run_bounded "$GATE_TIMEOUT" cargo test \
        --no-default-features --features desktop --test mode_consistency_test
}

echo "=== [1/2] both modes describe the same UI ==="
if ! run_cases; then
    echo ""
    echo "FAIL: mode 1 and mode 2 disagree about the tree, the properties or the handlers."
    exit 1
fi

echo ""
echo "=== [2/2] reverse injection: dropping a control must fail the gate ==="
BACKUP="$(mktemp)"
cp src/designer/generator.rs "$BACKUP"
# shellcheck disable=SC2064
trap "cp '$BACKUP' src/designer/generator.rs; rm -f '$BACKUP'" EXIT

"$PYTHON" - "$ROOT_DIR/src/designer/generator.rs" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
text = path.read_text()
needle = """    let next_depth = depth + 1;
    for (child_index, child) in node.children.iter().enumerate() {
        let Some(child_path) = project_child_path(project, node, child_index) else {"""
assert needle in text, "the injection point moved; update this script"
# Emit every child but the last: a smaller, still-compiling, still-plausible tree.
text = text.replace(
    needle,
    """    let next_depth = depth + 1;
    for (child_index, child) in node.children.iter().enumerate() {
        if child_index + 1 == node.children.len() { continue; }
        let Some(child_path) = project_child_path(project, node, child_index) else {""",
    1,
)
path.write_text(text)
PY

if run_cases > id.1.diag 2>&1; then
    cat id.1.diag
    rm -f id.1.diag
    echo "FAIL: the generator can drop a control without this gate noticing" >&2
    exit 1
fi
rm -f id.1.diag
echo "  injection detected as expected (the dropped control is reported)"

cp "$BACKUP" src/designer/generator.rs
if ! run_cases > /dev/null 2>&1; then
    echo "FAIL: the gate does not pass again after the injection is reverted" >&2
    exit 1
fi
rm -f "$BACKUP"
trap - EXIT

echo ""
echo "mode consistency checks passed."
