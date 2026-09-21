#!/usr/bin/env python3
"""Find `theme ... .background_color` reads whose classifying role is `Surface`.

Defect class 3: `resolved_theme_style(X).background_color` resolves to
`theme.colors.background` when `WidgetRole::for_kind_name(X)` falls through to
`Surface` -- the *window's own fill*. Using that to colour an affordance (a filled
action button, a selected indicator, a primary/accent element, a focus ring, a
progress fill) paints it grey. The widget must read `primary` (action) or
`semantic_color(...)` (state) instead.

The same class covers the `Input`/`Choice`/`Accent`/`Surface` role families:

  * `Input`/`Choice` -> `input_background()`: a *field* colour, still wrong for an
    affordance.
  * `Accent`         -> `accent`: legitimate for a value indicator.
  * `Primary`        -> `primary`: legitimate.
"""
import re
import pathlib
import sys

SRC = pathlib.Path("src")
ROLE_TABLE = pathlib.Path("src/theme/types.rs")


def load_roles():
    """Parse the `"name" | "name" => Self::Role` arms out of the role table."""
    text = ROLE_TABLE.read_text()
    start = text.index("pub fn for_kind_name")
    end = text.index("fn ", start + 10)
    body = text[start:end]
    roles = {}
    for m in re.finditer(r"((?:\s*\|?\s*\"[a-z]+\"\s*)+)\s*=>\s*\{\s*(Self::\w+)", body):
        names = re.findall(r'"([a-z]+)"', m.group(1))
        for n in names:
            roles[n] = m.group(2).replace("Self::", "")
    for m in re.finditer(r"((?:\s*\|?\s*\"[a-z]+\"\s*)+)\s*=>\s*(Self::\w+)", body):
        names = re.findall(r'"([a-z]+)"', m.group(1))
        for n in names:
            roles.setdefault(n, m.group(2).replace("Self::", ""))
    return roles


ROLES = load_roles()

# `resolved_theme_style(<name>)` bound to a local, then `<local>.as_ref().and_then(...background_color)`
STYLE_BIND = re.compile(r'let\s+(\w+)\s*=\s*crate::style::resolved_theme_style\(\s*"([\w_]+)"\s*\)')
BG_READ = re.compile(r'\b(\w+)\s*\.\s*as_ref\(\)\s*\.\s*and_then\(\s*\|\w+\|\s*\w+\s*\.\s*background_color\s*\)')
THEME_BG = re.compile(r'\btheme\s*\.\s*as_ref\(\)\s*\.\s*and_then\(\s*\|\w+\|\s*\w+\s*\.\s*background_color\s*\)')


def scan(path):
    text = path.read_text()
    lines = text.split("\n")
    binds = {}
    for i, line in enumerate(lines):
        m = STYLE_BIND.search(line)
        if m:
            binds[m.group(1)] = (m.group(2), i + 1)
    hits = []
    for i, line in enumerate(lines):
        for m in BG_READ.finditer(line):
            local = m.group(1)
            if local in binds:
                name, bl = binds[local]
                role = ROLES.get(name, "Surface")
                hits.append((i + 1, name, role, local, line.strip()))
        if THEME_BG.search(line):
            # the local `theme` is only meaningful if the bind is in this file
            for local, (name, bl) in binds.items():
                role = ROLES.get(name, "Surface")
                hits.append((i + 1, name, role, "theme", line.strip()))
    return hits, binds


def main():
    only_bad = "-a" not in sys.argv
    total = 0
    for path in sorted(SRC.rglob("*.rs")):
        if "/tests" in str(path) or path.name == "tests.rs":
            continue
        hits, binds = scan(path)
        if not hits:
            continue
        rows = []
        for ln, name, role, local, txt in hits:
            if only_bad and role in ("Primary", "Accent"):
                continue
            rows.append((ln, name, role, local, txt))
        if rows:
            print(f"\n### {path}")
            for ln, name, role, local, txt in rows:
                flag = "  <== SURFACE (window fill!)" if role == "Surface" else ""
                print(f"  L{ln:<5} style('{name}') role={role:<8} via {local}{flag}")
                print(f"        {txt[:110]}")
            total += len(rows)
    print(f"\ntotal candidate reads: {total}")
    print("\nrole table parsed:", len(ROLES), "names")
    for n in sorted(ROLES):
        if ROLES[n] in ("Surface",):
            print("   Surface:", n)


if __name__ == "__main__":
    main()
