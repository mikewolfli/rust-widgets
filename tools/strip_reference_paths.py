#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Third pass: neutralize the read-only *checkout paths* used as citation addresses.

# Why

The documents record how each claim was checked, and the check was a read of a reference tree
checked out beside this repo (`/home/mikeli/workspace/flutter`, `/home/mikeli/workspace/qtdeclarative`,
and the project-local symlink `flutter_ref`). That is the right thing for a plan to record — an
unverifiable claim is worse than an uncited one — but a literal path into a product's source tree
is an address, and the user asked that those names go.

# What the replacement keeps

The *provenance*, not the address: a reader can still see that a claim was checked against an
external reference tree, and how to re-create that setup, without the document naming or linking a
product. The substitution is deliberately visible (`<reference-tree>/…`) rather than silent: a
reader must be able to tell that something was elided, and this file is that record.
"""

import collections
import pathlib
import re

TARGETS = sorted(pathlib.Path("docs/plans").glob("blue*.md")) + sorted(
    pathlib.Path("docs/log").glob("*.md")
)

SUBS = [
    # absolute checkout paths
    (re.compile(r"/home/mikeli/workspace/qtdeclarative"), "<reference-tree>/qtdeclarative"),
    (re.compile(r"/home/mikeli/workspace/flutter"), "<reference-tree>/material"),
    (re.compile(r"/home/mikeli/workspace/\w+"), "<reference-tree>"),
    # the project-local symlink used to reach it
    (re.compile(r"\bflutter_ref\b"), "the reference symlink"),
    # subpaths of the checkouts
    (re.compile(r"packages/flutter/lib/src/material/\*\.dart"), "<reference-tree>/material/*.dart"),
    (re.compile(r"packages/flutter/lib/src/material/"), "<reference-tree>/material/"),
    (re.compile(r"packages/flutter/lib/src/"), "<reference-tree>/material/"),
    (re.compile(r"packages/flutter/"), "<reference-tree>/material/"),
    (re.compile(r"src/quickcontrols/basic/"), "<reference-tree>/controls/"),
    (re.compile(r"src/quickcontrols/"), "<reference-tree>/controls/"),
    (re.compile(r"src/quicktemplates/"), "<reference-tree>/templates/"),
    # a couple of bare product-ish words left in prose
    (re.compile(r"\bqt-qml\b"), "the reference toolkit"),
    (re.compile(r"\bmatplotlib\b"), "a mainstream plotting library"),
]

# The package-name plural that the earlier passes left as a bare identifier in a table cell.
BARE = [
    (re.compile(r"\bflutter\b", re.I), "the material reference"),
]


def main() -> None:
    touched = collections.Counter()
    for path in TARGETS:
        original = path.read_text(encoding="utf-8")
        text = original
        for pattern, replacement in SUBS:
            text, n = pattern.subn(replacement, text)
            touched[path.name] += n
        for pattern, replacement in BARE:
            text, n = pattern.subn(replacement, text)
            touched[path.name] += n
        if text != original:
            path.write_text(text, encoding="utf-8")

    for name, count in sorted(touched.items(), key=lambda kv: -kv[1]):
        if count:
            print(f"{count:4d}  {name}")
    print(f"files touched: {sum(1 for v in touched.values() if v)}  "
          f"replacements: {sum(touched.values())}")


if __name__ == "__main__":
    main()
