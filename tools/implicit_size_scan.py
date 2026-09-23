#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# The rule this implements is stated in full — what it proves and what it does not, with the
# exemption list and its rationale — in tools/check_implicit_size_uses_metrics.sh, which is the only
# caller. That file is the documentation of record.
#
# This is a separate module rather than a heredoc inside the shell script because bash 3.2 — the
# bash on this project's macOS development host — cannot parse a here-document inside a command
# substitution, and the failure it reports is a misleading "unexpected EOF while looking for
# matching quote".
#
# Prints one `file:line: ...` finding per offender, then a single `checked=N failed=M` summary line.
# Exit status is always 0: the shell wrapper makes the pass/fail judgement, so the exit code and the
# printed summary cannot disagree.

import pathlib
import re

# ── Where a size_hint lives ─────────────────────────────────────────────────────────────────────
# The whole widget tree, because a size hint can be written on any control. Test modules are
# excluded by `production_lines` below: a test that pins a hint's value has to name the value.
SIZE_HINT_IMPL = re.compile(r"\bfn\s+size_hint\s*\(")

# ── The vocabulary that makes a size hint metric-driven ─────────────────────────────────────────
# Any one of these in the body is enough. The list is the *widening* set: `ControlMetrics` is the
# shared derivation, `estimate_text_width` / `estimate_line_height` are the pure measurement
# primitives it is built from, and `dimensions::` is the named-constant table. A hint that uses
# none of them is computing its answer from literals.
METRIC_VOCABULARY = (
    "ControlMetrics",
    "estimate_text_width",
    "estimate_line_height",
    "dimensions::",
)

# ── The exception ───────────────────────────────────────────────────────────────────────────────
# `Widget::size_hint`'s default implementation in the trait definition is the *mechanism* — it is
# what a control that has no opinion inherits. Flagging it would be flagging the trait itself.
SIZE_HINT_DEFINITION_FILE = "src/widget/widget_trait.rs"

EXEMPTION_TABLE = "tools/implicit_size_exemptions.txt"


def exemptions():
    """The `path:line` keys of each non-comment, non-blank line of the exemption table.

    Keyed by path *and* line rather than by path alone, so a control that gains a second
    `size_hint` cannot inherit the first one's exemption by accident — which is how a debt listing
    silently covers a new instance of the very thing it was listing.
    """
    table = pathlib.Path(EXEMPTION_TABLE)
    if not table.exists():
        return set()
    keys = set()
    for line in table.read_text(encoding="utf-8").splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        fields = stripped.split()
        if len(fields) >= 2:
            keys.add(f"{fields[0]}:{fields[1]}")
    return keys


def production_lines(path):
    """Returns [(line_number, text)] for lines outside a `mod tests` block.

    Test modules are excluded because a test that pins a size hint's value is the *fixture* for an
    assertion — it has to name the numbers it expects. The rule's subject is what a control claims
    at runtime, not what a test verifies about it.
    """
    text = path.read_text(encoding="utf-8").splitlines()
    out = []
    i = 0
    while i < len(text):
        if re.match(r"\s*(pub\s+)?mod\s+tests\b", text[i]):
            depth = 0
            started = False
            while i < len(text):
                depth += text[i].count("{") - text[i].count("}")
                if "{" in text[i]:
                    started = True
                i += 1
                if started and depth <= 0:
                    break
            continue
        out.append((i + 1, text[i]))
        i += 1
    return out


def body_of(lines, start_index):
    """The `fn size_hint` body starting at `start_index`, by brace balance.

    Returns `(first_body_line_number, body_text)`. Brace depth is counted on the raw text, which is
    enough here: the tree's size hints contain no braces inside string or character literals, and a
    balance-counting parser that tried to handle those would be a lexer pretending to be a
    formatter for one rule's benefit.
    """
    depth = 0
    started = False
    body = []
    for number, text in lines[start_index:]:
        body.append(text)
        depth += text.count("{") - text.count("}")
        if "{" in text:
            started = True
        if started and depth <= 0:
            break
    first = lines[start_index][0] if lines[start_index:] else 0
    return first, "\n".join(body)


def main():
    checked = 0
    findings = []
    exempt = exemptions()
    hint_count = 0

    for path in sorted(pathlib.Path("src/widget").rglob("*.rs")):
        rel = path.as_posix()
        if rel == SIZE_HINT_DEFINITION_FILE:
            continue
        lines = production_lines(path)
        for index, (number, text) in enumerate(lines):
            if not SIZE_HINT_IMPL.search(text):
                continue
            hint_count += 1
            first, body = body_of(lines, index)
            if any(token in body for token in METRIC_VOCABULARY):
                continue
            if f"{rel}:{first}" in exempt:
                continue
            findings.append(
                f"{rel}:{first}: `size_hint` derives its answer without ControlMetrics, "
                f"estimate_text_width/estimate_line_height or dimensions::"
            )
        checked += 1

    for finding in findings:
        print(finding)
    print(f"checked={checked} failed={len(findings)}")


if __name__ == "__main__":
    main()
