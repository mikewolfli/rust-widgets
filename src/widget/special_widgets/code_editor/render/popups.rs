// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Completion list and context menu.
//!
//! Split out of `render.rs` (principle #9). The entry point stays in the
//! parent; these are the paint helpers it calls.

use super::*;

impl CodeEditor {
    pub(super) fn draw_completion(&self, context: &mut RenderContext, chrome: &EditorChrome) {
        if !self.completion.visible {
            return;
        }
        let rect = self.completion_rect();
        context.face_with_gradient(
            rect,
            chrome.popup_background,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );
        context.draw_rect(rect, chrome.popup_border);
        let font = self.chrome_font();
        let line_box = context.measure_text("M", font).height as i32;
        let label_dy = (MIN_TOUCH_TARGET - line_box).max(0) / 2;
        for (index, item) in self.completion.items.iter().enumerate() {
            let row_y = rect.y + index as i32 * MIN_TOUCH_TARGET;
            if index == self.completion.selected {
                context.fill_rect(
                    Rect::new(rect.x + 1, row_y, rect.width - 2, MIN_TOUCH_TARGET as u32),
                    chrome.popup_selected,
                );
            }
            let sigil = item.kind.map(|kind| format!("{} ", kind)).unwrap_or_default();
            let label = match &item.detail {
                Some(detail) => format!("{}{}  {}", sigil, item.label, detail),
                None => format!("{}{}", sigil, item.label),
            };
            // Vertically centred by the measured line box, not by a literal `+19`:
            // a row is `MIN_TOUCH_TARGET` tall and the face's box is a different height,
            // so the constant put the label near the row's bottom edge.
            context.draw_text(
                Point::new(rect.x + 8, row_y + label_dy),
                &label,
                font,
                chrome.ink,
                HorizontalAlignment::Left,
            );
        }
    }

    pub(super) fn draw_context_menu(&self, context: &mut RenderContext, chrome: &EditorChrome) {
        if !self.context_menu.visible {
            return;
        }
        let Some(rect) = self.context_menu_rect() else { return };
        context.face_with_gradient(
            rect,
            chrome.popup_background,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );
        context.draw_rect(rect, chrome.popup_border);
        let font = self.chrome_font();
        let line_box = context.measure_text("M", font).height as i32;
        let label_dy = (MIN_TOUCH_TARGET - line_box).max(0) / 2;
        for (index, item) in self.context_menu.items.iter().enumerate() {
            let row_y = rect.y + index as i32 * MIN_TOUCH_TARGET;
            if index == self.context_menu.selected && !item.disabled {
                context.fill_rect(
                    Rect::new(rect.x + 1, row_y, rect.width - 2, MIN_TOUCH_TARGET as u32),
                    chrome.popup_selected,
                );
            }
            // A disabled row is the dim ink; an enabled one is the resolved body ink.
            let color = if item.disabled { chrome.dim_ink } else { chrome.ink };
            context.draw_text(
                Point::new(rect.x + 8, row_y + label_dy),
                &item.label,
                font,
                color,
                HorizontalAlignment::Left,
            );
            if let Some(shortcut) = &item.shortcut {
                context.draw_text(
                    Point::new(rect.x + rect.width as i32 - 8, row_y + label_dy),
                    shortcut,
                    font,
                    chrome.dim_ink,
                    HorizontalAlignment::Right,
                );
            }
        }
    }
}
