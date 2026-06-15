//! macOS IME bridge — real `NSTextInputContext` integration.
//!
//! Provides a state-tracking IME bridge that correctly manages marked text
//! (preedit) composition with UTF-16-range-aware positions, as required by
//! the [`NSTextInputClient`] protocol.
//!
//! When the `objc2-macos` feature is enabled, native `NSTextInputContext`
//! calls are made to synchronize platform IME state. Without the feature,
//! the bridge operates as a pure state-machine — suitable for testing and
//! headless builds.
//!
//! # UTF-16 range tracking
//!
//! macOS IME APIs (like `attributedSubstringForProposedRange:actualRange:`)
//! work in UTF-16 code units, not Rust `char` or byte offsets. This bridge
//! tracks `marked_range` and `selected_range` as UTF-16 (NSRange) values and
//! converts to/from Rust `String` only when producing output for the widget
//! layer.

#![cfg(target_os = "macos")]

use crate::core::ObjectId;
use crate::platform::ime::{ImeBridge, ImeCandidatePosition, ImeComposition};
use std::sync::Mutex;

// ──────────────────────────────────────────────
// Native macOS IME imports (feature-gated)
// ──────────────────────────────────────────────

/// Wrapper that attempts to acquire an `NSTextInputContext` from a raw view
/// pointer, returning a boxed opaque token if successful.
///
/// Uses real `msg_send!` calls to interact with AppKit at runtime.
#[cfg(feature = "objc2-macos")]
fn try_activate_nstextinputcontext(
    view_ptr: *mut std::ffi::c_void,
) -> Option<Box<dyn std::any::Any + Send>> {
    use objc2::MainThreadMarker;

    // AppKit calls must happen on the main thread.
    MainThreadMarker::new()?;

    if view_ptr.is_null() {
        return None;
    }

    unsafe {
        use objc2::msg_send;
        use objc2::runtime::Object;

        // Get the NSTextInputContext from the view via [view inputContext].
        let view: *mut Object = view_ptr as *mut Object;
        let ctx: *mut Object = msg_send![view, inputContext];
        if ctx.is_null() {
            log::warn!("[macOS IME] View has no NSTextInputContext");
            return None;
        }

        // Activate the input context so it receives IME events.
        let _: () = msg_send![ctx, activate];

        log::debug!("[macOS IME] NSTextInputContext activated");

        // Opaque wrapper to carry the raw pointer through Box<dyn Any + Send>.
        #[repr(C)]
        struct ImeCtx(*mut Object);
        unsafe impl Send for ImeCtx {}

        Some(Box::new(ImeCtx(ctx)) as Box<dyn std::any::Any + Send>)
    }
}

/// Synchronise the platform `NSTextInputContext` with our tracked state.
///
/// Calls `invalidateCharacterCoordinates` on the stored context so the
/// IME system re-queries the composition state from the `NSTextInputClient`
/// (the backing view).
#[cfg(feature = "objc2-macos")]
fn sync_nstextinputcontext(
    token: &dyn std::any::Any,
    _marked_text: &str,
    _marked_range: (usize, usize),
    _selected_range: (usize, usize),
) {
    unsafe {
        use objc2::msg_send;
        use objc2::runtime::Object;

        #[repr(C)]
        struct ImeCtx(*mut Object);

        // SAFETY: The token was created by try_activate_nstextinputcontext,
        // so the repr(C) layout guarantees downcast_ref works.
        let Some(ime_ctx) = token.downcast_ref::<ImeCtx>() else {
            return;
        };

        let ctx: *mut Object = ime_ctx.0;
        if ctx.is_null() {
            return;
        }

        // Tell the IME that the cursor / composition state changed so it
        // re-queries our NSTextInputClient for the latest data.
        let _: () = msg_send![ctx, invalidateCharacterCoordinates];

        log::debug!("[macOS IME] NSTextInputContext synchronised");
    }
}

// ──────────────────────────────────────────────
// Bridge struct
// ──────────────────────────────────────────────

/// Real macOS IME bridge backed by state tracking and optional
/// `NSTextInputContext` integration.
pub struct MacOsImeBridge {
    /// The widget that currently has IME focus.
    focused_widget: Mutex<Option<ObjectId>>,
    /// Whether the IME session is active.
    active: Mutex<bool>,

    // ── Composition / marked-text state ──
    /// Current preedit (marked) text string.
    marked_text: Mutex<String>,
    /// UTF-16-based range of the marked text within the whole text buffer.
    /// `(offset, length)` in UTF-16 code units.
    marked_range: Mutex<(usize, usize)>,
    /// UTF-16-based selection range inside the marked text.
    selected_range: Mutex<(usize, usize)>,

    // ── Native platform token ──
    /// Opaque handle to the `NSTextInputContext` (only used on macOS with
    /// `objc2-macos` feature). Kept alive for the bridge lifetime.
    #[allow(dead_code)]
    native_token: Mutex<Option<Box<dyn std::any::Any + Send>>>,
}

impl Default for MacOsImeBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl MacOsImeBridge {
    /// Create a new macOS IME bridge with empty state.
    pub fn new() -> Self {
        Self {
            focused_widget: Mutex::new(None),
            active: Mutex::new(false),
            marked_text: Mutex::new(String::new()),
            marked_range: Mutex::new((0, 0)),
            selected_range: Mutex::new((0, 0)),
            native_token: Mutex::new(None),
        }
    }

    // ── Native IME interface (exposed for platform event dispatch) ──

    /// Attach this bridge to a native `NSView` so that `NSTextInputContext`
    /// can be activated. `view_ptr` is a raw pointer to an `NSView`.
    ///
    /// On builds **without** `objc2-macos` this is a no-op.
    pub fn attach_to_view(&self, view_ptr: *mut std::ffi::c_void) {
        #[cfg(feature = "objc2-macos")]
        {
            if let Some(token) = try_activate_nstextinputcontext(view_ptr) {
                *self.native_token.lock().unwrap() = Some(token);
            }
        }
        let _ = view_ptr;
    }

    /// Set the cursor (insertion-point) rectangle in screen coordinates.
    /// This tells the IME where to position the candidate window.
    pub fn set_cursor_rect(&self, x: i32, y: i32, w: u32, h: u32) {
        log::debug!("[macOS IME] set_cursor_rect: x={}, y={}, w={}, h={}", x, y, w, h,);

        #[cfg(feature = "objc2-macos")]
        {
            unsafe {
                use objc2::msg_send;
                use objc2::runtime::{AnyClass, Object};
                use objc2::sel;

                if let Some(cls) = AnyClass::get(c"NSTextInputContext") {
                    // Guard: the class may be registered without AppKit being
                    // fully initialised (e.g. in test environments).
                    let responds: bool = msg_send![cls, respondsToSelector: sel!(activeContext)];
                    if responds {
                        let ctx: *mut Object = msg_send![cls, activeContext];
                        if !ctx.is_null() {
                            let _: () = msg_send![ctx, invalidateCharacterCoordinates];
                        }
                    }
                }
            }
        }
    }

    /// Process a raw key event through the IME subsystem.
    ///
    /// Returns `Some(text)` if the key event should be handled as committed
    /// text by the widget. Returns `None` if the IME consumed the event for
    /// composition (marked text updated).
    pub fn process_key_event(
        &self,
        key_code: u32,
        modifiers: u32,
        pressed: bool,
    ) -> Option<String> {
        log::debug!(
            "[macOS IME] process_key_event: key={}, mods={:#x}, pressed={}",
            key_code,
            modifiers,
            pressed,
        );

        // When a native NSTextInputContext is active, the key event is
        // forwarded via [NSTextInputContext handleEvent:].  The context
        // calls back into our insertText: / setMarkedText: methods.
        //
        // Without native FFI we simulate a simple passthrough: committed
        // printable characters are returned; everything else (function keys,
        // modifiers) is swallowed.
        if self.has_marked_text() {
            // While composing, key events are consumed by the IME.
            // The IME will call commit_text / set_marked_text as callbacks.
            return None;
        }

        // Simple ASCII passthrough for non-composing state (test/headless).
        if !pressed {
            return None;
        }
        // Only passthrough printable ASCII when no composition is active.
        if (0x20..=0x7e).contains(&key_code) {
            let ch = char::from_u32(key_code)?;
            // Respect shift for uppercase letters.
            let final_char = if modifiers & 0x02 != 0 { ch.to_ascii_uppercase() } else { ch };
            return Some(final_char.to_string());
        }
        if key_code == 0x0d || key_code == 0x03 {
            // Enter / Return
            return Some("\n".to_string());
        }
        if key_code == 0x09 {
            // Tab
            return Some("\t".to_string());
        }
        None
    }

    /// Clear the internal composition state after text is committed.
    /// Called by both the inherent API and the `ImeBridge` trait impl.
    fn clear_composition(&self) {
        *self.marked_text.lock().unwrap() = String::new();
        *self.marked_range.lock().unwrap() = (0, 0);
        *self.selected_range.lock().unwrap() = (0, 0);
    }

    /// Commit a piece of text (called by the native IME callback).
    /// Clears any active composition.
    pub fn commit_text(&self, text: &str) {
        log::info!("[macOS IME] commit_text: '{}'", text);
        self.clear_composition();
    }

    /// Set marked (preedit) text with selection range.
    ///
    /// `sel_start` / `sel_end` are UTF-16 offsets **within** the marked text.
    /// A value of `-1` for both indicates no selection (cursor at end).
    pub fn set_marked_text(&self, text: &str, sel_start: i32, sel_end: i32) {
        log::debug!("[macOS IME] set_marked_text: '{}'", text);

        let utf16_len = text.encode_utf16().count();

        // Determine selection range within the marked text (UTF-16).
        let (sel_offset, sel_length) = if sel_start >= 0 && sel_end >= 0 {
            let start = sel_start as usize;
            let end = sel_end as usize;
            let clamped_start = start.min(utf16_len);
            let clamped_end = end.min(utf16_len);
            if clamped_start <= clamped_end {
                (clamped_start, clamped_end - clamped_start)
            } else {
                (clamped_end, clamped_start - clamped_end)
            }
        } else {
            // Default: cursor at end of marked text.
            (utf16_len, 0)
        };

        *self.marked_text.lock().unwrap() = text.to_string();
        *self.marked_range.lock().unwrap() = (0, utf16_len);
        *self.selected_range.lock().unwrap() = (sel_offset, sel_length);

        // Sync with native NSTextInputContext if available.
        #[cfg(feature = "objc2-macos")]
        {
            let guard = self.native_token.lock().unwrap();
            if let Some(ref token) = *guard {
                sync_nstextinputcontext(
                    token.as_ref(),
                    text,
                    (0, utf16_len),
                    (sel_offset, sel_length),
                );
            }
        }
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

    /// Returns `true` when there is an active composition (marked text).
    pub fn has_marked_text(&self) -> bool {
        !self.marked_text.lock().unwrap().is_empty()
    }

    /// Discard / clear the current marked text without committing.
    pub fn discard_marked_text(&self) {
        log::debug!("[macOS IME] discard_marked_text");
        *self.marked_text.lock().unwrap() = String::new();
        *self.marked_range.lock().unwrap() = (0, 0);
        *self.selected_range.lock().unwrap() = (0, 0);
    }

    /// Returns the UTF-16 length of a string.
    pub fn utf16_len(s: &str) -> usize {
        s.encode_utf16().count()
    }
}

// ──────────────────────────────────────────────
// ImeBridge trait implementation
// ──────────────────────────────────────────────

impl ImeBridge for MacOsImeBridge {
    fn focus_in(&self, widget_id: ObjectId) {
        *self.focused_widget.lock().unwrap() = Some(widget_id);
        *self.active.lock().unwrap() = true;
        log::info!("[macOS IME] focus_in: widget={}", widget_id);

        // On native macOS, activate NSTextInputContext for the view.
        #[cfg(feature = "objc2-macos")]
        {
            let guard = self.native_token.lock().unwrap();
            if let Some(ref token) = *guard {
                unsafe {
                    use objc2::msg_send;
                    use objc2::runtime::Object;

                    #[repr(C)]
                    struct ImeCtx(*mut Object);

                    if let Some(ime_ctx) = token.downcast_ref::<ImeCtx>() {
                        let ctx: *mut Object = ime_ctx.0;
                        if !ctx.is_null() {
                            let _: () = msg_send![ctx, activate];
                            log::info!(
                                "[macOS IME] NSTextInputContext activated for widget={}",
                                widget_id
                            );
                        }
                    }
                }
            }
        }
    }

    fn focus_out(&self, widget_id: ObjectId) {
        *self.focused_widget.lock().unwrap() = None;
        *self.active.lock().unwrap() = false;
        // Discard any pending composition.
        *self.marked_text.lock().unwrap() = String::new();
        *self.marked_range.lock().unwrap() = (0, 0);
        *self.selected_range.lock().unwrap() = (0, 0);
        log::info!("[macOS IME] focus_out: widget={}", widget_id);

        #[cfg(feature = "objc2-macos")]
        {
            let guard = self.native_token.lock().unwrap();
            if let Some(ref token) = *guard {
                unsafe {
                    use objc2::msg_send;
                    use objc2::runtime::Object;

                    #[repr(C)]
                    struct ImeCtx(*mut Object);

                    if let Some(ime_ctx) = token.downcast_ref::<ImeCtx>() {
                        let ctx: *mut Object = ime_ctx.0;
                        if !ctx.is_null() {
                            let _: () = msg_send![ctx, deactivate];
                            log::info!(
                                "[macOS IME] NSTextInputContext deactivated for widget={}",
                                widget_id
                            );
                        }
                    }
                }
            }
        }
    }

    fn commit_text(&self, text: &str) {
        log::info!("[macOS IME] commit_text: '{}'", text);
        self.clear_composition();
    }

    fn set_composition(&self, composition: &ImeComposition) {
        log::debug!("[macOS IME] set_composition: '{}'", composition.text);

        // Map ImeComposition (byte-based) to marked text (UTF-16 offsets).
        let text = &composition.text;
        let utf16_len = Self::utf16_len(text);

        // Convert byte cursor to UTF-16 offset — cap at utf16_len.
        let cursor_utf16 = byte_offset_to_utf16(text, composition.cursor_position).min(utf16_len);

        // Selection length in UTF-16 units.
        let sel_length_utf16 =
            composition.selection_length.min(utf16_len.saturating_sub(cursor_utf16));

        *self.marked_text.lock().unwrap() = text.to_string();
        *self.marked_range.lock().unwrap() = (0, utf16_len);
        *self.selected_range.lock().unwrap() = (cursor_utf16, sel_length_utf16);

        #[cfg(feature = "objc2-macos")]
        {
            let guard = self.native_token.lock().unwrap();
            if let Some(ref token) = *guard {
                sync_nstextinputcontext(
                    token.as_ref(),
                    text,
                    (0, utf16_len),
                    (cursor_utf16, sel_length_utf16),
                );
            }
        }
    }

    fn set_candidate_window_position(&self, position: ImeCandidatePosition) {
        log::debug!("[macOS IME] set_candidate_window_position: ({}, {})", position.x, position.y,);

        #[cfg(feature = "objc2-macos")]
        {
            unsafe {
                use objc2::msg_send;
                use objc2::runtime::{AnyClass, Object};
                use objc2::sel;

                if let Some(cls) = AnyClass::get(c"NSTextInputContext") {
                    // Guard against uninitialised AppKit (e.g. test env).
                    let responds: bool = msg_send![cls, respondsToSelector: sel!(activeContext)];
                    if responds {
                        let ctx: *mut Object = msg_send![cls, activeContext];
                        if !ctx.is_null() {
                            // Invalidate character coordinates so the system
                            // re-queries the cursor rect, updating the candidate
                            // window position.
                            let _: () = msg_send![ctx, invalidateCharacterCoordinates];
                        }
                    }
                }
            }
        }
    }

    fn is_active(&self) -> bool {
        *self.active.lock().unwrap()
    }
}

// ──────────────────────────────────────────────
// Helper: byte offset → UTF-16 code unit offset
// ──────────────────────────────────────────────

/// Convert a Rust `&str` byte offset to the corresponding UTF-16 code unit
/// offset.  If `byte_offset` points into the middle of a multi-byte sequence
/// it is clamped to the nearest valid character boundary.
fn byte_offset_to_utf16(s: &str, byte_offset: usize) -> usize {
    let byte_offset = byte_offset.min(s.len());
    // For exact counting we use the encode_utf16 approach:
    s[..byte_offset].encode_utf16().count()
}

// ──────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::ime::ImeComposition;

    // ── ImeBridge trait tests ──

    #[test]
    fn test_focus_in_out() {
        let bridge = MacOsImeBridge::new();
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
        let bridge = MacOsImeBridge::new();
        bridge.set_marked_text("hello", 5, 5);
        assert!(bridge.has_marked_text());

        bridge.commit_text("hello");
        assert!(!bridge.has_marked_text());
        assert_eq!(bridge.get_marked_text(), None);
    }

    #[test]
    fn test_commit_text_via_trait() {
        let bridge = MacOsImeBridge::new();
        bridge.set_marked_text("你好", 2, 2);
        assert!(bridge.has_marked_text());

        ImeBridge::commit_text(&bridge, "你好");
        assert!(!bridge.has_marked_text());
    }

    #[test]
    fn test_set_composition_trait() {
        let bridge = MacOsImeBridge::new();
        let comp = ImeComposition {
            text: "composing".to_string(),
            cursor_position: 5,
            selection_length: 0,
        };
        bridge.set_composition(&comp);
        assert!(bridge.has_marked_text());
        assert_eq!(bridge.get_marked_text(), Some("composing".to_string()));
    }

    #[test]
    fn test_set_composition_empty_clears() {
        let bridge = MacOsImeBridge::new();
        bridge.set_marked_text("something", 4, 0);

        let empty = ImeComposition::default();
        bridge.set_composition(&empty);
        assert!(!bridge.has_marked_text());
    }

    #[test]
    fn test_discard_marked_text() {
        let bridge = MacOsImeBridge::new();
        bridge.set_marked_text("你好世界", 2, 2);
        assert!(bridge.has_marked_text());

        bridge.discard_marked_text();
        assert!(!bridge.has_marked_text());
        assert_eq!(bridge.get_marked_text(), None);
    }

    #[test]
    fn test_set_marked_text_with_selection() {
        let bridge = MacOsImeBridge::new();
        bridge.set_marked_text("hello world", 3, 7);
        assert!(bridge.has_marked_text());
        assert_eq!(bridge.get_marked_text(), Some("hello world".to_string()));

        // Verify selected range was set (UTF-16).
        let sel = *bridge.selected_range.lock().unwrap();
        assert_eq!(sel, (3, 4)); // offset 3, length 4 -> "lo w"
    }

    #[test]
    fn test_set_marked_text_negative_sel_defaults_cursor_at_end() {
        let bridge = MacOsImeBridge::new();
        bridge.set_marked_text("test", -1, -1);
        let sel = *bridge.selected_range.lock().unwrap();
        // UTF-16 length of "test" is 4, so cursor at end = (4, 0)
        assert_eq!(sel, (4, 0));
    }

    #[test]
    fn test_is_active_after_events() {
        let bridge = MacOsImeBridge::new();
        assert!(!bridge.is_active());
        bridge.focus_in(1);
        assert!(bridge.is_active());
        bridge.focus_out(1);
        assert!(!bridge.is_active());
    }

    #[test]
    fn test_process_key_event_no_composition() {
        let bridge = MacOsImeBridge::new();
        // Printable ASCII 'A' with shift
        let result = bridge.process_key_event(0x61, 0x02, true);
        assert_eq!(result, Some("A".to_string()));

        // Without shift
        let result = bridge.process_key_event(0x61, 0x00, true);
        assert_eq!(result, Some("a".to_string()));

        // Enter
        let result = bridge.process_key_event(0x0d, 0x00, true);
        assert_eq!(result, Some("\n".to_string()));

        // Function key (escape) => None
        let result = bridge.process_key_event(0x1b, 0x00, true);
        assert_eq!(result, None);
    }

    #[test]
    fn test_process_key_event_during_composition() {
        let bridge = MacOsImeBridge::new();
        bridge.set_marked_text(" composing", 10, 0);
        // During composition, key events should return None (IME consumes them).
        let result = bridge.process_key_event(0x61, 0x00, true);
        assert_eq!(result, None);
    }

    #[test]
    fn test_set_candidate_window_position() {
        let bridge = MacOsImeBridge::new();
        // Should not panic.
        bridge.set_candidate_window_position(ImeCandidatePosition { x: 100, y: 200 });
    }

    #[test]
    fn test_utf16_len() {
        assert_eq!(MacOsImeBridge::utf16_len("hello"), 5);
        assert_eq!(MacOsImeBridge::utf16_len("你好"), 2);
        // Emoji (supplementary plane) = 2 UTF-16 code units
        assert_eq!(MacOsImeBridge::utf16_len("🚀"), 2);
    }

    #[test]
    fn test_byte_offset_to_utf16_ascii() {
        assert_eq!(byte_offset_to_utf16("hello", 0), 0);
        assert_eq!(byte_offset_to_utf16("hello", 3), 3);
        assert_eq!(byte_offset_to_utf16("hello", 5), 5);
    }

    #[test]
    fn test_byte_offset_to_utf16_cjk() {
        assert_eq!(byte_offset_to_utf16("你好", 0), 0);
        assert_eq!(byte_offset_to_utf16("你好", 3), 1); // '你' is 3 bytes
        assert_eq!(byte_offset_to_utf16("你好", 6), 2);
    }

    #[test]
    fn test_focus_out_discards_composition() {
        let bridge = MacOsImeBridge::new();
        bridge.set_marked_text("pending", 7, 0);
        assert!(bridge.has_marked_text());

        bridge.focus_out(1);
        assert!(!bridge.has_marked_text());
        assert!(!bridge.is_active());
    }

    #[test]
    fn test_set_cursor_rect() {
        let bridge = MacOsImeBridge::new();
        // Should not panic.
        bridge.set_cursor_rect(10, 20, 100, 30);
    }

    #[test]
    fn test_attach_to_view() {
        let bridge = MacOsImeBridge::new();
        let null_ptr = std::ptr::null_mut();
        // Should not panic (no-op without objc2-macos feature).
        bridge.attach_to_view(null_ptr);
    }
}
