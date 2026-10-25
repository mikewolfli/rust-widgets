// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Keyboard widget — on-screen virtual keyboard (BLUE13 R2.6).
//!
//! Displays a grid of keys (QWERTY layout by default). Each key generates
//! a [`Signal1<(u32, u32)>`] with the key code and modifiers on press.
//! Special keys (Enter, Backspace, Space) also emit dedicated signals.

use crate::compat::{format, vec, String, ToString, Vec};
use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::key_codes;
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::borrow::Cow;

/// The key code this board's own layout table gives its Shift key.
///
/// Local rather than in `event::key_codes` because Shift is a **board** key: it is the board's
/// layout that assigns the code, and a host deliver is not obliged to use it. The framework-wide
/// codes (Enter, Backspace) come from `event::key_codes`, so the board and a dialog agree on those.
const SHIFT_CODE: u32 = 16;

/// The code a key carries when it produces no character at all.
///
///"No key" rather than a key: the board's rows include layout spacers whose whole definition is
/// that they emit nothing, and they are drawn as special keys for that reason.
const NO_KEY: u32 = 0;

/// Height of the suggestion bar painted over the first key row.
const SUGGESTION_BAR_HEIGHT: u32 = 28;

/// The character a key code denotes, when it denotes one at all.
fn code_char(code: u32) -> Option<char> {
    match code {
        // Letter codes are ASCII uppercase.
        65..=90 => char::from_u32(code),
        32 => Some(' '),
        _ => char::from_u32(code).filter(|ch| ch.is_ascii_graphic()),
    }
}

/// Composes a dead key with a base letter, when the pair has a precomposed form.
fn compose_accent(dead: char, base: char) -> Option<char> {
    let lower = base.to_ascii_lowercase();
    let composed = match (dead, lower) {
        ('`', 'a') => 'à',
        ('`', 'e') => 'è',
        ('`', 'i') => 'ì',
        ('`', 'o') => 'ò',
        ('`', 'u') => 'ù',
        ('^', 'a') => 'â',
        ('^', 'e') => 'ê',
        ('^', 'i') => 'î',
        ('^', 'o') => 'ô',
        ('^', 'u') => 'û',
        _ => return None,
    };
    Some(if base.is_uppercase() { composed.to_ascii_uppercase() } else { composed })
}

/// Layout variants for the on-screen keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardLayout {
    /// Standard QWERTY letter layout (default).
    Qwerty,
    /// QWERTZ letter layout, used in German-speaking regions.
    Qwertz,
    /// Numeric/symbol layout.
    Numeric,
    /// Emoji grid. Each key commits its glyph through [`Keyboard::text_committed`].
    Emoji,
}

/// Virtual keyboard key definition.
#[derive(Debug, Clone)]
pub struct KeyDefinition {
    /// Display label (e.g. "A", "Space", "Enter").
    pub label: String,
    /// Key code emitted when this key is pressed.
    pub key_code: u32,
    /// Width multiplier relative to a standard key (1.0 = normal, 1.5 = wider, 2.0+ = wide).
    pub width_ratio: f32,
}

/// On-screen virtual keyboard widget.
///
/// Displays rows of clickable keys. Pressing a key emits the [`key_pressed`]
/// signal and, for the three special keys, their dedicated signal as well.
///
/// [`key_pressed`]: #structfield.key_pressed
pub struct Keyboard {
    base: BaseWidget,
    layout: KeyboardLayout,
    keys: Vec<Vec<KeyDefinition>>,
    /// Current shift state (`true` = uppercase).
    shift: bool,
    /// Whether to show lowercase when shift is off.
    lowercase: bool,
    /// The key currently held down, as `(row, column)`, or `None` when none is.
    ///
    /// # Why the keyboard needed this while the mouse is "pressed"
    ///
    /// `BaseWidget` records that the **control** is pressed, not which of its keys is, and a
    /// keyboard is a grid of independent targets: pressing one key and seeing the whole board react
    /// would be a lie about which key is being activated. Until this field existed a key down gave
    /// no visual confirmation at all — the signal fired, and the board looked exactly as it did a
    /// moment before, which is the one affordance a virtual keyboard most needs because the user's
    /// finger is covering the key they pressed.
    ///
    /// The pair rather than the key code: the layout has two keys with the same code in different
    /// rows (a numeric row and a keypad column both carry digit codes), so a code alone cannot name
    /// the key to highlight.
    pressed_key: Option<(usize, usize)>,
    /// Signal emitted with `(key_code, modifiers)` when any key is pressed.
    pub key_pressed: Signal1<(u32, u32)>,
    /// Signal emitted when Enter (key code 13) is pressed.
    pub enter_pressed: GenericSignal,
    /// Signal emitted when Backspace (key code 8) is pressed.
    pub backspace_pressed: GenericSignal,
    /// Signal emitted when Space (key code 32) is pressed.
    pub space_pressed: GenericSignal,
    /// Text committed by a key that does not map to a key code (an emoji), by a dead-key
    /// composition, or by accepting a suggestion.
    pub text_committed: Signal1<String>,
    /// Whether Caps Lock is on. Shift and Caps Lock combine by exclusive-or.
    caps_lock: bool,
    /// A pending dead key (an accent) that composes with the next letter.
    dead_key: Option<char>,
    /// The key held down, for auto-repeat and long-press accents.
    held_key: Option<(usize, usize)>,
    /// Milliseconds the held key has been down.
    repeat_elapsed: u32,
    /// The open long-press accent popup: the key and its variants.
    accent_popup: Option<(usize, usize, Vec<char>)>,
    /// Remaining press-flash weight, in `0..=1`, animated by [`Keyboard::tick`].
    press_progress: f32,
    /// Whether a pressed key scales down and trails a ripple ring.
    ///
    /// Off by default: the press flash above is the whole of the resting contract, so a board
    /// that never asks for motion paints exactly the frame it always did (and the reference
    /// snapshots stay byte-identical). Turned on, the key under the finger shrinks by at most
    /// two pixels and a ring expands out of it as the flash decays — the touch-keyboard
    /// affordance that a brightness change alone does not give.
    press_animation: bool,
    /// Suggestion-bar entries, shown above the keys when non-empty.
    suggestions: Vec<String>,
    /// Keys crossed while the pointer is held, for swipe input.
    swipe_text: String,
    /// The repeat and long-press rhythm, configurable per host (I-21).
    timing: KeyboardTiming,
}

/// The repeat and long-press rhythm of the on-screen keyboard (I-21).
///
/// `tick` carried the three numbers as literals, so a host could not tune the board for its own
/// device — a touch panel and a desktop window want very different repeat rates, and a
/// large-screen kiosk wants a longer long-press than a phone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyboardTiming {
    /// How long a key must be held before it starts repeating, in milliseconds.
    pub repeat_initial_ms: u32,
    /// The gap between repeats once repeating has started, in milliseconds.
    pub repeat_interval_ms: u32,
    /// How long a hold must last before the accent popup opens, in milliseconds.
    pub long_press_ms: u32,
}

impl Default for KeyboardTiming {
    /// The values the board has always used.
    fn default() -> Self {
        Self { repeat_initial_ms: 450, repeat_interval_ms: 90, long_press_ms: 500 }
    }
}

impl Keyboard {
    /// Create a new keyboard widget with the given bounding rectangle.
    ///
    /// Initial layout is QWERTY, shift is off, lowercase is enabled.
    pub fn new(rect: Rect) -> Self {
        let base = BaseWidget::new(WidgetKind::Keyboard, rect, "Keyboard");
        let mut kbd = Self {
            base,
            layout: KeyboardLayout::Qwerty,
            keys: Vec::new(),
            shift: false,
            lowercase: true,
            pressed_key: None,
            key_pressed: Signal1::new(),
            enter_pressed: GenericSignal::new(),
            backspace_pressed: GenericSignal::new(),
            space_pressed: GenericSignal::new(),
            text_committed: Signal1::new(),
            caps_lock: false,
            dead_key: None,
            held_key: None,
            repeat_elapsed: 0,
            accent_popup: None,
            press_progress: 0.0,
            press_animation: false,
            suggestions: Vec::new(),
            swipe_text: String::new(),
            timing: KeyboardTiming::default(),
        };
        kbd.build_qwerty_layout();
        kbd
    }

    /// Set the keyboard layout and rebuild the key grid.
    pub fn set_layout(&mut self, layout: KeyboardLayout) {
        self.layout = layout;
        match layout {
            KeyboardLayout::Qwerty => self.build_qwerty_layout(),
            KeyboardLayout::Qwertz => self.build_qwertz_layout(),
            KeyboardLayout::Numeric => self.build_numeric_layout(),
            KeyboardLayout::Emoji => self.build_emoji_layout(),
        }
        self.base.request_redraw();
    }

    /// Return the current layout variant.
    pub fn layout(&self) -> KeyboardLayout {
        self.layout
    }

    /// Installs a caller-defined key grid (I-20).
    ///
    /// [`KeyDefinition`] was a public type that could not be installed: `keys` is private and
    /// [`Self::set_layout`] accepts only the four presets, so a host that wanted a functional row,
    /// a regional layout or a keypad of its own had a type with no entry point. This takes the
    /// same invalidation path the presets do — a held key, its repeat and any open accent popup
    /// describe the *old* grid and are cleared before the new one is painted.
    ///
    /// [`KeyboardLayout`] stays the preset convenience: [`Self::layout`] keeps reporting the last
    /// preset that was applied, because a caller-defined grid is not one of them.
    pub fn set_keys(&mut self, rows: Vec<Vec<KeyDefinition>>) {
        self.keys = rows;
        self.held_key = None;
        self.repeat_elapsed = 0;
        self.accent_popup = None;
        self.pressed_key = None;
        self.base.request_redraw();
    }

    /// The repeat and long-press rhythm (I-21).
    pub fn timing(&self) -> KeyboardTiming {
        self.timing
    }

    /// Replaces the repeat and long-press rhythm (I-21).
    ///
    /// A zero interval is accepted but read as one millisecond, because a repeat every zero
    /// milliseconds would spin `tick` forever on a single hold.
    pub fn set_timing(&mut self, timing: KeyboardTiming) {
        if self.timing == timing {
            return;
        }
        self.timing = timing;
    }

    /// Get the key at the given position (in widget-local coordinates).
    ///
    /// Returns `Some((row_index, col_index))` if a key covers that position,
    /// or `None` if the position is outside the keyboard or in a gap.
    pub fn key_at_position(&self, pos: Point) -> Option<(usize, usize)> {
        let rect = self.geometry();
        if !rect.contains_point(pos) {
            return None;
        }
        if self.keys.is_empty() {
            return None;
        }

        let total_height = rect.height as f32;
        let row_count = self.keys.len() as f32;
        let row_height = total_height / row_count;

        let local_x = pos.x as f32 - rect.x as f32;
        let local_y = pos.y as f32 - rect.y as f32;

        let row = (local_y / row_height) as usize;
        if row >= self.keys.len() {
            return None;
        }

        let row_keys = &self.keys[row];
        let total_ratio: f32 = row_keys.iter().map(|k| k.width_ratio).sum();
        let row_width = rect.width as f32;

        let mut cursor_x = 0.0f32;
        for (col, key) in row_keys.iter().enumerate() {
            let key_w = row_width * key.width_ratio / total_ratio;
            if local_x >= cursor_x && local_x < cursor_x + key_w {
                return Some((row, col));
            }
            cursor_x += key_w;
        }

        None
    }

    /// The accent popup's rectangle, derived the same way the paint derives it.
    pub fn accent_popup_rect(&self) -> Option<Rect> {
        let (row, col, variants) = self.accent_popup.as_ref()?;
        let rect = self.geometry();
        if self.keys.is_empty() {
            return None;
        }
        let row_height = rect.height as f32 / self.keys.len() as f32;
        let row_width = rect.width as f32;
        let total_ratio: f32 =
            self.keys[*row].iter().map(|key| key.width_ratio).sum::<f32>().max(0.001);
        let mut key_x = rect.x as f32;
        for key in self.keys[*row].iter().take(*col) {
            key_x += row_width * key.width_ratio / total_ratio;
        }
        let cell_width = 26i32;
        let popup_width = cell_width * variants.len() as i32;
        let popup_y = (rect.y + *row as i32 * row_height as i32 - 30)
            .max(rect.y)
            .min(rect.y + rect.height as i32 - 30);
        let popup_x = (key_x as i32).min(rect.x + rect.width as i32 - popup_width).max(rect.x);
        Some(Rect::new(popup_x, popup_y, popup_width.max(1) as u32, 28))
    }

    /// Toggle the shift state between uppercase and lowercase.
    pub fn toggle_shift(&mut self) {
        self.shift = !self.shift;
        self.base.request_redraw();
    }

    /// Return the current shift state.
    pub fn is_shifted(&self) -> bool {
        self.shift
    }

    /// Set whether lowercase letters are shown when shift is off.
    pub fn set_lowercase(&mut self, enabled: bool) {
        if self.lowercase == enabled {
            return;
        }
        self.lowercase = enabled;
        self.base.request_redraw();
    }

    /// Return whether lowercase mode is enabled.
    pub fn lowercase(&self) -> bool {
        self.lowercase
    }

    /// Whether Caps Lock is on.
    pub fn caps_lock(&self) -> bool {
        self.caps_lock
    }

    /// Sets Caps Lock. It combines with Shift by exclusive-or, so either one produces
    /// uppercase letters and both cancel.
    pub fn set_caps_lock(&mut self, caps_lock: bool) {
        if self.caps_lock == caps_lock {
            return;
        }
        self.caps_lock = caps_lock;
        self.base.request_redraw();
    }

    /// The pending dead key, if one is armed.
    pub fn dead_key(&self) -> Option<char> {
        self.dead_key
    }

    /// Arms a dead key; the next letter commits the composed character.
    ///
    /// A dead key with no composition leaves the accent followed by the letter, which is
    /// what every platform does when the pair has no precomposed form.
    pub fn set_dead_key(&mut self, dead_key: Option<char>) {
        if self.dead_key == dead_key {
            return;
        }
        self.dead_key = dead_key;
        self.base.request_redraw();
    }

    /// Whether the pressed-key scale/ripple animation is enabled.
    pub fn press_animation(&self) -> bool {
        self.press_animation
    }

    /// Enables or disables the pressed-key scale/ripple animation.
    ///
    /// The feature is opt-in so the default board keeps its static press flash.
    pub fn set_press_animation(&mut self, enabled: bool) {
        if self.press_animation == enabled {
            return;
        }
        self.press_animation = enabled;
        self.base.request_redraw();
    }

    /// The rectangle a key is painted in, including the opt-in press scale.
    ///
    /// The inset is derived from [`Self::press_progress`], so the key grows back to its full
    /// rectangle as the flash decays rather than snapping at release. A key that is not the
    /// pressed one, or a board with the animation off, keeps its exact resting rectangle.
    fn painted_key_rect(&self, key_rect: Rect, at: (usize, usize)) -> Rect {
        if !self.press_animation || self.pressed_key != Some(at) || self.press_progress <= 0.0 {
            return key_rect;
        }
        let inset = (2.0 * self.press_progress).round() as u32;
        let width = key_rect.width.saturating_sub(inset * 2).max(1);
        let height = key_rect.height.saturating_sub(inset * 2).max(1);
        Rect::new(key_rect.x + inset as i32, key_rect.y + inset as i32, width, height)
    }

    /// Commits text that did not come from a key code (an emoji, a dead-key composition or
    /// an accepted suggestion) through [`Self::text_committed`].
    pub fn commit_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if !text.is_empty() {
            self.text_committed.emit(text);
        }
    }

    /// The suggestion-bar entries.
    pub fn suggestions(&self) -> &[String] {
        &self.suggestions
    }

    /// Replaces the suggestion-bar entries. An empty list hides the bar.
    pub fn set_suggestions(&mut self, suggestions: Vec<String>) {
        if self.suggestions == suggestions {
            return;
        }
        self.suggestions = suggestions;
        self.base.request_redraw();
    }

    /// Accepts suggestion `index` and commits its text.
    pub fn commit_suggestion(&mut self, index: usize) -> bool {
        let Some(text) = self.suggestions.get(index).cloned() else {
            return false;
        };
        self.text_committed.emit(text);
        true
    }

    /// The accent variants offered for a letter key (long-press candidates).
    ///
    /// Keys without a precomposed accent family return an empty list and therefore show no
    /// popup. The set is deliberately small and explicit: it is the Latin accents a touch
    /// keyboard actually offers, not a Unicode combining-mark generator.
    pub fn accent_variants(key_code: u32) -> Vec<char> {
        let Some(base) = char::from_u32(key_code).map(|ch| ch.to_ascii_lowercase()) else {
            return Vec::new();
        };
        let variants: &[char] = match base {
            'a' => &['à', 'á', 'â', 'ä', 'å', 'ã'],
            'e' => &['è', 'é', 'ê', 'ë', 'ē'],
            'i' => &['ì', 'í', 'î', 'ï', 'ī'],
            'o' => &['ò', 'ó', 'ô', 'ö', 'õ', 'ø'],
            'u' => &['ù', 'ú', 'û', 'ü', 'ū'],
            'c' => &['ç', 'ć', 'č'],
            'n' => &['ñ', 'ń'],
            's' => &['ś', 'š', 'ß'],
            'y' => &['ý', 'ÿ'],
            _ => &[],
        };
        variants.to_vec()
    }

    /// The open accent popup, if any: the key's position and its variants.
    pub fn accent_popup(&self) -> Option<((usize, usize), &[char])> {
        self.accent_popup.as_ref().map(|(row, col, variants)| ((*row, *col), variants.as_slice()))
    }

    /// Opens the accent popup for a key, when that key has variants.
    pub fn show_accents(&mut self, row: usize, col: usize) -> bool {
        let Some(key) = self.keys.get(row).and_then(|keys| keys.get(col)) else {
            return false;
        };
        let variants = Self::accent_variants(key.key_code);
        if variants.is_empty() {
            return false;
        }
        self.accent_popup = Some((row, col, variants));
        self.base.request_redraw();
        true
    }

    /// Commits variant `index` from the open accent popup.
    pub fn commit_accent(&mut self, index: usize) -> bool {
        let Some((_, _, variants)) = self.accent_popup.as_ref() else {
            return false;
        };
        let Some(character) = variants.get(index).copied() else {
            return false;
        };
        let uppercase = self.shift ^ self.caps_lock;
        let text =
            if uppercase { character.to_uppercase().to_string() } else { character.to_string() };
        self.accent_popup = None;
        self.text_committed.emit(text);
        self.base.request_redraw();
        true
    }

    /// Advances held-key repeat, the long-press timer and the press flash.
    ///
    /// Returns whether another frame is due. A held key repeats after the platform's
    /// initial delay; the same hold opens the accent popup when the key has variants.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        let mut due = false;
        if self.press_progress > 0.0 {
            self.press_progress = (self.press_progress - delta_ms as f32 / 140.0).max(0.0);
            due = true;
            self.base.request_redraw();
        }
        if let Some((row, col)) = self.held_key {
            self.repeat_elapsed = self.repeat_elapsed.saturating_add(delta_ms);
            due = true;
            // The three thresholds are the host's, not literals (I-21).
            let initial = self.timing.repeat_initial_ms;
            let interval = self.timing.repeat_interval_ms.max(1);
            if self.accent_popup.is_none() && self.repeat_elapsed >= self.timing.long_press_ms {
                self.show_accents(row, col);
            }
            if self.repeat_elapsed >= initial {
                let mut remaining = self.repeat_elapsed - initial;
                while remaining >= interval {
                    remaining -= interval;
                    if let Some(key) = self.keys.get(row).and_then(|keys| keys.get(col)) {
                        let key_code = key.key_code;
                        self.emit_key_signals(key_code);
                    }
                }
                self.repeat_elapsed = initial + remaining;
            }
        }
        due
    }

    /// Stops repeat and long-press tracking for the held key.
    pub fn release_key(&mut self) {
        self.held_key = None;
        self.repeat_elapsed = 0;
        self.accent_popup = None;
        self.pressed_key = None;
        self.base.request_redraw();
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    fn build_qwerty_layout(&mut self) {
        self.keys = vec![
            // Row 0: q w e r t y u i o p
            vec![
                KeyDefinition { label: "q".into(), key_code: 81, width_ratio: 1.0 },
                KeyDefinition { label: "w".into(), key_code: 87, width_ratio: 1.0 },
                KeyDefinition { label: "e".into(), key_code: 69, width_ratio: 1.0 },
                KeyDefinition { label: "r".into(), key_code: 82, width_ratio: 1.0 },
                KeyDefinition { label: "t".into(), key_code: 84, width_ratio: 1.0 },
                KeyDefinition { label: "y".into(), key_code: 89, width_ratio: 1.0 },
                KeyDefinition { label: "u".into(), key_code: 85, width_ratio: 1.0 },
                KeyDefinition { label: "i".into(), key_code: 73, width_ratio: 1.0 },
                KeyDefinition { label: "o".into(), key_code: 79, width_ratio: 1.0 },
                KeyDefinition { label: "p".into(), key_code: 80, width_ratio: 1.0 },
            ],
            // Row 1: a s d f g h j k l
            vec![
                KeyDefinition { label: "a".into(), key_code: 65, width_ratio: 1.0 },
                KeyDefinition { label: "s".into(), key_code: 83, width_ratio: 1.0 },
                KeyDefinition { label: "d".into(), key_code: 68, width_ratio: 1.0 },
                KeyDefinition { label: "f".into(), key_code: 70, width_ratio: 1.0 },
                KeyDefinition { label: "g".into(), key_code: 71, width_ratio: 1.0 },
                KeyDefinition { label: "h".into(), key_code: 72, width_ratio: 1.0 },
                KeyDefinition { label: "j".into(), key_code: 74, width_ratio: 1.0 },
                KeyDefinition { label: "k".into(), key_code: 75, width_ratio: 1.0 },
                KeyDefinition { label: "l".into(), key_code: 76, width_ratio: 1.0 },
            ],
            // Row 2: Shift, z x c v b n m, Backspace
            vec![
                KeyDefinition { label: "Shift".into(), key_code: 16, width_ratio: 2.0 },
                KeyDefinition { label: "z".into(), key_code: 90, width_ratio: 1.0 },
                KeyDefinition { label: "x".into(), key_code: 88, width_ratio: 1.0 },
                KeyDefinition { label: "c".into(), key_code: 67, width_ratio: 1.0 },
                KeyDefinition { label: "v".into(), key_code: 86, width_ratio: 1.0 },
                KeyDefinition { label: "b".into(), key_code: 66, width_ratio: 1.0 },
                KeyDefinition { label: "n".into(), key_code: 78, width_ratio: 1.0 },
                KeyDefinition { label: "m".into(), key_code: 77, width_ratio: 1.0 },
                KeyDefinition { label: "Bksp".into(), key_code: 8, width_ratio: 2.0 },
            ],
            // Row 3: 123?, Space (width 4), Enter
            vec![
                KeyDefinition { label: "123".into(), key_code: 0, width_ratio: 1.2 },
                KeyDefinition { label: "Space".into(), key_code: 32, width_ratio: 4.0 },
                KeyDefinition { label: "Enter".into(), key_code: 13, width_ratio: 1.5 },
            ],
        ];
    }

    /// Builds the QWERTZ letter layout: QWERTY with the `Y` and `Z` keys exchanged.
    fn build_qwertz_layout(&mut self) {
        self.build_qwerty_layout();
        for row in &mut self.keys {
            for key in row {
                match key.label.as_str() {
                    "y" => {
                        key.label = "z".into();
                        key.key_code = 90;
                    }
                    "z" => {
                        key.label = "y".into();
                        key.key_code = 89;
                    }
                    _ => {}
                }
            }
        }
    }

    /// Builds an emoji grid. Emoji have no key code, so their text is committed through
    /// [`Keyboard::text_committed`] and the key code they carry is the no-key sentinel.
    fn build_emoji_layout(&mut self) {
        let glyphs = [
            "😀", "😄", "😉", "😍", "😎", "🤔", "😢", "😡", "👍", "👎", "👏", "🙏", "💪", "👋",
            "🤝", "✌", "❤", "💔", "✨", "🔥", "🎉", "🎁", "☀", "🌙", "🍎", "🍕", "☕", "🍺", "⚽",
            "🎵", "📷", "💡",
        ];
        self.keys = glyphs
            .chunks(8)
            .map(|chunk| {
                chunk
                    .iter()
                    .map(|glyph| KeyDefinition {
                        label: (*glyph).into(),
                        key_code: NO_KEY,
                        width_ratio: 1.0,
                    })
                    .collect()
            })
            .collect();
        self.keys.push(vec![
            KeyDefinition { label: "ABC".into(), key_code: NO_KEY, width_ratio: 1.5 },
            KeyDefinition { label: "Space".into(), key_code: 32, width_ratio: 4.0 },
            KeyDefinition { label: "Bksp".into(), key_code: 8, width_ratio: 1.5 },
            KeyDefinition { label: "Enter".into(), key_code: 13, width_ratio: 1.5 },
        ]);
    }

    fn build_numeric_layout(&mut self) {
        self.keys = vec![
            // Row 0: 1 2 3 4 5 6 7 8 9 0
            (0..=9)
                .map(|d| KeyDefinition {
                    label: format!("{d}"),
                    key_code: if d == 0 { 48 } else { 48 + d as u32 },
                    width_ratio: 1.0,
                })
                .collect(),
            // Row 1: - / : ; ( ) $ & @ "
            vec![
                KeyDefinition { label: "-".into(), key_code: 45, width_ratio: 1.0 },
                KeyDefinition { label: "/".into(), key_code: 47, width_ratio: 1.0 },
                KeyDefinition { label: ":".into(), key_code: 58, width_ratio: 1.0 },
                KeyDefinition { label: ";".into(), key_code: 59, width_ratio: 1.0 },
                KeyDefinition { label: "(".into(), key_code: 40, width_ratio: 1.0 },
                KeyDefinition { label: ")".into(), key_code: 41, width_ratio: 1.0 },
                KeyDefinition { label: "$".into(), key_code: 36, width_ratio: 1.0 },
                KeyDefinition { label: "&".into(), key_code: 38, width_ratio: 1.0 },
                KeyDefinition { label: "@".into(), key_code: 64, width_ratio: 1.0 },
                KeyDefinition { label: "\"".into(), key_code: 34, width_ratio: 1.0 },
            ],
            // Row 2: . , ? ! ' ` ~ ^
            vec![
                KeyDefinition { label: ".".into(), key_code: 46, width_ratio: 1.0 },
                KeyDefinition { label: ",".into(), key_code: 44, width_ratio: 1.0 },
                KeyDefinition { label: "?".into(), key_code: 63, width_ratio: 1.0 },
                KeyDefinition { label: "!".into(), key_code: 33, width_ratio: 1.0 },
                KeyDefinition { label: "'".into(), key_code: 39, width_ratio: 1.0 },
                KeyDefinition { label: "`".into(), key_code: 96, width_ratio: 1.0 },
                KeyDefinition { label: "~".into(), key_code: 126, width_ratio: 1.0 },
                KeyDefinition { label: "^".into(), key_code: 94, width_ratio: 1.0 },
            ],
            // Row 3: ABC?, Space, Enter
            vec![
                KeyDefinition { label: "ABC".into(), key_code: 0, width_ratio: 1.5 },
                KeyDefinition { label: "Space".into(), key_code: 32, width_ratio: 4.0 },
                KeyDefinition { label: "Enter".into(), key_code: 13, width_ratio: 1.5 },
            ],
        ];
    }

    /// Emit the appropriate signals for a key press.
    fn emit_key_signals(&self, key_code: u32) {
        self.key_pressed.emit((key_code, 0));
        match key_code {
            13 => self.enter_pressed.emit(),
            8 => self.backspace_pressed.emit(),
            32 => self.space_pressed.emit(),
            _ => {}
        }
    }

    /// Activates the key at `at`: layout switches, modifiers, dead keys, emoji and ordinary
    /// characters all go through this one entry point.
    ///
    /// A virtual keyboard's key means more than its key code: `Caps` and `123` carry the
    /// no-key sentinel, and an emoji has no code at all. Routing every activation through one
    /// function is what keeps a press, a touch and auto-repeat from each re-deciding them.
    fn activate_key(&mut self, at: (usize, usize)) {
        let Some(key) = self.keys.get(at.0).and_then(|row| row.get(at.1)) else {
            return;
        };
        let code = key.key_code;
        let label = key.label.clone();
        match label.as_str() {
            "Shift" => {
                self.toggle_shift();
                return;
            }
            "Caps" => {
                self.set_caps_lock(!self.caps_lock);
                return;
            }
            "123" => {
                self.set_layout(KeyboardLayout::Numeric);
                return;
            }
            "ABC" => {
                self.set_layout(KeyboardLayout::Qwerty);
                return;
            }
            _ => {}
        }
        if code == NO_KEY {
            // An emoji, or any other glyph the layout carries as text.
            self.text_committed.emit(label);
            self.base.clicked.emit();
            return;
        }
        // Dead keys: an accent key arms the composition instead of committing itself.
        if code == 96 || code == 94 {
            self.set_dead_key(code_char(code));
            return;
        }
        if let Some(dead) = self.dead_key.take() {
            if let Some(base) = code_char(code) {
                let composed = compose_accent(dead, base).unwrap_or(base);
                let uppercase = self.shift ^ self.caps_lock;
                let text = if uppercase {
                    composed.to_uppercase().to_string()
                } else {
                    composed.to_string()
                };
                self.text_committed.emit(text);
                self.key_pressed.emit((code, 0));
                self.base.clicked.emit();
                return;
            }
        }
        self.emit_key_signals(code);
        if (65..=90).contains(&code) || code == 32 {
            if let Some(base) = code_char(code) {
                let uppercase = self.shift ^ self.caps_lock;
                let text =
                    if uppercase { base.to_uppercase().to_string() } else { base.to_string() };
                self.text_committed.emit(text);
            }
        }
        self.base.clicked.emit();
    }

    /// Get the display label for a key, respecting shift/lowercase state.
    ///
    /// Borrowed when the label is already in the case the state asks for (the default
    /// board is lowercase, so the common frame allocates nothing); only a label that
    /// actually has to change case is rebuilt. The draw calls this once per key per
    /// frame, and the owned `String` it used to return meant one allocation per key.
    fn key_display_label<'k>(&self, key: &'k KeyDefinition) -> Cow<'k, str> {
        // Only letter keys are affected by shift and Caps Lock.
        if (65..=90).contains(&key.key_code) {
            if self.shift ^ self.caps_lock || !self.lowercase {
                if key.label.chars().all(|ch| !ch.is_lowercase()) {
                    Cow::Borrowed(key.label.as_str())
                } else {
                    Cow::Owned(key.label.to_uppercase())
                }
            } else if key.label.chars().all(|ch| !ch.is_uppercase()) {
                Cow::Borrowed(key.label.as_str())
            } else {
                Cow::Owned(key.label.to_lowercase())
            }
        } else {
            Cow::Borrowed(key.label.as_str())
        }
    }
}

impl Widget for Keyboard {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        Size::new(
            crate::widget::metrics::dimensions::KEYBOARD_DEFAULT_WIDTH,
            crate::widget::metrics::dimensions::KEYBOARD_DEFAULT_HEIGHT,
        )
    }

    /// A held key and a fading press flash both owe frames.
    fn is_animating(&self) -> bool {
        self.held_key.is_some() || self.press_progress > 0.0
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
            "key_pressed" => Some(EventSignalRef::mapped("key_pressed", &self.key_pressed, |v| {
                CapabilityValue::Tuple(alloc::vec![
                    CapabilityValue::UInt(v.0 as u64),
                    CapabilityValue::UInt(v.1 as u64),
                ])
            })),
            "enter_pressed" => Some(EventSignalRef::unit("enter_pressed", &self.enter_pressed)),
            "backspace_pressed" => {
                Some(EventSignalRef::unit("backspace_pressed", &self.backspace_pressed))
            }
            "space_pressed" => Some(EventSignalRef::unit("space_pressed", &self.space_pressed)),
            "text_committed" => {
                Some(EventSignalRef::mapped("text_committed", &self.text_committed, |v| {
                    crate::widget::capability::CapabilityValue::String(v.clone())
                }))
            }
            _ => None,
        }
    }
}

/// `Keyboard`'s property contract.
///
/// `layout` maps to and from the two lower-case tokens the old arms used
/// (`qwerty`, `numeric`); anything else is a type mismatch, not a silent
/// fallback to QWERTY.
impl WidgetProperties for Keyboard {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "layout" => {
                let token = match self.layout() {
                    KeyboardLayout::Qwerty => "qwerty",
                    KeyboardLayout::Qwertz => "qwertz",
                    KeyboardLayout::Numeric => "numeric",
                    KeyboardLayout::Emoji => "emoji",
                };
                Ok(CapabilityValue::String(token.to_string()))
            }
            "lowercase" => Ok(CapabilityValue::Bool(self.lowercase())),
            "press_animation" => Ok(CapabilityValue::Bool(self.press_animation())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "layout" => {
                let token = expect_string(value)?;
                let layout = match token.as_str() {
                    "qwerty" => KeyboardLayout::Qwerty,
                    "qwertz" => KeyboardLayout::Qwertz,
                    "numeric" => KeyboardLayout::Numeric,
                    "emoji" => KeyboardLayout::Emoji,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_layout(layout);
                Ok(())
            }
            "lowercase" => {
                self.set_lowercase(expect_bool(value)?);
                Ok(())
            }
            "press_animation" => {
                self.set_press_animation(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `KEYBOARD_PROPERTIES`.
        property_names_of!["layout", "lowercase", "press_animation", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `keyboard` publishes.
    ///
    /// `toggle_shift` is the zero-argument action — the same one the on-screen Shift key
    /// performs — so it goes through the control's own method. `set_layout` and
    /// `set_lowercase` assign state and need a payload, so they are answered through the
    /// property route; `layout`'s accepted tokens are published by `property_tokens`.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "toggle_shift" => {
                self.toggle_shift();
                Ok(())
            }
            "set_layout" | "set_lowercase" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for Keyboard {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            // Only handle MousePress (modern variant) to avoid double-trigger
            // with MouseDown (legacy variant).
            Event::MousePress { pos, .. } => {
                // The suggestion bar sits above the key grid and is hit-tested first.
                if !self.suggestions.is_empty() {
                    let rect = self.geometry();
                    if pos.y >= rect.y && pos.y < rect.y + SUGGESTION_BAR_HEIGHT as i32 {
                        let cell_width =
                            (rect.width as f32 / self.suggestions.len() as f32).max(1.0);
                        let index = (((pos.x - rect.x) as f32) / cell_width) as usize;
                        self.commit_suggestion(index);
                        return;
                    }
                }
                // An open accent popup takes the press before the key under it.
                if self.accent_popup.is_some() {
                    let variants = self
                        .accent_popup
                        .as_ref()
                        .map(|(_, _, variants)| variants.len())
                        .unwrap_or(0);
                    if let Some(popup) = self.accent_popup_rect() {
                        if popup.contains_point(*pos) && variants > 0 {
                            let cell = (popup.width as i32 / variants as i32).max(1);
                            let index =
                                (((pos.x - popup.x) / cell).max(0) as usize).min(variants - 1);
                            self.commit_accent(index);
                            return;
                        }
                    }
                    self.accent_popup = None;
                }
                let hit = self.key_at_position(*pos);
                if let Some(at) = hit {
                    // The latch is what the draw path reads, so the key the user pressed is the
                    // key that lights up. Set from the **hit test**, the same function the click
                    // itself uses, so the highlight and the action can never name different keys.
                    self.pressed_key = Some(at);
                    self.held_key = Some(at);
                    self.repeat_elapsed = 0;
                    self.press_progress = 1.0;
                    self.swipe_text.clear();
                    self.activate_key(at);
                    self.base.request_redraw();
                }
            }
            // Crossing keys while held builds a swipe word; a release commits it as text.
            Event::MouseMove { pos } => {
                if let Some(held) = self.held_key {
                    if let Some(at) = self.key_at_position(*pos) {
                        if at != held {
                            self.pressed_key = Some(at);
                            if let Some(key) = self.keys.get(at.0).and_then(|row| row.get(at.1)) {
                                if key.key_code != NO_KEY && key.key_code != 8 && key.key_code != 13
                                {
                                    self.swipe_text.push_str(&key.label);
                                }
                            }
                            self.base.request_redraw();
                        }
                    }
                }
            }
            // The press ends when the pointer goes up, wherever it went up: the key was activated
            // on the press (that is this control's contract), so the confirmation is released with
            // the pointer rather than being latched until the next press.
            Event::MouseRelease { .. } => {
                if self.swipe_text.chars().count() > 1 {
                    let text = std::mem::take(&mut self.swipe_text);
                    self.text_committed.emit(text);
                }
                self.swipe_text.clear();
                self.release_key();
            }
            Event::MouseLeave { .. } => {
                self.swipe_text.clear();
                self.release_key();
            }
            #[cfg(feature = "touch")]
            Event::TouchBegin { pos, .. } => {
                if let Some(at) = self.key_at_position(*pos) {
                    self.pressed_key = Some(at);
                    self.held_key = Some(at);
                    self.repeat_elapsed = 0;
                    self.press_progress = 1.0;
                    self.activate_key(at);
                    self.base.request_redraw();
                }
            }
            #[cfg(feature = "touch")]
            Event::TouchEnd { .. } => self.release_key(),
            Event::KeyPress { key, modifiers: _ } => {
                match *key {
                    8 => {
                        // Backspace
                        self.backspace_pressed.emit();
                        self.key_pressed.emit((8, 0));
                    }
                    13 => {
                        // Enter
                        self.enter_pressed.emit();
                        self.key_pressed.emit((13, 0));
                    }
                    32 => {
                        // Space
                        self.space_pressed.emit();
                        self.key_pressed.emit((32, 0));
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

impl Draw for Keyboard {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        // Chrome colours resolve the explicit style first, then the theme's resolved style for
        // this control, and only then fall back to a literal. `keyboard` is not in the role table,
        // so it classifies as `Surface` and its resolved background is the window fill itself —
        // which is why the board below derives its own distinct surface rather than painting the
        // window's. Every key colour used to be a literal too, so a light/dark switch left the
        // whole board unchanged and the census reported the control as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's mutex
        // is not re-entrant.
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("keyboard");
        // Read as its own lock acquisition and copied out as values, so the guard is dropped
        // before anything else touches the theme. A stripped device build has no theme
        // module, so the literals are its only rung — the same ones the `None` arm uses.
        #[cfg(device_profile)]
        let (window_fill, foreground, primary, muted) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => (
                    active.colors.background,
                    active.colors.foreground,
                    active.colors.primary,
                    active.colors.secondary,
                ),
                None => (
                    Color::rgb(240, 240, 240),
                    Color::BLACK,
                    Color::rgb(33, 150, 243),
                    Color::rgb(158, 158, 158),
                ),
            }
        };
        #[cfg(not(device_profile))]
        let (window_fill, foreground, primary, muted) = (
            Color::rgb(240, 240, 240),
            Color::BLACK,
            Color::rgb(33, 150, 243),
            Color::rgb(158, 158, 158),
        );

        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(foreground);
        // The board: one step from the window fill toward the text colour, so it is a distinct
        // element on a light theme and on a dark one. The filter is on the **resolved** value, not
        // only on the theme's: the active theme is applied to every control before it is drawn, so
        // `style.background_color` already holds `Surface`'s window fill and letting it through
        // unfiltered is exactly the invisible-board defect this guards against. A caller's own
        // colour still wins.
        let board_from_theme = window_fill.blend(&ink, 0.10);
        let board = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => board_from_theme,
        };
        context.face_with_gradient(
            rect,
            board,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );

        let row_count = self.keys.len();
        if row_count == 0 {
            return;
        }

        let total_height = rect.height as f32;
        let row_height = total_height / row_count as f32;
        let row_width = rect.width as f32;

        // Key colours are raised from the board so the grid reads on either appearance: an
        // ordinary key is one step out of the board, a modifier a further step, and a latched
        // shift carries the theme's primary so the state is visible rather than a fixed blue.
        let key_bg = board.blend(&ink, 0.10);
        let key_border = board.blend(&muted, 0.45);
        let special_bg = board.blend(&ink, 0.20);
        let shift_bg =
            if self.shift ^ self.caps_lock { primary.blend(&board, 0.45) } else { special_bg };
        // A key's label must contrast with the key it sits on, which is now a theme colour rather
        // than a fixed light grey.
        let text_color = ink;

        let font = effective_font(style);

        for (row_idx, row_keys) in self.keys.iter().enumerate() {
            let total_ratio: f32 = row_keys.iter().map(|k| k.width_ratio).sum();

            let mut cursor_x = rect.x as f32;
            for (col_idx, key) in row_keys.iter().enumerate() {
                let key_w = row_width * key.width_ratio / total_ratio;
                let key_rect = self.painted_key_rect(
                    Rect::from_f32(
                        cursor_x,
                        rect.y as f32 + row_idx as f32 * row_height,
                        key_w,
                        row_height,
                    ),
                    (row_idx, col_idx),
                );

                // Choose key background.
                //
                // `SHIFT_CODE` is local because Shift is **this board's** key, not a framework
                // convention: the board's own layout table is what gives it code 16. Enter and
                // Backspace are framework-wide and come from `event::key_codes`, so the board and a
                // dialog cannot disagree about which code is Enter.
                let kbg = if key.key_code == SHIFT_CODE {
                    shift_bg
                } else if key.key_code == key_codes::ENTER
                    || key.key_code == key_codes::BACKSPACE
                    || key.key_code == NO_KEY
                {
                    special_bg
                } else {
                    key_bg
                };

                // A key under the pointer right now steps once more out of the board, so the press
                // is visible under the finger that is covering the key. `pressed_key` names the key
                // by position because two keys in different rows can share a key code.
                let kbg = if self.pressed_key == Some((row_idx, col_idx)) {
                    // The flash decays after release, so the key eases out of its pressed look
                    // rather than snapping back on the next unrelated frame.
                    kbg.blend(&ink, 0.25 * self.press_progress.max(0.35))
                } else {
                    kbg
                };

                // Fill key background.
                context.fill_rect(key_rect, kbg);
                // Draw key border.
                context.draw_rect(key_rect, key_border);
                // The opt-in ripple: a ring expanding out of the scaled key as its flash decays.
                if self.press_animation
                    && self.pressed_key == Some((row_idx, col_idx))
                    && self.press_progress > 0.0
                {
                    let spread = ((1.0 - self.press_progress) * 4.0).round() as i32;
                    let ripple = Rect::new(
                        key_rect.x - spread,
                        key_rect.y - spread,
                        key_rect.width + (spread * 2) as u32,
                        key_rect.height + (spread * 2) as u32,
                    );
                    context.draw_rect(ripple, key_border.blend(&ink, 0.35 * self.press_progress));
                }

                // Draw label centered in the key.
                let label = self.key_display_label(key);
                if !label.is_empty() {
                    // The fit box is the key's own interior, not a synthetically enlarged one:
                    // `draw_text_fitted` insets by `TEXT_FIT_MARGIN` at each end, so a key
                    // narrower than 2*margin yields an empty interior and the label is
                    // dropped rather than squeezed out. That is the only fitting rule that
                    // satisfies P5 on every key, because the average key at the census
                    // rectangle is narrower than "Shift".
                    let inner = Rect::new(
                        key_rect.x + 2,
                        key_rect.y + 2,
                        key_rect.width.saturating_sub(4),
                        key_rect.height.saturating_sub(4),
                    );
                    // The label is centred both ways. The horizontal `… - text_w / 2` origin
                    // rounded down to a **negative** x for a label wider than its key — the
                    // 36 px Shift key drew "Shift" at x = -3 — and the vertical term only
                    // subtracted half the line height, so the glyph box sat below centre.
                    //
                    // `draw_text_fitted` fixes the horizontal half but **not** the vertical
                    // one: its contract is *fit horizontally, align horizontally*, and its
                    // origin is `bounds.y` unchanged (see `RenderContext::draw_text_fitted`).
                    // Using it for a key cap therefore still pinned every cap to the key's top
                    // edge and left the glyph box two pixels below where a centred one belongs
                    // — the `keyboard.svg` keys drew their letters at `key_rect.y + 2` with no
                    // share of the 30 px row's remaining height. `draw_text_line` is the entry
                    // point that does both halves, and it is the one the doc-comment above
                    // already promised.
                    let key_text = if key.key_code == 16 && (self.shift ^ self.caps_lock) {
                        shift_bg.contrast_color()
                    } else {
                        text_color
                    };
                    context.draw_text_line_cow(
                        inner,
                        &label,
                        font,
                        key_text,
                        HorizontalAlignment::Center,
                    );
                }

                cursor_x += key_w;
            }
        }

        // ── Suggestion bar ──
        // Painted over the first key row when the caller supplied candidates. The strip is
        // opt-in: an empty list paints exactly the board it always did.
        if !self.suggestions.is_empty() {
            let strip =
                Rect::new(rect.x, rect.y, rect.width, SUGGESTION_BAR_HEIGHT.min(rect.height));
            context.fill_rect(strip, board.blend(&ink, 0.18));
            let mut cell_x = strip.x;
            let cell_width = (strip.width as f32 / self.suggestions.len() as f32).max(1.0) as u32;
            let line = context.text_line(strip, font);
            for suggestion in &self.suggestions {
                context.draw_text_fitted_cow(
                    Rect::new(cell_x + 4, line.y, cell_width.saturating_sub(8), line.height),
                    suggestion,
                    font,
                    text_color,
                    HorizontalAlignment::Center,
                );
                cell_x += cell_width as i32;
            }
        }

        // ── Long-press accent popup ──
        // A row of candidate glyphs above the held key, each cell a target for
        // [`Keyboard::commit_accent`].
        if let Some((row, col, variants)) = &self.accent_popup {
            let total_ratio: f32 = self
                .keys
                .get(*row)
                .map(|keys| keys.iter().map(|k| k.width_ratio).sum())
                .unwrap_or(1.0);
            let mut key_x = rect.x as f32;
            if let Some(keys) = self.keys.get(*row) {
                for (index, key) in keys.iter().enumerate() {
                    let key_w = row_width * key.width_ratio / total_ratio;
                    if index == *col {
                        break;
                    }
                    key_x += key_w;
                }
            }
            let cell_width = 26i32;
            let popup_width = cell_width * variants.len() as i32;
            let popup_y = (rect.y + *row as i32 * row_height as i32 - 30)
                .max(rect.y)
                .min(rect.y + rect.height as i32 - 30);
            let popup_x = (key_x as i32).min(rect.x + rect.width as i32 - popup_width).max(rect.x);
            let popup = Rect::new(popup_x, popup_y, popup_width as u32, 28);
            context.fill_rect(popup, board.blend(&ink, 0.25));
            context.draw_rect(popup, key_border);
            let line = context.text_line(popup, font);
            for (index, variant) in variants.iter().enumerate() {
                context.draw_text_fitted_cow(
                    Rect::new(
                        popup_x + index as i32 * cell_width,
                        line.y,
                        cell_width as u32,
                        line.height,
                    ),
                    &variant.to_string(),
                    font,
                    text_color,
                    HorizontalAlignment::Center,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;

    #[test]
    fn keyboard_creation_defaults() {
        let kbd = Keyboard::new(Rect::new(0, 0, 320, 160));
        assert_eq!(kbd.layout(), KeyboardLayout::Qwerty);
        assert!(!kbd.is_shifted());
        assert!(kbd.lowercase());
        assert_eq!(kbd.keys.len(), 4);
        // Row 0 should have 10 keys (q w e r t y u i o p).
        assert_eq!(kbd.keys[0].len(), 10);
        // Row 1 should have 9 keys (a s d f g h j k l).
        assert_eq!(kbd.keys[1].len(), 9);
        // Row 2 should have 9 keys (Shift, z x c v b n m, Backspace).
        assert_eq!(kbd.keys[2].len(), 9);
        // Row 3 should have 3 keys (123, Space, Enter).
        assert_eq!(kbd.keys[3].len(), 3);
    }

    #[test]
    fn keyboard_set_layout() {
        let mut kbd = Keyboard::new(Rect::new(0, 0, 320, 160));
        assert_eq!(kbd.layout(), KeyboardLayout::Qwerty);

        kbd.set_layout(KeyboardLayout::Numeric);
        assert_eq!(kbd.layout(), KeyboardLayout::Numeric);
        // Numeric row 0: 10 digits.
        assert_eq!(kbd.keys[0].len(), 10);
        // Row 3: ABC, Space, Enter.
        assert_eq!(kbd.keys[3].len(), 3);

        // Toggle back.
        kbd.set_layout(KeyboardLayout::Qwerty);
        assert_eq!(kbd.layout(), KeyboardLayout::Qwerty);
        assert_eq!(kbd.keys[0].len(), 10);
    }

    #[test]
    fn keyboard_key_at_position() {
        let kbd = Keyboard::new(Rect::new(0, 0, 320, 160));
        // Top-left corner should hit row 0, col 0 (key 'q').
        let hit = kbd.key_at_position(Point::new(5, 5));
        assert!(hit.is_some());
        let (row, col) = hit.unwrap();
        assert_eq!(row, 0);
        assert_eq!(col, 0);
        assert_eq!(kbd.keys[row][col].key_code, 81);

        // Bottom area — row 3, space key (col 1).
        let space_hit = kbd.key_at_position(Point::new(160, 140));
        assert!(space_hit.is_some());
        let (row, col) = space_hit.unwrap();
        assert_eq!(row, 3);
        // Space key has width_ratio 4.0, total row 3 ratio = 1.5 + 4.0 + 1.5 = 7.0.
        // Space starts at ratio 1.5/7.0 = ~21% width.
        // At x=160, width=320, that's 50% width → definitely in space.
        assert_eq!(kbd.keys[row][col].key_code, 32);
    }

    #[test]
    fn keyboard_key_at_position_returns_none_outside() {
        let kbd = Keyboard::new(Rect::new(0, 0, 320, 160));
        assert!(kbd.key_at_position(Point::new(500, 500)).is_none());
        assert!(kbd.key_at_position(Point::new(-1, -1)).is_none());
    }

    #[test]
    fn keyboard_toggle_shift() {
        let mut kbd = Keyboard::new(Rect::new(0, 0, 320, 160));
        assert!(!kbd.is_shifted());

        kbd.toggle_shift();
        assert!(kbd.is_shifted());

        kbd.toggle_shift();
        assert!(!kbd.is_shifted());
    }

    #[test]
    fn keyboard_key_display_label_respects_shift() {
        let kbd = Keyboard::new(Rect::new(0, 0, 320, 160));
        // Lowercase mode, shift off → lowercase.
        let key = KeyDefinition { label: "a".into(), key_code: 65, width_ratio: 1.0 };
        let label = kbd.key_display_label(&key);
        assert_eq!(label, "a");

        // Non-letter key unaffected.
        let space_key = KeyDefinition { label: "Space".into(), key_code: 32, width_ratio: 4.0 };
        assert_eq!(kbd.key_display_label(&space_key), "Space");
    }

    #[test]
    fn keyboard_set_lowercase() {
        let mut kbd = Keyboard::new(Rect::new(0, 0, 320, 160));
        assert!(kbd.lowercase());

        kbd.set_lowercase(false);
        assert!(!kbd.lowercase());
    }

    #[test]
    fn a_host_can_install_its_own_key_grid() {
        // I-20: `KeyDefinition` was public while the grid was private and only the presets could
        // be installed, so a custom board had no entry point at all.
        let mut kbd = Keyboard::new(Rect::new(0, 0, 320, 160));
        let rows = vec![
            vec![KeyDefinition { label: "1".into(), key_code: 49, width_ratio: 1.0 }],
            vec![KeyDefinition { label: "2".into(), key_code: 50, width_ratio: 2.0 }],
        ];
        kbd.set_keys(rows);
        let hit = kbd.key_at_position(Point::new(5, 5)).expect("first row, first key");
        assert_eq!(hit, (0, 0));
        assert_eq!(kbd.keys[hit.0][hit.1].key_code, 49);
    }

    #[test]
    fn the_repeat_rhythm_is_configurable() {
        // I-21: the 450/90/500 ms values were literals inside `tick`.
        let mut kbd = Keyboard::new(Rect::new(0, 0, 320, 160));
        assert_eq!(kbd.timing(), KeyboardTiming::default());
        kbd.set_timing(KeyboardTiming {
            repeat_initial_ms: 100,
            repeat_interval_ms: 50,
            long_press_ms: 200,
        });
        assert_eq!(kbd.timing().repeat_initial_ms, 100);
        // The long-press threshold is read from the field, so a shorter hold reacts sooner.
        kbd.held_key = Some((0, 0));
        kbd.tick(60);
        assert!(kbd.accent_popup.is_none());
    }

    #[test]
    #[allow(unused_mut)]
    fn keyboard_draw_does_not_panic() {
        let mut kbd = Keyboard::new(Rect::new(0, 0, 320, 160));
        // Create a minimal render context. We mock by using the software
        // surface's RenderContext if available, or just verify no panic.
        // For testing we create a default RenderContext via the known path.
        #[cfg(feature = "software")]
        {
            use crate::core::Size;
            use crate::render::{PaintBackend, RenderContext, SoftwarePaintBackend};
            let mut backend = SoftwarePaintBackend::new(Size::new(320, 160), 1.0);
            backend.begin_frame(crate::core::Color::WHITE);
            let mut ctx = RenderContext::new(&mut backend);
            kbd.draw(&mut ctx);
        }
        #[cfg(not(feature = "software"))]
        {
            // If software rendering is not available, just ensure no panic
            // by verifying internal state is consistent.
            assert_eq!(kbd.keys.len(), 4);
        }
    }

    /// A key that is held down is painted differently from the same key at rest.
    ///
    /// # The defect this pins
    ///
    /// The keyboard had no press feedback at all: `handle_event` fired the signals and the board
    /// looked identical a moment later. `BaseWidget` records that the **control** is pressed, not
    /// which of its keys is, and a keyboard is a grid of independent targets — highlighting the whole
    /// board on any key press would be a worse lie than highlighting nothing. The latch is therefore
    /// per key, and it names the key by position because two keys in different rows can share a code.
    ///
    /// # Why the assertion compares one key's fill against its own resting fill
    ///
    /// A whole-document comparison would pass for a board where *any* pixel changed — including one
    /// this fix does not touch. The helper reads the fill of the rectangle at a named key's centre,
    /// so the assertion is about that key.
    #[test]
    #[cfg(all(device_profile, feature = "desktop"))]
    fn a_pressed_key_is_painted_as_pressed() {
        // The key fills are derived from the active theme, so pin the appearance and hold the
        // registry guard; otherwise a concurrently-running test can switch the theme between the
        // resting and pressed reads and the comparison is of two different appearances.
        let _guard = crate::style::theme_test_guard();
        crate::widget::census::install_preset_appearances();
        crate::theme::global_theme_manager().set_appearance(crate::theme::AppearanceMode::Light);
        let rect = Rect::new(0, 0, 320, 160);
        let mut kbd = Keyboard::new(rect);
        // A key in the middle of the first row: far enough from the edges that the sample is plainly
        // inside it whatever the row heights work out to.
        let row = 0usize;
        let col = 2usize;
        let centre = key_centre(&kbd, row, col).expect("the key must have a rectangle");

        let resting = key_fill(&mut kbd, rect, centre).expect("a key paints a fill");
        kbd.handle_event(&Event::MousePress { pos: centre, button: 1, modifiers: 0 });
        let pressed = key_fill(&mut kbd, rect, centre).expect("a pressed key paints a fill");
        assert_ne!(
            pressed, resting,
            "the key under the pointer must be visible as pressed; both fills were {resting}"
        );

        kbd.handle_event(&Event::MouseRelease { pos: centre, button: 1 });
        let released = key_fill(&mut kbd, rect, centre).expect("the key still paints a fill");
        assert_eq!(
            released, resting,
            "releasing must return the key to its resting fill, not latch it"
        );
    }

    /// The centre of the key at `(row, col)`, from the same geometry the draw path uses.
    #[cfg(all(device_profile, feature = "desktop"))]
    fn key_centre(kbd: &Keyboard, row: usize, col: usize) -> Option<Point> {
        let rect = kbd.geometry();
        let row_keys = kbd.keys.get(row)?;
        let total: f32 = row_keys.iter().map(|k| k.width_ratio).sum();
        let key = row_keys.get(col)?;
        let before: f32 = row_keys.iter().take(col).map(|k| k.width_ratio).sum();
        let row_height = rect.height as f32 / kbd.keys.len() as f32;
        let x = rect.x as f32 + (before + key.width_ratio / 2.0) / total * rect.width as f32;
        let y = rect.y as f32 + (row as f32 + 0.5) * row_height;
        Some(Point::new(x as i32, y as i32))
    }

    /// The fill of the **innermost** SVG `<rect>` that contains `at`.
    ///
    /// Innermost rather than first: the document opens with the board's own full-canvas rectangle,
    /// which also contains the sample point. Keeping the smallest-area match is what makes the
    /// returned fill the *key's*, which is the element the assertion is about — the same "name the
    /// element" rule the fill assertions elsewhere in the crate follow.
    ///
    /// Split on the self-closing tag rather than searching forward for it: an `element[..end]`
    /// cursor that slices at the *first* `/>` from a position can land inside the next element when
    /// a tag is emitted without the space before it, which read the wrong rectangle's attributes
    /// (measured: every candidate came back `320x160`).
    #[cfg(all(device_profile, feature = "desktop"))]
    fn key_fill(kbd: &mut Keyboard, rect: Rect, at: Point) -> Option<String> {
        let svg = crate::widget::svg::render_widget_to_svg(kbd, rect);
        let mut best: Option<(u32, String)> = None;
        for element in svg.split("/>") {
            let Some(open) = element.find("<rect ") else {
                continue;
            };
            let element = &element[open + "<rect ".len()..];
            let attr = |name: &str| -> Option<i32> {
                let key = format!("{name}=\"");
                let at = element.find(&key)? + key.len();
                let to = element[at..].find('"')? + at;
                element[at..to].parse().ok()
            };
            let (Some(x), Some(y), Some(w), Some(h)) =
                (attr("x"), attr("y"), attr("width"), attr("height"))
            else {
                continue;
            };
            if at.x < x || at.x >= x + w || at.y < y || at.y >= y + h {
                continue;
            }
            let Some(fill_at) = element.find("fill=\"") else {
                continue;
            };
            let from = fill_at + "fill=\"".len();
            let Some(to) = element[from..].find('"') else {
                continue;
            };
            let area = (w as u32).saturating_mul(h as u32);
            if best.as_ref().map(|(a, _)| area < *a).unwrap_or(true) {
                best = Some((area, element[from..from + to].to_string()));
            }
        }
        best.map(|(_, fill)| fill)
    }

    #[test]
    fn keyboard_signal_emission() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let kbd = Keyboard::new(Rect::new(0, 0, 320, 160));

        let pressed = Arc::new(AtomicBool::new(false));
        {
            let p = pressed.clone();
            kbd.key_pressed.connect(move |args| {
                let (code, _mods) = *args;
                if code == 32 {
                    p.store(true, Ordering::SeqCst);
                }
            });
        }

        // Simulate pressing the space key through the event handler.
        // We need &mut self to call handle_event, but signals are on &self.
        // So we call emit_key_signals directly.
        kbd.emit_key_signals(32);
        assert!(pressed.load(Ordering::SeqCst));
    }

    #[test]
    fn keyboard_enter_signal() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let kbd = Keyboard::new(Rect::new(0, 0, 320, 160));

        let entered = Arc::new(AtomicBool::new(false));
        let e = entered.clone();
        kbd.enter_pressed.connect(move || {
            e.store(true, Ordering::SeqCst);
        });

        kbd.emit_key_signals(13);
        assert!(entered.load(Ordering::SeqCst));
    }

    #[test]
    fn keyboard_backspace_signal() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let kbd = Keyboard::new(Rect::new(0, 0, 320, 160));

        let bs = Arc::new(AtomicBool::new(false));
        let b = bs.clone();
        kbd.backspace_pressed.connect(move || {
            b.store(true, Ordering::SeqCst);
        });

        kbd.emit_key_signals(8);
        assert!(bs.load(Ordering::SeqCst));
    }

    #[test]
    fn keyboard_shift_press_toggles_via_event() {
        let mut kbd = Keyboard::new(Rect::new(0, 0, 320, 160));
        assert!(!kbd.is_shifted());

        // Row 2, col 0 is the Shift key. Simulate a click on it.
        let shift_pos = Point::new(10, 90); // Roughly in row 2.
        kbd.handle_event(&Event::MousePress { pos: shift_pos, button: 1, modifiers: 0 });
        assert!(kbd.is_shifted());

        // Press again to toggle back.
        kbd.handle_event(&Event::MousePress { pos: shift_pos, button: 1, modifiers: 0 });
        assert!(!kbd.is_shifted());
    }

    #[test]
    fn keyboard_geometry_delegation() {
        let mut kbd = Keyboard::new(Rect::new(0, 0, 320, 160));
        assert_eq!(kbd.geometry(), Rect::new(0, 0, 320, 160));

        kbd.set_geometry(Rect::new(10, 20, 300, 150));
        assert_eq!(kbd.geometry(), Rect::new(10, 20, 300, 150));
    }

    #[test]
    fn keyboard_visibility() {
        let mut kbd = Keyboard::new(Rect::new(0, 0, 320, 160));
        assert!(kbd.is_visible());
        kbd.hide();
        assert!(!kbd.is_visible());
        kbd.show();
        assert!(kbd.is_visible());
    }

    #[test]
    fn keyboard_enabled() {
        let mut kbd = Keyboard::new(Rect::new(0, 0, 320, 160));
        assert!(kbd.is_enabled());
        kbd.set_enabled(false);
        assert!(!kbd.is_enabled());
    }

    #[test]
    fn keyboard_id_kind() {
        let kbd = Keyboard::new(Rect::new(0, 0, 320, 160));
        assert_eq!(kbd.kind(), WidgetKind::Keyboard);
        // IDs should be unique across instances.
        let kbd2 = Keyboard::new(Rect::new(0, 0, 320, 160));
        assert_ne!(kbd.id(), kbd2.id());
    }

    #[test]
    fn keyboard_size_hint() {
        let kbd = Keyboard::new(Rect::new(0, 0, 320, 160));
        let hint = kbd.size_hint();
        assert_eq!(hint.width, 320);
        assert_eq!(hint.height, 160);
    }

    /// The scale/ripple animation is opt-in and only ever moves the pressed key.
    #[test]
    fn a_press_animation_scales_the_pressed_key_only() {
        let mut kbd = Keyboard::new(Rect::new(0, 0, 320, 160));
        let key = Rect::new(10, 10, 30, 40);
        kbd.pressed_key = Some((0, 2));
        kbd.press_progress = 1.0;

        assert!(!kbd.press_animation());
        assert_eq!(kbd.painted_key_rect(key, (0, 2)), key, "off by default");

        kbd.set_press_animation(true);
        assert!(kbd.press_animation());
        assert_eq!(
            kbd.painted_key_rect(key, (0, 2)),
            Rect::new(12, 12, 26, 36),
            "a pressed key is inset while its flash is at full weight"
        );
        assert_eq!(
            kbd.painted_key_rect(key, (0, 3)),
            key,
            "a neighbouring key keeps its resting rectangle"
        );

        kbd.press_progress = 0.0;
        assert_eq!(
            kbd.painted_key_rect(key, (0, 2)),
            key,
            "the key grows back to its full rectangle once the flash has decayed"
        );
    }

    /// A held key repeats and opens its long-press accent popup.
    #[test]
    fn a_held_key_repeats_and_opens_the_accent_popup() {
        use std::sync::{Arc, Mutex};

        let mut kbd = Keyboard::new(Rect::new(0, 0, 320, 160));
        let presses = Arc::new(Mutex::new(Vec::<u32>::new()));
        let observed = presses.clone();
        kbd.key_pressed.connect(move |args| {
            let (code, _) = *args;
            observed.lock().unwrap().push(code);
        });

        // Row 1, column 0 is the `a` key (row height 40, so its centre is y = 60).
        kbd.handle_event(&Event::MousePress { pos: Point::new(17, 60), button: 1, modifiers: 0 });
        assert_eq!(presses.lock().unwrap().len(), 1, "the press itself fires once");

        assert!(kbd.tick(600), "a held key owes another frame");
        assert!(
            presses.lock().unwrap().len() >= 2,
            "the held key must auto-repeat after the initial delay"
        );
        let popup = kbd.accent_popup().expect("a held letter offers its accents");
        assert!(popup.1.contains(&'á'));
    }

    /// Crossing keys while held commits the crossed text on release.
    #[test]
    fn swiping_across_keys_commits_the_crossed_text() {
        use std::sync::{Arc, Mutex};

        let mut kbd = Keyboard::new(Rect::new(0, 0, 320, 160));
        let committed = Arc::new(Mutex::new(Vec::<String>::new()));
        let observed = committed.clone();
        kbd.text_committed.connect(move |text| observed.lock().unwrap().push(text.to_string()));

        // q (row 0, col 0), then w (col 1), then e (col 2); keys are ~32 px wide.
        kbd.handle_event(&Event::MousePress { pos: Point::new(16, 20), button: 1, modifiers: 0 });
        kbd.handle_event(&Event::MouseMove { pos: Point::new(48, 20) });
        kbd.handle_event(&Event::MouseMove { pos: Point::new(80, 20) });
        kbd.handle_event(&Event::MouseRelease { pos: Point::new(80, 20), button: 1 });

        assert_eq!(committed.lock().unwrap().last().map(String::as_str), Some("we"));
    }

    /// The suggestion strip commits the entry under the press.
    #[test]
    fn a_pressed_suggestion_is_committed() {
        use std::sync::{Arc, Mutex};

        let mut kbd = Keyboard::new(Rect::new(0, 0, 320, 160));
        kbd.set_suggestions(vec!["one".to_string(), "two".to_string()]);
        let committed = Arc::new(Mutex::new(String::new()));
        let observed = committed.clone();
        kbd.text_committed.connect(move |text| *observed.lock().unwrap() = text.to_string());

        // The second cell of the strip that overlays the first key row.
        kbd.handle_event(&Event::MousePress { pos: Point::new(200, 6), button: 1, modifiers: 0 });
        assert_eq!(*committed.lock().unwrap(), "two");
    }

    /// The emoji layout commits glyphs that carry no key code.
    #[test]
    fn the_emoji_layout_commits_its_glyph() {
        use std::sync::{Arc, Mutex};

        let mut kbd = Keyboard::new(Rect::new(0, 0, 320, 160));
        kbd.set_layout(KeyboardLayout::Emoji);
        let committed = Arc::new(Mutex::new(String::new()));
        let observed = committed.clone();
        kbd.text_committed.connect(move |text| *observed.lock().unwrap() = text.to_string());

        // Five rows over 160 px, eight keys over 320 px: the first cell is 40 x 32.
        kbd.handle_event(&Event::MousePress { pos: Point::new(20, 16), button: 1, modifiers: 0 });
        assert_eq!(*committed.lock().unwrap(), "😀");
    }
}
