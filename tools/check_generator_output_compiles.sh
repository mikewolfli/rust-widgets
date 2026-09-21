#!/usr/bin/env bash
# T-23: the generator's output must COMPILE on every target profile it claims to support.
#
# # Why this is a compile, not a text match
#
# BLUE19's DoD for T-23 says the stripped template's output must compile for real under
# `--no-default-features --features mini` and `--features embedded` — "**不是**「应该没问题」".
# The reason is that the failure modes are *profile-specific*:
#
#   * `crate::view` / `crate::json` / `widget::runtime` are `cfg`-gated out of a stripped build;
#   * 118 `create_*` functions carry `cfg(not(alloc_frugal))`;
#   * the standard prelude is absent under `alloc_frugal`, so `String`/`&str` behave differently;
#   * a text-bearing constructor takes `(text, geometry)` while a value control takes `(geometry)`.
#
# Every one of those compiles **fine on the desktop host**. A generator with any of them looks
# correct until the target is built, so a text assertion or a host-only test would pass while the
# delivered program does not build. This gate is the only check that can find them.
#
# It found each of the four defects above during T-23, one per run.
#
# # What it runs
#
# `tests/generator_output_compiles_test.rs`, whose cases are `#[ignore]`d so the normal test run stays
# fast — each case shells out to `cargo check` on a throwaway crate.
#
# # Reverse injection
#
# Step 2 plants a `crate::view::Node` reference in the *stripped* template and requires the gate to
# FAIL. Without it, a check that compared nothing would pass on any generator.
#
# Usage: tools/check_generator_output_compiles.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"
. "$ROOT_DIR/tools/lib_timeout.sh"

# Each case compiles this library plus a small probe crate. Five compiles, so the budget has to be
# generous — but it is still a **bound**, per rule #58: a gate that can hang is worse than one that
# fails.
GATE_TIMEOUT="${GATE_TIMEOUT:-2400}"

run_cases() {
    rw_run_bounded "$GATE_TIMEOUT" cargo test \
        --no-default-features --features desktop \
        --test generator_output_compiles_test -- --ignored --nocapture --test-threads=1
}

echo "=== [1/2] the generated programs compile on every target ==="
if ! run_cases; then
    echo ""
    echo "FAIL: a generated program does not compile on its target profile."
    echo "      The compiler error above names the symbol the generator should not have emitted."
    exit 1
fi

echo ""
echo "=== [2/2] reverse injection: a cross-profile symbol must fail the gate ==="
BACKUP="$(mktemp)"
cp src/designer/generator.rs "$BACKUP"
# shellcheck disable=SC2064
trap "cp '$BACKUP' src/designer/generator.rs; rm -f '$BACKUP'" EXIT

# # Why the injection runs ONE case, not all four
#
# The first version re-ran every case to prove the injection was caught. That was circular: case [1]
# had just run unchanged and passed, so re-running it could only re-confirm the same result while
# paying a full compile again — it was 185s of the gate's 265s and proved nothing the first run had
# not already established.
#
# What the injection actually has to show is that the **stripped** case goes red when the stripped
# output names something the target lacks. `--exact` on that one case is enough: if the check has any
# teeth, that is where they are.
RUN_CASE='--exact stripped_template_output_compiles_for_mini'

"$PYTHON" - "$ROOT_DIR/src/designer/generator.rs" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
text = path.read_text()
needle = 'source.push_str("use rust_widgets::widget::Widget;\\n");'
assert needle in text, "the injection point moved; update this script"
# The stripped import block gains a `crate::view` name, which `mini`/`embedded` do not compile.
text = text.replace(
    needle,
    'source.push_str("use rust_widgets::view::Node;\\nuse rust_widgets::widget::Widget;\\n");',
    1,
)
path.write_text(text)
PY

if rw_run_bounded "$GATE_TIMEOUT" cargo test \
    --no-default-features --features desktop \
    --test generator_output_compiles_test -- --ignored --nocapture --test-threads=1 $RUN_CASE \
    > id.1.diag 2>&1; then
    cat id.1.diag
    rm -f id.1.diag
    echo "FAIL: a cross-profile symbol in the stripped output does not fail the gate" >&2
    exit 1
fi
rm -f id.1.diag
echo "  injection detected as expected (the target refuses the symbol)"

cp "$BACKUP" src/designer/generator.rs
if ! rw_run_bounded "$GATE_TIMEOUT" cargo test \
    --no-default-features --features desktop \
    --test generator_output_compiles_test -- --ignored --test-threads=1 $RUN_CASE > /dev/null 2>&1; then
    echo "FAIL: the gate does not pass again after the injection is reverted" >&2
    exit 1
fi
rm -f "$BACKUP"
trap - EXIT

echo ""
echo "generated-output compile checks passed."
