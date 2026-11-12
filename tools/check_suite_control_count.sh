#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
# ============================================================================
# check_suite_control_count.sh — the suites' control counts must match the code
# ============================================================================
# T3-03/T3-52: the maps group registered **8** controls (it gained
# `map_marker_detail`) while `crates/rust-widgets-suites/README.md` and both root
# READMEs still said 7/39. `check_widget_kind_count.sh` could not see the drift:
# it guards the parent crate's `WidgetKind` total and never reads the suites'
# per-group table.
#
# The source of truth is each group's install table, whose array length is a
# compile-time fact: `let rows: [GroupRow; N] = [`. This gate parses N, the
# suites README's per-group `Controls` column, its stated total, and the root
# READMEs' total.
#
# Reverse injection: `--inject=form` pretends the form install table gained a row while
# the documents stay put, so the gate must report a count finding; `--inject=<unknown>`
# must name a group this gate does not scan.
#
# Usage: tools/check_suite_control_count.sh [--inject=<group>]
# Exit 0 = every count agrees. Exit 1 = a document disagrees. Exit 2 = unknown group.
# ============================================================================
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
. "$ROOT_DIR/tools/lib_python.sh"

# The check is a function so the reverse-injection self-check below can run the
# exact same code path with an injected count instead of a second copy of it.
run_check() {
  "$PYTHON" - "$@" <<'PY'
import re
import sys
from pathlib import Path

# `--inject=<group>` pretends that group's install table gained a row while every
# document stayed put. That is the drift this gate exists for, so the self-check
# can require the gate to go red without editing the tree.
injected_group = None
for arg in sys.argv[1:]:
    if arg.startswith("--inject="):
        injected_group = arg.split("=", 1)[1]
    else:
        print(f"unknown argument: {arg}", file=sys.stderr)
        sys.exit(2)

GROUPS = {
    "form": "crates/rust-widgets-suites/src/form_widgets/capability.rs",
    "webview": "crates/rust-widgets-suites/src/webview/capability.rs",
    "finance": "crates/rust-widgets-suites/src/finance/capability.rs",
    "maps": "crates/rust-widgets-suites/src/maps/capability.rs",
    "project": "crates/rust-widgets-suites/src/project/capability.rs",
    "devtools": "crates/rust-widgets-suites/src/devtools/capability.rs",
    "media-playback": "crates/rust-widgets-suites/src/media_playback/capability.rs",
}

counts = {}
for group, path in GROUPS.items():
    source = Path(path).read_text(encoding="utf-8")
    match = re.search(r"let rows:\s*\[\s*[^\]]*;\s*(\d+)\s*\]\s*=", source)
    if not match:
        print(f"could not read the install table length from {path}", file=sys.stderr)
        sys.exit(2)
    counts[group] = int(match.group(1))

if injected_group is not None:
    if injected_group not in counts:
        print(f"--inject={injected_group}: not a scanned group", file=sys.stderr)
        sys.exit(2)
    counts[injected_group] += 1

actual_total = sum(counts.values())
errors = 0

# --- [1] the suites README's per-group table ---
suites = Path("crates/rust-widgets-suites/README.md").read_text(encoding="utf-8")
for group, count in counts.items():
    row = re.search(rf"^\|\s*`{re.escape(group)}`[^|\n]*\|[^|\n]*\|\s*(\d+)\s*\|", suites, re.M)
    if not row:
        print(f"FAIL suites README has no row for group `{group}`")
        errors += 1
        continue
    stated = int(row.group(1))
    if stated != count:
        print(f"FAIL suites README says {stated} for `{group}`, install registers {count}")
        errors += 1

total_claim = re.search(r"\*\*(\d+)\s+controls in \d+\s+groups\*\*", suites)
if not total_claim:
    print("FAIL suites README no longer states '**N controls in G groups**'")
    errors += 1
elif int(total_claim.group(1)) != actual_total:
    print(f"FAIL suites README total says {total_claim.group(1)}, install registers {actual_total}")
    errors += 1

# --- [2] the two root READMEs' total ---
for name in ("README.md", "README.zh-CN.md"):
    text = Path(name).read_text(encoding="utf-8")
    hit = re.search(r"\*\*(\d+)\s+(?:more controls in \d+ groups|个控件)\*\*", text)
    if not hit:
        print(f"FAIL {name} no longer states the suites control total")
        errors += 1
        continue
    stated = int(hit.group(1))
    if stated != actual_total:
        print(f"FAIL {name} suites total says {stated}, install registers {actual_total}")
        errors += 1

if errors:
    print(f"suite control count: FAILED ({errors} finding(s))")
    sys.exit(1)

print(f"suite control count: PASS ({actual_total} controls in {len(counts)} groups)")
PY
}

run_check "$@"

# Reverse injection (principle #106): a count gate that cannot be made to fail is
# indistinguishable from one that always passes. `--inject=form` pretends the form
# install table gained a control while the READMEs stayed at their current numbers,
# which is exactly the drift that left `map_marker_detail` unregistered in prose.
injection_output="$(run_check --inject=form 2>&1 || true)"
if ! printf '%s' "$injection_output" | grep -q "suite control count: FAILED"; then
    echo "FAIL: --inject=form did not produce a count finding:" >&2
    printf '%s\n' "$injection_output" >&2
    exit 1
fi

# An injection naming a group this gate does not scan must be reported as such,
# not silently ignored (which would make the self-check vacuous after a rename).
injection_output="$(run_check --inject=no_such_group 2>&1 || true)"
if ! printf '%s' "$injection_output" | grep -q "not a scanned group"; then
    echo "FAIL: an injection naming a group that is not scanned was not reported" >&2
    exit 1
fi

echo "suite control count reverse-injection checks passed."
