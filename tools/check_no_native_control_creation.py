#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""The `src/` tree must contain no OS control-creation call — only window/surface creation.

# Why this gate exists

The library's central architectural claim is that it **paints every `WidgetKind` itself**, on every
backend, and that no per-kind native control is created anywhere. Both READMEs state it as a
falsifiable fact about the source ("没有一处 `NSButton` / `gtk_button_new` / `android.widget.Button`"),
and a host relies on it: it is why the same widget looks identical on every platform, why `mini` needs
no GUI toolkit, and why the SVG/PNG backends are possible at all.

Nothing checked it. That is the same shape as the capability flags and the platform docs — a claim
the code is supposed to satisfy, with no gate reading it — and it had already drifted: the README
enumerated `CreateWindowExW` among the things the crate does not contain, while five occurrences
existed, because **window creation** legitimately uses it. The claim was over-broad, not the code.

# The distinction this gate draws

| Allowed | Forbidden |
|---|---|
| Creating a **window** or a **drawing surface** — `CreateWindowExW` with the window class, a GTK window, an `NSWindow` | Creating a **control** — `CreateWindowExW` with `BUTTON`/`EDIT`/…, `gtk_button_new`, `NSButton`, `android.widget.Button` |

That distinction is the architecture, not a detail: the host supplies a window and a surface, and the
library draws into it. So the gate's job is not "no OS symbols" (the backends are full of them) but
"no OS *control* creation".

# What is checked

A closed list of per-kind creator names across the four GUI toolkits the crate supports, plus the
Win32 `CreateWindowExW`-with-a-control-class spelling. Each is looked for in **code only** (comments
stripped), because a backend's rationale comments name exactly these functions to explain why they
are *not* used — counting those would make the gate pass on the strength of the prose explaining the
absence.

# What this deliberately does not attempt

It does not verify that the painted output is correct — that is the snapshot and SVG suite's job. It
asserts only the negative: no control is created by a host API, so the painting really is the only
path.

Usage: python3 tools/check_no_native_control_creation.py
"""

from __future__ import annotations

import pathlib
import re
import sys

# Per-kind control creators, by toolkit. Each pattern requires an **invocation or a real
# construction**, not a bare mention of the class name.
#
# # Why the distinction is load-bearing
#
# The `cupertino` widget family deliberately *cites* AppKit/UIKit class names as the specification
# for what it paints — `src/widget/cupertino/core.rs` asserts that an on-state switch is painted
# `rgba(52,199,89)` and says so with the words "`UISwitch.onTintColor`". That is a true statement
# about a painted colour, and a pattern matching the bare identifier reports it as a native control.
# So each entry below is anchored to a call or a constructor: a `(` for the C-style toolkit
# functions, an alloc/init/`new` shape for the Objective-C classes.
FORBIDDEN: tuple[tuple[str, str], ...] = (
    # GTK: `gtk_button_new()` and every sibling. `gtk_window_new` is excluded ahead of the pattern
    # because creating a window is the host's legitimate half of the contract.
    ("gtk", r"\bgtk_(?!window_new|init|main|application_new)[a-z_]*_new\s*\("),
    # Win32: the control-class spelling. `CreateWindowExW` is *allowed* (the window path uses it),
    # so this matches the control class-name constants instead.
    ("win32", r"WC_BUTTON\b|WC_EDIT\b|WC_STATIC\b|WC_LISTBOX\b|WC_COMBOBOX\b|BUTTON_CLASS\b"),
    # AppKit: a per-kind view *constructed* (`[NSButton alloc]`, `NSButton::new`, `NSButton::alloc`).
    # `NSWindow` is allowed — a window — and `NSView` is the surface.
    (
        "appkit",
        r"\bNS(?:Button|TextField|TextView|PopUpButton|Slider|ProgressIndicator|ScrollView"
        r"|TableView|CollectionView|MenuItem|Alert|OpenPanel|SavePanel|ColorPanel|FontPanel"
        r"|Popover|SegmentedControl|SegmentedCell|Switch|DatePicker|Stepper)"
        r"\s*(?:::\s*(?:new|alloc)|\[)",
    ),
    # UIKit: likewise construction, not mention. `UIWindow` is allowed; `UIView` is the surface.
    (
        "uikit",
        r"\bUI(?:Button|Label|TextField|TextView|Switch|Slider|ProgressView|ScrollView|TableView"
        r"|CollectionView|PickerView|StackView|ActivityIndicatorView|SegmentedControl|DatePicker"
        r"|Stepper|AlertController)"
        r"\s*(?:::\s*(?:new|alloc)|\[)",
    ),
    # Android/JVM: the widget package reached through a class-name string (what a JNI bridge would
    # need), or a per-kind creator symbol.
    ("android", r'"android\.widget\.\w+"|\bnativeCreate[A-Z]\w*View\b'),
)

# A mention is allowed when the line uses it to say the thing is *not* done. Deliberately generous:
# the corrected documents and comments all cite these names in negations, and reporting the
# explanation of the absence as the absence is the false positive this exists to avoid.
NEGATION_MARKERS = (
    "no ",
    "No ",
    "not ",
    "never",
    "without",
    "instead of",
    "rather than",
    "removed",
    "Removed",
    "deleted",
    "Deleted",
    "former",
    "Former",
    "used to",
    "不再",
    "没有",
    "而不是",
    "已移除",
    "已删除",
)


def code_only(path: pathlib.Path) -> str:
    """The file's code with comments stripped, so a rationale note is not read as a call."""
    parts = []
    for line in path.read_text(encoding="utf-8").splitlines():
        stripped = line.lstrip()
        if stripped.startswith("//") or stripped.startswith("/*") or stripped.startswith("*"):
            continue
        parts.append(line.split("//", 1)[0])
    return "\n".join(parts)


def check() -> list[str]:
    findings: list[str] = []
    patterns = [(name, re.compile(pat)) for name, pat in FORBIDDEN]
    for path in sorted(pathlib.Path("src").rglob("*.rs")):
        text = code_only(path)
        for lineno, line in enumerate(text.splitlines(), start=1):
            if any(marker in line for marker in NEGATION_MARKERS):
                continue
            for name, pattern in patterns:
                m = pattern.search(line)
                if m:
                    findings.append(
                        f"{path.as_posix()}:{lineno} creates a {name} control "
                        f"(`{m.group(0).strip()}`). The library paints every `WidgetKind` itself "
                        f"on every backend, so no host control creator may appear in a widget path"
                    )
    return findings


def inject() -> int:
    """Plant a `gtk_button_new` call and require the gate to notice."""
    path = pathlib.Path("src/platform/linux/platform_impl.rs")
    original = path.read_text(encoding="utf-8")
    marker = "impl Platform for LinuxPlatform {"
    if marker not in original:
        print("❌ injection point not found in src/platform/linux/platform_impl.rs")
        return 1
    broken = original.replace(
        marker,
        marker + "\n    // injection\n    fn _injected() { let _ = gtk_button_new(); }",
        1,
    )
    try:
        path.write_text(broken, encoding="utf-8")
        found = check()
    finally:
        path.write_text(original, encoding="utf-8")
    if found:
        print("✅ reverse injection: a native control-creation call is detected")
        return 0
    print("❌ reverse injection: a native control-creation call was NOT detected")
    return 1


def main() -> int:
    if "--inject" in sys.argv:
        return inject()

    findings = check()
    print(f"native control creation: {len(FORBIDDEN)} toolkit pattern(s) checked across src/")
    print()
    if findings:
        print(f"❌ a host control creator appears in the widget path ({len(findings)}):")
        for finding in findings:
            print(f"   {finding}")
        return 1
    print("✅ no native control creation: the library paints every WidgetKind on every backend")
    return 0


if __name__ == "__main__":
    sys.exit(main())
