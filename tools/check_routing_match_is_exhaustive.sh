#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_routing_match_is_exhaustive.sh — BLUE23 §0A.6 ITEM 0（编译期路由闭合的后半条）
# ============================================================================
# The rule this guards (FUTURE.md ITEM 0, as carried by BLUE23 §0A.6):
#
#   **Adding a `WidgetKind` without a routing decision must fail the build.**
#
# # Why this is a gate and why it checks for an *absence*
#
# `src/control_backend/routing.rs` encodes that rule as an exhaustive `match` with no wildcard
# arm, and the compiler enforces it: a new variant makes the match non-exhaustive and the test
# build stops compiling. That mechanism is real, and it was verified by mutation:
#
#     $ （把某条 arm 删掉）
#     error[E0004]: non-exhaustive patterns: `kind::WidgetKind::DropZone` not covered
#
# The problem is that it is **one character from being destroyed**. Appending
#
#     _ => ControlRoutePreference::CustomRequired,
#
# is the natural thing to write when the compiler complains about a new variant, and it makes the
# match compile while covering nothing. Measured on this repository:
#
#     $ （追加 catch-all arm）
#     $ cargo test --features desktop control_backend::routing
#     test result: ok. 2 passed                     # 全绿
#     $ bash tools/check_control_route_matrix.sh    # 运行时矩阵
#     Control route matrix generated …              # 仍然 0 missing
#
# Nothing caught it. That is why this gate asserts the **absence of a wildcard arm** rather than
# the presence of a match: a gate that merely checked "the function exists" would be satisfied by
# the very mutation it is here to prevent.
#
# # What this gate proves
#
#   * `route_is_library_painted` exists and still holds a `match`.
#   * That match has no `_ =>` arm, so the compiler's exhaustiveness check still bites.
#
# # What this gate does NOT prove
#
#   * That the match is exhaustive *today* — that is the compiler's job, and `cargo test` does it.
#     This gate protects the mechanism, not the answer.
#   * That each arm's *value* is right. Every arm is `CustomRequired` by policy; whether that
#     policy is correct is a design question, stated in the function's own docs.
#
# # Reverse injection
#
# The scanner is run against a copy of the file with a wildcard arm appended, and must report a
# finding. That is the mutation above, mechanised.
#
# Usage: tools/check_routing_match_is_exhaustive.sh
# Exit 0 = the compile-time guarantee is intact.
# Exit 1 = a finding is printed above.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

ROUTING="src/control_backend/routing.rs"
OUT="$("$PYTHON" tools/routing_match_scan.py --routing "$ROUTING")"
echo "$OUT"
case "$OUT" in
    *"findings=0"*) ;;
    *)
        echo ""
        echo "  The routing decision no longer fails the build for an unrouted WidgetKind."
        echo "  Remove the wildcard arm from $ROUTING so the compiler checks exhaustiveness."
        exit 1
        ;;
esac

# ── Reverse injection: the mutation this gate exists to catch ───────────────────────────────────
INJECT_DIR="$(mktemp -d)"
trap 'rm -rf "$INJECT_DIR"' EXIT
python3 - "$ROUTING" "$INJECT_DIR/injected.rs" <<'PY'
import pathlib, sys
source, destination = sys.argv[1], sys.argv[2]
text = pathlib.Path(source).read_text()
needle = "            | WidgetKind::DropZone => ControlRoutePreference::CustomRequired,\n        }"
if needle not in text:
    print("injection-point-missing")
    sys.exit(3)
text = text.replace(
    needle,
    "            | WidgetKind::DropZone => ControlRoutePreference::CustomRequired,\n"
    "            _ => ControlRoutePreference::CustomRequired,\n        }",
)
pathlib.Path(destination).write_text(text)
PY
if "$PYTHON" tools/routing_match_scan.py --routing "$INJECT_DIR/injected.rs" \
        | grep -q "findings=0"; then
    echo "FAIL: appending a wildcard arm did not fail the scan, so the gate cannot see the mutation"
    exit 1
fi

echo "routing-match checks passed."
