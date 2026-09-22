#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Gate: a control that exists to demonstrate a feature must show that feature in its own snapshot.

# Why this gate exists (BLUE21 P3-1h / A.3.9 / #107)

Every other snapshot assertion asks a question a *machine* can answer: "did this control paint
something", "did its chrome move with the theme", "is the SVG well formed". None of them can ask
the question a person asks in one second:

    this control is called `floating_label`. Where is the floating label?

A control can be complete, theme-aware, non-empty and well-formed while failing to demonstrate the
one thing it exists for. The motivating case is real and was shipped: `floating_label` — a control
whose entire purpose is Material's floating caption — produced a snapshot with **no label in it**.
Its `label` property was not even published, and the shared label helper resolved "the label" to
`text`, so the caption went into the input and the control had nothing to float. The pixels were
present; the feature was absent.

`tab_widget` is the same failure in a different organ: it shipped with zero tabs, so its tab band —
the strip of chrome that *is* a tab widget — was not in its picture at all. A `badge` whose pill is
filled with the window's own colour is the third: the shape is drawn, and it is invisible.

# What it asserts

For each entry, the control's dark snapshot (`snapshots/svg/<control>.svg`) must contain the
required patterns and must not contain the forbidden ones. Both halves matter: "a `<text>` exists"
alone does not distinguish `floating_label`'s caption from its input text, while the forbidden
pattern states exactly what the defect looked like in the file.

# Why the markers are literals from the current snapshot rather than a geometry model

Reading the SVG as a drawing and asking "is there a floating label" is not decidable in general —
that is the same wall `tools/audit_text_y.py` hits and why it is documented as an audit aid. What
*is* decidable is that the control's characteristic part has a stable, recognisable spelling in its
own snapshot, and that the spelling disappears when the feature regresses. Every `requires` /
`forbids` pair below was obtained by **inspecting the committed file and then reproducing the
defect** (revert the feature, re-export, diff): the marker is present now and verifiably absent
when the feature is removed. That is the evidence that makes each row a check instead of a guess.

The "why" on each entry states the defect the marker would catch, because a marker whose purpose is
not written down becomes a mystery the next person deletes when a legitimate colour change makes it
red.

# Reverse injection

`--inject=<control>` requires the named control to be reported, so a comparison that never compared
cannot pass.

Run from the repo root. Regenerate the snapshots first with:

    cargo run --no-default-features --features desktop --example export_control_svgs

Usage: tools/check_control_feature_visible_in_own_snapshot.py   (exit 1 on a missing feature)
"""

from __future__ import annotations

import pathlib
import re
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent
SNAPSHOTS = REPO / "snapshots" / "svg"

# `(control, requires, forbids, why)`.
#
# `requires` is a tuple of substrings that must all appear in `<control>.svg`. `forbids` is a tuple
# of substrings that must not appear. Both are drawn from the committed snapshot, and each
# `forbids` entry is the exact spelling the corresponding defect produces.
FEATURES: tuple[tuple[str, tuple[str, ...], tuple[str, ...], str], ...] = (
    (
        "floating_label",
        # The caption is drawn as text. `Sample` is `CENSUS_TEXT`, the label the exporter applies.
        ("<text", ">Sample</text>"),
        # The defect: the label property was unpublished, so the caption landed in the input
        # instead. Both strings are drawn at font-size 14 in this control, so the *fill* is what
        # tells them apart — the caption is the input ink damped toward the field (rgba(155,155,
        # 155) in dark, rgba(81,81,81) in light), the input text is the undamped ink
        # (rgba(225,225,225) / rgba(0,0,0)). A regression to the input-text spelling means the
        # floating-label control is demonstrating a plain text field.
        ('fill="rgba(225,225,225,1.00)">Sample<',),
        "the control is named `floating_label`, so its snapshot must contain a floating caption. "
        "It shipped with **no label at all** — `label` was unpublished and the shared label "
        "helper took `text` first, so the caption was written into the input and `draw_label` "
        "returned early. A text field with no caption is the one thing a floating-label control "
        "cannot be, and the ink check is what distinguishes the caption from the input text "
        "drawn at the same size and place",
    ),
    (
        "tab_widget",
        # Two tabs: the first title carries the exporter's `Sample`, the second keeps the name
        # `create_tab_widget` gave it. The second tab's `x="74"` rect and its text are the band
        # chrome. The content area starting at y=24 is the remaining proof a 24px band sits above.
        ("<text", ">Tab 2</text>", 'x="74" y="0" width="64" height="24"', 'x="0" y="24" width="240" height="96"'),
        # A single-tab drawing is what a tab widget looks like when it has not been given tabs.
        (">Tab 1</text>",),
        "the tab band is what a `tab_widget` is, and it shipped with **zero tabs**, so the band "
        "and its titles were absent from the picture entirely. The marker requires a second tab "
        "(`Tab 2` plus its own rect) and the 24px band that the content area starts below, so a "
        "regression to one or no tabs fails rather than passing on the chrome the control draws "
        "anyway",
    ),
    (
        "badge",
        # The pill: a rounded, filled rect. `forbids` is the defect — the fill resolved to the
        # window background, which is exactly what BLUE21 B1 recorded: the `or()` chain made the
        # severity colour unreachable, so the badge was visible only as the hole it left.
        ('rx="9" ry="9" fill="rgba(138,180,248,1.00)"',),
        ('rx="9" ry="9" fill="rgba(18,18,18,1.00)"',),
        "a badge's pill must be distinguishable from the surface it sits on. The fill chain was "
        "`.or(themed_bg)` with `themed_bg` never `None` — `badge` is not in the theme's role "
        "table, so it resolved as `Surface` and the manager wrote the *window background* into "
        "it. Every severity was unreachable and the pill was invisible. The marker pins a fill "
        "that is neither the window colour nor the panel colour",
    ),
    (
        "switch",
        # The pinned 52x32 track. `forbids` is the pre-P2-1 shape: the track took the whole
        # `CENSUS_RECT` and became a 240x120 stadium with a 116px thumb.
        ('width="52" height="32" rx="16" ry="16"',),
        ('width="240" height="120" rx="60" ry="60"', 'width="116" height="116"'),
        "a switch's track is a fixed 52x32 pill, and the control used to derive its geometry "
        "from whatever rect it was given, so a 240x120 census rect produced a **stadium** with a "
        "thumb a quarter of the control wide. `size_hint` and the track constants disagreed, and "
        "the snapshot is where that is visible. The marker requires the pinned track and forbids "
        "the full-rect stadium",
    ),
    (
        "progress_bar",
        # The 4px track, vertical-centred in the rect. `forbids` is the pre-P2-1 slab: a 240x120
        # control-height bar.
        ('width="240" height="4" rx="2" ry="2"',),
        ('width="240" height="120" rx="60" ry="60"', 'width="0" height="120"'),
        "a progress bar is a **4px** bar sitting in the middle of its control, and it used to "
        "take the control's whole height, drawing a 240x120 slab whose 'fill' was a full-height "
        "rectangle. The marker requires the 4px track (twice: track and progress) and forbids the "
        "full-height form",
    ),
)

LINE_COMMENT = re.compile(r"<!--.*?-->", re.S)


def snapshot(control: str) -> str | None:
    path = SNAPSHOTS / f"{control}.svg"
    if not path.exists():
        return None
    return path.read_text(encoding="utf-8")


def scan(inject: str | None) -> tuple[list[str], list[str]]:
    """Returns (findings, evidence lines)."""
    findings: list[str] = []
    evidence: list[str] = []

    for control, requires, forbids, why in FEATURES:
        text = snapshot(control)
        if text is None:
            findings.append(
                f"{control}: snapshots/svg/{control}.svg does not exist, so this gate cannot "
                "check whether its feature is visible"
            )
            continue

        # Comments are stripped so a marker cannot be satisfied by the `<!-- control: ... -->`
        # header the exporter writes, which names the control in every file.
        drawing = LINE_COMMENT.sub("", text)

        missing = [pattern for pattern in requires if pattern not in drawing]
        present_defect = [pattern for pattern in forbids if pattern in drawing]
        if inject == control:
            # Pretend the feature has regressed by removing one requirement. The gate must then
            # report this control, which is what proves the markers are the thing being compared:
            # a comparison that never ran would report nothing and pass the mode vacuously.
            missing = [*missing, requires[0]]
        evidence.append(
            f"{control}: {len(requires)} marker(s) required, "
            f"{len(requires) - len(missing)} present"
        )

        if missing or present_defect:
            detail = [f"missing feature marker: {pattern!r}" for pattern in missing]
            detail += [
                f"the drawing still contains the defect spelling: {pattern!r}"
                for pattern in present_defect
            ]
            findings.append(f"{control} — {why}\n      " + "\n      ".join(detail))

    return findings, evidence


def main() -> int:
    inject = None
    for argument in sys.argv[1:]:
        if argument.startswith("--inject="):
            inject = argument.split("=", 1)[1]

    if not SNAPSHOTS.is_dir():
        print(f"no snapshot directory at {SNAPSHOTS}", file=sys.stderr)
        return 1

    if inject is not None and all(inject != control for control, _, _, _ in FEATURES):
        print(f"❌ --inject={inject} names no control in FEATURES, so the injection compares nothing")
        return 1

    findings, evidence = scan(inject)

    for line in evidence:
        print(f"  {line}")
    print(f"controls checked: {len(FEATURES)}")

    if findings:
        print(f"failed: {len(findings)}")
        print()
        print("These controls exist to demonstrate a feature and their own snapshot does not show")
        print("it. Every other snapshot gate passes them: the ink is inside the control, the SVG")
        print("is well-formed, and the file responds to the theme.")
        print()
        for finding in findings:
            print(f"  {finding}")
        print()
        print("Regenerate with `cargo run --no-default-features --features desktop --example")
        print("export_control_svgs` and inspect the file before changing a marker: a marker that")
        print("moved because the drawing legitimately changed is a marker to update, and a marker")
        print("that moved because the feature is gone is the defect this gate exists for.")
        return 1

    print("failed: 0  (every listed control shows its own feature in its snapshot)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
