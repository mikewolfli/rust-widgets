// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Probe: the minimap is the real "open a 1M-line file and die" bug.
//!
//! `render.rs:1015` iterates **every line of the document** on **every frame**
//! and issues one `fill_rect` per non-blank line. This probe measures:
//!
//!   1. how long that loop takes at 1M lines,
//!   2. what a bucketed minimap costs instead (fixed cost per pixel row).
//!
//! Run with `cargo run --release --example probe_minimap`.

use std::time::Instant;

const LINE: &str = "    let value = compute(1) + 1;";

fn main() {
    // A minimap strip is ~120 px tall on a 1080p-ish window.
    const MINIMAP_PX: usize = 120;

    println!("lines\tminimap_loop_ms\tbucketed_ms\tratio");

    for &lines in &[10_000usize, 100_000, 1_000_000, 5_000_000] {
        let doc: Vec<String> = (0..lines).map(|_| LINE.to_string()).collect();

        // 1) Current: one pass over every line, per frame.
        const N: usize = 20;
        let start = Instant::now();
        for _ in 0..N {
            let mut drawn = 0usize;
            let scale = MINIMAP_PX as f32 / lines as f32;
            let dot = scale.max(1.0);
            for (index, text) in doc.iter().enumerate() {
                let _y = (index as f32 * scale).round() as i32;
                let indent = text.chars().take_while(|c| c.is_whitespace()).count();
                let length = text.trim_end().chars().count().saturating_sub(indent);
                if length == 0 {
                    continue;
                }
                let _w = ((length as f32 / 120.0) * 112.0).clamp(1.0, 112.0);
                drawn += 1;
                let _ = dot;
            }
            std::hint::black_box(drawn);
        }
        let current = start.elapsed().as_secs_f64() * 1e3 / N as f64;

        // 2) Bucketed: one slot per minimap pixel row. Cost is bounded by the
        //    strip height, not the document length.
        let start = Instant::now();
        for _ in 0..N {
            let mut buckets = vec![(0u32, 0u32, 0u32); MINIMAP_PX]; // (indent, len, count)
            let per_bucket = (lines as f64 / MINIMAP_PX as f64).ceil().max(1.0) as usize;
            for (index, text) in doc.iter().enumerate() {
                let bucket = index / per_bucket;
                if bucket >= MINIMAP_PX {
                    break;
                }
                let indent = text.chars().take_while(|c| c.is_whitespace()).count() as u32;
                let length = text.trim_end().chars().count() as u32;
                let slot = &mut buckets[bucket];
                // Running average keeps it O(1) per line and O(px) per frame.
                slot.0 += indent;
                slot.1 += length;
                slot.2 += 1;
            }
            std::hint::black_box(&buckets);
        }
        let bucketed = start.elapsed().as_secs_f64() * 1e3 / N as f64;

        println!("{}\t{:.2}\t{:.2}\t{:.0}x", lines, current, bucketed, current / bucketed);
    }

    println!("\n注意：bucketed 版仍遍历全部行，但只画 {} 个矩形（不是每行一个）。", MINIMAP_PX);
    println!("真正的 O(1) 版本应只统计可见视口 + 用行索引增量维护，见白皮书 §16。");
}
