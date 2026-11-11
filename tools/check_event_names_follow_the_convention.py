#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# The rule this implements is stated in full in
# `tools/check_event_names_follow_the_convention.sh`, which is the only caller and the
# documentation of record.
#
# Short version: a *published* event name is what a document, a designer or a language
# binding writes. `on_` is a listener-method prefix in other toolkits (`on_click=`), not an
# event name; the crate had exactly one, `on_line_crossed`, which is how DX-10 found the
# vocabulary drifting by prefix rather than by concept. The census is the source of truth
# for published names, so the scan reads it and, as defence in depth, the generated table.

from __future__ import annotations

import pathlib
import re
import sys

CENSUS = pathlib.Path("tools/event_published_census.txt")
TABLE = pathlib.Path("src/widget/capability/event_payloads.rs")
SUITES = pathlib.Path("crates/rust-widgets-suites/src")

# A published name must not start with this prefix. The alias registry may still carry an
# `on_` spelling as an *alias* — that is the record of a legacy name, not a publication.
FORBIDDEN_PREFIX = "on_"

# The remaining prefixed names in the **suites** crate, frozen rather than hidden.
#
# This was an eight-entry backlog when the gate landed (the suites crate published
# `on_matches_changed`, `on_overview_drag`, `on_entry_activated`, `on_chapter_entered`,
# `on_chapter_selected`, `on_header_clicked`, `on_task_selected` and `on_window_requested`).
# All eight have since been migrated the way the root crate's three were: field renamed,
# `event_signal_dyn` keeping both spellings, `EventSchema` publishing the unprefixed name, and an
# `EVENT_ALIASES` row for the old one.
#
# The set must stay **empty**. A name added here would be a silent exemption, which is the shape
# this project treats as a gate that lies; the gate fails both when a published name appears that
# is not listed and when a listed name is no longer published.
SUITES_BACKLOG: set[str] = set()

CENSUS_ROW = re.compile(r"^([a-z0-9_]+):\s*(.*)$")
SCHEMA_NAME = re.compile(r'EventSchema\s*\{\s*name:\s*"([^"]+)"')


def suites_prefixed() -> set[str]:
    """Prefixed names in every suites `EventSchema`, i.e. the second crate's publications."""
    found: set[str] = set()
    if not SUITES.is_dir():
        return found
    for path in sorted(SUITES.rglob("capability.rs")):
        for name in SCHEMA_NAME.findall(path.read_text(encoding="utf-8")):
            if name.startswith(FORBIDDEN_PREFIX):
                found.add(name)
    return found


def census_names() -> list[tuple[str, str]]:
    if not CENSUS.is_file():
        raise SystemExit(f"{CENSUS}: missing — the published-name census must exist")
    rows: list[tuple[str, str]] = []
    for line in CENSUS.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        match = CENSUS_ROW.match(line)
        if not match:
            raise SystemExit(f"{CENSUS}: unparsable row: {line}")
        control = match.group(1)
        if not match.group(2).strip():
            continue
        for name in match.group(2).split(","):
            name = name.strip()
            if name:
                rows.append((control, name))
    if not rows:
        raise SystemExit(f"{CENSUS}: parsed no published names — an empty census measures nothing")
    return rows


def main() -> int:
    rows = census_names()
    offenders = [(control, name) for control, name in rows if name.startswith(FORBIDDEN_PREFIX)]

    schema_offenders: list[str] = []
    if TABLE.is_file():
        for name in SCHEMA_NAME.findall(TABLE.read_text(encoding="utf-8")):
            if name.startswith(FORBIDDEN_PREFIX):
                schema_offenders.append(name)

    suites = suites_prefixed()
    new_suites = sorted(suites - SUITES_BACKLOG)
    fixed_suites = sorted(SUITES_BACKLOG - suites)

    print(f"published event names checked: {len(rows)} (across {len({c for c, _ in rows})} controls)")
    print(f"suites publications with the prefix: {len(suites)} ({len(SUITES_BACKLOG)} on the backlog)")
    if not offenders and not schema_offenders and not new_suites and not fixed_suites:
        print(f"PASS  no published event name carries the `{FORBIDDEN_PREFIX}` listener prefix")
        print("PASS  the suites backlog is unchanged (rename one, remove its entry)")
        return 0

    if offenders:
        print("FAIL  these published event names carry the listener prefix:")
        for control, name in offenders:
            print(f"  {control}: {name}")
    if schema_offenders:
        print("FAIL  the generated payload table still publishes a prefixed name:")
        for name in sorted(set(schema_offenders)):
            print(f"  {name}")
    if new_suites:
        print("FAIL  the suites crate publishes a prefix that is not on the backlog:")
        for name in new_suites:
            print(f"  {name}")
    if fixed_suites:
        print("FAIL  the suites backlog lists names that are no longer published:")
        for name in fixed_suites:
            print(f"  {name}")
        print("  Remove them from SUITES_BACKLOG — a stale allowlist is a silent exemption.")
    print()
    print("Rename the signal so the published name describes the event (`line_crossed`, not")
    print("`on_line_crossed`), and record the old spelling in `EVENT_ALIASES` so a document")
    print("written against it still binds. `event_signal_dyn` keeps answering the old name.")
    return 1


if __name__ == "__main__":
    sys.exit(main())
