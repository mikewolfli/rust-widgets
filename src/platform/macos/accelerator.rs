// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Shared macOS accelerator parsing for the objc2 and cocoa backends.
//!
//! # Why this is not in either backend
//!
//! Both macOS backends need to turn displayed accelerator text (`⌘Z`,
//! `Ctrl+Shift+Z`) into a Cocoa key equivalent plus an `NSEventModifierFlag`
//! mask, and the two must agree — a shortcut that works in one backend has to
//! work in the other.
//!
//! It lives here rather than in `macos/types.rs` because that module is gated on
//! `cocoa-legacy`, while the objc2 backend is selected by the `macos` feature.
//! Hosting the parser in the cocoa module made `--features macos` alone fail to
//! compile (`cannot find types in macos`), and made every profile that does not
//! pull in `cocoa-legacy` unusable with the default backend.
//!
//! [`crate::platform::macos::macos_bridge`] is the existing ungated home for
//! macOS pieces shared across backends, so this sits alongside it.

#![cfg(target_os = "macos")]
// Only the `cocoa-legacy` backend renders self-drawn frames, and only it keeps a
// window registry; the objc2 backend still needs `parse_shortcut` for menu
// accelerators. In a build with neither consumer (for example `--features
// os-auto`, which selects no backend at all) these items are genuinely unused, so
// the dead-code lint is silenced rather than papered over with `_`-prefixed names.
#![allow(dead_code)]

// The accelerator parser builds owned key strings, so it needs the alloc bridge
// rather than `std`: under `--no-default-features --features mini` the `std`
// prelude is not the source of `String`/`ToString`, and the bare names used to
// fail with 33 errors (`cannot find type String in this scope`).
use crate::compat::{String, ToString};

/// `NSEventModifierFlagShift`.
pub(crate) const MOD_SHIFT: u64 = 1 << 17;
/// `NSEventModifierFlagControl`.
pub(crate) const MOD_CONTROL: u64 = 1 << 18;
/// `NSEventModifierFlagOption` (Option/Alt).
pub(crate) const MOD_OPTION: u64 = 1 << 19;
/// `NSEventModifierFlagCommand`.
pub(crate) const MOD_COMMAND: u64 = 1 << 20;

/// Normalises an AppKit virtual key code to the framework's key-code convention.
///
/// # Why the raw `keyCode` cannot be forwarded
///
/// AppKit's `keyCode` is a **hardware** code: Return is 36, Escape 53, Backspace 51,
/// Left 123. The widget layer matches the framework's logical codes (Return 13,
/// Escape 27, Backspace 8, Left 37 — see [`crate::event::key_codes`]), so forwarding
/// the hardware code meant Enter was read as Home, Escape was unhandled, and the
/// arrow keys moved the caret the wrong way. The Linux/GTK backend had the mirror
/// defect with GDK keyvals (Return `0xFF0D`), so both desktop backends now translate
/// at the platform boundary instead of teaching every control two conventions.
///
/// `characters` is the event's text with modifiers ignored, used for chords
/// (`⌘A`, `⌘C`): the widget layer matches upper-case ASCII (`65` for `A`), which is
/// what [`crate::event::key_codes::ascii_key_code`] produces. A key with no printable
/// form falls back to its hardware code, which is the honest "no framework name"
/// answer for a layout-specific key.
#[cfg(widgets_unstripped)]
pub(crate) fn logical_key(appkit_code: u16, characters: Option<&str>) -> u32 {
    use crate::event::key_codes as keys;

    let named = match appkit_code {
        36 | 76 => keys::ENTER,
        53 => keys::ESCAPE,
        51 => keys::BACKSPACE,
        117 => keys::DELETE,
        123 => keys::LEFT,
        124 => keys::RIGHT,
        125 => keys::DOWN,
        126 => keys::UP,
        115 => keys::HOME,
        119 => keys::END,
        116 => keys::PAGE_UP,
        121 => keys::PAGE_DOWN,
        48 => keys::TAB,
        122 => keys::F1,
        120 => keys::F2,
        99 => keys::F3,
        118 => keys::F4,
        96 => keys::F5,
        97 => keys::F6,
        98 => keys::F7,
        100 => keys::F8,
        101 => keys::F9,
        109 => keys::F10,
        103 => keys::F11,
        111 => keys::F12,
        _ => 0,
    };
    if named != 0 {
        return named;
    }
    if let Some(ch) = characters.and_then(|text| text.chars().next()) {
        if let Some(code) = keys::ascii_key_code(ch) {
            return code;
        }
    }
    appkit_code as u32
}

/// Converts AppKit's `NSEventModifierFlag` bits into the widget-layer bitfield.
///
/// Split out of [`translate_key`] so a **pointer** event can carry the same
/// modifier state a key event does. A `Shift`/`Ctrl` press is what selects a range or
/// toggles one row in an extended-selection view, and the widget layer can only honour
/// that if the press it receives says which modifiers were held.
///
/// Lives here rather than in `macos/types.rs` for the same reason [`parse_shortcut`]
/// does: both macOS backends convert modifier flags, and `macos/types.rs` is gated on
/// `cocoa-legacy` while the objc2 backend is selected by the `macos` feature.
///
/// Command **must** reach the widget as bit 3: it is the macOS primary accelerator (`⌘Z`,
/// `⌘S`, ...). Dropping it made every Command chord arrive as a bare key press, which is why
/// application shortcuts never fired on macOS.
#[cfg(widgets_unstripped)]
pub(crate) fn map_modifiers(appkit_flags: u64) -> u32 {
    const WIDGET_SHIFT: u32 = 1;
    const WIDGET_CONTROL: u32 = 2;
    const WIDGET_ALT: u32 = 4;
    const WIDGET_META: u32 = 8;
    let mut modifiers = 0u32;
    if appkit_flags & MOD_SHIFT != 0 {
        modifiers |= WIDGET_SHIFT;
    }
    if appkit_flags & MOD_CONTROL != 0 {
        modifiers |= WIDGET_CONTROL;
    }
    if appkit_flags & MOD_OPTION != 0 {
        modifiers |= WIDGET_ALT;
    }
    if appkit_flags & MOD_COMMAND != 0 {
        modifiers |= WIDGET_META;
    }
    modifiers
}

/// Everything an AppKit key event translates to, spelled once for both macOS backends.
///
/// # Why both key codes are carried
///
/// The two backends read an `NSEvent` through different crates (the legacy one through `objc`
/// message sends, objc2 through its typed `NSEvent`), so only the conversion can be shared, not
/// the reading. What they did **not** share was the conversion itself: `printable_characters`,
/// `logical_key` selection and the modifier mapping were copied into both canvases, and the
/// copies drifted (the cocoa canvas normalised Enter/arrows to the framework's codes while the
/// objc2 canvas forwarded AppKit's hardware codes).
///
/// This value keeps both spellings so each backend can keep sending exactly what it sent before:
///
/// * [`Self::appkit_key_code`] — AppKit's `keyCode`, unchanged (the objc2 canvas's spelling).
/// * [`Self::logical_key`] — the framework's normalised code (Enter 13, Left 37, ...), which is
///   the cocoa canvas's spelling.
/// * [`Self::logical_key_with_ignored_modifiers`] — the same normalisation applied to the
///   event's `charactersIgnoringModifiers`, so a chord reports the letter it was pressed with
///   (`⌘A` → 65) instead of the character it would type.
///
/// The text fields are already filtered: a key that typed only a control character carries
/// `None`, so neither caller has to remember that rule.
pub(crate) struct TranslatedKey {
    /// AppKit's hardware key code, verbatim.
    pub(crate) appkit_key_code: u16,
    /// Framework key code with no character spelling (named keys normalised).
    pub(crate) logical_key: u32,
    /// Framework key code derived from `charactersIgnoringModifiers`.
    pub(crate) logical_key_with_ignored_modifiers: u32,
    /// Widget-layer modifier bits (see [`map_modifiers`]).
    pub(crate) modifiers: u32,
    /// The event's printable text, control characters removed.
    pub(crate) text: Option<String>,
    /// The event's printable text with modifiers ignored, control characters removed.
    pub(crate) ignored_modifiers_text: Option<String>,
}

/// Translates the AppKit values a key `NSEvent` exposes into widget-layer facts.
///
/// Both macOS canvases call this with the four values they read from their own `NSEvent`
/// spelling, so the modifier mapping, the printable-character filter and the named-key
/// normalisation have one implementation. Gated with [`logical_key`], its only
/// `widgets_unstripped` consumer.
#[cfg(widgets_unstripped)]
pub(crate) fn translate_key(
    appkit_key_code: u16,
    appkit_flags: u64,
    characters: Option<&str>,
    characters_ignoring_modifiers: Option<&str>,
) -> TranslatedKey {
    let text = printable_text(characters);
    let ignored_modifiers_text = printable_text(characters_ignoring_modifiers);
    TranslatedKey {
        appkit_key_code,
        logical_key: logical_key(appkit_key_code, None),
        logical_key_with_ignored_modifiers: logical_key(
            appkit_key_code,
            ignored_modifiers_text.as_deref(),
        ),
        modifiers: map_modifiers(appkit_flags),
        text,
        ignored_modifiers_text,
    }
}

/// Removes control characters from AppKit's character string.
///
/// Enter, Tab and Escape are meaningful as `KeyPress` and are mapped there; treating them as
/// text as well would insert a newline or a tab into a field on every command key. Both macOS
/// canvases applied this rule separately before this helper existed.
#[cfg(widgets_unstripped)]
fn printable_text(characters: Option<&str>) -> Option<String> {
    let printable: String = characters?.chars().filter(|ch| !ch.is_control()).collect();
    if printable.is_empty() {
        None
    } else {
        Some(printable)
    }
}

/// Parses a **displayed** accelerator (e.g. `"⌘Z"`, `"Ctrl+Shift+Z"`, `"F1"`)
/// into a Cocoa key equivalent string plus its `NSEventModifierFlag` mask.
///
/// Both notations are accepted: the glyph form macOS renders (`⌘⇧Z`) and the
/// spelled-out form used by Windows/Linux callers (`Ctrl+Shift+Z`). Accepting
/// both keeps `menu_add_item` usable no matter which platform produced the text.
///
/// The returned key string is empty when `shortcut` is absent or unparseable, and
/// callers treat an empty key as "no accelerator".
pub(crate) fn parse_shortcut(shortcut: Option<&str>) -> (String, u64) {
    // Parse textual accelerator into Cocoa key + modifier mask.
    let Some(raw) = shortcut.map(|s| s.trim()).filter(|s| !s.is_empty()) else {
        return (String::new(), 0);
    };
    let mut modifiers: u64 = 0;
    let mut key = String::new();
    // The glyph forms are single characters rather than `+`-separated tokens, so
    // scan for them before splitting on the separator.
    for ch in raw.chars() {
        match ch {
            '⌘' => modifiers |= MOD_COMMAND,
            '⌃' | '^' => modifiers |= MOD_CONTROL,
            '⌥' => modifiers |= MOD_OPTION,
            '⇧' => modifiers |= MOD_SHIFT,
            _ => {}
        }
    }
    for part in raw.split('+') {
        let token = part.trim().to_lowercase();
        // A token may still carry a leading glyph, so strip glyphs first.
        let token = token.trim_start_matches(['⌘', '⌃', '^', '⌥', '⇧']).to_string();
        match token.as_str() {
            // "Primary" is the portable spelling: on macOS it is Command.
            "primary" | "cmdorctrl" | "cmd" | "command" | "meta" => modifiers |= MOD_COMMAND,
            "ctrl" | "control" => modifiers |= MOD_CONTROL,
            "alt" | "option" => modifiers |= MOD_OPTION,
            "shift" => modifiers |= MOD_SHIFT,
            // Named keys whose glyph/menu spelling has to become an AppKit key
            // equivalent rather than a literal word.
            "enter" | "return" => key = "\r".to_string(),
            "tab" => key = "\t".to_string(),
            "space" => key = " ".to_string(),
            "delete" | "del" | "backspace" => key = "\u{8}".to_string(),
            "esc" | "escape" => key = "\u{1b}".to_string(),
            "left" => key = "\u{f702}".to_string(),
            "right" => key = "\u{f703}".to_string(),
            "up" => key = "\u{f700}".to_string(),
            "down" => key = "\u{f701}".to_string(),
            "pageup" | "pgup" => key = "\u{f72c}".to_string(),
            "pagedown" | "pgdn" => key = "\u{f72d}".to_string(),
            "home" => key = "\u{f729}".to_string(),
            "end" => key = "\u{f72b}".to_string(),
            "⌘" | "⌃" | "⌥" | "⇧" | "↩" | "⇥" | "⎋" | "⌫" | "←" | "→" | "↑" | "↓" | "⇞" | "⇟"
            | "↖" | "↘" => {
                // Glyphs were already folded into the mask above; a lone glyph
                // token carries no key of its own.
            }
            _ if !token.is_empty() => {
                key = token;
            }
            _ => { /* Other keys are not relevant */ }
        }
    }
    // The glyph form (`⌘↩`) arrives as a single token with no separator, so the
    // loop above leaves `key` holding mixed glyphs. Reduce it to the key part.
    if key.is_empty() {
        let stripped: String =
            raw.chars().filter(|ch| !matches!(ch, '⌘' | '⌃' | '^' | '⌥' | '⇧')).collect();
        let stripped = stripped.trim().to_string();
        if !stripped.is_empty() {
            key = match stripped.as_str() {
                "↩" => "\r".to_string(),
                "⇥" => "\t".to_string(),
                "⎋" => "\u{1b}".to_string(),
                "⌫" => "\u{8}".to_string(),
                "←" => "\u{f702}".to_string(),
                "→" => "\u{f703}".to_string(),
                "↑" => "\u{f700}".to_string(),
                "↓" => "\u{f701}".to_string(),
                "⇞" => "\u{f72c}".to_string(),
                "⇟" => "\u{f72d}".to_string(),
                "↖" => "\u{f729}".to_string(),
                "↘" => "\u{f72b}".to_string(),
                other => other.to_string(),
            };
        }
    }
    if !key.is_empty() && modifiers == 0 {
        modifiers = MOD_COMMAND;
    }
    (key, modifiers)
}

#[cfg(test)]
mod parse_shortcut_tests {
    use super::{parse_shortcut, MOD_COMMAND, MOD_CONTROL, MOD_OPTION, MOD_SHIFT};
    // `String` is not in the prelude of the frugal profiles this module also
    // compiles under, so the test module imports it through the alloc bridge too.
    use crate::compat::String;

    /// No shortcut, or an empty one, must yield "no accelerator".
    #[test]
    fn absent_shortcut_produces_no_key() {
        assert_eq!(parse_shortcut(None), (String::new(), 0));
        assert_eq!(parse_shortcut(Some("")), (String::new(), 0));
        assert_eq!(parse_shortcut(Some("   ")), (String::new(), 0));
    }

    /// The spelled-out form (what Windows/Linux callers pass) maps to Command.
    #[test]
    fn primary_spelling_maps_to_command() {
        let (key, mask) = parse_shortcut(Some("Primary+S"));
        assert_eq!(key, "s");
        assert_eq!(mask, MOD_COMMAND);
    }

    /// The glyph form macOS itself renders must round-trip back to Command.
    #[test]
    fn glyph_form_maps_to_command() {
        let (key, mask) = parse_shortcut(Some("⌘S"));
        assert_eq!(key, "s");
        assert_eq!(mask, MOD_COMMAND);
    }

    /// Ctrl and Shift keep their own flags rather than collapsing into Command.
    #[test]
    fn control_and_shift_are_distinct_from_command() {
        let (key, mask) = parse_shortcut(Some("Ctrl+Shift+Z"));
        assert_eq!(key, "z");
        assert_eq!(mask, MOD_CONTROL | MOD_SHIFT);
    }

    /// A glyph-only combination must still recover the key that follows it.
    #[test]
    fn glyph_combo_recovers_named_key() {
        // Shift+Command+Z, written the way a macOS menu displays it.
        let (key, mask) = parse_shortcut(Some("⇧⌘Z"));
        assert_eq!(key, "z");
        assert_eq!(mask, MOD_SHIFT | MOD_COMMAND);
    }

    /// Option/Alt is its own flag.
    #[test]
    fn option_spelling_maps_to_option() {
        let (key, mask) = parse_shortcut(Some("Alt+F4"));
        assert_eq!(key, "f4");
        assert_eq!(mask, MOD_OPTION);
    }

    /// A bare key with no modifier defaults to Command (macOS menu convention).
    #[test]
    fn bare_key_defaults_to_command() {
        let (key, mask) = parse_shortcut(Some("F1"));
        assert_eq!(key, "f1");
        assert_eq!(mask, MOD_COMMAND);
    }
}

/// AppKit's hardware key codes must arrive at the widget layer as the framework's
/// logical codes; otherwise Enter is read as Home and Escape is ignored.
#[cfg(all(test, widgets_unstripped))]
mod logical_key_tests {
    use super::logical_key;
    use crate::event::key_codes as keys;

    #[test]
    fn editing_and_navigation_keys_are_normalised() {
        // AppKit: Return 36, Escape 53, Backspace 51, ForwardDelete 117.
        assert_eq!(logical_key(36, None), keys::ENTER);
        assert_eq!(logical_key(76, None), keys::ENTER, "keypad Enter is Enter");
        assert_eq!(logical_key(53, None), keys::ESCAPE);
        assert_eq!(logical_key(51, None), keys::BACKSPACE);
        assert_eq!(logical_key(117, None), keys::DELETE);
        // AppKit: Left 123, Right 124, Down 125, Up 126 (note the inverted pair).
        assert_eq!(logical_key(123, None), keys::LEFT);
        assert_eq!(logical_key(124, None), keys::RIGHT);
        assert_eq!(logical_key(125, None), keys::DOWN);
        assert_eq!(logical_key(126, None), keys::UP);
        // AppKit: Home 115, End 119, PageUp 116, PageDown 121.
        assert_eq!(logical_key(115, None), keys::HOME);
        assert_eq!(logical_key(119, None), keys::END);
        assert_eq!(logical_key(116, None), keys::PAGE_UP);
        assert_eq!(logical_key(121, None), keys::PAGE_DOWN);
    }

    #[test]
    fn function_keys_map_to_the_conventional_range() {
        assert_eq!(logical_key(122, None), keys::F1);
        assert_eq!(logical_key(120, None), keys::F2);
        assert_eq!(logical_key(99, None), keys::F3);
        assert_eq!(logical_key(118, None), keys::F4);
        assert_eq!(logical_key(96, None), keys::F5);
        assert_eq!(logical_key(97, None), keys::F6);
        assert_eq!(logical_key(98, None), keys::F7);
        assert_eq!(logical_key(100, None), keys::F8);
        assert_eq!(logical_key(101, None), keys::F9);
        assert_eq!(logical_key(109, None), keys::F10);
        assert_eq!(logical_key(103, None), keys::F11);
        assert_eq!(logical_key(111, None), keys::F12);
    }

    #[test]
    fn a_chord_reports_the_letters_upper_case_code() {
        // "a" is AppKit key code 0; `charactersIgnoringModifiers` gives the letter.
        assert_eq!(logical_key(0, Some("a")), 65, "⌘A reaches LineEdit as 65");
        assert_eq!(logical_key(8, Some("c")), 67, "⌘C reaches LineEdit as 67");
        assert_eq!(logical_key(7, Some("x")), 88, "⌘X reaches LineEdit as 88");
        assert_eq!(logical_key(6, Some("z")), 90, "⌘Z reaches LineEdit as 90");
        assert_eq!(logical_key(16, Some("y")), 89, "⌘Y reaches LineEdit as 89");
    }

    #[test]
    fn an_unnamed_key_keeps_its_hardware_code() {
        assert_eq!(logical_key(10, None), 10, "§ has no framework name");
    }
}

/// The shared translation is the one both macOS canvases call, so these pin the parts each
/// backend consumes rather than one backend's whole event policy.
#[cfg(all(test, widgets_unstripped))]
mod translate_key_tests {
    use super::{translate_key, MOD_COMMAND, MOD_SHIFT};
    use crate::event::key_codes as keys;

    #[test]
    fn a_plain_key_carries_both_spellings_and_its_text() {
        let key = translate_key(0, 0, Some("a"), Some("a"));
        assert_eq!(key.appkit_key_code, 0, "objc2 forwards the hardware code");
        // With no named-key match and no character spelling, the framework code falls back to
        // the hardware code; a plain letter travels as `TextInput` anyway.
        assert_eq!(key.logical_key, 0);
        assert_eq!(key.logical_key_with_ignored_modifiers, 65, "⌘A's letter");
        assert_eq!(key.modifiers, 0);
        assert_eq!(key.text.as_deref(), Some("a"));
        assert_eq!(key.ignored_modifiers_text.as_deref(), Some("a"));
    }

    #[test]
    fn a_chord_reports_the_letter_not_the_typed_character() {
        let key = translate_key(0, MOD_COMMAND | MOD_SHIFT, Some("A"), Some("a"));
        assert_eq!(key.modifiers, 8 | 1, "Command is meta, Shift is bit 0");
        assert_eq!(key.logical_key_with_ignored_modifiers, 65);
        assert_eq!(key.text.as_deref(), Some("A"));
    }

    #[test]
    fn control_characters_are_not_text() {
        let enter = translate_key(36, 0, Some("\r"), Some("\r"));
        assert_eq!(enter.text, None, "Enter is a KeyPress, not an inserted newline");
        assert_eq!(enter.logical_key, keys::ENTER);

        let left = translate_key(123, 0, None, None);
        assert_eq!(left.text, None);
        assert_eq!(left.logical_key, keys::LEFT);
        assert_eq!(left.appkit_key_code, 123, "objc2 still sees AppKit's code");
    }
}
