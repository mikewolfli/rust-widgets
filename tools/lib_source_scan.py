# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Shared Rust comment stripping for the Python QA gates.

Why this module exists
----------------------
Fifteen gates each carried a private ``strip_comments`` and the copies had drifted
apart (W6-36): some blanked comments with spaces, some deleted them, some only
dropped whole-line comments and left trailing ones, and several used
``re.sub(r"//[^\\n]*", "", text)``, which also deletes ``//`` inside a string
literal. A gate that deletes text *before* it computes a line number reports the
wrong line; a gate that deletes ``//`` inside a URL reports a finding that is not
in the code.

The rule this module implements
-------------------------------
A comment is text the compiler ignores. ``//`` inside ``"..."``, ``r#"..."#`` or
``'…'`` is *text*, not a comment. Stripping removes comment characters only:

* ``keep_offsets=True`` (the default) replaces every comment character with a
  space, newlines included as themselves. Byte offsets and line numbers after the
  strip address the original file, so a regex match still names the right line —
  this is what the gates that report ``file:line`` need.
* ``keep_offsets=False`` deletes the comment characters but keeps its newlines,
  for the scans that compare token adjacency. Line numbers stay correct either
  way, which the old delete-style regexes did not guarantee for block comments.

Block comments (``/* … */``) are stripped too, because several gates' subjects
contain Rustdoc prose that names a construct; a doc mention must not satisfy a
code check. Nested block comments (``/* /* */ */``) are not modelled — Rust's
nesting rule is rare in this tree and a wrong answer there is still a *comment*
being removed, never code.

What this module does not understand
------------------------------------
Raw strings with more than 255 leading hashes, and macro bodies whose tokens are
not Rust. Those are properties of a full lexer; every caller here scans ordinary
source files.
"""

from __future__ import annotations

import sys


def _skip_quoted(text: str, start: int, quote: str) -> int:
    """Index just past the closing ``quote`` of a literal opened at ``start``."""
    index = start + 1
    length = len(text)
    while index < length:
        char = text[index]
        if char == "\\":
            index += 2
            continue
        if char == quote:
            return index + 1
        index += 1
    return length


def _raw_string_end(text: str, quote_index: int, hashes: int) -> int:
    """End index of a raw string whose opening quote is at ``quote_index``.

    The terminator is a quote followed by exactly ``hashes`` ``#`` characters, so
    ``r#"…"#`` keeps every bare ``"`` in its body as text.
    """
    terminator = '"' + ("#" * hashes)
    end = text.find(terminator, quote_index + 1)
    return len(text) if end == -1 else end + len(terminator)


def _raw_opener(text: str, index: int) -> tuple[int, int]:
    """``(length, hashes)`` of a raw-string opener at ``index``, or ``(0, 0)``.

    Handles ``r"…"``, ``r#"…"#`` and the ``b``/``c`` prefixed spellings. The ``r``
    needs a word boundary, or ``error``/``for``/``rgb`` would be read as openers.
    """
    if text[index] == "r":
        prefix_end = index + 1
    elif text[index] in ("b", "c") and index + 1 < len(text) and text[index + 1] == "r":
        prefix_end = index + 2
    else:
        return 0, 0
    if index > 0 and (text[index - 1].isalnum() or text[index - 1] == "_"):
        return 0, 0
    cursor = prefix_end
    hashes = 0
    while cursor < len(text) and text[cursor] == "#":
        hashes += 1
        cursor += 1
    if cursor < len(text) and text[cursor] == '"':
        return cursor - index + 1, hashes
    return 0, 0


def _comment_spans(text: str) -> list[tuple[int, int]]:
    """``(start, end)`` of every comment in ``text``, outside literals."""
    spans: list[tuple[int, int]] = []
    index = 0
    length = len(text)
    while index < length:
        char = text[index]

        if char == "/" and index + 1 < length and text[index + 1] == "/":
            end = text.find("\n", index)
            end = length if end == -1 else end
            spans.append((index, end))
            index = end
            continue

        if char == "/" and index + 1 < length and text[index + 1] == "*":
            end = text.find("*/", index + 2)
            end = length if end == -1 else end + 2
            spans.append((index, end))
            index = end
            continue

        if char == '"':
            index = _skip_quoted(text, index, '"')
            continue

        if char in ("r", "b", "c"):
            opener, hashes = _raw_opener(text, index)
            if opener:
                index = _raw_string_end(text, index + opener - 1, hashes)
                continue

        if char == "'":
            following = text[index + 1] if index + 1 < length else ""
            # A backslash-quoted char, or one character between quotes. Anything
            # else is a lifetime such as `&'a str`, which does not open a literal.
            if following == "\\":
                index = _skip_quoted(text, index, "'")
                continue
            if index + 2 < length and text[index + 2] == "'":
                index += 3
                continue
            index += 1
            continue

        index += 1
    return spans


def strip_comments(text: str, *, keep_offsets: bool = True) -> str:
    """Return ``text`` with its comments removed.

    ``keep_offsets`` (default) replaces comment characters with spaces so every
    offset and line number still points at the same place in the original text.
    Passing ``False`` deletes them, keeping newlines.
    """
    pieces: list[str] = []
    last = 0
    for start, end in _comment_spans(text):
        pieces.append(text[last:start])
        chunk = text[start:end]
        if keep_offsets:
            pieces.append("".join("\n" if char == "\n" else " " for char in chunk))
        else:
            pieces.append("".join(char for char in chunk if char == "\n"))
        last = end
    pieces.append(text[last:])
    return "".join(pieces)


def strip_line_comment(line: str) -> str:
    """``line`` cut at its first ``//`` comment that is outside a string literal.

    The line-level callers scan code line by line and only ever need the code part
    of one line, so this returns the prefix rather than a same-length blank.
    """
    for start, _end in _comment_spans(line):
        if line.startswith("//", start):
            return line[:start]
    return line


def selftest() -> int:
    """Checks the two properties every caller relies on.

    Run directly (``python tools/lib_source_scan.py``) so a regression in the
    stripper is visible without running a gate whose verdict it would distort.
    """
    failures = 0

    def check(name: str, got: object, want: object) -> None:
        nonlocal failures
        if got != want:
            failures += 1
            print(f"FAIL {name}: got {got!r}, want {want!r}")

    source = 'let url = "http://example"; // trailing\n// whole line\nlet c = \'/\';\n'
    stripped = strip_comments(source)
    check("line count preserved", stripped.count("\n"), source.count("\n"))
    check("string url preserved", '"http://example"' in stripped, True)
    check("trailing comment removed", "trailing" in stripped, False)
    check("whole line blanked", stripped.splitlines()[1].strip(), "")
    check("char literal slash preserved", "let c = '/';" in stripped, True)

    block = "let a = 1; /* Shadow { x: 0 } */\nlet b = 2; // ok\n"
    check("block comment removed", "Shadow" in strip_comments(block), False)
    check("code after block kept", "let b = 2;" in strip_comments(block), True)

    deleted = strip_comments("a /* x\ny */ b\n", keep_offsets=False)
    check("delete keeps newline", deleted.count("\n"), 2)

    raw = 'let r = r#"a // not a comment"#; // yes\n'
    check("raw string preserved", 'r#"a // not a comment"#' in strip_comments(raw), True)
    check("raw trailing removed", "yes" in strip_comments(raw), False)

    line = 'x = 1; // comment "quoted // still comment"'
    check("line helper", strip_line_comment(line), "x = 1; ")
    check("line helper string", strip_line_comment('let u = "//";'), 'let u = "//";')

    if failures:
        print(f"lib_source_scan: {failures} failure(s)")
        return 1
    print("lib_source_scan: OK")
    return 0


if __name__ == "__main__":
    sys.exit(selftest())
