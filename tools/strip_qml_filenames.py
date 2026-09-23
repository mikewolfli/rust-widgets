#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Second pass: neutralize `.qml` filename and checkout-path citations.

# Why a second pass

The first pass (`strip_third_party_names.py`) removed the product *names*. What it could not
remove is the shape those citations take: `Button.qml:39-40`, `ProgressBar.qml:27-30`,
`src/quickcontrols/basic/*.qml`. A file extension plus a line range is still unambiguously a
pointer into a specific product's source tree.

# What the replacement keeps

The **evidence**, not the address. A claim in these documents reads "the reference toolkit's
button floor is 100x40"; the citation's job is to say which control and which fact. So:

    `Button.qml:39-40`          ->  `reference: button floor`
    `ProgressBar.qml:27-30`     ->  `reference: progress-bar height`
    `src/quickcontrols/basic/*.qml` -> `the reference toolkit's control sources`

The mapping is a table rather than a mechanical transform, because the value of each citation is
*semantic*: `SpinBox.qml:20-21` is cited only ever for the "padding derived from the sibling's
width" rule, and a blind rewrite to `reference: spin-box geometry` would lose that. Any `.qml`
file not in the table falls back to a generic mechanism name derived from the control, and is
reported so a human can give it a better one.
"""

import collections
import pathlib
import re

TARGETS = sorted(pathlib.Path("docs/plans").glob("blue*.md")) + sorted(
    pathlib.Path("docs/log").glob("*.md")
)

# control (as it appears in the filename) -> the fact the citation always carries in this corpus
MECHANISM = {
    "Button": "the button's implicit-size formula and padding cascade",
    "Switch": "the switch's track, thumb and transition",
    "Slider": "the slider's handle size and travel",
    "ProgressBar": "the progress bar's thickness, radius and fill origin",
    "ScrollBar": "the scroll bar's minimum-length and hide-delay rules",
    "SpinBox": "padding derived from the sibling's own width",
    "MenuItem": "the menu row's indicator and trailing-column padding",
    "CheckBox": "`spacing` as the indicator-to-text gap",
    "ComboBox": "`spacing` as the indicator-to-text gap",
    "GroupBox": "the title-band reserve: padding + label height + spacing",
    "TextField": "the text field's minimum height and padding",
    "TabBar": "the tab strip's run and spacing",
    "Control": "the base control's background/padding contract",
    "Check": "the check indicator's size",
    "AbstractButton": "the abstract button's state contract",
    "RadioButton": "the radio indicator's size",
    "ToolBar": "the tool bar's item spacing",
    "Dialog": "the dialog's button row and padding",
    "DialogButtonBox": "the dialog's button row and padding",
    "ToolTip": "the tooltip's padding and dwell timing",
    "Popup": "the popup's positioning and overlay policy",
}

# `Foo.qml:12-34` / `Foo.qml` / `Foo.qml:12,34` etc.
QML_FILE = re.compile(r"`?([A-Z]\w+)\.qml(?::([\d,\-–\u2013]+))?`?")
# the two checkout path shapes this corpus uses for the same thing
CHECKOUT_PATHS = [
    (re.compile(r"`?src/quickcontrols/basic/\*\.qml`?"), "the reference toolkit's control sources"),
    (re.compile(r"`?src/quicktemplates/\*\.cpp`?"), "the reference toolkit's template sources"),
    (re.compile(r"`?src/quickcontrols/[a-z]+/`?"), "the reference toolkit's control sources"),
    (re.compile(r"`?Basic/\*\.qml`?"), "the reference toolkit's control sources"),
]
# a fenced ```qml block -> a neutral fence (the content is pseudocode we authored)
FENCE = re.compile(r"```qml\b")


def main() -> None:
    used = collections.Counter()
    unknown = collections.Counter()
    touched = collections.Counter()

    def replace_qml(match: "re.Match[str]") -> str:
        control, _lines = match.group(1), match.group(2)
        used[control] += 1
        fact = MECHANISM.get(control)
        if fact is None:
            unknown[control] += 1
            fact = f"the {control.lower()} reference figure"
        return f"`reference: {fact}`"

    for path in TARGETS:
        original = path.read_text(encoding="utf-8")
        text = original

        for pattern, replacement in CHECKOUT_PATHS:
            text, n = pattern.subn(replacement, text)
            touched[path.name] += n

        text, n = QML_FILE.subn(replace_qml, text)
        touched[path.name] += n

        text, n = FENCE.subn("```text", text)
        touched[path.name] += n

        if text != original:
            path.write_text(text, encoding="utf-8")

    print("citations rewritten by control:")
    for control, count in sorted(used.items(), key=lambda kv: -kv[1]):
        print(f"  {count:4d}  {control}")
    if unknown:
        print("\nUNMAPPED (a human should give these a mechanism):")
        for control, count in sorted(unknown.items(), key=lambda kv: -kv[1]):
            print(f"  {count:4d}  {control}")
    print(f"\nfiles touched: {len(touched)}  replacements: {sum(touched.values())}")


if __name__ == "__main__":
    main()
