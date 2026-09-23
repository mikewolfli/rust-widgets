// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! BLUE20 layer 1 — the rendering census gate, as assertions.
//!
//! # What this asserts, and why each one can fail
//!
//! The census renders **every** control the factory publishes, twice (light and
//! dark), and this test turns the measurements into four assertions:
//!
//! | # | Assertion | The defect it catches |
//! |---|---|---|
//! | P1 | the control painted ≥1 pixel ≠ its background | laid out off-canvas (round 58 defect A) |
//! | P2 | its dominant colour ≠ its background | painted, but in the window's colour (defect D) |
//! | P3 | light dominant ≠ dark dominant | chrome is hardcoded, so a theme switch does nothing (defect C) |
//! | P4 | each semantic token has a reader, and dark ≠ light | `theme.colors.{error,…}` declared and unread (§1.4) |
//! | P5 | every painted element stays inside the control's own box | geometry derived from a coordinate space the painter is not in |
//!
//! # Why these are *relative* judgements
//!
//! Nothing here asserts a literal colour. "This control is `#2196f3`" is a fact
//! about today's theme; "this control's fill is not the colour behind it" is the
//! fact the user can see, and it stays true when the palette changes. Literals
//! live only in the baseline file, where they are diffed, not asserted.
//!
//! # Why the widget set is walked by name
//!
//! 13 `WidgetKind`s are shared by 2–5 controls each, so walking kinds would skip
//! 19 controls. The set comes from `WidgetFactory::widget_names()`, which is
//! derived from the same table the factory constructs from — it cannot under-count.
//!
//! # Known-invisible set
//!
//! 23 controls currently paint nothing at the census geometry. They are listed
//! explicitly in `KNOWN_INVISIBLE` so the count cannot quietly grow: a new
//! invisible control fails the test, and removing one from the list is the only
//! way to record a fix. The same shape is used for `KNOWN_THEME_BLIND`.
//!
//! # Why P5 exists (and why P1–P4 could not see the defect it catches)
//!
//! P1–P4 are all measured from a **raster** — ink counts and modal colours — and a
//! raster is bounded by the surface it was rendered into. A control that paints
//! *outside* its own box therefore produces a **normal-looking** census: the
//! out-of-bounds pixels are clipped away, the in-bounds ones are counted, and
//! nothing is wrong. The SVG snapshots are the opposite: they carry absolute
//! coordinates and no bound at all, so the same defect shows up there as drawing
//! that escapes the picture — which is exactly how `group_box` shipped a title whose
//! text sat at `y = -8` (over half of it above the frame) while every raster
//! assertion passed.
//!
//! Two independent geometries agreeing is the point of the layer: P5 renders through
//! the SVG backend and requires each element it emits to be **wholly** inside the
//! control's rectangle, which is the one property both backends must share.

// The census needs the theme module and the full widget registry, which exist only on a
// **device** profile (`desktop`/`tablet`/`mobile`). `not(mini)` was too weak: `embedded` is
// also stripped of both, so the test failed to compile there with `cannot find theme in
// rust_widgets`. Naming the requirement directly is what keeps this true when a profile is
// added, rather than a `not(...)` list that has to be extended.
#![cfg(all(not(feature = "mini"), not(feature = "embedded"), not(target_arch = "wasm32")))]

use rust_widgets::theme::theme_test_guard;
use rust_widgets::widget::census::{
    census_all_controls, install_preset_appearances, ControlCensus, CENSUS_RECT, CENSUS_TEXT,
};

/// Controls that paint **nothing** at the census geometry.
///
/// Each entry is a control whose `Draw` produces no pixel distinguishable from
/// the background. The list is asserted against the census in both directions:
/// a control here that *does* paint fails, and a control not here that paints
/// nothing fails. So this is a to-do list that can only shrink by fixing, never
/// by silent drift.
const KNOWN_INVISIBLE: &[&str] = &[];

/// Controls whose dominant colour is identical in light and dark, and are **not**
/// exempted as data colours.
///
/// These are hardcoded-chrome defects: a theme switch leaves them unchanged. The
/// list is asserted in both directions like [`KNOWN_INVISIBLE`].
const KNOWN_THEME_BLIND: &[&str] = &[];

/// Controls whose painted body is **defined** to be the surface they sit on.
///
/// Parsed from `tools/control_surface_coincidence_exemptions.txt`, so the reason for
/// each entry lives next to the exemption. Separate from the data-colour table
/// because it exempts a different judgement: that file excuses P3 (invariant across
/// appearances), this one excuses P2 (coincident with the surface).
fn surface_coincidence_exemptions() -> Vec<String> {
    let Ok(text) = std::fs::read_to_string("tools/control_surface_coincidence_exemptions.txt")
    else {
        return Vec::new();
    };
    text.lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_string)
        .collect()
}

/// Controls whose dominant colour may legitimately not change with appearance.
///
/// Parsed from `tools/control_color_exemptions.txt`, so the reason for each entry
/// lives next to the exemption rather than in this file. A control named in that
/// table is excused from P3; nothing else is.
fn data_color_exemptions() -> Vec<String> {
    let Ok(text) = std::fs::read_to_string("tools/control_color_exemptions.txt") else {
        return Vec::new();
    };
    text.lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_string)
        .collect()
}

/// Controls whose drawing legitimately leaves their own rectangle.
///
/// Parsed from `tools/control_overflow_exemptions.txt`, so the reason for each entry
/// lives next to the exemption rather than in this file. Read the same way the other two
/// exemption tables are read: the first whitespace-separated field of each non-comment
/// line. It is currently empty, and that is a result rather than a default — every
/// flagged control was examined and fixed (see the table's own header).
fn overflow_exemptions() -> Vec<String> {
    let Ok(text) = std::fs::read_to_string("tools/control_overflow_exemptions.txt") else {
        return Vec::new();
    };
    text.lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_string)
        .collect()
}

/// One drawing element's bounds, as the SVG backend emitted them.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ElementBounds {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

/// Extracts the bounds of every element in an SVG document.
///
/// Deliberately **not** a full SVG parser: the backend emits a closed vocabulary of
/// element shapes, and this reads the geometry attributes those shapes use. An element
/// whose bounds cannot be determined is reported as a failure by the caller rather than
/// silently skipped, because "unmeasurable" would otherwise be indistinguishable from
/// "in bounds" — the vacuous-gate shape rule #107 forbids.
///
/// `<rect>`/`<circle>`/`<line>`/`<text>` and the `<path>` command stream are all covered;
/// `<g>`, `<clipPath>`, `<filter>`, `<linearGradient>` and the `</...>` closers carry no
/// geometry of their own and are skipped.
fn element_bounds(svg: &str) -> Vec<(String, Option<ElementBounds>)> {
    let mut out = Vec::new();
    for raw in svg.lines() {
        let line = raw.trim();
        let tag = line.split([' ', '>', '/']).next().unwrap_or("");
        match tag {
            "<rect" => {
                let (x, y) = (attr(line, "x="), attr(line, "y="));
                let (w, h) = (attr(line, "width="), attr(line, "height="));
                let (Some(x), Some(y), Some(w), Some(h)) = (x, y, w, h) else {
                    out.push((line.to_string(), None));
                    continue;
                };
                out.push((line.to_string(), Some(bounds(x as f32, y as f32, w as f32, h as f32))));
            }
            "<circle" => {
                let (Some(cx), Some(cy), Some(r)) =
                    (attr(line, "cx="), attr(line, "cy="), attr(line, "r="))
                else {
                    out.push((line.to_string(), None));
                    continue;
                };
                let (cx, cy, r) = (cx as f32, cy as f32, r as f32);
                // The stroke is centred on the radius, so the painted extent is half a
                // stroke wider than the circle's own geometry.
                let stroke = attr(line, "stroke-width=").map(|w| w as f32 / 2.0).unwrap_or(0.0);
                out.push((
                    line.to_string(),
                    Some(bounds(
                        cx - r - stroke,
                        cy - r - stroke,
                        r * 2.0 + stroke * 2.0,
                        r * 2.0 + stroke * 2.0,
                    )),
                ));
            }
            "<line" => {
                let (x1, y1) = (attr(line, "x1="), attr(line, "y1="));
                let (x2, y2) = (attr(line, "x2="), attr(line, "y2="));
                let (Some(x1), Some(y1), Some(x2), Some(y2)) = (x1, y1, x2, y2) else {
                    out.push((line.to_string(), None));
                    continue;
                };
                let (x1, y1, x2, y2) = (x1 as f32, y1 as f32, x2 as f32, y2 as f32);
                // A line's `stroke-width` is absolute in SVG, so half of it sits outside the
                // chord on each side. Defaulting to a *width* rather than to a half-width is
                // what keeps a 1 px border flush with `x = 0` from reading as an escape.
                let half_stroke =
                    attr(line, "stroke-width=").map(|w| w as f32 / 2.0).unwrap_or(1.0);
                out.push((
                    line.to_string(),
                    Some(bounds(
                        x1.min(x2) - half_stroke,
                        y1.min(y2) - half_stroke,
                        (x2 - x1).abs() + half_stroke * 2.0,
                        (y2 - y1).abs() + half_stroke * 2.0,
                    )),
                ));
            }
            "<text" => {
                let (Some(x), Some(y)) = (attr(line, "x="), attr(line, "y=")) else {
                    out.push((line.to_string(), None));
                    continue;
                };
                // The emitted `y` is a **baseline**: this crate's `draw_text` takes a glyph
                // box *top* edge, and the SVG backend converts it by adding the ascent it
                // measured (`baseline_y = origin.y + ascent`). Emitting an explicit baseline is
                // what removed the `dominant-baseline="text-before-edge"` attribute — SVG 2
                // dropped that value, and a keyword a renderer only maps "for backwards
                // compatibility" cannot be what makes two backends agree about where ink is.
                //
                // So the box's top edge is `y - ascent`, and the extent is (advance x line
                // height) measured *downward* from there. Reading `y` as the top edge would
                // report every label `ascent` too low.
                //
                // The advance must be the **same model the renderer uses**, not an estimate.
                // `PaintBackend::shape_text` gives one cluster per `char`, each advancing by
                // `estimate_cluster_advance`: a wide scalar takes the full em, everything
                // else `0.6` em, and a space `0.33` em. Assuming one whole em per character
                // reported a 106 px title as 182 px, which demanded that ~40 controls
                // truncate text that fits perfectly well — a gate whose judgement is wrong
                // in the strict direction is as good as one that passes everything.
                let size = attr(line, "font-size=").map(|s| s as f32).unwrap_or(14.0);
                let label = element_text(line);
                let advance: f32 = label.chars().map(|ch| glyph_advance(ch, size)).sum();
                // The same `ascent` the backend adds, from the same rule: `line_height` is the
                // font size, and the ascent is 80% of it (`PaintBackend::measure_text`).
                let line_height = size.max(1.0).round();
                let ascent = (line_height * 0.8).round();
                let top = y as f32 - ascent;
                out.push((
                    line.to_string(),
                    Some(bounds(x as f32, top, advance.max(glyph_advance('M', size)), line_height)),
                ));
            }
            "<path" => {
                let Some(d) = attr_text(line, "d=") else {
                    out.push((line.to_string(), None));
                    continue;
                };
                match path_bounds(&d) {
                    Some(b) => out.push((line.to_string(), Some(b))),
                    None => out.push((line.to_string(), None)),
                }
            }
            _ => {}
        }
    }
    out
}

fn bounds(x: f32, y: f32, w: f32, h: f32) -> ElementBounds {
    ElementBounds { left: x, top: y, right: x + w, bottom: y + h }
}

/// The advance the renderer's text shaper gives one scalar at `size`.
///
/// A mirror of `render::pipeline::estimate_cluster_advance`, which is `pub(crate)` and so not
/// reachable from an integration test. It is repeated rather than approximated because P5's
/// whole value is that it measures what is drawn: an estimate that disagrees with the shaper
/// either excuses a real overflow or demands truncation that is not needed.
///
/// Mirroring is the reason this is four lines and not a formula: the shaper's rule is exactly
/// "a wide scalar takes the full em, a space a third, everything else 60%", and a cluster of
/// several scalars (a combining mark, a ZWJ sequence) takes the widest member's factor.
fn glyph_advance(ch: char, size: f32) -> f32 {
    if ch.is_whitespace() {
        (size * 0.33).max(1.0)
    } else if is_wide_scalar(ch) {
        size.max(1.0)
    } else {
        (size * 0.6).max(1.0)
    }
}

/// Whether `ch` occupies a full em in the renderer's advance model.
///
/// The same ranges the shaper treats as wide: the CJK blocks, the full-width forms, and the
/// emoji planes. A private list here would drift from the renderer's, so it is kept to the
/// ranges a control's own labels can actually contain.
fn is_wide_scalar(ch: char) -> bool {
    let code = ch as u32;
    matches!(code,
        0x1100..=0x115F      // Hangul Jamo initial consonants
        | 0x2E80..=0x303E    // CJK radicals, Kangxi, CJK symbols
        | 0x3041..=0x33FF    // Hiragana, Katakana, CJK compatibility
        | 0x3400..=0x4DBF    // CJK extension A
        | 0x4E00..=0x9FFF    // CJK unified ideographs
        | 0xA000..=0xA4CF    // Yi
        | 0xAC00..=0xD7A3    // Hangul syllables
        | 0xF900..=0xFAFF    // CJK compatibility ideographs
        | 0xFE30..=0xFE6F    // CJK compatibility forms
        | 0xFF00..=0xFF60    // Full-width forms
        | 0xFFE0..=0xFFE6
        | 0x1F300..=0x1FAFF // Emoji and pictographs
    )
}

/// Reads a numeric attribute, `None` when absent or not numeric.
fn attr(line: &str, prefix: &str) -> Option<f64> {
    attr_text(line, prefix)?.parse::<f64>().ok()
}

/// Reads a quoted attribute's value verbatim.
fn attr_text(line: &str, prefix: &str) -> Option<String> {
    let start = line.find(prefix)? + prefix.len();
    let rest = line[start..].trim_start_matches('"');
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// The text content of an SVG text element.
fn element_text(line: &str) -> String {
    let Some(open) = line.find('>') else {
        return String::new();
    };
    let rest = &line[open + 1..];
    match rest.find("</text>") {
        Some(end) => rest[..end].to_string(),
        None => rest.to_string(),
    }
}

/// The bounding box of an SVG path command stream, or `None` when it carries no
/// absolute drawing commands this reader understands.
///
/// Handles the two verbs the backend emits for shapes — `M x y` and `L x y` — plus the arc form
/// `A rx ry rot large sweep x y`, whose own end point is what the backend positions. A `Z` close
/// contributes nothing.
///
/// # The text form
///
/// Text is emitted as one `<path>` of axis-aligned rectangles, one per set `font8x8` bitmap bit,
/// in the **compact relative** form `M{x} {y}h{w}v{h}h-{w}z` with no spaces between the verbs:
///
/// ```text
/// M8 8h1v1h-1z M9 8h1v1h-1z M8 9h1v2h-1z
/// ```
///
/// (The spaces shown are the subpath separators the emitter writes; within a subpath the numbers
/// run straight into the next verb, which is why the token splitter above cannot parse it.) The
/// union of those rectangles **is** the run's ink, so a reader that skipped this form would leave
/// every text path unmeasured — and P5 treats an unmeasured element as a failure rather than a
/// pass, because a skipped input is not a passing one. That is what this branch exists for.
///
/// Each subpath is accumulated as it is walked, so the rectangles are bounded exactly rather than
/// approximated: `h`/`v` move the pen relatively, `H`/`V` absolutely, and every corner the pen
/// passes through is folded into the box. An unrecognised verb skips its numeric operand so the
/// scan cannot read a coordinate as a command letter.
fn path_bounds(d: &str) -> Option<ElementBounds> {
    if let Some(bounds) = relative_rect_path_bounds(d) {
        return Some(bounds);
    }
    let tokens: Vec<&str> = d.split_whitespace().collect();
    let mut xs: Vec<f32> = Vec::new();
    let mut ys: Vec<f32> = Vec::new();
    let mut index = 0usize;
    while index < tokens.len() {
        match tokens[index] {
            "M" | "L" => {
                let (x, y) = (tokens.get(index + 1)?, tokens.get(index + 2)?);
                xs.push(x.parse::<f32>().ok()?);
                ys.push(y.parse::<f32>().ok()?);
                index += 3;
            }
            "A" => {
                // `A rx ry rot large sweep x y`: the arc bulges up to `rx`/`ry` away from
                // the chord, so the conservative bound is the end point inflated by both
                // radii. Over-approximating can only make P5 stricter, never laxer.
                let (rx, ry) = (tokens.get(index + 1)?, tokens.get(index + 2)?);
                let (x, y) = (tokens.get(index + 6)?, tokens.get(index + 7)?);
                let (rx, ry) = (rx.parse::<f32>().ok()?, ry.parse::<f32>().ok()?);
                let (x, y) = (x.parse::<f32>().ok()?, y.parse::<f32>().ok()?);
                xs.push(x - rx.abs());
                xs.push(x + rx.abs());
                ys.push(y - ry.abs());
                ys.push(y + ry.abs());
                index += 8;
            }
            _ => index += 1,
        }
    }
    if xs.is_empty() || ys.is_empty() {
        return None;
    }
    let left = xs.iter().copied().fold(f32::INFINITY, f32::min);
    let right = xs.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let top = ys.iter().copied().fold(f32::INFINITY, f32::min);
    let bottom = ys.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    Some(ElementBounds { left, top, right, bottom })
}

/// Bounds a path written in the compact relative form the text emitter uses.
///
/// Returns `None` when the stream contains no `M`, which is the case for every shape path in
/// this backend — those are emitted with spaces and are handled by the token walker instead. The
/// `None` is therefore "this is not the text form", not "this failed to parse".
fn relative_rect_path_bounds(d: &str) -> Option<ElementBounds> {
    if !d.contains('M') {
        return None;
    }
    let bytes = d.as_bytes();
    let mut index = 0usize;
    let mut xs: Vec<f32> = Vec::new();
    let mut ys: Vec<f32> = Vec::new();
    // The pen, and whether it has been placed by an `M` yet.
    let mut pen: Option<(f32, f32)> = None;
    while index < bytes.len() {
        let verb = bytes[index];
        index += 1;
        match verb {
            b'M' => {
                let (x, y, next) = number_pair(d, index)?;
                pen = Some((x, y));
                xs.push(x);
                ys.push(y);
                index = next;
            }
            b'm' => {
                let (dx, dy, next) = number_pair(d, index)?;
                let (x, y) = pen?;
                pen = Some((x + dx, y + dy));
                xs.push(x + dx);
                ys.push(y + dy);
                index = next;
            }
            b'h' | b'H' | b'v' | b'V' => {
                let (value, next) = number(d, index)?;
                let (x, y) = pen?;
                let moved = match verb {
                    b'h' => (x + value, y),
                    b'H' => (value, y),
                    b'v' => (x, y + value),
                    _ => (x, value),
                };
                pen = Some(moved);
                xs.push(moved.0);
                ys.push(moved.1);
                index = next;
            }
            b'z' | b'Z' | b' ' | b',' | b'\t' | b'\n' | b'\r' => {}
            _ => {
                if let Some((_, next)) = number(d, index) {
                    index = next;
                }
            }
        }
    }
    if xs.is_empty() || ys.is_empty() {
        return None;
    }
    Some(ElementBounds {
        left: xs.iter().copied().fold(f32::INFINITY, f32::min),
        top: ys.iter().copied().fold(f32::INFINITY, f32::min),
        right: xs.iter().copied().fold(f32::NEG_INFINITY, f32::max),
        bottom: ys.iter().copied().fold(f32::NEG_INFINITY, f32::max),
    })
}

/// Reads a run of digits, with an optional sign, starting at `at`.
fn number(text: &str, at: usize) -> Option<(f32, usize)> {
    let bytes = text.as_bytes();
    let mut index = at;
    while index < bytes.len() && (bytes[index] == b' ' || bytes[index] == b',') {
        index += 1;
    }
    let start = index;
    if index < bytes.len() && (bytes[index] == b'-' || bytes[index] == b'+') {
        index += 1;
    }
    let digits_start = index;
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        index += 1;
    }
    if index == digits_start {
        return None;
    }
    text[start..index].parse::<f32>().ok().map(|value| (value, index))
}

/// Reads two numbers separated by whitespace or a comma.
fn number_pair(text: &str, at: usize) -> Option<(f32, f32, usize)> {
    let (first, after_first) = number(text, at)?;
    let (second, after_second) = number(text, after_first)?;
    Some((first, second, after_second))
}

#[test]
fn p5_every_control_paints_inside_its_own_box() {
    let _guard = theme_test_guard();
    install_preset_appearances();
    let factory = rust_widgets::widget::WidgetFactory::new_with_defaults();
    let exemptions = overflow_exemptions();

    let mut escaped: Vec<String> = Vec::new();
    let mut unmeasurable: Vec<String> = Vec::new();
    let mut no_longer_overflowing: Vec<String> = Vec::new();

    for name in factory.widget_names() {
        rust_widgets::theme::global_theme_manager()
            .set_appearance(rust_widgets::theme::AppearanceMode::Dark);
        let Some(mut widget) = factory.create(name, CENSUS_RECT, CENSUS_TEXT) else {
            continue;
        };
        rust_widgets::theme::apply_theme_to_widget(widget.as_mut());
        let Some(drawable) = rust_widgets::widget::draw_bridge::draw_of(widget.as_mut()) else {
            continue;
        };
        let svg = rust_widgets::widget::svg::render_widget_to_svg(drawable, CENSUS_RECT);

        // A half-pixel of slack: the SVG attributes are integral, but a stroke's own
        // half-width is fractional and a bound that rounds the wrong way would report a
        // perfectly placed border as an escape. A 1 px border centred on `x = 0` reaches
        // `x = -0.5`, which must be classified as *on the edge*, not as leaving the box.
        let slack = 0.5f32;
        let left = CENSUS_RECT.x as f32 - slack;
        let top = CENSUS_RECT.y as f32 - slack;
        let right = (CENSUS_RECT.x + CENSUS_RECT.width as i32) as f32 + slack;
        let bottom = (CENSUS_RECT.y + CENSUS_RECT.height as i32) as f32 + slack;

        let exempt = exemptions.iter().any(|exempted| exempted == name);
        let mut overflowed = false;
        for (element, element_bounds) in element_bounds(&svg) {
            let Some(b) = element_bounds else {
                unmeasurable.push(format!("{name}: {element}"));
                continue;
            };
            if b.left < left || b.top < top || b.right > right || b.bottom > bottom {
                overflowed = true;
                if !exempt {
                    escaped.push(format!(
                        "{name}: [{:.0},{:.0}..{:.0},{:.0}] escapes the box \n             {}",
                        b.left, b.top, b.right, b.bottom, element
                    ));
                }
            }
        }
        // The reverse direction: an exemption is a licence to overflow, and a licence that
        // is no longer needed would wave through a future regression on the same control.
        if exempt && !overflowed {
            no_longer_overflowing.push(name.to_string());
        }
    }

    assert!(
        unmeasurable.is_empty(),
        "these elements' bounds could not be determined, so this assertion cannot claim \
         they are inside the control — teach `element_bounds` the element rather than \
         letting it pass unmeasured (a skipped input is not a passing one):\n{}",
        unmeasurable.join("\n")
    );
    assert!(
        escaped.is_empty(),
        "these controls paint outside their own rectangle. The raster backends clip it \
         away, so P1-P4 cannot see it; the SVG snapshot shows it as drawing that leaves \
         the picture. A geometry built from the wrong coordinate space (a child-space \
         rect used as an absolute one, or a label centred on an edge instead of inside) \
         is the usual cause:\n{}",
        escaped.join("\n")
    );
    assert!(
        no_longer_overflowing.is_empty(),
        "these controls no longer paint outside their box, so their P5 exemption is now \
         a blanket permission for a future regression — remove them from \
         tools/control_overflow_exemptions.txt: {no_longer_overflowing:?}"
    );
}

/// Renders the whole registry once and returns it in canonical-name order.
fn census() -> Vec<ControlCensus> {
    let _guard = theme_test_guard();
    install_preset_appearances();
    census_all_controls()
}

#[test]
fn every_published_control_is_measured() {
    let rows = census();
    // The number is the registry's own, not a literal: if the registry grows, this
    // test measures the growth rather than failing on it.
    let expected = rust_widgets::widget::WidgetFactory::new_with_defaults().widget_names().len();
    assert_eq!(
        rows.len(),
        expected,
        "the census must cover every registered control, not a sample"
    );
    let mut names: Vec<&str> = rows.iter().map(|row| row.name).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), rows.len(), "each control must appear exactly once");
}

#[test]
fn p1_every_control_paints_something_unless_known_invisible() {
    let rows = census();
    let mut unexpected = Vec::new();
    for row in &rows {
        let invisible = !row.light.paints_anything();
        let known = KNOWN_INVISIBLE.contains(&row.name);
        if invisible && !known {
            unexpected.push(row.name);
        }
    }
    assert!(
        unexpected.is_empty(),
        "these controls painted nothing and are not recorded as known-invisible: {unexpected:?}"
    );

    // The reverse direction: a control recorded as invisible that now paints must
    // be removed from the list, or the list would keep claiming a defect is open.
    let mut fixed = Vec::new();
    for name in KNOWN_INVISIBLE {
        if let Some(row) = rows.iter().find(|row| row.name == *name) {
            if row.light.paints_anything() {
                fixed.push(*name);
            }
        }
    }
    assert!(
        fixed.is_empty(),
        "these controls now paint but are still listed as invisible — remove them: {fixed:?}"
    );
}

#[test]
fn p2_every_visible_control_differs_from_its_background() {
    let rows = census();
    let surface_exemptions = surface_coincidence_exemptions();
    let mut failures = Vec::new();
    let mut unexpectedly_exempt = Vec::new();
    for row in &rows {
        // P1 governs "did it paint at all" (probe render); P2 governs "can the user
        // see it where it sits" (surface render). A control that paints nothing is
        // P1's finding, so it is not double-reported here.
        if !row.light.paints_anything() {
            continue;
        }
        let exempt = surface_exemptions.iter().any(|name| name == row.name);
        let visible = row.visible_against_its_surface();
        if !visible && !exempt {
            failures.push(row.name);
        }
        // The reverse direction: an exemption that is no longer needed must be
        // removed, or the list would keep excusing a defect that has been fixed.
        if visible && exempt {
            unexpectedly_exempt.push(row.name);
        }
    }
    assert!(
        failures.is_empty(),
        "these controls paint, but only in the colour of the surface they sit on, \
         so the user cannot see them: {failures:?}"
    );
    assert!(
        unexpectedly_exempt.is_empty(),
        "these controls are visible against their surface and no longer need a \
         P2 exemption — remove them from \
         tools/control_surface_coincidence_exemptions.txt: {unexpectedly_exempt:?}"
    );
}

#[test]
fn p3_chrome_follows_the_appearance_unless_exempted_as_data() {
    let rows = census();
    let exemptions = data_color_exemptions();
    assert!(
        !exemptions.is_empty(),
        "the exemption table must be readable; an empty table would silently \
         exempt nothing and every data-coloured chart would fail"
    );

    // The four controls that carry *semantic* colours must never be exempted: if
    // they were, "the theme declares four tokens and nobody reads them" would be
    // permanently legalised.
    for forbidden in ["banner", "calendar", "progress_dialog", "message_box"] {
        assert!(
            !exemptions.iter().any(|name| name == forbidden),
            "{forbidden} carries a semantic colour and must not be data-exempt"
        );
    }

    let mut unexpected = Vec::new();
    for row in &rows {
        if exemptions.iter().any(|name| name == row.name) {
            continue;
        }
        if !row.differs_between_appearances() && !KNOWN_THEME_BLIND.contains(&row.name) {
            unexpected.push(row.name);
        }
    }
    assert!(
        unexpected.is_empty(),
        "these controls render identically in light and dark and are not recorded \
         as theme-blind nor exempted as data colours: {unexpected:?}"
    );

    // The reverse direction. An exemption is a licence to render the same colour in
    // both appearances; once the control's chrome follows the theme the licence
    // excuses nothing, and leaving it in place would wave through a future
    // regression. So an exempted control that now differs is a finding, exactly like
    // a `KNOWN_THEME_BLIND` entry whose control has been fixed.
    let mut stale = Vec::new();
    for name in &exemptions {
        let Some(row) = rows.iter().find(|row| row.name == name) else {
            continue;
        };
        if row.differs_between_appearances() {
            stale.push(name.clone());
        }
    }
    assert!(
        stale.is_empty(),
        "these data-colour exemptions are no longer needed — the control now follows \
         the appearance, so remove them from tools/control_color_exemptions.txt: {stale:?}"
    );
}

#[test]
fn p4_every_semantic_token_is_consumed_and_moves_with_the_appearance() {
    let rows = census();

    // A banner is the control that reads all four tokens, so its four severities
    // are the probe. Each must paint, and each must differ between appearances —
    // which is only true if the token it reads differs between appearances.
    let banner = rows
        .iter()
        .find(|row| row.name == "banner")
        .expect("the banner control must be registered");
    assert!(
        banner.light.paints_anything(),
        "the banner must paint, or the semantic tokens have no visible consumer"
    );
    assert!(
        banner.differs_between_appearances(),
        "the banner must change with the appearance, which proves it reads a token \
         rather than a literal"
    );

    // Each token must resolve to a colour under the active theme, and the light and
    // dark presets must give different colours — otherwise "dark ≠ light" for a
    // control reading it would be impossible.
    let _guard = theme_test_guard();
    install_preset_appearances();
    let mut manager = rust_widgets::theme::global_theme_manager();
    manager.set_appearance(rust_widgets::theme::AppearanceMode::Light);
    let light: Vec<_> = rust_widgets::theme::SemanticColor::ALL
        .iter()
        .map(|token| token.of(manager.current_theme().expect("a theme is active")))
        .collect();
    manager.set_appearance(rust_widgets::theme::AppearanceMode::Dark);
    let dark: Vec<_> = rust_widgets::theme::SemanticColor::ALL
        .iter()
        .map(|token| token.of(manager.current_theme().expect("a theme is active")))
        .collect();

    for (index, token) in rust_widgets::theme::SemanticColor::ALL.iter().enumerate() {
        assert_ne!(
            light[index],
            dark[index],
            "semantic token `{}` resolves to the same colour in light and dark, so a \
             control reading it could not respond to a theme switch",
            token.token()
        );
    }
}

#[test]
fn known_theme_blind_matches_the_census() {
    let rows = census();
    let exemptions = data_color_exemptions();

    let mut fixed = Vec::new();
    for name in KNOWN_THEME_BLIND {
        let Some(row) = rows.iter().find(|row| row.name == *name) else {
            continue;
        };
        if row.differs_between_appearances() {
            fixed.push(*name);
        }
    }
    assert!(
        fixed.is_empty(),
        "these controls now follow the appearance but are still listed as \
         theme-blind — remove them: {fixed:?}"
    );

    // And the reverse: a control that stopped following the appearance without
    // being recorded fails, so a regression cannot hide behind the list.
    let mut regressed = Vec::new();
    for row in &rows {
        if exemptions.iter().any(|name| name == row.name) || KNOWN_THEME_BLIND.contains(&row.name) {
            continue;
        }
        if row.light.paints_anything() && !row.differs_between_appearances() {
            regressed.push(row.name);
        }
    }
    assert!(regressed.is_empty(), "these controls stopped following the appearance: {regressed:?}");
}
