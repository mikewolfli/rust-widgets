#!/usr/bin/env bash
# Cookbook documentation gate.
#
# The cookbook is the project's user-facing API reference, but nothing compiles
# it: a renamed or deleted item leaves the prose confidently wrong (principle
# #18). This gate closes that hole from two directions:
#
#   [1] NAMES     every `pub fn|struct|enum|trait|const|type` declared in an
#                 `api-reference.md` must exist somewhere in `src/`, unless it is
#                 explicitly allowlisted as an illustrative example. This is what
#                 caught 13 declarations naming APIs that did not exist, including
#                 a whole removed module (`crate::chart`).
#   [2] BUILD     all three books must build. A book that does not build cannot be
#                 read at all, so "the docs are correct" is unverifiable until
#                 this passes.
#
# Usage: tools/check_cookbook.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

fail=0

echo "[1/2] API names: cookbook declarations must exist in src/"
if ! "$PYTHON" tools/check_cookbook_api_names.py --strict; then
    echo "FAIL: cookbook declares names that do not exist in src/"
    fail=1
fi

echo
echo "[2/2] Build all three books (warnings are failures)"
if ! command -v mdbook >/dev/null 2>&1; then
    echo "HOST-GATED: mdbook is not installed; skipping the build checks."
    echo "  install with: cargo install mdbook"
    echo "  (This is an environment gap, not a documentation pass — the name check above still ran.)"
else
    for book in en zh-CN zh-TW; do
        out=$(cd "cookbook/$book" && mdbook build 2>&1)
        if echo "$out" | grep -qE "^(ERROR|error)"; then
            echo "FAIL: cookbook/$book does not build:"
            echo "$out" | grep -E "^(ERROR|error|Caused|  )" | head -10
            fail=1
        elif echo "$out" | grep -q "WARN"; then
            # A warning here is a rendering defect (an unclosed tag, a broken link
            # target) that silently degrades the published page, so it fails too.
            echo "FAIL: cookbook/$book built with warnings:"
            echo "$out" | grep -A2 "WARN" | head -10
            fail=1
        else
            echo "  cookbook/$book: OK"
        fi
    done
fi

echo
if [ "$fail" -ne 0 ]; then
    echo "Cookbook checks FAILED."
    exit 1
fi
echo "Cookbook checks passed."
