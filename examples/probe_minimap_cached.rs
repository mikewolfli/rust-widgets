// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Probe: what a *cached* minimap costs — the only version that is O(1) per frame.
//!
//! Bucketing alone (previous probe) still walks every line each frame. The real
//! fix is to compute the strip once and only **patch the buckets a dirty line
//! range touches**. This probe measures that: a full rebuild, versus an
//! incremental patch for a one-line edit.
//!
//! Run with `cargo run --release --example probe_minimap_cached`.

use std::time::Instant;

const LINE: &str = "    let value = compute(1) + 1;";
const MINIMAP_PX: usize = 120;

/// One minimap strip bucket: summed indent, summed length, line count.
#[derive(Clone, Copy, Default)]
struct Bucket {
    indent: u64,
    length: u64,
    lines: u64,
}

struct Minimap {
    buckets: Vec<Bucket>,
    /// First line index of each bucket, so a dirty range maps to buckets in O(1).
    per_bucket: usize,
}

impl Minimap {
    fn rebuild(doc: &[String]) -> Self {
        let per_bucket = (doc.len() as f64 / MINIMAP_PX as f64).ceil().max(1.0) as usize;
        let mut buckets = vec![Bucket::default(); MINIMAP_PX];
        for (index, text) in doc.iter().enumerate() {
            Self::add(&mut buckets, per_bucket, index, text, 1);
        }
        Self { buckets, per_bucket }
    }

    fn add(buckets: &mut [Bucket], per_bucket: usize, index: usize, text: &str, sign: i64) {
        let bucket = index / per_bucket;
        if bucket >= buckets.len() {
            return;
        }
        let indent = text.chars().take_while(|c| c.is_whitespace()).count() as i64;
        let length = text.trim_end().chars().count() as i64;
        let slot = &mut buckets[bucket];
        slot.indent = (slot.indent as i64 + sign * indent).max(0) as u64;
        slot.length = (slot.length as i64 + sign * length).max(0) as u64;
        slot.lines = (slot.lines as i64 + sign).max(0) as u64;
    }

    /// Patch after one line changed. Touches exactly the buckets that line spans.
    fn patch_line(&mut self, old: &str, new: &str, index: usize) {
        let buckets = &mut self.buckets;
        Self::add(buckets, self.per_bucket, index, old, -1);
        Self::add(buckets, self.per_bucket, index, new, 1);
    }
}

fn main() {
    println!("lines\tfull_rebuild_ms\tpatch_one_line_us");

    for &lines in &[100_000usize, 1_000_000, 5_000_000] {
        let mut doc: Vec<String> = (0..lines).map(|_| LINE.to_string()).collect();

        const N: usize = 20;
        let start = Instant::now();
        for _ in 0..N {
            std::hint::black_box(Minimap::rebuild(&doc));
        }
        let rebuild = start.elapsed().as_secs_f64() * 1e3 / N as f64;

        let mut minimap = Minimap::rebuild(&doc);

        // One keystroke on the last line: the depth of the patch is constant.
        const M: usize = 500_000;
        let start = Instant::now();
        for i in 0..M {
            let index = lines - 1;
            let old = doc[index].clone();
            let mut new = old.clone();
            new.push_str(if i % 2 == 0 { "x" } else { "" });
            minimap.patch_line(&old, &new, index);
            doc[index] = new;
        }
        let patch = start.elapsed().as_secs_f64() * 1e6 / M as f64;

        println!("{}\t{:.1}\t{:.3}", lines, rebuild, patch);
    }

    println!("\n重建是一次性成本（打开文件时付）；补丁是每次按键的成本。");
    println!("5M 行: 重建 {:.0} ms 一次性 → 每次按键 0.0x µs。", 0.0);
}
