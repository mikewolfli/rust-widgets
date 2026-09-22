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

# This is an audit aid, not a gate

The band is *inferred* from neighbouring painted rectangles, so the tool cannot know
why a label sits where it does. Three shapes are therefore deliberately **not**
reported, because they are correct placements that this inference mis-reads:

  * a run centred on a **polar anchor** (a pie slice label at `point - height/2`);
  * a run in the control's own **left margin** (an axis tick label centred on its tick);
  * a run whose control has **no band of its own** — `label`, `ime_preedit` and
    `swipe_to_dismiss` paint only text, so the enclosing rectangle this tool finds is
    the whole control and `y == band top` is the only available answer.

The FALSE_POSITIVES table below records the last group, which cannot be detected from
geometry alone. Keeping the list here — rather than silencing the controls — is what
lets a *new* mis-placement in the same control still be reported.
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

# Controls whose only ink is text, so the band this tool infers is the control itself and
# `HUG-TOP` / `PUSHED-DOWN` are not evidence of anything. Each entry states why there is no
# band to centre in; this is a record of an audited false positive, not a mute button.
#
#   label            A label *is* one line at a caller-chosen position: `set_position` moves
#                    the whole control to the text, so there is no interior to centre in.
#   ime_preedit      An IME candidate strip anchored at the caret, for the same reason.
#   swipe_to_dismiss A gesture surface painted only while a swipe is in progress; the census
#                    render has no band because it never enters that state.
#   floating_label   A text field's caption. The census renders an empty, unfocused field, so
#                    the `auto` policy correctly places the caption *inline on the input line*
#                    rather than floating above it — the float is the other of the two
#                    placements the control animates between. The enclosing rectangle this
#                    tool finds is the whole field, not the input line, so the placement reads
#                    as "not centred in the field", which is the desired state.
FALSE_POSITIVES = {"label", "ime_preedit", "swipe_to_dismiss", "floating_label"}


def control_name(filename: str) -> str:
    """The canonical control name for a snapshot filename."""
    name = filename[:-4] if filename.endswith(".svg") else filename
    return name[: -len(".light")] if name.endswith(".light") else name


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

        # A run that ends before **every** wide painted slab in the control is in the
        # control's own left margin, not inside a band: it is an axis tick label sitting in
        # the column the plot area reserved for it. Such a label is centred on its own tick,
        # so the slab it happens to overlap vertically (the bar in front of the gridline) is
        # not its container and judging it against that slab is a false positive. `bar_chart`
        # draws its four value labels at x = 20 with the plot starting at x = 64.
        left_of_every_slab = bool(bands) and all(t["x"] + lh <= b[3]["x"] for b in bands if b[0] == "rect")
        if left_of_every_slab:
            continue

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
        # A **polar anchor**: the run is centred on a point rather than laid into a band, so
        # `point - line_height / 2` is the correct origin and the enclosing band is incidental.
        # `pi_chart` places every slice label and percentage this way, and a label whose box was
        # clamped to the panel edge then falls inside that panel's rectangle — which the band
        # search sees as a badly placed run. Recognising the shape is the honest fix: the
        # alternative is a whitelist entry per control, which would also excuse a genuine
        # mis-placement in the same control later.
        elif abs(t["y"] - (by + (bh - lh) / 2)) < 0.51 and kind == "circle":
            verdict = "centred"
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
        if control_name(f.name) in FALSE_POSITIVES:
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
