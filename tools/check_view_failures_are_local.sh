#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_view_failures_are_local.sh — BLUE23 §5A.5（P1-15 错误边界，判据 14/31）
# ============================================================================
# The rule this guards (BLUE23 §5A.5):
#
#   **A failure applying one node must mark that node and leave its siblings alone.**
#
# # Why this is a gate, and why it is structural
#
# The defect it removes is precise: `engine.rs` used to drop the **whole document** when `build`
# or apply failed, so "the 137th control has a typo" and "the window cannot render at all" had
# exactly the same consequence. `ViewError` was a flat type with no node in it, so a caller could
# not even tell *which* node failed.
#
# A test on one input cannot establish the property, because the property is "the code cannot
# express whole-document failure" — an error path that exists for some other input would pass
# any number of green tests. So this checks the *structure* of the apply path:
#
#   1. a `Vec<ViewError>` sink exists, so failures are accumulated rather than returned;
#   2. the walk `errors.push(ViewError::…)`es and continues;
#   3. a placeholder builder exists, so a failed node renders as something visible (判据 13).
#
# The behavioural half is already covered by unit tests, which this gate does not duplicate:
#
#     $ cargo test --features desktop a_bad_child_does_not_remove_its_good_siblings   # 判据 11
#     $ cargo test --features desktop a_failure_carries_the_widget_name_key_and_ancestor_chain
#     $ cargo test --features desktop a_failure_yields_a_placeholder_that_names_it    # 判据 13
#
# # What this gate proves
#
#   * The engine still *has* local-failure machinery (the three structures above).
#
# # What this gate does NOT prove
#
#   * That the machinery is *used* on every failure path. That is what the unit tests are for, and
#     they are named above so a reader can run them rather than trusting this scan.
#   * That the placeholder is legible, or that the ancestor chain in `ViewError` is useful. Those
#     are human judgements about the drawing.
#
# # Reverse injection
#
# The scanner is run against a copy of the view module with the failure sink removed and with the
# placeholder accessor removed, and must report a finding for each. That is the "revert to
# `return Err(whole)`" mutation the plan names, mechanised. Both halves are injected separately so a
# check that only one of them feeds cannot pass this gate.
#
# Usage: tools/check_view_failures_are_local.sh
# Exit 0 = the locality machinery is intact.
# Exit 1 = a finding is printed above.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

OUT="$("$PYTHON" tools/view_failure_locality_scan.py)"
echo "$OUT"
case "$OUT" in
    *"failed=0"*) ;;
    *)
        echo ""
        echo "  A node failure is no longer local. Check src/view/ (apply.rs holds the report and"
        echo "  the placeholder accessor; engine.rs pushes into it). See BLUE23 §5A.5."
        exit 1
        ;;
esac

# ── Reverse injection: the two halves of the mutation ───────────────────────────────────────────
INJECT_DIR="$(mktemp -d)"
trap 'rm -rf "$INJECT_DIR"' EXIT
mkdir -p "$INJECT_DIR/view"
cp src/view/engine.rs "$INJECT_DIR/view/engine.rs"
cp src/view/apply.rs "$INJECT_DIR/view/apply.rs"

# (a) Remove the failure sink — the "return the first error out of the whole apply" shape.
#     The sink is renamed rather than deleted so the rest of the file still parses; what the scan
#     looks for is the `Vec<ViewError>` field, which a whole-document-failure engine would not have.
"$PYTHON" - "$INJECT_DIR/view/apply.rs" <<'PY'
import pathlib, re, sys
path = pathlib.Path(sys.argv[1])
text = path.read_text()
stripped = re.sub(r"errors\s*:\s*(crate::compat::)?Vec<(\.{1,2})?ViewError>", "errors: Vec<()>", text)
if stripped == text:
    raise SystemExit("injection-point-missing")
path.write_text(stripped)
PY
if "$PYTHON" tools/view_failure_locality_scan.py --src "$INJECT_DIR/view" | grep -q "failed=0"; then
    echo "FAIL: removing the ViewError sink did not fail the scan"
    exit 1
fi

# (b) Remove the placeholder accessor — the "failure is an invisible hole" shape.
cp src/view/engine.rs "$INJECT_DIR/view/engine.rs"
cp src/view/apply.rs "$INJECT_DIR/view/apply.rs"
"$PYTHON" - "$INJECT_DIR/view/apply.rs" <<'PY'
import pathlib, re, sys
path = pathlib.Path(sys.argv[1])
text = path.read_text()
# The name is derived rather than hard-coded so a rename does not defeat the injection.
match = re.search(r"fn\s+\w*placeholders?\s*\(", text, re.IGNORECASE)
if match is None:
    raise SystemExit("injection-point-missing")
path.write_text(text.replace(match.group(0), "fn withdrawn_placeholder_accessor(", 1))
PY
if "$PYTHON" tools/view_failure_locality_scan.py --src "$INJECT_DIR/view" | grep -q "failed=0"; then
    echo "FAIL: removing the placeholder accessor did not fail the scan"
    exit 1
fi

echo "view-failure locality checks passed."
