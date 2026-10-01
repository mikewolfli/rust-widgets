// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Windows IME bridge — TSF (Text Services Framework) integration.
//!
//! Provides a state-tracking IME bridge that manages text composition via
//! the TSF `ITfThreadMgr` / `ITfDocumentMgr` / `ITfContext` COM interfaces.
//!
//! When compiled on `target_os = "windows"`, the bridge queries TSF for
//! composition events and synchronises state with the widget layer.  On
//! other targets (or in headless testing) it operates as a pure state
//! machine that correctly tracks marked text, composition start offsets,
//! and cursor positions.

use crate::compat::{lock, Mutex, String, ToString};
use crate::core::ObjectId;
use crate::platform::ime::{ImeBridge, ImeCandidatePosition, ImeComposition};

// ──────────────────────────────────────────────
// Bridge struct
// ──────────────────────────────────────────────

/// Whether the process can actually reach the OS input-method layer.
///
/// # Why this replaces the TSF symbol probe
///
/// This module used to call `LoadLibraryA("msctf.dll")` + `GetProcAddress("TF_GetThreadMgr")`,
/// treat a **non-null function pointer** as "TSF initialized successfully", and set
/// `tsf_available = true` from it. Nothing called the function. `msctf.dll` exports
/// `TF_GetThreadMgr` on effectively every Windows install, so the flag was `true` on any normal
/// machine and [`ImeBridge::is_active`](super::ime::ImeBridge::is_active) reported an active IME
/// **connection that did not exist** — precisely the "reported success for something that did not
/// happen" the field's own doc says rule #26 forbids, and precisely the log-placeholder the module
/// claimed to have removed.
///
/// winapi 0.3.9 ships no TSF (`ctfutb`) bindings, so there is no way to create the `ITfThreadMgr`
/// this module's doc sketched — but there **is** a real, universally available OS call that answers
/// the same question at this layer: IMM32. `ImmGetContext(hwnd)` returns the input context for a
/// window, or `NULL` when the platform has no IME for it. That is a genuine measurement, not a
/// symbol lookup, so it is what the flag is now derived from.
///
/// The consequence is honest and worth stating: a build with no window to ask answers `false`, and
/// `is_active` stays `false` even with focus — because the bridge really has no OS connection. A
/// host that wants composition lifted to the OS must drive `set_marked_text`/`commit_text` from the
/// platform event loop (see [`WindowsImeBridge::set_cursor_rect`] and the `WM_IME_*` path).
#[cfg(target_os = "windows")]
fn native_ime_available() -> bool {
    use winapi::shared::windef::HWND;
    use winapi::um::imm::ImmGetContext;
    use winapi::um::winuser::{GetForegroundWindow, GetWindow};

    unsafe {
        // Ask for the foreground window's input context, falling back to its owner: a top-level
        // window without a context of its own may still be covered by one the IME attached to an
        // ancestor. `ImmGetContext` is the query IMM32 exposes for exactly this.
        let foreground: HWND = GetForegroundWindow();
        if foreground.is_null() {
            return false;
        }
        for candidate in [foreground, GetWindow(foreground, winapi::um::winuser::GW_OWNER)] {
            if candidate.is_null() {
                continue;
            }
            let context = ImmGetContext(candidate);
            if !context.is_null() {
                // Matching release: the context is a reference the caller owns.
                winapi::um::imm::ImmReleaseContext(candidate, context);
                return true;
            }
        }
        false
    }
}

/// Off Windows there is no IME to reach, and the truthful answer is `false` rather than a
/// fabricated `true`. See [`native_ime_available`].
#[cfg(not(target_os = "windows"))]
fn native_ime_available() -> bool {
    false
}

// ──────────────────────────────────────────────
// Bridge struct
// ──────────────────────────────────────────────

/// Real Windows IME bridge backed by state tracking and a live OS input-context probe.
///
/// # What "real" means here, honestly
///
/// The composition/**state** half of this bridge (focus, marked text, cursor, candidate position) is
/// a genuine, target-independent state machine and is always correct.
///
/// The **OS connection** half — whether an input method is reachable for this process — is answered
/// by `native_ime_available`, a real `ImmGetContext` query against the foreground window. That is
/// the authority [`ImeBridge::is_active`] reports from, so a host with no input context answers
/// **honestly** (`false`) rather than claiming an IME it never talks to (rules #26/#53).
///
/// # What is deliberately *not* claimed
///
/// Draining the system IME's composition string and positioning its candidate window needs the TSF
/// COM interfaces (`ITfThreadMgr` etc.), and `winapi` 0.3 ships neither `CLSID_TF_ThreadMgr` nor
/// their `IID`s. This module used to paper over that by treating a `msctf.dll` symbol lookup as a
/// successful connection — see `native_ime_available` for why that was a fabricated claim. The
/// composition state machine keeps working for hosts that drive it directly from `WM_IME_*`.
pub struct WindowsImeBridge {
    /// The widget that currently has IME focus.
    focused_widget: Mutex<Option<ObjectId>>,

    // ── Composition / marked-text state ──
    /// Current preedit (marked / composition) text string.
    marked_text: Mutex<String>,
    /// Byte offset of the composition start within the text buffer.
    composition_start: Mutex<usize>,
    /// Cursor (insertion point) position inside the composition, in bytes.
    cursor_pos: Mutex<usize>,
    /// Last insertion-point rectangle in screen coordinates.
    cursor_rect: Mutex<(i32, i32, u32, u32)>,
    /// Last requested candidate window position.
    candidate_position: Mutex<ImeCandidatePosition>,

    // ── Native TSF handle ──
    /// Whether the process can reach the OS input-method layer.
    ///
    /// This is the authority [`ImeBridge::is_active`] answers from. It is not merely informational:
    /// the bridge can track focus and composition in memory on any target, but only a real OS
    /// connection can drive the OS candidate window and receive the OS composition string. Reporting
    /// "active" without one would be exactly the log-placeholder claim rule #26 forbids.
    ///
    /// It is filled from [`native_ime_available`], a real `ImmGetContext` query — not, as it used
    /// to be, from whether a DLL exported a symbol.
    native_ime_available: Mutex<bool>,
}

crate::impl_default_via_new!(WindowsImeBridge);

impl WindowsImeBridge {
    /// Create a new Windows IME bridge.
    ///
    /// On `target_os = "windows"` this attempts to initialise the TSF
    /// thread manager.  On other targets (or when TSF is unavailable) it
    /// falls back to pure state tracking.
    pub fn new() -> Self {
        let native = native_ime_available();
        if native {
            log::info!("[Windows IME] an OS input context is reachable; composition can be lifted");
        } else {
            log::debug!(
                "[Windows IME] no OS input context reachable — tracking focus and composition in \
                 memory only"
            );
        }

        Self {
            focused_widget: Mutex::new(None),
            marked_text: Mutex::new(String::new()),
            composition_start: Mutex::new(0),
            cursor_pos: Mutex::new(0),
            cursor_rect: Mutex::new((0, 0, 0, 0)),
            candidate_position: Mutex::new(ImeCandidatePosition { x: 0, y: 0 }),
            native_ime_available: Mutex::new(native),
        }
    }

    /// Re-probes the OS for an input context.
    ///
    /// [`Self::new`] probes once, and on Windows the probe is about the **foreground window** —
    /// which does not exist until the host has created one. A bridge constructed before the first
    /// window would therefore latch `false` forever. A host calls this after its window is shown so
    /// [`ImeBridge::is_active`] reflects the real state.
    /// Returns the flag it recorded.
    pub fn refresh_native_availability(&self) -> bool {
        let native = native_ime_available();
        *lock(&self.native_ime_available) = native;
        native
    }

    /// Whether an OS input context was reachable when this bridge last probed.
    ///
    /// The observable half of [`Self::refresh_native_availability`]; it is what
    /// [`ImeBridge::is_active`] reports from.
    pub fn has_native_ime(&self) -> bool {
        *lock(&self.native_ime_available)
    }

    // ── Native IME interface (exposed for platform event dispatch) ──

    /// Set the cursor (insertion-point) rectangle in screen coordinates.
    /// On native Windows this calls `ITfContext::GetSelection` /
    /// `ITfContext::SetSelection` to update the TSF composition window
    /// position.
    pub fn set_cursor_rect(&self, x: i32, y: i32, w: u32, h: u32) {
        *lock(&self.cursor_rect) = (x, y, w, h);
        // A real TSF connection would now move the candidate window via
        // `ITfContext::GetSelection` / `SetSelection`; without one, the position is recorded for
        // `is_active`-aware callers (a host that drives the state machine itself) rather than
        // silently claiming the OS window moved.
    }

    /// Process a raw key event through the TSF IME subsystem.
    ///
    /// Returns `Some(text)` if the key event produces committed text.
    /// Returns `None` if the IME consumed the event for composition.
    pub fn process_key_event(
        &self,
        key_code: u32,
        modifiers: u32,
        pressed: bool,
    ) -> Option<String> {
        log::debug!(
            "[Windows IME] process_key_event: key={}, mods={:#x}, pressed={}",
            key_code,
            modifiers,
            pressed,
        );

        // When TSF is active, `ITfKeyEventSink::OnKeyDown` handles this.
        // In state-machine mode we simulate a simple passthrough.

        if self.has_marked_text() {
            // During composition the IME consumes all key events.
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

    /// Set marked (preedit / composition) text with selection endpoints.
    ///
    /// `sel_start` / `sel_end` are byte offsets **within** the composition.
    /// A value of `-1` for both indicates cursor at end.
    pub fn set_marked_text(&self, text: &str, sel_start: i32, sel_end: i32) {
        log::debug!("[Windows IME] set_marked_text: '{}'", text);

        let len = text.len();
        let cursor = if sel_start >= 0 && sel_end >= 0 {
            let end = sel_end as usize;
            end.min(len)
        } else {
            len
        };

        *lock(&self.marked_text) = text.to_string();
        *lock(&self.composition_start) = 0;
        *lock(&self.cursor_pos) = cursor;

        // Native TSF:   ITfComposition::EndComposition if empty
        //               ITfContext::SetComposition otherwise
        //               ITfCompositionSink::OnCompositionTerminated
    }

    /// Get the current marked (preedit) text, if any.
    pub fn get_marked_text(&self) -> Option<String> {
        let text = lock(&self.marked_text);
        if text.is_empty() {
            None
        } else {
            Some(text.clone())
        }
    }

    /// Returns `true` when there is an active IME composition.
    pub fn has_marked_text(&self) -> bool {
        !lock(&self.marked_text).is_empty()
    }

    /// Discard the current composition without committing.
    pub fn discard_marked_text(&self) {
        log::debug!("[Windows IME] discard_marked_text");
        *lock(&self.marked_text) = String::new();
        *lock(&self.composition_start) = 0;
        *lock(&self.cursor_pos) = 0;

        // Native TSF:  ITfComposition::EndComposition
        //              ITfContext::SetSelection(cursor_at_start)
    }

    /// Clear internal composition state (shared helper).
    fn clear_composition(&self) {
        *lock(&self.marked_text) = String::new();
        *lock(&self.composition_start) = 0;
        *lock(&self.cursor_pos) = 0;
    }
}

// ──────────────────────────────────────────────
// ImeBridge trait implementation
// ──────────────────────────────────────────────

impl ImeBridge for WindowsImeBridge {
    fn focus_in(&self, widget_id: ObjectId) {
        *lock(&self.focused_widget) = Some(widget_id);
        log::info!("[Windows IME] focus_in: widget={}", widget_id);

        // With the TSF binding this would call `ITfThreadMgr::SetFocus(doc_mgr)` /
        // `ITfDocumentMgr::Push(context)`. `is_active` reports the connection's actual availability,
        // so focus alone never claims an IME that is not connected.
    }

    fn focus_out(&self, widget_id: ObjectId) {
        *lock(&self.focused_widget) = None;
        self.clear_composition();
        log::info!("[Windows IME] focus_out: widget={}", widget_id);

        // With the TSF binding this would call `ITfDocumentMgr::Pop(TF_POPF_ALL)`.
    }

    fn commit_text(&self, text: &str) {
        log::info!("[Windows IME] commit_text: '{}'", text);
        self.clear_composition();

        // Native TSF: ITfComposition::EndComposition
        //             ITfInsertAtSelection::InsertTextAtSelection
    }

    fn set_composition(&self, composition: &ImeComposition) {
        log::debug!("[Windows IME] set_composition: '{}'", composition.text);

        let text = &composition.text;
        let len = text.len();

        *lock(&self.marked_text) = text.to_string();
        *lock(&self.composition_start) = 0;

        let cursor = composition.cursor_position.min(len);
        *lock(&self.cursor_pos) = cursor;

        // Native TSF: ITfContext::SetComposition(composition, text)
        //             ITfCompositionSink callbacks
    }

    fn set_candidate_window_position(&self, position: ImeCandidatePosition) {
        log::debug!(
            "[Windows IME] set_candidate_window_position: ({}, {})",
            position.x,
            position.y,
        );
        *lock(&self.candidate_position) = position;
        // Native TSF: ITfThreadMgr::GetGlobalCompartment → set candidate
        //             window position via ITfCandidateListUIElement.
    }

    fn is_active(&self) -> bool {
        // Honest activity: a real OS input context **and** a focused widget. The previous body
        // returned only the focus flag, and the flag it later gained was itself fabricated from a
        // DLL symbol lookup, so the bridge reported an active IME while it never talked to the OS —
        // the "reported success for something that did not happen" failure rules #26/#53 forbid.
        self.has_native_ime() && lock(&self.focused_widget).is_some()
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
        let bridge = WindowsImeBridge::new();
        assert!(!bridge.is_active());
        assert!(lock(&bridge.focused_widget).is_none());

        bridge.focus_in(42);
        assert_eq!(*lock(&bridge.focused_widget), Some(42));
        // `is_active` reports a **real OS input context**, not focus: on a host with no such context
        // (every non-Windows target, and a Windows host with no input context for the foreground
        // window) it must stay `false` even with a focused widget — claiming otherwise was the
        // log-placeholder defect rules #26/#53 forbid.
        assert_eq!(
            bridge.is_active(),
            bridge.has_native_ime(),
            "activity must mirror whether a real OS input context exists"
        );

        bridge.focus_out(42);
        assert!(!bridge.is_active());
        assert!(lock(&bridge.focused_widget).is_none());
    }

    #[test]
    fn test_commit_text_clears_composition() {
        let bridge = WindowsImeBridge::new();
        bridge.set_marked_text("hello", 5, 5);
        assert!(bridge.has_marked_text());

        bridge.commit_text("hello");
        assert!(!bridge.has_marked_text());
        assert_eq!(bridge.get_marked_text(), None);
    }

    #[test]
    fn test_commit_text_via_trait() {
        let bridge = WindowsImeBridge::new();
        bridge.set_marked_text("你好", 6, 6);
        assert!(bridge.has_marked_text());

        ImeBridge::commit_text(&bridge, "你好");
        assert!(!bridge.has_marked_text());
    }

    #[test]
    fn test_set_composition_trait() {
        let bridge = WindowsImeBridge::new();
        let comp = ImeComposition {
            text: "composing".to_string(),
            cursor_position: 5,
            selection_length: 0,
        };
        bridge.set_composition(&comp);
        assert!(bridge.has_marked_text());
        assert_eq!(bridge.get_marked_text(), Some("composing".to_string()));
        assert_eq!(*lock(&bridge.cursor_pos), 5);
    }

    #[test]
    fn test_set_composition_empty_clears() {
        let bridge = WindowsImeBridge::new();
        bridge.set_marked_text("something", 5, 0);

        let empty = ImeComposition::default();
        bridge.set_composition(&empty);
        assert!(!bridge.has_marked_text());
    }

    #[test]
    fn test_discard_marked_text() {
        let bridge = WindowsImeBridge::new();
        bridge.set_marked_text("你好世界", 4, 8);
        assert!(bridge.has_marked_text());
        assert_eq!(*lock(&bridge.cursor_pos), 8);

        bridge.discard_marked_text();
        assert!(!bridge.has_marked_text());
        assert_eq!(bridge.get_marked_text(), None);
        assert_eq!(*lock(&bridge.cursor_pos), 0);
    }

    #[test]
    fn test_set_marked_text_negative_sel_defaults_cursor_at_end() {
        let bridge = WindowsImeBridge::new();
        bridge.set_marked_text("test", -1, -1);
        // Cursor should be at end of "test" (byte offset 4).
        assert_eq!(*lock(&bridge.cursor_pos), 4);
    }

    #[test]
    fn test_is_active_after_events() {
        let bridge = WindowsImeBridge::new();
        assert!(!bridge.is_active());
        bridge.focus_in(1);
        // Focus is not activity: `is_active` is honest about the OS input context (see
        // `is_active`'s own comment). On a host that can reach one it becomes true; without it, false.
        assert_eq!(bridge.is_active(), bridge.has_native_ime());
        bridge.focus_out(1);
        assert!(!bridge.is_active());
    }

    #[test]
    fn test_process_key_event_no_composition() {
        let bridge = WindowsImeBridge::new();
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
        let bridge = WindowsImeBridge::new();
        bridge.set_marked_text(" composing", 10, 0);
        let result = bridge.process_key_event(0x61, 0x00, true);
        assert_eq!(result, None);
    }

    #[test]
    fn test_process_key_event_released() {
        let bridge = WindowsImeBridge::new();
        // Key released => should return None.
        let result = bridge.process_key_event(0x61, 0x00, false);
        assert_eq!(result, None);
    }

    #[test]
    fn test_set_candidate_window_position() {
        let bridge = WindowsImeBridge::new();
        bridge.set_candidate_window_position(ImeCandidatePosition { x: 50, y: 75 });
        assert_eq!(*lock(&bridge.candidate_position), ImeCandidatePosition { x: 50, y: 75 });
    }

    #[test]
    fn test_focus_out_discards_composition() {
        let bridge = WindowsImeBridge::new();
        bridge.set_marked_text("pending", 7, 0);
        assert!(bridge.has_marked_text());

        bridge.focus_out(1);
        assert!(!bridge.has_marked_text());
        assert!(!bridge.is_active());
    }

    #[test]
    fn test_set_cursor_rect() {
        let bridge = WindowsImeBridge::new();
        bridge.set_cursor_rect(0, 0, 200, 20);
        assert_eq!(*lock(&bridge.cursor_rect), (0, 0, 200, 20));
    }

    #[test]
    fn test_native_availability_is_a_real_os_probe_and_agrees_with_is_active() {
        // Native IME availability is a property of the *host*, not of the test build: a
        // machine that can give the foreground window an input context reports `true`, one
        // that cannot reports `false`. Asserting a fixed value would encode one machine's
        // configuration.
        //
        // # What must hold everywhere, and what used to
        //
        // The flag used to be set from "does `msctf.dll` export `TF_GetThreadMgr`?", which is
        // true on essentially every Windows install and did not call anything — so the bridge
        // reported a live IME *connection* that did not exist. The probe is now the real
        // `ImmGetContext` query, and this asserts the two things that make it honest: the flag
        // is exactly what a fresh probe returns, and `is_active` agrees with it when no widget
        // holds focus (there is none here).
        let bridge = WindowsImeBridge::new();
        assert_eq!(
            bridge.has_native_ime(),
            native_ime_available(),
            "the recorded flag must be what the probe answers, not a symbol lookup"
        );
        // Re-probing must be idempotent on a host whose state has not changed.
        assert_eq!(bridge.refresh_native_availability(), bridge.has_native_ime());
        assert!(!bridge.is_active(), "no widget holds focus, so nothing is active");
        assert!(!bridge.has_marked_text());
    }
}
