/// Represents a keyboard shortcut (key combination).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Shortcut {
    /// Main key (e.g., 'A', 'F1', 'Enter').
    pub key: Key,
    /// Modifier keys (Ctrl, Alt, Shift, Meta).
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
    /// Supported formats: "Ctrl+A", "Alt+F4", "Ctrl+Shift+S", "F1".
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
    pub fn format_shortcut(&self) -> String {
        use std::fmt::Write;
        let mut result = String::new();
        if self.modifiers.contains(Modifiers::CTRL) {
            if !result.is_empty() {
                result.push('+');
            }
            result.push_str("Ctrl");
        }
        if self.modifiers.contains(Modifiers::ALT) {
            if !result.is_empty() {
                result.push('+');
            }
            result.push_str("Alt");
        }
        if self.modifiers.contains(Modifiers::SHIFT) {
            if !result.is_empty() {
                result.push('+');
            }
            result.push_str("Shift");
        }
        if self.modifiers.contains(Modifiers::META) {
            if !result.is_empty() {
                result.push('+');
            }
            result.push_str("Meta");
        }
        if !result.is_empty() {
            result.push('+');
        }
        let _ = write!(result, "{}", self.key);
        result
    }
    /// Returns true if this shortcut conflicts with another.
    pub fn conflicts_with(&self, other: &Shortcut) -> bool {
        self.key == other.key && self.modifiers == other.modifiers
    }
}
impl Default for Shortcut {
    fn default() -> Self {
        Self::new(Key::None, Modifiers::empty())
    }
}
impl std::fmt::Display for Shortcut {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.format_shortcut())
    }
}
/// Keyboard keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    /// No key.
    None,
    /// Letter keys.
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    /// Number keys (top row).
    Num0,
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,
    /// Function keys.
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    /// Special keys.
    Escape,
    Tab,
    Enter,
    Space,
    Backspace,
    Delete,
    Insert,
    Home,
    End,
    PageUp,
    PageDown,
    /// Arrow keys.
    Left,
    Right,
    Up,
    Down,
    /// Other keys.
    Minus,
    Equals,
    LeftBracket,
    RightBracket,
    Semicolon,
    Quote,
    Comma,
    Period,
    Slash,
    Backslash,
}
impl Key {
    /// Parses a key from a string.
    pub fn from_string(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "a" => Some(Key::A),
            "b" => Some(Key::B),
            "c" => Some(Key::C),
            "d" => Some(Key::D),
            "e" => Some(Key::E),
            "f" => Some(Key::F),
            "g" => Some(Key::G),
            "h" => Some(Key::H),
            "i" => Some(Key::I),
            "j" => Some(Key::J),
            "k" => Some(Key::K),
            "l" => Some(Key::L),
            "m" => Some(Key::M),
            "n" => Some(Key::N),
            "o" => Some(Key::O),
            "p" => Some(Key::P),
            "q" => Some(Key::Q),
            "r" => Some(Key::R),
            "s" => Some(Key::S),
            "t" => Some(Key::T),
            "u" => Some(Key::U),
            "v" => Some(Key::V),
            "w" => Some(Key::W),
            "x" => Some(Key::X),
            "y" => Some(Key::Y),
            "z" => Some(Key::Z),
            "0" => Some(Key::Num0),
            "1" => Some(Key::Num1),
            "2" => Some(Key::Num2),
            "3" => Some(Key::Num3),
            "4" => Some(Key::Num4),
            "5" => Some(Key::Num5),
            "6" => Some(Key::Num6),
            "7" => Some(Key::Num7),
            "8" => Some(Key::Num8),
            "9" => Some(Key::Num9),
            "f1" => Some(Key::F1),
            "f2" => Some(Key::F2),
            "f3" => Some(Key::F3),
            "f4" => Some(Key::F4),
            "f5" => Some(Key::F5),
            "f6" => Some(Key::F6),
            "f7" => Some(Key::F7),
            "f8" => Some(Key::F8),
            "f9" => Some(Key::F9),
            "f10" => Some(Key::F10),
            "f11" => Some(Key::F11),
            "f12" => Some(Key::F12),
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
            "left" => Some(Key::Left),
            "right" => Some(Key::Right),
            "up" => Some(Key::Up),
            "down" => Some(Key::Down),
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
    pub fn format_key(&self) -> &'static str {
        match self {
            Key::None => "None",
            Key::A => "A",
            Key::B => "B",
            Key::C => "C",
            Key::D => "D",
            Key::E => "E",
            Key::F => "F",
            Key::G => "G",
            Key::H => "H",
            Key::I => "I",
            Key::J => "J",
            Key::K => "K",
            Key::L => "L",
            Key::M => "M",
            Key::N => "N",
            Key::O => "O",
            Key::P => "P",
            Key::Q => "Q",
            Key::R => "R",
            Key::S => "S",
            Key::T => "T",
            Key::U => "U",
            Key::V => "V",
            Key::W => "W",
            Key::X => "X",
            Key::Y => "Y",
            Key::Z => "Z",
            Key::Num0 => "0",
            Key::Num1 => "1",
            Key::Num2 => "2",
            Key::Num3 => "3",
            Key::Num4 => "4",
            Key::Num5 => "5",
            Key::Num6 => "6",
            Key::Num7 => "7",
            Key::Num8 => "8",
            Key::Num9 => "9",
            Key::F1 => "F1",
            Key::F2 => "F2",
            Key::F3 => "F3",
            Key::F4 => "F4",
            Key::F5 => "F5",
            Key::F6 => "F6",
            Key::F7 => "F7",
            Key::F8 => "F8",
            Key::F9 => "F9",
            Key::F10 => "F10",
            Key::F11 => "F11",
            Key::F12 => "F12",
            Key::Escape => "Esc",
            Key::Tab => "Tab",
            Key::Enter => "Enter",
            Key::Space => "Space",
            Key::Backspace => "Backspace",
            Key::Delete => "Del",
            Key::Insert => "Ins",
            Key::Home => "Home",
            Key::End => "End",
            Key::PageUp => "PgUp",
            Key::PageDown => "PgDn",
            Key::Left => "Left",
            Key::Right => "Right",
            Key::Up => "Up",
            Key::Down => "Down",
            Key::Minus => "-",
            Key::Equals => "=",
            Key::LeftBracket => "[",
            Key::RightBracket => "]",
            Key::Semicolon => ";",
            Key::Quote => "'",
            Key::Comma => ",",
            Key::Period => ".",
            Key::Slash => "/",
            Key::Backslash => "\\",
        }
    }
}
impl std::fmt::Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.format_key())
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
    /// Unique identifier for the action.
    pub action_id: String,
    /// Human-readable description.
    pub description: String,
    /// The shortcut.
    pub shortcut: Shortcut,
    /// Whether the shortcut is currently enabled.
    pub enabled: bool,
}
