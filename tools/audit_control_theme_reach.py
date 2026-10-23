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

# Aliases sit between `canonical_name` and `properties` in every declaration (measured across the
# whole table), and a constructor can be named after an alias: `create_textarea` for `text_area`,
# `create_web_view` for `web_engine_view`. Without reading aliases those two controls can never
# resolve, and the scan reports them as unresolved noise every run. Some declarations carry a
# comment between the fields (e.g. `check_box`), so the alias list is read from a window after
# the anchor instead of requiring adjacency.
CAPABILITY_ANCHOR = re.compile(r'kind:\s*WidgetKind::(\w+),\s*canonical_name:\s*"([^"]+)"')
ALIAS_LIST = re.compile(r'aliases:\s*&\[([^\]]*)\]')
ALIAS_NAME = re.compile(r'"(\w+)"')
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
    r"FocusRing::|StateOverlay|"
    # Cross-file theme helpers: a control whose *own* file never names the theme can still be
    # themed through a shared resolver in a sibling module. `resolve_palette`/`wrapper_palette`
    # are layout_family's palette resolvers (flex/flow/wrap layouts draw through them), and
    # `for_active_appearance` is code_editor's SyntaxPalette constructor. Without these names the
    # scan reported five already-correct controls as theme-blind (measured: every one of them
    # renders differently across the dark/light snapshot pairs).
    r"resolve_palette|wrapper_palette|for_active_appearance"
)


# Constructors may spell the type differently from `X::new`: `Dialog::with_title` is the dialog
# constructor, and `create_data_view` delegates to `create_virtual_list` rather than naming any
# type at all. The type search therefore accepts `::with_*` and follows one delegation hop.
TYPE_CALL = re.compile(r"\b(\w+)::(?:new|with_\w+)\b")
DELEGATION = re.compile(r"\bcreate_(\w+)\(")


def build_index() -> tuple[dict[str, str], dict[str, list[tuple[str, bool]]], dict[str, str], dict[str, list[str]]]:
    properties = pathlib.Path(PROPERTIES).read_text(encoding="utf-8")
    canonical: dict[str, str] = {}
    aliases: dict[str, list[str]] = {}
    anchors = list(CAPABILITY_ANCHOR.finditer(properties))
    for index, anchor in enumerate(anchors):
        kind, name = anchor.group(1), anchor.group(2)
        canonical[name] = kind
        # One declaration's window ends where the next begins; aliases always fall inside it.
        window_end = anchors[index + 1].start() if index + 1 < len(anchors) else len(properties)
        window = properties[anchor.end():window_end]
        alias_match = ALIAS_LIST.search(window)
        names = [name] + (ALIAS_NAME.findall(alias_match.group(1)) if alias_match else [])
        aliases[name] = names

    sources = {p: open(p, encoding="utf-8", errors="ignore").read() for p in glob.glob("src/**/*.rs", recursive=True)}

    # struct name -> candidate files, with "this file has its `impl Draw`" sorting first. A name
    # can occur in more than one module (`Action` exists in both `menu_toolbar` and `action`),
    # and picking the wrong one reads the wrong theme-reach for the control.
    structs: dict[str, list[tuple[str, bool]]] = {}
    for path, text in sources.items():
        for match in STRUCT.finditer(text):
            name = match.group(1)
            structs.setdefault(name, []).append((path, f"impl Draw for {name}" in text))
    return canonical, structs, sources, aliases


def constructed_type(body: str, constructors: str, structs, depth: int = 0) -> str | None:
    """The struct a constructor body builds: `X::new`, `X::with_*`, or one delegation hop."""
    for type_match in TYPE_CALL.finditer(body):
        type_name = type_match.group(1)
        if type_name in structs:
            return type_name
    # A constructor that only forwards (`create_data_view` -> `create_virtual_list`) names no
    # type; follow the call it makes and read *that* constructor instead. One hop with a depth
    # bound keeps a delegation cycle from looping.
    if depth < 3:
        for delegate in DELEGATION.finditer(body):
            target = delegate.group(1)
            for match in CONSTRUCTOR.finditer(constructors):
                if match.group(1) != target:
                    continue
                resolved = constructed_type(match.group(2).replace("\\\n", " "), constructors, structs, depth + 1)
                if resolved is not None:
                    return resolved
    return None


def locate(name: str, aliases: list[str], constructors: str, structs):
    """The type `create_<name>` constructs, and the file that declares it.

    Searches every constructor whose name matches exactly, never by prefix:
    `create_chart` and `create_chart_widget` are different functions, and a prefix search
    would answer for the wrong one. The candidates are the canonical name, every declared
    alias, and the name with its underscores stripped (the `create_textarea` spelling).
    """
    candidates: list[str] = []
    for candidate in [name] + aliases + [name.replace("_", "")]:
        if candidate not in candidates:
            candidates.append(candidate)
    for candidate in candidates:
        for match in CONSTRUCTOR.finditer(constructors):
            if match.group(1) != candidate:
                continue
            type_name = constructed_type(match.group(2).replace("\\\n", " "), constructors, structs)
            if type_name is None:
                continue
            files = sorted(structs[type_name], key=lambda entry: not entry[1])
            return files[0][0], type_name
    return None, None


def main() -> int:
    theme_blind_only = "--theme-blind-only" in sys.argv
    canonical, structs, sources, aliases = build_index()
    constructors = sources.get(CONSTRUCTORS, "")

    rows = []
    for name in sorted(canonical):
        path, type_name = locate(name, aliases.get(name, [name]), constructors, structs)
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
