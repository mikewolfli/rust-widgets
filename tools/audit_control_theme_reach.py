#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Measure each canonical control's implementation against the appendix-A defects.

# Why this exists

Appendix A of `blue23.md` is a 204-row table and most rows are written as a *prediction*
("cascader does not read style", "meter's ticks are 90 degrees off"). Reading 150 rows by eye
does not scale, and doing it from memory produces exactly the defect the log keeps recording:
a fix aimed at code that was already correct (see the log's sections on the audit regex
counting 14 real findings as 66).

This tool asks the machine the part the machine can answer — *does this control's
implementation touch the theme at all, and how many colour literals does it carry* — and
prints one row per canonical control. The judgement ("is this literal a defect or a no-theme
fallback") stays with the reader, because only the source says which.

# What "touches the theme" means

A file is counted as themed when it reaches any of the crate's theme entry points:

  * `style()` / `style.<role>_color` — the control's resolved style;
  * `theme_manager()` / `resolved_theme_style` — the active theme's roles;
  * `colors.<role>` on a `Theme`;
  * the shared helpers that already do the above for their callers (`axis_chrome`,
    `layer_color`, `semantic_color`); and
  * the derivations every themed control uses (`contrast_color`, `legible_on`), plus
    `FocusRing` / `StateOverlay`, which are theme-keyed by construction.

The helper names matter: an earlier version of this scan looked only at the `fn draw` body
and reported 29 controls as theme-blind, of which 26 read the theme through `axis_chrome()`
in a helper. Reporting a control that is already correct is not a harmless extra row — it is
the failure mode this repo has paid for twice.

Usage: python3 tools/audit_control_theme_reach.py [--theme-blind-only]
"""

import glob
import pathlib
import re
import sys

PROPERTIES = "src/widget/capability/properties.rs"
CONSTRUCTORS = "src/widget/capability/constructors.rs"

KIND_AND_NAME = re.compile(r'kind:\s*WidgetKind::(\w+),\s*canonical_name:\s*"([^"]+)"')
STRUCT = re.compile(r"pub struct (\w+)")
# The constructor search is bounded by the **next** `pub fn` rather than by a brace count: the
# bodies contain nested blocks, and a `[\s\S]{0,800}` window overran into the following function,
# which made every control resolve to whatever the next one constructed (`button.rs` for nearly
# all of them — measured). Bounding on the next declaration keeps the window inside one function
# without needing a brace matcher.
CONSTRUCTOR = re.compile(r"pub fn create_(\w+)\b([\s\S]*?)(?=\n(?:pub fn |#\[|\}\s*$))", re.MULTILINE)
LITERAL = re.compile(r"Color::rgb\(|Color::rgba\(")

THEME_REACH = re.compile(
    r"\.style\(\)|theme_manager\(\)|resolved_theme_style|"
    r"colors\.(?:background|foreground|primary|secondary|surface_container|outline|error|scrim)|"
    r"axis_chrome|layer_color|semantic_color|contrast_color\(\)|legible_on\(|"
    r"FocusRing::|StateOverlay"
)


def build_index() -> tuple[dict[str, str], dict[str, list[tuple[str, bool]]], dict[str, str]]:
    properties = pathlib.Path(PROPERTIES).read_text(encoding="utf-8")
    canonical = {name: kind for kind, name in KIND_AND_NAME.findall(properties)}

    sources = {p: open(p, encoding="utf-8", errors="ignore").read() for p in glob.glob("src/**/*.rs", recursive=True)}

    # struct name -> candidate files, with "this file has its `impl Draw`" sorting first. A name
    # can occur in more than one module (`Action` exists in both `menu_toolbar` and `action`),
    # and picking the wrong one reads the wrong theme-reach for the control.
    structs: dict[str, list[tuple[str, bool]]] = {}
    for path, text in sources.items():
        for match in STRUCT.finditer(text):
            name = match.group(1)
            structs.setdefault(name, []).append((path, f"impl Draw for {name}" in text))
    return canonical, structs, sources


def locate(name: str, constructors: str, structs: dict[str, list[tuple[str, bool]]]):
    """The type `create_<name>` constructs, and the file that declares it.

    Searches every constructor whose name is exactly `create_<name>`, not a prefix match:
    `create_chart` and `create_chart_widget` are different functions, and a prefix search
    would answer for the wrong one.
    """
    for match in CONSTRUCTOR.finditer(constructors):
        if match.group(1) != name:
            continue
        body = match.group(2).replace("\\\n", " ")
        for type_match in re.finditer(r"\b(\w+)::new\b", body):
            type_name = type_match.group(1)
            if type_name in structs:
                files = sorted(structs[type_name], key=lambda entry: not entry[1])
                return files[0][0], type_name
    return None, None


def main() -> int:
    theme_blind_only = "--theme-blind-only" in sys.argv
    canonical, structs, sources = build_index()
    constructors = sources.get(CONSTRUCTORS, "")

    rows = []
    for name in sorted(canonical):
        path, type_name = locate(name, constructors, structs)
        if path is None:
            rows.append((name, canonical[name], None, None, 0, False))
            continue
        text = sources[path]
        rows.append(
            (
                name,
                canonical[name],
                path,
                type_name,
                len(LITERAL.findall(text)),
                bool(THEME_REACH.search(text)),
            )
        )

    unresolved = [row for row in rows if row[2] is None]
    blind = [row for row in rows if row[2] is not None and not row[5]]

    print(f"=== canonical controls: {len(canonical)} ===")
    print(f"=== constructors not resolved to a struct: {len(unresolved)} ===")
    for row in unresolved:
        print(f"    {row[0]:26s} kind={row[1]}")

    print(f"=== implementations that reach no theme entry point: {len(blind)} ===")
    for row in blind:
        print(f"    {row[0]:26s} lits={row[4]:3d}  {row[2]}")

    if not theme_blind_only:
        print("=== all rows (name / literals / themed) ===")
        for row in sorted(rows, key=lambda r: (r[5], -r[4])):
            if row[2] is None:
                continue
            print(f"    {row[0]:26s} {row[4]:3d}  {'themed' if row[5] else 'NO-THEME':9s} {row[2]}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
