#!/usr/bin/env python3
"""Count how many controls share a `WidgetKind`, and how many a kind-only sweep misses.

# Why this exists

`WidgetKind` and the capability registry are not the same size. A specialised control
reuses its base kind (`split_button` and `tool_button` both declare `ToolButton`), so any
sweep that iterates kinds silently skips every control that shares one — and a skipped
control does not error, it just never appears in the report.

That is the failure mode BLUE20 §105 forbids: the sweep unit must be the control's
`canonical_name`, not its kind. This script prints the evidence for that rule, so the
number in the plan can be re-derived rather than trusted.

# Usage

    python3 tools/audit_kind_sharing.py

Exit status is 0 even when kinds are shared: sharing is legitimate, and this is a
measurement, not a gate. The gate that enforces the sweep unit is
`tools/check_control_rendering.sh`.
"""

import re
import sys
from collections import Counter, OrderedDict

PROPERTIES = "src/widget/capability/properties.rs"


def main() -> int:
    try:
        source = open(PROPERTIES, encoding="utf-8").read()
    except OSError as error:
        print(f"cannot read {PROPERTIES}: {error}", file=sys.stderr)
        return 2

    names = re.findall(r'canonical_name:\s*"([^"]+)"', source)
    kinds = re.findall(r"\bkind:\s*WidgetKind::([A-Za-z0-9_]+)", source)

    if len(names) != len(kinds):
        # A record whose `kind` or `canonical_name` is spelled differently would make the
        # pairing below silently wrong, which is worse than not running.
        print(
            f"refusing to pair: {len(names)} canonical_name vs {len(kinds)} kind fields "
            "— the record shape changed, update this script",
            file=sys.stderr,
        )
        return 2

    print(f"capability records (canonical_name): {len(names)}")
    print(f"kind occurrences: {len(kinds)}")
    print(f"distinct kinds: {len(set(kinds))}")

    by_kind: "OrderedDict[str, list]" = OrderedDict()
    for name, kind in zip(names, kinds):
        by_kind.setdefault(kind, []).append(name)

    shared = {kind: owners for kind, owners in by_kind.items() if len(owners) > 1}
    print(f"\nkinds shared by more than one control: {len(shared)}")
    for kind, owners in sorted(shared.items()):
        print(f"  {kind}: {owners}")

    missed = len(names) - len(by_kind)
    print(f"\ncontrols a kind-only sweep would miss: {missed}")

    duplicates = [name for name, count in Counter(names).items() if count > 1]
    if duplicates:
        print(f"\nWARNING duplicate canonical names: {duplicates}", file=sys.stderr)
        return 1

    return 0


if __name__ == "__main__":
    sys.exit(main())
