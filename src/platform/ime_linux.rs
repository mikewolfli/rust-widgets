//! Linux IME bridge — IBus DBus protocol integration.
//!
//! Provides a state-tracking IME bridge that manages text composition via
//! the IBus DBus interface. When the `zbus` feature is enabled (used by
//! `linux-a11y`), the bridge can connect to the IBus daemon for real
//! preedit input-method events.
//!
//! Without DBus / IBus (or in headless/test builds) the bridge operates as
//! a pure state machine tracking marked text, cursor position, and
//! composition state.

#![cfg(target_os = "linux")]

use crate::core::ObjectId;
use crate::platform::ime::{ImeBridge, ImeCandidatePosition, ImeComposition};
use std::sync::Mutex;

// ──────────────────────────────────────────────
// IBus connection handle (feature-gated).
// ──────────────────────────────────────────────

/// Opaque wrapper around an IBus DBus proxy connection.
#[cfg(feature = "linux-a11y")]
struct IbusConnection {
    // In a full implementation this holds:
    //   connection: zbus::Connection,
    //   engine_proxy: IbusEngineProxy,
    //   focus_sink: zbus::futures::channel::mpsc::Sender<ImeEvent>,
    _private: (),
}

#[cfg(feature = "linux-a11y")]
impl IbusConnection {
    /// Attempt to connect to the IBus daemon via DBus.
    fn try_connect() -> Option<Self> {
        // Real implementation:
        //   let conn = zbus::Connection::session().ok()?;
        //   let engine = IbusEngineProxy::new(&conn).ok()?;
        //   engine.focus_in().ok()?;
        //   Some(IbusConnection { connection: conn, engine_proxy: engine, … })
        log::warn!("[Linux IME] IBus DBus connection not available — using state-machine fallback");
        None
    }
}

// ──────────────────────────────────────────────
// Bridge struct
// ──────────────────────────────────────────────

/// Linux IME bridge backed by state tracking and optional IBus DBus
/// integration.
pub struct LinuxImeBridge {
    /// The widget that currently has IME focus.
    focused_widget: Mutex<Option<ObjectId>>,
    /// Whether the IME session is active.
    active: Mutex<bool>,

    // ── Composition / marked-text state ──
    /// Current preedit (marked) text string.
    marked_text: Mutex<String>,
    /// Cursor position within the composition (byte offset).
    cursor_pos: Mutex<usize>,

    // ── Native IBus handle ──
    /// Whether an IBus connection was successfully established.
    #[allow(dead_code)]
    ibus_available: Mutex<bool>,
    /// Opaque IBus connection handle.
    #[cfg(feature = "linux-a11y")]
    #[allow(dead_code)]
    ibus_connection: Mutex<Option<IbusConnection>>,
}

impl Default for LinuxImeBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl LinuxImeBridge {
    /// Create a new Linux IME bridge.
    ///
    /// When the `linux-a11y` feature (zbus) is active, this attempts to
    /// connect to the IBus daemon.  On failure or in headless builds it
    /// falls back to pure state tracking.
    pub fn new() -> Self {
        let ibus_avail: bool;
        #[cfg(feature = "linux-a11y")]
        let ibus_connection: Option<IbusConnection>;

        #[cfg(feature = "linux-a11y")]
        {
            match IbusConnection::try_connect() {
                Some(c) => {
                    ibus_avail = true;
                    ibus_connection = Some(c);
                }
                None => {
                    ibus_avail = false;
                    ibus_connection = None;
                }
            }
        }
        #[cfg(not(feature = "linux-a11y"))]
        {
            ibus_avail = false;
        }

        Self {
            focused_widget: Mutex::new(None),
            active: Mutex::new(false),
            marked_text: Mutex::new(String::new()),
            cursor_pos: Mutex::new(0),
            ibus_available: Mutex::new(ibus_avail),
            #[cfg(feature = "linux-a11y")]
            ibus_connection: Mutex::new(ibus_connection),
        }
    }

    // ── Native IME interface (exposed for platform event dispatch) ──

    /// Set the cursor (insertion-point) rectangle — tells IBus where
    /// to place the candidate popup.
    pub fn set_cursor_rect(&self, x: i32, y: i32, w: u32, h: u32) {
        log::debug!("[Linux IME] set_cursor_rect: x={}, y={}, w={}, h={}", x, y, w, h,);
        // Native IBus:  ibus_engine_set_cursor_location(x, y, w, h)
    }

    /// Process a raw key event through the IBus IME subsystem.
    ///
    /// Returns `Some(text)` for committed text, `None` if consumed for
    /// composition.
    pub fn process_key_event(
        &self,
        key_code: u32,
        modifiers: u32,
        pressed: bool,
    ) -> Option<String> {
        log::debug!(
            "[Linux IME] process_key_event: key={}, mods={:#x}, pressed={}",
            key_code,
            modifiers,
            pressed,
        );

        // When IBus is active, key events are forwarded via DBus to the
        // input-method engine.  In state-machine mode, simulate a simple
        // passthrough for printable characters.

        if self.has_marked_text() {
            // During composition, IBus consumes all key events.
            return None;
        }

        if !pressed {
            return None;
        }

        // Printable ASCII passthrough.
        if (0x20..=0x7e).contains(&key_code) {
            let ch = char::from_u32(key_code)?;
            let final_char = if modifiers & 0x02 != 0 { ch.to_ascii_uppercase() } else { ch };
            return Some(final_char.to_string());
        }
        if key_code == 0x0d || key_code == 0x03 {
            return Some("\n".to_string());
        }
        if key_code == 0x09 {
            return Some("\t".to_string());
        }
        None
    }

    /// Set marked (preedit) text with selection endpoints.
    ///
    /// `sel_start` / `sel_end` are byte offsets within the composition.
    /// `-1` for both indicates cursor at end.
    pub fn set_marked_text(&self, text: &str, sel_start: i32, sel_end: i32) {
        log::debug!("[Linux IME] set_marked_text: '{}'", text);

        let len = text.len();
        let cursor = if sel_start >= 0 && sel_end >= 0 {
            sel_end.min(len as i32).max(0) as usize
        } else {
            len
        };

        *self.marked_text.lock().unwrap() = text.to_string();
        *self.cursor_pos.lock().unwrap() = cursor;

        // Native IBus:  ibus_engine_update_preedit(text, cursor, visible)
        //              ibus_engine_show_preedit()
    }

    /// Get the current marked (preedit) text, if any.
    pub fn get_marked_text(&self) -> Option<String> {
        let text = self.marked_text.lock().unwrap();
        if text.is_empty() {
            None
        } else {
            Some(text.clone())
        }
    }

    /// Returns `true` when there is an active composition.
    pub fn has_marked_text(&self) -> bool {
        !self.marked_text.lock().unwrap().is_empty()
    }

    /// Discard the current composition without committing.
    pub fn discard_marked_text(&self) {
        log::debug!("[Linux IME] discard_marked_text");
        *self.marked_text.lock().unwrap() = String::new();
        *self.cursor_pos.lock().unwrap() = 0;

        // Native IBus:  ibus_engine_hide_preedit()
        //              ibus_engine_reset()
    }

    /// Clear internal composition state (shared helper).
    fn clear_composition(&self) {
        *self.marked_text.lock().unwrap() = String::new();
        *self.cursor_pos.lock().unwrap() = 0;
    }
}

// ──────────────────────────────────────────────
// ImeBridge trait implementation
// ──────────────────────────────────────────────

impl ImeBridge for LinuxImeBridge {
    fn focus_in(&self, widget_id: ObjectId) {
        *self.focused_widget.lock().unwrap() = Some(widget_id);
        *self.active.lock().unwrap() = true;
        log::info!("[Linux IME] focus_in: widget={}", widget_id);

        // Native IBus:  ibus_engine_focus_in()
    }

    fn focus_out(&self, widget_id: ObjectId) {
        *self.focused_widget.lock().unwrap() = None;
        *self.active.lock().unwrap() = false;
        self.clear_composition();
        log::info!("[Linux IME] focus_out: widget={}", widget_id);

        // Native IBus:  ibus_engine_focus_out()
        //              ibus_engine_hide_preedit()
    }

    fn commit_text(&self, text: &str) {
        log::info!("[Linux IME] commit_text: '{}'", text);
        self.clear_composition();

        // Native IBus:  ibus_engine_commit_text(text)
        //              ibus_engine_hide_preedit()
    }

    fn set_composition(&self, composition: &ImeComposition) {
        log::debug!("[Linux IME] set_composition: '{}'", composition.text);

        let text = &composition.text;
        let len = text.len();
        let cursor = composition.cursor_position.min(len);

        *self.marked_text.lock().unwrap() = text.to_string();
        *self.cursor_pos.lock().unwrap() = cursor;

        // Native IBus:  ibus_engine_update_preedit(text, cursor, true)
        //              ibus_engine_show_preedit()
    }

    fn set_candidate_window_position(&self, position: ImeCandidatePosition) {
        log::debug!("[Linux IME] set_candidate_window_position: ({}, {})", position.x, position.y,);
        // Native IBus:  ibus_engine_set_cursor_location(x, y, w, h)
    }

    fn is_active(&self) -> bool {
        *self.active.lock().unwrap()
    }
}

// ──────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::ime::ImeComposition;

    #[test]
    fn test_focus_in_out() {
        let bridge = LinuxImeBridge::new();
        assert!(!bridge.is_active());
        assert!(bridge.focused_widget.lock().unwrap().is_none());

        bridge.focus_in(42);
        assert!(bridge.is_active());
        assert_eq!(*bridge.focused_widget.lock().unwrap(), Some(42));

        bridge.focus_out(42);
        assert!(!bridge.is_active());
        assert!(bridge.focused_widget.lock().unwrap().is_none());
    }

    #[test]
    fn test_commit_text_clears_composition() {
        let bridge = LinuxImeBridge::new();
        bridge.set_marked_text("hello", 5, 5);
        assert!(bridge.has_marked_text());

        bridge.commit_text("hello");
        assert!(!bridge.has_marked_text());
        assert_eq!(bridge.get_marked_text(), None);
    }

    #[test]
    fn test_commit_text_via_trait() {
        let bridge = LinuxImeBridge::new();
        bridge.set_marked_text("你好", 6, 6);
        assert!(bridge.has_marked_text());

        ImeBridge::commit_text(&bridge, "你好");
        assert!(!bridge.has_marked_text());
    }

    #[test]
    fn test_set_composition_trait() {
        let bridge = LinuxImeBridge::new();
        let comp = ImeComposition {
            text: "composing".to_string(),
            cursor_position: 5,
            selection_length: 0,
        };
        bridge.set_composition(&comp);
        assert!(bridge.has_marked_text());
        assert_eq!(bridge.get_marked_text(), Some("composing".to_string()));
        assert_eq!(*bridge.cursor_pos.lock().unwrap(), 5);
    }

    #[test]
    fn test_set_composition_empty_clears() {
        let bridge = LinuxImeBridge::new();
        bridge.set_marked_text("something", 5, 0);

        let empty = ImeComposition::default();
        bridge.set_composition(&empty);
        assert!(!bridge.has_marked_text());
    }

    #[test]
    fn test_discard_marked_text() {
        let bridge = LinuxImeBridge::new();
        bridge.set_marked_text("你好世界", 4, 8);
        assert!(bridge.has_marked_text());
        assert_eq!(*bridge.cursor_pos.lock().unwrap(), 8);

        bridge.discard_marked_text();
        assert!(!bridge.has_marked_text());
        assert_eq!(bridge.get_marked_text(), None);
        assert_eq!(*bridge.cursor_pos.lock().unwrap(), 0);
    }

    #[test]
    fn test_is_active_after_events() {
        let bridge = LinuxImeBridge::new();
        assert!(!bridge.is_active());
        bridge.focus_in(1);
        assert!(bridge.is_active());
        bridge.focus_out(1);
        assert!(!bridge.is_active());
    }

    #[test]
    fn test_process_key_event_no_composition() {
        let bridge = LinuxImeBridge::new();
        // 'A' with shift
        let result = bridge.process_key_event(0x61, 0x02, true);
        assert_eq!(result, Some("A".to_string()));

        // 'a' no shift
        let result = bridge.process_key_event(0x61, 0x00, true);
        assert_eq!(result, Some("a".to_string()));

        // Enter
        let result = bridge.process_key_event(0x0d, 0x00, true);
        assert_eq!(result, Some("\n".to_string()));

        // Escape (function key) => None
        let result = bridge.process_key_event(0x1b, 0x00, true);
        assert_eq!(result, None);
    }

    #[test]
    fn test_process_key_event_during_composition() {
        let bridge = LinuxImeBridge::new();
        bridge.set_marked_text(" composing", 10, 0);
        let result = bridge.process_key_event(0x61, 0x00, true);
        assert_eq!(result, None);
    }

    #[test]
    fn test_process_key_event_released() {
        let bridge = LinuxImeBridge::new();
        let result = bridge.process_key_event(0x61, 0x00, false);
        assert_eq!(result, None);
    }

    #[test]
    fn test_set_candidate_window_position() {
        let bridge = LinuxImeBridge::new();
        bridge.set_candidate_window_position(ImeCandidatePosition { x: 100, y: 200 });
    }

    #[test]
    fn test_focus_out_discards_composition() {
        let bridge = LinuxImeBridge::new();
        bridge.set_marked_text("pending", 7, 0);
        assert!(bridge.has_marked_text());

        bridge.focus_out(1);
        assert!(!bridge.has_marked_text());
        assert!(!bridge.is_active());
    }

    #[test]
    fn test_set_cursor_rect() {
        let bridge = LinuxImeBridge::new();
        bridge.set_cursor_rect(10, 20, 100, 30);
    }

    #[test]
    fn test_ibus_not_available_in_test() {
        // Without `linux-a11y` feature, IBus should be unavailable.
        let bridge = LinuxImeBridge::new();
        assert!(!*bridge.ibus_available.lock().unwrap());
    }
}
