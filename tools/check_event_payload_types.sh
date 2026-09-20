#!/usr/bin/env bash
# Every declared event payload must equal the real Rust type of the signal behind it, and the
# gate must fail when that is not so.
#
# # Why this gate exists
#
# BLUE19 rule #95 gives each published event a payload kind so a designer can show "value: number"
# and refuse a wire that cannot be made. `src/widget/capability/event_payloads.rs` is generated
# from the signal declarations, but a generated table is only trustworthy if something other than
# its generator reads it: the generator and the table would otherwise agree by construction, and a
# derivation bug would appear on both sides.
#
# So the check is `tools/check_event_payload_types.py`, which re-derives every payload from the
# signals and compares against what the generated file actually declares. It is run twice:
#
#   1. clean — the table must match;
#   2. with `--inject=slider.value_changed`, which declares a payload the signal does not have —
#      the gate **must** fail. Step 2 is what makes step 1 mean something: a comparison that can
#      only print "ok" passes step 1 trivially.
#
# Usage: tools/check_event_payload_types.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

if ! "$PYTHON" tools/check_event_payload_types.py; then
    echo "FAIL: a declared event payload disagrees with its signal"
    exit 1
fi

# Reverse injection. `set -e` is deliberately suspended for this one command: a non-zero exit is
# the expected outcome, and it is the *assertion* rather than an error.
if "$PYTHON" tools/check_event_payload_types.py --inject=slider.value_changed >/dev/null 2>&1; then
    echo "FAIL: injecting a wrong payload did not make the check fail, so it is not checking"
    exit 1
fi

echo "event payload type checks passed."
