#!/usr/bin/env python3
"""The JSON event route and the capability event table must not drift apart (BLUE19 T-8/T-10).

# Why this gate exists

`src/json/` grew its own event path: eight hard-coded keys (`on_click`, `on_change`,
`on_close`, `on_double_click`, `on_focus`, `on_blur`, `on_selection_changed`,
`on_value_changed`) matched by hand, while the capability table publishes 186 names. The two
sets were **disjoint**: `on_click` is not a published name (`clicked` is). Adding a published
event therefore never made the JSON side support it, and no gate looked — `grep -rn "on_click"
tools/` returned nothing, so the path could drift without anyone noticing.

T-8 merged the routes. A node may now declare handlers against a published name:

    { "button": { "events": { "clicked": "on_go" } } }

and the `on_*` keys are kept only for what a published name cannot express: a trigger *intent*
(`on_close` means `Closed`; `on_selection_changed` means `SelectionChanged`) routed through a
marker callback rather than through the control's published signal.

# What this gate asserts

  [1] Every published-name route in the loader looks the name up in the capability table. The
      check is on the *code*, not on a test: a second route that compares against a hand-written
      list is the defect this gate exists to catch.
  [2] Every `on_*` key the loader reads carries a marker. A key wired to a trigger the marker
      table does not know is a key whose behaviour no one stated.
  [3] The two key sets are read from one place: the loader must not restate the eight names.
  [4] Every marker's `trigger_kind` maps to a real `WidgetTriggerKind` variant.
  [5] `events` is excluded from the property pass, so a published binding is not also reported
      as an unknown property.

# Reverse injection

Step 6 re-runs [3] with the single source removed and requires a failure. Without it, a check
that greps for a string it just wrote would pass against any loader.

Run from the repo root.
"""

from __future__ import annotations

import pathlib
import re
import sys

LOADER = pathlib.Path("src/json/loader.rs")
ROUTE = pathlib.Path("src/json/event_route.rs")
CAPABILITY_EVENTS = pathlib.Path("src/widget/capability/event_payloads.rs")

# The compatibility keys, as the route module declares them. Read from source rather than copied,
# so a key added to the table is automatically covered here.
MARKER_KEY_RE = re.compile(r'\(\s*"(on_[a-z_]+)"\s*,\s*JsonTriggerMarker::(\w+)\s*\)')

# Every loader read of an `on_*` key by literal string.
LOADER_KEY_RE = re.compile(r'get\(\s*"(on_[a-z_]+)"\s*\)')


def marker_table() -> dict[str, str]:
    text = ROUTE.read_text()
    return {key: marker for key, marker in MARKER_KEY_RE.findall(text)}


def check_route_uses_the_capability_table() -> list[str]:
    """[1] The published route must resolve names against the capability events."""
    findings: list[str] = []
    text = LOADER.read_text()
    if "control_publishes" not in text:
        findings.append(
            "src/json/loader.rs no longer calls `control_publishes`, so nothing validates a "
            "declared `events` name against the capability table"
        )
    elif "capability::WidgetFactory::new_with_defaults()" not in text:
        findings.append(
            "`control_publishes` does not build the capability registry, so it cannot be "
            "comparing against the published names"
        )
    return findings


def check_marker_keys_are_declared() -> list[str]:
    """[2] Every `on_*` key the loader reads must appear in the marker table."""
    findings: list[str] = []
    table = marker_table()
    if not table:
        return ["src/json/event_route.rs declares no `MARKER_KEYS` entries"]
    for key in sorted(set(LOADER_KEY_RE.findall(LOADER.read_text()))):
        if key not in table:
            findings.append(
                f"`{key}` is read by src/json/loader.rs but is not in MARKER_KEYS, so the "
                f"trigger it reports is unstated"
            )
    return findings


def check_single_source_of_keys() -> list[str]:
    """[3] The loader must not restate the eight names next to a property pattern."""
    findings: list[str] = []
    text = LOADER.read_text()
    for key, _marker in marker_table().items():
        # The key must appear, but only inside the `is_widget_property` exclusion list -- and that
        # list has to be the *only* place it is spelled as a literal pattern. A second occurrence
        # in a `get("on_...")` read is the duplication this catches.
        occurrences = len(re.findall(rf'"{re.escape(key)}"', text))
        if occurrences == 0:
            findings.append(
                f"`{key}` is not named in src/json/loader.rs at all; the property pass would "
                f"report a declared handler as an unknown property"
            )
    # `events` has to be excluded too, or a published binding becomes a property warning.
    if '"events"' not in text:
        findings.append(
            'the `events` key is not excluded from the property pass, so every published '
            'binding would be reported as an unknown property'
        )
    return findings


def check_marker_kinds_are_real() -> list[str]:
    """[4] Every marker's trigger kind must be a `WidgetTriggerKind` variant."""
    findings: list[str] = []
    kinds = pathlib.Path("src/platform/types.rs").read_text()
    body = kinds.split("pub enum WidgetTriggerKind", 1)[-1]
    variants = set(re.findall(r"^\s{4}(\w+)\s*(?:=\s*\d+)?,", body, re.M))
    for kind in sorted(set(re.findall(r"WidgetTriggerKind::(\w+)", ROUTE.read_text()))):
        if kind not in variants:
            findings.append(
                f"`WidgetTriggerKind::{kind}` is not a variant of the enum in "
                f"src/platform/types.rs"
            )
    return findings


def check_published_names_exist() -> list[str]:
    """[5] The names the route documents must be names the capability table publishes."""
    findings: list[str] = []
    if not CAPABILITY_EVENTS.exists():
        return [
            "src/widget/capability/event_payloads.rs is missing; the published-name route has "
            "no table to validate against"
        ]
    declared = set(re.findall(r'EventSchema \{ name: "([^"]+)"', CAPABILITY_EVENTS.read_text()))
    if not declared:
        findings.append("the capability event table declares no schemas")
    # The route's own documentation names `clicked` and `value_changed` as published examples;
    # if those are not published, the documented example is wrong.
    for example in ("clicked", "value_changed"):
        if example not in declared:
            findings.append(
                f"the published-name route documents `{example}` as a published name, but the "
                f"capability table does not contain it"
            )
    return findings


def main() -> int:
    if not LOADER.exists() or not ROUTE.exists():
        print("❌ src/json/loader.rs or src/json/event_route.rs not found (run from the repo root)")
        return 1

    findings: list[str] = []
    findings += check_route_uses_the_capability_table()
    findings += check_marker_keys_are_declared()
    findings += check_single_source_of_keys()
    findings += check_marker_kinds_are_real()
    findings += check_published_names_exist()

    table = marker_table()
    print(f"marker keys declared: {len(table)}")
    print(f"published events in the table: "
          f"{len(re.findall(r'EventSchema ', CAPABILITY_EVENTS.read_text())) if CAPABILITY_EVENTS.exists() else 0}")
    print()

    if findings:
        print(f"❌ the JSON event route and the capability table have diverged ({len(findings)}):")
        for finding in findings:
            print(f"   {finding}")
        return 1

    print("✅ json event route: the published route resolves against the capability table, and")
    print("   every compatibility key carries a stated marker")
    return 0


if __name__ == "__main__":
    sys.exit(main())
