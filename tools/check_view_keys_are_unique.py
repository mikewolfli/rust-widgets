#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Declarative key-uniqueness gate (BLUE18 rule #88).

# What the rule is

A `Node`'s `key` is what lets a diff recognise a control across rebuilds. Two
siblings claiming the same key make identity *ambiguous*: a diff could never move
or remove either one unambiguously, and the resulting patch would be applied to an
arbitrary one of them. So keys must be unique among siblings.

# Why the check is static

The ambiguity is a property of the *tree the view builds*, and `View::build` is an
arbitrary function — it may branch on state, and a given run only ever exercises
the branches it takes. Running a view and walking its output therefore proves
nothing about the branches that run did not take; a duplicate key in a branch that
only fires on an empty list would pass every test and still corrupt identity in
production.

So the gate reads the *construction sites*: every `Node::new(..).key("...")` in a
`#[cfg(test)]` block or in `src/view/`'s own tests, grouped by the scope that
contains it, and reports a key that appears twice with the same sibling scope.

# What counts as a sibling scope

Two keys are siblings when they are attached to the same parent expression. The
gate approximates that with the enclosing *statement chain*: a chain of `.child(..)`
/ `.children_of(..)` calls off one `Node::new(..)` is one sibling set, and keys in
different chains are in different sets. That is the same grouping a reader uses, and
it is the one that matters: a duplicate inside one chain is a real collision.

A duplicate *across* chains is not reported, because the two chains may well be
different subtrees. That deliberately makes this gate a floor rather than a proof —
it catches the mistake people actually make (pasting a `.key("x")` twice in one
builder chain) without inventing failures for trees it cannot see.

# Known limitation, stated rather than hidden

A key built from a loop variable (`format!("row-{i}")`) is not analysed as a
literal; it is recorded as dynamic and skipped, because deciding whether a format
string can collide needs the loop bounds. A `key(..)` call with a non-literal
argument is reported in the summary as dynamic so the reader knows the check was
partial for that node, rather than believing it was total.

Exit 0 = no literal duplicate sibling key. Exit 1 = at least one was found.
"""

from __future__ import annotations

import pathlib
import re
import sys
from collections import defaultdict

# Where `Node` construction is written. `src/view`'s own tests are included because
# the module's examples are the ones most likely to be copied into a real view.
SEARCH_ROOTS = ("src", "examples", "tests", "tools", "demo")

# Function scopes whose duplicates are the *subject* of the test rather than a
# mistake in it. These are the tests that verify duplicate detection: they must
# build a collision, so reporting one here would make the gate unable to pass while
# the capability it guards is itself tested.
#
# Listed by name rather than by file so that adding the next such test means adding
# one entry here, which is a visible decision rather than a silent exception.
INTENTIONAL_DUPLICATE_TESTS = {
    "duplicate_sibling_keys_are_reported":
        "asserts that a duplicate key is reported, so it must build one",
    "duplicate_report_covers_only_direct_children":
        "same key in two different sibling sets must be legal, so it must build one",
    "duplicate_report_is_not_confused_by_grandchildren":
        "asserts a grandchild key does not collide with its parent's sibling",
}

# `Node::new("button")` … optionally chained `.key("ok")`.
NODE_NEW_RE = re.compile(r"Node::new\s*\(")

# `.key("literal")` — a string literal argument.
KEY_LITERAL_RE = re.compile(r"\.key\s*\(\s*\"(?P<key>[^\"]*)\"\s*\)")

# `.key(expression)` — anything else, i.e. a key the gate cannot evaluate.
KEY_DYNAMIC_RE = re.compile(r"\.key\s*\(\s*(?!\")")


def rust_files() -> list[pathlib.Path]:
    """Every `.rs` file under the search roots that exist."""
    out: list[pathlib.Path] = []
    for root in SEARCH_ROOTS:
        base = pathlib.Path(root)
        if base.is_dir():
            out.extend(sorted(base.rglob("*.rs")))
    return out


def chain_spans(text: str) -> list[tuple[int, int]]:
    """Byte spans of `Node::new(..)` builder chains, one per chain.

    A chain runs from a `Node::new(` to the end of that statement.

    # Why this walks the chain deliberately rather than searching for a terminator

    A naive "scan to the next `;`" merges adjacent statements: in

        let a = Node::new("x").key("k");
        let b = Node::new("y").key("k");

    the first chain would swallow the second, and a *legal* pair of same-key nodes
    in two different trees would be reported as one collision. A second version that
    stopped at any newline at depth zero cut real chains in half, because
    `.child(` is commonly written at the start of the next line.

    So this follows the grammar instead: after the constructor's argument list
    closes, consume only method calls — optional whitespace/newlines, then `.`,
    a name, and the call's own bracket pair — and stop at the first character that
    is not the start of another call in the same chain.
    """
    spans: list[tuple[int, int]] = []
    for match in NODE_NEW_RE.finditer(text):
        index = _skip_bracketed_group(text, match.end() - 1)

        # Phase 2: follow `.method( .. )` calls, one at a time.
        while True:
            probe = _skip_trivia(text, index)
            if probe >= len(text) or text[probe] != ".":
                break
            # A `.` must be followed by a name and then an opening bracket, and the
            # name must be an identifier (not `..` from a range expression).
            name_end = probe + 1
            while name_end < len(text) and (text[name_end].isalnum() or text[name_end] == "_"):
                name_end += 1
            if name_end == probe + 1:
                break
            call_open = _skip_trivia(text, name_end)
            if call_open >= len(text) or text[call_open] not in "([":
                # A field access, not a call: not part of a builder chain, and not
                # something this gate needs to read into.
                break
            index = _skip_bracketed_group(text, call_open)

        spans.append((match.start(), index))
    return spans


def _skip_trivia(text: str, index: int) -> int:
    """Advance past whitespace and line/block comments."""
    while index < len(text):
        char = text[index]
        if char in " \t\n\r":
            index += 1
        elif char == "/" and text.startswith("//", index):
            newline = text.find("\n", index)
            index = len(text) if newline < 0 else newline + 1
        elif char == "/" and text.startswith("/*", index):
            end = text.find("*/", index + 2)
            index = len(text) if end < 0 else end + 2
        else:
            return index
    return index


def _skip_bracketed_group(text: str, open_index: int) -> int:
    """Return the index just past the bracket group starting at `open_index`.

    Handles nested and mismatched bracket types, string literals, and char
    literals, so a `)` inside a string cannot close the group early. Returns
    `len(text)` when the group is unterminated.
    """
    open_char = text[open_index]
    close_char = {"(": ")", "[": "]", "{": "}"}[open_char]
    depth = 0
    index = open_index
    while index < len(text):
        char = text[index]
        if char == '"':
            index = _skip_string(text, index)
            continue
        if char == "'":
            # A lifetime (`'a`) or a char literal; only the latter has a closing
            # quote, and skipping to the next `'` handles both without eating code.
            nxt = text.find("'", index + 1)
            index = index + 1 if nxt < 0 else nxt + 1
            continue
        if char in "([{":
            depth += 1
        elif char in ")]}":
            depth -= 1
            if depth == 0:
                return index + 1
        index += 1
    del close_char
    return len(text)


def _skip_string(text: str, quote_index: int) -> int:
    """Index just past the string literal starting at `quote_index`."""
    index = quote_index + 1
    while index < len(text):
        if text[index] == "\\":
            index += 2
            continue
        if text[index] == '"':
            return index + 1
        index += 1
    return len(text)


def enclosing_scope(text: str, position: int) -> str:
    """A stable identifier for the function/test containing `position`.

    Used only for reporting, so a reader can find the collision. The nearest
    preceding `fn name(` is enough for that.
    """
    prefix = text[:position]
    names = re.findall(r"(?:fn|macro_rules!)\s+(\w+)", prefix)
    return names[-1] if names else "<file scope>"


def main() -> int:
    duplicates: list[tuple[str, str, str, int]] = []
    dynamic: list[tuple[str, str]] = []
    literal_chains = 0
    excused = 0

    for path in rust_files():
        try:
            text = path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            continue
        if "Node::new" not in text:
            continue

        for start, end in chain_spans(text):
            chain = text[start:end]
            keys = KEY_LITERAL_RE.findall(chain)
            if keys:
                literal_chains += 1
            counts: dict[str, int] = defaultdict(int)
            for key in keys:
                counts[key] += 1
            scope = enclosing_scope(text, start)
            line = text.count("\n", 0, start) + 1
            for key, count in counts.items():
                if count > 1:
                    if scope in INTENTIONAL_DUPLICATE_TESTS:
                        excused += 1
                        continue
                    duplicates.append((str(path), scope, key, line))
            if KEY_DYNAMIC_RE.search(chain) and not keys:
                dynamic.append((str(path), scope))

    print("declarative key-uniqueness gate (BLUE18 rule #88)")
    print(f"  builder chains with literal keys checked: {literal_chains}")
    print(f"  intentional duplicates in detection tests (excused): {excused}")
    if dynamic:
        print(f"  chains with a computed key (not checkable, reported): {len(dynamic)}")
        for file, scope in dynamic[:10]:
            print(f"    · {file}::{scope}")
        if len(dynamic) > 10:
            print(f"    · … and {len(dynamic) - 10} more")

    if duplicates:
        print("")
        print("❌ duplicate sibling keys found:")
        for file, scope, key, line in duplicates:
            print(f"   {file}:{line}  {scope}  key={key!r}")
        print("")
        print("   Two siblings with the same key make a diff unable to tell them")
        print("   apart, so a Move or Remove would be applied to an arbitrary one.")
        return 1

    print("")
    print("✅ no duplicate sibling key in any checked builder chain")
    return 0


if __name__ == "__main__":
    sys.exit(main())
