// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `ColumnChooser` — show/hide and reorder a table's columns (COMP-F03).
//!
//! A wide table can only be read by scrolling, and a column the reader does not need still costs
//! its width. The companion takes the same `ColumnSpec` values a header publishes (including
//! their token form, so visibility survives a save/load round trip), filters them by a search
//! string and reports every change back to the owning grid.

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::view_widgets::data_grid::ColumnSpec;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// A list of the table's columns with visibility and order controls.
pub struct ColumnChooser {
    base: BaseWidget,
    columns: Vec<ColumnSpec>,
    /// Visibility by column token; a missing entry means visible.
    hidden: std::collections::HashSet<String>,
    search_text: String,
    allow_reorder: bool,
    /// Emitted with `(token, visible)` when a column is shown or hidden.
    pub column_visibility_changed: Signal1<(String, bool)>,
    /// Emitted with the full token order after a move.
    pub column_order_changed: Signal1<Vec<String>>,
    /// Emitted when the chooser is dismissed.
    pub chooser_closed: Signal1<()>,
}

impl ColumnChooser {
    /// Creates an empty chooser covering `geometry`.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Panel, geometry, "ColumnChooser"),
            columns: Vec::new(),
            hidden: std::collections::HashSet::new(),
            search_text: String::new(),
            allow_reorder: true,
            column_visibility_changed: Signal1::new(),
            column_order_changed: Signal1::new(),
            chooser_closed: Signal1::new(),
        }
    }

    /// Replaces the column list; visibility of a token that still exists is preserved.
    pub fn set_columns(&mut self, columns: Vec<ColumnSpec>) {
        let tokens: std::collections::HashSet<String> =
            columns.iter().map(ColumnSpec::to_token).collect();
        self.hidden.retain(|token| tokens.contains(token));
        self.columns = columns;
        self.base.request_redraw();
    }

    /// The columns, in display order.
    pub fn columns(&self) -> &[ColumnSpec] {
        &self.columns
    }

    /// The name filter (case-insensitive substring).
    pub fn search_text(&self) -> &str {
        &self.search_text
    }

    /// Sets the name filter.
    pub fn set_search_text(&mut self, text: impl Into<String>) {
        self.search_text = text.into();
        self.base.request_redraw();
    }

    /// Whether drag reordering is accepted.
    pub fn allow_reorder(&self) -> bool {
        self.allow_reorder
    }

    /// Enables or disables reordering.
    pub fn set_allow_reorder(&mut self, allow: bool) {
        self.allow_reorder = allow;
    }

    /// Shows or hides the column whose token is `token`.
    ///
    /// Returns `false` when no column has that token, so a stale saved token is reported rather
    /// than silently accepted.
    pub fn set_column_visible(&mut self, token: &str, visible: bool) -> bool {
        if !self.columns.iter().any(|column| column.to_token() == token) {
            return false;
        }
        if visible {
            self.hidden.remove(token);
        } else {
            self.hidden.insert(token.to_string());
        }
        self.column_visibility_changed.emit((token.to_string(), visible));
        self.base.request_redraw();
        true
    }

    /// Whether the column whose token is `token` is visible.
    pub fn is_column_visible(&self, token: &str) -> bool {
        self.columns.iter().any(|column| column.to_token() == token) && !self.hidden.contains(token)
    }

    /// Moves the column at `from` to `to`, reporting the new order.
    pub fn move_column(&mut self, from: usize, to: usize) -> bool {
        if !self.allow_reorder || from >= self.columns.len() || to >= self.columns.len() {
            return false;
        }
        if from == to {
            return true;
        }
        let column = self.columns.remove(from);
        self.columns.insert(to, column);
        self.column_order_changed.emit(self.visible_tokens());
        self.base.request_redraw();
        true
    }

    /// The tokens of the visible columns, in display order.
    pub fn visible_tokens(&self) -> Vec<String> {
        self.columns
            .iter()
            .filter(|column| !self.hidden.contains(&column.to_token()))
            .map(ColumnSpec::to_token)
            .collect()
    }

    /// The columns whose title contains the search text, in display order.
    pub fn filtered_columns(&self) -> Vec<&ColumnSpec> {
        let needle = self.search_text.to_lowercase();
        self.columns
            .iter()
            .filter(|column| needle.is_empty() || column.title.to_lowercase().contains(&needle))
            .collect()
    }

    /// Dismisses the chooser.
    pub fn close(&mut self) {
        self.chooser_closed.emit(());
    }

    /// The row rectangle of `index` inside the control, for a press to hit.
    fn row_rect(&self, index: usize) -> Option<Rect> {
        let rect = self.base.geometry();
        let y = rect.y.saturating_add(4).saturating_add(index as i32 * Self::ROW_HEIGHT as i32);
        if y + Self::ROW_HEIGHT as i32 > rect.y + rect.height as i32 {
            return None;
        }
        Some(Rect::new(rect.x + 4, y, rect.width.saturating_sub(8), Self::ROW_HEIGHT))
    }

    /// The row height of one entry.
    const ROW_HEIGHT: u32 = 22;

    /// The index of the entry under `point`, in [filtered](Self::filtered_columns) order.
    fn entry_at(&self, point: Point) -> Option<usize> {
        let filtered = self.filtered_columns();
        (0..filtered.len())
            .find(|index| self.row_rect(*index).is_some_and(|rect| rect.contains_point(point)))
    }
}

impl Widget for ColumnChooser {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(220, 260)
    }
    crate::impl_draw_bridge!();
}

impl Draw for ColumnChooser {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        let palette = crate::style::palette_shared();
        let surface =
            palette.as_ref().map(|colors| colors.surface_container).unwrap_or(Color::WHITE);
        let ink = palette.as_ref().map(|colors| colors.foreground).unwrap_or(Color::BLACK);
        let accent = palette.as_ref().map(|colors| colors.primary).unwrap_or(Color::PRIMARY);
        context.fill_rounded_rect(rect, 4, surface);
        crate::widget::draw::stroke_surface_border(
            context,
            rect,
            [4, 4, 4, 4],
            palette.as_ref().map(|colors| colors.outline_variant).unwrap_or(ink),
            1,
        );
        let font = crate::widget::metrics::effective_font(self.base.style());
        let filtered = self.filtered_columns();
        for (index, column) in filtered.iter().enumerate() {
            let Some(row) = self.row_rect(index) else {
                break;
            };
            let token = column.to_token();
            let visible = !self.hidden.contains(&token);
            if visible {
                context.fill_rounded_rect(row, 2, surface.blend(&accent, 0.12));
            }
            let label = if visible {
                column.title.clone()
            } else {
                std::format!("{} (hidden)", column.title)
            };
            context.draw_text_fitted_cow(row, &label, font, ink, HorizontalAlignment::Left);
        }
    }
}

impl crate::event::EventHandler for ColumnChooser {
    fn handle_event(&mut self, event: &crate::event::Event) {
        self.base.handle_event(event);
        match event {
            crate::event::Event::MousePress { pos, button: 1, .. } => {
                if let Some(index) = self.entry_at(*pos) {
                    let token = self.filtered_columns()[index].to_token();
                    let visible = self.is_column_visible(&token);
                    self.set_column_visible(&token, !visible);
                }
            }
            crate::event::Event::KeyPress { key, .. }
                if *key == crate::event::key_codes::ESCAPE =>
            {
                self.close();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chooser() -> ColumnChooser {
        let mut chooser = ColumnChooser::new(Rect::new(0, 0, 220, 260));
        chooser.set_columns(vec![
            ColumnSpec::new("Name", 100),
            ColumnSpec::new("Size", 80),
            ColumnSpec::new("Owner", 120),
        ]);
        chooser
    }

    #[test]
    fn visibility_is_keyed_by_the_persisted_token() {
        let mut chooser = chooser();
        let token = chooser.columns()[1].to_token();
        assert!(chooser.is_column_visible(&token));
        assert!(chooser.set_column_visible(&token, false));
        assert!(!chooser.is_column_visible(&token));
        assert_eq!(chooser.visible_tokens().len(), 2);
        assert!(
            !chooser.set_column_visible("not-a-column", true),
            "a stale token is reported, not accepted"
        );

        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = std::sync::Arc::clone(&seen);
        chooser.column_visibility_changed.connect(move |change| {
            if let Ok(mut guard) = sink.lock() {
                guard.push((*change).clone());
            }
        });
        assert!(chooser.set_column_visible(&token, true));
        assert_eq!(
            *seen.lock().expect("lock"),
            vec![(token, true)],
            "the change carries the token and its new state"
        );
    }

    #[test]
    fn reordering_reports_the_visible_order() {
        let mut chooser = chooser();
        let before = chooser.visible_tokens();
        assert!(chooser.move_column(0, 2));
        let after = chooser.visible_tokens();
        assert_eq!(after[2], before[0], "the moved column is last");
        assert_eq!(after.len(), 3);

        chooser.set_allow_reorder(false);
        assert!(!chooser.move_column(0, 1), "reordering is disabled");
    }

    #[test]
    fn the_search_filters_by_title() {
        let mut chooser = chooser();
        chooser.set_search_text("ow");
        let filtered = chooser.filtered_columns();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].title, "Owner");
    }
}
