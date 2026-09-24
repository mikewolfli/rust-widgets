#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# The rule this implements is stated in full — what it proves, what it does not, and the
# reverse injection that keeps it honest — in tools/check_routing_match_is_exhaustive.sh,
# which is the only caller. That file is the documentation of record.
#
# `control_backend/routing.rs` carries the routing decision for every `WidgetKind` as an
# **exhaustive `match` with no wildcard arm**, so adding a variant without a routing decision
# fails to compile. That guarantee is one line from being silently destroyed: appending
# `_ => ControlRoutePreference::CustomRequired` makes the match compile while covering nothing,
# and it is the natural thing to write when the compiler complains. Measured: with a catch-all
# appended, this crate's whole test suite still passes and `check_control_route_matrix.sh` still
# reports 0 missing routes.
#
# So this scan asserts the *absence* of the defeat, not the presence of the match — the only
# form that cannot be satisfied by the mutation it exists to prevent.
#
# Prints one `finding: ...` line per problem, then a `matches=N wildcards=M` summary line.
# Exit status is always 0; the shell wrapper makes the pass/fail judgement.

import argparse
import pathlib
import re
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent

# The function whose match is the compile-time guarantee. Named rather than discovered: a scan
# that ranged over every `match` in the file would flag the incidental ones and miss the point.
GUARANTEED_FN = "route_is_library_painted"

# An arm that begins a pattern list with `_` — `_ =>`, `_ if … =>`. A wildcard *inside* a
# comment or a string is not an arm, so comments are stripped before scanning.
WILDCARD_ARM = re.compile(r"^\s*_\s*(?:if\b[^=]*)?=>", re.MULTILINE)


def strip_comments(text):
    """Removes `//`-style comments, so a wildcard mentioned in prose is not an arm.

    A hand-rolled stripper rather than a parser: this file is Rust source scanned by a gate that
    must run with nothing installed. The failure mode of over-stripping (a `//` inside a string
    literal) cannot occur in `routing.rs`, which holds no string literals — asserted below.
    """
    return "\n".join(line.split("//", 1)[0] for line in text.splitlines())


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--routing", default=str(REPO_ROOT / "src" / "control_backend" / "routing.rs"))
    args = parser.parse_args(argv)

    path = pathlib.Path(args.routing)
    if not path.exists():
        print(f"finding: {path} does not exist, so the routing guarantee cannot be checked")
        print("matches=0 wildcards=0")
        return 0

    raw = path.read_text(encoding="utf-8", errors="replace")
    text = strip_comments(raw)

    # Locate the guaranteed function's body by brace matching from its signature.
    signature = re.search(rf"fn\s+{GUARANTEED_FN}\s*\(", text)
    if signature is None:
        print(f"finding: {GUARANTEED_FN} is gone; the routing decision has no compile-time check")
        print("matches=0 wildcards=0")
        return 0

    body_start = None
    depth = 0
    for index in range(signature.start(), len(text)):
        char = text[index]
        if char == "{":
            depth += 1
            if depth == 1 and body_start is None:
                body_start = index
        elif char == "}":
            depth -= 1
            if depth == 0 and body_start is not None:
                body = text[body_start : index + 1]
                break
    else:
        print(f"finding: {GUARANTEED_FN}'s body could not be delimited")
        print("matches=0 wildcards=0")
        return 0

    matches = len(re.findall(r"\bmatch\b", body))
    wildcards = WILDCARD_ARM.findall(body)

    findings = []
    # The stripper is naive, so assert the assumption it relies on within the region that matters:
    # a `//` inside a *string literal* would truncate the literal and could hide an arm. The
    # function body holds none, and a gate that cannot trust its own scan should say so rather than
    # report a clean result. Scoped to `body`, not the whole file: the test module below legitimately
    # holds string literals in assertion messages.
    if '"' in body:
        findings.append(
            f"{GUARANTEED_FN}'s body holds a string literal, so this scan's comment stripper cannot "
            "be trusted to have seen every arm"
        )
    if matches == 0:
        findings.append(
            f"{GUARANTEED_FN} holds no `match`, so it no longer enumerates the kinds and the "
            "compiler is no longer checking anything"
        )
    if wildcards:
        findings.append(
            f"{GUARANTEED_FN} has {len(wildcards)} wildcard arm(s). A `_ =>` arm makes the match "
            "compile while covering nothing: adding a `WidgetKind` variant would then be routed "
            "silently. This is the one edit the exhaustive match exists to prevent."
        )

    for finding in findings:
        print(f"finding: {finding}")
    print(f"matches={matches} wildcards={len(wildcards)} findings={len(findings)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
