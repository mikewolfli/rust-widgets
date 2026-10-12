// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! BLUE25 ICON-6 — the icon census gate, as assertions.
//!
//! # What this asserts, and why each one can fail
//!
//! The census renders **every** `IconName` through the real draw pipeline and records what came
//! out. This test turns the measurements into six assertions:
//!
//! | # | Assertion | The defect it catches |
//! |---|---|---|
//! | C1 | every token draws at least one subpath | a token whose data never reached the draw (the census would be blank for it) |
//! | C2 | no two tokens draw byte-identical pictures | the `Close == Cross` class: two names, one picture |
//! | C3 | the committed `tools/icon_census.txt` matches a fresh run | a data change nobody regenerated the census for |
//! | C4 | every token has a row, and every row names a token | a token that escapes the census, or a stale row for one that is gone |
//! | C5 | the counters of a compound icon stay transparent | a compound outline whose inner contour is painted as a separate solid polygon |
//! | C6 | the painted pixels are the non-zero winding fill of the emitted contours | a fill rule that paints *more* than the geometry asks for (§19.3.1) |
//!
//! # Why C5 and C6 exist, and why the earlier four could not see the defect
//!
//! BLUE27 §19.3.1 records the defect as measured, not predicted: `draw_outline_in` submitted each
//! contour separately and the scanline filler filled each polygon even-odd, so every counter was
//! painted over — `info` rendered as a solid disk. C1–C4 are all blind to it. They count subpaths
//! (4 is 4 either way), compare pictures (all four are distinct either way), compare the census
//! against a fresh run (which agrees with itself), and check coverage (every token still has one).
//! The census could not see it either, because `ink` was a copy of `contours` — it counted a
//! counter as ink, which is the same wrong model the filler had.
//!
//! So the evidence has to be about **pixels**: C5 checks two of them directly, and C6 checks all of
//! them against the fill rule, computed here by a crossing-number implementation that shares no
//! code with the rasteriser. C6 is deliberately not a hand-picked token list: it re-derives the
//! expected picture from the geometry the *other* backends receive, so it also fails if the SVG
//! backend and the software backend ever disagree about which contours an icon has.
//!
//! # Why C2 is a *rendering* comparison and not a data one
//!
//! `tests/icon_data_integrity_test.rs` already proves the `d` strings differ. That is necessary and
//! not sufficient: two different `d` values can still flatten to one polygon set — through a
//! transform, a scale, or a flattening bug — and the user would see one picture under two names.
//! C2 compares the emitted SVG, which is the picture.
//!
//! # Why the census is committed
//!
//! A gate that only re-derives its expectation cannot detect drift: it would compute today's answer
//! and agree with itself. The committed `tools/icon_census.txt` is the *record*; C3 requires it to
//! still equal a fresh run, so a change in what an icon draws appears as a diff in review.

// Gated on `widgets_unstripped` rather than on `not(mini)`: this file renders the `Icon` **widget**,
// and the widget set is what a sized profile drops. With `icons` on and a sized profile the payload
// is present but there is no `Icon` to render, so `not(mini)` claimed a configuration the file
// cannot run in — the same shape of false gate that let a build failure read as a defect elsewhere.
#![cfg(all(feature = "icons", widgets_unstripped))]

use std::collections::BTreeMap;

use rust_widgets::core::{Color, Rect, Size};
use rust_widgets::render::{PaintBackend, RenderContext, SoftwarePaintBackend};
use rust_widgets::widget::draw::Draw;
use rust_widgets::widget::svg::render_to_svg;
use rust_widgets::widget::{Icon, IconName};

/// Where the committed census lives, relative to the crate root.
const CENSUS_PATH: &str = "tools/icon_census.txt";

/// The frame the census rasterises into, in pixels.
const SIDE: u32 = 24;

/// One icon's census row.
struct Row {
    token: &'static str,
    contours: usize,
    ink: usize,
    runs: usize,
    digest: String,
    svg: String,
    /// Painted pixels of the 24 px software rendering, row-major.
    painted: Vec<bool>,
}

/// Renders every token and measures it.
fn census() -> Vec<Row> {
    IconName::ALL
        .into_iter()
        .map(|token| {
            let mut icon = Icon::new(Rect::new(0, 0, SIDE, SIDE));
            icon.set_icon_enum(token);
            let svg = render_to_svg(&mut icon);
            let contours = svg.matches("M ").count();
            let (ink, runs, painted) = measure_ink(&mut icon);
            Row {
                token: token.as_str(),
                contours,
                ink,
                runs,
                digest: sha256_hex(svg.as_bytes()),
                svg,
                painted,
            }
        })
        .collect()
}

/// Draws `icon` into a fresh software frame and measures the ink it laid down.
///
/// This is the same measurement `examples/icon_census.rs` makes, restated here because a test
/// cannot import from an example; C3 is what keeps the two honest — if they disagreed, the
/// committed census would read as stale.
fn measure_ink(icon: &mut Icon) -> (usize, usize, Vec<bool>) {
    let mut backend = SoftwarePaintBackend::new(Size::new(SIDE, SIDE), 1.0);
    backend.begin_frame(Color::TRANSPARENT);
    {
        let mut context = RenderContext::new(&mut backend);
        icon.draw(&mut context);
    }
    backend.end_frame();
    let pixels = backend.frame_rgba();
    let painted: Vec<bool> = (0..SIDE * SIDE).map(|i| pixels[(i * 4 + 3) as usize] > 0).collect();

    let ink = painted.iter().filter(|&&hit| hit).count();
    let mut runs = 0;
    let mut inside = false;
    for x in 0..SIDE as usize {
        let hit = painted[(SIDE as usize / 2) * SIDE as usize + x];
        if hit && !inside {
            runs += 1;
        }
        inside = hit;
    }
    (ink, runs, painted)
}

/// C1 — a tokened icon that draws nothing is an icon the user cannot see.
#[test]
fn c1_every_icon_draws_an_outline() {
    for row in census() {
        assert!(
            row.contours > 0,
            "{} drew no outline subpath (the data did not reach the draw):\n{}",
            row.token,
            row.svg
        );
        assert!(
            row.ink > 0,
            "{} laid down no ink: {} contours reached the backend and none of them painted a pixel",
            row.token,
            row.contours
        );
    }
}

/// C2 — two distinct names must not draw one picture.
#[test]
fn c2_no_two_icons_draw_one_picture() {
    let mut seen: BTreeMap<String, &'static str> = BTreeMap::new();
    for row in census() {
        if let Some(other) = seen.insert(row.digest.clone(), row.token) {
            panic!(
                "{other} and {} render byte-identical pictures, so two distinct icon names draw \
                 one icon",
                row.token
            );
        }
    }
}

/// C3 — the committed census must equal a fresh run.
#[test]
fn c3_the_committed_census_matches_a_fresh_render() {
    let fresh = render_census_text();
    let committed = std::fs::read_to_string(CENSUS_PATH).unwrap_or_else(|error| {
        panic!("{CENSUS_PATH} could not be read ({error}); regenerate the icon census")
    });
    assert_eq!(
        normalise(&fresh),
        normalise(&committed),
        "{CENSUS_PATH} is stale. Regenerate with:\n    \
         cargo run --no-default-features --features desktop,icons --example icon_census > {CENSUS_PATH}"
    );
}

/// C4 — the census covers exactly `IconName::ALL`, in order, with nothing extra.
#[test]
fn c4_the_census_covers_every_token_once() {
    let rows = census();
    assert_eq!(rows.len(), IconName::ALL.len());
    for (row, token) in rows.iter().zip(IconName::ALL) {
        assert_eq!(row.token, token.as_str(), "the census order must follow `IconName::ALL`");
        assert!(row.ink > 0, "{} laid down no ink", row.token);
    }
    // A duplicate token would mean `ALL` repeats a variant, which would also hide a missing one.
    let mut tokens: Vec<&str> = rows.iter().map(|row| row.token).collect();
    tokens.sort_unstable();
    let before = tokens.len();
    tokens.dedup();
    assert_eq!(before, tokens.len(), "`IconName::ALL` repeats a token");
}

/// C5 — every compound icon the plan names must keep its counters as empty space.
///
/// Two measurements, because either alone can be satisfied while the picture is wrong. The pixel
/// check is the direct one: a point inside `info`'s ring must come out unpainted. The `runs` check
/// is the shape one: a ring crossed by the middle scanline yields two or more opaque segments, a
/// solid disk yields one. A token that is a disk *and* has an unpainted pixel somewhere (an
/// unrelated gap) would pass the first and fail the second.
#[test]
fn c5_compound_icons_keep_their_counters_transparent() {
    let info = Rect::new(0, 0, SIDE, SIDE);
    let mut icon = Icon::new(info);
    icon.set_icon_enum(IconName::Info);
    let (_, _, painted) = measure_ink(&mut icon);
    assert!(
        !painted[12 * SIDE as usize + 7],
        "the interior of the info counter must remain unpainted"
    );

    // BLUE27 §19.13 I16: these four are the ring-bearing tokens named by the plan.
    let mut seen: Vec<&str> = Vec::new();
    for row in census() {
        if !matches!(row.token, "info" | "settings" | "mail" | "star") {
            continue;
        }
        seen.push(row.token);
        assert!(
            row.runs >= 2,
            "{} is a ring, so the middle scanline must cross at least two opaque segments; it \
             crossed {} — the counters were painted over",
            row.token,
            row.runs
        );
    }
    seen.sort_unstable();
    assert_eq!(
        seen,
        ["info", "mail", "settings", "star"],
        "the plan's ring-bearing tokens must all be present in the census"
    );
}

/// C6 — the painted pixels must be the non-zero winding fill of the emitted contours.
///
/// # Why a winding oracle in a test is not a second rasteriser
///
/// The rasteriser answers "which pixels does this contour set cover"; the helper below answers
/// "what does the winding rule say about this one point", by counting signed ray crossings. They
/// are different algorithms over the same input, which is what makes agreement evidence — a copy of
/// the rasteriser's own loop would only show that it is deterministic.
///
/// # Why the margin
///
/// A pixel centre within `MARGIN` of an edge is skipped: coverage and antialiasing at a boundary
/// are the rasteriser's business and its answer there may legitimately differ from a point test.
/// What this asserts is the *rule*, not the edge policy.
///
/// # Why the floors
///
/// Half of this test is a measurement of the icon set rather than of a token: how many tokens
/// paint *less* than their contours would suggest, because the winding rule erased a counter. On
/// this set that is 33 tokens and 1784 pixels. If a change made the counters unobservable — the
/// data flattening a ring into a disk, say — the assertions above would keep passing while
/// measuring nothing, so the floors fail instead.
#[test]
fn c6_painted_pixels_are_the_non_zero_winding_fill() {
    const MARGIN: f32 = 1.0;
    const MIN_TOKENS_THAT_ERASE: usize = 30;
    const MIN_ERASED_PIXELS: usize = 1500;
    const PLAN_RING_TOKENS: [&str; 4] = ["info", "mail", "settings", "star"];

    let mut erasing_tokens = 0usize;
    let mut erased_pixels = 0usize;
    for row in census() {
        let contours = compound_contours(&row.svg);
        assert_eq!(
            contours.len(),
            row.contours,
            "{}: the census counted {} contours, the emitted path parses to {}",
            row.token,
            row.contours,
            contours.len()
        );
        let mut erased_here = 0usize;
        for y in 0..SIDE {
            for x in 0..SIDE {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                if distance_to_contours(&contours, px, py) < MARGIN {
                    continue;
                }
                let winding = winding_number(&contours, px, py);
                let painted = row.painted[(y * SIDE + x) as usize];
                assert_eq!(
                    painted,
                    winding != 0,
                    "{}: pixel ({x}, {y}) is painted={painted} while its winding number is {winding}",
                    row.token
                );
                if winding == 0 && inside_any_contour(&contours, px, py) {
                    erased_here += 1;
                }
            }
        }
        if erased_here > 0 {
            erasing_tokens += 1;
            erased_pixels += erased_here;
        }
        if PLAN_RING_TOKENS.contains(&row.token) {
            assert!(
                erased_here > 0,
                "{}: the winding fill erases nothing, so the counter is gone before it is drawn — \
                 the ring was flattened into a solid shape",
                row.token
            );
        }
    }
    assert!(
        erasing_tokens >= MIN_TOKENS_THAT_ERASE,
        "only {erasing_tokens} tokens erase ink that the per-contour fill would have painted, \
         expected at least {MIN_TOKENS_THAT_ERASE}: the counters are no longer exercised"
    );
    assert!(
        erased_pixels >= MIN_ERASED_PIXELS,
        "only {erased_pixels} pixels are erased by the winding rule, expected at least \
         {MIN_ERASED_PIXELS}: the counters are no longer exercised"
    );
}

/// The closed contours of the icon in `svg`, as `(x, y)` points.
///
/// Reads what the SVG backend emitted — the same contour list every other backend is handed — so a
/// disagreement about *which* contours an icon has shows up here instead of hiding behind a second
/// reading of the source data. Every qualifying `<path` is read, not just the first: one fill
/// command emits one path, so a backend that submitted the contours separately would hand the
/// software rasteriser a different picture, and that has to reach the comparison rather than
/// stopping at the parse.
fn compound_contours(svg: &str) -> Vec<Vec<(f32, f32)>> {
    let mut contours = Vec::new();
    for element in svg.split("<path ").skip(1) {
        let element = match element.find("/>") {
            Some(end) => &element[..end],
            None => element,
        };
        // A **text run** is emitted as a `<path` too. The icon's own fill is the one with the
        // non-zero rule and no `data-text`; anywhere else in this file the same rule identifies it.
        if element.contains("data-text") || !element.contains("fill-rule=\"nonzero\"") {
            continue;
        }
        let Some(open) = element.find("d=\"") else {
            continue;
        };
        let rest = &element[open + 3..];
        let Some(close) = rest.find('"') else {
            continue;
        };
        contours.extend(parse_path_data(&rest[..close]));
    }
    contours
}

/// The closed contours of one SVG path's `d` attribute.
fn parse_path_data(data: &str) -> Vec<Vec<(f32, f32)>> {
    data.split("M ")
        .map(str::trim)
        .filter(|subpath| !subpath.is_empty())
        .map(|subpath| {
            let numbers: Vec<f32> = subpath
                .split(|character: char| {
                    !(character.is_ascii_digit() || character == '-' || character == '.')
                })
                .filter_map(|number| number.parse().ok())
                .collect();
            let mut points: Vec<(f32, f32)> =
                numbers.as_chunks::<2>().0.iter().map(|pair| (pair[0], pair[1])).collect();
            // The emitter may or may not repeat the first point before `Z`; a repeated point is a
            // zero-length edge to the winding test, so drop it and let the closing edge be implicit.
            if points.len() > 1 && points[0] == points[points.len() - 1] {
                points.pop();
            }
            points
        })
        .collect()
}

/// The non-zero winding number of `contours` around `(px, py)`: signed crossings of a +x ray.
fn winding_number(contours: &[Vec<(f32, f32)>], px: f32, py: f32) -> i32 {
    let mut winding = 0;
    for contour in contours {
        for (index, &(x0, y0)) in contour.iter().enumerate() {
            let (x1, y1) = contour[(index + 1) % contour.len()];
            let side = (x1 - x0) * (py - y0) - (px - x0) * (y1 - y0);
            if y0 <= py {
                if y1 > py && side > 0.0 {
                    winding += 1;
                }
            } else if y1 <= py && side < 0.0 {
                winding -= 1;
            }
        }
    }
    winding
}

/// Whether `(px, py)` is inside any single contour — the union the per-contour fill used to paint.
fn inside_any_contour(contours: &[Vec<(f32, f32)>], px: f32, py: f32) -> bool {
    contours.iter().any(|contour| inside_contour(contour, px, py))
}

/// An even-odd crossing test for one closed contour.
fn inside_contour(contour: &[(f32, f32)], px: f32, py: f32) -> bool {
    let mut inside = false;
    for (index, &(x0, y0)) in contour.iter().enumerate() {
        let (x1, y1) = contour[(index + 1) % contour.len()];
        if (y0 > py) != (y1 > py) {
            let along = (py - y0) / (y1 - y0);
            if px < x0 + along * (x1 - x0) {
                inside = !inside;
            }
        }
    }
    inside
}

/// The distance from `(px, py)` to the nearest contour edge.
fn distance_to_contours(contours: &[Vec<(f32, f32)>], px: f32, py: f32) -> f32 {
    let mut nearest = f32::MAX;
    for contour in contours {
        for (index, &(x0, y0)) in contour.iter().enumerate() {
            let (x1, y1) = contour[(index + 1) % contour.len()];
            let (dx, dy) = (x1 - x0, y1 - y0);
            let length_squared = dx * dx + dy * dy;
            let along = if length_squared == 0.0 {
                0.0
            } else {
                (((px - x0) * dx + (py - y0) * dy) / length_squared).clamp(0.0, 1.0)
            };
            let distance =
                ((px - (x0 + along * dx)).powi(2) + (py - (y0 + along * dy)).powi(2)).sqrt();
            nearest = nearest.min(distance);
        }
    }
    nearest
}

/// The census body, exactly as `examples/icon_census.rs` prints it, minus the header comment.
fn render_census_text() -> String {
    let mut out = String::new();
    for row in census() {
        out.push_str(&format!(
            "{}\tcontours={}\tink={}\truns={}\tsha256={}\n",
            row.token, row.contours, row.ink, row.runs, row.digest
        ));
    }
    out
}

/// Strips the leading comment block and normalises line endings, so the comparison is of rows.
fn normalise(text: &str) -> String {
    text.replace("\r\n", "\n")
        .lines()
        .filter(|line| !line.starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
}

/// A SHA-256 of `data`, hex-encoded.
///
/// The same hand-rolled digest the probe uses. It is duplicated rather than shared because a test
/// cannot import from an example: the two must agree, and `c3` is what enforces that — a divergence
/// in the digest would show up as a stale census immediately.
fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut message = data.to_vec();
    let bit_len = (data.len() as u64) * 8;
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in message.chunks(64) {
        let mut w = [0u32; 64];
        for (index, word) in chunk.chunks(4).enumerate() {
            w[index] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for index in 16..64 {
            let s0 = w[index - 15].rotate_right(7)
                ^ w[index - 15].rotate_right(18)
                ^ (w[index - 15] >> 3);
            let s1 = w[index - 2].rotate_right(17)
                ^ w[index - 2].rotate_right(19)
                ^ (w[index - 2] >> 10);
            w[index] = w[index - 16].wrapping_add(s0).wrapping_add(w[index - 7]).wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for index in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let temp1 =
                hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[index]).wrapping_add(w[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    let mut out = String::with_capacity(64);
    for word in h {
        out.push_str(&format!("{word:08x}"));
    }
    out
}
