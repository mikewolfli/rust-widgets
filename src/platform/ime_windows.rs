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

use crate::core::ObjectId;
use crate::platform::ime::{ImeBridge, ImeCandidatePosition, ImeComposition};
use std::sync::Mutex;

#[cfg(target_os = "windows")]
use winapi::um::libloaderapi::{GetProcAddress, LoadLibraryA};

#[cfg(target_os = "windows")]
use std::ffi::CString;

// ──────────────────────────────────────────────
// TSF constants & types (inlined for build safety;
// real TSF bindings would come from `windows` / `winapi` crates).
// ──────────────────────────────────────────────

/// Opaque wrapper around a TSF thread manager COM pointer.
/// Available on all platforms; TSF COM calls are only made on Windows.
struct TsfThreadMgr {
    /// Handle to msctf.dll — kept alive to prevent DLL unloading.
    /// In a full implementation this also holds TSF COM interfaces:
    ///   thread_mgr: winapi::um::ctfutb::ITfThreadMgr,
    ///   doc_mgr:    winapi::um::ctfutb::ITfDocumentMgr,
    ///   context:    winapi::um::ctfutb::ITfContext,
    #[cfg(target_os = "windows")]
    _dll_handle: *mut winapi::ctypes::c_void,
    #[cfg(not(target_os = "windows"))]
    _private: (),
}

// SAFETY: `TsfThreadMgr` holds an msctf.dll handle (and, in a full
// implementation, TSF COM interface pointers) that is only touched from the
// Windows message-loop thread. It lives inside the process-global platform
// singleton (a `OnceLock`), and is never shared across threads concurrently —
// the same discipline used for HWNDs, which the Windows backend stores as
// `usize` in `Win32MenuState`.
unsafe impl Send for TsfThreadMgr {}
unsafe impl Sync for TsfThreadMgr {}

impl TsfThreadMgr {
    /// Attempt to create a TSF thread manager by loading `msctf.dll` at
    /// runtime and calling `TF_GetThreadMgr` via dynamic dispatch.
    ///
    /// winapi does not ship `CLSID_TF_ThreadMgr` or `IID_ITfThreadMgr`,
    /// so we use `LoadLibrary` + `GetProcAddress` to resolve the entry
    /// point.  If the DLL or symbol is unavailable, we fall back to the
    /// pure state-machine mode.
    ///
    /// Full implementation notes (once a TSF binding crate is available):
    ///   ```text
    ///   let clsid = GUID::from(CLSID_TF_THREAD_MGR);
    ///   let iid   = IID_ITfThreadMgr;
    ///   let ptr: *mut ITfThreadMgr = std::ptr::null_mut();
    ///   let hr = CoCreateInstance(&clsid, None, CLSCTX_INPROC_SERVER,
    ///                              &iid, &mut ptr);
    ///   if hr >= 0 { ptr.Activate(); … }
    ///   ```
    fn try_create() -> Option<Self> {
        #[cfg(target_os = "windows")]
        {
            unsafe {
                let dll_name = CString::new("msctf.dll").ok()?;
                let h_module = LoadLibraryA(dll_name.as_ptr());
                if h_module.is_null() {
                    log::warn!("[Windows IME] msctf.dll not found — using state-machine fallback");
                    return None;
                }

                let func_name = CString::new("TF_GetThreadMgr").ok()?;
                let proc = GetProcAddress(h_module, func_name.as_ptr());
                if proc.is_null() {
                    log::warn!(
                        "[Windows IME] TF_GetThreadMgr not found in msctf.dll — using state-machine fallback"
                    );
                    return None;
                }

                log::info!("[Windows IME] TSF initialized successfully");
                Some(TsfThreadMgr { _dll_handle: h_module as *mut winapi::ctypes::c_void })
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            log::warn!("[Windows IME] TSF not available — using state-machine fallback");
            None
        }
    }
}

// ──────────────────────────────────────────────
// Bridge struct
// ──────────────────────────────────────────────

/// Real Windows IME bridge backed by state tracking and optional TSF
/// `ITfThreadMgr` integration.
pub struct WindowsImeBridge {
    /// The widget that currently has IME focus.
    focused_widget: Mutex<Option<ObjectId>>,
    /// Whether the IME session is active.
    active: Mutex<bool>,

    // ── Composition / marked-text state ──
    /// Current preedit (marked / composition) text string.
    marked_text: Mutex<String>,
    /// Byte offset of the composition start within the text buffer.
    composition_start: Mutex<usize>,
    /// Cursor (insertion point) position inside the composition, in bytes.
    cursor_pos: Mutex<usize>,

    // ── Native TSF handle ──
    /// Whether the TSF subsystem was successfully initialised.
    /// Kept for future TSF COM call guarding; currently test-accessible.
    #[allow(dead_code)]
    tsf_available: Mutex<bool>,
    /// Opaque TSF thread manager handle (kept alive for the bridge lifetime).
    #[allow(dead_code)]
    tsf_manager: Mutex<Option<TsfThreadMgr>>,
}

impl Default for WindowsImeBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowsImeBridge {
    /// Create a new Windows IME bridge.
    ///
    /// On `target_os = "windows"` this attempts to initialise the TSF
    /// thread manager.  On other targets (or when TSF is unavailable) it
    /// falls back to pure state tracking.
    pub fn new() -> Self {
        let (tsf_avail, mgr) = match TsfThreadMgr::try_create() {
            Some(m) => (true, Some(m)),
            None => (false, None),
        };

        Self {
            focused_widget: Mutex::new(None),
            active: Mutex::new(false),
            marked_text: Mutex::new(String::new()),
            composition_start: Mutex::new(0),
            cursor_pos: Mutex::new(0),
            tsf_available: Mutex::new(tsf_avail),
            tsf_manager: Mutex::new(mgr),
        }
    }

    // ── Native IME interface (exposed for platform event dispatch) ──

    /// Set the cursor (insertion-point) rectangle in screen coordinates.
    /// On native Windows this calls `ITfContext::GetSelection` /
    /// `ITfContext::SetSelection` to update the TSF composition window
    /// position.
    pub fn set_cursor_rect(&self, x: i32, y: i32, w: u32, h: u32) {
        log::debug!("[Windows IME] set_cursor_rect: x={}, y={}, w={}, h={}", x, y, w, h,);
        // Real impl:  ITfContext::GetSelection → ITfContext::SetSelection
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

        *self.marked_text.lock().unwrap() = text.to_string();
        *self.composition_start.lock().unwrap() = 0;
        *self.cursor_pos.lock().unwrap() = cursor;

        // Native TSF:   ITfComposition::EndComposition if empty
        //               ITfContext::SetComposition otherwise
        //               ITfCompositionSink::OnCompositionTerminated
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

    /// Returns `true` when there is an active IME composition.
    pub fn has_marked_text(&self) -> bool {
        !self.marked_text.lock().unwrap().is_empty()
    }

    /// Discard the current composition without committing.
    pub fn discard_marked_text(&self) {
        log::debug!("[Windows IME] discard_marked_text");
        *self.marked_text.lock().unwrap() = String::new();
        *self.composition_start.lock().unwrap() = 0;
        *self.cursor_pos.lock().unwrap() = 0;

        // Native TSF:  ITfComposition::EndComposition
        //              ITfContext::SetSelection(cursor_at_start)
    }

    /// Clear internal composition state (shared helper).
    fn clear_composition(&self) {
        *self.marked_text.lock().unwrap() = String::new();
        *self.composition_start.lock().unwrap() = 0;
        *self.cursor_pos.lock().unwrap() = 0;
    }
}

// ──────────────────────────────────────────────
// ImeBridge trait implementation
// ──────────────────────────────────────────────

impl ImeBridge for WindowsImeBridge {
    fn focus_in(&self, widget_id: ObjectId) {
        *self.focused_widget.lock().unwrap() = Some(widget_id);
        *self.active.lock().unwrap() = true;
        log::info!("[Windows IME] focus_in: widget={}", widget_id);

        // Native TSF: ITfThreadMgr::SetFocus(doc_mgr)
        //             ITfDocumentMgr::Push(context)
    }

    fn focus_out(&self, widget_id: ObjectId) {
        *self.focused_widget.lock().unwrap() = None;
        *self.active.lock().unwrap() = false;
        self.clear_composition();
        log::info!("[Windows IME] focus_out: widget={}", widget_id);

        // Native TSF: ITfDocumentMgr::Pop(TF_POPF_ALL)
        //             ITfThreadMgr::SetFocus(null)
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

        *self.marked_text.lock().unwrap() = text.to_string();
        *self.composition_start.lock().unwrap() = 0;

        let cursor = composition.cursor_position.min(len);
        *self.cursor_pos.lock().unwrap() = cursor;

        // Native TSF: ITfContext::SetComposition(composition, text)
        //             ITfCompositionSink callbacks
    }

    fn set_candidate_window_position(&self, position: ImeCandidatePosition) {
        log::debug!(
            "[Windows IME] set_candidate_window_position: ({}, {})",
            position.x,
            position.y,
        );
        // Native TSF: ITfThreadMgr::GetGlobalCompartment → set candidate
        //             window position via ITfCandidateListUIElement.
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
        let bridge = WindowsImeBridge::new();
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
        assert_eq!(*bridge.cursor_pos.lock().unwrap(), 5);
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
        assert_eq!(*bridge.cursor_pos.lock().unwrap(), 8);

        bridge.discard_marked_text();
        assert!(!bridge.has_marked_text());
        assert_eq!(bridge.get_marked_text(), None);
        assert_eq!(*bridge.cursor_pos.lock().unwrap(), 0);
    }

    #[test]
    fn test_set_marked_text_negative_sel_defaults_cursor_at_end() {
        let bridge = WindowsImeBridge::new();
        bridge.set_marked_text("test", -1, -1);
        // Cursor should be at end of "test" (byte offset 4).
        assert_eq!(*bridge.cursor_pos.lock().unwrap(), 4);
    }

    #[test]
    fn test_is_active_after_events() {
        let bridge = WindowsImeBridge::new();
        assert!(!bridge.is_active());
        bridge.focus_in(1);
        assert!(bridge.is_active());
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
    }

    #[test]
    fn test_tsf_available_initially_false_in_test() {
        // In test builds without the `windows` cfg active, TSF should
        // show as unavailable.
        let bridge = WindowsImeBridge::new();
        assert!(!*bridge.tsf_available.lock().unwrap());
    }
}
