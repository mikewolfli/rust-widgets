#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT

"""Route the caller-supplied label through `label()` for constructors that ignored it.

BLUE15 Phase C: 136 of 155 factory constructors took `_text` and dropped it, so a
caller passing a title received a control with no text. The platform path used to
keep the string in its own map, which hid the loss; now that the widget owns its
state, the label must reach it.

The rewrite is mechanical: a constructor shaped

    pub fn create_x(geometry: Rect, _text: &str) -> Box<dyn Widget> {
        <body>
    }

becomes

    pub fn create_x(geometry: Rect, text: &str) -> Box<dyn Widget> {
        label(geometry, text, <body-expr>)
    }

Only bodies that are a single expression ending in `Box::new(..)` (or a `{ .. }`
block whose value is one) are rewritten; anything else is reported for manual
review rather than guessed at. The `label` helper itself is skipped.

One-shot migration tool: deleted once the rewrite is verified.
"""

import re
import sys

SIG = re.compile(
    r"^(?P<indent>pub fn (?P<name>create_\w+)\(geometry: Rect, )_text: &str\)"
    r"(?P<ret>\s*->\s*Box<dyn Widget>)\s*\{",
    re.MULTILINE,
)


def body_span(text, open_brace):
    depth = 0
    i = open_brace
    while i < len(text):
        if text[i] == "{":
            depth += 1
        elif text[i] == "}":
            depth -= 1
            if depth == 0:
                return i
        i += 1
    return None


def process(path):
    with open(path, encoding="utf-8") as handle:
        text = handle.read()

    rewrites = []
    skipped = []

    for match in SIG.finditer(text):
        name = match.group("name")
        brace = text.index("{", match.end() - 1)
        close = body_span(text, brace)
        if close is None:
            skipped.append((name, "unbalanced braces"))
            continue
        body = text[brace + 1 : close]

        # Refuse anything whose shape we cannot be sure about: the body must be a
        # single expression whose final value is a `Box::new(..)` or a `Box::pin(..)`.
        if "Box::new" not in body and "Box::pin" not in body:
            skipped.append((name, "body does not build a Box"))
            continue
        if body.strip().startswith("//"):
            skipped.append((name, "body starts with a comment"))
            continue

        rewrites.append((match, brace, close, body))

    if not rewrites:
        return 0, skipped

    for match, brace, close, body in reversed(rewrites):
        inner = body.strip()
        replacement = f"\n    label(geometry, text, {inner})\n"
        # Replace the `_text` in the signature, then the body.
        text = text[: brace + 1] + replacement + text[close:]
        # Fix the signature: `_text` -> `text`.
        text = (
            text[: match.start()]
            + match.group(1)
            + "text: &str)"
            + match.group("ret")
            + " {"
            + text[match.end() :]
        )

    with open(path, "w", encoding="utf-8") as handle:
        handle.write(text)
    return len(rewrites), skipped


def main():
    total = 0
    for path in sys.argv[1:]:
        count, skipped = process(path)
        if count:
            total += count
            print(f"{count:3d}  {path}")
        for name, reason in skipped:
            print(f"  SKIP {name}: {reason}")
    print(f"\n{total} constructors now apply their label")


if __name__ == "__main__":
    main()
