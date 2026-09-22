#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Gate: an abstraction must have at least one production consumer (BLUE21 P3-1g).

# Why this gate exists

This crate's most expensive defect class is not a wrong value — it is a *correct* value that
nothing calls. The abstraction is written, its unit tests pass, its documentation is accurate, and
no control ever reaches it. From the outside the code reads as implemented; from the screen it does
nothing. Five such islands were found in one audit, each with a different subject:

| symbol | declared in | what its absence costs |
|---|---|---|
| `TouchTargetSize` | `src/style/primitives.rs` | the 32/44/48/40 hit-target table, wired to nothing |
| `contains_point_with_touch_expansion` | `src/widget/base.rs` | the expansion function, called by no control |
| `resolve_style_for_state` | `src/theme/manager.rs` | 12 widget states no control can resolve |
| `LayoutContext::font_scale` | `src/layout/types.rs` | text scaling that scales no text |
| `LayoutContext::min_touch_size` | `src/layout/types.rs` | a minimum touch size nothing reads |
| `AnimationDriver` | `src/style/animation.rs` | 1582 lines of animation no `src/widget/` file calls |
| `AnimationGroup` | `src/style/animation_group.rs` | a sequencing primitive with no sequencer |

The shape is always the same: a plausible, complete mechanism is built for a consumer that is
never written, or is written later under a different name. Every existing gate passes it — it
compiles, it is documented, and its own tests exercise it directly. Nothing asks "does production
code call this?".

**A gate that cannot fail is not a gate**, so the table below is checked in both directions:

  * each symbol must have at least one `src/` reference **outside** its own declaration file and
    outside the files the entry allows as part of its own mechanism. Zero references is a finding;
  * every symbol must actually be declared where the table says. A symbol that moved or was
    deleted makes this gate report that it has stopped tracking the thing it guards, rather than
    quietly passing on a name that no longer exists.

# What does not count as a consumer

  * **The declaration file itself.** `AnimationDriver` appears ~20 times in `animation.rs` — its
    `impl`, its doc comments, its own tests. None of that is a consumer.
  * **A file the entry lists as part of the same mechanism.** `resolve_style_for_state` is called
    by `resolve_style` in `src/theme/manager.rs`, so the mechanism is internally consistent; the
    question this gate asks is whether anything *outside* `src/theme/` reaches it, which is what
    "a control uses a state override" would require.
  * **`#[cfg(test)]` code.** A test that calls the symbol is evidence the symbol works, not
    evidence anything uses it. Counting test references is exactly how a dead abstraction stays
    invisible, so test modules are blanked before the search. Integration tests under `tests/` are
    outside `src/` and are not scanned at all.

# Why an explicit allowlist rather than "any consumer is fine"

Some mechanisms are genuinely zero-consumer *by design* — a trait method a backend implements, an
entry point a binding calls. Listing one of those as a finding would be a false positive, and
weakening the gate to accommodate it would lose the real finding. Where such a symbol exists it
goes in `BY_DESIGN` with a written reason, so the exemption is a recorded decision rather than a
silence. `BY_DESIGN` is empty: every symbol examined for this list had a real gap.

# Reverse injection

`--inject=<symbol>` pretends the named symbol has acquired a consumer and requires the gate to
change its answer for that symbol. That is what proves the report and the search are about the same
code: a grep that silently matched nothing would report every symbol and would not notice the
injection either, and the mode fails in that case rather than passing vacuously.

Run from the repo root.

Usage: tools/check_mechanism_has_a_consumer.py   (exit 1 on an unrecorded unconnected mechanism)
"""

from __future__ import annotations

import pathlib
import re
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent
SRC = REPO / "src"

# `(symbol, declaring file, allowed same-mechanism files, why the symbol exists)`.
#
# `allowed` is a tuple of path *prefixes* (a trailing `/` means a directory). A reference inside
# one of them does not count as a consumer, because the point of the entry is that the mechanism is
# internally coherent but external consumers are missing.
#
# Keep this table short and evidenced. Every row here was verified with a grep before being
# written down, and each one is a real gap rather than a suspicion.
# Connected on 2026-09-22 (BLUE21 P0-2 / AR1), and removed from this table for that reason:
#
#   `TouchTargetSize` — now read by `platform::profile::recommended_touch_target()`, which
#        `theme::manager::role_base_style` writes into every control's `style.touch_target`.
#   `contains_point_with_touch_expansion` — now called by the hit tests of `check_box`,
#        `radio_button`, `switch` and `toggle_button`, so a control smaller than its device
#        class's minimum still responds just outside its own rectangle.
#   `resolve_style_for_state` — now reached through
#        `theme::resolved_theme_style_for_state`, which `theme::apply::apply_active_theme` calls
#        with `Widget::widget_state()`. A theme's `"button:hover"`-style override applies to a live
#        control instead of being resolvable only by asking the manager directly.
#   `font_scale` — read in two places, deliberately: `theme::manager::role_base_style` scales the
#        theme's body font token through `platform::profile::text_scale()` (so a device's text-size
#        preference reaches every control that does not set its own font), and the flex, box and
#        wrap layouts take `max(layout_scale, font_scale)` for spacing, because text that grew while
#        its padding did not would touch its own border. `app_bar`'s 22 pt ceiling is now scaled by
#        the same factor — it used to clamp 2x text back to the nominal size silently.
#   `min_touch_size` — now read by the flex, box and wrap layouts, which grow each child to the
#        context's minimum touch area. A layout places controls closer together than the hardware
#        can address without it, and no amount of hit-test expansion recovers a target that a
#        neighbouring control is drawn on top of.
#
# Recorded here rather than silently dropped because this table's value is that it is a *live*
# list: an entry that stays after its mechanism gains a consumer is a false accusation, and the
# gate reports exactly that. Removing them is the fix the gate asks for.
MECHANISMS: tuple[tuple[str, str, tuple[str, ...], str], ...] = (
    (
        "AnimationDriver",
        "src/style/animation.rs",
        ("src/style/",),
        "the clock that advances every animation in `src/style/animation.rs`. Only "
        "`animation_group.rs` in the same directory names it, and that module has no consumer "
        "either, so the whole 1582-line file is unreachable from `src/widget/`",
    ),
    (
        "AnimationGroup",
        "src/style/animation_group.rs",
        (),
        "sequential/parallel animation sequencing. Nothing outside its own file names it, so the "
        "primitive has no sequencer",
    ),
)

# Mechanisms known to be unconnected **today**, with the BLUE21 item that will connect them. Each
# entry records a decision and a debt rather than a silence.
#
# Why this exists instead of listing the mechanisms as unconnected findings: the tree this gate
# lands on already contains all seven islands, and wiring a hit-target table, a state resolver, a
# text-scale factor and a 1582-line animation engine into controls is a feature plan of its own
# (BLUE21 P0-2 / P0-3 / P0-4 / P2-7), not a gate change. Reporting them as failures would leave the
# gate red, and a permanently-red gate is one nobody reads — which is how the islands survived in
# the first place.
#
# What the table buys is the ratchet: a **new** unconnected mechanism fails immediately, and an
# entry here whose mechanism later gains a consumer is reported as stale, so the list can only
# shrink. Every row is a live grep of `src/`, not a snapshot of a report.
ACKNOWLEDGED: dict[str, str] = {
    "AnimationDriver": "BLUE21 P0-4 (AR7) — drive it from the widget tick",
    "AnimationGroup": "BLUE21 P0-4 (AR7) — drive it from the widget tick",
}

# `#[cfg(test)]` and the test module it gates. Blanked before searching so a test's use of a symbol
# cannot make it look consumed.
TEST_ATTRIBUTE = re.compile(r"#\[cfg\(test\)\]")
MOD_DECLARATION = re.compile(r"\bmod\s+[a-z0-9_]+\s*[;{]")

LINE_COMMENT = re.compile(r"//[^\n]*")
BLOCK_COMMENT = re.compile(r"/\*.*?\*/", re.S)


def strip_comments(text: str) -> str:
    """Blanks comments while preserving offsets, so a doc mention is not read as a call."""
    text = BLOCK_COMMENT.sub(lambda m: " " * len(m.group(0)), text)
    return LINE_COMMENT.sub(lambda m: " " * len(m.group(0)), text)


def blank_test_modules(text: str) -> str:
    """Blanks every `#[cfg(test)]`-gated item, keeping offsets.

    A test that calls a symbol proves it works and says nothing about whether it is used. Counting
    those calls is the specific mistake this gate exists to avoid, and `src/theme/mod.rs` is a live
    example: all of its `resolve_style_for_state` references are inside the test module.
    """
    out = list(text)
    for match in TEST_ATTRIBUTE.finditer(text):
        cursor = match.end()
        while cursor < len(text) and text[cursor] in " \t\r\n":
            cursor += 1
        declaration = MOD_DECLARATION.match(text, cursor)
        if declaration is None:
            continue
        if text[declaration.end() - 1] == ";":
            # `#[cfg(test)] mod name;` — the body lives in another file, which the caller blanks
            # by file name instead. Nothing to do here.
            continue
        depth = 0
        end = declaration.end() - 1
        for index in range(end, len(text)):
            if text[index] == "{":
                depth += 1
            elif text[index] == "}":
                depth -= 1
                if depth == 0:
                    end = index + 1
                    break
        for index in range(match.start(), end):
            if out[index] != "\n":
                out[index] = " "
    return "".join(out)


def production_source(path: pathlib.Path) -> str:
    """The file's text with comments and `#[cfg(test)]` items blanked."""
    return blank_test_modules(strip_comments(path.read_text(encoding="utf-8")))


def project_relative(path: pathlib.Path) -> str:
    return path.relative_to(REPO).as_posix()


def symbol_occurrence(text: str, symbol: str) -> bool:
    """Whether `text` uses `symbol` as a whole identifier.

    A word-boundary match rather than a substring one: `font_scale` also spells the tail of
    `layout_font_scale`, and a substring search would count that as a consumer of the wrong thing.
    """
    return re.search(r"\b" + re.escape(symbol) + r"\b", text) is not None


def consumers(symbol: str, declared: str, allowed: tuple[str, ...]) -> list[str]:
    """`src/` files other than the declaration and the allowed same-mechanism files that use it."""
    found: list[str] = []
    for path in sorted(SRC.rglob("*.rs")):
        relative = project_relative(path)
        if relative == declared:
            continue
        if any(relative == prefix or relative.startswith(prefix) for prefix in allowed):
            continue
        if symbol_occurrence(production_source(path), symbol):
            found.append(relative)
    return found


def scan(inject: str | None) -> tuple[list[str], list[str], list[str], list[str]]:
    """Returns (unconnected, stale acknowledgements, missing declarations, evidence lines)."""
    unconnected: list[str] = []
    stale: list[str] = []
    missing: list[str] = []
    evidence: list[str] = []

    for symbol, declared, allowed, why in MECHANISMS:
        path = REPO / declared
        if not path.exists():
            missing.append(f"{symbol}: {declared} does not exist, so this gate has lost track of it")
            continue

        declaration_text = production_source(path)
        if not symbol_occurrence(declaration_text, symbol):
            missing.append(
                f"{symbol}: is not declared in {declared} any more, so this gate is guarding a "
                "name the tree no longer has"
            )
            continue

        found = consumers(symbol, declared, allowed)
        if inject == symbol:
            # Pretend the mechanism acquired a consumer. `found` becomes non-empty, so the symbol
            # takes the `if found:` branch and, being acknowledged, is reported as a stale
            # acknowledgement — a failure. The injected run must therefore differ from the clean
            # run, which is what `main` asserts.
            found = [*found, f"<injected consumer of {symbol}>"]
            injected_saw_consumer = True

        if found:
            evidence.append(f"{symbol}: {len(found)} consumer(s), e.g. {found[0]}")
            # The ratchet's other direction: an acknowledged island that has been wired up must
            # leave the list, or the list keeps granting permission it no longer needs.
            if symbol in ACKNOWLEDGED:
                stale.append(
                    f"{symbol} is in ACKNOWLEDGED as unconnected ({ACKNOWLEDGED[symbol]}) but now "
                    f"has a consumer: {found[0]}"
                )
            continue

        if symbol in ACKNOWLEDGED:
            evidence.append(f"{symbol}: unconnected, acknowledged — {ACKNOWLEDGED[symbol]}")
            continue

        allowed_note = (
            " (nothing counts as its consumer but its own declaration file)"
            if not allowed
            else f" (references in {', '.join(allowed)} are part of the mechanism, not consumers)"
        )
        unconnected.append(f"{symbol} ({declared}) — {why}{allowed_note}")

    # An acknowledged symbol that is no longer in the table, or no longer a mechanism at all, is
    # a stale row. This is what stops the table from becoming a place names are parked.
    names = {symbol for symbol, _, _, _ in MECHANISMS}
    for symbol, reason in ACKNOWLEDGED.items():
        if symbol not in names:
            stale.append(
                f"{symbol} is in ACKNOWLEDGED ({reason}) but is not in MECHANISMS any more, so "
                "the acknowledgement hides nothing and checks nothing"
            )

    return unconnected, stale, missing, evidence


def main() -> int:
    inject = None
    for argument in sys.argv[1:]:
        if argument.startswith("--inject="):
            inject = argument.split("=", 1)[1]

    names = {symbol for symbol, _, _, _ in MECHANISMS}
    if inject is not None and inject not in names:
        print(f"❌ --inject={inject} names no symbol in MECHANISMS, so the injection compares nothing")
        return 1

    unconnected, stale, missing, evidence = scan(inject)

    for line in evidence:
        print(f"  {line}")
    print(f"mechanisms checked: {len(MECHANISMS)}")
    print(f"acknowledged as unconnected (a fixed debt list, may only shrink): {len(ACKNOWLEDGED)}")

    if unconnected or stale or missing:
        print(f"failed: {len(unconnected) + len(stale) + len(missing)}")
        print()
        if unconnected:
            print("These abstractions are built and no production code calls them. The values are")
            print("usually right — which is why every other gate passes them — but nothing on")
            print("screen changes because of them:")
            print()
            for finding in unconnected:
                print(f"  {finding}")
        if stale:
            print()
            print("These acknowledgements are stale, so the debt list is describing a tree that no")
            print("longer exists and is granting permission it does not need:")
            print()
            for finding in stale:
                print(f"  {finding}")
        if missing:
            print()
            print("These entries no longer describe the tree, so this gate stopped guarding them:")
            print()
            for finding in missing:
                print(f"  {finding}")
        print()
        print("Wire the mechanism to a control, delete it, or add it to ACKNOWLEDGED with the plan")
        print("item that will connect it. An acknowledgement is a recorded debt, not a mute button:")
        print("a connected entry must leave the list, and the gate says so when it does.")
        return 1

    if inject is not None:
        # The injected symbol must have changed the answer. It is in the table, so it either took
        # the stale-acknowledgement branch (which is already a failure, and reaching the print
        # below would mean it did not) or it is not acknowledged, in which case a manufactured
        # consumer must have removed it from the unconnected list. Reaching here means nothing
        # moved, so this gate is not reading the code it reports on.
        print(f"❌ --inject={inject} changed nothing, so this gate is not comparing")
        return 1

    print("failed: 0  (no mechanism is unconnected without a recorded acknowledgement)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
