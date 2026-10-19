// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Wayland `text-input-v3` IME bridge (BLUE28 B28.12).
//!
//! The compositor's input method owns the composition: a client enables the text-input object for
//! the focused surface, tells it the cursor rectangle, and receives `preedit_string`,
//! `commit_string` and `delete_surrounding_text` events. That direction is the reason this bridge
//! is the inverse of the desktop state machines: [`ImeBridge::set_composition`] cannot push a
//! preedit here (the protocol has no such request), so the composition is delivered when the
//! compositor sends it.
//!
//! # Availability
//!
//! `zwp_text_input_manager_v3` is an unstable protocol. The backend binds it when the compositor
//! advertises it and answers `capabilities().ime == false` when it does not; a compositor without
//! it must not look like one with a working input method. The `unstable` feature of
//! `wayland-protocols` is what generates the bindings.

#![cfg(all(feature = "wayland-native", unix_desktop))]

use crate::compat::{lock, Arc, Mutex, ToString};
use crate::core::ObjectId;
use crate::platform::ime::{ImeBridge, ImeCandidatePosition, ImeComposition};
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::{Connection, QueueHandle};
use wayland_protocols::wp::text_input::zv3::client::zwp_text_input_manager_v3::ZwpTextInputManagerV3;
use wayland_protocols::wp::text_input::zv3::client::zwp_text_input_v3::{
    ContentHint, ContentPurpose, ZwpTextInputV3,
};

/// The IME state shared by the platform and the protocol dispatch.
#[derive(Clone)]
pub(crate) struct WaylandIme {
    inner: Arc<Mutex<Inner>>,
}

struct Channel {
    #[allow(dead_code)]
    manager: ZwpTextInputManagerV3,
    text_input: ZwpTextInputV3,
    conn: Connection,
}

struct Inner {
    channel: Option<Channel>,
    focused_widget: Option<ObjectId>,
    candidate: ImeCandidatePosition,
    /// Set by `enter` and cleared by `leave`: the compositor is routing input to our surface.
    active: bool,
}

impl WaylandIme {
    /// Creates an inactive bridge with no protocol channel.
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                channel: None,
                focused_widget: None,
                candidate: ImeCandidatePosition { x: 0, y: 0 },
                active: false,
            })),
        }
    }

    /// Drops protocol objects from a session that is going away.
    pub(crate) fn clear_channel(&self) {
        let mut inner = lock(&self.inner);
        inner.channel = None;
        inner.active = false;
    }

    /// Records the `zwp_text_input_v3` created for the session's seat.
    pub(crate) fn attach_channel(
        &self,
        manager: ZwpTextInputManagerV3,
        text_input: ZwpTextInputV3,
        conn: Connection,
    ) {
        let mut inner = lock(&self.inner);
        inner.channel = Some(Channel { manager, text_input, conn });
        inner.active = false;
    }

    /// Whether the compositor advertised `text-input-v3`, which is what `capabilities().ime`
    /// promises.
    pub(crate) fn available(&self) -> bool {
        lock(&self.inner).channel.is_some()
    }

    /// Records the compositor's `enter`/`leave` for a surface.
    pub(crate) fn set_active(&self, active: bool) {
        lock(&self.inner).active = active;
    }

    /// A `preedit_string` event arrived: deliver it to the focused widget.
    pub(crate) fn preedit(&self, text: &str, cursor_begin: i32, cursor_end: i32) {
        let widget = lock(&self.inner).focused_widget;
        let Some(widget) = widget else { return };
        let cursor = if cursor_begin < 0 { text.len() } else { utf16_to_byte(text, cursor_begin) };
        let end = if cursor_end < 0 { cursor } else { utf16_to_byte(text, cursor_end) };
        let composition = ImeComposition {
            text: text.to_string(),
            cursor_position: cursor,
            selection_length: end.saturating_sub(cursor),
        };
        let _ = crate::platform::ime::deliver_composition(widget, &composition);
    }

    /// A `commit_string` event arrived: deliver it to the focused widget.
    pub(crate) fn commit(&self, text: &str) {
        let widget = lock(&self.inner).focused_widget;
        let Some(widget) = widget else { return };
        let _ = crate::platform::ime::deliver_commit(widget, text);
    }
}

/// Converts a UTF-16 code-unit offset to the nearest Rust byte offset, clamping into the string.
fn utf16_to_byte(text: &str, offset: i32) -> usize {
    let offset = offset.max(0) as usize;
    let mut units = 0usize;
    for (byte_index, ch) in text.char_indices() {
        if units >= offset {
            return byte_index;
        }
        let next = units + ch.len_utf16();
        if next > offset {
            return byte_index;
        }
        units = next;
    }
    text.len()
}

impl Inner {
    /// Sends `enable`/`disable` plus the cursor rectangle, then flushes.
    fn apply(&mut self, enabled: bool) {
        let Some(channel) = self.channel.as_ref() else { return };
        if enabled {
            channel.text_input.enable();
            channel.text_input.set_content_type(ContentHint::None, ContentPurpose::Normal);
            channel.text_input.set_cursor_rectangle(self.candidate.x, self.candidate.y, 1, 1);
        } else {
            channel.text_input.disable();
        }
        channel.text_input.commit();
        if let Err(e) = channel.conn.flush() {
            log::warn!("[wayland] text-input flush failed: {e}");
        }
    }
}

impl ImeBridge for WaylandIme {
    fn focus_in(&self, widget_id: ObjectId) {
        let mut inner = lock(&self.inner);
        inner.focused_widget = Some(widget_id);
        inner.apply(true);
    }

    fn focus_out(&self, _widget_id: ObjectId) {
        let mut inner = lock(&self.inner);
        inner.focused_widget = None;
        inner.active = false;
        inner.apply(false);
    }

    /// Delivers a host-supplied commit to the focused widget.
    ///
    /// `wp_text_input_v3` has no client-to-IME commit request, so this is the only honest meaning
    /// of the call: the text goes to the control, not to the compositor.
    fn commit_text(&self, text: &str) {
        let widget = lock(&self.inner).focused_widget;
        if let Some(widget) = widget {
            let _ = crate::platform::ime::deliver_commit(widget, text);
        }
    }

    /// The compositor owns the preedit, so this records nothing on the wire.
    ///
    /// A composition produced by the widget layer (rather than by the input method) has no
    /// protocol representation; reporting it as sent would be false. The method is still
    /// meaningful to the widget layer, which may use it to render a preview locally.
    fn set_composition(&self, _composition: &ImeComposition) {}

    fn set_candidate_window_position(&self, position: ImeCandidatePosition) {
        let mut inner = lock(&self.inner);
        inner.candidate = position;
        let enabled = inner.focused_widget.is_some();
        if inner.channel.is_some() {
            inner.apply(enabled);
        }
    }

    fn is_active(&self) -> bool {
        lock(&self.inner).active
    }
}

/// Creates the text-input object once the manager and seat are both known.
pub(crate) fn create_text_input(
    ime: &WaylandIme,
    manager: &ZwpTextInputManagerV3,
    seat: &WlSeat,
    qh: &QueueHandle<super::platform_impl::WaylandSessionState>,
    conn: &Connection,
) -> ZwpTextInputV3 {
    let text_input = manager.get_text_input(seat, qh, ());
    ime.attach_channel(manager.clone(), text_input.clone(), conn.clone());
    text_input
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_offsets_map_to_byte_offsets() {
        assert_eq!(utf16_to_byte("abc", 0), 0);
        assert_eq!(utf16_to_byte("abc", 2), 2);
        assert_eq!(utf16_to_byte("abc", 99), 3);
        // A CJK character is one UTF-16 unit but three bytes.
        assert_eq!(utf16_to_byte("你好", 1), 3);
        // An emoji is two UTF-16 units; an offset inside it clamps to its start.
        assert_eq!(utf16_to_byte("😀x", 1), 0);
        assert_eq!(utf16_to_byte("😀x", 2), 4);
    }

    #[test]
    fn an_unbound_bridge_reports_no_channel() {
        let ime = WaylandIme::new();
        assert!(!ime.available());
        assert!(!ime.is_active());
        // Both are no-ops without a focused widget.
        ime.preedit("あ", 0, 0);
        ime.commit("あ");
    }
}
