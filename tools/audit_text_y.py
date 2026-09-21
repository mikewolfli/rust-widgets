#!/usr/bin/env python3
"""Audit SVG snapshots for vertically mis-placed single-line text.

Model of the renderer (verified against snapshots/svg/wizard_dialog.svg, whose
geometry is known-correct):

  * `draw_text_fitted(bounds, ...)` calls `draw_text(Point::new(..., bounds.y))`.
  * `SvgPaintBackend` emits `<text y="{origin.y}">` verbatim.
  * The glyph box therefore runs DOWNWARD from `y` for one line height
    `round(font.size() * dpi)` -- the origin is the glyph's TOP edge.

So for a label drawn with `draw_text_fitted(Rect::new(x, Y, w, h))`:

    glyph box = [Y, Y + round(size)]
    proper centring in a band [B, B + H] requires Y == B + (H - line_height) / 2

Two wrong shapes are detectable straight from the numbers:

  DEFECT-hug : Y == band top (bounds passed unchanged; label on the top edge)
  DEFECT-down: Y - round(size) is the *centred* value (ascent added on top of
               an already-centred y; glyph box one line too low)
"""
import re
import sys
import pathlib
from collections import defaultdict

TEXT_RE = re.compile(r"<text (?P<attrs>[^>]*?)>(?P<body>.*?)</text>", re.S)
ATTR_RE = re.compile(r'(\w[\w-]*)="([^"]*)"')
RECT_RE = re.compile(r"<rect (?P<attrs>[^>]*?)/>")
LINE_RE = re.compile(r"<line (?P<attrs>[^>]*?)/>")
CIRCLE_RE = re.compile(r"<circle (?P<attrs>[^>]*?)/>")

R = pathlib.Path("snapshots") / "svg"


def attrs(m):
    return dict(ATTR_RE.findall(m.group("attrs")))


def nums(a, keys):
    out = {}
    for k in keys:
        if k not in a:
            return None
        try:
            out[k] = float(a[k])
        except ValueError:
            return None
    return out


def parse(path):
    src = path.read_text()
    texts, rects, lines, circles = [], [], [], []
    for m in TEXT_RE.finditer(src):
        a = nums(attrs(m), ["y", "x", "font-size"])
        if a:
            a["size"] = a.pop("font-size")
            a["weight"] = attrs(m).get("font-weight", "")
            a["body"] = m.group("body")
            texts.append(a)
    for m in RECT_RE.finditer(src):
        a = nums(attrs(m), ["x", "y", "width", "height"])
        if a:
            a["fill"] = attrs(m).get("fill", "")
            a["stroke"] = attrs(m).get("stroke", "")
            rects.append(a)
    for m in LINE_RE.finditer(src):
        a = nums(attrs(m), ["x1", "y1", "x2", "y2"])
        if a:
            lines.append(a)
    for m in CIRCLE_RE.finditer(src):
        a = nums(attrs(m), ["cx", "cy", "r"])
        if a:
            a["fill"] = attrs(m).get("fill", "")
            circles.append(a)
    return texts, rects, lines, circles


def is_opaque(fill: str) -> bool:
    """A fill that is actually visible ink (not `none`, not fully transparent)."""
    if not fill or fill == "none":
        return False
    m = re.search(r",([0-9.]+)\)$", fill)
    return (float(m.group(1)) if m else 1.0) > 0.05


def classify(ctrl, texts, rects, lines, circles, verbose=False):
    """Find the enclosing band for each text run and judge its vertical placement."""
    findings = []
    for t in texts:
        lh = round(t["size"])
        if lh <= 0:
            continue
        top, bot = t["y"], t["y"] + lh
        # Candidate bands: horizontal painted slabs in the background, i.e. rects or
        # circles that contain the run horizontally and are tall enough to be a box.
        bands = []
        for r in rects:
            if r["width"] < 8 or not is_opaque(r["fill"]):
                continue
            # a band is a capsule/rounded body: must span the control's width or be
            # tall enough to be a button (>= 1.5 line heights)
            if r["height"] >= lh * 1.4:
                bands.append(("rect", r["y"], r["height"], r))
        for c in circles:
            bands.append(("circle", c["cy"] - c["r"], c["r"] * 2, c))

        verdict = None
        best = None
        for kind, by, bh, src in bands:
            # the text run's glyph box, when correctly centred, sits inside the band
            centred = by + (bh - lh) / 2
            if kind == "rect":
                inside = by - 2 <= top and bot <= by + bh + 2
            else:
                inside = by <= top and bot <= by + bh
            if not inside:
                continue
            slack = min(abs(t["y"] - centred), abs(t["y"] - by))
            if best is None or slack < best[0]:
                best = (slack, kind, by, bh, centred, src)
        if best is None:
            if verbose:
                print(f"  [no band] y={t['y']:.0f} size={t['size']:.0f} {t['body'][:30]!r}")
            continue
        slack, kind, by, bh, centred, src = best
        if abs(t["y"] - centred) < 0.51:
            verdict = "centred"
        elif abs(t["y"] - by) < 0.51:
            verdict = "HUG-TOP"
        elif abs(t["y"] - (centred + lh)) < 0.51:
            verdict = "PUSHED-DOWN"
        elif abs(t["y"] - (by + lh)) < 1.01:
            verdict = "PUSHED-DOWN"
        if verbose and verdict != "centred":
            print(
                f"  y={t['y']:.0f} size={t['size']:.0f} band=[{by:.0f},{by + bh:.0f}] "
                f"centred={centred:.0f} lh={lh} -> {verdict} {t['body'][:30]!r}"
            )
        if verdict and verdict != "centred":
            findings.append(
                dict(
                    control=ctrl,
                    y=t["y"],
                    size=t["size"],
                    band=(by, bh),
                    centred=centred,
                    lh=lh,
                    verdict=verdict,
                    label=t["body"][:30],
                )
            )
    return findings


def main():
    verbose = "-v" in sys.argv
    filt = next((a for a in sys.argv[1:] if not a.startswith("-")), None)
    all_findings = []
    scanned = 0
    for f in sorted(R.glob("*.svg")):
        if filt and filt not in f.name:
            continue
        scanned += 1
        texts, rects, lines, circles = parse(f)
        if not texts:
            continue
        found = classify(f.name, texts, rects, lines, circles, verbose)
        all_findings.extend(found)
    print(f"\nscanned {scanned} files; suspicious placements: {len(all_findings)}")
    for x in all_findings:
        by, bh = x["band"]
        print(
            f"  {x['control']:<44} {x['verdict']:<12} y={x['y']:.0f} "
            f"band=[{by:.0f},{by + bh:.0f}] centred={x['centred']:.0f} size={x['size']:.0f} "
            f"{x['label']!r}"
        )


if __name__ == "__main__":
    main()
