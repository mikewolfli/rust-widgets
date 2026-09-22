#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# The rule this implements is stated in full — what it proves and what it does not, with the
# exemption list and its rationale — in tools/check_transition_durations_are_tokens.sh, which is the
# only caller. That file is the documentation of record.
#
# This is a separate module rather than a heredoc inside the shell script because bash 3.2 — the
# bash on this project's macOS development host — cannot parse a here-document inside a command
# substitution, and the failure it reports is a misleading "unexpected EOF while looking for matching
# quote" attributed to a line near the end of the file rather than to the construct that caused it.
#
# Prints one `file:line: ...` finding per offender, then a single `checked=N failed=M` summary line.
# Exit status is always 0: the shell wrapper makes the pass/fail judgement, so the exit code and the
# printed summary cannot disagree.

import pathlib
import re

# ── The sites excluded by definition, not by exemption ──────────────────────────────────────────
# A token's *definition site* is where the number is supposed to live. Flagging it would be flagging
# the mechanism itself.
TOKEN_DEFINITION_DIRS = (
    "src/style/animation.rs",
    "src/theme/",
)

# ── The two literal shapes that carry a duration ────────────────────────────────────────────────
# (1) An argument to `from_millis`. Whether the argument is a literal decides the verdict.
FROM_MILLIS = re.compile(r"from_millis\(([^)]*)\)")

# (2) A `const` whose name says it is a duration and whose value is a bare number — the second way
#     a hardcoded duration hides, and the way `floating_label` used. The name pattern is
#     deliberately narrow (`_MS`, `_DELAY`, `_DURATION`) so it does not fire on unrelated constants
#     that merely happen to be numbers.
DURATION_CONST = re.compile(
    r"const\s+([A-Z0-9_]*(?:_MS|_DELAY|_DURATION)[A-Z0-9_]*)\s*:\s*(?:u32|u64|i32|f32|f64)\s*=\s*([0-9][0-9_]*\.?[0-9]*)\s*;"
)

# A numeric literal, possibly suffixed (`0.5`, `2.0f32`, `1_000`). Anything else — an identifier, a
# field read, a function call — is derived from somewhere and is not this rule's subject.
NUMERIC_LITERAL = re.compile(r"^[0-9][0-9_]*(\.[0-9]+)?(u32|u64|i32|f32|f64)?$")


def production_lines(path):
    """Returns [(line_number, text)] for lines outside a `mod tests` block.

    Test modules are excluded because a duration literal in a test is the *fixture* for an
    assertion — a test that advances an animation by 250 ms has to say 250 — and requiring those to
    be tokens would force every test to import a constant it is deliberately not exercising.
    Excluding them is what keeps the rule's subject "the durations a control actually runs at".
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


def exempt_paths():
    """The first whitespace-separated field of each non-comment line of the exemption table."""
    table = pathlib.Path("tools/transition_duration_exemptions.txt")
    if not table.exists():
        return set()
    names = set()
    for line in table.read_text(encoding="utf-8").splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        names.add(stripped.split()[0])
    return names


def findings_for(path, exempt):
    """Yields finding strings for one file."""
    key = str(path)
    if key.startswith(TOKEN_DEFINITION_DIRS) or key in exempt:
        return

    for number, line in production_lines(path):
        stripped = line.strip()
        # A comment that mentions a duration is documentation, not a duration.
        if stripped.startswith("//"):
            continue

        match = FROM_MILLIS.search(line)
        if match:
            argument = match.group(1).strip()
            if NUMERIC_LITERAL.match(argument):
                yield (
                    f"{key}:{number}: `from_millis({argument})` is a hardcoded duration literal; "
                    f"read it from `theme.motion` (fast/normal/slow) instead: {stripped}"
                )
            continue

        match = DURATION_CONST.search(line)
        if match:
            yield (
                f"{key}:{number}: `const {match.group(1)} = {match.group(2)}` hardcodes a "
                f"transition duration; read it from `theme.motion` (fast/normal/slow) instead: "
                f"{stripped}"
            )


def main():
    exempt = exempt_paths()
    offenders = []
    checked = 0
    for path in sorted(pathlib.Path("src/widget").rglob("*.rs")):
        checked += 1
        offenders.extend(findings_for(path, exempt))

    for offender in offenders:
        print(offender)
    print(f"checked={checked} failed={len(offenders)}")


if __name__ == "__main__":
    main()
