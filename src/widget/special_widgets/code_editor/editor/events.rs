// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Event routing: keys, IME, pointer and wheel.
//!
//! Split out of `editor.rs` (principle #9: one file, one responsibility).
//! The parent module owns the [`CodeEditor`](super::CodeEditor) type and its
//! fields; these methods are inherent, so moving them does not change the API.

use super::*;

// ─────────────────────────────────────────────────────────────────────────────
// Event handling
// ─────────────────────────────────────────────────────────────────────────────

impl EventHandler for CodeEditor {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::KeyPress { key, modifiers } => self.dispatch_key(*key, *modifiers),
            Event::KeyRelease { .. } => {
                // Text-producing virtual keyboards deliver payload via
                // `TextInput`/`ImeCommit`; a key release never mutates the buffer.
            }
            Event::TextInput { text } => self.input_text(text),
            Event::ImeCommit { text } => {
                // The commit **replaces** the composition: the preedit overlay goes away
                // in the same event that inserts the bytes, or the string would be drawn
                // twice for one frame and the caret would sit past it.
                self.preedit = None;
                self.input_text(text);
            }
            Event::ImePreedit { text, cursor } => {
                // Composition is host-owned; the editor's half is to show it at the caret
                // and keep the caret in view. An empty composition is the IME cancelling,
                // which is a clear rather than an empty overlay.
                if text.is_empty() {
                    self.preedit = None;
                } else {
                    self.preedit =
                        Some(Preedit { text: text.clone(), cursor: (*cursor).min(text.len()) });
                    self.ensure_caret_visible();
                }
                self.base.request_redraw();
            }
            Event::MousePress { pos, button, modifiers } if *button == 1 => {
                self.pointer_press_with_modifiers(*pos, *modifiers);
            }
            Event::MouseRelease { .. } => self.pointer_release(),
            Event::MouseMove { pos } => self.pointer_drag(*pos),
            Event::MouseDoubleClick { pos, .. } => {
                self.select_word_at_point(*pos);
            }
            Event::MouseEnter { pos } => {
                // Enter carries the position, so a tab can light up without waiting for
                // the first motion inside the widget.
                self.update_hovered_tab(*pos);
                self.base.request_redraw();
            }
            Event::MouseLeave { .. } => {
                if self.hovered_tab.take().is_some() {
                    self.base.request_redraw();
                }
            }
            Event::Wheel { delta, modifiers } => self.handle_wheel(*delta, *modifiers),
            Event::TouchBegin { pos, .. } => self.pointer_press(*pos),
            Event::TouchMove { pos, .. } => self.pointer_drag(*pos),
            Event::TouchEnd { .. } => self.pointer_release(),
            Event::LongPress { pos } => {
                self.select_word_at_point(*pos);
            }
            Event::Resize { .. } => {
                self.refresh_visible_rows();
                self.base.request_layout();
            }
            _ => {}
        }
    }
}

impl CodeEditor {
    fn handle_wheel(&mut self, delta: Point, modifiers: u32) {
        if modifiers & Self::MODIFIER_CONTROL != 0 {
            // Ctrl+wheel is the universal editor font-size gesture.
            let step = if delta.y < 0 { 1.0 } else { -1.0 };
            let size = (self.config.font_size + step).clamp(6.0, 48.0);
            if size != self.config.font_size {
                self.config.font_size = size;
                // Clearing the explicit advance restores the derived
                // `font_size * line_height_ratio`, so zooming keeps the leading
                // ratio the configuration asked for instead of its own constant.
                self.config.line_advance = 0.0;
                // The cell width was measured in the *old* font size, and the
                // measurement cache is keyed by geometry alone, so it has to be
                // invalidated explicitly or zooming would change the glyph size
                // while every column kept its old pitch.
                self.measured_geometry = None;
                self.refresh_visible_rows();
                self.base.request_layout();
                self.base.request_redraw();
            }
            return;
        }
        if delta.y != 0 {
            let lines = (delta.y as f32 / self.line_height()).round();
            if lines.abs() >= 1.0 {
                self.scroll_by(lines as isize);
            }
        } else if delta.x != 0 {
            let columns = (delta.x as f32 / self.cell_width()).round();
            if columns.abs() >= 1.0 {
                self.scroll_columns_by(columns as isize);
            }
        }
    }
}
