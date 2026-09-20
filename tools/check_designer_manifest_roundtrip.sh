#!/usr/bin/env bash
# The designer's capability document must survive export -> load -> export unchanged.
#
# # Why this gate exists
#
# A designer saves a project as one of these documents and reopens it later. If writing it twice can
# produce different bytes then every save is a spurious diff, and information can be lost between
# the two passes because nothing compares them. «The two are equal» is also satisfied by two empty
# documents, so the check additionally requires each control's own event names — and, for an event
# that carries a value, its payload token — to be present in the output.
#
# # Why it runs the test binary rather than a script
#
# The round trip has two halves, and only one of them is text: `DesignerManifest::from_json` is the
# independent parser, and the export walks the live capability table (186 published names across
# 187 controls) with each property's default value read from the property API. A script can compare
# strings it is given; it cannot drive the factory. So the assertions live in
# `tests/designer_manifest_roundtrip_test.rs` and this gate runs them.
#
# # What it does *not* re-check
#
# The payload *types* in the document are not re-derived here — that is
# `tools/check_event_payload_types.sh`'s job, and duplicating it would give two answers to the same
# question. This gate is about the round trip and the sentinels.
#
# Usage: tools/check_designer_manifest_roundtrip.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_timeout.sh"

# The round trip is a library test, not a script, because it drives the capability factory — see
# the header. `rw_run_bounded` gives it the same timeout discipline every other gate has.
LOG="$(mktemp)"
trap 'rm -f "$LOG"' EXIT

if ! rw_run_bounded "${RW_GATE_TIMEOUT:-1800}" cargo test --no-default-features --features desktop \
    --test designer_manifest_roundtrip_test >"$LOG" 2>&1; then
    echo "FAIL: the designer manifest does not round-trip"
    # The failing assertions are the whole value of this gate, so the log is printed rather than
    # summarized: the names of the controls that failed are only in the test output.
    cat "$LOG"
    exit 1
fi

grep -E '^test result: ' "$LOG" || true
echo "designer manifest round-trip checks passed."
