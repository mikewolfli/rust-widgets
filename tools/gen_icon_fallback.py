#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Generate `src/widget/icon_fallback_data.rs` from the vendored Material Symbols SVGs.

# Why this generator exists

`icon_fallback_data.rs` is the geometry a build **without** the opt-in `icons` feature draws. It used
to be a hand-maintained table: `icon_data.rs` had a generator, the fallback did not, so the same
outlines were read by two paths — one generated, one transcribed. That is the shape principle #101
forbids (one concept, two implementations), and the file's own header admitted it was an unclosed
follow-up. This script closes it.

# Why the output is *polygons*, not `d`

The fallback exists precisely so a build without `icons` does not link the SVG parser (see the
module docs of `icon_data.rs` and `flatten.rs`). Emitting `d` here would pull the parser back in and
defeat the split. So this script flattens the curves itself and emits flat `(i16, i16)` vertices, on
the same 960-unit design grid as `icon_data.rs` — the two tables carry the **same numbers** for one
icon, told as curves (data) and as polygons (fallback).

# The flattening rule (mirrors `src/render/path/flatten.rs`)

Uniform recursion by the curve's flatness is what the renderer does, but reproducing its
device-pixel-relative tolerance here would tie this table to a pixel size, and the fallback is drawn
at whatever size the caller asks. Instead this uses a **fixed subdivision depth** — the same
"subdivide until flat, with a depth bound" shape, with the bound chosen so the visible error at icon
sizes is under a pixel (measured: see the round log). The depth is a named constant so the choice is
reviewable rather than scattered.

# Completeness guarantees (the same three `gen_icon_data.py` makes)

  1. **Every token has geometry** — a token with no vendor file is refused by name.
  2. **No two tokens share one picture** — every token's vertex list is hashed and a collision is
     refused, so the `Close == Cross` defect cannot re-enter through this table either.
  3. **The numbers are on the design grid** — vertices are design-space units, and the y axis is
     recorded as upstream has it (negative-up), so `flatten.rs`'s single mapping applies to both
     tables.

# Licence

The output is a derivative of Material Symbols (Apache-2.0, Google LLC), the same chain as
`icon_data.rs`. `--license=<id>` is **required** for the same reason: third-party geometry may not
enter the tree through a run that did not name the licence it relies on. This script is **offline** —
it reads only the vendor tree.

Usage:
    python3 tools/gen_icon_fallback.py --license=apache-2.0            # write the table
    python3 tools/gen_icon_fallback.py --license=apache-2.0 --check    # exit 1 if stale
"""

from __future__ import annotations

import argparse
import hashlib
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
VENDOR_DIR = REPO_ROOT / "tools" / "material_symbols"
OUT = REPO_ROOT / "src" / "widget" / "icon_fallback_data.rs"

sys.path.insert(0, str(Path(__file__).resolve().parent))
from gen_icon_data import (  # noqa: E402
    DECLARATION_ORDER,
    PATH_RE,
    REQUIRED_LICENCE,
    check_licence,
    read_vendor,
)
from vendor_material_symbols import UPSTREAM_SHA  # noqa: E402

# Curve subdivision depth. A cubic at depth d is drawn with 2**d chords; depth 4 gives 16 chords per
# curve segment, which holds the chord-to-curve deviation under a pixel for a 24 px icon (the
# smallest bundled size) without inflating the table. Bumping this only ever makes the fallback
# closer to the data path — `tests/icon_census_test.rs` compares the two.
SUBDIVIDE_DEPTH = 4

# The design grid every table shares. `icon_data.rs` writes it literally per entry; here it is one
# constant because the fallback has no per-icon variation today (and `grid()` below is where a future
# differing grid would be read).
GRID = 960


def flatten_token(token: str) -> list[list[tuple[float, float]]]:
    """Return `token`'s outline as a list of closed contours of design-space `(x, y)` points.

    The reading half is `gen_icon_data.py`'s — the same `PATH_RE` over the same vendored file — so
    the two tables are provably derived from one source rather than from two parses that could differ.
    """
    d = read_vendor(token)
    return flatten_d(d)


# One SVG path command plus its numbers. Matches `M mx my`, `c x1 y1 x2 y2 x y`, and the implicit
# repetitions SVG allows after a command.
CMD_RE = re.compile(r"([MmLlHhVvCcSsQqTtAaZz])([^MmLlHhVvCcSsQqTtAaZz]*)")
NUM_RE = re.compile(r"-?\d*\.?\d+(?:[eE][-+]?\d+)?")


def flatten_d(d: str) -> list[list[tuple[float, float]]]:
    """Parse `d` and return its closed contours as flattened design-space polygons.

    Supports the commands Material Symbols emits (`M L H V C S Q T Z` and their relative forms) — the
    same supported set as `src/render/path/parser.rs`. An unsupported command (an arc `A`) is refused
    rather than skipped: a partially read outline is a wrong icon.
    """
    contours: list[list[tuple[float, float]]] = []
    current: list[tuple[float, float]] = []
    pen = (0.0, 0.0)
    start = (0.0, 0.0)
    # The reflected control point for `S`/`T`, and which command produced it (reflection only applies
    # when the previous command was the matching curve family).
    last_ctrl: tuple[float, float] | None = None
    last_cmd = ""

    for cmd, body in CMD_RE.findall(d):
        nums = [float(n) for n in NUM_RE.findall(body)]

        if cmd in "Aa":
            raise SystemExit(
                f"REFUSED: an arc command `A` appeared in a vendored outline; this generator does "
                f"not implement arcs (see src/render/path/parser.rs for the renderer's own rule): "
                f"{d[:80]!r}"
            )

        if cmd in "Zz":
            if len(current) >= 3:
                contours.append(current)
            current = []
            pen = start
            last_ctrl = None
            last_cmd = cmd
            continue

        if cmd in "Mm":
            # `M` takes pairs; only the **first** pair moves the pen and starts the contour — the
            # rest are implicit linetos (the SVG rule, and the one `stop`'s `M320-640v320-320Z`
            # depends on). Starting a contour per pair instead would split one outline into a
            # degenerate point plus the real polygon.
            pairs = _pairs(nums)
            if not pairs:
                raise SystemExit(f"REFUSED: a bare `{cmd}` with no coordinates: {d[:80]!r}")
            if len(current) >= 3:
                contours.append(current)
            current = []
            for index, (nx, ny) in enumerate(pairs):
                point = (nx, ny) if cmd == "M" else (pen[0] + nx, pen[1] + ny)
                if index == 0:
                    start = point
                    current = [point]
                else:
                    current.append(point)
                pen = point
            last_ctrl = None
            last_cmd = cmd
            continue

        if cmd in "LlHhVv":
            # Explicit per command, with a total `else` that can only be `v`: an `elif` chain whose
            # last arm tested one command name left the *other* one falling through to the `else`
            # and moving the wrong axis — `H` moved y, so `stop`'s inner square collapsed.
            for (value, _unused) in _line_pairs(nums, cmd):
                if cmd == "L":
                    pen = (value, _unused)
                elif cmd == "l":
                    pen = (pen[0] + value, pen[1] + _unused)
                elif cmd in "Hh":
                    pen = (value, pen[1]) if cmd == "H" else (pen[0] + value, pen[1])
                elif cmd == "V":
                    pen = (pen[0], value)
                else:  # "v"
                    pen = (pen[0], pen[1] + value)
                current.append(pen)
            last_ctrl = None
            last_cmd = cmd
            continue

        if cmd in "CcSsQqTt":
            # A run of curves after one command letter: each chunk ends where the next begins, so the
            # pen advances within the group before the next command's state update.
            step = {"C": 6, "S": 4, "Q": 4, "T": 2}[cmd.upper()]
            if not nums:
                raise SystemExit(f"REFUSED: a bare `{cmd}` with no coordinates: {d[:80]!r}")
            if len(nums) % step:
                raise SystemExit(
                    f"REFUSED: `{cmd}` needs a multiple of {step} coordinates, got {len(nums)}"
                )
            relative = cmd.islower()
            for offset in range(0, len(nums), step):
                chunk = nums[offset : offset + step]
                pen, last_ctrl = _curve_step(
                    cmd, chunk, current, pen, last_ctrl, last_cmd
                )
            last_cmd = cmd
            continue

        raise SystemExit(f"REFUSED: unhandled command {cmd!r} in {d[:80]!r}")

    if len(current) >= 3:
        contours.append(current)
    return contours


def _pairs(nums: list[float], cmd: str = "L") -> list[tuple[float, float]]:
    if len(nums) % 2:
        raise SystemExit(f"REFUSED: an odd coordinate count ({len(nums)}) after `{cmd}`")
    return [(nums[i], nums[i + 1]) for i in range(0, len(nums), 2)]


def _line_pairs(nums: list[float], cmd: str) -> list[tuple[float, float]]:
    """One entry per iteration for `L`/`l`, or one per **value** for `H`/`h`/`V`/`v`.

    SVG lets a command repeat implicitly: `L x y x y` is two linetos, and so is `v 320 -320` — two
    vertical movers, not one. Both forms appear in the vendored outlines (`stop`'s `M320-640v320-320Z`
    is the one that exposed this), so a one-axis command must consume **all** its numbers rather than
    the first.

    The one-axis forms carry a single number each, so they cannot go through `_pairs` without
    inventing a second coordinate; the caller reads whichever axis the command names and ignores the
    other. Returning `(value, 0.0)` and reading the wrong axis was the earlier defect here — it turned
    `v320-320` into a single move and left `stop`'s first contour degenerate.
    """
    if cmd in "Ll":
        return _pairs(nums, cmd)
    return [(value, 0.0) for value in nums]


def _curve_step(
    cmd: str,
    chunk: list[float],
    current: list[tuple[float, float]],
    pen: tuple[float, float],
    last_ctrl: tuple[float, float] | None,
    last_cmd: str,
) -> tuple[tuple[float, float], tuple[float, float] | None]:
    """Append one curve to `current`; return the new pen and the control point `S`/`T` reflect.

    One function for all four curve commands, because they differ only in how many control points
    they carry and in whether `S`/`T` reflect the previous one. `C`/`S` are cubics and `Q`/`T` are
    quadratics; both are sampled at `SUBDIVIDE_DEPTH`.
    """
    upper = cmd.upper()
    relative = cmd.islower()
    p0 = pen
    if upper in ("C", "S"):
        if upper == "C":
            c1 = _abs(chunk[0], chunk[1], relative, p0)
        else:
            # `S`'s first control is the reflection of the previous cubic's second control — but only
            # when the previous command was a cubic, so `last_ctrl` is `None` otherwise and the
            # reflection degrades to the pen, which is SVG's rule.
            c1 = _reflect(p0, last_ctrl if last_cmd.upper() in ("C", "S") else None)
        c2 = _abs(chunk[-4], chunk[-3], relative, p0)
        end = _abs(chunk[-2], chunk[-1], relative, p0)
        for step in range(1, 2**SUBDIVIDE_DEPTH + 1):
            t = step / (2**SUBDIVIDE_DEPTH)
            current.append(_cubic_at(p0, c1, c2, end, t))
        return end, c2

    if upper == "Q":
        c1 = _abs(chunk[0], chunk[1], relative, p0)
    else:
        c1 = _reflect(p0, last_ctrl if last_cmd.upper() in ("Q", "T") else None)
    end = _abs(chunk[-2], chunk[-1], relative, p0)
    for step in range(1, 2**SUBDIVIDE_DEPTH + 1):
        t = step / (2**SUBDIVIDE_DEPTH)
        current.append(_quad_at(p0, c1, end, t))
    return end, c1


def _abs(x: float, y: float, relative: bool, pen: tuple[float, float]) -> tuple[float, float]:
    return (pen[0] + x, pen[1] + y) if relative else (x, y)


def _reflect(pen: tuple[float, float], ctrl: tuple[float, float] | None) -> tuple[float, float]:
    if ctrl is None:
        return pen
    return (2.0 * pen[0] - ctrl[0], 2.0 * pen[1] - ctrl[1])


def _emit_cubic(
    current: list[tuple[float, float]],
    p0: tuple[float, float],
    c1: tuple[float, float],
    c2: tuple[float, float],
    p1: tuple[float, float],
) -> tuple[float, float]:
    for step in range(1, 2**SUBDIVIDE_DEPTH + 1):
        t = step / (2**SUBDIVIDE_DEPTH)
        current.append(_cubic_at(p0, c1, c2, p1, t))
    return p1


def _emit_quad(
    current: list[tuple[float, float]],
    p0: tuple[float, float],
    c1: tuple[float, float],
    p1: tuple[float, float],
) -> tuple[float, float]:
    for step in range(1, 2**SUBDIVIDE_DEPTH + 1):
        t = step / (2**SUBDIVIDE_DEPTH)
        current.append(_quad_at(p0, c1, p1, t))
    return p1


def _cubic_at(
    p0: tuple[float, float],
    c1: tuple[float, float],
    c2: tuple[float, float],
    p1: tuple[float, float],
    t: float,
) -> tuple[float, float]:
    u = 1.0 - t
    return (
        u * u * u * p0[0] + 3 * u * u * t * c1[0] + 3 * u * t * t * c2[0] + t * t * t * p1[0],
        u * u * u * p0[1] + 3 * u * u * t * c1[1] + 3 * u * t * t * c2[1] + t * t * t * p1[1],
    )


def _quad_at(
    p0: tuple[float, float],
    c1: tuple[float, float],
    p1: tuple[float, float],
    t: float,
) -> tuple[float, float]:
    u = 1.0 - t
    return (
        u * u * p0[0] + 2 * u * t * c1[0] + t * t * p1[0],
        u * u * p0[1] + 2 * u * t * c1[1] + t * t * p1[1],
    )


def collect() -> list[tuple[str, list[list[tuple[float, float]]], str]]:
    """Return `(token, contours, digest)` for every icon, checking distinctness."""
    rows: list[tuple[str, list[list[tuple[float, float]]], str]] = []
    seen: dict[str, str] = {}
    for token in DECLARATION_ORDER:
        contours = flatten_token(token)
        if not contours:
            raise SystemExit(f"REFUSED: {token} flattened to no contours")
        canonical = ";".join(
            ",".join(f"{_round(x)},{_round(y)}" for x, y in contour) for contour in contours
        )
        digest = hashlib.sha256(canonical.encode("utf-8")).hexdigest()
        if digest in seen:
            raise SystemExit(
                "two icon tokens resolve to the same fallback picture (the `Close == Cross`\n"
                f"defect):\n    {seen[digest]} and {token}\n"
                "Give one of them a different upstream icon."
            )
        seen[digest] = token
        rows.append((token, contours, digest))
    return rows


def _round(value: float) -> int:
    """Design-space rounding: the table is `i16`, and a half unit is far below a device pixel."""
    return int(round(value))


def render(rows: list[tuple[str, list[list[tuple[float, float]]], str]]) -> str:
    count = len(rows)
    lines: list[str] = []
    lines.append(
        "// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)"
    )
    lines.append("// SPDX-License-Identifier: MIT")
    lines.append("//")
    lines.append("// GENERATED FILE — DO NOT EDIT BY HAND.")
    lines.append("// Produced by `tools/gen_icon_fallback.py` from the vendored Material Symbols")
    lines.append("// outlines. Regenerate with:")
    lines.append("//     python3 tools/gen_icon_fallback.py --license=apache-2.0")
    lines.append("//")
    lines.append("// Icon source: Material Symbols, by Google LLC — the per-icon SVG outlines from")
    lines.append("//     https://github.com/google/material-design-icons")
    lines.append(f"//     pinned at commit {UPSTREAM_SHA}")
    lines.append("// Licence of the derived geometry: Apache License 2.0 (Google LLC), the same chain as")
    lines.append("// `icon_data.rs`. The licence copy is shipped at `tools/material_symbols/LICENSE`; the")
    lines.append("// attribution is recorded in the repository-root `NOTICE`.")
    lines.append("//")
    lines.append("// # What this table is")
    lines.append("//")
    lines.append("// The geometry a build **without** the opt-in `icons` feature draws. It is a flattened")
    lines.append("// form of the *same* outlines `icon_data.rs` carries as SVG `d`, so the two tables cannot")
    lines.append(f"// describe different shapes for one token. Curves are sampled at depth {SUBDIVIDE_DEPTH}")
    lines.append("// (`gen_icon_fallback.py`), which keeps the fallback's deviation from the data path under a")
    lines.append("// pixel at icon sizes without linking the SVG parser — the parser's absence is the whole")
    lines.append("// reason this table holds polygons rather than `d`.")
    lines.append("//")
    lines.append("// `#[rustfmt::skip]` keeps `cargo fmt --check` stable over the generated table.")
    lines.append("")
    lines.append("/// One icon's fallback geometry: flattened polygons on the 960-unit design grid.")
    lines.append("///")
    lines.append("/// Owned here rather than borrowed as `d` so a build without `icons` does not link the SVG")
    lines.append("/// parser: the fallback draws polygons directly, which is what keeps its `RenderCommand`")
    lines.append("/// stream the same kind it always was.")
    lines.append("pub(crate) struct IconFallback {")
    lines.append("    /// The canonical token, matching `IconName::as_str`.")
    lines.append("    pub name: &'static str,")
    lines.append("    /// The design grid: `x` spans `0..grid`, `y` spans `-grid..0`.")
    lines.append("    pub grid: u16,")
    lines.append("    /// One entry per closed contour; each is a flat run of `(x, y)` pairs.")
    lines.append("    pub contours: &'static [&'static [(i16, i16)]],")
    lines.append("}")
    lines.append("")
    lines.append(
        f"/// Every icon's fallback geometry, indexed by `IconName`'s declaration order. {count} entries."
    )
    lines.append("#[rustfmt::skip]")
    lines.append(f"pub(crate) static ICON_FALLBACK: [IconFallback; {count}] = [")
    for token, contours, _digest in rows:
        lines.append("    IconFallback {")
        lines.append(f'        name: "{token}",')
        lines.append(f"        grid: {GRID},")
        lines.append("        contours: &[")
        for contour in contours:
            points = ", ".join(f"({_round(x)}, {_round(y)})" for x, y in contour)
            lines.append(f"            &[{points}],")
        lines.append("        ],")
        lines.append("    },")
    lines.append("];")
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--license",
        dest="license_id",
        help=f"the licence this run relies on for the inbound geometry (required: {REQUIRED_LICENCE})",
    )
    parser.add_argument("--check", action="store_true", help="exit 1 if the file is stale")
    args = parser.parse_args()

    # `--check` reads only what is already in the tree, so it does not need the inbound licence claim;
    # a writing run does, for the same reason `gen_icon_data.py` demands it.
    if not args.check:
        check_licence(args.license_id)

    rows = collect()
    rendered = render(rows)

    if args.check:
        if not OUT.exists():
            print(f"FAIL  {OUT.relative_to(REPO_ROOT)} does not exist", file=sys.stderr)
            return 1
        if OUT.read_text(encoding="utf-8") != rendered:
            print(
                f"FAIL  {OUT.relative_to(REPO_ROOT)} is stale; run\n"
                f"    python3 tools/gen_icon_fallback.py --license={REQUIRED_LICENCE}",
                file=sys.stderr,
            )
            return 1
        print(f"icon fallback OK: {len(rows)} icons, matches the vendored outlines")
        return 0

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(rendered, encoding="utf-8")
    vertices = sum(len(c) for _t, cs, _d in rows for c in cs)
    print(f"wrote {OUT.relative_to(REPO_ROOT)}: {len(rows)} icons, {vertices} vertices")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
