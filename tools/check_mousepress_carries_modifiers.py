#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Gate: every `Event::MousePress` *struct literal* must carry the `modifiers` field.

`Event::MousePress` gained a `modifiers` bitmask so a `Shift`/`Ctrl` click can select a
range or toggle one row, which is the whole of `SelectionMode::Extended`'s contract.
Rust lets a struct literal omit a field only when it is initialised elsewhere, so a
missing one is a compile error — but a literal that *spells* `modifiers: 0` where the
platform layer already has the real state silently reports "no modifier" and the
feature stops working with no test failing. This gate exists to make that second
mistake visible at the one place it can be reasoned about: the platform backends.

The check has two parts:

  [1/2] No struct literal in `src/` or `tests/` omits `modifiers`, and none inside
        `src/platform/` hard-codes `modifiers: 0` on a *press* — the backends are
        exactly where the live modifier state is available.
  [2/2] The three desktop backends each build their press through `mouse_press_with`
        (or an equivalent call carrying a live bitmask), rather than through the
        zero-modifier convenience constructor.

Reverse injection: deleting the `modifiers` field from any backend's press, or
replacing `mouse_press_with(..)` with `mouse_press(..)`, must fail step [1/2] or [2/2].
"""

from __future__ import annotations

import pathlib
import re
import sys

START = "Event::MousePress {"
# A press built with the zero-modifier convenience constructor.
ZERO_CONSTRUCTOR = re.compile(r"Event::mouse_press\(")
LIVE_CONSTRUCTOR = re.compile(r"Event::mouse_press_with\(")

BACKENDS = [
    "src/platform/windows/canvas.rs",
    "src/platform/macos/canvas.rs",
    "src/platform/linux/canvas.rs",
]


def find_literals(text: str):
    """Yield body text for each `Event::MousePress { .. }` literal, balancing braces.

    Brace balancing is what makes this work on `Event::MousePress { pos: Point { x, y },
    .. }` — a naive `[^{}]*` stops at the nested `}`.
    """
    pos = 0
    while True:
        start = text.find(START, pos)
        if start == -1:
            return
        depth = 0
        index = start + len(START) - 1
        while index < len(text):
            char = text[index]
            if char == "{":
                depth += 1
            elif char == "}":
                depth -= 1
                if depth == 0:
                    yield text[start + len(START):index]
                    pos = index + 1
                    break
            index += 1
        else:
            return


def fail(message: str) -> None:
    print(f"❌ {message}")
    sys.exit(1)


def main() -> int:
    root = pathlib.Path(__file__).resolve().parent.parent
    sources = sorted(root.joinpath("src").rglob("*.rs"))
    sources += sorted(root.joinpath("tests").rglob("*.rs"))

    failures: list[str] = []

    for path in sources:
        rel = path.relative_to(root).as_posix()
        text = path.read_text(encoding="utf-8")
        for body in find_literals(text):
            # A pattern (`..`) is not a literal; it destructures whatever it is given.
            if ".." in body:
                continue
            if "button" not in body:
                continue
            if "modifiers" not in body:
                failures.append(f"{rel}: struct literal omits `modifiers`")
            elif rel.startswith("src/platform/") and "modifiers: 0" in body:
                failures.append(
                    f"{rel}: press hard-codes `modifiers: 0`; the live state is "
                    "available here, so a modifier click cannot work"
                )

    if failures:
        print(f"❌ MousePress modifier wiring: {len(failures)} problem(s)")
        for item in failures:
            print(f"   {item}")
        return 1
    print("  [1/2] every MousePress literal carries `modifiers`; no backend zeroes it")

    for rel in BACKENDS:
        path = root / rel
        if not path.exists():
            fail(f"{rel} is missing")
        text = path.read_text(encoding="utf-8")
        if not LIVE_CONSTRUCTOR.search(text):
            fail(f"{rel}: no `Event::mouse_press_with(..)` — the press cannot carry modifiers")
        # A backend may still use the zero constructor for a synthetic press (a test, or a
        # press it manufactures with no input behind it), so only the *press handler* is
        # checked: the one that carries live state.
        if "current_modifiers()" not in text and "modifier_state_bits" not in text \
                and "map_modifiers" not in text:
            fail(f"{rel}: reads no live modifier state (expected a platform helper call)")

    print(f"  [2/2] {len(BACKENDS)} desktop backends build their press with live modifiers")
    print("✅ MousePress carries modifiers end to end")
    return 0


if __name__ == "__main__":
    sys.exit(main())
