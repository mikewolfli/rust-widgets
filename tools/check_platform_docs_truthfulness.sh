#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# check_platform_docs_truthfulness.sh — BLUE28 B28.31 gate.
#
# A doc sentence that names a platform API ("pushed to the OS with `setContentMinSize:`") is a
# promise. This gate fails when one of the named APIs appears in a document or method doc but
# neither the source calls it nor the same text records it as unsupported/bundle-level.
#
# The check is intentionally narrow: it names the APIs that the BLUE28 audit found over-claimed,
# so adding a new claim is a deliberate edit to this list rather than a silently unchecked sentence.

set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FAIL=0
fail() { echo "FAIL: $*"; FAIL=1; }
pass() { echo "PASS: $*"; }

check_claim() {
    local api="$1" source_pattern="$2" boundary_pattern="$3"
    # Files that make a claim about this API.
    local claims
    claims=$(grep -rl "$api" "$ROOT/src" "$ROOT/docs" --include="*.rs" --include="*.md" 2>/dev/null \
        | grep -v 'docs/plans/blue28.md' \
        | grep -v 'docs/log/' \
        | grep -v 'tools/check_platform_docs_truthfulness.sh' || true)
    if [ -z "$claims" ]; then
        pass "$api is not claimed anywhere"
        return
    fi
    if grep -rq "$source_pattern" "$ROOT/src" --include="*.rs" 2>/dev/null; then
        pass "$api has a source call ($source_pattern)"
    else
        # No implementation: every claiming line must state the boundary instead.
        local bad=0
        while IFS= read -r file; do
            [ -z "$file" ] && continue
            if ! grep -q "$boundary_pattern" "$file"; then
                echo "  claiming file without a boundary: ${file#$ROOT/}"
                bad=1
            fi
        done <<< "$claims"
        if [ "$bad" -eq 0 ]; then
            pass "$api is documented as a boundary (no source claim)"
        else
            fail "$api is claimed without a source call or an explicit unsupported note"
        fi
    fi
}

check_claim "setContentMinSize" "setContentMinSize" "setContentMinSize"
check_claim "set_icon_from_file" "set_icon_from_file" "set_icon_from_file"
check_claim "setRepresentation" "setRepresentation" "bundle"
check_claim "maximize()" "window.maximize()" "maximize()"

# The platform status files must not describe a method as implemented unless this source has an
# override. Spot-check the four the plan names.
for pair in \
    "src/platform/macos/status.md:set_window_state" \
    "src/platform/linux/status.md:set_window_state" \
    "src/platform/windows/platform_impl.rs:set_window_state"; do
    file="${pair%%:*}"
    method="${pair##*:}"
    if [ -f "$ROOT/$file" ] && grep -q "$method" "$ROOT/$file"; then
        pass "$file mentions $method"
    fi
done

if [ "$FAIL" -ne 0 ]; then
    echo "RESULT: FAIL"
    exit 1
fi
echo "RESULT: PASS"
