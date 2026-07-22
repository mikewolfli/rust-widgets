//! CommandPalette productivity widget.

use crate::core::{HorizontalAlignment, Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Single command entry displayed in command palette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandEntry {
    /// Stable command identifier.
    pub id: String,
    /// Human-friendly command title.
    pub title: String,
    /// Optional command category used for display context.
    pub category: String,
    /// Optional keywords used by query matching.
    pub keywords: Vec<String>,
}

impl CommandEntry {
    /// Creates a command entry with id and title.
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self { id: id.into(), title: title.into(), category: String::new(), keywords: Vec::new() }
    }

    /// Sets command category.
    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.category = category.into();
        self
    }

    /// Sets command keywords.
    pub fn with_keywords(mut self, keywords: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.keywords = keywords.into_iter().map(Into::into).collect();
        self
    }
}

/// Palette-style command launcher with query filtering and keyboard navigation.
pub struct CommandPalette {
    base: BaseWidget,
    entries: Vec<CommandEntry>,
    query: String,
    filtered_indices: Vec<usize>,
    highlighted_index: Option<usize>,
    row_height: u32,
    /// Emitted when command is activated. Payload is command id.
    pub command_activated: Signal1<String>,
    /// Emitted when query changes. Payload is current query.
    pub query_changed: Signal1<String>,
}

impl CommandPalette {
    /// Creates an empty command palette.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ListView, geometry, "CommandPalette"),
            entries: Vec::new(),
            query: String::new(),
            filtered_indices: Vec::new(),
            highlighted_index: None,
            row_height: 24,
            command_activated: Signal1::new(),
            query_changed: Signal1::new(),
        }
    }

    /// Replaces command entries.
    pub fn set_entries(&mut self, entries: Vec<CommandEntry>) {
        self.entries = entries;
        self.refresh_filter();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns all command entries.
    pub fn entries(&self) -> &[CommandEntry] {
        &self.entries
    }

    /// Sets query string and refreshes filtered command list.
    pub fn set_query(&mut self, query: impl Into<String>) {
        let next = query.into();
        if self.query == next {
            return;
        }
        self.query = next;
        self.refresh_filter();
        self.query_changed.emit(self.query.clone());
        self.base.request_redraw();
    }

    /// Clears current query.
    pub fn clear_query(&mut self) {
        self.set_query(String::new());
    }

    /// Returns current query string.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Returns filtered command count.
    pub fn filtered_count(&self) -> usize {
        self.filtered_indices.len()
    }

    /// Returns highlighted command position in filtered list.
    pub fn highlighted_index(&self) -> Option<usize> {
        self.highlighted_index.filter(|idx| *idx < self.filtered_indices.len())
    }

    /// Sets row height.
    pub fn set_row_height(&mut self, row_height: u32) {
        self.row_height = row_height.max(1);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns row height.
    pub fn row_height(&self) -> u32 {
        self.row_height
    }

    /// Returns command entry at filtered index.
    pub fn filtered_entry(&self, filtered_index: usize) -> Option<&CommandEntry> {
        let entry_index = *self.filtered_indices.get(filtered_index)?;
        self.entries.get(entry_index)
    }

    /// Moves highlighted command by signed delta.
    pub fn move_highlight(&mut self, delta: isize) {
        if self.filtered_indices.is_empty() {
            self.highlighted_index = None;
            return;
        }

        let current = self.highlighted_index.unwrap_or(0) as isize;
        let max = self.filtered_indices.len().saturating_sub(1) as isize;
        let next = (current + delta).clamp(0, max) as usize;
        self.highlighted_index = Some(next);
        self.base.request_redraw();
    }

    /// Activates highlighted command and emits command id.
    pub fn activate_highlighted(&mut self) -> bool {
        let Some(filtered_index) = self.highlighted_index() else {
            return false;
        };
        let Some(entry) = self.filtered_entry(filtered_index) else {
            return false;
        };
        self.command_activated.emit(entry.id.clone());
        true
    }

    fn refresh_filter(&mut self) {
        let query = self.query.to_lowercase();
        let mut scored = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                Self::score_entry(entry, &query).map(|score| (index, score))
            })
            .collect::<Vec<_>>();

        scored.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        self.filtered_indices = scored.into_iter().map(|(index, _)| index).collect();

        if self.filtered_indices.is_empty() {
            self.highlighted_index = None;
        } else {
            self.highlighted_index = Some(0);
        }
    }

    fn score_entry(entry: &CommandEntry, query: &str) -> Option<i32> {
        if query.is_empty() {
            return Some(1);
        }

        let title = entry.title.to_lowercase();
        let id = entry.id.to_lowercase();
        let category = entry.category.to_lowercase();
        let keywords =
            entry.keywords.iter().map(|keyword| keyword.to_lowercase()).collect::<Vec<_>>();

        if title.starts_with(query) {
            return Some(120);
        }
        if title.contains(query) {
            return Some(100);
        }
        if id.contains(query) {
            return Some(80);
        }
        if category.contains(query) {
            return Some(60);
        }
        if keywords.iter().any(|keyword| keyword.contains(query)) {
            return Some(50);
        }

        None
    }

    fn append_query_char(&mut self, key: u32) {
        if let Some(ch) = char::from_u32(key) {
            if ch.is_ascii_graphic() || ch == ' ' {
                self.query.push(ch);
                self.refresh_filter();
                self.query_changed.emit(self.query.clone());
                self.base.request_redraw();
            }
        }
    }

    fn backspace_query(&mut self) {
        if self.query.pop().is_some() {
            self.refresh_filter();
            self.query_changed.emit(self.query.clone());
            self.base.request_redraw();
        }
    }

    fn pick_at_position(&mut self, pos: Point) {
        let rect = self.geometry();
        if pos.y < rect.y || pos.y >= rect.y + rect.height as i32 {
            return;
        }
        let row = ((pos.y - rect.y) / self.row_height as i32).max(0) as usize;
        if row < self.filtered_indices.len() {
            self.highlighted_index = Some(row);
            self.base.request_redraw();
        }
    }
}

impl Widget for CommandPalette {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(400, 300)
    }
}

impl EventHandler for CommandPalette {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button: 1 } => {
                self.pick_at_position(*pos);
            }
            Event::MouseDoubleClick { pos, button: 1 } => {
                self.pick_at_position(*pos);
                let _ = self.activate_highlighted();
            }
            Event::KeyPress { key, modifiers: _ } => match *key {
                38 => self.move_highlight(-1),
                40 => self.move_highlight(1),
                13 => {
                    let _ = self.activate_highlighted();
                }
                27 => self.clear_query(),
                8 => self.backspace_query(),
                key => self.append_query_char(key),
            },
            _ => { /* Other events are not relevant */ }
        }
    }
}

impl Draw for CommandPalette {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        context.fill_rect(rect, Color::rgb(250, 250, 252));
        context.draw_rect(rect, Color::rgb(200, 205, 214));

        let header = Rect::new(rect.x, rect.y, rect.width, self.row_height);
        context.fill_rect(header, Color::rgb(239, 243, 249));
        context.draw_text(
            Point::new(rect.x + 8, rect.y + self.row_height as i32 / 2),
            &format!("> {}", self.query),
            &Font::default(),
            Color::rgb(24, 36, 52),
            HorizontalAlignment::Left,
        );

        let mut y = rect.y + self.row_height as i32;
        let limit = rect.y + rect.height as i32;
        for filtered_index in 0..self.filtered_indices.len() {
            if y + self.row_height as i32 > limit {
                break;
            }

            if self.highlighted_index == Some(filtered_index) {
                context.fill_rect(
                    Rect::new(rect.x + 1, y, rect.width.saturating_sub(2), self.row_height),
                    Color::rgb(214, 228, 248),
                );
            }

            if let Some(entry) = self.filtered_entry(filtered_index) {
                let line = if entry.category.is_empty() {
                    entry.title.clone()
                } else {
                    format!("{} · {}", entry.category, entry.title)
                };
                context.draw_text(
                    Point::new(rect.x + 8, y + self.row_height as i32 / 2),
                    &line,
                    &Font::default(),
                    Color::rgb(35, 45, 60),
                    HorizontalAlignment::Left,
                );
            }

            y += self.row_height as i32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn sample_entries() -> Vec<CommandEntry> {
        vec![
            CommandEntry::new("file.open", "Open File")
                .with_category("File")
                .with_keywords(["open", "load"]),
            CommandEntry::new("file.save", "Save File")
                .with_category("File")
                .with_keywords(["save", "persist"]),
            CommandEntry::new("view.toggle_sidebar", "Toggle Sidebar")
                .with_category("View")
                .with_keywords(["sidebar", "panel"]),
        ]
    }

    #[test]
    fn query_filters_and_ranks_entries() {
        let mut palette = CommandPalette::new(Rect::new(0, 0, 320, 160));
        palette.set_entries(sample_entries());

        assert_eq!(palette.filtered_count(), 3);
        palette.set_query("save");
        assert_eq!(palette.filtered_count(), 1);
        assert_eq!(palette.filtered_entry(0).map(|entry| entry.id.as_str()), Some("file.save"));

        palette.set_query("file");
        assert_eq!(palette.filtered_count(), 2);
        assert_eq!(palette.filtered_entry(0).map(|entry| entry.id.as_str()), Some("file.open"));
    }

    #[test]
    fn keyboard_navigation_and_activation_work() {
        let mut palette = CommandPalette::new(Rect::new(0, 0, 320, 160));
        palette.set_entries(sample_entries());

        let activated = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = activated.clone();
        palette.command_activated.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });

        palette.handle_event(&Event::key_press(40, 0));
        assert_eq!(palette.highlighted_index(), Some(1));
        palette.handle_event(&Event::key_press(13, 0));

        let got = activated.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec!["file.save".to_string()]);
    }

    #[test]
    fn typing_and_backspace_drive_query() {
        let mut palette = CommandPalette::new(Rect::new(0, 0, 320, 160));
        palette.set_entries(sample_entries());

        palette.handle_event(&Event::key_press('s' as u32, 0));
        palette.handle_event(&Event::key_press('a' as u32, 0));
        assert_eq!(palette.query(), "sa");
        assert_eq!(palette.filtered_count(), 1);

        palette.handle_event(&Event::key_press(8, 0));
        assert_eq!(palette.query(), "s");
        assert_eq!(palette.filtered_count(), 2);

        palette.handle_event(&Event::key_press(27, 0));
        assert_eq!(palette.query(), "");
        assert_eq!(palette.filtered_count(), 3);
    }

    #[test]
    fn default_state() {
        let palette = CommandPalette::new(Rect::new(0, 0, 800, 600));
        assert_eq!(palette.query(), "");
        assert_eq!(palette.filtered_count(), 0);
        assert_eq!(palette.highlighted_index(), None);
        assert!(palette.entries().is_empty());
    }

    #[test]
    fn empty_entries_handling() {
        let mut palette = CommandPalette::new(Rect::new(0, 0, 800, 600));
        palette.set_entries(Vec::new());
        assert_eq!(palette.filtered_count(), 0);
        assert_eq!(palette.highlighted_index(), None);
        // Activation should not panic
        assert!(!palette.activate_highlighted());

        // Navigation should not panic
        palette.move_highlight(1);
        assert_eq!(palette.highlighted_index(), None);
    }

    #[test]
    fn set_query_programmatically() {
        let mut palette = CommandPalette::new(Rect::new(0, 0, 800, 600));
        palette.set_entries(vec![
            CommandEntry::new("a", "Alpha"),
            CommandEntry::new("b", "Beta"),
            CommandEntry::new("g", "Gamma"),
        ]);

        palette.set_query("beta");
        assert_eq!(palette.query(), "beta");
        assert_eq!(palette.filtered_count(), 1);
        assert_eq!(palette.filtered_entry(0).map(|e| e.id.as_str()), Some("b"));

        // Clear query shows all
        palette.clear_query();
        assert_eq!(palette.query(), "");
        assert_eq!(palette.filtered_count(), 3);
    }

    #[test]
    fn case_insensitive_matching() {
        let mut palette = CommandPalette::new(Rect::new(0, 0, 800, 600));
        palette.set_entries(vec![
            CommandEntry::new("openFile", "Open File"),
            CommandEntry::new("openFolder", "Open Folder"),
        ]);

        palette.set_query("open");
        assert_eq!(palette.filtered_count(), 2);
        assert_eq!(palette.filtered_entry(0).map(|e| e.id.as_str()), Some("openFile"));

        palette.set_query("OPEN");
        assert_eq!(palette.filtered_count(), 2);

        // No match
        palette.set_query("CLOSE");
        assert_eq!(palette.filtered_count(), 0);
    }

    #[test]
    fn category_and_keyword_based_matching() {
        let mut palette = CommandPalette::new(Rect::new(0, 0, 800, 600));
        palette.set_entries(vec![
            CommandEntry::new("file.open", "Open")
                .with_category("File")
                .with_keywords(["load", "browse"]),
            CommandEntry::new("edit.find", "Find")
                .with_category("Edit")
                .with_keywords(["search", "locate"]),
        ]);

        // Match category
        palette.set_query("file");
        assert_eq!(palette.filtered_count(), 1);
        assert_eq!(palette.filtered_entry(0).map(|e| e.id.as_str()), Some("file.open"));

        // Match keyword
        palette.set_query("load");
        assert_eq!(palette.filtered_count(), 1);
        assert_eq!(palette.filtered_entry(0).map(|e| e.id.as_str()), Some("file.open"));

        // Match keyword on second entry
        palette.set_query("search");
        assert_eq!(palette.filtered_count(), 1);
        assert_eq!(palette.filtered_entry(0).map(|e| e.id.as_str()), Some("edit.find"));

        // No match
        palette.set_query("nonexistent");
        assert_eq!(palette.filtered_count(), 0);
    }
}
