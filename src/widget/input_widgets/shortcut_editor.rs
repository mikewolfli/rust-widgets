// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! ShortcutEditor widget — a keyboard shortcut editor similar to VS Code's
//! keyboard shortcuts UI.
//!
//! Displays a categorized list of shortcuts with command names and key bindings.
//! Supports filtering by text, editing keybindings by clicking them, and
//! organizing shortcuts by category.

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::shortcut::{Key, Modifiers, PlatformShortcutStyle, Shortcut};
use crate::signal::Signal1;
use crate::undo::{UndoStack, ValueCommand};
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::role_font;
use crate::widget::text_utils::elide_end;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

/// Returns true when a key press is `expected` with exactly the host's primary
/// accelerator.
///
/// This is the match `Shortcut::primary(expected)` describes: Control on
/// Windows and Linux, Command on macOS. `Shift`, `Alt` and every other modifier
/// bit must be absent, so `Shift+Primary+Z` is a different chord and is not
/// plain undo.
///
/// The shortcut layer's matcher lives behind `ShortcutManager`'s private
/// lookup, so the text-editing controls share this one entry point instead of
/// re-deriving the platform rule at each call site. If the shortcut layer ever
/// exposes its matcher, this function becomes a forwarder to it.
pub(crate) fn primary_shortcut_pressed(key_code: u32, event_modifiers: u32, expected: Key) -> bool {
    primary_shortcut_matches(
        key_code,
        event_modifiers,
        expected,
        crate::platform::platform_facts().shortcut_style(),
    )
}

/// Style-parameterised body of [`primary_shortcut_pressed`].
///
/// Tests call this with both conventions so the Windows/Linux and macOS
/// encodings are asserted on every host.
fn primary_shortcut_matches(
    key_code: u32,
    event_modifiers: u32,
    expected: Key,
    style: PlatformShortcutStyle,
) -> bool {
    if Key::from_key_code(key_code) != Some(expected) {
        return false;
    }
    let pressed = Modifiers::from_event_bits_primary_is_ctrl(event_modifiers);
    // Resolve the event into the same vocabulary the shortcut is declared in,
    // then compare exactly. The platforms arrive from opposite directions:
    // macOS already reports Command as PRIMARY (plus the Control bit Command
    // implies), while on Windows and Linux the primary accelerator is the
    // physical Control key and has to be lifted to PRIMARY.
    let resolved = match style {
        PlatformShortcutStyle::Mac => pressed.without(Modifiers::CTRL),
        PlatformShortcutStyle::Desktop => {
            if pressed.contains(Modifiers::CTRL) && !pressed.contains(Modifiers::PRIMARY) {
                pressed.without(Modifiers::CTRL) | Modifiers::PRIMARY
            } else {
                pressed
            }
        }
    };
    resolved == Shortcut::primary(expected).modifiers
}

/// Event modifier bits for the host's primary accelerator.
///
/// Tests synthesise the host's own chord through this rather than hard-coding
/// Control, which is not the primary accelerator on macOS.
#[cfg(test)]
pub(crate) fn primary_modifier_event_bits() -> u32 {
    match crate::platform::platform_facts().shortcut_style() {
        PlatformShortcutStyle::Mac => 0b1000,
        PlatformShortcutStyle::Desktop => 0b0010,
    }
}

/// Event modifier bits for the accelerator that is not primary on this host.
#[cfg(test)]
pub(crate) fn non_primary_modifier_event_bits() -> u32 {
    match crate::platform::platform_facts().shortcut_style() {
        PlatformShortcutStyle::Mac => 0b0010,
        PlatformShortcutStyle::Desktop => 0b1000,
    }
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
    /// Generation of [`Self::shortcuts`], bumped by every path that can change it.
    ///
    /// [`Self::shortcuts_mut`] hands out `&mut Vec<ShortcutEntry>`, so a caller can change the
    /// table with no hook of ours running afterwards. Bumping **when the reference is handed
    /// out** is conservative and sufficient: the grouping cache is already marked stale before
    /// any such mutation can be observed, so the next draw rebuilds instead of drawing a stale
    /// grouping.
    table_generation: u64,
    /// The draw path's grouping, reused while the table and the filter are unchanged.
    grouping_cache: RefCell<GroupingCache>,
    /// The entry currently being recorded, if any.
    capture_id: Option<String>,
    /// Keys captured so far in the current recording.
    capture_keys: Vec<String>,
}

/// Cached category grouping for one table generation and filter string.
///
/// Stores **indices** rather than borrowed rows: the public
/// [`ShortcutEditor::shortcuts_by_category`] returns `&ShortcutEntry` references, which cannot be
/// cached without a self-referential borrow. Indices are plain data, so the cache can live in the
/// widget and the draw reuses it across frames.
#[derive(Default)]
struct GroupingCache {
    /// The table generation these groups were built from.
    generation: u64,
    /// The filter string they were built with.
    filter: String,
    /// Category name, then the indices into `ShortcutEditor::shortcuts` in that category.
    groups: Vec<(String, Vec<usize>)>,
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
            table_generation: 0,
            grouping_cache: RefCell::new(GroupingCache::default()),
            capture_id: None,
            capture_keys: Vec::new(),
        }
    }

    /// Adds a shortcut entry.
    pub fn add_shortcut(&mut self, entry: ShortcutEntry) {
        let before = self.shortcuts.clone();
        self.shortcuts.push(entry);
        self.table_generation = self.table_generation.wrapping_add(1);
        self.record_shortcut_state(before);
        self.base.request_redraw();
    }

    /// Removes a shortcut by its id. Returns `true` if found and removed.
    pub fn remove_shortcut(&mut self, id: &str) -> bool {
        let initial_len = self.shortcuts.len();
        let before = self.shortcuts.clone();
        self.shortcuts.retain(|s| s.id != id);
        if self.shortcuts.len() != initial_len {
            self.table_generation = self.table_generation.wrapping_add(1);
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
        self.table_generation = self.table_generation.wrapping_add(1);
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
            self.table_generation = self.table_generation.wrapping_add(1);
            self.record_shortcut_state(before);
            self.shortcut_changed.emit((id.to_string(), keys));
            self.base.request_redraw();
            return true;
        }
        false
    }

    /// Restores every shortcut that declares [`ShortcutEntry::default_keys`] to those
    /// keys, emitting `shortcut_changed` for each entry that actually moved.
    ///
    /// This is the reader that makes `default_keys` live state rather than a field the
    /// caller stores and nothing reads: a binding's shipped default is declared at
    /// construction and the control can put it back. Entries whose defaults are empty
    /// (the builder was never called) are left alone — "no default declared" is not
    /// "the default is unbound", so it must not silently clear a user's binding.
    /// Like [`Self::update_shortcut_keys`], this is a programmatic assignment, so the
    /// `editable` flag (which governs the UI) does not gate it.
    pub fn reset_all_shortcuts(&mut self) {
        let before = self.shortcuts.clone();
        let mut changed: Vec<(String, Vec<String>)> = Vec::new();
        for entry in &mut self.shortcuts {
            if entry.default_keys.is_empty() || entry.keys == entry.default_keys {
                continue;
            }
            entry.keys = entry.default_keys.clone();
            changed.push((entry.id.clone(), entry.keys.clone()));
        }
        if changed.is_empty() {
            return;
        }
        self.table_generation = self.table_generation.wrapping_add(1);
        self.record_shortcut_state(before);
        for (id, keys) in changed {
            self.shortcut_changed.emit((id, keys));
        }
        self.base.request_redraw();
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
                contains_ignore_ascii_case(&s.name, &lower)
                    || contains_ignore_ascii_case(&s.id, &lower)
                    || contains_ignore_ascii_case(&s.category, &lower)
                    || s.keys.iter().any(|k| contains_ignore_ascii_case(k, &lower))
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
        // See `table_generation`: the caller may change anything through this reference, so the
        // cache is invalidated now rather than left to notice the change some other way.
        self.table_generation = self.table_generation.wrapping_add(1);
        &mut self.shortcuts
    }

    /// The filtered table grouped by category, rebuilt only when the table or filter changed.
    ///
    /// This is the draw path's form of [`Self::shortcuts_by_category`]: it caches indices instead
    /// of borrowed rows and returns them without allocating a new grouping per frame. The borrow
    /// is shared, so the caller may read `self.shortcuts` while holding it.
    fn grouped(&self) -> std::cell::Ref<'_, GroupingCache> {
        let stale = {
            let cache = self.grouping_cache.borrow();
            cache.generation != self.table_generation || cache.filter != self.filter_text
        };
        if stale {
            let mut cache = self.grouping_cache.borrow_mut();
            cache.generation = self.table_generation;
            cache.filter = self.filter_text.clone();
            cache.groups.clear();
            // Filter inline rather than through `filtered_shortcuts`: that method builds a
            // `Vec<&ShortcutEntry>` this path would immediately throw away.
            let lower = self.filter_text.to_lowercase();
            for (index, entry) in self.shortcuts.iter().enumerate() {
                let matches = lower.is_empty()
                    || contains_ignore_ascii_case(&entry.name, &lower)
                    || contains_ignore_ascii_case(&entry.id, &lower)
                    || contains_ignore_ascii_case(&entry.category, &lower)
                    || entry.keys.iter().any(|key| contains_ignore_ascii_case(key, &lower));
                if !matches {
                    continue;
                }
                match cache.groups.iter_mut().find(|(name, _)| *name == entry.category) {
                    Some((_, indices)) => indices.push(index),
                    None => cache.groups.push((entry.category.clone(), vec![index])),
                }
            }
        }
        self.grouping_cache.borrow()
    }

    /// The entry currently being recorded, if any.
    pub fn capture_id(&self) -> Option<&str> {
        self.capture_id.as_deref()
    }

    /// The keys captured so far in the current recording.
    pub fn capture_keys(&self) -> &[String] {
        &self.capture_keys
    }

    /// Whether a recording is in progress.
    pub fn is_capturing(&self) -> bool {
        self.capture_id.is_some()
    }

    /// Begins recording keys for the entry with `id`.
    ///
    /// Returns whether the id named a shortcut. Recording is per row, so a capture can only
    /// affect the entry the user opened.
    pub fn begin_capture(&mut self, id: &str) -> bool {
        if !self.shortcuts.iter().any(|entry| entry.id == id) {
            return false;
        }
        self.capture_id = Some(id.to_string());
        self.capture_keys.clear();
        self.base.request_redraw();
        true
    }

    /// Cancels the recording without changing the binding.
    pub fn cancel_capture(&mut self) {
        if self.capture_id.take().is_none() {
            return;
        }
        self.capture_keys.clear();
        self.base.request_redraw();
    }

    /// Ends the recording and writes the captured sequence to its entry.
    ///
    /// Returns whether a binding was written. An empty sequence leaves the entry alone:
    /// clicking into and out of a row must not silently unbind it.
    pub fn commit_capture(&mut self) -> bool {
        let Some(id) = self.capture_id.take() else {
            return false;
        };
        let keys = std::mem::take(&mut self.capture_keys);
        if keys.is_empty() {
            self.base.request_redraw();
            return false;
        }
        self.update_shortcut_keys(&id, keys)
    }

    /// Clears one entry's binding, leaving the rest of the table alone.
    pub fn clear_shortcut(&mut self, id: &str) -> bool {
        self.update_shortcut_keys(id, Vec::new())
    }

    /// Restores one entry's shipped default binding.
    ///
    /// Returns `false` when the entry has no declared default or is already at it.
    pub fn reset_shortcut(&mut self, id: &str) -> bool {
        let Some(defaults) = self
            .shortcuts
            .iter()
            .find(|entry| entry.id == id)
            .map(|entry| entry.default_keys.clone())
        else {
            return false;
        };
        if defaults.is_empty() {
            return false;
        }
        self.update_shortcut_keys(id, defaults)
    }

    /// The ids of entries whose binding collides with the entry named by `id`.
    ///
    /// A collision is an identical, non-empty key sequence. An unbound entry collides with
    /// nothing, because "no keys" is not a binding.
    pub fn conflicting_shortcuts(&self, id: &str) -> Vec<String> {
        let Some(entry) = self.shortcuts.iter().find(|entry| entry.id == id) else {
            return Vec::new();
        };
        if entry.keys.is_empty() {
            return Vec::new();
        }
        self.shortcuts
            .iter()
            .filter(|other| other.id != id && other.keys == entry.keys)
            .map(|other| other.id.clone())
            .collect()
    }

    /// Whether the entry's binding collides with another entry's.
    pub fn has_conflict(&self, id: &str) -> bool {
        !self.conflicting_shortcuts(id).is_empty()
    }

    /// Formats a key sequence with the modifier symbols a platform shows.
    ///
    /// The stored tokens stay stable, portable names (`Ctrl+T`); this is the display form, so
    /// a binding reads as `⌃T` rather than as a concatenation of English words.
    pub fn format_keys(keys: &[String]) -> String {
        keys.iter()
            .map(|sequence| {
                let mut out = String::new();
                for part in sequence.split('+') {
                    match part {
                        "Shift" => out.push('⇧'),
                        "Ctrl" | "Control" => out.push('⌃'),
                        "Alt" | "Option" => out.push('⌥'),
                        "Meta" | "Cmd" | "Command" => out.push('⌘'),
                        other => out.push_str(other),
                    }
                }
                out
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Reverts the most recent shortcut-table change.
    ///
    /// Returns `false` and changes nothing when there is nothing to undo;
    /// otherwise restores the previous [`ShortcutEntry`] list.
    pub fn undo(&mut self) -> bool {
        if self.undo_stack.undo().is_err() {
            return false;
        }
        self.restore_shortcut_state();
        true
    }

    /// Reapplies the most recently undone shortcut-table change.
    ///
    /// Returns `false` and changes nothing when there is nothing to redo.
    pub fn redo(&mut self) -> bool {
        if self.undo_stack.redo().is_err() {
            return false;
        }
        self.restore_shortcut_state();
        true
    }

    /// Returns whether there is a shortcut-table change to undo.
    pub fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }

    /// Returns whether there is an undone shortcut-table change to reapply.
    pub fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }

    /// Returns how many shortcuts are in the table, before filtering.
    pub fn shortcut_count(&self) -> usize {
        self.shortcuts.len()
    }

    /// Returns how many shortcuts match the current filter.
    ///
    /// Separate from the total because "the table is empty" and "the filter matched nothing" look the
    /// same in a list but need different responses — the first means there is nothing to configure, the
    /// second means the query is wrong.
    pub fn visible_count(&self) -> usize {
        self.filtered_shortcuts().len()
    }

    /// Returns how many distinct categories the matching shortcuts span.
    pub fn category_count(&self) -> usize {
        self.shortcuts_by_category().len()
    }

    /// Returns the distinct category names of the matching shortcuts, in first-seen order.
    pub fn categories(&self) -> Vec<String> {
        self.shortcuts_by_category().into_iter().map(|(name, _)| name).collect()
    }

    /// Returns whether the editor currently offers an empty state to draw.
    ///
    /// True when the table holds nothing at all, which is the condition the control's placeholder row is
    /// for. Derived here so the painter and a test agree about when it applies.
    pub fn is_empty(&self) -> bool {
        self.shortcuts.is_empty()
    }

    fn record_shortcut_state(&mut self, before: Vec<ShortcutEntry>) {
        if before == self.shortcuts {
            return;
        }
        let after = self.shortcuts.clone();
        *self.history_target.borrow_mut() = after.clone();
        self.undo_stack.push(Box::new(ValueCommand::new(
            self.history_target.clone(),
            before,
            after,
            "Edit shortcut list",
            "shortcut_editor",
        )));
    }

    fn restore_shortcut_state(&mut self) {
        self.shortcuts = self.history_target.borrow().clone();
        self.table_generation = self.table_generation.wrapping_add(1);
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
                        || contains_ignore_ascii_case(&entry.name, &lower)
                        || contains_ignore_ascii_case(&entry.id, &lower)
                        || contains_ignore_ascii_case(&entry.category, &lower)
                        || entry.keys.iter().any(|k| contains_ignore_ascii_case(k, &lower)))
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
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(300, 200)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why the field is named per arm
    ///
    /// The published name and the Rust field name are not always the same (`find_next` is backed by
    /// `find_next_signal`, `dismissed` by a `Signal1<()>` field). `connect_event` validates a name
    /// against the capability table and registers a hub slot; only `event_signal_dyn` joins that
    /// name to the signal the control actually **emits**. A wrong arm is worse than no arm, because
    /// it reports a wire as live and never fires it, so each field is named explicitly here rather
    /// than derived from the published name.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "shortcut_changed" => {
                Some(EventSignalRef::mapped("shortcut_changed", &self.shortcut_changed, |v| {
                    // The declared shape is `Mixed`, so the payload arrives as the (id, keys) pair
                    // the signal carries rather than its `Debug` spelling: the id is a string and
                    // the key list is itself a `Tuple` of strings, so a subscriber can read both
                    // parts instead of parsing a debug dump.
                    let (id, keys) = v;
                    CapabilityValue::Tuple(crate::compat::Vec::from([
                        CapabilityValue::String(id.clone()),
                        CapabilityValue::Tuple(
                            keys.iter().map(|key| CapabilityValue::String(key.clone())).collect(),
                        ),
                    ]))
                }))
            }
            _ => None,
        }
    }
}

/// `ShortcutEditor`'s property contract.
///
/// # The defect this replaces
///
/// `filter_text` was the *only* property. That is the one thing the caller already knows — it is what
/// they typed — while everything the control knows and the caller needs was unpublished: how many
/// shortcuts there are, how many the filter left, which categories they span, and whether an undo is
/// available. A consumer could set a filter and then had no way to ask what it did.
///
/// The writer takes `&str` (`set_filter`) rather than an owned `String`, so the
/// coerced value is borrowed before the call — the same shape the old arm used.
impl WidgetProperties for ShortcutEditor {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "filter_text" => Ok(CapabilityValue::String(self.filter_text().to_string())),
            "shortcut_count" => Ok(CapabilityValue::UInt(self.shortcut_count() as u64)),
            "visible_count" => Ok(CapabilityValue::UInt(self.visible_count() as u64)),
            "category_count" => Ok(CapabilityValue::UInt(self.category_count() as u64)),
            // A comma-joined list rather than an array: `CapabilityValue` has no list carrier, and
            // inventing one for a single property would widen the wire format for everything.
            "categories" => Ok(CapabilityValue::String(self.categories().join(", "))),
            "empty" => Ok(CapabilityValue::Bool(self.is_empty())),
            "can_undo" => Ok(CapabilityValue::Bool(self.can_undo())),
            "can_redo" => Ok(CapabilityValue::Bool(self.can_redo())),
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
            // Every other name above is derived from the table, the filter or the history stack, so
            // writing one would be a second way to say something the control already answers.
            "shortcut_count" | "visible_count" | "category_count" | "categories" | "empty"
            | "can_undo" | "can_redo" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `SHORTCUT_EDITOR_PROPERTIES`.
        property_names_of![
            "filter_text",
            "shortcut_count",
            "visible_count",
            "category_count",
            "categories",
            "empty",
            "can_undo",
            "can_redo",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `shortcut_editor` publishes.
    ///
    /// `set_filter_text` assigns the filter string and needs a payload, so it is
    /// answered through the property route — the capability publishes no zero-argument
    /// action for it. `reset_all_shortcuts` needs no payload and restores every entry
    /// that declared default keys, emitting one `shortcut_changed` per entry.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_filter_text" => Err(CapabilityAccessError::OutOfRange),
            "reset_all_shortcuts" => {
                self.reset_all_shortcuts();
                Ok(())
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for ShortcutEditor {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        // Chrome colours resolve the explicit style first, then the theme's resolved style for
        // this control, and only then fall back to a literal. Every colour below used to be a
        // literal, so a light/dark switch left the panel, its filter bar, its separators and its
        // rows unchanged — the rendering census reported the control as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's mutex
        // is not re-entrant.
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("shortcut_editor");
        // Read as its own lock acquisition and copied out as values, so the guard is dropped
        // before anything else touches the theme.
        let (window_fill, foreground, secondary) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => {
                    (active.colors.background, active.colors.foreground, active.colors.secondary)
                }
                None => (Color::rgb(240, 240, 240), Color::BLACK, Color::rgb(158, 158, 158)),
            }
        };

        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(foreground);
        // The panel: one step from the window fill toward the text colour, so it is a distinct
        // element on a light theme and on a dark one. The filter is on the **resolved** value, not
        // only on the theme's: a control classified as `Surface` already carries the window fill,
        // and letting it through unfiltered would make the panel invisible. A caller's own colour
        // still wins.
        let panel_from_theme = window_fill.blend(&ink, 0.06);
        let panel = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => panel_from_theme,
        };
        // The rule under the filter bar, a fixed step out of the panel so it stays visible whatever
        // the panel is.
        let rule = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| panel.blend(&secondary, 0.40));
        // The placeholder grey, the muted key colour and the category header are all de-emphasised
        // steps of the control's own ink. "De-emphasised" is a *relation* to the panel, not a
        // fraction of the way to it: a fixed fraction is a lightness assumption, and `0.45`
        // measured 3.45:1 here, i.e. the hint that tells a user what the field is for could not
        // be read. Each step is dimmed and then held to a floor that its role justifies — the
        // hint and the muted text are read as text, the header is a label.
        let placeholder = ink.blend(&panel, 0.45).legible_on(panel, 4.5);
        let muted_ink = ink.blend(&panel, 0.30).legible_on(panel, 4.5);
        let header_ink = ink.blend(&panel, 0.15).legible_on(panel, 4.5);

        // Draw background
        context.face_with_gradient(
            rect,
            panel,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );

        let margin = 8;
        let mut y = rect.y + margin;

        // A recording session is announced before the rows: the user needs to know the next
        // keys are being captured rather than typed into a filter.
        if self.is_capturing() {
            let capture_font = role_font(style, 13.0);
            let prompt = if self.capture_keys.is_empty() {
                "Press a key sequence (Esc cancels, Backspace clears)".to_string()
            } else {
                format!("Recording: {}", Self::format_keys(&self.capture_keys))
            };
            context.draw_text_fitted_cow(
                Rect::new(rect.x + margin, y, rect.width.saturating_sub(margin as u32 * 2), 18),
                &prompt,
                &capture_font,
                crate::style::semantic_color(crate::style::SemanticColor::Info).unwrap_or(ink),
                HorizontalAlignment::Left,
            );
            y += 20;
        }

        // Draw filter bar
        let filter_font = role_font(style, 13.0);
        // Borrowed when idle — the placeholder is a literal, so owning it cost a string per
        // frame — and built only while a filter is actually typed.
        let filter_label: Cow<'_, str> = if self.filter_text.is_empty() {
            Cow::Borrowed("Type to filter shortcuts...")
        } else {
            Cow::Owned(format!("Filter: {}", self.filter_text))
        };
        context.draw_text_fitted_cow(
            Rect::new(rect.x + margin, y, rect.width.saturating_sub((margin * 2) as u32), 16),
            filter_label.as_ref(),
            &filter_font,
            if self.filter_text.is_empty() { placeholder } else { ink },
            HorizontalAlignment::Left,
        );
        y += 24;

        // Separator
        context.draw_line(
            Point::new(rect.x + margin, y),
            Point::new(rect.x + rect.width as i32 - margin, y),
            rule,
        );
        y += 8;

        // Draw categorized shortcuts
        //
        // The grouping is cached against the table generation and the filter string, so a frame
        // with no edits reuses the previous frame's result instead of rebuilding two Vecs of
        // references. The cache holds indices; the rows themselves live in `self.shortcuts`, which
        // is borrowed here through the same shared borrow.
        let grouped = self.grouped();
        let cat_font = role_font(style, 12.0).with_bold(true);
        let name_font = role_font(style, 12.0);
        let key_font = role_font(style, 11.0);
        let row_height = 22;
        // Reused row buffers: the name was cloned and the key join allocated a `String` for
        // every visible row on every frame.
        let mut key_text = String::new();

        for (category, indices) in &grouped.groups {
            if y + row_height > rect.y + rect.height as i32 {
                break;
            }

            // Category header
            context.draw_text(
                Point::new(rect.x + margin + 4, y),
                category,
                &cat_font,
                header_ink,
                HorizontalAlignment::Left,
            );
            y += row_height;

            for &index in indices {
                let Some(entry) = self.shortcuts.get(index) else {
                    // The cache is keyed by a generation that every writer bumps, so this is
                    // unreachable; skipping is still the honest response to an index that no
                    // longer addresses a row.
                    continue;
                };
                if y + row_height > rect.y + rect.height as i32 {
                    break;
                }

                // Command name, elided by the shared character-budget primitive. The
                // previous byte-boundary cut could split a multi-byte name and reserved
                // three ASCII dot glyphs for the ellipsis.
                let name_text: Cow<'_, str> = elide_end(&entry.name, 30);

                // Key binding (right-aligned)
                key_text.clear();
                if entry.keys.is_empty() {
                    key_text.push_str("unbound");
                } else {
                    // The display form uses the platform's modifier symbols, so a binding
                    // reads as `⌃⇧T` rather than as concatenated English words.
                    key_text.push_str(&Self::format_keys(&entry.keys));
                }
                // A collision is called out on the row it affects, because the conflict is the
                // one thing about a binding that must be noticed before saving.
                let conflict = !entry.keys.is_empty()
                    && self.shortcuts.iter().any(|other| {
                        other.id != entry.id && !other.keys.is_empty() && other.keys == entry.keys
                    });
                // Unbound rows are de-emphasised rather than hidden, so "this shortcut has no key"
                // reads differently from "this shortcut has a key".
                let key_color = if entry.keys.is_empty() {
                    muted_ink
                } else if conflict {
                    crate::style::semantic_color(crate::style::SemanticColor::Error).unwrap_or(ink)
                } else {
                    ink
                };
                // Measured, not estimated: the binding was placed from a UTF-8 byte count at
                // 8 px/byte, capped at 150 px, so a wide font, a text-scale preference or a
                // non-ASCII key name made the two columns overlap.
                let key_width = context.measure_text(&key_text, &key_font).width as i32;
                let name_x = rect.x + margin + 16;
                let key_x = (rect.x + rect.width as i32 - margin - key_width).max(name_x);
                let name_width = (key_x - name_x - 8).max(0) as u32;
                // The fitted draw insets its band by `TEXT_FIT_MARGIN`, so the band is widened and
                // shifted back: a name or key that fits is not shifted right or elided early.
                let fit_inset = crate::render::TEXT_FIT_MARGIN as i32;
                context.draw_text_fitted_cow(
                    Rect::new(
                        name_x - fit_inset,
                        y,
                        name_width + 2 * fit_inset as u32,
                        name_font.size().max(1.0) as u32,
                    ),
                    &name_text,
                    &name_font,
                    ink,
                    HorizontalAlignment::Left,
                );
                context.draw_text_fitted_cow(
                    Rect::new(
                        key_x - fit_inset,
                        y,
                        key_width.max(1) as u32 + 2 * fit_inset as u32,
                        key_font.size().max(1.0) as u32,
                    ),
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
                let mods = crate::shortcut::Modifiers::from_event_bits(*modifiers);
                // A recording session owns every key: Escape cancels it, Backspace clears the
                // captured sequence, Primary+Enter commits it, and anything else is appended.
                if self.is_capturing() {
                    if *key == 27 {
                        self.cancel_capture();
                        return;
                    }
                    if *key == 8 {
                        self.capture_keys.clear();
                        self.base.request_redraw();
                        return;
                    }
                    if *key == 13
                        && (mods.contains(crate::shortcut::Modifiers::CTRL)
                            || mods.contains(crate::shortcut::Modifiers::PRIMARY))
                    {
                        self.commit_capture();
                        return;
                    }
                    let key_name = keycode_to_name(*key, *modifiers);
                    if !key_name.is_empty() {
                        self.capture_keys.push(key_name);
                        self.base.request_redraw();
                    }
                    return;
                }
                // Undo and redo are the platform primary chord: Control+Z/Y on
                // Windows and Linux, Command+Z/Y on macOS. A shifted chord is a
                // different binding and stays unbound here.
                if primary_shortcut_pressed(*key, *modifiers, Key::Z) {
                    let _ = self.undo();
                    return;
                }
                if primary_shortcut_pressed(*key, *modifiers, Key::Y) {
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
            Event::FocusLost => {
                if self.is_capturing() {
                    self.commit_capture();
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
    let mods = crate::shortcut::Modifiers::from_event_bits(modifiers);
    let mut parts: Vec<&str> = Vec::new();
    let primary = mods.contains(crate::shortcut::Modifiers::PRIMARY);
    if mods.contains(crate::shortcut::Modifiers::SHIFT) {
        parts.push("Shift");
    }
    if primary {
        parts.push("Meta");
    } else if mods.contains(crate::shortcut::Modifiers::CTRL) {
        parts.push("Ctrl");
    }
    if mods.contains(crate::shortcut::Modifiers::ALT) {
        parts.push("Alt");
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

/// Case-insensitive substring test for the shortcut filter.
///
// The one case-insensitive substring implementation lives in `text_utils`, shared with the
// other filter-driven controls (`auto_complete_edit`, `cascader`, `mention`) that used to
// allocate a lowered `String` per candidate per keystroke.
use crate::widget::text_utils::contains_ignore_ascii_case;

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
    fn grouping_cache_is_invalidated_by_table_and_filter_changes() {
        let mut se = ShortcutEditor::new(Rect::new(0, 0, 400, 300));
        se.add_shortcut(ShortcutEntry::new("save", "Save File", "File"));
        se.add_shortcut(ShortcutEntry::new("copy", "Copy", "Edit"));

        {
            let groups = se.grouped();
            assert_eq!(groups.groups.len(), 2);
            assert_eq!(groups.groups[0].0, "File");
            assert_eq!(groups.groups[0].1, vec![0]);
        }

        // `shortcuts_mut` hands out a raw `&mut Vec`, so no setter runs; the generation bump at
        // the hand-out must still invalidate the cache.
        se.shortcuts_mut().push(ShortcutEntry::new("open", "Open", "File"));
        {
            let groups = se.grouped();
            let file = groups.groups.iter().find(|(name, _)| name == "File");
            assert_eq!(file.map(|(_, indices)| indices.clone()), Some(vec![0, 2]));
        }

        // The filter is part of the cache key, so a filter change re-groups even though the table
        // generation did not move.
        se.set_filter("copy");
        {
            let groups = se.grouped();
            assert_eq!(groups.groups.len(), 1);
            assert_eq!(groups.groups[0].0, "Edit");
            assert_eq!(groups.groups[0].1, vec![1]);
        }
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

    /// The contract describes what the *filter did*, not just what was typed into it.
    ///
    /// # The defect this pins
    ///
    /// `filter_text` was the only published property, and it is the one fact the caller supplied
    /// themselves. A consumer could set a filter and then had no way to ask how many shortcuts survived
    /// it — so an "N results" label or an "empty" state had to be rebuilt from the table, and a
    /// `filter_text` that matched nothing was indistinguishable from one that worked.
    #[test]
    fn the_contract_reports_what_the_filter_matched() {
        use crate::widget::capability::WidgetProperties;

        let mut se = ShortcutEditor::new(Rect::new(0, 0, 400, 300));
        assert_eq!(se.get("shortcut_count").unwrap().as_u64(), Some(0));
        assert_eq!(se.get("empty").unwrap().as_bool(), Some(true));
        assert_eq!(se.get("categories").unwrap().as_str(), Some(""));

        se.add_shortcut(ShortcutEntry::new("save", "Save", "File"));
        se.add_shortcut(ShortcutEntry::new("open", "Open", "File"));
        se.add_shortcut(ShortcutEntry::new("copy", "Copy", "Edit"));

        assert_eq!(se.get("shortcut_count").unwrap().as_u64(), Some(3));
        assert_eq!(se.get("visible_count").unwrap().as_u64(), Some(3));
        assert_eq!(se.get("category_count").unwrap().as_u64(), Some(2));
        assert_eq!(se.get("categories").unwrap().as_str(), Some("File, Edit"));
        assert_eq!(se.get("empty").unwrap().as_bool(), Some(false));

        // A filter that matches narrows all of them together.
        se.set_filter("sa");
        assert_eq!(se.get("filter_text").unwrap().as_str(), Some("sa"));
        assert_eq!(se.get("visible_count").unwrap().as_u64(), Some(1));
        assert_eq!(se.get("category_count").unwrap().as_u64(), Some(1));
        assert_eq!(se.get("categories").unwrap().as_str(), Some("File"));
        // The total is unchanged: the table still holds three, the filter only hid two.
        assert_eq!(
            se.get("shortcut_count").unwrap().as_u64(),
            Some(3),
            "hiding rows must not look like deleting them"
        );
        assert!(!se.is_empty(), "a filter matching nothing is not an empty table");

        // A filter matching nothing is a different state from an empty table.
        se.set_filter("zzzz");
        assert_eq!(se.get("visible_count").unwrap().as_u64(), Some(0));
        assert_eq!(se.get("empty").unwrap().as_bool(), Some(false));

        // Every derived name is read-only.
        for name in [
            "shortcut_count",
            "visible_count",
            "category_count",
            "categories",
            "empty",
            "can_undo",
            "can_redo",
        ] {
            assert!(
                se.set(name, CapabilityValue::UInt(0)).is_err(),
                "{name} must be refused as read-only"
            );
        }
    }

    /// Undo availability is published, so a toolbar button can bind to it.
    #[test]
    fn the_contract_publishes_undo_availability() {
        use crate::widget::capability::WidgetProperties;

        let mut se = ShortcutEditor::new(Rect::new(0, 0, 400, 300));
        assert_eq!(se.get("can_undo").unwrap().as_bool(), Some(false));
        assert_eq!(se.get("can_redo").unwrap().as_bool(), Some(false));

        se.add_shortcut(ShortcutEntry::new("save", "Save", "File"));
        assert_eq!(se.get("can_undo").unwrap().as_bool(), Some(true));

        se.undo();
        assert_eq!(se.get("can_undo").unwrap().as_bool(), Some(false));
        assert_eq!(se.get("can_redo").unwrap().as_bool(), Some(true));
    }

    /// The shared matcher follows `Shortcut::primary`: Control on Windows/Linux,
    /// Command on macOS, for both undo and redo.
    #[test]
    fn primary_shortcut_matches_ctrl_on_desktop_and_command_on_mac() {
        use crate::shortcut::PlatformShortcutStyle::{Desktop, Mac};

        // Windows/Linux encoding: Control is event bit 1.
        assert!(primary_shortcut_matches(90, 0b0010, Key::Z, Desktop));
        assert!(primary_shortcut_matches(89, 0b0010, Key::Y, Desktop));

        // macOS encoding: Command is event bit 3.
        assert!(primary_shortcut_matches(90, 0b1000, Key::Z, Mac));
        assert!(primary_shortcut_matches(89, 0b1000, Key::Y, Mac));
    }

    /// Only the plain primary chord is undo: a shifted chord and the other
    /// platform's accelerator are different bindings.
    #[test]
    fn primary_shortcut_rejects_shift_and_the_other_platform_chord() {
        use crate::shortcut::PlatformShortcutStyle::{Desktop, Mac};

        // Shift+Control+Z and Shift+Command+Z are not plain undo.
        assert!(!primary_shortcut_matches(90, 0b0011, Key::Z, Desktop));
        assert!(!primary_shortcut_matches(90, 0b1001, Key::Z, Mac));

        // Command is not the primary accelerator on Windows/Linux, and Control
        // is not the primary accelerator on macOS.
        assert!(!primary_shortcut_matches(90, 0b1000, Key::Z, Desktop));
        assert!(!primary_shortcut_matches(90, 0b0010, Key::Z, Mac));

        // Alt turns the chord into a different binding on either platform.
        assert!(!primary_shortcut_matches(90, 0b0110, Key::Z, Desktop));

        // Only Z undoes and only Y redoes.
        assert!(!primary_shortcut_matches(89, 0b0010, Key::Z, Desktop));
        assert!(!primary_shortcut_matches(90, 0b0010, Key::Y, Desktop));
    }

    /// The control routes its own key handling through the shared matcher.
    #[test]
    fn the_shortcut_editor_undoes_with_the_primary_chord() {
        let mut se = ShortcutEditor::new(Rect::new(0, 0, 400, 300));
        se.add_shortcut(ShortcutEntry::new("save", "Save", "File").with_keys(vec!["Ctrl+S"]));
        se.update_shortcut_keys("save", vec!["Ctrl+Shift+S".to_string()]);

        se.handle_event(&Event::KeyPress { key: 90, modifiers: primary_modifier_event_bits() });
        assert_eq!(se.shortcuts()[0].keys, vec!["Ctrl+S"]);

        se.handle_event(&Event::KeyPress { key: 89, modifiers: primary_modifier_event_bits() });
        assert_eq!(se.shortcuts()[0].keys, vec!["Ctrl+Shift+S"]);
    }

    /// A recording session captures every key, supports sequences and can be abandoned.
    #[test]
    fn recording_captures_a_multi_key_sequence() {
        let mut se = ShortcutEditor::new(Rect::new(0, 0, 400, 300));
        se.add_shortcut(ShortcutEntry::new("save", "Save", "File").with_keys(vec!["Ctrl+S"]));
        assert!(se.begin_capture("save"));
        assert!(se.is_capturing());

        se.handle_event(&Event::KeyPress { key: 75, modifiers: 2 }); // Ctrl+K
        se.handle_event(&Event::KeyPress { key: 83, modifiers: 2 }); // Ctrl+S
        assert_eq!(se.capture_keys().len(), 2);
        se.handle_event(&Event::KeyPress { key: 13, modifiers: 2 }); // Ctrl+Enter commits
        assert!(!se.is_capturing());
        assert_eq!(se.shortcuts()[0].keys, vec!["Ctrl+K", "Ctrl+S"]);

        // Escape abandons the next session, and Backspace clears what was captured so far.
        assert!(se.begin_capture("save"));
        se.handle_event(&Event::KeyPress { key: 65, modifiers: 0 });
        assert_eq!(se.capture_keys().len(), 1);
        se.handle_event(&Event::KeyPress { key: 8, modifiers: 0 });
        assert!(se.capture_keys().is_empty());
        se.handle_event(&Event::KeyPress { key: 27, modifiers: 0 });
        assert!(!se.is_capturing());
        assert_eq!(se.shortcuts()[0].keys, vec!["Ctrl+K", "Ctrl+S"]);
    }

    /// Conflicts are reported, and one binding can be cleared or reset without the table.
    #[test]
    fn conflicts_and_per_entry_clear_and_reset() {
        let mut se = ShortcutEditor::new(Rect::new(0, 0, 400, 300));
        se.add_shortcut(
            ShortcutEntry::new("open", "Open", "File")
                .with_keys(vec!["Ctrl+O"])
                .with_default_keys(vec!["Ctrl+O"]),
        );
        se.add_shortcut(ShortcutEntry::new("other", "Other", "File").with_keys(vec!["Ctrl+O"]));
        assert_eq!(se.conflicting_shortcuts("open"), vec!["other"]);
        assert!(se.has_conflict("other"));

        assert!(se.clear_shortcut("open"));
        assert!(se.shortcuts()[0].keys.is_empty());
        assert!(!se.has_conflict("other"), "an unbound entry collides with nothing");
        assert!(se.reset_shortcut("open"));
        assert_eq!(se.shortcuts()[0].keys, vec!["Ctrl+O"]);
    }

    /// The display form uses modifier symbols while the stored tokens stay portable.
    #[test]
    fn key_sequences_are_rendered_with_modifier_symbols() {
        assert_eq!(
            ShortcutEditor::format_keys(&["Ctrl+Shift+T".to_string(), "Alt+F4".to_string()]),
            "⌃⇧T, ⌥F4"
        );
    }
}
