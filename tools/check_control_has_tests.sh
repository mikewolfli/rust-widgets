#!/usr/bin/env bash
# Every control must have at least one test that builds it.
#
# # Why this gate exists
#
# `Toast` shipped with zero tests of its own while every gate passed. It was
# registered in the factory, had a property contract, was reachable from JSON, CSS
# and the C ABI, and had a row in the capability matrix — and none of those gates
# asks the one question that matters for a control: *is there a test that exercises
# this control?* A control whose contract is correct but which nothing ever mounts
# satisfies all of them, because "the contract answers" is provable by reading the
# contract.
#
# The check is `tools/check_control_has_tests.py`; see its docstring for what counts
# as a test and for the two false answers an earlier version gave (it reported
# 179/179 while `Toast` was untested, because an enum-listing test mentions every
# kind).
#
# # Why the validator also runs bounded, and its output is checked
#
# The gate used to accept any zero exit. A zero exit that printed nothing cannot be
# told apart from a validator that never really ran, so the validator runs under
# `rw_run_bounded` and its result contract is asserted: on success it must print
# the `controls with a test that names them:` count line. A zero exit without that
# line is a FAIL.
#
# Usage: tools/check_control_has_tests.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

OUTPUT=""
if ! OUTPUT="$(rw_run_bounded 600 "$PYTHON" tools/check_control_has_tests.py)"; then
    printf '%s\n' "$OUTPUT"
    echo "FAIL: a control has no test that builds it"
    exit 1
fi

# The result contract: a passing run reports how many controls were tested.
if ! printf '%s\n' "$OUTPUT" | grep -q 'controls with a test that names them:'; then
    printf '%s\n' "$OUTPUT"
    echo "FAIL: check_control_has_tests.py exited 0 without its result summary"
    exit 1
fi

printf '%s\n' "$OUTPUT"
echo "control test coverage checks passed."
