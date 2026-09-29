// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! One-off probe: cost of the *structural* alternatives to the current
//! join-splice-split `EditorModel`, at the same document sizes as
//! `probe_splice_cost`.
//!
//! Variant A — `Vec<String>`, one `String` per line: edit one line in place.
//! Variant B — rope-like chunked tree is approximated by measuring the raw
//!             `Vec` insertion cost at the same index, which is the dominant
//!             term a rope would replace.
//!
//! Run with `cargo run --release --example probe_line_vec_cost`.

use std::time::Instant;

const LINE: &str = "    let value = compute(1) + 1;";

fn build(lines: usize) -> Vec<String> {
    (0..lines).map(|_| LINE.to_string()).collect()
}

fn main() {
    println!("lines\tin_place_us\tinsert_line_us\tjoin_us");

    for &lines in &[1_000usize, 10_000, 50_000, 100_000] {
        let mut doc = build(lines);

        // A: edit one character inside an existing line. This is what a real
        // editor does on every keystroke — no rebuild of the document.
        const N: usize = 10_000;
        let start = Instant::now();
        for i in 0..N {
            let line = &mut doc[lines - 1];
            line.insert(4 + (i % 8), 'x');
            line.remove(4 + (i % 8));
        }
        let in_place = start.elapsed().as_secs_f64() * 1e6 / N as f64;

        // B: insert a whole new line in the middle of the vector.
        const M: usize = 2_000;
        let start = Instant::now();
        for _ in 0..M {
            doc.insert(lines / 2, LINE.to_string());
            doc.remove(lines / 2);
        }
        let insert_line = start.elapsed().as_secs_f64() * 1e6 / M as f64;

        // C: the join a full-text snapshot still costs (undo mirror).
        const K: usize = 500;
        let start = Instant::now();
        for _ in 0..K {
            std::hint::black_box(doc.join("\n"));
        }
        let join = start.elapsed().as_secs_f64() * 1e6 / K as f64;

        println!("{}\t{:.2}\t{:.2}\t{:.1}", lines, in_place, insert_line, join);
    }
}
