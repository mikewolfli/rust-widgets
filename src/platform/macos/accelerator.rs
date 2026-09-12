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

/// `NSEventModifierFlagShift`.
pub(crate) const MOD_SHIFT: u64 = 1 << 17;
/// `NSEventModifierFlagControl`.
pub(crate) const MOD_CONTROL: u64 = 1 << 18;
/// `NSEventModifierFlagOption` (Option/Alt).
pub(crate) const MOD_OPTION: u64 = 1 << 19;
/// `NSEventModifierFlagCommand`.
pub(crate) const MOD_COMMAND: u64 = 1 << 20;

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
