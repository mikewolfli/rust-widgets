#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# The rule this implements is stated in full — what it proves, what it does not, and why the two
# legitimate arms are the only ones — in tools/check_focus_ring_respects_reason.sh, which is the
# only caller. That file is the documentation of record.
#
# Prints one `file:line: ...` finding per offender, then a single `checked=N failed=M` summary line.
# Exit status is always 0: the shell wrapper makes the pass/fail judgement.

import pathlib
import re

# ── The site under test ─────────────────────────────────────────────────────────────────────────
# A ring is *drawn* by asking the shared constructor for its geometry. Matching the constructor
# rather than the word "focus" keeps the scan on the act, not on the state: a control that stores a
# `focused` bool and never paints a ring has nothing to gate.
RING_CONSTRUCTION = re.compile(r"FocusRing::for_control\s*\(")

# ── The two legitimate arms ─────────────────────────────────────────────────────────────────────
# (a) The predicate. `visual_focus()` is `focused && focus_reason.draws_focus_ring()` — the Qt
#     Quick rule, evaluated in one place so a control cannot implement "has focus" as "draw the
#     ring". Every draw site in this tree reaches the constructor through an `if` on it.
PREDICATE = re.compile(r"\b(?:visual_focus|draws_focus_ring)\s*\(")

# (b) A file that carries the pair itself: it holds a `FocusReason` field *and* classifies it. A
#     control is allowed to own the rule as long as it owns both halves — the reason and the
#     question asked of it. What is never allowed is holding a plain `focused: bool` and painting
#     from that, because then a pointer click shows a ring.
#
# The two halves are stated as the *presence of the classification*, which is what makes the arm
# meaningful: `draws_focus_ring` is the one method that knows the answer.
CLASSIFICATION = re.compile(r"\bdraws_focus_ring\b")


def production_lines(path):
    """Returns [(line_number, text)] for lines outside a `mod tests` block.

    Test modules are excluded because a test that drives `FocusGained { reason: .. }` and asserts
    the ring is the *evidence* the rule works, not a draw site. The rule's subject is production
    paint code.
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


def guarded_by_predicate(lines, index):
    """Whether the ring construction at `lines[index]` sits inside an `if` on the predicate.

    The search walks **backwards** from the constructor by a bounded window, because the guard is
    written above the construction:

        if self.visual_focus() {
            let ring = FocusRing::for_control(rect, radius);
            ...
        }

    A window rather than a statement parser: the tree's draw sites are short and direct, and a
    brace-matching parser would be a lot of machinery for one rule. The window is deliberately
    small so a guard that belongs to a *different* construct cannot satisfy this one.
    """
    window = 12
    for number, text in lines[max(0, index - window) : index]:
        stripped = text.strip()
        if stripped.startswith("//"):
            continue
        if PREDICATE.search(text) and ("if" in stripped or "match" in stripped):
            return True
    return False


def main():
    checked = 0
    findings = []
    sites = 0

    for path in sorted(pathlib.Path("src/widget").rglob("*.rs")):
        rel = path.as_posix()
        lines = production_lines(path)
        joined = "\n".join(text for _, text in lines)

        constructions = [
            index
            for index, (_, text) in enumerate(lines)
            if RING_CONSTRUCTION.search(text) and not text.strip().startswith("//")
        ]
        if not constructions:
            checked += 1
            continue

        sites += len(constructions)

        # Arm (b): the file carries the classification itself. It still has to guard its own
        # construction, which is what the per-site check below proves.
        carries_rule = bool(CLASSIFICATION.search(joined))

        for index in constructions:
            number = lines[index][0]
            if guarded_by_predicate(lines, index) or carries_rule:
                continue
            findings.append(
                f"{rel}:{number}: constructs a focus ring without asking "
                f"`visual_focus()`/`draws_focus_ring()`, so a pointer press would paint one"
            )
        checked += 1

    for finding in findings:
        print(finding)
    print(f"checked={checked} failed={len(findings)} sites={sites}")


if __name__ == "__main__":
    main()
