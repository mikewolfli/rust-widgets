#!/usr/bin/env python3
"""Add the published event names a capability's control emits but does not publish.

# Why this exists

`tools/check_capability_events_are_emitted.py` checks both directions. Its reverse direction —
*every name a control declares pub and emits must be published* — is what this repairs: a signal
that is public, emitted from production code and documented, but absent from the capability's
`events:` list, is rejected by `connect_event` with `UnknownCommand`, so the only route to it is
the Rust field accessor. For a designer that is a missing option in the event panel; for a
subscriber it is a name that cannot be subscribed to at all.

The names are supplied per capability, and the result is asserted by re-running the gate, so this
is a repair and not a list of names that looked plausible.

Usage:  python3 tools/publish_emitted_events.py          # apply the additions
        python3 tools/publish_emitted_events.py --check  # exit 1 if any addition is missing
"""

from __future__ import annotations

import pathlib
import re
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent
PROPERTIES = REPO / "src/widget/capability/properties.rs"

# capability -> names its control emits that were not published. Taken from the per-name audit
# recorded in `docs/log/log-20260920-2.md`, which lists the emit sites it verified in production
# code. Each name is appended to the capability's existing list, keeping the current order so the
# diff is an addition and nothing else.
ADDITIONS: dict[str, list[str]] = {
    "slider": ["slider_pressed", "slider_released"],
    "code_editor": ["tab_changed", "fold_changed", "search_changed", "completion_changed"],
    "file_dialog": ["current_changed"],
    "popup_window": ["opened", "closed"],
    "web_engine_view": [
        "page_created",
        "page_destroyed",
        "console_message",
        "download_requested",
        "certificate_error",
    ],
    "toggle_button": ["state_changed"],
    "chart": ["data_point_unhovered"],
    "empty_state": ["action_pressed"],
    "color_history": ["color_hovered"],
    "adaptive_scaffold": ["nav_selected"],
    "wizard_dialog": ["step_changed"],
    "rich_edit": ["selection_changed", "cursor_position_changed", "read_only_changed"],
    "search_bar": ["canceled"],
    "tool_button": ["triggered"],
}

# Reused verbatim from the derivation, so both tools agree on what a capability declaration is.
CAPABILITY_RE = re.compile(r"pub\(crate\) fn \w+\(\) -> WidgetCapability \{(.*?)\n\}", re.S)
NAME_RE = re.compile(r'canonical_name:\s*"([^"]+)"')
EVENTS_RE = re.compile(r"events:\s*&\[(.*?)\s*\]", re.S)
QUOTED_RE = re.compile(r'"([^"]+)"')


def main() -> int:
    check_only = "--check" in sys.argv
    source = PROPERTIES.read_text()
    applied: list[str] = []
    missing: list[str] = []

    def patch(match: re.Match[str]) -> str:
        body = match.group(0)
        name_match = NAME_RE.search(body)
        if not name_match:
            return body
        capability = name_match.group(1)
        additions = ADDITIONS.get(capability)
        if not additions:
            return body
        events_match = EVENTS_RE.search(body)
        if events_match is None:
            missing.append(f"{capability}: no `events:` list to extend")
            return body
        existing = QUOTED_RE.findall(events_match.group(1))
        to_add = [name for name in additions if name not in existing]
        if not to_add:
            return body
        missing.extend(f"{capability}: {name}" for name in to_add)
        applied.extend(f"{capability}: {name}" for name in to_add)
        merged = existing + to_add
        rendered = ", ".join(f'"{name}"' for name in merged)
        # rustfmt's list formatting, so the file is already formatted when the migration rewrites
        # these lines and no reformat shows up in the diff as unrelated churn.
        one_line = f"events: &[{rendered}]"
        replacement = (
            one_line
            if len(one_line) <= 100
            else "events: &[\n" + "".join(f'            "{name}",\n' for name in merged) + "        ]"
        )
        return body[: events_match.start()] + replacement + body[events_match.end() :]

    updated = CAPABILITY_RE.sub(patch, source)

    if check_only:
        if missing:
            print(f"❌ {len(missing)} emitted signal(s) are still unpublished:")
            for entry in missing:
                print(f"   {entry}")
            return 1
        print(f"✅ every emitted signal is published ({len(ADDITIONS)} capabilities checked)")
        return 0

    if updated != source:
        PROPERTIES.write_text(updated)
    print(f"published {len(applied)} previously unpublished signal(s)")
    for entry in applied:
        print(f"   {entry}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
