#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# The rule this implements is stated in full — what it proves, what it does not, and the reverse
# injection that keeps it honest — in tools/check_icon_licences.sh, which is the only caller. That
# file is the documentation of record.
#
# Icon data is the second dependency in this crate whose licence is not the crate's own (the first
# is the font data, checked by tools/font_license_scan.py). It ships as a generated source file
# derived from vendored SVGs, so the check is structural: the vendor tree must carry the required
# licence copy, the generated table must declare its provenance in its header, and NOTICE must
# repeat every fact.
#
# Prints one `finding: ...` line per problem, then a `checked=N failed=M` summary line.
# Exit status is always 0; the shell wrapper makes the pass/fail judgement, so the exit code and the
# printed summary cannot disagree.

import argparse
import hashlib
import pathlib
import re
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
VENDOR_DIR = REPO_ROOT / "tools" / "material_symbols"
GENERATED = REPO_ROOT / "src" / "widget" / "icon_data.rs"

# The pinned upstream revision and licence digest. These are literals here *and* in
# tools/vendor_material_symbols.py. Two copies is deliberate: this gate must be able to fail when
# the vendor script's pin is edited without the record being updated, and a shared import would
# make the two agree by construction.
UPSTREAM_SHA = "bd8cb85bd4bad964fe6918f79665bb40c3a8efef"
UPSTREAM_LICENSE_SHA256 = "58d1e17ffe5109a7ae296caafcadfdbe6a7d176f0bc4ab01e12a689b0499d8bd"

# The section marker NOTICE uses for the icon record. Its own heading is the anchor a reader
# follows, so the scan requires it verbatim.
NOTICE_HEADING = "Material Symbols — SVG path subsets for `icons`"

# Markers in the generated table's header.
GENERATED_MARKER = "GENERATED FILE"
ORIGIN_LINE = re.compile(r"^//\s*Icon source:\s*(?P<name>.+?)\s*$", re.MULTILINE)
USER_SHIPPED = re.compile(r"^//\s*([a-z_]+)\s*$", re.MULTILINE)


class Finding(Exception):
    """One problem, raised where it is discovered and printed by `main`."""


def sha256(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def icon_tokens() -> list[str]:
    """The vendor map's keys, read from the vendor script so the two cannot drift.

    The map is the single statement of "which icons this crate ships"; reading it here means a
    token added to the map but not vendored is caught, and the count in this gate is always the
    count the generator would produce.
    """
    source = (REPO_ROOT / "tools" / "vendor_material_symbols.py").read_text(encoding="utf-8")
    match = re.search(r"^ICONS:[^=]*=\s*\{(.*?)^\}", source, re.MULTILINE | re.DOTALL)
    if match is None:
        raise Finding("tools/vendor_material_symbols.py has no `ICONS` map to read")
    return re.findall(r'^\s*"([a-z_]+)":', match.group(1), re.MULTILINE)


def check_vendor_tree() -> int:
    """Assert the licence copy and every vendored outline exist. Returns the token count."""
    tokens = icon_tokens()
    if not tokens:
        raise Finding("the vendor map names no icons, so this scan is looking at nothing")

    licence = VENDOR_DIR / "LICENSE"
    if not licence.exists():
        raise Finding(f"{licence.relative_to(REPO_ROOT)} is missing (Apache-2.0 §4(a))")
    digest = sha256(licence)
    if digest != UPSTREAM_LICENSE_SHA256:
        raise Finding(
            f"{licence.relative_to(REPO_ROOT)} digest {digest[:16]}… != the pinned upstream "
            f"{UPSTREAM_LICENSE_SHA256[:16]}…, so the copy is not the pinned revision's licence"
        )

    for token in tokens:
        if not (VENDOR_DIR / f"{token}.svg").exists():
            raise Finding(f"tools/material_symbols/{token}.svg is missing; {token} has no source")

    # §4(d): upstream ships no NOTICE, so the obligation is not triggered. The marker is the
    # evidence, and its absence means either the probe was never run or upstream added a NOTICE —
    # both need a human, so both are findings rather than a silent pass.
    if not (VENDOR_DIR / "UPSTREAM_HAS_NO_NOTICE").exists():
        raise Finding(
            "tools/material_symbols/UPSTREAM_HAS_NO_NOTICE is missing; upstream may now ship a "
            "NOTICE (Apache-2.0 §4(d)), which must be vendored rather than dropped"
        )

    return len(tokens)


def check_generated_table(tokens: int) -> None:
    """Assert the generated table's header names its origin, its pin and its licence."""
    if not GENERATED.exists():
        raise Finding(f"{GENERATED.relative_to(REPO_ROOT)} does not exist; run the generator")
    text = GENERATED.read_text(encoding="utf-8")
    if GENERATED_MARKER not in text:
        raise Finding(f"{GENERATED.relative_to(REPO_ROOT)} has no `{GENERATED_MARKER}` marker")

    origin = ORIGIN_LINE.search(text)
    if origin is None:
        raise Finding(f"{GENERATED.relative_to(REPO_ROOT)} has no `Icon source:` line")
    project = origin.group("name").split(",")[0].strip()
    if project != "Material Symbols":
        raise Finding(
            f"{GENERATED.relative_to(REPO_ROOT)} names the origin {project!r}, not 'Material Symbols'"
        )

    if UPSTREAM_SHA not in text:
        raise Finding(
            f"{GENERATED.relative_to(REPO_ROOT)} does not name the pinned revision {UPSTREAM_SHA[:12]}…"
        )
    if "Apache License 2.0" not in text and "Apache-2.0" not in text:
        raise Finding(
            f"{GENERATED.relative_to(REPO_ROOT)} does not state the licence (Apache-2.0, §4(b))"
        )
    if "copied verbatim" not in text:
        raise Finding(
            f"{GENERATED.relative_to(REPO_ROOT)} does not state that the outlines are unmodified; "
            "§4(b) requires the modification status to be visible"
        )

    # The table's own entry count must match the vendor tree, so an icon cannot be declared and
    # then quietly dropped from the payload.
    entries = text.count("IconData {")
    if entries != tokens:
        raise Finding(
            f"{GENERATED.relative_to(REPO_ROOT)} declares {entries} icons but the vendor tree has "
            f"{tokens}; regenerate with `tools/gen_icon_data.py --license=apache-2.0`"
        )


def check_notice(path: pathlib.Path, tokens: int) -> None:
    """Assert NOTICE carries a complete Material Symbols record (§4(c))."""
    if not path.exists():
        raise Finding(f"{path} is missing; bundled icon data has no licence record")
    notice = path.read_text(encoding="utf-8")
    if NOTICE_HEADING not in notice:
        raise Finding(f"NOTICE: no section headed {NOTICE_HEADING!r}")
    if UPSTREAM_SHA not in notice:
        raise Finding(f"NOTICE: does not repeat the pinned revision {UPSTREAM_SHA[:12]}…")
    if UPSTREAM_LICENSE_SHA256 not in notice:
        raise Finding("NOTICE: does not repeat the upstream LICENSE digest")
    if "Apache License, Version 2.0" not in notice:
        raise Finding("NOTICE: the icon record does not state the licence")
    if "src/widget/icon_data.rs" not in notice:
        raise Finding("NOTICE: does not name the generated file the licence covers")
    if "tools/material_symbols/LICENSE" not in notice:
        raise Finding("NOTICE: does not name the licence copy §4(a) requires")
    if str(tokens) not in notice:
        # The record states the icon count, so a vendor refresh that adds icons must update it —
        # the same "counted, not assumed" shape the font record uses.
        raise Finding(f"NOTICE: the icon record does not state the icon count ({tokens})")


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--notice", default=str(REPO_ROOT / "NOTICE"))
    args = parser.parse_args(argv)

    failed = 0
    tokens = 0
    try:
        tokens = check_vendor_tree()
        check_generated_table(tokens)
        check_notice(pathlib.Path(args.notice), tokens)
    except Finding as problem:
        print(f"finding: {problem}")
        failed = 1

    if tokens == 0:
        print("finding: no icon tokens were found; the scan is looking in the wrong place")
        failed = 1
    print(f"checked={tokens} failed={failed}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
