#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_web_engine_honest.sh — BLUE20 layer 5 (WebEngine 诚实降级，裁定 W1)
# ============================================================================
# The rule this guards:
#
#   **The library must not claim a rendering capability it does not have, and the
#   degradation it *does* have must be queryable.**
#
# Why the WebKit wrapper was deleted (ruling W1, 2026-09-21)
# ----------------------------------------------------------
# A real engine used to be reachable on Linux behind the `webkit-engine` feature:
# 76 lines of one-line forwards to `webkit2gtk`, one platform, and the `WebView` was
# **never added to a GTK container**. So no user could ever have seen a page through
# this library, while the docs and the feature list said the opposite. It was removed
# rather than completed because completing it meant 500–1500 lines per platform plus a
# hard dependency on system libraries, and because the JS half of "web support" never
# went through that trait at all (it runs on the pure-Rust `boa` engine).
#
# What this gate checks
# ---------------------
#   [1] the removed names have not returned (zero residue)
#   [2] the capability question exists and answers honestly
#   [3] the degradation is queryable from the widget, and the query is not hardcoded
#   [4] JavaScript evaluation still works (the capability that was *kept*)
#
# Step [3] matters most: the defect this layer fixes is not "the engine is missing",
# it is that whether an engine existed was **not exposed**, so a caller could not tell
# a simulated view from a rendered one. That is the same class as an event that is
# published but never emitted (BLUE19 #97).
#
# Exit 0 = honest. Exit 1 = a finding.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

# shellcheck source=tools/lib_source_scan.sh
. "$ROOT_DIR/tools/lib_source_scan.sh"
. "$ROOT_DIR/tools/lib_timeout.sh"
. "$ROOT_DIR/tools/lib_cargo_cache.sh"
. "$ROOT_DIR/tools/lib_python.sh"

STEP_BUDGET="${RW_GATE_TIMEOUT:-900}"

echo "[1/4] zero residue: the removed WebKit wrapper has not come back"
# The names are matched as *identifiers*, not as prose. The module docs deliberately
# name them to explain the removal, and a gate that forbade the words would forbid the
# explanation. What must not exist is a live use: a `use` of the trait, a `mod`
# declaration, a feature, or a dependency.
RESIDUE="$("$PYTHON" - <<'PY'
import pathlib
import re

PATTERNS = [
    # A live item, not a mention in a comment or doc string.
    re.compile(r"^\s*pub\s+trait\s+NativeWebEngine", re.M),
    re.compile(r"^\s*(pub\(crate\)\s+)?mod\s+webkit_engine", re.M),
    # The *platform* engine factory. `create_web_engine_view` is a different thing and is
    # retained: it constructs the `web_engine_view` **control** (the widget that models
    # navigation), which is what §3.5.4 lists under "must keep". Matching the prefix would
    # forbid the control along with the engine.
    re.compile(r"^\s*fn\s+create_web_engine\s*\(", re.M),
    re.compile(r"^\s*use\s+.*NativeWebEngine", re.M),
    re.compile(r'^\s*webkit2gtk\s*=', re.M),
    re.compile(r'^\s*webkit-engine\s*=', re.M),
    re.compile(r'^\s*"webkit-engine",', re.M),
]

hits = []
for root in (pathlib.Path("src"), pathlib.Path("crates/rust-widgets-suites/src/webview")):
    for path in sorted(root.rglob("*.rs")):
        text = path.read_text(encoding="utf-8", errors="replace")
        for pattern in PATTERNS:
            for match in pattern.finditer(text):
                line = text.count("\n", 0, match.start()) + 1
                hits.append(f"{path}:{line}: {match.group(0).strip()}")
cargo = pathlib.Path("Cargo.toml")
if cargo.exists():
    text = cargo.read_text(encoding="utf-8", errors="replace")
    for pattern in PATTERNS:
        for match in pattern.finditer(text):
            line = text.count("\n", 0, match.start()) + 1
            hits.append(f"Cargo.toml:{line}: {match.group(0).strip()}")

if hits:
    print("\n".join(hits))
PY
)"
if [[ -n "$RESIDUE" ]]; then
    echo "  FAIL  the removed native web engine is referenced again:"
    printf '%s\n' "$RESIDUE"
    echo "  The deletion was ruling W1 (2026-09-21); see docs/plans/archive/blue20.md 3.5.4."
    exit 1
fi
echo "  PASS  no live reference to the removed wrapper"

echo "[2/4] the capability question exists and is not hardcoded true"
# The declaration is located by its *shape* — an anchored `fn supports_web_engine`
# at the start of a code line — not by the token appearing anywhere in the file.
# The old judgement took the first line between any mention of the name and the
# first four-space `    }`, so a doc comment mentioning the function, or a comment
# containing the word `false`, could stand in for the returned value (W6-35).
DEFAULT_FN_LINE="$(rw_function_line src/platform/types/traits.rs supports_web_engine || true)"
if [ -z "$DEFAULT_FN_LINE" ]; then
    echo "  FAIL  Platform::supports_web_engine is missing"
    echo "        It is the one question a caller can ask before claiming a rendering engine."
    exit 1
fi
# The default must be the `false` *literal*: the function body is sliced by brace
# balance and read with comments removed, so `true // false` is `true`, and a
# multi-expression override is not silently reduced to its first line.
DEFAULT_BODY="$(rw_slice_function_body src/platform/types/traits.rs "$DEFAULT_FN_LINE" || true)"
DEFAULT_ANSWER="$(printf '%s\n' "$DEFAULT_BODY" | rw_strip_line_comments \
    | sed -n '2,$p' \
    | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//' \
    | grep -vE '^$|^\{$|^\}$' || true)"
if [ "$DEFAULT_ANSWER" != "false" ] && [ "$DEFAULT_ANSWER" != "false;" ]; then
    echo "  FAIL  the default body does not answer 'false'; got: ${DEFAULT_ANSWER:-<empty>}"
    echo "        A backend that declares nothing declares no engine. The answer is read as"
    echo "        the body's own expression, with comments stripped."
    exit 1
fi
echo "  PASS  supports_web_engine() exists and defaults to false"

echo "[3/4] the widget's degradation is queryable and agrees with the platform"
if ! grep -q "pub fn has_real_engine" crates/rust-widgets-suites/src/webview/web_engine_enhanced.rs; then
    echo "  FAIL  WebEngineViewEnhanced::has_real_engine is missing"
    exit 1
fi
if ! rw_cargo_cached "$STEP_BUDGET" test \
    --manifest-path crates/rust-widgets-suites/Cargo.toml --features webview --lib \
    'webview::web_engine_enhanced::tests::test_has_real_engine_agrees_with_the_platform' \
    > /tmp/rw_web_honest.log 2>&1; then
    echo "  FAIL  the widget's answer disagrees with the platform's"
    sed -n '1,60p' /tmp/rw_web_honest.log
    exit 1
fi
if ! rw_cargo_cached "$STEP_BUDGET" test \
    --manifest-path crates/rust-widgets-suites/Cargo.toml --features webview --lib \
    'webview::web_engine_enhanced::tests::test_no_backend_renders_the_web_today' \
    >> /tmp/rw_web_honest.log 2>&1; then
    echo "  FAIL  a backend claims to render the web while the docs say none does"
    sed -n '1,60p' /tmp/rw_web_honest.log
    exit 1
fi
# Reverse-injection check: the query must actually consult the platform. A hardcoded
# `true`/`false` in the widget would satisfy the equality test only by coincidence, so the
# *whole* body is sliced (brace-balanced) and searched with comments removed. The old
# `grep -A 3` looked at three lines, which is neither the function's body nor an anchor.
HAS_REAL_ENGINE_LINE="$(rw_function_line crates/rust-widgets-suites/src/webview/web_engine_enhanced.rs has_real_engine || true)"
PLATFORM_CALL=""
if [ -n "$HAS_REAL_ENGINE_LINE" ]; then
    PLATFORM_CALL="$(rw_slice_function_body crates/rust-widgets-suites/src/webview/web_engine_enhanced.rs "$HAS_REAL_ENGINE_LINE" \
        | rw_strip_line_comments | grep -E 'supports_web_engine\(' || true)"
fi
if [ -z "$PLATFORM_CALL" ]; then
    echo "  FAIL  has_real_engine does not ask the platform, so it is a hardcoded answer"
    exit 1
fi
echo "  PASS  has_real_engine() asks the platform and agrees with it"

echo "[4/4] the retained capability (JavaScript) still works"
if ! rw_cargo_cached "$STEP_BUDGET" test \
    --manifest-path crates/rust-widgets-suites/Cargo.toml --features webview --lib \
    'webview::js_engine' \
    > /tmp/rw_web_js.log 2>&1; then
    echo "  FAIL  the JS engine regressed"
    sed -n '1,80p' /tmp/rw_web_js.log
    exit 1
fi
grep -E '^test result' /tmp/rw_web_js.log || true

echo ""
echo "check_web_engine_honest: checked=4 skipped=0 failed=0 (no rendering engine claimed; JS retained)"
