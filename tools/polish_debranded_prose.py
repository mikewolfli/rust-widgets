#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Fourth pass: make the de-branded prose read naturally.

# Why this pass exists

Passes 1-3 removed the names mechanically. The result is technically clean but reads badly:
`a mainstream material toolkit/a mainstream declarative toolkit 全部是声明式 + 保留式`,
`the Material design spec 3`, `a mainstream material toolkit/the Material palette spec`. A
document that swaps `Flutter` for a nine-word noun phrase every time is *harder* to review, and a
plan nobody reads has failed at the only job a plan has.

# The rule this pass applies

Prefer the shortest honest phrase. The documents' own vocabulary already has one for "the external
implementations these numbers were checked against": **外部对标 / 参考实现**. So the long noun
phrases collapse to that where they are enumerating sources, and stay long only where the sentence
is making a *distinction* between two kinds of reference.

This pass is deliberately conservative: it only rewrites exact phrases left by passes 1-3, so it
cannot touch prose that was already fine.
"""

import collections
import pathlib
import re

TARGETS = sorted(pathlib.Path("docs/plans").glob("blue*.md")) + sorted(
    pathlib.Path("docs/log").glob("*.md")
)

# Order matters: longest first, so a phrase is not partially consumed by a shorter rule.
SUBS = [
    # ── enumerated source lists: collapse to one phrase ──
    (
        re.compile(
            r"a mainstream material toolkit\s*(?:/|、|,|，)\s*"
            r"(?:the reference toolkit|Qt\(the reference markup set\)|the reference markup set)\s*"
            r"(?:/|、|,|，)\s*a mainstream declarative toolkit"
        ),
        "外部对标（三家）",
    ),
    (
        re.compile(
            r"a mainstream material toolkit\s*(?:/|、|,|，)\s*the Material palette spec"
        ),
        "外部对标",
    ),
    (
        re.compile(r"the Material design spec\s*3"), "Material 规范"),
    (re.compile(r"the Material design spec"), "Material 规范"),
    (re.compile(r"the Material palette spec"), "Material 规范"),
    # ── the reference toolkit, with or without its markup-set parenthetical ──
    (re.compile(r"Qt\(the reference markup set\)"), "参考工具包"),
    (re.compile(r"Qt\s*(?:/|、|,|，)\s*the reference markup set"), "参考工具包"),
    (re.compile(r"the reference markup set"), "参考工具包的标记语言"),
    # ── the two long noun phrases on their own ──
    (re.compile(r"a mainstream material toolkit"), "主流 material 实现"),
    (re.compile(r"a mainstream declarative toolkit"), "主流声明式实现"),
    (re.compile(r"the reference toolkit"), "参考工具包"),
    # ── tidy the double-space and stray slash the earlier passes left ──
    (re.compile(r"主流 material 实现\s*/\s*"), "外部对标 / "),
    (re.compile(r"  +"), " "),
]

# A bare `Qt` that the earlier passes did not reach (it appears alone in comparison tables).
BARE_QT = re.compile(r"(?<![\w])Qt(?![\w])")


def main() -> None:
    touched = collections.Counter()
    for path in TARGETS:
        original = path.read_text(encoding="utf-8")
        text = original
        for pattern, replacement in SUBS:
            text, n = pattern.subn(replacement, text)
            touched[path.name] += n
        text, n = BARE_QT.subn("参考工具包", text)
        touched[path.name] += n
        if text != original:
            path.write_text(text, encoding="utf-8")

    for name, count in sorted(touched.items(), key=lambda kv: -kv[1]):
        if count:
            print(f"{count:4d}  {name}")
    print(f"files: {sum(1 for v in touched.values() if v)}  replacements: {sum(touched.values())}")


if __name__ == "__main__":
    main()
