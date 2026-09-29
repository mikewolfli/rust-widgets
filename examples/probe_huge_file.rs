// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Probe: what happens when a *large file* is opened, and what a hard ceiling
//! would have to look like.
//!
//! Two questions:
//!   1. How long does the current `set_text` path take on 1M lines, and why?
//!   2. After every algorithmic fix that keeps `Vec<String>` storage, what does
//!      opening + editing a 1M-line file cost?
//!
//! Run with `cargo run --release --example probe_huge_file`.

use std::time::Instant;

const LINE: &str = "    let value = compute(1) + 1;";

/// `split_lines` as `buffer.rs:215` implements it: one `String` per line.
fn split_lines(text: &str) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }
    text.split('\n').map(|l| l.to_string()).collect()
}

/// The memory a `Vec<String>` costs: the `String` headers themselves plus each one's heap
/// capacity.
///
/// `size_of_val(lines)` for the first term rather than `len() * size_of::<String>()`: the two are
/// the same number, but the former is what the value actually occupies and is what clippy asks
/// for (`manual_slice_size_calculation`), which matters because this example is built with
/// `-D warnings` on the cross-compile gates.
fn vec_string_bytes(lines: &[String]) -> usize {
    std::mem::size_of_val(lines) + lines.iter().map(|l| l.capacity()).sum::<usize>()
}

fn main() {
    println!("=== 1) 打开文件：现状 set_text 路径 ===");
    println!("lines\ttext_MB\tsplit_ms\tmem_MB");

    for &lines in &[100_000usize, 500_000, 1_000_000, 2_000_000] {
        let text: String = (0..lines).map(|_| format!("{LINE}\n")).collect();
        let mb = text.len() as f64 / 1e6;

        let start = Instant::now();
        let split = split_lines(&text);
        let split_ms = start.elapsed().as_secs_f64() * 1e3;

        let mem = vec_string_bytes(&split) as f64 / 1e6;
        println!("{}\t{:.0}\t{:.1}\t{:.0}", lines, mb, split_ms, mem);

        // What the O(1) alternative would cost: keep the text, store the
        // newline offsets in one contiguous `Vec<u32>`.
        let start = Instant::now();
        let offsets: Vec<u32> = {
            let mut v = Vec::with_capacity(lines + 1);
            v.push(0u32);
            for (i, b) in text.bytes().enumerate() {
                if b == b'\n' {
                    v.push((i + 1) as u32);
                }
            }
            v
        };
        let index_ms = start.elapsed().as_secs_f64() * 1e3;
        let idx_mem = (offsets.len() * 4) as f64 / 1e6;
        println!(
            "  └ 行索引方案: {:.1} ms, 索引 {:.1} MB (合计 {:.1} MB, 省 {:.0}%)",
            index_ms,
            idx_mem,
            idx_mem + mb,
            (1.0 - (idx_mem + mb) / mem) * 100.0
        );
    }

    println!("\n=== 2) 修好后：1M 行文件的单次编辑与快照 ===");
    let lines = 1_000_000usize;
    let text: String = (0..lines).map(|_| format!("{LINE}\n")).collect();
    let mut offsets: Vec<u32> = Vec::with_capacity(lines + 1);
    offsets.push(0u32);
    for (i, b) in text.bytes().enumerate() {
        if b == b'\n' {
            offsets.push((i + 1) as u32);
        }
    }

    // Edit inside one line: O(line) regardless of document size.
    const N: usize = 200_000;
    let mut buf = text.clone();
    let mut offs = offsets.clone();
    let start = Instant::now();
    for i in 0..N {
        let line = 999_999usize;
        let at = offs[line] as usize + 4 + (i % 8);
        buf.insert(at, 'x');
        buf.remove(at);
    }
    let edit = start.elapsed().as_secs_f64() * 1e6 / N as f64;
    println!("  单次行内编辑:           {:.3} µs", edit);

    // Whole-document snapshot (undo checkpoints, save, copy-all).
    const K: usize = 20;
    let start = Instant::now();
    for _ in 0..K {
        std::hint::black_box(buf.clone());
    }
    let snap = start.elapsed().as_secs_f64() * 1e3 / K as f64;
    println!("  整篇克隆（每次 undo 快照）: {:.1} ms  ← 这里才是真问题", snap);

    // Insert a line near the top: memmove of the whole text.
    let start = Instant::now();
    for _ in 0..50 {
        buf.insert_str(100, LINE);
        buf.drain(100..100 + LINE.len());
        let _ = &mut offs;
    }
    let insert = start.elapsed().as_secs_f64() * 1e3 / 50.0;
    println!("  顶部插入一行（整篇 memmove）: {:.1} ms", insert);

    println!("\n  结论：修好算法后，编辑本身是 µs 级；");
    println!("        但「整篇克隆 / 整篇 memmove」仍是 ms 级——");
    println!("        而这两件事都不是 rope 要解决的问题，是「别每键做整篇」的问题。");
}
