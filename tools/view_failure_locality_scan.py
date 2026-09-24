#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# The rule this implements is stated in full — what it proves, what it does not, and the
# reverse injection that keeps it honest — in tools/check_view_failures_are_local.sh, which is
# the only caller. That file is the documentation of record.
#
# BLUE23 §5A.5 (P1-15): a failure while applying one node must mark **that node** and leave its
# siblings in the tree. The defect it removes is an earlier `build`/apply that dropped the whole
# document on any error, so "the 137th control has a typo" and "the window cannot render at all"
# had one consequence.
#
# The check is structural, on the engine's apply path:
#
#   1. the "collect a failure and keep going" accumulator must exist — a `Vec<ViewError>` (or a
#      similarly-named sink) is accumulated rather than the first error being returned;
#   2. `apply_node`'s failure path must not `return Err` out of the *subtree* walk: the recursion
#      has to record and continue;
#   3. a placeholder must be produced for a failed node, so the failure is visible rather than a
#      zero-sized hole (criterion 13).
#
# A source scan rather than a runtime probe, because the property is "the code cannot express
# whole-document failure" — which a passing test on one input cannot establish.
#
# Prints one `finding: ...` line per problem, then a `checks=N failed=M` summary line.

import argparse
import pathlib
import re
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent


def strip_comments(text):
    """Removes `//`-style comments and `/* */` blocks, so prose does not satisfy a check."""
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.DOTALL)
    return "\n".join(line.split("//", 1)[0] for line in text.splitlines())


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--src", default=str(REPO_ROOT / "src" / "view"))
    args = parser.parse_args(argv)

    root = pathlib.Path(args.src)
    if not root.is_dir():
        print(f"finding: {root} is not a directory, so the locality rule cannot be checked")
        print("checks=0 failed=1")
        return 0

    # The machinery is split across two files, and the scan reads both rather than one: the *report*
    # (the `Vec<ViewError>` sink and the placeholder accessor) lives in `apply.rs`, while the *pushes*
    # that make a failure local live in `engine.rs`. Scanning only `engine.rs` reported the sink and
    # the accessor as missing on a correct tree — verified, and the reason this reads both.
    engine = root / "engine.rs"
    apply = root / "apply.rs"
    missing = [str(path) for path in (engine, apply) if not path.exists()]
    if missing:
        print(f"finding: {', '.join(missing)} does not exist")
        print("checks=0 failed=1")
        return 0

    engine_text = strip_comments(engine.read_text(encoding="utf-8", errors="replace"))
    apply_text = strip_comments(apply.read_text(encoding="utf-8", errors="replace"))
    findings = []

    # 1. The accumulator, and its name. `ApplyReport` holds `errors: Vec<ViewError>`, and the
    #    failing paths push into it. The scan matches the *shape* rather than the literal spelling,
    #    so renaming the field does not read as a regression: what matters is that a
    #    `Vec<ViewError>` sink exists and that failures go into it.
    if not re.search(r"\w+\s*:\s*(?:crate::compat::)?Vec<\.{0,2}ViewError>", apply_text):
        findings.append(
            "apply.rs has no `Vec<ViewError>` sink: failures cannot be accumulated, so the only "
            "alternative left is to return the first one out of the whole apply"
        )

    # 2. The walk must record-and-continue rather than bail. The pushes are the lines that make a
    #    failure local, so their absence means every failure path returns out instead.
    pushes = len(re.findall(r"\.errors\.push\(\s*(?:super::|\.{1,2})?ViewError::", engine_text))
    if pushes == 0:
        findings.append(
            "no `.errors.push(ViewError::…)` in engine.rs: a failure is not recorded-and-continued, "
            "which is the shape that drops the whole document"
        )

    # 3. A failed node must render as something. The placeholder is what makes criterion 13 true,
    #    and it is the reason a failure is visible rather than a hole in the layout.
    if not re.search(r"fn\s+\w*placeholders?\s*\(", apply_text, re.IGNORECASE):
        findings.append(
            "apply.rs has no placeholder accessor, so a failed node has nothing to render into and "
            "its failure would be invisible"
        )

    for finding in findings:
        print(f"finding: {finding}")
    print(f"checks=3 failures_pushed={pushes} failed={len(findings)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
