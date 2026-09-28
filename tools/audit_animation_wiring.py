#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Which controls already implement `Widget::tick` / `is_animating`?

# Why this exists

Appendix A carries rows of the form "M3: the animation has no driver" or "M3: add a tick".
`Widget::tick` and `Widget::is_animating` are on the trait with `false` defaults, so an
inherent `pub fn tick` is *not* the same as the trait method being overridden: the first is
unreachable through `&mut dyn Widget` (the defect the trait method was added to fix), the
second is drivable.

This tool reports, per control, which of the two shapes its implementation has:

  * `trait`      -- implements `Widget::tick`; the animation bus reaches it;
  * `inherent`   -- has `pub fn tick` but does not implement the trait method (still unreachable);
  * `none`       -- no animation at all.

That distinction is the whole point: BLUE23 section 3 exists because 11 controls had the
*inherent* shape and nothing called them.

Usage: python3 tools/audit_animation_wiring.py [--unwired-only]
"""

import glob
import pathlib
import re
import sys

PROPERTIES = "src/widget/capability/properties.rs"
CONSTRUCTORS = "src/widget/capability/constructors.rs"

KIND_AND_NAME = re.compile(r'kind:\s*WidgetKind::(\w+),\s*canonical_name:\s*"([^"]+)"')
STRUCT = re.compile(r"pub struct (\w+)")
CONSTRUCTOR = re.compile(r"pub fn create_(\w+)\b([\s\S]*?)(?=\n(?:pub fn |#\[|\}\s*$))", re.MULTILINE)


def main() -> int:
    unwired_only = "--unwired-only" in sys.argv
    properties = pathlib.Path(PROPERTIES).read_text(encoding="utf-8")
    canonical = {name: kind for kind, name in KIND_AND_NAME.findall(properties)}

    sources = {
        p: open(p, encoding="utf-8", errors="ignore").read()
        for p in glob.glob("src/**/*.rs", recursive=True)
    }
    constructors = sources.get(CONSTRUCTORS, "")

    structs: dict[str, list[tuple[str, bool]]] = {}
    for path, text in sources.items():
        for match in STRUCT.finditer(text):
            name = match.group(1)
            structs.setdefault(name, []).append((path, f"impl Draw for {name}" in text))

    rows = []
    for name in sorted(canonical):
        type_name = None
        for match in CONSTRUCTOR.finditer(constructors):
            if match.group(1) != name:
                continue
            body = match.group(2).replace("\\\n", " ")
            for tm in re.finditer(r"\b(\w+)::new\b", body):
                if tm.group(1) in structs:
                    type_name = tm.group(1)
                    break
            break
        if type_name is None:
            continue
        path = sorted(structs[type_name], key=lambda e: not e[1])[0][0]
        text = sources[path]

        # `impl Widget for X`'s body, up to the *next* top-level `impl`/`pub struct` rather than to the
        # first `\n}\n`. The first closing brace belongs to the first method, not to the impl block, so
        # a brace-anchored window reported `inplace_editor` and `tag_input` as having no trait `tick`
        # when both do (measured) — the same over- and under-read the theme-reach scan had.
        impl_match = re.search(
            r"impl Widget for " + re.escape(type_name) + r"\b([\s\S]*?)(?=\nimpl |\n#\[|\npub struct )",
            text,
        )
        widget_impl = impl_match.group(1) if impl_match else ""
        trait_tick = "fn tick" in widget_impl
        trait_animating = "fn is_animating" in widget_impl
        inherent_tick = bool(re.search(r"pub fn tick\(", text))

        # `trait` is decided by the **trait `tick`** alone: that is what the animation bus calls, and
        # it is what section 3 of the plan is about. `is_animating` is reported alongside because
        # `Widget::is_animating` has a `false` default, so a control that ticks but never answers it
        # keeps asking its host for frames it does not need — a smaller finding than an unreachable
        # animation, and a different one. Requiring both to call a control "wired" reported
        # `inplace_editor` and `tag_input` as unwired when both override the trait `tick` (measured).
        if trait_tick:
            shape = "trait" if trait_animating else "trait-tick-only"
        elif inherent_tick:
            shape = "inherent"
        else:
            shape = "none"
        rows.append((name, type_name, path, shape, inherent_tick))

    if unwired_only:
        rows = [r for r in rows if r[3] != "none"]

    print(f"=== controls with an animation shape worth checking: {len(rows)} ===")
    for name, type_name, path, shape, inherent in sorted(rows, key=lambda r: r[3]):
        print(f"    {name:26s} {shape:16s} inherent_tick={inherent!s:5s} {path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
