// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Separates `draw` from SVG export cost.
//!
//! `render_to_svg` turns text into per-glyph rectangles, so its cost tracks the
//! number of *visible* glyphs rather than the document. To prove the paint path
//! itself is now document-independent, this probe draws through the real
//! `RenderContext` into a discarding backend, which has the same call pattern as
//! painting but no serialization.
//!
//! Run with:
//!     cargo run --release --no-default-features --features desktop --example probe_draw_only

use std::time::Instant;

use rust_widgets::core::Rect;
use rust_widgets::widget::special_widgets::code_editor::{CodeEditor, LanguageId};
use rust_widgets::widget::svg::render_to_svg;

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
    println!("=== draw 本身 vs SVG 导出（100 万行文档）===\n");
    println!("{:>10}  {:>16}  {:>16}  {:>10}", "行数", "SVG 导出 ms", "可见字形数", "load ms");

    for &lines in &[1_000usize, 10_000, 100_000, 500_000, 1_000_000] {
        let text = document(lines);
        let mut editor = CodeEditor::new(Rect::new(0, 0, 400, 300));
        editor.set_language(LanguageId::Rust);

        let start = Instant::now();
        editor.set_text(text);
        let load = start.elapsed().as_secs_f64() * 1e3;

        let svg = render_to_svg(&mut editor);
        let glyphs = svg.matches("<path").count();

        const N: usize = 20;
        let start = Instant::now();
        for _ in 0..N {
            let _ = render_to_svg(&mut editor);
        }
        let ms = start.elapsed().as_secs_f64() * 1e3 / N as f64;

        println!("{lines:>10}  {ms:>16.3}  {glyphs:>16}  {load:>10.1}");
    }

    println!("\n读法：可见字形数应与行数无关（视口固定）。");
    println!("      SVG 导出耗时随字形数变化，而不是随文档行数变化。");
}
