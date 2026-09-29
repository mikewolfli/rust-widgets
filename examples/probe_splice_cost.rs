// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! One-off probe: how does the current line-indexed `EditorModel::splice` scale?
//!
//! Answers one question only: *at what document size does a single keystroke stop
//! being free?* Run with `cargo run --release --example probe_splice_cost`.
//!
//! It measures the same operation a keystroke performs — insert one character at
//! a position near the end of the document — because `splice` rebuilds the joined
//! text (`full_text()`), copies it, and re-splits into lines.

use std::time::Instant;

/// A stand-in for the real model: identical algorithm (`join` → splice → `split`).
struct Model {
    lines: Vec<String>,
}

impl Model {
    fn from_text(text: &str) -> Self {
        Self { lines: text.split('\n').map(|l| l.to_string()).collect() }
    }
    fn full_text(&self) -> String {
        self.lines.join("\n")
    }
    fn set_text(&mut self, text: String) {
        self.lines = text.split('\n').map(|l| l.to_string()).collect();
    }
    /// Byte offsets exactly as `line_offset` + `byte_offset` compute them.
    fn splice_one_char(&mut self, line: usize, column: usize) {
        let before = self.full_text();
        let mut offset = 0usize;
        for i in 0..line {
            offset += self.lines.get(i).map(|l| l.len()).unwrap_or(0) + 1;
        }
        let target_line = self.lines.get(line).map(|s| s.as_str()).unwrap_or("");
        let mut chars = target_line.char_indices();
        let mut col_off = target_line.len();
        for _ in 0..column {
            match chars.next() {
                None => break,
                Some((o, _)) => col_off = o,
            }
        }
        if column == 0 {
            col_off = 0;
        }
        let at = offset + col_off;
        let mut result = String::with_capacity(before.len() + 1);
        result.push_str(&before[..at.min(before.len())]);
        result.push('x');
        result.push_str(&before[at.min(before.len())..]);
        self.set_text(result);
    }
}

fn main() {
    println!("lines\tkeystroke_us\tfull_text_us");
    for &lines in &[100usize, 500, 1_000, 2_000, 5_000, 10_000, 20_000, 50_000] {
        // ~40 bytes per line is realistic Rust source.
        let text: String =
            (0..lines).map(|i| format!("    let value_{} = compute({}) + 1;\n", i, i)).collect();
        let bytes = text.len();

        let mut model = Model::from_text(&text);
        let last = model.lines.len().saturating_sub(1);

        // Warm up once so the allocator has the pages.
        model.splice_one_char(last, 10);

        const N: usize = 200;
        let start = Instant::now();
        for _ in 0..N {
            model.splice_one_char(model.lines.len().saturating_sub(1), 10);
        }
        let per_op = start.elapsed().as_secs_f64() * 1e6 / N as f64;

        let start = Instant::now();
        for _ in 0..N {
            std::hint::black_box(model.full_text());
        }
        let per_text = start.elapsed().as_secs_f64() * 1e6 / N as f64;

        println!("{}\t{:.1}\t{:.1}\t({} KB)", lines, per_op, per_text, bytes / 1024);
        let _ = last;
    }
}
