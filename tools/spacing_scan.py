#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# The rule this implements is stated in full — what it proves, what it does not, and the two
# legitimate readings of `spacing` — in tools/check_spacing_is_not_sibling_layout.sh, which is the
# only caller. That file is the documentation of record.
#
# Prints one `file:line: ...` finding per offender, then a single `checked=N failed=M` summary line.
# Exit status is always 0: the shell wrapper makes the pass/fail judgement.

import pathlib
import re

# ── What `Style::spacing` is allowed to mean ────────────────────────────────────────────────────
# `Style::spacing` (src/style/primitives.rs) is documented as **the distance from a control's own
# indicator to its own text** — a fact about one control's contents. The distance between two
# siblings is the *layout's* fact (`FlexLayout::gap`). Writing the same field into both roles is
# what made the crate unable to change either safely, so this gate names the two legitimate
# readings and flags everything else.

# A read of the *style field*, in the two spellings the tree uses. The bare word `spacing` is not
# enough and must not be used: `let spacing = TAB_SPACING`, `"spacing" => ...` (a capability key)
# and `pub fn set_bar_spacing(&mut self, spacing: f32)` are three different things that share the
# word. Matching `style().spacing` / `self.style().spacing` keeps the scan on the field.
SPACING_FIELD_READ = re.compile(r"\bstyle\(\)\s*\.\s*spacing\b")


# The accessor that reads `Style::spacing` for the indicator-to-text role. This is the claim a
# control makes in place, next to the derivation it protects — not an entry in a side table.
OWN_GAP_ACCESSOR = re.compile(r"\bfn\s+label_gap\b|\blabel_gap\s*\(\s*\)")


def production_lines(path):
    """Returns [(line_number, text)] for lines outside a `mod tests` block.

    Test modules are excluded because a test that *sets* a spacing to observe the result is the
    fixture for an assertion, not a consumer in production.
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


def main():
    checked = 0
    findings = []
    consumers = 0

    for path in sorted(pathlib.Path("src/widget").rglob("*.rs")):
        rel = path.as_posix()
        lines = production_lines(path)
        reads = [
            (number, text)
            for number, text in lines
            if SPACING_FIELD_READ.search(text) and not text.strip().startswith("//")
        ]
        if not reads:
            checked += 1
            continue

        consumers += 1
        joined = "\n".join(text for _, text in lines)
        if OWN_GAP_ACCESSOR.search(joined):
            checked += 1
            continue

        for number, text in reads:
            findings.append(
                f"{rel}:{number}: reads `Style::spacing` without a `label_gap` accessor, so the "
                f"field is not confined to the indicator-to-text role"
            )
        checked += 1

    for finding in findings:
        print(finding)
    print(f"checked={checked} failed={len(findings)}")


if __name__ == "__main__":
    main()
