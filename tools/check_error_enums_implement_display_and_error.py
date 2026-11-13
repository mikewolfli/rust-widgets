#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Every public `*Error` enum carries the standard error contract (R-31).

# Why this gate exists

The repository deliberately keeps **domain** error enums: a path parser's failure and a
capability lookup's failure answer different questions, and merging them into one `WidgetError`
would erase the type information callers match on (principles #51 and #54). What *is* shared is
the contract a public error type owes its callers: `Display` for a human-readable sentence and
`core::error::Error` (or `std::error::Error` in the std-only side crates) so `?`, `Box<dyn Error>`
and error chaining work.

R-31 found 31 local enums with no single stated contract: some had `Display`, some had `Error`,
many had neither. This gate makes the contract explicit: a `pub enum *Error` must have both.

# Scope

`src/**/*.rs` and `crates/*/src/**/*.rs`, excluding `target/`. `pub(crate)` and private enums are
not public API and are not checked. The impl may appear anywhere in the same file; the gate asks
whether the type is *named* by an impl, so a generated or macro-written impl is counted too.

Usage: tools/check_error_enums_implement_display_and_error.sh
Exit 0 = every public error enum has both impls.
Exit 1 = a finding (file:line and the missing half).
"""

from __future__ import annotations

import pathlib
import re
import sys

ENUM = re.compile(r"(?m)^\s*pub\s+enum\s+(\w*Error)\b")
DISPLAY = r"impl\s+[\w:]*\bDisplay\s+for\s+{}"
ERROR = r"impl\s+[\w:]*\bError\s+for\s+{}"


def sources() -> list[pathlib.Path]:
    roots = [pathlib.Path("src")]
    roots.extend(sorted(pathlib.Path("crates").glob("*/src")))
    out: list[pathlib.Path] = []
    for root in roots:
        if not root.is_dir():
            continue
        for path in root.rglob("*.rs"):
            if "target" in path.parts:
                continue
            out.append(path)
    return out


def self_test() -> list[str]:
    """Prove the two patterns still separate a complete enum from an incomplete one.

    A gate whose regex silently stopped matching would report every enum as complete; this is
    the reverse injection principle #106 asks for, run on every invocation.
    """
    complete = (
        "pub enum SampleError { A }\n"
        "impl core::fmt::Display for SampleError {}\n"
        "impl core::error::Error for SampleError {}\n"
    )
    missing = "pub enum SampleError { A }\n"
    problems = []
    for name in ("SampleError",):
        if not re.search(DISPLAY.format(name), complete):
            problems.append("self-test: Display pattern does not match a real impl")
        if not re.search(ERROR.format(name), complete):
            problems.append("self-test: Error pattern does not match a real impl")
        if re.search(DISPLAY.format(name), missing) or re.search(ERROR.format(name), missing):
            problems.append("self-test: an enum with no impls was reported as complete")
    return problems


def main() -> int:
    problems = self_test()
    if problems:
        for problem in problems:
            print(f"FAIL: {problem}")
        return 1

    checked = 0
    findings: list[str] = []
    for path in sources():
        text = path.read_text(encoding="utf-8", errors="ignore")
        for match in ENUM.finditer(text):
            name = match.group(1)
            checked += 1
            line = text[: match.start()].count("\n") + 1
            if not re.search(DISPLAY.format(re.escape(name)), text):
                findings.append(f"{path}:{line}: {name} has no Display impl")
            if not re.search(ERROR.format(re.escape(name)), text):
                findings.append(f"{path}:{line}: {name} has no Error impl")

    if checked == 0:
        print("FAIL: no public `*Error` enums were found; the scan is not reading the tree")
        return 1

    for finding in findings:
        print(finding)
    print(f"checked={checked} failed={len(findings)}")
    if findings:
        print(
            "  Add `impl core::fmt::Display` and `impl core::error::Error` next to the enum\n"
            "  (side crates use `std::fmt` / `std::error`). The enums stay separate by domain."
        )
        return 1
    print("✅ every public error enum implements Display and Error")
    return 0


if __name__ == "__main__":
    sys.exit(main())
