// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! End-to-end probe: the *whole* per-keystroke cost of one edit, not just `splice`.
//!
//! `probe_splice_cost` measured only `splice`. But one keystroke in the real
//! widget also runs, inside `commit_edit`:
//!
//!   * `self.text()`                     → clone the whole mirrored String
//!   * `commit_edit` second `text()`      → another whole clone
//!   * `refresh_derived_state()`         → `find_all` scans EVERY line
//!
//! So the model's `splice` is only one of several O(document) passes per key.
//! This probe counts them all, so the rope-vs-not decision rests on the real
//! load rather than on one primitive.

use std::time::Instant;

/// Mirrors `EditorModel`'s storage plus the per-keystroke work `commit_edit`
/// performs on it.
struct Widget {
    lines: Vec<String>,
    mirror: String,
    /// `find.query` — the widget re-scans all lines for it on every edit.
    query: String,
}

impl Widget {
    fn from_text(text: &str, query: &str) -> Self {
        Self {
            lines: text.split('\n').map(|l| l.to_string()).collect(),
            mirror: text.to_string(),
            query: query.to_string(),
        }
    }

    fn full_text(&self) -> String {
        self.lines.join("\n")
    }

    /// `EditorModel::splice` as it exists today: rebuild the whole document.
    fn splice(&mut self, line: usize, column: usize, payload: &str) {
        let before = self.full_text();
        let mut offset = 0usize;
        for i in 0..line {
            offset += self.lines.get(i).map(|l| l.len()).unwrap_or(0) + 1;
        }
        let target = self.lines.get(line).map(|s| s.as_str()).unwrap_or("");
        let col_off = target.char_indices().nth(column).map(|(o, _)| o).unwrap_or(target.len());
        let at = offset + col_off;
        let mut result = String::with_capacity(before.len() + payload.len());
        result.push_str(&before[..at.min(before.len())]);
        result.push_str(payload);
        result.push_str(&before[at.min(before.len())..]);
        self.mirror = result.clone();
        self.lines = result.split('\n').map(|l| l.to_string()).collect();
    }

    /// `commit_edit`'s whole-clone read of the mirrored text.
    fn text_clone(&self) -> String {
        self.mirror.clone()
    }

    /// `refresh_derived_state` → `recompute_match_cache` → `find_all`.
    /// Re-scans every line, allocating a `Vec<char>` per line.
    fn find_all_scan(&self, query: &str) -> usize {
        if query.is_empty() {
            return 0;
        }
        let needle: Vec<char> = query.chars().collect();
        let mut hits = 0usize;
        for line in &self.lines {
            let haystack: Vec<char> = line.chars().collect();
            if haystack.len() < needle.len() {
                continue;
            }
            let mut start = 0usize;
            while start + needle.len() <= haystack.len() {
                if &haystack[start..start + needle.len()] == needle.as_slice() {
                    hits += 1;
                }
                start += 1;
            }
        }
        hits
    }

    /// One keystroke exactly as `splice_payload` + `commit_edit` perform it.
    fn keystroke(&mut self) {
        let _before = self.text_clone(); // splice_payload: `let before = self.text()`
        self.splice(self.lines.len() - 1, 10, "x");
        let _after = self.text_clone(); // commit_edit: text() for TextSnapshotCommand
        let _ = self.find_all_scan(&self.query.clone());
    }
}

fn main() {
    println!("lines\tsplice_only_us\ttotal_keystroke_us\tfind_scan_us\toverhead_x");

    for &lines in &[1_000usize, 5_000, 10_000, 20_000, 50_000] {
        let text: String = (0..lines)
            .map(|i| format!("    let value_{} = compute({}) + 1; // token\n", i, i))
            .collect();

        // 1) splice alone
        let mut w = Widget::from_text(&text, "value");
        w.splice(w.lines.len() - 1, 10, "x");
        const N: usize = 100;
        let start = Instant::now();
        for _ in 0..N {
            w.splice(w.lines.len() - 1, 10, "x");
        }
        let splice_only = start.elapsed().as_secs_f64() * 1e6 / N as f64;

        // 2) the full keystroke
        let mut w = Widget::from_text(&text, "value");
        w.keystroke();
        let start = Instant::now();
        for _ in 0..N {
            w.keystroke();
        }
        let total = start.elapsed().as_secs_f64() * 1e6 / N as f64;

        // 3) find scan alone
        let w = Widget::from_text(&text, "value");
        let start = Instant::now();
        for _ in 0..N {
            std::hint::black_box(w.find_all_scan("value"));
        }
        let find_us = start.elapsed().as_secs_f64() * 1e6 / N as f64;

        println!(
            "{}\t{:.1}\t{:.1}\t{:.1}\t{:.2}x",
            lines,
            splice_only,
            total,
            find_us,
            total / splice_only
        );
    }
}
