#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Strip third-party toolkit *names* from the plan and log documents.

# Why

The user asked that names of other GUI toolkits be removed from our documents, to avoid
unnecessary legal exposure. The names are used in this corpus only as *citations* -- "this
crate's layout channel follows the reference toolkit's min/pref/max, see `qquickcontrol.cpp`" --
never as our own identifiers.

# What this must NOT do

A blanket find-and-replace would destroy the technical content: `qquickcontrol.cpp:2043`, for
instance, is not decoration, it is the evidence a claim rests on, and `Qt Quick's handlePress /
handleMove / handleRelease / handleUngrab` names a four-segment contract. So each pattern below
carries a replacement that *preserves the mechanism* and only drops the vendor name.

# Scope

`docs/plans/blue*.md` and `docs/log/*.md` only. `gtk-native`, `cocoa` and `objc2` are deliberately
NOT touched: they are this crate's own feature and module names (`Cargo.toml`,
`src/platform/{linux,macos,macos_objc2}/`), i.e. code identifiers rather than citations.
"""

import collections
import pathlib
import re

TARGETS = sorted(pathlib.Path("docs/plans").glob("blue*.md")) + sorted(
    pathlib.Path("docs/log").glob("*.md")
)

# Each entry is (pattern, replacement). The replacement states the mechanism the citation was
# carrying; a bare `the reference toolkit` would leave a sentence that no longer says anything.
SUBS = [
    # ── file:line citations of the reference checkouts: keep the coordinates, drop the product ──
    (re.compile(r"Qt Quick (\w+\.[a-z]+:\d[\w\-,\u2013.]*)"), r"the reference toolkit (\1)"),
    (re.compile(r"\bQML (\w+\.[a-z]+:\d[\w\-,\u2013.]*)"), r"the reference toolkit (\1)"),
    (re.compile(r"qquick(\w+)\.cpp:(\d[\w\u2013,]*)"), r"reference-toolkit \1.cpp:\2"),
    # ── possessives ──
    (re.compile(r"Qt Quick's"), "the reference toolkit's"),
    (re.compile(r"QML's"), "the reference markup set's"),
    (re.compile(r"Flutter's"), "a mainstream material toolkit's"),
    (re.compile(r"SwiftUI's"), "a mainstream declarative toolkit's"),
    (re.compile(r"\bQt's\b"), "the reference toolkit's"),
    # ── bare names ──
    (re.compile(r"\bQt Quick\b"), "the reference toolkit"),
    (re.compile(r"\bQML\b"), "the reference markup set"),
    (re.compile(r"\bFlutter\b"), "a mainstream material toolkit"),
    (re.compile(r"\bSwiftUI\b"), "a mainstream declarative toolkit"),
    (re.compile(r"\bMaterial 3\b"), "the Material palette spec"),
    (re.compile(r"\bMaterial Design\b"), "the Material design spec"),
    (re.compile(r"\bReact\b"), "a web UI framework"),
]


def main() -> None:
    changed = collections.Counter()
    for path in TARGETS:
        original = path.read_text(encoding="utf-8")
        text = original
        for pattern, replacement in SUBS:
            text, count = pattern.subn(replacement, text)
            if count:
                changed[path.name] += count
        if text != original:
            path.write_text(text, encoding="utf-8")

    for name, count in sorted(changed.items(), key=lambda kv: -kv[1]):
        print(f"{count:5d}  {name}")
    print(f"files touched: {len(changed)}  replacements: {sum(changed.values())}")


if __name__ == "__main__":
    main()
