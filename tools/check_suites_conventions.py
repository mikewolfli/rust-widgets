#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""The main-package conventions also hold in `rust-widgets-suites`.

# Why this gate exists (issue §8 extension direction 1)

Every ice-berg cleanup so far used the main package's file list as its boundary; the suites
crate never entered the crossing radius. W5-03/13/14/15/38 were all one root: a convention landed
in `src/widget/**` and stopped there. This gate is the four-criterion crossing check:

  [1] key codes — a `KeyPress` arm in suites matches `key_codes::*`, not a raw virtual-key number;
  [2] disabled — a suites module with an input gate (`is_enabled`) and a `Draw` impl names one of
      the shared disabled mechanisms (`veil_if_disabled` / `disabled_*` / `StateOverlay`);
  [3] focus ring — every suites control that overrides `is_focusable` paints a focus ring;
  [4] RTL — every column-layout control W5-15 named consumes the shared direction helpers.

# What it does not prove

It is lexical. It proves the convention is *named* in the file, not that every branch honours it;
the behavioural evidence stays in the crate's own tests (and `state_visual_probe` for the main
package). Test modules are stripped before the scan, so a test literal cannot satisfy a criterion.

Usage: tools/check_suites_conventions.sh
Exit 0 = all four conventions hold across the suites crate.
"""

from __future__ import annotations

import pathlib
import re
import sys

ROOT = pathlib.Path("crates/rust-widgets-suites/src")

KEY_ARM = re.compile(
    r"(?:^|[^0-9])(?:8|9|13|27|32|33|34|35|36|37|38|39|40|45|46|112|113|114|115|116|117|118|119|120|121|122|123)"
    r"\s*=>"
)
DISABLED = ("veil_if_disabled", "paint_disabled_veil", "disabled_surface_near", "disabled_tint",
            "disabled_ink_on", "StateOverlay", "let enabled =", "let is_enabled =")
RTL_FILES = [
    "finance/quote_board.rs",
    "finance/order_book.rs",
    "devtools/diff_viewer/file_list.rs",
    "project/gantt/gantt_companions/task_grid.rs",
    "media_playback/chapter_list_panel.rs",
]
RTL = ("mirror_span", "mirror_point_x", "mirror_offset", "leading_alignment",
       "default_text_direction")


def production_text(path: pathlib.Path) -> str:
    lines = path.read_text(encoding="utf-8", errors="ignore").splitlines()
    out: list[str] = []
    i = 0
    while i < len(lines):
        is_mod = False
        if re.match(r"^#\[cfg\(test\)\]\s*$", lines[i]):
            j = i + 1
            while j < len(lines) and not lines[j].strip():
                j += 1
            is_mod = j < len(lines) and re.match(r"\s*(?:pub\s+)?mod\s", lines[j]) is not None
        if is_mod:
            depth = 0
            started = False
            while i < len(lines):
                depth += lines[i].count("{") - lines[i].count("}")
                started = started or "{" in lines[i]
                i += 1
                if started and depth <= 0:
                    break
            continue
        out.append(lines[i])
        i += 1
    return "\n".join(out)


def modules(files: list[pathlib.Path]) -> dict[pathlib.Path, str]:
    grouped: dict[pathlib.Path, list[pathlib.Path]] = {}
    for path in files:
        parent = path.parent
        key = parent if (parent / "mod.rs").exists() else (
            parent / path.stem if (parent / path.stem).is_dir() else parent
        )
        grouped.setdefault(key, []).append(path)
    return {key: "\n".join(production_text(p) for p in paths) for key, paths in grouped.items()}


def self_test() -> list[str]:
    problems = []
    if not KEY_ARM.search("37 => true"):
        problems.append("self-test: a raw key arm was not detected")
    if KEY_ARM.search("key_codes::LEFT => true"):
        problems.append("self-test: a `key_codes::` arm was flagged as raw")
    return problems


def key_match_bodies(text: str) -> list[str]:
    """The bodies of `match key` / `match *key` blocks.

    An SGR parser matches numeric escape parameters with `=>` arms too; scanning every numeric
    arm reported those as keyboard codes. Keyboard arms live in a match on the key itself, so
    the scan is scoped to those blocks (and to `key == <n>` guards).
    """
    bodies: list[str] = []
    for match in re.finditer(r"match\s+\*?key\s*\{", text):
        start = match.end() - 1
        depth = 0
        for index in range(start, len(text)):
            if text[index] == "{":
                depth += 1
            elif text[index] == "}":
                depth -= 1
                if depth == 0:
                    bodies.append(text[start : index + 1])
                    break
    return bodies


KEY_COMPARE = re.compile(r"\*key\s*==\s*(\d+)")


def main() -> int:
    problems = self_test()
    if problems:
        for problem in problems:
            print(f"FAIL: {problem}")
        return 1
    if not ROOT.is_dir():
        print(f"FAIL: {ROOT} is missing (run from the repository root)")
        return 1

    files = sorted(p for p in ROOT.rglob("*.rs") if "target" not in p.parts)
    findings: list[str] = []
    checked_keys = 0

    for path in files:
        text = production_text(path)
        for body in key_match_bodies(text):
            for match in KEY_ARM.finditer(body):
                offset = text.index(body) + match.start()
                line = text[:offset].count("\n") + 1
                findings.append(
                    f"{path}:{line}: raw key code in a `match key` arm; use `key_codes::*`"
                )
                checked_keys += 1
        for match in KEY_COMPARE.finditer(text):
            line = text[: match.start()].count("\n") + 1
            findings.append(
                f"{path}:{line}: raw key code in a key comparison; use `key_codes::*`"
            )
            checked_keys += 1

    for key, text in modules(files).items():
        if "impl Draw" not in text:
            continue
        if "is_enabled" not in text:
            continue
        if not any(token in text for token in DISABLED):
            findings.append(f"{key}: input gate + Draw but no disabled visual mechanism")
        focusable = "fn is_focusable" in text
        if focusable and not ("draw_focus_ring" in text or "FocusRing::for_control" in text):
            findings.append(f"{key}: overrides is_focusable but paints no focus ring")

    for relative in RTL_FILES:
        path = ROOT / relative
        if not path.exists():
            findings.append(f"{path}: RTL column control is missing")
            continue
        if not any(token in production_text(path) for token in RTL):
            findings.append(f"{path}: column control reads no RTL/direction helper")

    for finding in findings:
        print(finding)
    print(
        f"checked files={len(files)} raw_key_arms={checked_keys} "
        f"modules={len(modules(files))} findings={len(findings)}"
    )
    if findings:
        print("  Fix the named file; the convention lives in `src/widget/` and must cross over.")
        return 1
    print("✅ suites follow the key-code, disabled, focus-ring and RTL conventions")
    return 0


if __name__ == "__main__":
    sys.exit(main())
