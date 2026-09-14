// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! ShortcutEditor widget — a keyboard shortcut editor similar to VS Code's
//! keyboard shortcuts UI.
//!
//! Displays a categorized list of shortcuts with command names and key bindings.
//! Supports filtering by text, editing keybindings by clicking them, and
//! organizing shortcuts by category.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::undo::{CommandDescription, CommandId, UndoCommand, UndoStack};
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_SHORTCUT_EDITOR_COMMAND_ID: AtomicU64 = AtomicU64::new(1);

struct ShortcutEditorCommand {
    id: CommandId,
    target: Rc<RefCell<Vec<ShortcutEntry>>>,
    before: Vec<ShortcutEntry>,
    after: Vec<ShortcutEntry>,
}

impl ShortcutEditorCommand {
    fn new(
        target: Rc<RefCell<Vec<ShortcutEntry>>>,
        before: Vec<ShortcutEntry>,
        after: Vec<ShortcutEntry>,
    ) -> Self {
        Self {
            id: CommandId(NEXT_SHORTCUT_EDITOR_COMMAND_ID.fetch_add(1, Ordering::Relaxed)),
            target,
            before,
            after,
        }
    }
}

impl UndoCommand for ShortcutEditorCommand {
    fn id(&self) -> CommandId {
        self.id
    }

    fn description(&self) -> CommandDescription {
        CommandDescription {
            text: "Edit shortcut list".to_string(),
            timestamp_ms: 0,
            command_type: "shortcut_editor",
        }
    }

    fn execute(&mut self) -> Result<(), String> {
        *self.target.borrow_mut() = self.after.clone();
        Ok(())
    }

    fn undo(&mut self) -> Result<(), String> {
        *self.target.borrow_mut() = self.before.clone();
        Ok(())
    }
}

fn floor_char_boundary(s: &str, index: usize) -> usize {
    let len = s.len();
    if index >= len {
        return len;
    }
    let bytes = s.as_bytes();
    let mut i = index;
    while i > 0 && bytes[i] & 0xC0 == 0x80 {
        i -= 1;
    }
    i
}

/// A single shortcut entry in the editor.
///
/// # Not the same as [`crate::shortcut::ShortcutEntry`]
///
/// Both are called `ShortcutEntry` but model different things (principle #49):
///
/// * this one — a **row in the editor UI**, carrying editable fields (default
///   keys, category, whether the user may rebind it);
/// * the registered binding — which action a shortcut fires, and whether it is
///   currently active.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutEntry {
    /// Unique identifier for this shortcut.
    pub id: String,
    /// Human-readable command name.
    pub name: String,
    /// Current key combinations (e.g. ["Ctrl+S", "Ctrl+Shift+S"]).
    pub keys: Vec<String>,
    /// Default key combinations.
    pub default_keys: Vec<String>,
    /// Category grouping.
    pub category: String,
    /// Whether this shortcut can be edited.
    pub editable: bool,
}

impl ShortcutEntry {
    /// Creates a new shortcut entry.
    pub fn new(id: &str, name: &str, category: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            keys: Vec::new(),
            default_keys: Vec::new(),
            category: category.to_string(),
            editable: true,
        }
    }

    /// Sets the current key bindings.
    pub fn with_keys(mut self, keys: Vec<&str>) -> Self {
        self.keys = keys.into_iter().map(|k| k.to_string()).collect();
        self
    }

    /// Sets the default key bindings.
    pub fn with_default_keys(mut self, keys: Vec<&str>) -> Self {
        self.default_keys = keys.into_iter().map(|k| k.to_string()).collect();
        self
    }

    /// Sets whether this entry is editable.
    pub fn with_editable(mut self, editable: bool) -> Self {
        self.editable = editable;
        self
    }
}

/// A keyboard shortcut editor widget that displays and manages shortcut bindings.
pub struct ShortcutEditor {
    base: BaseWidget,
    shortcuts: Vec<ShortcutEntry>,
    filter_text: String,
    /// Emitted when a shortcut's key binding changes. Emits (id, new_keys).
    pub shortcut_changed: Signal1<(String, Vec<String>)>,
    undo_stack: UndoStack,
    history_target: Rc<RefCell<Vec<ShortcutEntry>>>,
}

impl ShortcutEditor {
    /// Creates a new ShortcutEditor widget with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ShortcutEditor, geometry, "ShortcutEditor"),
            shortcuts: Vec::new(),
            filter_text: String::new(),
            shortcut_changed: Signal1::new(),
            undo_stack: UndoStack::new(),
            history_target: Rc::new(RefCell::new(Vec::new())),
        }
    }

    /// Adds a shortcut entry.
    pub fn add_shortcut(&mut self, entry: ShortcutEntry) {
        let before = self.shortcuts.clone();
        self.shortcuts.push(entry);
        self.record_shortcut_state(before);
        self.base.request_redraw();
    }

    /// Removes a shortcut by its id. Returns `true` if found and removed.
    pub fn remove_shortcut(&mut self, id: &str) -> bool {
        let initial_len = self.shortcuts.len();
        let before = self.shortcuts.clone();
        self.shortcuts.retain(|s| s.id != id);
        if self.shortcuts.len() != initial_len {
            self.record_shortcut_state(before);
            self.base.request_redraw();
            return true;
        }
        false
    }

    /// Removes all shortcuts.
    pub fn clear_shortcuts(&mut self) {
        let before = self.shortcuts.clone();
        self.shortcuts.clear();
        self.record_shortcut_state(before);
        self.base.request_redraw();
    }

    /// Updates the key bindings for a shortcut by id. Returns `true` if found.
    pub fn update_shortcut_keys(&mut self, id: &str, keys: Vec<String>) -> bool {
        let before = self.shortcuts.clone();
        if let Some(entry) = self.shortcuts.iter_mut().find(|s| s.id == id) {
            if entry.keys == keys {
                return false;
            }
            entry.keys = keys.clone();
            self.record_shortcut_state(before);
            self.shortcut_changed.emit((id.to_string(), keys));
            self.base.request_redraw();
            return true;
        }
        false
    }

    /// Sets the filter text for searching shortcuts.
    pub fn set_filter(&mut self, text: &str) {
        let text = text.to_string();
        if self.filter_text != text {
            self.filter_text = text;
            self.base.request_redraw();
        }
    }

    /// Returns the current filter text.
    pub fn filter_text(&self) -> &str {
        &self.filter_text
    }

    /// Returns a reference to all shortcuts.
    pub fn shortcuts(&self) -> &[ShortcutEntry] {
        &self.shortcuts
    }

    /// Returns shortcuts matching the current filter.
    pub fn filtered_shortcuts(&self) -> Vec<&ShortcutEntry> {
        if self.filter_text.is_empty() {
            return self.shortcuts.iter().collect();
        }
        let lower = self.filter_text.to_lowercase();
        self.shortcuts
            .iter()
            .filter(|s| {
                s.name.to_lowercase().contains(&lower)
                    || s.id.to_lowercase().contains(&lower)
                    || s.category.to_lowercase().contains(&lower)
                    || s.keys.iter().any(|k| k.to_lowercase().contains(&lower))
            })
            .collect()
    }

    /// Groups filtered shortcuts by category.
    pub fn shortcuts_by_category(&self) -> Vec<(String, Vec<&ShortcutEntry>)> {
        let filtered = self.filtered_shortcuts();
        let mut categories: Vec<(String, Vec<&ShortcutEntry>)> = Vec::new();

        for entry in &filtered {
            let cat = &entry.category;
            if let Some((_, group)) = categories.iter_mut().find(|(c, _)| c == cat) {
                group.push(entry);
            } else {
                categories.push((cat.clone(), vec![*entry]));
            }
        }

        categories
    }

    /// Returns a mutable reference to shortcuts.
    pub fn shortcuts_mut(&mut self) -> &mut Vec<ShortcutEntry> {
        &mut self.shortcuts
    }

    pub fn undo(&mut self) -> bool {
        if self.undo_stack.undo().is_err() {
            return false;
        }
        self.restore_shortcut_state();
        true
    }

    pub fn redo(&mut self) -> bool {
        if self.undo_stack.redo().is_err() {
            return false;
        }
        self.restore_shortcut_state();
        true
    }

    pub fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }

    fn record_shortcut_state(&mut self, before: Vec<ShortcutEntry>) {
        if before == self.shortcuts {
            return;
        }
        let after = self.shortcuts.clone();
        *self.history_target.borrow_mut() = after.clone();
        self.undo_stack.push(Box::new(ShortcutEditorCommand::new(
            self.history_target.clone(),
            before,
            after,
        )));
    }

    fn restore_shortcut_state(&mut self) {
        self.shortcuts = self.history_target.borrow().clone();
        self.base.request_redraw();
    }

    fn update_first_visible_editable_shortcut(&mut self, key_name: String) -> bool {
        let lower = self.filter_text.to_lowercase();
        let target_id = self
            .shortcuts
            .iter()
            .find(|entry| {
                entry.editable
                    && (lower.is_empty()
                        || entry.name.to_lowercase().contains(&lower)
                        || entry.id.to_lowercase().contains(&lower)
                        || entry.category.to_lowercase().contains(&lower)
                        || entry.keys.iter().any(|k| k.to_lowercase().contains(&lower)))
            })
            .map(|entry| entry.id.clone());

        if let Some(id) = target_id {
            return self.update_shortcut_keys(&id, vec![key_name]);
        }
        false
    }
}

impl Widget for ShortcutEditor {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(300, 200)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `ShortcutEditor`'s property contract.
///
/// The writer takes `&str` (`set_filter`) rather than an owned `String`, so the
/// coerced value is borrowed before the call — the same shape the old arm used.
impl WidgetProperties for ShortcutEditor {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "filter_text" => Ok(CapabilityValue::String(self.filter_text().to_string())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "filter_text" => {
                let text = expect_string(value)?;
                self.set_filter(&text);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `SHORTCUT_EDITOR_PROPERTIES`.
        property_names_of!["filter_text", BASE_PROPERTY_NAMES]
    }
}

impl Draw for ShortcutEditor {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        // Draw background
        context.fill_rect(rect, Color::WHITE);

        let margin = 8;
        let mut y = rect.y + margin;

        // Draw filter bar
        let filter_font = Font::new("sans-serif", 13.0, false, false);
        let filter_label = if self.filter_text.is_empty() {
            "Type to filter shortcuts...".to_string()
        } else {
            format!("Filter: {}", self.filter_text)
        };
        context.draw_text(
            Point::new(rect.x + margin, y),
            &filter_label,
            &filter_font,
            if self.filter_text.is_empty() {
                Color::rgba(150, 150, 150, 255)
            } else {
                Color::rgba(50, 50, 50, 255)
            },
            HorizontalAlignment::Left,
        );
        y += 24;

        // Separator
        context.draw_line(
            Point::new(rect.x + margin, y),
            Point::new(rect.x + rect.width as i32 - margin, y),
            Color::rgba(200, 200, 200, 255),
        );
        y += 8;

        // Draw categorized shortcuts
        let categories = self.shortcuts_by_category();
        let cat_font = Font::new("sans-serif", 12.0, true, false);
        let name_font = Font::new("sans-serif", 12.0, false, false);
        let key_font = Font::new("sans-serif", 11.0, false, false);
        let row_height = 22;

        for (category, entries) in &categories {
            if y + row_height > rect.y + rect.height as i32 {
                break;
            }

            // Category header
            context.draw_text(
                Point::new(rect.x + margin + 4, y),
                category,
                &cat_font,
                Color::rgba(80, 80, 80, 255),
                HorizontalAlignment::Left,
            );
            y += row_height;

            for entry in entries {
                if y + row_height > rect.y + rect.height as i32 {
                    break;
                }

                // Command name
                let name_text = if entry.name.len() > 30 {
                    // Use floor_char_boundary to avoid mid-char panic
                    let boundary = floor_char_boundary(&entry.name, 27);
                    format!("{}...", &entry.name[..boundary])
                } else {
                    entry.name.clone()
                };
                context.draw_text(
                    Point::new(rect.x + margin + 16, y),
                    &name_text,
                    &name_font,
                    Color::rgba(30, 30, 30, 255),
                    HorizontalAlignment::Left,
                );

                // Key binding (right-aligned)
                let key_text = if entry.keys.is_empty() {
                    "unbound".to_string()
                } else {
                    entry.keys.join(", ")
                };
                let key_color = if entry.keys.is_empty() {
                    Color::rgba(180, 180, 180, 255)
                } else {
                    Color::rgba(60, 60, 60, 255)
                };
                // Measure approximate key text width
                let key_x =
                    rect.x + rect.width as i32 - margin - (key_text.len() as i32 * 8).min(150);
                context.draw_text(
                    Point::new(key_x, y),
                    &key_text,
                    &key_font,
                    key_color,
                    HorizontalAlignment::Left,
                );

                y += row_height;
            }
        }
    }
}

impl EventHandler for ShortcutEditor {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::KeyPress { key, modifiers } => {
                if *key == 90 && *modifiers == 2 {
                    let _ = self.undo();
                    return;
                }
                if *key == 89 && *modifiers == 2 {
                    let _ = self.redo();
                    return;
                }
                // Record key events for shortcut editing
                // Ignore modifier-only presses
                if *key > 0 && *key < 0xFF {
                    let key_name = keycode_to_name(*key, *modifiers);
                    if !key_name.is_empty() {
                        self.update_first_visible_editable_shortcut(key_name);
                        self.base.key_down.emit((*key, *modifiers));
                        return;
                    }
                }
                self.base.handle_event(event);
            }
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

/// Converts a keycode and modifier mask to a human-readable key name.
fn keycode_to_name(key: u32, modifiers: u32) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if modifiers & 0x01 != 0 {
        parts.push("Shift");
    }
    if modifiers & 0x02 != 0 {
        parts.push("Ctrl");
    }
    if modifiers & 0x04 != 0 {
        parts.push("Alt");
    }
    if modifiers & 0x08 != 0 {
        parts.push("Meta");
    }

    let modifier_prefix =
        if parts.is_empty() { String::new() } else { format!("{}+", parts.join("+")) };

    let key_name = match key {
        0x08 => "Backspace",
        0x09 => "Tab",
        0x0D => "Enter",
        0x1B => "Escape",
        0x20 => "Space",
        0x21 => "PageUp",
        0x22 => "PageDown",
        0x23 => "End",
        0x24 => "Home",
        0x25 => "Left",
        0x26 => "Up",
        0x27 => "Right",
        0x28 => "Down",
        0x2D => "Insert",
        0x2E => "Delete",
        0x70..=0x7A => {
            let n = key - 0x70 + 1;
            return format!("{}F{}", modifier_prefix.clone(), n);
        }
        _ => {
            if (0x30..=0x39).contains(&key) {
                let c = char::from_u32(key).unwrap_or('?');
                return format!("{}{}", modifier_prefix.clone(), c);
            }
            if (0x41..=0x5A).contains(&key) {
                let c = char::from_u32(key).unwrap_or('?');
                return format!("{}{}", modifier_prefix.clone(), c);
            }
            return String::new();
        }
    };
    parts.push(key_name);
    parts.join("+")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;

    #[test]
    fn shortcut_editor_initial_state() {
        let se = ShortcutEditor::new(Rect::new(0, 0, 400, 300));
        assert!(se.shortcuts().is_empty());
        assert!(se.filter_text().is_empty());
        assert_eq!(se.kind(), WidgetKind::ShortcutEditor);
    }

    #[test]
    fn shortcut_editor_add_and_remove() {
        let mut se = ShortcutEditor::new(Rect::new(0, 0, 400, 300));
        let entry = ShortcutEntry::new("save", "Save File", "File").with_keys(vec!["Ctrl+S"]);
        se.add_shortcut(entry);

        assert_eq!(se.shortcuts().len(), 1);
        assert_eq!(se.shortcuts()[0].name, "Save File");

        assert!(se.remove_shortcut("save"));
        assert!(se.shortcuts().is_empty());

        assert!(!se.remove_shortcut("nonexistent"));
    }

    #[test]
    fn shortcut_editor_clear() {
        let mut se = ShortcutEditor::new(Rect::new(0, 0, 400, 300));
        se.add_shortcut(ShortcutEntry::new("a", "A", "General"));
        se.add_shortcut(ShortcutEntry::new("b", "B", "General"));
        se.clear_shortcuts();
        assert!(se.shortcuts().is_empty());
    }

    #[test]
    fn shortcut_editor_update_keys() {
        let mut se = ShortcutEditor::new(Rect::new(0, 0, 400, 300));
        let entry = ShortcutEntry::new("save", "Save", "File").with_keys(vec!["Ctrl+S"]);
        se.add_shortcut(entry);

        let changed = std::sync::Arc::new(std::sync::Mutex::new(None));
        let changed_clone = changed.clone();
        se.shortcut_changed.connect(move |val| {
            *changed_clone.lock().unwrap() = Some((*val).clone());
        });

        assert!(se.update_shortcut_keys("save", vec!["Ctrl+Shift+S".to_string()]));
        assert_eq!(se.shortcuts()[0].keys, vec!["Ctrl+Shift+S"]);

        let result = changed.lock().unwrap().take();
        assert!(result.is_some());
        let (id, keys) = result.unwrap();
        assert_eq!(id, "save");
        assert_eq!(keys, vec!["Ctrl+Shift+S"]);

        assert!(!se.update_shortcut_keys("nonexistent", vec![]));
    }

    #[test]
    fn shortcut_editor_undo_redo_restores_shortcut_keys() {
        let mut se = ShortcutEditor::new(Rect::new(0, 0, 400, 300));
        se.add_shortcut(ShortcutEntry::new("save", "Save", "File").with_keys(vec!["Ctrl+S"]));
        se.update_shortcut_keys("save", vec!["Ctrl+Shift+S".to_string()]);

        assert!(se.can_undo());
        assert!(se.undo());
        assert_eq!(se.shortcuts()[0].keys, vec!["Ctrl+S"]);
        assert!(se.can_redo());
        assert!(se.redo());
        assert_eq!(se.shortcuts()[0].keys, vec!["Ctrl+Shift+S"]);
    }

    #[test]
    fn shortcut_editor_keypress_updates_first_editable_visible_entry() {
        let mut se = ShortcutEditor::new(Rect::new(0, 0, 400, 300));
        se.add_shortcut(ShortcutEntry::new("save", "Save", "File"));

        se.handle_event(&Event::KeyPress { key: 83, modifiers: 2 });

        assert_eq!(se.shortcuts()[0].keys, vec!["Ctrl+S"]);
    }

    #[test]
    fn shortcut_editor_filter_limits_keypress_target() {
        let mut se = ShortcutEditor::new(Rect::new(0, 0, 400, 300));
        se.add_shortcut(ShortcutEntry::new("save", "Save", "File"));
        se.add_shortcut(ShortcutEntry::new("copy", "Copy", "Edit"));
        se.set_filter("copy");

        se.handle_event(&Event::KeyPress { key: 67, modifiers: 2 });

        assert!(se.shortcuts()[0].keys.is_empty());
        assert_eq!(se.shortcuts()[1].keys, vec!["Ctrl+C"]);
    }

    #[test]
    fn shortcut_editor_filter() {
        let mut se = ShortcutEditor::new(Rect::new(0, 0, 400, 300));
        se.add_shortcut(ShortcutEntry::new("save", "Save File", "File").with_keys(vec!["Ctrl+S"]));
        se.add_shortcut(ShortcutEntry::new("open", "Open File", "File").with_keys(vec!["Ctrl+O"]));
        se.add_shortcut(ShortcutEntry::new("copy", "Copy", "Edit").with_keys(vec!["Ctrl+C"]));

        // No filter — all results
        assert_eq!(se.filtered_shortcuts().len(), 3);

        // Filter by name
        se.set_filter("Save");
        assert_eq!(se.filtered_shortcuts().len(), 1);
        assert_eq!(se.filtered_shortcuts()[0].id, "save");

        // Filter by key
        se.set_filter("Ctrl+C");
        assert_eq!(se.filtered_shortcuts().len(), 1);
        assert_eq!(se.filtered_shortcuts()[0].id, "copy");

        // Clear filter
        se.set_filter("");
        assert_eq!(se.filtered_shortcuts().len(), 3);
    }

    #[test]
    fn shortcut_editor_categories() {
        let mut se = ShortcutEditor::new(Rect::new(0, 0, 400, 300));
        se.add_shortcut(ShortcutEntry::new("save", "Save", "File"));
        se.add_shortcut(ShortcutEntry::new("open", "Open", "File"));
        se.add_shortcut(ShortcutEntry::new("copy", "Copy", "Edit"));
        se.add_shortcut(ShortcutEntry::new("paste", "Paste", "Edit"));
        se.add_shortcut(ShortcutEntry::new("about", "About", "Help"));

        let categories = se.shortcuts_by_category();
        assert_eq!(categories.len(), 3);

        let file_cat = categories.iter().find(|(c, _)| c == "File").unwrap();
        assert_eq!(file_cat.1.len(), 2);

        let edit_cat = categories.iter().find(|(c, _)| c == "Edit").unwrap();
        assert_eq!(edit_cat.1.len(), 2);
    }

    #[test]
    fn shortcut_entry_builder() {
        let entry = ShortcutEntry::new("test", "Test Command", "Testing")
            .with_keys(vec!["Ctrl+T", "Ctrl+Shift+T"])
            .with_default_keys(vec!["Ctrl+T"])
            .with_editable(false);

        assert_eq!(entry.id, "test");
        assert_eq!(entry.keys, vec!["Ctrl+T", "Ctrl+Shift+T"]);
        assert_eq!(entry.default_keys, vec!["Ctrl+T"]);
        assert!(!entry.editable);
    }

    #[test]
    fn shortcut_editor_add_duplicate_does_not_dedup() {
        let mut se = ShortcutEditor::new(Rect::new(0, 0, 400, 300));
        se.add_shortcut(ShortcutEntry::new("a", "A", "General"));
        se.add_shortcut(ShortcutEntry::new("a", "A (dup)", "General"));
        assert_eq!(se.shortcuts().len(), 2);
    }

    #[test]
    fn test_keycode_to_name() {
        assert_eq!(keycode_to_name(0x41, 0x02), "Ctrl+A");
        assert_eq!(keycode_to_name(0x53, 0x06), "Ctrl+Alt+S");
        assert_eq!(keycode_to_name(0x1B, 0), "Escape");
        assert_eq!(keycode_to_name(0x0D, 0), "Enter");
        assert_eq!(keycode_to_name(0x20, 0x01), "Shift+Space");
        assert_eq!(keycode_to_name(0x70, 0), "F1");
        assert_eq!(keycode_to_name(0x75, 0x02), "Ctrl+F6");
    }
}
