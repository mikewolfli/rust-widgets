//! Win32 menu accelerators.
//!
//! Win32 has no per-menu-item shortcut API. Chords are declared in a single
//! `HACCEL` table owned by a window, and the message loop translates a key press
//! into a `WM_COMMAND` carrying the item's command id before normal dispatch.
//! That is a different model from macOS, where each `NSMenuItem` owns its own key
//! equivalent, so it gets its own module rather than being squeezed into the
//! generic menu code.
//!
//! See `docs/plans/platform_differences.md`.
//!
//! # Why this exists
//!
//! Before this module `menu_add_item` ignored its `shortcut` argument entirely
//! (the parameter was literally named `_shortcut`), so a shortcut could be shown
//! in a label but the chord did nothing. This builds a real `HACCEL` and installs
//! it on the owning window.

#![cfg(target_os = "windows")]

use std::collections::HashMap;
use std::sync::Mutex;

use winapi::shared::minwindef::WORD;
use winapi::shared::windef::HACCEL;
use winapi::um::winuser::{
    CreateAcceleratorTableW, DestroyAcceleratorTable, ACCEL, FALT, FCONTROL, FSHIFT, FVIRTKEY,
};

/// One parsed accelerator: the virtual key plus Win32 modifier flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Win32Accelerator {
    /// Virtual key code (e.g. `0x5A` for `Z`).
    pub(crate) vk: u16,
    /// `FVIRTKEY | FCONTROL | FALT | FSHIFT` bitmask.
    pub(crate) modifiers: u8,
}

impl Win32Accelerator {
    /// Converts to the `ACCEL` entry Win32 expects.
    fn to_accel(self, command_id: u32) -> ACCEL {
        ACCEL { fVirt: self.modifiers, key: self.vk as WORD, cmd: command_id as WORD }
    }
}

/// Registry of `HACCEL` tables, keyed by owning window.
///
/// A table may only be installed on the window it was built for, so the mapping
/// is explicit instead of a single global handle.
fn accel_tables() -> &'static Mutex<HashMap<u64, usize>> {
    static TABLES: std::sync::OnceLock<Mutex<HashMap<u64, usize>>> = std::sync::OnceLock::new();
    TABLES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Entries accumulated per window, keyed by window id.
///
/// `CreateAcceleratorTableW` is destructive only in the sense that rebuilding a
/// table invalidates the previous handle, so the full entry list has to be kept
/// to rebuild whenever an item is added.
fn accel_entries() -> &'static Mutex<HashMap<u64, Vec<(Win32Accelerator, u32)>>> {
    static ENTRIES: std::sync::OnceLock<Mutex<HashMap<u64, Vec<(Win32Accelerator, u32)>>>> =
        std::sync::OnceLock::new();
    ENTRIES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Records an accelerator for `window_id` and installs the rebuilt table.
///
/// Destroys the previous table for that window before installing the new one so
/// repeated calls cannot leak handles.
pub(crate) fn install_accelerator(accel: Win32Accelerator, command_id: u32, window_id: u64) {
    let Ok(mut entries) = accel_entries().lock() else {
        log::error!("[windows] install_accelerator: entry registry mutex poisoned");
        return;
    };
    let list = entries.entry(window_id).or_default();
    // Re-adding the same command id replaces it rather than duplicating the chord.
    list.retain(|(_, id)| *id != command_id);
    list.push((accel, command_id));

    let mut raw: Vec<ACCEL> = list.iter().map(|(a, id)| a.to_accel(*id)).collect();
    // SAFETY: `raw` is a live, correctly-sized slice of ACCEL entries and the
    // count passed matches its length, which is what the API requires.
    let handle = unsafe { CreateAcceleratorTableW(raw.as_mut_ptr(), raw.len() as i32) };
    if handle.is_null() {
        log::error!(
            "[windows] install_accelerator: CreateAcceleratorTableW failed \
             (GetLastError={})",
            winapi::um::errhandlingapi::GetLastError()
        );
        return;
    }
    if let Ok(mut tables) = accel_tables().lock() {
        if let Some(previous) = tables.insert(window_id, handle as usize) {
            // SAFETY: `previous` came from CreateAcceleratorTableW and is no
            // longer referenced once replaced.
            unsafe {
                DestroyAcceleratorTable(previous as HACCEL);
            }
        }
    } else {
        log::error!("[windows] install_accelerator: table registry mutex poisoned");
        // The new table could not be recorded, so release it rather than leak.
        unsafe {
            DestroyAcceleratorTable(handle);
        }
    }
}

/// Returns the `HACCEL` installed for a window, if any.
pub(crate) fn accel_table_for(window_id: u64) -> Option<HACCEL> {
    let tables = accel_tables().lock().ok()?;
    tables.get(&window_id).map(|handle| *handle as HACCEL)
}

/// Releases the accelerator table owned by a window.
pub(crate) fn release_accelerator_table(window_id: u64) {
    let table = accel_entries().lock().ok().and_then(|mut entries| entries.remove(&window_id));
    let _ = table;
    if let Ok(mut tables) = accel_tables().lock() {
        if let Some(handle) = tables.remove(&window_id) {
            // SAFETY: the handle came from CreateAcceleratorTableW and is being
            // retired with its window.
            unsafe {
                DestroyAcceleratorTable(handle as HACCEL);
            }
        }
    }
}

/// Parses displayed accelerator text (e.g. `"Ctrl+Shift+Z"`) for Win32.
///
/// Accepts the spelled-out desktop notation and the macOS glyph form, so text
/// from `Platform::format_shortcut` on any host can be passed through. Named
/// keys map onto their Win32 virtual key codes; anything unrecognised yields
/// `None` so a bad shortcut is not silently bound to the wrong key.
pub(crate) fn parse_accelerator(shortcut: Option<&str>) -> Option<Win32Accelerator> {
    let raw = shortcut.map(|s| s.trim()).filter(|s| !s.is_empty())?;
    let mut modifiers = FVIRTKEY;
    let mut vk: Option<u16> = None;
    // Glyph forms carry modifiers as standalone characters rather than tokens.
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
            // "Cmd" has no Windows meaning; it resolves to Control, which is what
            // the desktop style displays for it.
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

/// Maps an accelerator token to its Win32 virtual key code.
///
/// Letters and digits use their ASCII codes, which is exactly what Win32 expects
/// for `A`-`Z` and `0`-`9`.
fn virtual_key_for_token(token: &str) -> Option<u16> {
    use winapi::um::winuser::{
        VK_BACK, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE, VK_F1, VK_HOME, VK_INSERT, VK_LEFT,
        VK_NEXT, VK_PRIOR, VK_RETURN, VK_RIGHT, VK_SPACE, VK_TAB, VK_UP,
    };
    let named = match token {
        "backspace" | "del" | "delete" => Some(VK_BACK as u16),
        "esc" | "escape" => Some(VK_ESCAPE as u16),
        "enter" | "return" => Some(VK_RETURN as u16),
        "space" => Some(VK_SPACE as u16),
        "tab" => Some(VK_TAB as u16),
        "insert" | "ins" => Some(VK_INSERT as u16),
        "home" => Some(VK_HOME as u16),
        "end" => Some(VK_END as u16),
        "pageup" | "pgup" => Some(VK_PRIOR as u16),
        "pagedown" | "pgdn" => Some(VK_NEXT as u16),
        "left" => Some(VK_LEFT as u16),
        "right" => Some(VK_RIGHT as u16),
        "up" => Some(VK_UP as u16),
        "down" => Some(VK_DOWN as u16),
        _ => None,
    };
    if named.is_some() {
        return named;
    }
    // `F1`-`F24` are contiguous. The range is capped at F12 because `VK_F13` and
    // beyond collide with other virtual key codes (VK_F16 shares the value of
    // VK_DELETE), so binding them by arithmetic would silently mistranslate.
    if let Some(rest) = token.strip_prefix('f') {
        if let Ok(number) = rest.parse::<u16>() {
            if (1..=12).contains(&number) {
                return Some(VK_F1 as u16 + (number - 1));
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
    // Punctuation maps to its OEM virtual key code, which for the US layout is
    // the ASCII code's value for `/`, `.`, `,`, `-` and `;`.
    match first {
        '/' | '.' | ',' | '-' | '=' | ';' | '\'' | '[' | ']' | '\\' => Some(first as u16),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winapi::um::winuser::{
        VK_BACK, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE, VK_F1, VK_F12, VK_HOME, VK_LEFT, VK_NEXT,
        VK_PRIOR, VK_RETURN, VK_RIGHT, VK_SPACE, VK_TAB, VK_UP,
    };

    /// Absent or blank text must not produce an accelerator.
    #[test]
    fn blank_input_produces_none() {
        assert!(parse_accelerator(None).is_none());
        assert!(parse_accelerator(Some("")).is_none());
        assert!(parse_accelerator(Some("   ")).is_none());
    }

    /// The canonical desktop spelling binds Control + letter.
    #[test]
    fn ctrl_letter_binds_control() {
        let accel = parse_accelerator(Some("Ctrl+S")).expect("Ctrl+S must bind");
        assert_eq!(accel.vk, 'S' as u16);
        assert_eq!(accel.modifiers, FVIRTKEY | FCONTROL);
    }

    /// `Primary` is the portable spelling and maps to Control on Windows.
    #[test]
    fn primary_spelling_binds_control() {
        let primary = parse_accelerator(Some("Primary+S")).expect("Primary+S must bind");
        assert_eq!(primary, parse_accelerator(Some("Ctrl+S")).expect("Ctrl+S must bind"));
    }

    /// The macOS glyph form must survive the round trip.
    #[test]
    fn mac_glyph_form_binds_control() {
        let accel = parse_accelerator(Some("⌘S")).expect("glyph form must bind");
        assert_eq!(accel.vk, 'S' as u16);
        assert_eq!(accel.modifiers, FVIRTKEY | FCONTROL);
    }

    /// Shift+Control is distinct from Control alone.
    #[test]
    fn shift_control_is_distinct() {
        let accel = parse_accelerator(Some("Ctrl+Shift+Z")).expect("chord must bind");
        assert_eq!(accel.vk, 'Z' as u16);
        assert_eq!(accel.modifiers, FVIRTKEY | FCONTROL | FSHIFT);
        assert_ne!(accel, parse_accelerator(Some("Ctrl+Z")).expect("Ctrl+Z must bind"));
    }

    /// Alt and Shift are their own flags.
    #[test]
    fn alt_binds_alt_flag() {
        let accel = parse_accelerator(Some("Alt+F4")).expect("Alt+F4 must bind");
        assert_eq!(accel.vk, VK_F1 + 3);
        assert_eq!(accel.modifiers, FVIRTKEY | FALT);
    }

    /// Named keys map to their virtual key codes, not to letters.
    #[test]
    fn navigation_keys_map_to_virtual_codes() {
        let cases = [
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
            ("Ctrl+Backspace", VK_BACK),
            ("Ctrl+Delete", VK_DELETE),
        ];
        for (text, expected) in cases {
            let accel = parse_accelerator(Some(text)).unwrap_or_else(|| panic!("{text} must bind"));
            assert_eq!(accel.vk, expected as u16, "{text} mapped to the wrong key");
        }
    }

    /// Function keys use the contiguous VK_F1..VK_F12 range.
    #[test]
    fn function_keys_map_into_range() {
        assert_eq!(parse_accelerator(Some("F1")).expect("F1 must bind").vk, VK_F1 as u16);
        assert_eq!(parse_accelerator(Some("F12")).expect("F12 must bind").vk, VK_F12 as u16);
        // Beyond F12 the VK range collides with other keys, so it is refused.
        assert!(parse_accelerator(Some("F13")).is_none());
    }

    /// An unknown key must be refused rather than bound to something arbitrary.
    #[test]
    fn unknown_key_is_refused() {
        assert!(parse_accelerator(Some("Ctrl+NotAKey")).is_none());
        assert!(parse_accelerator(Some("Ctrl+")).is_none());
    }
}
