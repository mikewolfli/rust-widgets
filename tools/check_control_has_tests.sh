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
# Usage: tools/check_control_has_tests.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

if ! "$PYTHON" tools/check_control_has_tests.py; then
    echo "FAIL: a control has no test that builds it"
    exit 1
fi

echo "control test coverage checks passed."
