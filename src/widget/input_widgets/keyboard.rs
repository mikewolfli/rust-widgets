// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Keyboard widget — on-screen virtual keyboard (BLUE13 R2.6).
//!
//! Displays a grid of keys (QWERTY layout by default). Each key generates
//! a [`Signal1<(u32, u32)>`] with the key code and modifiers on press.
//! Special keys (Enter, Backspace, Space) also emit dedicated signals.

use crate::compat::{format, vec, String, ToString, Vec};
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Layout variants for the on-screen keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardLayout {
    /// Standard QWERTY letter layout (default).
    Qwerty,
    /// Numeric/symbol layout.
    Numeric,
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
    /// Signal emitted with `(key_code, modifiers)` when any key is pressed.
    pub key_pressed: Signal1<(u32, u32)>,
    /// Signal emitted when Enter (key code 13) is pressed.
    pub enter_pressed: GenericSignal,
    /// Signal emitted when Backspace (key code 8) is pressed.
    pub backspace_pressed: GenericSignal,
    /// Signal emitted when Space (key code 32) is pressed.
    pub space_pressed: GenericSignal,
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
            key_pressed: Signal1::new(),
            enter_pressed: GenericSignal::new(),
            backspace_pressed: GenericSignal::new(),
            space_pressed: GenericSignal::new(),
        };
        kbd.build_qwerty_layout();
        kbd
    }

    /// Set the keyboard layout and rebuild the key grid.
    pub fn set_layout(&mut self, layout: KeyboardLayout) {
        self.layout = layout;
        match layout {
            KeyboardLayout::Qwerty => self.build_qwerty_layout(),
            KeyboardLayout::Numeric => self.build_numeric_layout(),
        }
        self.base.request_redraw();
    }

    /// Return the current layout variant.
    pub fn layout(&self) -> KeyboardLayout {
        self.layout
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
        self.lowercase = enabled;
        self.base.request_redraw();
    }

    /// Return whether lowercase mode is enabled.
    pub fn lowercase(&self) -> bool {
        self.lowercase
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
                KeyDefinition { label: "Shift".into(), key_code: 16, width_ratio: 1.5 },
                KeyDefinition { label: "z".into(), key_code: 90, width_ratio: 1.0 },
                KeyDefinition { label: "x".into(), key_code: 88, width_ratio: 1.0 },
                KeyDefinition { label: "c".into(), key_code: 67, width_ratio: 1.0 },
                KeyDefinition { label: "v".into(), key_code: 86, width_ratio: 1.0 },
                KeyDefinition { label: "b".into(), key_code: 66, width_ratio: 1.0 },
                KeyDefinition { label: "n".into(), key_code: 78, width_ratio: 1.0 },
                KeyDefinition { label: "m".into(), key_code: 77, width_ratio: 1.0 },
                KeyDefinition { label: "Bksp".into(), key_code: 8, width_ratio: 1.5 },
            ],
            // Row 3: 123?, Space (width 4), Enter
            vec![
                KeyDefinition { label: "123".into(), key_code: 0, width_ratio: 1.5 },
                KeyDefinition { label: "Space".into(), key_code: 32, width_ratio: 4.0 },
                KeyDefinition { label: "Enter".into(), key_code: 13, width_ratio: 1.5 },
            ],
        ];
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

    /// Get the display label for a key, respecting shift/lowercase state.
    fn key_display_label(&self, key: &KeyDefinition) -> String {
        // Only letter keys are affected by shift.
        if key.key_code >= 65 && key.key_code <= 90 {
            if self.shift || !self.lowercase {
                key.label.to_uppercase()
            } else {
                key.label.to_lowercase()
            }
        } else {
            key.label.clone()
        }
    }
}

impl Widget for Keyboard {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        Size::new(320, 160)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
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
                    KeyboardLayout::Numeric => "numeric",
                };
                Ok(CapabilityValue::String(token.to_string()))
            }
            "lowercase" => Ok(CapabilityValue::Bool(self.lowercase())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "layout" => {
                let token = expect_string(value)?;
                let layout = match token.as_str() {
                    "qwerty" => KeyboardLayout::Qwerty,
                    "numeric" => KeyboardLayout::Numeric,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_layout(layout);
                Ok(())
            }
            "lowercase" => {
                self.set_lowercase(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `KEYBOARD_PROPERTIES`.
        property_names_of!["layout", "lowercase", BASE_PROPERTY_NAMES]
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
            Event::MousePress { pos, button: _ } => {
                let key_code = self.key_at_position(*pos).and_then(|(r, c)| {
                    self.keys.get(r).and_then(|row| row.get(c)).map(|k| k.key_code)
                });
                if let Some(code) = key_code {
                    if code == 16 {
                        self.toggle_shift();
                    } else {
                        self.emit_key_signals(code);
                    }
                    self.base.clicked.emit();
                }
            }
            #[cfg(feature = "touch")]
            Event::TouchBegin { pos, .. } => {
                let key_code = self.key_at_position(*pos).and_then(|(r, c)| {
                    self.keys.get(r).and_then(|row| row.get(c)).map(|k| k.key_code)
                });
                if let Some(code) = key_code {
                    if code == 16 {
                        self.toggle_shift();
                    } else {
                        self.emit_key_signals(code);
                    }
                    self.base.clicked.emit();
                }
            }
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
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("keyboard");
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
        context.fill_rect(rect, board);

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
        let shift_bg = if self.shift { primary.blend(&board, 0.45) } else { special_bg };
        // A key's label must contrast with the key it sits on, which is now a theme colour rather
        // than a fixed light grey.
        let text_color = ink;

        let default_font = Font::default();

        for (row_idx, row_keys) in self.keys.iter().enumerate() {
            let total_ratio: f32 = row_keys.iter().map(|k| k.width_ratio).sum();

            let mut cursor_x = rect.x as f32;
            for key in row_keys {
                let key_w = row_width * key.width_ratio / total_ratio;
                let key_rect = Rect::from_f32(
                    cursor_x,
                    rect.y as f32 + row_idx as f32 * row_height,
                    key_w,
                    row_height,
                );

                // Choose key background.
                let kbg = if key.key_code == 16 {
                    shift_bg
                } else if key.key_code == 13 || key.key_code == 8 || key.key_code == 0 {
                    special_bg
                } else {
                    key_bg
                };

                // Fill key background.
                context.fill_rect(key_rect, kbg);
                // Draw key border.
                context.draw_rect(key_rect, key_border);

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
                    // `draw_text_fitted` with `Center` derives the origin from the fitted
                    // string *inside* the key, so neither can leave it.
                    let key_text = if key.key_code == 16 && self.shift {
                        shift_bg.contrast_color()
                    } else {
                        text_color
                    };
                    context.draw_text_fitted(
                        inner,
                        &label,
                        &default_font,
                        key_text,
                        HorizontalAlignment::Center,
                    );
                }

                cursor_x += key_w;
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
        kbd.handle_event(&Event::MousePress { pos: shift_pos, button: 1 });
        assert!(kbd.is_shifted());

        // Press again to toggle back.
        kbd.handle_event(&Event::MousePress { pos: shift_pos, button: 1 });
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
}
