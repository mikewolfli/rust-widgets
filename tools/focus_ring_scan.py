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

# (c) The default-button emphasis ring. `Button::draw` paints a second ring for `is_default()` --
#     the on-screen cue that Enter activates this button -- and that ring is not focus-driven at
#     all: it must stay visible on a pointer-focused window as much as on a Tab-focused one.
#     The scan matches the shared constructor, so this site is allowed only when it sits inside
#     an `if` on `is_default()`. Anchoring to that shape is what keeps the exemption from
#     becoming a file-level allowlist that would also hide a real focus-ring defect.
EMPHASIS = re.compile(r"\bis_default\s*\(")

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
    """Whether the ring construction at `lines[index]` sits inside an `if` on an allowed guard.

    The search walks **outwards** through the construction's lexical block structure: braces are
    counted backwards from the construction, so every step to depth 0 lands on the `{` that opens
    a block containing it. Nested blocks (`else` arms, `if let` arms) are crossed without letting
    their headers count, which is what a fixed-line window could not do -- a legitimate guard can
    sit more than twelve lines above its construction once a branch and its comments intervene.

    Braces are counted rather than statements parsed: the tree's draw sites are short and direct,
    and a full parser would be a lot of machinery for one rule. Because the walk only ever
    considers blocks that lexically **contain** the construction, a guard belonging to a sibling
    construct can never satisfy this one -- which is the property the old window only approximated.
    """
    depth = 0
    previous_code = ""
    for _, text in reversed(lines[:index]):
        code = text.split("//", 1)[0]
        stripped = code.strip()
        if not stripped:
            continue
        opens = stripped.count("{")
        closes = stripped.count("}")
        if depth == 0 and opens:
            # Blocks whose brace sits alone on its own line take their header from the nearest
            # earlier code line; rustfmt keeps the shared style (`if cond {`) in this tree.
            header = stripped.split("{", 1)[0].strip() or previous_code
            if (PREDICATE.search(header) or EMPHASIS.search(header)) and (
                "if" in header or "match" in header
            ):
                return True
        depth += closes - opens
        if depth < 0:
            # A macro or a brace inside a string balanced the walk early; keep climbing out so a
            # real guard further up is still found. The direction of error stays a false positive.
            depth = 0
        previous_code = stripped
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
