#!/usr/bin/env python3
"""Prepend the project SPDX copyright + license header to every source file.

Rust files get `//` line comments; Markdown files get an HTML comment so the
header does not render into the document body.

The script is idempotent: a file that already carries an
`SPDX-License-Identifier` line is left untouched, so re-running it cannot stack
duplicate headers.

Usage: python3 tools/add_spdx_headers.py [--check]
  --check  report files still missing the header; do not modify anything
"""

import io
import os
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(REPO, "src")

COPYRIGHT = (
    "SPDX-FileCopyrightText: Copyright (c) 2026 "
    "Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)"
)
LICENSE = "SPDX-License-Identifier: MIT"

# Extension -> line comment template. Each format gets a two-line header
# followed by a blank line, so the module docs (`//!`) that typically open a
# Rust file stay visually separated from the license block.
STYLES = {
    ".rs": "// {line}",
    ".md": "<!-- {line} -->",
}


def header_for(ext: str) -> str:
    template = STYLES[ext]
    lines = [template.format(line=COPYRIGHT), template.format(line=LICENSE)]
    return "\n".join(lines) + "\n\n"


def main() -> int:
    check_only = "--check" in sys.argv[1:]
    changed = 0
    missing = 0
    for root, _dirs, files in os.walk(SRC):
        for name in sorted(files):
            ext = os.path.splitext(name)[1]
            if ext not in STYLES:
                continue
            path = os.path.join(root, name)
            text = io.open(path, encoding="utf-8").read()
            # Idempotency: skip when the header is already present in the first
            # few lines (a file's module docs may legitimately mention SPDX much
            # later, which must not count as a header).
            head = "\n".join(text.split("\n")[:3])
            if "SPDX-License-Identifier" in head:
                continue
            missing += 1
            rel = os.path.relpath(path, REPO)
            if check_only:
                print("missing:", rel)
                continue
            io.open(path, "w", encoding="utf-8").write(header_for(ext) + text)
            changed += 1
    if check_only:
        print(f"{missing} file(s) still missing the SPDX header")
        return 1 if missing else 0
    print(f"added SPDX header to {changed} file(s)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
