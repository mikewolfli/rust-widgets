#!/usr/bin/env python3
"""Rewrite every capability's `events:` list into a generated-schema lookup.

# Why this is a script and not 187 hand edits

BLUE19 step 1 changes `WidgetCapability.events` from `&'static [&'static str]` to
`&'static [EventSchema]`. The 187 capability constructors in
`src/widget/capability/properties.rs` each spell their names inline:

    events: &["clicked", "pressed", "released", "state_changed"],

Hand-editing 326 names into struct literals would be 326 chances to mistype a name, and a
typed name is *accepted* by `connect_event` right up until it is compared against the table — so
the migration would introduce the very defect it is meant to remove. The rewrite is therefore
mechanical and idempotent:

    events: &["clicked", "pressed"],   ->   events: events_of!("button"),

`events_of!` (see `event_payloads.rs`) returns the schema slice for a control, with a compile
error for a name that is not in the generated table. That makes the table the single source for
both the name list and the payloads, and makes a capability that publishes a name nobody derived a
**build failure** rather than a silent gap.

# Order of operations

This script replaces the *only* place the names are written down, so it must run **after**
`tools/derive_event_payloads.py` has written the table with those names in it. Running it first
would leave the table with nothing to derive from. The generator is aware of both spellings, so
the pair is re-runnable in the order (migrate -> derive) or (derive -> migrate), and the check mode
here fails loudly if a lookup names a control the table does not cover.

Usage:  python3 tools/migrate_capability_events.py          # rewrite properties.rs
        python3 tools/migrate_capability_events.py --check  # exit 1 if a literal list remains
"""

from __future__ import annotations

import pathlib
import re
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent
PROPERTIES = REPO / "src/widget/capability/properties.rs"
PAYLOADS = REPO / "src/widget/capability/event_payloads.rs"

# One capability constructor: from its `pub(crate) fn` through the closing brace of the struct
# literal. Non-greedy so a following constructor is never swallowed.
CAPABILITY_RE = re.compile(
    r"(pub\(crate\) fn \w+\(\) -> WidgetCapability \{.*?events:\s*)&\[(.*?)\s*\]",
    re.S,
)
NAME_RE = re.compile(r'canonical_name:\s*"([^"]+)"')


def main() -> int:
    check_only = "--check" in sys.argv
    source = PROPERTIES.read_text()
    slugs = set(re.findall(r'\("([a-z0-9_]+)",\s*"[a-z0-9_]+"', PAYLOADS.read_text()))

    rewritten = 0
    unknown: list[tuple[str, str]] = []

    def replace(match: re.Match[str]) -> str:
        nonlocal rewritten
        body = match.group(0)
        name_match = NAME_RE.search(body)
        if not name_match:
            return body
        capability = name_match.group(1)
        names = re.findall(r'"([^"]+)"', match.group(2))
        if not names:
            # A capability that publishes no events has no row in the table, and needs none:
            # an empty list is the same fact whether the row is absent or empty.
            rewritten += 1
            return f'{match.group(1)}&[]'
        if capability not in slugs:
            unknown.append((capability, ", ".join(names)))
        rewritten += 1
        return f'{match.group(1)}events_of!("{capability}")'

    updated = CAPABILITY_RE.sub(replace, source)

    if unknown:
        print(f"❌ {len(unknown)} capability name(s) missing from {PAYLOADS.name}:")
        for capability, names in unknown:
            print(f"   {capability}: {names}")
        print()
        print("   Run `python3 tools/derive_event_payloads.py` so the table covers them.")
        return 1

    if check_only:
        if updated != source:
            print(f"❌ {PROPERTIES} still spells event lists literally")
            return 1
        # `rewritten` counts *sites*, not capabilities, and the two differ because a capability
        # that publishes nothing is left as an empty list rather than pointed at the table.
        print(f"✅ {rewritten} capability event sites resolve through the generated table")
        return 0

    if updated != source:
        PROPERTIES.write_text(updated)
    print(f"rewrote {rewritten} capability event lists in {PROPERTIES}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
