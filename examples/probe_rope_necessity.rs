// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Follow-up probe: after the *algorithmic* fixes (no per-key whole-document
//! join, no per-key whole-document find scan), is a rope still needed?
//!
//! Two variants, both keeping the existing `Vec<String>` storage:
//!
//!   * `fixed`   — edit one line in place; skip the find scan when idle.
//!   * `ropeish` — an *optimistic lower bound* for any rope: the cost of the
//!                 same in-place edit plus one O(log n) tree descent. The
//!                 descent is approximated by a binary search over the line
//!                 index, which is the cheapest a real tree could possibly be.
//!
//! If `ropeish` is within noise of `fixed`, no rope can pay for itself here.

use std::time::Instant;

fn build(lines: usize) -> Vec<String> {
    (0..lines).map(|i| format!("    let value_{} = compute({}) + 1;", i, i)).collect()
}

fn main() {
    println!("lines\tfixed_inplace_us\trope_lower_bound_us\tfull_join_us");

    for &lines in &[1_000usize, 10_000, 50_000, 100_000, 500_000] {
        let mut doc = build(lines);

        // `fixed`: one character inside the last line, no document rebuild.
        const N: usize = 200_000;
        let start = Instant::now();
        for i in 0..N {
            let line = &mut doc[lines - 1];
            let at = 4 + (i % 8);
            line.insert(at, 'x');
            line.remove(at);
        }
        let fixed = start.elapsed().as_secs_f64() * 1e6 / N as f64;

        // `rope_lower_bound`: the same edit, plus the cheapest conceivable
        // O(log n) index descent a tree would charge before finding the leaf.
        let start = Instant::now();
        for i in 0..N {
            let mut lo = 0usize;
            let mut hi = doc.len();
            let target = doc.len() - 1;
            while lo < hi {
                let mid = lo + (hi - lo) / 2;
                if mid < target {
                    lo = mid + 1;
                } else {
                    hi = mid;
                }
            }
            let line = &mut doc[lo];
            let at = 4 + (i % 8);
            line.insert(at, 'x');
            line.remove(at);
        }
        let rope = start.elapsed().as_secs_f64() * 1e6 / N as f64;

        // What only a snapshot needs — the one remaining O(document) pass.
        const K: usize = 500;
        let start = Instant::now();
        for _ in 0..K {
            std::hint::black_box(doc.join("\n"));
        }
        let join = start.elapsed().as_secs_f64() * 1e6 / K as f64;

        println!("{}\t{:.3}\t{:.3}\t{:.1}", lines, fixed, rope, join);
    }
}
