// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! BLUE25 ICON-6 — the icon census gate, as assertions.
//!
//! # What this asserts, and why each one can fail
//!
//! The census renders **every** `IconName` through the real draw pipeline and records what came
//! out. This test turns the measurements into four assertions:
//!
//! | # | Assertion | The defect it catches |
//! |---|---|---|
//! | C1 | every token draws at least one outline | a token whose data never reached the draw (the census would be blank for it) |
//! | C2 | no two tokens draw byte-identical pictures | the `Close == Cross` class: two names, one picture |
//! | C3 | the committed `tools/icon_census.txt` matches a fresh run | a data change nobody regenerated the census for |
//! | C4 | every token has a row, and every row names a token | a token that escapes the census, or a stale row for one that is gone |
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

#![cfg(all(feature = "icons", not(feature = "mini")))]

use std::collections::BTreeMap;

use rust_widgets::core::Rect;
use rust_widgets::widget::svg::render_to_svg;
use rust_widgets::widget::{Icon, IconName};

/// Where the committed census lives, relative to the crate root.
const CENSUS_PATH: &str = "tools/icon_census.txt";

/// One icon's census row.
struct Row {
    token: &'static str,
    contours: usize,
    ink: usize,
    digest: String,
    svg: String,
}

/// Renders every token and measures it.
fn census() -> Vec<Row> {
    IconName::ALL
        .into_iter()
        .map(|token| {
            let mut icon = Icon::new(Rect::new(0, 0, 24, 24));
            icon.set_icon_enum(token);
            let svg = render_to_svg(&mut icon);
            // The backdrop `<rect>` is not the icon, so it does not count as ink.
            let ink = svg.matches("<path").count()
                + svg.matches("<line").count()
                + svg.matches("<circle").count()
                + svg.matches("<rect").count();
            let contours = svg.matches("<path").count();
            Row {
                token: token.as_str(),
                contours,
                ink: ink.saturating_sub(1),
                digest: sha256_hex(svg.as_bytes()),
                svg,
            }
        })
        .collect()
}

/// C1 — a tokened icon that draws no outline is an icon the user cannot see.
#[test]
fn c1_every_icon_draws_an_outline() {
    for row in census() {
        assert!(
            row.contours > 0,
            "{} drew no outline path (the data did not reach the draw):\n{}",
            row.token,
            row.svg
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

/// The census body, exactly as `examples/icon_census.rs` prints it, minus the header comment.
fn render_census_text() -> String {
    let mut out = String::new();
    for row in census() {
        out.push_str(&format!(
            "{}\tcontours={}\tink={}\tsha256={}\n",
            row.token, row.contours, row.ink, row.digest
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
