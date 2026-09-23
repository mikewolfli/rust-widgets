#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# The rule this implements is stated in full — what it proves, what it does not, and the
# reverse injection that keeps it honest — in tools/check_text_coverage_claim_matches_features.sh,
# which is the only caller. That file is the documentation of record.
#
# BLUE23 §0A.2 is a *declaration*: the crate must say what it actually draws, and must not say
# more. A declaration decays in both directions — the day data is added the "Latin only" claim
# becomes an understatement, and the day a claim is written without data it becomes an
# overstatement. So the claim is checked against the feature graph rather than read.
#
# Prints one `finding: ...` line per problem, then a `docs=N features=M failed=K` summary line.
# Exit status is always 0; the shell wrapper makes the pass/fail judgement.

import argparse
import pathlib
import re
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent

# Where the coverage boundary is declared. Both are read: a crate that told its Rust users and
# not its GitHub visitors would have half a declaration.
DOC_FILES = ("src/lib.rs", "README.md")

# The claim's required vocabulary: the scripts the default build *does* draw, the marker for
# what it draws instead of the rest, and the pointer to the opt-in path.
REQUIRED = {
    "Latin": "names the default script coverage",
    "ASCII": "names the default script coverage",
    "font data": "says what full coverage would need",
}

FONT_FEATURE = re.compile(r"^fonts-(\S+?)\s*=", re.MULTILINE)


def has_font_feature() -> bool:
    text = (REPO_ROOT / "Cargo.toml").read_text(encoding="utf-8")
    return bool(FONT_FEATURE.search(text))


def display_name(path: pathlib.Path) -> str:
    """How a finding refers to `path` — repo-relative when it is inside the repo.

    A reverse-injection copy lives in a temp directory, and it is the *finding* that it has no
    repository name rather than a reason to crash.
    """
    try:
        return path.relative_to(REPO_ROOT).as_posix()
    except ValueError:
        return path.as_posix()


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--file", action="append", default=None,
                        help="check these files instead of the defaults (repeatable)")
    args = parser.parse_args(argv)

    targets = [pathlib.Path(p) for p in (args.file or [])] or [REPO_ROOT / p for p in DOC_FILES]
    findings = []
    for path in targets:
        if not path.exists():
            findings.append(f"{path}: missing")
            continue
        text = path.read_text(encoding="utf-8")
        name = display_name(path)
        for token, why in REQUIRED.items():
            if token.lower() not in text.lower():
                findings.append(f"{name}: does not mention {token!r}, which {why}")

    # With font data declared, a claim that stops at "Latin/ASCII" is now an understatement: the
    # reader has to be able to find the switch. This is the half that would rot unnoticed,
    # because the day a face is added nothing in the default build changes.
    if has_font_feature():
        for path in targets:
            if not path.exists():
                continue
            text = path.read_text(encoding="utf-8")
            if "fonts-" not in text:
                findings.append(
                    f"{display_name(path)}: font data exists but the coverage note never names "
                    "a `fonts-` feature"
                )

    for finding in findings:
        print(f"finding: {finding}")
    print(f"docs={len(targets)} features={int(has_font_feature())} failed={len(findings)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
