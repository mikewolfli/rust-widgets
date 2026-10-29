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
        // Another view may have changed the shared buffer list; re-home before the event is
        // interpreted against this view's active index.
        let _ = self.sync_with_document();
        // An attached line-number gutter receives line clicks through this poll before the
        // event's own meaning is applied, so a click on the gutter is a navigation request
        // rather than an edit (COMP-A8).
        let _ = self.poll_line_number_gutter();
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
            Event::MousePress { pos, button, modifiers } => match *button {
                crate::event::mouse_button::PRIMARY => {
                    self.pointer_press_with_modifiers(*pos, *modifiers);
                }
                // A secondary click opens the context menu. The menu has always
                // existed, but no event path could open it: only button 1 was
                // handled, so the whole menu was unreachable from the pointer.
                crate::event::mouse_button::SECONDARY => {
                    self.open_context_menu(*pos);
                }
                _ => {}
            },
            Event::MouseRelease { .. } => self.pointer_release(),
            Event::MouseMove { pos } => self.pointer_drag(*pos),
            Event::MouseDoubleClick { pos, .. } => {
                // A double-click in the gutter folds the block, which is the
                // gesture every editor attaches to the fold chevron's column.
                if let Some(line) = self.hit_gutter_line(*pos) {
                    self.toggle_fold(line);
                } else {
                    self.select_word_at_point(*pos);
                    self.double_click_at = Some(*pos);
                }
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
                if self.hovered_diagnostic.take().is_some() {
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
            Event::FocusGained { .. } => {
                self.set_focused(true);
            }
            Event::FocusLost => {
                self.set_focused(false);
            }
            _ => {}
        }
    }
}

impl CodeEditor {
    fn handle_wheel(&mut self, delta: Point, modifiers: u32) {
        // A popup owns the wheel while it is open: scrolling the document behind
        // a candidate list the pointer is over is never what the reader meant.
        if self.completion.visible && delta.y != 0 {
            let rows = (delta.y.abs() as f32 / self.line_height()).ceil().max(1.0) as isize;
            let direction = if delta.y < 0 { rows } else { -rows };
            self.scroll_completion(direction);
            return;
        }
        if modifiers & Self::MODIFIER_CONTROL != 0 {
            // Ctrl+wheel is the universal editor font-size gesture. It goes through
            // the same path as `zoom_in`/`zoom_out`, so the signal, the cache
            // invalidation and the clamp cannot drift between the two entry points.
            let step = if delta.y < 0 { 1.0 } else { -1.0 };
            self.apply_zoom(self.config.font_size + step);
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
