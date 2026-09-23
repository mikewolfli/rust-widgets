#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""BLUE23 §5.5 gate: every declared colour role has a production consumer.

The defect this guards has happened repeatedly: a role is added to `Colors`, written into
every preset, documented -- and **no control reads it**. From the theme file it looks
supported; on screen nothing changes, because nothing consumes it. Seven layering roles
(`scrim`, `surface_container*`, `inverse_surface`, `on_inverse_surface`,
`outline_variant`) sat in that state at once, and `outline` had exactly one consumer.

This is the *token* level of `check_mechanism_has_a_consumer` (which is source level). It
reads the `Colors` struct's fields and requires each one to be read somewhere in `src/`
**outside** the theme module, or to be listed in `ALLOWED` with the reason it is reserved.

Run with `--inject` to prove the search reads the tree: the injection adds a field to
`Colors` and the scan must report it as unconsumed.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
COLORS_STRUCT = ROOT / "src" / "theme" / "types.rs"
SRC = ROOT / "src"

# Fields that are intentionally declared without a *control* consumer, each with the reason.
# This list is the ratchet: a new role must either be consumed or be named here, so an
# unused token can never again be added silently.
ALLOWED: dict[str, str] = {
    "background": "the window/page fill; read by every control's role fallback path",
    "foreground": "the default ink; read through role resolution, not by name",
    "secondary": "the neutral mid tone; consumed as the outline family's base",
}

# The file that *declares* the struct is not a consumer of its own fields.
DECLARATION_FILES = {
    str((ROOT / "src" / "theme" / "types.rs").relative_to(ROOT)),
}


def colors_fields() -> list[str]:
    text = COLORS_STRUCT.read_text(encoding="utf-8")
    start = text.index("pub struct Colors")
    # The struct body ends at the first `}` at column 0 after the fields.
    end = text.index("\n}", start)
    body = text[start:end]
    return re.findall(r"pub\s+(\w+)\s*:\s*Color\b", body)


def scan(extra_field: str | None) -> tuple[list[str], int]:
    """Returns (unconsumed fields, files scanned)."""
    fields = colors_fields()
    if extra_field:
        fields = fields + [extra_field]

    # Collect every `src/` line outside the declaration file, as one corpus. A field is
    # "consumed" if its name appears in a path expression (`.field`) anywhere in it.
    corpus_lines: list[str] = []
    files = 0
    for path in sorted(SRC.rglob("*.rs")):
        rel = str(path.relative_to(ROOT))
        if rel in DECLARATION_FILES:
            continue
        files += 1
        # Strip trailing test modules: a fixture reading a field proves the field compiles,
        # not that production code consumes it.
        text = path.read_text(encoding="utf-8")
        marker = text.find("\n#[cfg(test)]")
        corpus_lines.append(text[:marker] if marker != -1 else text)
    corpus = "\n".join(corpus_lines)

    unconsumed = [
        field
        for field in fields
        if field not in ALLOWED and not re.search(rf"\.{re.escape(field)}\b", corpus)
    ]
    return unconsumed, files


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--inject",
        action="store_true",
        help="pretend a new, unconsumed colour role was declared",
    )
    args = parser.parse_args(argv)

    injected = "definitely_unconsumed_role" if args.inject else None
    unconsumed, files = scan(injected)

    if files == 0:
        print("FAIL: no consumer sources found; the scan is not reading the tree")
        return 1

    if args.inject:
        # A visible injection exits non-zero so the wrapper sees the change.
        if "definitely_unconsumed_role" in unconsumed:
            return 1
        print("FAIL: --inject added an unconsumed role and the scan did not report it")
        return 0

    if unconsumed:
        print("FAIL: these `Colors` roles are declared but never read by production code:")
        for field in unconsumed:
            print(f"  colors.{field}")
        print()
        print("  Either consume it (a control reads it) or add it to ALLOWED in")
        print("  tools/check_declared_tokens_have_consumers.py with the reason it is reserved.")
        return 1

    print(f"declared-tokens scan: checked={len(colors_fields())} roles, failed=0")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
