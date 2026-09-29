// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! What batch 3 changed, measured through the widget's own edit path.
//!
//! Uses `CodeEditor` directly, so the numbers include the undo checkpoint, the
//! derived-state refresh and the match-cache update — everything a keystroke
//! actually costs.
//!
//! Run with:
//!     cargo run --release --no-default-features --features desktop --example verify_batch3

use std::time::Instant;

use rust_widgets::core::Rect;
use rust_widgets::widget::special_widgets::code_editor::{CodeEditor, LanguageId, TextPosition};

fn document(lines: usize) -> String {
    let mut text = String::with_capacity(lines * 46);
    for i in 0..lines {
        text.push_str("    let value_");
        text.push_str(&i.to_string());
        text.push_str(" = compute(");
        text.push_str(&i.to_string());
        text.push_str(") + 1; // token\n");
    }
    text
}

fn main() {
    println!("=== 批 3：单次按键成本（含 undo + 派生状态刷新）===\n");
    println!("{:>10}  {:>13}  {:>15}  {:>14}", "行数", "打字 µs", "开查找 µs", "Enter µs");

    for &lines in &[1_000usize, 10_000, 100_000] {
        let text = document(lines);

        // Typing: insert one character, then undo. Both halves are measured
        // because both run on the user's critical path.
        let mut editor = CodeEditor::new(Rect::new(0, 0, 400, 300));
        editor.set_language(LanguageId::Rust);
        editor.set_text(text.clone());
        editor.set_caret(TextPosition::new(0, 0), false);

        const N: usize = 200;
        let start = Instant::now();
        for _ in 0..N {
            editor.insert("x");
            editor.undo();
        }
        let typing = start.elapsed().as_secs_f64() * 1e6 / N as f64;

        // With the find bar open, every edit updates the hit list.
        let mut finding = CodeEditor::new(Rect::new(0, 0, 400, 300));
        finding.set_language(LanguageId::Rust);
        finding.set_text(text.clone());
        finding.open_find(false);
        finding.set_search_query("value");
        finding.set_caret(TextPosition::new(0, 0), false);
        let start = Instant::now();
        for _ in 0..N {
            finding.insert("x");
            finding.undo();
        }
        let with_find = start.elapsed().as_secs_f64() * 1e6 / N as f64;

        // Enter rebuilds the line index, which is inherently O(document).
        let mut newlining = CodeEditor::new(Rect::new(0, 0, 400, 300));
        newlining.set_language(LanguageId::Rust);
        newlining.set_text(text);
        newlining.set_caret(TextPosition::new(0, 0), false);
        const M: usize = 20;
        let start = Instant::now();
        for _ in 0..M {
            newlining.insert("\n");
            newlining.undo();
        }
        let newline = start.elapsed().as_secs_f64() * 1e6 / M as f64;

        println!("{lines:>10}  {typing:>13.1}  {with_find:>15.1}  {newline:>14.1}");
    }

    println!("\n对照（第 96 轮实测，修复前）：");
    println!("  10 000 行一次按键 = 1447.6 µs");
    println!("  50 000 行一次按键 = 8140.5 µs");
    println!("\n读数：打字列应与行数无关；Enter 列仍随行数增长（跨行编辑必须重建行索引）。");
}
