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
required patterns and must not contain the forbidden ones. Both halves matter: "a text run
exists" alone does not distinguish `floating_label`'s caption from its input text, while the
forbidden pattern states exactly what the defect looked like in the file.

# What a "text run" looks like in the snapshot

Text is no longer a `<text>` element: the SVG backend emits the **same `font8x8` rectangles the
software rasteriser fills**, one axis-aligned subpath per set bitmap bit, inside a single
`<path d="M{x} {y}h{w}v{h}h-{w}z...">`. Two consequences shape the markers below:

* the rendered string is **not in the document in any form**, so a marker cannot be `>Sample<`;
  a run has to be identified by its **geometry** (where its ink starts, `M<left> <top>`) or by its
  **fill**, which is what separates two runs drawn at the same size and place.
* `M<left> <top>` is a stable spelling for a run's **glyph-box top-left**, because the union of a
  run's subpaths spans the whole box and `left`/`top` are the box's own edges. A marker of the form
  `M9 13h` therefore means "a run whose glyph box starts at (9, 13)" — which is exactly the
  quantity a re-export changes when the feature moves or disappears.

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

A defect can be either "a wrong spelling appears" or "the right spelling is missing", and the two
are caught by the two halves: `requires` catches a missing feature, `forbids` catches a wrong one.
Where the defect is purely *absence* — the caption not drawn at all, the second tab never created —
`forbids` is empty rather than restated as a positive marker that the feature legitimately contains;
writing the feature's own spelling into `forbids` would make the row unsatisfiable.

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
        # The caption, drawn as a text run whose ink spans (8,13)-(56,27) and painted in the
        # *damped* ink (`rgba(155,155,155)` in dark, `rgba(81,81,81)` in light). Both markers are
        # needed: the geometry alone would also match a run that happened to land there, and the
        # fill alone would also match a differently-placed one.
        ('<path d="M9 13h', 'fill="rgba(155,155,155,1.00)"'),
        # The defect is *absence*: `draw_label` returned early, so there was no caption run at all
        # and the control was a plain text field. Verified by injecting `if true { return; }` at
        # the top of `draw_label` — the snapshot then contains not a single `<path>`. There is no
        # positive spelling to forbid, so the defect is expressed by the missing `requires`
        # markers above, and `forbids` is empty rather than restated as the thing that must exist.
        (),
        "the control is named `floating_label`, so its snapshot must contain a floating caption. "
        "It shipped with **no label at all** — `label` was unpublished and the shared label "
        "helper took `text` first, so the caption was written into the input and `draw_label` "
        "returned early. A text field with no caption is the one thing a floating-label control "
        "cannot be, and the ink colour is what distinguishes the caption from the input text "
        "drawn at the same size and place",
    ),
    (
        "tab_widget",
        # Two tabs, so two title runs on the 24px band, and the second tab's own `x="74"` rect.
        # The content area starting at y=24 is the remaining proof a 24px band sits above. The two
        # runs are required to be **distinct** (different glyph-box left edges), which is what a
        # second tab means and what a one-tab regression removes.
        #
        # # Why the second tab's numbers are now 88/76-62 (BLUE22 · §B.9)
        #
        # These were 87/74-64. The cause is **not** the layout and not the band: `tab_width_hints`
        # and `TabView::tab_widths` were still measuring a caption as `chars().count() * 8` -- a
        # fixed 8 px per cluster -- while every other label in the crate had moved to the shared
        # estimate (`metrics::estimate_text_width`, which is the renderer's own advance model: one
        # cluster at 0.6 em of the font size). `Tab 1` is five clusters, so the hand-rolled form said
        # 40+24 while the shared one says 42+24, and the two strips that exist in this crate were
        # each measuring their own captions differently from the text they draw.
        #
        # A hand-rolled byte-per-character estimate also mis-measures every non-Latin caption -- it
        # charges a fixed 8 px for a CJK cluster the renderer draws a full em wide -- which is the
        # defect class `check_implicit_size_uses_metrics` guards and the reason both tab controls now
        # read the shared estimate.
        #
        # This is a *legitimate* drawing change (a caption measured correctly), which is the case the
        # gate's own usage note calls a marker to update rather than a defect: verified by
        # re-exporting and reading the file, and by checking the feature is still present -- two
        # distinct title runs on a 24 px band with the content area below it. A one-tab regression
        # still removes the second run and the second rect, which is what the marker exists to catch.
        (
            '<path d="M13 5h',
            '<path d="M88 5h',
            'x="76" y="0" width="62" height="24"',
            'x="0" y="24" width="240" height="96"',
        ),
        # The defect is again *absence* rather than a wrong spelling: verified by removing the
        # second `add_tab` from `create_tab_widget`, which leaves only the `x="0"` rect and one
        # title run — the `requires` entries above fail on the missing second tab.
        (),
        "the tab band is what a `tab_widget` is, and it shipped with **zero tabs**, so the band "
        "and its titles were absent from the picture entirely. The marker requires a second tab "
        "(`Tab 2` as its own glyph-box run, plus its own rect) and the 24px band that the content "
        "area starts below, so a regression to one or no tabs fails rather than passing on the "
        "chrome the control draws anyway",
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
