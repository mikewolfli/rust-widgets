// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! DiffViewer widget.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Diff line state.
///
/// Classifies how the two sides relate for one compared line. Which variant a
/// line receives is decided by the diff algorithm, not by this type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffKind {
    /// The line is present and identical on both sides.
    Equal,
    /// The line exists only on the right-hand side.
    Added,
    /// The line exists only on the left-hand side.
    Removed,
    /// The line exists on both sides but its content differs.
    Changed,
}

/// One compared line entry.
///
/// `left` and `right` are the two sides of the same row; they are `None` when
/// that side has no line at this position, which is what distinguishes
/// [`DiffKind::Added`] (left is `None`) from [`DiffKind::Removed`] (right is
/// `None`). Nothing enforces that invariant, so a hand-built entry can express
/// combinations the diff algorithm never produces, such as `Changed` with one
/// side `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    /// Content shown in the left column, or `None` for a line that only exists
    /// on the right.
    pub left: Option<String>,
    /// Content shown in the right column, or `None` for a line that only exists
    /// on the left.
    pub right: Option<String>,
    /// How the two sides relate; see [`DiffKind`].
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

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(500, 300)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `DiffViewer`'s property contract.
///
/// Both texts are writable, but each name must be written through
/// `set_texts` — which recomputes the diff and emits `compared` — rather than
/// stored directly, so a reader never sees stale `line_count` / `change_count`
/// after a text write.
impl WidgetProperties for DiffViewer {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "left_text" => Ok(CapabilityValue::String(self.left.clone())),
            "right_text" => Ok(CapabilityValue::String(self.right.clone())),
            "line_count" => Ok(CapabilityValue::UInt(self.lines().len() as u64)),
            "change_count" => Ok(CapabilityValue::UInt(self.change_count() as u64)),
            "selected_index" => match self.selected_index() {
                Some(index) => Ok(CapabilityValue::UInt(index as u64)),
                None => Ok(CapabilityValue::Null),
            },
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "left_text" => match value {
                CapabilityValue::String(text) => {
                    let right = self.right.clone();
                    self.set_texts(text, right);
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "right_text" => match value {
                CapabilityValue::String(text) => {
                    let left = self.left.clone();
                    self.set_texts(left, text);
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "line_count" | "change_count" | "selected_index" => {
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "left_text",
            "right_text",
            "line_count",
            "change_count",
            "selected_index",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `diff_viewer` publishes.
    ///
    /// `select_index` names which diff line to select, and the control has no
    /// defensible default — line 0 of a two-sided diff is not "the interesting one",
    /// it is just the first — so a bare invocation is refused as
    /// [`CapabilityAccessError::OutOfRange`]: the name is valid and the index is what
    /// is missing. The property layer treats `selected_index` as read-only on purpose
    /// (see the note on `TimelineWidget` for the same rule), which is why the command
    /// is the only route and why it must not invent an argument. `set_texts` carries
    /// both snapshots.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "select_index" | "set_texts" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
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

        // Chrome colours resolve explicit style first, then the theme's resolved
        // style for this control, and only then a literal. The theme step is what
        // makes the appearance switch visible; previously every colour below was a
        // hardcoded literal, so light and dark rendered identically.
        //
        // `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style().clone();
        let theme = crate::theme::resolved_theme_style("diff_viewer");
        let background = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| background.blend(&Color::BLACK, 0.15));
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        // The split divider and the selection outline are secondary chrome, derived
        // from the resolved colours so they follow the appearance too.
        let divider = background.blend(&text_color, 0.15);
        let selection = crate::theme::semantic_color(crate::theme::SemanticColor::Info)
            .unwrap_or_else(|| background.blend(&text_color, 0.5));
        // Added / Removed / Changed are *states*, so they read the theme's semantic
        // tokens and are tinted over the resolved surface to stay legible in both
        // appearances.
        let added = crate::theme::semantic_color(crate::theme::SemanticColor::Success)
            .map(|token| token.blend(&background, 0.85));
        let removed = crate::theme::semantic_color(crate::theme::SemanticColor::Error)
            .map(|token| token.blend(&background, 0.85));
        let changed = crate::theme::semantic_color(crate::theme::SemanticColor::Warning)
            .map(|token| token.blend(&background, 0.85));

        context.fill_rect(rect, background);
        context.draw_rect(rect, border);

        let mid_x = rect.x + (rect.width as i32 / 2);
        context.draw_line(
            Point::new(mid_x, rect.y),
            Point::new(mid_x, rect.y + rect.height as i32),
            divider,
        );

        context.draw_text(
            Point::new(rect.x + 8, rect.y + 16),
            "LEFT",
            &Font::default(),
            text_color,
            HorizontalAlignment::Left,
        );
        context.draw_text(
            Point::new(mid_x + 8, rect.y + 16),
            "RIGHT",
            &Font::default(),
            text_color,
            HorizontalAlignment::Left,
        );

        for (idx, line) in self.lines.iter().take(12).enumerate() {
            let y = rect.y + 34 + (idx as i32) * 16;
            let bg = match line.kind {
                DiffKind::Equal => None,
                DiffKind::Added => added.or_else(|| Some(background.blend(&Color::GREEN, 0.12))),
                DiffKind::Removed => removed.or_else(|| Some(background.blend(&Color::RED, 0.12))),
                DiffKind::Changed => {
                    changed.or_else(|| Some(background.blend(&Color::YELLOW, 0.12)))
                }
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
                    selection,
                );
            }

            if let Some(text) = &line.left {
                context.draw_text(
                    Point::new(rect.x + 8, y),
                    text,
                    &Font::default(),
                    text_color,
                    HorizontalAlignment::Left,
                );
            }
            if let Some(text) = &line.right {
                context.draw_text(
                    Point::new(mid_x + 8, y),
                    text,
                    &Font::default(),
                    text_color,
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
