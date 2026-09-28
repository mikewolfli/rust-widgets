#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# The rule this implements is stated in full — what it proves, what it does not, and the reverse
# injection that keeps it honest — in tools/check_icons_not_symbol_glyphs.sh, which is the only
# caller. That file is the documentation of record.
#
# A control that wants a tick, a chevron, an arrow or a dot must draw an **`IconName` outline**,
# never a Unicode symbol character handed to `draw_text`. Sending a character instead has failed
# twice, the same way both times: no bundled face covers the geometric-shapes / arrows / dingbats
# blocks, so the glyph fell through the outline path to an 8x8 **bitmap** and the control painted a
# coarse blob. The character also carries no geometry contract, so nothing asserted on it.
#
# Prints one `finding: ...` line per problem, then a `scanned=N failed=M` summary line.
# Exit status is always 0; the shell wrapper makes the pass/fail judgement.

import argparse
import pathlib
import re
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent

# The Unicode blocks a "symbol icon" is drawn from. These are the blocks no shipped outline face
# covers, which is what makes them degrade to bitmaps. Ordered ranges are inclusive.
SYMBOL_BLOCKS = [
    (0x2000, 0x206F, "General Punctuation"),
    (0x2190, 0x21FF, "Arrows"),
    (0x2200, 0x22FF, "Mathematical Operators"),
    (0x2300, 0x23FF, "Miscellaneous Technical"),
    (0x2460, 0x24FF, "Enclosed Alphanumerics"),
    (0x25A0, 0x25FF, "Geometric Shapes"),
    (0x2600, 0x27BF, "Miscellaneous Symbols and Dingbats"),
    (0x2B00, 0x2BFF, "Miscellaneous Symbols and Arrows"),
    (0x1F000, 0x1FAFF, "Emoji / pictographs"),
]

# The `draw_text` family — the calls that hand a character to the glyph layer.
DRAW_CALL = re.compile(r"\bdraw_text(?:_fitted)?\s*\(")

# A string literal, honoring the backslash escapes a Rust literal may carry.
STRING_LITERAL = re.compile(r'"((?:[^"\\]|\\.)*)"')

# A `\u{XXXX}` escape, which is how a literal avoids writing the character directly.
UNICODE_ESCAPE = re.compile(r"\\u\{([0-9A-Fa-f]+)\}")

# Characters that are legitimately drawn as text even though they sit in a symbol block. Each
# entry must be justified: the character is a **typographic mark** in a text run the control is
# genuinely rendering (a truncation ellipsis, a host-supplied separator, a locale's arrow), not a
# stand-in for an icon. An entry is (codepoint, reason). Adding one is a deliberate act.
ALLOWED_TEXT_MARKS = {
    0x2026: "ellipsis: a truncation mark the fitted-text helper appends, not an icon",
    0x2014: "em dash: prose punctuation in messages and logs",
    0x2013: "en dash: prose punctuation",
    0x2018: "left single quote: prose punctuation",
    0x2019: "right single quote: prose punctuation",
    0x201C: "left double quote: prose punctuation",
    0x201D: "right double quote: prose punctuation",
}

# The one file permitted to *name* the drawing call inside a doc example. Nothing in production
# should need this; kept empty so the escape hatch is explicit if a real case appears.


def block_for(codepoint):
    for low, high, name in SYMBOL_BLOCKS:
        if low <= codepoint <= high:
            return name
    return None


def line_is_comment(line):
    stripped = line.lstrip()
    return stripped.startswith(("//", "*", "/*"))


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--src", default=str(REPO_ROOT / "src"))
    parser.add_argument(
        "--inject",
        action="append",
        default=[],
        help="an extra file to scan, to prove the scan looks (repeatable)",
    )
    args = parser.parse_args(argv)

    files = sorted(pathlib.Path(args.src).rglob("*.rs"))
    files.extend(pathlib.Path(p) for p in args.inject)

    findings = []
    scanned = 0
    for path in files:
        if not path.exists():
            continue
        # Test modules and the test files render fixtures, not controls, and their literals describe
        # a *document* rather than a control's chrome. The rule is about what ships.
        relative = path.relative_to(REPO_ROOT).as_posix() if path.is_relative_to(REPO_ROOT) else path.as_posix()
        if "/tests.rs" in relative or relative.endswith("/tests.rs"):
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        # A `#[cfg(test)] mod tests` block starts near a `mod tests`; skip from there to the end.
        cut = text.find("\nmod tests ")
        scannable = text if cut == -1 else text[:cut]
        scanned += 1
        for lineno, message in _scan_text(relative, scannable):
            findings.append(message)

    for finding in findings:
        print(f"finding: {finding}")
    print(f"scanned={scanned} failed={len(findings)}")
    return 0


def _scan_text(relative, text):
    for match in DRAW_CALL.finditer(text):
        start = match.start()
        lineno = text[:start].count("\n") + 1
        line_text = text.splitlines()[lineno - 1] if lineno - 1 < len(text.splitlines()) else ""
        if line_is_comment(line_text):
            continue
        window = text[start : start + 600]
        for literal in STRING_LITERAL.finditer(window):
            value = literal.group(1)
            candidates = list(value)
            candidates.extend(chr(int(m.group(1), 16)) for m in UNICODE_ESCAPE.finditer(value))
            for ch in candidates:
                block = block_for(ord(ch))
                if block is None or ord(ch) in ALLOWED_TEXT_MARKS:
                    continue
                yield lineno, (
                    f"{relative}:{lineno}: draws U+{ord(ch):04X} {ch!r} ({block}) as text; use an "
                    f"IconName outline (crate::widget::draw_icon_at) instead — no bundled face "
                    f"covers {block}, so the glyph degrades to an 8x8 bitmap"
                )


if __name__ == "__main__":
    sys.exit(main())
