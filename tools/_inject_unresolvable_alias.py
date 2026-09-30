#!/usr/bin/env python3
"""Reverse-injection helper for `check_generator_agrees_with_registry.sh`.

Makes the alias `"btn"` unresolvable, which is exactly the defect class the gate covers: a document
that spells a control by an alias the registry no longer answers for would generate a program that
builds nothing for that node. A gate that cannot fail on this is not defending anything.

The file is restored by the caller (it keeps a backup); this script only mutates in place.

Usage: _inject_unresolvable_alias.py <path-to-properties.rs>
"""

from __future__ import annotations

import pathlib
import sys

# The alias list for `button_capability`, which is where `"btn"` lives.
NEEDLE = 'aliases: &["pushbutton", "btn"],'
REPLACEMENT = 'aliases: &["pushbutton"],'


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: _inject_unresolvable_alias.py <properties.rs>", file=sys.stderr)
        return 2
    path = pathlib.Path(sys.argv[1])
    text = path.read_text(encoding="utf-8")
    if NEEDLE not in text:
        print(
            f"error: the injection point moved; expected to find:\n  {NEEDLE}",
            file=sys.stderr,
        )
        return 2
    path.write_text(text.replace(NEEDLE, REPLACEMENT, 1), encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
