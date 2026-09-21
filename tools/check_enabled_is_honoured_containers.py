#!/usr/bin/env python3
"""A container that owns programmatic mutators must honour `enabled` in the *emitter*.

# Why this gate exists

`tools/check_enabled_is_honoured.py` gates `handle_event`: a control that handles user input
must consult `is_enabled()`. That check has a structural blind spot, and the blind spot is
exactly where a container hides.

`StackedWidget` was allowlisted as "passive: its handler only delegates to the base". The
allowlist was honest about the handler and wrong about the contract. A page container has no
input of its own, but it *does* own mutators the host calls -- `set_current_index` -- and those
mutators emit `current_changed`. Until this round that signal fired while the control was
disabled, so a subscriber (typically "reload whatever the page shows") acted on a page the user
could not reach. `handle_event` was never involved, so the handler gate could not see it.

That is the same defect class as `MiniCanvas` emitting `clicked` while disabled, one layer down:
the event gate covers the entry point, this gate covers the *exit* point.

# What this gate asks

For every `src/widget/**/*.rs` file that:

  * is in the `NON_INTERACTIVE` allowlist of `check_enabled_is_honoured.py` (i.e. justified as
    having no interaction to disable), **and**
  * declares a mutator that emits a signal,

is `base.is_enabled()` consulted before the emit, or does the file say in writing why the signal
is deliberately not gated?

Requiring the guard everywhere would be wrong: a signal that reports a *platform* fact or a
derived readout (`focus_changed`, a hover redraw) is not a user-visible action and has nothing
to disable. The gate therefore accepts a written reason, on the same principle as the allowlist
it complements -- the interesting output is a file that neither guards nor explains.

# How the allowlist is obtained

It is **imported** from `check_enabled_is_honoured.py`, not copied. Two copies of the same list
would drift (principle #54), and a control added to one and not the other would be gated by
neither.

Run from the repo root.
"""

from __future__ import annotations

import importlib.util
import pathlib
import re
import sys

WIDGET_DIR = pathlib.Path("src/widget")
SIBLING = pathlib.Path("tools/check_enabled_is_honoured.py")

# `pub fn name(&mut self, ...)` -- a mutator, as opposed to an accessor.
MUTATOR_RE = re.compile(r"pub\s+fn\s+(\w+)\s*\(\s*&mut\s+self")
# `.emit(` on the receiver the mutator owns (`self.x.emit(..)` / `self.x_signal().emit(..)`).
EMIT_RE = re.compile(r"(\w+)\.emit\(")

# Phrases that count as a written justification for an ungated emit, by file.
#
# The reason must be *in the file*, next to the code, so that it cannot drift away from what it
# describes -- a comment in this script would be a claim, not evidence.
REASON_MARKERS = (
    "not gated by `enabled`",
    "deliberately not gated",
    "no `enabled` gate",
)


def load_allowlist() -> dict[str, str]:
    """Import `NON_INTERACTIVE` from the sibling gate so the two cannot drift."""
    spec = importlib.util.spec_from_file_location("_enabled_honoured", SIBLING)
    if spec is None or spec.loader is None:
        raise SystemExit(f"❌ cannot load {SIBLING}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.NON_INTERACTIVE


def handler_body(text: str) -> str | None:
    """The body of the first `handle_event`, by brace matching."""
    open_match = re.search(r"fn\s+handle_event\s*\([^)]*\)\s*\{", text)
    if not open_match:
        return None
    depth = 1
    index = open_match.end()
    while index < len(text) and depth > 0:
        char = text[index]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
        index += 1
    return text[open_match.end() : index]


def ungated_emitters(path: pathlib.Path, text: str) -> list[tuple[str, str]]:
    """(mutator, signal) pairs where a mutator emits a signal with no `enabled` guard.

    A mutator is considered guarded when `is_enabled()` appears between its opening brace and
    the emit statement.
    """
    found: list[tuple[str, str]] = []
    for match in MUTATOR_RE.finditer(text):
        name = match.group(1)
        # Locate the body of this mutator by brace matching from its opening brace.
        brace = text.find("{", match.end())
        if brace == -1:
            continue
        depth = 1
        index = brace + 1
        while index < len(text) and depth > 0:
            char = text[index]
            if char == "{":
                depth += 1
            elif char == "}":
                depth -= 1
            index += 1
        body = text[brace + 1 : index]
        for emit in EMIT_RE.finditer(body):
            if emit.group(1) == "base":
                # `self.base.request_redraw()` style plumbing is not a published signal.
                continue
            guarded = "is_enabled()" in body[: emit.start()]
            if not guarded:
                found.append((name, emit.group(1)))
    return found


def main() -> int:
    if not WIDGET_DIR.is_dir():
        print("❌ src/widget not found (run from the repo root)")
        return 1

    allowlist = load_allowlist()
    problems: list[tuple[str, str, str]] = []
    checked = 0
    exempt = 0

    for path in sorted(WIDGET_DIR.rglob("*.rs")):
        rel = path.as_posix()
        if rel not in allowlist:
            # Interactive controls are covered by `check_enabled_is_honoured.py`; this gate is
            # specifically the container/passive seam.
            continue
        text = path.read_text()
        for marker in ("\n#[cfg(test)]\nmod tests {", "\nmod tests {"):
            cut = text.find(marker)
            if cut != -1:
                text = text[:cut]
                break

        emitters = ungated_emitters(path, text)
        if not emitters:
            exempt += 1
            continue

        checked += 1
        if any(marker in text for marker in REASON_MARKERS):
            exempt += 1
            continue
        for mutator, signal in emitters:
            problems.append((rel, mutator, signal))

    print(f"Passive/container files with ungated emitting mutators: {checked}")
    print(f"  accepted (no emitting mutator, or a written reason): {exempt}")
    print()

    if problems:
        print(f"❌ {len(problems)} programmatic emit(s) ignore `enabled` in allowlisted file(s):")
        for rel, mutator, signal in problems:
            print(f"   {rel}::{mutator}() emits `{signal}` while the control may be disabled")
        print()
        print("   A host that calls `set_enabled(false)` still gets this signal, so a subscriber")
        print("   reacts to a change the user cannot reach. Either gate the emit with")
        print("   `if !self.base.is_enabled()` (plus a queryable reason), or state in the file")
        print("   why the signal is deliberately not gated.")
        return 1

    print("✅ enabled contract (containers): every programmatic emit in an allowlisted file is")
    print("   gated by `enabled`, absent, or carries a written reason")
    return 0


if __name__ == "__main__":
    assert handler_body("fn handle_event(&mut self) { x }") == " x }"
    sys.exit(main())
