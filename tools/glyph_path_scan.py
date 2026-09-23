#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# The rule this implements is stated in full — what it proves, what it does not, and the
# reverse injection that keeps it honest — in tools/check_glyph_source_is_the_only_glyph_path.sh,
# which is the only caller. That file is the documentation of record.
#
# A glyph must be reached through `crate::render::text`, never by handing a character to a face
# table directly. The scan is lexical and whole-tree: it names the one file allowed to know a
# face's name, and reports every other file that does.
#
# Prints one `finding: ...` line per problem, then a `scanned=N failed=M` summary line.
# Exit status is always 0; the shell wrapper makes the pass/fail judgement.

import argparse
import pathlib
import re
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent

# The only module permitted to name a face. Everything else must go through the stack, so that
# adding a face is a change in one file rather than in every renderer.
FACE_HOME = "src/render/text/glyph_source.rs"

# Direct access to a face's table. Both spellings are checked because a call site reaches for
# either the crate's name or the table's constant.
FACE_ACCESS = re.compile(r"\bBASIC_FONTS\b|\bfont8x8::")

# The accessor this rule replaced. Its return type (`[u8; 8]`) is the very thing that made a
# non-Latin face impossible, so its return is a finding even in the file that is allowed to name
# a face.
RETIRED_ACCESSOR = re.compile(r"\bfn\s+glyph_bitmap\s*\(")

# The one geometry derivation both renderers read. It is asserted to be *used* by both, because
# a backend that stopped calling it would be drawing glyphs by its own arithmetic again.
GEOMETRY = "glyph_rects("
GEOMETRY_CONSUMERS = ("src/render/pipeline/pixel_ops.rs", "src/render/svg/backend.rs")


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--src", default=str(REPO_ROOT / "src"))
    parser.add_argument("--inject", action="append", default=[],
                        help="an extra file to scan, to prove the scan looks (repeatable)")
    args = parser.parse_args(argv)

    files = sorted(pathlib.Path(args.src).rglob("*.rs"))
    files.extend(pathlib.Path(p) for p in args.inject)

    findings = []
    scanned = 0
    for path in files:
        if not path.exists():
            continue
        scanned += 1
        text = path.read_text(encoding="utf-8", errors="replace")
        try:
            relative = path.relative_to(REPO_ROOT).as_posix()
        except ValueError:
            relative = path.as_posix()

        for number, line in enumerate(text.splitlines(), 1):
            # Comments are skipped: this rule is about *code* that reaches a face, and a doc
            # comment that names `font8x8` (there are many, describing the old output) is
            # documentation, not access.
            stripped = line.lstrip()
            if stripped.startswith(("//", "*", "/*")):
                continue
            if FACE_ACCESS.search(line) and relative != FACE_HOME:
                findings.append(f"{relative}:{number}: reaches a face table outside {FACE_HOME}")

        if RETIRED_ACCESSOR.search(text):
            findings.append(f"{relative}: defines `glyph_bitmap`, the retired single-face accessor")

    for consumer in GEOMETRY_CONSUMERS:
        path = REPO_ROOT / consumer
        if not path.exists() or GEOMETRY not in path.read_text(encoding="utf-8"):
            findings.append(f"{consumer}: does not read the shared `glyph_rects` geometry")

    for finding in findings:
        print(f"finding: {finding}")
    print(f"scanned={scanned} failed={len(findings)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
