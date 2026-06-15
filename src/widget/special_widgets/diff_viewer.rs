//! DiffViewer widget.

use crate::core::{HorizontalAlignment, Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Diff line state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffKind {
    Equal,
    Added,
    Removed,
    Changed,
}

/// One compared line entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    pub left: Option<String>,
    pub right: Option<String>,
    pub kind: DiffKind,
}

/// Side-by-side diff viewer for text snapshots.
pub struct DiffViewer {
    base: BaseWidget,
    left: String,
    right: String,
    lines: Vec<DiffLine>,
    selected_index: Option<usize>,
    /// Emitted after compare. Payload is changed-lines count.
    pub compared: Signal1<usize>,
}

impl DiffViewer {
    /// Creates empty diff viewer.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Table, geometry, "DiffViewer"),
            left: String::new(),
            right: String::new(),
            lines: Vec::new(),
            selected_index: None,
            compared: Signal1::new(),
        }
    }

    /// Sets both text snapshots and recomputes diff.
    pub fn set_texts(&mut self, left: impl Into<String>, right: impl Into<String>) {
        self.left = left.into();
        self.right = right.into();
        self.recompute();
    }

    /// Returns diff lines.
    pub fn lines(&self) -> &[DiffLine] {
        &self.lines
    }

    /// Returns selected line index.
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    /// Selects line index.
    pub fn select_index(&mut self, index: usize) -> bool {
        if index >= self.lines.len() {
            return false;
        }
        self.selected_index = Some(index);
        self.base.request_redraw();
        true
    }

    /// Returns count of non-equal lines.
    pub fn change_count(&self) -> usize {
        self.lines.iter().filter(|line| line.kind != DiffKind::Equal).count()
    }

    fn recompute(&mut self) {
        let left_lines: Vec<&str> = self.left.lines().collect();
        let right_lines: Vec<&str> = self.right.lines().collect();
        let max_len = left_lines.len().max(right_lines.len());

        self.lines.clear();
        for idx in 0..max_len {
            let l = left_lines.get(idx).copied();
            let r = right_lines.get(idx).copied();
            let kind = match (l, r) {
                (Some(a), Some(b)) if a == b => DiffKind::Equal,
                (Some(_), Some(_)) => DiffKind::Changed,
                (Some(_), None) => DiffKind::Removed,
                (None, Some(_)) => DiffKind::Added,
                (None, None) => DiffKind::Equal,
            };
            self.lines.push(DiffLine {
                left: l.map(|s| s.to_string()),
                right: r.map(|s| s.to_string()),
                kind,
            });
        }

        self.selected_index = if self.lines.is_empty() { None } else { Some(0) };
        self.compared.emit(self.change_count());
        self.base.request_layout();
        self.base.request_redraw();
    }
}

impl Widget for DiffViewer {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl EventHandler for DiffViewer {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        if let Event::KeyPress { key, modifiers: _ } = event {
            match *key {
                38 => {
                    if let Some(index) = self.selected_index {
                        if index > 0 {
                            let _ = self.select_index(index - 1);
                        }
                    }
                }
                40 => {
                    if let Some(index) = self.selected_index {
                        if index + 1 < self.lines.len() {
                            let _ = self.select_index(index + 1);
                        }
                    }
                }
                _ => { /* Other keys are not relevant */ }
            }
        }
    }
}

impl Draw for DiffViewer {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        context.fill_rect(rect, Color::rgb(252, 252, 253));
        context.draw_rect(rect, Color::rgb(195, 202, 214));

        let mid_x = rect.x + (rect.width as i32 / 2);
        context.draw_line(
            Point::new(mid_x, rect.y),
            Point::new(mid_x, rect.y + rect.height as i32),
            Color::rgb(216, 222, 232),
        );

        context.draw_text(
            Point::new(rect.x + 8, rect.y + 16),
            "LEFT",
            &Font::default(),
            Color::rgb(54, 66, 85),
            HorizontalAlignment::Left,
        );
        context.draw_text(
            Point::new(mid_x + 8, rect.y + 16),
            "RIGHT",
            &Font::default(),
            Color::rgb(54, 66, 85),
            HorizontalAlignment::Left,
        );

        for (idx, line) in self.lines.iter().take(12).enumerate() {
            let y = rect.y + 34 + (idx as i32) * 16;
            let bg = match line.kind {
                DiffKind::Equal => None,
                DiffKind::Added => Some(Color::rgb(229, 246, 235)),
                DiffKind::Removed => Some(Color::rgb(251, 233, 232)),
                DiffKind::Changed => Some(Color::rgb(253, 244, 225)),
            };
            if let Some(color) = bg {
                context.fill_rect(
                    Rect::new(rect.x + 2, y - 10, rect.width.saturating_sub(4), 14),
                    color,
                );
            }
            if self.selected_index == Some(idx) {
                context.draw_rect(
                    Rect::new(rect.x + 2, y - 11, rect.width.saturating_sub(4), 16),
                    Color::rgb(114, 157, 220),
                );
            }

            if let Some(text) = &line.left {
                context.draw_text(
                    Point::new(rect.x + 8, y),
                    text,
                    &Font::default(),
                    Color::rgb(44, 57, 77),
                    HorizontalAlignment::Left,
                );
            }
            if let Some(text) = &line.right {
                context.draw_text(
                    Point::new(mid_x + 8, y),
                    text,
                    &Font::default(),
                    Color::rgb(44, 57, 77),
                    HorizontalAlignment::Left,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn recompute_detects_changes() {
        let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 300));
        viewer.set_texts("a\nb\nc", "a\nB\nc\nd");

        assert_eq!(viewer.lines().len(), 4);
        assert_eq!(viewer.change_count(), 2);
        assert_eq!(viewer.lines()[1].kind, DiffKind::Changed);
        assert_eq!(viewer.lines()[3].kind, DiffKind::Added);
    }

    #[test]
    fn compared_signal_emits_change_count() {
        let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 300));
        let counts = Arc::new(Mutex::new(Vec::<usize>::new()));
        let sink = counts.clone();
        viewer.compared.connect(move |count| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*count);
            }
        });

        viewer.set_texts("x", "y");

        let got = counts.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec![1]);
    }

    #[test]
    fn arrow_keys_move_selected_line() {
        let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 300));
        viewer.set_texts("a\nb", "a\nb\nc");
        assert_eq!(viewer.selected_index(), Some(0));

        viewer.handle_event(&Event::key_press(40, 0));
        assert_eq!(viewer.selected_index(), Some(1));

        viewer.handle_event(&Event::key_press(40, 0));
        assert_eq!(viewer.selected_index(), Some(2));

        viewer.handle_event(&Event::key_press(38, 0));
        assert_eq!(viewer.selected_index(), Some(1));
    }
}
