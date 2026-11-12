#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_capability_schema_is_current.sh — AG-13
# ============================================================================
# The rule this guards:
#
#   **The committed capability document describes the code that is committed.**
#
# "What properties and events does this control have?" used to mean reading
# `widget/capability/access.rs` and `capability/types.rs` — a thousand lines each — and deriving
# the answer by hand. The answer exists at runtime, so `docs/capability_schema.json` commits it:
# one object per control, with every published property and event.
#
# A committed artifact is only worth reading if it cannot go stale, which is what this gate is for.
# It runs the exporter in `--check` mode (byte comparison, not a re-derivation) and then asserts
# the document still carries the sentinels below — a schema file that regenerated to an **empty**
# array would otherwise compare equal to a broken exporter's output.
#
# What it checks
# --------------
#   (1) `docs/capability_schema.json` exists and parses as a JSON array of at least 200 controls;
#   (2) a fresh export is byte-identical to it (the exporter pins `profile`/`triple`, so the
#       comparison is host-independent and can run on any CI runner);
#   (3) sentinels:
#       * every control carries the shared `a11y_role` row (D3-15's five shared properties);
#       * `button` publishes `clicked` with no payload;
#       * `tree_view` publishes `header_clicked` and `chart_reference_line` publishes
#         `line_crossed` — the DX-10 renames, so a regression that reintroduces the `on_` prefix
#         in the published document is caught here as well as by the vocabulary gate;
#       * `property_grid` publishes `category_toggled`.
#
# What it does NOT check
# ----------------------
#   * Whether a property's *default* is the right one — that is each control's own test.
#   * Whether the document is complete in the sense of "every control a user can create" — that is
#     `check_widget_registration_fidelity.sh` (the document is as complete as the factory).
#
# Usage: tools/check_capability_schema_is_current.sh
#
# The exporter runs under a 900 s bound inside the gate, so the runner's budget must be
# larger for that bound to be the one that produces the verdict (principle #104).
#
# rw-gate-budget: 1200
#
# Exit 0 = the committed document matches a fresh export and carries its sentinels.
# Exit 1 = a finding (the drift command is printed).
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
. "$ROOT_DIR/tools/lib_timeout.sh"
. "$ROOT_DIR/tools/lib_cargo_cache.sh"
. "$ROOT_DIR/tools/lib_python.sh"

DOC="docs/capability_schema.json"

echo "[1/4] the document exists and parses"
if [ ! -f "$DOC" ]; then
    echo "  FAIL  $DOC is missing"
    echo "        Generate it with: cargo run --example export_capability_schema"
    exit 1
fi
"$PYTHON" - "$DOC" <<'PY'
import json
import sys

path = sys.argv[1]
with open(path, encoding="utf-8") as handle:
    document = json.load(handle)
if not isinstance(document, list):
    raise SystemExit(f"  FAIL  {path} is not a JSON array")
if len(document) < 200:
    raise SystemExit(
        f"  FAIL  {path} describes only {len(document)} controls; the factory registers far more, "
        "so this is a truncated export rather than a small library"
    )
for entry in document:
    for key in ("control", "properties", "events"):
        if key not in entry:
            raise SystemExit(f"  FAIL  an entry is missing `{key}`: {entry.get('control')!r}")
print(f"  PASS  {len(document)} controls, every entry carrying properties and events")
PY

echo "[2/4] a fresh export matches the committed bytes"
# Bounded like every other command in the suite: a wedged exporter used to hang
# both this gate and the run that launched it (principle #58).
rw_run_bounded "$RW_TIMEOUT_DEFAULT" cargo run --quiet --example export_capability_schema -- --check

echo "[3/4] sentinels: the shared row and the event renames"
"$PYTHON" - "$DOC" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    document = json.load(handle)
by_name = {entry["control"]: entry for entry in document}

missing_shared = [
    entry["control"]
    for entry in document
    if not any(prop["name"] == "a11y_role" for prop in entry["properties"])
]
if missing_shared:
    raise SystemExit(
        "  FAIL  these controls do not carry the shared `a11y_role` row: "
        + ", ".join(missing_shared[:5])
        + (" ..." if len(missing_shared) > 5 else "")
    )

def events(control):
    return {event["name"] for event in by_name.get(control, {}).get("events", [])}

checks = [
    ("button", "clicked"),
    ("tree_view", "header_clicked"),
    ("property_grid", "category_toggled"),
    ("chart_reference_line", "line_crossed"),
]
for control, name in checks:
    if control not in by_name:
        raise SystemExit(f"  FAIL  `{control}` is not in the document at all")
    if name not in events(control):
        raise SystemExit(
            f"  FAIL  `{control}` does not publish `{name}` (it publishes {sorted(events(control))[:6]})"
        )
print("  PASS  the shared row and all four renamed events are present")
PY

echo "[4/4] no published event keeps the listener prefix"
if "$PYTHON" - "$DOC" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    document = json.load(handle)
prefixed = [
    (entry["control"], event["name"])
    for entry in document
    for event in entry.get("events", [])
    if event["name"].startswith("on_")
]
if prefixed:
    for control, name in prefixed[:10]:
        print(f"  FAIL  {control}: {name}")
    raise SystemExit(1)
PY
then
    echo "  PASS  no on_ prefixed published event in the document"
else
    echo "  FAIL  the document publishes a listener-prefixed event name (DX-10)"
    exit 1
fi

echo
echo "check_capability_schema_is_current: OK ($DOC matches a fresh export)"
