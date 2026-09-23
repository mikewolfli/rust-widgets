#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""BLUE23 §3.3 gate: an animation state machine must be drivable by the bus.

The defect this guards is the crate's most repeated shape: a control implements an
animation correctly, tests it directly, and **nothing drives it**. Eleven controls had
`pub fn tick(delta_ms) -> bool`; not one production caller existed, so the hover faded
nowhere and the caret never blinked while every test stayed green.

The fix puts the frame contract on `Widget` (`tick` + `is_animating`) and drives it from
one place (`widget::runtime::tick_animations`). This gate keeps it closed: every file that
declares an animation `tick` must also implement the `Widget` trait's `tick`, so a new
control cannot reintroduce an island.

Lexical and build-free. A finding names the file.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
WIDGET_DIR = ROOT / "src" / "widget"

# A self-driven animation entry point on a control. `tick_count` / `tick_label_height`
# (a count, a label height) are not animations and are excluded by the `delta` parameter.
ANIM_TICK = re.compile(r"\bfn\s+tick\s*\(\s*&mut\s+self\s*,\s*(?:delta\w*)\s*:")
# The trait-side bridge a driven control must carry.
TRAIT_TICK = re.compile(r"\bfn\s+tick\s*\(\s*&mut\s+self\s*,\s*delta_ms\s*:\s*u32\s*\)\s*->\s*bool")

# The bus itself lives here and calls the trait method; it is not a control.
BUS_FILES = {"src/widget/runtime.rs", "src/widget/widget_trait.rs"}


def scanned_files() -> list[pathlib.Path]:
    return sorted(path.resolve() for path in WIDGET_DIR.rglob("*.rs"))


def scan(files: list[pathlib.Path]) -> list[str]:
    findings: list[str] = []
    for path in files:
        rel = str(path.relative_to(ROOT))
        if rel in BUS_FILES:
            continue
        text = path.read_text(encoding="utf-8")
        # Strip a trailing `#[cfg(test)]` module (fixtures, not production code). The split
        # is at the *last* such attribute so an earlier mention in a comment cannot hide a
        # real declaration above it.
        marker = text.rfind("\n#[cfg(test)]")
        body = text[:marker] if marker != -1 else text
        if ANIM_TICK.search(body) and not TRAIT_TICK.search(body):
            findings.append(rel)
    return findings


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--inject",
        metavar="WIDGET_FILE",
        help="append an undispatched `tick` to this file before scanning",
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
        # Insert before any trailing test module, so the injected declaration is in the
        # *production* body the scan reads -- appending after `#[cfg(test)]` would be
        # stripped, which is a false green rather than a detection.
        marker = backup.rfind("\n#[cfg(test)]")
        splice = (
            "\n/// Injected by --inject: an animation with no bus bridge.\n"
            "pub fn tick(&mut self, delta_us: u64) -> bool { let _ = delta_us; false }\n"
        )
        injected.write_text(
            backup[:marker] + splice + backup[marker:] if marker != -1 else backup + splice,
            encoding="utf-8",
        )

    try:
        findings = scan(files)
    finally:
        if injected is not None and backup is not None:
            injected.write_text(backup, encoding="utf-8")

    if args.inject:
        # A visible injection exits non-zero so the wrapper treats "the result changed"
        # as the detection it is.
        naming = [f for f in findings if f == str(injected.relative_to(ROOT))]
        if not naming:
            print("FAIL: --inject added an undispatched tick and the scan did not report it")
            return 0
        return 1

    if findings:
        print("FAIL: these controls declare an animation `tick` with no `Widget::tick`")
        print("      bridge, so `runtime::tick_animations` cannot advance them:")
        for finding in findings:
            print(f"  {finding}")
        print()
        print("  Add to the control's `impl Widget` block:")
        print("      fn tick(&mut self, delta_ms: u32) -> bool { Self::tick(self, delta_ms) }")
        print("      fn is_animating(&self) -> bool { /* true while moving */ }")
        return 1

    print(f"animation-driver scan: checked={len(files)} failed=0")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
