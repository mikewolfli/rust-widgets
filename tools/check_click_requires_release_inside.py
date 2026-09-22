#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# The rule this implements is stated in full — what it proves and, importantly, what it does not —
# in tools/check_click_requires_release_inside.sh, which is the only caller. That file is the
# documentation of record: read it for the three mechanisms a control may honour the rule through,
# the reasoning behind each, and the reverse injection this was verified with.
#
# This is a separate module rather than a heredoc inside the shell script because bash 3.2 — the
# `/bin/sh`-adjacent bash on this project's macOS development host — cannot parse a here-document
# inside a command substitution (`X="$(cmd <<'PY' ... PY)"`), and the failure it produces is a
# misleading "unexpected EOF while looking for matching quote" attributed to a line near the end of
# the file rather than to the construct that caused it.
#
# Prints one `file:line: ...` finding per offender, then a single `checked=N failed=M` summary line.
# Exit status is 0 regardless of findings: the shell wrapper makes the pass/fail judgement, so that
# the exit code and the printed summary cannot disagree.

import pathlib
import re

# A file is in the population only if it emits a click AND handles a release in production code.
# Both are required: a control with no release arm cannot have this defect (its click comes from a
# press, a key or `Tap`, which are different contracts), and a control that handles a release but
# never clicks commits nothing.
CLICK = "clicked.emit()"
RELEASE = "Event::MouseRelease"

# Arm (a): the containment vocabulary. `contains_point` and the `Rect` spelling that the crate's
# `BaseWidget::contains_point_with_touch_expansion` delegates to are both containment tests — the
# latter is what `canvas`, `chart`, `empty_state`, `font_combo_box` and `mini_canvas` use.
CONTAINMENT = ("contains_point", "geometry().contains")

# Arm (b): the abandon-the-press-on-leave latch-clear. Matched as the *combination* of the leave
# arm and clearing the latch, because either alone is unrelated to this rule.
LEAVE = "Event::MouseLeave"
LATCH_CLEAR = re.compile(r"self\.(pressed|mouse_pressed)\s*=\s*false")


def production_lines(path):
    """Returns [(line_number, text_or_None)] for every line of the file.

    Lines inside a `mod tests` block map to None. The exclusion is load-bearing: a test that
    asserts "a release outside must not click" has to name both `Event::MouseRelease` and
    `clicked.emit()`, so a test-bearing file would enter the population through its *tests* and be
    judged on the production code those tests are checking. Worse, the negative test's own text
    proves the property this gate asks about, so excluding the test module is what stops the gate
    passing a file because it was *tested* rather than because it is right. The module is consumed
    by brace depth so a nested block does not end it early, and line numbers are preserved (None
    keeps the slot) so a finding names the real line.
    """
    text = path.read_text(encoding="utf-8").splitlines()
    out = []
    i = 0
    while i < len(text):
        line = text[i]
        if re.match(r"\s*(pub\s+)?mod\s+tests\b", line):
            depth = 0
            started = False
            while i < len(text):
                depth += text[i].count("{") - text[i].count("}")
                if "{" in text[i]:
                    started = True
                out.append((i + 1, None))
                i += 1
                if started and depth <= 0:
                    break
            continue
        out.append((i + 1, line))
        i += 1
    return out


def exempt_paths():
    """The exemption table's first whitespace-separated field of each non-comment line."""
    table = pathlib.Path("tools/click_release_exemptions.txt")
    if not table.exists():
        return set()
    names = set()
    for line in table.read_text(encoding="utf-8").splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        names.add(stripped.split()[0])
    return names


def main():
    exempt = exempt_paths()
    offenders = []
    checked = 0
    for path in sorted(pathlib.Path("src/widget").rglob("*.rs")):
        body = [(n, t) for (n, t) in production_lines(path) if t is not None]
        text = "\n".join(t for (_, t) in body)
        if CLICK not in text or RELEASE not in text:
            continue
        checked += 1

        key = str(path)
        if key in exempt:
            continue
        if any(word in text for word in CONTAINMENT):
            continue
        if LEAVE in text and LATCH_CLEAR.search(text):
            continue

        for number, line in body:
            if CLICK in line:
                offenders.append(
                    f"{key}:{number}: emits `clicked` from a release path with no containment, "
                    f"no `MouseLeave` latch-clear, and no exemption: {line.strip()}"
                )
                break

    for offender in offenders:
        print(offender)
    print(f"checked={checked} failed={len(offenders)}")


if __name__ == "__main__":
    main()
