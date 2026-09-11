//! Linux IME bridge — in-process composition state with optional IBus probe.
//!
//! The bridge tracks marked text, cursor position, focus and composition
//! state, and (when the `linux-a11y`/zbus feature is active) probes the IBus
//! daemon over DBus to detect availability.
//!
//! **Protocol status**: the full IBus *engine* protocol (CreateInputContext,
//! ProcessKeyEvent round-trips, preedit streaming from the daemon) is not
//! wired here; that requires the active engine context, which this bridge
//! does not own. Commit/preedit operations are therefore modeled in-process
//! and published through the [`ImeBridge`](crate::platform::ime::ImeBridge) state, which the compositor or
//! host toolkit layer consumes.

#![cfg(target_os = "linux")]

use crate::core::ObjectId;
use crate::platform::ime::{ImeBridge, ImeCandidatePosition, ImeComposition};
use std::sync::Mutex;

// ──────────────────────────────────────────────
// IBus connection handle (feature-gated).
// ──────────────────────────────────────────────

/// Real IBus DBus proxy connection plus its input context.
///
/// Connects to the session bus, resolves the IBus service, and creates an
/// `org.freedesktop.IBus.InputContext` on the `org.freedesktop.IBus` interface.
/// The input context is the object the engine protocol operates on
/// (`FocusIn`/`FocusOut`/`SetCursorLocation`/`ProcessKeyEvent`), so holding it
/// is what turns the bridge from a state model into a real IME client.
#[cfg(feature = "linux-a11y")]
struct IbusConnection {
    /// Active zbus session connection.
    connection: zbus::Connection,
    /// Object path of the input context owned by this connection.
    ///
    /// IBus destroys the context when the owning bus connection drops, which is
    /// why the connection is held here for the bridge's lifetime.
    input_context_path: String,
}

/// IBus input-context interface name.
#[cfg(feature = "linux-a11y")]
const IBUS_IC_INTERFACE: &str = "org.freedesktop.IBus.InputContext";

#[cfg(feature = "linux-a11y")]
impl IbusConnection {
    /// Attempt to connect to the IBus daemon and create an input context.
    ///
    /// Uses the `org.freedesktop.IBus` well-known name on the session bus.
    /// If the IBus daemon is not running, the session bus is unavailable, or
    /// `CreateInputContext` fails, this returns `None` and the bridge falls
    /// back to state-machine mode.
    fn try_connect() -> Option<Self> {
        // Connect to the DBus session bus.
        let conn = pollster::block_on(zbus::Connection::session()).ok()?;

        let (owner, context_path): (String, String) = pollster::block_on(async {
            // Resolve the IBus daemon on the session bus.
            let dbus = zbus::Proxy::new(
                &conn,
                "org.freedesktop.DBus",
                "/org/freedesktop/DBus",
                "org.freedesktop.DBus",
            )
            .await
            .ok()?;
            // `GetNameOwner` returns a plain string (`s`), not a variant (`v`);
            // asking for `OwnedValue` fails the signature check and makes the
            // whole probe silently report "IBus unavailable".
            let owner: String = dbus.call("GetNameOwner", &("org.freedesktop.IBus",)).await.ok()?;

            // Create an input context owned by this connection.
            let ibus = zbus::Proxy::new(
                &conn,
                "org.freedesktop.IBus",
                "/org/freedesktop/IBus",
                "org.freedesktop.IBus",
            )
            .await
            .ok()?;
            let path: zbus::zvariant::OwnedObjectPath =
                ibus.call("CreateInputContext", &("rust_widgets",)).await.ok()?;
            Some((owner, path.to_string()))
        })?;

        log::info!("[Linux IME] IBus connected (owner: {owner}); input context: {context_path}");

        Some(IbusConnection { connection: conn, input_context_path: context_path })
    }

    /// Invoke a void method on this connection's input context with no arguments.
    fn call_input_context_no_args(&self, method: &str) -> bool {
        let conn = self.connection.clone();
        let path = self.input_context_path.clone();
        let method = method.to_string();
        pollster::block_on(async move {
            let proxy =
                zbus::Proxy::new(&conn, "org.freedesktop.IBus", path.as_str(), IBUS_IC_INTERFACE)
                    .await
                    .ok()?;
            match proxy.call_method(method.as_str(), &()).await {
                Ok(_) => Some(()),
                Err(e) => {
                    log::warn!("[Linux IME] IBus {method} call failed: {e}");
                    None
                }
            }
        })
        .is_some()
    }

    /// Invoke a void method on this connection's input context with one `u` argument.
    fn call_input_context_u(&self, method: &str, arg: u32) -> bool {
        let conn = self.connection.clone();
        let path = self.input_context_path.clone();
        let method = method.to_string();
        pollster::block_on(async move {
            let proxy =
                zbus::Proxy::new(&conn, "org.freedesktop.IBus", path.as_str(), IBUS_IC_INTERFACE)
                    .await
                    .ok()?;
            match proxy.call_method(method.as_str(), &(arg,)).await {
                Ok(_) => Some(()),
                Err(e) => {
                    log::warn!("[Linux IME] IBus {method} call failed: {e}");
                    None
                }
            }
        })
        .is_some()
    }

    /// Invoke a void method on this connection's input context with four `i` args.
    fn call_input_context_iiii(&self, method: &str, a: i32, b: i32, c: i32, d: i32) -> bool {
        let conn = self.connection.clone();
        let path = self.input_context_path.clone();
        let method = method.to_string();
        pollster::block_on(async move {
            let proxy =
                zbus::Proxy::new(&conn, "org.freedesktop.IBus", path.as_str(), IBUS_IC_INTERFACE)
                    .await
                    .ok()?;
            match proxy.call_method(method.as_str(), &(a, b, c, d)).await {
                Ok(_) => Some(()),
                Err(e) => {
                    log::warn!("[Linux IME] IBus {method} call failed: {e}");
                    None
                }
            }
        })
        .is_some()
    }

    /// Set input capabilities advertised to the engine.
    ///
    /// Signature: `SetCapabilities(u)`. `IBUS_CAP_PREEDIT_TEXT | IBUS_CAP_FOCUS`
    /// (1 | 4 = 5) tells the engine we can display preedit text and track focus,
    /// which is what the bridge models.
    fn set_capabilities(&self, flags: u32) -> bool {
        self.call_input_context_u("SetCapabilities", flags)
    }

    /// Notify the engine that focus entered the input context.
    ///
    /// Signature: `FocusIn()` — takes no arguments.
    fn focus_in(&self) -> bool {
        self.call_input_context_no_args("FocusIn")
    }

    /// Notify the engine that focus left the input context.
    ///
    /// Signature: `FocusOut()` — takes no arguments.
    fn focus_out(&self) -> bool {
        self.call_input_context_no_args("FocusOut")
    }

    /// Tell the engine where the insertion point is (candidate window anchor).
    ///
    /// Signature: `SetCursorLocation(iiii)` in screen coordinates.
    fn set_cursor_location(&self, x: i32, y: i32, w: u32, h: u32) -> bool {
        self.call_input_context_iiii("SetCursorLocation", x, y, w as i32, h as i32)
    }

    /// Forward a key event to the engine.
    ///
    /// Signature: `ProcessKeyEvent(uuu)` returning `b`. `keyval`/`keycode`
    /// follow the X11 conventions IBus expects; `state` is the X11 modifier
    /// mask. Returns `Some(true)` when the engine consumed the event (it is
    /// composing and will deliver preedit/commit as signals), `Some(false)`
    /// when it declined and the key should be handled locally, and `None` when
    /// the call itself failed (engine/connection gone).
    fn process_key_event(&self, keyval: u32, keycode: u32, state: u32) -> Option<bool> {
        let conn = self.connection.clone();
        let path = self.input_context_path.clone();
        pollster::block_on(async move {
            let proxy =
                zbus::Proxy::new(&conn, "org.freedesktop.IBus", path.as_str(), IBUS_IC_INTERFACE)
                    .await
                    .ok()?;
            match proxy.call_method("ProcessKeyEvent", &(keyval, keycode, state)).await {
                Ok(reply) => reply.body().deserialize::<bool>().ok(),
                Err(e) => {
                    log::warn!("[Linux IME] IBus ProcessKeyEvent call failed: {e}");
                    None
                }
            }
        })
    }
}

/// `IBUS_CAP_PREEDIT_TEXT | IBUS_CAP_FOCUS`.
#[cfg(feature = "linux-a11y")]
const IBUS_CAP_PREEDIT_TEXT_FOCUS: u32 = 1 | 4;

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
    /// Last insertion-point rectangle in screen coordinates.
    cursor_rect: Mutex<(i32, i32, u32, u32)>,
    /// Last requested candidate window position.
    candidate_position: Mutex<ImeCandidatePosition>,

    // ── Native IBus handle ──
    /// Whether an IBus connection was successfully established (probe result).
    ibus_available: Mutex<bool>,
    /// Opaque IBus session-bus connection plus its input context.
    ///
    /// Held open for the bridge's lifetime: IBus destroys the input context as
    /// soon as the owning bus connection drops, so this is also the keep-alive.
    #[cfg(feature = "linux-a11y")]
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
            cursor_rect: Mutex::new((0, 0, 0, 0)),
            candidate_position: Mutex::new(ImeCandidatePosition { x: 0, y: 0 }),
            ibus_available: Mutex::new(ibus_avail),
            #[cfg(feature = "linux-a11y")]
            ibus_connection: Mutex::new(ibus_connection),
        }
    }

    /// Whether the IBus daemon was reachable when the bridge was created.
    pub fn is_ibus_available(&self) -> bool {
        *self.ibus_available.lock().unwrap()
    }

    // ── Native IME interface (exposed for platform event dispatch) ──

    /// Set the cursor (insertion-point) rectangle — tells IBus where
    /// to place the candidate popup.
    ///
    /// Forwards to `SetCursorLocation` on the IBus input context when IBus is
    /// connected, and always records the rectangle locally so candidate
    /// placement works without a daemon too.
    pub fn set_cursor_rect(&self, x: i32, y: i32, w: u32, h: u32) {
        log::debug!("[Linux IME] set_cursor_rect: x={x}, y={y}, w={w}, h={h}");
        *self.cursor_rect.lock().unwrap() = (x, y, w, h);
        #[cfg(feature = "linux-a11y")]
        if let Some(conn) = self.ibus_connection.lock().unwrap().as_ref() {
            if !conn.set_cursor_location(x, y, w, h) {
                log::warn!("[Linux IME] SetCursorLocation failed (engine may be gone)");
            }
        }
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
            "[Linux IME] process_key_event: key={key_code}, mods={modifiers:#x}, pressed={pressed}",
        );

        // Forward to the engine first: IBus decides whether the key is consumed
        // for composition or passed through, and delivers preedit/commit back
        // through signals. Only when no engine consumed the key do we fall back
        // to the local printable-ASCII passthrough.
        #[cfg(feature = "linux-a11y")]
        if pressed {
            let consumed = {
                let guard = self.ibus_connection.lock().unwrap();
                guard
                    .as_ref()
                    .and_then(|conn| conn.process_key_event(key_code, key_code, modifiers))
            };
            if consumed == Some(true) {
                // The engine owns the decision; preedit/commit arrive as signals.
                return None;
            }
        }

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
        log::debug!("[Linux IME] set_marked_text: '{text}'");

        let len = text.len();
        let cursor = if sel_start >= 0 && sel_end >= 0 {
            sel_end.min(len as i32).max(0) as usize
        } else {
            len
        };

        *self.marked_text.lock().unwrap() = text.to_string();
        *self.cursor_pos.lock().unwrap() = cursor;
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
        log::info!("[Linux IME] focus_in: widget={widget_id}");
        // Announce capabilities once per connection, then focus the engine so it
        // starts composing into this context.
        #[cfg(feature = "linux-a11y")]
        if let Some(conn) = self.ibus_connection.lock().unwrap().as_ref() {
            conn.set_capabilities(IBUS_CAP_PREEDIT_TEXT_FOCUS);
            if !conn.focus_in() {
                log::warn!("[Linux IME] IBus FocusIn failed (engine may be gone)");
            }
        }
    }

    fn focus_out(&self, widget_id: ObjectId) {
        *self.focused_widget.lock().unwrap() = None;
        *self.active.lock().unwrap() = false;
        self.clear_composition();
        log::info!("[Linux IME] focus_out: widget={widget_id}");
        #[cfg(feature = "linux-a11y")]
        if let Some(conn) = self.ibus_connection.lock().unwrap().as_ref() {
            if !conn.focus_out() {
                log::warn!("[Linux IME] IBus FocusOut failed (engine may be gone)");
            }
        }
    }

    fn commit_text(&self, text: &str) {
        log::info!("[Linux IME] commit_text: '{text}'");
        self.clear_composition();
    }

    fn set_composition(&self, composition: &ImeComposition) {
        log::debug!("[Linux IME] set_composition: '{}'", composition.text);

        let text = &composition.text;
        let len = text.len();
        let cursor = composition.cursor_position.min(len);

        *self.marked_text.lock().unwrap() = text.to_string();
        *self.cursor_pos.lock().unwrap() = cursor;
    }

    fn set_candidate_window_position(&self, position: ImeCandidatePosition) {
        log::debug!("[Linux IME] set_candidate_window_position: ({}, {})", position.x, position.y);
        *self.candidate_position.lock().unwrap() = position;
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
        assert_eq!(
            *bridge.candidate_position.lock().unwrap(),
            ImeCandidatePosition { x: 100, y: 200 }
        );
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
        assert_eq!(*bridge.cursor_rect.lock().unwrap(), (10, 20, 100, 30));
    }

    #[test]
    fn test_ibus_availability_matches_feature() {
        // Without `linux-a11y` there is no DBus probe, so IBus must be reported
        // unavailable. With the feature the result depends on whether a session
        // bus and IBus daemon are reachable, so assert the consistency of the
        // reported flag with the feature gate rather than a fixed value.
        let bridge = LinuxImeBridge::new();
        #[cfg(not(feature = "linux-a11y"))]
        assert!(!bridge.is_ibus_available(), "no linux-a11y feature => no IBus probe");
        #[cfg(feature = "linux-a11y")]
        {
            // `is_ibus_available` reflects whether an input context was created;
            // the call must not panic regardless of the host environment.
            let _ = bridge.is_ibus_available();
        }
    }

    /// End-to-end check against a real IBus daemon.
    ///
    /// Runs only when `linux-a11y` is on and an IBus service owns
    /// `org.freedesktop.IBus` on the session bus. It asserts the bridge created
    /// an input context and that every engine-protocol call
    /// (`SetCapabilities`, `FocusIn`, `SetCursorLocation`, `ProcessKeyEvent`,
    /// `FocusOut`) is accepted by the daemon with its real D-Bus signature.
    ///
    /// Note on `ProcessKeyEvent`: the default `keyboard-us` engine *declines*
    /// plain ASCII (returns `false`) and only consumes keys while composing, so
    /// the test asserts the call is accepted rather than that it was consumed.
    #[cfg(feature = "linux-a11y")]
    #[test]
    fn test_real_ibus_input_context_when_daemon_present() {
        let bridge = LinuxImeBridge::new();
        if !bridge.is_ibus_available() {
            // No IBus on this host/CI runner: nothing to verify, and silently
            // passing would overstate coverage, so say so explicitly.
            eprintln!("skipped: no IBus daemon on the session bus");
            return;
        }

        let context_path = {
            let guard = bridge.ibus_connection.lock().unwrap();
            guard.as_ref().expect("available implies a connection").input_context_path.clone()
        };
        assert!(
            context_path.starts_with("/org/freedesktop/IBus/InputContext_"),
            "unexpected input-context path: {context_path}"
        );

        // Every engine call must be accepted by the daemon.
        {
            let guard = bridge.ibus_connection.lock().unwrap();
            let conn = guard.as_ref().unwrap();
            assert!(conn.set_capabilities(IBUS_CAP_PREEDIT_TEXT_FOCUS), "SetCapabilities");
            assert!(conn.focus_in(), "FocusIn");
            assert!(conn.set_cursor_location(10, 20, 100, 30), "SetCursorLocation");
            assert!(
                conn.process_key_event('a' as u32, 38, 0).is_some(),
                "ProcessKeyEvent must reach the engine"
            );
            assert!(conn.focus_out(), "FocusOut");
        }

        // The bridge-level path must also reach the engine (no silent fallback
        // to the local state machine while an engine is attached).
        bridge.focus_in(1);
        assert!(bridge.is_active(), "focus_in must mark the session active");
        bridge.set_cursor_rect(10, 20, 100, 30);
        bridge.focus_out(1);
        assert!(!bridge.is_active(), "focus_out must clear active state");
    }
}
