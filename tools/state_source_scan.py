#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""BLUE23 §2.2 gate: hover/press/focus state has exactly one source, `BaseWidget`.

The state channel works only if the answer to "am I hovered?" is recorded once.
The runtime already commits to *which* control the pointer is over
(`widget::runtime::dispatch_hover_transition` synthesises the `MouseEnter`/`MouseLeave`
pair), so a control that keeps a private `hovered: bool` maintains a **second** opinion
of a fact the base already owns -- and the two drift the moment one of them is wired up
and the other is not. That is the mechanism by which a `"button:hover"` theme entry was
unreachable while every test stayed green.

The same argument holds for the focus reason: a private `focus_reason` can disagree with
the base's, and `draws_focus_ring()` would then answer differently at two call sites.

This scan is lexical and build-free. It reports:
  * a `hovered: bool` / `pressed: bool` / `grabbed: bool` field declared outside
    `src/widget/base.rs` **in a struct that owns a `BaseWidget`** (part-hover like `hovered_tab`,
    data-hover like `hovered_item`, and paint-input structs like `ActionButtonPaint` are not
    fields of this shape and are not matched);
  * a `focus_reason: FocusReason` field in such a struct, declared outside `src/widget/base.rs`.

Run with `--inject` to prove the search actually reads the tree: the injection adds a
private field to a named widget and the scan must then report it.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
BASE = ROOT / "src" / "widget" / "base.rs"
WIDGET_DIR = ROOT / "src" / "widget"

# A private field of exactly the widget-level state shape. Names that merely *contain*
# `hovered` (`hovered_tab`, `minimize_hovered`) or that carry a data-selection meaning
# (`hovered_item`) are deliberately not matched: they answer a different question.
FIELD_PATTERNS = [
    re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?P<name>hovered)\s*:\s*bool\b", re.MULTILINE),
    re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?P<name>pressed)\s*:\s*bool\b", re.MULTILINE),
    re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?P<name>grabbed)\s*:\s*bool\b", re.MULTILINE),
    re.compile(
        r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?P<name>focus_reason)\s*:\s*FocusReason\b",
        re.MULTILINE,
    ),
]

INJECTION = (
    "\n/// Injected by --inject: a control-shaped probe with a second, private hover source.\n"
    "///\n"
    "/// The `base` field is what makes it a *control* rather than a transfer struct: the scan\n"
    "/// only reads the bodies of structs that own a `BaseWidget`, which is the state the rule is\n"
    "/// about.\n"
    "pub struct InjectedHoverProbe {\n"
    "    #[allow(dead_code)]\n"
    "    base: BaseWidget,\n"
    "    #[allow(dead_code)]\n"
    "    hovered: bool,\n"
    "}\n"
)

STRUCT_HEADER = re.compile(
    r"^\s*(?:pub(?:\([^)]*\))?\s+)?struct\s+(?P<name>\w+)[^;{]*\{\s*$", re.MULTILINE
)
OWNS_A_BASE = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?base\s*:\s*BaseWidget\b", re.MULTILINE)


def control_struct_bodies(text: str) -> list[tuple[int, str]]:
    """`(offset, body)` for every struct that owns a `BaseWidget`.

    # Why the scan is scoped to these structs

    The rule is about **widget-level** state: a control that keeps its own `hovered` beside the
    base's. A transfer struct — `ActionButtonPaint { armed, hovered, is_enabled, ink, surface }` in
    `empty_state.rs` — is the *opposite* shape: it carries the base's answer into the painter and
    owns no state at all. The un-scoped scan reported that field as a second source, which is a
    false failure whose printed remedy ("delete the private field and read the base accessor")
    would have deleted the parameter the painter reads.

    `base: BaseWidget` is the discriminator rather than a naming convention because it is the
    `Widget` contract: every control that can be hovered stores its base, and a struct that does
    not cannot answer `is_hovered()` at all.
    """
    bodies: list[tuple[int, str]] = []
    for header in STRUCT_HEADER.finditer(text):
        open_at = text.rfind("{", header.start(), header.end())
        depth = 0
        for index in range(open_at, len(text)):
            if text[index] == "{":
                depth += 1
            elif text[index] == "}":
                depth -= 1
                if depth == 0:
                    body = text[open_at : index + 1]
                    if OWNS_A_BASE.search(body):
                        bodies.append((open_at, body))
                    break
    return bodies


def scanned_files() -> list[pathlib.Path]:
    """Every widget source, excluding the one file that is allowed to own the fields."""
    files = []
    for path in sorted(WIDGET_DIR.rglob("*.rs")):
        if path.resolve() == BASE.resolve():
            continue
        files.append(path)
    return files


def scan(files: list[pathlib.Path]) -> list[str]:
    findings: list[str] = []
    for path in files:
        text = path.read_text(encoding="utf-8")
        # Strip test modules: a field declared inside `#[cfg(test)]` is a fixture, not a
        # second production source of state.
        if "#[cfg(test)]" in text:
            text = text.split("#[cfg(test)]")[0]
        for offset, body in control_struct_bodies(text):
            for pattern in FIELD_PATTERNS:
                for match in pattern.finditer(body):
                    line = text[: offset + match.start()].count("\n") + 1
                    findings.append(f"{path.relative_to(ROOT)}:{line}: {match.group('name')}")
    return findings


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--inject",
        metavar="WIDGET_FILE",
        help="append a private `hovered: bool` to this file before scanning",
    )
    args = parser.parse_args(argv)

    files = scanned_files()
    if not files:
        print("FAIL: no widget sources found; the scan is not reading the tree")
        return 1

    injected: pathlib.Path | None = None
    backup: str | None = None
    if args.inject:
        injected = (ROOT / args.inject).resolve()
        if not injected.exists():
            print(f"FAIL: --inject target {args.inject} does not exist")
            return 1
        backup = injected.read_text(encoding="utf-8")
        injected.write_text(backup + INJECTION, encoding="utf-8")

    try:
        findings = scan(files)
    finally:
        if injected is not None and backup is not None:
            injected.write_text(backup, encoding="utf-8")

    if args.inject:
        # The injection must be visible, or the search is not reading the code. A visible
        # injection exits **non-zero** so the shell wrapper can treat "the result changed"
        # as the successful detection it is.
        naming = [
            f
            for f in findings
            if f.startswith(str(injected.relative_to(ROOT)))
        ]
        if not naming:
            print("FAIL: --inject added a private hover field and the scan did not report it")
            return 0
        return 1

    if findings:
        print("FAIL: widget-level hover/press/focus state is declared outside BaseWidget:")
        for finding in findings:
            print(f"  {finding}")
        print()
        print("  These four facts are owned by BaseWidget (src/widget/base.rs) and answered")
        print("  through is_hovered()/is_pressed()/is_grabbed()/focus_reason(). A second")
        print("  declaration is a private opinion that drifts from the single source the")
        print("  runtime and the theme both read.")
        return 1

    print(f"state-source scan: checked={len(files)} failed=0")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
