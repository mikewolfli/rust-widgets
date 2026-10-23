#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Scratch scanner: allocation shapes inside `impl Draw for` bodies.

Reports, per file, every `format!` / `to_string()` / `vec![` / `String::new` /
`Vec::new` / `Vec::with_capacity` / `.collect()` occurrence that falls inside a
`fn draw` body. Deliberately textual: it is a lead generator, each hit must be
reviewed by hand before it counts as a defect.
"""

import glob
import re
import sys

PATTERNS = [
    ("format!", re.compile(r"\bformat!\s*\(")),
    ("to_string", re.compile(r"\.to_string\(\)")),
    ("to_owned", re.compile(r"\.to_owned\(\)")),
    ("vec!", re.compile(r"\bvec!\s*[\[\{]")),
    ("String::new", re.compile(r"\bString::new\(\)")),
    ("String::from", re.compile(r"\bString::from\(")),
    ("Vec::new", re.compile(r"\bVec::new\(\)")),
    ("Vec::with_capacity", re.compile(r"\bVec::with_capacity\(")),
    ("collect", re.compile(r"\.collect(?:::<[^>]*>)?\(\)")),
]

FN_DRAW = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?fn\s+draw(?:_with_context|_in|_text)?\s*[(<]")


def fn_body_end(lines, start):
    """Return the index of the last line of the brace-matched body."""
    depth = 0
    seen = False
    for i in range(start, len(lines)):
        for ch in lines[i]:
            if ch == "{":
                depth += 1
                seen = True
            elif ch == "}":
                depth -= 1
        if seen and depth <= 0:
            return i
    return len(lines) - 1


def main() -> int:
    total = 0
    for path in sorted(glob.glob("src/**/*.rs", recursive=True)):
        lines = open(path, encoding="utf-8", errors="ignore").read().splitlines()
        for i, line in enumerate(lines):
            if not FN_DRAW.match(line):
                continue
            end = fn_body_end(lines, i)
            for j in range(i, end + 1):
                for name, pat in PATTERNS:
                    if pat.search(lines[j]):
                        print(f"{path}:{j + 1}: {name}: {lines[j].strip()[:110]}")
                        total += 1
    print(f"\ntotal hits: {total}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
