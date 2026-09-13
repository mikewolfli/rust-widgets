// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

/// Represents a keyboard shortcut (key combination).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
    /// Creates a primary-modifier shortcut (`Cmd` on macOS, `Ctrl` elsewhere).
    ///
    /// Use this for every application command (Undo, Save, Copy, ...). Writing
    /// `Modifiers::CTRL` directly would hard-code the `Ctrl` convention onto
    /// macOS, where the platform-standard accelerator is `Command`.
    ///
    /// ```
    /// use rust_widgets::shortcut::{Key, Shortcut};
    ///
    /// let undo = Shortcut::primary(Key::Z);
    /// assert_eq!(undo.key, Key::Z);
    /// ```
    pub fn primary(key: Key) -> Self {
        Self::new(key, Modifiers::PRIMARY)
    }
    /// Creates a primary + Shift shortcut (e.g. Redo on Windows/Linux).
    pub fn primary_shift(key: Key) -> Self {
        Self::new(key, Modifiers::PRIMARY | Modifiers::SHIFT)
    }
    /// Creates a primary + Alt shortcut.
    pub fn primary_alt(key: Key) -> Self {
        Self::new(key, Modifiers::PRIMARY | Modifiers::ALT)
    }
    /// Creates a Ctrl+key shortcut.
    ///
    /// Prefer [`Shortcut::primary`] for application commands; `ctrl` remains for
    /// shortcuts that genuinely need the physical Control key on every platform.
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
    ///
    /// Supported formats: `"Ctrl+A"`, `"Alt+F4"`, `"Ctrl+Shift+S"`, `"F1"`.
    ///
    /// `primary`/`cmd`/`command` all produce [`Modifiers::PRIMARY`], which the
    /// backends resolve to `Command` on macOS and `Ctrl` on Windows/Linux. That
    /// keeps one shortcut declaration usable on every platform, and is also why
    /// `"Cmd+Z"` and `"Ctrl+Z"` parse to the *same* value rather than two
    /// different ones.
    pub fn from_string(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('+').map(|p| p.trim()).collect();
        if parts.is_empty() {
            return None;
        }
        let mut modifiers = Modifiers::empty();
        let mut key_str = "";
        for part in &parts {
            match part.to_lowercase().as_str() {
                "primary" | "cmdorctrl" | "cmd" | "command" | "ctrl" | "control" => {
                    modifiers |= Modifiers::PRIMARY
                }
                "alt" | "option" => modifiers |= Modifiers::ALT,
                "shift" => modifiers |= Modifiers::SHIFT,
                "meta" | "win" | "super" => modifiers |= Modifiers::META,
                _ => key_str = part,
            }
        }
        let key = Key::from_string(key_str)?;
        Some(Self::new(key, modifiers))
    }
    /// Returns a string representation of the shortcut.
    ///
    /// This is the **canonical, platform-independent** form: `PRIMARY` renders as
    /// `Primary`. Use [`crate::platform::Platform::format_shortcut`] (or
    /// [`crate::format_shortcut`]) to get the text a user should see on the
    /// current OS — `⌘⇧Z` on macOS, `Ctrl+Shift+Z` on Windows and Linux.
    pub fn format_shortcut(&self) -> String {
        use std::fmt::Write;
        let mut result = String::new();
        if self.modifiers.contains(Modifiers::PRIMARY) {
            result.push_str("Primary");
        }
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
    /// Converts the framework key-code convention used by `Event::KeyPress`.
    pub fn from_key_code(code: u32) -> Option<Self> {
        match code {
            65 | 97 => Some(Key::A),
            66 | 98 => Some(Key::B),
            67 | 99 => Some(Key::C),
            68 | 100 => Some(Key::D),
            69 | 101 => Some(Key::E),
            70 | 102 => Some(Key::F),
            71 | 103 => Some(Key::G),
            72 | 104 => Some(Key::H),
            73 | 105 => Some(Key::I),
            74 | 106 => Some(Key::J),
            75 | 107 => Some(Key::K),
            76 | 108 => Some(Key::L),
            77 | 109 => Some(Key::M),
            78 | 110 => Some(Key::N),
            79 | 111 => Some(Key::O),
            80 | 112 => Some(Key::P),
            81 | 113 => Some(Key::Q),
            82 | 114 => Some(Key::R),
            83 | 115 => Some(Key::S),
            84 | 116 => Some(Key::T),
            85 | 117 => Some(Key::U),
            86 | 118 => Some(Key::V),
            87 | 119 => Some(Key::W),
            88 | 120 => Some(Key::X),
            89 | 121 => Some(Key::Y),
            90 | 122 => Some(Key::Z),
            48 => Some(Key::Num0),
            49 => Some(Key::Num1),
            50 => Some(Key::Num2),
            51 => Some(Key::Num3),
            52 => Some(Key::Num4),
            53 => Some(Key::Num5),
            54 => Some(Key::Num6),
            55 => Some(Key::Num7),
            56 => Some(Key::Num8),
            57 => Some(Key::Num9),
            27 => Some(Key::Escape),
            9 => Some(Key::Tab),
            10 | 13 => Some(Key::Enter),
            32 => Some(Key::Space),
            8 => Some(Key::Backspace),
            46 | 127 => Some(Key::Delete),
            45 => Some(Key::Insert),
            36 => Some(Key::Home),
            35 => Some(Key::End),
            33 => Some(Key::PageUp),
            34 => Some(Key::PageDown),
            37 => Some(Key::Left),
            39 => Some(Key::Right),
            38 => Some(Key::Up),
            40 => Some(Key::Down),
            1120 => Some(Key::F1),
            1121 => Some(Key::F2),
            1122 => Some(Key::F3),
            1123 => Some(Key::F4),
            1124 => Some(Key::F5),
            1125 => Some(Key::F6),
            1126 => Some(Key::F7),
            1127 => Some(Key::F8),
            1128 => Some(Key::F9),
            1129 => Some(Key::F10),
            1130 => Some(Key::F11),
            1131 => Some(Key::F12),
            189 => Some(Key::Minus),
            187 => Some(Key::Equals),
            219 => Some(Key::LeftBracket),
            221 => Some(Key::RightBracket),
            186 => Some(Key::Semicolon),
            222 => Some(Key::Quote),
            188 => Some(Key::Comma),
            190 => Some(Key::Period),
            191 => Some(Key::Slash),
            220 => Some(Key::Backslash),
            _ => None,
        }
    }

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub struct Modifiers(u8);
impl Modifiers {
    pub const NONE: Self = Self(0);
    pub const SHIFT: Self = Self(1 << 0);
    pub const CTRL: Self = Self(1 << 1);
    pub const ALT: Self = Self(1 << 2);
    pub const META: Self = Self(1 << 3);
    /// The platform's **primary** accelerator modifier.
    ///
    /// Resolves to `Command` on macOS and `Ctrl` on Windows/Linux. Declaring a
    /// command as `Primary` instead of `Ctrl` is what lets a single shortcut
    /// table drive every desktop platform without `cfg` in application code.
    ///
    /// Kept as its own bit (rather than aliasing [`Modifiers::CTRL`]) so that a
    /// backend can still tell "primary" from "physical Control" — on macOS
    /// those are genuinely different keys.
    pub const PRIMARY: Self = Self(1 << 4);
    /// Bit position of the meta/command modifier in the `Event::KeyPress`
    /// convention (which predates this type and cannot be renumbered).
    const EVENT_META: u8 = 0b1000;
    /// Bit position of the control modifier in the event convention.
    const EVENT_CTRL: u8 = 0b0010;
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

    /// Returns a copy with `other`'s bits cleared.
    ///
    /// Written as an explicit method (rather than `Not`/`Sub`) because clearing
    /// bits is the only inverse operation callers need, and an expressive name
    /// keeps the intent readable at call sites such as modifier translation.
    pub const fn without(&self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    /// Converts the framework modifier bitmask used by `Event::KeyPress`.
    ///
    /// Meta becomes [`Modifiers::PRIMARY`]: a key event carrying Command matches
    /// a shortcut declared with `Shortcut::primary`.
    pub const fn from_event_bits(bits: u32) -> Self {
        Self::from_raw_event_bits(bits)
    }

    /// Converts an event bitmask where the **control** bit means the primary
    /// accelerator rather than the physical Control key.
    ///
    /// Windows is the reason this exists: it has no Command key, so the OS
    /// shortcut convention (`Ctrl+Z`, `Ctrl+S`) is expressed with Control, and a
    /// `Shortcut::primary` binding has to resolve against it. It is a distinct
    /// entry point rather than the default so the difference from macOS stays
    /// visible at the call site instead of hiding inside a shared helper.
    pub const fn from_event_bits_primary_is_ctrl(bits: u32) -> Self {
        let translated = if bits & Self::EVENT_META as u32 != 0 {
            // A Command bit still means PRIMARY; drop the raw Control bit so it
            // is not counted twice.
            (bits & !(Self::EVENT_CTRL as u32)) | Self::EVENT_META as u32
        } else {
            bits
        };
        Self::from_raw_event_bits(translated)
    }

    /// Shared bit-unpacking used by both event conversions above.
    const fn from_raw_event_bits(bits: u32) -> Self {
        let raw = bits as u8;
        // Event bits 0..=2 map 1:1 onto SHIFT/CTRL/ALT.
        let mut modifiers = Self(raw & (Self::SHIFT.0 | Self::CTRL.0 | Self::ALT.0));
        if raw & Self::EVENT_META != 0 {
            // The event's Command bit is the portable "primary accelerator"
            // signal, so it lands on PRIMARY. It also implies physical Control
            // on macOS, which is why CTRL is set alongside it: `Control+C` and
            // `Command+C` both mean "copy" at the widget layer.
            modifiers = Self(modifiers.0 | Self::PRIMARY.0 | Self::CTRL.0);
        }
        modifiers
    }
    /// Returns the raw bit pattern, for backends that need to translate it.
    pub const fn bits(&self) -> u8 {
        self.0
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
/// A shortcut binding registered in the global shortcut system.
///
/// # Not the same as [`crate::widget::input_widgets::shortcut_editor::ShortcutEntry`]
///
/// Both are called `ShortcutEntry` but model different things (principle #49):
///
/// * this one — a **registered binding**: which action a shortcut fires, and
///   whether it is currently active;
/// * the editor's entry — a **row in the editor UI**, carrying editable fields
///   (default keys, category, whether the user may rebind it).
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

/// How a platform writes the glyphs of a keyboard shortcut.
///
/// Menu accelerators are not spelled the same way everywhere: macOS uses symbol
/// glyphs joined without separators (`⌘⇧Z`), while Windows and Linux spell the
/// modifiers out and join them with `+` (`Ctrl+Shift+Z`).
///
/// Keeping this as a plain value type (rather than `cfg`-gated code) means the
/// formatting rules are unit-testable for *every* platform on any host — the
/// tests do not need a Windows or a Mac to assert what Windows or macOS shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlatformShortcutStyle {
    /// `⌘`, `⌥`, `⌃`, `⇧` — macOS AppKit convention.
    Mac,
    /// `Ctrl+`, `Alt+`, `Shift+` — Windows Win32 / GTK convention.
    Desktop,
}

impl PlatformShortcutStyle {
    /// The style used by the operating system the library is compiled for.
    ///
    /// This is the fallback for backends that do not declare a style of their
    /// own; the authoritative choice is [`Platform::shortcut_style`]. The
    /// compile-target mapping itself lives in `src/platform/`, which is the only
    /// layer allowed to know which OS is being built for (principle #36).
    ///
    /// [`Platform::shortcut_style`]: crate::platform::types::Platform::shortcut_style
    pub const fn current() -> Self {
        crate::platform::types::compile_target_shortcut_style()
    }

    /// Renders `shortcut` in this platform's notation.
    ///
    /// [`Modifiers::PRIMARY`] resolves to `⌘`/`Command` on [`Self::Mac`] and to
    /// `Ctrl` on [`Self::Desktop`], which is the whole point of the modifier: the
    /// caller writes one shortcut and every platform shows its own idiom.
    pub fn format(self, shortcut: &Shortcut) -> String {
        let modifiers = shortcut.modifiers;
        let key = shortcut.key.format_key();
        match self {
            Self::Mac => {
                // AppKit orders modifiers as Control, Option, Shift, Command.
                let mut rendered = String::new();
                if modifiers.contains(Modifiers::CTRL) {
                    rendered.push('⌃');
                }
                if modifiers.contains(Modifiers::ALT) {
                    rendered.push('⌥');
                }
                if modifiers.contains(Modifiers::SHIFT) {
                    rendered.push('⇧');
                }
                if modifiers.contains(Modifiers::PRIMARY) || modifiers.contains(Modifiers::META) {
                    rendered.push('⌘');
                }
                rendered.push_str(&Self::mac_key_label(shortcut.key));
                rendered
            }
            Self::Desktop => {
                let mut parts: Vec<&str> = Vec::new();
                if modifiers.contains(Modifiers::PRIMARY) {
                    parts.push("Ctrl");
                }
                if modifiers.contains(Modifiers::ALT) {
                    parts.push("Alt");
                }
                if modifiers.contains(Modifiers::SHIFT) {
                    parts.push("Shift");
                }
                if modifiers.contains(Modifiers::CTRL) {
                    // A physical-Control shortcut on Windows/Linux still reads
                    // "Ctrl"; it is only distinct from PRIMARY on macOS.
                    if !modifiers.contains(Modifiers::PRIMARY) {
                        parts.push("Ctrl");
                    }
                }
                if modifiers.contains(Modifiers::META) {
                    parts.push("Win");
                }
                if parts.is_empty() {
                    return key.to_string();
                }
                format!("{}+{key}", parts.join("+"))
            }
        }
    }

    /// Mac label for a key, substituting the glyphs macOS uses in menus.
    fn mac_key_label(key: Key) -> String {
        match key {
            Key::Enter => "↩".to_string(),
            Key::Tab => "⇥".to_string(),
            Key::Escape => "⎋".to_string(),
            Key::Space => "Space".to_string(),
            Key::Delete => "⌫".to_string(),
            Key::Backspace => "⌫".to_string(),
            Key::Insert => "Help".to_string(),
            Key::Left => "←".to_string(),
            Key::Right => "→".to_string(),
            Key::Up => "↑".to_string(),
            Key::Down => "↓".to_string(),
            Key::PageUp => "⇞".to_string(),
            Key::PageDown => "⇟".to_string(),
            Key::Home => "↖".to_string(),
            Key::End => "↘".to_string(),
            other => other.format_key().to_string(),
        }
    }
}

/// Renders a shortcut using `style`'s notation.
///
/// Prefer [`crate::format_shortcut`], which picks the style of the host OS; this
/// entry point exists so callers (and tests) can render the *other* platforms'
/// notation explicitly.
pub fn format_shortcut_for_platform(shortcut: &Shortcut, style: PlatformShortcutStyle) -> String {
    style.format(shortcut)
}
