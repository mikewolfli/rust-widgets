#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""The Python steps of `tools/check_svg_snapshots.sh`, one subcommand each.

# Why this is a module and not five heredocs

macOS's `/bin/bash` is 3.2, and 3.2 cannot parse a here-document inside a command substitution —
the same limitation `tools/implicit_size_scan.py` documents. Five of this gate's steps were written
that way, so the gate could not even be *parsed* on the project's development host: `bash -n`
reported "unexpected EOF while looking for matching `)`" at the first one, and the whole snapshot
check was unrunnable there while still appearing in the gate list. A gate nobody can run is a piece
of the verification that does not happen, so the blocks moved here.

Each subcommand prints exactly what the shell step consumes and exits 0. The shell wrapper owns the
pass/fail judgement, so a printed report and the exit code cannot disagree.
"""

import pathlib
import re
import sys

SNAPSHOT_DIR = "snapshots/svg"
EXPORTER = "examples/export_control_svgs.rs"
CENSUS_TEST = "tests/control_rendering_census_test.rs"


def default_features() -> None:
    """Prints the feature list the exporter should be built with.

    `_DEFAULT_FEATURES`, not `desktop`: the snapshot set is the picture of what a plain
    `cargo build` produces, and `default = ["desktop", "icons"]`. Rendering it from the `desktop`
    *profile* alone would leave two of those controls (`icon` among them) showing the shapes a
    build without the icon data draws — a picture that is current for nobody, since no caller gets
    `desktop` by itself without also asking for it. The gate's own contract is "what a reviewer
    sees is what ships", so the exporter runs what ships.
    """
    src = pathlib.Path("Cargo.toml").read_text(encoding="utf-8")
    match = re.search(r"^default\s*=\s*\[(.*?)\]", src, re.MULTILINE | re.DOTALL)
    names = re.findall(r'"([^"]+)"', match.group(1)) if match else []
    # A device profile is mutually exclusive and `default` names exactly one; the rest are carrying
    # features (`icons`). Keeping only the first profile avoids passing two at once.
    profiles = ["desktop", "tablet", "mobile", "embedded", "mini"]
    chosen = next((n for n in names if n in profiles), "desktop")
    print(",".join([chosen] + [n for n in names if n not in profiles]))


def _table_len(pattern: str) -> int:
    """How many `("a", "b"` rows the exporter's table matching `pattern` declares.

    A number written into the gate would have to be edited in lockstep with the table, and the
    failure mode of forgetting is the one this check exists to catch — a snapshot file that nothing
    accounts for. Reading the table means "which files exist" and "which states are declared" are
    the same fact.
    """
    src = pathlib.Path(EXPORTER).read_text(encoding="utf-8")
    match = re.search(pattern, src, re.DOTALL)
    if match is None:
        return 0
    return len(re.findall(r"\(\s*\x22[^\x22]+\x22\s*,", match.group(1)))


def extras() -> None:
    """Prints the number of declared extra appearances."""
    print(_table_len(r"EXTRA_APPEARANCES[^=]*=\s*&\[(.*?)\];"))


def pairs() -> None:
    """Prints the number of declared companion pairs."""
    print(_table_len(r"PAIRS[^=]*=\s*&\[(.*?)\];"))


def _exempted(path: str) -> set:
    """The first whitespace-separated field of each non-comment line of an exemption table.

    Read the same way the census test reads it, so the two gates cannot disagree about the same
    fact.
    """
    names = set()
    p = pathlib.Path(path)
    if not p.exists():
        return names
    for line in p.read_text(encoding="utf-8").splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        names.add(stripped.split()[0])
    return names


def empty() -> None:
    """Prints the snapshots that are not reviewable, one per line.

    One drawing element is enough to satisfy "not empty", but it is not enough to be a
    **reviewable** control: a bare frame plus one rect was the "light-board" class the survey
    (issue §16.1/§16.5) identified. The floor is therefore three elements, with an explicit
    whitelist for controls whose content *is* a single shape or a single text run.
    """
    drawing = re.compile(r"<(?:rect|circle|line|path|polygon|polyline|text|ellipse|image)\b")
    shape_only = {
        # Issue §16.5 A1 residuals: a line, an icon, a label, an empty window or a single
        # arc *is* the whole of these controls; asking for more would demand invented content.
        "action", "avatar", "command_link", "divider", "frame", "icon", "label",
        "line", "progress_circle", "window",
        # An image view's content *is* one raster: the snapshot carries the decoded pixels as a
        # single `<image>` element, so two elements (frame + raster) is the whole of the control.
        "image_view",
        # An icon's extra appearance is still one icon.
        "icon_info",
        # Skeleton placeholders are one shape by definition; the extra appearances prove the
        # other two shapes exist rather than adding a second element.
        "skeleton_loader", "skeleton_loader_circle",
        # A transparent swipe wrapper's content is its child label; the wrapper itself paints
        # no chrome until it is swiped.
        "swipe_to_dismiss",
    }
    # Content that is a **tree child** is not part of a single control Draw stream. These four
    # were blank after C2-M2 retired the registry path that used to paint the child inline, which
    # is the "looks degraded" regression a reviewer sees in control.md. The exporter mounts the
    # child explicitly now (compose_container_preview in examples/export_control_svgs.rs), so a
    # name here without any text means that arm was lost again. Keep the two lists in step.
    composite_content = {"splitter", "stacked_widget", "scroll_area", "mdi_area"}
    bad = []
    for path in sorted(pathlib.Path(SNAPSHOT_DIR).glob("*.svg")):
        text = path.read_text(encoding="utf-8")
        stem = path.stem.removesuffix(".light")
        count = len(drawing.findall(text))
        if "<svg" not in text:
            bad.append(f"{path}: no <svg> element")
        elif count == 0:
            bad.append(f"{path}: an empty skeleton with no drawing element")
        elif count < 3 and stem not in shape_only:
            bad.append(
                f"{path}: {count} drawing element(s); a reviewable control needs 3 "
                "unless it is in the shape-only whitelist"
            )
        if stem in composite_content and 'data-text="1"' not in text:
            bad.append(
                f"{path}: a composite container has no child text; its sample content was lost "
                "(see compose_container_preview)"
            )
    if bad:
        print("\n".join(bad))


def same() -> None:
    """Prints the controls whose two appearances render identically and are not exempt.

    A control whose two appearances render identically is theme-blind. That is the P3 judgement
    `tools/check_control_rendering.sh` already makes, with the same two exemption tables (data
    colours and recorded known-blind controls), so this step applies the SAME tables rather than
    inventing a third policy: an unrecorded control that renders the same in both files must be a
    *new* regression, and a *recorded* one must not be reported twice or the two gates would
    disagree about the same fact.

    The reason this is repeated here at all is that it is a different *measurement* of the same
    property: P3 compares dominant pixel colours after a real render, this compares the emitted SVG
    vector stream. The exporter bug this layer first shipped with was invisible to P3 — the census
    applies the theme itself, so it saw the colours change while the snapshots did not.
    """
    exempt = _exempted("tools/control_color_exemptions.txt") | _exempted(
        "tools/control_surface_coincidence_exemptions.txt"
    )
    # The recorded known-blind list lives in the census test, because it is a to-do list that must
    # shrink as the controls are fixed. Parsed rather than duplicated so the two cannot disagree
    # about what is still outstanding.
    census_src = pathlib.Path(CENSUS_TEST).read_text(encoding="utf-8")
    match = re.search(r"const KNOWN_THEME_BLIND: &\[&str\] = &\[(.*?)\];", census_src, re.S)
    known = set(re.findall(r'"([a-z0-9_]+)"', match.group(1))) if match else set()

    # An empty exemption set would silently pass everything, which is the vacuous-gate failure
    # mode: refuse to report success on a table that did not load.
    if not exempt:
        raise SystemExit("the exemption table could not be read, so this step cannot judge")

    # The extra appearances are single-appearance by design: they exist to show a *state* the two
    # default files cannot, and rendering each one in both themes would double the set for a
    # question the default pair already answers. They are therefore skipped rather than reported as
    # "no light companion", which is a real defect for a control and a category error for a state
    # file. Derived from the exporter table, not from the file name: a name pattern like `*_checked`
    # would silently absorb a future control that happened to end that way.
    extra_state = set()
    exporter = pathlib.Path(EXPORTER)
    if exporter.exists():
        match_extra = re.search(
            r"EXTRA_APPEARANCES[^=]*=\s*&\[(.*?)\];", exporter.read_text(encoding="utf-8"), re.S
        )
        if match_extra:
            pattern = r"\(\s*\"([^\"]+)\"\s*,\s*\"([^\"]+)\"\s*,\s*\"([^\"]+)\"\s*\)"
            for control, suffix, _ in re.findall(pattern, match_extra.group(1)):
                extra_state.add(f"{control}{suffix}")

    same_list = []
    for path in sorted(pathlib.Path(SNAPSHOT_DIR).glob("*.svg")):
        if path.name.endswith(".light.svg"):
            continue
        name = path.name[: -len(".svg")]
        if name in extra_state:
            continue
        # The icon sheets are a pair by file name, but they are not a *control*: `control.md` does
        # not list them and the theme-blind tables do not apply to them. Their pair is checked by
        # `tests/icon_census_test.rs` and by the fact that step 3 regenerates both files from one
        # run, so a sheet that stopped responding to the theme would show as a diff there.
        if name.startswith("icon_sheet"):
            continue
        light = path.with_name(path.name[: -len(".svg")] + ".light.svg")
        if not light.exists():
            same_list.append(f"{name}: no light companion")
            continue

        # The provenance comment names the appearance, so compare the drawing only. Comparing whole
        # files would report "differs" for every control, comment and all.
        def drawing(p):
            return [
                line
                for line in p.read_text(encoding="utf-8").splitlines()
                if not line.lstrip().startswith("<!--")
            ]

        if drawing(path) == drawing(light) and name not in exempt and name not in known:
            same_list.append(
                f"{name}: renders identically in light and dark, and is not exempt or recorded"
            )
    if same_list:
        print("\n".join(same_list))


COMMANDS = {
    "default-features": default_features,
    "extras": extras,
    "pairs": pairs,
    "empty": empty,
    "same": same,
}


def main(argv: list) -> int:
    if len(argv) != 2 or argv[1] not in COMMANDS:
        print(f"usage: {argv[0]} {{{'|'.join(COMMANDS)}}}", file=sys.stderr)
        return 2
    COMMANDS[argv[1]]()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
