//! Host-side verification of the Win32 accelerator parser.
//!
//! The parser lives in `src/platform/windows/accel.rs` and is only compiled on
//! Windows. Its *logic* (tokenising, modifier mapping, virtual-key lookup) is
//! platform independent, but `winapi` does not expose the `um` modules on
//! non-Windows targets, so the VK constants are declared here with their
//! documented Win32 values and the parser body is reproduced verbatim.
//!
//! Keep this file in step with `accel.rs`; it exists so the parsing rules can be
//! tested on any host rather than only on a Windows machine.

#![allow(dead_code)]

// Real Win32 virtual key codes (winuser.h).
const VK_BACK: u16 = 0x08;
const VK_TAB: u16 = 0x09;
const VK_RETURN: u16 = 0x0D;
const VK_ESCAPE: u16 = 0x1B;
const VK_SPACE: u16 = 0x20;
const VK_PRIOR: u16 = 0x21;
const VK_NEXT: u16 = 0x22;
const VK_END: u16 = 0x23;
const VK_HOME: u16 = 0x24;
const VK_LEFT: u16 = 0x25;
const VK_UP: u16 = 0x26;
const VK_RIGHT: u16 = 0x27;
const VK_DOWN: u16 = 0x28;
const VK_INSERT: u16 = 0x2D;
const VK_DELETE: u16 = 0x2E;
const VK_F1: u16 = 0x70;
const VK_F12: u16 = 0x7B;

// Accelerator modifier flags (winuser.h).
const FVIRTKEY: u8 = 0x01;
const FSHIFT: u8 = 0x04;
const FCONTROL: u8 = 0x08;
const FALT: u8 = 0x10;

/// Mirrors `accel::Win32Accelerator`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Win32Accelerator {
    pub vk: u16,
    pub modifiers: u8,
}

/// Mirrors `accel::virtual_key_for_token`.
fn virtual_key_for_token(token: &str) -> Option<u16> {
    let named = match token {
        "backspace" | "back" => Some(VK_BACK),
        "delete" | "del" => Some(VK_DELETE),
        "esc" | "escape" => Some(VK_ESCAPE),
        "enter" | "return" => Some(VK_RETURN),
        "space" => Some(VK_SPACE),
        "tab" => Some(VK_TAB),
        "insert" | "ins" => Some(VK_INSERT),
        "home" => Some(VK_HOME),
        "end" => Some(VK_END),
        "pageup" | "pgup" => Some(VK_PRIOR),
        "pagedown" | "pgdn" => Some(VK_NEXT),
        "left" => Some(VK_LEFT),
        "right" => Some(VK_RIGHT),
        "up" => Some(VK_UP),
        "down" => Some(VK_DOWN),
        _ => None,
    };
    if named.is_some() {
        return named;
    }
    if let Some(rest) = token.strip_prefix('f') {
        if let Ok(number) = rest.parse::<u16>() {
            if (1..=12).contains(&number) {
                return Some(VK_F1 + (number - 1));
            }
        }
    }
    let mut chars = token.chars();
    let first = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    if first.is_ascii_alphabetic() {
        return Some(first.to_ascii_uppercase() as u16);
    }
    if first.is_ascii_digit() {
        return Some(first as u16);
    }
    match first {
        '/' | '.' | ',' | '-' | '=' | ';' | '\'' | '[' | ']' | '\\' => Some(first as u16),
        _ => None,
    }
}

/// Mirrors `accel::parse_accelerator`.
fn parse_accelerator(shortcut: Option<&str>) -> Option<Win32Accelerator> {
    let raw = shortcut.map(|s| s.trim()).filter(|s| !s.is_empty())?;
    let mut modifiers = FVIRTKEY;
    let mut vk: Option<u16> = None;
    for ch in raw.chars() {
        match ch {
            '⌘' | '⌃' | '^' => modifiers |= FCONTROL,
            '⌥' => modifiers |= FALT,
            '⇧' => modifiers |= FSHIFT,
            _ => {}
        }
    }
    for part in raw.split('+') {
        let token = part.trim().to_lowercase();
        let token = token.trim_start_matches(['⌘', '⌃', '^', '⌥', '⇧']).to_string();
        match token.as_str() {
            "primary" | "cmdorctrl" | "cmd" | "command" | "ctrl" | "control" => {
                modifiers |= FCONTROL
            }
            "alt" | "option" => modifiers |= FALT,
            "shift" => modifiers |= FSHIFT,
            "⌘" | "⌃" | "⌥" | "⇧" => {}
            other => {
                if let Some(code) = virtual_key_for_token(other) {
                    vk = Some(code);
                }
            }
        }
    }
    vk.map(|vk| Win32Accelerator { vk, modifiers })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_input_produces_none() {
        assert!(parse_accelerator(None).is_none());
        assert!(parse_accelerator(Some("")).is_none());
        assert!(parse_accelerator(Some("   ")).is_none());
    }

    #[test]
    fn ctrl_letter_binds_control() {
        let accel = parse_accelerator(Some("Ctrl+S")).expect("Ctrl+S must bind");
        assert_eq!(accel.vk, 'S' as u16);
        assert_eq!(accel.modifiers, FVIRTKEY | FCONTROL);
    }

    #[test]
    fn primary_and_cmd_spellings_bind_control() {
        let ctrl = parse_accelerator(Some("Ctrl+S")).expect("Ctrl+S must bind");
        assert_eq!(parse_accelerator(Some("Primary+S")).expect("Primary+S must bind"), ctrl);
        assert_eq!(parse_accelerator(Some("Cmd+S")).expect("Cmd+S must bind"), ctrl);
        assert_eq!(parse_accelerator(Some("⌘S")).expect("glyph must bind"), ctrl);
    }

    #[test]
    fn shift_control_is_distinct() {
        let accel = parse_accelerator(Some("Ctrl+Shift+Z")).expect("chord must bind");
        assert_eq!(accel.vk, 'Z' as u16);
        assert_eq!(accel.modifiers, FVIRTKEY | FCONTROL | FSHIFT);
        assert_ne!(accel, parse_accelerator(Some("Ctrl+Z")).expect("Ctrl+Z must bind"));
    }

    #[test]
    fn glyph_combo_binds() {
        let accel = parse_accelerator(Some("⇧⌘Z")).expect("glyph combo must bind");
        assert_eq!(accel.vk, 'Z' as u16);
        assert_eq!(accel.modifiers, FVIRTKEY | FCONTROL | FSHIFT);
    }

    #[test]
    fn alt_binds_alt_flag() {
        let accel = parse_accelerator(Some("Alt+F4")).expect("Alt+F4 must bind");
        assert_eq!(accel.vk, VK_F1 + 3);
        assert_eq!(accel.modifiers, FVIRTKEY | FALT);
    }

    #[test]
    fn navigation_keys_map_to_virtual_codes() {
        let cases: &[(&str, u16)] = &[
            ("Ctrl+Home", VK_HOME),
            ("Ctrl+End", VK_END),
            ("Ctrl+PageUp", VK_PRIOR),
            ("Ctrl+PageDown", VK_NEXT),
            ("Ctrl+Left", VK_LEFT),
            ("Ctrl+Right", VK_RIGHT),
            ("Ctrl+Up", VK_UP),
            ("Ctrl+Down", VK_DOWN),
            ("Ctrl+Enter", VK_RETURN),
            ("Ctrl+Tab", VK_TAB),
            ("Ctrl+Escape", VK_ESCAPE),
            ("Ctrl+Space", VK_SPACE),
            ("Ctrl+Insert", VK_INSERT),
        ];
        for (text, expected) in cases {
            let accel = parse_accelerator(Some(text)).unwrap_or_else(|| panic!("{text} must bind"));
            assert_eq!(accel.vk, *expected, "{text} mapped to the wrong key");
        }
    }

    #[test]
    fn backspace_and_delete_are_distinct() {
        let backspace = parse_accelerator(Some("Ctrl+Backspace")).expect("Backspace must bind");
        let delete = parse_accelerator(Some("Ctrl+Delete")).expect("Delete must bind");
        assert_eq!(backspace.vk, VK_BACK);
        assert_eq!(delete.vk, VK_DELETE);
        assert_ne!(backspace.vk, delete.vk);
    }

    #[test]
    fn function_keys_map_into_range() {
        assert_eq!(parse_accelerator(Some("F1")).expect("F1").vk, VK_F1);
        assert_eq!(parse_accelerator(Some("F12")).expect("F12").vk, VK_F12);
        assert!(parse_accelerator(Some("F13")).is_none());
    }

    #[test]
    fn unknown_key_is_refused() {
        assert!(parse_accelerator(Some("Ctrl+NotAKey")).is_none());
        assert!(parse_accelerator(Some("Ctrl+")).is_none());
    }
}
