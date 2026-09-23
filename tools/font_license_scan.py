#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# The rule this implements is stated in full — what it proves, what it does not, and the
# reverse injection that keeps it honest — in tools/check_font_licenses.sh, which is the only
# caller. That file is the documentation of record.
#
# Font data is the one dependency in this crate whose licence is *not* the crate's own, and it
# ships inside a generated source file rather than beside the code. So the check is structural:
# a generated font table must declare its provenance in its own header, and every fact in that
# header must be repeated in the repository-root `NOTICE`.
#
# Prints one `finding: ...` line per problem, then a `checked=N failed=M` summary line.
# Exit status is always 0; the shell wrapper makes the pass/fail judgement, so the exit code and
# the printed summary cannot disagree.

import argparse
import pathlib
import re
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent

# ── How a generated font table identifies itself ────────────────────────────────────────────────
# Two markers, both required: the file must say it is generated (so nobody hand-edits glyph bits
# it cannot maintain) and must name where the glyphs came from. A file with one and not the
# other is a file whose provenance is half-stated, which is the state this gate exists to make
# impossible.
GENERATED_MARKER = "GENERATED FILE"
SOURCE_LINE = re.compile(r"^//[/!]?\s*Glyph source:\s*(?P<name>.+?)\s*$", re.MULTILINE)
SHA_LINE = re.compile(r"SHA-256[^:]*:\s*(?P<hex>[0-9a-f]{64})")
REGEN_LINE = re.compile(r"python3\s+(?P<tool>tools/\S+\.py)")

# A licence record has to say *both* halves of a dual licence, because a reader who takes the
# file under only one of them needs to know the other exists.
LICENCE_TOKENS = ("Open Font License", "embedding exception")


class Finding(Exception):
    """One problem, raised at the point it is discovered and printed by `main`."""


def find_font_tables(roots):
    """Every `.rs` file under `roots` that declares itself generated font data.

    `roots` is a list of directories. A file is a font table when its header carries the
    generated marker *and* a `Glyph source:` line; the two together are what distinguishes it
    from a generated file of some other kind.
    """
    found = []
    for root in roots:
        base = pathlib.Path(root)
        if not base.exists():
            continue
        for path in sorted(base.rglob("*.rs")):
            text = path.read_text(encoding="utf-8", errors="replace")
            if GENERATED_MARKER in text and SOURCE_LINE.search(text):
                found.append((path, text))
    return found


def check_one(path: pathlib.Path, text: str, notice: str) -> None:
    """Raise [`Finding`] if `path`'s declared provenance is not fully recorded in `notice`."""
    try:
        # How `NOTICE` names the file. A file outside the repository (a reverse-injection copy)
        # has no such name, which is the finding rather than a crash.
        relative = path.relative_to(REPO_ROOT).as_posix()
    except ValueError:
        relative = path.as_posix()

    source = SOURCE_LINE.search(text)
    if source is None:
        raise Finding(f"{relative}: no `Glyph source:` line")
    # The project name only — the rest of the line is prose ("…, the bitmap `.hex` build").
    project = source.group("name").split(",")[0].strip()
    if not project:
        raise Finding(f"{relative}: `Glyph source:` names nothing")

    digest = SHA_LINE.search(text)
    if digest is None:
        raise Finding(f"{relative}: no SHA-256 of the upstream source")
    digest = digest.group("hex")

    regenerator = REGEN_LINE.search(text)
    if regenerator is None:
        raise Finding(f"{relative}: no `python3 tools/…` regenerate command")
    tool = REPO_ROOT / regenerator.group("tool")
    if not tool.exists():
        raise Finding(f"{relative}: names {regenerator.group('tool')}, which does not exist")

    # The NOTICE section for this table. Its own file name is the anchor: a section that names
    # the upstream project but not the file leaves a reader unable to tell which shipped bytes
    # the licence covers.
    if relative not in notice:
        raise Finding(f"NOTICE: no section names {relative}")
    if digest not in notice:
        raise Finding(f"NOTICE: does not repeat {relative}'s source digest {digest}")
    if project not in notice:
        raise Finding(f"NOTICE: does not name the glyph source {project!r}")
    for token in LICENCE_TOKENS:
        if token not in notice:
            raise Finding(f"NOTICE: the licence record omits {token!r}")


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", action="append", default=None,
                        help="a directory to scan (repeatable); defaults to `src`")
    parser.add_argument("--inject", default=None,
                        help="treat this extra file as font data, to prove the scan looks")
    parser.add_argument("--notice", default=str(REPO_ROOT / "NOTICE"))
    args = parser.parse_args(argv)

    roots = args.root or [str(REPO_ROOT / "src")]
    notice_path = pathlib.Path(args.notice)
    notice = notice_path.read_text(encoding="utf-8") if notice_path.exists() else ""
    if not notice:
        print(f"finding: {notice_path} is missing or empty; bundled font data has no licence record")
        print("checked=0 failed=1")
        return 0

    tables = find_font_tables(roots)
    if args.inject:
        injected = pathlib.Path(args.inject)
        if not injected.exists():
            print(f"finding: --inject {args.inject} does not exist")
            print(f"checked={len(tables)} failed=1")
            return 0
        tables.append((injected, injected.read_text(encoding="utf-8", errors="replace")))

    failed = 0
    for path, text in tables:
        try:
            check_one(pathlib.Path(path), text, notice)
        except Finding as problem:
            print(f"finding: {problem}")
            failed += 1

    if not tables:
        print("finding: no generated font table found; the scan is looking in the wrong place")
        failed += 1

    print(f"checked={len(tables)} failed={failed}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
