//! Global shortcut system for menu items and actions.
//!
//! This module provides keyboard shortcut registration, conflict detection,
//! and dispatching for menu items across the application.

use std::collections::HashMap;

use crate::signal::Signal1;

/// Represents a keyboard shortcut (key combination).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Shortcut {
    /// Main key (e.g., 'A', 'F1', 'Enter')
    pub key: Key,
    /// Modifier keys (Ctrl, Alt, Shift, Meta)
    pub modifiers: Modifiers,
}

impl Shortcut {
    /// Creates a new shortcut with the given key and modifiers.
    pub fn new(key: Key, modifiers: Modifiers) -> Self {
        Self { key, modifiers }
    }

    /// Creates a simple shortcut with no modifiers.
    pub fn from_key(key: Key) -> Self {
        Self::new(key, Modifiers::empty())
    }

    /// Creates a Ctrl+key shortcut.
    pub fn ctrl(key: Key) -> Self {
        Self::new(key, Modifiers::CTRL)
    }

    /// Creates an Alt+key shortcut.
    pub fn alt(key: Key) -> Self {
        Self::new(key, Modifiers::ALT)
    }

    /// Creates a Shift+key shortcut.
    pub fn shift(key: Key) -> Self {
        Self::new(key, Modifiers::SHIFT)
    }

    /// Creates a Ctrl+Alt+key shortcut.
    pub fn ctrl_alt(key: Key) -> Self {
        Self::new(key, Modifiers::CTRL | Modifiers::ALT)
    }

    /// Creates a Ctrl+Shift+key shortcut.
    pub fn ctrl_shift(key: Key) -> Self {
        Self::new(key, Modifiers::CTRL | Modifiers::SHIFT)
    }

    /// Creates a shortcut from a string representation.
    /// Supported formats: "Ctrl+A", "Alt+F4", "Ctrl+Shift+S", "F1"
    pub fn from_string(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('+').map(|p| p.trim()).collect();
        if parts.is_empty() {
            return None;
        }

        let mut modifiers = Modifiers::empty();
        let mut key_str = "";

        for part in &parts {
            match part.to_lowercase().as_str() {
                "ctrl" | "control" => modifiers |= Modifiers::CTRL,
                "alt" => modifiers |= Modifiers::ALT,
                "shift" => modifiers |= Modifiers::SHIFT,
                "meta" | "cmd" | "command" | "win" => modifiers |= Modifiers::META,
                _ => key_str = part,
            }
        }

        let key = Key::from_string(key_str)?;
        Some(Self::new(key, modifiers))
    }

    /// Returns a string representation of the shortcut.
    pub fn to_string(&self) -> String {
        let mut result = String::new();

        if self.modifiers.contains(Modifiers::CTRL) {
            if !result.is_empty() { result.push('+'); }
            result.push_str("Ctrl");
        }
        if self.modifiers.contains(Modifiers::ALT) {
            if !result.is_empty() { result.push('+'); }
            result.push_str("Alt");
        }
        if self.modifiers.contains(Modifiers::SHIFT) {
            if !result.is_empty() { result.push('+'); }
            result.push_str("Shift");
        }
        if self.modifiers.contains(Modifiers::META) {
            if !result.is_empty() { result.push('+'); }
            result.push_str("Meta");
        }
        if !result.is_empty() { result.push('+'); }
        result.push_str(&self.key.to_string());
        result
    }

    /// Returns true if this shortcut conflicts with another.
    /// Two shortcuts conflict if they have the same key and overlapping modifiers.
    pub fn conflicts_with(&self, other: &Shortcut) -> bool {
        self.key == other.key && self.modifiers == other.modifiers
    }
}

impl Default for Shortcut {
    fn default() -> Self {
        Self::new(Key::None, Modifiers::empty())
    }
}

/// Keyboard keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    /// No key
    None,
    /// Letter keys
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    /// Number keys (top row)
    Num0, Num1, Num2, Num3, Num4, Num5, Num6, Num7, Num8, Num9,
    /// Function keys
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    /// Special keys
    Escape, Tab, Enter, Space, Backspace, Delete,
    Insert, Home, End, PageUp, PageDown,
    /// Arrow keys
    Left, Right, Up, Down,
    /// Other keys
    Minus, Equals, LeftBracket, RightBracket,
    Semicolon, Quote, Comma, Period, Slash, Backslash,
}

impl Key {
    /// Parses a key from a string.
    pub fn from_string(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "a" => Some(Key::A), "b" => Some(Key::B), "c" => Some(Key::C),
            "d" => Some(Key::D), "e" => Some(Key::E), "f" => Some(Key::F),
            "g" => Some(Key::G), "h" => Some(Key::H), "i" => Some(Key::I),
            "j" => Some(Key::J), "k" => Some(Key::K), "l" => Some(Key::L),
            "m" => Some(Key::M), "n" => Some(Key::N), "o" => Some(Key::O),
            "p" => Some(Key::P), "q" => Some(Key::Q), "r" => Some(Key::R),
            "s" => Some(Key::S), "t" => Some(Key::T), "u" => Some(Key::U),
            "v" => Some(Key::V), "w" => Some(Key::W), "x" => Some(Key::X),
            "y" => Some(Key::Y), "z" => Some(Key::Z),
            "0" => Some(Key::Num0), "1" => Some(Key::Num1), "2" => Some(Key::Num2),
            "3" => Some(Key::Num3), "4" => Some(Key::Num4), "5" => Some(Key::Num5),
            "6" => Some(Key::Num6), "7" => Some(Key::Num7), "8" => Some(Key::Num8),
            "9" => Some(Key::Num9),
            "f1" => Some(Key::F1), "f2" => Some(Key::F2), "f3" => Some(Key::F3),
            "f4" => Some(Key::F4), "f5" => Some(Key::F5), "f6" => Some(Key::F6),
            "f7" => Some(Key::F7), "f8" => Some(Key::F8), "f9" => Some(Key::F9),
            "f10" => Some(Key::F10), "f11" => Some(Key::F11), "f12" => Some(Key::F12),
            "esc" | "escape" => Some(Key::Escape),
            "tab" => Some(Key::Tab),
            "enter" | "return" => Some(Key::Enter),
            "space" => Some(Key::Space),
            "backspace" | "back" => Some(Key::Backspace),
            "delete" | "del" => Some(Key::Delete),
            "insert" | "ins" => Some(Key::Insert),
            "home" => Some(Key::Home),
            "end" => Some(Key::End),
            "pageup" | "page_up" => Some(Key::PageUp),
            "pagedown" | "page_down" => Some(Key::PageDown),
            "left" => Some(Key::Left), "right" => Some(Key::Right),
            "up" => Some(Key::Up), "down" => Some(Key::Down),
            "minus" | "-" => Some(Key::Minus),
            "equals" | "=" => Some(Key::Equals),
            "[" | "leftbracket" => Some(Key::LeftBracket),
            "]" | "rightbracket" => Some(Key::RightBracket),
            ";" | "semicolon" => Some(Key::Semicolon),
            "'" | "quote" => Some(Key::Quote),
            "," | "comma" => Some(Key::Comma),
            "." | "period" => Some(Key::Period),
            "/" | "slash" => Some(Key::Slash),
            "\\" | "backslash" => Some(Key::Backslash),
            _ => None,
        }
    }

    /// Returns a string representation of the key.
    pub fn to_string(&self) -> String {
        match self {
            Key::None => "None".to_string(),
            Key::A => "A".to_string(), Key::B => "B".to_string(), Key::C => "C".to_string(),
            Key::D => "D".to_string(), Key::E => "E".to_string(), Key::F => "F".to_string(),
            Key::G => "G".to_string(), Key::H => "H".to_string(), Key::I => "I".to_string(),
            Key::J => "J".to_string(), Key::K => "K".to_string(), Key::L => "L".to_string(),
            Key::M => "M".to_string(), Key::N => "N".to_string(), Key::O => "O".to_string(),
            Key::P => "P".to_string(), Key::Q => "Q".to_string(), Key::R => "R".to_string(),
            Key::S => "S".to_string(), Key::T => "T".to_string(), Key::U => "U".to_string(),
            Key::V => "V".to_string(), Key::W => "W".to_string(), Key::X => "X".to_string(),
            Key::Y => "Y".to_string(), Key::Z => "Z".to_string(),
            Key::Num0 => "0".to_string(), Key::Num1 => "1".to_string(),
            Key::Num2 => "2".to_string(), Key::Num3 => "3".to_string(),
            Key::Num4 => "4".to_string(), Key::Num5 => "5".to_string(),
            Key::Num6 => "6".to_string(), Key::Num7 => "7".to_string(),
            Key::Num8 => "8".to_string(), Key::Num9 => "9".to_string(),
            Key::F1 => "F1".to_string(), Key::F2 => "F2".to_string(),
            Key::F3 => "F3".to_string(), Key::F4 => "F4".to_string(),
            Key::F5 => "F5".to_string(), Key::F6 => "F6".to_string(),
            Key::F7 => "F7".to_string(), Key::F8 => "F8".to_string(),
            Key::F9 => "F9".to_string(), Key::F10 => "F10".to_string(),
            Key::F11 => "F11".to_string(), Key::F12 => "F12".to_string(),
            Key::Escape => "Esc".to_string(),
            Key::Tab => "Tab".to_string(),
            Key::Enter => "Enter".to_string(),
            Key::Space => "Space".to_string(),
            Key::Backspace => "Backspace".to_string(),
            Key::Delete => "Del".to_string(),
            Key::Insert => "Ins".to_string(),
            Key::Home => "Home".to_string(),
            Key::End => "End".to_string(),
            Key::PageUp => "PgUp".to_string(),
            Key::PageDown => "PgDn".to_string(),
            Key::Left => "Left".to_string(),
            Key::Right => "Right".to_string(),
            Key::Up => "Up".to_string(),
            Key::Down => "Down".to_string(),
            Key::Minus => "-".to_string(),
            Key::Equals => "=".to_string(),
            Key::LeftBracket => "[".to_string(),
            Key::RightBracket => "]".to_string(),
            Key::Semicolon => ";".to_string(),
            Key::Quote => "'".to_string(),
            Key::Comma => ",".to_string(),
            Key::Period => ".".to_string(),
            Key::Slash => "/".to_string(),
            Key::Backslash => "\\".to_string(),
        }
    }
}

/// Modifier keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifiers(u8);

impl Modifiers {
    pub const NONE: Self = Self(0);
    pub const SHIFT: Self = Self(1 << 0);
    pub const CTRL: Self = Self(1 << 1);
    pub const ALT: Self = Self(1 << 2);
    pub const META: Self = Self(1 << 3);

    /// Creates empty modifiers.
    pub const fn empty() -> Self {
        Self::NONE
    }

    /// Returns true if no modifiers are set.
    pub const fn is_empty(&self) -> bool {
        self.0 == 0
    }

    /// Returns true if the given modifier is set.
    pub const fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl std::ops::BitOr for Modifiers {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for Modifiers {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// Entry in the shortcut registry.
#[derive(Debug, Clone)]
pub struct ShortcutEntry {
    /// Unique identifier for the action
    pub action_id: String,
    /// Human-readable description
    pub description: String,
    /// The shortcut
    pub shortcut: Shortcut,
    /// Whether the shortcut is currently enabled
    pub enabled: bool,
}

/// Global shortcut manager for registering and dispatching shortcuts.
pub struct ShortcutManager {
    /// Map from shortcut to action ID
    shortcuts: HashMap<Shortcut, String>,
    /// Map from action ID to entry
    entries: HashMap<String, ShortcutEntry>,
    /// Emitted when a shortcut is triggered
    pub shortcut_triggered: Signal1<String>,
    /// Emitted when a shortcut conflict is detected
    pub conflict_detected: Signal1<(Shortcut, String, String)>,
}

impl ShortcutManager {
    /// Creates a new shortcut manager.
    pub fn new() -> Self {
        Self {
            shortcuts: HashMap::new(),
            entries: HashMap::new(),
            shortcut_triggered: Signal1::new(),
            conflict_detected: Signal1::new(),
        }
    }

    /// Registers a new shortcut.
    /// Returns true if registration succeeded, false if there was a conflict.
    pub fn register(&mut self, action_id: impl Into<String>, shortcut: Shortcut, description: impl Into<String>) -> bool {
        let action_id = action_id.into();
        let description = description.into();

        // Check for conflicts
        if let Some(existing_action) = self.shortcuts.get(&shortcut) {
            if existing_action != &action_id {
                self.conflict_detected.emit((shortcut.clone(), action_id.clone(), existing_action.clone()));
                return false;
            }
        }

        // Remove old shortcut for this action if exists
        if let Some(entry) = self.entries.get(&action_id) {
            self.shortcuts.remove(&entry.shortcut);
        }

        // Register new shortcut
        let entry = ShortcutEntry {
            action_id: action_id.clone(),
            description,
            shortcut: shortcut.clone(),
            enabled: true,
        };

        self.shortcuts.insert(shortcut, action_id.clone());
        self.entries.insert(action_id, entry);
        true
    }

    /// Unregisters a shortcut by action ID.
    pub fn unregister(&mut self, action_id: &str) -> bool {
        if let Some(entry) = self.entries.remove(action_id) {
            self.shortcuts.remove(&entry.shortcut);
            true
        } else {
            false
        }
    }

    /// Unregisters a shortcut by key combination.
    pub fn unregister_shortcut(&mut self, shortcut: &Shortcut) -> bool {
        if let Some(action_id) = self.shortcuts.remove(shortcut) {
            self.entries.remove(&action_id);
            true
        } else {
            false
        }
    }

    /// Handles a key event and triggers the associated action if a shortcut matches.
    /// Returns true if a shortcut was triggered.
    pub fn handle_key_event(&mut self, key: Key, modifiers: Modifiers) -> bool {
        let shortcut = Shortcut::new(key, modifiers);
        
        if let Some(action_id) = self.shortcuts.get(&shortcut) {
            if let Some(entry) = self.entries.get(action_id) {
                if entry.enabled {
                    self.shortcut_triggered.emit(action_id.clone());
                    return true;
                }
            }
        }
        false
    }

    /// Enables a shortcut by action ID.
    pub fn enable(&mut self, action_id: &str) -> bool {
        if let Some(entry) = self.entries.get_mut(action_id) {
            entry.enabled = true;
            true
        } else {
            false
        }
    }

    /// Disables a shortcut by action ID.
    pub fn disable(&mut self, action_id: &str) -> bool {
        if let Some(entry) = self.entries.get_mut(action_id) {
            entry.enabled = false;
            true
        } else {
            false
        }
    }

    /// Returns the shortcut for an action ID.
    pub fn get_shortcut(&self, action_id: &str) -> Option<&Shortcut> {
        self.entries.get(action_id).map(|e| &e.shortcut)
    }

    /// Returns the action ID for a shortcut.
    pub fn get_action(&self, shortcut: &Shortcut) -> Option<&String> {
        self.shortcuts.get(shortcut)
    }

    /// Returns all registered shortcuts.
    pub fn all_shortcuts(&self) -> &HashMap<Shortcut, String> {
        &self.shortcuts
    }

    /// Returns all entries.
    pub fn all_entries(&self) -> &HashMap<String, ShortcutEntry> {
        &self.entries
    }

    /// Clears all shortcuts.
    pub fn clear(&mut self) {
        self.shortcuts.clear();
        self.entries.clear();
    }

    /// Returns true if a shortcut is registered.
    pub fn has_shortcut(&self, shortcut: &Shortcut) -> bool {
        self.shortcuts.contains_key(shortcut)
    }

    /// Returns true if an action has a shortcut registered.
    pub fn has_action(&self, action_id: &str) -> bool {
        self.entries.contains_key(action_id)
    }

    /// Finds conflicts between the given shortcut and existing shortcuts.
    pub fn find_conflicts(&self, shortcut: &Shortcut) -> Vec<&ShortcutEntry> {
        self.entries
            .values()
            .filter(|e| e.shortcut.conflicts_with(shortcut))
            .collect()
    }
}

impl Default for ShortcutManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shortcut_parsing() {
        let shortcut = Shortcut::from_string("Ctrl+A").unwrap();
        assert_eq!(shortcut.key, Key::A);
        assert!(shortcut.modifiers.contains(Modifiers::CTRL));
        assert!(!shortcut.modifiers.contains(Modifiers::ALT));

        let shortcut = Shortcut::from_string("Alt+F4").unwrap();
        assert_eq!(shortcut.key, Key::F4);
        assert!(shortcut.modifiers.contains(Modifiers::ALT));

        let shortcut = Shortcut::from_string("Ctrl+Shift+S").unwrap();
        assert_eq!(shortcut.key, Key::S);
        assert!(shortcut.modifiers.contains(Modifiers::CTRL));
        assert!(shortcut.modifiers.contains(Modifiers::SHIFT));
    }

    #[test]
    fn test_shortcut_to_string() {
        let shortcut = Shortcut::ctrl(Key::A);
        assert_eq!(shortcut.to_string(), "Ctrl+A");

        let shortcut = Shortcut::new(Key::F1, Modifiers::CTRL | Modifiers::ALT);
        assert_eq!(shortcut.to_string(), "Ctrl+Alt+F1");
    }

    #[test]
    fn test_shortcut_manager_register() {
        let mut manager = ShortcutManager::new();
        
        assert!(manager.register("action1", Shortcut::ctrl(Key::A), "Action 1"));
        assert!(manager.has_action("action1"));
        assert!(manager.has_shortcut(&Shortcut::ctrl(Key::A)));
        
        // Duplicate registration should succeed for same action
        assert!(manager.register("action1", Shortcut::ctrl(Key::A), "Action 1"));
        
        // Different action with same shortcut should fail
        assert!(!manager.register("action2", Shortcut::ctrl(Key::A), "Action 2"));
    }

    #[test]
    fn test_shortcut_manager_trigger() {
        let mut manager = ShortcutManager::new();
        manager.register("action1", Shortcut::ctrl(Key::A), "Action 1");
        
        // Should trigger for matching shortcut
        assert!(manager.handle_key_event(Key::A, Modifiers::CTRL));
        
        // Should not trigger for non-matching shortcut
        assert!(!manager.handle_key_event(Key::B, Modifiers::CTRL));
        assert!(!manager.handle_key_event(Key::A, Modifiers::ALT));
    }

    #[test]
    fn test_shortcut_manager_enable_disable() {
        let mut manager = ShortcutManager::new();
        manager.register("action1", Shortcut::ctrl(Key::A), "Action 1");
        
        assert!(manager.handle_key_event(Key::A, Modifiers::CTRL));
        
        manager.disable("action1");
        assert!(!manager.handle_key_event(Key::A, Modifiers::CTRL));
        
        manager.enable("action1");
        assert!(manager.handle_key_event(Key::A, Modifiers::CTRL));
    }

    #[test]
    fn test_shortcut_conflict_detection() {
        let shortcut1 = Shortcut::ctrl(Key::A);
        let shortcut2 = Shortcut::ctrl(Key::A);
        let shortcut3 = Shortcut::alt(Key::A);
        
        assert!(shortcut1.conflicts_with(&shortcut2));
        assert!(!shortcut1.conflicts_with(&shortcut3));
    }
}
